//! The track pick's pictures for the mod tracks (P70).
//!
//! A disc track's card shows its stills out of `UI\trackpreview.res` ([`crate::menu::Art`]). A mod
//! brings its own picture as openmx-mods' standard allows (T1 §2: `previews/track.png` in the mod's
//! folder, optional), and where it has none the shell takes one: a still of the mod's map with no
//! HUD, shot by the shell's own binary run headless in the background
//! ([`still_job`]) and kept in the mods folder's `previews/<id>.v<N>.png`, beside `tracks/`, which the
//! scan never reads.
//!
//! P282: a picture is taken once and kept. A still is never taken again because `mod.json` is newer or
//! its framing is an older [`STILL_VERSION`]'s ([`kept`]); a world is never shot on the player's
//! machine (its still is a second whole world loaded beside the ride: on the Deck, world_ultra's took
//! both down), so a world with no picture of its own is a card with none; no still is shot while a
//! ride loads or runs ([`Ride`]), and one being shot when a ride starts is stopped and shot again
//! after it; and one still at a time is shot ([`shoot`]). A mod is meant to arrive with its own
//! [`OWN`]: the store's upload bakes one into the mod's folder first ([`bake`]).
//!
//! Since P166 the still is of the map, not of the rider: the shell's `--overview` draws no bike, rider
//! or plane, and looks from [`overview`]'s framing, raised and pulled back from the spawn over the
//! ground's middle, after the world has had [`STILL_FRAME`] frames to load.
//!
//! The picture's size is the standard's to leave open and this loader's to decide: any 8-bit PNG
//! up to [`MAX_SIDE`] a side, shown cropped to the card's 16:9 about its middle, so a 16:9 picture
//! (the stills are [`STILL_SIZE`]) shows whole. Nothing in here can panic on a mod's bytes: a PNG
//! that does not read is one log line and a card with no picture.

use game_render::pst::Texture;
use game_sim::archive::inflate;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Mutex;
use std::time::Duration;

/// A mod's own picture, inside its folder.
pub const OWN: &str = "previews/track.png";
/// Where the shell keeps the stills it takes, inside the mods folder.
pub const STILLS_DIR: &str = "previews";
/// The size of a still the shell takes: 16:9, twice a card's picture on a 1080p frame.
pub const STILL_SIZE: (usize, usize) = (512, 288);
/// The largest side a mod's picture may have.
pub const MAX_SIDE: usize = 2048;
/// How the stills are taken, in their names: 1 was P70's first frame from the spawn (named
/// `<id>.png`), 2 is P166's [`overview`]. A still of any other version is taken again.
pub const STILL_VERSION: u32 = 2;
/// The frame a still is shot on, the ride's ground, props and sky up by then.
pub const STILL_FRAME: u32 = 30;
/// The overview's horizontal fov, in degrees, as `--shot-pose` reads it.
pub const OVERVIEW_FOV: f32 = 80.0;
/// How far under the line to the target the overview looks, in degrees.
const OVERVIEW_DIP: f32 = 8.0;

/// A mod's own picture, `mods/tracks/<id>/previews/track.png`.
pub fn own(mods: &Path, id: &str) -> PathBuf {
    mods.join("tracks").join(id).join(OWN)
}

/// The still the shell took of a mod, `mods/previews/<id>.v<STILL_VERSION>.png`.
pub fn still(mods: &Path, id: &str) -> PathBuf {
    still_of(mods, id, STILL_VERSION)
}

/// The still of `version`'s framing; version 1 has no stamp in its name.
pub fn still_of(mods: &Path, id: &str, version: u32) -> PathBuf {
    match version {
        1 => mods.join(STILLS_DIR).join(format!("{id}.png")),
        v => mods.join(STILLS_DIR).join(format!("{id}.v{v}.png")),
    }
}

/// Where a still looks from: the eye in world feet, yaw and pitch in degrees and the horizontal fov,
/// the shell's `--shot-pose` numbers (yaw 0 along +z, 90 along +x; pitch up positive).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Framing {
    pub eye: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
}

