# Changelog

Uma linha por tarefa fechada. IDs referem-se ao [PLAN.md](PLAN.md).

## Não publicado — F8 APK polido

- Revisão por 9 agentes (03/09): 3 focados no Android e 6 abertos (bugs, código mal feito, melhorias, UI, UX, riscos). Aplicado: leitura da ROM do SAF fora da thread principal com limite de 8 MB; processo encerrado ao sair (o winit não recria o laço: o app "não abria" de novo); `configChanges` completo (recriar a Activity travava a UI thread); panic no logcat; layout de toque refeito (MENU fora do HUD, START/SELECT fora da imagem e do gesto de borda, exclusão de gesto no d-pad/A/B, rótulos escuros, visíveis por padrão); eixos de gamepad Bluetooth (`dispatchGenericMotionEvent`) e botões; "Abrir com" `.nes`; menus com rolagem, navegação por teclado/gamepad, seções nos Ajustes, "Abrindo…", erros legíveis por 6 s, R e remoção de recente com confirmação, cartucho do título proporcional; overlay só reenviado à GPU quando muda (era 10 MB/frame); superfície sem sRGB (botões translúcidos escureciam); overscan opcional; áudio pré-carregado, decaimento em underrun, controle fino de taxa, I16/U16; toasts que somem nos menus; multi-toque nos menus; `Config` gravado só ao soltar; recentes sem regravar a ROM e sem lotar o localStorage; backup na nuvem sem ROMs; web instalável (manifest, `100dvh`).
- Núcleo: DMC sem underflow e cancelado por `$4015`, DMC interrompe o DMA de OAM, `load_state` mantém a tela (rewind mostrava cinza) e a taxa de amostragem, reset preserva VRAM/OAM/paleta, passa-alta de 90/440 Hz corretos, MMC2/DxROM/MMC3 sem underflow com PRG pequena, N163 lê `$4800` com auto-incremento, NES 2.0 exponencial limitado.
- Mappers novos: 13 (CPROM), 79/113 (NINA-03/06), 118/119 (TxSROM/TQROM), 232 (Camerica Quattro) — 24 no total. `examples/sweep` varre uma pasta de ROMs; nos 857 títulos dos EUA de um pack (600 frames): 815 com imagem, 27 em intros longas, 12 sem mapper (64, 65, 68, 15, 41, 8, 228), 3 dumps ruins que executam lixo.
- CI: cargo-ndk 4.1.2, trunk 0.21.14, Gradle 8.9 fixados; ROMs de teste num commit fixo; snapshots/save state/rewind honram `RNFE_REQUIRE_ROMS`; Release exige a keystore. `FsStorage` com `.tmp` por arquivo; nomes de ROM sem tab/nova linha.

- UI/UX (03/09): menus com cantos arredondados, sombra e tema com cor de destaque; ação ao soltar o dedo (item aceso enquanto pressionado, arrastar para fora cancela); ajustes com sliders arrastáveis e toggles em pílula; pausa com "Continuar" em destaque, linha de ações rápidas (Salvar · Carregar · Voltar 5 s), tempo de jogo e reset com confirmação em dois toques; **3 slots de save state** com indicador salvo/vazio (F5/F7 usam o slot 1); tela inicial com marca; estado vazio nos recentes. Modelo do menu com `ItemKind` e sliders testado (24 testes no frontend).

- fix (revisão por agentes, 03/09): interface pelo DPI da janela (`scale_factor`: fonte ≈ 16 dp, linhas ≥ 48 dp no celular); overlay com alpha premultiplicado (os botões de toque ficavam quase invisíveis); overlay de 10 MB só redesenhado/reenviado quando muda; botão/gesto Voltar do Android (tecla lógica `BrowserBack`) = Esc; fila de áudio mono (latência caía pela metade errada: ~220 ms → ~110 ms); área de toque calculada pela borda real da imagem (8:7); limites `downlevel` da GPU + `on_uncaptured_error`; áudio recriado se o stream morrer e solto em `suspended`; pausa ao perder o foco; cursor limpo após o toque (item "aceso" errado); exceção JNI pendente limpa; tema com `shortEdges` (sem faixa preta no recorte da câmera).

