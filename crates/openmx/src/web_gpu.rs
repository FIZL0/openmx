//! The browser's GPU window (P48): the page's canvas with a wgpu surface on it — WebGPU where the
//! browser grants an adapter, WebGL2 where it does not — and the same calls as `gpuwin.rs`'s, so the
//! shell's loop draws in the page exactly as it draws on the desktop, HUD patches and all.
//!
//! A browser cannot block on the adapter or the device, so they are asked for before the shell
//! starts: [`prepare`] is awaited by `openmx_run` (web.rs), a promise the worker drives, and leaves
//! what it got for [`GpuWindow::open`] to take. With nothing prepared — `?software`, no canvas, or
//! neither backend to be had — `open` says why and the screen falls back to the software canvas
//! ([`crate::web_host::Canvas`]).
//!
//! The canvas is an `OffscreenCanvas` the page handed the worker; a surface made on it draws from the
//! worker as the desktop's draws into its window. Its size is the page's canvas in device pixels,
//! and the picture is shown when the worker hands the page its turn ([`crate::pace::shown`]): once an
//! animation frame, the display's own rate.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use game_render::gpu::{Renderer, Target, wgpu};

use crate::web_host::Canvas;

/// Which of the browser's GPU interfaces draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Api {
    WebGpu,
    WebGl2,
}

impl Api {
    pub fn name(self) -> &'static str {
        match self {
            Api::WebGpu => "webgpu",
            Api::WebGl2 => "webgl2",
        }
    }
}

/// A surface and a device for it, made before the shell starts.
struct Prepared {
    api: Api,
    canvas: web_sys::OffscreenCanvas,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
}

thread_local! {
    static PREPARED: RefCell<Option<Prepared>> = const { RefCell::new(None) };
    /// Why there is no GPU window, when [`prepare`] found none: what `open` says.
    static WHY: RefCell<String> = RefCell::new("the page gave no canvas".into());
}

/// A validation error the device reported, until the next frame reads it.
static ERROR: Mutex<Option<String>> = Mutex::new(None);

fn error() -> Option<String> {
    ERROR.lock().ok().and_then(|mut e| e.take())
}

/// Every validation error to the page's console with what made it, and the first kept for the
/// next frame (P257): the browser's own words name the call that failed only with its source.
fn report(source: &str, e: &wgpu::Error) {
    let e = said(e);
    crate::web_host::say(&format!("gpu: validation error in {source}: {e}"));
    if let Ok(mut slot) = ERROR.lock() {
        slot.get_or_insert(format!("gpu: {e} (in {source})"));
    }
}

/// An error's words with each cause under it that they do not already hold (P336): wgpu's own
/// validation keeps the failing call's error as the source of the one it reports. A browser's
/// cause is only a placeholder (`<WebGPU Error>`), left out.
fn said(e: &wgpu::Error) -> String {
    let mut text = e.to_string().trim().to_string();
    let mut cause = std::error::Error::source(e);
    while let Some(c) = cause {
        let more = c.to_string().trim().to_string();
        let placeholder = more.starts_with('<') && more.ends_with('>');
        if !more.is_empty() && !placeholder && !text.contains(&more) {
            text = format!("{text}: {more}");
        }
        cause = c.source();
    }
    text
}

/// The frame being drawn, or 0 outside one: what names a stage's scope with its frame (P336).
static FRAME: AtomicU64 = AtomicU64::new(0);

/// The [`game_render::gpu::Watcher`] the renderer hands its error scopes: each popped, and what it
/// caught reported, with the frame it was made in. A frame's stages each have one
/// (`Renderer::stage`, P336), so an encoder WebKit calls only "invalid" is named by what it held.
fn watcher() -> game_render::gpu::Watcher {
    Arc::new(|what: &'static str, scope: wgpu::ErrorScopeGuard| {
        let popped = scope.pop();
        let source = match FRAME.load(Ordering::Relaxed) {
            0 => what.to_string(),
            f => format!("{what}, frame {f}"),
        };
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(e) = popped.await {
                report(&source, &e);
            }
        });
    })
}

/// Whether a ride has started (P257): a WebGPU validation error before one — in the splash, the
/// menus — tells the page so ([`FRONT`]), and the page starts again on WebGL2.
static RODE: AtomicBool = AtomicBool::new(false);

/// What starts a WebGPU validation error the front end ends on: web/index.html's `fallBack` reads it.
pub const FRONT: &str = "webgpu failed in the front end: ";

/// What starts a WebGPU validation error a ride ends on (P336): the shell leaves with it rather than
/// going back to its menu, and web/index.html's `fallBack` starts the page again on WebGL2. A phone's
/// Safari broke every ride's first frame, its menus drawing well, so going back to the menu on WebGPU
/// only failed the next ride the same way.
pub const RIDE: &str = "webgpu failed in a ride: ";

