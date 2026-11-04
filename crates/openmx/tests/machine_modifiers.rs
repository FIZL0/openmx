//! P272 — the Kiowa's and the cars' Game Modifiers on the shell's own rides: each knob off 100% moves
//! the ride the way it is named ([`vehicle::CarRide::step`], [`vehicle::KiowaRide::step`] on a flat
//! floor), and every knob at its default leaves the machine's file, and so its step, bit for bit.
//!
//! The rides take the knobs as the shell's [`vehicle::Garage::modify`] hands them: on the machine's
//! file ([`Mods::car`], [`Mods::kiowa`]) before it is taken. Each test skips without a disc.
//!
//! `src/vehicle.rs` is compiled in by path, the way `tests/machines.rs` takes it.

#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;

use game_sim::input::{Control, EventQueue, Pad, Report};
use game_sim::markers::Spot;
use game_sim::modifiers::{Category, Knob, Mods};
use game_sim::world::Flat;
use ps2_float::{F, Vf};

const DT: f32 = 1.0 / 30.0;

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

/// The bits of a body's place, velocity, rotation and rates.
fn bits(b: &game_sim::biplane::Body) -> Vec<u32> {
    [b.position, b.velocity, b.quat, b.rates].iter().flat_map(|v| [v.x(), v.y(), v.z(), v.w()]).map(f32::to_bits).collect()
}

fn ground_speed(b: &game_sim::biplane::Body) -> f32 {
    (b.velocity.x() * b.velocity.x() + b.velocity.z() * b.velocity.z()).sqrt()
}

/// `machine` with `mods` driven `ticks` ticks on a flat floor, the pad's buttons `held(tick)`: each
/// tick's body.
fn drive(machines: &game_render::disc::Res, machine: vehicle::Machine, mods: &Mods, ticks: usize, held: impl Fn(usize) -> u16) -> Vec<game_sim::biplane::Body> {
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
            car.driven.as_ref().unwrap().body
        })
        .collect()
}

/// The Kiowa with `mods` flown `ticks` ticks off a flat floor, as [`drive`].
fn fly(machines: &game_render::disc::Res, mods: &Mods, ticks: usize, held: impl Fn(usize) -> u16) -> Vec<game_sim::biplane::Body> {
    let phy = machines.read_suffix(game_sim::kiowa::KIOWA_PHY).expect("kiowa.phy");
    let col = machines.read_suffix(game_sim::kiowa::KIOWA_RAYS).expect("kiowam.col");
    let mut heli = vehicle::KiowaRide::new(&phy, 0, None).with_rays(&col);
    mods.kiowa(0, &mut heli.phy);
    let mut floor = Flat::default();
    heli.take(&spot(), &mut floor).unwrap();
    let (tables, table) = (game_sim::rider::Tables::build(), game_sim::bindings::Table::SHIPPED);
    let mut pad = Pad::new(0);
    pad.connected = true;
    let mut events = EventQueue::new();
    (0..ticks)
        .map(|t| {
            pad.poll(F(DT), &Report::from_buttons(held(t)), &mut events);
            heli.step(&pad, &table, &tables.ride, F(DT), &mut floor);
            heli.flown.as_ref().unwrap().ride.body
        })
        .collect()
}

const CARS: [vehicle::Machine; 3] = [vehicle::Machine::Buggy, vehicle::Machine::Monster, vehicle::Machine::Baja];

#[test]
fn every_knob_at_its_default_leaves_each_machines_step_bit_exact() {
    let Some(machines) = machines() else { return println!("skipped: no disc") };
    // Every knob of every machine written at 100%, as a page left alone writes them.
    let mut all = Mods::original();
    for k in Knob::ALL {
        all.set(0, k, 100);
    }
    assert!(all.is_original());
    // And every other machine's knob moved: none of them reaches a car or the Kiowa.
    let others = |c: Category| {
        let mut m = Mods::original();
        for k in Knob::ALL.into_iter().filter(|k| k.category() != c && k.category() != Category::World) {
            m.set(0, k, 150);
        }
        m
    };
    let heli = vehicle::KiowaRide::new(&machines.read_suffix(game_sim::kiowa::KIOWA_PHY).unwrap(), 0, None);
    for mods in [Mods::original(), all, others(Category::Kiowa)] {
        let mut phy = heli.phy.clone();
        mods.kiowa(0, &mut phy);
        assert_eq!(format!("{phy:?}"), format!("{:?}", heli.phy), "{}", mods.text());
    }
    for machine in CARS {
        let stock = vehicle::CarRide::new(machine, 0, None, &mut |n| machines.read_suffix(n).ok()).unwrap().car;
        for mods in [Mods::original(), all, others(Category::Cars)] {
            let mut car = stock.clone();
            mods.car(0, &mut car);
            assert_eq!(format!("{car:?}"), format!("{stock:?}"), "{machine:?}: {}", mods.text());
        }
    }
    // The rides step bit for bit: gas and a turn in the car, a climb and a pedal in the Kiowa.
    let hands = |t: usize| match t {
        0..60 => Control::Cross.mask(),
        60..120 => Control::Cross.mask() | Control::Left.mask() | Control::L1.mask(),
        _ => Control::Up.mask(),
    };
    let body = |b: &[game_sim::biplane::Body]| b.iter().flat_map(bits).collect::<Vec<_>>();
    for machine in CARS {
        let plain = body(&drive(&machines, machine, &Mods::original(), 180, hands));
        assert_eq!(body(&drive(&machines, machine, &all, 180, hands)), plain, "{machine:?}");
        assert_eq!(body(&drive(&machines, machine, &others(Category::Cars), 180, hands)), plain, "{machine:?}");
    }
    let plain = body(&fly(&machines, &Mods::original(), 180, hands));
    assert_eq!(body(&fly(&machines, &all, 180, hands)), plain);
    assert_eq!(body(&fly(&machines, &others(Category::Kiowa), 180, hands)), plain);
}

