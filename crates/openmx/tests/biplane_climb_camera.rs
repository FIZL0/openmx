//! P275: the biplane's camera keeps the plane in the picture through a climb and a loop.
//!
//! The plane's camera is the original's race camera, on `biplanecpresets.ini` (V29). It looks along
//! the plane's heading flattened to the ground, so a plane pulling up climbs out of the top of the
//! picture, and the original's own camera loses it the same way. The shell turns the look after the
//! camera's frame ([`chase::keep_in_view`]). These tests fly the disc's plane through a climb and a
//! loop, and replay the original's own climb out of V29's capture. On every preset and every view
//! shape the shell draws, they hold the plane inside the picture.

#[allow(dead_code)]
#[path = "../src/vehicle.rs"]
mod vehicle;
#[allow(dead_code)]
#[path = "../src/chase.rs"]
mod chase;
#[allow(dead_code)]
#[path = "../src/aspect.rs"]
mod aspect;
#[allow(dead_code)]
#[path = "../src/split.rs"]
mod split;

use std::path::PathBuf;

use game_render::Disc;
use game_render::mat::v3;
use game_render::world::Camera;
use game_sim::camera::{CurveTable, Preset, Race, Ridden, Surface, Switches, Target, View};
use ps2_float::{F, Vf};

/// The shell's plane camera constants (`main.rs`): the reverse threshold, the speed reference, the
/// near clip.
const REVERSE_SPEED: f32 = 5.0;
const SPEED_REFERENCE: f32 = 117.332_8;
const NEAR_CLIP: f32 = 2.4;
const DT: f32 = 1.0 / 30.0;
/// The ticks of [`looped`]'s level second after the camera has settled from its snap.
const LEVEL: std::ops::Range<usize> = 20..30;

/// The view shapes the shell films the plane in: each layout's preset copies, and a view of the
/// shape it draws them into, on the television or the wide display.
const SHAPES: [(&str, usize, usize, bool); 7] = [
    ("", 640, 480, false),  // one player, 4:3
    ("h", 640, 240, true),  // stacked split, 4:3: a short view
    ("v", 320, 480, false), // side by side, 4:3
    ("q", 320, 240, false), // quarters, 4:3
    ("w", 1280, 720, false), // one player (and the stacked split's copies), 16:9
    ("x", 640, 720, false), // side by side, 16:9
    ("z", 640, 360, false), // quarters, 16:9
];

/// The ground under the camera's probes: none near. The flights are hundreds of feet up.
struct Below;

impl Surface for Below {
    fn on_surface(&mut self, point: Vf) -> Vf {
        Vf::new(point.x(), -1.0e6, point.z(), point.w())
    }
}

/// The plane's camera file and its machine archive, or `None` without a disc.
fn disc() -> Option<(Vec<u8>, game_sim::phy::Plane, game_sim::track::Rays)> {
    let mut disc = Disc::open_configured().ok()?;
    let names = disc.entries().iter().map(|e| e.name.clone()).collect::<Vec<_>>();
    let kit = names.iter().find(|n| n.to_ascii_lowercase().ends_with("free06_2.res"))?.clone();
    let presets = disc.res(&kit).ok()?.read_suffix("biplanecpresets.ini").ok()?;
    let machines = disc.res("machine.res").ok()?;
    let plane = game_sim::phy::Plane::read_bytes(&machines.read_suffix(game_sim::phy::BIPLANE_PHY).ok()?);
    let rays = game_sim::track::Rays::parse(&machines.read_suffix(game_sim::flight_search::BIPLANE_RAYS).ok()?).ok()?;
    Some((presets, plane, rays))
}

/// The disc's plane, 400 ft over flat ground at 120 ft/s, flown with full throttle and the stick held
/// back: it climbs, goes past vertical, over the top inverted and round again. Its camera block,
/// tick by tick.
fn looped(plane: &game_sim::phy::Plane, rays: &game_sim::track::Rays) -> Vec<Ridden> {
    use game_sim::biplane::Pad;
    use game_sim::flight::Flight;
    use game_sim::flight_search::Rays;
    let tables = game_sim::ride::Tables::build();
    let mut f = Flight::new(plane).with_rays(Rays::of(rays));
    let mut ground = game_sim::world::Flat::default();
    f.attitude.flown = true;
    f.place(Vf::new(0.0, 400.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), &tables.acos);
    f.body.velocity = Vf::new(0.0, 0.0, 120.0, 1.0);
    let pad = Pad { forward: F::ONE, elevator: F(-1.0), ..Pad::default() };
    let mut blocks = Vec::new();
    // Level for a second, so the camera settles behind it, then the stick back.
    for tick in 0..330 {
        f.frame(F(DT), if tick < 30 { Pad { forward: F::ONE, ..Pad::default() } } else { pad }, &mut ground, &tables);
        blocks.push(chase::flown(&f));
    }
    blocks
}

