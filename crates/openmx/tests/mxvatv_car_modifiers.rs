//! P288 — MX vs. ATV's cars' Game Modifiers ([`Category::MxvatvCars`]) on the shell's car: each knob
//! off 100% moves the loaded `.phy` and the ride the way it is named ([`vehicle::MxvatvCarRide::step`]
//! on a flat floor), and every knob at its default, or any other machine's moved, leaves the car's
//! load, and so its step, bit for bit.
//!
//! The car takes the knobs as the shell's [`vehicle::Garage::modify`] hands them: kept on the
//! [`vehicle::MxvatvCarRide`] and read as it is made ([`Mods::mxvatv_car`]). Each test that rides
//! skips without openmx-mods' `mxvatv_cars` machine pack in this checkout's `mods/machines/`.
//!
//! `src/vehicle.rs` is compiled in by path, the way `tests/atv_modifiers.rs` takes it.

#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;

use game_sim::bindings::Table;
use game_sim::car::MxvatvMachine;
use game_sim::input::{Manager, REPORT_BYTES, Report};
use game_sim::markers::Spot;
use game_sim::modifiers::{Category, Knob, Mods};
use game_sim::world::Flat;
use ps2_float::{F, Vf};
use std::path::Path;

const DT: f32 = 1.0 / 30.0;

/// The buggy off the pack, or `None` without the pack.
fn buggy() -> Option<vehicle::MxvatvCarRide> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/machines/mxvatv_cars");
    if !dir.join("mod.json").is_file() {
        println!("SKIPPED: no mods/machines/mxvatv_cars (openmx-mods' import-atv cars makes it)");
        return None;
    }
    let files = game_render::disc::Res::folder(&dir).expect("the pack's folder reads");
    let car = vehicle::MxvatvCarRide::read("mxvatv_cars", "buggy", "Veh/buggy.phy", "Veh/Anim/buggy.psg", &mut |n| files.read(&n.replace('/', "\\")).ok()).expect("the buggy builds");
    assert_eq!(car.machine, MxvatvMachine::Buggy);
    Some(car)
}

fn with(knob: Knob, pct: u16) -> Mods {
    let mut m = Mods::original();
    m.set(0, knob, pct);
    m
}

/// The car's file as the ride is made from it, on `mods`.
fn file(car: &vehicle::MxvatvCarRide, mods: &Mods) -> game_sim::car::Car {
    let mut c = car.load.file.car.clone();
    mods.mxvatv_car(0, &mut c);
    c
}

/// A pad report: Cross (the gas) at `gas` pressure when above zero, the left stick's x at `lx`
/// (0x7f centred, 0x00 full left).
fn report(gas: u8, lx: u8) -> Report {
    let buttons = if gas > 0 { 0xbf } else { 0xff };
    let hex = format!("0079ff{buttons:02x}7f7f{lx:02x}7f000000000000{gas:02x}{}", "00".repeat(17));
    let mut bytes = [0u8; REPORT_BYTES];
    for (k, b) in bytes.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[2 * k..2 * k + 2], 16).unwrap();
    }
    Report::from_bytes(bytes)
}

/// `ticks` ticks of the car on `mods`, taken at rest on a flat floor, `held` its pad each tick: its
/// body's position and velocity after each.
fn ride(car: &vehicle::MxvatvCarRide, mods: &Mods, ticks: usize, held: &dyn Fn(usize) -> Report) -> Vec<(Vf, Vf)> {
    let mut car = car.clone();
    (car.mods, car.seat) = (*mods, 0);
    let mut floor = Flat::default();
    let spot = Spot::still(Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0));
    car.take(&spot, &mut floor).expect("the floor is under it");
    let (mut input, table) = (Manager::connected(), Table::SHIPPED);
    (0..ticks)
        .map(|t| {
            input.tick(F(DT), &[held(t), Report::idle()]);
            car.step(input.pad(0), &table, F(DT), &mut floor);
            let b = &car.ridden.as_ref().unwrap().body;
            (b.position, b.velocity)
        })
        .collect()
}

