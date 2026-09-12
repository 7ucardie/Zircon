//! Rust Zircon game client (winit + wgpu).
//!
//! ```text
//! mir-client [--server HOST:PORT] [--assets DIR]
//!   ZIRCON_ASSETS  client root containing Data/ and Map/ (default ~/zircon-assets/Client)
//!   ZIRCON_SERVER  server address (default 127.0.0.1:7000)
//! ```

mod anim;
mod assets;
mod client;
mod effects;
mod game;
mod gfx;
mod items;
mod net;
mod scenes;
mod text;
mod ui;
mod windows;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::assets::Assets;
use crate::client::Client;
use crate::gfx::{Gpu, SpriteRenderer};
use crate::text::TextLayer;

struct Args {
    assets: PathBuf,
    server: String,
}

fn parse_args() -> Args {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let mut assets = std::env::var_os("ZIRCON_ASSETS")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join("zircon-assets/Client"));
    let mut server = std::env::var("ZIRCON_SERVER")
        .unwrap_or_else(|_| format!("127.0.0.1:{}", mir_proto::DEFAULT_PORT));
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--assets" => assets = PathBuf::from(it.next().expect("--assets DIR")),
            "--server" => server = it.next().expect("--server HOST:PORT"),
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    Args { assets, server }
}

struct App {
    args: Args,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    renderer: Option<SpriteRenderer>,
    text: Option<TextLayer>,
    client: Option<Client>,
    start: Instant,
    frames: u32,
    fps: f32,
    fps_time: Instant,
    scale: f64,
    /// Save a screenshot on the next frame.
    screenshot: Option<PathBuf>,
    /// Automation: `ZIRCON_SCREENSHOT=path[:seconds]` saves after a delay and exits.
    auto_screenshot: Option<(PathBuf, u64)>,
}

impl App {
    fn frame(&mut self) {
        let (Some(gpu), Some(renderer), Some(text), Some(client), Some(window)) = (
            self.gpu.as_mut(),
            self.renderer.as_mut(),
            self.text.as_mut(),
            self.client.as_mut(),
            self.window.as_ref(),
        ) else {
            return;
        };
        let now = self.start.elapsed().as_millis() as u64;
        if self.frames == 0 && self.fps == 0.0 {
            tracing::info!(now, "first frame");
        }
        self.frames += 1;
        if self.fps_time.elapsed().as_secs_f32() >= 1.0 {
            self.fps = self.frames as f32 / self.fps_time.elapsed().as_secs_f32();
            self.frames = 0;
            self.fps_time = Instant::now();
        }
        let scale = self.scale as f32;
        let width = (gpu.config.width as f32 / scale) as i32;
        let height = (gpu.config.height as f32 / scale) as i32;

        let surface = match gpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                let (w, h) = (gpu.config.width, gpu.config.height);
                gpu.resize(w, h);
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
            wgpu::CurrentSurfaceTexture::Validation => {
                tracing::error!("surface validation error");
                return;
            }
        };
        let view = surface
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        renderer.set_scale(scale);
        client.frame(gpu, renderer, text, width, height, now, self.fps);
        text.prepare(gpu, scale);

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            renderer.flush(gpu, &mut pass);
            text.render(&mut pass);
        }
        gpu.queue.submit([encoder.finish()]);
        if let Some((path, delay)) = &self.auto_screenshot {
            if now >= *delay {
                self.screenshot = Some(path.clone());
            }
        }
        if let Some(path) = self.screenshot.take() {
            let rgba = gpu.read_texture(&surface.texture);
            match save_png(&path, gpu.config.width, gpu.config.height, &rgba) {
                Ok(()) => tracing::info!(path = %path.display(), "screenshot saved"),
                Err(e) => tracing::error!("screenshot failed: {e}"),
            }
            if self.auto_screenshot.is_some() {
                std::process::exit(0);
            }
        }
        window.pre_present_notify();
        gpu.queue.present(surface);
        text.trim();
    }
}

