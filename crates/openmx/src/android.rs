//! The Android app (P308), as opentennis's is (M99, M101, M102, B418, B455) and as the iPhone's
//! (`ios.rs`) is here: the shell as it is on the desktop, its window winit's NativeActivity one
//! ([`crate::gpuwin`], `android_gpu.rs`) and its pads the controllers Android lists and the
//! on-screen [`TouchPad`], everything that takes JNI in `openmx-android`.
//!
//! * The disc, `settings.txt` and `mods/` live in the app's files folder on the shared storage
//!   (`/sdcard/Android/data/io.github.fizl0.openmx/files`), which a computer sees over USB. The shell
//!   looks for its disc through `kit.toml`, so the app works there and writes one naming the `.iso`
//!   it finds. With none there yet, a page says which disc the game needs and a tap opens the
//!   system's file picker (Downloads, an SD card, a cloud drive): the picked image is copied in, and
//!   the game goes on to the main menu.
//! * Everything the shell prints goes to `openmx-log.txt` there as well (M105), the launch before's
//!   kept as `openmx-log-previous.txt`, and to logcat.
//! * Pads are read as the iPhone's are: a [`PadNode`] puts a standard-mapping pad out as a js
//!   device's events, so the pad layer, its bindings and its seats are the desktop's.

use crate::touch_pad::TouchPad;
use openmx_android::pad::Held;
use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use winit::platform::android::activity::AndroidApp;

/// The on-screen pad: the window feeds it touches, the pad layer reads it as one more controller.
pub static TOUCH: LazyLock<Mutex<TouchPad>> = LazyLock::new(Default::default);

/// The touch pad's place among the pads: after any sixteen controllers.
const TOUCH_INDEX: u32 = 16;

/// The touch pad's name, which is also what the pad layer knows it by (P212).
pub const TOUCH_NAME: &str = "Touch pad";

/// The log, in the files folder.
const LOG: &str = "openmx-log.txt";

/// The name a picked disc image is copied in under.
const PICKED: &str = "disc.iso";

/// The NativeActivity's entry (android-activity calls it on a thread of its own). The shell's crate
/// forbids unsafe code but for this one symbol, which the activity looks up by name.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    main(app)
}

/// The app: the log, the files folder as the working directory, HTTPS, the disc, then the shell.
/// Ends the process when the shell ends.
fn main(app: AndroidApp) {
    // B492: a player's build reads none of the developers' variables (fixtures, update servers,
    // pad roots), only the texture reload `docs/` gives players.
    let dropped = match cfg!(feature = "ship") {
        true => openmx_android::drop_env("OPENMX_", &["OPENMX_TEXTURE_RELOAD"]),
        false => Vec::new(),
    };
    openmx_android::start(&app);
    let dir = openmx_android::files();
    let _ = std::fs::create_dir_all(&dir);
    let log = dir.join(LOG);
    if log.exists() {
        let _ = std::fs::rename(&log, dir.join("openmx-log-previous.txt"));
    }
    openmx_android::log_to(&log);
    crate::launched();
    println!(
        "openmx {} ({} build {}{}) on Android, {}",
        env!("CARGO_PKG_VERSION"),
        option_env!("OPENMX_COMMIT").unwrap_or("?"),
        option_env!("OPENMX_BUILD").unwrap_or("?"),
        if ps2_float::NATIVE { ", native sim" } else { "" },
        openmx_android::model().unwrap_or_else(|| "an unknown phone".into()),
    );
    if !dropped.is_empty() {
        println!("openmx: a player's build, ignoring {}", dropped.join(" "));
    }
    // B418: the store's, the updater's and the short codes' HTTPS checked by the phone.
    if !openmx_android::init_tls() {
        println!("android: no platform certificate check; HTTPS will fail");
    }
    winit::platform::android::lifecycle::set_hook(lifecycle);
    // The updater's download (`update.rs`), installed or not, by now.
    let _ = std::fs::remove_file(dir.join("update.apk"));
    if let Err(e) = std::env::set_current_dir(&dir) {
        println!("openmx: no files folder ({e})");
    }
    let code = match pollster::block_on(Box::pin(game(&dir))) {
        Ok(()) => 0,
        Err(e) => {
            println!("openmx: {e}");
            1
        }
    };
    println!("openmx: the shell ended ({code})");
    std::process::exit(code);
}

