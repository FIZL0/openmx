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
//! F28: a world with no picture of its own gets a map instead, drawn here and not by a shell: a
//! top-down picture of its ground's heights ([`map_picture`]), each tile's terrain read in turn and
//! let go before the next, on the stills' thread and never beside a ride, kept as
//! `previews/<id>.map.png` ([`map`]).
//!
//! Since P166 the still is of the map, not of the rider: the shell's `--overview` draws no bike, rider
//! or plane, and looks from [`overview`]'s framing, raised and pulled back from the spawn over the
//! ground's middle, after the world has had [`STILL_FRAME`] frames to load.
//!
//! The picture's size is the standard's to leave open and this loader's to decide: any 8-bit PNG
//! up to [`MAX_SIDE`] a side, shown cropped to the card's 16:9 about its middle, so a 16:9 picture
//! (the stills are [`STILL_SIZE`]) shows whole. Nothing in here can panic on a mod's bytes: a PNG
//! that does not read is one log line and a card with no picture.

use game_render::disc::Res;
use game_render::pst::Texture;
use game_sim::archive::inflate;
use game_sim::tiles::{Piece, Reach, Source, Tile, Tiled};
use game_sim::track::{self, Terrain, TerrainHead, TerrainLayout, TerrainNode};
use ps2_float::F;
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