/// A ride starts: a WebGPU validation error from now on ends the ride, starting with [`RIDE`].
pub fn riding() {
    RODE.store(true, Ordering::Relaxed);
}

/// The app is in front (MD97, `ios_gpu.rs`): always here, where the GPU takes work unfocused too.
pub fn in_front() -> bool {
    true
}

/// How many times the app has been sent away (MD97, `ios_gpu.rs`): never here.
pub fn times_away() -> u32 {
    0
}

/// Make the surface on `canvas` and the renderer for it: WebGPU first, unless `webgl2` asks for
/// WebGL2 only, then WebGL2. The API that drew, or why neither could.
pub async fn prepare(canvas: web_sys::OffscreenCanvas, webgl2: bool) -> Result<Api, String> {
    let mut why = Vec::new();
    if !webgl2 {
        match webgpu(&canvas).await {
            Ok(p) => return Ok(keep(p)),
            Err(e) => why.push(format!("webgpu: {e}")),
        }
    }
    match webgl(&canvas).await {
        Ok(p) => Ok(keep(p)),
        Err(e) => {
            why.push(format!("webgl2: {e}"));
            let why = why.join("; ");
            WHY.with(|w| *w.borrow_mut() = why.clone());
            Err(why)
        }
    }
}

fn keep(p: Prepared) -> Api {
    let api = p.api;
    PREPARED.with(|s| *s.borrow_mut() = Some(p));
    api
}

/// WebGPU: the adapter asked for before the surface, so a browser that grants none leaves the
/// canvas without a context, free for WebGL2 or the software path's 2D one.
async fn webgpu(canvas: &web_sys::OffscreenCanvas) -> Result<Prepared, String> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::BROWSER_WEBGPU;
    // `Instance::new` falls back to wgpu-core (WebGL2) when the browser has no `navigator.gpu`.
    if !wgpu::util::is_browser_webgpu_supported().await {
        return Err("the browser has no WebGPU adapter".into());
    }
    let instance = wgpu::Instance::new(desc);
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        })
        .await
        .map_err(|e| format!("no adapter: {e}"))?;
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::OffscreenCanvas(canvas.clone()))
        .map_err(|e| format!("no surface: {e}"))?;
    finish(Api::WebGpu, canvas, &adapter, surface).await
}

/// WebGL2: its adapter comes from the canvas's context, so the surface is made first.
async fn webgl(canvas: &web_sys::OffscreenCanvas) -> Result<Prepared, String> {
    // wgpu asks for its context with an alpha channel, which the page then composites: whatever the
    // frame leaves below opaque (the sky, the HUD's translucent pills) showed the page's black
    // behind it. A canvas keeps the context it was first given, but the context cannot be taken
    // here first and wgpu's own ask left to return it: Firefox's worker throws on a canvas's second
    // getContext ('Cannot update created context to different thread', B13). So wgpu's one ask is
    // made opaque instead: while wgpu makes its surface the canvas has its own getContext, which adds
    // `alpha: false` and calls the real one.
    let instance = {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::GL;
        wgpu::Instance::new(desc)
    };
    let surface = opaque(canvas, || {
        instance.create_surface(wgpu::SurfaceTarget::OffscreenCanvas(canvas.clone()))
    })?
    .map_err(|e| format!("no surface: {e}"))?;
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            ..Default::default()
        })
        .await
        .map_err(|e| format!("no adapter: {e}"))?;
    finish(Api::WebGl2, canvas, &adapter, surface).await
}

/// Run `make` with `canvas.getContext` asking for an opaque, unsmoothed context: an own property
/// that shadows the prototype's while `make` runs, and is gone after it.
fn opaque<T>(canvas: &web_sys::OffscreenCanvas, make: impl FnOnce() -> T) -> Result<T, String> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsValue;
    let js = |e: JsValue| format!("{e:?}");
    let target: JsValue = canvas.clone().into();
    // The prototype's, read before the canvas has one of its own.
    let real: js_sys::Function = js_sys::Reflect::get(&target, &"getContext".into())
        .map_err(js)?
        .dyn_into()
        .map_err(|_| "the canvas has no getContext".to_string())?;
    let this = target.clone();
    let asked = std::rc::Rc::new(std::cell::Cell::new(false));
    let seen = asked.clone();
    type Ask = dyn FnMut(JsValue, JsValue) -> Result<JsValue, JsValue>;
    let ask = Closure::<Ask>::new(
        move |id: JsValue, given: JsValue| {
            let opts = js_sys::Object::new();
            if given.is_object() {
                js_sys::Object::assign(&opts, given.unchecked_ref());
            }
            for (k, v) in [("alpha", false), ("antialias", false)] {
                js_sys::Reflect::set(&opts, &k.into(), &v.into())?;
            }
            seen.set(true);
            real.call2(&this, &id, &opts)
        },
    );
    js_sys::Reflect::set(&target, &"getContext".into(), ask.as_ref()).map_err(js)?;
    let made = make();
    js_sys::Reflect::delete_property(target.unchecked_ref::<js_sys::Object>(), &"getContext".into()).map_err(js)?;
    if !asked.get() {
        crate::web_host::say("gpu: wgpu took the canvas's context without its getContext; the picture may not be opaque");
    }
    Ok(made)
}

