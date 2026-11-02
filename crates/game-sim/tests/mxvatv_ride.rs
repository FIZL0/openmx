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

/// Rolled out on the flat for a second with the stick hard over: the front wheel's steering command
/// and the held stick after it.
fn hard_over(tables: &Tables, p: &Phy) -> (F, (F, F)) {
    let mut r = Ride::new(p, game_sim::stock::MOUNTS, Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0));
    let mut floor = Flat { y: F::ZERO, normal: Vf::new(0.0, 1.0, 0.0, 0.0) };
    let c = Controls { gas: F(0.5), stick: (F::ONE, F(0.85)), ..Controls::default() };
    for _ in 0..60 {
        r.frame(p, tables, &c, DT, &mut floor);
    }
    (r.front.mv.steer_command, r.held.stick)
}

/// MD63a on a ride: MX vs. ATV's limiter leaves the stick unsquared, and its steer ramp, with the
/// option on, changes the command a held lock asks for. On the original's handling neither is read.
#[test]
fn the_steer_ramp_and_the_unsquared_stick_reach_an_mxvatv_bike_only() {
    let tables = Tables::build();
    let mut atv = bike(&tables, Handling::MxVsAtv, Gyro::default());
    let (off, stick) = hard_over(&tables, &atv);
    assert_eq!(stick, (F::ONE, F(0.85)), "MX vs. ATV's stick is not squared");
    atv.steer_ramp = game_sim::handling::step::SteerRamp { on: true, amount: F(0.2), time: F(0.5) };
    let (on, _) = hard_over(&tables, &atv);
    assert_ne!(on.0.to_bits(), off.0.to_bits(), "the ramp did not reach the ride ({on:?})");

    let mut orig = bike(&tables, Handling::Original, Gyro::default());
    let (a, stick) = hard_over(&tables, &orig);
    assert_eq!(stick, (F::ONE, F(0.85) * F(0.85)), "the original's stick is squared");
    orig.steer_ramp = atv.steer_ramp;
    let (b, _) = hard_over(&tables, &orig);
    assert_eq!(a.0.to_bits(), b.0.to_bits(), "the original's handling read the steer ramp");
}

/// S223 — the backflip's rider side on a ride: the stunt id the clip's start takes (not over a trick
/// held), cleared as the flip ends, and a mid-clip end's reason from the steep test.
#[test]
fn the_backflips_stunt_and_its_mid_clip_reason() {
    use game_sim::air::{Air, Chassis, Step};
    use game_sim::chassis::Mat3;
    use game_sim::handling::{Flip, ride_backflip};
    let tables = Tables::build();
    let p = bike(&tables, Handling::MxVsAtv, Gyro::default()).air;
    // Over on its back with the torque in: the clip's start.
    let flipping = || {
        let mut air = Air::default();
        air.flip_active = true;
        air.flip_armed = true;
        air.flip_torque = Vf::new(-1000.0, 0.0, 0.0, 0.0);
        air
    };
    let over = Mat3::new(Vf::new(1.0, -0.1, 0.0, 0.0), Vf::new(0.0, 0.5, 0.0, 0.0), Vf::ZERO);
    for trick_held in [false, true] {
        let (mut air, mut ch, mut f) = (flipping(), Chassis::default(), Flip::default());
        let st = Step { stunt: trick_held, ..Step::default() };
        ride_backflip(&p, &mut air, &mut ch, &st, [false; 3], &over, &mut f, || panic!("asked the steep test"));
        assert_eq!(f.clip, 1, "the clip starts, trick held {trick_held}");
        assert_eq!(f.stunt, !trick_held, "the stunt id is the backflip's only with no trick held");
        // Down on the ground with the clip still running: the mid-clip wreck, and the id cleared.
        for steep in [false, true] {
            let (mut air, mut ch, mut f) = (air, ch, f);
            air.on_ground = true;
            ride_backflip(&p, &mut air, &mut ch, &Step::default(), [false; 3], &over, &mut f, || steep);
            assert!(f.wrecked && f.struck == steep && f.clip == 0 && !f.stunt, "{f:?}");
        }
    }
}
