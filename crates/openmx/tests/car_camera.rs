//! S246: the cars' cameras, against the original's own behind S149a's car drives.
//!
//! A car's camera is the race camera the bike and the plane have, built by the same constructor and
//! handed its kind's `AppStart\<Kind>CPresets.ini` (`research/oracle.txt`, S246): FourWheelCPresets.ini
//! for the baja, BuggyCPresets.ini for the buggy and MonsterTruckCPresets.ini for the truck. What is the
//! car's own is the block it fills once a frame. `watch/car-camera.toml` captured that block, the
//! car's body and measure, its camera and the viewport the camera wrote on every tick of S149a's three
//! tapes (`context/capture/s246/<car>.csv`, `research/journal/2026-11-02-s246-car-camera/capture.sh`).
//! The first test builds the block from the captured body and measure through `chase::driven` and
//! holds it to the original's word for word; the second replays the captured block through the
//! port's camera on free07's ground and compares the eye and the view with the original's.
//! Each skips without the disc or the capture.

#[allow(dead_code)]
#[path = "../src/vehicle.rs"]
mod vehicle;
#[allow(dead_code)]
#[path = "../src/chase.rs"]
mod chase;

use std::collections::HashMap;
use std::path::PathBuf;

use game_render::disc::Disc;
use game_sim::camera::{CurveTable, Preset, Race, Ridden, Surface, Switches, Target};
use game_sim::car::{Car, CarModel, Kind, Machine};
use game_sim::track::{Scene, Terrain};
use game_sim::world::Ground;
use ps2_float::{F, Vf};

/// The shell's constants for the follow cameras (`main.rs`): the race camera's reverse threshold, the
/// speed reference, the near clip. The cars' cameras in the S149a states carry the same speed
/// reference (+0x14c 117.3328) and height bias (3.0) as the plane's.
const REVERSE_SPEED: f32 = 5.0;
const SPEED_REFERENCE: f32 = 117.332_8;
const NEAR_CLIP: f32 = 2.4;

/// Each capture: its name, the car's class and its model. Each car's presets file is the shell's
/// own pick ([`chase::car_presets`]).
const CARS: [(&str, Kind, &str); 3] = [
    ("baja", Kind::Baja, "baja\\anim\\baja.psg"),
    ("buggy", Kind::Buggy, "buggy\\anim\\buggy.psg"),
    ("truck", Kind::MonsterTruck, "monstertruck\\anim\\monstertruck.psg"),
];

fn context() -> PathBuf {
    match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context"),
    }
}

/// A camera presets file off the disc (free06's archive carries all five machines'), `None` without it.
fn presets(disc: &mut Disc, file: &str) -> Option<Vec<Preset>> {
    let names: Vec<String> = disc.entries().iter().map(|e| e.name.clone()).filter(|n| n.to_ascii_lowercase().contains("free06")).collect();
    let bytes = names.iter().find_map(|n| disc.res(n).ok()?.read_suffix(file).ok())?;
    let presets = Preset::read_bytes(&bytes, "");
    (!presets.is_empty()).then_some(presets)
}

type Row = HashMap<String, String>;

/// A capture's rows, by column name. `None` without it.
fn capture(name: &str) -> Option<Vec<Row>> {
    let text = std::fs::read_to_string(context().join(format!("capture/s246/{name}.csv"))).ok()?;
    let mut lines = text.lines();
    let head: Vec<String> = lines.next()?.split(',').map(str::to_string).collect();
    Some(lines.map(|l| head.iter().cloned().zip(l.split(',').map(str::to_string)).collect()).collect())
}

fn num(r: &Row, k: &str) -> f32 {
    r[k].parse().unwrap()
}

fn quad(r: &Row, n: &str) -> Vf {
    Vf::new(num(r, &format!("{n}.x")), num(r, &format!("{n}.y")), num(r, &format!("{n}.z")), num(r, &format!("{n}.w")))
}

/// A word the recorder wrote as a float.
fn word(r: &Row, k: &str) -> u32 {
    num(r, k).to_bits()
}

fn bits(v: Vf) -> [u32; 4] {
    [v.x().to_bits(), v.y().to_bits(), v.z().to_bits(), v.w().to_bits()]
}

