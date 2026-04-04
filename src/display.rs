use pixels::{Pixels, SurfaceTexture};
use winit::application::ApplicationHandler;
use winit::event::{WindowEvent, ElementState};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

use crate::{font, nes::Nes};

pub struct App {
    win: Option<&'static Window>,
    px: Option<Pixels<'static>>,
    nes: Option<Box<Nes>>,
}

impl App {
    pub fn new() -> Self {
        Self {
            win: None,
            px: None,
            nes: None
        }
    }

    pub fn new_with_nes(nes: Box<Nes>) -> Self {
        Self {
            win: None,
            px: None,
            nes: Some(nes),
        }
    }

    fn init_pixels(&mut self) {
        let Some(w) = self.win else { return };
        let s = w.inner_size();
        let surf = SurfaceTexture::new(s.width, s.height, w);
        self.px = Some(Pixels::new(256, 240, surf).expect("pixels"));
    }

    fn draw(&mut self, el: &ActiveEventLoop) {
        let (Some(w), Some(px)) = (self.win, self.px.as_mut()) else { return };
        let fb = px.frame_mut();

        if let Some(ref mut nes) = self.nes {
            // Rodar até completar o frame
            loop {
                nes.clock();
                if nes.bus.ppu.frame_complete {
                    nes.bus.ppu.frame_complete = false;
                    break;
                }
            }

            // Copiar tela da PPU pro framebuffer
            for i in 0..(256 * 240) {
                let color = nes.bus.ppu.screen[i];
                let fb_idx = i * 4;
                fb[fb_idx] = color[0];
                fb[fb_idx + 1] = color[1];
                fb[fb_idx + 2] = color[2];
                fb[fb_idx + 3] = 255;
            }
        } else {
            fb.chunks_exact_mut(4).for_each(|p| { p.copy_from_slice(&[0,0,0,0xFF]); });
            font::draw_str(fb, 256, 240, "HELLO WORLD", 2, [255,255,255,255]);
        }

        w.pre_present_notify();
        if px.render().is_err() { el.exit(); }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.win.is_some() { return; }
        let attrs = WindowAttributes::default()
            .with_title("RNFE - NES Emulator")
            .with_inner_size(winit::dpi::PhysicalSize::new(768, 720));
        let owned = el.create_window(attrs).expect("window");
        self.win = Some(Box::leak(Box::new(owned)));
        self.init_pixels();
        self.win.unwrap().request_redraw();
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, ev: WindowEvent) {
        let Some(w) = self.win else { return };
        if w.id() != id { return; }
        match ev {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(s) => {
                if let Some(p) = self.px.as_mut() { let _ = p.resize_surface(s.width, s.height); }
                w.request_redraw();
            }
            WindowEvent::RedrawRequested => self.draw(el),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::Escape) => el.exit(),
                        PhysicalKey::Code(KeyCode::KeyR) => {
                            if let Some(ref mut nes) = self.nes {
                                nes.reset();
                                println!("NES Reset!");
                            }
                        }
                        _ => {}
                    }
                }
            },
            _ => {}
        }

        if self.nes.is_some() {
            w.request_redraw();
        }
    }
}

pub fn run() -> Result<(), winit::error::EventLoopError> {
    let el: EventLoop<()> = EventLoop::new()?;
    el.run_app(&mut App::new())
}

pub fn run_with_nes(nes: Box<Nes>) -> Result<(), winit::error::EventLoopError> {
    let el: EventLoop<()> = EventLoop::new()?;
    el.run_app(&mut App::new_with_nes(nes))
}
