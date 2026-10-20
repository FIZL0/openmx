//! **openmx in a browser** — the part of the shell that is the same in a web page as on the desktop,
//! and the few host calls a page answers in place of a file system and a window (N4).
//!
//! ```sh
//! tools/web.sh            # web/pkg/openmx_web.wasm, then serve web/ on localhost
//! ```
//!
//! The page (`web/index.html`) asks for the user's own disc once and copies it into the browser's
//! origin-private storage (OPFS); `web/worker.js` opens that copy with a synchronous access handle and
//! runs this module beside it. So the game's data is read the same way the desktop reads it — a
//! sector at a time, out of the user's own image, at runtime — and nothing is fetched from anywhere
//! but the user's own storage.
//!
//! # The host calls
//!
//! No `wasm-bindgen`: the workspace has no dependency outside itself, and what crosses the boundary is
//! four numbers-only functions the worker supplies under the import module `openmx`:
//!
//! * `disc_size() -> f64` and `disc_read(at: f64, ptr, len) -> i32` — the image, by offset, out of
//!   the OPFS file (an `f64` because the disc is larger than an `i32` can address, and JavaScript's
//!   numbers are exact up to 2^53);
//! * `present(ptr, width, height)` — one RGBA frame, which the worker puts into its canvas;
//! * `log(ptr, len)` — a UTF-8 line for the console, and where a panic's message goes.
//!
//! and it calls back the exports below (`openmx_start`, `openmx_frame`, `openmx_message`, …).
//!
//! # What it draws, for now
//!
//! The place the ride starts in: the track's terrain and every prop its scene stands there, from
//! behind the start line, lit and fogged by the scene's own values — the desktop shell's own ground
//! pass ([`ground`]) and the renderer's own scene pass, over the same disc. **It is not the ride.**
//! The desktop's loop is one function (`run` in `main.rs`) that owns its window, its clock and its
//! pad, and is being rebuilt by the front end (P36) and rollback (N2) at the same time as this; the
//! browser rides once that loop can be stepped a frame at a time from a host it does not own. The
//! journal entry `n4-in-a-browser` says what is left.

// The desktop shell's own ground pass, compiled into this build too; the parts of it only the ride
// asks for go unused here.
#[allow(dead_code)]
mod ground;

use game_render::disc::{Disc, Res};
use game_render::mat::v3;
use game_render::raster::Frame;
use game_render::world::{Assets, Camera, Lighting, draw_scene};
use game_render::{ini, scene as view};
use game_sim::track;

use ground::Ground;

/// The track the desktop shell starts on when nothing else is said.
pub const TRACK: &str = "free06";

/// How many samples round the camera's target the ground is drawn to: the desktop shell's own
/// radius, so the two pictures are the same picture.
const GROUND_RADIUS: u32 = 44;

/// The place the ride starts in, drawn into a `width` x `height` frame: terrain, then props.
///
/// The same function on both sides of the boundary — the desktop's test draws it natively and in
/// the browser's build, and compares the two.
pub fn start_line(disc: &mut Disc, track_name: &str, width: usize, height: usize) -> Result<Frame, String> {
    let member = member_ending(disc, &format!("{track_name}.res"))?;
    let archives: Vec<Res> = vec![disc.res(&member)?];
    let scn = archives[0]
        .entries()
        .iter()
        .find(|e| e.name.to_ascii_lowercase().ends_with(".scn"))
        .ok_or("the track archive has no .scn")?
        .name
        .clone();
    let scn_bytes = archives[0].read(&scn)?;
    let place = track::Scene::parse(&scn_bytes)?;
    let look = view::Scene::parse(&ini::text(&scn_bytes));
    let terrain_bytes = terrain_bytes(&archives[0], &place.terrain_file)?;
    let ground = Ground::new(track::Terrain::parse(&terrain_bytes)?, &place.terrain);

    let mut assets = Assets::default();
    let mut names: Vec<String> = look.models.iter().map(|p| p.model.clone()).collect();
    names.sort();
    names.dedup();
    for name in &names {
        assets.load(&archives, name);
    }

    let spawn = v3(place.start[0].0, place.start[1].0, place.start[2].0);
    let facing = v3(place.start_direction[0].0, place.start_direction[1].0, place.start_direction[2].0);
    let camera = Camera::behind(spawn, facing);
    let mut frame = Frame::new(width, height, look.fog_color);
    ground.draw(&mut frame, &camera, &Lighting::of(&look), spawn, GROUND_RADIUS);
    draw_scene(&mut frame, &look, &assets, &[], &camera);
    Ok(frame)
}

/// RGB to the RGBA a canvas's `ImageData` holds, opaque.
pub fn rgba(frame: &Frame, out: &mut Vec<u8>) {
    out.clear();
    out.reserve(frame.width * frame.height * 4);
    for p in frame.color.chunks_exact(3) {
        out.extend_from_slice(&[p[0], p[1], p[2], 0xff]);
    }
}

