# RNFE — plano de trabalho

Regras: uma tarefa por vez; cada tarefa termina com `scripts/check.sh` verde, `docs/STATUS.md` regenerado
e um commit `[ID] área: descrição`. A verdade sobre o progresso é `cargo test -p rnfe-core`
(`VERIFIED_LINES` do nestest + tabela `Pass/KnownFail` das ROMs blargg); os checkboxes só espelham.
Branch por fase (`fase-N-nome`), merge `--no-ff` em `main` no marco. Push só no fim.

Retomar de qualquer lugar:
```sh
git status -sb && git log --oneline -5
sed -n '/^## Onde parei
Tarefa: 2ª rodada de agentes (UX, "o que construir", código gui/frontend, Android entregues; problemas reais e compatibilidade ainda rodando; 8 caíram no limite da API) + lote de UX/Android aplicado
Estado: check + clippy wasm/android → commit em main → CI → APK (testar no g56: diagonal preta, "Continuar · jogo", START+SELECT, tela apaga nos menus)
Próximo: relatórios pendentes (problemas reais, compatibilidade); do agente Android: erro de GPU visível + retry com GL, áudio na taxa nativa (48 kHz) com foco de áudio, overlay em meia resolução, insets do recorte por JNI, exclusão de gesto ≤ 200 dp, predictive back (targetSdk 36); do UX: miniaturas nos slots, tela "Controles", indicador de rolagem, selo TURBO/rewind, GPU error na web; do "o que construir": .zip, filtros de vídeo, paleta, run-ahead, remapeamento, cheats, PWA offline

## Linha de base
F0-06, 02/09/2026, moto g56 5G, rustc 1.98, release (lto thin, cgu 1 no core):
- `bench --rom other/nestest.nes --frames 3000`: **299 fps · 3,34 ms/frame · 5,0× tempo real · VmHWM 3,5 MB**
- `bench --rom other/BladeBuster.nes --frames 1500`: 280 fps · 3,57 ms/frame · VmHWM 4,3 MB
- `bench --rom mmc3_test_2/1-clocking.nes --frames 1500`: 317 fps · 3,16 ms/frame
- `cargo test -p rnfe-core` (21 suítes blargg + nestest + snapshots + rom_parse): ~80 s
- `status` (120 ROMs, 6 threads): 38 s · `cargo build -p rnfe-core` debug: 9,5 s · release: pico de RAM do rustc 276 MB (`scripts/peak-rss.sh`)
- ROMs: 30 Pass / 90 KnownFail · nestest 5004/8991 · 10 snapshots
- `rnfe-tty --headless` BladeBuster: 260 fps; interativo em pty 40×120: 60 fps estáveis, ~2 KB/frame desenhado

M1, 02/09/2026 (mesma máquina/perfil):
- `bench --rom other/BladeBuster.nes --frames 600`: **405 fps · 2,47 ms/frame · VmHWM 4,6 MB** (F1-04 janela de pixels −17 %, F1-05 bus por acesso −30 %)
- `bench --rom other/nestest.nes --frames 600`: 470 fps · 2,13 ms/frame
- nestest 8991/8991 · `grep unsafe crates/rnfe-core` vazio

M2, 02/09/2026:
- ROMs: 104 Pass / 12 KnownFail (11 são MMC3/A12 → F3-02; 1 é a leitura dupla de `$2007`)
- `bench` BladeBuster: **3,8 ms/frame** (mínimo de 3 corridas) — piorou ~1,3 ms desde M1 com a APU/PPU por ciclo (frame counter, sprites, open bus). Fica para F7-01/F7-02 medir por subsistema; o celular também oscila de frequência (medições variam 3,2–5,0 ms).
- `framebuffer()` agora converte de índices sob demanda (61 KB×2 em vez de 184 KB de RGBA na PPU); `framebuffer_indexed()` + `ppu::PALETTE_RGBA` para paleta na GPU

M3, 02/09/2026:
- ROMs: **115 Pass / 1 KnownFail** (só a leitura dupla de `$2007`); mmc3_test_2 6/6, mmc3_irq_tests 6/6; 7 testes sintéticos de mapper; 6 de save state; 3 de save; 2 de rewind
- `bench` BladeBuster (MMC3): **3,16 ms/frame** (mínimo de 3), VmHWM 4,4 MB — igual/melhor que M2 apesar do A12 por acesso e do `MapperKind` por `match`
- save state ≈ 15 KB (23 KB com CHR RAM); `cargo test` com `--features rnfe-core/serde`: ~35 s (blargg 25 s)

M4, 02/09/2026 (CI ubuntu): 5 jobs verdes no 1º push; wasm 3,82 MB (1,43 MB gzip, meta < 2 MB); https://caua726.github.io/RNFE/ no ar.

M5, 02/09/2026 (CI ubuntu): job android verde (cargo-ndk -P 26 + gradle); APK de debug 7,9 MB (a .so tinha símbolos; `CARGO_PROFILE_RELEASE_STRIP=symbols` no job a partir de agora — meta < 5 MB fica para F7-03).

M6, 03/09/2026: 18 mappers (+5, 19, 24, 26); áudio de expansão pelo `Apu::clock(expansion)` só na amostra; nametables pelo mapper (`nt_source`); MMC5 sem split vertical, validado só por teste sintético + telas dos mmc5test/exram (sem imagem de referência). 12 testes sintéticos de mapper.

M7, 03/09/2026 (g56, release, mínimo de 3 corridas de 1 500 frames):
- BladeBuster (MMC3): **2,41 ms/frame** (415 fps, 6,9× tempo real) — F0 era 3,57 (1,48×; a meta de 2× não foi atingida), M2 3,8, M3 3,16
- nestest: 3,12 ms · mmc3_test_2/1-clocking: 2,52 ms · VmHWM 4,2 MB
- `bench --profile` BladeBuster: PPU 1,35 ms (55 %), APU 0,20 (8 %), CPU+bus ≈ 0,93 (37 %)
- wasm 3,8 MB / 1,43 MB gzip; APK de debug com strip: artefato de 2,4 MB no CI (era 7,9 MB sem strip)
- Metas do pior caso: ≥ 125 fps num núcleo ✅ (415), RSS < 50 MB ✅ (4,2), wasm < 2 MB gzip ✅, APK < 5 MB ✅ (2,4)

## F8 — APK polido (`fase-8-apk`) · marco M8: APK assinado, com ícone, menu de toque, ajustes e instalação pelo CI
- [x] F8-01 identidade: ícone adaptativo (vetor), versão 0.2.0, keystore de release (gerada no celular, secrets no GitHub), `release.yml` assina com ela; `scripts/apk.sh` baixa e instala o APK do último CI
- [x] F8-02 menu de toque: tela de pausa com botões grandes (abrir ROM, recentes, reset, save/load state, rewind, turbo, ajustes); ROMs recentes guardadas no Storage (`roms/<hash>.nes` + `recent`)
- [x] F8-03 som/desempenho no celular: tela sempre ligada, fila de áudio e latência medidas pelo overlay, orientação livre, opção de escala inteira/esticar
- [x] F8-04 tamanho: LTO fat + 1 cgu no build Android, medir; sem gilrs no Android se pesar
- [x] F8-05 acessibilidade: tamanho e opacidade dos botões de toque, texto maior, alto contraste, vibração ao tocar (JNI), controles sempre visíveis; tudo persistido em `config`

M8, 03/09/2026: APK assinado (release) de 4,7 MB — .so de 4,8 MB sem compressão no pacote, 2,3 MB zipado (LTO fat, 1 cgu, strip, sem gilrs) com ícone, menus de toque, ajustes persistidos e recentes; keystore em `~/.rnfe-release/` do celular (senha em `secrets.env`) e nos secrets do repositório.

## Alvos
- Tier 1 (60 fps + som): Web (Chrome/Firefox/Safari; Chrome Android 10+; Safari iOS 16+), Android arm64 8+, Desktop Linux x86_64 + Windows x86_64.
- Tier 2 (deve funcionar): Termux tty, Raspberry Pi, GPU só-OpenGL.
- Pior caso de referência: aparelho mais fraco que o moto g56 (2 GB / Cortex-A53). No g56: ≤ 8 ms/frame do núcleo em 1 núcleo, RSS < 50 MB nativo / < 100 MB web, APK < 5 MB, wasm < 2 MB gzip.

## F0 — Fundação (`fase-0-fundacao`) · marco M0: `cargo test` verde no celular e no CI; STATUS.md; SMB1 boota; jogo visível no tty
- [x] F0-01 repo: clone, identidade, branch, lixo versionado removido, .gitignore, sem `cargo-features`, LICENSE, PLAN.md
- [x] F0-02 [S] workspace: crates/rnfe-core (9 módulos + mappers), crates/rnfe-gui (display/ui), crates/rnfe-desktop; perfis; `default-members` sem wgpu; .cargo/config.toml
- [x] F0-03 core: `Cartridge::from_bytes` + `RomError`; `println!`→`log`; `diagnostic_report()->String`; `Buttons`; `Nes::new(cart)`; `run_frame/step_instruction/peek/set_controller/drain_audio/framebuffer` (RGBA8)
- [x] F0-04 NROM: máscara 32 KB (`& 0x3FFF` extra) + PRG RAM `$6000-$7FFF`
- [x] F0-05 harness: `src/testing/{list,runner}.rs`, `tests/blargg.rs` (KnownFail que passa = falha), `tests/nestest.rs` (`VERIFIED_LINES=5004`), `scripts/fetch-roms.sh`
- [x] F0-06 bins: `status` → docs/STATUS.md; `bench` (fps, ns/frame, VmHWM, `--profile`); `tests/snapshots.rs`; linha de base
- [x] F0-07 qualidade: `scripts/check.sh`, `scripts/peak-rss.sh`, rustfmt.toml, `[workspace.lints]`; clippy -D warnings limpo
- [x] F0-08 rnfe-frontend mínimo (FramePacer, InputState) + rnfe-tty (half-blocks, stty, panic hook)
- [x] F0-09 CI: ci.yml (core, desktop-check, bench)
- [x] F0-10 README real + CHANGELOG

## F1 — CPU exata (`fase-1-cpu`) · marco M1: nestest 8991/8991 · instr_test-v5 16/16 · instr_timing 2/2 · instr_misc 4/4 · cpu_dummy_* · cpu_interrupts 5/5
- [x] F1-01 tabela: 22 NOPs com modo certo, `EB`=SBC IMM, `97/B7`=4 ciclos → VERIFIED_LINES 8991
- [x] F1-02 ilegais: ANC ALR ARR XAA LAX# AXS LAS SHA TAS SHY SHX + JAM
- [x] F1-03 wrapping_add; BRK (B só no byte empilhado, I após push); reset I=1/SP-=3; NMI 7 ciclos
- [x] F1-04 PPU: mux/paleta/sprite-0/shift só na janela visível (61 440 dots)
- [x] F1-05 [S] bus ciclo a ciclo: `tick_pre/tick_post` por acesso; RAM antes do cartucho; OAM DMA inline; `cart_ptr` removido; `#![forbid(unsafe_code)]`; mirroring por acesso
- [x] F1-06 dummy reads/writes — feito dentro de F1-05 (modos `AbsXW/AbsYW/IndYW`, RMW read/write/write, dummy de ZPX/IZX/pilha)
- [x] F1-07 [S] interrupções — feito dentro de F1-05 (`Bus::nmi_line/irq_line`, borda de NMI, polling no penúltimo ciclo, hijack, quirk do branch; debugger gated por `enabled`)
- Pendências de M1 que dependem de F2: `instr_misc/04` e `cpu_dummy_writes_ppumem` (open bus, F2-02/F2-05); `cpu_interrupts 1–5` (a fonte de IRQ do teste é o frame counter da APU, F2-01)

