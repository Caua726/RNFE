# Changelog

Uma linha por tarefa fechada. IDs referem-se ao [PLAN.md](PLAN.md).

## Não publicado — F2 APU e PPU exatos

- F2-07 PPU: framebuffer por índice de paleta (9 bits: ênfase + cor), `PALETTE_RGBA` de 512 cores gerada em `const fn`, RGBA convertido sob demanda em `Nes::framebuffer()`, `framebuffer_indexed()` para frontends com paleta na GPU; grayscale e ênfase no render; backdrop mostra `palette[v]` com render desligado (full_palette exibe as 512 cores). Exemplo `ppu_dump`.
- F2-05 PPU: open bus ("decay register" por bit, ~600 ms), `$2007` durante o render incrementa X e Y, paleta lida com bits altos do open bus; PRG RAM de 8 KB em `$6000` para todo mapper. ppu_open_bus, cpu_exec_space_ppuio, cpu_dummy_writes_ppumem e test_ppu_read_buffer (Bisqwit) passam.
- F2-06 PPU: avaliação de sprites igual ao hardware (a partir de OAMADDR, bug do índice `m` no 9º sprite, dot exato da flag de overflow, OAM secundário copiado no dot 257, OAMADDR zerado nos dots 257–320), sprite 0 nunca em x=255, sprites deixam de aparecer 1 px à esquerda, `$2004` mascara o byte de atributo e só bumpa OAMADDR durante o render. sprite_hit 11/11, sprite_overflow 5/5, oam_stress.
- F2-04 PPU: `$2002` lido um dot antes do VBL suprime a flag/NMI do frame; dot pulado em (261,339→0,0) nos frames ímpares; `$2001` liga/desliga o render com 2 dots de atraso. ppu_vbl_nmi 10/10, vbl_nmi_timing 7/7.
- F2-01/02/03 APU reescrita em ciclos de CPU: frame counter com IRQ e atraso de `$4017`, `$4015` com flags, noise/DMC por ciclo, DMC com DMA que para a CPU (3–4 ciclos, leituras repetidas em `$2007`), latência de halt/reload dos length counters, reset como se `$4017` fosse escrito antes da 1ª instrução. Acesso da CPU no meio do ciclo (2 dots antes) e sem polling ao fim de BRK/IRQ. Open bus da CPU. Controle devolve 1 após 8 leituras. Harness: estilo `Crc`, resets múltiplos, `run_rom -v`, `disasm`. Passam agora: apu_test 8/8, blargg_apu_2005 11/11, apu_reset 6/6, cpu_interrupts 5/5, instr_misc 4/4, dmc_dma_during_read4 4/5, ppu_vbl_nmi 05/07/08, vbl_nmi_timing 6/7.

## Não publicado — F1 CPU exata

- F1-05 Bus ciclo a ciclo: cada acesso da CPU avança PPU (3 dots) e APU; CPU reescrita com tabela estática `(Op, Mode)`, dummy reads iguais ao hardware, OAM DMA inline (513/514 ciclos), polling de NMI (borda) e IRQ (nível) no penúltimo ciclo com hijack; `cart_ptr` removido e `#![forbid(unsafe_code)]`; +30 % de fps. Absorveu F1-06 (dummy reads/writes) e F1-07 (interrupções).
- F1-04 PPU só faz mux/sprite-0/escrita na janela visível e usa a paleta como `const`; +17 % de fps, snapshots idênticos.
- F1-03 BRK/reset/NMI exatos (`instr_test-v5` 16/16, `cpu_reset` 2/2); snapshots divergentes agora são gravados em PNG (encoder sem deps).
- F1-02 Opcodes não-oficiais ANC/ALR/ARR/XAA/LAX#/AXS/LAS/SHA/TAS/SHY/SHX e JAM.
- F1-01 Tabela de opcodes corrigida (22 NOPs, `EB`, `97/B7`): nestest 8991/8991.

## Não publicado — F0 Fundação

- F0-10 README real (rodar, controles, mappers, estrutura), CHANGELOG.
- F0-09 CI: fmt/clippy/testes do núcleo com as ROMs em cache, `cargo check` do desktop, bench informativo.
- F0-08 Crates `rnfe-frontend` (FramePacer, InputState) e `rnfe-tty` (half-blocks 24-bit, zero deps, roda no Termux).
- F0-07 `clippy -D warnings` limpo no núcleo; `scripts/check.sh`, `scripts/peak-rss.sh`, `rustfmt.toml`; `cargo fmt` em tudo.
- F0-06 `status` gera `docs/STATUS.md`; `bench` mede fps/ns por frame/VmHWM; snapshots de framebuffer (10 ROMs).
- F0-05 Harness: tabela de 120 ROMs (`Pass`/`KnownFail` com motivo), runner `$6000`/tela, `nestest` com `VERIFIED_LINES`, `scripts/fetch-roms.sh`; overflow de `pc`/`addr_abs`/`stkp` corrigido com `wrapping_*`.
- F0-04 NROM: leitura de PRG 32 KB corrigida (regressão do refactor de mappers) e PRG RAM em `$6000-$7FFF`.
- F0-03 API do núcleo: `Cartridge::from_bytes`/`RomError`, `Nes::new(cart)`, `run_frame`/`peek`/`framebuffer` (RGBA8)/`Buttons`/`drain_audio`; `println!` → `log`; `diagnostic_report()`.
- F0-02 Workspace: `crates/rnfe-core` (sem deps), `rnfe-gui`, `rnfe-desktop`; perfis Cargo para 1,5 GB de RAM; `default-members` sem wgpu.
- F0-01 Higiene: binário ELF e ROMs inválidas fora do repo; `cargo-features` removido (compila em Rust estável); LICENSE MIT; `PLAN.md`.
