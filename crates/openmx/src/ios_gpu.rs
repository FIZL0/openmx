//! The iPhone's window (P200): `gpuwin.rs`'s [`GpuWindow`] for winit's UIKit window, with the same
//! face to the shell — a size, a scale, a close, a resize, clicks — and Metal under wgpu.
//!
//! UIKit runs its own loop on the main thread and never gives it back, so the loop cannot be pumped
//! as the desktop's is: [`run_on_main`] runs it, and the shell runs on a thread of its own. The two
//! meet in one shared state: the main thread puts the window's size, its safe area, its touches and
//! its closing there as they come, and each [`GpuWindow::poll`] takes them. The surface is made on
//! the main thread once (Metal's layer is added to the view there, and a second would be a second
//! layer) and lent to whichever [`GpuWindow`] is open.
//!
//! The picture keeps to the safe area (M106): the window's `width` and `height` are the safe area's,
//! the frame is centred in it, and the notch, the Dynamic Island and the home indicator get black.
//! The touch pad ([`crate::ios::TOUCH`]) is drawn over the whole window, laid out from the same edges.
//!
//! The view controller is held the way [`openmx_ios::Holder`] says (M144, B694): either way up with
//! the device, and landscape with the camera on the left while rotation lock is on.

use crate::hud::{NEEDS_TAP_TEXT, NeedsTap};
use crate::ios::TOUCH;
use crate::touch_pad::{Phase, Touch};
use game_render::gpu::{Layer, Renderer, Target, wgpu};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::{TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

/// What the main thread hands the shell's.
struct Shared {
    window: Option<Arc<Window>>,
    instance: Option<wgpu::Instance>,
    /// The surface, while no [`GpuWindow`] has it.
    surface: Option<wgpu::Surface<'static>>,
    /// The window's size and its safe insets (left, top, right, bottom), in pixels.
    size: (u32, u32),
    safe: [u32; 4],
    resized: bool,
    closed: bool,
    /// In front: not sent away to the home screen.
    focused: bool,
    /// The menus' clicks, in window pixels.
    clicks: Vec<(i32, i32)>,
    /// A finger came down since the last poll (P233's "Tap screen to resume controller").
    touched: bool,
}

static SHARED: Mutex<Shared> = Mutex::new(Shared {
    window: None,
    instance: None,
    surface: None,
    size: (1, 1),
    safe: [0; 4],
    resized: false,
    closed: false,
    focused: true,
    clicks: Vec::new(),
    touched: false,
});

fn shared() -> MutexGuard<'static, Shared> {
    SHARED.lock().unwrap_or_else(|e| e.into_inner())
}

/// How often the main thread looks at the safe area and which way up the phone is.
const LOOK: Duration = Duration::from_millis(100);

/// The main thread's side.
struct Host {
    game: Option<Box<dyn FnOnce() + Send>>,
    holder: openmx_ios::Holder,
    started: Instant,
    next_look: Instant,
}

impl Host {
    /// The safe area, the touch pad's layout and the orientation hold, brought up to date.
    fn look(&mut self) {
        let Some(window) = shared().window.clone() else { return };
        let safe = openmx_ios::safe_area(&window);
        let mut s = shared();
        if safe != s.safe {
            s.safe = safe;
            s.resized = true;
        }
        let (size, safe) = (s.size, s.safe);
        drop(s);
        TOUCH
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .layout((size.0 as f32, size.1 as f32), safe.map(|v| v as f32));
        let (device, gravity) = openmx_ios::device_turn();
        let now = self.started.elapsed().as_secs_f32();
        if let Some(hold) = self.holder.step(now, device, gravity, size.0 > size.1) {
            println!("ios: holding {hold:?}");
            openmx_ios::hold(&window, hold);
        }
    }
}