#[test]
fn a_car_knob_changes_the_cars_ride() {
    let Some(machines) = machines() else { return println!("skipped: no disc") };
    let gas = |_: usize| Control::Cross.mask();
    for machine in CARS {
        let run = |k: Knob, pct: u16, ticks: usize, held: &dyn Fn(usize) -> u16| drive(&machines, machine, &with(k, pct), ticks, held);
        let top = |k: Knob, pct: u16| run(k, pct, 900, &gas).iter().map(ground_speed).fold(0.0, f32::max);
        let (stock, fast) = (top(Knob::CarSpeed, 100), top(Knob::CarSpeed, 150));
        println!("{machine:?} top speed: {stock} -> {fast} ft/s at 150%");
        assert!(fast > stock * 1.2, "{machine:?}: top speed {stock} -> {fast}");
        assert!(top(Knob::CarSpeed, 50) < stock * 0.8, "{machine:?}: half top speed");

        // Acceleration: the speed three seconds off the line (the monster truck's launch spins its
        // tires, so it gains less than the others).
        let at = |k: Knob, pct: u16| ground_speed(&run(k, pct, 90, &gas)[89]);
        let (stock, quick, slow) = (at(Knob::CarAccel, 100), at(Knob::CarAccel, 150), at(Knob::CarAccel, 50));
        println!("{machine:?} 3 s: {slow} / {stock} / {quick} ft/s at 50/100/150% acceleration");
        assert!(quick > stock * 1.1 && slow < stock * 0.9, "{machine:?}: acceleration {slow} {stock} {quick}");

        // Grip: a hard turn at speed, how far round the car's path has come (a car on slick tires
        // may spin about more, but it slides on along its way).
        let turn = |t: usize| if t < 90 { Control::Cross.mask() } else { Control::Cross.mask() | Control::Left.mask() };
        let path = |pct: u16| {
            let b = run(Knob::CarGrip, pct, 120, &turn);
            let way = |b: &game_sim::biplane::Body| b.velocity.x().atan2(b.velocity.z());
            let mut d = way(&b[119]) - way(&b[89]);
            if d > std::f32::consts::PI {
                d -= std::f32::consts::TAU;
            } else if d < -std::f32::consts::PI {
                d += std::f32::consts::TAU;
            }
            d.abs()
        };
        let (stock, slick) = (path(100), path(40));
        println!("{machine:?} path turned: {stock} -> {slick} rad at 40% grip");
        assert!(slick < stock * 0.9, "{machine:?}: grip {stock} -> {slick}");

        // Suspension: stiffer springs sag less, the car standing higher once settled.
        let stand = |pct: u16| run(Knob::CarSuspension, pct, 120, &|_| 0)[119].position.y();
        let (stock, stiff, soft) = (stand(100), stand(200), stand(50));
        println!("{machine:?} rest height: {soft} / {stock} / {stiff} at 50/100/200%");
        assert!(stiff > stock && stock > soft, "{machine:?}: suspension {soft} {stock} {stiff}");
    }
}

#[test]
fn a_kiowa_knob_changes_the_kiowas_ride() {
    let Some(machines) = machines() else { return println!("skipped: no disc") };
    let run = |k: Knob, pct: u16, ticks: usize, held: &dyn Fn(usize) -> u16| fly(&machines, &with(k, pct), ticks, held);
    let climb = |_: usize| Control::Cross.mask();
    // Climb: height after four seconds of Cross.
    let height = |pct: u16| run(Knob::KiowaClimb, pct, 120, &climb)[119].position.y();
    let (stock, quick, slow) = (height(100), height(150), height(50));
    println!("climb: {slow} / {stock} / {quick} ft at 50/100/150%");
    assert!(quick > stock * 1.15 && slow < stock * 0.85, "climb {slow} {stock} {quick}");

    // Ceiling: climbing for a minute stops under a halved max height.
    let ceiling = |pct: u16| run(Knob::KiowaCeiling, pct, 1800, &climb).iter().map(|b| b.position.y()).fold(f32::MIN, f32::max);
    let (stock, low) = (ceiling(100), ceiling(50));
    println!("ceiling: {stock} -> {low} ft at 50%");
    assert!(low < stock * 0.75, "ceiling {stock} -> {low}");

    // Speed: up two seconds, then the stick forward for twenty.
    let forward = |t: usize| if t < 60 { Control::Cross.mask() } else { Control::Up.mask() };
    let top = |pct: u16| run(Knob::KiowaSpeed, pct, 660, &forward).iter().map(ground_speed).fold(0.0, f32::max);
    let (stock, fast) = (top(100), top(150));
    println!("top speed: {stock} -> {fast} ft/s at 150%");
    assert!(fast > stock * 1.2, "speed {stock} -> {fast}");

    // Turn: up two seconds, then the pedal held; the yaw rate it reaches.
    let pedal = |t: usize| if t < 60 { Control::Cross.mask() } else { Control::L1.mask() };
    let yaw = |pct: u16| run(Knob::KiowaTurn, pct, 150, &pedal)[60..].iter().map(|b| b.rates.y().abs()).fold(0.0, f32::max);
    let (stock, quick) = (yaw(100), yaw(150));
    println!("yaw rate: {stock} -> {quick} at 150%");
    assert!(quick > stock * 1.2, "turn {stock} -> {quick}");
}
