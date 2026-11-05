//! The Android phone's window (P308): `gpuwin.rs`'s [`GpuWindow`] for winit's NativeActivity window,
//! with the same face to the shell — a size, a scale, a close, a resize, clicks — and Vulkan (or GL)
//! under wgpu.
//!
//! As on the desktop the event loop is pumped, never run, on the shell's own thread (the activity's
//! `android_main` one). Android makes one loop per process, so it and its window are made at the
//! first [`GpuWindow::open`] and kept, each window after taking them up again. The native window
//! goes away when the app leaves the screen and comes back as a new one: the surface is dropped in
//! the loop's `suspended` (before Android frees the window under it) and made again in `resumed`,
//! and in between [`GpuWindow::draw`] draws nothing and rests the frame loop.
//!
//! The picture is laid out as the iPhone's is (`ios_gpu.rs`, P240): the shell's frame in the room
//! inside the camera cutout's inset ([`kept_insets`]), the views and the menus' walls running on to
//! the window's edges ([`phone_frame`]), and the touch pad ([`crate::android::TOUCH`]) over the
//! whole window.

use crate::android::TOUCH;
use crate::aspect::{kept_insets, phone_frame, stretched};
use crate::touch_pad::{Phase, Touch};
use game_render::gpu::{Layer, Renderer, Target, wgpu};
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::{TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::platform::android::EventLoopBuilderExtAndroid;
use winit::platform::pump_events::{EventLoopExtPumpEvents, PumpStatus};
use winit::window::{Window, WindowId};

/// Whether the app has its window, and how many times it has lost it (MD97's [`in_front`] and
/// [`times_away`]).
static FRONT: AtomicBool = AtomicBool::new(true);
static AWAY: AtomicU32 = AtomicU32::new(0);

/// The app is in front: it has a native window to draw into. Away, the GPU work waits for it.
pub fn in_front() -> bool {
    FRONT.load(Ordering::Relaxed)
}

/// How many times the app has lost its window since it opened (MD97).
pub fn times_away() -> u32 {
    AWAY.load(Ordering::Relaxed)
}

/// How often the window's insets are asked of the activity.
const LOOK: Duration = Duration::from_millis(100);

/// How long one [`GpuWindow::draw`] rests while the app has no window: the loops still poll and
/// run what ticks are owed, ten times a second, rather than spin.
const AWAY_REST: Duration = Duration::from_millis(100);

/// What the event handler keeps between pumps, for the whole process.
struct State {
    window: Option<Arc<Window>>,
    instance: wgpu::Instance,
    /// The surface on the native window there is now; `None` while the app has none.
    surface: Option<wgpu::Surface<'static>>,
    /// The surface was made again since a [`GpuWindow`] last configured it.
    fresh: bool,
    failed: Option<String>,
    size: (u32, u32),
    resized: bool,
    closed: bool,
    /// The menus' clicks, in window pixels.
    clicks: Vec<(i32, i32)>,
    started: Instant,
}

impl State {
    fn surface_again(&mut self) {
        let Some(window) = self.window.clone() else { return };
        match self.instance.create_surface(window) {
            Ok(s) => {
                self.surface = Some(s);
                self.fresh = true;
            }
            Err(e) => println!("android: no surface: {e}"),
        }
    }
}

impl ApplicationHandler for State {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            match event_loop.create_window(Window::default_attributes().with_title("openMX")) {
                Ok(w) => self.window = Some(Arc::new(w)),
                Err(e) => {
                    self.failed = Some(format!("no window: {e}"));
                    return;
                }
            }
        }
        if let Some(w) = &self.window {
            let s = w.inner_size();
            let size = (s.width.max(1), s.height.max(1));
            self.resized |= size != self.size;
            self.size = size;
        }
        self.surface_again();
        FRONT.store(true, Ordering::Relaxed);
        println!("android: the window is back, {}x{}", self.size.0, self.size.1);
    }

    fn suspended(&mut self, _: &ActiveEventLoop) {
        // Before the handler returns: Android frees the native window once it has.
        self.surface = None;
        if FRONT.swap(false, Ordering::Relaxed) {
            AWAY.fetch_add(1, Ordering::Relaxed);
        }
        TOUCH.lock().unwrap_or_else(|e| e.into_inner()).reset();
        println!("android: the window went away");
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
            WindowEvent::Touch(t) => {
                let phase = match t.phase {
                    TouchPhase::Started => Phase::Down,
                    TouchPhase::Moved => Phase::Move,
                    TouchPhase::Ended => Phase::Up,
                    TouchPhase::Cancelled => Phase::Cancel,
                };
                let touch = Touch { id: t.id, phase, at: (t.location.x as f32, t.location.y as f32) };
                let now = self.started.elapsed().as_secs_f32();
                TOUCH.lock().unwrap_or_else(|e| e.into_inner()).touch(touch, now, &mut self.clicks);
            }
            _ => {}
        }
    }
}

