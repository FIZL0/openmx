//! **openmx in a browser** — the part of the shell that is the same in a web page as on the desktop,
//! and the few host calls a page answers in place of a file system and a window (N4).
//!
//! ```sh
//! tools/web.sh            # web/pkg/openmx_web.js + openmx_web_bg.wasm, then serve web/ on localhost
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
//! Through `wasm-bindgen`, as opentennis's build is (`tools/web.sh` runs its CLI): ggrs takes its
//! clock and its random numbers from the page through it, and so does iroh. The page's own four are
//! on the worker's global `openmx` object:
//!
//! * `disc_size() -> number` and `disc_read(at, into: Uint8Array) -> number` — the image, by offset,
//!   out of the OPFS file (`at` a JavaScript number, exact up to 2^53, as the disc is larger than an
//!   `i32` can address);
//! * `present(rgba: Uint8Array, width, height)` — one frame, which the worker puts into its canvas;
//! * `log(line)` — a line for the console, and where a panic's message goes.
//!
//! and it calls back the exports below: `openmx_start` and `openmx_frame` for the picture, and
//! [`Online`](host::Online) for a ride against other machines.
//!
//! # Online
//!
//! [`peer::Peer`]: N2's rollback session over N3's iroh transport, which in a browser is relay-only —
//! the page has no UDP socket, so every packet goes through the relay over a WebSocket, as
//! opentennis's does. The desktop rides the same [`peer`] code, so a page and a desktop are just two
//! peers of one lobby, joined by its ticket or room code; `crates/openmx/tests/netplay/browser.rs` has
//! them agree on every tick's hash.
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
// The desktop shell's own online session (N2), compiled into this build too (N4).
#[allow(dead_code)]
#[path = "net/mod.rs"]
pub mod net;
pub mod peer;
// The machines (N17): a seat's switch online is the session's, so the session needs them.
#[allow(dead_code, unused_imports)]
mod vehicle;

use game_render::disc::{Disc, Res};
use game_render::mat::v3;
use game_render::raster::Frame;
use game_render::world::{Assets, Camera, Lighting, draw_scene};
use game_render::{ini, scene as view};
use game_sim::track;

use ground::Ground;

