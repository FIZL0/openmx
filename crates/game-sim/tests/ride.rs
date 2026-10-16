//! P2 — the ride, as far as a test without the disc can prove it.
//!
//! This is not a function oracle: `ride.rs` is not a port of one function but the *wiring* of five
//! that are ported already (`move.rs`, `wheel.rs`, `chassis.rs`, `air.rs`, `collide.rs`), plus the
//! answers the original's own callees would have given them. There is no single retail address to run
//! against it, so what is proved here is what wiring can get wrong:
//!
//! 1. **The order is the ground step's order.** [`Ride::frame`] rewrites `move::step`'s spine inline,
//!    because a contact found inside a substep is read four lines later in the same substep and a call
//!    list replayed after the fact would run a frame behind itself. That rewrite is the one thing in
//!    this task that can silently drift from a proved module, so the frame's own ordered list of
//!    callees is compared against the list `move::step` hands out for the same inputs — every call, in
//!    order, with its `dt`.
//! 2. **The ride is deterministic.** The same inputs over the same world ride to the same per-tick
//!    hash, twice, from two separate states — which is what every port after this one is checked with
//!    and the one property a replay fixture is worthless without.
//! 3. **A prop stops the rider.** A rider thrown at a wall ends up on this side of it, with the
//!    body-contact flag the air state reads set, and does not pass through.
//! 4. **The state hash sees the whole ride.** Every field of the walk is distinct and the walk's leaf
//!    count is pinned, so a field added to [`Ride`] without a line in the walk fails here instead of
//!    hashing to the same value for ever.
//!
//! The bike is a `.phy` written in this file rather than the disc's, because the disc's cannot be
//! committed (`AGENT.md`). It is a plausible 250 cc four-stroke and nothing more: no number in it is
//! claimed to be the game's, and no test here asserts a *value* the parameters decide.

use game_sim::collide::{Bundle, Record};
use game_sim::hash::{self, Dump};
use game_sim::phy::Phy;
use game_sim::r#move::{self as mv, Frame};
use game_sim::ride::{Controls, Ride, Tables};
use game_sim::track::{Mesh, Node, Placement, Points, Triangle};
use game_sim::world::{Flat, Prop, Props, World};
use ps2_float::{F, Vf};

/// A bike's parameters, as a `.phy` the ported loader reads — hand-written, not the disc's (see the
/// file's own head).
const BIKE: &str = include_str!("freeride-bike.phy.txt");

/// The frame the whole file runs at: the engine's own clamped step, which is what a 60 Hz frame hands
/// the simulation.
const DT: F = F(1.0 / 60.0);

fn bike(tables: &Tables) -> Phy {
    Phy::read_bytes(BIKE.as_bytes(), &tables.curve)
}

fn mounts(p: &Phy) -> (Vf, Vf) {
    (p.front.body_tire_force_position, p.rear.body_tire_force_position)
}

/// A rider on the start line, facing +z, a foot above a flat floor.
fn rider(p: &Phy) -> Ride {
    Ride::new(p, mounts(p), Vf::new(0.0, 1.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0))
}

/// The dirt: flat, level, one kind of surface, nothing standing on it.
fn floor() -> Flat {
    Flat { y: F::ZERO, normal: Vf::new(0.0, 1.0, 0.0, 0.0) }
}

/// A rider's hands, frame by frame: the throttle on, the stick wandering, the brake coming in at the
/// end — enough that no two frames are the same and every branch of the input layer is reached.
fn hands(k: usize) -> Controls {
    let t = k as f32;
    Controls {
        gas: F(if k % 90 < 70 { 1.0 } else { 0.0 }),
        brake: F(if k % 90 < 70 { 0.0 } else { 0.6 }),
        clutch: F(if k < 12 { 1.0 } else { 0.0 }),
        stick: (F((t * 0.11).sin() * 0.8), F((t * 0.07).cos() * 0.6)),
        stunt: k % 120 > 100,
        dpad_y: k % 150 > 140,
        dpad_dir: if k % 150 > 140 { 1 } else { 0 },
        ..Controls::default()
    }
}

