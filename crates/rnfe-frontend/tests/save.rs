use rnfe_core::storage::{MemoryStorage, Storage};
use rnfe_core::{Cartridge, Nes};
use rnfe_frontend::{FsStorage, SaveManager};

fn nes(battery: bool) -> Nes {
    let mut v = b"NES\x1A".to_vec();
    v.extend_from_slice(&[1, 0, if battery { 0x02 } else { 0 }, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    let mut prg = vec![0xEAu8; 16384];
    prg[0x3FFC] = 0x00;
    prg[0x3FFD] = 0x80;
    v.extend_from_slice(&prg);
    Nes::new(Cartridge::from_bytes(&v).unwrap())
}

#[test]
fn fs_storage_roundtrip_and_key_validation() {
    let dir = std::env::temp_dir().join(format!("rnfe-test-{}", std::process::id()));
    let mut st = FsStorage::new(&dir);
    assert_eq!(st.read("sav/x.sav"), None);
    st.write("sav/x.sav", b"hello").unwrap();
    assert_eq!(st.read("sav/x.sav").as_deref(), Some(&b"hello"[..]));
    assert!(!dir.join("sav/x.tmp").exists(), "tmp renomeado");
    st.write("sav/x.sav", b"bye").unwrap();
    assert_eq!(st.read("sav/x.sav").as_deref(), Some(&b"bye"[..]));
    assert!(st.write("../fora", b"x").is_err());
    assert!(st.write("/abs", b"x").is_err());
    assert!(st.write("a/../b", b"x").is_err());
    st.remove("sav/x.sav").unwrap();
    st.remove("sav/x.sav").unwrap();
    assert_eq!(st.read("sav/x.sav"), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_manager_flushes_every_300_frames_when_dirty() {
    let mut n = nes(true);
    let mut st = MemoryStorage::new();
    let mut sm = SaveManager::new(&n);
    let key = sm.key().unwrap().to_string();
    assert!(key.starts_with("sav/") && key.ends_with(".sav"));
    assert!(!sm.load(&mut n, &st), "nada salvo ainda");
    for _ in 0..1000 {
        assert!(!sm.tick(&mut n, &mut st).unwrap(), "sem escrita, nunca grava");
    }
    n.cartridge_mut().cpu_write(0x6123, 0xAB);
    assert!(sm.tick(&mut n, &mut st).unwrap(), "já passaram 300 frames desde a última gravação");
    assert_eq!(st.read(&key).unwrap()[0x123], 0xAB);
    n.cartridge_mut().cpu_write(0x6123, 0xAC);
    for _ in 0..299 {
        assert!(!sm.tick(&mut n, &mut st).unwrap(), "no máximo uma gravação a cada 300 frames");
    }
    assert!(sm.tick(&mut n, &mut st).unwrap(), "300º frame grava");
    assert_eq!(st.read(&key).unwrap()[0x123], 0xAC);
    assert!(!sm.flush(&mut n, &mut st).unwrap(), "flush sem mudança não grava");
    n.cartridge_mut().cpu_write(0x6124, 0xCD);
    assert!(sm.flush(&mut n, &mut st).unwrap());

    // outro console com a mesma ROM carrega o save
    let mut n2 = nes(true);
    let mut sm2 = SaveManager::new(&n2);
    assert_eq!(sm2.key(), Some(key.as_str()));
    assert!(sm2.load(&mut n2, &st));
    assert_eq!(n2.cartridge().cpu_read(0x6123), Some(0xAC));
    assert_eq!(n2.cartridge().cpu_read(0x6124), Some(0xCD));
    assert!(!sm2.flush(&mut n2, &mut st).unwrap(), "carregar não suja");
}

#[test]
fn save_manager_without_battery_is_noop() {
    let mut n = nes(false);
    let mut st = MemoryStorage::new();
    let mut sm = SaveManager::new(&n);
    assert_eq!(sm.key(), None);
    n.cartridge_mut().cpu_write(0x6000, 1);
    for _ in 0..600 {
        assert!(!sm.tick(&mut n, &mut st).unwrap());
    }
    assert!(!sm.flush(&mut n, &mut st).unwrap());
    assert!(st.is_empty());
}