/// The member of `MX.PSR` whose name ends in `suffix`, as the desktop shell finds a track's archive.
fn member_ending(disc: &Disc, suffix: &str) -> Result<String, String> {
    disc.entries()
        .iter()
        .find(|e| e.name.to_ascii_lowercase().ends_with(suffix))
        .map(|e| e.name.clone())
        .ok_or_else(|| format!("no {suffix} on the disc"))
}

/// The terrain file a scene asks for, with the platform tag the archive spells it with when the
/// scene does not (the desktop shell's own rule, `terrain_bytes` in `main.rs`).
fn terrain_bytes(res: &Res, named: &str) -> Result<Vec<u8>, String> {
    if let Ok(bytes) = res.read_suffix(named) {
        return Ok(bytes);
    }
    let tagged = match named.rsplit_once('.') {
        Some((stem, ext)) => format!("{stem}.ps2.{ext}"),
        None => return Err(format!("the archive has no member ending in {named}")),
    };
    res.read_suffix(&tagged)
        .map_err(|_| format!("the archive has no member ending in {named} or {tagged}"))
}

/// The browser's side: the host calls, the disc behind them, and the exports the worker drives.
#[cfg(target_arch = "wasm32")]
mod host {
    use super::*;
    use std::cell::RefCell;
    use std::io::{Read, Seek, SeekFrom};

    #[link(wasm_import_module = "openmx")]
    unsafe extern "C" {
        fn disc_size() -> f64;
        fn disc_read(at: f64, ptr: *mut u8, len: u32) -> i32;
        fn present(ptr: *const u8, width: u32, height: u32);
        fn log(ptr: *const u8, len: u32);
    }

    /// One line to the page's console.
    pub fn say(text: &str) {
        // SAFETY: the host reads `len` bytes at `ptr` and keeps nothing.
        unsafe { log(text.as_ptr(), text.len() as u32) }
    }

    /// The disc the page copied into OPFS, read through the worker's synchronous access handle.
    struct HostImage {
        at: u64,
        size: u64,
    }

    impl Read for HostImage {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            // SAFETY: the host writes at most `len` bytes at `ptr`, which is `buf`.
            let n = unsafe { disc_read(self.at as f64, buf.as_mut_ptr(), buf.len() as u32) };
            if n < 0 {
                return Err(std::io::Error::other(format!("the browser's copy of the disc refused a read at {}", self.at)));
            }
            self.at += n as u64;
            Ok(n as usize)
        }
    }

    impl Seek for HostImage {
        fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
            let at = match to {
                SeekFrom::Start(n) => n as i128,
                SeekFrom::End(n) => self.size as i128 + n as i128,
                SeekFrom::Current(n) => self.at as i128 + n as i128,
            };
            if at < 0 {
                return Err(std::io::Error::other("seek before the start of the disc"));
            }
            self.at = at as u64;
            Ok(self.at)
        }
    }

    #[derive(Default)]
    struct State {
        frame: Option<Frame>,
        rgba: Vec<u8>,
        message: String,
    }

    thread_local! {
        static STATE: RefCell<State> = RefCell::new(State::default());
    }

    /// Open the disc, draw the start line into a `width` x `height` frame and present it. Zero when
    /// it worked; otherwise the length of [`openmx_message`]'s text, which says why.
    #[unsafe(no_mangle)]
    pub extern "C" fn openmx_start(width: u32, height: u32) -> u32 {
        std::panic::set_hook(Box::new(|info| say(&format!("openmx: {info}"))));
        // SAFETY: a plain call with no arguments.
        let size = unsafe { disc_size() } as u64;
        let drawn = Disc::read_from(Box::new(HostImage { at: 0, size }))
            .and_then(|mut disc| start_line(&mut disc, TRACK, width.max(1) as usize, height.max(1) as usize));
        STATE.with_borrow_mut(|s| match drawn {
            Ok(frame) => {
                s.frame = Some(frame);
                s.message.clear();
                0
            }
            Err(e) => {
                s.message = format!("disc: {e}");
                say(&s.message);
                s.message.len() as u32
            }
        })
    }

    /// Hand the page the current frame.
    #[unsafe(no_mangle)]
    pub extern "C" fn openmx_frame() {
        STATE.with_borrow_mut(|s| {
            let State { frame, rgba: out, .. } = s;
            if let Some(frame) = frame {
                rgba(frame, out);
                // SAFETY: `out` holds width * height * 4 bytes and outlives the call.
                unsafe { present(out.as_ptr(), frame.width as u32, frame.height as u32) }
            }
        })
    }

    /// Where the last error's text is (UTF-8, [`openmx_start`]'s return is its length).
    #[unsafe(no_mangle)]
    pub extern "C" fn openmx_message() -> *const u8 {
        STATE.with_borrow(|s| s.message.as_ptr())
    }
}
