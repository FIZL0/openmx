//! V26 — the original's HUD over each view: the speedometer reads the ride's speed, and the radar's
//! marks sit where the riders are, for one player and for four.
//!
//! The riders are real: [`game_sim::Sim`]s on `game_sim::stock::BIKE` (with `tests/ride.rs`'s
//! steering and drag patches, so the stick turns it and the throttle gets it somewhere), ridden
//! through pad reports on a flat floor. What the overlay is held to is worked out here a second way:
//! the speed off the distance the bike actually covered, and each mark off dot products with the
//! viewing rider's own forward and right — `src/overlay.rs` takes angles instead — so the two agree
//! only if the marks are where the riders are. Which way is the rider's right is not assumed either:
//! a bike steered right on the pad has to come out on the radar's right.
//!
//! The pure parts need no disc. The drawing tests load the disc's art and skip without it.

#[path = "../src/menu.rs"]
#[allow(dead_code)]
mod menu;
#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;
#[path = "../src/roster.rs"]
#[allow(dead_code)]
mod roster;
#[path = "../src/chase.rs"]
#[allow(dead_code)]
mod chase;
#[path = "../src/join.rs"]
#[allow(dead_code)]
mod join;
#[path = "../src/split.rs"]
#[allow(dead_code)]
mod split;
#[path = "../src/overlay.rs"]
#[allow(dead_code)]
mod overlay;

use game_render::{Disc, Frame};
use game_sim::camera::Ridden;
use game_sim::collide::{Bundle, Record};
use game_sim::input::{AXIS_CENTRE, CONTROL_MAP, Control, PADS, Report};
use game_sim::phy::Phy;
use game_sim::rider::{Rider, Tables};
use game_sim::trick::Set;
use game_sim::world::World;
use game_sim::{Sim, stock};
use ps2_float::{F, Vf};
use std::f32::consts::{PI, TAU};
use std::sync::Arc;

const DT: F = F(1.0 / 60.0);

/// A radar span short enough that riders a few seconds apart land inside it rather than on its ring:
/// a terrain sample every two feet, 384 feet across.
const SPACING: f32 = 2.0;

/// `tests/ride.rs`'s `RIDE_LIMITS`: without a `[Wrecks]` section every limit is zero and the bike
/// wrecks where it stands.
const RIDE_LIMITS: &str = "
[Wrecks]
MaxWreckTime = 2
CannonballWreckTime = 5
FrontWreckSpeedLimit = 60
BackWreckSpeedLimit = 60
SideWreckSpeedLimit = 40
RacingDownWreckSpeedLimit = 40
FreestyleDownWreckSpeedLimit = 40
GroundPitchDownWreckAngle = 60
GroundPitchUpWreckAngle = 60
AirbornePitchDownWreckAngle = 120
AirbornePitchUpWreckAngle = 120
AirborneRollWreckAngle = 100
NoHanderWreckSpeedLimit = 400
NoHanderMaxRollAngle = 45
JoustWreckHeight = 2
CrossupWreck = 1
CrossupWreckTime = 0.3
";

/// `tests/ride.rs`'s `ridden_bike`: the stock bike with its steering connected, its drag let off and
/// a rider's wreck limits.
fn ridden_bike() -> String {
    let mut out = stock::BIKE.to_string();
    for (key, with) in [
        (
            "XAxisRateLimit = 6\n",
            "XAxisRateLimit = 6\nSteerScale1 = 1.0\nSteerScale2 = 0.05\nSteerSpeed1 = 5\nSteerSpeed2 = 50\n",
        ),
        ("Drag = 0.5", "Drag = 0.037"),
    ] {
        assert!(out.contains(key), "game_sim::stock::BIKE no longer has `{}`", key.trim_end());
        out = out.replace(key, with);
    }
    format!("{out}{RIDE_LIMITS}")
}

struct Floor;

