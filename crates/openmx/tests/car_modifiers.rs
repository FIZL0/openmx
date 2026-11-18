//! P370 — MX Unleashed's stock cars' Car acceleration knob over 100% and the car's launch (the clutch's
//! torque scale, `MaxClutchTorqueScale` easing to 1 over `ClutchTorqueScaleTime`). P368 turned MX vs.
//! ATV's cars' launch off over 100% because a freshly taken one lands on the gas inside it and is held
//! (`tests/mxvatv_car_modifiers.rs`). The stock cars are taken with theirs spent, and a clutch drop's
//! launch over 100% does not hold them, so their knob leaves the launch alone. Each test skips without
//! a disc. P370a: the take stood the cars a tire radius too high, and over 100% the drop held them on
//! landing; stood right, a car taken on the gas is away at every value.
//!
//! `src/vehicle.rs` is compiled in by path, the way `tests/machine_modifiers.rs` takes it.

#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;

use game_sim::input::{Control, EventQueue, Pad, Report};
use game_sim::markers::Spot;
use game_sim::modifiers::{Knob, Mods};
use game_sim::world::Flat;
use ps2_float::{F, Vf};

const DT: f32 = 1.0 / 30.0;

const CARS: [vehicle::Machine; 3] = [vehicle::Machine::Buggy, vehicle::Machine::Monster, vehicle::Machine::Baja];

fn machines() -> Option<game_render::disc::Res> {
    let mut disc = game_render::Disc::open_configured().ok()?;
    disc.res("machine.res").ok()
}

fn with(knob: Knob, pct: u16) -> Mods {
    let mut m = Mods::original();
    m.set(0, knob, pct);
    m
}

fn spot() -> Spot {
    Spot::still(Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0))
}

fn ground_speed(b: &game_sim::biplane::Body) -> f32 {
    (b.velocity.x() * b.velocity.x() + b.velocity.z() * b.velocity.z()).sqrt()
}

/// One tick of a ride: the ground speed, the launch's boost and its timer and length.
struct Tick {
    speed: f32,
    grounded: bool,
    boost: F,
    timer: F,
    time: F,
}

/// `machine` with `mods` taken on a flat floor and driven `ticks` ticks, the pad's buttons
/// `held(tick)`, as the shell's garage hands the knobs to the ride ([`Mods::car`] on its file).
fn drive(machines: &game_render::disc::Res, machine: vehicle::Machine, mods: &Mods, ticks: usize, held: &dyn Fn(usize) -> u16) -> Vec<Tick> {
    let mut car = vehicle::CarRide::new(machine, 0, None, &mut |n| machines.read_suffix(n).ok()).unwrap();
    mods.car(0, &mut car.car);
    let mut floor = Flat::default();
    car.take(&spot(), &mut floor).unwrap();
    let (tables, table) = (game_sim::rider::Tables::build(), game_sim::bindings::Table::SHIPPED);
    let mut pad = Pad::new(0);
    pad.connected = true;
    let mut events = EventQueue::new();
    (0..ticks)
        .map(|t| {
            pad.poll(F(DT), &Report::from_buttons(held(t)), &mut events);
            car.step(&pad, &table, &tables.ride, F(DT), &mut floor);
            let m = car.driven.as_ref().unwrap();
            Tick { speed: ground_speed(&m.body), grounded: m.wheels.iter().any(|w| w.s.grounded), boost: m.drivetrain.launch_boost, timer: m.launch.timer, time: m.launch.time }
        })
        .collect()
}

/// The acceleration knob leaves each car's launch as its file has it, at every value.
#[test]
fn car_acceleration_leaves_the_launch() {
    let Some(machines) = machines() else { return println!("skipped: no disc") };
    for machine in CARS {
        let launch = |pct: u16| {
            let mut car = vehicle::CarRide::new(machine, 0, None, &mut |n| machines.read_suffix(n).ok()).unwrap().car;
            with(Knob::CarAccel, pct).car(0, &mut car);
            (car.max_clutch_torque_scale, car.clutch_torque_scale_time)
        };
        let stock = launch(100);
        assert!(stock.0.is_some_and(|s| s > F::ONE), "{machine:?} has a launch: {stock:?}");
        for pct in [50, 110, 150, 500] {
            assert_eq!(launch(pct), stock, "{machine:?} {pct}%");
        }
    }
}