fn save_png(path: &PathBuf, w: u32, h: u32, rgba: &[u8]) -> anyhow::Result<()> {
    let file = std::fs::File::create(path)?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header()?;
    writer.write_image_data(rgba)?;
    Ok(())
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Legend of Mir 3 - Zircon (Rust)")
            .with_inner_size(LogicalSize::new(1024.0, 768.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("window"));
        self.scale = window.scale_factor();
        let gpu = Gpu::new(window.clone()).expect("gpu");
        let renderer = SpriteRenderer::new(&gpu);
        let text = TextLayer::new(&gpu);
        let client = Client::new(Assets::new(&self.args.assets), self.args.server.clone());
        self.window = Some(window);
        self.gpu = Some(gpu);
        self.renderer = Some(renderer);
        self.text = Some(text);
        self.client = Some(client);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.resize(size.width, size.height);
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => self.scale = scale_factor,
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(c) = self.client.as_mut() {
                    c.input.mouse = (
                        (position.x / self.scale) as f32,
                        (position.y / self.scale) as f32,
                    );
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(c) = self.client.as_mut() {
                    let down = state == ElementState::Pressed;
                    match button {
                        MouseButton::Left => {
                            c.input.lmb_down = down;
                            if down {
                                c.input.lmb_pressed = true;
                            } else {
                                c.input.lmb_released = true;
                            }
                        }
                        MouseButton::Right => {
                            c.input.rmb_down = down;
                            if down {
                                c.input.rmb_pressed = true;
                            }
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::F12) => {
                            let name = format!("zircon-{}.png", chrono_stamp());
                            self.screenshot = Some(PathBuf::from(name));
                        }
                        PhysicalKey::Code(KeyCode::Backquote) => {
                            if let Some(c) = self.client.as_mut() {
                                c.debug = !c.debug;
                            }
                        }
                        _ => {}
                    }
                    if let Some(c) = self.client.as_mut() {
                        if let PhysicalKey::Code(code) = event.physical_key {
                            let f = match code {
                                KeyCode::F1 => Some(1),
                                KeyCode::F2 => Some(2),
                                KeyCode::F3 => Some(3),
                                KeyCode::F4 => Some(4),
                                KeyCode::F5 => Some(5),
                                KeyCode::F6 => Some(6),
                                KeyCode::F7 => Some(7),
                                KeyCode::F8 => Some(8),
                                KeyCode::F9 => Some(9),
                                KeyCode::F10 => Some(10),
                                KeyCode::F11 => Some(11),
                                _ => None,
                            };
                            if f.is_some() {
                                c.input.fkey = f;
                            }
                            let digit = match code {
                                KeyCode::Digit1 => Some(0),
                                KeyCode::Digit2 => Some(1),
                                KeyCode::Digit3 => Some(2),
                                KeyCode::Digit4 => Some(3),
                                KeyCode::Digit5 => Some(4),
                                KeyCode::Digit6 => Some(5),
                                KeyCode::Digit7 => Some(6),
                                KeyCode::Digit8 => Some(7),
                                KeyCode::Digit9 => Some(8),
                                KeyCode::Digit0 => Some(9),
                                _ => None,
                            };
                            if digit.is_some() {
                                c.input.digit = digit;
                            }
                        }
                        match &event.logical_key {
                            Key::Named(NamedKey::Backspace) => c.input.backspace = true,
                            Key::Named(NamedKey::Enter) => c.input.enter = true,
                            Key::Named(NamedKey::Tab) => c.input.tab = true,
                            Key::Named(NamedKey::Escape) => c.input.escape = true,
                            Key::Named(NamedKey::Delete) => c.input.delete = true,
                            Key::Named(NamedKey::Space) => c.input.text.push(' '),
                            _ => {
                                if let Some(t) = &event.text {
                                    c.input.text.push_str(t);
                                }
                            }
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(c) = self.client.as_mut() {
                    c.input.wheel += match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                        winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                    };
                }
            }
            WindowEvent::RedrawRequested => self.frame(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

fn chrono_stamp() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    t.to_string()
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,wgpu_core=warn,wgpu_hal=warn,naga=warn".into()),
        )
        .init();
    let args = parse_args();
    tracing::info!(assets = %args.assets.display(), server = args.server, "starting client");
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        args,
        window: None,
        gpu: None,
        renderer: None,
        text: None,
        client: None,
        start: Instant::now(),
        frames: 0,
        fps: 0.0,
        fps_time: Instant::now(),
        scale: 1.0,
        screenshot: None,
        auto_screenshot: std::env::var("ZIRCON_SCREENSHOT").ok().map(|v| {
            let (path, secs) = match v.rsplit_once(':') {
                Some((p, s)) if s.parse::<f64>().is_ok() => {
                    (p.to_string(), s.parse::<f64>().unwrap())
                }
                _ => (v.clone(), 3.0),
            };
            (PathBuf::from(path), (secs * 1000.0) as u64)
        }),
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}
