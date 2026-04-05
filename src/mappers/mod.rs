pub mod nrom;
pub mod mmc1;
pub mod uxrom;
pub mod cnrom;
pub mod mmc3;
pub mod axrom;
pub mod mmc2;
pub mod colordreams;
pub mod bnrom;
pub mod gxrom;
pub mod fme7;
pub mod camerica;
pub mod dxrom;
pub mod mapper227;

use crate::cartridge::Mirror;

pub struct CartData {
    pub prg: Vec<u8>,
    pub chr: Vec<u8>,
    pub prg_ram: Vec<u8>,
    pub prg_banks: u8,
    pub chr_banks: u8,
    pub mirror: Mirror,
}

pub trait Mapper {
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8>;
    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool;
    fn ppu_read(&mut self, addr: u16, data: &CartData) -> Option<u8>;
    fn clock_scanline(&mut self) {}
    fn mapper_irq(&mut self) -> bool { false }
    fn reset(&mut self, prg_banks: u8);
    fn print_state(&self) {}
}

pub fn create_mapper(id: u8, prg_banks: u8) -> Box<dyn Mapper> {
    match id {
        0 => Box::new(nrom::Nrom),
        1 => Box::new(mmc1::Mmc1::new()),
        2 => Box::new(uxrom::Uxrom::new()),
        3 => Box::new(cnrom::Cnrom::new()),
        4 => Box::new(mmc3::Mmc3::new(prg_banks)),
        7 => Box::new(axrom::Axrom::new()),
        9 => Box::new(mmc2::Mmc2::new()),
        11 => Box::new(colordreams::ColorDreams::new()),
        34 => Box::new(bnrom::Bnrom::new()),
        66 => Box::new(gxrom::Gxrom::new()),
        69 => Box::new(fme7::Fme7::new()),
        71 => Box::new(camerica::Camerica::new()),
        206 => Box::new(dxrom::Dxrom::new(prg_banks)),
        227 => Box::new(mapper227::Mapper227::new()),
        _ => Box::new(nrom::Nrom), // fallback
    }
}