/// A freshly taken stock car is past its launch: on the gas from the take, at 100% and over, the
/// boost is 1 throughout and the timer past the launch's length, so the launch cannot hold it (what
/// P368 found for MX vs. ATV's cars, whose take starts the timer at 0).
#[test]
fn a_freshly_taken_car_has_its_launch_spent() {
    let Some(machines) = machines() else { return println!("skipped: no disc") };
    for machine in CARS {
        for pct in [100, 110, 150, 500] {
            for (t, tick) in drive(&machines, machine, &with(Knob::CarAccel, pct), 60, &|_| Control::Cross.mask()).iter().enumerate() {
                assert!(tick.boost == F::ONE && tick.timer > tick.time, "{machine:?} {pct}% tick {t}: boost {} timer {} of {}", tick.boost, tick.timer, tick.time);
            }
        }
    }
}

/// Settled for 2 s, the clutch (L1) held with the gas for 1 s, then let out on the gas: the launch
/// starts at the drop. Over 100% it does not hold the car: every 10% from 110 to the knob's top, each
/// car is at least half as fast as at 100% a second after the drop and 0.8 times two seconds after
/// (a held car is under 12 ft/s at one second; the slowest here is the buggy at 490%, 22 against 34,
/// and the launch off makes it no better). Turning the launch off over 100% as P368 does for MX vs.
/// ATV's cars was tried: it slowed the monster truck to 52 against 72 ft/s at 110%.
#[test]
fn a_clutch_drop_over_100_is_not_held() {
    let Some(machines) = machines() else { return println!("skipped: no disc") };
    let hands = |t: usize| match t {
        0..60 => 0,
        60..90 => Control::Cross.mask() | Control::L1.mask(),
        _ => Control::Cross.mask(),
    };
    for machine in CARS {
        let stock = drive(&machines, machine, &Mods::original(), 150, &hands);
        assert!(stock[90].boost > F::ONE, "{machine:?}: the drop starts the launch ({})", stock[90].boost);
        let (one, two) = (stock[119].speed, stock[149].speed);
        assert!(one > 20.0, "{machine:?} at 100% is away: {one}");
        for pct in (110..=Knob::CarAccel.most()).step_by(10) {
            let b = drive(&machines, machine, &with(Knob::CarAccel, pct), 150, &hands);
            println!("{machine:?} {pct}%: {} / {} ft/s 1 / 2 s after the drop (100%: {one} / {two})", b[119].speed, b[149].speed);
            assert!(b[119].speed >= one * 0.5 && b[149].speed >= two * 0.8, "{machine:?} held at {pct}%: {} {}", b[119].speed, b[149].speed);
        }
    }
}

/// P370a: a car is taken with its tires' bottoms half a foot over the floor, so it is down in 6
/// ticks (it was 12 or 13 when the take counted the tires' radius twice, and the drop spun its rear
/// wheels up).
#[test]
fn a_taken_car_lands_at_once() {
    let Some(machines) = machines() else { return println!("skipped: no disc") };
    for machine in CARS {
        let ride = drive(&machines, machine, &Mods::original(), 20, &|_| Control::Cross.mask());
        let down = ride.iter().position(|t| t.grounded);
        assert!(down.is_some_and(|t| t <= 6), "{machine:?} first down at tick {down:?}");
    }
}

/// P370a: on the gas from the take, every 10% from 110 to the knob's top, each car is at least half
/// as fast as at 100% a second in and 0.9 times two seconds in. Held, the baja was at 11.7 against
/// 28 ft/s a second in at every value, and the monster truck at 21 against 35 from 180%; the slowest
/// now (the baja at 200%, about 25 against 37) spins its rear wheels on the torque it is given.
/// P370b left that: raising the driven tires' forward grip with k (as Knob::AtvAccel does for the
/// quad) put the baja at 0.84 times or better, but tipped the monster truck onto its tail and off the
/// floor from 280% (from 300% with the raise held to 1.2).
#[test]
fn a_car_taken_on_the_gas_over_100_is_not_held() {
    let Some(machines) = machines() else { return println!("skipped: no disc") };
    let gas = |_| Control::Cross.mask();
    for machine in CARS {
        let stock = drive(&machines, machine, &Mods::original(), 60, &gas);
        let (one, two) = (stock[29].speed, stock[59].speed);
        assert!(one > 20.0, "{machine:?} at 100% is away: {one}");
        for pct in (110..=Knob::CarAccel.most()).step_by(10) {
            let b = drive(&machines, machine, &with(Knob::CarAccel, pct), 60, &gas);
            println!("{machine:?} {pct}%: {} / {} ft/s 1 / 2 s in (100%: {one} / {two})", b[29].speed, b[59].speed);
            assert!(b[29].speed >= one * 0.5 && b[59].speed >= two * 0.9, "{machine:?} held at {pct}%: {} {}", b[29].speed, b[59].speed);
        }
    }
}
