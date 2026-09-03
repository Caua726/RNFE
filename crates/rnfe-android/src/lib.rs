//! Entrada nativa do app Android: `android_main` (winit + android-activity) e a ponte JNI com
//! a `MainActivity` Java, que abre o seletor de arquivos do sistema (SAF) e devolve os bytes.
//!
//! Fluxo da ROM: `Launch::picker` guarda o `EventLoopProxy` e chama `MainActivity.pickRom()`;
//! o Java lê o arquivo escolhido e chama `onRomPicked(bytes, nome)`, que vira
//! `UserEvent::RomLoaded` no laço do frontend.
#![cfg(target_os = "android")]

use jni::JNIEnv;
use jni::objects::{JByteArray, JObject, JString};
use rnfe_frontend::FsStorage;
use rnfe_gui::{Launch, UserEvent};
use std::sync::Mutex;
use winit::event_loop::EventLoopProxy;
use winit::platform::android::activity::AndroidApp;

static PROXY: Mutex<Option<EventLoopProxy<UserEvent>>> = Mutex::new(None);
/// ROM escolhida antes de o laço existir (processo recriado pelo sistema durante o SAF).
static PENDING: Mutex<Option<(String, Vec<u8>)>> = Mutex::new(None);

/// Chama um método `void` sem argumentos da `MainActivity` pela JNI.
fn call_activity(app: &AndroidApp, method: &str) -> Result<(), String> {
    // SAFETY: os ponteiros vêm do android-activity e são a JavaVM e a Activity vivas deste app.
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM) }
        .map_err(|e| e.to_string())?;
    let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
    if let Err(e) = env.call_method(&activity, method, "()V", &[]) {
        // exceção Java pendente derrubaria a próxima chamada JNI
        let _ = env.exception_describe();
        let _ = env.exception_clear();
        return Err(e.to_string());
    }
    Ok(())
}

fn call_pick_rom(app: &AndroidApp) -> Result<(), String> {
    call_activity(app, "pickRom")
}

/// Chamado pelo Java com o conteúdo da ROM escolhida (ou vazio se cancelou).
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_caua726_rnfe_MainActivity_onRomPicked<'l>(
    mut env: JNIEnv<'l>,
    _this: JObject<'l>,
    data: JByteArray<'l>,
    name: JString<'l>,
) {
    let bytes = env.convert_byte_array(&data).unwrap_or_default();
    let name: String = env.get_string(&name).map(|s| s.into()).unwrap_or_default();
    let ev = if bytes.is_empty() {
        UserEvent::RomLoadFailed("cancelado".into())
    } else {
        UserEvent::RomLoaded { name, bytes }
    };
    match PROXY.lock().ok().and_then(|g| g.clone()) {
        Some(p) => {
            let _ = p.send_event(ev);
        }
        None => {
            if let UserEvent::RomLoaded { name, bytes } = ev {
                if let Ok(mut g) = PENDING.lock() {
                    *g = Some((name, bytes));
                }
            }
        }
    }
}

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("rnfe"),
    );
    let data_dir =
        app.internal_data_path().unwrap_or_else(|| std::path::PathBuf::from("/data/local/tmp")).join("rnfe");
    log::info!("dados em {}", data_dir.display());
    let picker_app = app.clone();
    let mut launch = Launch::new(Box::new(FsStorage::new(data_dir)));
    launch.picker = Some(Box::new(move |proxy| {
        if let Err(e) = call_pick_rom(&picker_app) {
            log::error!("pickRom: {e}");
            let _ = proxy.send_event(UserEvent::RomLoadFailed(e));
        }
    }));
    let haptic_app = app.clone();
    launch.haptic = Some(Box::new(move || {
        if let Err(e) = call_activity(&haptic_app, "vibrate") {
            log::debug!("vibrate: {e}");
        }
    }));
    let on_proxy = |proxy: EventLoopProxy<UserEvent>| {
        if let Ok(mut g) = PROXY.lock() {
            *g = Some(proxy.clone());
        }
        if let Some((name, bytes)) = PENDING.lock().ok().and_then(|mut g| g.take()) {
            let _ = proxy.send_event(UserEvent::RomLoaded { name, bytes });
        }
    };
    if let Err(e) = rnfe_gui::run_android(app, launch, on_proxy) {
        log::error!("laço de eventos: {e}");
    }
}
