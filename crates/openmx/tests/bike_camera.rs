//! S103: the bike's race camera keeps its eye out of the ground as the original's does.
//!
//! The race camera's filter asks its class, before the lags and again after them, to keep the eye a
//! floor above the ground under it: twice the near clip, or the camera's own minimum height if that
//! is more (`research/oracle.txt`, S103). `game-sim`'s `tests/camera.rs` holds the port's floor to the
//! disc's code bit for bit; this holds it to the original's camera behind the human's free09 ride, over
//! the track's own terrain. The journal's `bike-camera.toml` captured the rider's camera block, the
//! camera's floor state and the eye on every tick of `first_recording.p2m2`, twice: as the disc has it
//! (`bike-camera.csv`), where the eye never comes within the floor of the dirt, and with the camera's
//! minimum height poked to 12 ft after the state loaded (`bike-camera-raised.csv`), where the floor
//! holds the eye up for half the ride, over the hollows and the hills.

use std::collections::HashMap;
use std::path::PathBuf;

use game_render::Disc;
use game_sim::archive::Res;
use game_sim::camera::{CurveTable, Preset, Race, Ridden, Surface, Switches, Target};
use game_sim::track::{Scene, Terrain};
use game_sim::world::Ground;
use ps2_float::{F, Vf};

/// The shell's constants for the bike's camera (`main.rs`).
const REVERSE_SPEED: f32 = 5.0;
const SPEED_REFERENCE: f32 = 117.332_8;
const NEAR_CLIP: f32 = 2.4;

/// The recording's track.
const TRACK: &str = "free09";

fn context() -> PathBuf {
    match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context"),
    }
}

/// The shortest member of the disc whose name ends in `suffix`, as the shell picks a track's.
fn member_ending(disc: &Disc, suffix: &str) -> Option<String> {
    disc.entries()
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().ends_with(suffix))
        .min_by_key(|e| e.name.len())
        .map(|e| e.name.clone())
}

/// The track's camera presets, scene and terrain bytes, `None` without the disc. A track is two
/// archives, as the shell loads it: the terrain is in the first, the follow camera in the second.
fn track() -> Option<(Vec<Preset>, Scene, Vec<u8>)> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            eprintln!("skipped: {why}");
            return None;
        }
    };
    let mut archives = Vec::new();
    for suffix in [format!("{TRACK}.res"), format!("{TRACK}_2.res")] {
        let bytes = disc.member(&member_ending(&disc, &suffix)?).ok()?;
        let res = Res::parse(&bytes).ok()?;
        archives.push((bytes, res));
    }
    let read = |end: &str| {
        archives.iter().find_map(|(bytes, res)| {
            let e = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(end))?;
            res.read(bytes, e).ok()
        })
    };
    let scene = Scene::parse(&read(".scn")?).ok()?;
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let terrain = read(&leaf)?;
    let presets = Preset::read_bytes(&read("cpresets.ini")?, "");
    Some((presets, scene, terrain))
}

/// A capture's rows, by column name. `None` without it.
fn capture(name: &str) -> Option<Vec<HashMap<String, String>>> {
    let text = std::fs::read_to_string(context().join("capture/s103").join(name)).ok()?;
    let mut lines = text.lines();
    let head: Vec<String> = lines.next()?.split(',').map(str::to_string).collect();
    Some(
        lines
            .map(|l| head.iter().cloned().zip(l.split(',').map(str::to_string)).collect())
            .collect(),
    )
}

fn num(r: &HashMap<String, String>, k: &str) -> f32 {
    r[k].parse().unwrap()
}

fn quad(r: &HashMap<String, String>, n: &str) -> Vf {
    Vf::new(num(r, &format!("{n}.x")), num(r, &format!("{n}.y")), num(r, &format!("{n}.z")), num(r, &format!("{n}.w")))
}

fn dist(a: Vf, b: Vf) -> f32 {
    ((a.x() - b.x()).powi(2) + (a.y() - b.y()).powi(2) + (a.z() - b.z()).powi(2)).sqrt()
}

