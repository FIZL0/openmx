//! The iPhone app (P200), as opentennis's M91 is: the shell as it is on the desktop, its window
//! winit's UIKit one ([`crate::gpuwin`], `ios_gpu.rs`) and its pads the GameController framework's
//! and the on-screen [`TouchPad`], everything the phone does in `openmx-ios`.
//!
//! * The disc, `settings.txt` and `mods/` live in the app's Documents folder, which Files shows as
//!   On My iPhone → openMX (`UIFileSharingEnabled`, `LSSupportsOpeningDocumentsInPlace`). The shell
//!   looks for its disc through `kit.toml`, so the app works in Documents and writes one there naming
//!   the `.iso` it finds; until there is one, it says where to put it and waits.
//! * Everything the shell prints goes to `Documents/openmx-log.txt` as well (M105): a phone with no
//!   Mac attached has nowhere else to show it.
//! * Pads are read as the browser's are (`web_host.rs`): a [`PadNode`] puts a standard-mapping pad
//!   out as a js device's events, so the pad layer, its bindings and its seats are the desktop's.

use crate::touch_pad::TouchPad;
use std::collections::VecDeque;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

/// The on-screen pad: the window feeds it touches, the pad layer reads it as one more controller.
pub static TOUCH: LazyLock<Mutex<TouchPad>> = LazyLock::new(Default::default);

/// The touch pad's place among the pads: after any sixteen controllers.
const TOUCH_INDEX: u32 = 16;

/// The touch pad's name, which is also what the pad layer knows it by (P212).
pub const TOUCH_NAME: &str = "Touch pad";

/// The log, in Documents.
const LOG: &str = "openmx-log.txt";

/// The app: the log, Documents as the working directory, the disc, then the shell on its own thread
/// while this one, the main thread, runs UIKit. Never returns.
pub fn main() {
    let docs = openmx_ios::documents();
    let _ = std::fs::create_dir_all(&docs);
    openmx_ios::log_to(&docs.join(LOG));
    println!(
        "openmx {} ({} build {}) on iOS, {}",
        env!("CARGO_PKG_VERSION"),
        option_env!("OPENMX_COMMIT").unwrap_or("?"),
        option_env!("OPENMX_BUILD").unwrap_or("?"),
        openmx_ios::memory()
    );
    if let Err(e) = std::env::set_current_dir(&docs) {
        println!("openmx: no Documents folder ({e})");
    }
    crate::gpuwin::run_on_main(move || {
        let code = match pollster::block_on(Box::pin(game(&docs))) {
            Ok(()) => 0,
            Err(e) => {
                println!("openmx: {e}");
                1
            }
        };
        println!("openmx: the shell ended ({code}), {}", openmx_ios::memory());
        std::process::exit(code);
    });
}

/// The shell, once there is a disc.
async fn game(docs: &Path) -> Result<(), String> {
    if find_iso(docs).is_none() {
        wait_for_disc(docs)?;
    }
    write_kit_toml(docs)?;
    crate::run().await
}

/// The first disc image in Documents, by name.
fn find_iso(docs: &Path) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(docs)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("iso")))
        .collect();
    found.sort();
    found.into_iter().next()
}

/// `kit.toml` naming the disc in Documents, unless one already names a disc that is there.
fn write_kit_toml(docs: &Path) -> Result<(), String> {
    let path = docs.join("kit.toml");
    if let Ok(text) = std::fs::read_to_string(&path)
        && text.lines().any(|l| l.trim_start().starts_with("iso"))
        && game_render::disc::configured_image().is_ok_and(|p| p.is_file())
    {
        return Ok(());
    }
    let iso = find_iso(docs).ok_or("no .iso in Documents")?;
    let name = iso.file_name().and_then(|n| n.to_str()).ok_or("the disc's name is not text")?;
    let text = format!("# Written by the app (P200): the disc it found in this folder.\n[game]\niso = \"{name}\"\n");
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("disc: {name}");
    Ok(())
}