fn dist(a: Vf, b: Vf) -> f32 {
    ((a.x() - b.x()).powi(2) + (a.y() - b.y()).powi(2) + (a.z() - b.z()).powi(2)).sqrt()
}

/// The angle between two directions, radians (in f64, off the cross product: an f32 `acos` cannot
/// tell angles under ~5e-4 apart).
fn angle(a: Vf, b: Vf) -> f32 {
    let (a, b) = ([a.x() as f64, a.y() as f64, a.z() as f64], [b.x() as f64, b.y() as f64, b.z() as f64]);
    let cross = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    (cross.iter().map(|c| c * c).sum::<f64>().sqrt()).atan2(dot) as f32
}

/// Row `r`'s target block, as the capture read it.
fn block(r: &Row) -> Ridden {
    Ridden {
        target: Target {
            body_point: quad(r, "tgt_point"),
            forward: quad(r, "tgt_fwd"),
            up: quad(r, "tgt_up"),
            vel: quad(r, "tgt_vel"),
            lean: F(quad(r, "tgt_tail").y()),
        },
        airborne: word(r, "tgt_flag_speed.x") != 0,
        speed: F(num(r, "tgt_flag_speed.y")),
    }
}

/// The block's rows are the measure's of the tick before them: rows whose body moved since the
/// writer ran (a poll that straddled the tick) do not hold the block to the body.
fn torn(r: &Row, kind: Kind) -> bool {
    let point = quad(r, "tgt_point");
    let last = quad(r, "body_last");
    let reach = match kind {
        Kind::MonsterTruck => 3.6,
        _ => 1.4,
    };
    dist(point, last) > reach || bits(quad(r, "tgt_vel"))[..3] != bits(quad(r, "body_vel"))[..3]
}

/// Each car's block, made by `chase::driven` from the captured body and measure, is the original's
/// word for word: the point off the centre of mass where the last substep began, moved on by the
/// turned centre of mass for the baja and the buggy and back by it for the truck; the measure's rows,
/// the body's velocity, the world yaw rate, "no tire down" and the speed. Without the class's sign
/// the truck's point is 7 ft out.
#[test]
fn each_cars_block_is_the_originals() {
    let Ok(mut disc) = Disc::open_configured() else {
        eprintln!("skipped: no disc");
        return;
    };
    let res = disc.res("machine.res").unwrap();
    let mut seen = 0;
    for (name, kind, model) in CARS {
        let Some(rows) = capture(name) else {
            eprintln!("skipped {name}: no context/capture/s246/{name}.csv");
            continue;
        };
        let car = Car::read_bytes(&res.read(kind.phy()).unwrap(), kind);
        let model = CarModel::read(&res.read(model).unwrap()).unwrap();
        let mut m = Machine::new(&car, &model, Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, -1.0, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0));
        let (mut compared, mut torn_rows, mut airborne) = (0, 0, 0);
        for r in &rows {
            if torn(r, kind) {
                torn_rows += 1;
                continue;
            }
            m.body.last_position = quad(r, "body_last");
            m.body.position = quad(r, "body_pos");
            m.body.velocity = quad(r, "body_vel");
            m.body.quat = quad(r, "body_quat");
            m.body.cofg = quad(r, "body_cofg");
            m.forward = quad(r, "car_fwd");
            m.up = quad(r, "car_up");
            m.world_rates = quad(r, "car_rates");
            m.grounded = word(r, "car_grounded") != 0;
            m.ground_speed = F(num(r, "car_speed"));
            let got = chase::driven(&m, kind);
            let want = block(r);
            let at = r["frame_counter"].as_str();
            assert_eq!(bits(got.target.body_point), bits(want.target.body_point), "{name} {at}: the point");
            assert_eq!(bits(got.target.forward), bits(want.target.forward), "{name} {at}: forward");
            assert_eq!(bits(got.target.up), bits(want.target.up), "{name} {at}: up");
            assert_eq!(bits(got.target.vel)[..3], bits(want.target.vel)[..3], "{name} {at}: velocity");
            assert_eq!(got.target.lean.0.to_bits(), want.target.lean.0.to_bits(), "{name} {at}: the yaw rate");
            assert_eq!(got.airborne, want.airborne, "{name} {at}: the flag");
            assert_eq!(got.speed.0.to_bits(), want.speed.0.to_bits(), "{name} {at}: the speed");
            airborne += got.airborne as usize;
            compared += 1;
        }
        eprintln!("{name}: {compared} blocks the original's, {airborne} airborne, {torn_rows} torn rows skipped");
        assert!(compared > 600, "{name}: only {compared} of {} rows compared", rows.len());
        seen += 1;
    }
    eprintln!("{seen} cars' captures compared");
}

