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

#[test]
fn n163_banks_nametables_irq_audio() {
    let mut cart = rom(19, 8, 0); // horizontal
    assert_eq!(cart.cpu_read(0xE000), Some(7));
    cart.cpu_write(0xE000, 5);
    assert_eq!(cart.cpu_read(0x8000), Some(2), "banco de 8 KB 5 = metade alta do 16 KB 2");
    // nametables: padrão segue o header (horizontal: $2000/$2400 → CIRAM 0, $2800/$2C00 → 1)
    let mut ciram = [[0u8; 1024]; 4];
    ciram[0][5] = 0xAA;
    ciram[1][5] = 0xBB;
    assert_eq!(cart.nt_read(0x2005, &ciram), 0xAA);
    assert_eq!(cart.nt_read(0x2405, &ciram), 0xAA);
    assert_eq!(cart.nt_read(0x2805, &ciram), 0xBB);
    cart.cpu_write(0xC000, 0xE1); // $2000 → CIRAM 1
    assert_eq!(cart.nt_read(0x2005, &ciram), 0xBB);
    cart.cpu_write(0xC000, 0x00); // $2000 → CHR ROM banco 0 (CHR RAM aqui: zeros)
    cart.chr_write(0x0005, 0x77);
    assert_eq!(cart.nt_read(0x2005, &ciram), 0x77);
    cart.nt_write(0x2006, 0x66, &mut ciram);
    assert_eq!(cart.chr_read(0x0006), 0x66, "escrita em NT mapeada para CHR RAM");

    // IRQ: contador de 15 bits até $7FFF
    cart.cpu_write(0x5000, 0xFD);
    cart.cpu_write(0x5800, 0xFF); // $7FFD, enable
    cart.cpu_clock();
    assert!(!cart.irq_pending());
    cart.cpu_clock();
    assert!(cart.irq_pending());
    assert_eq!(cart.cpu_read(0x5800), Some(0xFF));
    cart.cpu_write(0x5800, 0x00); // ack + disable
    assert!(!cart.irq_pending());

    // Áudio: 1 canal (reg $7F = 0), onda quadrada na RAM $00-$0F (32 amostras), volume 15
    cart.cpu_write(0xF800, 0x80); // endereço 0 com auto-incremento
    for i in 0..16 {
        cart.cpu_write(0x4800, if i < 8 { 0xFF } else { 0x00 });
    }
    cart.cpu_write(0xF800, 0x80 | 0x78); // canal 7
    for v in [0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0x0F] {
        // freq $001000, fase 0, comprimento 256-128 = 32 amostras, offset 0, volume 15
        cart.cpu_write(0x4800, v);
    }
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for _ in 0..40_000 {
        cart.cpu_clock();
        let o = cart.audio_output();
        lo = lo.min(o);
        hi = hi.max(o);
    }
    assert!(hi > 0.1 && lo < -0.1, "onda do N163 deveria oscilar: {lo}..{hi}");
    cart.cpu_write(0xE000, 0x40); // som desligado
    assert_eq!(cart.audio_output(), 0.0);
}

