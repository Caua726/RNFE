# RNFE — Rust Nintendo Famicom Emulator

[![CI](https://github.com/Caua726/RNFE/actions/workflows/ci.yml/badge.svg)](https://github.com/Caua726/RNFE/actions/workflows/ci.yml)

Emulador de NES/Famicom escrito em Rust, do zero: CPU 6502, PPU e APU exatas ao ciclo, 40 mappers com áudio de expansão (VRC6, N163, MMC5, 5B),
save states e rewind, com um núcleo **sem dependências** que roda em qualquer lugar — navegador,
desktop, Android e terminal.

**Jogar agora:** <https://caua726.github.io/RNFE/> — abra uma ROM `.nes`, toque na tela (celular)
ou use o teclado. Saves e save states ficam no `localStorage` do navegador.

> *A NES/Famicom emulator in Rust. The core has zero dependencies and is verified against
> nestest and the blargg test ROMs on every commit — see [docs/STATUS.md](docs/STATUS.md).*

## Estado

O progresso é medido por ROMs de teste, não por sensação: [docs/STATUS.md](docs/STATUS.md) é
gerado a cada marco e lista as 116 ROMs (blargg e outras) com o resultado atual — 115 passam —
e o `nestest` é comparado instrução a instrução (8 991 linhas) com o log de referência.

Numa varredura de 857 ROMs dos EUA de um pack (600 frames cada, `examples/sweep`), 815 mostram imagem,
27 ainda estão em intros longas (todas as conferidas renderizam depois), 4 usam mappers que faltam
(15, 41, 8, 228) e 3 travam executando lixo (dumps ruins). Klax (RAMBO-1), After Burner (Sunsoft-4) e
NWC 1990 (NES-EVENT) chegam ao jogo. Castlevania III (MMC5), Akumajou Densetsu
(VRC6), Gimmick! (5B), Splatterhouse (N163), Batman: Return of the Joker (FME-7) e Punch-Out!! (MMC2)
chegam à fase 1 com HUD correto.

O plano de trabalho, com o que vem a seguir, está em [PLAN.md](PLAN.md).

## Rodar

```sh
# web — https://caua726.github.io/RNFE/ (build local: rustup target add wasm32-unknown-unknown; cargo install trunk)
trunk serve                    # http://127.0.0.1:8080/RNFE/

# desktop (Linux/Windows/macOS) — janela com wgpu, som e gamepad
cargo run -p rnfe-desktop --release -- caminho/para/jogo.nes

# terminal (inclusive Termux no Android) — sem dependências além do próprio Rust
cargo run -p rnfe-tty --release -- caminho/para/jogo.nes

# Android (arm64, 8.0+): APK em Releases (tag v*) ou no artefato "rnfe-android-apk" de cada CI (scripts/apk.sh baixa e instala no Termux);
# build local: cargo install cargo-ndk; cargo ndk -t arm64-v8a -P 26 -o android/app/src/main/jniLibs build -p rnfe-android --release
#              cd android && gradle assembleDebug   (precisa do Android SDK + NDK)

# só medir velocidade do núcleo
cargo run -p rnfe-core --release --bin bench -- --rom jogo.nes --frames 3000
```

No desktop, `cargo run -p rnfe-desktop` sem argumentos abre a janela com o botão **Open ROM**.
Saves de bateria (`.sav`) e save states ficam em `~/.local/share/rnfe` (`$RNFE_DATA_DIR` muda).

### Android

Instale o APK (Releases ou o artefato do CI; o Android pede para permitir "fontes desconhecidas"),
abra o app e toque em **Open ROM**: o seletor de arquivos do sistema abre e qualquer `.nes` do
aparelho serve (Downloads, Drive…). Os controles de toque aparecem no primeiro toque; um gamepad
Bluetooth também funciona. Saves e save states ficam na pasta interna do app.

Publicação na web: o job `web` do CI faz o `trunk build` e o job `pages` publica em GitHub Pages
a cada push em `main` — no repositório, uma vez, ative **Settings → Pages → Source: GitHub Actions**.

### Exemplos

```sh
cargo run -p rnfe-core --release --features serde --example embed -- jogo.nes   # embutir o núcleo: vídeo, áudio, save state
cargo run -p rnfe-core --release --example run_rom -- -v nestest                  # veredito de uma ROM de teste
cargo run -p rnfe-core --release --example ppu_dump -- jogo.nes 120 tela.png      # estado da PPU + captura
cargo run -p rnfe-frontend --example menu_layout -- 1080 2340 settings            # menus de toque em ASCII
cargo run -p rnfe-core --release --example sweep -- pasta/de/roms 600 4 > out.csv # varredura de compatibilidade
```

### Testes

```sh
bash scripts/fetch-roms.sh     # baixa as ROMs de teste (clone esparso de nes-test-roms)
cargo test                     # núcleo + frontend: nestest, 21 suítes blargg, mappers, save states, snapshots
bash scripts/check.sh          # fmt + clippy -D warnings + testes (o mesmo que o CI roda)
```

`cargo test`/`cargo build` na raiz tocam só o núcleo, o frontend comum e o tty — o desktop e a web
(wgpu/winit) compilam com `-p rnfe-desktop` / `--target wasm32-unknown-unknown` e são verificados no CI.

## Controles

| NES | Desktop / Web (teclado) | Toque (web, celular) | Gamepad | Terminal |
|---|---|---|---|---|
| D-pad | setas ou WASD | d-pad na tela | d-pad ou analógico esquerdo | setas ou WASD |
| A / B | Z / X | A / B | Sul·Leste / Oeste·Norte | Z / X |
| Start / Select | Enter / Tab | START / SELECT | Start / Select | Enter / Tab (ou C) |
| Reset | R | menu → Reset | — | R |
| Pausa / menu | Esc | MENU (ou START+SELECT) | Mode/Guide ou Start+Select | — |
| Jogador 2 | I J K L · O / U · . / , | — | 2º gamepad | — |
| Zapper (Duck Hunt) | clique na imagem | toque na imagem | — | — |
| Abrir ROM | O ou clique | toque em Open ROM | — | — |
| Save / load state | F5 / F7 (slot 1) | menu → Salvar / Carregar (3 slots) | — | 1 / 2 |
| Captura de tela | F12 ou menu (PNG em `shots/`) | — | — | — |
| Rewind (segurar) | Backspace | — | — | Backspace |
| Turbo (segurar) | Espaço | — | — | — |
| Sair | Esc duas vezes (sem ROM) | — | — | Q ou Ctrl-C |
| Debug | F3 overlay · F4 cobertura · F6 diagnóstico · F9 trace · F11 tela cheia | — | — | — |

Os controles de toque aparecem no primeiro toque; em retrato a imagem fica em cima e os botões
embaixo, em paisagem ficam nas laterais. O botão **MENU** (ou Esc) abre o menu de pausa: salvar e
carregar state (3 slots), voltar 5 s, turbo, reset (com confirmação), abrir outra ROM e
**Ajustes** com sliders — filtro de vídeo (nítido/suave/scanlines), Zapper, tamanho e opacidade dos botões de toque, botões sempre visíveis, tamanho
do texto, alto contraste, vibração, escala inteira e volume. Tudo fica guardado, junto com a lista
de ROMs recentes (reabrem sem o seletor).

## Mappers

| # | Nome | Jogos típicos |
|---|---|---|
| 0 | NROM | Super Mario Bros., Donkey Kong |
| 1 | MMC1 | Zelda, Metroid, Mega Man 2 |
| 2 | UxROM | Castlevania, Contra, Mega Man |
| 3 | CNROM | Arkanoid, Gradius |
| 4 | MMC3 | Super Mario Bros. 3, Kirby's Adventure |
| 5 | MMC5 (+ áudio, divisão vertical) | Castlevania III, Just Breed |
| 6 / 8 / 17 | Copiadores FFE (F4/F3/F8) | dumps de copiador (87 de 109 do pack rodam) |
| 7 | AxROM | Battletoads, Marble Madness |
| 9 / 10 | MMC2 / MMC4 | Punch-Out!!, Fire Emblem, Famicom Wars |
| 11 | Color Dreams | — |
| 13 | CPROM | Videomation |
| 19 | Namco 163 (+ áudio wavetable) | Rolling Thunder, Megami Tensei II |
| 21 / 22 / 23 / 25 | VRC2 / VRC4 | Contra (J), Gradius II, Ganbare Goemon 2 |
| 24 / 26 | VRC6 (+ áudio) | Akumajou Densetsu, Madara |
| 28 | Action 53 | multicarts homebrew |
| 30 | UNROM 512 | homebrew (Twin Dragons, Lizard) |
| 34 | BNROM / NINA-001 | Deadly Towers, Impossible Mission 2 |
| 64 | RAMBO-1 | Klax, Skull & Crossbones |
| 65 | Irem H3001 | Daiku no Gen-san 2, Spartan X 2 |
| 66 | GxROM | Dragon Ball, Doraemon |
| 67 | Sunsoft-3 | Mickey Mousecapade, Fantasy Zone II |
| 68 | Sunsoft-4 | After Burner, Maharaja |
| 69 | FME-7 / Sunsoft 5B (+ áudio) | Batman: Return of the Joker, Gimmick! |
| 71 | Camerica | Micro Machines |
| 79 / 113 | NINA-03/06 | Krazy Kreatures, Tiles of Fate |
| 105 | NES-EVENT | Nintendo World Championships 1990 |
| 118 / 119 | TxSROM / TQROM (MMC3) | Armadillo, Pin Bot, High Speed |
| 206 | DxROM / Namco 118 | Gauntlet, Karnov |
| 227 | 1200-in-1 | multicarts |
| 232 | Camerica Quattro | Quattro Adventure/Arcade/Sports |

## Estrutura

```
crates/rnfe-core       núcleo: cpu6502, ppu, apu, bus, cartridge, mappers, storage, state — sem dependências
                       (feature `serde`: save states com serde + postcard)
crates/rnfe-frontend   comum a todo frontend (só `log`): pacer, input, toque, anel de áudio, menus,
                       ajustes e recentes, FsStorage, SaveManager (.sav), Rewind
crates/rnfe-tty        frontend de terminal (half-blocks, cor 24-bit)
crates/rnfe-gui        frontend gráfico (winit + wgpu + cpal + gilrs), o mesmo código no desktop e na web
crates/rnfe-desktop    binário de desktop (fino)
crates/rnfe-web        binário wasm32 + index.html (Trunk); saves no localStorage
crates/rnfe-android    biblioteca nativa (android_main + JNI para o seletor de arquivos)
android/               projeto Gradle mínimo (NativeActivity) que embala a biblioteca no APK
scripts/               fetch-roms.sh · check.sh · peak-rss.sh
docs/STATUS.md         resultado das ROMs de teste (gerado)
PLAN.md                plano de trabalho e ponto onde parou
```

## Licença

MIT — veja [LICENSE](LICENSE).
