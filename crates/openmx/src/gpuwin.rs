//! The window the GPU draws into (P44): a winit window with a wgpu surface on it.
//!
//! [`crate::window`] talks X11 itself and pushes finished pixels down the socket, which is all the
//! software renderer needs and is no use to a GPU: a surface needs a handle the driver knows. This is
//! the same window to the shell — a size, a scale, the keys held by X keycode, a close and a resize —
//! with a [`Renderer`] behind it instead of `PutImage`.
//!
//! The event loop is pumped, never run: the shell's loop owns the frame, and each [`GpuWindow::poll`]
//! drains what winit has without waiting.

use crate::window::KEYCODES;
use game_render::gpu::{Renderer, Target, wgpu};
use std::sync::Arc;
use std::time::Duration;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::platform::pump_events::{EventLoopExtPumpEvents, PumpStatus};
use winit::platform::scancode::PhysicalKeyExtScancode;
use winit::window::{Window, WindowId};

/// What the event handler keeps between pumps.
struct State {
    title: String,
    /// The size asked for, in window pixels; `None` for the whole screen.
    asked: Option<(u32, u32)>,
    window: Option<Arc<Window>>,
    failed: Option<String>,
    keys: [bool; KEYCODES],
    size: (u32, u32),
    resized: bool,
    closed: bool,
}

impl ApplicationHandler for State {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let mut attributes = Window::default_attributes().with_title(self.title.clone());
        match self.asked {
            Some((w, h)) => attributes = attributes.with_inner_size(winit::dpi::PhysicalSize::new(w, h)),
            None => {
                if let Some(m) = event_loop.primary_monitor().or_else(|| event_loop.available_monitors().next()) {
                    attributes = attributes.with_inner_size(m.size());
                }
            }
        }
        match event_loop.create_window(attributes) {
            Ok(w) => {
                let s = w.inner_size();
                self.size = (s.width.max(1), s.height.max(1));
                self.window = Some(Arc::new(w));
            }
            Err(e) => self.failed = Some(format!("no window: {e}")),
        }
    }

    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => self.closed = true,
            WindowEvent::Resized(s) => {
                let size = (s.width.max(1), s.height.max(1));
                if size != self.size {
                    self.size = size;
                    self.resized = true;
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                // The X keycode, as `window::key` names them: Linux's scancode, eight on.
                if let Some(code) = event.physical_key.to_scancode()
                    && let Some(held) = self.keys.get_mut(code as usize + 8)
                {
                    *held = event.state == ElementState::Pressed;
                }
            }
            WindowEvent::Focused(false) => self.keys = [false; KEYCODES],
            _ => {}
        }
    }
}

/// A window and the GPU drawing into it.
pub struct GpuWindow {
    events: EventLoop<()>,
    state: State,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pub renderer: Renderer,
    /// The window's size in pixels.
    pub width: usize,
    pub height: usize,
    /// How many window pixels one frame pixel becomes.
    pub scale: usize,
    pub screen: (usize, usize),
    /// How long the last [`GpuWindow::draw`] waited on the surface: for a frame to draw into, and to
    /// show it. `--fps` counts it as the present.
    pub waited: Duration,
}