impl World for Floor {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        (Vf::new(at.x(), 0.0, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0))
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

/// A rider put down at `at` facing `facing`, stopped.
fn rider(tables: &Arc<Tables>, at: [f32; 2], facing: [f32; 2]) -> Sim {
    let phy = Phy::read_bytes(ridden_bike().as_bytes(), &tables.ride.curve);
    let geom = Phy::read_bytes(stock::BIKE.as_bytes(), &tables.ride.curve);
    let spawn = Vf::new(at[0], stock::SPAWN.y(), at[1], 1.0);
    let facing = Vf::new(facing[0], 0.0, facing[1], 0.0);
    let rider = Rider::new(&phy, stock::mounts(&geom), spawn, facing, Set::default());
    Sim::with_rider(0, tables.clone(), phy, rider)
}

/// The report a pad sends with the throttle (Cross) full on and the left stick at `stick` (-1..1).
fn hands(gas: bool, stick: f32) -> Report {
    let mut report = Report::idle();
    let buttons = if gas { Control::Cross.mask() } else { 0 };
    let low = !buttons;
    report.0[2] = (low >> 8) as u8;
    report.0[3] = low as u8;
    for binding in CONTROL_MAP {
        if binding.is_analogue() && buttons & binding.mask != 0 {
            report.0[binding.pressure as usize] = 0xff;
        }
    }
    let x = (AXIS_CENTRE as i32 + (stick * 128.0) as i32).clamp(0, 255) as u8;
    report.set_axis(2, x);
    report
}

/// Ride `sim` for `frames` with these hands, returning the distance it covered each frame — up and
/// down as well, because the chassis' speed is its velocity's whole length.
fn ride(sim: &mut Sim, frames: usize, gas: bool, stick: f32) -> Vec<f32> {
    let mut steps = Vec::with_capacity(frames);
    for _ in 0..frames {
        let before = sim.rider.ride.position();
        let reports: [Report; PADS] = [hands(gas, stick), Report::missing()];
        let controls = sim.poll(DT, &reports);
        sim.step(DT, &controls, &mut Floor);
        let after = sim.rider.ride.position();
        let d = [after.x() - before.x(), after.y() - before.y(), after.z() - before.z()];
        steps.push((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt());
    }
    steps
}

fn ground(v: Vf) -> [f32; 2] {
    [v.x(), v.z()]
}

/// Where `other` should be on `me`'s radar, worked out from `me`'s own forward and right rather than
/// from angles: ahead is up, the rider's right is right, `RadarRadius` over the span a foot.
fn expected_mark(me: &Ridden, other: &Ridden, span: f32) -> (f32, f32) {
    let f = ground(me.target.forward);
    let len = (f[0] * f[0] + f[1] * f[1]).sqrt();
    let ahead = [f[0] / len, f[1] / len];
    let right = [ahead[1], -ahead[0]];
    let (a, b) = (ground(me.target.body_point), ground(other.target.body_point));
    let mut rel = [b[0] - a[0], b[1] - a[1]];
    let d = (rel[0] * rel[0] + rel[1] * rel[1]).sqrt();
    if d > span / 2.0 {
        rel = [rel[0] * span / 2.0 / d, rel[1] * span / 2.0 / d];
    }
    let k = overlay::RADAR_RADIUS / span;
    let (r, u) = (rel[0] * right[0] + rel[1] * right[1], rel[0] * ahead[0] + rel[1] * ahead[1]);
    (overlay::RADAR_AT.0 + r * k, overlay::RADAR_AT.1 - u * k)
}

/// Four riders, set off from four places four ways and ridden apart: one straight, one hard right,
/// one hard left, one straight and slower off a later start.
fn four_riders(tables: &Arc<Tables>) -> Vec<Sim> {
    let mut sims = vec![
        rider(tables, [0.0, 0.0], [0.0, 1.0]),
        rider(tables, [40.0, 10.0], [1.0, 0.0]),
        rider(tables, [-30.0, 60.0], [0.0, -1.0]),
        rider(tables, [10.0, -50.0], [-0.6, 0.8]),
    ];
    for (sim, (frames, stick)) in sims.iter_mut().zip([(150, 0.0), (150, 1.0), (150, -1.0), (60, 0.0)]) {
        ride(sim, frames, true, stick);
    }
    sims
}

fn ridden(sims: &[Sim]) -> Vec<Ridden> {
    sims.iter().map(|s| chase::bike(&s.rider.ride)).collect()
}

// ---- the speedometer ---------------------------------------------------------------------------

/// What the dial says is what the ride is doing: on the way up from a standstill, the number and the
/// needle follow the chassis' speed, and that speed is the distance the bike covers a second.
#[test]
fn the_speedometer_reads_the_rides_speed() {
    let tables = Arc::new(Tables::build());
    let mut sim = rider(&tables, [0.0, 0.0], [0.0, 1.0]);
    let at_rest = chase::bike(&sim.rider.ride);
    assert_eq!(overlay::speed_text(at_rest.speed.0), "0");
    assert_eq!(overlay::needle_degrees(at_rest.speed.0), overlay::NEEDLE_REST);

    let mut checked = 0;
    let mut fastest = 0.0f32;
    for batch in 0..40 {
        let steps = ride(&mut sim, 15, true, 0.0);
        let seen = chase::bike(&sim.rider.ride);
        let speed = seen.speed.0;
        // The speed the ride hands the HUD is the one the bike is going: the last frame's distance,
        // once it has come down off its spawn onto the floor (the first three quarters of a second).
        let covered = steps[steps.len() - 1] / DT.0;
        if batch >= 3 && speed > 5.0 {
            assert!(
                (covered - speed).abs() < 0.05 * speed,
                "the HUD's speed {speed} ft/s against the {covered} ft/s the bike covered"
            );
            checked += 1;
        }
        // And what is drawn for it: whole miles an hour, the needle on the dial's marks.
        let mph = speed * 0.68182;
        assert_eq!(overlay::speed_text(speed), format!("{}", mph.round() as i32), "at {speed} ft/s");
        let needle = overlay::needle_degrees(speed);
        assert!((needle - (132.0 - 2.64 * mph.min(105.0))).abs() < 1e-3);
        fastest = fastest.max(mph);
    }
    assert!(checked > 20 && fastest > 30.0, "the ride never got going: {fastest} mph");

    // The dial's own marks: 0 at the lower left, 50 straight up, 100 at the lower right, and the
    // needle stopping just past it.
    let at = |mph: f32| overlay::needle_degrees(mph / overlay::MPH);
    assert!((at(0.0) - 132.0).abs() < 1e-3);
    assert!(at(50.0).abs() < 1e-3);
    assert!((at(100.0) + 132.0).abs() < 1e-3);
    assert!((at(150.0) - (132.0 - 2.64 * 105.0)).abs() < 1e-3);
    assert_eq!(overlay::speed_text(-3.0), "0");
    assert_eq!(overlay::odometer_text(0.0), "00000.0");
    assert_eq!(overlay::odometer_text(5280.0 * 12.34), "00012.3");
}

// ---- the radar ---------------------------------------------------------------------------------

/// One player: no marks but the rider's own arrow in the middle, and the rose turned so that the way
/// the bike faces is up.
#[test]
fn one_rider_is_the_arrow_in_the_middle() {
    let tables = Arc::new(Tables::build());
    let mut sim = rider(&tables, [5.0, 5.0], [0.0, 1.0]);
    ride(&mut sim, 120, true, 1.0);
    let me = chase::bike(&sim.rider.ride);
    let f = ground(me.target.forward);
    // Steered right from facing +z, so it now faces partly along +x: north turns anticlockwise on
    // the screen (the rose turns by less than a whole turn), by exactly the heading.
    let heading = f32::atan2(f[0], f[1]);
    assert!(heading > 0.3, "the bike did not turn: heading {heading}");
    let turn = overlay::radar_turn(f);
    assert!((turn - (std::f32::consts::TAU - heading)).abs() < 1e-4, "turned {turn} for heading {heading}");
    // Itself, seen from where it is: the middle.
    let own = ground(me.target.body_point);
    let at = overlay::radar_mark(own, f, own, overlay::radar_span(SPACING));
    assert_eq!(at, overlay::RADAR_AT);
}

/// The radar's right is the rider's right: a bike steered right on the pad from beside a rider facing
/// the same way comes out on the right of that rider's radar, and one steered left on its left.
#[test]
fn a_rider_steered_right_shows_on_the_right() {
    let tables = Arc::new(Tables::build());
    let watcher = rider(&tables, [0.0, 0.0], [0.0, 1.0]);
    let me = chase::bike(&watcher.rider.ride);
    let span = overlay::radar_span(SPACING);
    for (stick, right) in [(1.0, true), (-1.0, false)] {
        let mut other = rider(&tables, [0.0, 0.0], [0.0, 1.0]);
        ride(&mut other, 150, true, stick);
        let them = chase::bike(&other.rider.ride);
        let (x, y) = overlay::radar_mark(ground(me.target.body_point), ground(me.target.forward), ground(them.target.body_point), span);
        assert!(y < overlay::RADAR_AT.1, "a bike ridden ahead is not up the radar: {y}");
        assert_eq!(x > overlay::RADAR_AT.0, right, "steered {stick}: the mark is at x {x}");
    }
}

/// Four players: every view's marks sit where the other three riders are, off that view's rider —
/// and a rider out past the radar's reach sits on its ring, the right way off.
#[test]
fn four_riders_are_marked_where_they_are() {
    let tables = Arc::new(Tables::build());
    let sims = four_riders(&tables);
    let riders = ridden(&sims);
    let span = overlay::radar_span(SPACING);
    let mut inside = 0;
    for (v, me) in riders.iter().enumerate() {
        for (i, other) in riders.iter().enumerate() {
            if i == v {
                continue;
            }
            let at = overlay::radar_mark(
                ground(me.target.body_point),
                ground(me.target.forward),
                ground(other.target.body_point),
                span,
            );
            let want = expected_mark(me, other, span);
            assert!(
                (at.0 - want.0).abs() < 0.01 && (at.1 - want.1).abs() < 0.01,
                "view {v}: rider {i} drawn at {at:?}, is at {want:?}"
            );
            let (a, b) = (ground(me.target.body_point), ground(other.target.body_point));
            if ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt() < span / 2.0 {
                inside += 1;
            }
        }
    }
    assert!(inside >= 6, "only {inside} of the twelve marks were inside the radar");

    // Far off: on the ring, along the bearing.
    let far = overlay::radar_mark([0.0, 0.0], [0.0, 1.0], [10_000.0, 0.0], span);
    assert!((far.0 - (overlay::RADAR_AT.0 + overlay::RADAR_RADIUS / 2.0)).abs() < 1e-3);
    assert!((far.1 - overlay::RADAR_AT.1).abs() < 1e-3);
}

// ---- drawn, off the disc -------------------------------------------------------------------------

fn art() -> Option<overlay::Art> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipped, no disc: {e}");
            return None;
        }
    };
    Some(overlay::Art::load(&mut disc).expect("fstyleui.res's HUD art"))
}

/// The bike's gauges, nothing preloaded and no score, `feet` along.
fn bike(feet: f32) -> overlay::Gauges {
    overlay::Gauges { feet, bike: true, ..Default::default() }
}