// ---------------------------------------------------------------------------------------------------
// (1) the order is the ground step's order
// ---------------------------------------------------------------------------------------------------

/// One call, as a line a human can read and a test can compare: the callee's name and the scalar it
/// was handed. The vectors are left out on purpose — `Call::Unrotate` carries the chassis'
/// acceleration, and the two sides of this comparison integrate different amounts of it (the ride's
/// integrator runs for real; `move::step` only records that it was called), so comparing the payload
/// would be comparing the two spines' *state* and not their order.
fn order(calls: &[mv::Call]) -> Vec<String> {
    calls
        .iter()
        .map(|c| match *c {
            mv::Call::Preload(dt) => format!("preload {}", dt.0),
            mv::Call::Backflip(dt) => format!("backflip {}", dt.0),
            mv::Call::AirborneControl(dt) => format!("airborne_control {}", dt.0),
            mv::Call::Rotate(_) => "rotate".to_string(),
            mv::Call::Contacts(dt) => format!("contacts {}", dt.0),
            mv::Call::Suspension(side, dt) => format!("suspension {side:?} {}", dt.0),
            mv::Call::TireForces(side, dt) => format!("tire_forces {side:?} {}", dt.0),
            mv::Call::WheelIntegrate(side, dt) => format!("wheel_integrate {side:?} {}", dt.0),
            mv::Call::CollisionBegin(first) => format!("collision_begin {first}"),
            mv::Call::Unrotate(_) => "unrotate".to_string(),
            mv::Call::Integrate(dt) => format!("integrate {}", dt.0),
            mv::Call::SearchReset => "search_reset".to_string(),
            mv::Call::SearchGather => "search_gather".to_string(),
        })
        .collect()
}

/// Everything `move::step` asks of the world, answered the way a bike with no transform and no
/// neighbours would be: this side of the comparison is being run for its *call list*, so the answers
/// only have to be the same on every run.
struct Nothing;

impl mv::Outside for Nothing {
    fn unrotate(&mut self, _frame: Frame, v: Vf) -> Vf {
        v
    }
    fn forward(&mut self, _frame: Frame) -> Vf {
        Vf::new(0.0, 0.0, 1.0, 0.0)
    }
    fn noise(&mut self, _t: F) -> F {
        F::ZERO
    }
    fn set_axes(&mut self, _forward: Vf, _up: Vf) {}
}

#[test]
fn the_frame_hands_its_callees_out_in_the_ground_steps_own_order() {
    let tables = Tables::build();
    let p = bike(&tables);
    let mut ride = rider(&p);
    let mut world = floor();

    // The same frame, on both spines: the ride's own, and `move::step`'s, which is proved against the
    // original over two thousand generated frames in `tests/move.rs`.
    for k in 0..90 {
        let c = hands(k);
        let mine = ride.frame(&p, &tables, &c, DT, &mut world);

        let mut bike = mv::Ride::default();
        let mut chassis = mv::Chassis { has_transform: true, ..mv::Chassis::default() };
        let (mut front, mut rear) = (mv::Wheel::default(), mv::Wheel::default());
        let mut engine = p.engine;
        let step = mv::Step {
            dt: DT,
            balancing: c.balance,
            reverse: c.reverse,
            ..mv::Step::default()
        };
        let theirs = mv::step(
            &p.mv,
            &mut mv::Bike {
                ride: &mut bike,
                chassis: &mut chassis,
                front: &mut front,
                rear: &mut rear,
                engine: &mut engine,
            },
            &step,
            &mv::Tables { acos: &tables.acos, curve: &tables.curve },
            &mut Nothing,
        );

        assert_eq!(
            order(&mine),
            order(&theirs),
            "frame {k}: the ride's spine must hand its callees out in `move::step`'s order"
        );
    }
}