/// The activity's lifecycle (the patched winit's hook, M108). Paused or stopped, the app has gone
/// away (P311, [`openmx_android::away`]): the shell pauses the ride and saves the settings, the
/// sound stands still till it is resumed. Finished — swiped away in recents, closed — the process
/// ends rather than idling on with no activity; a destroy to be made again (a configuration change
/// the manifest does not take on itself) does not.
fn lifecycle(phase: winit::platform::android::lifecycle::Phase) {
    use openmx_android::away;
    use winit::platform::android::lifecycle::Phase;
    println!("android: activity {phase:?}");
    match phase {
        Phase::Pause => drop(away::pause()),
        Phase::Stop => drop(away::stop()),
        Phase::Resume => drop(away::resume()),
        Phase::Destroy if openmx_android::finishing() => std::process::exit(0),
        Phase::Start | Phase::Destroy => {}
    }
}

/// The shell, once there is a disc.
async fn game(dir: &Path) -> Result<(), String> {
    if find_iso(dir).is_none() {
        wait_for_disc(dir)?;
    }
    write_kit_toml(dir)?;
    crate::run().await
}

/// The first disc image in the folder, by name (one copied in whole, not a picker's half-copy).
fn find_iso(dir: &Path) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("iso")))
        .collect();
    found.sort();
    found.into_iter().next()
}

/// `kit.toml` naming the disc in the folder, unless one already names a disc that is there.
fn write_kit_toml(dir: &Path) -> Result<(), String> {
    let path = dir.join("kit.toml");
    if let Ok(text) = std::fs::read_to_string(&path)
        && text.lines().any(|l| l.trim_start().starts_with("iso"))
        && game_render::disc::configured_image().is_ok_and(|p| p.is_file())
    {
        return Ok(());
    }
    let iso = find_iso(dir).ok_or("no .iso in the files folder")?;
    let name = iso.file_name().and_then(|n| n.to_str()).ok_or("the disc's name is not text")?;
    let text = format!("# Written by the app (P308): the disc it found in this folder.\n[game]\niso = \"{name}\"\n");
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("disc: {name}");
    Ok(())
}

/// Where the disc page is.
enum Stage {
    /// Waiting for a tap; the last try's error, if any.
    Idle(Option<String>),
    /// The system's picker is up.
    Picking,
    /// Copying the disc in: bytes so far, and the result once the thread is through.
    Copying(std::sync::Arc<std::sync::atomic::AtomicU64>, std::sync::mpsc::Receiver<Result<(), String>>),
}

