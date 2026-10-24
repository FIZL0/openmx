//! P55 — the game modifiers: multipliers on what a ride loads, applied as the shell applies them.
//!
//! The shell's `ride` reads the bike's `.phy`, hands it to [`Mods::bike`] before the rider is built,
//! to [`Mods::ride`] once it is, the biplane's file to [`Mods::plane`] before its flight, and the
//! engine its time scale off [`Mods::time_scale`]. These tests do the same on the stand-in bike
//! ([`stock::BIKE`], with `tests/ride.rs`'s two rideable patches), headless, with no disc; the
//! biplane's is the disc's and skips without one.
//!
//! What they hold:
//! - every modifier at its default touches nothing: the loaded bike, the built ride and the biplane
//!   are bit for bit what they were, a ride of them is the same ride tick for tick, and a tape of it
//!   is written exactly as before P55;
//! - gravity ×0.5 halves the fall's acceleration over a tick;
//! - bike speed ×2 raises the top speed;
//! - a tape made with modifiers says so, reads back with them, and replays the ride it was made on
//!   only when they are applied;
//! - every modifier off its default reaches something the ride loads.

use game_sim::collide::{Bundle, Record};
use game_sim::modifiers::{self, Knob, Mods};
use game_sim::phy::Phy;
use game_sim::replay::{PadFrame, PadTape};
use game_sim::ride::Controls;
use game_sim::rider::{Rider, Tables};
use game_sim::trick::Set;
use game_sim::world::World;
use game_sim::{Sim, stock};
use ps2_float::{F, Vf};
use std::sync::Arc;

/// The shell's fixed update's frame.
const DT: F = F(1.0 / 60.0);

/// `tests/ride.rs`'s wreck limits: with none at all every landing is a wreck, and with no wreck body
/// to tumble a wrecked bike stays where it is.
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

/// The air's sections the stand-in bike has none of, for the test that every modifier reaches a
/// number: a modifier on a zero is still a zero. Plausible numbers, not the disc's.
const AIR: &str = "
[Airborne Control]
AirborneControlForce = 40
[Preload]
PreloadTime = 0.2
MaxPreload = 300
[Backflip]
MinBackflipTorque = 100
MaxBackflipTorque = 400
BackflipTorqueRateIncrease = 50
BackflipTorqueRateDecrease = 50
";

/// `tests/ride.rs`'s `ridden_bike`: the stand-in bike with a steering scale, the drag that lets it
/// reach the original's top speed, and the wreck limits of a bike being ridden.
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

/// Flat ground at y = 0, nothing solid on it.
struct Flat;