/// Row `i`'s target block, as the capture read it. The airborne flag is a word the recorder wrote
/// as a float.
fn block(r: &HashMap<String, String>) -> Ridden {
    Ridden {
        target: Target {
            body_point: quad(r, "tgt_point"),
            forward: quad(r, "tgt_fwd"),
            up: quad(r, "tgt_up"),
            vel: quad(r, "tgt_vel"),
            lean: F(quad(r, "tgt_tail").y()),
        },
        airborne: num(r, "tgt_flag_speed.x") != 0.0,
        speed: F(num(r, "tgt_flag_speed.y")),
    }
}

/// Ticks the camera's lags take to forget a block the capture read wrong, or a wreck's hand-back.
const SETTLE: usize = 120;

/// What one replay found.
#[derive(Default, Debug)]
struct Replay {
    compared: usize,
    /// The farthest the port's eye was from the original's, and the row.
    worst: (f32, usize),
    /// Rows the port's eye is the original's on to a hundredth of a foot, and further rows whose
    /// whole difference is the two grounds' under the eye: both eyes lifted onto the floor, each over
    /// its own ground.
    exact: usize,
    grounds: usize,
    /// The most the two grounds differed under a lifted eye.
    ground_gap: f32,
    /// Rows the original's floor lock was set on, and rows where the port's disagreed.
    locked: usize,
    lock_off: usize,
    /// The lowest the captured eye came over free09's dirt as the port has it, and the floor the
    /// capture says the camera had.
    lowest: f32,
    floor: f32,
    /// The wrecks the replay stepped over.
    wrecks: usize,
}

/// The capture's block replayed through the port's camera, on the preset the capture's camera had,
/// over free09's ground.
///
/// The capture is polled, not synced (`capture-rows-read-at-three-phases`): a row's eye is taken as
/// the port's eye of that tick, the one before or the one after, whichever is nearest. While the rider
/// is down the wreck camera has the viewport, which is not this camera's; back on, the original eases
/// the eye back from where the wreck camera left it, so the rows after a wreck are not compared either.
fn replay(rows: &[HashMap<String, String>], presets: Vec<Preset>, ground: &mut Ground) -> Replay {
    let curve = CurveTable::build();
    let mut cam = Race::new(presets, F(REVERSE_SPEED));
    cam.cam.set_preset(num(&rows[0], "cam_preset") as usize);
    // The state was saved with the camera already behind the bike: a snap is where it is.
    cam.cam.snap = true;
    cam.cam.speed_reference = F(SPEED_REFERENCE);
    // The camera's floor and ceiling as the capture read them (the raised run's floor was poked).
    cam.cam.min_height = F(num(&rows[0], "cam_floor_ceiling.z"));
    cam.cam.ceiling = F(num(&rows[0], "cam_floor_ceiling.w"));
    let near = num(&rows[0], "vp_near");
    let mut out = Replay { lowest: f32::MAX, floor: (near * 2.0).max(cam.cam.min_height.0), ..Replay::default() };
    // The port's eye and lock for every row, `None` on the rows the wreck camera had.
    let mut ours: Vec<Option<(Vf, bool)>> = Vec::with_capacity(rows.len());
    let mut skip = vec![false; rows.len()];
    let mut last: Option<Ridden> = None;
    let mut settled_at = SETTLE / 4;
    for (i, r) in rows.iter().enumerate() {
        if num(r, "wreck_reason") != 0.0 {
            if !cam.cam.snap {
                out.wrecks += 1;
            }
            // Back on, the follow camera takes over with a snap (`chase::WreckCamera::switch`).
            cam.cam.snap = true;
            last = None;
            settled_at = i + SETTLE;
            ours.push(None);
            continue;
        }
        let rider = block(r);
        if let Some(p) = &last {
            // A block read before the rider wrote it: last tick's point, while it is moving.
            if p.target.body_point == rider.target.body_point && p.speed.0 > 1.0 {
                settled_at = i + SETTLE;
            }
        }
        let view = cam.update(
            F(num(r, "dt")),
            &rider,
            F(near),
            &curve,
            Switches { no_look_ahead: false, short_viewport: false },
            ground,
        );
        last = Some(rider);
        ours.push(Some((view.position, cam.cam.pitch_down_locked)));
        skip[i] = i < settled_at;
    }
    for (i, r) in rows.iter().enumerate() {
        let Some((_, port_locked)) = ours[i] else { continue };
        let eye = quad(r, "vp_eye");
        let height = eye.y() - ground.on_surface(eye).y();
        // The first row back on still has the wreck camera's last eye in the viewport.
        if i == 0 || ours[i - 1].is_some() {
            out.lowest = out.lowest.min(height);
        }
        if skip[i] {
            continue;
        }
        let near_by = i.saturating_sub(1)..(i + 2).min(rows.len());
        let off = ours[near_by].iter().flatten().map(|(p, _)| dist(*p, eye)).fold(f32::MAX, f32::min);
        let locked = num(r, "cam_lock") != 0.0;
        out.compared += 1;
        if off > out.worst.0 {
            out.worst = (off, i);
        }
        if off < 0.01 {
            out.exact += 1;
        } else if locked && (off - (out.floor - height).abs()).abs() < 0.01 {
            out.grounds += 1;
            out.ground_gap = out.ground_gap.max(off);
        }
        out.locked += usize::from(locked);
        out.lock_off += usize::from(locked != port_locked);
    }
    out
}

