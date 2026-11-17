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

/// MD71's wiring: a rider nobody owns, put down for no control on its first frame, is put back on the
/// first frame it is driven again on MX vs. ATV — the bike's own unwreck, which asks for no owner, and
/// the clock back to zero — and stays down on the original's handling, whose reset request has no
/// owner to go to. The machine itself is proved in `tests/mxvatv_step.rs`.
#[test]
fn an_unowned_mxvatv_rider_is_put_back_and_the_originals_is_not() {
    use game_sim::rider::{self, Rider};
    use game_sim::trick::{Set, Wreck};
    let tables = rider::Tables::build();
    let ride = |handling: Handling| {
        let p = bike(&tables.ride, handling, Gyro::default());
        let mut r = Rider::new(&p, game_sim::stock::MOUNTS, Vf::new(0.0, 1.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
        r.edge_armed = false;
        r.sense.owner = None;
        r.sense.ai_route = false;
        let mut floor = Flat { y: F::ZERO, normal: Vf::new(0.0, 1.0, 0.0, 0.0) };
        let first = r.frame(&p, &tables, &Controls::default(), DT, &mut floor);
        assert_eq!(first.wreck, Some(Wreck::NO_CONTROL), "{handling:?}");
        assert_eq!(r.state.elapsed, p.wreck_times.max_wreck_time, "{handling:?}: the clock preloaded");
        r.sense.ai_route = true;
        let second = r.frame(&p, &tables, &Controls::default(), DT, &mut floor);
        (second.respawned, r.state)
    };
    let (back, state) = ride(Handling::MxVsAtv);
    assert!(back, "MX vs. ATV puts the rider back");
    assert_eq!((state.wreck, state.elapsed, state.counting), (None, F::ZERO, true));
    let (back, state) = ride(Handling::Original);
    assert!(!back, "the original's machine has no owner to put the rider back for");
    assert_eq!(state.wreck, Some(Wreck::NO_CONTROL));
}

/// MD72's wiring: an airborne MX vs. ATV rider whose body rays strike a wall wrecks at reason 15
/// ([`game_sim::air::Wreck::Struck`]) when the wall's collision file is the stadium class's
/// (`Col\SxCl.col`, [`game_sim::world::stadium_class`]), and the same wall under another name does
/// not; on the original's handling the name changes nothing. The class rule is proved against
/// SLUS-21104 in `tests/mxvatv_air.rs`. Needs the disc's bike and its body rays (skips without).
#[test]
fn an_airborne_mxvatv_rider_wrecks_struck_on_a_stadium_wall_only() {
    use game_render::disc::Disc;
    use game_sim::bike_col::{self, BikeCol};
    use game_sim::collide::{Bundle, Record};
    use game_sim::contact_search::Heightfield;
    use game_sim::rider::{self, Rider};
    use game_sim::track::{Mesh, Node, Placement, Points, Triangle};
    use game_sim::trick::{Set, Wreck};
    use game_sim::world::{Prop, Props, World};
    use std::sync::Arc;

    struct Yard {
        floor: Flat,
        props: Props,
    }
    impl World for Yard {
        fn ground(&mut self, at: Vf) -> (Vf, Vf) {
            self.floor.ground(at)
        }
        fn solid(&mut self, bundle: &Bundle, record: &mut Record) -> bool {
            self.props.sweep(bundle, record)
        }
        fn heightfield(&mut self) -> Option<Box<dyn Heightfield + '_>> {
            Some(Box::new(self.floor))
        }
        fn collision(&mut self) -> (Option<Box<dyn Heightfield + '_>>, Option<&Props>) {
            (Some(Box::new(self.floor)), Some(&self.props))
        }
    }
    // A wall across the path at `z`, twenty feet wide and tall, wound to face the rider (-z).
    let wall = |z: f32, file: &str| {
        let v = |x: f32, y: f32| [F(x), F(y), F(z)];
        let tri = |vertices| Triangle { distance: F::ZERO, normal: [F::ZERO; 3], vertices };
        let mesh = Mesh {
            version: 0,
            surface: None,
            transform: [[F::ONE, F::ZERO, F::ZERO], [F::ZERO, F::ONE, F::ZERO], [F::ZERO, F::ZERO, F::ONE], [F::ZERO; 3]],
            vertices: Points::Wide(vec![v(-10.0, 0.0), v(10.0, 0.0), v(10.0, 20.0), v(-10.0, 20.0)]),
            normals: None,
            nodes: vec![Node {
                volume: F::ZERO,
                centre: [F::ZERO, F(10.0), F(z)],
                half_extent: [F(10.0), F(10.0), F(0.5)],
                children: [0x8000, 0x8001],
            }],
            triangles: vec![tri([0, 2, 1]), tri([0, 3, 2])],
        };
        Prop::place(mesh, &Placement { file: file.to_string(), ..Placement::default() })
    };

    let tables = Arc::new(rider::Tables::build());
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => return ps2_oracle::skip(&why),
    };
    let res = disc.res("Tracks\\free09_ss_2.res").expect("the split track's archive");
    let read = |name: String| res.read_suffix(&name).unwrap_or_else(|e| panic!("{e}"));
    let col = Arc::new(
        BikeCol::load(&read(bike_col::rays_file(bike_col::MODEL)), &read(bike_col::shape_file(bike_col::MODEL)))
            .expect("the bike's collision files"),
    );
    let ride = |handling: Handling, file: &str| {
        let p = bike(&tables.ride, handling, Gyro::default());
        let mut r = Rider::new(&p, game_sim::stock::MOUNTS, Vf::new(0.0, 4.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
        r.edge_armed = false;
        r.carries_bike_col(Arc::clone(&col));
        let mut yard = Yard { floor: Flat { y: F::ZERO, normal: Vf::new(0.0, 1.0, 0.0, 0.0) }, props: Props { props: vec![wall(1.0, file)] } };
        assert_eq!(yard.props.props[0].stadium, game_sim::world::stadium_class(file));
        // The first frame: the wall stands through the bike, so its body rays strike it at once, as
        // the airborne attitude's reason 8 (asked after reason 15) also fires.
        let out = r.frame(&p, &tables, &Controls::default(), DT, &mut yard);
        assert!(r.ride.touching, "{handling:?} {file}: the body rays touch the wall");
        assert_eq!(r.ride.struck_stadium, game_sim::world::stadium_class(file), "{handling:?} {file}: the struck prop's class");
        out.wreck
    };
    let struck = Wreck::from(game_sim::air::Wreck::Struck);
    let attitude = Some(Wreck(8));
    assert_eq!(ride(Handling::MxVsAtv, "Col\\SxCl.col"), Some(struck), "MX vs. ATV on the stadium wall");
    assert_eq!(ride(Handling::MxVsAtv, "Data\\Col\\sxcl.col"), Some(struck), "MX vs. ATV, the name in another case");
    assert_eq!(ride(Handling::MxVsAtv, "Col\\wall_col.col"), attitude, "MX vs. ATV on a plain wall");
    assert_eq!(ride(Handling::Original, "Col\\SxCl.col"), attitude, "the original's handling on the stadium wall");
    assert_eq!(ride(Handling::Original, "Col\\wall_col.col"), attitude, "the original's handling on a plain wall");
}