/// Ticks the camera's lags take to forget a block the capture read wrong.
const SETTLE: usize = 120;

/// The original's own ground under the look-at's probe, as the capture recorded it: the camera's
/// lift (`+0x18c`) row by row, over free07's ground for everything else.
///
/// The frame asks the surface twice or three times: the probe first (moving, about a point a step
/// ahead of the target point), then the floor (about the eye, a follow distance back). The probe's
/// question is told by its place, within [`PROBE_REACH`] of the block's point. A row whose lift is
/// not last row's decayed is answered with the ground that much over the point, over six tenths
/// (the probe's own gain); any other with the point itself, so the probe decays its own lift.
struct Lifted<'a> {
    ground: Ground<'a>,
    point: Vf,
    lift: F,
    last: F,
}

/// How far from the block's point the probe's question can be: the preset's offset, a step of
/// lead and the probe's reach (1.5 ft at most) are each a few feet. The eye is 15 ft back or more.
const PROBE_REACH: f32 = 8.0;

impl Surface for Lifted<'_> {
    fn on_surface(&mut self, point: Vf) -> Vf {
        if dist(point, self.point) > PROBE_REACH {
            return self.ground.on_surface(point);
        }
        if self.lift <= F::ZERO || self.lift.0.to_bits() == (self.last * 0.02).0.to_bits() {
            return point;
        }
        Vf::new(point.x(), (F(point.y()) + self.lift / 0.6).0, point.z(), point.w())
    }
}

/// Either ground the replay can run over.
enum Under<'a> {
    Free07(Ground<'a>),
    Captured(Lifted<'a>),
}

impl Surface for Under<'_> {
    fn on_surface(&mut self, point: Vf) -> Vf {
        match self {
            Under::Free07(g) => g.on_surface(point),
            Under::Captured(l) => l.on_surface(point),
        }
    }
}

/// What a replay found, over the rows it compared.
#[derive(Debug, Default)]
struct Replay {
    compared: usize,
    /// The farthest the eye was from the original's, and its row.
    eye: (f32, usize),
    /// The most the view was turned from the original's, and its row.
    view: (f32, usize),
    /// The nearest the look-at came to the eye.
    reach: f32,
    /// The most the port's lift and the original's differed.
    gap: f32,
    /// Rows the original lifted its look-at on.
    lifted: usize,
}

/// One car's capture replayed through a camera on `presets`' "Race Cam 1" over `under`.
///
/// The capture is polled (`frame-counter-trigger-tears-captures`): a row that read the viewport a
/// tick late is held to either of the port's last two eyes, and a row whose block was read a tick
/// early (last row's point, at speed) is fed to the camera wrong, so the rows a lag would still
/// remember it by are not compared.
fn replay(rows: &[Row], presets: Vec<Preset>, mut under: Under) -> Replay {
    let curve = CurveTable::build();
    let mut cam = Race::new(presets, F(REVERSE_SPEED));
    cam.cam.set_preset(Preset::find(&cam.cam.presets, "Race Cam 1").unwrap());
    // The state was saved with the camera already behind the car, at rest: a snap is where it is.
    cam.cam.snap = true;
    cam.cam.speed_reference = F(SPEED_REFERENCE);
    let mut out = Replay { reach: f32::MAX, ..Replay::default() };
    let mut last: Option<Ridden> = None;
    let mut before = Vf::new(0.0, 0.0, 0.0, 1.0);
    let mut settled_at = SETTLE / 4;
    let mut was = F::ZERO;
    for (i, r) in rows.iter().enumerate() {
        let rider = block(r);
        if let Some(p) = &last
            && p.target.body_point == rider.target.body_point
            && p.speed.0 > 1.0
        {
            settled_at = i + SETTLE;
        }
        let lift = F(num(r, "cam_lift"));
        if let Under::Captured(l) = &mut under {
            (l.point, l.lift, l.last) = (rider.target.body_point, lift, was);
        }
        was = lift;
        let view = cam.update(F(num(r, "dt")), &rider, F(NEAR_CLIP), &curve, Switches { no_look_ahead: false, short_viewport: false }, &mut under);
        last = Some(rider);
        let eye = quad(r, "vp_eye");
        let off = dist(view.position, eye).min(dist(before, eye));
        let late = dist(before, eye) < dist(view.position, eye);
        before = view.position;
        out.lifted += (lift != F::ZERO) as usize;
        if i < settled_at {
            continue;
        }
        out.compared += 1;
        if off > out.eye.0 {
            out.eye = (off, i);
        }
        let gap = (cam.cam.ground_lift - lift).0.abs();
        out.gap = out.gap.max(gap);
        if !late {
            let look = view.look_at.sub(view.position);
            let a = angle(look, quad(r, "vp_dir"));
            if a > out.view.0 {
                out.view = (a, i);
            }
            out.reach = out.reach.min(dist(view.look_at, view.position));
        }
    }
    out
}