/// No disc yet: a page saying where it goes, until one is there. A note is written into Documents
/// too, so Files shows the app's folder before anything has been put in it.
fn wait_for_disc(docs: &Path) -> Result<(), String> {
    let note = docs.join("Put your disc image here.txt");
    if !note.exists() {
        let _ = std::fs::write(
            &note,
            "openMX needs your own copy of the game's disc: put its .iso in this folder (On My iPhone → openMX),\n\
             then open the app again or wait a moment. settings.txt and mods/ go here too.\n",
        );
    }
    println!("disc: none in {}, waiting", docs.display());
    let mut window = crate::gpuwin::GpuWindow::open(None, 1, "openMX", true)?;
    let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
    loop {
        window.poll();
        if last.elapsed().as_secs_f32() >= 1.0 {
            last = std::time::Instant::now();
            if find_iso(docs).is_some() {
                return Ok(());
            }
        }
        let (w, h) = (window.width.max(1), window.height.max(1));
        let frame = no_disc_page(w, h);
        window.present(&frame.color, w, h)?;
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// The page [`wait_for_disc`] shows, drawn by the CPU into a frame of `w` by `h`.
fn no_disc_page(w: usize, h: usize) -> game_render::raster::Frame {
    use crate::menu::ink as art;
    let mut frame = game_render::raster::Frame::new(w, h, [0, 0, 0]);
    let ink = art::ink();
    let size = (w.min(h * 2) as f32 / 26.0).max(12.0);
    let lines: [(art::Weight, &str, f32); 4] = [
        (art::Weight::Heavy, "No disc yet", 1.6),
        (art::Weight::Bold, "Put your own copy of the game's .iso in", 1.0),
        (art::Weight::Bold, "Files → On My iPhone → openMX", 1.0),
        (art::Weight::Bold, "and it starts by itself.", 1.0),
    ];
    let mut out = Vec::new();
    let total: f32 = lines.iter().map(|l| l.2 * size * 1.5).sum();
    let mut y = (h as f32 - total) / 2.0;
    for (weight, text, scale) in lines {
        let s = size * scale;
        y += s * 1.5;
        let tw = ink.measure(weight, &text.replace('→', ">"), s);
        ink.text(&mut out, 0, weight, &text.replace('→', ">"), (w as f32 - tw) / 2.0, y, s, 0.0, [255, 255, 255, 255]);
    }
    art::raster(&mut frame, &out, &[&ink.atlas]);
    frame
}

// ---- pads -----------------------------------------------------------------------------------------

/// The GameController pads seen so far, by their controller object: the place each was given, so a
/// pad keeps its number while it is connected.
static PLACES: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// A standard-mapping snapshot of one pad.
#[derive(Clone, Copy, PartialEq)]
struct Pad {
    buttons: [f32; 17],
    axes: [f32; 4],
}

/// What the pad layer last said of the touch pad's player (P212): whether it holds a seat (or is
/// 1P's device), whether that was in a ride, and whether a controller joined it in one.
struct TouchSeat {
    in_use: bool,
    riding: bool,
    kept: bool,
}

static SEAT: Mutex<TouchSeat> = Mutex::new(TouchSeat { in_use: false, riding: false, kept: false });

/// The pad layer, each frame: whether the touch pad's player has a seat, and whether this is a ride.
/// A controller connecting mid-ride is a second player, and the touch pad stays to keep riding; one
/// connecting in the menus is most likely 1P picking it up, and the touch pad goes.
pub fn touch_seat(in_use: bool, riding: bool) {
    let mut seat = SEAT.lock().unwrap_or_else(|e| e.into_inner());
    seat.in_use = in_use;
    seat.riding = riding;
}

/// Every pad now, by its place: the controllers, then the touch pad while it is shown — which is
/// whenever no controller is connected, or one joined the touch player mid-ride ([`touch_seat`]). The touch pad is read (its taps let go) only with `read`.
fn snapshot(read: bool) -> Vec<(u32, Pad)> {
    let pads = openmx_ios::pads();
    let mut places = PLACES.lock().unwrap_or_else(|e| e.into_inner());
    places.retain(|id| pads.iter().any(|p| p.id == *id));
    let mut out = Vec::new();
    for p in &pads {
        let at = match places.iter().position(|id| *id == p.id) {
            Some(at) => at,
            None if places.len() < TOUCH_INDEX as usize => {
                places.push(p.id);
                places.len() - 1
            }
            None => continue,
        };
        out.push((at as u32, Pad { buttons: p.buttons, axes: p.axes }));
    }
    let mut touch = TOUCH.lock().unwrap_or_else(|e| e.into_inner());
    let mut seat = SEAT.lock().unwrap_or_else(|e| e.into_inner());
    if pads.is_empty() || !seat.in_use {
        seat.kept = false;
    } else if touch.shown && seat.riding {
        seat.kept = true;
    }
    let shown = pads.is_empty() || seat.kept;
    if shown != touch.shown {
        touch.shown = shown;
        touch.reset();
    }
    if shown {
        let (buttons, axes) = if read { touch.read() } else { ([0.0; 17], [0.0; 4]) };
        out.push((TOUCH_INDEX, Pad { buttons, axes }));
    }
    out
}

/// The pads as the pad layer's device nodes: `gamepad/<place>`.
pub fn pads() -> Vec<(u32, PathBuf)> {
    snapshot(false).into_iter().map(|(i, _)| (i, format!("gamepad/{i}").into())).collect()
}

fn index_of(path: &Path) -> Option<u32> {
    path.to_str()?.strip_prefix("gamepad/")?.parse().ok()
}

/// A pad's name, for the pad layer: the controller's own, or the touch pad's.
pub fn pad_name(node: &str) -> String {
    let Ok(i) = node.parse::<u32>() else { return String::new() };
    if i == TOUCH_INDEX {
        return TOUCH_NAME.into();
    }
    let places = PLACES.lock().unwrap_or_else(|e| e.into_inner());
    let id = places.get(i as usize).copied();
    drop(places);
    openmx_ios::pads().into_iter().find(|p| Some(p.id) == id).map(|p| p.name).unwrap_or_default()
}

/// The js numbers of the standard mapping's buttons, as `web_host.rs` has them.
const JS_BUTTON: [(usize, u8); 11] = [(0, 0), (1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (8, 6), (9, 7), (16, 8), (10, 9), (11, 10)];

/// A pad read as a js device, as `web_host.rs`'s is: each [`Read`] one event, a pad no longer
/// listed the end of the file.
pub struct PadNode {
    index: u32,
    last: Option<Pad>,
    queue: VecDeque<[u8; 8]>,
    gone: bool,
}

impl PadNode {
    pub fn open(path: &Path) -> std::io::Result<PadNode> {
        let index = index_of(path).ok_or_else(|| std::io::Error::other("not a gamepad"))?;
        Ok(PadNode { index, last: None, queue: Default::default(), gone: false })
    }

    fn event(&mut self, kind: u8, number: u8, value: i16, init: bool) {
        let ms = (crate::window::shell_us() / 1000) as u32;
        let mut e = [0u8; 8];
        e[..4].copy_from_slice(&ms.to_le_bytes());
        e[4..6].copy_from_slice(&value.to_le_bytes());
        e[6] = kind | if init { 0x80 } else { 0 };
        e[7] = number;
        self.queue.push_back(e);
    }

    fn take(&mut self) {
        let Some((_, now)) = snapshot(true).into_iter().find(|(i, _)| *i == self.index) else {
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