/// The loop and its state, between one [`GpuWindow`] and the next.
struct Host {
    events: EventLoop<()>,
    state: State,
}

thread_local! {
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
}

/// The loop, made the first time: on the activity's [`AndroidApp`](openmx_android::AndroidApp).
fn host() -> Result<Host, String> {
    if let Some(h) = HOST.with(|h| h.borrow_mut().take()) {
        return Ok(h);
    }
    let app = openmx_android::app().ok_or("no activity")?;
    let events = EventLoop::builder()
        .with_android_app(app)
        .build()
        .map_err(|e| format!("no event loop: {e}"))?;
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(
        events.owned_display_handle(),
    )));
    let state = State {
        window: None,
        instance,
        surface: None,
        fresh: false,
        failed: None,
        size: (1, 1),
        resized: false,
        closed: false,
        clicks: Vec::new(),
        started: Instant::now(),
    };
    Ok(Host { events, state })
}

/// A window and the GPU drawing into it.
pub struct GpuWindow {
    /// Lent while this window is open, and put back when it drops.
    host: Option<Host>,
    config: wgpu::SurfaceConfiguration,
    pub renderer: Renderer,
    /// The room inside the kept insets ([`kept_insets`]), in pixels.
    pub width: usize,
    pub height: usize,
    /// How many window pixels one frame pixel becomes.
    pub scale: usize,
    pub screen: (usize, usize),
    /// How long the last [`GpuWindow::draw`] waited on the surface.
    pub waited: Duration,
    pub refresh: Option<f32>,
    modes: Vec<wgpu::PresentMode>,
    /// The whole window's size and the insets kept clear of, as of the last poll.
    size: (u32, u32),
    safe: [u32; 4],
    /// The activity's insets as last asked, and when to ask next.
    insets: [u32; 4],
    next_look: Instant,
    stretch: bool,
    resized: bool,
    closed: bool,
    clicks: Vec<(i32, i32)>,
    /// The touch pad's sprites and their type, made at its first drawing.
    layer: Option<Layer>,
    /// Since when the frame loop has rested with no window, for the log.
    resting: Option<Instant>,
}

/// As `gpuwin.rs`'s.
fn present_mode(vsync: bool, modes: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    let off = [wgpu::PresentMode::Mailbox, wgpu::PresentMode::Immediate];
    match vsync {
        true => wgpu::PresentMode::Fifo,
        false => off.into_iter().find(|m| modes.contains(m)).unwrap_or(wgpu::PresentMode::Fifo),
    }
}

/// How many of the device's uncaptured errors [`log_gpu_error`] writes, as `ios_gpu.rs`'s.
const GPU_ERRORS_LOGGED: usize = 8;

fn log_gpu_error(e: wgpu::Error) {
    static SEEN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    match SEEN.fetch_add(1, Ordering::Relaxed) {
        n if n < GPU_ERRORS_LOGGED => println!("gpu error: {e}"),
        GPU_ERRORS_LOGGED => println!("gpu error: more than {GPU_ERRORS_LOGGED}; the rest are not logged"),
        _ => {}
    }
}

