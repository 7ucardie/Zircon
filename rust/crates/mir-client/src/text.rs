//! Text rendering via glyphon, in logical pixels.

use std::collections::HashMap;

use glyphon::{
    Attrs, Buffer, Cache, Color, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache,
    TextArea, TextAtlas, TextBounds, TextRenderer, Viewport,
};

use crate::gfx::Gpu;

pub struct TextLayer {
    font_system: FontSystem,
    swash: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    renderer: TextRenderer,
    buffers: HashMap<(String, u32), Buffer>,
    queued: Vec<Queued>,
}

struct Queued {
    key: (String, u32),
    x: f32,
    y: f32,
    color: [u8; 4],
    centered: bool,
}

impl TextLayer {
    pub fn new(gpu: &Gpu) -> TextLayer {
        let font_system = FontSystem::new();
        let swash = SwashCache::new();
        let cache = Cache::new(&gpu.device);
        let viewport = Viewport::new(&gpu.device, &cache);
        let mut atlas = TextAtlas::new(&gpu.device, &gpu.queue, &cache, gpu.config.format);
        let renderer = TextRenderer::new(
            &mut atlas,
            &gpu.device,
            wgpu::MultisampleState::default(),
            None,
        );
        TextLayer {
            font_system,
            swash,
            viewport,
            atlas,
            renderer,
            buffers: HashMap::new(),
            queued: Vec::new(),
        }
    }

    fn buffer(&mut self, text: &str, size: u32) -> &Buffer {
        let key = (text.to_string(), size);
        if !self.buffers.contains_key(&key) {
            if self.buffers.len() > 2000 {
                self.buffers.clear();
            }
            let mut b = Buffer::new(
                &mut self.font_system,
                Metrics::new(size as f32, size as f32 * 1.3),
            );
            b.set_size(Some(1000.0), Some(100.0));
            b.set_text(
                text,
                &Attrs::new().family(Family::SansSerif),
                Shaping::Advanced,
                None,
            );
            b.shape_until_scroll(&mut self.font_system, false);
            self.buffers.insert(key.clone(), b);
        }
        &self.buffers[&key]
    }

    pub fn width(&mut self, text: &str, size: u32) -> f32 {
        let b = self.buffer(text, size);
        b.layout_runs()
            .map(|r| r.glyphs.iter().map(|g| g.w).sum::<f32>())
            .fold(0.0, f32::max)
    }

    /// Queue text with its top-left at (x, y) logical pixels.
    pub fn draw(&mut self, text: &str, size: u32, x: f32, y: f32, color: [u8; 4]) {
        if text.is_empty() {
            return;
        }
        self.buffer(text, size);
        self.queued.push(Queued {
            key: (text.to_string(), size),
            x,
            y,
            color,
            centered: false,
        });
    }

    /// Queue text horizontally centred on x.
    pub fn draw_centered(&mut self, text: &str, size: u32, x: f32, y: f32, color: [u8; 4]) {
        if text.is_empty() {
            return;
        }
        self.buffer(text, size);
        self.queued.push(Queued {
            key: (text.to_string(), size),
            x,
            y,
            color,
            centered: true,
        });
    }

    /// Prepare all queued text. `scale` is the window scale factor.
    pub fn prepare(&mut self, gpu: &Gpu, scale: f32) {
        self.viewport.update(
            &gpu.queue,
            Resolution {
                width: gpu.config.width,
                height: gpu.config.height,
            },
        );
        let mut areas = Vec::with_capacity(self.queued.len());
        // Compute widths first (needs &mut self), then build areas (needs &self).
        let mut centered_offsets = Vec::with_capacity(self.queued.len());
        for i in 0..self.queued.len() {
            let (text, size) = self.queued[i].key.clone();
            let w = if self.queued[i].centered {
                self.width(&text, size) / 2.0
            } else {
                0.0
            };
            centered_offsets.push(w);
        }
        for (q, off) in self.queued.iter().zip(centered_offsets) {
            let buffer = &self.buffers[&q.key];
            areas.push(TextArea {
                buffer,
                left: (q.x - off) * scale,
                top: q.y * scale,
                scale,
                bounds: TextBounds {
                    left: 0,
                    top: 0,
                    right: gpu.config.width as i32,
                    bottom: gpu.config.height as i32,
                },
                default_color: Color::rgba(q.color[0], q.color[1], q.color[2], q.color[3]),
                custom_glyphs: &[],
            });
        }
        if let Err(e) = self.renderer.prepare(
            &gpu.device,
            &gpu.queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            areas,
            &mut self.swash,
        ) {
            tracing::warn!("text prepare failed: {e:?}");
        }
        self.queued.clear();
    }

    pub fn render<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if let Err(e) = self.renderer.render(&self.atlas, &self.viewport, pass) {
            tracing::warn!("text render failed: {e:?}");
        }
    }

    pub fn trim(&mut self) {
        self.atlas.trim();
    }
}