#[test]
fn the_frames_list_is_the_substep_loop_it_claims_to_be() {
    let tables = Tables::build();
    let p = bike(&tables);
    let mut ride = rider(&p);
    let calls = ride.frame(&p, &tables, &Controls::default(), DT, &mut floor());
    let names = order(&calls);

    // `(int)(55 * dt) + 1` substeps, which at 60 Hz is one.
    assert_eq!(names.iter().filter(|n| n.starts_with("integrate")).count(), 1);
    assert_eq!(names.first().map(String::as_str), Some("preload 0.016666668"));
    assert_eq!(&names[names.len() - 2..], ["search_reset", "search_gather"]);
    // The one call in the loop that gets the frame's own `dt` rather than the substep's.
    assert!(names.contains(&"contacts 0.016666668".to_string()));

    // And at a frame long enough to need more than one, the loop runs again and the collision record
    // is opened on the first substep only.
    let long = ride.frame(&p, &tables, &Controls::default(), F(0.1), &mut floor());
    let names = order(&long);
    assert_eq!(names.iter().filter(|n| n.starts_with("integrate")).count(), 6);
    assert_eq!(names.iter().filter(|n| *n == "collision_begin true").count(), 1);
    assert_eq!(names.iter().filter(|n| *n == "collision_begin false").count(), 5);
}

// ---------------------------------------------------------------------------------------------------
// (2) the ride is deterministic
// ---------------------------------------------------------------------------------------------------

/// A run of the ride, as the hash of every tick — which is exactly what a replay fixture holds.
fn ridden(frames: usize) -> Vec<u64> {
    let tables = Tables::build();
    let p = bike(&tables);
    let mut ride = rider(&p);
    let mut world = floor();
    (0..frames)
        .map(|k| {
            ride.frame(&p, &tables, &hands(k), DT, &mut world);
            hash::of(&ride)
        })
        .collect()
}

#[test]
fn the_same_inputs_ride_to_the_same_hash_twice() {
    let a = ridden(600);
    let b = ridden(600);
    assert_eq!(a.len(), 600);
    assert_eq!(a, b, "ten seconds of the same ride must hash the same way twice");

    // And it is a ride, not a freeze: a state that never changed would pass the comparison above
    // while proving nothing at all.
    let distinct: std::collections::BTreeSet<u64> = a.iter().copied().collect();
    assert!(distinct.len() > 500, "the ride has to move: {} distinct hashes of 600", distinct.len());
}

#[test]
fn a_divergence_in_the_ride_is_named_by_its_field() {
    let tables = Tables::build();
    let p = bike(&tables);
    let (mut a, mut b) = (rider(&p), rider(&p));
    for k in 0..30 {
        a.frame(&p, &tables, &hands(k), DT, &mut floor());
        b.frame(&p, &tables, &hands(k), DT, &mut floor());
    }
    assert!(hash::first_difference(&a, &b).is_none(), "two identical rides");

    // One lane of one field, so the difference the walk reports can only be that lane.
    let v = b.chassis.vel;
    b.chassis.vel = Vf::new(v.x(), v.y(), v.z() + 1.0, v.w());
    let why = hash::first_difference(&a, &b).expect("a changed velocity is a changed ride");
    assert_eq!(why.path(), "chassis.vel[2]", "{why}");
}

// ---------------------------------------------------------------------------------------------------
// (3) a prop stops the rider
// ---------------------------------------------------------------------------------------------------

/// A flat floor with things standing on it: what the shell rides, with the terrain query replaced by a
/// plane so that no disc file is needed.
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
}

