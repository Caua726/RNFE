use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use winit::application::ApplicationHandler;
use winit::event::{WindowEvent, ElementState};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};
use wgpu::util::DeviceExt;

use crate::{font, nes::Nes};

const NES_WIDTH: u32 = 256;
const NES_HEIGHT: u32 = 240;

const SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
    // Fullscreen quad com 6 vertices (2 triangulos)
    var positions = array<vec2<f32>, 6>(
        vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(1.0, 1.0),
        vec2(-1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0),
    );
    var uvs = array<vec2<f32>, 6>(
        vec2(0.0, 1.0), vec2(1.0, 1.0), vec2(1.0, 0.0),
        vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(0.0, 0.0),
    );
    var out: VertexOutput;
    out.pos = vec4(positions[idx], 0.0, 1.0);
    out.uv = uvs[idx];
    return out;
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, in.uv);
}
"#;

struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    texture: wgpu::Texture,
}

impl GpuState {
    fn new(window: &'static Window) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window).expect("surface");
        let adapter = pollster::block_on(
            instance.request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
        ).expect("adapter");

        let (device, queue) = pollster::block_on(
            adapter.request_device(&wgpu::DeviceDescriptor::default(), None)
        ).expect("device");

        let surface_caps = surface.get_capabilities(&adapter);
        let format = surface_caps.formats[0];

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Textura NES 256x240 RGBA
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("nes_screen"),
            size: wgpu::Extent3d { width: NES_WIDTH, height: NES_HEIGHT, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let tex_view = texture.create_view(&Default::default());

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&tex_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_main"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        GpuState { surface, device, queue, config, pipeline, bind_group, texture }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
    }

    fn render(&mut self, pixels: &[u8]) {
        // Upload pixels pra textura
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(NES_WIDTH * 4),
                rows_per_image: Some(NES_HEIGHT),
            },
            wgpu::Extent3d { width: NES_WIDTH, height: NES_HEIGHT, depth_or_array_layers: 1 },
        );

        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..6, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }
}

pub struct App {
    win: Option<&'static Window>,
    gpu: Option<GpuState>,
    nes: Option<Box<Nes>>,
    framebuffer: Vec<u8>,
    audio_buffer: Arc<Mutex<VecDeque<f32>>>,
    _audio_stream: Option<cpal::Stream>,
}

impl App {
    pub fn new() -> Self {
        Self {
            win: None, gpu: None, nes: None,
            framebuffer: vec![0u8; (NES_WIDTH * NES_HEIGHT * 4) as usize],
            audio_buffer: Arc::new(Mutex::new(VecDeque::new())),
            _audio_stream: None,
        }
    }

    pub fn new_with_nes(mut nes: Box<Nes>) -> Self {
        let audio_buffer = Arc::new(Mutex::new(VecDeque::with_capacity(8192)));
        let stream = Self::init_audio(audio_buffer.clone(), &mut nes);
        Self {
            win: None, gpu: None, nes: Some(nes),
            framebuffer: vec![0u8; (NES_WIDTH * NES_HEIGHT * 4) as usize],
            audio_buffer,
            _audio_stream: stream,
        }
    }

    fn init_audio(buffer: Arc<Mutex<VecDeque<f32>>>, nes: &mut Nes) -> Option<cpal::Stream> {
        let host = cpal::default_host();
        let device = host.default_output_device()?;
        let config = device.default_output_config().ok()?;
        let sample_rate = config.sample_rate();
        nes.bus.apu.set_sample_rate(sample_rate as f32);

        let stream = device.build_output_stream(
            &config.into(),
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut buf = buffer.lock().unwrap();
                for sample in data.iter_mut() {
                    *sample = buf.pop_front().unwrap_or(0.0);
                }
            },
            |err| eprintln!("Audio error: {}", err),
            None,
        ).ok()?;

        stream.play().ok()?;
        Some(stream)
    }

    fn draw(&mut self) {
        let Some(gpu) = self.gpu.as_mut() else { return };

        if let Some(ref mut nes) = self.nes {
            loop {
                nes.clock();
                if nes.bus.ppu.frame_complete {
                    nes.bus.ppu.frame_complete = false;
                    break;
                }
            }

            // Enviar samples de audio
            if !nes.bus.apu.sample_buffer.is_empty() {
                if let Ok(mut buf) = self.audio_buffer.lock() {
                    for &s in &nes.bus.apu.sample_buffer {
                        buf.push_back(s);
                    }
                    // Limitar buffer pra não acumular latência
                    while buf.len() > 4096 {
                        buf.pop_front();
                    }
                }
                nes.bus.apu.sample_buffer.clear();
            }

            // PPU screen (RGB) -> framebuffer (RGBA)
            for i in 0..(NES_WIDTH * NES_HEIGHT) as usize {
                let color = nes.bus.ppu.screen[i];
                let fb_idx = i * 4;
                self.framebuffer[fb_idx] = color[0];
                self.framebuffer[fb_idx + 1] = color[1];
                self.framebuffer[fb_idx + 2] = color[2];
                self.framebuffer[fb_idx + 3] = 255;
            }
        } else {
            // Tela preta com texto
            self.framebuffer.fill(0);
            // Setar alpha
            for i in 0..(NES_WIDTH * NES_HEIGHT) as usize {
                self.framebuffer[i * 4 + 3] = 255;
            }
            font::draw_str(&mut self.framebuffer, NES_WIDTH, NES_HEIGHT, "HELLO WORLD", 2, [255,255,255,255]);
        }

        gpu.render(&self.framebuffer);
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.win.is_some() { return; }
        let attrs = WindowAttributes::default()
            .with_title("RNFE - NES Emulator")
            .with_inner_size(winit::dpi::PhysicalSize::new(768, 720));
        let owned = el.create_window(attrs).expect("window");
        let win: &'static Window = Box::leak(Box::new(owned));
        self.win = Some(win);
        self.gpu = Some(GpuState::new(win));
        win.request_redraw();
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, ev: WindowEvent) {
        let Some(w) = self.win else { return };
        if w.id() != id { return; }
        match ev {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(s) => {
                if let Some(gpu) = self.gpu.as_mut() { gpu.resize(s.width, s.height); }
                w.request_redraw();
            }
            WindowEvent::RedrawRequested => self.draw(),
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(ref mut nes) = self.nes {
                    let pressed = event.state == ElementState::Pressed;
                    let bit = match event.physical_key {
                        PhysicalKey::Code(KeyCode::KeyZ)       => Some(0x80),
                        PhysicalKey::Code(KeyCode::KeyX)       => Some(0x40),
                        PhysicalKey::Code(KeyCode::Tab)        => Some(0x20),
                        PhysicalKey::Code(KeyCode::Enter)      => Some(0x10),
                        PhysicalKey::Code(KeyCode::ArrowUp)    => Some(0x08),
                        PhysicalKey::Code(KeyCode::ArrowDown)  => Some(0x04),
                        PhysicalKey::Code(KeyCode::ArrowLeft)  => Some(0x02),
                        PhysicalKey::Code(KeyCode::ArrowRight) => Some(0x01),
                        _ => None,
                    };
                    if let Some(b) = bit {
                        if pressed { nes.bus.controller[0] |= b; } else { nes.bus.controller[0] &= !b; }
                    }
                    if pressed {
                        match event.physical_key {
                            PhysicalKey::Code(KeyCode::Escape) => el.exit(),
                            PhysicalKey::Code(KeyCode::KeyR) => { nes.reset(); println!("NES Reset!"); }
                            _ => {}
                        }
                    }
                } else if event.state == ElementState::Pressed {
                    if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key { el.exit(); }
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