impl World for Flat {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        (Vf::new(at.x(), 0.0, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0))
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

/// A ride on `mods`, as the shell loads one for player `seat`: the file, the modifiers on it, the
/// rider built from it, the ride's own limit after. Started `height` feet up.
fn sim(mods: &Mods, seat: usize, height: f32) -> Sim {
    let tables = Arc::new(Tables::build());
    let mut phy = Phy::read_bytes(ridden_bike().as_bytes(), &tables.ride.curve);
    mods.bike(seat, &mut phy);
    let geom = Phy::read_bytes(stock::BIKE.as_bytes(), &tables.ride.curve);
    let at = Vf::new(stock::SPAWN.x(), stock::SPAWN.y() + height, stock::SPAWN.z(), 1.0);
    let mut rider = Rider::new(&phy, stock::mounts(&geom), at, stock::FACING, Set::default());
    mods.ride(seat, &mut rider.ride);
    Sim::with_rider(0, tables, phy, rider)
}

/// What the hands do on tick `i`: full gas, a slow weave of the stick, a brake late on.
fn hands(i: usize) -> Controls {
    let weave = ((i / 90) % 3) as f32 - 1.0;
    Controls {
        gas: F(if i < 600 { 1.0 } else { 0.0 }),
        brake: F(if i >= 600 { 1.0 } else { 0.0 }),
        stick: (F(0.4 * weave), F(0.0)),
        ..Controls::default()
    }
}

/// Everything about the ride a player would notice and a good deal they would not, as bits.
fn state(sim: &Sim) -> Vec<u32> {
    let ride = &sim.rider.ride;
    let m = ride.transform();
    let mut out: Vec<u32> = (0..4).flat_map(|r| (0..4).map(move |c| (r, c))).map(|(r, c)| m.0[r].0[c].to_bits()).collect();
    out.extend((0..4).map(|c| ride.chassis.vel.0[c].to_bits()));
    out.push(ride.chassis.speed.0.to_bits());
    out.push(ride.air.on_ground as u32);
    out
}

/// `ticks` ticks of [`hands`] on `sim`, each tick's `dt` the time scale's, and the state after each.
fn ride(sim: &mut Sim, mods: &Mods, ticks: usize) -> Vec<Vec<u32>> {
    let dt = modifiers::scaled_dt(DT, mods.time_scale());
    (0..ticks)
        .map(|i| {
            sim.step(dt, &hands(i), &mut Flat);
            state(sim)
        })
        .collect()
}

/// A set with `knob` at `pct` for every player.
fn with(knob: Knob, pct: u16) -> Mods {
    let mut mods = Mods::original();
    for seat in 0..game_sim::MAX_PLAYERS {
        mods.set(seat, knob, pct);
    }
    mods
}

/// The disc's `biplane.phy`, or `None` and a note without a disc.
fn the_discs_plane() -> Option<game_sim::phy::Plane> {
    use game_render::disc::Disc;
    let mut disc = match Disc::open_configured() {
        Ok(disc) => disc,
        Err(e) => {
            eprintln!("skipped: {e}");
            return None;
        }
    };
    let name = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("machine.res"))?.name.clone();
    let res = disc.res(&name).expect("machine.res");
    let bytes = res.read_suffix(game_sim::phy::BIPLANE_PHY).expect("machine.res carries biplane.phy");
    Some(game_sim::phy::Plane::read_bytes(&bytes))
}

#[test]
fn every_modifier_at_its_default_leaves_the_ride_bit_for_bit() {
    let tables = Tables::build();
    let stock_phy = Phy::read_bytes(ridden_bike().as_bytes(), &tables.ride.curve);
    // A set that has been moved and moved back is the original too.
    let mut moved = Mods::original();
    for knob in Knob::ALL {
        moved.set(2, knob, 250);
        moved.set(2, knob, modifiers::ORIGINAL);
    }
    assert_eq!(moved, Mods::original());
    assert!(Mods::original().is_original() && Mods::original().text().is_empty());
    assert_eq!(Mods::original().time_scale(), None, "the engine's time scale stays off");
    for seat in 0..game_sim::MAX_PLAYERS {
        let mut phy = stock_phy.clone();
        moved.bike(seat, &mut phy);
        // Debug prints every float at the precision that tells its bits apart.
        assert_eq!(format!("{phy:?}"), format!("{stock_phy:?}"), "player {}'s bike was touched", seat + 1);
    }
    // The ride built on it, its speed limit included.
    let (a, b) = (sim(&Mods::original(), 0, 0.0), {
        let tables = Arc::new(Tables::build());
        let phy = Phy::read_bytes(ridden_bike().as_bytes(), &tables.ride.curve);
        let geom = Phy::read_bytes(stock::BIKE.as_bytes(), &tables.ride.curve);
        let rider = Rider::new(&phy, stock::mounts(&geom), stock::SPAWN, stock::FACING, Set::default());
        Sim::with_rider(0, tables, phy, rider)
    });
    assert_eq!(a.rider.ride.chassis.max_speed_sq.0.to_bits(), b.rider.ride.chassis.max_speed_sq.0.to_bits());
    // And twenty seconds of riding it are the same twenty seconds, tick for tick.
    let (mut a, mut b) = (a, b);
    let one = ride(&mut a, &Mods::original(), 1200);
    let two: Vec<Vec<u32>> = (0..1200)
        .map(|i| {
            b.step(DT, &hands(i), &mut Flat);
            state(&b)
        })
        .collect();
    for (i, (x, y)) in one.iter().zip(&two).enumerate() {
        assert_eq!(x, y, "tick {i}: the ride on the original modifiers parted from the unmodified one");
    }
    // The ride went somewhere, or the comparison proves nothing.
    assert!(a.rider.ride.chassis.speed.0 > 1.0 || a.rider.ride.position().z().abs() > 100.0);
    // The time scale's dt at its default is the frame's own.
    for dt in [F(0.0), DT, F(1.0 / 30.0), F(0.0005)] {
        assert_eq!(modifiers::scaled_dt(dt, None).0.to_bits(), dt.0.to_bits());
    }
    // And the biplane's file, with the disc.
    if let Some(plane) = the_discs_plane() {
        let mut p = plane;
        moved.plane(0, &mut p);
        assert_eq!(p, plane, "the biplane was touched");
    }
}

