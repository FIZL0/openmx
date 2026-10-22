//! P98 — the game modifiers changed in the ride's pause take when the pause closes, from where the
//! ride is.
//!
//! The shell's `remodify` hands each seat's ride to [`Mods::resim`] with the seat's stock `.phy` (and
//! its biplane to [`Mods::replane`] with the stock `biplane.phy`), sets the engine's time scale, and a
//! tape being written takes the change as an event at its tick ([`Recorder::modifiers`]); a played
//! tape puts it on again before that tick ([`Head::change`]). These tests do the same headless on
//! `tests/modifiers.rs`'s stand-in bike, with no disc; the biplane's is the disc's and skips without
//! one.
//!
//! What they hold:
//! - a ride moved onto a set before its first tick rides, tick for tick and bit for bit, the ride
//!   built on that set — for every modifier, both ways, so nothing the ride copied out of the file
//!   at its start is left on the old numbers;
//! - mid-ride the change moves nothing: the same place, the same velocity; from the next tick it
//!   rides on the new numbers (twice the top speed, half the fall — after the one tick the
//!   integrator's memory of the last acceleration blends);
//! - a tape recorded across a change says so, reads back with it, and replays bit for bit only
//!   when the change is put on again at its tick.

use game_sim::collide::{Bundle, Record};
use game_sim::input::{Control, PADS, Report};
use game_sim::modifiers::{self, Knob, Mods};
use game_sim::phy::Phy;
use game_sim::replay::{Head, PadTape, Recorder};
use game_sim::rider::{Rider, Tables};
use game_sim::trick::Set;
use game_sim::world::World;
use game_sim::{Sim, stock};
use ps2_float::{F, Vf};
use std::sync::Arc;

/// The shell's fixed update's frame.
const DT: F = F(1.0 / 60.0);

/// `tests/modifiers.rs`'s bike: `tests/ride.rs`'s `ridden_bike`, the stand-in with a steering scale,
/// the drag that lets it reach the original's top speed, and the wreck limits of a bike being ridden,
/// plus the air's sections the stand-in has none of (a modifier on a zero is still a zero).
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
    format!(
        "{out}
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
"
    )
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

/// The bike's file as the disc has it: what the shell keeps per seat to build a change from.
fn stock_phy(tables: &Tables) -> Phy {
    Phy::read_bytes(ridden_bike().as_bytes(), &tables.ride.curve)
}

/// A ride on `mods` as the shell loads one for player `seat`, `height` feet up.
fn sim(mods: &Mods, seat: usize, height: f32) -> Sim {
    let tables = Arc::new(Tables::build());
    let mut phy = stock_phy(&tables);
    mods.bike(seat, &mut phy);
    let geom = Phy::read_bytes(stock::BIKE.as_bytes(), &tables.ride.curve);
    let at = Vf::new(stock::SPAWN.x(), stock::SPAWN.y() + height, stock::SPAWN.z(), 1.0);
    let mut rider = Rider::new(&phy, stock::mounts(&geom), at, stock::FACING, Set::default());
    mods.ride(seat, &mut rider.ride);
    Sim::with_rider(0, tables, phy, rider)
}

/// The pad on tick `i`: Cross held (the gas) for the first ten seconds, a slow weave of the left
/// stick, nothing after.
fn pad(i: usize) -> [Report; PADS] {
    let mut r = if i < 600 { Report::from_buttons(Control::Cross.mask()) } else { Report::idle() };
    let weave = [0x50u8, 0x80, 0xb0][(i / 90) % 3];
    r.set_axis(2, weave);
    [r, Report::missing()]
}

/// One tick of the shell's pass: the pad, then the bike.
fn tick(sim: &mut Sim, dt: F, i: usize) {
    let c = sim.poll(dt, &pad(i));
    sim.step(dt, &c, &mut Flat);
}