/// The map picture drawn of world `id` (F28), `mods/previews/<id>.map.png`.
pub fn map(mods: &Path, id: &str) -> PathBuf {
    mods.join(STILLS_DIR).join(format!("{id}.map.png"))
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

/// What the pick has for each of `ids` now, `(id, picture)`, and the ids that need a still taken, or
/// a world's map drawn (F28: a world is shot nowhere, P282). A picture that does not read is a line
/// in `log` and no picture. The browser takes no still and draws no map: a mod with no picture of
/// its own is a card with none.
pub fn gather(mods: &Path, ids: &[String], log: &mut Vec<String>) -> (Vec<(String, Texture)>, Vec<String>) {
    let (mut have, mut take) = (Vec::new(), Vec::new());
    for id in ids {
        let path = if let Some(own) = own_picture(mods, id) {
            own
        } else if cfg!(target_arch = "wasm32") {
            continue;
        } else if let Some(still) = kept(mods, id) {
            still
        } else if is_world(mods, id) && map(mods, id).is_file() {
            map(mods, id)
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

/// Take a still of each of `ids` in turn, off the menu's thread, or draw a world's map (F28, no shell
/// run); each that is taken comes back as `(id, picture)`, and each that fails as `(id, why)`. None is
/// shot or drawn while a ride loads or runs: the job waits for it, and one a ride stops is taken
/// again after it (P282).
pub fn still_job(exe: PathBuf, mods: PathBuf, ids: Vec<String>) -> Receiver<(String, Result<Texture, String>)> {
    let (send, hear) = mpsc::channel();
    std::thread::spawn(move || {
        for id in ids {
            let got = loop {
                while riding() {
                    std::thread::sleep(WATCH);
                }
                let took = if is_world(&mods, &id) { take_map(&mods, &id) } else { take_still(&exe, &mods, &id) };
                match took {
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

/// World `id`'s map drawn (F28) and kept as [`map`], alone ([`SHOOTING`]) and never beside a ride: a
/// ride starting stops it between two tiles' reads.
fn take_map(mods: &Path, id: &str) -> Result<Texture, Shot> {
    let _one = SHOOTING.lock().unwrap_or_else(|e| e.into_inner());
    if riding() {
        return Err(Shot::Ride);
    }
    let pieces = map_pieces(mods, id).map_err(Shot::Failed)?;
    let tiles = pieces.iter().map(|p| p.tile.clone()).collect();
    let rgb = map_picture(tiles, |i| std::fs::read(&pieces[i].terrain).map_err(|e| format!("{}: {e}", pieces[i].terrain.display())), &riding)?;
    let dir = mods.join(STILLS_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| Shot::Failed(format!("{}: {e}", dir.display())))?;
    let (out, part) = (map(mods, id), dir.join(format!("{id}.map.part.png")));
    let (w, h) = STILL_SIZE;
    std::fs::write(&part, game_render::png::encode_rgb(w, h, &rgb)).map_err(|e| Shot::Failed(format!("{}: {e}", part.display())))?;
    std::fs::rename(&part, &out).map_err(|e| Shot::Failed(format!("{}: {e}", out.display())))?;
    load(&out).map_err(Shot::Failed)
}

/// One tile or join of a world as its map needs it: the simulation's statement of it, and its
/// terrain file.
struct MapPiece {
    tile: Tile,
    terrain: PathBuf,
}

/// A world's tiles, then its joins, out of its `mod.json`, each tile's scene and each terrain's header
/// and tree root: what `worlds::find` reads, without the scenes kept, the `drop`s or a wrapped
/// world's seam joins (the map leaves the seams' strips empty). No ground is read.
fn map_pieces(mods: &Path, id: &str) -> Result<Vec<MapPiece>, String> {
    use crate::track_mods::Json;
    let dir = mods.join("tracks").join(id);
    let text = std::fs::read_to_string(dir.join("mod.json")).map_err(|e| format!("{id}: mod.json: {e}"))?;
    let json = Json::parse(&text).map_err(|e| format!("{id}: mod.json: {e}"))?;
    let list = |key: &str| match json.get(key) {
        Some(Json::Arr(a)) => a.clone(),
        _ => Vec::new(),
    };
    let numbers = |v: Option<&Json>| -> Vec<f64> {
        match v {
            Some(Json::Arr(a)) => a.iter().filter_map(|n| n.num().and_then(|s| s.parse().ok())).collect(),
            _ => Vec::new(),
        }
    };
    let at_of = |v: Option<&Json>, name: &str| -> Result<[F; 2], String> {
        match numbers(v)[..] {
            [x, z] => Ok([F(x as f32), F(z as f32)]),
            _ => Err(format!("{id}: {name}'s at is not two numbers")),
        }
    };
    let mut out = Vec::new();
    for t in list("tiles") {
        let name = t.get("tile").and_then(Json::str).ok_or_else(|| format!("{id}: a tile with no name"))?.to_string();
        let at = at_of(t.get("at"), &name)?;
        let keep = match numbers(t.get("keep"))[..] {
            [a, b, c, d] => [a as u32, b as u32, c as u32, d as u32],
            _ => return Err(format!("{id}: {name}'s keep is not four numbers")),
        };
        let folder = dir.join("tiles").join(&name);
        let res = Res::folder(&folder).map_err(|e| format!("{id}: {name} is not installed: {e}"))?;
        let scenes: Vec<&str> = res.entries().iter().map(|e| e.name.as_str()).filter(|n| n.to_ascii_lowercase().ends_with(".scn")).collect();
        let [scn] = scenes.as_slice() else { return Err(format!("{id}: {name}: {} .scn files, and a tile has one", scenes.len())) };
        let scene = track::Scene::parse(&res.read(scn)?).map_err(|e| format!("{id}: {name}: {scn}: {e}"))?;
        let terrain = crate::track_mods::terrain_path(&res, &folder, &scene.terrain_file).map_err(|e| format!("{id}: {name}: {e}"))?;
        let (spacing, levels, _) = terrain_shape(&terrain).map_err(|e| format!("{id}: {name}: {e}"))?;
        out.push(MapPiece { tile: Tile { name, at, layout: scene.terrain, spacing, levels, keep, first_owner: 0 }, terrain });
    }
    for j in list("joins") {
        let name = j.get("join").and_then(Json::str).ok_or_else(|| format!("{id}: a join with no name"))?.to_string();
        let at = at_of(j.get("at"), &name)?;
        let folder = dir.join("joins").join(&name);
        let tagged = folder.join(format!("{name}.ps2.trn"));
        let terrain = if tagged.is_file() { tagged } else { folder.join(format!("{name}.trn")) };
        let (spacing, levels, (width, breadth)) = terrain_shape(&terrain).map_err(|e| format!("{id}: {name}: {e}"))?;
        let layout = TerrainLayout { width, breadth, width_scale: spacing, breadth_scale: spacing, ..TerrainLayout::default() };
        let mut tile = Tile { name, at, layout, spacing, levels, keep: [0; 4], first_owner: 0 };
        tile.keep = tile.every_cell();
        out.push(MapPiece { tile, terrain });
    }
    Ok(out)
}

/// A terrain file's spacing, tree levels and root cells across and along, from its header and its
/// tree's root alone, as `worlds::find` reads them: a few kilobytes of the file.
fn terrain_shape(path: &Path) -> Result<(F, u32, (u32, u32)), String> {
    use std::io::{Read, Seek, SeekFrom};
    let name = || path.display().to_string();
    let mut file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", name()))?;
    let len = file.metadata().map_err(|e| format!("{}: {e}", name()))?.len();
    let mut read_at = |at: u64, n: usize| -> Result<Vec<u8>, String> {
        let n = len.saturating_sub(at).min(n as u64) as usize;
        let mut out = vec![0; n];
        file.seek(SeekFrom::Start(at)).and_then(|_| file.read_exact(&mut out)).map_err(|e| format!("{}: {e}", name()))?;
        Ok(out)
    };
    let head = TerrainHead::parse(&read_at(0, TerrainHead::MOST)?)?;
    head.fits(len)?;
    let root = TerrainNode::read(&read_at(u64::from(head.root_at), TerrainNode::BYTES)?).ok_or("no tree")?;
    let (mut w, mut b) = (0, 0);
    if root.children_at != 0 {
        let table = read_at(u64::from(root.children_at), TerrainNode::TABLE_BYTES)?;
        for cz in 0..track::TREE_ARITY {
            for cx in 0..track::TREE_ARITY {
                let at = (cx + cz * track::TREE_ARITY) as usize * 4;
                if table.get(at..at + 4).is_some_and(|w| w != [0; 4]) {
                    w = w.max(cx + 1);
                    b = b.max(cz + 1);
                }
            }
        }
    }
    Ok((head.scale, head.levels, (w.max(1), b.max(1))))
}

/// The owners a world's map asks, which read nothing.
struct NoGround;

impl Source for NoGround {
    fn load(&mut self, _: usize) -> Result<Piece, String> {
        Err("a map reads no piece through its owners".into())
    }
}

/// The colour off the ground, and the ground's from low to high.
const MAP_SEA: [f32; 3] = [24.0, 28.0, 34.0];
const MAP_RAMP: [[f32; 3]; 3] = [[66.0, 104.0, 58.0], [150.0, 128.0, 88.0], [222.0, 218.0, 204.0]];
/// How much steeper the shading draws the ground than it is, so a map miles across shows its hills.
const MAP_RELIEF: f32 = 4.0;

/// A world's map (F28), [`STILL_SIZE`] RGB, north (`+z`) up and east (`+x`) right as the HUD's map
/// has them: the whole world fitted in and centred, each pixel the ground's height at its middle on the
/// piece that owns the place ([`Tiled::owner`]), coloured low to high over the heights
/// found and shaded as lit from the north-west; off the ground [`MAP_SEA`]. `terrain(i)` is piece
/// `i`'s file, read one piece at a time and let go before the next, only for the pieces that own a
/// pixel; `stop` is asked before each, and stops the map as [`Shot::Ride`].
pub fn map_picture(tiles: Vec<Tile>, mut terrain: impl FnMut(usize) -> Result<Vec<u8>, String>, stop: &dyn Fn() -> bool) -> Result<Vec<u8>, Shot> {
    let tiled = Tiled::new(tiles.clone(), Box::new(NoGround), Reach::DESKTOP).map_err(Shot::Failed)?;
    let [width, depth] = tiled.extent().map(|e| e.0);
    if !(width > 0.0 && depth > 0.0) {
        return Err(Shot::Failed("a world with no ground".into()));
    }
    let (w, h) = STILL_SIZE;
    let feet = (width / w as f32).max(depth / h as f32);
    let (x0, z1) = (width / 2.0 - feet * w as f32 / 2.0, depth / 2.0 + feet * h as f32 / 2.0);
    let mut asks: Vec<Vec<(usize, f32, f32)>> = vec![Vec::new(); tiles.len()];
    for py in 0..h {
        for px in 0..w {
            let (x, z) = (x0 + (px as f32 + 0.5) * feet, z1 - (py as f32 + 0.5) * feet);
            if let Some((i, lx, lz)) = tiled.owner(F(x), F(z)) {
                asks[i].push((py * w + px, lx.0, lz.0));
            }
        }
    }
    let mut heights = vec![f32::NAN; w * h];
    for (i, points) in asks.iter().enumerate().filter(|(_, p)| !p.is_empty()) {
        if stop() {
            return Err(Shot::Ride);
        }
        let bytes = terrain(i).map_err(Shot::Failed)?;
        let t = Terrain::parse(&bytes).map_err(|e| Shot::Failed(format!("{}: terrain: {e}", tiles[i].name)))?;
        for &(at, lx, lz) in points {
            heights[at] = height_near(&t, &tiles[i], lx, lz).unwrap_or(f32::NAN);
        }
    }
    let (lo, hi) = heights.iter().filter(|v| v.is_finite()).fold((f32::MAX, f32::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
    let rise = (hi - lo).max(1.0e-3);
    let at = |x: usize, y: usize| heights[y.min(h - 1) * w + x.min(w - 1)];
    // The light from the north-west and above: west is -x, north up the picture.
    let light = {
        let l = [-1.0f32, 1.5, 1.0];
        let n = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
        l.map(|v| v / n)
    };
    let mut rgb = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for x in 0..w {
            let v = at(x, y);
            if !v.is_finite() {
                rgb.extend(MAP_SEA.map(|c| c as u8));
                continue;
            }
            // A neighbour off the ground stands at this pixel's height.
            let near = |nx: usize, ny: usize| Some(at(nx, ny)).filter(|n| n.is_finite()).unwrap_or(v);
            let east = (near(x + 1, y) - near(x.saturating_sub(1), y)) / (2.0 * feet) * MAP_RELIEF;
            let north = (near(x, y.saturating_sub(1)) - near(x, y + 1)) / (2.0 * feet) * MAP_RELIEF;
            let n = [-east, 1.0, -north];
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            let lit = (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]) / len;
            let shade = (0.35 + 0.65 * lit / light[1]).clamp(0.45, 1.25);
            let t = ((v - lo) / rise).clamp(0.0, 1.0) * 2.0;
            let (a, b, f) = if t < 1.0 { (MAP_RAMP[0], MAP_RAMP[1], t) } else { (MAP_RAMP[1], MAP_RAMP[2], t - 1.0) };
            rgb.extend((0..3).map(|k| ((a[k] + (b[k] - a[k]) * f) * shade).clamp(0.0, 255.0) as u8));
        }
    }
    Ok(rgb)
}

/// The height in world feet of a point in a piece's own feet: across the cell it is in, or where
/// that cell does not read, the sample nearest it from a neighbour's corner, as `worlds::stand_in`
/// finds its corners.
fn height_near(t: &Terrain, tile: &Tile, lx: f32, lz: f32) -> Option<f32> {
    let spacing = tile.spacing.0;
    let [sx_n, sz_n] = tile.samples();
    let (fx, fz) = ((lx / spacing).max(0.0), (lz / spacing).max(0.0));
    let (cx, cz) = ((fx as u32).min(sx_n.saturating_sub(2)), (fz as u32).min(sz_n.saturating_sub(2)));
    if let Some(q) = t.cell(cx, cz, 1) {
        let (u, v) = ((fx - cx as f32).clamp(0.0, 1.0), (fz - cz as f32).clamp(0.0, 1.0));
        let [a, b, c, d] = q.height.map(|h| h.0);
        return Some(t.scale.0 * ((a * (1.0 - u) + b * u) * (1.0 - v) + (c * (1.0 - u) + d * u) * v));
    }
    let sx = (fx.round() as u32).min(sx_n.saturating_sub(1));
    let sz = (fz.round() as u32).min(sz_n.saturating_sub(1));
    [(0, 0, 0), (1, 1, 0), (2, 0, 1), (3, 1, 1)]
        .into_iter()
        .filter(|&(_, dx, dz)| sx >= dx && sz >= dz)
        .find_map(|(k, dx, dz)| t.cell(sx - dx, sz - dz, 1).map(|q| t.scale.0 * q.height[k].0))
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
    /// older one) are shown however much newer its `mod.json` is, and no still is taken of them; only
    /// a mod with none is taken, a world's as its map (F28).
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
        // The world with none is a map to draw (F28), which `still_job` draws with no shell.
        assert_eq!(take, ["bare", "big"], "only the mods with no picture: {log:?}");
        assert!(log.is_empty(), "{log:?}");
        // A kept map is a world's picture, and not a track's.
        let mods2 = scratch("kept-map", &[("big_map", "world"), ("trk", "track")]);
        picture(&map(&mods2, "big_map"));
        picture(&map(&mods2, "trk"));
        let (have, take) = gather(&mods2, &["big_map".into(), "trk".into()], &mut log);
        assert_eq!((have.len(), have[0].0.as_str(), take), (1, "big_map", vec!["trk".to_string()]));
        let _ = std::fs::remove_dir_all(&mods2);
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

    /// F28: a world with no picture gets a map of its ground drawn here, kept as `<id>.map.png` and
    /// read back by the next gather; no shell is run for it (the stand-in shell marks every run), and
    /// none is drawn while a ride is on. Two generated tiles side by side, the east one a hill.
    #[cfg(unix)]
    #[test]
    fn a_world_with_no_picture_gets_a_map_and_no_child() {
        use game_sim::tiles::generate;
        use std::os::unix::fs::PermissionsExt;
        let mods = scratch("map", &[]);
        let world = mods.join("tracks").join("f28_world");
        let write = |rel: &str, bytes: &[u8]| {
            let path = world.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        };
        // 4 x 4 root cells of 16 samples at 8 feet: 512 feet a tile, kept to 63 cells.
        for (name, at, hill) in [("west", 0, false), ("east", 504, true)] {
            let stem = name.to_uppercase();
            let height = |sx: u32, sz: u32| if hill { 40.0 + 200.0 * (-(((sx as f64 - 32.0).powi(2) + (sz as f64 - 32.0).powi(2)) / 200.0)).exp() } else { 40.0 };
            write(&format!("tiles/{name}/Env/{stem}/{stem}.ps2.trn"), &generate::terrain(name, (4, 4), 8.0, height, |_, _| 1));
            let scene = format!("[SceneInfo]\r\nSceneName=F28 {name}\r\n[Environment]\r\nTerrainFile={stem}.trn\r\nTerrainWidth=4\r\nTerrainBreadth=4\r\nTerrainWidthScale=8.00\r\nTerrainBreadthScale=8.00\r\n");
            write(&format!("tiles/{name}/Env/{stem}/{stem}.scn"), scene.as_bytes());
        }
        let json = r#"{"standard": 1, "kind": "world", "id": "f28_world", "name": "F28", "tiles": [{"tile": "west", "at": [0, 0], "keep": [0, 0, 63, 63]}, {"tile": "east", "at": [504, 0], "keep": [0, 0, 63, 63]}]}"#;
        write("mod.json", json.as_bytes());
        let (exe, marks) = (mods.join("shell.sh"), mods.join("marks"));
        std::fs::write(&exe, format!("#!/bin/sh\necho run >> '{}'\n", marks.display())).unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();

        let ids = vec!["f28_world".to_string()];
        let mut log = Vec::new();
        let (have, take) = gather(&mods, &ids, &mut log);
        assert!(have.is_empty() && take == ids, "{log:?}");
        let ride = Ride::on();
        let job = still_job(exe.clone(), mods.clone(), take);
        std::thread::sleep(Duration::from_millis(300));
        assert!(job.try_recv().is_err() && !map(&mods, "f28_world").exists(), "not drawn beside a ride");
        drop(ride);
        let (id, got) = job.recv_timeout(Duration::from_secs(60)).expect("drawn after the ride");
        let t = got.unwrap_or_else(|e| panic!("{id}: {e}"));
        assert!(!marks.exists(), "no shell was run");
        assert_eq!((id.as_str(), (t.width, t.height)), ("f28_world", STILL_SIZE));
        assert!(map(&mods, "f28_world").is_file() && kept(&mods, "f28_world").is_none(), "kept as the map, not a still");
        // The world is 1008 x 504 feet: it fills the picture's width, centred up and down, north up.
        let feet = 1008.0 / t.width as f32;
        let px = |x: f32, z: f32| -> [u8; 3] {
            let (col, row) = ((x / feet) as usize, ((t.height as f32 * feet / 2.0 + 252.0 - z) / feet) as usize);
            let i = (row * t.width + col) * 4;
            [t.rgba[i], t.rgba[i + 1], t.rgba[i + 2]]
        };
        let sea = MAP_SEA.map(|c| c as u8);
        assert_eq!(&t.rgba[4 * (2 * t.width + 256)..][..3], &sea[..], "off the ground over the world");
        // The hill's top on the east tile is the lightest, the flat west tile lower; and the hill's
        // north-west side, toward the light, lighter than its south-east side.
        let sum = |c: [u8; 3]| c.iter().map(|&v| v as u32).sum::<u32>();
        let (west, top) = (px(250.0, 250.0), px(760.0, 256.0));
        assert!(west != sea && sum(top) > sum(west) + 100, "the hill's top {top:?}, the flat {west:?}");
        let (lit, dark) = (px(730.0, 286.0), px(790.0, 226.0));
        assert!(sum(lit) > sum(dark) + 30, "lit {lit:?}, dark {dark:?}");
        // Kept: the next gather shows it, and takes nothing.
        let (have, take) = gather(&mods, &ids, &mut log);
        assert!(have.len() == 1 && take.is_empty(), "{log:?}");
        assert!(!marks.exists());
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