#[test]
fn a_tape_on_the_original_modifiers_is_written_as_before() {
    let mut tape = PadTape::new("stock");
    tape.frames.push(PadFrame::new(DT, [game_sim::input::Report::idle(); game_sim::input::PADS]));
    let text = tape.to_text();
    assert!(!text.contains("modifiers"), "{text}");
    let back = PadTape::parse(&text).unwrap();
    assert!(back.modifiers.is_original());
    assert_eq!(back.to_text(), text);
}

#[test]
fn half_gravity_halves_the_falls_acceleration() {
    // High enough that a tick never reaches the ground; the bike starts at rest, so no drag either.
    let fall = |mods: &Mods| {
        let mut sim = sim(mods, 0, 500.0);
        sim.step(DT, &Controls::default(), &mut Flat);
        let v0 = sim.rider.ride.chassis.vel.y();
        sim.step(DT, &Controls::default(), &mut Flat);
        let v1 = sim.rider.ride.chassis.vel.y();
        assert!(!sim.rider.ride.air.on_ground, "the fall touched down");
        (v0, v1 - v0)
    };
    let (first, full) = fall(&Mods::original());
    let (first_half, half) = fall(&with(Knob::Gravity, 50));
    assert!(full < -0.1, "the stock bike did not fall: {full} ft/s over a tick");
    let ratio = half / full;
    assert!((ratio - 0.5).abs() < 0.01, "gravity x0.5: {half} ft/s a tick against {full}, x{ratio}");
    let ratio = first_half / first;
    assert!((ratio - 0.5).abs() < 0.01, "the first tick: {first_half} against {first}, x{ratio}");
}

#[test]
fn double_bike_speed_raises_the_top_speed() {
    let top = |mods: &Mods| {
        let mut sim = sim(mods, 0, 0.0);
        let full = Controls { gas: F(1.0), ..Controls::default() };
        let mut top = 0.0f32;
        for _ in 0..1800 {
            sim.step(DT, &full, &mut Flat);
            top = top.max(sim.rider.ride.chassis.speed.0);
        }
        top
    };
    let stock = top(&Mods::original());
    let fast = top(&with(Knob::BikeSpeed, 200));
    println!("top speed: {stock:.2} ft/s stock, {fast:.2} at bike speed x2");
    assert!(stock > 50.0, "the stock bike only reached {stock} ft/s");
    assert!(fast > stock * 1.5, "bike speed x2 reached {fast} ft/s against {stock}");
    // And the speed limit the ride itself keeps is out of the way.
    let mut s = sim(&with(Knob::BikeSpeed, 200), 0, 0.0);
    let limit = s.rider.ride.chassis.max_speed_sq.0.sqrt();
    assert!(limit > fast, "the integrator's limit {limit} is under the speed {fast}");
    s.step(DT, &Controls::default(), &mut Flat);
}

