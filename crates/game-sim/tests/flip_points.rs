//! S234 — a flip scores the frame it starts ([`Score::flip_at_start`], the user's ask), not when the
//! original's watch sees it come round.
//!
//! The original puts a backflip on the line only once its four-stage watch on the chassis' attitude
//! ([`Score::flip`]) has seen the bike go over and come back the right way up, no more than 0.79 nose
//! down; a flip landed before that — the bike still steeper than ~52° nose down on a steep landing,
//! or touching down the tick it would have come round — scores nothing (the C8 film shows "BackFlip
//! 10000" only after the rotation). With the switch up, what holds:
//!
//! - the flip is on the line, worth its 10 000 times the combo's length, the tick the bike starts
//!   turning it over ([`Sense::flipping`] going up), and once only: the watch coming round adds
//!   nothing more for that rotation;
//! - landed, it is banked even when the watch never came round; crashed, it is lost with the combo;
//! - a trick still growing when the flip starts keeps growing;
//! - the port's front flip (MD16) follows the same rules on its own watch;
//! - on a whole ride ([`Sim`]) off a ramp, the backflip's node is there the first tick
//!   [`game_sim::air::Air::flipping`] is, and the flip landed banks it.
//!
//! With the switch down every call is the original's, which `tests/score.rs` proves.

use game_sim::collide::{Bundle, Record};
use game_sim::phy::Phy;
use game_sim::ride::Controls;
use game_sim::rider::{Rider, Tables};
use game_sim::score::{self, Score, Sense};
use game_sim::trick::{Set, Trick};
use game_sim::world::World;
use game_sim::{Sim, stock};
use ps2_float::{F, Vf};
use std::sync::Arc;

const DT: F = F(0.03333);

/// In the air at a pitch `deg` turned back (a backflip's way) from level: the chassis' up and
/// forward axes' heights, as the rider's sense reads them.
fn aloft(deg: f32, flipping: u8) -> Sense {
    let r = deg.to_radians();
    Sense { airborne: true, hang_time: F(0.5), up_y: F(r.cos()), forward_y: F(r.sin()), flipping, ..Sense::default() }
}

/// Down on both wheels at a pitch `deg`.
fn down(deg: f32) -> Sense {
    let r = deg.to_radians();
    Sense { front: true, rear: true, up_y: F(r.cos()), forward_y: F(r.sin()), ..Sense::default() }
}

fn tick(s: &mut Score, sense: &Sense) {
    s.frame(DT, sense);
    s.grow(DT);
}

/// A backflip turned from level through `to` degrees, 10 a tick, the flip's word up while it turns,
/// then landed at `to`; the card with the switch as given. The tick its first node showed, if any.
fn backflip(at_start: bool, to: f32) -> (Score, Option<usize>) {
    let mut s = Score { flip_at_start: at_start, ..Score::default() };
    let mut shown = None;
    tick(&mut s, &aloft(0.0, 0));
    let mut k = 0;
    let mut deg = 0.0;
    while deg < to {
        k += 1;
        deg = (deg + 10.0f32).min(to);
        tick(&mut s, &aloft(deg, score::BACKFLIP));
        if shown.is_none() && s.nodes.iter().any(|n| n.id == score::BACKFLIP) {
            shown = Some(k);
        }
    }
    tick(&mut s, &down(to));
    (s, shown)
}

/// The flip is on the line the tick it starts, and a flip landed before the original's watch came
/// round (300°, 60° nose down) still banks; the original's card scores it nothing.
#[test]
fn a_flip_shows_on_the_line_the_frame_it_starts() {
    let (s, shown) = backflip(true, 300.0);
    assert_eq!(shown, Some(1), "not on the line the tick it started");
    assert_eq!(s.total, score::BACKFLIP_POINTS, "the landed flip was not banked");

    let (original, shown) = backflip(false, 300.0);
    assert_eq!(shown, None, "the original's card showed a flip that never came round");
    assert_eq!(original.total, F::ZERO, "the original's card banked a flip that never came round");
}