/// What the views draw is what the ride says: one player's readout, or the dial and needle when
/// asked for, at its speed, four players' readouts at theirs, and each view's marks, in the view's pixels, where the riders are —
/// drawn there in the human riders' yellow.
#[test]
fn the_views_draw_the_riders() {
    let Some(art) = art() else { return };
    let tables = Arc::new(Tables::build());
    let span = overlay::radar_span(SPACING);

    // One player, a full 640x448 screen.
    let mut one = rider(&tables, [0.0, 0.0], [0.0, 1.0]);
    ride(&mut one, 200, true, 0.0);
    let solo = ridden(std::slice::from_ref(&one));
    let mut frame = Frame::new(640, 448, [0, 0, 0]);
    let analog = overlay::Gauges { speedometer: menu::Speedometer::Analog, ..bike(1000.0) };
    let drawn = art.draw(&mut frame, (640, 448), &solo, 0, span, &analog);
    assert_eq!(drawn.needle, Some(overlay::needle_degrees(solo[0].speed.0)));
    assert_eq!(drawn.speed, None, "the dial, not the readout");
    assert_eq!(drawn.odometer.as_deref(), Some("00000.2"));
    assert!(drawn.marks.is_empty());
    assert_eq!(drawn.player, overlay::RADAR_AT);
    assert!(solo[0].speed.0 > 20.0);
    assert_eq!(drawn.score, "0");
    assert_eq!(drawn.preload, Some(0.0));
    let mut frame = Frame::new(640, 448, [0, 0, 0]);
    let drawn = art.draw(&mut frame, (640, 448), &solo, 0, span, &bike(1000.0));
    assert_eq!(drawn.speed, Some(overlay::speed_text(solo[0].speed.0)), "the readout by default");
    assert_eq!((drawn.needle, drawn.odometer), (None, None), "the digital layout has no dial or odometer");
    assert_eq!(drawn.place, overlay::Place::Full);

    // Four players, four 320x224 views of a 640x448 window: the parts at full size, the radar from
    // each view's bottom-left.
    let sims = four_riders(&tables);
    let riders = ridden(&sims);
    let mut marked = 0;
    for v in 0..4 {
        let mut view = Frame::new(320, 224, [0, 0, 0]);
        // Asking for the dial does not get it in a split: the original forces the readout there.
        let analog = overlay::Gauges { speedometer: menu::Speedometer::Analog, ..bike(0.0) };
        let drawn = art.draw(&mut view, (640, 448), &riders, v, span, &analog);
        assert_eq!(drawn.scale, 1.0);
        assert_eq!(drawn.speed, Some(overlay::speed_text(riders[v].speed.0)), "view {v}");
        assert_eq!(drawn.needle, None);
        assert_eq!(drawn.preload, Some(0.0), "view {v} has its preload meter");
        assert_eq!(drawn.place, if v < 2 { overlay::Place::Top } else { overlay::Place::Bottom });
        let up = 448.0 - 224.0;
        assert_eq!(drawn.player, (overlay::RADAR_AT.0, overlay::RADAR_AT.1 - up));
        assert_eq!(drawn.marks.iter().map(|m| m.0).collect::<Vec<_>>(), (0..4).filter(|&i| i != v).collect::<Vec<_>>());
        for &(i, (x, y)) in &drawn.marks {
            let want = expected_mark(&riders[v], &riders[i], span);
            assert!(
                (x - want.0).abs() < 0.01 && (y - (want.1 - up)).abs() < 0.01,
                "view {v}: rider {i}'s mark at {x},{y}, wanted {want:?}"
            );
            // Clear of the arrow, the mark is there in the frame and it is yellow (P92: the original's
            // colour words are 0xAARRGGBB, which V26 read the other way round, as cyan).
            if (x - drawn.player.0).abs() > 12.0 || (y - drawn.player.1).abs() > 12.0 {
                let at = 3 * (y as usize * view.width + x as usize);
                let c = &view.color[at..at + 3];
                assert!(c[0] > 150 && c[1] > 150 && c[2] < 90, "view {v}: rider {i}'s mark is {c:?}");
                marked += 1;
            }
        }
    }
    assert!(marked >= 4, "only {marked} marks were clear of the arrows to look at");
}

/// How far apart two RGB frames are: the mean difference per channel, and the share of pixels with a
/// channel more than 48 off.
fn apart(a: &[u8], b: &[u8]) -> (f64, f64) {
    let sum: u64 = a.iter().zip(b).map(|(&x, &y)| x.abs_diff(y) as u64).sum();
    let off = a.chunks(3).zip(b.chunks(3)).filter(|(x, y)| x.iter().zip(y.iter()).any(|(&p, &q)| p.abs_diff(q) > 48)).count();
    (sum as f64 / a.len() as f64, off as f64 / (a.len() / 3) as f64)
}

/// V30: the GPU draws the HUD as sprites the way the CPU draws it — one player's readout or dial,
/// needle, score, odometer and preload, and four views' readouts and radars, each in its own corner of the window.
/// The shapes' edges are smoothed by distance rather than by four samples, so a few edge pixels may
/// part; the rest is the same picture.
#[test]
fn the_gpu_draws_the_hud_as_the_cpu_does() {
    let Some(mut art) = art() else { return };
    // Every kind of the radar's other icons in every view (P92), the markers' gates turned.
    art.radar = vec![every_icon([0.0, 0.0]); 4];
    let mut gpu = match game_render::gpu::Renderer::headless() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    gpu.set_images(&art.images());
    let tables = Arc::new(Tables::build());
    let span = overlay::radar_span(SPACING);
    let mut one = rider(&tables, [0.0, 0.0], [0.0, 1.0]);
    ride(&mut one, 200, true, 0.0);
    let solo = ridden(std::slice::from_ref(&one));
    let four = ridden(&four_riders(&tables));
    let analog = overlay::Gauges { feet: 1234.0, bike: true, score: 4321, preload: Some(0.6), modified: false, speedometer: menu::Speedometer::Analog };
    let digital = overlay::Gauges { speedometer: menu::Speedometer::Digital, ..analog };
    let (w, h) = (1280usize, 720usize);
    let texture = gpu.offscreen(w as u32, h as u32);
    let view = texture.create_view(&Default::default());
    let target = game_render::gpu::Target { view: &view, size: (w as u32, h as u32), frame: [0, 0, w as u32, h as u32], scale: 1 };
    let quarters = (0..4).map(|v| ((v % 2) * w / 2, (v / 2) * h / 2, w / 2, h / 2)).collect();
    let halves = (0..2).map(|v| (v * w / 2, 0, w / 2, h)).collect();
    for (riders, views, gauges) in
        [(&solo[..], vec![(0, 0, w, h)], analog), (&solo[..], vec![(0, 0, w, h)], digital), (&four[..], quarters, digital), (&four[..2], halves, digital)]
    {
        let mut frame = Frame::new(w, h, [0, 0, 0]);
        let mut sprites = Vec::new();
        for (v, &(x, y, vw, vh)) in views.iter().enumerate() {
            let mut part = Frame::new(vw, vh, [0, 0, 0]);
            art.draw(&mut part, (w, h), riders, v, span, &gauges);
            for row in 0..vh {
                frame.color[3 * ((y + row) * w + x)..][..3 * vw].copy_from_slice(&part.color[3 * row * vw..][..3 * vw]);
            }
            art.sprites(&mut sprites, (x, y, vw, vh), (w, h), riders, v, span, &gauges);
        }
        gpu.draw_sprites(&target, &sprites);
        let got = gpu.read(&texture);
        let (mean, off) = apart(&frame.color, &got);
        eprintln!("{} view(s): {} sprites, {mean:.3} apart on average, {:.4}% of pixels off", views.len(), sprites.len(), off * 100.0);
        assert!(sprites.len() > 10);
        assert!(mean < 0.5 && off < 0.001, "{} view(s): {mean:.3} apart on average, {:.3}% of pixels off", views.len(), off * 100.0);
    }
}

/// Any shape of window (V24): the parts keep their size and go to the corners.
#[test]
fn a_wide_window_puts_the_parts_in_its_corners() {
    let Some(art) = art() else { return };
    let tables = Arc::new(Tables::build());
    let riders = ridden(&[rider(&tables, [0.0, 0.0], [0.0, 1.0])]);
    for (w, h) in [(1280, 448), (640, 896), (1920, 1080)] {
        let mut frame = Frame::new(w, h, [0, 0, 0]);
        let analog = overlay::Gauges { speedometer: menu::Speedometer::Analog, ..bike(0.0) };
        let drawn = art.draw(&mut frame, (w, h), &riders, 0, 384.0, &analog);
        let s = (w as f32 / 640.0).min(h as f32 / 448.0);
        assert_eq!(drawn.scale, s);
        let want = (overlay::RADAR_AT.0 * s, h as f32 - (448.0 - overlay::RADAR_AT.1) * s);
        assert!((drawn.player.0 - want.0).abs() < 1e-3 && (drawn.player.1 - want.1).abs() < 1e-3, "{w}x{h}");
        // The dial's face is drawn at the bottom right: something lit there, nothing on the far side.
        let lit = |x0: usize, x1: usize| {
            (h * 3 / 4..h).any(|y| (x0..x1).any(|x| frame.color[3 * (y * w + x)..3 * (y * w + x) + 3].iter().any(|&c| c > 0)))
        };
        assert!(lit(w - (160.0 * s) as usize, w), "{w}x{h}: no dial in the bottom right");
        assert!(!lit(w / 2 - 20, w / 2 + 20), "{w}x{h}: something drawn mid-screen");
    }
}