/// The framing a mod map's still is taken from (P166): over the map rather than at the rider. The
/// ground runs from 0 to `extent` on x and z; `height` is the ground under a point, `None` off it.
///
/// It looks from the spawn toward the ground's middle (along the spawn's `facing` when the spawn is
/// near the middle already), from an eye pulled back behind the spawn and raised over the higher of
/// its ground and the spawn's, both by a share of the map's diagonal, kept on the map; and it pitches
/// down a little past the ground at the middle, or at a third of the diagonal ahead, between 10 and
/// 50 degrees.
pub fn overview(spawn: [f32; 3], facing: [f32; 2], extent: (f32, f32), mut height: impl FnMut(f32, f32) -> Option<f32>) -> Framing {
    let (w, d) = (extent.0.max(1.0), extent.1.max(1.0));
    let span = w.hypot(d);
    let middle = [w / 2.0, d / 2.0];
    let to_middle = [middle[0] - spawn[0], middle[1] - spawn[2]];
    let far = to_middle[0].hypot(to_middle[1]);
    let unit = |v: [f32; 2]| {
        let l = v[0].hypot(v[1]);
        if l > 1.0e-3 && l.is_finite() { [v[0] / l, v[1] / l] } else { [0.0, 1.0] }
    };
    let (dir, reach) = if far > 0.1 * span { (unit(to_middle), far) } else { (unit(facing), span / 3.0) };
    let back = (0.1 * span).clamp(60.0, 350.0);
    let rise = (0.1 * span).clamp(50.0, 350.0);
    let margin = (0.02 * span).min(w / 4.0).min(d / 4.0);
    let x = (spawn[0] - dir[0] * back).clamp(margin, w - margin);
    let z = (spawn[2] - dir[1] * back).clamp(margin, d - margin);
    let under = height(x, z).filter(|h| h.is_finite()).unwrap_or(spawn[1]);
    let y = under.max(spawn[1]) + rise;
    let (tx, tz) = (spawn[0] + dir[0] * reach, spawn[2] + dir[1] * reach);
    let ty = height(tx, tz).filter(|h| h.is_finite()).unwrap_or(spawn[1]);
    let run = (tx - x).hypot(tz - z).max(1.0);
    // A little under the line to the target, so the ground fills more of the picture than the sky.
    let pitch = -((y - ty).atan2(run).to_degrees() + OVERVIEW_DIP).clamp(10.0, 50.0);
    Framing { eye: [x, y, z], yaw: dir[0].atan2(dir[1]).to_degrees(), pitch, fov: OVERVIEW_FOV }
}

/// The overview's numbers as the shell prints them, one `overview: ` line.
pub fn overview_line(f: &Framing) -> String {
    format!("overview: {}, {}, {}, {}, {}, {}", f.eye[0], f.eye[1], f.eye[2], f.yaw, f.pitch, f.fov)
}

/// The still the shell took of `id` before, of this framing or an older one (P282: kept whatever
/// `mod.json`'s age or the framing's version, never taken again), the newest framing's first.
pub fn kept(mods: &Path, id: &str) -> Option<PathBuf> {
    (1..=STILL_VERSION).rev().map(|v| still_of(mods, id, v)).find(|p| p.is_file())
}

/// Whether `mods/tracks/<id>` is a world: its `mod.json` says `"kind": "world"`.
fn is_world(mods: &Path, id: &str) -> bool {
    let Ok(text) = crate::files::read(mods.join("tracks").join(id).join("mod.json")) else { return false };
    let json = std::str::from_utf8(&text).ok().and_then(|t| crate::track_mods::Json::parse(t).ok());
    json.and_then(|j| j.get("kind").and_then(crate::track_mods::Json::str).map(|k| k == "world")).unwrap_or(false)
}

/// How many rides are loading or running now (P282), which no still is shot beside.
static RIDES: AtomicUsize = AtomicUsize::new(0);

/// A ride loading or running, from [`Ride::on`] until it is dropped: no still is shot meanwhile.
pub struct Ride(());

impl Ride {
    pub fn on() -> Ride {
        RIDES.fetch_add(1, Ordering::SeqCst);
        Ride(())
    }
}