/// **The put-back keeps the raised limit** (P101): the bike's unwreck sets the riding speed limit
/// again, and on a bike made faster that is [`Mods::ride`]'s raised one, not the stock 135 ft/s — so a
/// rider at bike speed x2 who wrecks once still rides past 135 ft/s after it.
#[test]
fn double_bike_speed_passes_135_after_a_wreck_and_its_put_back() {
    let fast = with(Knob::BikeSpeed, 200);
    let mut s = sim(&fast, 0, 0.0);
    let raised = s.rider.ride.chassis.max_speed_sq;
    assert_eq!(raised.0, 135.0 * 135.0 * 4.0, "bike speed x2 did not raise the limit");
    // Down, the wreck clock on its limit: the next tick asks for the put-back (and a page, if the
    // seat has a screen, a few ticks later).
    s.rider.state.enter(game_sim::trick::Wreck::ATTITUDE);
    s.rider.state.counting = true;
    s.rider.state.elapsed = s.phy.wreck_times.max_wreck_time;
    assert!((0..30).any(|_| s.step(DT, &Controls::default(), &mut Flat).respawned), "the wreck was not put back");
    assert_eq!(s.rider.state.wreck, None);
    assert_eq!(s.rider.ride.chassis.max_speed_sq.0.to_bits(), raised.0.to_bits(), "the put-back took the limit back to stock");
    let full = Controls { gas: F(1.0), ..Controls::default() };
    let mut top = 0.0f32;
    for _ in 0..1800 {
        s.step(DT, &full, &mut Flat);
        top = top.max(s.rider.ride.chassis.speed.0);
    }
    println!("top speed after a wreck at bike speed x2: {top:.2} ft/s");
    assert!(top > 135.0, "after the put-back bike speed x2 tops out at {top} ft/s");
    // And a change of set in the pause moves the put-back's limit too, wrecked or not.
    let tables = Tables::build();
    let stock = Phy::read_bytes(ridden_bike().as_bytes(), &tables.ride.curve);
    s.rider.ride.chassis.max_speed_sq = game_sim::off_map::THROWN_MAX_SPEED * game_sim::off_map::THROWN_MAX_SPEED;
    with(Knob::BikeSpeed, 300).resim(&fast, 0, &stock, &mut s);
    assert_eq!(s.rider.ride.riding_speed_sq.0, 135.0 * 135.0 * 9.0, "the change left the put-back's limit");
    assert_eq!(s.rider.ride.chassis.max_speed_sq.0, 350.0 * 350.0, "the change moved a thrown bike's limit");
    Mods::original().resim(&with(Knob::BikeSpeed, 300), 0, &stock, &mut s);
    assert_eq!(s.rider.ride.riding_speed_sq.0, 135.0 * 135.0, "back on the original, the put-back's limit");
}

/// P134: two players on one machine ride on their own bike speeds. 2P's own top speed doubled
/// (the set 2P's page writes, `bike_speed.2=200`) from the start, and then 1P's raised from the pause
/// mid-ride the way the shell moves every seat onto a change (`Mods::resim` seat by seat): each seat
/// is on its own, 2P out-running 1P and 1P moved only once its own change came.
#[test]
fn two_local_seats_ride_on_their_own_bike_speeds() {
    let tables = Tables::build();
    let stock = Phy::read_bytes(ridden_bike().as_bytes(), &tables.ride.curve);
    let was = Mods::parse("bike_speed.2=200").unwrap();
    let mut seats = [sim(&was, 0, 0.0), sim(&was, 1, 0.0)];
    let full = Controls { gas: F(1.0), ..Controls::default() };
    let top = |seats: &mut [Sim; 2], ticks: usize| {
        let mut top = [0.0f32; 2];
        for _ in 0..ticks {
            for (s, sim) in seats.iter_mut().enumerate() {
                sim.step(DT, &full, &mut Flat);
                top[s] = top[s].max(sim.rider.ride.chassis.speed.0);
            }
        }
        top
    };
    let before = top(&mut seats, 1800);
    println!("top speeds, 1P on the original and 2P at x2: {before:?}");
    assert!(before[1] > before[0] * 1.5, "2P at bike speed x2 reached {} ft/s against 1P's {}", before[1], before[0]);
    // 1P's own from the pause: 1P at x3, 2P's untouched.
    let to = Mods::parse("bike_speed.1=300 bike_speed.2=200").unwrap();
    let limit = seats[1].rider.ride.chassis.max_speed_sq;
    for (s, sim) in seats.iter_mut().enumerate() {
        to.resim(&was, s, &stock, sim);
    }
    assert_eq!(seats[1].rider.ride.chassis.max_speed_sq.0.to_bits(), limit.0.to_bits(), "2P's bike moved with 1P's change");
    let after = top(&mut seats, 1800);
    println!("after 1P's change to x3: {after:?}");
    assert!(after[0] > before[1] * 1.2, "1P at x3 reached {} ft/s, 2P at x2 {}", after[0], before[1]);
    assert!((after[1] - before[1]).abs() < 0.05 * before[1], "2P's top speed moved: {} then {}", before[1], after[1]);
}