/// Where `point` is drawn in `view` for a view `w` by `h`: its x and y, -1..1 across the picture,
/// and whether it is in front of the eye.
fn drawn(view: &View, point: Vf, w: usize, h: usize) -> (f32, f32, bool) {
    let camera = Camera {
        eye: v3(view.position.x(), view.position.y(), view.position.z()),
        at: v3(view.look_at.x(), view.look_at.y(), view.look_at.z()),
        fov: aspect::fov(view.fov.0, w, h),
        near: NEAR_CLIP,
        far: 4000.0,
    };
    let m = camera.view_projection(w, h).0;
    let p = [point.x(), point.y(), point.z(), 1.0];
    let clip: Vec<f32> = (0..4).map(|c| (0..4).map(|r| p[r] * m[r][c]).sum()).collect();
    (clip[0] / clip[3], clip[1] / clip[3], clip[3] > 0.0)
}

/// The furthest from the middle the plane is drawn, over a flight, through a camera on preset
/// `name` of the copies `suffix` reads: as a share of the half picture (over 1 is off it), and the
/// tick, and the ticks of [`LEVEL`] the turn changed. With `keep` the shell's turn is applied.
fn flown_through(bytes: &[u8], (suffix, w, h, short): (&str, usize, usize, bool), name: &str, blocks: &[Ridden], keep: bool) -> (f32, usize, usize) {
    let curve = CurveTable::build();
    let mut cam = Race::new(presets(bytes, suffix), F(REVERSE_SPEED));
    let Some(at) = Preset::find(&cam.cam.presets, name) else { return (f32::INFINITY, 0, 0) };
    cam.cam.set_preset(at);
    cam.cam.snap = true;
    cam.cam.speed_reference = F(SPEED_REFERENCE);
    let (mut worst, mut level) = ((0.0f32, 0), 0);
    for (tick, rider) in blocks.iter().enumerate() {
        let switches = Switches { no_look_ahead: false, short_viewport: short };
        let mut view = cam.update(F(DT), rider, F(NEAR_CLIP), &curve, switches, &mut Below);
        if keep {
            let fov = aspect::fov(view.fov.0, w, h);
            let before = view;
            chase::keep_in_view(&mut view, rider.target.body_point, fov, w as f32 / h as f32);
            level += usize::from(LEVEL.contains(&tick) && view != before);
        }
        let (x, y, ahead) = drawn(&view, rider.target.body_point, w, h);
        let off = if ahead { x.abs().max(y.abs()) } else { f32::INFINITY };
        if off > worst.0 {
            worst = (off, tick);
        }
    }
    (worst.0, worst.1, level)
}

/// The copies of the plane's presets a layout reads, or the full screen's where the file has none,
/// as the shell's loader falls back (`main.rs`'s `layout_presets`).
fn presets(bytes: &[u8], suffix: &str) -> Vec<Preset> {
    let own = Preset::read_bytes(bytes, suffix);
    if own.iter().any(|p| !p.name.is_empty()) { own } else { Preset::read_bytes(bytes, "") }
}

/// Every preset name in the plane's file.
fn names(bytes: &[u8]) -> Vec<String> {
    Preset::read_bytes(bytes, "").into_iter().map(|p| p.name).filter(|n| !n.is_empty()).collect()
}

/// The disc's plane pulled up into a climb and a loop: the original's camera loses it off the top of
/// the picture on its first preset (behind the eye, over the top). With the shell's turn every
/// preset on every view shape keeps it in, never further out than [`chase::IN_VIEW`] of the half
/// picture, and in level flight each preset's own look is left as the original's.
#[test]
fn the_plane_stays_in_the_picture_through_a_loop() {
    let Some((bytes, plane, rays)) = disc() else {
        eprintln!("skipped: no disc configured");
        return;
    };
    let blocks = looped(&plane, &rays);
    // It was a loop: past vertical, then upside down over the top.
    let steepest = blocks.iter().map(|b| b.target.forward.y()).fold(f32::MIN, f32::max);
    let inverted = blocks.iter().map(|b| b.target.up.y()).fold(f32::MAX, f32::min);
    assert!(steepest > 0.95 && inverted < -0.9, "no loop: forward.y to {steepest}, up.y to {inverted}");

    let (lost, tick, _) = flown_through(&bytes, SHAPES[0], "Flight Cam 1", &blocks, false);
    assert!(lost > 1.0, "the original's camera kept the plane in (at most {lost} of the half picture, tick {tick})");
    eprintln!("the original's Flight Cam 1, 4:3: the plane {lost:.2} of the half picture out at tick {tick}");

    let (mut filmed, mut reframed) = (0, Vec::new());
    for shape in SHAPES {
        for name in names(&bytes) {
            let (off, tick, level) = flown_through(&bytes, shape, &name, &blocks, true);
            eprintln!("{name:?} {:?} {}x{}: at most {off:.3} of the half picture out, at tick {tick}", shape.0, shape.1, shape.2);
            assert!(off <= chase::IN_VIEW + 1.0e-3, "{name:?} on {:?}: the plane is {off} of the half picture out at tick {tick}", shape.0);
            if level > 0 {
                reframed.push(format!("{name} {}", shape.0));
            }
            filmed += 1;
        }
    }
    // The one preset whose own look already has the plane past the edge of the box in level flight:
    // 1P Stunt, at 0.99 of the stacked strip's half height and 0.73 of a 16:9 picture's.
    assert_eq!(reframed, ["1P Stunt h", "1P Stunt w", "1P Stunt z"], "level flight was turned");
    assert!(filmed >= 5 * SHAPES.len(), "only {filmed} presets filmed");
}