async fn finish(
    api: Api,
    canvas: &web_sys::OffscreenCanvas,
    adapter: &wgpu::Adapter,
    surface: wgpu::Surface<'static>,
) -> Result<Prepared, String> {
    let caps = surface.get_capabilities(adapter);
    // A plain eight-bit format, as the desktop's window takes (gpuwin.rs).
    let plain = [wgpu::TextureFormat::Bgra8Unorm, wgpu::TextureFormat::Rgba8Unorm];
    let format = plain
        .into_iter()
        .find(|f| caps.formats.contains(f))
        .or_else(|| caps.formats.iter().copied().find(|f| !f.is_srgb()))
        .or_else(|| caps.formats.first().copied())
        .ok_or("the surface takes no format")?;
    crate::web_host::using(api.name());
    // What the adapter can take, which the renderer's limits are raised to (gpu.rs's
    // `Renderer::request`): a phone's are the first thing a phone's failure is read against.
    let l = adapter.limits();
    crate::web_host::say(&format!(
        "gpu: adapter limits: 2D {}, layers {}, buffer {}, sampled textures a stage {}, samplers a stage {}, bind groups {}, inter-stage variables {}",
        l.max_texture_dimension_2d,
        l.max_texture_array_layers,
        l.max_buffer_size,
        l.max_sampled_textures_per_shader_stage,
        l.max_samplers_per_shader_stage,
        l.max_bind_groups,
        l.max_inter_stage_shader_variables,
    ));
    let renderer = Renderer::request_watched(adapter, format, Some(watcher())).await?;
    renderer.device.on_uncaptured_error(Arc::new(|e: wgpu::Error| report("an unwatched call", &e)));
    let (width, height) = (canvas.width().max(1), canvas.height().max(1));
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width,
        height,
        // The page's animation frame paces the pictures (`pace::shown`): the browser shows each at
        // the display's next refresh, which Fifo is.
        present_mode: wgpu::PresentMode::Fifo,
        desired_maximum_frame_latency: 1,
        // Opaque, so nothing of the page shows through what the frame leaves translucent.
        alpha_mode: if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            wgpu::CompositeAlphaMode::Opaque
        } else {
            caps.alpha_modes.first().copied().unwrap_or(wgpu::CompositeAlphaMode::Auto)
        },
        view_formats: vec![],
        color_space: wgpu::SurfaceColorSpace::Auto,
    };
    surface.configure(&renderer.device, &config);
    crate::web_host::say(&format!("gpu: {} into a {format:?} canvas ({})", renderer.adapter, api.name()));
    Ok(Prepared { api, canvas: canvas.clone(), surface, config, renderer })
}

/// The canvas and the GPU drawing into it.
pub struct GpuWindow {
    pub renderer: Renderer,
    pub width: usize,
    pub height: usize,
    pub scale: usize,
    pub screen: (usize, usize),
    pub waited: Duration,
    /// The browser does not say the display's rate; its animation frames keep to it whatever it is.
    pub refresh: Option<f32>,
    api: Api,
    canvas: web_sys::OffscreenCanvas,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    /// The page's keys, pointer and size, read as the software canvas reads them.
    input: Canvas,
    /// Whether a frame has been shown yet: the first one tells the page which API drew it.
    shown: bool,
    /// Frames drawn, which name the error scope around each (P257).
    frames: u64,
}

impl GpuWindow {
    /// The window [`prepare`] made, at the page canvas's size in device pixels.
    pub fn open(_: Option<(usize, usize)>, _: usize, title: &str, _: bool) -> Result<GpuWindow, String> {
        let Some(p) = PREPARED.with(|s| s.borrow_mut().take()) else {
            return Err(WHY.with(|w| w.borrow().clone()));
        };
        let input = Canvas::open_whole(title)?;
        let (width, height) = (input.width, input.height);
        Ok(GpuWindow {
            renderer: p.renderer,
            width,
            height,
            scale: 1,
            screen: (width, height),
            waited: Duration::ZERO,
            refresh: None,
            api: p.api,
            canvas: p.canvas,
            surface: p.surface,
            config: p.config,
            input,
            shown: false,
            frames: 0,
        })
    }

