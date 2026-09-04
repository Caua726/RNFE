//! wgpu: uma textura 256×240 para o NES (escalada com o aspecto 8:7) e uma textura na
//! resolução da janela para overlays (menus, toque, debug), com alpha.

use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

pub const NES_WIDTH: u32 = 256;
pub const NES_HEIGHT: u32 = 240;
/// Uniform do overlay: quad inteiro, textura inteira.
const OVERLAY_XFORM: [f32; 12] = [1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 1.0, 1.0];

const SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct Xform {
    scale: vec4<f32>,  // escala xy, deslocamento zw (clip space)
    uv: vec4<f32>,     // origem xy e tamanho zw da janela de textura (overscan)
    // x = filtro (0 nítido, 1 suave, 2 scanlines), y = pixels de tela por pixel do NES,
    // zw = tamanho da textura em pixels
    params: vec4<f32>,
};
@group(0) @binding(2) var<uniform> xform: Xform;

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
    // Quad como 2 triângulos: (0,1,2) = BL,BR,TR e (3,4,5) = BL,TR,TL. Os cantos saem de
    // aritmética com o índice, sem array local: drivers móveis (Mali/Adreno) já devolveram lixo
    // ao indexar um array<> de função com vertex_index e o 2º triângulo sumia (metade da tela
    // preta na diagonal).
    let t = idx % 3u;
    var c = t; // canto: 0 = BL, 1 = BR, 2 = TR, 3 = TL
    if idx >= 3u {
        c = t * 2u - u32(t == 2u);
    }
    let x = f32(((c + 1u) >> 1u) & 1u);
    let y = f32(c >> 1u);
    var out: VertexOutput;
    out.pos = vec4(vec2(x, y) * 2.0 - 1.0, 0.0, 1.0);
    out.pos = vec4(out.pos.xy * xform.scale.xy + xform.scale.zw, 0.0, 1.0);
    out.uv = xform.uv.xy + vec2(x, 1.0 - y) * xform.uv.zw;
    return out;
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Um sampler só (Nearest) e nenhum ramo em volta da amostragem: `textureSample` exige fluxo
    // uniforme e drivers móveis recusam o shader inteiro quando desconfiam — a tela fica preta.
    // A interpolação do modo "suave" é feita à mão com quatro leituras.
    let size = max(xform.params.zw, vec2(1.0, 1.0));
    let scale = max(xform.params.y, 1.0);
    let texel = in.uv * size;
    // sharp bilinear: a mistura acontece só na largura de um pixel de tela, na borda do texel
    let edge = clamp((fract(texel) - 0.5) * scale + 0.5, vec2(0.0), vec2(1.0));
    let p = floor(texel) + edge - 0.5;
    let b = floor(p);
    let f = p - b;
    let uv00 = (b + vec2(0.5, 0.5)) / size;
    let uv10 = (b + vec2(1.5, 0.5)) / size;
    let uv01 = (b + vec2(0.5, 1.5)) / size;
    let uv11 = (b + vec2(1.5, 1.5)) / size;
    let top = mix(textureSampleLevel(tex, samp, uv00, 0.0), textureSampleLevel(tex, samp, uv10, 0.0), f.x);
    let bot = mix(textureSampleLevel(tex, samp, uv01, 0.0), textureSampleLevel(tex, samp, uv11, 0.0), f.x);
    let soft = mix(top, bot, f.y);
    let nearest = textureSampleLevel(tex, samp, in.uv, 0.0);
    var c = select(soft, nearest, xform.params.x < 0.5);
    // scanlines: escurece a metade de baixo de cada linha do NES
    let line = fract(in.uv.y * size.y);
    c = select(c, c * (1.0 - 0.28 * smoothstep(0.45, 1.0, line)), xform.params.x > 1.5);
    return c;
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
    /// Múltiplos inteiros de 256×240 (pixels quadrados) em vez de preencher com aspecto 8:7.
    integer_scale: bool,
    /// 0 = nítido, 1 = suave, 2 = scanlines.
    filter: u8,
    /// Corta 8 linhas em cima e embaixo (área que as TVs não mostravam).
    overscan: bool,
    /// Janela minimizada (0×0): não há o que desenhar até o próximo `Resized`.
    minimized: bool,
    tex_format: wgpu::TextureFormat,
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
        // Só usamos 2 texturas e 1 uniform: os limites "downlevel" cabem em qualquer GLES3/Vulkan
        let limits = if cfg!(target_arch = "wasm32") {
            wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits())
        } else {
            wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits())
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
        // Erro de validação (ex.: swapchain na rotação) vira log, não panic
        device.on_uncaptured_error(Box::new(|e| log::error!("wgpu: {e}")));
        log::info!("GPU: {} ({:?})", adapter.get_info().name, adapter.get_info().backend);

        let caps = surface.get_capabilities(&adapter);
        // A paleta do NES e o overlay já são bytes sRGB compostos em gamma (ui::blend): sem sRGB
        // na superfície nem nas texturas eles passam intactos e o blend premultiplicado do
        // overlay acontece no mesmo espaço em que foi calculado.
        // superfície incompatível com o adaptador devolve listas vazias (indexar seria panic)
        let first = *caps.formats.first().ok_or("a GPU não suporta esta superfície")?;
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(first);
        let tex_format = if format.is_srgb() {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes.first().copied().unwrap_or(wgpu::CompositeAlphaMode::Auto),
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
        let overlay_pipeline = make_pipeline(Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING));

        let scale = Self::calc_xform(config.width, config.height, false, false, 0);
        let nes = Self::make_layer(
            &device,
            &bind_group_layout,
            &sampler,
            tex_format,
            NES_WIDTH,
            NES_HEIGHT,
            scale,
            "nes",
        );
        let overlay = Self::make_layer(
            &device,
            &bind_group_layout,
            &sampler,
            tex_format,
            config.width,
            config.height,
            OVERLAY_XFORM,
            "overlay",
        );
        let viewport = Self::calc_viewport(config.width, config.height, false, false);
        Ok(GpuState {
            integer_scale: false,
            filter: 0,
            overscan: false,
            minimized: false,
            tex_format,
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

    #[allow(clippy::too_many_arguments)]
    fn make_layer(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        scale: [f32; 12],
        label: &str,
    ) -> Layer {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
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

    /// Linhas visíveis com o overscan cortado.
    fn visible_lines(overscan: bool) -> f32 {
        if overscan { 224.0 } else { 240.0 }
    }

    /// Uniform do quad do NES: escala xy + deslocamento zw (clip space) e a janela de textura
    /// (overscan). Em retrato a imagem fica no alto com uma margem (recorte da câmera/barra de
    /// status) e os controles de toque ocupam o resto; em paisagem, centralizada.
    fn calc_xform(win_w: u32, win_h: u32, integer: bool, overscan: bool, filter: u8) -> [f32; 12] {
        let (w, h) = (win_w.max(1) as f32, win_h.max(1) as f32);
        let portrait = h > w;
        let lines = Self::visible_lines(overscan);
        let aspect = (256.0 * 8.0 / 7.0) / lines;
        let (sx, sy) = if integer {
            let k = (w / NES_WIDTH as f32).min(h / lines).floor().max(1.0);
            ((NES_WIDTH as f32 * k / w).min(1.0), (lines * k / h).min(1.0))
        } else {
            let win_aspect = w / h;
            if win_aspect > aspect { (aspect / win_aspect, 1.0) } else { (1.0, win_aspect / aspect) }
        };
        let oy = if portrait {
            let top = (h * 0.035).min((1.0 - sy) * h * 0.5); // margem no topo, se sobrar espaço
            1.0 - sy - 2.0 * top / h
        } else {
            0.0
        };
        let (v0, vh) = if overscan { (8.0 / 240.0, 224.0 / 240.0) } else { (0.0, 1.0) };
        // quantos pixels da tela cabem num pixel do NES (para o filtro suave)
        let ppx = (w * sx / NES_WIDTH as f32).max(1.0);
        [sx, sy, 0.0, oy, 0.0, v0, 1.0, vh, filter as f32, ppx, NES_WIDTH as f32, 240.0]
    }

    /// Retângulo da imagem do NES na janela (px).
    fn calc_viewport(win_w: u32, win_h: u32, integer: bool, overscan: bool) -> (f32, f32, f32, f32) {
        let [sx, sy, _, oy, ..] = Self::calc_xform(win_w, win_h, integer, overscan, 0);
        let w = win_w as f32 * sx;
        let h = win_h as f32 * sy;
        let y = (1.0 - oy - sy) * 0.5 * win_h as f32;
        ((win_w as f32 - w) * 0.5, y, w, h)
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// Escala inteira (pixels quadrados) e corte de overscan.
    pub fn set_video(&mut self, integer_scale: bool, overscan: bool, filter: u8) {
        if self.integer_scale != integer_scale || self.overscan != overscan || self.filter != filter {
            self.integer_scale = integer_scale;
            self.overscan = overscan;
            self.filter = filter;
            self.update_xform();
        }
    }

    fn update_xform(&mut self) {
        let x = Self::calc_xform(
            self.config.width,
            self.config.height,
            self.integer_scale,
            self.overscan,
            self.filter,
        );
        self.queue.write_buffer(&self.nes.scale_buffer, 0, bytemuck::cast_slice(&x));
        self.viewport =
            Self::calc_viewport(self.config.width, self.config.height, self.integer_scale, self.overscan);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            self.minimized = true;
            return;
        }
        self.minimized = false;
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.overlay = Self::make_layer(
            &self.device,
            &self.bind_group_layout,
            &self.sampler,
            self.tex_format,
            width,
            height,
            OVERLAY_XFORM,
            "overlay",
        );
        self.update_xform();
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

    /// Desenha um frame: imagem do NES (se houver) e o overlay (se `show_overlay`), com alpha
    /// premultiplicado. `overlay_rgba` só precisa vir quando o overlay mudou (upload de
    /// w×h×4 bytes). Devolve `false` se a superfície foi perdida (vale pedir outro redraw).
    pub fn render(
        &mut self,
        nes_rgba: Option<&[u8]>,
        show_overlay: bool,
        overlay_rgba: Option<&[u8]>,
    ) -> bool {
        if self.minimized {
            return true; // nada a desenhar (e nada de laço ocupado pedindo redraw)
        }
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
            Err(wgpu::SurfaceError::Lost) => {
                self.surface.configure(&self.device, &self.config);
                return false;
            }
            Err(wgpu::SurfaceError::Outdated) => return false, // o Resized que vem reconfigura
            Err(e) => {
                log::warn!("surface: {e:?}");
                return false;
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
            if show_overlay {
                pass.set_pipeline(&self.overlay_pipeline);
                pass.set_bind_group(0, &self.overlay.bind_group, &[]);
                pass.draw(0..6, 0..1);
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        true
    }
}

#[cfg(test)]
mod tests {
    /// O shader não roda numa GPU no CI, mas passa pelo mesmo parser e validador do naga que o
    /// wgpu usa em `create_shader_module` (roda no job desktop: `cargo test -p rnfe-gui --lib`).
    #[test]
    fn shader_wgsl_valido() {
        use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
        let module = wgpu::naga::front::wgsl::parse_str(super::SHADER).expect("WGSL inválido");
        Validator::new(ValidationFlags::all(), Capabilities::empty()).validate(&module).expect("validação");
    }
}