/// One full rotation landed scores once: the watch coming round pays nothing more for it.
#[test]
fn one_full_rotation_landed_scores_once() {
    let (s, shown) = backflip(true, 360.0);
    assert_eq!(shown, Some(1));
    assert_eq!(s.total, score::BACKFLIP_POINTS, "a full flip scored {} — not once", s.total.0);
    assert_eq!((s.flip, s.shown, s.turning), (0, 0, 0), "the landing left the flip's words up");
    // The original's card pays the same flip the same on its watch.
    let (original, _) = backflip(false, 360.0);
    assert_eq!(original.total, score::BACKFLIP_POINTS);
}

/// Two flips in one jump are two starts, each on the line as it starts, and the combo's length
/// multiplies the second.
#[test]
fn a_double_flip_is_two_flips() {
    let mut s = Score { flip_at_start: true, ..Score::default() };
    tick(&mut s, &aloft(0.0, 0));
    let mut deg = 0.0f32;
    for _ in 0..36 {
        deg += 10.0;
        tick(&mut s, &aloft(deg, score::BACKFLIP));
    }
    // The bike comes round, the rotation ends, and a second one starts.
    tick(&mut s, &aloft(0.0, 0));
    assert_eq!(s.nodes.len(), 1, "the first flip's watch scored it again");
    for _ in 0..36 {
        deg += 10.0;
        tick(&mut s, &aloft(deg, score::BACKFLIP));
    }
    assert_eq!(s.nodes.len(), 2);
    tick(&mut s, &down(0.0));
    assert_eq!(s.total, score::BACKFLIP_POINTS * F(3.0), "10000 and then 10000 × 2");
}

/// Started and crashed: the flip goes with the combo.
#[test]
fn a_flip_crashed_scores_nothing() {
    let mut s = Score { flip_at_start: true, ..Score::default() };
    tick(&mut s, &aloft(0.0, 0));
    tick(&mut s, &aloft(10.0, score::BACKFLIP));
    assert_eq!(s.combo_points, score::BACKFLIP_POINTS);
    s.wrecked();
    let wreck = Sense { wrecked: true, ..down(120.0) };
    for _ in 0..30 {
        tick(&mut s, &wreck);
    }
    assert_eq!(s.total, F::ZERO, "a crashed flip kept its points");
    assert!(s.nodes.is_empty() && !s.combo);
}

/// A trick growing when the flip starts goes on growing, the flip under it.
#[test]
fn a_trick_under_way_keeps_growing_through_the_flip() {
    let mut s = Score { flip_at_start: true, ..Score::default() };
    let air = aloft(0.0, 0);
    s.start(3, &air);
    for _ in 0..10 {
        tick(&mut s, &air);
    }
    tick(&mut s, &aloft(10.0, score::BACKFLIP));
    assert_eq!(s.nodes.iter().map(|n| n.id).collect::<Vec<_>>(), [3, score::BACKFLIP]);
    let held = s.nodes[0].points;
    tick(&mut s, &aloft(20.0, score::BACKFLIP));
    assert!(s.scoring && s.growing && held < s.nodes[0].points, "the trick stopped growing");
    assert_eq!(s.nodes[1].points, score::BACKFLIP_POINTS * F(2.0), "the flip is the combo's second");
}

