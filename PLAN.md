# RNFE — plano de trabalho

Regras: uma tarefa por vez; cada tarefa termina com `scripts/check.sh` verde, `docs/STATUS.md` regenerado
e um commit `[ID] área: descrição`. A verdade sobre o progresso é `cargo test -p rnfe-core`
(`VERIFIED_LINES` do nestest + tabela `Pass/KnownFail` das ROMs blargg); os checkboxes só espelham.
Branch por fase (`fase-N-nome`), merge `--no-ff` em `main` no marco. Push só no fim.

Retomar de qualquer lugar:
```sh
git status -sb && git log --oneline -5
sed -n '/^## Onde parei/,/^## /p' PLAN.md
cargo test -p rnfe-core 2>&1 | grep -E 'test result|panicked|SKIP|agora PASSA'
cargo run -q -p rnfe-core --release --bin status | tail -3
```

## Onde parei
Tarefa: F0-03 (core: API)
Estado: concluída
Próximo: F0-04 — NROM 32 KB + PRG RAM

## Linha de base
(preenchida em F0-06)

## Alvos
- Tier 1 (60 fps + som): Web (Chrome/Firefox/Safari; Chrome Android 10+; Safari iOS 16+), Android arm64 8+, Desktop Linux x86_64 + Windows x86_64.
- Tier 2 (deve funcionar): Termux tty, Raspberry Pi, GPU só-OpenGL.
- Pior caso de referência: aparelho mais fraco que o moto g56 (2 GB / Cortex-A53). No g56: ≤ 8 ms/frame do núcleo em 1 núcleo, RSS < 50 MB nativo / < 100 MB web, APK < 5 MB, wasm < 2 MB gzip.

## F0 — Fundação (`fase-0-fundacao`) · marco M0: `cargo test` verde no celular e no CI; STATUS.md; SMB1 boota; jogo visível no tty
- [x] F0-01 repo: clone, identidade, branch, lixo versionado removido, .gitignore, sem `cargo-features`, LICENSE, PLAN.md
- [x] F0-02 [S] workspace: crates/rnfe-core (9 módulos + mappers), crates/rnfe-gui (display/ui), crates/rnfe-desktop; perfis; `default-members` sem wgpu; .cargo/config.toml
- [x] F0-03 core: `Cartridge::from_bytes` + `RomError`; `println!`→`log`; `diagnostic_report()->String`; `Buttons`; `Nes::new(cart)`; `run_frame/step_instruction/peek/set_controller/drain_audio/framebuffer` (RGBA8)
- [ ] F0-04 NROM: máscara 32 KB (`& 0x3FFF` extra) + PRG RAM `$6000-$7FFF`
- [ ] F0-05 harness: `src/testing/{list,runner}.rs`, `tests/blargg.rs` (KnownFail que passa = falha), `tests/nestest.rs` (`VERIFIED_LINES=5004`), `scripts/fetch-roms.sh`
- [ ] F0-06 bins: `status` → docs/STATUS.md; `bench` (fps, ns/frame, VmHWM, `--profile`); `tests/snapshots.rs`; linha de base
- [ ] F0-07 qualidade: `scripts/check.sh`, `scripts/peak-rss.sh`, rustfmt.toml, `[workspace.lints]`; clippy -D warnings limpo
- [ ] F0-08 rnfe-frontend mínimo (FramePacer, InputState) + rnfe-tty (half-blocks, stty, panic hook)
- [ ] F0-09 CI: ci.yml (core, desktop-check, bench)
- [ ] F0-10 README real + CHANGELOG

## F1 — CPU exata (`fase-1-cpu`) · marco M1: nestest 8991/8991 · instr_test-v5 16/16 · instr_timing 2/2 · instr_misc 4/4 · cpu_dummy_* · cpu_interrupts 5/5
- [ ] F1-01 tabela: 22 NOPs com modo certo, `EB`=SBC IMM, `97/B7`=4 ciclos → VERIFIED_LINES 8991
- [ ] F1-02 ilegais: ANC ALR ARR XAA LAX# AXS LAS SHA TAS SHY SHX + JAM
- [ ] F1-03 wrapping_add; BRK (B só no byte empilhado, I após push); reset I=1/SP-=3; NMI 7 ciclos
- [ ] F1-04 PPU: mux/paleta/sprite-0/shift só na janela visível (61 440 dots)
- [ ] F1-05 [S] bus ciclo a ciclo: `Bus::tick()` por acesso; RAM antes do cartucho; OAM DMA inline; remover `cart_ptr`; `#![forbid(unsafe_code)]`; mirroring por acesso
- [ ] F1-06 dummy reads/writes (`Access` na Instruction; RMW read/write/write)
- [ ] F1-07 [S] interrupções: `IrqLine` nivelada, `nmi_line` por borda, polling no penúltimo ciclo, hijack, quirk do branch; debugger gated