/// free07's scene and terrain file, where S149a drove all three cars.
fn free07(disc: &mut Disc) -> (Scene, Vec<u8>) {
    let track = disc.res("Tracks\\free07.res").unwrap();
    let scene = Scene::parse(&track.read_suffix(".scn").unwrap()).unwrap();
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let trn = track.read_suffix(leaf.rsplit('\\').next().unwrap()).unwrap();
    (scene, trn)
}

/// Each car's captured block, replayed through the port's race camera on its own file's first preset
/// with the original's lift under its look-at (and free07's ground under its eye), puts the eye where the original's viewport has it and points
/// it the same way: the car's camera is the bike's and the plane's camera on the car's presets.
#[test]
fn each_cars_camera_films_the_drive_as_the_original() {
    let Ok(mut disc) = Disc::open_configured() else {
        eprintln!("skipped: no disc");
        return;
    };
    let (scene, trn) = free07(&mut disc);
    let terrain = Terrain::parse(&trn).unwrap();
    let mut seen = 0;
    for (name, kind, _) in CARS {
        let file = chase::car_presets(kind);
        let Some(rows) = capture(name) else {
            eprintln!("skipped {name}: no context/capture/s246/{name}.csv");
            continue;
        };
        let presets = presets(&mut disc, file).unwrap_or_else(|| panic!("the disc has no {file}"));
        let ground = Ground::new(&terrain, &scene.terrain);
        let out = replay(&rows, presets, Under::Captured(Lifted { ground, point: Vf::new(0.0, 0.0, 0.0, 1.0), lift: F::ZERO, last: F::ZERO }));
        eprintln!("{name}: {out:?}");
        assert!(out.compared > 450, "{name}: only {} of {} rows compared", out.compared, rows.len());
        assert!(out.eye.0 < 0.01, "{name}: the eye is {} ft off at row {}", out.eye.0, out.eye.1);
        assert!(out.view.0 < 1.0e-4, "{name}: the view is turned {} rad off at row {}", out.view.0, out.view.1);
        seen += 1;
    }
    eprintln!("{seen} cars' cameras compared");
}

/// The same replay over free07's ground (where S149a drove all three), the ground the shell gives the
/// camera. The lift lands on the look-at alone, so the eye is still the original's; the view is off
/// by no more than the most the two grounds' lifts differ turns it at the nearest look-at (the lags
/// carry a gap a few ticks, so it is the run's most and not the row's). `world::Ground` is its own
/// blend of the terrain, not the game's query (`bike_camera.rs`): the lifts part by hundredths of a
/// foot.
#[test]
fn over_free07s_ground_only_the_lift_parts_the_view() {
    let Ok(mut disc) = Disc::open_configured() else {
        eprintln!("skipped: no disc");
        return;
    };
    let (scene, trn) = free07(&mut disc);
    let terrain = Terrain::parse(&trn).unwrap();
    let mut seen = 0;
    for (name, kind, _) in CARS {
        let file = chase::car_presets(kind);
        let Some(rows) = capture(name) else {
            eprintln!("skipped {name}: no context/capture/s246/{name}.csv");
            continue;
        };
        let presets = presets(&mut disc, file).unwrap_or_else(|| panic!("the disc has no {file}"));
        let out = replay(&rows, presets, Under::Free07(Ground::new(&terrain, &scene.terrain)));
        eprintln!("{name}: {out:?}");
        assert!(out.compared > 450, "{name}: only {} of {} rows compared", out.compared, rows.len());
        assert!(out.eye.0 < 0.01, "{name}: the eye is {} ft off at row {}", out.eye.0, out.eye.1);
        let bound = out.gap / out.reach + 1.0e-4;
        assert!(out.view.0 < bound, "{name}: the view is turned {} rad off at row {}, more than the lifts' gap turns it ({bound})", out.view.0, out.view.1);
        // Six tenths of the grounds' difference: the truck's 0.14 is 0.24 ft of ground, under what
        // `bike_camera.rs` allows the same two grounds (0.5).
        assert!(out.gap < 0.25, "{name}: the two grounds' lifts are {} ft apart", out.gap);
        seen += 1;
    }
    eprintln!("{seen} cars' cameras compared");
}