impl ApplicationHandler for Host {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let mut s = shared();
        s.focused = true;
        if s.window.is_none() {
            match event_loop.create_window(Window::default_attributes().with_title("openMX")) {
                Ok(w) => {
                    let w = Arc::new(w);
                    let size = w.inner_size();
                    s.size = (size.width.max(1), size.height.max(1));
                    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(
                        Box::new(event_loop.owned_display_handle()),
                    ));
                    match instance.create_surface(w.clone()) {
                        Ok(surface) => s.surface = Some(surface),
                        Err(e) => println!("ios: no surface: {e}"),
                    }
                    s.instance = Some(instance);
                    s.window = Some(w);
                }
                Err(e) => println!("ios: no window: {e}"),
            }
        }
        drop(s);
        self.look();
        if let Some(game) = self.game.take() {
            // The shell's loop wants the desktop's main-thread stack, not a spawned thread's 512 KB.
            if let Err(e) = std::thread::Builder::new().name("shell".into()).stack_size(64 << 20).spawn(game) {
                println!("ios: no thread for the shell: {e}");
            }
        }
    }

    fn suspended(&mut self, _: &ActiveEventLoop) {
        shared().focused = false;
        TOUCH.lock().unwrap_or_else(|e| e.into_inner()).reset();
    }

    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => shared().closed = true,
            WindowEvent::Resized(size) => {
                let mut s = shared();
                let size = (size.width.max(1), size.height.max(1));
                if size != s.size {
                    s.size = size;
                    s.resized = true;
                }
                drop(s);
                self.look();
            }
            WindowEvent::Focused(focused) => shared().focused = focused,
            WindowEvent::Touch(t) => {
                if t.phase == TouchPhase::Started {
                    shared().touched = true;
                }
                let phase = match t.phase {
                    TouchPhase::Started => Phase::Down,
                    TouchPhase::Moved => Phase::Move,
                    TouchPhase::Ended => Phase::Up,
                    TouchPhase::Cancelled => Phase::Cancel,
                };
                let touch = Touch { id: t.id, phase, at: (t.location.x as f32, t.location.y as f32) };
                let mut clicks = Vec::new();
                let now = self.started.elapsed().as_secs_f32();
                TOUCH.lock().unwrap_or_else(|e| e.into_inner()).touch(touch, now, &mut clicks);
                if !clicks.is_empty() {
                    shared().clicks.extend(clicks);
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if now >= self.next_look {
            self.next_look = now + LOOK;
            self.look();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_look));
    }
}

/// Run UIKit on this, the main thread, and `game` — the shell — on a thread of its own once the
/// window is up. Never returns: the app ends when `game` exits the process, or iOS ends it.
pub fn run_on_main(game: impl FnOnce() + Send + 'static) {
    let events = match EventLoop::new() {
        Ok(e) => e,
        Err(e) => {
            println!("ios: no event loop: {e}");
            std::process::exit(1);
        }
    };
    let now = Instant::now();
    let mut host = Host { game: Some(Box::new(game)), holder: Default::default(), started: now, next_look: now };
    if let Err(e) = events.run_app(&mut host) {
        println!("ios: the event loop ended: {e}");
    }
}

/// A window and the GPU drawing into it.
pub struct GpuWindow {
    surface: Option<wgpu::Surface<'static>>,
    config: wgpu::SurfaceConfiguration,
    pub renderer: Renderer,
    /// The safe area's size in pixels.
    pub width: usize,
    pub height: usize,
    /// How many window pixels one frame pixel becomes.
    pub scale: usize,
    pub screen: (usize, usize),
    /// How long the last [`GpuWindow::draw`] waited on the surface.
    pub waited: Duration,
    /// The display's refresh in hertz, where it says.
    pub refresh: Option<f32>,
    modes: Vec<wgpu::PresentMode>,
    /// The whole window's size and its safe insets, as of the last poll.
    size: (u32, u32),
    safe: [u32; 4],
    resized: bool,
    closed: bool,
    focused: bool,
    clicks: Vec<(i32, i32)>,
    /// The touch pad's sprites and their type, made at its first drawing.
    layer: Option<Layer>,
    /// "Tap screen to resume controller" is up (P233).
    needs_tap: NeedsTap,
}

/// As `gpuwin.rs`'s.
/// How many of the device's uncaptured errors [`log_gpu_error`] writes before it stops: the same
/// error comes back every frame, and the log is for reading.
const GPU_ERRORS_LOGGED: usize = 8;