/// The score bar is drawn top left with the score in it; the bike's dial has its odometer and
/// preload meter, the plane's dial is alone (V28) — the original's biplane rides show neither.
#[test]
fn the_score_and_the_preload_meter_are_drawn() {
    let Some(art) = art() else { return };
    let tables = Arc::new(Tables::build());
    let riders = ridden(&[rider(&tables, [0.0, 0.0], [0.0, 1.0])]);
    let gauges = overlay::Gauges { feet: 5280.0, bike: true, preload: Some(0.5), score: 1234, modified: false, speedometer: menu::Speedometer::Analog };
    let mut frame = Frame::new(640, 448, [255, 255, 255]);
    let drawn = art.draw(&mut frame, (640, 448), &riders, 0, 384.0, &gauges);
    assert_eq!(drawn.score, "1234");
    assert_eq!(drawn.preload, Some(0.5));
    assert!(drawn.odometer.is_some());
    // The bar darkens a white frame inside its box, and nowhere just below it.
    let dark = |x0: usize, x1: usize, y0: usize, y1: usize| {
        (y0..y1).any(|y| (x0..x1).any(|x| frame.color[3 * (y * 640 + x)..3 * (y * 640 + x) + 3].iter().any(|&c| c < 200)))
    };
    assert!(dark(20, 180, 28, 43), "no score bar");
    assert!(!dark(20, 180, 50, 60), "the score bar runs too low");
    // The preload meter, right of the dial.
    assert!(dark(600, 618, 320, 395), "no preload meter");

    let plane = overlay::Gauges { bike: false, ..gauges };
    let mut frame = Frame::new(640, 448, [255, 255, 255]);
    let drawn = art.draw(&mut frame, (640, 448), &riders, 0, 384.0, &plane);
    assert_eq!(drawn.odometer, None);
    assert_eq!(drawn.preload, None);
    assert_eq!(drawn.score, "1234");
    assert!(!(320..395).any(|y| (610..618).any(|x| frame.color[3 * (y * 640 + x)] < 200)), "the plane has no preload meter");
}

/// V42: a fresh settings file rides with the readout, its right edge where the layout's `X` is, and
/// the preload meter beside it, in one view and in either split: `speedometer.lyt`'s readout at
/// 588,329 and its meter 12 up from the Flash movie's 599,326; `speedometerhorizontal1.lyt`'s at
/// 588,155 and 194 up, `speedometerhorizontal2.lyt`'s at 588,145 and 10 down, both half-height views;
/// `speedometervertical1free.lyt`'s at 270,340 and 320 left of a half-width view. The readout's
/// place is the view's, the meter's the whole screen's. The plane has the readout alone.
#[test]
fn a_fresh_settings_file_rides_with_the_readout_and_the_meter() {
    let Some(art) = art() else { return };
    let style = menu::Settings::parse("").speedometer;
    assert_eq!(style, menu::Speedometer::Digital);
    let tables = Arc::new(Tables::build());
    let mut one = rider(&tables, [0.0, 0.0], [0.0, 1.0]);
    ride(&mut one, 200, true, 0.0);
    let mut two = vec![one, rider(&tables, [SPACING, 0.0], [0.0, 1.0])];
    ride(&mut two[1], 100, true, 0.0);
    let two = ridden(&two);
    let gauges = overlay::Gauges { bike: true, speedometer: style, ..Default::default() };
    let back = [90, 110, 140];
    // The view, the riders, which of them, the place, the readout's right edge, the meter's top left.
    let cases = [
        ((640, 448), &two[..1], 0, overlay::Place::Full, 588, (599, 314)),
        ((640, 224), &two[..], 0, overlay::Place::Top, 588, (599, 132)),
        ((640, 224), &two[..], 1, overlay::Place::Bottom, 588, (599, 112)),
        ((320, 448), &two[..], 0, overlay::Place::Side, 270, (279, 326)),
        ((320, 448), &two[..], 1, overlay::Place::Side, 270, (279, 326)),
    ];
    for ((w, h), riders, v, place, right, (mx, my)) in cases {
        let mut frame = Frame::new(w, h, back);
        let drawn = art.draw(&mut frame, (640, 448), riders, v, 384.0, &gauges);
        let what = format!("{place:?} view {v}");
        assert_eq!(drawn.place, place, "{what}");
        assert_eq!(drawn.speed, Some(overlay::speed_text(riders[v].speed.0)), "{what}");
        assert_eq!((drawn.needle, drawn.odometer.as_deref(), drawn.preload), (None, None, Some(0.0)), "{what}");
        // What is lit or dark in a box, as its bounds.
        let bounds = |x0: usize, x1: usize, y0: usize, y1: usize, hit: &dyn Fn(&[u8]) -> bool| {
            let mut b = (usize::MAX, usize::MAX, 0, 0);
            for y in y0..y1.min(h) {
                for x in x0..x1.min(w) {
                    if hit(&frame.color[3 * (y * w + x)..][..3]) {
                        b = (b.0.min(x), b.1.min(y), b.2.max(x + 1), b.3.max(y + 1));
                    }
                }
            }
            b
        };
        let meter = bounds(mx - 8, mx + 28, my - 8, my + 93, &|c| c.iter().all(|&c| c < 50));
        let want = (mx, my, mx + 20, my + 85);
        let near = |a: usize, b: usize| a.abs_diff(b) <= 2;
        assert!(
            near(meter.0, want.0) && near(meter.1, want.1) && near(meter.2, want.2) && near(meter.3, want.3),
            "{what}: the meter is at {meter:?}, wanted {want:?}"
        );
        let text = bounds(w / 2, w, 0, h, &|c| c.iter().all(|&c| c > 180));
        assert!(text.2 <= right + 1 && text.2 + 4 >= right, "{what}: the readout ends at {}, wanted {right}", text.2);
        assert!(text.2 < meter.0 && text.1 < meter.3 && text.3 > meter.1, "{what}: the readout {text:?} is not beside the meter {meter:?}");
    }

    let plane = overlay::Gauges { bike: false, ..gauges };
    let mut frame = Frame::new(640, 448, back);
    let drawn = art.draw(&mut frame, (640, 448), &two[..1], 0, 384.0, &plane);
    assert_eq!((drawn.speed.is_some(), drawn.needle, drawn.preload), (true, None, None));
    let analog = overlay::Gauges { speedometer: menu::Speedometer::Analog, ..plane };
    let drawn = art.draw(&mut frame, (640, 448), &two[..1], 0, 384.0, &analog);
    assert_eq!((drawn.speed, drawn.needle.is_some(), drawn.odometer, drawn.preload), (None, true, None, None));
}

/// The movie's levels, tick by tick from the tick a preload fired, as the original drew them for
/// four shoves of recording2 (V53, PCSX2's video capture of the playback, each tick two identical
/// frames): up two at a time, fifteen ticks at the value, down the odd levels, then empty.
#[test]
fn the_preload_meter_plays_the_originals_movie() {
    let seen: [(i16, &[u8]); 4] = [
        (10, &[2, 4, 6, 8, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 9, 7, 5, 3, 1]),
        (7, &[2, 4, 6, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 5, 3, 1]),
        (5, &[2, 4, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 3, 1]),
        (3, &[2, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 1]),
    ];
    for (n, levels) in seen {
        let played: Vec<u8> = (0..).map_while(|t| overlay::PreloadMeter::level_at(n, t)).collect();
        assert_eq!(played, levels, "value {n}");
        assert_eq!(played.len(), 14 + n as usize, "value {n}: the original holds its latch 14 + n ticks");
    }
    // The ends of the range: one level is held and gone, twenty climbs to the top.
    assert_eq!((0..).map_while(|t| overlay::PreloadMeter::level_at(1, t)).collect::<Vec<_>>(), [1; 15]);
    let full: Vec<u8> = (0..).map_while(|t| overlay::PreloadMeter::level_at(20, t)).collect();
    assert_eq!((full.len(), &full[..10], full[9 + 15]), (34, &[2, 4, 6, 8, 10, 12, 14, 16, 18, 20][..], 19));
}

/// The shove's strength as the speedometer writes it: twenty steps of `MaxPreload`, rounded by
/// adding a half and truncating, never below one. The first four are recording2's (V53).
#[test]
fn the_preload_meter_shows_the_shoves_strength() {
    use overlay::PreloadMeter as M;
    assert_eq!(M::strength(285.71551, 600.0), 10);
    assert_eq!(M::strength(222.19357, 600.0), 7);
    assert_eq!(M::strength(148.93628, 600.0), 5);
    assert_eq!(M::strength(104.08241, 600.0), 3);
    assert_eq!((M::strength(0.0, 600.0), M::strength(45.0, 600.0), M::strength(600.0, 600.0)), (1, 2, 20));
}