/// The track the desktop shell starts on when nothing else is said.
pub const TRACK: &str = "free06";

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
    let mut look = view::Scene::parse(&ini::text(&scn_bytes));
    let terrain_bytes = terrain_bytes(&archives[0], &place.terrain_file)?;
    let ground = Ground::new(track::Terrain::parse(&terrain_bytes)?, &place.terrain);

    let mut assets = Assets::default();
    let mut names: Vec<String> = look.models.iter().map(|p| p.model.clone()).collect();
    names.sort();
    names.dedup();
    for name in &names {
        assets.load_placed(&archives, name);
    }
    let eco_terrain = track::Terrain::parse(&terrain_bytes)?;
    look.ecosystem = game_render::eco::Grove::load(&archives, &look, &place.terrain, &eco_terrain, &mut assets)?;

    let spawn = v3(place.start[0].0, place.start[1].0, place.start[2].0);
    let facing = v3(place.start_direction[0].0, place.start_direction[1].0, place.start_direction[2].0);
    let camera = Camera::behind(spawn, facing);
    let mut frame = Frame::new(width, height, look.fog_color);
    // The desktop shell's own radius, so the two pictures are the same picture.
    ground.draw(&mut frame, &camera, &Lighting::of(&look), spawn, ground.radius());
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
    use std::sync::Arc;

    use js_sys::{Object, Reflect};
    use openmx_net::{Board, Cmd, Names, Net, Online as Settings, Pick, Relay, Room, HOST};
    use wasm_bindgen::prelude::*;

    use crate::peer::{self, Peer};

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = openmx)]
        fn disc_size() -> f64;
        #[wasm_bindgen(js_namespace = openmx)]
        fn disc_read(at: f64, into: &mut [u8]) -> i32;
        #[wasm_bindgen(js_namespace = openmx)]
        fn present(rgba: &[u8], width: u32, height: u32);
        #[wasm_bindgen(js_namespace = openmx, js_name = log)]
        fn host_log(line: &str);
    }

    /// One line to the page's console.
    pub fn say(text: &str) {
        host_log(text)
    }

    /// The disc the page copied into OPFS, read through the worker's synchronous access handle.
    struct HostImage {
        at: u64,
        size: u64,
    }

    impl Read for HostImage {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = disc_read(self.at as f64, buf);
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
    }

    thread_local! {
        static STATE: RefCell<State> = RefCell::new(State::default());
    }

    fn hook() {
        std::panic::set_hook(Box::new(|info| say(&format!("openmx: {info}"))));
    }

    /// Open the disc, draw the start line into a `width` x `height` frame and keep it. Empty when it
    /// worked; otherwise why not.
    #[wasm_bindgen]
    pub fn openmx_start(width: u32, height: u32) -> String {
        hook();
        let size = disc_size() as u64;
        let drawn = Disc::read_from(Box::new(HostImage { at: 0, size }))
            .and_then(|mut disc| start_line(&mut disc, TRACK, width.max(1) as usize, height.max(1) as usize));
        STATE.with_borrow_mut(|s| match drawn {
            Ok(frame) => {
                s.frame = Some(frame);
                String::new()
            }
            Err(e) => {
                let why = format!("disc: {e}");
                say(&why);
                why
            }
        })
    }

    /// Hand the page the current frame.
    #[wasm_bindgen]
    pub fn openmx_frame() {
        STATE.with_borrow_mut(|s| {
            let State { frame, rgba: out } = s;
            if let Some(frame) = frame {
                rgba(frame, out);
                present(out, frame.width as u32, frame.height as u32);
            }
        })
    }

    /// A ride against other machines (N4): this page's endpoint in a lobby, and once the host
    /// starts, the session. The page's seat rides [`peer::mash`] for now — the browser has no ride
    /// of its own on screen to steer yet.
    #[wasm_bindgen]
    pub struct Online {
        peer: Peer,
    }

    #[wasm_bindgen]
    impl Online {
        /// `relay`: the relay's URL, or empty for n0's public ones. `code_servers`: the pkarr
        /// relays asked for room codes, comma-separated (`None`: relay.pkarr.org; empty: none, so
        /// only tickets join).
        #[wasm_bindgen(constructor)]
        pub fn new(name: String, relay: String, code_servers: Option<String>) -> Online {
            hook();
            let relay = if relay.is_empty() { Relay::N0 } else { Relay::Own(relay) };
            let code_servers = code_servers.map(|c| c.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect());
            let cfg = Settings { name, relay, board: Board::Off, lan: false, code_servers, relay_only: true, ..Settings::default() };
            let mut peer = Peer::new(Net::start(cfg, Arc::new(Names)));
            // a browser has no file system for a desync's report: it goes to the console instead
            peer.dumps = std::path::PathBuf::new();
            Online { peer }
        }

        /// Host a lobby of `seats` with this page in the first.
        pub fn host(&self, seats: u32, bike: String) {
            let mut r = Room::new("", TRACK, seats.clamp(2, openmx_net::SEATS as u32) as usize);
            r.take(HOST, &[Pick::new("thq1", &bike)]);
            self.peer.net.send(Cmd::Host(r));
        }

        /// Join by room code or ticket, one seat.
        pub fn join(&self, code: String, bike: String) {
            self.peer.net.send(Cmd::Join(code, vec![Pick::new("thq1", &bike)]));
        }

        pub fn ready(&self, on: bool) {
            self.peer.net.send(Cmd::Prepared(on));
        }

        /// The host's Start.
        pub fn start(&self) {
            self.peer.net.send(Cmd::Start);
        }

        pub fn leave(&self) {
            self.peer.net.send(Cmd::Leave);
        }

        /// One frame of `dt` seconds.
        pub fn frame(&mut self, dt: f32) {
            self.peer.frame(&peer::mash, dt);
            for line in self.peer.said.drain(..) {
                say(&line);
            }
        }

        /// Where things are: `status`, `ticket`, `code`, `taken`, `ready` (every guest), `riding`,
        /// `me`, `tick`, `confirmed`, `desyncs`, `failed`, `too_big`.
        pub fn state(&self) -> Object {
            let o = Object::new();
            let set = |k: &str, v: JsValue| {
                let _ = Reflect::set(&o, &k.into(), &v);
            };
            let p = &self.peer;
            set("status", p.status.as_str().into());
            set("ticket", p.ticket.as_str().into());
            set("code", p.room.as_ref().map_or("", |r| r.short.as_str()).into());
            set("taken", (p.room.as_ref().map_or(0, Room::taken) as u32).into());
            set("ready", p.room.as_ref().is_some_and(Room::all_ready).into());
            set("riding", p.riding.is_some().into());
            if let Some(r) = &p.riding {
                set("me", r.np.me.as_str().into());
                set("tick", (r.np.tick() as u32).into());
                set("confirmed", (r.np.confirmed() as f64).into());
                set("desyncs", (r.np.desyncs.len() as u32).into());
            }
            set("failed", p.failed.as_deref().map_or(JsValue::NULL, JsValue::from));
            set("too_big", p.too_big.into());
            o
        }

        /// Ticks `from..to` as ridden here, one line each: the state's hash before the tick and the
        /// FNV-1a of every seat's pad bytes it ran on, both hex — for holding up against another
        /// machine's ([`peer::ticks`]).
        pub fn ticks(&self, from: u32, to: u32) -> String {
            self.peer.riding.as_ref().map_or(String::new(), |r| peer::ticks(&r.np, from as usize, to as usize))
        }
    }
}