## F2 — APU e PPU exatos (`fase-2-apu-ppu`) · marco M2
- [x] F2-01 APU frame counter em ciclos de CPU + IRQ flag/inhibit + `$4015` R + delay `$4017`
- [x] F2-02 noise/DMC por ciclo; DMC IRQ; DMA do DMC com stall de 4; `$4000-$4013` open bus
- [x] F2-03 length counter halt/reload ordering
- [x] F2-04 VBL/NMI exatos: (241,1), prevent_vbl, supressão, odd-frame em (261,339)
- [x] F2-05 open bus da PPU; `$2004`/`$2007` durante render; CPU open bus
- [x] F2-06 avaliação de sprites em lote no dot 65 com `overflow_dot`; sprite 0 hit x<255
- [x] F2-07 paleta 512 cores: framebuffer por índice + LUT; grayscale/emphasis; backdrop

## F3 — Mappers, saves, rewind (`fase-3-mappers-saves`) · marco M3
- [x] F3-01 [S] trait `Mapper` novo + `enum MapperKind`; NES 2.0; bateria; four-screen; bounds; CHR só via cartucho
- [x] F3-02 `A12Watcher` na PPU + MMC3 rev B (reload flag, ack nivelado, `$A001`); rev A por submapper
- [x] F3-03 MMC1: writes consecutivos, SUROM/SOROM/SXROM, PRG RAM enable, CHR RAM banking
- [x] F3-04 FME-7 IRQ + `$6000`
- [x] F3-05 persistência: trait `Storage`; `FsStorage` + `SaveManager`; `.sav`
- [x] F3-06 save states: feature `serde` (postcard), `static LOOKUP`, header RNFS, round-trip test
- [x] F3-07 rewind