/// What the meter does around a shove, off the port's own preload fields: dark, lit while the lip
/// test passes — and still empty while the stick is pumped — the strength held through the movie
/// whatever the preload does meanwhile, and nothing for a rider who is down.
#[test]
fn the_preload_meter_follows_the_preload() {
    let mut air = game_sim::air::Air::default();
    let mut m = overlay::PreloadMeter::default();
    let sense = |air: &game_sim::air::Air| overlay::PreloadSense::of(air, 600.0, false);
    m.step(sense(&air));
    assert_eq!((m.value, m.shown()), (-1, None), "a standing bike's meter is dark");
    air.preload_ready = true;
    air.preload_armed = true;
    for _ in 0..5 {
        m.step(sense(&air));
        assert_eq!((m.value, m.shown()), (0, Some(0.0)), "pumped, the meter is lit and empty");
    }
    air.preload_firing = true;
    air.preload_force = Vf::new(0.0, 285.71551, 0.0, 0.0);
    let mut shown = Vec::new();
    for t in 0..30 {
        if t == 7 {
            air.preload_firing = false;
            air.preload_ready = false;
        }
        m.step(sense(&air));
        shown.push((m.value, m.latched, m.level()));
    }
    assert_eq!(shown[0], (10, true, 2));
    assert_eq!(shown[4], (10, true, 10));
    assert_eq!(m.shown(), None, "back to dark in the air");
    assert_eq!(shown[23], (10, true, 1));
    assert_eq!(shown[24], (10, false, 0), "the movie lets go 24 ticks on, the value still written");
    assert_eq!(shown[25], (-1, false, 0));
    let drawn = |s: &[(i16, bool, u8)]| s.iter().map(|x| x.2).collect::<Vec<_>>();
    assert_eq!(drawn(&shown[..24]), (0..24).map(|t| overlay::PreloadMeter::level_at(10, t).unwrap()).collect::<Vec<_>>());

    let mut down = overlay::PreloadMeter::default();
    air.preload_ready = true;
    air.preload_firing = true;
    down.step(overlay::PreloadSense::of(&air, 600.0, true));
    assert_eq!((down.value, down.latched, down.level()), (-1, false, 0), "a rider who is down sees nothing");
}

/// One row of a V53 capture: what the speedometer read off the rider, and what it wrote.
struct MeterRow {
    frame: u32,
    sense: overlay::PreloadSense,
    value: i16,
    latched: bool,
}

fn meter_rows(path: &std::path::Path) -> Vec<MeterRow> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().expect("a header").split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name).unwrap_or_else(|| panic!("no {name} column"));
    let [frame, var, latch, ready, firing, force, max, wreck] =
        ["frame_counter", "meter_var", "meter_latch", "avail", "firing", "force", "max", "hide"].map(col);
    let int = |s: &str| if let Some(h) = s.strip_prefix("0x") { u32::from_str_radix(h, 16).unwrap() } else { s.parse().unwrap() };
    lines
        .map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            MeterRow {
                frame: int(c[frame]),
                sense: overlay::PreloadSense {
                    ready: int(c[ready]) != 0,
                    firing: int(c[firing]) != 0,
                    force: c[force].parse().unwrap(),
                    max: c[max].parse().unwrap(),
                    wrecked: int(c[wreck]) != 0,
                },
                // The variable's word at +0xc: its value is the high half.
                value: (int(c[var]) >> 16) as u16 as i16,
                latched: int(c[latch]) != 0,
            }
        })
        .collect()
}

/// Over recording2 played back on the original — its pumped preloads and the backflips off them —
/// the port's meter, fed the rider's preload fields row by row, writes the speedometer's
/// `preloadMeter` variable and holds its latch as the original did, every tick (V53). A capture row
/// is read while the frame runs, so a row can catch the rider updated and the HUD not yet: each row
/// matches the port's tick or the one before, nearly all the tick itself, each shove's value is
/// exact, and its hold is the original's to the row that tore.
///
/// The captures are `context/capture/v53/` (git-ignored), `./kit record` of the spec beside them
/// during a playback of recording2.p2m2 (C6's play_capture.sh); without them this skips.
#[test]
fn the_preload_meter_writes_the_originals_variable() {
    use ps2_oracle::{Kit, skip};
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    let dir = kit.ctx("capture").join("v53");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "csv")).collect())
        .unwrap_or_default();
    files.sort();
    if files.is_empty() {
        skip(&format!("no V53 capture in {}", dir.display()));
        return;
    }
    for path in files {
        let rows = meter_rows(&path);
        let mut m = overlay::PreloadMeter::default();
        let port: Vec<(i16, bool)> = rows
            .iter()
            .map(|r| {
                m.step(r.sense);
                (m.value, m.latched)
            })
            .collect();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let mut same = 0;
        for (i, r) in rows.iter().enumerate() {
            let got = (r.value, r.latched);
            same += usize::from(got == port[i]);
            assert!(
                got == port[i] || (i > 0 && got == port[i - 1]),
                "{name} frame {}: the original wrote {got:?}, the port {:?} (before it {:?})",
                r.frame,
                port[i],
                port.get(i.wrapping_sub(1))
            );
        }
        assert!(same * 100 >= rows.len() * 99, "{name}: only {same} of {} rows on the port's own tick", rows.len());
        // Each shove: its strength, held as long as the original held it.
        let held = |v: &[(i16, bool)]| {
            let mut runs = Vec::new();
            for (i, w) in v.iter().enumerate() {
                if w.1 && (i == 0 || !v[i - 1].1) {
                    runs.push((w.0, v[i..].iter().take_while(|x| x.1).count()));
                }
            }
            runs
        };
        let original: Vec<(i16, bool)> = rows.iter().map(|r| (r.value, r.latched)).collect();
        let shoves = held(&original);
        assert!(shoves.len() >= 4, "{name}: {} shoves, recording2 pumps four", shoves.len());
        // A torn row can start or end a hold a row late in the capture: one row either way.
        let ported = held(&port);
        assert_eq!(ported.iter().map(|s| s.0).collect::<Vec<_>>(), shoves.iter().map(|s| s.0).collect::<Vec<_>>(), "{name}: the shoves");
        for (p, o) in ported.iter().zip(&shoves) {
            assert_eq!(p.1, 14 + p.0 as usize, "{name}: the port's hold of {}", p.0);
            assert!(p.1.abs_diff(o.1) <= 1, "{name}: the original held {} for {} rows, the port {}", o.0, o.1, p.1);
        }
    }
}

/// Each rider's name and number from the garage (P86): the view's own under the score, the others'
/// numbers beside their marks; a rider with neither has nothing written.
#[test]
fn the_riders_names_and_numbers_are_drawn() {
    let Some(mut art) = art() else { return };
    let tables = Arc::new(Tables::build());
    let sims = four_riders(&tables);
    let riders = ridden(&sims);
    let tag = |name: &str, number: &str| overlay::Tag { name: name.into(), number: number.into() };
    art.tags = vec![tag("Kaito", "72"), tag("", "5"), tag("Ann", ""), tag("", "")];
    let mut blank = Frame::new(320, 224, [255, 255, 255]);
    let mut view = Frame::new(320, 224, [255, 255, 255]);
    let drawn = art.draw(&mut view, (640, 448), &riders, 0, overlay::radar_span(SPACING), &bike(0.0));
    assert_eq!(drawn.tag.as_deref(), Some("#72 Kaito"));
    assert_eq!(drawn.mark_tags, [(1, "5".to_string()), (2, "A".to_string())]);
    art.tags.clear();
    let bare = art.draw(&mut blank, (640, 448), &riders, 0, overlay::radar_span(SPACING), &bike(0.0));
    assert_eq!((bare.tag, bare.mark_tags.len()), (None, 0));
    assert_ne!(view.color, blank.color, "the tags drew");
    let drawn = {
        art.tags = vec![tag("Kaito", "72"), tag("", "5")];
        art.draw(&mut Frame::new(320, 224, [0, 0, 0]), (640, 448), &riders, 1, overlay::radar_span(SPACING), &bike(0.0))
    };
    assert_eq!(drawn.tag.as_deref(), Some("#5"));
    assert_eq!(drawn.mark_tags, [(0, "72".to_string())], "seats past the tags have none");
}

// ---- the radar's other icons (P92) -------------------------------------------------------------

