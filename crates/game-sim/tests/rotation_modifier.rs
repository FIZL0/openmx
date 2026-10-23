//! P96 — the rotation-speed modifier, [`Knob::Rotation`]: one factor that every rotation in the air
//! turns at, not a torque a trick at a time.
//!
//! What it holds:
//! - a backflip off a lip, a front flip off the stick and a 360 off the stick each come a full turn
//!   round in about 2/3 of the ticks at 150% (and in more at 70%) — the bike ridden whole, headless,
//!   by [`Sim`], over a world with nothing under it once it is in the air; and so do the trick-pack
//!   moves MD16 added, the R1 front flip off a lip and MX vs. ATV's R1 360;
//! - every [`game_sim::air::Rotation`] row there is, the disc's backflip and the port's own ([`game_sim::air::INVENTED`],
//!   [`UNLEASHED_ROTATIONS`]' front flip), and the 360's [`SpinTuning`] ([`SpinTuning::tuned`]),
//!   reads its numbers through the one hook ([`game_sim::air::Rotation::tuned`]): a row added later is a row
//!   added to [`game_sim::air::Rotations::new`]'s set, and this test's `every_row_reads_the_factor` covers it
//!   with no change; a rotation that is not a row (as the stick's are) plugs in by reading
//!   [`game_sim::air::Params`] or the chassis' windows, which the factor has already moved;
//! - at 100% nothing at all is touched: the bike's air numbers and the chassis' angular limits are
//!   the file's, bit for bit, and the old `spin_rate` word still reads as the torque knob it was.
//!
//! The bike is the stand-in ([`stock::BIKE`]) with air sections written here — the stand-in has none,
//! and these are the test's own numbers, not the disc's — and wreck angles past a full turn, so a
//! flip in the air is never a crash.

use game_sim::air::{RotationSpeed, Rotations, SpinTuning, UNLEASHED_ROTATIONS};
use game_sim::collide::{Bundle, Record};
use game_sim::modifiers::{Knob, Mods};
use game_sim::phy::Phy;
use game_sim::ride::Controls;
use game_sim::rider::{Rider, Tables};
use game_sim::trick::Set;
use game_sim::world::World;
use game_sim::{Sim, stock};
use ps2_float::{F, Vf};
use std::f32::consts::TAU;
use std::sync::Arc;

/// The shell's fixed update's frame.
const DT: F = F(1.0 / 60.0);

/// The air's numbers, the test's own. The yaw moment is big enough that the yaw window
/// (`YawRateLimit`, below) is what holds a 360, so the window has to move with the factor too.
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

/// The stand-in bike, a yaw window of 2 rad/s, and [`AIR`]; with `fast`, `tests/ride.rs`'s drag, as
/// the stand-in's own tops out under the lip's 30 ft/s. (A drop is left on the stand-in's drag: on
/// the low one, seconds of falling end in a fore-and-aft load past any wreck limit.)
fn bike(fast: bool) -> String {
    let mut out = stock::BIKE.to_string();
    let drag = if fast { "Drag = 0.037" } else { "Drag = 0.5" };
    for (key, with) in [("Inertia = 22.4, 18.4, 10.9\n", "Inertia = 22.4, 18.4, 10.9\nYawRateLimit = 2\n"), ("Drag = 0.5", drag)] {
        assert!(out.contains(key), "game_sim::stock::BIKE no longer has `{}`", key.trim_end());
        out = out.replace(key, with);
    }
    format!("{out}{AIR}")
}

/// Flat ground up to a ramp of slope 1/2, [`RAMP_AT`] ft out and [`RAMP_LONG`] long, and nothing at
/// all past it: a bike that leaves the ramp falls for as long as the test cares to watch.
struct Cliff;

const RAMP_AT: f32 = 108.0;
const RAMP_LONG: f32 = 12.0;

