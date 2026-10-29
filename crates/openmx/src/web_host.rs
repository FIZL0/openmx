//! The page's side of the desktop shell (P196): what a browser has in place of the desktop's window,
//! pads, speakers and files, answered by `web/worker.js` through the host calls below, on the global
//! `openmx` object the worker sets up (web.rs lists the first four, this file the rest).
//!
//! * [`Canvas`] is the shell's window ([`crate::screen`]'s software one): its shape is the page's
//!   canvas, which fills the window in device pixels and follows a resize or a turn of the phone, and
//!   its size that shape at no more than the PS2's own frame's pixels ([`PIXELS`]), which the page
//!   stretches over the whole canvas; a frame goes up with `present`; its keys are the page's key events, put on X keycodes
//!   ([`crate::window::key`]) by the page so the shell's keyboard layout is the desktop's.
//! * [`PadNode`] is a pad: a gamepad of the Gamepad API's standard mapping, its buttons and axes put
//!   out as the joystick interface's eight-byte events in the order Linux gives a gamepad, so
//!   [`crate::pad`] reads it as it reads `/dev/input/js*` — the same map, dead zones and seats.
//! * The speakers are `play`: the mixer's stereo floats, which the page queues into WebAudio.
//! * The files beside the disc ([`crate::files`]) are the page's origin-private storage: read once
//!   before the shell starts, written back as they change.
//! * [`next_frame`] is the end of a picture: the page's turn, until its next animation frame.
//!
//! Lives at the crate's root, not in `main.rs`'s tree: wasm-bindgen's glue is unsafe code, which the
//! shell forbids.

use std::io::{Read, Write};
use std::sync::{Mutex, Weak};

use wasm_bindgen::prelude::*;

use crate::audio::{Mixer, RATE};
use crate::window::{KEYCODES, KeyMoment, shell_us};

#[wasm_bindgen]
extern "C" {
    /// The canvas's size in device pixels: `[width, height]`.
    #[wasm_bindgen(js_namespace = openmx)]
    fn view() -> Vec<u32>;
    /// The keys gone down and up since the last call, each `code << 1 | down`, X keycodes.
    #[wasm_bindgen(js_namespace = openmx)]
    fn keys() -> Vec<u32>;
    /// Where the pointer went down since the last call, in canvas pixels: `x, y, x, y…`.
    #[wasm_bindgen(js_namespace = openmx)]
    fn clicks() -> Vec<i32>;
    /// Every gamepad connected, [`PAD_FIELDS`] numbers each ([`Pad`]).
    #[wasm_bindgen(js_namespace = openmx)]
    fn gamepads() -> Vec<f32>;
    /// A gamepad's name, by its index.
    #[wasm_bindgen(js_namespace = openmx)]
    fn gamepad_name(index: u32) -> String;
    /// The mixer's next stereo frames, interleaved, at [`crate::audio`]'s rate.
    #[wasm_bindgen(js_namespace = openmx)]
    fn play(samples: &[f32]);
    /// How many stereo frames the page has queued and not played yet.
    #[wasm_bindgen(js_namespace = openmx)]
    fn queued() -> u32;
    /// A file of the page's storage, if there is one.
    #[wasm_bindgen(js_namespace = openmx)]
    fn file_read(path: &str) -> Option<Vec<u8>>;
    /// A file of the page's storage written, or (`None`) removed.
    #[wasm_bindgen(js_namespace = openmx)]
    fn file_write(path: &str, bytes: Option<Vec<u8>>);
    /// A promise of the page's next animation frame.
    #[wasm_bindgen(js_namespace = openmx)]
    fn frame() -> js_sys::Promise;
    /// The GPU's first frame is up (P48): which API drew it (`webgpu`, `webgl2`) and the canvas's
    /// size. Until the page hears it, a failure is the GPU's, and the page starts again on the next
    /// API down.
    #[wasm_bindgen(js_namespace = openmx, js_name = drawn)]
    fn host_drawn(api: &str, width: u32, height: u32);
    /// What is about to draw the picture (P48): `webgpu` or `webgl2` as its device is asked for,
    /// `software` once neither is to be had.
    #[wasm_bindgen(js_namespace = openmx)]
    fn gpu(api: &str);
    /// The mod store (P198): `url` fetched into the page's storage as the download `name`, streamed
    /// through a sync access handle; a promise of its bytes, refused with why.
    #[wasm_bindgen(js_namespace = openmx, js_name = store_fetch)]
    fn host_store_fetch(url: &str, name: &str, size: f64) -> js_sys::Promise;
    /// How many bytes of the download `name` are in so far.
    #[wasm_bindgen(js_namespace = openmx, js_name = store_got)]
    fn host_store_got(name: &str) -> f64;
    /// `n` bytes of the download `name` from `at`, or none.
    #[wasm_bindgen(js_namespace = openmx, js_name = store_read)]
    fn host_store_read(name: &str, at: f64, n: u32) -> Option<Vec<u8>>;
    /// The download `name` closed and removed.
    #[wasm_bindgen(js_namespace = openmx, js_name = store_close)]
    fn host_store_close(name: &str);
    /// A promise of an installed mod's file written into the page's `mods/` (`mods/<kind>/<id>/…`).
    #[wasm_bindgen(js_namespace = openmx, js_name = store_put)]
    fn host_store_put(path: &str, bytes: &[u8]) -> js_sys::Promise;
    /// A promise of a folder of the page's `mods/` removed, all of it.
    #[wasm_bindgen(js_namespace = openmx, js_name = store_drop)]
    fn host_store_drop(path: &str) -> js_sys::Promise;
    /// A page opened in a new tab (the store's Report).
    #[wasm_bindgen(js_namespace = openmx, js_name = open_url)]
    fn host_open_url(url: &str);
}