/// The ticks the car is left to come down onto the floor and settle before the gas: it is taken a
/// foot over the ground, and drive wheels spun up in that drop land spinning backwards on SLUS-21104's
/// step and hold the car for a second (with 150% acceleration), which is not what these measure.
const SETTLE: usize = 30;

/// Cross held, after [`SETTLE`].
fn gas(t: usize) -> Report {
    if t < SETTLE { report(0, 0x7f) } else { report(0xff, 0x7f) }
}

fn speed(b: &(Vf, Vf)) -> f32 {
    b.1.x().hypot(b.1.z())
}

fn bits(bodies: &[(Vf, Vf)]) -> Vec<u32> {
    bodies.iter().flat_map(|(p, v)| [*p, *v]).flat_map(|v| [v.x(), v.y(), v.z(), v.w()]).map(f32::to_bits).collect()
}

#[test]
fn the_mxvatv_car_knobs_are_their_own_category() {
    assert_eq!(Category::MxvatvCars.knobs(), [Knob::MxvatvCarSpeed, Knob::MxvatvCarAccel, Knob::MxvatvCarGrip, Knob::MxvatvCarSuspension]);
    assert!(!Category::ALL.contains(&Category::MxvatvCars), "an addition's, not the disc's");
    for k in Category::MxvatvCars.knobs() {
        assert!(k.per_seat());
        assert_eq!(Knob::from_word(k.word()), Some(k));
        assert_eq!(Mods::parse(&with(k, 150).text()).unwrap(), with(k, 150));
    }
}

#[test]
fn every_knob_at_its_default_leaves_the_mxvatv_car_bit_exact() {
    let Some(car) = buggy() else { return };
    let mut all = Mods::original();
    for k in Knob::ALL {
        all.set(0, k, 100);
    }
    assert!(all.is_original());
    // Every other machine's and the world's moved: none of them reaches the car.
    let mut others = Mods::original();
    for k in Knob::ALL.into_iter().filter(|k| k.category() != Category::MxvatvCars) {
        others.set(0, k, k.step(k.most(), false));
    }
    let stock = format!("{:?}", car.load.file.car);
    for mods in [all, others] {
        assert_eq!(format!("{:?}", file(&car, &mods)), stock, "{}", mods.text());
    }
    // And these knobs are not the stock cars': theirs leave MX Unleashed's cars alone.
    let mut mine = Mods::original();
    Category::MxvatvCars.knobs().into_iter().for_each(|k| mine.set(0, k, 200));
    let mut c = car.load.file.car.clone();
    mine.car(0, &mut c);
    assert_eq!(format!("{c:?}"), stock);
    let turn = |t: usize| report(0xff, if t < 60 { 0x7f } else { 0x00 });
    let plain = bits(&ride(&car, &Mods::original(), 120, &turn));
    assert_eq!(bits(&ride(&car, &all, 120, &turn)), plain);
    assert_eq!(bits(&ride(&car, &others, 120, &turn)), plain);
    // Another seat's moved: not this seat's car.
    let mut two = Mods::original();
    two.set(1, Knob::MxvatvCarSpeed, 200);
    assert_eq!(bits(&ride(&car, &two, 120, &turn)), plain);
}