fn context() -> PathBuf {
    match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context"),
    }
}

/// The original's own flight (V29's capture of `biplane.p2m2`: the take-off, the climb, past
/// vertical, the dive): its camera block, row by row, and the plane's centre as drawn in its own
/// viewport. `None` without the capture.
fn captured() -> Option<Vec<(Ridden, Vf, Vf)>> {
    let text = std::fs::read_to_string(context().join("capture/v29/biplane-camera.csv")).ok()?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next()?.split(',').collect();
    let col = |k: &str| head.iter().position(|h| *h == k).unwrap();
    let quad = |r: &[f32], n: &str| Vf::new(r[col(&format!("{n}.x"))], r[col(&format!("{n}.y"))], r[col(&format!("{n}.z"))], r[col(&format!("{n}.w"))]);
    Some(
        lines
            .map(|l| {
                let r: Vec<f32> = l.split(',').map(|v| v.parse().unwrap_or(0.0)).collect();
                let block = Ridden {
                    target: Target {
                        body_point: quad(&r, "tgt_point"),
                        forward: quad(&r, "tgt_fwd"),
                        up: quad(&r, "tgt_up"),
                        vel: quad(&r, "tgt_vel"),
                        lean: F(quad(&r, "tgt_tail").y()),
                    },
                    airborne: r[col("tgt_flag_speed.x")] != 0.0,
                    speed: F(r[col("tgt_flag_speed.y")]),
                };
                (block, quad(&r, "vp_eye"), quad(&r, "vp_dir"))
            })
            .collect(),
    )
}

/// The original's climb, as its own viewport drew it: the plane goes out of the top of its 4:3
/// picture. The same view with the shell's turn keeps it inside [`chase::IN_VIEW`], and every row
/// the original already had it inside is left as it was.
#[test]
fn the_originals_climb_loses_the_plane_and_the_shell_keeps_it() {
    let Some(rows) = captured() else {
        eprintln!("skipped: no context/capture/v29/biplane-camera.csv");
        return;
    };
    let (w, h) = (640, 480);
    let (mut lost, mut kept, mut untouched) = (0, 0.0f32, 0);
    for (block, eye, dir) in &rows {
        let point = block.target.body_point;
        let look = Vf::new(eye.x() + dir.x() * 20.0, eye.y() + dir.y() * 20.0, eye.z() + dir.z() * 20.0, 1.0);
        let original = View { position: *eye, look_at: look, fov: F(85.0) };
        let (x, y, ahead) = drawn(&original, point, w, h);
        if !ahead || x.abs().max(y.abs()) > 1.0 {
            lost += 1;
        }
        let mut view = original;
        chase::keep_in_view(&mut view, point, aspect::fov(85.0, w, h), w as f32 / h as f32);
        let (x2, y2, _) = drawn(&view, point, w, h);
        kept = kept.max(x2.abs().max(y2.abs()));
        if ahead && x.abs() <= chase::IN_VIEW && y.abs() <= chase::IN_VIEW && view.look_at == original.look_at {
            untouched += 1;
        }
        if ahead && x.abs() <= chase::IN_VIEW * 0.99 && y.abs() <= chase::IN_VIEW * 0.99 {
            assert_eq!(view.look_at, original.look_at, "a plane already in view was turned to");
        }
    }
    eprintln!("the original lost the plane on {lost} of {} rows; with the turn, at most {kept:.3} out; {untouched} left alone", rows.len());
    assert!(lost > 20, "the original's climb kept the plane in: lost on {lost} rows");
    assert!(kept <= chase::IN_VIEW + 1.0e-3, "the turned view has the plane {kept} of the half picture out");
}
