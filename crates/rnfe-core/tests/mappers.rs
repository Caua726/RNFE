//! Comportamentos de mapper que nenhuma ROM de teste cobre: ROMs sintéticas por mapper.
use rnfe_core::Cartridge;

/// ROM iNES com `prg_16k` bancos de 16 KB, cada um preenchido com o seu índice; CHR RAM.
fn rom(mapper: u8, prg_16k: usize, flags6_low: u8) -> Cartridge {
    let mut v = b"NES\x1A".to_vec();
    v.extend_from_slice(&[
        prg_16k as u8,
        0,
        (mapper << 4) | flags6_low,
        mapper & 0xF0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ]);
    for b in 0..prg_16k {
        v.extend(std::iter::repeat_n(b as u8, 16384));
    }
    Cartridge::from_bytes(&v).unwrap()
}

/// Escreve um registrador do MMC1 (5 bits, LSB primeiro), com 4 ciclos entre escritas.
fn mmc1_write(cart: &mut Cartridge, addr: u16, value: u8) {
    for i in 0..5 {
        cart.data.cpu_cycle += 4;
        cart.cpu_write(addr, (value >> i) & 1);
    }
}

#[test]
fn mmc1_power_on_fixes_last_bank_and_switches() {
    let mut cart = rom(1, 4, 0);
    assert_eq!(cart.cpu_read(0x8000), Some(0), "modo 3: banco 0 em $8000");
    assert_eq!(cart.cpu_read(0xC000), Some(3), "modo 3: último banco em $C000");
    mmc1_write(&mut cart, 0xE000, 2);
    assert_eq!(cart.cpu_read(0x8000), Some(2));
    assert_eq!(cart.cpu_read(0xC000), Some(3));
    // modo 32 KB (control bits 2-3 = 0): banco par de 32 KB
    mmc1_write(&mut cart, 0x8000, 0x03);
    assert_eq!(cart.cpu_read(0x8000), Some(2));
    assert_eq!(cart.cpu_read(0xC000), Some(3));
}

#[test]
fn mmc1_ignores_second_write_of_rmw() {
    let mut cart = rom(1, 4, 0);
    // INC $E000 escreve duas vezes em ciclos consecutivos: só a primeira entra no shift
    for i in 0..4 {
        cart.data.cpu_cycle += 4;
        cart.cpu_write(0xE000, (2 >> i) & 1);
    }
    cart.data.cpu_cycle += 4;
    cart.cpu_write(0xE000, 0); // 5º bit (0) — banco 2
    assert_eq!(cart.cpu_read(0x8000), Some(2));
    // agora um RMW: 1º write no ciclo N, 2º no N+1
    for i in 0..4 {
        cart.data.cpu_cycle += 4;
        cart.cpu_write(0xE000, (1 >> i) & 1);
    }
    cart.data.cpu_cycle += 4;
    cart.cpu_write(0xE000, 0); // 5º bit → banco 1
    cart.data.cpu_cycle += 1;
    cart.cpu_write(0xE000, 1); // ignorado (ciclo seguinte); senão viraria o 1º bit de outro valor
    assert_eq!(cart.cpu_read(0x8000), Some(1));
    // o shift continua vazio: 5 escritas normais carregam um valor inteiro
    mmc1_write(&mut cart, 0xE000, 3);
    assert_eq!(cart.cpu_read(0x8000), Some(3));
}

#[test]
fn mmc1_surom_selects_256k_half_with_chr_bit4() {
    let mut cart = rom(1, 32, 0); // 512 KB
    assert_eq!(cart.cpu_read(0xC000), Some(15), "último banco da metade baixa");
    mmc1_write(&mut cart, 0xA000, 0x10);
    assert_eq!(cart.cpu_read(0xC000), Some(31), "último banco da metade alta");
    mmc1_write(&mut cart, 0xE000, 5);
    assert_eq!(cart.cpu_read(0x8000), Some(16 + 5));
}

#[test]
fn mmc1_prg_ram_enable_bit() {
    let mut cart = rom(1, 2, 0);
    cart.cpu_write(0x6000, 0x42);
    assert_eq!(cart.cpu_read(0x6000), Some(0x42));
    mmc1_write(&mut cart, 0xE000, 0x10); // bit 4 = RAM desabilitada
    assert_eq!(cart.cpu_read(0x6000), None, "open bus com a RAM desligada");
    cart.cpu_write(0x6000, 0x99);
    mmc1_write(&mut cart, 0xE000, 0x00);
    assert_eq!(cart.cpu_read(0x6000), Some(0x42), "escrita com a RAM desligada foi ignorada");
}

#[test]
fn mmc3_prg_ram_protect() {
    let mut cart = rom(4, 2, 0);
    cart.cpu_write(0x6000, 0x11);
    assert_eq!(cart.cpu_read(0x6000), Some(0x11));
    cart.cpu_write(0xA001, 0xC0); // habilitada, protegida
    cart.cpu_write(0x6000, 0x22);
    assert_eq!(cart.cpu_read(0x6000), Some(0x11));
    cart.cpu_write(0xA001, 0x00); // desabilitada
    assert_eq!(cart.cpu_read(0x6000), None);
    cart.cpu_write(0xA001, 0x80);
    cart.cpu_write(0x6000, 0x33);
    assert_eq!(cart.cpu_read(0x6000), Some(0x33));
}

