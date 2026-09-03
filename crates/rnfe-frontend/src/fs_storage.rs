//! [`Storage`] em arquivos: `<dir>/<chave>`, escrita atômica (tmp + rename).

use rnfe_core::storage::{Storage, StorageError, valid_key};
use std::path::{Path, PathBuf};

pub struct FsStorage {
    dir: PathBuf,
}

impl FsStorage {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        FsStorage { dir: dir.into() }
    }

    /// Pasta padrão de dados: `$RNFE_DATA_DIR`, senão `$XDG_DATA_HOME/rnfe`, senão
    /// `~/.local/share/rnfe` (`%APPDATA%\rnfe` no Windows).
    pub fn default_dir() -> PathBuf {
        if let Some(d) = std::env::var_os("RNFE_DATA_DIR") {
            return PathBuf::from(d);
        }
        if let Some(d) = std::env::var_os("XDG_DATA_HOME") {
            return PathBuf::from(d).join("rnfe");
        }
        if let Some(d) = std::env::var_os("APPDATA") {
            return PathBuf::from(d).join("rnfe");
        }
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
        home.join(".local/share/rnfe")
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self, key: &str) -> Result<PathBuf, StorageError> {
        if !valid_key(key) {
            return Err(StorageError(format!("chave inválida: {key:?}")));
        }
        Ok(self.dir.join(key))
    }
}

impl Storage for FsStorage {
    fn read(&self, key: &str) -> Option<Vec<u8>> {
        std::fs::read(self.path(key).ok()?).ok()
    }

    fn write(&mut self, key: &str, data: &[u8]) -> Result<(), StorageError> {
        let path = self.path(key)?;
        let err = |e: std::io::Error| StorageError(format!("{}: {e}", path.display()));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(err)?;
        }
        let tmp = std::path::PathBuf::from(format!("{}.tmp", path.display()));
        std::fs::write(&tmp, data).map_err(err)?;
        std::fs::rename(&tmp, &path).map_err(err)
    }

    fn remove(&mut self, key: &str) -> Result<(), StorageError> {
        let path = self.path(key)?;
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(StorageError(format!("{}: {e}", path.display()))),
        }
    }
}