impl World for Cliff {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        let t = at.z() - RAMP_AT;
        if t < 0.0 {
            (Vf::new(at.x(), 0.0, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0))
        } else if t <= RAMP_LONG {
            let n = 1.0 / (1.0f32 + 0.25).sqrt();
            (Vf::new(at.x(), t * 0.5, at.z(), 1.0), Vf::new(0.0, n, -0.5 * n, 0.0))
        } else {
            (Vf::new(at.x(), -1.0e6, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0))
        }
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

/// A set with [`Knob::Rotation`] at `pct` for every player.
fn at(pct: u16) -> Mods {
    let mut mods = Mods::original();
    for seat in 0..game_sim::MAX_PLAYERS {
        mods.set(seat, Knob::Rotation, pct);
    }
    mods
}

/// A trick pack's `[Three Sixty]`, the test's own numbers, and its stunt table's two 360 rows.
const THREE_SIXTY: &str = "
[Three Sixty]
MaxThreeSixtyTorqueX=300
MinThreeSixtyTorqueX=200
MaxThreeSixtyTorqueY=1100
MinThreeSixtyTorqueY=200
ThreeSixtyTorqueRateIncrease=480
ThreeSixtyTorqueRateDecrease=1960
MinThreeSixtyTime=0
MaxThreeSixtyTime=60
";
const TABLE: &str = "
[Stunt01]
Name=threesixtyL
Duration=1.5
HoldPointValue=15000
[Stunt02]
Name=threesixtyR
Duration=1.5
HoldPointValue=15000
";

/// The trick pack's moves, laid over a bike as the shell lays them: before the modifiers.
fn pack(phy: &mut Phy) {
    use game_sim::trick_pack;
    let rows = trick_pack::table(TABLE.as_bytes()).expect("the test's table reads");
    let ini = game_sim::phy::Ini::parse(THREE_SIXTY);
    let mut moves = trick_pack::moves(&ini, &rows).expect("the test's file has a [Three Sixty]");
    // The gestures here are MX vs. ATV's step; MD25's easy 360 (the default) would fire on the arm push.
    moves.spin_style = game_sim::air::SpinStyle::MxVsAtv;
    trick_pack::apply(phy, &ini, moves);
}

/// Player 1's bike on `mods`, loaded as the shell loads it, stood at `spawn`.
fn sim(mods: &Mods, spawn: Vf, fast: bool) -> Sim {
    sim_with(mods, spawn, fast, false)
}

/// [`sim`], with the trick pack's moves on the bike when `tricks`.
fn sim_with(mods: &Mods, spawn: Vf, fast: bool, tricks: bool) -> Sim {
    let tables = Arc::new(Tables::build());
    let text = bike(fast);
    let mut phy = Phy::read_bytes(text.as_bytes(), &tables.ride.curve);
    if tricks {
        pack(&mut phy);
    }
    mods.bike(0, &mut phy);
    let geom = Phy::read_bytes(stock::BIKE.as_bytes(), &tables.ride.curve);
    let mut rider = Rider::new(&phy, stock::mounts(&geom), spawn, stock::FACING, Set::default());
    mods.ride(0, &mut rider.ride);
    Sim::with_rider(0, tables, phy, rider)
}

/// From the tick `from` says the rotation has started, the angular velocity's `lane` (in the bike's
/// own frame) summed over the ticks until it has come a full turn round: how many ticks that took,
/// and the turn's sign. `None` if it never did in `most` ticks.
fn full_turn(
    sim: &mut Sim,
    world: &mut dyn World,
    most: usize,
    lane: usize,
    hands: &mut dyn FnMut(&Sim, usize) -> Controls,
    from: &dyn Fn(&Sim) -> bool,
) -> Option<(usize, f32)> {
    let (mut start, mut turned) = (None, 0.0f32);
    for tick in 0..most {
        let c = hands(sim, tick);
        let frame = sim.step(DT, &c, world);
        assert!(frame.wreck.is_none(), "the bike wrecked at tick {tick}: {:?}", frame.wreck);
        if start.is_none() && from(sim) {
            start = Some(tick);
        }
        if let Some(s) = start {
            turned += sim.rider.ride.chassis.ang_vel.0[lane] * DT.0;
            if turned.abs() >= TAU {
                return Some((tick - s + 1, turned.signum()));
            }
        }
    }
    None
}

/// The backflip off the ramp: flat out from rest, and on the lip the stick forward for two ticks
/// (arming it) and then back and held there (firing it, and keeping its torque up). Counted from the
/// first tick its torque turns the bike.
fn backflip(mods: &Mods) -> (usize, f32) {
    let mut s = sim(mods, stock::SPAWN, true);
    let mut lip_at = None;
    let mut hands = |s: &Sim, tick: usize| {
        let ride = &s.rider.ride;
        let lip = ride.air.on_ground && ride.chassis.vel.y() > 5.0 && ride.air.contact_vel.z() > 30.0;
        if lip_at.is_none() && lip {
            lip_at = Some(tick);
        }
        let y = match lip_at {
            Some(l) if tick < l + 2 => -1.0,
            Some(_) => 1.0,
            None => 0.0,
        };
        Controls { gas: F::ONE, stick: (F::ZERO, F(y)), ..Controls::default() }
    };
    full_turn(&mut s, &mut Cliff, 1800, 0, &mut hands, &|s| s.rider.ride.air.flipping)
        .unwrap_or_else(|| panic!("no backflip came round at {:?}", mods.text()))
}

/// Off the ramp flat out with R1 held, a gesture on the lip: `arm` for a few ticks, then `fire` held
/// (the stick and R1 keep the move's torque up). The MD16 moves on the trick pack. Counted from the
/// first tick `from` says the move is turning, about `lane`.
fn r1_off_the_lip(mods: &Mods, arm: (f32, f32), fire: (f32, f32), lane: usize, from: &dyn Fn(&Sim) -> bool) -> (usize, f32) {
    let mut s = sim_with(mods, stock::SPAWN, true, true);
    let mut lip_at = None;
    let mut hands = |s: &Sim, tick: usize| {
        let ride = &s.rider.ride;
        let lip = ride.air.on_ground && ride.chassis.vel.y() > 5.0 && ride.air.contact_vel.z() > 30.0;
        if lip_at.is_none() && lip {
            lip_at = Some(tick);
        }
        let (x, y) = match lip_at {
            Some(l) if tick < l + 2 => arm,
            Some(_) => fire,
            None => (0.0, 0.0),
        };
        Controls { gas: F::ONE, stick: (F(x), F(y)), trick: [false, false, true], ..Controls::default() }
    };
    full_turn(&mut s, &mut Cliff, 1800, lane, &mut hands, from)
        .unwrap_or_else(|| panic!("no R1 move about lane {lane} came round at {:?}", mods.text()))
}

/// The bike dropped from high up with the stick held one way from the start: a front flip with it
/// forward (`y` negative), a 360 with it to the side.
fn off_the_stick(mods: &Mods, stick: (f32, f32), lane: usize) -> (usize, f32) {
    let mut s = sim(mods, Vf::new(0.0, 500.0, 0.0, 1.0), false);
    let mut hands = |_: &Sim, _| Controls { stick: (F(stick.0), F(stick.1)), ..Controls::default() };
    full_turn(&mut s, &mut Cliff, 3600, lane, &mut hands, &|s| !s.rider.ride.air.on_ground)
        .unwrap_or_else(|| panic!("no turn about lane {lane} came round at {:?}", mods.text()))
}

/// 150% against 100%: about 2/3 the ticks, and 70% more ticks than 100%.
fn faster(name: &str, run: &dyn Fn(&Mods) -> (usize, f32)) -> f32 {
    let (stock, way) = run(&Mods::original());
    let (fast, fast_way) = run(&at(150));
    let (slow, slow_way) = run(&at(70));
    let ratio = fast as f32 / stock as f32;
    println!("{name}: {stock} ticks a turn at 100%, {fast} at 150% ({ratio:.3}), {slow} at 70%");
    assert_eq!((fast_way, slow_way), (way, way), "{name} turned the other way");
    assert!((0.6..=0.73).contains(&ratio), "{name} at 150% took {fast} ticks against {stock}: {ratio:.3}, not about 2/3");
    assert!(slow > stock, "{name} at 70% took {slow} ticks, no more than 100%'s {stock}");
    way
}

#[test]
fn a_backflip_turns_in_two_thirds_the_ticks_at_150() {
    let way = faster("backflip", &backflip);
    // The disc's backflip is a negative pitch.
    assert_eq!(way, -1.0);
}

#[test]
fn a_front_flip_turns_in_two_thirds_the_ticks_at_150() {
    let way = faster("front flip", &|m| off_the_stick(m, (0.0, -1.0), 0));
    assert_eq!(way, 1.0, "the stick forward turned the bike backwards");
}

#[test]
fn a_360_turns_in_two_thirds_the_ticks_at_150() {
    faster("360", &|m| off_the_stick(m, (1.0, 0.0), 1));
}

/// MD16's front flip, the port's own row under R1: stick back on the lip, then forward.
#[test]
fn the_r1_front_flip_turns_in_two_thirds_the_ticks_at_150() {
    let row = |s: &Sim| s.rider.ride.air.flipping && s.rider.ride.tricks.as_ref().is_some_and(|t| t.flip_row == 1);
    let way = faster("R1 front flip", &|m| r1_off_the_lip(m, (0.0, 1.0), (0.0, -1.0), 0, &row));
    assert_eq!(way, 1.0, "the front flip turned the bike backwards");
}

/// MX vs. ATV's 360 off a trick pack: R1 held, stick left then right on the lip.
#[test]
fn the_r1_360_turns_in_two_thirds_the_ticks_at_150() {
    let spun = |s: &Sim| s.rider.ride.tricks.as_ref().is_some_and(|t| t.spin.applied);
    faster("R1 360", &|m| r1_off_the_lip(m, (-1.0, 0.0), (1.0, 0.0), 1, &spun));
    // Both 360s are held by the yaw window (`YawRateLimit`, moved with the factor), so the turn
    // alone does not show the 360's own torque moved: the torque it fires with is k^2 as big.
    let fired = |mods: &Mods| {
        let torque = std::cell::Cell::new(None);
        r1_off_the_lip(mods, (-1.0, 0.0), (1.0, 0.0), 1, &|s: &Sim| {
            let t = s.rider.ride.tricks.as_ref().filter(|t| t.spin.applied);
            if let (None, Some(t)) = (torque.get(), t) {
                torque.set(Some(t.spin.torque.y()));
            }
            t.is_some()
        });
        torque.get().expect("the 360 fired")
    };
    let (stock, fast) = (fired(&Mods::original()), fired(&at(150)));
    assert_eq!(fast, (F(stock) * (F(1.5) * F(1.5))).0, "the 360 fired with {fast}, not 2.25 x {stock}");
}

/// Every row there is — the disc's backflip and the port's own — reads its numbers through
/// [`game_sim::air::Rotation::tuned`], and so turns at the factor's speed: torques `k^2`, their rates `k^3`.
/// A row added to the set is checked here as it stands.
#[test]
fn every_row_reads_the_factor() {
    let tables = Tables::build();
    let plain = Phy::read_bytes(bike(true).as_bytes(), &tables.ride.curve);
    let mut phy = plain.clone();
    at(150).bike(0, &mut phy);
    assert_eq!(phy.air.rotation_speed, Some(RotationSpeed(F(1.5))));
    let k = F(1.5);
    let rows = Rotations::new(true);
    assert!(rows.rows().len() > 1, "the port's own rows are not in the set");
    for row in rows.rows().iter().chain(UNLEASHED_ROTATIONS) {
        let was = row.tuned(&plain.air);
        let now = row.tuned(&phy.air);
        assert_eq!(now.min_torque, was.min_torque * (k * k), "{}", row.name);
        assert_eq!(now.max_torque, was.max_torque * (k * k), "{}", row.name);
        assert_eq!(now.rate_increase, was.rate_increase * (k * k * k), "{}", row.name);
        assert_eq!(now.rate_decrease, was.rate_decrease * (k * k * k), "{}", row.name);
        assert_eq!((now.min_time, now.max_time), (was.min_time, was.max_time), "{}", row.name);
    }
    // The backflip's keys themselves are the file's: the row scales them, once.
    assert_eq!(phy.air.max_backflip_torque, plain.air.max_backflip_torque);
    // MX vs. ATV's 360 is not a row: its own tuning reads the same factor, the same way.
    let mut packed = plain.clone();
    pack(&mut packed);
    let t: SpinTuning = packed.tricks.expect("the pack's moves").spin;
    let now = t.tuned(&phy.air);
    for (a, b) in [(now.min_torque_x, t.min_torque_x), (now.max_torque_x, t.max_torque_x), (now.min_torque_y, t.min_torque_y), (now.max_torque_y, t.max_torque_y)] {
        assert_eq!(a, b * (k * k));
    }
    assert_eq!((now.rate_increase, now.rate_decrease), (t.rate_increase * (k * k * k), t.rate_decrease * (k * k * k)));
    assert_eq!((now.min_time, now.max_time), (t.min_time, t.max_time));
}

/// At 100% nothing is touched: the air numbers, every row's tuning and the chassis' angular limits
/// are the file's to the bit — and so are the ride's with any other knob alone off its default.
#[test]
fn at_one_hundred_percent_nothing_moves() {
    let tables = Tables::build();
    let plain = Phy::read_bytes(bike(true).as_bytes(), &tables.ride.curve);
    let mut phy = plain.clone();
    at(100).bike(0, &mut phy);
    assert_eq!(format!("{:?}", phy.air), format!("{:?}", plain.air));
    assert_eq!(phy.air.rotation_speed, None);
    for row in Rotations::new(true).rows().iter().chain(UNLEASHED_ROTATIONS) {
        assert_eq!(format!("{:?}", row.tuned(&phy.air)), format!("{:?}", (row.tuning)(&plain.air)));
    }
    let mut packed = plain.clone();
    pack(&mut packed);
    let t = packed.tricks.expect("the pack's moves").spin;
    assert_eq!(format!("{:?}", t.tuned(&phy.air)), format!("{t:?}"));
    let geom = Phy::read_bytes(stock::BIKE.as_bytes(), &tables.ride.curve);
    let mut rider = Rider::new(&phy, stock::mounts(&geom), stock::SPAWN, stock::FACING, Set::default());
    let c = rider.ride.chassis.clone();
    at(100).ride(0, &mut rider.ride);
    let d = &rider.ride.chassis;
    assert_eq!(d.max_ang_speed_sq.0.to_bits(), c.max_ang_speed_sq.0.to_bits());
    for i in 0..3 {
        assert_eq!(d.ang_vel_max[i].0.to_bits(), c.ang_vel_max[i].0.to_bits());
        assert_eq!(d.ang_vel_min[i].0.to_bits(), c.ang_vel_min[i].0.to_bits());
    }
    // And at 150% the windows are wider by the factor: the yaw's is the file's 2 rad/s.
    at(150).ride(0, &mut rider.ride);
    assert_eq!(rider.ride.chassis.ang_vel_max[1], F(3.0));
}

/// The old word still reads as the torque knob it was, and the new one is its own: a preset or a
/// tape made with `spin_rate` rides the numbers it was made with.
#[test]
fn spin_rate_still_reads_as_the_torque_it_was() {
    let old = Mods::parse("spin_rate=150").unwrap();
    assert_eq!(old.get(0, Knob::SpinRate), 150);
    assert_eq!(old.get(0, Knob::Rotation), 100);
    assert!(!Knob::MENU.contains(&Knob::SpinRate) && Knob::MENU.contains(&Knob::Rotation));
    let new = Mods::parse("rotation_speed=150").unwrap();
    assert_eq!(new, at(150));
    assert_eq!(new.text(), "rotation_speed=150");
    assert_eq!(Knob::Rotation.label(), "Flip rotation speed");

    let tables = Tables::build();
    let plain = Phy::read_bytes(bike(true).as_bytes(), &tables.ride.curve);
    let mut phy = plain.clone();
    old.bike(0, &mut phy);
    assert_eq!(phy.air.max_backflip_torque, plain.air.max_backflip_torque * F(1.5));
    assert_eq!(phy.air.rotation_speed, None);
    assert_eq!(format!("{:?}", phy.air.airborne_control_moment), format!("{:?}", plain.air.airborne_control_moment));
}