/// The port's front flip (MD16) on its own watch: on the line at its start with the pack's points,
/// once, and banked.
#[test]
fn a_front_flip_shows_at_its_start() {
    let points = F(12000.0);
    let mut s = Score { flip_at_start: true, ..Score::default() };
    let mut watch = 0u8;
    let front = |deg: f32, flipping: u8| aloft(-deg, flipping);
    s.frame_with(DT, &front(0.0, 0), Some((&mut watch, points)));
    let mut deg = 0.0;
    let mut shown = None;
    for k in 1..=36 {
        deg += 10.0;
        s.frame_with(DT, &front(deg, score::FRONTFLIP), Some((&mut watch, points)));
        s.grow(DT);
        if shown.is_none() && !s.nodes.is_empty() {
            shown = Some(k);
        }
    }
    assert_eq!(shown, Some(1));
    assert_eq!(s.nodes.iter().map(|n| (n.id, n.points)).collect::<Vec<_>>(), [(score::FRONTFLIP, points)]);
    assert_eq!(watch, 0, "the watch came round to score it again");
    s.frame_with(DT, &down(0.0), Some((&mut watch, points)));
    assert_eq!(s.total, points);
}

// ---------------------------------------------------------------------------------------------------
// a whole ride
// ---------------------------------------------------------------------------------------------------

/// The air's numbers and wreck limits past a full turn, as `tests/rotation_modifier.rs` has them —
/// the test's own, not the disc's.
const AIR: &str = "
[Airborne Control]
AirborneDamping = 400
AirborneControlForce = 40
AirborneControlMoment = -400.0, 1500.0, -100.0
BrakeGyroTorque = 300
UnrollGain = 3
UnrollTorque = 50
UnyawGain = 3
UnyawTorque = 400
[Backflip]
MaxBackflipTorque = 1400
MinBackflipTorque = 800
BackflipTorqueRateIncrease = 500
BackflipTorqueRateDecrease = 2000
MinBackflipTime = 0
MaxBackflipTime = 0
[Wrecks]
MaxWreckTime = 2
CannonballWreckTime = 5
FrontWreckSpeedLimit = 400
BackWreckSpeedLimit = 400
SideWreckSpeedLimit = 400
RacingDownWreckSpeedLimit = 40
FreestyleDownWreckSpeedLimit = 400
GroundPitchDownWreckAngle = 60
GroundPitchUpWreckAngle = 60
AirbornePitchDownWreckAngle = 400
AirbornePitchUpWreckAngle = 400
AirborneRollWreckAngle = 400
NoHanderWreckSpeedLimit = 400
NoHanderMaxRollAngle = 45
JoustWreckHeight = 2
CrossupWreck = 1
CrossupWreckTime = 0.3
";

/// Flat ground up to a ramp of slope 1/2, then a drop to flat ground [`DROP`] ft below.
struct Jump;

const RAMP_AT: f32 = 108.0;
const RAMP_LONG: f32 = 12.0;
const DROP: f32 = 60.0;