/// The ride as bits: the whole state hash, and the transform and velocity beside it so a failure
/// says what moved.
fn state(sim: &Sim) -> (u64, Vec<u32>) {
    let ride = &sim.rider.ride;
    let m = ride.transform();
    let mut out: Vec<u32> = (0..4).flat_map(|r| (0..4).map(move |c| (r, c))).map(|(r, c)| m.0[r].0[c].to_bits()).collect();
    out.extend((0..4).map(|c| ride.chassis.vel.0[c].to_bits()));
    out.push(ride.chassis.speed.0.to_bits());
    (sim.hash(), out)
}

/// A set with `knob` at `pct` for every player.
fn with(knob: Knob, pct: u16) -> Mods {
    let mut mods = Mods::original();
    for seat in 0..game_sim::MAX_PLAYERS {
        mods.set(seat, knob, pct);
    }
    mods
}

/// `ticks` ticks from tick `from` on, at `mods`' frame, and the state after each.
fn ride(sim: &mut Sim, mods: &Mods, from: usize, ticks: usize) -> Vec<(u64, Vec<u32>)> {
    let dt = modifiers::scaled_dt(DT, mods.time_scale());
    (from..from + ticks)
        .map(|i| {
            tick(sim, dt, i);
            state(sim)
        })
        .collect()
}

#[test]
fn a_ride_moved_onto_a_set_at_its_start_is_the_ride_built_on_it() {
    let tables = Tables::build();
    let stock = stock_phy(&tables);
    let mut sets: Vec<Mods> = Knob::ALL.iter().flat_map(|&k| [with(k, k.least().max(50)), with(k, 200)]).collect();
    sets.push(Mods::parse("gravity=70 time_scale=150 bike_speed=130 grip.1=80 suspension=60 bike_power=140").unwrap());
    for to in sets {
        // Original → `to`, and `to` → original: the speed limit has to come back down too.
        for (was, to) in [(Mods::original(), to), (to, Mods::original())] {
            let mut moved = sim(&was, 0, 0.0);
            to.resim(&was, 0, &stock, &mut moved);
            let mut built = sim(&to, 0, 0.0);
            assert_eq!(format!("{:?}", moved.phy), format!("{:?}", built.phy), "{} → {}: the file", was.text(), to.text());
            assert_eq!(
                moved.rider.ride.chassis.max_speed_sq.0.to_bits(),
                built.rider.ride.chassis.max_speed_sq.0.to_bits(),
                "{} → {}: the speed limit",
                was.text(),
                to.text()
            );
            let a = ride(&mut moved, &to, 0, 900);
            let b = ride(&mut built, &to, 0, 900);
            for (i, (x, y)) in a.iter().zip(&b).enumerate() {
                assert_eq!(x, y, "{} → {}, tick {i}: the moved ride parted from the one built on it", was.text(), to.text());
            }
        }
    }
    // The rides went somewhere, or the comparisons prove nothing.
    let mut s = sim(&Mods::original(), 0, 0.0);
    ride(&mut s, &Mods::original(), 0, 600);
    assert!(s.rider.ride.chassis.speed.0 > 20.0, "Cross did not ride the bike: {} ft/s", s.rider.ride.chassis.speed.0);
}