/// A refused promise's reason, as text.
fn refused(e: JsValue) -> String {
    e.as_string().unwrap_or_else(|| format!("{e:?}"))
}

/// [`crate::modstore`]'s downloads, files and pages, on the page (`web/worker.js`).
pub async fn store_fetch(url: &str, name: &str, size: u64) -> Result<u64, String> {
    let got = wasm_bindgen_futures::JsFuture::from(host_store_fetch(url, name, size as f64)).await.map_err(refused)?;
    Ok(got.as_f64().unwrap_or(0.0) as u64)
}

pub fn store_got(name: &str) -> u64 {
    host_store_got(name) as u64
}

pub fn store_read(name: &str, at: u64, n: usize) -> Result<Vec<u8>, String> {
    host_store_read(name, at as f64, n as u32).filter(|b| b.len() == n).ok_or_else(|| "the zip is cut short".to_string())
}

pub fn store_close(name: &str) {
    host_store_close(name)
}

pub async fn store_put(path: &str, bytes: &[u8]) -> Result<(), String> {
    wasm_bindgen_futures::JsFuture::from(host_store_put(path, bytes)).await.map(|_| ()).map_err(refused)
}

pub async fn store_drop(path: &str) -> Result<(), String> {
    wasm_bindgen_futures::JsFuture::from(host_store_drop(path)).await.map(|_| ()).map_err(refused)
}

pub fn open_url(url: &str) {
    host_open_url(url)
}

/// [`gpu`]: told before each API's device is asked for, so a panic in the asking is that API's.
pub fn using(api: &str) {
    gpu(api)
}

/// [`host_drawn`]: the GPU window's first frame (`web_gpu.rs`).
pub fn drawn(api: &str, width: u32, height: u32) {
    host_drawn(api, width, height)
}

/// One line to the page's console.
pub fn say(line: &str) {
    crate::host::say(line)
}

/// Back to the page until its next animation frame, the speakers topped up first.
pub async fn next_frame() {
    feed();
    let _ = wasm_bindgen_futures::JsFuture::from(frame()).await;
}

