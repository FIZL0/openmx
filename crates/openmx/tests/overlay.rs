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
/// drawn there in the human riders' cyan.
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
            // Clear of the arrow, the mark is there in the frame and it is cyan.
            if (x - drawn.player.0).abs() > 12.0 || (y - drawn.player.1).abs() > 12.0 {
                let at = 3 * (y as usize * view.width + x as usize);
                let c = &view.color[at..at + 3];
                assert!(c[0] < 90 && c[1] > 150 && c[2] > 150, "view {v}: rider {i}'s mark is {c:?}");
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
    let Some(art) = art() else { return };
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
    let analog = overlay::Gauges { feet: 1234.0, bike: true, score: 4321, preload: 0.6, modified: false, speedometer: menu::Speedometer::Analog };
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
    let gauges = overlay::Gauges { feet: 5280.0, bike: true, preload: 0.5, score: 1234, modified: false, speedometer: menu::Speedometer::Analog };
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

/// The meter fills with the preload's shove and empties as its timer runs out; it is empty unless
/// a preload is firing.
#[test]
fn the_preload_meter_follows_the_preload() {
    assert_eq!(overlay::preload_fill(false, 10.0, 10.0, 1.0, 1.0), 0.0);
    assert_eq!(overlay::preload_fill(true, 10.0, 10.0, 1.0, 1.0), 1.0);
    assert_eq!(overlay::preload_fill(true, 5.0, 10.0, 1.0, 1.0), 0.5);
    assert_eq!(overlay::preload_fill(true, 10.0, 10.0, 0.25, 1.0), 0.25);
    assert_eq!(overlay::preload_fill(true, 30.0, 10.0, 1.0, 1.0), 1.0);
    assert_eq!(overlay::preload_fill(true, 10.0, 0.0, 1.0, 1.0), 0.0);
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
