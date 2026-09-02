//! Persistência abstrata: o núcleo só sabe ler e escrever blobs por chave.
//!
//! Cada frontend traz a implementação (arquivos, `localStorage`, pasta interna do Android).
//! Chaves são caminhos relativos com `/` (`sav/<hash>.sav`, `state/<hash>/1.rnfs`).

use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageError(pub String);

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for StorageError {}

pub trait Storage {
    /// `None` se a chave não existe (ou não pôde ser lida).
    fn read(&self, key: &str) -> Option<Vec<u8>>;
    fn write(&mut self, key: &str, data: &[u8]) -> Result<(), StorageError>;
    fn remove(&mut self, key: &str) -> Result<(), StorageError>;
}

/// Armazenamento em memória: testes e fallback quando não há onde gravar.
#[derive(Default, Debug)]
pub struct MemoryStorage {
    items: HashMap<String, Vec<u8>>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl Storage for MemoryStorage {
    fn read(&self, key: &str) -> Option<Vec<u8>> {
        self.items.get(key).cloned()
    }

    fn write(&mut self, key: &str, data: &[u8]) -> Result<(), StorageError> {
        self.items.insert(key.to_string(), data.to_vec());
        Ok(())
    }

    fn remove(&mut self, key: &str) -> Result<(), StorageError> {
        self.items.remove(key);
        Ok(())
    }
}

/// Uma chave é válida se for um caminho relativo simples: sem `..`, sem começar com `/`,
/// só ASCII imprimível. Implementações de arquivo devem recusar o resto.
pub fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && !key.starts_with('/')
        && !key.contains('\\')
        && key.split('/').all(|seg| !seg.is_empty() && seg != "." && seg != "..")
        && key.bytes().all(|b| (0x21..0x7F).contains(&b))
}
