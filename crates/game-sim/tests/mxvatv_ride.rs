//! MD62 — MX vs. ATV's airborne control on a ride: the gyro options reach the bike on
//! `Handling::MxVsAtv`, and on the original's handling they are not read at all. The step itself is
//! proved against SLUS-21104 in `tests/mxvatv_air.rs`; this is the wiring.
//!
//! The bike is `tests/freeride-bike.phy.txt`, the hand-written one `tests/ride.rs` rides, with a
//! `GasGyroTorque` laid in: no number here is claimed to be a game's.

use game_sim::handling::{Gyro, Handling};
use game_sim::phy::Phy;
use game_sim::ride::{Controls, Ride, Tables};
use game_sim::world::Flat;
use ps2_float::{F, Vf};

const BIKE: &str = include_str!("freeride-bike.phy.txt");
const DT: F = F(1.0 / 60.0);

fn bike(tables: &Tables, handling: Handling, gyro: Gyro) -> Phy {
    let mut p = Phy::read_bytes(BIKE.as_bytes(), &tables.curve);
    p.handling = handling;
    p.gas_gyro_torque = F(175.0);
    p.gyro = gyro;
    p
}

/// Dropped from 30 ft with the throttle pinned and the stick centred: half a second of air, never
/// touching down. Answers the chassis' angular velocity after it.
fn drop(tables: &Tables, p: &Phy) -> Vf {
    let mut r = Ride::new(p, game_sim::stock::MOUNTS, Vf::new(0.0, 30.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0));
    let mut floor = Flat { y: F::ZERO, normal: Vf::new(0.0, 1.0, 0.0, 0.0) };
    let c = Controls { gas: F::ONE, ..Controls::default() };
    for _ in 0..30 {
        r.frame(p, tables, &c, DT, &mut floor);
    }
    r.chassis.ang_vel
}

fn bits(v: Vf) -> [u32; 4] {
    v.0.map(f32::to_bits)
}

#[test]
fn the_gas_gyro_pitches_an_mxvatv_bike_and_not_the_originals() {
    let tables = Tables::build();
    let off = Gyro::default();
    let on = Gyro { brake: true, gas: true };
    assert!(!off.gas && off.brake, "the game ships brake gyro on, gas gyro off");

    let atv_off = drop(&tables, &bike(&tables, Handling::MxVsAtv, off));
    let atv_on = drop(&tables, &bike(&tables, Handling::MxVsAtv, on));
    // The gas gyro takes GasGyroTorque off the pitch lane every airborne substep: nose down.
    assert!(atv_on.x() < atv_off.x(), "gas gyro on {atv_on:?}, off {atv_off:?}");

    let orig_off = drop(&tables, &bike(&tables, Handling::Original, off));
    let orig_on = drop(&tables, &bike(&tables, Handling::Original, on));
    assert_eq!(bits(orig_on), bits(orig_off), "the original's handling read the gyro options");
}