/// One of every kind of icon, spread round `at` at distances inside and outside half the span.
fn every_icon(at: [f32; 2]) -> Vec<overlay::Blip> {
    use overlay::{Activity, Blip, Icon};
    use vehicle::Machine;
    let kinds = [
        Icon::Activity(Activity::Hit),
        Icon::Activity(Activity::Run),
        Icon::Activity(Activity::Race),
        Icon::Activity(Activity::Stunt),
        Icon::Machine(Machine::Biplane, 0),
        Icon::Machine(Machine::Biplane, 1),
        Icon::Machine(Machine::Helicopter, 0),
        Icon::Machine(Machine::Buggy, 1),
        Icon::Machine(Machine::Monster, 0),
        Icon::Machine(Machine::Baja, 1),
        Icon::Bike,
        Icon::Point { done: false },
        Icon::Point { done: true },
        Icon::Marker { slot: 0, selected: false },
        Icon::Marker { slot: 3, selected: true },
    ];
    let n = kinds.len() as f32;
    kinds
        .into_iter()
        .enumerate()
        .map(|(i, icon)| {
            let (a, d) = (i as f32 / n * TAU, 60.0 + 70.0 * i as f32);
            Blip { icon, at: [at[0] + d * a.sin(), at[1] + d * a.cos()] }
        })
        .collect()
}

/// Every kind of icon goes where [`overlay::radar_mark`] puts a rider there — so held to the ring
/// past half the span, as the original clamps its distance for all of them — in its own texture, at
/// its own size and in the original's colour for it.
#[test]
fn each_icon_kind_is_placed_by_radar_mark() {
    use overlay::{Activity, Icon};
    use vehicle::Machine;
    let span = overlay::radar_span(SPACING);
    let (own, forward) = ([100.0, -40.0], [0.6, 0.8]);
    let mut seen = std::collections::HashSet::new();
    for blip in every_icon(own) {
        let p = overlay::radar_icon(own, forward, blip, span);
        assert_eq!(p.at, overlay::radar_mark(own, forward, blip.at, span), "{blip:?}");
        let off = ((p.at.0 - overlay::RADAR_AT.0).powi(2) + (p.at.1 - overlay::RADAR_AT.1).powi(2)).sqrt();
        let d = ((blip.at[0] - own[0]).powi(2) + (blip.at[1] - own[1]).powi(2)).sqrt();
        if d >= span / 2.0 {
            assert!((off - overlay::RADAR_RADIUS / 2.0).abs() < 1e-3, "{blip:?} past the span is off the ring: {off}");
        } else {
            assert!((off - d * overlay::RADAR_RADIUS / span).abs() < 1e-3, "{blip:?} at {d} drawn {off} out");
        }
        let (texture, tint) = match blip.icon {
            Icon::Activity(Activity::Hit) => ("map_icon_h", [0x6e, 0xa6, 0x37, 255]),
            Icon::Activity(Activity::Run) => ("map_icon_r", [0xff, 0x99, 0x33, 255]),
            Icon::Activity(Activity::Race) => ("map_icon_m", [0x33, 0x73, 0xcc, 255]),
            Icon::Activity(Activity::Stunt) => ("map_icon_s", [0x81, 0x3b, 0xb5, 255]),
            Icon::Machine(Machine::Biplane, 0) => ("map_icon_bplane", [0xe4, 0x19, 0x1a, 255]),
            Icon::Machine(Machine::Biplane, _) => ("map_icon_bplane", [0xe7, 0xc1, 0x13, 255]),
            Icon::Machine(Machine::Helicopter, _) => ("map_icon_heli", [0x4c, 0x90, 0xbc, 255]),
            Icon::Machine(Machine::Buggy, _) => ("map_icon_buggy", [0x84, 0x8b, 0x6b, 255]),
            Icon::Machine(Machine::Monster, _) => ("map_icon_monster", [0xa1, 0xcc, 0x29, 255]),
            Icon::Machine(Machine::Baja, _) => ("map_icon_trophy", [0xe0, 0x98, 0x0e, 255]),
            Icon::Bike => ("map_icon_bike", [0xc8, 0xc8, 0xc8, 255]),
            Icon::Point { done: false } => ("map_icon_hit_freestyle", [0x30, 0xcf, 0x00, 255]),
            Icon::Point { done: true } => ("map_icon_hit_freestyle", [0x70, 0x70, 0x70, 255]),
            // The port's own (P94): drawn, no texture of the set, in its slot's colour.
            Icon::Marker { slot, .. } => ("", overlay::MARKER_COLOURS[slot]),
        };
        assert_eq!((p.texture, p.tint), (texture, tint), "{blip:?}");
        let size = match blip.icon {
            Icon::Activity(_) => (overlay::zone_size(d, span), overlay::zone_size(d, span)),
            Icon::Machine(..) | Icon::Bike => (32.0, 32.0),
            Icon::Point { .. } => (8.0, 8.0),
            Icon::Marker { selected: false, .. } => (13.0, 13.0),
            Icon::Marker { selected: true, .. } => (17.0, 17.0),
        };
        assert_eq!(p.size, size, "{blip:?}");
        assert_eq!(p.turn, 0.0, "{blip:?} is not turned");
        seen.insert(texture);
    }
    assert_eq!(seen.len(), 12, "every texture of the set, and the markers' none: {seen:?}");
}

/// The activities' icons grow as they near: half size from half the span out, four fifths from
/// within a fifth of it, and between, `0.3` more than the share of half the span they are nearer by.
#[test]
fn an_activitys_icon_grows_as_it_nears() {
    let span = overlay::radar_span(SPACING);
    let r = span / 2.0;
    for (d, want) in [(r * 3.0, 16.0), (r, 16.0), (r * 0.8, 16.0), (r * 0.65, 20.8), (r * 0.5, 25.6), (0.0, 25.6)] {
        let got = overlay::zone_size(d, span);
        assert!((got - want).abs() < 1e-3, "{d}: {got}, wanted {want}");
    }
}

/// A marker's radar icon is its slot's (P94): each slot its own colour, none of them a colour of
/// the original's icons or riders, and the selected one drawn bigger for its ring. The rider's right
/// is the radar's right: a marker straight to the right is drawn right of the middle, on the level.
#[test]
fn a_markers_icon_is_its_slots() {
    use overlay::{Blip, Icon};
    let span = overlay::radar_span(SPACING);
    let forward = [0.8, -0.6];
    let colours: Vec<[u8; 4]> = (0..game_sim::markers::SLOTS)
        .map(|slot| overlay::radar_icon([0.0, 0.0], forward, Blip { icon: Icon::Marker { slot, selected: false }, at: [10.0, 10.0] }, span).tint)
        .collect();
    for (i, c) in colours.iter().enumerate() {
        assert!(colours[..i].iter().all(|o| o != c), "slot {} shares a colour: {colours:?}", i + 1);
        // The rider's mark (yellow), an AI's (red), the bikes' grey and the original's set's colours.
        let theirs = [[0xff, 0xff, 0x00, 255], [0xff, 0x00, 0x00, 255], [0xc8, 0xc8, 0xc8, 255], [0x6e, 0xa6, 0x37, 255], [0xff, 0x99, 0x33, 255], [0x33, 0x73, 0xcc, 255], [0x81, 0x3b, 0xb5, 255], [0x30, 0xcf, 0x00, 255]];
        assert!(!theirs.contains(c), "slot {} is in one of the original's colours", i + 1);
    }
    let plain = overlay::radar_icon([0.0, 0.0], forward, Blip { icon: Icon::Marker { slot: 2, selected: false }, at: [10.0, 10.0] }, span);
    let picked = overlay::radar_icon([0.0, 0.0], forward, Blip { icon: Icon::Marker { slot: 2, selected: true }, at: [10.0, 10.0] }, span);
    assert!(picked.size.0 > plain.size.0 && picked.tint == plain.tint && picked.at == plain.at);
    let p = overlay::radar_icon([0.0, 0.0], forward, Blip { icon: Icon::Bike, at: [-60.0, -80.0] }, span);
    assert!(p.at.0 > overlay::RADAR_AT.0 + 1.0 && (p.at.1 - overlay::RADAR_AT.1).abs() < 1e-3, "{:?}", p.at);
}