#[test]
fn fme7_irq_counter() {
    let mut cart = rom(69, 2, 0);
    assert!(cart.wants_cpu_clock());
    cart.cpu_write(0x8000, 0xE);
    cart.cpu_write(0xA000, 3); // counter = 3
    cart.cpu_write(0x8000, 0xF);
    cart.cpu_write(0xA000, 0);
    cart.cpu_write(0x8000, 0xD);
    cart.cpu_write(0xA000, 0x81); // contar + IRQ
    for _ in 0..3 {
        cart.cpu_clock();
        assert!(!cart.irq_pending());
    }
    cart.cpu_clock(); // 0 → $FFFF
    assert!(cart.irq_pending());
    cart.cpu_clock();
    assert!(cart.irq_pending(), "nível: continua até o ack");
    cart.cpu_write(0xA000, 0x81); // escrever $D reconhece
    assert!(!cart.irq_pending());
    cart.cpu_write(0xA000, 0x80); // contar sem IRQ
    for _ in 0..70000 {
        cart.cpu_clock();
    }
    assert!(!cart.irq_pending());
}

#[test]
fn fme7_wram_window() {
    let mut cart = rom(69, 2, 0);
    assert_eq!(cart.cpu_read(0x6000), Some(0), "padrão: banco 0 de ROM em $6000");
    cart.cpu_write(0x8000, 8);
    cart.cpu_write(0xA000, 0xC0); // RAM habilitada
    cart.cpu_write(0x6000, 0x77);
    assert_eq!(cart.cpu_read(0x6000), Some(0x77));
    cart.cpu_write(0xA000, 0x40); // RAM selecionada mas desabilitada → open bus
    assert_eq!(cart.cpu_read(0x6000), None);
    cart.cpu_write(0xA000, 0x01); // ROM banco 1
    assert_eq!(cart.cpu_read(0x6000), Some(0));
    assert_eq!(cart.cpu_read(0x7FFF), Some(0));
    cart.cpu_write(0xA000, 0x02); // ROM banco 2 (8 KB) = 2ª metade... do banco 16K nº 1
    assert_eq!(cart.cpu_read(0x6000), Some(1));
}

// ---------------------------------------------------------------- F6: VRC6 e 5B

#[test]
fn vrc6_banks_irq_and_audio() {
    let mut cart = rom(24, 8, 0); // 128 KB, 16 bancos de 8 KB
    assert_eq!(cart.cpu_read(0xE000), Some(7), "último banco de 16 KB (índice 7) fixo em $E000");
    cart.cpu_write(0x8000, 2);
    assert_eq!(cart.cpu_read(0x8000), Some(2));
    cart.cpu_write(0xC000, 9); // banco de 8 KB 9 = metade alta do banco de 16 KB 4
    assert_eq!(cart.cpu_read(0xC000), Some(4));
    // PRG RAM só com o bit 7 de $B003
    assert_eq!(cart.cpu_read(0x6000), None);
    cart.cpu_write(0xB003, 0x80);
    cart.cpu_write(0x6000, 0x5A);
    assert_eq!(cart.cpu_read(0x6000), Some(0x5A));

    // IRQ em modo scanline: latch 0xFE → 2 clocks de 113⅔ ciclos até estourar
    cart.cpu_write(0xF000, 0xFE);
    cart.cpu_write(0xF001, 0x02);
    for _ in 0..227 {
        cart.cpu_clock();
    }
    assert!(!cart.irq_pending(), "ainda não");
    for _ in 0..5 {
        cart.cpu_clock();
    }
    assert!(cart.irq_pending(), "0xFE → 0xFF → estouro na 2ª scanline");
    cart.cpu_write(0xF002, 0); // ack
    assert!(!cart.irq_pending());

    // Áudio: pulso 1 ligado, volume 15, duty 7 (metade) → saída > 0 em algum momento
    cart.cpu_write(0x9000, 0x7F);
    cart.cpu_write(0x9001, 0x10);
    cart.cpu_write(0x9002, 0x80);
    let mut max = 0.0f32;
    for _ in 0..2000 {
        cart.cpu_clock();
        max = max.max(cart.audio_output());
    }
    assert!(max > 0.1, "pulso do VRC6 deveria soar: {max}");
    cart.cpu_write(0x9003, 0x01); // halt
    let v = cart.audio_output();
    for _ in 0..100 {
        cart.cpu_clock();
    }
    assert_eq!(cart.audio_output(), v, "congelado");
}

#[test]
fn vrc6_mapper_26_swaps_a0_a1() {
    let mut cart = rom(26, 4, 0);
    // No mapper 26, $F002 (A1) faz o papel de $F001 (controle) e vice-versa
    cart.cpu_write(0xF000, 0xFF);
    cart.cpu_write(0xF002, 0x06); // = $F001 do 24: enable + cycle mode
    cart.cpu_clock();
    assert!(cart.irq_pending(), "modo ciclo com latch 0xFF estoura no 1º clock");
    cart.cpu_write(0xF001, 0); // = $F002 do 24: ack
    assert!(!cart.irq_pending());
}

#[test]
fn sunsoft_5b_tone() {
    let mut cart = rom(69, 2, 0);
    cart.cpu_write(0x8000, 0x0F); // FME-7 cmd (irrelevante)
    cart.cpu_write(0xC000, 0); // reg 0: período A baixo
    cart.cpu_write(0xE000, 0x10);
    cart.cpu_write(0xC000, 7); // mixer: tom A ligado (bit 0 = 0)
    cart.cpu_write(0xE000, 0xFE);
    cart.cpu_write(0xC000, 8); // volume A
    cart.cpu_write(0xE000, 0x0F);
    let mut seen_on = false;
    let mut seen_off = false;
    for _ in 0..1000 {
        cart.cpu_clock();
        let o = cart.audio_output();
        if o > 0.05 {
            seen_on = true;
        } else {
            seen_off = true;
        }
    }
    assert!(seen_on && seen_off, "onda quadrada do 5B: on={seen_on} off={seen_off}");
}