impl World for Jump {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        let t = at.z() - RAMP_AT;
        if t < 0.0 {
            (Vf::new(at.x(), 0.0, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0))
        } else if t <= RAMP_LONG {
            let n = 1.0 / (1.0f32 + 0.25).sqrt();
            (Vf::new(at.x(), t * 0.5, at.z(), 1.0), Vf::new(0.0, n, -0.5 * n, 0.0))
        } else {
            (Vf::new(at.x(), -DROP, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0))
        }
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

/// The stand-in bike with [`AIR`] and a drag low enough to make the lip's 30 ft/s.
fn sim() -> Sim {
    let tables = Arc::new(Tables::build());
    let mut text = stock::BIKE.to_string();
    assert!(text.contains("Drag = 0.5"), "game_sim::stock::BIKE no longer has `Drag = 0.5`");
    text = text.replace("Drag = 0.5", "Drag = 0.037");
    let phy = Phy::read_bytes(format!("{text}{AIR}").as_bytes(), &tables.ride.curve);
    let geom = Phy::read_bytes(stock::BIKE.as_bytes(), &tables.ride.curve);
    let rider = Rider::new(&phy, stock::mounts(&geom), stock::SPAWN, stock::FACING, Set::default());
    let mut sim = Sim::with_rider(0, tables, phy, rider);
    sim.rider.sense.wrecks_enabled = true;
    sim
}

/// What one ride off the jump did, flat out with the backflip's gesture on the lip.
struct Flight {
    /// The tick the bike first turned the flip over, and whether the line had it that tick.
    started: Option<(usize, bool)>,
    /// The ticks the line showed a backflip on (any node of the open combo).
    shown_at: Option<usize>,
    /// The tick a start was told for a backflip.
    told: Option<usize>,
    landed: bool,
    wrecked: bool,
    total: F,
    /// How far the bike turned about its pitch, radians.
    turned: f32,
}

fn ride(at_start: bool) -> Flight {
    let mut s = sim();
    s.rider.score.flip_at_start = at_start;
    let mut f = Flight { started: None, shown_at: None, told: None, landed: false, wrecked: false, total: F::ZERO, turned: 0.0 };
    let mut lip_at = None;
    let mut aloft = false;
    for tick in 0..1200 {
        let ride = &s.rider.ride;
        let lip = ride.air.on_ground && ride.chassis.vel.y() > 5.0 && ride.air.contact_vel.z() > 30.0;
        if lip_at.is_none() && lip {
            lip_at = Some(tick);
        }
        let y = match lip_at {
            Some(l) if tick < l + 2 => -1.0,
            Some(_) if !aloft || s.rider.ride.air.flipping => 1.0,
            _ => 0.0,
        };
        let c = Controls { gas: F::ONE, stick: (F::ZERO, F(y)), ..Controls::default() };
        let frame = s.step(DT, &c, &mut Jump);
        if frame.wreck.is_some() {
            f.wrecked = true;
            break;
        }
        if frame.started == Some(Trick::BACKFLIP) && f.told.is_none() {
            f.told = Some(tick);
        }
        let flipping = s.rider.ride.air.flipping;
        let on_line = s.rider.score.nodes.iter().any(|n| n.id == score::BACKFLIP);
        if flipping && f.started.is_none() {
            f.started = Some((tick, on_line));
        }
        if on_line && f.shown_at.is_none() {
            f.shown_at = Some(tick);
        }
        if f.started.is_some() {
            f.turned += s.rider.ride.chassis.ang_vel.x() * DT.0;
        }
        aloft |= !s.rider.ride.air.on_ground && f.started.is_some();
        if aloft && s.rider.ride.air.on_ground {
            f.landed = true;
        }
        // Ridden out past the landing and its lockout: banked or not by now.
        if f.landed && s.rider.ride.air.on_ground && !s.rider.score.combo && tick > lip_at.unwrap_or(0) + 200 {
            break;
        }
    }
    f.total = s.rider.score.total;
    f
}

/// On a whole ride: the backflip is on the line the first tick the bike turns it over, told as a
/// start that tick, and the flip landed banks its 10 000.
#[test]
fn a_ridden_backflip_shows_at_its_start_and_banks_landed() {
    let f = ride(true);
    println!(
        "started {:?}, shown {:?}, told {:?}, turned {:.1}°, landed {}, wrecked {}, total {}",
        f.started,
        f.shown_at,
        f.told,
        f.turned.to_degrees(),
        f.landed,
        f.wrecked,
        f.total.0
    );
    let (start, on_line) = f.started.expect("the bike never turned a flip over");
    assert!(on_line, "the flip was not on the line the tick it started ({start})");
    assert_eq!(f.shown_at, Some(start));
    assert_eq!(f.told, Some(start), "the start was not told the tick it came");
    assert!(f.landed && !f.wrecked, "the flip did not land clean (turned {:.1}°)", f.turned.to_degrees());
    assert!(f.turned.abs() > 300f32.to_radians(), "turned only {:.1}°", f.turned.to_degrees());
    assert_eq!(f.total, score::BACKFLIP_POINTS, "the landed flip banked {}", f.total.0);

    // The original's card sees nothing at the start.
    let o = ride(false);
    assert_eq!(o.started.map(|s| s.1), Some(false), "the original's card had the flip at its start");
}