impl Drop for Ride {
    fn drop(&mut self) {
        RIDES.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Whether a ride is loading or running.
pub fn riding() -> bool {
    RIDES.load(Ordering::SeqCst) > 0
}

/// The picture `id` brings, read through [`crate::files`] (P261: the browser's OPFS `mods/` too): its
/// [`OWN`]; in the browser, which cannot take a still, else the first other `previews/*.png` of its
/// folder in name order, as a store zip may carry one.
pub fn own_picture(mods: &Path, id: &str) -> Option<PathBuf> {
    let own = own(mods, id);
    if crate::files::is_file(&own) {
        return Some(own);
    }
    if !cfg!(target_arch = "wasm32") {
        return None;
    }
    let mut pngs: Vec<PathBuf> = crate::files::list(mods.join("tracks").join(id).join(STILLS_DIR))
        .into_iter()
        .flatten()
        .filter(|l| !l.dir && l.path.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")))
        .map(|l| l.path)
        .collect();
    pngs.sort();
    pngs.into_iter().next()
}

/// What the pick has for each of `ids` now, `(id, picture)`, and the ids that need a still taken.
/// A picture that does not read is a line in `log` and no picture. The browser takes no still: a mod
/// with no picture of its own is a card with none; nor is a world shot anywhere (P282).
pub fn gather(mods: &Path, ids: &[String], log: &mut Vec<String>) -> (Vec<(String, Texture)>, Vec<String>) {
    let (mut have, mut take) = (Vec::new(), Vec::new());
    for id in ids {
        let path = if let Some(own) = own_picture(mods, id) {
            own
        } else if cfg!(target_arch = "wasm32") {
            continue;
        } else if let Some(still) = kept(mods, id) {
            still
        } else if is_world(mods, id) {
            log.push(format!("mods: tracks/{id} has no picture, and a world is not shot here"));
            continue;
        } else {
            take.push(id.clone());
            continue;
        };
        match load(&path) {
            Ok(t) => have.push((id.clone(), t)),
            Err(e) => log.push(format!("mods: tracks/{id}'s picture: {}: {e}", path.display())),
        }
    }
    (have, take)
}

/// The shell run headless over `id` with no machine or rider drawn, from [`overview`]'s framing,
/// shot into `out` on [`STILL_FRAME`]: no HUD, markers, minimap or sound, one player, the world's
/// ground all built.
pub fn still_command(exe: &Path, mods: &Path, id: &str, out: &Path) -> Command {
    let mut c = Command::new(exe);
    let size = format!("{}x{}", STILL_SIZE.0, STILL_SIZE.1);
    let (frames, shot) = ((STILL_FRAME + 1).to_string(), STILL_FRAME.to_string());
    c.args(["--headless", "--overview", "--track", id, "--mods"]).arg(mods);
    c.args(["--players", "1", "--markers", "off", "--minimap", "off", "--no-sound", "--world-ground", "all"]);
    c.args(["--frames", &frames, "--shot-frame", &shot, "--size", &size, "--overlay", "none", "--shot"]).arg(out);
    c.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    c
}

/// Take a still of each of `ids` in turn, off the menu's thread; each that is taken comes back as
/// `(id, picture)`, and each that fails as `(id, why)`. None is shot while a ride loads or runs: the
/// job waits for it, and one a ride stops is shot again after it (P282).
pub fn still_job(exe: PathBuf, mods: PathBuf, ids: Vec<String>) -> Receiver<(String, Result<Texture, String>)> {
    let (send, hear) = mpsc::channel();
    std::thread::spawn(move || {
        for id in ids {
            let got = loop {
                while riding() {
                    std::thread::sleep(WATCH);
                }
                match take_still(&exe, &mods, &id) {
                    Err(Shot::Ride) => continue,
                    Err(Shot::Failed(why)) => break Err(why),
                    Ok(t) => break Ok(t),
                }
            };
            if send.send((id, got)).is_err() {
                return;
            }
        }
    });
    hear
}

/// How often a still being shot looks for a ride starting.
const WATCH: Duration = Duration::from_millis(100);

/// One still at a time, the menu's and an upload's alike (P282).
static SHOOTING: Mutex<()> = Mutex::new(());

/// Why a still was not shot.
#[derive(Debug, PartialEq)]
pub enum Shot {
    /// A ride was loading or started: the still was not shot, or was stopped.
    Ride,
    Failed(String),
}

/// `command` run as one still's shell, alone and never beside a ride: it does not start while a ride
/// loads or runs, and is killed if one starts.
pub fn shoot(mut command: Command) -> Result<(), Shot> {
    let _one = SHOOTING.lock().unwrap_or_else(|e| e.into_inner());
    if riding() {
        return Err(Shot::Ride);
    }
    let mut child = command.spawn().map_err(|e| Shot::Failed(format!("the shell: {e}")))?;
    loop {
        if riding() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Shot::Ride);
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => return Err(Shot::Failed(format!("the ride over the map did not start ({status})"))),
            Ok(None) => std::thread::sleep(WATCH),
            Err(e) => {
                let _ = child.kill();
                return Err(Shot::Failed(format!("the shell: {e}")));
            }
        }
    }
}

fn take_still(exe: &Path, mods: &Path, id: &str) -> Result<Texture, Shot> {
    let dir = mods.join(STILLS_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| Shot::Failed(format!("{}: {e}", dir.display())))?;
    let out = still(mods, id);
    // Shot under another name and moved over, so a half-written still is never read.
    let part = dir.join(format!("{id}.part.png"));
    let shot = shoot(still_command(exe, mods, id, &part));
    if shot.is_err() || !part.is_file() {
        let _ = std::fs::remove_file(&part);
        shot?;
        return Err(Shot::Failed("the ride over the map wrote no still".into()));
    }
    std::fs::rename(&part, &out).map_err(|e| Shot::Failed(format!("{}: {e}", out.display())))?;
    // The stills of the framings before this one are not read again.
    for v in 1..STILL_VERSION {
        let _ = std::fs::remove_file(still_of(mods, id, v));
    }
    load(&out).map_err(Shot::Failed)
}

/// P282: `id`'s own [`OWN`] put in its folder if it has none, so the mod carries its picture wherever
/// it goes (the store's upload, before it packs): the still the shell kept of it, else, where `exe` is
/// given, one shot now (a track's or a world's: an upload is its maker's own machine; it is not shot
/// beside a ride, and a ride starting stops it). `Ok(true)` when one was put there, `Ok(false)` when it
/// had one already.
pub fn bake(mods: &Path, id: &str, exe: Option<&Path>) -> Result<bool, String> {
    let own = own(mods, id);
    if own.is_file() {
        return Ok(false);
    }
    let dir = own.parent().ok_or("no previews folder")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let from = match kept(mods, id) {
        Some(still) => still,
        None => {
            let exe = exe.ok_or("no still of it to bake")?;
            let part = dir.join("track.part.png");
            let shot = shoot(still_command(exe, mods, id, &part));
            if shot.is_err() || !part.is_file() {
                let _ = std::fs::remove_file(&part);
                return Err(match shot {
                    Err(Shot::Ride) => "a ride started, so no still was shot".into(),
                    Err(Shot::Failed(why)) => why,
                    Ok(()) => "the ride over the map wrote no still".into(),
                });
            }
            part
        }
    };
    // Checked first: a picture the pick cannot read is not one to carry.
    load(&from)?;
    if from.parent() == Some(dir) {
        std::fs::rename(&from, &own)
    } else {
        std::fs::copy(&from, &own).map(|_| ())
    }
    .map_err(|e| format!("{}: {e}", own.display()))?;
    Ok(true)
}

/// A PNG file as a texture, read through [`crate::files`].
pub fn load(path: &Path) -> Result<Texture, String> {
    decode_png(&crate::files::read(path).map_err(|e| e.to_string())?)
}

/// A PNG as a texture: 8 bits a channel, grey, grey and alpha, RGB, RGBA or a palette (with its
/// `tRNS`, and grey's and RGB's), not interlaced. What a paint program or the shell's own `--shot` writes.
pub fn decode_png(d: &[u8]) -> Result<Texture, String> {
    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if d.get(..8) != Some(&SIGNATURE[..]) {
        return Err("not a PNG".into());
    }
    let be = |b: &[u8]| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize;
    let (mut width, mut height, mut colour, mut depth, mut interlace) = (0, 0, 0u8, 0u8, 0u8);
    let (mut palette, mut alpha, mut data) = (Vec::new(), Vec::new(), Vec::new());
    let mut at = 8;
    loop {
        let head = d.get(at..at + 8).ok_or("cut short")?;
        let (len, kind) = (be(head), &head[4..8]);
        let body = d.get(at + 8..at + 8 + len).ok_or("cut short")?;
        match kind {
            b"IHDR" if body.len() >= 13 => {
                (width, height) = (be(&body[0..4]), be(&body[4..8]));
                (depth, colour, interlace) = (body[8], body[9], body[12]);
            }
            b"PLTE" => palette = body.to_vec(),
            b"tRNS" => alpha = body.to_vec(),
            b"IDAT" => data.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
        at += 12 + len;
    }
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return Err(format!("{width}x{height}, and a picture is 1 to {MAX_SIDE} a side"));
    }
    if depth != 8 || interlace != 0 {
        return Err(format!("{depth}-bit{}, and the pick reads 8-bit, not interlaced", if interlace != 0 { " interlaced" } else { "" }));
    }
    let channels = match colour {
        0 | 3 => 1,
        4 => 2,
        2 => 3,
        6 => 4,
        c => return Err(format!("colour type {c}")),
    };
    let (raw, _) = inflate::zlib(&data)?;
    let stride = width * channels;
    if raw.len() < height * (stride + 1) {
        return Err("its pixels are cut short".into());
    }
    let mut rows = vec![0u8; height * stride];
    for y in 0..height {
        let filter = raw[y * (stride + 1)];
        let src = &raw[y * (stride + 1) + 1..(y + 1) * (stride + 1)];
        let (done, row) = rows.split_at_mut(y * stride);
        let up = if y > 0 { &done[(y - 1) * stride..] } else { &[][..] };
        let row = &mut row[..stride];
        for x in 0..stride {
            let a = if x >= channels { row[x - channels] as i32 } else { 0 };
            let b = up.get(x).map_or(0, |&v| v as i32);
            let c = if x >= channels { up.get(x - channels).map_or(0, |&v| v as i32) } else { 0 };
            let add = match filter {
                0 => 0,
                1 => a,
                2 => b,
                3 => (a + b) / 2,
                4 => {
                    let p = a + b - c;
                    let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
                    if pa <= pb && pa <= pc {
                        a
                    } else if pb <= pc {
                        b
                    } else {
                        c
                    }
                }
                f => return Err(format!("row filter {f}")),
            };
            row[x] = (src[x] as i32 + add) as u8;
        }
    }
    // Grey's and RGB's `tRNS` is the one colour that is see-through, 16 bits a channel.
    let key = |p: &[u8]| alpha.len() == 2 * p.len() && p.iter().enumerate().all(|(i, &v)| alpha[2 * i] == 0 && alpha[2 * i + 1] == v);
    let mut rgba = Vec::with_capacity(width * height * 4);
    for p in rows.chunks_exact(channels) {
        let px = match colour {
            0 => [p[0], p[0], p[0], if key(p) { 0 } else { 255 }],
            4 => [p[0], p[0], p[0], p[1]],
            2 => [p[0], p[1], p[2], if key(p) { 0 } else { 255 }],
            6 => [p[0], p[1], p[2], p[3]],
            _ => {
                let i = p[0] as usize;
                let c = palette.get(3 * i..3 * i + 3).ok_or("a colour past its palette")?;
                [c[0], c[1], c[2], alpha.get(i).copied().unwrap_or(255)]
            }
        };
        rgba.extend_from_slice(&px);
    }
    Ok(Texture { width, height, rgba })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P166: the overview looks from behind and above the spawn toward the map's middle, down onto
    /// it, from an eye on the map; from a spawn at the middle along the spawn's own facing; and a
    /// bigger map is looked at from further back and higher up.
    #[test]
    fn the_overview_looks_over_the_map_from_behind_the_spawn() {
        let flat = |_: f32, _: f32| Some(10.0);
        let f = overview([200.0, 12.0, 200.0], [1.0, 0.0], (1000.0, 1000.0), flat);
        assert!((f.yaw - 45.0).abs() < 1e-3, "toward the middle, not along the facing: {f:?}");
        assert!(f.eye[0] < 200.0 && f.eye[2] < 200.0 && f.eye[1] > 12.0 + 40.0, "{f:?}");
        assert!((-50.0..=-10.0).contains(&f.pitch) && f.fov == OVERVIEW_FOV, "{f:?}");
        // At the middle the spawn's facing says where to look.
        let f = overview([500.0, 12.0, 501.0], [-1.0, 0.0], (1000.0, 1000.0), flat);
        assert!((f.yaw + 90.0).abs() < 1e-3 && f.eye[0] > 500.0, "{f:?}");
        // A spawn at the edge, looking out: the eye stays on the map.
        let f = overview([2.0, 12.0, 2.0], [0.0, 0.0], (1000.0, 1000.0), flat);
        assert!(f.eye[0] > 0.0 && f.eye[2] > 0.0, "{f:?}");
        // Over a hill behind the spawn the eye rises over the hill, not through it.
        let hill = |x: f32, _: f32| Some(if x < 150.0 { 300.0 } else { 0.0 });
        let f = overview([200.0, 0.0, 500.0], [1.0, 0.0], (1000.0, 1000.0), hill);
        assert!(f.eye[0] < 150.0 && f.eye[1] > 300.0 + 40.0, "{f:?}");
        // Off the ground's edge or with no ground, the spawn's height stands in.
        let f = overview([200.0, 12.0, 500.0], [1.0, 0.0], (1000.0, 1000.0), |_, _| None);
        assert!(f.eye[1] > 12.0 && f.pitch.is_finite() && f.yaw.is_finite(), "{f:?}");
        let (small, big) = (overview([250.0, 0.0, 250.0], [0.0, 1.0], (500.0, 500.0), flat), overview([2000.0, 0.0, 2000.0], [0.0, 1.0], (4000.0, 4000.0), flat));
        assert!(big.eye[1] > small.eye[1] && 2000.0 - big.eye[2] > 250.0 - small.eye[2], "{small:?} {big:?}");
        // A map of no size, a spawn far off it and a ground of no height are still a pose.
        let f = overview([1e30, -1e30, 0.0], [0.0, 0.0], (0.0, -5.0), |_, _| Some(f32::INFINITY));
        assert!(f.pitch < 0.0 && f.pitch > -90.0 && f.yaw.is_finite() && f.eye.iter().all(|v| v.is_finite()), "{f:?}");
    }

    /// The still's name says its framing, and the first framing's was the bare id.
    #[test]
    fn a_still_is_named_for_its_framing() {
        let mods = Path::new("mods");
        assert_eq!(still_of(mods, "a", 1), mods.join("previews/a.png"));
        assert_eq!(still(mods, "a"), mods.join(format!("previews/a.v{STILL_VERSION}.png")));
        assert!(STILL_VERSION > 1);
    }

    /// A scratch mods folder with `tracks/<id>/mod.json` of `kind` for each of `mods`.
    fn scratch(name: &str, mods: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("openmx-previews-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (id, kind) in mods {
            let at = dir.join("tracks").join(id);
            std::fs::create_dir_all(&at).unwrap();
            std::fs::write(at.join("mod.json"), format!(r#"{{"standard": 1, "kind": "{kind}", "id": "{id}", "name": "{id}"}}"#)).unwrap();
        }
        dir
    }

    fn picture(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, game_render::png::encode_rgb(4, 4, &[50; 48])).unwrap();
    }

    /// P282: a picture is taken once and kept. A mod's own and a kept still (of this framing or an
    /// older one) are shown however much newer its `mod.json` is, and no still is taken of them; a
    /// world with no picture is shot by no one; only a track with none is.
    #[test]
    fn a_picture_is_kept_and_a_world_never_shot() {
        let mods = scratch("kept", &[("own", "track"), ("shot", "track"), ("old", "track"), ("bare", "track"), ("big", "world"), ("big_shot", "world")]);
        picture(&own(&mods, "own"));
        picture(&still(&mods, "shot"));
        picture(&still_of(&mods, "old", 1));
        picture(&still(&mods, "big_shot"));
        // every mod.json written after its picture
        std::thread::sleep(Duration::from_millis(20));
        for id in ["own", "shot", "old", "bare", "big", "big_shot"] {
            let json = mods.join("tracks").join(id).join("mod.json");
            std::fs::write(&json, std::fs::read(&json).unwrap()).unwrap();
        }
        let ids: Vec<String> = ["own", "shot", "old", "bare", "big", "big_shot"].map(String::from).to_vec();
        let mut log = Vec::new();
        let (have, take) = gather(&mods, &ids, &mut log);
        assert_eq!(have.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), ["own", "shot", "old", "big_shot"], "{log:?}");
        assert_eq!(take, ["bare"], "only the track with no picture: {log:?}");
        assert!(log.iter().any(|l| l.contains("tracks/big") && l.contains("world")), "{log:?}");
        assert_eq!(kept(&mods, "old"), Some(still_of(&mods, "old", 1)));
        let _ = std::fs::remove_dir_all(&mods);
    }

    /// P282: no still is shot beside a ride. Picked while a still waits, the ride starts no child, and
    /// the still is shot after it; a ride starting while one is shot stops that child at once.
    #[cfg(unix)]
    #[test]
    fn no_still_is_shot_beside_a_ride() {
        use std::os::unix::fs::PermissionsExt;
        let mods = scratch("ride", &[("t", "track")]);
        // a stand-in shell: each run leaves a mark, then takes as long as its first word says
        let (exe, marks) = (mods.join("shell.sh"), mods.join("marks"));
        std::fs::write(&exe, format!("#!/bin/sh\necho run >> '{}'\nsleep \"$(cat '{}' 2>/dev/null || echo 0)\"\n", marks.display(), mods.join("wait").display())).unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let runs = || std::fs::read_to_string(&marks).map_or(0, |t| t.lines().count());

        let ride = Ride::on();
        let job = still_job(exe.clone(), mods.clone(), vec!["t".into()]);
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(runs(), 0, "no child while the ride is on");
        assert!(job.try_recv().is_err(), "and the still waits for it");
        drop(ride);
        let (id, got) = job.recv_timeout(Duration::from_secs(20)).expect("shot after the ride");
        assert_eq!((id.as_str(), runs()), ("t", 1));
        assert!(got.is_err(), "the stand-in writes no picture");

        // a ride starting mid-shot kills the child and the still is shot again after the ride
        std::fs::write(mods.join("wait"), "60").unwrap();
        let job = still_job(exe.clone(), mods.clone(), vec!["t".into()]);
        let end = std::time::Instant::now() + Duration::from_secs(20);
        while runs() < 2 {
            assert!(std::time::Instant::now() < end, "the shot started");
            std::thread::sleep(Duration::from_millis(10));
        }
        let ride = Ride::on();
        let stopped = std::time::Instant::now();
        let _one = SHOOTING.lock().unwrap_or_else(|e| e.into_inner());
        assert!(stopped.elapsed() < Duration::from_secs(10), "the child killed, not waited out: {:?}", stopped.elapsed());
        drop(_one);
        std::fs::write(mods.join("wait"), "0").unwrap();
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(runs(), 2, "not shot again while the ride is on");
        drop(ride);
        let (_, got) = job.recv_timeout(Duration::from_secs(20)).expect("shot after the ride");
        assert!(got.is_err());
        assert_eq!(runs(), 3);
        let _ = std::fs::remove_dir_all(&mods);
    }

    #[test]
    fn the_shells_own_shots_read_back() {
        let rgb: Vec<u8> = (0..7 * 5 * 3).map(|i| (i * 37 % 251) as u8).collect();
        let t = decode_png(&game_render::png::encode_rgb(7, 5, &rgb)).expect("reads");
        assert_eq!((t.width, t.height), (7, 5));
        for (i, px) in t.rgba.chunks_exact(4).enumerate() {
            assert_eq!(px, [rgb[3 * i], rgb[3 * i + 1], rgb[3 * i + 2], 255]);
        }
    }

    #[test]
    fn a_bad_picture_is_an_error_not_a_panic() {
        assert!(decode_png(b"nope").is_err());
        let good = game_render::png::encode_rgb(4, 4, &[9; 48]);
        for cut in 0..good.len() {
            let _ = decode_png(&good[..cut]);
        }
        let mut bent = good.clone();
        for i in 8..bent.len() {
            bent[i] ^= 0x5a;
            let _ = decode_png(&bent);
            bent[i] ^= 0x5a;
        }
    }
}