## F2 — APU e PPU exatos (`fase-2-apu-ppu`) · marco M2
- [ ] F2-01 APU frame counter em ciclos de CPU + IRQ flag/inhibit + `$4015` R + delay `$4017`
- [ ] F2-02 noise/DMC por ciclo; DMC IRQ; DMA do DMC com stall de 4; `$4000-$4013` open bus
- [ ] F2-03 length counter halt/reload ordering
- [ ] F2-04 VBL/NMI exatos: (241,1), prevent_vbl, supressão, odd-frame em (261,339)
- [ ] F2-05 open bus da PPU; `$2004`/`$2007` durante render; CPU open bus
- [ ] F2-06 avaliação de sprites em lote no dot 65 com `overflow_dot`; sprite 0 hit x<255
- [ ] F2-07 paleta 512 cores: framebuffer por índice + LUT; grayscale/emphasis; backdrop

## F3 — Mappers, saves, rewind (`fase-3-mappers-saves`) · marco M3
- [ ] F3-01 [S] trait `Mapper` novo + `enum MapperKind`; NES 2.0; bateria; four-screen; bounds; CHR só via cartucho
- [ ] F3-02 `A12Watcher` na PPU + MMC3 rev B (reload flag, ack nivelado, `$A001`); rev A por submapper
- [ ] F3-03 MMC1: writes consecutivos, SUROM/SOROM/SXROM, PRG RAM enable, CHR RAM banking
- [ ] F3-04 FME-7 IRQ + `$6000`
- [ ] F3-05 persistência: trait `Storage`; `FsStorage` + `SaveManager`; `.sav`
- [ ] F3-06 save states: feature `serde` (postcard), `static LOOKUP`, header RNFS, round-trip test
- [ ] F3-07 rewind

## F4 — Web (`fase-4-web`) · marco M4: Pages jogável no celular com toque e som
- [ ] F4-01 [S] rnfe-gui: app/gpu(async)/audio/platform/ui; `Arc<Window>`; `about_to_wait` + WaitUntil; ring SPSC; web_time; UserEvent; áudio lazy
- [ ] F4-02 wasm32: rnfe-web, index.html, WebStorage, webgl fallback, rfd async, Trunk.toml; toolchain no celular
- [ ] F4-03 toque: TouchLayout/InputState multi-touch; overlay em ui.rs
- [ ] F4-04 CI web + deploy Pages
- [ ] F4-05 gamepads

## F5 — Android (`fase-5-android`) · marco M5: APK arm64 roda SMB1 com toque e som
- [ ] F5-01 rnfe-android cdylib (android-activity, FsStorage, oboe, SAF)
- [ ] F5-02 release.yml: Linux, Windows, APK, web em tag
- [ ] F5-03 README Android

## F6 — Mappers estendidos e áudio (`fase-6-mappers-ext`) · marco M6
- [ ] F6-01 mix de expansão na amostra + áudio 5B (FME-7)
- [ ] F6-02 VRC6
- [ ] F6-03 Namco 163
- [ ] F6-04 MMC5 (+ split, atributo estendido, áudio)

## F7 — Performance e tamanho (`fase-7-perf`) · marco M7: metas do pior caso
- [ ] F7-01 bench de referência numa ROM de jogo
- [ ] F7-02 otimizações medidas (bancos CHR sem despacho, ring de samples, cache de glifos, overlay 1×)
- [ ] F7-03 wasm < 2 MB gzip; 60 fps no Chrome do g56
- [ ] F7-04 frame skip adaptativo + buffer de áudio maior

## Registro de decisões
- 2026-09-02: plano mestre aprovado (8 fases). Tarefa por tarefa; commits locais; push no fim; web antes de Android.
- 2026-09-02: tabela de testes vive em `crates/rnfe-core/src/testing/list.rs` (compartilhada por `cargo test` e `status`).