/// No disc yet: a page saying which disc the game needs, a tap for the system's file picker, and the
/// picked image copied into `dir`. Also takes one put there by hand (`adb push`, USB).
fn wait_for_disc(dir: &Path) -> Result<(), String> {
    println!("disc: none in {}, asking", dir.display());
    let mut window = crate::gpuwin::GpuWindow::open(None, 1, "openMX", true)?;
    let mut stage = Stage::Idle(None);
    let mut last = std::time::Instant::now();
    let mut pushed = None;
    loop {
        window.poll();
        if window.closed() {
            return Err("the window closed".into());
        }
        let tapped = !window.clicks().is_empty();
        stage = match stage {
            Stage::Idle(_) if tapped => match openmx_android::pick() {
                true => Stage::Picking,
                false => Stage::Idle(Some("The file picker didn't open.".into())),
            },
            Stage::Picking => match openmx_android::taken() {
                openmx_android::Picked::Waiting => Stage::Picking,
                openmx_android::Picked::Cancelled => Stage::Idle(None),
                openmx_android::Picked::File(file) => {
                    let done = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
                    let (tell, told) = std::sync::mpsc::channel();
                    let (d, to) = (done.clone(), dir.to_path_buf());
                    std::thread::spawn(move || drop(tell.send(copy_in(file, &to, &d))));
                    Stage::Copying(done, told)
                }
            },
            Stage::Copying(done, told) => match told.try_recv() {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(e)) => Stage::Idle(Some(e)),
                Err(std::sync::mpsc::TryRecvError::Empty) => Stage::Copying(done, told),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => Stage::Idle(Some("The copy stopped.".into())),
            },
            idle => idle,
        };
        if matches!(stage, Stage::Idle(_)) && last.elapsed().as_secs_f32() >= 1.0 {
            last = std::time::Instant::now();
            // one pushed by hand is taken once its size holds for a look: not while adb still writes it
            let size = find_iso(dir).and_then(|p| std::fs::metadata(p).ok()).map(|m| m.len()).filter(|&n| n > 0);
            if size.is_some() && size == pushed {
                return Ok(());
            }
            pushed = size;
        }
        let (w, h) = (window.width.max(1), window.height.max(1));
        let frame = disc_page(w, h, &stage);
        window.present(&frame.color, w, h)?;
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// The ISO 9660 volume descriptor's mark, at byte 32769 of a disc image.
const CD001: &[u8] = b"CD001";

/// The picked file copied into `dir` as [`PICKED`], `done` counting the bytes: written under a name
/// [`find_iso`] does not take until it is whole. A file that is not a disc image is refused.
fn copy_in(mut file: std::fs::File, dir: &Path, done: &std::sync::atomic::AtomicU64) -> Result<(), String> {
    use std::io::Write;
    let part = dir.join(format!("{PICKED}.part"));
    let mut out = std::fs::File::create(&part).map_err(|e| format!("Couldn't write the copy: {e}"))?;
    let mut buf = vec![0u8; 1 << 20];
    let mut total = 0u64;
    let r = (|| {
        loop {
            let n = file.read(&mut buf).map_err(|e| format!("Couldn't read the file: {e}"))?;
            if n == 0 {
                break;
            }
            if total < 32774 && total + n as u64 >= 32774 {
                let at = (32769 - total) as usize;
                if &buf[at..at + 5] != CD001 {
                    return Err("That file isn't a disc image. Choose your MX Unleashed (USA) .iso.".to_string());
                }
            }
            out.write_all(&buf[..n]).map_err(|e| format!("Couldn't write the copy: {e}"))?;
            total += n as u64;
            done.store(total, std::sync::atomic::Ordering::Relaxed);
        }
        if total < 32774 {
            return Err("That file isn't a disc image. Choose your MX Unleashed (USA) .iso.".to_string());
        }
        out.sync_all().map_err(|e| format!("Couldn't write the copy: {e}"))?;
        std::fs::rename(&part, dir.join(PICKED)).map_err(|e| format!("Couldn't write the copy: {e}"))
    })();
    if r.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    println!("disc: copied {} MB in: {r:?}", total >> 20);
    r
}

/// The page [`wait_for_disc`] shows, drawn by the CPU into a frame of `w` by `h`.
fn disc_page(w: usize, h: usize, stage: &Stage) -> game_render::raster::Frame {
    use crate::menu::ink as art;
    let mut frame = game_render::raster::Frame::new(w, h, [0, 0, 0]);
    let ink = art::ink();
    let size = (w.min(h * 2) as f32 / 26.0).max(12.0);
    let status = match stage {
        Stage::Idle(None) => "Tap anywhere to choose it.".to_string(),
        Stage::Idle(Some(e)) => format!("{e} Tap to choose again."),
        Stage::Picking => "Waiting for the file picker...".to_string(),
        Stage::Copying(done, _) => {
            format!("Copying the disc in: {} MB", done.load(std::sync::atomic::Ordering::Relaxed) >> 20)
        }
    };
    let lines: [(art::Weight, &str, f32); 5] = [
        (art::Weight::Heavy, "No disc yet", 1.6),
        (art::Weight::Bold, "openMX plays from your own copy of the game:", 1.0),
        (art::Weight::Bold, "MX Unleashed (USA), SLUS-20767, as a disc image (.iso).", 1.0),
        (art::Weight::Bold, "It is copied into the app; then you can delete it.", 1.0),
        (art::Weight::Bold, &status, 1.0),
    ];
    // the widest line within nine tenths of the width (measure grows with the size)
    let widest = lines.iter().map(|l| ink.measure(l.0, l.1, size * l.2) / (w as f32 * 0.9)).fold(1.0, f32::max);
    let size = size / widest;
    let mut out = Vec::new();
    let total: f32 = lines.iter().map(|l| l.2 * size * 1.5).sum();
    let mut y = (h as f32 - total) / 2.0;
    for (weight, text, scale) in lines {
        let s = size * scale;
        y += s * 1.5;
        let tw = ink.measure(weight, text, s);
        ink.text(&mut out, 0, weight, text, (w as f32 - tw) / 2.0, y, s, 0.0, [255, 255, 255, 255]);
    }
    art::raster(&mut frame, &out, &[&ink.atlas]);
    frame
}

// ---- pads -----------------------------------------------------------------------------------------

/// The controllers' places ([`crate::pad::Places`]): a pad keeps its number while it is
/// connected, and takes it again when Android lists it anew.
static PLACES: LazyLock<Mutex<crate::pad::Places>> = LazyLock::new(Default::default);

/// The controllers as their events left them, by Android's device id, and the list Android last
/// gave (the ids, names), read at most once a second.
struct Controllers {
    held: HashMap<i32, Held>,
    listed: Vec<openmx_android::Device>,
    looked: Option<std::time::Instant>,
}

static CONTROLLERS: LazyLock<Mutex<Controllers>> =
    LazyLock::new(|| Mutex::new(Controllers { held: HashMap::new(), listed: Vec::new(), looked: None }));

/// The controllers now: the queued events taken in, the device list asked again after a second.
/// A pad whose event comes before the list has it is listed by its id.
fn controllers() -> Vec<(i32, String, Held)> {
    use winit::platform::android::gamepad::PadEvent;
    let mut c = CONTROLLERS.lock().unwrap_or_else(|e| e.into_inner());
    if c.looked.is_none_or(|t| t.elapsed().as_secs_f32() >= 1.0) {
        c.looked = Some(std::time::Instant::now());
        if let Some(list) = openmx_android::pads()
            && list != c.listed
        {
            println!("pads: Android lists {:?}", list.iter().map(|d| &d.name).collect::<Vec<_>>());
            c.listed = list;
        }
    }
    for e in winit::platform::android::gamepad::take() {
        match e {
            PadEvent::Key { device, keycode, pressed } => c.held.entry(device).or_default().key(keycode, pressed),
            PadEvent::Motion { device, axes } => c.held.entry(device).or_default().motion(axes),
        }
    }
    let mut out: Vec<(i32, String, Held)> =
        c.listed.iter().map(|d| (d.id, d.name.clone(), c.held.get(&d.id).copied().unwrap_or_default())).collect();
    for (id, held) in &c.held {
        if !c.listed.iter().any(|d| d.id == *id) && *id >= 0 {
            out.push((*id, format!("Controller {id}"), *held));
        }
    }
    out
}

/// A standard-mapping snapshot of one pad.
#[derive(Clone, Copy, PartialEq)]
struct Pad {
    buttons: [f32; 17],
    axes: [f32; 4],
}

/// Every pad now, by its place: the controllers, then the touch pad while it is shown — whenever no
/// controller is connected ([`crate::pad::touch_listed`]). The touch pad is read (its taps let go)
/// only with `read`.
fn snapshot(read: bool) -> Vec<(u32, Pad)> {
    let pads = controllers();
    let listed: Vec<(usize, &str)> = pads.iter().map(|p| (p.0 as usize, p.1.as_str())).collect();
    let at = PLACES.lock().unwrap_or_else(|e| e.into_inner()).place(&listed, TOUCH_INDEX as usize);
    let mut out = Vec::new();
    for (p, at) in pads.iter().zip(at) {
        let Some(at) = at else { continue };
        let (buttons, axes) = p.2.snapshot();
        out.push((at as u32, Pad { buttons, axes }));
    }
    let mut touch = TOUCH.lock().unwrap_or_else(|e| e.into_inner());
    let shown = crate::pad::touch_listed(pads.len());
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
    let Some(id) = PLACES.lock().unwrap_or_else(|e| e.into_inner()).id(i as usize) else { return String::new() };
    let c = CONTROLLERS.lock().unwrap_or_else(|e| e.into_inner());
    c.listed.iter().find(|d| d.id as usize == id).map_or_else(|| format!("Controller {id}"), |d| d.name.clone())
}

/// Rumble the pad at place `node` (P183, P311, `pad::Motor`), as the iPhone's [`crate::ios`] does:
/// a controller through its own vibrator, the touch pad through the phone's — the touch pad's
/// player holds the phone. Never waits ([`openmx_android::vibrate`] asks on a thread of its own).
pub fn rumble(node: u32, r: crate::pad::Rumble) {
    let to = if node == TOUCH_INDEX {
        openmx_android::Vibrator::Phone
    } else {
        let Some(id) = PLACES.lock().unwrap_or_else(|e| e.into_inner()).id(node as usize) else { return };
        openmx_android::Vibrator::Controller(id as i32)
    };
    openmx_android::vibrate(to, r.level, u32::from(r.millis()));
}

/// The js numbers of the standard mapping's buttons, as `web_host.rs` has them.
const JS_BUTTON: [(usize, u8); 11] = [(0, 0), (1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (8, 6), (9, 7), (16, 8), (10, 9), (11, 10)];

/// A pad read as a js device, as `ios.rs`'s is: each [`Read`] one event, a pad no longer listed the
/// end of the file.
pub struct PadNode {
    index: u32,
    last: Option<Pad>,
    queue: VecDeque<[u8; 8]>,
    gone: bool,
    /// The snapshots' times (P258).
    arrivals: crate::window::Arrivals,
}

impl PadNode {
    pub fn open(path: &Path) -> std::io::Result<PadNode> {
        let index = index_of(path).ok_or_else(|| std::io::Error::other("not a gamepad"))?;
        Ok(PadNode { index, last: None, queue: Default::default(), gone: false, arrivals: Default::default() })
    }

    fn event(&mut self, kind: u8, number: u8, value: i16, init: bool) {
        let ms = (self.arrivals.at(!init && crate::window::at_rest(kind, number, value)) / 1000) as u32;
        let mut e = [0u8; 8];
        e[..4].copy_from_slice(&ms.to_le_bytes());
        e[4..6].copy_from_slice(&value.to_le_bytes());
        e[6] = kind | if init { 0x80 } else { 0 };
        e[7] = number;
        self.queue.push_back(e);
    }

    fn take(&mut self) {
        self.arrivals.poll();
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