#[test]
fn mmc5_prg_modes_multiplier_fill_exram_and_scanlines() {
    let mut cart = rom(5, 8, 0); // 128 KB = 16 bancos de 8 KB (valor = índice de 16 KB)
    // power-on: modo 3, $5117 = $FF → último banco em $E000
    assert_eq!(cart.cpu_read(0xE000), Some(7));
    cart.cpu_write(0x5100, 3);
    cart.cpu_write(0x5114, 0x80 | 2); // $8000 ← banco 8 KB 2 (= 16 KB 1)
    cart.cpu_write(0x5115, 0x80 | 5);
    cart.cpu_write(0x5116, 0x80 | 9);
    assert_eq!(cart.cpu_read(0x8000), Some(1));
    assert_eq!(cart.cpu_read(0xA000), Some(2));
    assert_eq!(cart.cpu_read(0xC000), Some(4));
    cart.cpu_write(0x5100, 0); // 32 KB por $5117 = $FF → bancos 12-15
    assert_eq!(cart.cpu_read(0x8000), Some(6));
    assert_eq!(cart.cpu_read(0xE000), Some(7));
    cart.cpu_write(0x5100, 1); // 16 KB: $5115 (5 → par 4-5) e $5117
    assert_eq!(cart.cpu_read(0x8000), Some(2));
    assert_eq!(cart.cpu_read(0xA000), Some(2));
    assert_eq!(cart.cpu_read(0xC000), Some(7));

    // PRG RAM: só grava com $5102 = 2 e $5103 = 1
    cart.cpu_write(0x6000, 0x11);
    assert_eq!(cart.cpu_read(0x6000), Some(0));
    cart.cpu_write(0x5102, 2);
    cart.cpu_write(0x5103, 1);
    cart.cpu_write(0x6000, 0x11);
    assert_eq!(cart.cpu_read(0x6000), Some(0x11));

    // multiplicador
    cart.cpu_write(0x5205, 200);
    cart.cpu_write(0x5206, 3);
    assert_eq!(cart.cpu_read(0x5205), Some((600 & 0xFF) as u8));
    assert_eq!(cart.cpu_read(0x5206), Some((600u16 >> 8) as u8));

    // nametables: $2000 fill, $2400 ExRAM, $2800 CIRAM 0, $2C00 CIRAM 1
    let mut ciram = [[0u8; 1024]; 4];
    ciram[1][0x10] = 0xC1;
    cart.cpu_write(0x5105, 0b01_00_10_11);
    cart.cpu_write(0x5106, 0x42);
    cart.cpu_write(0x5107, 2);
    assert_eq!(cart.nt_read(0x2010, &ciram), 0x42, "fill tile");
    assert_eq!(cart.nt_read(0x23C5, &ciram), 0xAA, "fill attr 2 replicado");
    cart.cpu_write(0x5104, 0); // ExRAM como nametable
    cart.nt_write(0x2410, 0x77, &mut ciram);
    assert_eq!(cart.nt_read(0x2410, &ciram), 0x77);
    assert_eq!(cart.nt_read(0x2C10, &ciram), 0xC1);
    cart.cpu_write(0x5104, 2); // ExRAM como RAM da CPU
    assert_eq!(cart.cpu_read(0x5C10), Some(0x77));

    // detecção de scanline: 3 leituras iguais → in_frame; depois cada trio conta uma linha
    assert_eq!(cart.cpu_read(0x5204), Some(0x00));
    cart.cpu_write(0x5203, 2);
    cart.cpu_write(0x5204, 0x80);
    for _ in 0..3 {
        cart.nt_read(0x2800, &ciram);
    }
    assert_eq!(cart.cpu_read(0x5204), Some(0x40), "in_frame, scanline 0");
    cart.nt_read(0x2801, &ciram); // quebra a sequência
    for _ in 0..3 {
        cart.nt_read(0x2802, &ciram);
    }
    assert!(!cart.irq_pending(), "scanline 1");
    cart.nt_read(0x2803, &ciram);
    for _ in 0..3 {
        cart.nt_read(0x2804, &ciram);
    }
    assert!(cart.irq_pending(), "scanline 2 == $5203");
    assert_eq!(cart.cpu_read(0x5204), Some(0xC0));
    cart.cpu_read_mut(0x5204);
    assert!(!cart.irq_pending(), "ler $5204 reconhece");
    for _ in 0..400 {
        cart.cpu_clock();
    }
    assert_eq!(cart.cpu_read(0x5204), Some(0x00), "sem leituras → fim do frame");
}

#[test]
fn mmc5_vertical_split() {
    let mut cart = rom(5, 2, 0);
    let ciram = [[0u8; 1024]; 4];
    // ExRAM como nametable normal em $2000 (modo 0) com um padrão conhecido; CHR RAM
    cart.cpu_write(0x5104, 0);
    cart.cpu_write(0x5105, 0b00_00_00_00); // tudo CIRAM 0 (zeros)
    for i in 0..960u16 {
        cart.cpu_write(0x5104, 2); // modo RAM para escrever pela CPU
        cart.cpu_write(0x5C00 + i, ((i / 32) * 16 + i % 32) as u8); // row*16 + col (trunca)
    }
    cart.cpu_write(0x5C00 + 0x3C0 + 1, 0b11_10_01_00); // atributo do bloco (colunas 4-7, linhas 0-3)
    cart.cpu_write(0x5104, 0);
    cart.cpu_write(0x5101, 0); // CHR 8 KB
    cart.cpu_write(0x5127, 0);
    cart.chr_write(0x1000 + 0x0035, 0xAB); // banco 1 de 4 KB, tile 3, linha fina 5
    cart.cpu_write(0x5200, 0x80 | 6); // split à esquerda: colunas 0-5
    cart.cpu_write(0x5201, 8 * 3 + 5); // rolagem: linha 29 → row 3, fine y 5 (na scanline 0)
    cart.cpu_write(0x5202, 1);

    // scanline 0 detectada (3 leituras iguais); essa 3ª leitura é a busca do tile idx 0 = coluna 2
    // (endereços distintos entre buscas, para não disparar outra detecção sem querer)
    let t = cart.nt_read(0x2010, &ciram);
    let _ = cart.nt_read(0x2010, &ciram);
    let t0 = cart.nt_read(0x2010, &ciram);
    assert_eq!(t, 0, "antes da detecção: CIRAM");
    assert_eq!(t0, 3 * 16 + 2, "coluna 2 da região dividida, linha 3 da ExRAM");
    // atributo desse tile: bloco 0 (col 2 → col/4 = 0), quadrante col&2=2, row&2=2 → shift 6
    cart.cpu_write(0x5104, 2);
    cart.cpu_write(0x5C00 + 0x3C0, 0b10_00_00_00);
    cart.cpu_write(0x5104, 0);
    assert_eq!(cart.nt_read(0x23C0, &ciram), 0b10 * 0x55, "paleta 2 replicada");
    // padrão: banco $5202 (4 KB 1), tile 3 (endereço $0030 pedido pela PPU), linha fina do split (5)
    assert_eq!(cart.chr_read(0x0030), 0xAB, "linha fina vem da rolagem do split, banco de $5202");
    // idx 1 → coluna 3 (ainda split); idx 2..3 → colunas 4,5 (split); idx 4 → coluna 6 (normal)
    assert_eq!(cart.nt_read(0x2001, &ciram), 3 * 16 + 3);
    let _ = cart.nt_read(0x2002, &ciram);
    let _ = cart.nt_read(0x2003, &ciram);
    assert_eq!(cart.nt_read(0x2004, &ciram), 0, "coluna 6: nametable normal (CIRAM 0)");
    assert_eq!(cart.chr_read(0x0030), 0, "fora do split: banco normal (0), CHR RAM zerada");
    // lado direito
    cart.cpu_write(0x5200, 0x80 | 0x40 | 30); // colunas 30 e 31
    for i in 5..28u16 {
        let _ = cart.nt_read(0x2020 + i, &ciram);
    }
    assert_eq!(cart.nt_read(0x2050, &ciram), 3 * 16 + 30, "idx 28 = coluna 30");
    assert_eq!(cart.nt_read(0x2051, &ciram), 3 * 16 + 31);
    assert_eq!(cart.nt_read(0x2052, &ciram), 0, "coluna 32: fora da tela, normal");
    let _ = cart.nt_read(0x2053, &ciram); // idx 31
    // idx 32/33 = colunas 0/1 da linha seguinte (scanline 1 → linha 30 → ainda row 3)
    cart.cpu_write(0x5200, 0x80 | 2);
    assert_eq!(cart.nt_read(0x2060, &ciram), 3 * 16, "coluna 0 da próxima linha");
    assert_eq!(cart.nt_read(0x2061, &ciram), 3 * 16 + 1);
}

