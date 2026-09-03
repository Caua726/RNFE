//! Mapper 004 (MMC3): 8 registradores de banco + contador de scanline clockado por A12.
use super::{CartData, Mapper};
use crate::cartridge::Mirror;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct Mmc3 {
    bank_select: u8,
    regs: [u8; 8],
    irq_counter: u8,
    irq_latch: u8,
    /// `$C001` escrito: o próximo clock recarrega em vez de decrementar.
    irq_reload: bool,
    irq_enabled: bool,
    irq_pending: bool,
    /// `$A001`: bit 7 habilita a PRG RAM, bit 6 protege contra escrita.
    ram_ctrl: u8,
    /// Revisão A (NEC, submapper 4.4): IRQ só na transição para 0, não a cada clock em 0.
    rev_a: bool,
    /// 118 (TxSROM): o bit 7 de cada banco de CHR escolhe a página de nametable.
    txsrom: bool,
    /// 119 (TQROM): o bit 6 do banco de CHR escolhe CHR RAM (8 KB) em vez da ROM.
    tqrom: bool,
}

impl Mmc3 {
    pub fn new(data: &CartData) -> Self {
        Mmc3 {
            bank_select: 0,
            regs: [0, 2, 4, 5, 6, 7, 0, 1],
            irq_counter: 0,
            irq_latch: 0,
            irq_reload: false,
            irq_enabled: false,
            irq_pending: false,
            ram_ctrl: 0x80,
            rev_a: data.submapper == 4,
            txsrom: data.mapper == 118,
            tqrom: data.mapper == 119,
        }
    }

    /// Registrador de CHR que cobre o endereço (com a inversão de A12 do bit 7).
    #[inline]
    fn chr_reg(&self, addr: u16) -> (usize, usize) {
        let a = addr ^ ((self.bank_select as u16 & 0x80) << 5); // inverte A12 se bit 7
        match a >> 10 {
            0 => (self.regs[0] as usize, 0),
            1 => (self.regs[0] as usize, 1),
            2 => (self.regs[1] as usize, 0),
            3 => (self.regs[1] as usize, 1),
            n => (self.regs[n as usize - 2] as usize, 0),
        }
    }
}