/// Full throttle for `frames` ticks, switching into `car` at `up` and back to the bike at `down`
/// (`car_switch.rs`'s tape).
fn car_tape(frames: usize, car: &str, up: usize, down: usize) -> String {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape the {car} on the gas\n");
    let report = format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17));
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    text += &format!("switch {up} {car}\nswitch {down} bike\n");
    text
}

/// The shell follows a driven car with the car's own camera: each car's track loads it on its file
/// and on "Race Cam 1", and from the switch into the car to the switch out the trace's follow camera
/// stands at that preset's distance (the baja's 15 ft, the buggy's 16, the truck's 25), where the
/// bike's camera stands at its own 13 before and after. On the original framing (`--camera-view
/// original`): a Camera page's framing stretches the cars' presets as it does the bike's.
#[test]
fn a_driven_car_is_followed_by_its_own_camera() {
    if let Err(e) = Disc::open_configured() {
        eprintln!("skipped: {e}");
        return;
    }
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("s246-car-camera");
    std::fs::create_dir_all(&dir).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (track, car, file, distance) in [("free09", "monster", "monstertruckcpresets.ini", 25.0), ("free07", "baja", "fourwheelcpresets.ini", 15.0), ("free11", "buggy", "buggycpresets.ini", 16.0)] {
        let tape = dir.join(format!("{car}.tape"));
        std::fs::write(&tape, car_tape(240, car, 30, 200)).unwrap();
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_openmx"))
            .current_dir(&root)
            .args(["--headless", "--track", track, "--frames", "260", "--steady-clock", "--trace-camera", "--camera-view", "original"])
            .args(["--play", tape.to_str().unwrap()])
            .output()
            .expect("the shell runs");
        let said = String::from_utf8_lossy(&out.stdout).to_string();
        assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
        let start = said.lines().find(|l| l.starts_with(&format!("{car}: its camera starts on"))).unwrap_or_else(|| panic!("{track}: no {car} camera line:\n{said}"));
        assert!(start.contains("Race Cam 1") && start.ends_with(file), "{track}: {start}");
        // Each follow line's tick and distance.
        let follows: Vec<(u32, f32)> = said
            .lines()
            .filter(|l| l.starts_with("camera 0 "))
            .filter_map(|l| {
                let w: Vec<&str> = l.split_whitespace().collect();
                let d = w.iter().position(|x| *x == "distance")?;
                Some((w[2].parse().ok()?, w[d + 1].parse().ok()?))
            })
            .collect();
        let driven: Vec<f32> = follows.iter().filter(|(t, _)| (40..195).contains(t)).map(|(_, d)| *d).collect();
        let ridden: Vec<f32> = follows.iter().filter(|(t, _)| *t < 25 || *t > 210).map(|(_, d)| *d).collect();
        assert!(driven.len() > 100 && ridden.len() > 20, "{track}: {} driven and {} ridden camera lines", driven.len(), ridden.len());
        assert!(driven.iter().all(|d| (d - distance).abs() < 0.5), "{track}: the {car}'s camera distances {driven:?}");
        assert!(ridden.iter().all(|d| (d - 13.0).abs() < 0.5), "{track}: the bike's camera distances {ridden:?}");
        eprintln!("{track}: the {car} followed at {} ft over {} ticks; {start}", driven[0], driven.len());
    }
}