#[test]
fn an_mxvatv_car_knob_moves_its_keys() {
    let Some(car) = buggy() else { return };
    let stock = file(&car, &Mods::original());
    let at = |k: Knob, pct: u16| file(&car, &with(k, pct));

    let fast = at(Knob::MxvatvCarSpeed, 200);
    assert_eq!(fast.engine.final_gear_ratio, stock.engine.final_gear_ratio / F(2.0));
    assert_eq!(fast.engine.torque[0], stock.engine.torque[0] * F(2.0));
    assert_eq!(fast.aero.aero_surface_area, stock.aero.aero_surface_area / F(4.0));
    assert_eq!(fast.engine.engine_braking, stock.engine.engine_braking * F(2.0));

    let quick = at(Knob::MxvatvCarAccel, 200);
    assert_eq!(quick.engine.final_gear_ratio, stock.engine.final_gear_ratio);
    assert_eq!(quick.engine.torque[0], stock.engine.torque[0] * F(2.0));
    assert_eq!(quick.aero.aero_surface_area, stock.aero.aero_surface_area * F(2.0));
    assert_eq!(quick.tires[0].surfaces[0].wheel_friction, stock.tires[0].surfaces[0].wheel_friction * F(2.0));

    let slick = at(Knob::MxvatvCarGrip, 50);
    assert_eq!(slick.tires[2].surfaces[0].friction_peak, stock.tires[2].surfaces[0].friction_peak * F(0.5));

    let stiff = at(Knob::MxvatvCarSuspension, 200);
    assert_eq!(stiff.tires[1].spring, stock.tires[1].spring * F(2.0));
    assert_eq!(stiff.tires[1].bounce_damping, stock.tires[1].bounce_damping * F(2.0).sqrt());

    // And the made car's keys are the moved file's.
    let keys = |mods: &Mods| {
        let mut c = car.clone();
        (c.mods, c.seat) = (*mods, 0);
        let mut floor = Flat::default();
        let spot = Spot::still(Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0));
        c.take(&spot, &mut floor).unwrap();
        format!("{:?}", c.ridden.unwrap().keys)
    };
    assert_ne!(keys(&with(Knob::MxvatvCarSuspension, 200)), keys(&Mods::original()));
}

#[test]
fn an_mxvatv_car_knob_changes_the_cars_ride() {
    let Some(car) = buggy() else { return };
    let run = |k: Knob, pct: u16, ticks: usize, held: &dyn Fn(usize) -> Report| ride(&car, &with(k, pct), ticks, held);

    // Top speed: the fastest it goes in twenty seconds of gas.
    let top = |pct: u16| run(Knob::MxvatvCarSpeed, pct, 600, &gas).iter().map(speed).fold(0.0, f32::max);
    let (stock, fast, slow) = (top(100), top(150), top(50));
    println!("top speed: {slow} / {stock} / {fast} ft/s at 50/100/150%");
    assert!(fast > stock * 1.2 && slow < stock * 0.8, "top speed {slow} {stock} {fast}");

    // Acceleration: the speed two seconds off the line.
    let at = |pct: u16| speed(&run(Knob::MxvatvCarAccel, pct, SETTLE + 60, &gas)[SETTLE + 59]);
    let (stock, quick, slow) = (at(100), at(150), at(50));
    println!("2 s: {slow} / {stock} / {quick} ft/s at 50/100/150% acceleration");
    assert!(quick > stock * 1.1 && slow < stock * 0.9, "acceleration {slow} {stock} {quick}");

    // Grip: a hard turn on the gas, the sideways pull the tires hold (the speed times the rate the
    // path turns) over a second of it.
    let turn = |t: usize| if t < SETTLE { report(0, 0x7f) } else { report(0xff, if t < 90 { 0x7f } else { 0x00 }) };
    let pull = |pct: u16| {
        let b = run(Knob::MxvatvCarGrip, pct, 132, &turn);
        let way = |b: &(Vf, Vf)| b.1.x().atan2(b.1.z());
        let turned = |t: usize| {
            let d = way(&b[t + 1]) - way(&b[t]);
            (d + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
        };
        (101..131).map(|t| speed(&b[t]) * turned(t).abs() / DT).sum::<f32>() / 30.0
    };
    let (stock, slick) = (pull(100), pull(40));
    println!("sideways pull in the turn: {stock} -> {slick} ft/s^2 at 40% grip");
    assert!(slick < stock * 0.8, "grip {stock} -> {slick}");

    // Suspension: stiffer springs sag less, the car standing higher once settled.
    let stand = |pct: u16| run(Knob::MxvatvCarSuspension, pct, 120, &|_| report(0, 0x7f))[119].0.y();
    let (stock, stiff, soft) = (stand(100), stand(200), stand(50));
    println!("rest height: {soft} / {stock} / {stiff} at 50/100/200%");
    assert!(stiff > stock && stock > soft, "suspension {soft} {stock} {stiff}");
}