#[test]
fn a_tape_made_with_modifiers_replays_with_them() {
    let mods = Mods::parse("gravity=70 time_scale=150 bike_speed=130 grip.1=80").unwrap();
    assert!(!mods.is_original());
    // The ride, recorded as the shell records one: its dt and its hands each tick.
    let mut tape = PadTape::new("modified");
    tape.modifiers = mods;
    let mut made = sim(&mods, 0, 0.0);
    let dt = modifiers::scaled_dt(DT, mods.time_scale());
    assert!((dt.0 - DT.0 * 1.5).abs() < 1e-6, "time x1.5 is a frame of {}", dt.0);
    let mut seen = Vec::new();
    for i in 0..900 {
        tape.frames.push(PadFrame::new(dt, [game_sim::input::Report::idle(); game_sim::input::PADS]));
        made.step(dt, &hands(i), &mut Flat);
        seen.push(state(&made));
    }
    // Written and read back, the modifiers come with it.
    let text = tape.to_text();
    assert!(text.lines().any(|l| l.starts_with("modifiers ")), "{}", &text[..200.min(text.len())]);
    let back = PadTape::parse(&text).unwrap();
    assert_eq!(back.modifiers, mods);
    assert_eq!(back.to_text(), text);
    // Played on the tape's own modifiers and its own dt, it is the same ride.
    let mut played = sim(&back.modifiers, 0, 0.0);
    for (i, frame) in back.frames.iter().enumerate() {
        played.step(frame.dt(), &hands(i), &mut Flat);
        assert_eq!(state(&played), seen[i], "tick {i}: the replay parted from the ride");
    }
    // And on the original game's it is not, which is why the tape carries them.
    let mut plain = sim(&Mods::original(), 0, 0.0);
    for (i, frame) in back.frames.iter().enumerate() {
        plain.step(frame.dt(), &hands(i), &mut Flat);
    }
    assert_ne!(state(&plain), seen[seen.len() - 1], "the modifiers made no difference to the ride");
}

#[test]
fn every_modifier_off_its_default_reaches_the_ride() {
    let tables = Tables::build();
    let stock_phy = Phy::read_bytes(format!("{}{AIR}", ridden_bike()).as_bytes(), &tables.ride.curve);
    let plane = the_discs_plane();
    for knob in Knob::ALL {
        for pct in [knob.least().max(10), 150] {
            let mods = with(knob, pct);
            assert!(!mods.is_original());
            assert_eq!(Mods::parse(&mods.text()).unwrap(), mods, "{knob:?} at {pct} does not read back");
            let mut phy = stock_phy.clone();
            mods.bike(0, &mut phy);
            let bike = format!("{phy:?}") != format!("{stock_phy:?}");
            let time = mods.time_scale().is_some();
            let flown = plane.is_some_and(|p| {
                let mut q = p;
                mods.plane(0, &mut q);
                q != p
            });
            match knob {
                Knob::TimeScale => assert!(time && !bike, "{knob:?} at {pct}"),
                Knob::PlaneSpeed | Knob::PlaneAccel | Knob::PlaneTopSpeed | Knob::PlaneTurn | Knob::PlaneCeiling => assert!(!bike && (flown || plane.is_none()), "{knob:?} at {pct}"),
                _ => assert!(bike && !time, "{knob:?} at {pct} left the bike alone"),
            }
        }
    }
    // Per player: 2P's set reaches 2P's bike and not 1P's.
    let mut mods = Mods::original();
    mods.set(1, Knob::Grip, 50);
    let (mut one, mut two) = (stock_phy.clone(), stock_phy.clone());
    mods.bike(0, &mut one);
    mods.bike(1, &mut two);
    assert_eq!(format!("{one:?}"), format!("{stock_phy:?}"));
    assert_ne!(format!("{two:?}"), format!("{stock_phy:?}"));
    // A world modifier is everyone's whichever seat is named.
    let mut mods = Mods::original();
    mods.set(5, Knob::Gravity, 50);
    assert_eq!(mods.get(0, Knob::Gravity), 50);
}
