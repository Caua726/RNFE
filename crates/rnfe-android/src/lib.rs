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
static PENDING: Mutex<Option<UserEvent>> = Mutex::new(None);

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

/// Chama um método `void(boolean)` da `MainActivity`.
fn call_activity_bool(app: &AndroidApp, method: &str, on: bool) -> Result<(), String> {
    // SAFETY: idem `call_activity`.
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM) }
        .map_err(|e| e.to_string())?;
    let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
    let arg = jni::objects::JValue::Bool(u8::from(on));
    if let Err(e) = env.call_method(&activity, method, "(Z)V", &[arg]) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
        return Err(e.to_string());
    }
    Ok(())
}

/// Chama um método `void(String)` da `MainActivity`.
fn call_activity_str(app: &AndroidApp, method: &str, arg: &str) -> Result<(), String> {
    // SAFETY: idem `call_activity`.
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM) }
        .map_err(|e| e.to_string())?;
    let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
    let s = env.new_string(arg).map_err(|e| e.to_string())?;
    let val = jni::objects::JValue::Object(&s);
    if let Err(e) = env.call_method(&activity, method, "(Ljava/lang/String;)V", &[val]) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
        return Err(e.to_string());
    }
    Ok(())
}

fn call_pick_rom(app: &AndroidApp) -> Result<(), String> {
    call_activity(app, "pickRom")
}

/// Entrega um evento ao laço do winit; antes de o laço existir fica guardado em `PENDING`.
/// Depois de guardar, o proxy é conferido de novo: `on_proxy` pode ter passado no meio
/// (arranque frio por "Abrir com") e já ter drenado um `PENDING` vazio.
fn deliver(ev: UserEvent) {
    if let Some(p) = PROXY.lock().ok().and_then(|g| g.clone()) {
        let _ = p.send_event(ev);
        return;
    }
    if let Ok(mut g) = PENDING.lock() {
        *g = Some(ev);
    }
    if let Some(p) = PROXY.lock().ok().and_then(|g| g.clone()) {
        if let Some(ev) = PENDING.lock().ok().and_then(|mut g| g.take()) {
            let _ = p.send_event(ev);
        }
    }
}

/// Chamado pelo Java com o conteúdo da ROM escolhida (vazio = cancelou).
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_caua726_rnfe_MainActivity_onRomPicked<'l>(
    mut env: JNIEnv<'l>,
    _this: JObject<'l>,
    data: JByteArray<'l>,
    name: JString<'l>,
) {
    let bytes = env.convert_byte_array(&data).unwrap_or_default();
    let name: String = env.get_string(&name).map(|s| s.into()).unwrap_or_default();
    // "vazio" já significou "cancelou": um .nes legítimo de 0 byte sumia sem aviso nenhum.
    // Cancelar agora tem função própria (`onRomCancelled`).
    deliver(if bytes.is_empty() {
        UserEvent::RomLoadFailed(if name.is_empty() {
            "o arquivo escolhido está vazio".into()
        } else {
            format!("{name} está vazio")
        })
    } else {
        UserEvent::RomLoaded { name, bytes }
    });
}

/// Chamado pelo Java quando o usuário fechou o seletor sem escolher nada.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_caua726_rnfe_MainActivity_onRomCancelled<'l>(
    _env: JNIEnv<'l>,
    _this: JObject<'l>,
) {
    deliver(UserEvent::RomLoadFailed("cancelado".into()));
}

/// Chamado pelo Java quando a ROM não pôde ser lida (motivo legível para o aviso).
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_caua726_rnfe_MainActivity_onRomFailed<'l>(
    mut env: JNIEnv<'l>,
    _this: JObject<'l>,
    why: JString<'l>,
) {
    let why: String = env.get_string(&why).map(|s| s.into()).unwrap_or_default();
    deliver(UserEvent::RomLoadFailed(if why.is_empty() { "não consegui ler a ROM".into() } else { why }));
}

/// Eixos do gamepad vindos do Java (d-pad como hat ou analógico esquerdo).
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_caua726_rnfe_MainActivity_onPadAxes<'l>(
    _env: JNIEnv<'l>,
    _this: JObject<'l>,
    x: f32,
    y: f32,
) {
    if let Some(p) = PROXY.lock().ok().and_then(|g| g.clone()) {
        let _ = p.send_event(UserEvent::PadAxes { x, y });
    }
}

/// Chama `MainActivity.setGestureExclusion(l1,t1,r1,b1,l2,t2,r2,b2)`.
fn call_gesture_exclusion(app: &AndroidApp, rects: [[i32; 4]; 2]) -> Result<(), String> {
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM) }
        .map_err(|e| e.to_string())?;
    let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
    let args: Vec<jni::objects::JValue> =
        rects.iter().flatten().map(|&v| jni::objects::JValue::Int(v)).collect();
    if let Err(e) = env.call_method(&activity, "setGestureExclusion", "(IIIIIIII)V", &args) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
        return Err(e.to_string());
    }
    Ok(())
}

