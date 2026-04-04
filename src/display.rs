use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::{WindowEvent, ElementState, MouseButton};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Fullscreen, Window, WindowAttributes, WindowId};
use wgpu::util::DeviceExt;

use crate::{font, nes::Nes, ui::Ui};

const NES_WIDTH: u32 = 256;
const NES_HEIGHT: u32 = 240;

// NES aspect ratio: 256 pixels * 8/7 per pixel = ~292.57 visible width
// Aspect ratio = (256 * 8/7) / 240 = ~1.219
const NES_ASPECT: f32 = (256.0 * 8.0 / 7.0) / 240.0;

const SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(2) var<uniform> scale: vec2<f32>;

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 6>(
        vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(1.0, 1.0),
        vec2(-1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0),
    );
    var uvs = array<vec2<f32>, 6>(
        vec2(0.0, 1.0), vec2(1.0, 1.0), vec2(1.0, 0.0),
        vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(0.0, 0.0),
    );
    var out: VertexOutput;
    out.pos = vec4(positions[idx] * scale, 0.0, 1.0);
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
    // NES rendering (256x240 com aspect ratio)
    bind_group: wgpu::BindGroup,
    texture: wgpu::Texture,
    scale_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    overlay_pipeline: wgpu::RenderPipeline,
    // Menu rendering (resolução da janela, sem scaling)
    menu_texture: wgpu::Texture,
    menu_bind_group: wgpu::BindGroup,
    menu_scale_buffer: wgpu::Buffer,
    menu_w: u32,
    menu_h: u32,
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

        // Scale uniform pra aspect ratio
        let scale = Self::calc_scale(size.width, size.height);
        let scale_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scale"),
            contents: bytemuck::cast_slice(&scale),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
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
                wgpu::BindGroupEntry { binding: 2, resource: scale_buffer.as_entire_binding() },
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

        // Pipeline com alpha blending pra overlays
        let overlay_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("overlay"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_main"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Menu texture (resolução da janela)
        let menu_w = size.width.max(1);
        let menu_h = size.height.max(1);
        let (menu_texture, menu_bind_group, menu_scale_buffer) =
            Self::create_menu_resources(&device, &queue, &bind_group_layout, &sampler, menu_w, menu_h);

        GpuState {
            surface, device, queue, config, pipeline, bind_group, texture,
            scale_buffer, bind_group_layout, sampler, overlay_pipeline,
            menu_texture, menu_bind_group, menu_scale_buffer, menu_w, menu_h,
        }
    }

    fn create_menu_resources(
        device: &wgpu::Device, queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout, sampler: &wgpu::Sampler,
        w: u32, h: u32,
    ) -> (wgpu::Texture, wgpu::BindGroup, wgpu::Buffer) {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("menu"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        // Scale = 1.0, 1.0 (sem aspect ratio correction)
        let scale_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("menu_scale"),
            contents: bytemuck::cast_slice(&[1.0f32, 1.0f32]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None, layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: scale_buf.as_entire_binding() },
            ],
        });
        (tex, bg, scale_buf)
    }

    fn calc_scale(win_w: u32, win_h: u32) -> [f32; 2] {
        let win_aspect = win_w as f32 / win_h.max(1) as f32;
        if win_aspect > NES_ASPECT {
            // Janela mais larga que NES - pillarbox
            [NES_ASPECT / win_aspect, 1.0]
        } else {
            // Janela mais alta - letterbox
            [1.0, win_aspect / NES_ASPECT]
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);

        let scale = Self::calc_scale(width, height);
        self.queue.write_buffer(&self.scale_buffer, 0, bytemuck::cast_slice(&scale));

        // Recriar menu texture na nova resolução
        let (mt, mbg, msb) = Self::create_menu_resources(
            &self.device, &self.queue, &self.bind_group_layout, &self.sampler, width.max(1), height.max(1));
        self.menu_texture = mt;
        self.menu_bind_group = mbg;
        self.menu_scale_buffer = msb;
        self.menu_w = width.max(1);
        self.menu_h = height.max(1);
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

    fn render_menu(&mut self, pixels: &[u8]) {
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.menu_texture, mip_level: 0,
                origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.menu_w * 4),
                rows_per_image: Some(self.menu_h),
            },
            wgpu::Extent3d { width: self.menu_w, height: self.menu_h, depth_or_array_layers: 1 },
        );

        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => { self.surface.configure(&self.device, &self.config); return; }
        };
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view, resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.menu_bind_group, &[]);
            pass.draw(0..6, 0..1);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }

    // Renderiza overlay com alpha blending por cima do frame atual
    fn render_overlay(&mut self, pixels: &[u8]) {
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.menu_texture, mip_level: 0,
                origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.menu_w * 4),
                rows_per_image: Some(self.menu_h),
            },
            wgpu::Extent3d { width: self.menu_w, height: self.menu_h, depth_or_array_layers: 1 },
        );

        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => { self.surface.configure(&self.device, &self.config); return; }
        };
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view, resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(&self.overlay_pipeline);
            pass.set_bind_group(0, &self.menu_bind_group, &[]);
            pass.draw(0..6, 0..1);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }
}

