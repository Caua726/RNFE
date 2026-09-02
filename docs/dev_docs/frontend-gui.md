# Frontend gráfico (F4)

`rnfe-gui` é um só código para desktop e web. O que difere por plataforma está em `platform.rs`.

## Laço

- `resumed` cria a janela (`Arc<Window>`, sem `Box::leak`). No desktop a GPU é criada na hora
  (`pollster::block_on`); na web `GpuState::new` roda como futuro e chega por
  `UserEvent::GpuReady` — enquanto isso a tela inicial não desenha nada.
- `about_to_wait` é o coração: `FramePacer::frames_due(now)` diz quantos frames emular (com
  catch-up limitado), roda-os, empurra o áudio para o anel e pede um redraw; depois agenda
  `ControlFlow::WaitUntil(próximo vencimento)`. Não há `thread::sleep` nem laço ocupado. Na web
  o `WaitUntil` vira `setTimeout` e o redraw vira `requestAnimationFrame`.
- `RedrawRequested` só desenha: `nes.framebuffer()` (RGBA, convertido sob demanda) + overlay.

## Áudio

`AudioRing` (em `rnfe-frontend`) é um anel SPSC com `AtomicU32` por amostra — sem `unsafe`,
sem `Mutex` no callback do cpal. Underrun repete a última amostra. O laço apara o anel a
~50 ms para a latência não crescer. O stream só é criado no primeiro gesto do usuário
(navegadores exigem; no desktop é inofensivo) e a APU recebe a taxa real do dispositivo.

## Entrada

`InputState` (teclado) ∪ `TouchState` (multi-toque com `TouchLayout::hit`) ∪ gamepad (gilrs,
botões + analógico esquerdo com zona morta de 0,5) → `set_controller` uma vez por frame.
O `TouchLayout` é recalculado a cada `Resized` e desenhado só depois do primeiro toque.

## Arquivos e saves

- ROM: `rfd::AsyncFileDialog` num futuro (thread no desktop, `spawn_local` na web) →
  `UserEvent::RomLoaded { name, bytes }`.
- `Storage`: `FsStorage` no desktop, `WebStorage` (localStorage + base64) na web. As chaves
  são as mesmas (`sav/<hash>.sav`, `state/<hash>/1.rnfs`), então o `SaveManager` e os save
  states não sabem em que plataforma estão.

## Web

`crates/rnfe-web` é um `main` de três linhas + `index.html` (canvas `#rnfe`, `touch-action:
none`). `Trunk.toml` na raiz: `trunk build --release` gera `dist/` com `public_url = /RNFE/`.
wgpu com `webgl` como fallback e `Limits::downlevel_webgl2_defaults()`. O CI (`web`) roda clippy
em wasm32 e o trunk; o job `pages` publica.

Validação no celular: `cargo check/clippy --target wasm32-unknown-unknown -p rnfe-web` roda no
Termux (~7 min); o build com trunk e o teste no navegador ficam para o CI/Pages.