fn ride(name: &str) -> Option<Replay> {
    let Some((presets, scene, bytes)) = track() else {
        eprintln!("skipped: no {TRACK} on the disc");
        return None;
    };
    let Some(rows) = capture(name) else {
        eprintln!("skipped: no context/capture/s103/{name}");
        return None;
    };
    let terrain = Terrain::parse(&bytes).expect("free09's terrain");
    let mut ground = Ground::new(&terrain, &scene.terrain);
    let out = replay(&rows, presets, &mut ground);
    println!("{name}: {out:?} of {} rows", rows.len());
    Some(out)
}

/// The human's ride as the disc has it: the floor is twice the near clip, 4.8 ft, and the original's
/// eye never comes that near free09's dirt, so the floor never locks — the port's neither — and the
/// port's eye is the original's on every compared row, through two wrecks.
#[test]
fn the_bike_camera_keeps_the_originals_eye_over_free09() {
    let Some(out) = ride("bike-camera.csv") else { return };
    assert_eq!(out.floor, 4.8);
    assert_eq!(out.wrecks, 2);
    assert!(out.compared > 1100, "{} rows compared", out.compared);
    // Two rows a run read the block mid-tick (`frame-counter-trigger-tears-captures`).
    assert!(out.exact + 2 >= out.compared, "{out:?}");
    assert!(out.worst.0 < 0.2, "the eye is {} ft off at row {}", out.worst.0, out.worst.1);
    assert_eq!(out.locked, 0, "the disc's floor never locks on this ride");
    assert_eq!(out.lock_off, 0);
    assert!(out.lowest >= out.floor, "the original's eye came {} ft over free09's ground", out.lowest);
}

/// The same ride with the camera's minimum height poked to 12 ft: the floor holds the eye up over the
/// hollows and the hills for half the ride, and the port's floor locks on the rows the original's
/// does and holds the eye where the original's is — up to the ground under it, which is
/// `world::Ground`'s own blend and not the game's terrain query (`world.rs`'s module note): where the
/// two disagree, each eye sits the floor over its own ground, and the difference is theirs.
#[test]
fn the_raised_floor_holds_the_eye_where_the_originals_is() {
    let Some(out) = ride("bike-camera-raised.csv") else { return };
    assert_eq!(out.floor, 12.0);
    assert!(out.compared > 1100, "{} rows compared", out.compared);
    assert!(out.locked > 500, "the floor locked on {} rows", out.locked);
    // One row, where the eye sits on the floor within the grounds' difference.
    assert!(out.lock_off <= 2, "the port's lock disagrees on {} rows", out.lock_off);
    // The rest is the lags carrying a lift the two grounds set differently, a few hundredths of a foot.
    assert!(out.exact + out.grounds >= out.compared * 80 / 100, "{out:?}");
    assert!(out.grounds > 50, "{out:?}");
    assert!(out.worst.0 <= out.ground_gap + 0.01, "the eye is {} ft off at row {}, more than the grounds ever differ ({})", out.worst.0, out.worst.1, out.ground_gap);
    assert!(out.ground_gap < 0.5, "{out:?}");
    // The original's eye, lifted over its own ground, never under the floor over the port's by more
    // than the grounds differ.
    assert!(out.lowest >= out.floor - 0.5, "the original's eye came {} ft over free09's ground", out.lowest);
}