const FRAME_DURATION: Duration = Duration::from_nanos(16_639_267); // ~60.0988 Hz (NTSC)

pub struct App {
    win: Option<&'static Window>,
    gpu: Option<GpuState>,
    nes: Option<Box<Nes>>,
    framebuffer: Vec<u8>,
    audio_buffer: Arc<Mutex<VecDeque<f32>>>,
    _audio_stream: Option<cpal::Stream>,
    last_frame: Instant,
    cursor_pos: (f64, f64),
    ui: Ui,
    menu_fb: Vec<u8>,
    paused: bool,
    debug_overlay: bool,
    fps_counter: u32,
    fps_timer: Instant,
    fps_display: u32,
}

impl App {
    pub fn new() -> Self {
        Self {
            win: None, gpu: None, nes: None,
            framebuffer: vec![0u8; (NES_WIDTH * NES_HEIGHT * 4) as usize],
            audio_buffer: Arc::new(Mutex::new(VecDeque::new())),
            _audio_stream: None,
            last_frame: Instant::now(),
            cursor_pos: (0.0, 0.0),
            ui: Ui::new(),
            menu_fb: Vec::new(),
            paused: false,
            debug_overlay: false,
            fps_counter: 0,
            fps_timer: Instant::now(),
            fps_display: 0,
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
            last_frame: Instant::now(),
            cursor_pos: (0.0, 0.0),
            ui: Ui::new(),
            menu_fb: Vec::new(),
            paused: false,
            debug_overlay: false,
            fps_counter: 0,
            fps_timer: Instant::now(),
            fps_display: 0,
        }
    }

    fn init_audio(buffer: Arc<Mutex<VecDeque<f32>>>, nes: &mut Nes) -> Option<cpal::Stream> {
        let host = cpal::default_host();
        let device = host.default_output_device()?;
        let config = device.default_output_config().ok()?;
        let sample_rate = config.sample_rate();
        let channels = config.channels() as usize;
        nes.bus.apu.set_sample_rate(sample_rate as f32);

        let stream = device.build_output_stream(
            &config.into(),
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut buf = buffer.lock().unwrap();
                // Preencher frames (cada frame tem N canais)
                for frame in data.chunks_mut(channels) {
                    let sample = buf.pop_front().unwrap_or(0.0);
                    // Mesmo sample pra todos os canais (mono -> stereo)
                    for ch in frame.iter_mut() {
                        *ch = sample;
                    }
                }
            },
            |err| eprintln!("Audio error: {}", err),
            None,
        ).ok()?;

