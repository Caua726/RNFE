use pixels::{Pixels, SurfaceTexture};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

#[derive(Debug, Default)]
pub struct Janela {
    window: Option<Window>,
    frame: u64,
}

impl ApplicationHandler for Janela {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = WindowAttributes::default()
                .with_title("RNFE")
                .with_inner_size(winit::dpi::PhysicalSize::new(800, 600));
            self.window = match event_loop.create_window(attrs) {
                Ok(w) => Some(w),
                Err(err) => {
                    eprintln!("erro criando janela: {err}");
                    event_loop.exit();
                    return;
                }
            };
            if let Some(w) = self.window.as_ref() {
                w.request_redraw(); // força o primeiro draw
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        // só trata eventos da nossa janela
        let is_our_window = self.window.as_ref().map(Window::id) == Some(window_id);

        eprintln!("[event] {event:?}");

        match event {
            WindowEvent::CloseRequested if is_our_window => {
                event_loop.exit();
            }
            WindowEvent::Resized(_new_size) if is_our_window => {
                if let Some(w) = self.window.as_ref() {
                    w.request_redraw();
                }
            }
            WindowEvent::RedrawRequested if is_our_window => {
                let window = self.window.as_ref().expect("redraw without window");
                window.pre_present_notify();

                // cria Pixels só para este frame (evita lifetime/auto-ref)
                let size = window.inner_size();
                if size.width == 0 || size.height == 0 {
                    return; // janela minimizada etc.
                }
                let surf = SurfaceTexture::new(size.width, size.height, window);
                let mut pixels =
                    Pixels::new(size.width, size.height, surf).expect("falha ao criar Pixels");

                let frame = pixels.frame_mut();

                pixels.render().expect("render falhou");
            }
            _ => {}
        }
    }
}

pub fn run() -> Result<(), winit::error::EventLoopError> {
    let event_loop: EventLoop<()> = EventLoop::new()?;
    let mut app = Janela::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}