/// How far ahead of what the page has played the mix is written: the desktop's lead, and a frame
/// or two of the page's clock to cover a late one.
const AHEAD: f64 = 0.1;

thread_local! {
    /// The mixer the page plays, while the shell's speakers keep it.
    static MIXER: std::cell::RefCell<Weak<Mutex<Mixer>>> = std::cell::RefCell::new(Weak::new());
}

/// The speakers ([`crate::audio::Output`]): this mixer from now on.
pub fn sound_from(mixer: Weak<Mutex<Mixer>>) {
    MIXER.with(|m| *m.borrow_mut() = mixer);
}

/// The mixer rendered as far as [`AHEAD`] of what the page has played, and handed it.
fn feed() {
    let Some(mixer) = MIXER.with(|m| m.borrow().upgrade()) else { return };
    let want = (AHEAD * RATE as f64) as usize;
    let have = queued() as usize;
    if have >= want {
        return;
    }
    let mut out = vec![0i16; (want - have) * 2];
    match mixer.lock() {
        Ok(mut m) => m.render(&mut out),
        Err(_) => return,
    }
    let floats: Vec<f32> = out.iter().map(|&s| s as f32 / 32768.0).collect();
    play(&floats);
}

/// The most pixels a software frame is drawn with: the PS2's own 640×448. The software renderer is
/// the desktop's rasteriser on one thread, the page's last fallback, and a window's worth of pixels
/// is a picture a second; at this many the page stretches a frame of the window's shape over it.
/// The GPU's frame (P48's `web_gpu.rs`, the page's renderer wherever WebGPU or WebGL2 can be had)
/// is not held to it: it is the canvas's whole size in device pixels, a phone's 2532×1170 at 30 fps
/// and more (P205).
pub const PIXELS: usize = 640 * 448;

/// The frame for a canvas of `view` device pixels: its shape, at no more than [`PIXELS`].
pub fn frame_of(view: (usize, usize)) -> (usize, usize) {
    let (w, h) = (view.0.max(1), view.1.max(1));
    let k = ((PIXELS as f64) / (w * h) as f64).sqrt().min(1.0);
    (((w as f64 * k) as usize).max(1), ((h as f64 * k) as usize).max(1))
}

/// The canvas, as the shell's window.
pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pub scale: usize,
    pub screen: (usize, usize),
    keys: [bool; KEYCODES],
    tapped: [bool; KEYCODES],
    moments: Vec<KeyMoment>,
    clicks: Vec<(i32, i32)>,
    resized: bool,
    rgba: Vec<u8>,
    /// The canvas in device pixels, which a press is given in.
    view: (usize, usize),
    /// Whether the frame is the canvas's whole size (the GPU window, P48) rather than [`frame_of`]
    /// it.
    whole: bool,
}

impl Canvas {
    /// The page's canvas; the arguments are the desktop's, and the canvas is the size the page made it.
    pub fn open(_: &str, _: Option<(usize, usize)>, _: usize, _: &str) -> Result<Canvas, String> {
        let mut c = Canvas {
            width: 0,
            height: 0,
            scale: 1,
            screen: (0, 0),
            keys: [false; KEYCODES],
            tapped: [false; KEYCODES],
            moments: Vec::new(),
            clicks: Vec::new(),
            resized: false,
            rgba: Vec::new(),
            view: (1, 1),
            whole: false,
        };
        c.size();
        c.resized = false;
        Ok(c)
    }

    /// The page's canvas as the GPU window reads it (P48): its keys and pointer, at its whole size
    /// in device pixels, which the GPU draws at.
    pub fn open_whole(title: &str) -> Result<Canvas, String> {
        let mut c = Canvas::open(title, None, 1, "")?;
        c.whole = true;
        c.size();
        c.resized = false;
        Ok(c)
    }