/// The device's uncaptured errors into the log (P212), as `web_gpu.rs` keeps them for the page.
fn log_gpu_error(e: wgpu::Error) {
    static SEEN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    match SEEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed) {
        n if n < GPU_ERRORS_LOGGED => println!("gpu error: {e}"),
        GPU_ERRORS_LOGGED => println!("gpu error: more than {GPU_ERRORS_LOGGED}; the rest are not logged"),
        _ => {}
    }
}

fn present_mode(vsync: bool, modes: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    let off = [wgpu::PresentMode::Mailbox, wgpu::PresentMode::Immediate];
    match vsync {
        true => wgpu::PresentMode::Fifo,
        false => off.into_iter().find(|m| modes.contains(m)).unwrap_or(wgpu::PresentMode::Fifo),
    }
}

impl GpuWindow {
    /// The app's window — always the whole screen, whatever `size` asks — and a device to draw into
    /// it. Waits for the main thread to have made it.
    pub fn open(_size: Option<(usize, usize)>, scale: usize, _title: &str, vsync: bool) -> Result<GpuWindow, String> {
        let t0 = Instant::now();
        let (window, instance, surface) = loop {
            let mut s = shared();
            if let (Some(w), Some(i)) = (&s.window, &s.instance)
                && s.surface.is_some()
            {
                let (w, i) = (w.clone(), i.clone());
                break (w, i, s.surface.take().expect("checked"));
            }
            drop(s);
            if t0.elapsed() > Duration::from_secs(10) {
                return Err("the window never opened".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let refresh = window
            .current_monitor()
            .and_then(|m| m.refresh_rate_millihertz())
            .map(|mhz| mhz as f32 / 1000.0)
            .filter(|hz| *hz >= 30.0);
        let size = shared().size;
        let screen = window.current_monitor().map_or((size.0 as usize, size.1 as usize), |m| {
            (m.size().width as usize, m.size().height as usize)
        });
        let built = (|| {
            let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
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
            // A validation or pipeline error Metal raises lands here rather than in a panic (P212):
            // without this, a model wgpu refuses to draw is only missing from the picture.
            renderer.device.on_uncaptured_error(Arc::new(log_gpu_error));
            Ok::<_, String>((caps, format, renderer))
        })();
        let (caps, format, renderer) = match built {
            Ok(b) => b,
            Err(e) => {
                shared().surface = Some(surface);
                return Err(e);
            }
        };
        let present_mode = present_mode(vsync, &caps.present_modes);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.0,
            height: size.1,
            present_mode,
            // Three drawables, not the desktop's two (P238): with one always on screen, two left the
            // CPU waiting on the GPU and the GPU on the display, and a ~20 ms frame took every other
            // refresh — a dead 30 fps on a 120 Hz iPhone whatever the frame limit.
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes.first().copied().unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&renderer.device, &config);
        println!("gpu: {} into a {format:?} surface, {present_mode:?}", renderer.adapter);
        let mut w = GpuWindow {
            surface: Some(surface),
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
            resized: false,
            closed: false,
            focused: true,
            clicks: Vec::new(),
            layer: None,
            needs_tap: NeedsTap::default(),
        };
        w.poll();
        w.resized = false;
        Ok(w)
    }

    fn surface(&self) -> &wgpu::Surface<'static> {
        self.surface.as_ref().expect("the surface is lent until the window drops")
    }

    pub fn set_vsync(&mut self, vsync: bool) {
        let mode = present_mode(vsync, &self.modes);
        if mode != self.config.present_mode {
            self.config.present_mode = mode;
            self.surface().configure(&self.renderer.device, &self.config);
            println!("gpu: presenting {mode:?}");
        }
    }

    pub fn waits(&self) -> bool {
        self.config.present_mode == wgpu::PresentMode::Fifo
    }

    /// Take in what the main thread has put by.
    pub fn poll(&mut self) {
        let mut s = shared();
        self.size = s.size;
        self.safe = s.safe;
        self.resized |= std::mem::take(&mut s.resized);
        self.closed = s.closed;
        self.focused = s.focused;
        let [l, t, ..] = self.safe;
        self.clicks = s.clicks.drain(..).map(|(x, y)| (x - l as i32, y - t as i32)).collect();
        let touched = std::mem::take(&mut s.touched);
        drop(s);
        let was = self.needs_tap.up;
        self.needs_tap.frame(touched, crate::ios::take_live(), openmx_ios::focus_notice());
        if self.needs_tap.up != was {
            println!("controller needs a tap: {}", self.needs_tap.up);
        }
        let [l, t, r, b] = self.safe;
        self.width = self.size.0.saturating_sub(l + r).max(1) as usize;
        self.height = self.size.1.saturating_sub(t + b).max(1) as usize;
    }

    pub fn closed(&self) -> bool {
        self.closed
    }

    pub fn focused(&self) -> bool {
        self.focused
    }

    /// The menus' taps since the last poll, in the safe area's pixels.
    pub fn clicks(&self) -> &[(i32, i32)] {
        &self.clicks
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

    /// Draw one frame of `width` by `height` frame pixels, centred in the safe area at its scale,
    /// then the touch pad over it, and show it. `None` while the app is in the background, or the
    /// display slow to hand a frame back.
    pub fn draw<T>(
        &mut self,
        width: usize,
        height: usize,
        draw: impl FnOnce(&mut Renderer, &Target) -> T,
    ) -> Result<Option<T>, String> {
        if !self.focused {
            return Ok(None);
        }
        let size = self.size;
        if (self.config.width, self.config.height) != size {
            (self.config.width, self.config.height) = size;
            self.surface().configure(&self.renderer.device, &self.config);
        }
        let acquire = Instant::now();
        let texture = match self.surface().get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface().configure(&self.renderer.device, &self.config);
                match self.surface().get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                    other => return Err(format!("the surface gave no frame: {other:?}")),
                }
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return Ok(None),
            other => return Err(format!("the surface gave no frame: {other:?}")),
        };
        self.waited = acquire.elapsed();
        let view = texture.texture.create_view(&Default::default());
        let [l, t, ..] = self.safe;
        let (room_w, room_h) = (self.width as u32, self.height as u32);
        let (across, down) = ((width * self.scale) as u32, (height * self.scale) as u32);
        let (left, top) = (l + room_w.saturating_sub(across) / 2, t + room_h.saturating_sub(down) / 2);
        let target = Target {
            view: &view,
            size,
            frame: [left, top, across.min(room_w), down.min(room_h)],
            scale: self.scale as u32,
        };
        let out = draw(&mut self.renderer, &target);
        let mut sprites = {
            let touch = TOUCH.lock().unwrap_or_else(|e| e.into_inner());
            touch.sprites(0)
        };
        if self.needs_tap.up {
            needs_tap_sprites(&mut sprites, size);
        }
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

/// "Tap screen to resume controller" (P233) over the whole window, notch and home-bar strips too:
/// the menus' popup dimmer and their heavy slanted chalk type with its shadow, centred.
fn needs_tap_sprites(out: &mut Vec<game_render::gpu::Sprite>, (w, h): (u32, u32)) {
    use crate::menu::ink as art;
    let (w, h) = (w as f32, h as f32);
    art::fill(out, [0.0, 0.0, w, h], [0, 0, 0, 150]);
    let ink = art::ink();
    let size = w.min(h) * 0.06;
    let across = ink.measure(art::Weight::Heavy, NEEDS_TAP_TEXT, size);
    let (x, y) = ((w - across) / 2.0, h / 2.0 + size * (ink.ascent - ink.descent.abs()) / 2.0);
    let drop = size * 0.06;
    ink.text(out, 0, art::Weight::Heavy, NEEDS_TAP_TEXT, x + drop, y + drop, size, art::SLANT, [0, 0, 0, 150]);
    ink.text(out, 0, art::Weight::Heavy, NEEDS_TAP_TEXT, x, y, size, art::SLANT, [0xf5, 0xf0, 0xe8, 255]);
}

impl Drop for GpuWindow {
    /// The surface goes back for the next window.
    fn drop(&mut self) {
        if let Some(surface) = self.surface.take() {
            shared().surface = Some(surface);
        }
    }
}