impl GpuWindow {
    /// The app's window — always the whole screen, whatever `size` asks — and a device to draw into
    /// it. Waits for Android to give the activity its native window.
    pub fn open(_size: Option<(usize, usize)>, scale: usize, _title: &str, vsync: bool) -> Result<GpuWindow, String> {
        let mut host = host()?;
        let t0 = Instant::now();
        while host.state.surface.is_none() {
            if let PumpStatus::Exit(code) = host.events.pump_app_events(Some(Duration::from_millis(10)), &mut host.state) {
                return Err(format!("the event loop ended ({code}) before a window opened"));
            }
            if let Some(e) = host.state.failed.take() {
                return Err(e);
            }
            if host.state.closed {
                return Err("the window closed".into());
            }
            // Sent away before it opened: no deadline while the app is not on the screen.
            if t0.elapsed() > Duration::from_secs(10) && in_front() {
                return Err("the window never opened".into());
            }
        }
        let window = host.state.window.clone().ok_or("the window never opened")?;
        let refresh = window
            .current_monitor()
            .and_then(|m| m.refresh_rate_millihertz())
            .map(|mhz| mhz as f32 / 1000.0)
            .filter(|hz| *hz >= 30.0);
        let size = host.state.size;
        let screen = window.current_monitor().map_or((size.0 as usize, size.1 as usize), |m| {
            (m.size().width as usize, m.size().height as usize)
        });
        let surface = host.state.surface.as_ref().expect("waited for");
        let adapter = pollster::block_on(host.state.instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(surface),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU adapter for the window: {e}"))?;
        let caps = surface.get_capabilities(&adapter);
        let plain = [wgpu::TextureFormat::Bgra8Unorm, wgpu::TextureFormat::Rgba8Unorm];
        let format = plain
            .into_iter()
            .find(|f| caps.formats.contains(f))
            .or_else(|| caps.formats.iter().copied().find(|f| !f.is_srgb()))
            .or_else(|| caps.formats.first().copied())
            .ok_or("the surface takes no format")?;
        let renderer = Renderer::new(&adapter, format)?;
        renderer.device.on_uncaptured_error(Arc::new(log_gpu_error));
        let present_mode = present_mode(vsync, &caps.present_modes);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.0,
            height: size.1,
            present_mode,
            // The iPhone's three drawables (P238), for the same reason: a phone's display.
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes.first().copied().unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&renderer.device, &config);
        host.state.fresh = false;
        println!("gpu: {} into a {format:?} surface, {present_mode:?}", renderer.adapter);
        let mut w = GpuWindow {
            host: Some(host),
            config,
            renderer,
            width: 1,
            height: 1,
            scale,
            screen,
            waited: Duration::ZERO,
            refresh,
            modes: caps.present_modes,
            size,
            safe: [0; 4],
            insets: [0; 4],
            next_look: Instant::now(),
            stretch: stretched(),
            resized: false,
            closed: false,
            clicks: Vec::new(),
            layer: None,
            resting: None,
        };
        w.poll();
        w.resized = false;
        Ok(w)
    }

    fn host(&mut self) -> &mut Host {
        self.host.as_mut().expect("the loop is lent until the window drops")
    }

    pub fn set_vsync(&mut self, vsync: bool) {
        let mode = present_mode(vsync, &self.modes);
        if mode != self.config.present_mode {
            self.config.present_mode = mode;
            if let Some(s) = &self.host.as_ref().expect("lent").state.surface {
                s.configure(&self.renderer.device, &self.config);
            }
            println!("gpu: presenting {mode:?}");
        }
    }

    pub fn waits(&self) -> bool {
        self.config.present_mode == wgpu::PresentMode::Fifo
    }

    /// Take in whatever the window has said, without waiting, and the activity's insets every
    /// [`LOOK`].
    pub fn poll(&mut self) {
        let host = self.host();
        host.state.clicks.clear();
        if let PumpStatus::Exit(_) = host.events.pump_app_events(Some(Duration::ZERO), &mut host.state) {
            host.state.closed = true;
        }
        let (size, resized, closed) = (host.state.size, std::mem::take(&mut host.state.resized), host.state.closed);
        let clicks = std::mem::take(&mut host.state.clicks);
        let now = Instant::now();
        if now >= self.next_look && in_front() {
            self.next_look = now + LOOK;
            if let Some(insets) = openmx_android::safe_area()
                && insets != self.insets
            {
                println!("android: insets {insets:?}");
                self.insets = insets;
                self.resized = true;
            }
        }
        let stretch = stretched();
        self.resized |= resized || stretch != self.stretch || size != self.size;
        self.stretch = stretch;
        self.size = size;
        self.closed = closed;
        self.safe = kept_insets(self.insets, stretch);
        let [l, t, r, b] = self.safe;
        self.clicks = clicks.into_iter().map(|(x, y)| (x - l as i32, y - t as i32)).collect();
        self.width = size.0.saturating_sub(l + r).max(1) as usize;
        self.height = size.1.saturating_sub(t + b).max(1) as usize;
        let mut touch = TOUCH.lock().unwrap_or_else(|e| e.into_inner());
        touch.layout((size.0 as f32, size.1 as f32), self.safe.map(|v| v as f32));
        let shown = touch.shown;
        drop(touch);
        // The ride HUD stands clear of the touch pad (P304), as on the iPhone.
        crate::menu::touch_room::set(shown.then_some((self.width, self.height)));
    }

