//! Grava a PRG RAM com bateria (`.sav`) no [`Storage`] do frontend.
//!
//! Chame [`SaveManager::load`] logo depois de criar o console, [`SaveManager::tick`] uma vez por
//! frame e [`SaveManager::flush`] ao trocar de ROM ou sair. Sem bateria, tudo é no-op.

use rnfe_core::Nes;
use rnfe_core::storage::{Storage, StorageError};

pub struct SaveManager {
    key: Option<String>,
    dirty: bool,
    frames_since_flush: u32,
}

impl SaveManager {
    /// Frames entre gravações automáticas (~5 s).
    pub const FLUSH_INTERVAL: u32 = 300;

    /// Chave do `.sav` deste cartucho (só se tem bateria).
    pub fn key_for(nes: &Nes) -> Option<String> {
        let cart = nes.cartridge();
        cart.has_battery().then(|| format!("sav/{:016x}.sav", cart.rom_hash()))
    }

    pub fn new(nes: &Nes) -> Self {
        SaveManager { key: Self::key_for(nes), dirty: false, frames_since_flush: 0 }
    }

    /// Sem cartucho ainda.
    pub fn none() -> Self {
        SaveManager { key: None, dirty: false, frames_since_flush: 0 }
    }

    pub fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }

    /// Copia o `.sav` (se existir) para a PRG RAM. `true` se carregou.
    pub fn load(&mut self, nes: &mut Nes, storage: &dyn Storage) -> bool {
        let Some(key) = &self.key else { return false };
        let Some(data) = storage.read(key) else { return false };
        let ram = nes.cartridge_mut().prg_ram_mut();
        let n = ram.len().min(data.len());
        ram[..n].copy_from_slice(&data[..n]);
        nes.cartridge_mut().take_prg_ram_dirty();
        self.dirty = false;
        true
    }

    /// Uma vez por frame: grava a cada `FLUSH_INTERVAL` frames se a RAM mudou.
    pub fn tick(&mut self, nes: &mut Nes, storage: &mut dyn Storage) -> Result<bool, StorageError> {
        if self.key.is_none() {
            return Ok(false);
        }
        if nes.cartridge_mut().take_prg_ram_dirty() {
            self.dirty = true;
        }
        self.frames_since_flush += 1;
        if self.dirty && self.frames_since_flush >= Self::FLUSH_INTERVAL {
            self.flush(nes, storage)
        } else {
            Ok(false)
        }
    }

    /// Grava agora se houver mudança pendente. `true` se gravou.
    pub fn flush(&mut self, nes: &mut Nes, storage: &mut dyn Storage) -> Result<bool, StorageError> {
        let Some(key) = &self.key else { return Ok(false) };
        if nes.cartridge_mut().take_prg_ram_dirty() {
            self.dirty = true;
        }
        self.frames_since_flush = 0;
        if !self.dirty {
            return Ok(false);
        }
        storage.write(key, nes.cartridge().prg_ram())?;
        self.dirty = false;
        Ok(true)
    }
}