#[test]
fn nina_cprom_quattro_and_mmc3_variants() {
    // NINA-03 (79): registrador em $4100 com A8 = 1
    let mut cart = rom(79, 4, 0); // 64 KB = 2 bancos de 32 KB
    assert_eq!(cart.cpu_read(0x8000), Some(0));
    cart.cpu_write(0x4100, 0x08); // PRG 1, CHR 0
    assert_eq!(cart.cpu_read(0x8000), Some(2), "banco de 32 KB 1 = 16 KB nº 2");
    cart.cpu_write(0x4000, 0x00); // A8 = 0: ignorado
    assert_eq!(cart.cpu_read(0x8000), Some(2));
    // Quattro (232): bloco 2, banco 1 → 16 KB nº 9; $C000 = último do bloco (11)
    let mut cart = rom(232, 16, 0);
    cart.cpu_write(0x8000, 0x10);
    cart.cpu_write(0xC000, 0x01);
    assert_eq!(cart.cpu_read(0x8000), Some(9));
    assert_eq!(cart.cpu_read(0xC000), Some(11));
    // CPROM (13): metade alta da CHR RAM comutável
    let mut cart = rom(13, 2, 0);
    cart.chr_write(0x1000, 0x11);
    cart.cpu_write(0x8000, 3);
    cart.chr_write(0x1000, 0x33);
    assert_eq!(cart.chr_read(0x1000), 0x33);
    cart.cpu_write(0x8000, 0);
    assert_eq!(cart.chr_read(0x1000), 0x11);
    // TxSROM (118): bit 7 do banco de CHR escolhe a nametable
    let mut cart = rom(118, 4, 0);
    let mut ciram = [[0u8; 1024]; 4];
    ciram[0][3] = 0xA0;
    ciram[1][3] = 0xB1;
    cart.cpu_write(0x8000, 0); // R0
    cart.cpu_write(0x8001, 0x80); // bit 7 → CIRAM 1 para $2000/$2400
    cart.cpu_write(0x8000, 1); // R1
    cart.cpu_write(0x8001, 0x00); // $2800/$2C00 → CIRAM 0
    assert_eq!(cart.nt_read(0x2003, &ciram), 0xB1);
    assert_eq!(cart.nt_read(0x2803, &ciram), 0xA0);
    // TQROM (119): bit 6 do banco → CHR RAM
    let mut cart = rom(119, 4, 0);
    cart.cpu_write(0x8000, 2); // R2 → $1000-$13FF
    cart.cpu_write(0x8001, 0x41); // RAM, banco 1
    cart.chr_write(0x1005, 0x77);
    assert_eq!(cart.chr_read(0x1005), 0x77);
    cart.cpu_write(0x8001, 0x01); // ROM banco 1 (CHR RAM do header: zeros)
    assert_eq!(cart.chr_read(0x1005), 0x00);
}