    pub fn closed(&self) -> bool {
        self.closed
    }

    pub fn focused(&self) -> bool {
        in_front()
    }

    /// The menus' taps since the last poll, in pixels of the room inside the kept insets.
    pub fn clicks(&self) -> &[(i32, i32)] {
        &self.clicks
    }

    /// No drag is read here yet (P214): the phone reports taps only.
    pub fn mouse(&self) -> &[crate::window::MouseEvent] {
        &[]
    }

    /// A phone has no cursor.
    pub fn show_cursor(&self, _shown: bool) {}

    /// No keyboard is read on the phone.
    pub fn held(&self, _code: u8) -> bool {
        false
    }

    pub fn key_moments(&self) -> &[crate::window::KeyMoment] {
        &[]
    }

    pub fn tapped(&self, _code: u8) -> bool {
        false
    }

    pub fn resized(&mut self) -> bool {
        std::mem::take(&mut self.resized)
    }

    /// Draw one frame of `width` by `height` frame pixels, centred in the room inside the kept insets
    /// and running on to the window's edges ([`phone_frame`]), then the touch pad over it, and show
    /// it. `None` while the app has no window (after a rest of [`AWAY_REST`]), or the display is slow
    /// to hand a frame back.
    pub fn draw<T>(
        &mut self,
        width: usize,
        height: usize,
        draw: impl FnOnce(&mut Renderer, &Target) -> T,
    ) -> Result<Option<T>, String> {
        let size = self.size;
        let (device, mut config) = (self.renderer.device.clone(), self.config.clone());
        let state = &mut self.host.as_mut().expect("lent").state;
        let Some(surface) = state.surface.as_ref() else {
            if self.resting.is_none() {
                println!("android: no window, the frame loop rests");
            }
            self.resting.get_or_insert_with(Instant::now);
            std::thread::sleep(AWAY_REST);
            return Ok(None);
        };
        if let Some(since) = self.resting.take() {
            println!("android: back, the frame loop rested {:.1} s", since.elapsed().as_secs_f32());
        }
        if std::mem::take(&mut state.fresh) || (config.width, config.height) != size {
            (config.width, config.height) = size;
            surface.configure(&device, &config);
        }
        let acquire = Instant::now();
        let texture = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                surface.configure(&device, &config);
                match surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                    other => return Err(format!("the surface gave no frame: {other:?}")),
                }
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return Ok(None),
            other => return Err(format!("the surface gave no frame: {other:?}")),
        };
        self.config = config;
        self.waited = acquire.elapsed();
        let view = texture.texture.create_view(&Default::default());
        let (frame, bleed) = phone_frame(size, self.safe, (width, height), self.scale);
        let target = Target { view: &view, size, frame, scale: self.scale as u32 };
        self.renderer.bleed = bleed;
        let out = draw(&mut self.renderer, &target);
        self.renderer.bleed = [0; 4];
        let sprites = TOUCH.lock().unwrap_or_else(|e| e.into_inner()).sprites(0);
        if !sprites.is_empty() {
            let layer = self.layer.get_or_insert_with(|| self.renderer.layer(&[&crate::menu::ink::ink().atlas]));
            let whole = Target { view: &view, size, frame: [0, 0, size.0, size.1], scale: 1 };
            self.renderer.draw_layer(layer, &whole, &sprites);
        }
        let present = Instant::now();
        self.renderer.queue.present(texture);
        self.waited += present.elapsed();
        Ok(Some(out))
    }

    pub fn present(&mut self, rgb: &[u8], width: usize, height: usize) -> Result<(), String> {
        self.draw(width, height, |r, target| r.blit(target, rgb, width as u32, height as u32))
            .map(|_| ())
    }

    pub fn present_again(&mut self, width: usize, height: usize) -> Result<(), String> {
        self.draw(width, height, |r, target| r.blit_again(target)).map(|_| ())
    }
}

impl Drop for GpuWindow {
    /// The loop, its window and its surface go back for the next window.
    fn drop(&mut self) {
        if let Some(host) = self.host.take() {
            HOST.with(|h| *h.borrow_mut() = Some(host));
        }
    }
}
