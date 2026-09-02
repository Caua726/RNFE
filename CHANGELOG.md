# Changelog

Uma linha por tarefa fechada. IDs referem-se ao [PLAN.md](PLAN.md).

## Não publicado — F4 Web

- F4-05 Gamepad: gilrs (desktop) / Gamepad API (web) — botões, d-pad e analógico esquerdo.
- F4-04 CI: job `web` (clippy em wasm32 + `trunk build`, tamanho do wasm no resumo) e job `pages` (GitHub Pages a cada push em `main`); clippy do desktop passa a ser erro; README com web, controles de toque/gamepad, saves.
- F4-03 Toque: `TouchLayout` (retrato/paisagem, d-pad por ângulo com zona morta, A/B, Start/Select, MENU) e `TouchState` multi-toque em `rnfe-frontend` (testes de hit-test); overlay translúcido desenhado após o primeiro toque.
- F4-02 Web: crate `rnfe-web` (wasm32) + `index.html` + `Trunk.toml`; `WebStorage` (localStorage, base64 sem dependência); wgpu com fallback WebGL2; áudio criado no primeiro gesto; ROM por `rfd::AsyncFileDialog`. `cargo clippy --target wasm32-unknown-unknown` limpo no celular.
- F4-01 `rnfe-gui` reescrito em `app/gpu/audio/platform/ui`: `Arc<Window>` (sem `Box::leak`), GPU assíncrona, `about_to_wait` + `WaitUntil` com `FramePacer` (sem `thread::sleep`), `AudioRing` SPSC sem `unsafe`/`Mutex`, `set_controller` 1×/frame, `web_time`, `UserEvent{GpuReady,RomLoaded,RomLoadFailed}`, cache de glifos, `Launch { nes, storage }`; save states (F5/F7), rewind (Backspace), turbo (Espaço), menu com state. `rnfe-desktop` fino com `env_logger`.

## Não publicado — F3 Mappers, saves e rewind

- F3-07 Rewind: `rnfe_frontend::Rewind` (anel de states a cada 5 frames, limite de memória, padrão 32 MB); no tty Backspace volta no tempo, 1/2 salvam/carregam state em `state/<hash>/1.rnfs`.
- F3-06 Save states: feature `serde` no núcleo (serde + postcard, sem std neles) com `Nes::save_state()`/`load_state()` — header `RNFS` + versão + `rom_hash`, payload com CPU, PPU (sem framebuffer), APU (sem buffer de áudio), bus, PRG RAM, CHR RAM e mapper; ~15 KB (23 KB com CHR RAM). `tests/savestate.rs`: round-trip NROM/MMC1/MMC3/APU com hash de frame e ciclos iguais, `save(load(s)) == s`, rejeição de ROM/versão/lixo. `check.sh` e o CI testam com a feature.
- F3-05 Persistência: trait `Storage` (+ `MemoryStorage`) no núcleo; `FsStorage` (tmp+rename, `$RNFE_DATA_DIR` / `$XDG_DATA_HOME/rnfe` / `~/.local/share/rnfe`) e `SaveManager` (`sav/<hash>.sav`, grava no máximo a cada 300 frames se a PRG RAM mudou, flush ao trocar de ROM/sair) em `rnfe-frontend`; tty e desktop usam.
- F3-04 FME-7: contador de IRQ de 16 bits por ciclo de CPU (comandos $D/$E/$F, ack em $D) e janela `$6000` ROM/RAM; testes sintéticos em `tests/mappers.rs`.
- F3-03 MMC1: segunda escrita de um RMW ignorada (via `CartData::cpu_cycle`), SUROM/SOROM/SXROM (bit 4 do CHR escolhe 256 KB de PRG; bits 2-3 o banco de PRG RAM), bit 4 de `$E000` desliga a PRG RAM, mirroring do header até a 1ª escrita no control.
- F3-02 PPU: detector de borda de A12 com filtro (todo endereço no barramento, inclusive `v` fora do render e as buscas descartadas dos slots de sprite); busca de sprites nos dots 257–320 como no hardware (slots vazios buscam `$FF`). MMC3: latch/reload flag (`$C001`), ack por nível (`$E000`), `$A001` (PRG RAM enable/protect), revisão A por submapper 4.4. mmc3_test_2 6/6, mmc3_irq_tests 6/6 (`docs/dev_docs/a12-mmc3.md`).
- F3-01 Cartucho reescrito: header **NES 2.0** (mapper de 12 bits, submapper, tamanhos exponenciais, PRG/CHR RAM), bateria, four-screen (4 nametables na PPU), `RomHeader` público, `rom_hash()` FNV-1a, `prg_ram()`/`take_prg_ram_dirty()`; PRG/CHR preenchidos até potência de 2 (acesso por máscara, sem divisão nem bounds check); trait `Mapper` novo (`chr_offset`, `ppu_write`, `a12_rise`, `cpu_clock` sob demanda, `irq_pending` nível, `audio_output`) e `enum MapperKind` com despacho por `match` (sem `dyn`); os 14 mappers reescritos (DxROM com máscara correta de CHR, mapper 227 com banco fixo L, FME-7 com RAM/ROM em `$6000`).

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
