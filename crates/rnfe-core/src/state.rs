//! Save states (feature `serde`): todo o estado do console em um blob `RNFS`.
//!
//! Formato: `b"RNFS"` · versão `u16 LE` · `rom_hash u64 LE` · payload postcard.
//! O payload é o estado de CPU, PPU, APU, bus, PRG RAM, CHR RAM e mapper — nunca a ROM.
//! O framebuffer e o buffer de áudio ficam de fora (o próximo frame recria os dois).

use crate::cartridge::Mirror;
use crate::mappers::MapperKind;
use crate::nes::Nes;
use core::fmt;
use serde::{Deserialize, Serialize};

pub const MAGIC: &[u8; 4] = b"RNFS";
pub const VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateError {
    /// Não começa com `RNFS`.
    BadMagic,
    /// Versão de formato que este binário não lê.
    Version(u16),
    /// O state é de outra ROM.
    RomMismatch { expected: u64, got: u64 },
    /// Payload truncado ou inconsistente com o cartucho.
    Corrupt(String),
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateError::BadMagic => write!(f, "não é um save state RNFS"),
            StateError::Version(v) => write!(f, "save state versão {v} (este RNFE lê a {VERSION})"),
            StateError::RomMismatch { expected, got } => {
                write!(f, "save state de outra ROM ({got:016x}; esta é {expected:016x})")
            }
            StateError::Corrupt(why) => write!(f, "save state corrompido: {why}"),
        }
    }
}

impl std::error::Error for StateError {}

/// Lê uma sequência de bytes de qualquer formato (bytes nativos ou seq de u8).
struct BytesVisitor;

impl<'de> serde::de::Visitor<'de> for BytesVisitor {
    type Value = Vec<u8>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bytes")
    }

    fn visit_bytes<E: serde::de::Error>(self, v: &[u8]) -> Result<Vec<u8>, E> {
        Ok(v.to_vec())
    }

    fn visit_byte_buf<E: serde::de::Error>(self, v: Vec<u8>) -> Result<Vec<u8>, E> {
        Ok(v)
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<u8>, A::Error> {
        let mut v = Vec::with_capacity(seq.size_hint().unwrap_or(0));
        while let Some(b) = seq.next_element::<u8>()? {
            v.push(b);
        }
        Ok(v)
    }
}

/// `[u8; N]` como bytes (serde só deriva arrays até 32).
pub mod bytes {
    use super::BytesVisitor;
    use serde::de::Error;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S: Serializer, const N: usize>(a: &[u8; N], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(a)
    }

    pub fn deserialize<'de, D: Deserializer<'de>, const N: usize>(d: D) -> Result<[u8; N], D::Error> {
        let v = d.deserialize_bytes(BytesVisitor)?;
        v.try_into().map_err(|v: Vec<u8>| D::Error::custom(format!("esperava {N} bytes, veio {}", v.len())))
    }
}

/// `[[u8; 1024]; 4]` (nametables) como 4096 bytes.
pub mod nt {
    use super::BytesVisitor;
    use serde::de::Error;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S: Serializer>(a: &[[u8; 1024]; 4], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(a.as_flattened())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[[u8; 1024]; 4], D::Error> {
        let v = d.deserialize_bytes(BytesVisitor)?;
        if v.len() != 4096 {
            return Err(D::Error::custom(format!("esperava 4096 bytes de nametable, veio {}", v.len())));
        }
        let mut out = [[0u8; 1024]; 4];
        for (i, chunk) in v.chunks_exact(1024).enumerate() {
            out[i].copy_from_slice(chunk);
        }
        Ok(out)
    }
}

/// Estado do bus fora de PPU/APU/cartucho.
#[derive(Serialize, Deserialize)]
pub struct BusState {
    #[serde(with = "bytes")]
    pub ram: [u8; 2048],
    pub cpu_cycles: u64,
    pub oam_dma_page: Option<u8>,
    pub open_bus: u8,
    pub controller: [u8; 2],
    pub controller_state: [u8; 2],
    pub controller_strobe: bool,
}

/// Estado do cartucho sem a ROM.
#[derive(Serialize, Deserialize)]
pub struct CartState {
    pub prg_ram: Vec<u8>,
    /// Só se o cartucho tem CHR RAM.
    pub chr_ram: Option<Vec<u8>>,
    pub mirror: Mirror,
    pub mapper: MapperKind,
}

#[derive(Serialize, Deserialize)]
struct State {
    cpu: crate::cpu6502::Cpu6502,
    ppu: crate::ppu::Ppu,
    apu: crate::apu::Apu,
    bus: BusState,
    cart: CartState,
}

impl Nes {
    /// Serializa o estado completo do console (a ROM fica de fora; o `rom_hash` vai no header).
    pub fn save_state(&self) -> Vec<u8> {
        let st = State {
            cpu: self.cpu.clone(),
            ppu: self.bus.ppu.clone_state(),
            apu: self.bus.apu.clone(),
            bus: self.bus.state(),
            cart: self.bus.cartridge.state(),
        };
        let mut out = Vec::with_capacity(16 * 1024);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&self.bus.cartridge.rom_hash().to_le_bytes());
        postcard::to_extend(&st, out).expect("serialização em memória não falha")
    }

    /// Restaura um estado gravado por `save_state` com a mesma ROM.
    pub fn load_state(&mut self, data: &[u8]) -> Result<(), StateError> {
        if data.len() < 14 || &data[0..4] != MAGIC {
            return Err(StateError::BadMagic);
        }
        let version = u16::from_le_bytes([data[4], data[5]]);
        if version != VERSION {
            return Err(StateError::Version(version));
        }
        let got = u64::from_le_bytes(data[6..14].try_into().unwrap());
        let expected = self.bus.cartridge.rom_hash();
        if got != expected {
            return Err(StateError::RomMismatch { expected, got });
        }
        let st: State = postcard::from_bytes(&data[14..]).map_err(|e| StateError::Corrupt(e.to_string()))?;
        self.bus.cartridge.restore(st.cart)?;
        self.cpu = st.cpu;
        self.bus.ppu = st.ppu;
        self.bus.apu = st.apu;
        self.bus.restore(st.bus);
        self.mark_dirty();
        Ok(())
    }
}
