//! Minimal 2D sprite renderer on wgpu.
//!
//! Sprites are uploaded lazily into 4096x4096 RGBA8 atlas pages and drawn as
//! textured quads in submission order, so the caller controls layering by draw
//! order exactly like Zircon's D3D sprite pipeline. Two blend modes exist:
//! normal alpha and additive (used for "blended" tiles and effects).

use std::collections::HashMap;
use std::sync::Arc;

use guillotiere::{size2, AtlasAllocator};
use wgpu::util::DeviceExt;
use winit::window::Window;

pub const ATLAS_SIZE: u32 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpriteKey {
    pub library: u16,
    pub index: u32,
    pub surface: Surface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum Surface {
    Image,
    Shadow,
    Overlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blend {
    Alpha,
    /// Zircon's blended tiles/effects: `src * (1 - dst) + dst`.
    Screen,
}

#[derive(Debug, Clone, Copy)]
pub struct SpriteRegion {
    pub page: u16,
    pub width: u32,
    pub height: u32,
    /// Atlas UV rectangle.
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    screen: [f32; 2],
    _pad: [f32; 2],
}

struct Page {
    #[allow(dead_code)]
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    allocator: AtlasAllocator,
}

struct DrawCall {
    page: u16,
    blend: Blend,
    first_vertex: u32,
    vertex_count: u32,
}

pub struct Gpu {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    #[allow(dead_code)]
    pub window: Arc<Window>,
}

pub struct SpriteRenderer {
    pipeline_alpha: wgpu::RenderPipeline,
    pipeline_screen: wgpu::RenderPipeline,
    texture_layout: wgpu::BindGroupLayout,
    globals_bind_group: wgpu::BindGroup,
    globals_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
    pages: Vec<Page>,
    sprites: HashMap<SpriteKey, Option<SpriteRegion>>,
    vertices: Vec<Vertex>,
    calls: Vec<DrawCall>,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity: usize,
    white: SpriteRegion,
    scale: f32,
}

impl Gpu {
    pub fn new(window: Arc<Window>) -> anyhow::Result<Gpu> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let surface = instance.create_surface(window.clone())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("mir-client"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
                ..Default::default()
            }))?;
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or_else(|| anyhow::anyhow!("surface not supported by adapter"))?;
        // Prefer a non-sRGB swapchain so sprite colours are passed through untouched,
        // exactly like the original D3D9/D3D11 client.
        let caps = surface.get_capabilities(&adapter);
        if let Some(fmt) = caps.formats.iter().find(|f| !f.is_srgb()) {
            config.format = *fmt;
        }
        config.present_mode = wgpu::PresentMode::AutoVsync;
        config.usage |= wgpu::TextureUsages::COPY_SRC;
        surface.configure(&device, &config);
        tracing::info!(backend = ?adapter.get_info().backend, name = adapter.get_info().name, format = ?config.format, "gpu ready");
        Ok(Gpu {
            surface,
            device,
            queue,
            config,
            window,
        })
    }

    /// Read back a texture as RGBA8 (converting from BGRA if needed).
    pub fn read_texture(&self, texture: &wgpu::Texture) -> Vec<u8> {
        let (w, h) = (texture.width(), texture.height());
        let bytes_per_row = (w * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (bytes_per_row * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("readback"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        rx.recv().expect("map").expect("map ok");
        let data = slice.get_mapped_range().expect("mapped range");
        let bgra = matches!(
            texture.format(),
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for row in 0..h {
            let start = (row * bytes_per_row) as usize;
            let px = &data[start..start + (w * 4) as usize];
            if bgra {
                for p in px.as_chunks::<4>().0 {
                    out.extend_from_slice(&[p[2], p[1], p[0], 255]);
                }
            } else {
                for p in px.as_chunks::<4>().0 {
                    out.extend_from_slice(&[p[0], p[1], p[2], 255]);
                }
            }
        }
        drop(data);
        buffer.unmap();
        out
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }
}

const SHADER: &str = r#"
struct Globals { screen: vec2<f32>, pad: vec2<f32> };
@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var atlas_tex: texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

struct VsIn { @location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32> };
struct VsOut { @builtin(position) clip: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) color: vec4<f32> };

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let ndc = vec2<f32>(in.pos.x / globals.screen.x * 2.0 - 1.0, 1.0 - in.pos.y / globals.screen.y * 2.0);
    out.clip = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let t = textureSample(atlas_tex, atlas_sampler, in.uv);
    return t * in.color;
}
"#;

#[allow(clippy::too_many_arguments)]
impl SpriteRenderer {
    pub fn new(gpu: &Gpu) -> SpriteRenderer {
        let device = &gpu.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprite"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("atlas"),
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprite"),
            bind_group_layouts: &[Some(&globals_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let make_pipeline = |blend: wgpu::BlendState, label: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4],
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: gpu.config.format,
                        blend: Some(blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipeline_alpha = make_pipeline(wgpu::BlendState::ALPHA_BLENDING, "sprite-alpha");
        let pipeline_screen = make_pipeline(
            wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::OneMinusDst,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            },
            "sprite-screen",
        );
        let globals_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("globals"),
            contents: bytemuck::bytes_of(&Globals {
                screen: [gpu.config.width as f32, gpu.config.height as f32],
                _pad: [0.0; 2],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            }],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("nearest"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let vertex_capacity = 1 << 16;
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite-vertices"),
            size: (vertex_capacity * std::mem::size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut r = SpriteRenderer {
            pipeline_alpha,
            pipeline_screen,
            texture_layout,
            globals_bind_group,
            globals_buffer,
            sampler,
            pages: Vec::new(),
            sprites: HashMap::new(),
            vertices: Vec::new(),
            calls: Vec::new(),
            vertex_buffer,
            vertex_capacity,
            scale: 1.0,
            white: SpriteRegion {
                page: 0,
                width: 1,
                height: 1,
                u0: 0.0,
                v0: 0.0,
                u1: 0.0,
                v1: 0.0,
            },
        };
        // A 2x2 white pixel block for solid rectangles (HP bars, debug overlays).
        r.white = r.upload_rgba(gpu, 2, 2, &[255u8; 16]);
        r.white = SpriteRegion {
            // Sample the center of the white block to avoid bleeding.
            u0: r.white.u0 + 0.5 / ATLAS_SIZE as f32,
            v0: r.white.v0 + 0.5 / ATLAS_SIZE as f32,
            u1: r.white.u0 + 1.5 / ATLAS_SIZE as f32,
            v1: r.white.v0 + 1.5 / ATLAS_SIZE as f32,
            ..r.white
        };
        r
    }

    fn new_page(&mut self, gpu: &Gpu) -> u16 {
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("atlas-page"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atlas-page"),
            layout: &self.texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.pages.push(Page {
            texture,
            bind_group,
            allocator: AtlasAllocator::new(size2(ATLAS_SIZE as i32, ATLAS_SIZE as i32)),
        });
        tracing::debug!(pages = self.pages.len(), "new atlas page");
        (self.pages.len() - 1) as u16
    }

    /// Upload an RGBA8 image into the atlas and return its region.
    pub fn upload_rgba(&mut self, gpu: &Gpu, width: u32, height: u32, rgba: &[u8]) -> SpriteRegion {
        assert_eq!(rgba.len(), (width * height * 4) as usize);
        // 1px padding on each side prevents neighbours bleeding in with filtering.
        let alloc_w = (width + 2) as i32;
        let alloc_h = (height + 2) as i32;
        let mut chosen = None;
        for (i, page) in self.pages.iter_mut().enumerate() {
            if let Some(a) = page.allocator.allocate(size2(alloc_w, alloc_h)) {
                chosen = Some((i as u16, a));
                break;
            }
        }
        let (page_idx, alloc) = match chosen {
            Some(c) => c,
            None => {
                let p = self.new_page(gpu);
                let a = self.pages[p as usize]
                    .allocator
                    .allocate(size2(alloc_w, alloc_h))
                    .expect("sprite larger than atlas page");
                (p, a)
            }
        };
        let x = alloc.rectangle.min.x as u32 + 1;
        let y = alloc.rectangle.min.y as u32 + 1;
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.pages[page_idx as usize].texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let s = ATLAS_SIZE as f32;
        SpriteRegion {
            page: page_idx,
            width,
            height,
            u0: x as f32 / s,
            v0: y as f32 / s,
            u1: (x + width) as f32 / s,
            v1: (y + height) as f32 / s,
        }
    }

    /// Look up a sprite, loading it through `load` on first use. `load` returns
    /// `(width, height, rgba)` or `None` if the image is empty.
    pub fn sprite(
        &mut self,
        gpu: &Gpu,
        key: SpriteKey,
        load: impl FnOnce() -> Option<(u32, u32, Vec<u8>)>,
    ) -> Option<SpriteRegion> {
        if let Some(r) = self.sprites.get(&key) {
            return *r;
        }
        let region = load().map(|(w, h, rgba)| self.upload_rgba(gpu, w, h, &rgba));
        self.sprites.insert(key, region);
        region
    }

    #[allow(dead_code)]
    pub fn is_loaded(&self, key: &SpriteKey) -> bool {
        self.sprites.contains_key(key)
    }

    /// Queue a sprite quad at pixel position (x, y) with the given tint and blend.
    pub fn draw(&mut self, region: SpriteRegion, x: f32, y: f32, color: [f32; 4], blend: Blend) {
        self.draw_scaled(
            region,
            x,
            y,
            region.width as f32,
            region.height as f32,
            color,
            blend,
        );
    }

    pub fn draw_scaled(
        &mut self,
        region: SpriteRegion,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
        blend: Blend,
    ) {
        let first = self.vertices.len() as u32;
        let (x0, y0, x1, y1) = (x, y, x + w, y + h);
        let v = |px: f32, py: f32, u: f32, vv: f32| Vertex {
            pos: [px, py],
            uv: [u, vv],
            color,
        };
        self.vertices.extend_from_slice(&[
            v(x0, y0, region.u0, region.v0),
            v(x1, y0, region.u1, region.v0),
            v(x1, y1, region.u1, region.v1),
            v(x0, y0, region.u0, region.v0),
            v(x1, y1, region.u1, region.v1),
            v(x0, y1, region.u0, region.v1),
        ]);
        match self.calls.last_mut() {
            Some(c) if c.page == region.page && c.blend == blend => c.vertex_count += 6,
            _ => self.calls.push(DrawCall {
                page: region.page,
                blend,
                first_vertex: first,
                vertex_count: 6,
            }),
        }
    }

    /// Arbitrary quad (corners in order top-left, top-right, bottom-right, bottom-left).
    pub fn draw_quad(
        &mut self,
        region: SpriteRegion,
        c: [[f32; 2]; 4],
        color: [f32; 4],
        blend: Blend,
    ) {
        let first = self.vertices.len() as u32;
        let v = |p: [f32; 2], u: f32, vv: f32| Vertex {
            pos: p,
            uv: [u, vv],
            color,
        };
        let (u0, v0, u1, v1) = (region.u0, region.v0, region.u1, region.v1);
        self.vertices.extend_from_slice(&[
            v(c[0], u0, v0),
            v(c[1], u1, v0),
            v(c[2], u1, v1),
            v(c[0], u0, v0),
            v(c[2], u1, v1),
            v(c[3], u0, v1),
        ]);
        match self.calls.last_mut() {
            Some(cl) if cl.page == region.page && cl.blend == blend => cl.vertex_count += 6,
            _ => self.calls.push(DrawCall {
                page: region.page,
                blend,
                first_vertex: first,
                vertex_count: 6,
            }),
        }
    }

    /// Draw a sub-rectangle (`sx, sy, sw, sh` in sprite pixels) of a sprite at (x, y),
    /// stretched to `dw` x `dh`.
    pub fn draw_part(
        &mut self,
        region: SpriteRegion,
        sx: f32,
        sy: f32,
        sw: f32,
        sh: f32,
        x: f32,
        y: f32,
        dw: f32,
        dh: f32,
        color: [f32; 4],
    ) {
        let (rw, rh) = (region.width as f32, region.height as f32);
        let sw = sw.min(rw - sx).max(0.0);
        let sh = sh.min(rh - sy).max(0.0);
        if sw <= 0.0 || sh <= 0.0 {
            return;
        }
        let part = SpriteRegion {
            u0: region.u0 + (region.u1 - region.u0) * (sx / rw),
            u1: region.u0 + (region.u1 - region.u0) * ((sx + sw) / rw),
            v0: region.v0 + (region.v1 - region.v0) * (sy / rh),
            v1: region.v0 + (region.v1 - region.v0) * ((sy + sh) / rh),
            ..region
        };
        self.draw_scaled(part, x, y, dw, dh, color, Blend::Alpha);
    }

    /// Draw only the left `frac` (0..=1) of a sprite, for gauges.
    pub fn draw_cropped(
        &mut self,
        region: SpriteRegion,
        x: f32,
        y: f32,
        frac: f32,
        color: [f32; 4],
    ) {
        let frac = frac.clamp(0.0, 1.0);
        if frac <= 0.0 {
            return;
        }
        let w = (region.width as f32 * frac).floor();
        let cropped = SpriteRegion {
            u1: region.u0 + (region.u1 - region.u0) * (w / region.width as f32),
            ..region
        };
        self.draw_scaled(cropped, x, y, w, region.height as f32, color, Blend::Alpha);
    }

    /// Solid rectangle.
    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        let white = self.white;
        self.draw_scaled(white, x, y, w, h, color, Blend::Alpha);
    }

    /// Submit everything queued since the last flush into `pass`.
    pub fn flush<'a>(&'a mut self, gpu: &Gpu, pass: &mut wgpu::RenderPass<'a>) {
        if self.vertices.is_empty() {
            return;
        }
        if self.vertices.len() > self.vertex_capacity {
            self.vertex_capacity = self.vertices.len().next_power_of_two();
            self.vertex_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("sprite-vertices"),
                size: (self.vertex_capacity * std::mem::size_of::<Vertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        gpu.queue
            .write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&self.vertices));
        gpu.queue.write_buffer(
            &self.globals_buffer,
            0,
            bytemuck::bytes_of(&Globals {
                screen: [
                    gpu.config.width as f32 / self.scale,
                    gpu.config.height as f32 / self.scale,
                ],
                _pad: [0.0; 2],
            }),
        );
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_bind_group(0, &self.globals_bind_group, &[]);
        let mut current: Option<(u16, Blend)> = None;
        for call in &self.calls {
            if current != Some((call.page, call.blend)) {
                pass.set_pipeline(match call.blend {
                    Blend::Alpha => &self.pipeline_alpha,
                    Blend::Screen => &self.pipeline_screen,
                });
                pass.set_bind_group(1, &self.pages[call.page as usize].bind_group, &[]);
                current = Some((call.page, call.blend));
            }
            pass.draw(
                call.first_vertex..call.first_vertex + call.vertex_count,
                0..1,
            );
        }
        self.vertices.clear();
        self.calls.clear();
    }

    /// Logical-to-physical scale (window scale factor). Draw calls use logical pixels.
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale.max(0.01);
    }

    pub fn stats(&self) -> (usize, usize) {
        (self.pages.len(), self.sprites.len())
    }
}