/// `hit.dat`'s start zones, read as the disc writes them (free07's `Type = Hit` spaced, the rest
/// not), and the ones a player who has started nothing is shown: the first of each activity, and no
/// machine race from inside a machine.
#[test]
fn the_start_zones_are_the_first_of_each_activity() {
    use overlay::Activity::*;
    let text = "[HitData]\nHitDBFilename = free.hit\n\n[Activity Start Zones]\nASZCount = 6\n\n\
        [ASZ 1]\nPosition = 2922.0, 332.0, 2768.0, 1.000000\nLookVector = -0.25, 0.0, 0.97, 1.0\nType = Hit\n\n\
        [ASZ 2]\nPosition = 3053.0, 244.0, 2635.0, 1.000000\nType=Hit\n\n\
        [ASZ 3]\nPosition = 4479.0, 0.0, 2324.0, 1.000000\nType=Run\n\n\
        [ASZ 4]\nType=Race\nPosition = 2627.0, 0.0, 2685.0, 1.000000\n\n\
        [ASZ 5]\nPosition = 1.0, 2.0, 3.0, 1.0\nType=Bowling\n\n\
        [ASZ 6]\nPosition = 3220.0, 0.0, 3089.0, 1.000000\r\nType=Stunt\r\n";
    let zones = overlay::zones(text);
    let got: Vec<_> = zones.iter().map(|z| (z.activity, z.at)).collect();
    assert_eq!(
        got,
        [(Hit, [2922.0, 2768.0]), (Hit, [3053.0, 2635.0]), (Run, [4479.0, 2324.0]), (Race, [2627.0, 2685.0]), (Stunt, [3220.0, 3089.0])]
    );
    let shown = |m| overlay::shown_zones(&zones, m).iter().map(|z| (z.activity, z.at[0])).collect::<Vec<_>>();
    assert_eq!(shown(false), [(Hit, 2922.0), (Run, 4479.0), (Race, 2627.0), (Stunt, 3220.0)]);
    assert_eq!(shown(true), [(Hit, 2922.0), (Run, 4479.0), (Stunt, 3220.0)]);
    assert!(overlay::zones("[HitData]\nType=Hit\nPosition = 1, 2, 3\n").is_empty(), "only the [ASZ n] sections");
}

/// On the disc: every freeride track has its twelve start zones, five hits, five runs, a race and a
/// stunt competition — free06's first of each where the original's own save has them (the human's
/// biplane state, `research/oracle.txt` P92).
#[test]
fn the_freeride_tracks_have_their_start_zones() {
    use overlay::Activity::*;
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipped, no disc: {e}");
            return;
        }
    };
    for track in ["free06", "free07", "free09", "free10", "free11"] {
        let res = disc.res(&format!("Tracks\\{track}_2.res")).unwrap_or_else(|e| panic!("{track}: {e}"));
        let text = res.read_suffix(&format!("{track}\\hit.dat")).unwrap_or_else(|e| panic!("{track}: {e}"));
        let zones = overlay::zones(&String::from_utf8_lossy(&text));
        let count = |a| zones.iter().filter(|z| z.activity == a).count();
        assert_eq!([count(Hit), count(Run), count(Race), count(Stunt)], [5, 5, 1, 1], "{track}");
        if track == "free06" {
            let first: Vec<_> = overlay::shown_zones(&zones, false).iter().map(|z| z.at).collect();
            assert_eq!(first, [[2922.0, 2768.0], [4479.0, 2324.0], [2627.0, 2685.0], [3220.0, 3089.0]]);
        }
    }
}

/// Drawn: each icon of [`Art::radar`]'s view goes into the view where `radar_icon` says, in its own
/// texture and colour, under the riders' marks and the arrow; another view's are not drawn there.
#[test]
fn the_radar_draws_its_icons() {
    let Some(mut art) = art() else { return };
    let tables = Arc::new(Tables::build());
    let span = overlay::radar_span(SPACING);
    let sims = four_riders(&tables);
    let riders = ridden(&sims);
    let own = ground(riders[0].target.body_point);
    let forward = ground(riders[0].target.forward);
    let blips = every_icon(own);
    art.radar = vec![blips.clone()];
    let mut frame = Frame::new(1280, 896, [0, 0, 0]);
    let drawn = art.draw(&mut frame, (1280, 896), &riders, 0, span, &bike(0.0));
    assert_eq!(drawn.icons.len(), blips.len());
    for (&(icon, (x, y)), blip) in drawn.icons.iter().zip(&blips) {
        assert_eq!(icon, blip.icon);
        let want = overlay::radar_icon(own, forward, *blip, span).at;
        // From the view's bottom-left, at the view's scale.
        let px = (want.0 * drawn.scale, 896.0 - (448.0 - want.1) * drawn.scale);
        assert!(
            (x - px.0).abs() < 0.01 && (y - px.1).abs() < 0.01,
            "{icon:?} at {x},{y}, wanted {want:?} at scale {}",
            drawn.scale
        );
    }
    // The GPU's sprites carry each icon's colour, and the markers are discs (P94).
    let mut sprites = Vec::new();
    art.sprites(&mut sprites, (0, 0, 1280, 896), (1280, 896), &riders, 0, span, &bike(0.0));
    for blip in &blips {
        let p = overlay::radar_icon(own, forward, *blip, span);
        assert!(sprites.iter().any(|s| s.tint == p.tint), "{blip:?}: no sprite in {:?}", p.tint);
    }
    let disc = |s: &&game_render::gpu::Sprite| matches!(s.shape, game_render::gpu::Shape::Capsule { .. });
    let discs = |tint: [u8; 4]| sprites.iter().filter(disc).filter(|s| s.tint == tint).count();
    assert_eq!(discs([255, 255, 255, 255]), 1, "the selected marker's ring");
    assert_eq!((discs(overlay::MARKER_COLOURS[0]), discs(overlay::MARKER_COLOURS[3])), (1, 1), "a disc a marker");
    // Another view's icons are its own.
    let none = art.draw(&mut Frame::new(640, 448, [0, 0, 0]), (1280, 896), &riders, 1, span, &bike(0.0));
    assert!(none.icons.is_empty());
}

/// The lit pixels' bounds (left, top, right, bottom) in a box of `frame` — or `None` if none is.
fn lit_in(frame: &Frame, (x0, y0, x1, y1): (usize, usize, usize, usize)) -> Option<(usize, usize, usize, usize)> {
    let mut b: Option<(usize, usize, usize, usize)> = None;
    for y in y0..y1.min(frame.height) {
        for x in x0..x1.min(frame.width) {
            if frame.color[3 * (y * frame.width + x)..][..3].iter().any(|&c| c > 0) {
                let o = b.unwrap_or((x, y, x + 1, y + 1));
                b = Some((o.0.min(x), o.1.min(y), o.2.max(x + 1), o.3.max(y + 1)));
            }
        }
    }
    b
}

/// V52: the Minimap setting off draws the speedometer and not one pixel of the radar — its rose, the
/// arrow, the other riders' marks or the icons on it — in one view and in each of four.
#[test]
fn minimap_off_draws_the_speedometer_and_no_radar() {
    let Some(mut art) = art() else { return };
    let tables = Arc::new(Tables::build());
    let span = overlay::radar_span(SPACING);
    let solo = ridden(&[rider(&tables, [0.0, 0.0], [0.0, 1.0])]);
    let four = ridden(&four_riders(&tables));
    art.radar = vec![every_icon([0.0, 0.0]); 4];
    for (riders, (w, h), views) in [(&solo[..], (1280usize, 720usize), 1usize), (&four[..], (1280, 720), 4)] {
        let (vw, vh) = if views == 1 { (w, h) } else { (w / 2, h / 2) };
        for v in 0..views {
            for minimap in [true, false] {
                art.minimap = minimap;
                let mut frame = Frame::new(vw, vh, [0, 0, 0]);
                let drawn = art.draw(&mut frame, (w, h), riders, v, span, &bike(0.0));
                // The radar's corner, the view's bottom-left under the score and left of the middle;
                // the speedometer's, its bottom-right.
                let radar = lit_in(&frame, (0, vh / 3, vw / 2, vh));
                let speed = lit_in(&frame, (vw / 2, vh / 3, vw, vh));
                let what = format!("{} view(s), view {v}, minimap {minimap}", views);
                assert!(speed.is_some() && drawn.speed.is_some(), "{what}: no speedometer");
                assert_eq!(drawn.preload, Some(0.0), "{what}");
                assert_eq!(radar.is_some(), minimap, "{what}: radar pixels {radar:?}");
                assert_eq!(drawn.icons.is_empty(), !minimap, "{what}");
                assert_eq!(drawn.marks.is_empty(), !minimap || views == 1, "{what}");
            }
        }
    }
    // The setting: on unless the file says off, which is written only then.
    assert!(menu::Settings::parse("").minimap);
    assert!(!menu::Settings::parse("minimap = off\n").minimap);
    assert!(!menu::Settings::default().text().contains("minimap"));
}