- F8-05 Acessibilidade: ajustes de tamanho e opacidade dos botões de toque, botões sempre visíveis, tamanho do texto, alto contraste (tema próprio, contorno escuro nos controles), vibração ao tocar (Android via JNI), escala inteira, volume — persistidos em `config`. Exemplos `embed` (núcleo) e `menu_layout` (menus em ASCII); 6 testes novos (config, recentes, layout dos menus em 4 tamanhos, hit-test, ajustes).
- F8-03 Android: tela sempre ligada, permissão de vibração; fila de áudio visível no overlay de debug.
- F8-02 Menus de toque (modelo puro em `rnfe_frontend::menu`, desenho no gui): início (Abrir ROM, Recentes, Ajustes), pausa (continuar, salvar/carregar state, voltar 5 s, turbo, reset, outra ROM, ajustes, sair no desktop), ajustes, recentes (as ROMs abertas ficam guardadas no Storage e reabrem sem o seletor).
- F8-01 APK: ícone adaptativo, versão 0.2.0, assinatura de release com keystore gerada no celular e guardada em secrets do GitHub (CI e Release assinam), `scripts/apk.sh` baixa e instala o APK do último CI.

## Não publicado — F7 Performance e tamanho

- F7-04 Frontend: fila de áudio de 100 ms na web e no Android (50 ms no desktop); contador de frames pulados (o laço já desenha só uma vez por chamada quando atrasa) no overlay de debug.
- F7-03 Web medido no CI/Pages: wasm de 3,8 MB, 1,43 MB gzip (meta < 2 MB) com `wasm-opt -Oz`; `panic = "abort"` e LTO thin já vinham do perfil release.
- F7-02 Núcleo, cada passo medido por A/B intercalado no g56 (BladeBuster, mín. de 5): buffer de sprites por scanline em vez de 8 shifters por dot e `apply_pending` da APU só após escrita (-8,5 %); cache de bancos de CHR e de nametables no cartucho, recalculado só em escrita da CPU (-7 %); `Ppu::step` por faixa de dots e linha de IRQ do mapper em cache (-16 %). Total: 4,33 → 3,05 ms/frame no dia da medição (-30 %); snapshots, blargg e save states idênticos.
- F7-01 `bench --profile`: PPU e APU isoladas para estimar a fatia de cada subsistema (feature `profile-no-ppu` para medir CPU+bus); linha de base registrada no PLAN.

## Não publicado — F6 Mappers estendidos e áudio de expansão

- F6-04 MMC5 (mapper 5): PRG em 4 modos com ROM/RAM por banco e PRG RAM até 64 KB protegida por `$5102/$5103`, CHR em 4 modos com conjuntos A/B (sprites 8×16 usam A para sprites e B para o fundo — a PPU informa a fase da busca via `CartData`), ExRAM (nametable, atributos estendidos com banco de 4 KB por tile, RAM da CPU), fill mode, multiplicador, IRQ por scanline detectada com 3 leituras iguais de nametable (`$5204` com in-frame e ack na leitura via `on_cpu_read`), 2 pulsos + PCM. Sem a divisão vertical.
- F6-03 Namco 163 (mapper 19): CHR/nametables por registrador (CIRAM ou CHR), IRQ de 15 bits, RAM interna por `$4800/$F800`, proteção da PRG RAM, áudio wavetable de 1–8 canais. Infra `nt_source`/`nt_write` no trait `Mapper` (nametables pelo cartucho) e busca de NT no dot 1 na PPU.
- F6-02 VRC6 (mappers 24/26): bancos, mirroring, PRG RAM, IRQ scanline/ciclo, 2 pulsos + serra, `$9003`.
- F6-01 `Apu::clock(expansion)`: o áudio do cartucho entra só na amostra; Sunsoft 5B (3 tons do YM2149) no FME-7.

## Não publicado — F5 Android

- F5-03 README: instalar o APK, abrir ROM pelo seletor do sistema, controles de toque/gamepad, onde ficam os saves.
- F5-02 CI: correções após o 1º push (NDK via `$ANDROID_NDK_LATEST_HOME`, `cargo ndk -P 26` por causa do libaaudio, strip de símbolos na .so). Job `android` (cargo-ndk arm64-v8a + gradle `assembleDebug`, APK como artefato a cada push) e `release.yml` em tag `v*` (Linux x86_64, Windows x86_64, APK arm64, `dist` web anexados à Release).
- F5-01 Android: crate `rnfe-android` (cdylib, `android_main` com winit `android-native-activity`, `FsStorage` na pasta interna, log no logcat) + projeto Gradle mínimo em `android/` (`MainActivity extends NativeActivity` abre o SAF e devolve a ROM por JNI → `UserEvent::RomLoaded`). No gui: `Launch::picker`, `run_android`, `suspended` solta GPU/janela, imagem alinhada ao topo em retrato; `rfd` só fora do Android.

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