impl GpuWindow {
    /// Open a window as [`crate::window::Window::open`] does — for frames of `size` at `scale`, or
    /// the whole screen — and a device to draw into it with.
    pub fn open(size: Option<(usize, usize)>, scale: usize, title: &str) -> Result<GpuWindow, String> {
        let mut events = EventLoop::new().map_err(|e| format!("no event loop: {e}"))?;
        let mut state = State {
            title: title.to_string(),
            asked: size.map(|(w, h)| ((w * scale) as u32, (h * scale) as u32)),
            window: None,
            failed: None,
            keys: [false; KEYCODES],
            size: (1, 1),
            resized: false,
            closed: false,
        };
        // The window is made when the loop first resumes.
        for _ in 0..100 {
            if let PumpStatus::Exit(code) = events.pump_app_events(Some(Duration::from_millis(10)), &mut state) {
                return Err(format!("the event loop ended ({code}) before a window opened"));
            }
            if state.window.is_some() || state.failed.is_some() {
                break;
            }
        }
        if let Some(e) = state.failed.take() {
            return Err(e);
        }
        let window = state.window.clone().ok_or("the window never opened")?;
        let screen = window
            .current_monitor()
            .map_or((state.size.0 as usize, state.size.1 as usize), |m| (m.size().width as usize, m.size().height as usize));
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(
            events.owned_display_handle(),
        )));
        let surface = instance.create_surface(window.clone()).map_err(|e| format!("no surface: {e}"))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU adapter for the window: {e}"))?;
        let caps = surface.get_capabilities(&adapter);
        // A plain eight-bit format, as the frame's bytes are plain: the shaders write the rasteriser's
        // colours. A float surface is linear, and would wash them out.
        let plain = [wgpu::TextureFormat::Bgra8Unorm, wgpu::TextureFormat::Rgba8Unorm];
        let format = plain
            .into_iter()
            .find(|f| caps.formats.contains(f))
            .or_else(|| caps.formats.iter().copied().find(|f| !f.is_srgb()))
            .or_else(|| caps.formats.first().copied())
            .ok_or("the surface takes no format")?;
        // The loop paces itself at 60 Hz; a present that waits for the display would double that wait,
        // so the newest frame replaces any not yet shown, and the display still never tears.
        let present_mode = [wgpu::PresentMode::Mailbox, wgpu::PresentMode::Immediate]
            .into_iter()
            .find(|m| caps.present_modes.contains(m))
            .unwrap_or(wgpu::PresentMode::Fifo);
        let renderer = Renderer::new(&adapter, format)?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: state.size.0,
            height: state.size.1,
            present_mode,
            desired_maximum_frame_latency: 1,
            alpha_mode: caps.alpha_modes.first().copied().unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&renderer.device, &config);
        println!("gpu: {} into a {format:?} surface, {present_mode:?}", renderer.adapter);
        let (width, height) = (state.size.0 as usize, state.size.1 as usize);
        Ok(GpuWindow {
            events,
            state,
            window,
            surface,
            config,
            renderer,
            width,
            height,
            scale,
            screen,
            waited: Duration::ZERO,
        })
    }

    /// Take in whatever the window has said, without waiting.
    pub fn poll(&mut self) {
        if let PumpStatus::Exit(_) = self.events.pump_app_events(Some(Duration::ZERO), &mut self.state) {
            self.state.closed = true;
        }
        (self.width, self.height) = (self.state.size.0 as usize, self.state.size.1 as usize);
    }

    pub fn closed(&self) -> bool {
        self.state.closed
    }

    pub fn held(&self, code: u8) -> bool {
        self.state.keys[code as usize]
    }

    /// Whether the window has changed size since this was last asked.
    pub fn resized(&mut self) -> bool {
        std::mem::take(&mut self.state.resized)
    }

    /// Draw one frame of `width` by `height` frame pixels, centred in the window at its scale, and
    /// show it. `draw` gets the renderer and where to draw. `None` where the window has no frame to
    /// give — hidden, or the display slow to hand one back — and the frame is skipped.
    pub fn draw<T>(
        &mut self,
        width: usize,
        height: usize,
        draw: impl FnOnce(&mut Renderer, &Target) -> T,
    ) -> Result<Option<T>, String> {
        let size = (self.state.size.0, self.state.size.1);
        if (self.config.width, self.config.height) != size {
            (self.config.width, self.config.height) = size;
            self.surface.configure(&self.renderer.device, &self.config);
        }
        let acquire = std::time::Instant::now();
        let texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.renderer.device, &self.config);
                match self.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                    other => return Err(format!("the surface gave no frame: {other:?}")),
                }
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return Ok(None),
            other => return Err(format!("the surface gave no frame: {other:?}")),
        };
        self.waited = acquire.elapsed();
        let view = texture.texture.create_view(&Default::default());
        let (across, down) = ((width * self.scale) as u32, (height * self.scale) as u32);
        let (left, top) = (size.0.saturating_sub(across) / 2, size.1.saturating_sub(down) / 2);
        let target = Target {
            view: &view,
            size,
            frame: [left, top, across.min(size.0), down.min(size.1)],
            scale: self.scale as u32,
        };
        let out = draw(&mut self.renderer, &target);
        let present = std::time::Instant::now();
        self.window.pre_present_notify();
        self.renderer.queue.present(texture);
        self.waited += present.elapsed();
        Ok(Some(out))
    }

    /// Show a frame the CPU drew.
    pub fn present(&mut self, rgb: &[u8], width: usize, height: usize) -> Result<(), String> {
        self.draw(width, height, |r, target| r.blit(target, rgb, width as u32, height as u32))
            .map(|_| ())
    }
}