    fn size(&mut self) {
        let v = view();
        self.view = (v.first().copied().unwrap_or(640).max(1) as usize, v.get(1).copied().unwrap_or(448).max(1) as usize);
        let (w, h) = if self.whole { self.view } else { frame_of(self.view) };
        if (w, h) != (self.width, self.height) {
            (self.width, self.height) = (w, h);
            self.screen = (w, h);
            self.resized = true;
        }
    }

    pub fn poll(&mut self) {
        self.tapped = [false; KEYCODES];
        self.moments.clear();
        let now = shell_us();
        for k in keys() {
            let (code, down) = ((k >> 1) as u8, k & 1 != 0);
            if down && !self.keys[code as usize] {
                self.tapped[code as usize] = true;
            }
            if down != self.keys[code as usize] {
                self.moments.push((now, code, down));
            }
            self.keys[code as usize] = down;
        }
        self.size();
        let (sx, sy) = (self.width as f64 / self.view.0 as f64, self.height as f64 / self.view.1 as f64);
        self.clicks = clicks().chunks_exact(2).map(|p| ((p[0] as f64 * sx) as i32, (p[1] as f64 * sy) as i32)).collect();
    }

    pub fn closed(&self) -> bool {
        false
    }

    pub fn focused(&self) -> bool {
        true
    }

    pub fn show_cursor(&mut self, _: bool) {}

    pub fn held(&self, code: u8) -> bool {
        self.keys[code as usize]
    }

    pub fn tapped(&self, code: u8) -> bool {
        self.tapped[code as usize]
    }

    pub fn clicks(&self) -> &[(i32, i32)] {
        &self.clicks
    }

    pub fn key_moments(&self) -> &[KeyMoment] {
        &self.moments
    }

    pub fn resized(&mut self) -> bool {
        std::mem::take(&mut self.resized)
    }

    /// One frame up, as RGBA, the size it was drawn: the page stretches it over the canvas.
    pub fn present(&mut self, rgb: &[u8], width: usize, height: usize) -> Result<(), String> {
        self.rgba.clear();
        self.rgba.reserve(width * height * 4);
        for p in rgb.chunks_exact(3).take(width * height) {
            self.rgba.extend_from_slice(&[p[0], p[1], p[2], 0xff]);
        }
        crate::host_present(&self.rgba, width as u32, height as u32);
        Ok(())
    }
}

/// window.rs's display socket, which a page has none of: it never connects.
pub struct NoSocket;

impl NoSocket {
    pub fn connect(_: &str) -> std::io::Result<NoSocket> {
        Err(std::io::Error::other("a browser has no X display"))
    }

    pub fn set_nonblocking(&self, _: bool) -> std::io::Result<()> {
        Ok(())
    }
}

impl Read for NoSocket {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        Ok(0)
    }
}