/// V52: HUD Scale 150 % draws the radar and the speedometer half again as big, each still in its own
/// corner and inside the view — one view, with the dial and with the readout, and each of a four-way
/// split. A four-way split of a window under about 2:1 has no room for 150 % of the original's size
/// between its corners, so that one is held to the most the view holds instead. The radar is what
/// the minimap's switch takes away; the speedometer is what is lit in the view's right half.
#[test]
fn hud_scale_grows_the_radar_and_speedometer_about_their_corners() {
    let Some(mut art) = art() else { return };
    let tables = Arc::new(Tables::build());
    let span = overlay::radar_span(SPACING);
    let mut one = rider(&tables, [0.0, 0.0], [0.0, 1.0]);
    ride(&mut one, 200, true, 0.0);
    let solo = ridden(std::slice::from_ref(&one));
    let four = ridden(&four_riders(&tables));
    let analog = overlay::Gauges { speedometer: menu::Speedometer::Analog, ..bike(1000.0) };
    // The scale, the radar's and the speedometer's bounds of view `v` of a window `w` by `h`.
    let parts = |art: &mut overlay::Art, riders: &[Ridden], (w, h): (usize, usize), views: usize, v: usize, gauges: &overlay::Gauges, scale: f32| {
        let (vw, vh) = if views == 1 { (w, h) } else { (w / 2, h / 2) };
        art.scale = scale;
        art.minimap = false;
        let mut bare = Frame::new(vw, vh, [0, 0, 0]);
        art.draw(&mut bare, (w, h), riders, v, span, gauges);
        art.minimap = true;
        let mut frame = Frame::new(vw, vh, [0, 0, 0]);
        let drawn = art.draw(&mut frame, (w, h), riders, v, span, gauges);
        for (a, b) in frame.color.iter_mut().zip(&bare.color) {
            *a = a.abs_diff(*b);
        }
        let radar = lit_in(&frame, (0, 0, vw, vh)).expect("a radar");
        let speed = lit_in(&bare, (vw / 2, 0, vw, vh)).expect("a speedometer");
        (drawn.scale, radar, speed)
    };
    let near = |a: usize, b: f32| (a as f32 - b).abs() <= 3.0;
    let cases = [(&solo[..], (1280usize, 720usize), 1usize, analog), (&solo[..], (1280, 720), 1, bike(0.0)), (&four[..], (2400, 900), 4, bike(0.0))];
    for (riders, (w, h), views, gauges) in cases {
        let (vw, vh) = if views == 1 { (w, h) } else { (w / 2, h / 2) };
        for v in 0..views {
            let what = format!("{w}x{h}, {views} view(s), view {v}, {:?}", gauges.speedometer);
            let (s1, r1, p1) = parts(&mut art, riders, (w, h), views, v, &gauges, 1.0);
            let (s15, r15, p15) = parts(&mut art, riders, (w, h), views, v, &gauges, 1.5);
            assert!((s15 - 1.5 * s1).abs() < 1e-4, "{what}: scale {s15} against {s1}");
            // Half again as big, within a pixel or two of rounding at each edge, and inside the view.
            for (part, small, big) in [("radar", r1, r15), ("speedometer", p1, p15)] {
                let (sw, sh) = ((small.2 - small.0) as f32, (small.3 - small.1) as f32);
                assert!(near(big.2 - big.0, 1.5 * sw) && near(big.3 - big.1, 1.5 * sh), "{what}: the {part} {small:?} -> {big:?}");
                assert!(big.0 > 0 && big.1 > 0 && big.2 < vw && big.3 < vh, "{what}: the {part} at the view's edge: {big:?}");
            }
            // About their corners: the radar's gaps to the left and bottom edges, and the
            // speedometer's to the right and bottom, half again as wide.
            assert!(near(r15.0, 1.5 * r1.0 as f32), "{what}: radar left {r1:?} -> {r15:?}");
            assert!(near(vh - r15.3, 1.5 * (vh - r1.3) as f32), "{what}: radar bottom {r1:?} -> {r15:?}");
            assert!(near(vw - p15.2, 1.5 * (vw - p1.2) as f32), "{what}: speedometer right {p1:?} -> {p15:?}");
            assert!(near(vh - p15.3, 1.5 * (vh - p1.3) as f32), "{what}: speedometer bottom {p1:?} -> {p15:?}");
            // And still apart.
            assert!(r15.2 < p15.0, "{what}: the radar {r15:?} runs into the speedometer {p15:?}");
        }
    }
    // A four-way split of 1280x720 has less room: the parts grow until they would meet across the
    // view's 640 pixels, and no further — inside it, the radar clear of the speedometer.
    for v in 0..4 {
        let (s, radar, speed) = parts(&mut art, &four, (1280, 720), 4, v, &bike(0.0), 1.5);
        assert!((s - 640.0 / 311.0).abs() < 1e-4, "view {v}: {s}");
        assert!(radar.0 > 0 && radar.1 > 0 && radar.3 < 360 && speed.2 < 640 && speed.3 < 360, "view {v}: {radar:?} {speed:?}");
        assert!(radar.2 < speed.0, "view {v}: {radar:?} runs into {speed:?}");
    }
    // 100 % is the original's size, and the smaller steps shrink it.
    art.scale = 0.5;
    let drawn = art.draw(&mut Frame::new(640, 448, [0, 0, 0]), (640, 448), &solo, 0, span, &bike(0.0));
    assert_eq!(drawn.scale, 0.5);
}

/// Over the user's animandpump ride played back on the original (S109, 1204 ticks of riding,
/// pumped through five lips with no shove fired), the port's meter, fed the rider's preload fields
/// row by row, writes the speedometer's `preloadMeter` variable as the original did on every row:
/// dark while the lip test fails, lit and empty while it passes, never latched. The capture is
/// `context/capture/s109/run1.csv` (git-ignored, research/journal/2026-10-23-s109-animandpump);
/// without it this skips.
#[test]
fn the_preload_meter_is_the_originals_over_animandpump() {
    use ps2_oracle::{Kit, skip};
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    // run2 is the whole movie (2361 ticks, a wreck at 3716..3915); run1 its first half.
    let path = kit.ctx("capture").join("s109").join("run2.csv");
    if !path.exists() {
        skip(&format!("no S109 capture at {}", path.display()));
        return;
    }
    let rows = meter_rows(&path);
    let mut m = overlay::PreloadMeter::default();
    let (mut lit, mut runs, mut was) = (0, 0, false);
    for r in &rows {
        m.step(r.sense);
        assert_eq!((m.value, m.latched), (r.value, r.latched), "frame {}: the port against the original", r.frame);
        let on = r.value == 0;
        lit += usize::from(on);
        runs += usize::from(on && !was);
        was = on;
    }
    assert!(rows.len() >= 2300 && lit >= 300 && runs >= 8, "{} rows, {lit} lit in {runs} runs", rows.len());
}

/// Every one of the 64 marker slots (P139) has its own colour: the first five still
/// [`overlay::MARKER_COLOURS`], none the original's, and no two alike on the radar.
#[test]
fn sixty_four_marker_colours_are_all_apart() {
    use game_sim::markers::{MAX_SLOTS, SLOTS};
    let colours: Vec<[u8; 4]> = (0..MAX_SLOTS).map(overlay::marker_colour).collect();
    assert_eq!(colours[..SLOTS], overlay::MARKER_COLOURS);
    let theirs = [[0xff, 0xff, 0x00, 255], [0xff, 0x00, 0x00, 255], [0xc8, 0xc8, 0xc8, 255], [0x6e, 0xa6, 0x37, 255], [0xff, 0x99, 0x33, 255], [0x33, 0x73, 0xcc, 255], [0x81, 0x3b, 0xb5, 255], [0x30, 0xcf, 0x00, 255]];
    let apart = |a: [u8; 4], b: [u8; 4]| (0..3).map(|i| (a[i] as f32 - b[i] as f32).powi(2)).sum::<f32>().sqrt();
    let mut least = f32::MAX;
    for (i, &c) in colours.iter().enumerate() {
        assert_eq!(c[3], 255, "slot {} is not opaque", i + 1);
        for &o in colours[..i].iter().chain(&theirs) {
            least = least.min(apart(c, o));
        }
    }
    eprintln!("closest two: {least}");
    assert!(least >= 28.0, "two colours {least} apart");
}