        stream.play().ok()?;
        Some(stream)
    }

    fn open_rom(&mut self) {
        if let Some(path) = crate::pick_rom() {
            if let Some(mut new_nes) = crate::load_rom(&path) {
                // Configurar audio
                if self._audio_stream.is_none() {
                    self._audio_stream = Self::init_audio(self.audio_buffer.clone(), &mut new_nes);
                } else if let Some(ref old_nes) = self.nes {
                    new_nes.bus.apu.set_sample_rate(old_nes.bus.apu.sample_rate);
                }
                self.nes = Some(new_nes);
                if let Ok(mut buf) = self.audio_buffer.lock() {
                    buf.clear();
                }
            }
        }
    }

    fn draw(&mut self) {
        let Some(gpu) = self.gpu.as_mut() else { return };

        if let Some(ref mut nes) = self.nes {
            if self.paused {
                // Tela de pausa com menu
                let mw = gpu.menu_w;
                let mh = gpu.menu_h;
                let size = (mw * mh * 4) as usize;
                self.menu_fb.resize(size, 0);

                for i in 0..(mw * mh) as usize {
                    let idx = i * 4;
                    self.menu_fb[idx] = 8;
                    self.menu_fb[idx + 1] = 8;
                    self.menu_fb[idx + 2] = 14;
                    self.menu_fb[idx + 3] = 255;
                }

                let mx = self.cursor_pos.0 as i32;
                let my = self.cursor_pos.1 as i32;

                self.ui.draw_text_centered(&mut self.menu_fb, mw, mh, "PAUSED", 36.0, (mh as f32 * 0.35) as i32, [200, 200, 200, 255]);
                self.ui.draw_text_centered(&mut self.menu_fb, mw, mh, "ESC to resume", 14.0, (mh as f32 * 0.35) as i32 + 48, [70, 70, 70, 255]);

                self.ui.draw_menubar(&mut self.menu_fb, mw, mh, mx, my);

                gpu.render_menu(&self.menu_fb);
                return;
            }

            // Frame timing
            let elapsed = self.last_frame.elapsed();
            if elapsed < FRAME_DURATION {
                std::thread::sleep(FRAME_DURATION - elapsed);
            }
            self.last_frame = Instant::now();

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

            // FPS counter
            self.fps_counter += 1;
            if self.fps_timer.elapsed() >= Duration::from_secs(1) {
                self.fps_display = self.fps_counter;
                self.fps_counter = 0;
                self.fps_timer = Instant::now();
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

            gpu.render(&self.framebuffer);

            // Debug overlay na resolução da janela (F3)
            if self.debug_overlay {
                let mw = gpu.menu_w;
                let mh = gpu.menu_h;
                let size = (mw * mh * 4) as usize;
                self.menu_fb.resize(size, 0);
                self.menu_fb.fill(0);

                let bg = [0u8, 0, 0, 160];
                let sz = 14.0f32;
                let green = [0u8, 255, 80, 255];
                let gray = [200u8, 200, 200, 255];

                self.ui.fill_rect_pub(&mut self.menu_fb, mw, mh, 4, 4, 360, 70, bg);

                let fps_text = format!("FPS: {}", self.fps_display);
                let cpu_text = format!(
                    "PC:{:04X}  A:{:02X}  X:{:02X}  Y:{:02X}  SP:{:02X}  P:{:02X}",
                    nes.cpu.pc, nes.cpu.a, nes.cpu.x, nes.cpu.y, nes.cpu.stkp, nes.cpu.status
                );
                let ppu_text = format!(
                    "SL:{}  CYC:{}  CTRL:{:02X}  MASK:{:02X}  STAT:{:02X}",
                    nes.bus.ppu.scanline, nes.bus.ppu.cycle,
                    nes.bus.ppu.control, nes.bus.ppu.mask, nes.bus.ppu.status
                );

                self.ui.draw_text(&mut self.menu_fb, mw, mh, &fps_text, sz, 12, 10, green);
                self.ui.draw_text(&mut self.menu_fb, mw, mh, &cpu_text, sz, 12, 28, gray);
                self.ui.draw_text(&mut self.menu_fb, mw, mh, &ppu_text, sz, 12, 46, gray);

                gpu.render_overlay(&self.menu_fb);
            }
            return;
        } else {
            // Tela inicial na resolução da janela
            let mw = gpu.menu_w;
            let mh = gpu.menu_h;
            let size = (mw * mh * 4) as usize;
            self.menu_fb.resize(size, 0);

            for i in 0..(mw * mh) as usize {
                let idx = i * 4;
                self.menu_fb[idx] = 12;
                self.menu_fb[idx + 1] = 12;
                self.menu_fb[idx + 2] = 16;
                self.menu_fb[idx + 3] = 255;
            }

            let cx = mw as i32 / 2;
            let title_y = (mh as f32 * 0.30) as i32;

            self.ui.draw_text_centered(&mut self.menu_fb, mw, mh, "RNFE", 56.0, title_y, [220, 220, 220, 255]);
            self.ui.draw_text_centered(&mut self.menu_fb, mw, mh, "NES Emulator", 16.0, title_y + 65, [80, 80, 80, 255]);

            let mx = self.cursor_pos.0 as i32;
            let my = self.cursor_pos.1 as i32;

            let btn_y = (mh as f32 * 0.58) as i32;
            let (bx, by, bw, bh) = self.ui.button_rect("Open ROM", 18.0, cx, btn_y);
            let hover = mx >= bx && mx < bx + bw && my >= by && my < by + bh;

            if hover {
                self.ui.draw_button(&mut self.menu_fb, mw, mh, "Open ROM", 18.0, cx, btn_y,
                    [255, 255, 255, 255], [150, 150, 150, 255]);
            } else {
                self.ui.draw_button(&mut self.menu_fb, mw, mh, "Open ROM", 18.0, cx, btn_y,
                    [180, 180, 180, 255], [80, 80, 80, 255]);
            }

            self.ui.draw_text_centered(&mut self.menu_fb, mw, mh, "press O", 12.0, btn_y + 38, [50, 50, 50, 255]);

            self.ui.draw_menubar(&mut self.menu_fb, mw, mh, mx, my);

            gpu.render_menu(&self.menu_fb);
        }
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
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_pos = (position.x, position.y);
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                let mx = self.cursor_pos.0 as i32;
                let my = self.cursor_pos.1 as i32;

                if self.nes.is_none() || self.paused {
                    let mut action = self.ui.handle_click(mx, my);
                    // Botão central só na tela inicial
                    if action == crate::ui::MenuAction::None && self.nes.is_none() {
                        let win_size = w.inner_size();
                        let cx = win_size.width as i32 / 2;
                        let btn_y = (win_size.height as f32 * 0.58) as i32;
                        let (bx, by, bw, bh) = self.ui.button_rect("Open ROM", 18.0, cx, btn_y);
                        if mx >= bx && mx < bx + bw && my >= by && my < by + bh {
                            action = crate::ui::MenuAction::OpenRom;
                        }
                    }
                    match action {
                        crate::ui::MenuAction::OpenRom => { self.paused = false; self.open_rom(); },
                        crate::ui::MenuAction::Reset => {
                            if let Some(ref mut nes) = self.nes { nes.reset(); }
                            self.paused = false;
                        },
                        crate::ui::MenuAction::Quit => el.exit(),
                        crate::ui::MenuAction::None => {},
                    }
                }
            }
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
                            PhysicalKey::Code(KeyCode::Escape) => { self.paused = !self.paused; },
                            PhysicalKey::Code(KeyCode::KeyR) => { nes.reset(); println!("NES Reset!"); }
                            PhysicalKey::Code(KeyCode::KeyO) => self.open_rom(),
                            PhysicalKey::Code(KeyCode::F3) => { self.debug_overlay = !self.debug_overlay; }
                            PhysicalKey::Code(KeyCode::F11) => {
                                if w.fullscreen().is_some() {
                                    w.set_fullscreen(None);
                                } else {
                                    w.set_fullscreen(Some(Fullscreen::Borderless(None)));
                                }
                            }
                            _ => {}
                        }
                    }
                } else if event.state == ElementState::Pressed {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::Escape) => el.exit(),
                        PhysicalKey::Code(KeyCode::KeyO) => self.open_rom(),
                        PhysicalKey::Code(KeyCode::F11) => {
                            if w.fullscreen().is_some() {
                                w.set_fullscreen(None);
                            } else {
                                w.set_fullscreen(Some(Fullscreen::Borderless(None)));
                            }
                        }
                        _ => {}
                    }
                }
            },
            _ => {}
        }

        w.request_redraw();
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