impl Write for NoSocket {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        Ok(b.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The numbers `gamepads` gives each pad: its index, its 17 buttons' values (0..1) and its 4 axes
/// (-1..1), in the standard mapping's order.
pub const PAD_FIELDS: usize = 1 + 17 + 4;

/// One gamepad's state, as the page last said it.
#[derive(Clone, Copy, PartialEq)]
struct Pad {
    buttons: [f32; 17],
    axes: [f32; 4],
}

fn snapshot() -> Vec<(u32, Pad)> {
    gamepads()
        .chunks_exact(PAD_FIELDS)
        .map(|f| {
            let mut pad = Pad { buttons: [0.0; 17], axes: [0.0; 4] };
            pad.buttons.copy_from_slice(&f[1..18]);
            pad.axes.copy_from_slice(&f[18..22]);
            (f[0] as u32, pad)
        })
        .collect()
}

/// The gamepads connected, as the pad layer's device nodes: `gamepad/<index>`, numbered as the
/// joystick interface's `jsN` are.
pub fn pads() -> Vec<(u32, std::path::PathBuf)> {
    snapshot().into_iter().map(|(i, _)| (i, format!("gamepad/{i}").into())).collect()
}

fn index_of(path: &std::path::Path) -> Option<u32> {
    path.to_str()?.strip_prefix("gamepad/")?.parse().ok()
}

/// What the pad layer knows of a device ([`crate::pad::DeviceInfo`]): its name. No sysfs, so the
/// joystick interface's own order, which is the one these events are put out in.
pub fn pad_name(node: &str) -> String {
    node.parse().map(gamepad_name).unwrap_or_default()
}

/// The joystick interface's js numbers for the standard mapping's buttons: south, east, west, north,
/// the shoulders, select, start, the guide, the two stick clicks (Linux's gamepad order).
const JS_BUTTON: [(usize, u8); 11] = [(0, 0), (1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (8, 6), (9, 7), (16, 8), (10, 9), (11, 10)];

/// A gamepad read as a js device: each [`Read`] one event, the changes since the last snapshot
/// queued as they are found; a pad the page no longer lists reads as the end of the file.
pub struct PadNode {
    index: u32,
    last: Option<Pad>,
    queue: std::collections::VecDeque<[u8; 8]>,
    gone: bool,
}

impl PadNode {
    pub fn open(path: &std::path::Path) -> std::io::Result<PadNode> {
        let index = index_of(path).ok_or_else(|| std::io::Error::other("not a gamepad"))?;
        Ok(PadNode { index, last: None, queue: Default::default(), gone: false })
    }

    fn event(&mut self, kind: u8, number: u8, value: i16, init: bool) {
        let ms = (shell_us() / 1000) as u32;
        let mut e = [0u8; 8];
        e[..4].copy_from_slice(&ms.to_le_bytes());
        e[4..6].copy_from_slice(&value.to_le_bytes());
        e[6] = kind | if init { 0x80 } else { 0 };
        e[7] = number;
        self.queue.push_back(e);
    }

    fn take(&mut self) {
        let Some((_, now)) = snapshot().into_iter().find(|(i, _)| *i == self.index) else {
            self.gone = true;
            return;
        };
        let init = self.last.is_none();
        let was = self.last.unwrap_or(now);
        let axis = |v: f32| (v.clamp(-1.0, 1.0) * 32767.0) as i16;
        let trigger = |v: f32| (v.clamp(0.0, 1.0) * 65534.0 - 32767.0) as i16;
        for (from, js) in JS_BUTTON {
            let down = now.buttons[from] > 0.5;
            if init || down != (was.buttons[from] > 0.5) {
                self.event(0x01, js, down as i16, init);
            }
        }
        // The left stick, the left trigger, the right stick, the right trigger, then the d-pad as a
        // hat: the joystick interface's axis order for a gamepad.
        let value = |p: &Pad, n: u8| match n {
            0 => axis(p.axes[0]),
            1 => axis(p.axes[1]),
            2 => trigger(p.buttons[6]),
            3 => axis(p.axes[2]),
            4 => axis(p.axes[3]),
            5 => trigger(p.buttons[7]),
            6 => axis((p.buttons[15] - p.buttons[14]).round()),
            _ => axis((p.buttons[13] - p.buttons[12]).round()),
        };
        for n in 0..8 {
            let v = value(&now, n);
            if init || v != value(&was, n) {
                self.event(0x02, n, v, init);
            }
        }
        self.last = Some(now);
    }
}

impl Read for PadNode {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.queue.is_empty() && !self.gone {
            self.take();
            if self.queue.is_empty() && !self.gone {
                return Err(std::io::ErrorKind::WouldBlock.into());
            }
        }
        match self.queue.pop_front() {
            Some(e) => {
                let n = e.len().min(buf.len());
                buf[..n].copy_from_slice(&e[..n]);
                Ok(n)
            }
            None => Ok(0),
        }
    }
}

/// [`crate::files`]' reads and writes, on the page's storage.
pub fn read(path: &std::path::Path) -> Option<Vec<u8>> {
    file_read(&path.to_string_lossy())
}

pub fn write(path: &std::path::Path, bytes: Option<Vec<u8>>) {
    file_write(&path.to_string_lossy(), bytes)
}

