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
    // behind it. A canvas keeps the context it was first given, whatever a later call asks for, so
    // it is taken here first, opaque, and wgpu's own ask returns this one.
    let opts = js_sys::Object::new();
    for (k, v) in [("alpha", false), ("antialias", false)] {
        js_sys::Reflect::set(&opts, &k.into(), &v.into()).map_err(|e| format!("{e:?}"))?;
    }
    match canvas.get_context_with_context_options("webgl2", &opts) {
        Ok(Some(_)) => {}
        Ok(None) => return Err("the canvas gave no webgl2 context".into()),
        Err(e) => return Err(format!("the canvas refused a webgl2 context: {e:?}")),
    }
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::GL;
    let instance = wgpu::Instance::new(desc);
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::OffscreenCanvas(canvas.clone()))
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
    let renderer = Renderer::request(adapter, format).await?;
    renderer.device.on_uncaptured_error(Arc::new(|e: wgpu::Error| {
        if let Ok(mut slot) = ERROR.lock() {
            slot.get_or_insert(format!("gpu: {e}"));
        }
    }));
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
    /// WebGL2 when it came before the first frame.
    pub fn draw<T>(
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
        let out = draw(&mut self.renderer, &target);
        self.renderer.queue.present(texture);
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