#[test]
fn a_change_mid_ride_keeps_the_place_and_rides_the_new_top_speed() {
    let tables = Tables::build();
    let stock = stock_phy(&tables);
    let full = |sim: &mut Sim, ticks: usize| {
        let gas = [Report::from_buttons(Control::Cross.mask()), Report::missing()];
        let mut top = 0.0f32;
        for _ in 0..ticks {
            let c = sim.poll(DT, &gas);
            sim.step(DT, &c, &mut Flat);
            top = top.max(sim.rider.ride.chassis.speed.0);
        }
        top
    };
    let mut s = sim(&Mods::original(), 0, 0.0);
    let stock_top = full(&mut s, 1800);
    assert!(stock_top > 50.0, "the stock bike only reached {stock_top} ft/s");
    // The change itself: nothing about where the bike is or how it moves. (The state hash does move:
    // the engine's tuning is part of the ride's state, as the original's engine object holds it.)
    let before = state(&s);
    let fast = with(Knob::BikeSpeed, 200);
    fast.resim(&Mods::original(), 0, &stock, &mut s);
    assert_eq!(state(&s).1, before.1, "the change moved the bike");
    assert_ne!(state(&s).0, before.0, "the engine was not retuned");
    // And the next half minute is on the new numbers.
    let fast_top = full(&mut s, 1800);
    println!("top speed: {stock_top:.2} ft/s, then {fast_top:.2} after bike speed x2 mid-ride");
    assert!(fast_top > stock_top * 1.5, "bike speed x2 mid-ride reached {fast_top} ft/s against {stock_top}");
    assert!(s.rider.ride.chassis.max_speed_sq.0.sqrt() > fast_top, "the speed limit was not raised");
    // Back to the original: it slows to no more than the stock top. (Not to it: coming down from
    // twice the speed the box has shifted up, and the stand-in bike's second gear holds it at less than
    // its first does, where the stock run stays all the way.)
    Mods::original().resim(&fast, 0, &stock, &mut s);
    full(&mut s, 1800);
    let back = s.rider.ride.chassis.speed.0;
    assert!(back < stock_top + 0.5, "back on the original it rides at {back} ft/s, past the stock {stock_top}");
    assert_eq!(s.rider.ride.chassis.max_speed_sq.0, 135.0 * 135.0, "the speed limit did not come back down");
}

#[test]
fn half_gravity_mid_fall_halves_the_next_ticks_acceleration() {
    let tables = Tables::build();
    let stock = stock_phy(&tables);
    let idle = [Report::idle(), Report::missing()];
    let mut s = sim(&Mods::original(), 0, 500.0);
    let fall = |s: &mut Sim| {
        let v = s.rider.ride.chassis.vel.y();
        let c = s.poll(DT, &idle);
        s.step(DT, &c, &mut Flat);
        assert!(!s.rider.ride.air.on_ground, "the fall touched down");
        s.rider.ride.chassis.vel.y() - v
    };
    fall(&mut s);
    let full = fall(&mut s);
    let at = s.rider.ride.position();
    with(Knob::Gravity, 50).resim(&Mods::original(), 0, &stock, &mut s);
    assert_eq!(s.rider.ride.position(), at, "the change moved the bike");
    // The integrator carries the last tick's acceleration (a fall's first tick is 1.5 g, every one
    // after it g), so the tick of the change is 1.5 × g/2 − 0.5 × g = g/4, and every one after it g/2:
    // the ride built on half gravity, from where this one was.
    let turn = fall(&mut s);
    let half = fall(&mut s);
    assert!(full < -0.1, "the stock bike did not fall: {full} ft/s over a tick");
    let ratio = turn / full;
    assert!((ratio - 0.25).abs() < 0.01, "the tick of the change: {turn} ft/s against {full}, x{ratio}");
    let ratio = half / full;
    assert!((ratio - 0.5).abs() < 0.01, "gravity x0.5 mid-fall: {half} ft/s a tick against {full}, x{ratio}");
}

