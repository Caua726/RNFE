//! wgpu: uma textura 256×240 para o NES (escalada com o aspecto 8:7) e uma textura na
//! resolução da janela para overlays (menus, toque, debug), com alpha.

use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

pub const NES_WIDTH: u32 = 256;
pub const NES_HEIGHT: u32 = 240;
/// Pixel do NES é 8:7 → imagem de 256 px "vale" 292,6 px de largura.
pub const NES_ASPECT: f32 = (256.0 * 8.0 / 7.0) / 240.0;

const SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(2) var<uniform> xform: vec4<f32>; // escala xy, deslocamento zw

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
    out.pos = vec4(positions[idx] * xform.xy + xform.zw, 0.0, 1.0);
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

struct Layer {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    scale_buffer: wgpu::Buffer,
    width: u32,
    height: u32,
}

pub struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    overlay_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    nes: Layer,
    overlay: Layer,
    /// Onde a imagem do NES ficou na janela (px): x, y, w, h — para o layout de toque.
    pub viewport: (f32, f32, f32, f32),
}

impl GpuState {
    pub async fn new(window: Arc<Window>) -> Result<GpuState, String> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone()).map_err(|e| format!("surface: {e}"))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                power_preference: wgpu::PowerPreference::LowPower,
                ..Default::default()
            })
            .await
            .ok_or("nenhum adaptador de GPU (WebGPU/WebGL indisponível?)")?;
        let limits = if cfg!(target_arch = "wasm32") {
            wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits())
        } else {
            wgpu::Limits::default().using_resolution(adapter.limits())
        };
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("rnfe"),
                    required_features: wgpu::Features::empty(),
                    required_limits: limits,
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            )
            .await
            .map_err(|e| format!("device: {e}"))?;
        log::info!("GPU: {} ({:?})", adapter.get_info().name, adapter.get_info().backend);

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
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
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let make_pipeline = |blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
        };
        let pipeline = make_pipeline(None);
        let overlay_pipeline = make_pipeline(Some(wgpu::BlendState::ALPHA_BLENDING));

        let scale = Self::calc_scale(config.width, config.height);
        let nes =
            Self::make_layer(&device, &bind_group_layout, &sampler, NES_WIDTH, NES_HEIGHT, scale, "nes");
        let overlay = Self::make_layer(
            &device,
            &bind_group_layout,
            &sampler,
            config.width,
            config.height,
            [1.0, 1.0, 0.0, 0.0],
            "overlay",
        );
        let viewport = Self::calc_viewport(config.width, config.height);
        Ok(GpuState {
            surface,
            device,
            queue,
            config,
            pipeline,
            overlay_pipeline,
            bind_group_layout,
            sampler,
            nes,
            overlay,
            viewport,
        })
    }

    fn make_layer(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        width: u32,
        height: u32,
        scale: [f32; 4],
        label: &str,
    ) -> Layer {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let scale_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(&scale),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: scale_buffer.as_entire_binding() },
            ],
        });
        Layer { texture, bind_group, scale_buffer, width, height }
    }

    /// Escala xy e deslocamento zw (clip space) do quad do NES. Em retrato a imagem gruda no
    /// topo (os controles de toque ocupam o resto); em paisagem, centralizada.
    fn calc_scale(win_w: u32, win_h: u32) -> [f32; 4] {
        let win_aspect = win_w as f32 / win_h.max(1) as f32;
        if win_aspect > NES_ASPECT {
            [NES_ASPECT / win_aspect, 1.0, 0.0, 0.0]
        } else {
            let sy = win_aspect / NES_ASPECT;
            [1.0, sy, 0.0, 1.0 - sy]
        }
    }

    /// Retângulo da imagem do NES na janela (px).
    fn calc_viewport(win_w: u32, win_h: u32) -> (f32, f32, f32, f32) {
        let [sx, sy, _, oy] = Self::calc_scale(win_w, win_h);
        let w = win_w as f32 * sx;
        let h = win_h as f32 * sy;
        let y = (1.0 - oy - sy) * 0.5 * win_h as f32;
        ((win_w as f32 - w) * 0.5, y, w, h)
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        let scale = Self::calc_scale(width, height);
        self.queue.write_buffer(&self.nes.scale_buffer, 0, bytemuck::cast_slice(&scale));
        self.overlay = Self::make_layer(
            &self.device,
            &self.bind_group_layout,
            &self.sampler,
            width,
            height,
            [1.0, 1.0, 0.0, 0.0],
            "overlay",
        );
        self.viewport = Self::calc_viewport(width, height);
    }

    fn upload(&self, layer: &Layer, rgba: &[u8]) {
        debug_assert_eq!(rgba.len(), (layer.width * layer.height * 4) as usize);
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &layer.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(layer.width * 4),
                rows_per_image: Some(layer.height),
            },
            wgpu::Extent3d { width: layer.width, height: layer.height, depth_or_array_layers: 1 },
        );
    }

    /// Desenha um frame: imagem do NES (se houver) e overlay RGBA na resolução da janela
    /// (se houver), com alpha.
    pub fn render(&mut self, nes_rgba: Option<&[u8]>, overlay_rgba: Option<&[u8]>) {
        if let Some(px) = nes_rgba {
            self.upload(&self.nes, px);
        }
        if let Some(px) = overlay_rgba {
            if px.len() == (self.overlay.width * self.overlay.height * 4) as usize {
                self.upload(&self.overlay, px);
            }
        }
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            Err(e) => {
                log::warn!("surface: {e:?}");
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
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            if nes_rgba.is_some() {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.nes.bind_group, &[]);
                pass.draw(0..6, 0..1);
            }
            if overlay_rgba.is_some() {
                pass.set_pipeline(&self.overlay_pipeline);
                pass.set_bind_group(0, &self.overlay.bind_group, &[]);
                pass.draw(0..6, 0..1);
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }
}