    /// The present always waits on the page's animation frame: there is no VSync to turn off.
    pub fn set_vsync(&mut self, _: bool) {}

    pub fn waits(&self) -> bool {
        true
    }

    pub fn poll(&mut self) {
        self.input.poll();
        (self.width, self.height) = (self.input.width, self.input.height);
        self.screen = (self.width, self.height);
    }

    pub fn closed(&self) -> bool {
        false
    }

    pub fn focused(&self) -> bool {
        true
    }

    pub fn clicks(&self) -> &[(i32, i32)] {
        self.input.clicks()
    }

    /// The pointer's drags since the last poll (P319, the Tricks page's orbit).
    pub fn mouse(&self) -> &[crate::window::MouseEvent] {
        self.input.mouse()
    }

    pub fn show_cursor(&self, _: bool) {}

    pub fn held(&self, code: u8) -> bool {
        self.input.held(code)
    }

    pub fn key_moments(&self) -> &[crate::window::KeyMoment] {
        self.input.key_moments()
    }

    pub fn tapped(&self, code: u8) -> bool {
        self.input.tapped(code)
    }

    pub fn resized(&mut self) -> bool {
        self.input.resized()
    }

    /// Draw one frame of `width` by `height` frame pixels, centred in the canvas, as the desktop's
    /// window does. A WebGPU validation error ends the ride with it, so the page can start again on
    /// WebGL2 when it came before the first frame, or before the first ride ([`FRONT`]).
    pub fn draw<T>(
        &mut self,
        width: usize,
        height: usize,
        draw: impl FnOnce(&mut Renderer, &Target) -> T,
    ) -> Result<Option<T>, String> {
        let r = self.draw_frame(width, height, draw);
        match r {
            Err(e) if self.api == Api::WebGpu && e.starts_with("gpu: ") => {
                Err(format!("{}{e}", if RODE.load(Ordering::Relaxed) { RIDE } else { FRONT }))
            }
            r => r,
        }
    }

    fn draw_frame<T>(
        &mut self,
        width: usize,
        height: usize,
        draw: impl FnOnce(&mut Renderer, &Target) -> T,
    ) -> Result<Option<T>, String> {
        if let Some(e) = error() {
            return Err(e);
        }
        let size = (self.width.max(1) as u32, self.height.max(1) as u32);
        if (self.config.width, self.config.height) != size {
            (self.config.width, self.config.height) = size;
            self.surface.configure(&self.renderer.device, &self.config);
        }
        let texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.renderer.device, &self.config);
                match self.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                    other => return Err(format!("the canvas gave no frame: {other:?}")),
                }
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return Ok(None),
            other => return Err(format!("the canvas gave no frame: {other:?}")),
        };
        let view = texture.texture.create_view(&Default::default());
        let (across, down) = ((width * self.scale) as u32, (height * self.scale) as u32);
        let (left, top) = (size.0.saturating_sub(across) / 2, size.1.saturating_sub(down) / 2);
        let target = Target {
            view: &view,
            size,
            frame: [left, top, across.min(size.0), down.min(size.1)],
            scale: self.scale as u32,
        };
        // The frame in a scope of its own, so what it made wrong is named by its frame; a create
        // the renderer watches is named by its own scope inside this one.
        self.frames += 1;
        let scope = self.renderer.device.push_error_scope(wgpu::ErrorFilter::Validation);
        FRAME.store(self.frames, Ordering::Relaxed);
        let out = draw(&mut self.renderer, &target);
        FRAME.store(0, Ordering::Relaxed);
        self.renderer.queue.present(texture);
        let (frame, popped) = (self.frames, scope.pop());
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(e) = popped.await {
                report(&format!("frame {frame}"), &e);
            }
        });
        if let Some(e) = error() {
            return Err(e);
        }
        if !self.shown {
            self.shown = true;
            crate::web_host::drawn(self.api.name(), self.canvas.width(), self.canvas.height());
        }
        Ok(Some(out))
    }

    /// Show a frame the CPU drew.
    pub fn present(&mut self, rgb: &[u8], width: usize, height: usize) -> Result<(), String> {
        self.draw(width, height, |r, target| r.blit(target, rgb, width as u32, height as u32))
            .map(|_| ())
    }

    /// Show the frame [`GpuWindow::present`] last showed, which the GPU still has.
    pub fn present_again(&mut self, width: usize, height: usize) -> Result<(), String> {
        self.draw(width, height, |r, target| r.blit_again(target)).map(|_| ())
    }
}