## F4 — Web (`fase-4-web`) · marco M4: Pages jogável no celular com toque e som
- [x] F4-01 [S] rnfe-gui: app/gpu(async)/audio/platform/ui; `Arc<Window>`; `about_to_wait` + WaitUntil; ring SPSC; web_time; UserEvent; áudio lazy
- [x] F4-02 wasm32: rnfe-web, index.html, WebStorage, webgl fallback, rfd async, Trunk.toml; toolchain no celular
- [x] F4-03 toque: TouchLayout/InputState multi-touch; overlay em ui.rs
- [x] F4-04 CI web + deploy Pages
- [x] F4-05 gamepads

## F5 — Android (`fase-5-android`) · marco M5: APK arm64 roda SMB1 com toque e som
- [x] F5-01 rnfe-android cdylib (android-activity, FsStorage, oboe, SAF)
- [x] F5-02 release.yml: Linux, Windows, APK, web em tag
- [x] F5-03 README Android

## F6 — Mappers estendidos e áudio (`fase-6-mappers-ext`) · marco M6
- [x] F6-01 mix de expansão na amostra + áudio 5B (FME-7)
- [x] F6-02 VRC6
- [x] F6-03 Namco 163
- [x] F6-04 MMC5 (+ split, atributo estendido, áudio)

## F7 — Performance e tamanho (`fase-7-perf`) · marco M7: metas do pior caso
- [x] F7-01 bench de referência numa ROM de jogo
- [x] F7-02 otimizações medidas (bancos CHR sem despacho, ring de samples, cache de glifos, overlay 1×)
- [x] F7-03 wasm < 2 MB gzip; 60 fps no Chrome do g56
- [x] F7-04 frame skip adaptativo + buffer de áudio maior

## Registro de decisões
- 2026-09-02: plano mestre aprovado (8 fases). Tarefa por tarefa; commits locais; push no fim; web antes de Android.
- 2026-09-02: tabela de testes vive em `crates/rnfe-core/src/testing/list.rs` (compartilhada por `cargo test` e `status`).