/// Publica os itens do menu na `MainActivity` para o leitor de tela: um vetor de rótulos e um
/// de retângulos (l, t, r, b por item, achatados).
fn call_a11y(app: &AndroidApp, nodes: Vec<(String, [i32; 4])>) -> Result<(), String> {
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM) }
        .map_err(|e| e.to_string())?;
    let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
    // sem quadro local próprio, N referências de String por publicação vazam até o retorno
    env.push_local_frame(nodes.len() as i32 * 2 + 16).map_err(|e| e.to_string())?;
    let vazio = env.new_string("").map_err(|e| e.to_string())?;
    let labels =
        env.new_object_array(nodes.len() as i32, "java/lang/String", &vazio).map_err(|e| e.to_string())?;
    let mut rects: Vec<i32> = Vec::with_capacity(nodes.len() * 4);
    for (i, (label, r)) in nodes.iter().enumerate() {
        let s = env.new_string(label).map_err(|e| e.to_string())?;
        env.set_object_array_element(&labels, i as i32, &s).map_err(|e| e.to_string())?;
        rects.extend_from_slice(r);
    }
    let arr = env.new_int_array(rects.len() as i32).map_err(|e| e.to_string())?;
    env.set_int_array_region(&arr, 0, &rects).map_err(|e| e.to_string())?;
    let args = [jni::objects::JValue::Object(&labels), jni::objects::JValue::Object(&arr)];
    let r = env.call_method(&activity, "setA11yNodes", "([Ljava/lang/String;[I)V", &args);
    if let Err(e) = r {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
        // SAFETY: o quadro foi empilhado logo acima, nesta mesma thread anexada.
        let _ = unsafe { env.pop_local_frame(&JObject::null()) };
        return Err(e.to_string());
    }
    // SAFETY: idem.
    let _ = unsafe { env.pop_local_frame(&JObject::null()) };
    Ok(())
}

/// O leitor de tela ativou um item do menu (índice na lista publicada por `setA11yNodes`).
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_caua726_rnfe_MainActivity_onA11yClick<'l>(
    _env: JNIEnv<'l>,
    _this: JObject<'l>,
    index: i32,
) {
    if index >= 0 {
        deliver(UserEvent::A11yActivate(index as u32));
    }
}

/// Área segura da janela em px (recorte da câmera, barras, faixa de gesto), vinda do Java.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_caua726_rnfe_MainActivity_onInsets<'l>(
    _env: JNIEnv<'l>,
    _this: JObject<'l>,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
) {
    // Não passa por `PENDING`: ele tem uma vaga só, e um inset chegando no arranque frio
    // sobrescrevia a ROM que o "Abrir com" tinha acabado de guardar lá.
    if let Some(p) = PROXY.lock().ok().and_then(|g| g.clone()) {
        let _ = p.send_event(UserEvent::SafeInsets { left, top, right, bottom });
    }
}

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    // `panic = "abort"` (perfil release): sem hook, a mensagem do panic vai para o stderr, que
    // o Android descarta, e o logcat só mostra "Fatal signal 6 (SIGABRT)".
    std::panic::set_hook(Box::new(|info| {
        log::error!("panic: {info}");
    }));
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("rnfe"),
    );
    let data_dir =
        app.internal_data_path().unwrap_or_else(|| std::path::PathBuf::from("/data/local/tmp")).join("rnfe");
    log::info!("dados em {}", data_dir.display());
    let picker_app = app.clone();
    let mut launch = Launch::new(Box::new(FsStorage::new(data_dir.clone())));
    launch.data_dir = Some(data_dir.display().to_string());
    launch.picker = Some(Box::new(move |proxy| {
        if let Err(e) = call_pick_rom(&picker_app) {
            log::error!("pickRom: {e}");
            let _ = proxy.send_event(UserEvent::RomLoadFailed(e));
        }
    }));
    let gesture_app = app.clone();
    launch.gesture_exclusion = Some(Box::new(move |rects| {
        if let Err(e) = call_gesture_exclusion(&gesture_app, rects) {
            log::debug!("gesture exclusion: {e}");
        }
    }));
    let a11y_app = app.clone();
    launch.a11y = Some(Box::new(move |nodes| {
        if let Err(e) = call_a11y(&a11y_app, nodes) {
            log::debug!("a11y: {e}");
        }
    }));
    let notify_app = app.clone();
    launch.notify = Some(Box::new(move |msg: &str| {
        if let Err(e) = call_activity_str(&notify_app, "toast", msg) {
            log::debug!("toast: {e}");
        }
    }));
    let kso_app = app.clone();
    launch.keep_screen_on = Some(Box::new(move |on| {
        if let Err(e) = call_activity_bool(&kso_app, "setKeepScreenOn", on) {
            log::debug!("setKeepScreenOn: {e}");
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
        if let Some(ev) = PENDING.lock().ok().and_then(|mut g| g.take()) {
            let _ = proxy.send_event(ev);
        }
    };
    if let Err(e) = rnfe_gui::run_android(app, launch, on_proxy) {
        log::error!("laço de eventos: {e}");
    }
}
