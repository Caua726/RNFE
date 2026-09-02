# RNFE — Rust Nintendo Famicom Emulator

[![CI](https://github.com/Caua726/RNFE/actions/workflows/ci.yml/badge.svg)](https://github.com/Caua726/RNFE/actions/workflows/ci.yml)

Emulador de NES/Famicom escrito em Rust, do zero: CPU 6502, PPU, APU e 14 mappers, com um
núcleo **sem dependências** que roda em qualquer lugar — desktop, terminal, e (em breve) web e Android.

> *A NES/Famicom emulator in Rust. The core has zero dependencies and is verified against
> nestest and the blargg test ROMs on every commit — see [docs/STATUS.md](docs/STATUS.md).*

## Estado

O progresso é medido por ROMs de teste, não por sensação: [docs/STATUS.md](docs/STATUS.md) é
gerado a cada marco e lista as 120 ROMs (blargg e outras) com o resultado atual, e o
`nestest` é comparado instrução a instrução com o log de referência.

O plano de trabalho, com o que vem a seguir, está em [PLAN.md](PLAN.md).

## Rodar

```sh
# desktop (Linux/Windows/macOS) — janela com wgpu + som
cargo run -p rnfe-desktop --release -- caminho/para/jogo.nes

# terminal (inclusive Termux no Android) — sem dependências além do próprio Rust
cargo run -p rnfe-tty --release -- caminho/para/jogo.nes

# só medir velocidade do núcleo
cargo run -p rnfe-core --release --bin bench -- --rom jogo.nes --frames 3000
```

No desktop, `cargo run -p rnfe-desktop` sem argumentos abre a janela com o botão **Open ROM**.

### Testes

```sh
bash scripts/fetch-roms.sh     # baixa as ROMs de teste (clone esparso de nes-test-roms)
cargo test                     # núcleo + frontend: nestest, 21 suítes blargg, snapshots
bash scripts/check.sh          # fmt + clippy -D warnings + testes (o mesmo que o CI roda)
```

`cargo test`/`cargo build` na raiz tocam só o núcleo, o frontend comum e o tty — o desktop
(wgpu/winit) compila com `-p rnfe-desktop` e é verificado no CI.

## Controles

| NES | Desktop | Terminal |
|---|---|---|
| D-pad | setas | setas ou WASD |
| A / B | Z / X | Z / X |
| Start / Select | Enter / Tab | Enter / Tab (ou C) |
| Reset | R | R |
| Pausa / menu | Esc | — |
| Abrir ROM | O | — |
| Sair | Esc (sem ROM) | Q ou Ctrl-C |
| Debug | F3 overlay · F4 cobertura · F5 trace · F6 diagnóstico · F11 tela cheia | — |

## Mappers

| # | Nome | Jogos típicos |
|---|---|---|
| 0 | NROM | Super Mario Bros., Donkey Kong |
| 1 | MMC1 | Zelda, Metroid, Mega Man 2 |
| 2 | UxROM | Castlevania, Contra, Mega Man |
| 3 | CNROM | Arkanoid, Gradius |
| 4 | MMC3 | Super Mario Bros. 3, Kirby's Adventure |
| 7 | AxROM | Battletoads, Marble Madness |
| 9 | MMC2 | Punch-Out!! |
| 11 | Color Dreams | — |
| 34 | BNROM | Deadly Towers |
| 66 | GxROM | Dragon Ball, Doraemon |
| 69 | FME-7 | Batman: Return of the Joker, Gimmick! |
| 71 | Camerica | Micro Machines |
| 206 | DxROM / Namco 118 | Gauntlet, Karnov |
| 227 | 1200-in-1 | multicarts |

## Estrutura

```
crates/rnfe-core       núcleo: cpu6502, ppu, apu, bus, cartridge, mappers — sem dependências
crates/rnfe-frontend   cadência de frames e input, comuns a todo frontend — sem dependências
crates/rnfe-tty        frontend de terminal (half-blocks, cor 24-bit)
crates/rnfe-gui        frontend gráfico (winit + wgpu + cpal), compartilhado por desktop e web
crates/rnfe-desktop    binário de desktop
scripts/               fetch-roms.sh · check.sh · peak-rss.sh
docs/STATUS.md         resultado das ROMs de teste (gerado)
PLAN.md                plano de trabalho e ponto onde parou
```

## Licença

MIT — veja [LICENSE](LICENSE).