/// A wall across the rider's path at `z`: two triangles, in one leaf, standing twenty feet wide and
/// ten high. The mesh is already in world units and its placement is the origin, so the prop's two
/// transforms are the identity and the sweep's own space is the world's.
fn wall(z: f32) -> Prop {
    let v = |x: f32, y: f32| [F(x), F(y), F(z)];
    let mesh = Mesh {
        version: 0,
        surface: None,
        transform: [
            [F::ONE, F::ZERO, F::ZERO],
            [F::ZERO, F::ONE, F::ZERO],
            [F::ZERO, F::ZERO, F::ONE],
            [F::ZERO, F::ZERO, F::ZERO],
        ],
        vertices: Points::Wide(vec![v(-10.0, 0.0), v(10.0, 0.0), v(10.0, 10.0), v(-10.0, 10.0)]),
        normals: None,
        nodes: vec![Node {
            volume: F::ZERO,
            centre: [F::ZERO, F(5.0), F(z)],
            half_extent: [F(10.0), F(5.0), F(0.5)],
            children: [0x8000, 0x8001],
        }],
        triangles: vec![
            // Wound so the face the sweep computes off the vertices points back at the oncoming
            // rider: `collide::sweep` takes a back face as no hit at all.
            Triangle { distance: F::ZERO, normal: [F::ZERO; 3], vertices: [0, 2, 1] },
            Triangle { distance: F::ZERO, normal: [F::ZERO; 3], vertices: [0, 3, 2] },
        ],
    };
    Prop::place(mesh, &Placement::default())
}

#[test]
fn a_prop_stops_the_rider() {
    let tables = Tables::build();
    let p = bike(&tables);
    let mut ride = rider(&p);
    let mut yard = Yard { floor: floor(), props: Props { props: vec![wall(30.0)] } };

    // Thrown at the wall rather than ridden into it: what is being proved is the sweep and what the
    // frame does with its record, not the engine's acceleration.
    ride.chassis.vel = Vf::new(0.0, 0.0, 60.0, 0.0);

    let mut touched = false;
    for _ in 0..120 {
        ride.frame(&p, &tables, &Controls::default(), DT, &mut yard);
        touched |= ride.touching;
        let z = ride.position().z();
        assert!(z < 31.0, "the rider went through the wall, to z = {z}");
    }
    assert!(touched, "the body contact the air state reads was never set");
    assert!(ride.position().z() > 20.0, "the rider never reached the wall");

    // The same throw with nothing in the way goes straight past where the wall was, which is what
    // makes the assertion above a test of the wall and not of the bike's drag.
    let mut open = rider(&p);
    open.chassis.vel = Vf::new(0.0, 0.0, 60.0, 0.0);
    let mut nothing = Yard { floor: floor(), props: Props::default() };
    for _ in 0..120 {
        open.frame(&p, &tables, &Controls::default(), DT, &mut nothing);
    }
    assert!(
        open.position().z() > 31.0,
        "with no wall the same throw has to pass it: z = {}",
        open.position().z()
    );
    assert!(!open.touching, "nothing to touch");
}

// ---------------------------------------------------------------------------------------------------
// (4) the state hash sees the whole ride
// ---------------------------------------------------------------------------------------------------

#[test]
fn the_walk_names_every_field_once() {
    let tables = Tables::build();
    let p = bike(&tables);
    let ride = rider(&p);
    let dump = Dump::of(&ride);

    let mut seen = std::collections::BTreeSet::new();
    for f in &dump.fields {
        assert!(seen.insert(f.path.clone()), "two leaves share the path {}", f.path);
    }

    // The two numbers that catch a field added to the struct and not to the walk. Both move on
    // purpose when the ride gains state, and the per-tick hashes of every fixture move with them.
    assert_eq!(dump.fields.len(), 471, "the walk's leaves");
    assert_eq!(std::mem::size_of::<Ride>(), 1800, "the ride's own size");

    // A few paths spelled out, so a reordered walk is a readable failure rather than a number.
    assert_eq!(dump.fields[0].path, "mv.g_peak[0]");
    assert!(dump.get("chassis.quat[3]").is_some());
    assert!(dump.get("front.w.steer_angle").is_some());
    assert!(dump.get("rear.mv.spin").is_some());
    assert!(dump.get("air.hang_time").is_some());
    assert!(dump.get("engine.curve.frac").is_some());
    assert!(dump.get("held.stick_hold").is_some());
    assert_eq!(dump.fields.last().map(|f| f.path.as_str()), Some("touching"));
}
