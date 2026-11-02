//! P255 — the port's gas gyro: the throttle held in the air pitches the bike back (nose up), the
//! brake gyro's turn the other way (the brake pitches the nose down) at [`air::GAS_GYRO_SHARE`] of
//! its torque. MX Unleashed's own air step reads the brakes and not
//! the throttle (its `+0x650`/`+0x654`, no throttle field), so this is a switch on [`Phy`] the app
//! turns on, and off the ride is the original's.
//!
//! The bike is `tests/freeride-bike.phy.txt`, the hand-written one `tests/ride.rs` rides, with a
//! 250's `BrakeGyroTorque = 400` laid in: no number here is claimed to be a game's.

use game_sim::air;
use game_sim::phy::Phy;
use game_sim::ride::{Controls, Ride, Tables};
use game_sim::world::Flat;
use ps2_float::{F, Vf};

const BIKE: &str = include_str!("freeride-bike.phy.txt");
const DT: F = F(1.0 / 60.0);

fn bike(tables: &Tables, gas_gyro: bool) -> Phy {
    let mut p = Phy::read_bytes(BIKE.as_bytes(), &tables.curve);
    p.air.brake_gyro_torque = F(400.0);
    p.air_gas_gyro = gas_gyro;
    p
}

/// Dropped from 30 ft with the stick centred, on `c`: half a second of air, never touching down.
/// Answers the chassis' pitch rate and how steeply its nose points (the forward axis' `y`, up +).
fn drop(tables: &Tables, p: &Phy, c: &Controls) -> (f32, f32, Ride) {
    let mut r = Ride::new(p, game_sim::stock::MOUNTS, Vf::new(0.0, 30.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0));
    let mut floor = Flat { y: F::ZERO, normal: Vf::new(0.0, 1.0, 0.0, 0.0) };
    for _ in 0..30 {
        r.frame(p, tables, c, DT, &mut floor);
        assert!(r.air.airborne || !r.air.on_ground, "the drop touched down");
    }
    (r.chassis.ang_vel.x(), r.axes().0.y(), r)
}

fn coast() -> Controls {
    Controls::default()
}

fn gas() -> Controls {
    Controls { gas: F::ONE, ..Controls::default() }
}

fn brake() -> Controls {
    Controls { brake: F::ONE, ..Controls::default() }
}

#[test]
fn the_gas_pulls_the_nose_up_a_little_less_than_the_brake() {
    let tables = Tables::build();
    let p = bike(&tables, true);
    let (w_coast, nose_coast, _) = drop(&tables, &p, &coast());
    let (w_gas, nose_gas, _) = drop(&tables, &p, &gas());
    let (w_brake, nose_brake, _) = drop(&tables, &p, &brake());
    // The brake gyro (+x) pitches the nose down: the braked drop's nose ends lower than coasting.
    assert!(w_brake > w_coast && nose_brake < nose_coast, "brake {w_brake} / {nose_brake}, coasting {w_coast} / {nose_coast}");
    // The gas pulls it back, nose up, at less than the brake's rate.
    assert!(w_gas < w_coast && nose_gas > nose_coast, "gas {w_gas} / {nose_gas}, coasting {w_coast} / {nose_coast}");
    assert!(w_coast - w_gas > 0.5 * (w_brake - w_coast) && w_coast - w_gas < w_brake - w_coast, "gas {w_gas}, brake {w_brake}, coasting {w_coast}");
    // (Not the noses' heights: half a second of brake turns the nose past straight down and back.)
}

#[test]
fn with_the_switch_off_the_gas_does_not_pitch_the_bike() {
    let tables = Tables::build();
    let off = bike(&tables, false);
    let (w_coast, nose_coast, _) = drop(&tables, &off, &coast());
    let (w_gas, nose_gas, _) = drop(&tables, &off, &gas());
    // The original's air step reads no throttle: the drop pitches the same on the gas as coasting.
    assert_eq!((w_gas.to_bits(), nose_gas.to_bits()), (w_coast.to_bits(), nose_coast.to_bits()));
    // And the switch on changes nothing a ride does without the throttle open.
    let on = bike(&tables, true);
    let (_, _, a) = drop(&tables, &off, &brake());
    let (_, _, b) = drop(&tables, &on, &brake());
    assert_eq!(bits(a.chassis.ang_vel), bits(b.chassis.ang_vel));
    assert_eq!(bits(a.chassis.vel), bits(b.chassis.vel));
}

#[test]
fn the_gas_gyro_waits_for_the_air_and_gives_way_to_a_brake() {
    let p = bike(&Tables::build(), true).air;
    let mut ch = air::Chassis::default();
    let mut a = air::Air { on_ground: false, ..air::Air::default() };
    let st = air::Step::default();
    air::gas_gyro(&p, &a, &mut ch, &st, F(0.5));
    assert!((ch.moment.x() + 140.0).abs() < 1e-3, "a half-open throttle's torque: {}", ch.moment.x());
    let before = ch.moment;
    air::gas_gyro(&p, &a, &mut ch, &air::Step { rear_brake: F::ONE, ..st }, F::ONE);
    air::gas_gyro(&p, &a, &mut ch, &air::Step { stunt: true, ..st }, F::ONE);
    air::gas_gyro(&p, &a, &mut ch, &st, F::ZERO);
    a.on_ground = true;
    air::gas_gyro(&p, &a, &mut ch, &st, F::ONE);
    assert_eq!(bits(ch.moment), bits(before), "braking, a trick, a shut throttle or the ground took gas torque");
}

fn bits(v: Vf) -> [u32; 4] {
    v.0.map(f32::to_bits)
}