impl Mapper for Mmc3 {
    #[inline]
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr < 0x8000 {
            return if addr >= 0x6000 && self.ram_ctrl & 0x80 != 0 {
                Some(data.prg_ram_at((addr & 0x1FFF) as usize))
            } else {
                None
            };
        }
        let swap = self.bank_select & 0x40 != 0;
        let last = data.prg_8k() - 1;
        let bank = match (addr >> 13) & 3 {
            0 => {
                if swap {
                    last.saturating_sub(1)
                } else {
                    self.regs[6] as usize
                }
            }
            1 => self.regs[7] as usize,
            2 => {
                if swap {
                    self.regs[6] as usize
                } else {
                    last.saturating_sub(1)
                }
            }
            _ => last,
        };
        Some(data.prg_at(bank * 0x2000 + (addr & 0x1FFF) as usize))
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        if addr < 0x8000 {
            if addr >= 0x6000 {
                if self.ram_ctrl & 0xC0 == 0x80 {
                    data.prg_ram_set((addr & 0x1FFF) as usize, val);
                }
                return true;
            }
            return false;
        }
        match addr & 0xE001 {
            0x8000 => self.bank_select = val,
            0x8001 => {
                let r = (self.bank_select & 0x07) as usize;
                self.regs[r] = match r {
                    0 | 1 => val & 0xFE,
                    6 | 7 => val & 0x3F,
                    _ => val,
                };
            }
            0xA000 => {
                data.mirror = if val & 0x01 != 0 { Mirror::Horizontal } else { Mirror::Vertical };
            }
            0xA001 => self.ram_ctrl = val,
            0xC000 => self.irq_latch = val,
            0xC001 => {
                // limpa o contador agora; recarrega no próximo clock (sem IRQ)
                self.irq_counter = 0;
                self.irq_reload = true;
            }
            0xE000 => {
                self.irq_enabled = false;
                self.irq_pending = false; // ack
            }
            _ => self.irq_enabled = true,
        }
        true
    }

    fn manages_prg_ram(&self) -> bool {
        true
    }

    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        let (reg, odd) = self.chr_reg(addr);
        let bank = if self.tqrom { reg & 0x3F } else { reg } + odd;
        bank * 0x0400 + (addr & 0x03FF) as usize
    }

    fn chr_dynamic(&self) -> bool {
        self.tqrom
    }

    fn nt_dynamic(&self) -> bool {
        false
    }

    #[inline]
    fn ppu_read(&mut self, addr: u16, data: &CartData) -> u8 {
        if self.tqrom {
            let (reg, odd) = self.chr_reg(addr);
            if reg & 0x40 != 0 {
                // CHR RAM de 8 KB: 8 bancos de 1 KB no fim da CHR (o cartucho reserva)
                let bank = ((reg & 0x07) + odd) & 0x07;
                return data.chr_ram_at(bank * 0x0400 + (addr & 0x03FF) as usize);
            }
        }
        data.chr_at(self.chr_offset(addr))
    }

    #[inline]
    fn ppu_write(&mut self, addr: u16, val: u8, data: &mut CartData) {
        if self.tqrom {
            let (reg, odd) = self.chr_reg(addr);
            if reg & 0x40 != 0 {
                let bank = ((reg & 0x07) + odd) & 0x07;
                data.chr_ram_set(bank * 0x0400 + (addr & 0x03FF) as usize, val);
            }
            return;
        }
        data.chr_set(self.chr_offset(addr), val);
    }

    #[inline]
    fn nt_source(&mut self, addr: u16, _data: &CartData) -> Option<crate::cartridge::NtSource> {
        if !self.txsrom {
            return None;
        }
        // TxSROM: nametable de cada quadrante = bit 7 do registrador de CHR que cobre a
        // mesma posição em $0000-$0FFF (modo normal) — regs 0,0,1,1 ou 2,3,4,5 com A12 invertido
        let q = (addr >> 10) as usize & 3;
        let reg = if self.bank_select & 0x80 == 0 { [0, 0, 1, 1][q] } else { [2, 3, 4, 5][q] };
        Some(crate::cartridge::NtSource::Ciram((self.regs[reg] >> 7) & 1))
    }

    fn a12_rise(&mut self) {
        let before = self.irq_counter;
        if before == 0 || self.irq_reload {
            self.irq_counter = self.irq_latch;
        } else {
            self.irq_counter -= 1;
        }
        // Rev B (Sharp): IRQ sempre que o contador está em 0 após o clock (inclusive recarregado
        // com 0, a cada clock). Rev A (NEC): só quando passou a 0 agora, ou recarregou após $C001.
        if self.irq_counter == 0 && self.irq_enabled && (!self.rev_a || before != 0 || self.irq_reload) {
            self.irq_pending = true;
        }
        self.irq_reload = false;
    }

    #[inline]
    fn irq_pending(&self) -> bool {
        self.irq_pending
    }

    fn reset(&mut self, _data: &mut CartData) {
        self.bank_select = 0;
        self.regs = [0, 2, 4, 5, 6, 7, 0, 1];
        self.irq_counter = 0;
        self.irq_latch = 0;
        self.irq_reload = false;
        self.irq_enabled = false;
        self.irq_pending = false;
        self.ram_ctrl = 0x80;
    }

    fn state_string(&self) -> String {
        format!(
            "  MMC3 bank_select: ${:02X}  regs: {:?}  $A001: ${:02X}  rev {}\n  MMC3 IRQ: counter={} latch={} reload={} enabled={} pending={}\n",
            self.bank_select,
            self.regs,
            self.ram_ctrl,
            if self.rev_a { "A" } else { "B" },
            self.irq_counter,
            self.irq_latch,
            self.irq_reload,
            self.irq_enabled,
            self.irq_pending
        )
    }
}