#[test]
fn a_tape_recorded_across_a_change_replays_bit_for_bit() {
    let tables = Tables::build();
    let stock = stock_phy(&tables);
    let first = Mods::parse("gravity=80").unwrap();
    let then = Mods::parse("bike_speed=200 time_scale=150 grip=70").unwrap();
    // The ride, recorded as the shell records one: each pass's dt and pad, and the change between
    // two passes at the tick it came — twice, the second overriding the first, as two closes of the
    // pause between the same two passes would not, but a recorder must still keep only the last.
    let mut rec = Recorder::new("changed mid-ride");
    rec.tape.modifiers = first;
    let mut made = sim(&first, 0, 0.0);
    let mut seen = Vec::new();
    let mut mods = first;
    for i in 0..900 {
        if i == 300 {
            let scratch = Mods::parse("gravity=10").unwrap();
            rec.modifiers(scratch);
            rec.modifiers(then);
            then.resim(&mods, 0, &stock, &mut made);
            mods = then;
        }
        let dt = modifiers::scaled_dt(DT, mods.time_scale());
        rec.frame(dt, &pad(i));
        tick(&mut made, dt, i);
        seen.push(state(&made));
    }
    // Written and read back, the change comes with it.
    let text = rec.to_text();
    assert!(text.lines().any(|l| l == format!("modifiers_from 300 {}", then.text())), "{}", &text[..300.min(text.len())]);
    let back = PadTape::parse(&text).unwrap();
    assert_eq!(back.changes, vec![(300, then)]);
    assert_eq!(back.modifiers_at(299), first);
    assert_eq!(back.modifiers_at(300), then);
    assert_eq!(back.to_text(), text);
    // Played as the shell plays one — the head's change before the frame, the frame's own dt and pad —
    // it is the same ride.
    let replay = |apply: bool| {
        let mut played = sim(&back.modifiers, 0, 0.0);
        let mut mods = back.modifiers;
        let mut head = Head::new(back.clone());
        let mut out = Vec::new();
        loop {
            if let Some(to) = head.change()
                && apply
            {
                to.resim(&mods, 0, &stock, &mut played);
                mods = to;
            }
            let Some(frame) = head.next() else { break };
            let c = played.poll(frame.dt(), &frame.reports);
            played.step(frame.dt(), &c, &mut Flat);
            out.push(state(&played));
        }
        out
    };
    let again = replay(true);
    assert_eq!(again.len(), seen.len());
    for (i, (x, y)) in again.iter().zip(&seen).enumerate() {
        assert_eq!(x, y, "tick {i}: the replay parted from the ride");
    }
    // Without the change put on it is not, which is why the tape carries it.
    let plain = replay(false);
    assert_eq!(plain[299], seen[299], "the ride before the change is the tape's either way");
    assert_ne!(plain[899], seen[899], "the change made no difference to the ride");
}

#[test]
fn a_tape_with_no_change_is_written_as_before() {
    let mut rec = Recorder::new("kept");
    rec.frame(DT, &pad(0));
    let text = rec.to_text();
    assert!(!text.contains("modifiers"), "{text}");
    assert!(PadTape::parse(&text).unwrap().changes.is_empty());
    // A change written out of order is refused by its line.
    let bad = format!("{text}modifiers_from 5 gravity=50\nmodifiers_from 2 gravity=60\n");
    assert!(PadTape::parse(&bad).is_err());
}

#[test]
fn the_biplane_takes_a_change_in_flight() {
    use game_render::disc::Disc;
    let mut disc = match Disc::open_configured() {
        Ok(disc) => disc,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let Some(name) = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("machine.res")).map(|e| e.name.clone()) else {
        eprintln!("skipped: no machine.res");
        return;
    };
    let res = disc.res(&name).expect("machine.res");
    let bytes = res.read_suffix(game_sim::phy::BIPLANE_PHY).expect("machine.res carries biplane.phy");
    let stock = game_sim::phy::Plane::read_bytes(&bytes);
    for to in [with(Knob::PlaneSpeed, 200), with(Knob::Gravity, 50), with(Knob::AirDrag, 30), Mods::original()] {
        let mut moved = game_sim::flight::Flight::new(&stock);
        to.replane(0, &stock, &mut moved);
        let mut file = stock;
        to.plane(0, &mut file);
        let built = game_sim::flight::Flight::new(&file);
        // Debug prints every float at the precision that tells its bits apart.
        assert_eq!(format!("{moved:?}"), format!("{built:?}"), "{}: the flight moved onto it is not the one built on it", to.text());
        println!("biplane: {} retuned as built", to.text());
    }
}


