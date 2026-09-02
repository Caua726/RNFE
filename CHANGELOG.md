# Changelog

Uma linha por tarefa fechada. IDs referem-se ao [PLAN.md](PLAN.md).

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
