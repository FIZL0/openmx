//! The track pick's pictures for the mod tracks (P70).
//!
//! A disc track's card shows its stills out of `UI\trackpreview.res` ([`crate::menu::Art`]). A mod
//! brings its own picture as openmx-mods' standard allows (T1 §2: `previews/track.png` in the mod's
//! folder, optional), and where it has none the shell takes one: a still of the mod's map with no
//! HUD, shot by the shell's own binary run headless in the background
//! ([`still_job`]) and kept in the mods folder's `previews/<id>.v<N>.png`, beside `tracks/`, which the
//! scan never reads. A still older than the mod's `mod.json` is taken again, and so is one of another
//! [`STILL_VERSION`].
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
use std::sync::mpsc::{self, Receiver};

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

/// Whether the still of `id` is there and no older than its `mod.json`.
pub fn still_fresh(mods: &Path, id: &str) -> bool {
    let made = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    match (made(&still(mods, id)), made(&mods.join("tracks").join(id).join("mod.json"))) {
        (Some(s), Some(j)) => s >= j,
        (Some(_), None) => true,
        _ => false,
    }
}

/// What the pick has for each of `ids` now, `(id, picture)`, and the ids that need a still taken.
/// A picture that does not read is a line in `log` and no picture.
pub fn gather(mods: &Path, ids: &[String], log: &mut Vec<String>) -> (Vec<(String, Texture)>, Vec<String>) {
    let (mut have, mut take) = (Vec::new(), Vec::new());
    for id in ids {
        let own = own(mods, id);
        let path = if own.is_file() {
            own
        } else if still_fresh(mods, id) {
            still(mods, id)
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
/// `(id, picture)`, and each that fails as `(id, why)`.
pub fn still_job(exe: PathBuf, mods: PathBuf, ids: Vec<String>) -> Receiver<(String, Result<Texture, String>)> {
    let (send, hear) = mpsc::channel();
    std::thread::spawn(move || {
        for id in ids {
            let got = take_still(&exe, &mods, &id);
            if send.send((id, got)).is_err() {
                return;
            }
        }
    });
    hear
}

fn take_still(exe: &Path, mods: &Path, id: &str) -> Result<Texture, String> {
    let dir = mods.join(STILLS_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let out = still(mods, id);
    // Shot under another name and moved over, so a half-written still is never read.
    let part = dir.join(format!("{id}.part.png"));
    let status = still_command(exe, mods, id, &part).status().map_err(|e| format!("the shell: {e}"))?;
    if !status.success() || !part.is_file() {
        let _ = std::fs::remove_file(&part);
        return Err(format!("the ride over the map did not start ({status})"));
    }
    std::fs::rename(&part, &out).map_err(|e| format!("{}: {e}", out.display()))?;
    // The stills of the framings before this one are not read again.
    for v in 1..STILL_VERSION {
        let _ = std::fs::remove_file(still_of(mods, id, v));
    }
    load(&out)
}

/// A PNG file as a texture.
pub fn load(path: &Path) -> Result<Texture, String> {
    decode_png(&std::fs::read(path).map_err(|e| e.to_string())?)
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
