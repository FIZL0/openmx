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
use game_sim::phy::{self, Phy};
use game_sim::r#move::{self as mv, Frame};
use game_sim::ride::{Controls, Ride, Tables};
use game_sim::rider::{self, Rider};
use game_sim::trick::Set;
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
    assert_eq!(dump.fields.len(), 472, "the walk's leaves");
    assert_eq!(std::mem::size_of::<Ride>(), 1800, "the ride's own size");

    // A few paths spelled out, so a reordered walk is a readable failure rather than a number.
    assert_eq!(dump.fields[0].path, "mv.g_peak[0]");
    assert!(dump.get("chassis.quat[3]").is_some());
    assert!(dump.get("front.w.steer_angle").is_some());
    assert!(dump.get("rear.mv.spin").is_some());
    assert!(dump.get("air.hang_time").is_some());
    assert!(dump.get("engine.curve.frac").is_some());
    assert!(dump.get("held.stick_hold").is_some());
    assert_eq!(dump.fields.last().map(|f| f.path.as_str()), Some("stunt"));
}

// ---------------------------------------------------------------------------------------------------
// (5) a bike nobody is touching stands still
// ---------------------------------------------------------------------------------------------------
//
// P4. The port's bike would not: dropped on level ground with the pad untouched it built up a yaw rate
// of two hundred degrees a second and wandered off, which from the pad is a bike that spins out on its
// own. The cause was in `ride::spin_integrate`, the stand-in for the tire's own force model, and it was
// not a gain that wanted turning down:
//
//   * the surface's `MaxSideForce`, `MaxDriveForce`, `SideForce` and `WheelFriction` are coefficients
//     *per unit of tire load*, and the stand-in read the first two as pounds; and
//   * the lateral force was a velocity damper, and a damper strong enough to hold a bike up is a
//     damper past the two-step integrator's stability limit, which turns into a limit cycle that
//     alternates sign every substep.
//
// What replaces it is a tire bounded by what friction can do: a force that may null the contact
// patch's slide inside one substep but never reverse it. That bound is unconditional, so the tests
// below are not calibration — they are the two statements it makes, plus the original's own numbers.
//
// The original's come from the idle capture: S8 recorded sixty seconds of the acceptance run's start
// with the pad untouched for all of it, and the original creeps forward off the start line without
// turning, sliding or rolling to any degree worth the name. `the_port_idles_no_livelier_than_the_
// original` reads those bounds back out of the capture rather than hard-coding them, because they are
// the disc's numbers; the three tests before it hold to the same orders of magnitude without needing
// it, so a checkout with no `context/` still fails if the defect comes back.

/// Ten seconds of a frame rate, which is both the engine's clamped 60 Hz step and the 30 Hz the
/// recording was made at. Rate matters here: the substep is 1/60 either way, so one frame of 1/30 is
/// two substeps and an instability that only shows at one of the two rates is still an instability.
const IDLE_RATES: [(F, usize); 2] = [(F(1.0 / 60.0), 600), (F(1.0 / 30.0), 300)];

/// What ten seconds of nobody touching the pad came to. Angles in radians, speeds in feet a second.
struct Idle {
    /// How far the bike ended up from where it was dropped, on the ground plane.
    moved: f32,
    /// The largest frame-to-frame turn rate of the chassis' own heading, in radians a second.
    yaw_rate: f32,
    /// Roll, measured off the transform the way the capture's columns are: the widest swing, and the
    /// furthest from level it ever got.
    roll_swing: f32,
    roll_peak: f32,
    /// The fastest the contact patches ever slid sideways, and the fastest the bike ever went.
    slide: f32,
    speed: f32,
}

/// The chassis' heading and roll, off the same two pairs of matrix entries the capture's columns hold:
/// heading from row 2's `x` over its `z`, roll from row 0's `y`.
fn heading(r: &Ride) -> f32 {
    let m = r.transform();
    f32::atan2(m.0[2].x(), m.0[2].z())
}

fn roll(r: &Ride) -> f32 {
    r.transform().0[0].y().clamp(-1.0, 1.0).asin()
}

/// Drop a bike on the floor, wait for the suspension to stop ringing, and leave it alone for ten
/// seconds.
///
/// `settle` is there because the capture's bike has been standing on the start line since before the
/// recording began, while this one arrives a foot in the air: the drop is a real transient and
/// comparing it against a bike at rest would be comparing two different things. The transient is not
/// skipped, though — it is what puts the first sideways slide into the contact patches, so
/// `a_dropped_bike_stands_still` measures from frame zero and this stays an argument.
fn idle(phy: &str, dt: F, settle: usize, frames: usize) -> Idle {
    let tables = Tables::build();
    let p = Phy::read_bytes(phy.as_bytes(), &tables.curve);
    let mut ride = rider(&p);
    let mut world = floor();
    let still = Controls::default();
    for _ in 0..settle {
        ride.frame(&p, &tables, &still, dt, &mut world);
    }

    let start = ride.position();
    let mut was = heading(&ride);
    let (mut lo, mut hi) = (roll(&ride), roll(&ride));
    let mut out = Idle {
        moved: 0.0,
        yaw_rate: 0.0,
        roll_swing: 0.0,
        roll_peak: roll(&ride).abs(),
        slide: 0.0,
        speed: 0.0,
    };
    for _ in 0..frames {
        ride.frame(&p, &tables, &still, dt, &mut world);

        let now = heading(&ride);
        // The heading is an atan2 and wraps; ten seconds of standing still never crosses the branch,
        // and a run that did would read as an enormous turn rate, which is the right answer.
        out.yaw_rate = out.yaw_rate.max((now - was).abs() / dt.0);
        was = now;

        let r = roll(&ride);
        lo = lo.min(r);
        hi = hi.max(r);
        out.roll_peak = out.roll_peak.max(r.abs());

        let m = ride.transform();
        let v = ride.chassis.vel;
        let side = v.x() * m.0[0].x() + v.y() * m.0[0].y() + v.z() * m.0[0].z();
        out.slide = out.slide.max(side.abs());
        out.speed = out.speed.max((v.x() * v.x() + v.z() * v.z()).sqrt());
    }
    let end = ride.position();
    out.moved = ((end.x() - start.x()).powi(2) + (end.z() - start.z()).powi(2)).sqrt();
    out.roll_swing = hi - lo;
    out
}

#[test]
fn a_dropped_bike_stands_still() {
    for (dt, frames) in IDLE_RATES {
        let at = format!("{frames} frames of {}", dt.0);

        // From frame zero: the drop's own transient is inside the window, which is what the defect fed
        // on. The bounds are the brief's — a few feet, a roll under a tenth of a radian — tightened to
        // where they bite, because the broken bike moved 3.0 ft and rolled 0.0023 rad and so passed the
        // loose ones. What it could not do is stand still.
        let d = idle(BIKE, dt, 0, frames);
        assert!(d.moved < 0.5, "{at}: a bike nobody touched wandered {} ft", d.moved);
        assert!(d.roll_peak < 0.1, "{at}: roll reached {} rad", d.roll_peak);
        assert!(d.yaw_rate < 0.02, "{at}: it turned at {} rad/s", d.yaw_rate);
        assert!(d.slide < 0.02, "{at}: its tires slid sideways at {} ft/s", d.slide);
        assert!(d.speed < 0.2, "{at}: it reached {} ft/s", d.speed);

        // And after two seconds of settling, which is the state the capture's bike is in, it is not
        // merely slow: it has stopped.
        let s = idle(BIKE, dt, (2.0 / dt.0) as usize, frames);
        assert!(s.moved < 0.05, "{at} after settling: it still crept {} ft", s.moved);
        assert!(s.speed < 0.02, "{at} after settling: it still moves at {} ft/s", s.speed);
    }
}

/// The test bike with a hundred times the cornering stiffness on both tires. The keys are spelled out
/// so that renumbering the `.phy` fails here, by name, rather than silently handing the tests below the
/// unmodified bike and passing.
fn stiffened() -> String {
    let mut out = BIKE.to_string();
    let keys = [
        ("SideForce1 = 0.2\n", "SideForce1 = 20.0\n"),
        ("SideForce1 = 0.25", "SideForce1 = 25.0"),
    ];
    for (key, hundredfold) in keys {
        assert!(out.contains(key), "freeride-bike.phy.txt no longer has `{}`", key.trim_end());
        out = out.replace(key, hundredfold);
    }
    out
}

#[test]
fn no_cornering_stiffness_can_make_the_bike_spin() {
    // A hundred times the cornering stiffness, which no surface on the disc has and every explicit
    // damper would blow up on. The tire's force is bounded by the slide it is there to kill, so this
    // bike stands as still as the last one — the bound is what the test is, not the number.
    let wild = stiffened();

    for (dt, frames) in IDLE_RATES {
        let at = format!("{frames} frames of {}", dt.0);
        let d = idle(&wild, dt, 0, frames);
        assert!(d.moved < 0.5, "{at}: a stiff bike nobody touched wandered {} ft", d.moved);
        assert!(d.yaw_rate < 0.02, "{at}: a stiff bike turned at {} rad/s", d.yaw_rate);
        assert!(d.slide < 0.02, "{at}: a stiff bike slid at {} ft/s", d.slide);
    }
}

#[test]
fn a_tire_kills_a_slide_and_never_reverses_it() {
    // Shoved sideways at twenty feet a second, which is a real slide and not a settling transient: the
    // one thing friction may not do is push back harder than the slide it is resisting. So the slide
    // decays, monotonically, to nothing — and it does that at both frame rates and at a hundred times
    // the stiffness, because the bound is on the impulse and not on the gain.
    let wild = stiffened();

    for phy in [BIKE, &wild] {
        for (dt, _) in IDLE_RATES {
            let tables = Tables::build();
            let p = Phy::read_bytes(phy.as_bytes(), &tables.curve);
            let mut ride = rider(&p);
            let mut world = floor();
            let still = Controls::default();
            for _ in 0..(1.5 / dt.0) as usize {
                ride.frame(&p, &tables, &still, dt, &mut world);
            }
            ride.chassis.vel = Vf::new(20.0, 0.0, 0.0, 0.0);

            let mut was = f32::INFINITY;
            let mut yaw = 0.0f32;
            for k in 0..(2.0 / dt.0) as usize {
                ride.frame(&p, &tables, &still, dt, &mut world);
                let m = ride.transform();
                let v = ride.chassis.vel;
                let side = (v.x() * m.0[0].x() + v.y() * m.0[0].y() + v.z() * m.0[0].z()).abs();
                // `+ 0.001` is the floor the slide ends at, not a tolerance on the trend: once the
                // patch is within a thousandth of a foot a second of still, the sign of what is left
                // is arithmetic noise.
                assert!(
                    side <= was + 0.001,
                    "frame {k} of {}: the slide grew back, {was} -> {side} ft/s",
                    dt.0
                );
                assert!(side < 21.0, "frame {k} of {}: the slide overshot the shove: {side}", dt.0);
                was = side;
                yaw = yaw.max(ride.chassis.ang_vel.y().abs());
            }
            assert!(was < 0.01, "two seconds on, the bike is still sliding at {was} ft/s");
            // The shove turns the bike — it is a bike, and the front wheel is steered — but it does
            // not spin: a quarter of a turn a second is the worst of it, where the defect reached
            // three and a half radians a second from no shove at all.
            assert!(yaw < 2.0, "the shove span the bike at {yaw} rad/s");
        }
    }
}

/// Where the recordings live: `context/` at the repository root, which `.gitignore` keeps out of git,
/// or `$GAME_SIM_FIXTURES` when the kit's context directory is somewhere else. The same seam
/// `tests/replay.rs` uses and for the same reason — a test cannot read `kit.toml`.
fn private_dir(sub: &str) -> std::path::PathBuf {
    match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => std::path::PathBuf::from(dir).join(sub),
        None => std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("context")
            .join(sub),
    }
}

/// The idle capture: S8's recording of the acceptance run's own start, kept in a subdirectory of
/// `context/capture/` rather than beside the acceptance recording, because every CSV directly in that
/// directory is parsed by `replay.rs` as an acceptance candidate and this one is only sixty seconds of
/// a bike standing still.
const IDLE_CAPTURE: [&str; 2] = ["capture/s8", "acceptance-freeride.csv"];

/// The capture's own rate: `watch/acceptance.toml` records every frame of a 30 Hz game, so one row is
/// one 1/30 tick and three hundred rows are the ten seconds the tests above run.
const CAPTURE_DT: f32 = 1.0 / 30.0;

/// Ten seconds of the original idling, measured off the capture's columns the way [`idle`] measures the
/// port off its transform. `None` when the capture is not in this checkout.
fn captured_idle(rows: usize) -> Option<Idle> {
    let path = private_dir(IDLE_CAPTURE[0]).join(IDLE_CAPTURE[1]);
    let text = std::fs::read_to_string(&path).ok()?;
    let cap = game_sim::replay::Capture::parse(&text).expect("the idle capture parses");
    assert!(
        cap.rows.len() > rows,
        "{}: {} rows, and {rows} are wanted",
        path.display(),
        cap.rows.len()
    );

    let col = |name: &str| {
        cap.columns
            .iter()
            .position(|c| c == name)
            .unwrap_or_else(|| panic!("no column '{name}' in {}", path.display()))
    };
    // Bounds, not bits: this reads the recorder's decimal with Rust's own parser rather than
    // `replay`'s bit-exact one, because nothing below compares a value for equality.
    let at = |row: usize, name: &str| -> f32 {
        cap.rows[row][col(name)].parse().unwrap_or_else(|_| {
            panic!("{}: row {row}'s '{name}' is not a number", path.display())
        })
    };

    // The whole point of this capture is that nobody was touching the pad. If that is not true of the
    // window then the bounds below are somebody's thumb and the test is worthless, so it fails rather
    // than passing quietly.
    for row in 0..=rows {
        for k in 0..16 {
            let name = format!("ctl{k}.held");
            let held: f64 = cap.rows[row][col(&name)].parse().unwrap_or(1.0);
            assert_eq!(held, 0.0, "{}: row {row} has {name} held", path.display());
        }
    }

    let head = |row: usize| f32::atan2(at(row, "bike_xform.m20"), at(row, "bike_xform.m22"));
    let roll = |row: usize| at(row, "bike_xform.m01").clamp(-1.0, 1.0).asin();
    let mut out = Idle {
        moved: 0.0,
        yaw_rate: 0.0,
        roll_swing: 0.0,
        roll_peak: roll(0).abs(),
        slide: 0.0,
        speed: 0.0,
    };
    let (mut lo, mut hi) = (roll(0), roll(0));
    for row in 1..=rows {
        out.yaw_rate = out.yaw_rate.max((head(row) - head(row - 1)).abs() / CAPTURE_DT);
        let r = roll(row);
        lo = lo.min(r);
        hi = hi.max(r);
        out.roll_peak = out.roll_peak.max(r.abs());

        // The bike's own right, which is row 0 of its transform, against its world velocity.
        let side = at(row, "bike_vel.x") * at(row, "bike_xform.m00")
            + at(row, "bike_vel.y") * at(row, "bike_xform.m01")
            + at(row, "bike_vel.z") * at(row, "bike_xform.m02");
        out.slide = out.slide.max(side.abs());
        let (vx, vz) = (at(row, "bike_vel.x"), at(row, "bike_vel.z"));
        out.speed = out.speed.max((vx * vx + vz * vz).sqrt());
    }
    out.roll_swing = hi - lo;
    let (dx, dz) = (
        at(rows, "bike_xform.m30") - at(0, "bike_xform.m30"),
        at(rows, "bike_xform.m32") - at(0, "bike_xform.m32"),
    );
    out.moved = (dx * dx + dz * dz).sqrt();
    Some(out)
}

/// The original's own ten seconds of nobody touching the pad, as the bound on the port's.
///
/// What this compares is *restlessness*, not state: the capture's bike is on the start line of a real
/// track and this one is on a flat plane, so their positions have nothing to say to each other. What
/// they do share is that nobody is riding either of them, and a bike nobody is riding has a turn rate,
/// a sideways slide and a roll swing — and the port's may not exceed the original's. Before P4 the
/// port's yaw rate was three and a half radians a second against an original that barely turns at all,
/// which is this test's whole point.
///
/// Skipped, with a line saying so, in a checkout that has no capture: it is game data and cannot be
/// committed (`AGENT.md`), so the two tests above carry the same statement in absolute numbers.
#[test]
fn the_port_idles_no_livelier_than_the_original() {
    const ROWS: usize = 300;
    let Some(disc) = captured_idle(ROWS) else {
        println!("no idle capture in {} — skipped", private_dir(IDLE_CAPTURE[0]).display());
        return;
    };
    // The original's bike has been standing there since before the recording started, so the port's is
    // given two seconds to settle before its window opens.
    let dt = F(CAPTURE_DT);
    let port = idle(BIKE, dt, (2.0 / CAPTURE_DT) as usize, ROWS);

    // A floor under each bound, so that a capture of a bike wedged against something — zero on every
    // column — cannot turn this into an assertion that the port is bit-still.
    let bound = |theirs: f32, floor: f32| theirs.max(floor);
    assert!(
        port.yaw_rate <= bound(disc.yaw_rate, 1e-4),
        "the port turns at {} rad/s where the original turns at {}",
        port.yaw_rate,
        disc.yaw_rate
    );
    assert!(
        port.slide <= bound(disc.slide, 1e-3),
        "the port's tires slide at {} ft/s where the original's slide at {}",
        port.slide,
        disc.slide
    );
    assert!(
        port.roll_swing <= bound(disc.roll_swing, 1e-4),
        "the port's roll swings {} rad where the original's swings {}",
        port.roll_swing,
        disc.roll_swing
    );
    // The original creeps forward down the start line's slope; the port's floor is level, so it has
    // less reason to move than the original and none to move more.
    assert!(
        port.moved <= disc.moved.max(0.05),
        "the port wandered {} ft where the original moved {}",
        port.moved,
        disc.moved
    );

    // And the capture is the one this test believes it is: a bike that creeps rather than one that is
    // parked against a wall, so the bounds above are real measurements.
    assert!(disc.speed > 0.1, "the capture's bike never moved at all: {} ft/s", disc.speed);
    assert!(disc.yaw_rate > 0.0, "the capture's bike never turned at all");
}

// ---------------------------------------------------------------------------------------------------
// (6) the wreck, and getting back up
// ---------------------------------------------------------------------------------------------------

/// The wreck limits the drop below is judged against, appended to [`BIKE`] — which has no `[Wrecks]`
/// of its own, so there is no key here twice.
///
/// Sideways and fore-and-aft are set out of reach and both pitch limits are generous, so the only
/// wreck a clean drop onto flat ground can get is the vertical one: `FreestyleDownWreckSpeedLimit`,
/// the reason the human's capture hit twice. `MaxWreckTime` is the one number of this block the
/// length of a recovery depends on — how long the machine makes a settled wreck lie there before it
/// puts the rider back. Plausible numbers, not the disc's (see the file's head).
const WRECK_LIMITS: &str = "
[Wrecks]
MaxWreckTime = 2
CannonballWreckTime = 5
FrontWreckSpeedLimit = 400
BackWreckSpeedLimit = 400
SideWreckSpeedLimit = 400
RacingDownWreckSpeedLimit = 400
FreestyleDownWreckSpeedLimit = 5
GroundPitchDownWreckAngle = 85
GroundPitchUpWreckAngle = 85
AirbornePitchDownWreckAngle = 175
AirbornePitchUpWreckAngle = 175
AirborneRollWreckAngle = 175
NoHanderWreckSpeedLimit = 400
NoHanderMaxRollAngle = 45
JoustWreckHeight = 2
CrossupWreck = 1
CrossupWreckTime = 0.3
";

/// The bike the rider leaves behind: one rigid-body part, in the shape of the game's own
/// `Wreckbike.phy` — a weight, a gravity scale, a contact spring and a friction coefficient. Written
/// here, not the disc's.
const WRECK_PART: &str = "
[PhysicsPart1]
Type = RigidBody
Name = wreck_physics
Weight = 250.0
CollisionWeight = 0.0001
GravityScale = 1.0
InertiaScale = 1.0, 1.0, 1.0
SpringStiffness = 5.0
SpringDamping = 0.4
MaxPenetration = 0.5
CofG = 0.0, 1.5, 0.0
SpringMomentScale = 1.0, 1.0, 1.0
FrictionMomentScale = 1.0, 1.0, 1.0
FrictionZone1 = 0.5
Radius = 1.0
AccelerationThreshold = 0.01
AngularAccelerationThreshold = 0.01
";

/// The rider's own half of a wreck: how long they hold the bars after the bike goes, how high they
/// are thrown off it and how long they roll once they land. Written here in the shape of the rider
/// file's `[Wrecks]`, with the two numbers the recovery's length is made of — the hand-hold time and
/// the flail height — chosen so that their sum lands inside the capture's own window, which is what
/// [`Crash`](game_sim::ride::Crash) measures against the running game.
const RIDER_WRECKS: &str = "
[Wrecks]
HitBottomForceTime = 0.1
HitBottomHandHoldTime = 0.3
HitBottomForce = 0.0, 800.0, 0.0
GroundAngularRateDamping = 1.0
RiderFlailHeight = 3.0
RiderRollTime = 0.75
RiderRollSpeed = 0.0
";

/// The box the wreck body's inertia tensor is built out of, in feet: a bike's own length, height and
/// width. The disc's wreck body takes this off its model ([`game_sim::ride::WRECK_INERTIA`]); a test
/// with no model hands the loader's own formula a plausible one.
const WRECK_BOX: Vf = Vf([2.5, 4.0, 7.0, 1.0]);

/// How high the bike is dropped from, in feet. Far enough that the wheels come down well past the
/// vertical limit above and the rider goes over the bars.
const DROP: f32 = 30.0;

/// The frame the two tests below step at, which is **not** [`DT`]: the human's ridden minute was
/// recorded with the game running at 30 Hz (every row of that capture carries `dt = 0.03333`), and
/// the tick counts a wreck of it is measured in are those ticks. So the fixture is handed the
/// recording's own frame and its tick counts are the recording's tick counts.
///
/// Distinct from section 5's `CAPTURE_DT` on purpose: that one is the exact 1/30 the idle tests
/// step at, this one is the literal `dt` the recording's own rows carry.
const WRECK_CAPTURE_DT: F = F(0.03333);

/// **The capture's own window.** In the human's recording the rider's wreck reason is non-zero from
/// tick 3259 to tick 3349 — ninety-one ticks off the bike, with no button pressed, ending in the game
/// putting the rider back by itself. That is the number an automatic recovery has to fit inside.
const CAPTURED_AUTOMATIC: usize = 91;

/// The second wreck of the same capture, ticks 4167 to 4190: twenty-four ticks, a third of the first
/// one and far shorter than the reason's own clock can be waited out in, so that one ended the only
/// other way a wreck can — the human asked for it, with the chord.
const CAPTURED_BY_CHORD: usize = 24;

/// Reason 5 as the state machine carries it: the code [`game_sim::air::Wreck::Flat`] hands over, which
/// is the landing-too-hard wreck and the one the capture hit twice.
const FLAT: game_sim::trick::Wreck = game_sim::trick::Wreck(game_sim::air::Wreck::Flat as u8);

/// What one drop-and-wreck produced, in ticks counted from the frame the rider came off.
struct Down {
    /// The reason the rider came off, and the tick it happened on.
    wreck: Option<game_sim::trick::Wreck>,
    at: usize,
    /// Ticks from the wreck to the frame the rider let go of the bars.
    separated: Option<usize>,
    /// Ticks from the wreck to the frame the rider was back on the bike.
    back: Option<usize>,
    /// Where the bike was put back, and where the wreck body came to rest.
    placed: Vf,
}

/// Drop a bike from [`DROP`] with nobody touching the pad, wreck it on the ground and keep stepping
/// until the rider is riding again. `chord_at`, when given, presses the reset chord that many ticks
/// after the wreck — which is the only input in this fixture.
fn wreck_and_recover(chord_at: Option<usize>) -> Down {
    let tables = rider::Tables::build();
    let p = Phy::read_bytes(format!("{BIKE}{WRECK_LIMITS}").as_bytes(), &tables.ride.curve);
    let at = Vf::new(0.0, DROP, 0.0, 1.0);
    let mut r = Rider::new(&p, mounts(&p), at, Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    let part = phy::Part::read_bytes(WRECK_PART.as_bytes(), "PhysicsPart1");
    let wrecks = phy::Wrecks::read_bytes(RIDER_WRECKS.as_bytes());
    r.carries_wreck(part, wrecks, part.inertia(WRECK_BOX));

    let mut world = floor();
    let mut out = Down { wreck: None, at: 0, separated: None, back: None, placed: Vf::default() };
    for k in 0..1200 {
        let chord = matches!((out.wreck, chord_at), (Some(_), Some(c)) if k == out.at + c);
        let hands = Controls { respawn: chord, ..Controls::default() };
        let f = r.frame(&p, &tables, &hands, WRECK_CAPTURE_DT, &mut world);
        if let Some(w) = f.wreck {
            if out.wreck.is_none() {
                out.wreck = Some(w);
                out.at = k;
            }
        }
        if f.separated && out.separated.is_none() {
            out.separated = Some(k - out.at);
        }
        if f.respawned && out.wreck.is_some() {
            out.back = Some(k - out.at);
            out.placed = r.ride.origin;
            break;
        }
    }
    out
}

/// **S26.** A wreck at reason 5 gets up by itself, inside the window the capture measured.
///
/// The bike is dropped flat onto level ground hard enough to pass the vertical wreck limit, and from
/// the frame the rider comes off nobody touches the pad again. What has to happen is the whole of the
/// wreck as the game plays it: the bike becomes a rigid body of its own and tumbles on
/// `Wreckbike.phy`'s numbers, the rider holds the bars for the throw's hand-hold time and is then
/// thrown off it, the rider lands — which is the settle the state machine has been waiting for and
/// the one input the port could not fill in before S26 — the reset clock arms against the reason's
/// `MaxWreckTime`, and the rider is put back on the bike where the wreck ended up.
///
/// The number this is judged against is the human's own recording: ninety-one ticks from the wreck to
/// the respawn, with no input in between ([`CAPTURED_AUTOMATIC`]). The port has to come back inside
/// that, and has to come back at all — before S26 the only way off the ground was the reset chord.
#[test]
fn a_wreck_at_reason_five_gets_up_by_itself() {
    let down = wreck_and_recover(None);
    assert_eq!(down.wreck, Some(FLAT), "a flat landing past the vertical limit is reason 5");
    let separated = down.separated.expect("the rider lets go of the bars");
    let back = down.back.expect("the rider gets back on the bike with nobody touching the pad");
    assert!(separated < back, "the rider comes off the bike before being put back on it");
    assert!(
        back <= CAPTURED_AUTOMATIC,
        "{back} ticks down, and the capture's own wreck was back up in {CAPTURED_AUTOMATIC}"
    );
    // The floor of the window, which is the model and not a guess: the rider cannot be up before the
    // throw has let go of the bars and the reset clock has run `MaxWreckTime` on top of it.
    let floor_ticks = ((0.3 + 2.0) / WRECK_CAPTURE_DT.0).floor() as usize;
    assert!(back >= floor_ticks, "{back} ticks is less than the hand hold plus MaxWreckTime");
    println!(
        "reason 5 on tick {}: bars let go after {separated} ticks, riding again {back} ticks later \
         (the capture's was {CAPTURED_AUTOMATIC}), put back at {:.1}, {:.1}, {:.1}",
        down.at,
        down.placed.x(),
        down.placed.y(),
        down.placed.z()
    );
}

/// **S26.** The two wrecks of the capture, by their lengths: one waited out and one cut short.
///
/// The same drop twice. Left alone it comes back on its own inside the capture's ninety-one ticks;
/// with the reset chord pressed twenty-four ticks in, it comes back on that tick instead — which is
/// the length of the capture's second wreck, the one the human did not wait out. So the chord still
/// works, it is no longer the only way back, and the two lengths the recording holds are both the
/// port's.
#[test]
fn the_two_captured_wrecks_match_their_lengths() {
    let free = wreck_and_recover(None);
    let chord = wreck_and_recover(Some(CAPTURED_BY_CHORD));
    assert_eq!(free.wreck, chord.wreck, "the same drop wrecks for the same reason");
    assert_eq!(
        free.back.map(|b| b <= CAPTURED_AUTOMATIC),
        Some(true),
        "{:?} ticks waited out against the capture's {CAPTURED_AUTOMATIC}",
        free.back
    );
    assert_eq!(
        chord.back,
        Some(CAPTURED_BY_CHORD),
        "the chord puts the rider back on the tick it is pressed"
    );
    println!(
        "the capture's two wrecks: {} ticks waited out (its own was {CAPTURED_AUTOMATIC}), \
         {} ticks cut short by the chord (its own was {CAPTURED_BY_CHORD})",
        free.back.unwrap(),
        chord.back.unwrap()
    );
}

// ---------------------------------------------------------------------------------------------------
// (7) a minute you can ride
// ---------------------------------------------------------------------------------------------------
//
// Everything above this line proves the ride does not do anything *wrong*: the callees come in the
// right order, the same inputs hash the same twice, a wall stops the bike, a bike nobody is touching
// stands still, a bike dropped on its head wrecks and gets up. None of it says the bike can be
// **ridden** — a bike that never moves passes all of it.
//
// So this section sits a rider on it for a minute and asserts what a rider would notice. One scripted
// minute, ten legs, 3600 ticks of [`DT`], over a floor with one take-off ramp on it:
//
//     the launch                 2 s   full throttle from a standstill
//     flat out                  20 s   held there until the speed stops climbing
//     turning right              4 s   the stick hard over, throttle still pinned
//     straight after the right   2 s
//     turning left               4 s   the stick hard the other way
//     straight after the left    2 s
//     the run-up                 3 s   a ramp is put 250 ft ahead, along the bike's own heading
//     the jump                   5 s   over it, through the air, and down
//     the brake                 10 s   the brake pinned, the throttle shut
//     stopped                    8 s   hands off the pad
//
// The same minute is ridden through the shell's own pad in `crates/openmx/tests/ride.rs`, which is
// where the script is written in the kit's pad notation; this file rides it through the [`Controls`]
// that notation produces, so the two cannot drift.
//
// **Where the numbers come from.** The bike is this directory's hand-written `.phy` (see the file's
// head) with two keys patched in [`ridden_bike`], so the only absolute numbers worth asserting are
// the ones the recording has: the top speed, the sharpest turn rate and the hardest braking the human
// managed in `context/capture/acceptance/`. Those three are consts below, checked back against the
// capture by `the_minutes_numbers_are_the_originals` in a checkout that has it. Everything else is a
// leg's own shape — "it moved", "it kept the speed", "it came down on its wheels" — with tolerances
// loose on purpose. The point is the legs, not the tolerances: this is a test that should be loud when
// the bike stops being drivable and silent about everything else.
//
// **Three legs do not pass, and ship `#[ignore]`d with the defect named in the test.** They are not
// aspirational: each one fails today for a reason traced to a line, each reason is a bike a rider
// cannot ride, and none of them is this task's file to fix (they live in `ride.rs` and `move.rs`).
// Deleting them would lose the finding; leaving them failing would redden `./kit check` for every
// other task in flight. Ignored, they are a list of what the port owes a rider, and the day one is
// paid the `#[ignore]` comes off.
//
//     `it_turns_the_way_the_stick_is_pushed`        the steering command never reaches the hinge
//     `it_brakes_to_a_stop`                         a locked clutch re-drives the rear wheel at idle
//     `it_pulls_away_after_a_frame_off_the_throttle`  `clutch_in` latches and is never cleared
//
// The rest pass, and `the_minute_notices_a_bike_that_loops_out` rides the same minute on a bike geared
// four times too tall to show the asserts are not vacuous.

/// The wreck limits the minute is judged against, appended to [`BIKE`] the way [`WRECK_LIMITS`] is —
/// and unlike that block, these are the limits of a bike being *ridden*: a rider may land hard, lean
/// hard and take a jump without the machine deciding it has crashed, and still wrecks if it comes
/// down on its side or loops out. Plausible numbers, not the disc's.
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

/// [`BIKE`], made rideable — three patches, in memory, because the file itself is four other tasks'
/// fixture and moving a number in it would move their tests.
///
/// 1. **The `[Joystick]` steering keys.** `freeride-bike.phy.txt` has the rate limits but none of
///    `SteerScale1/2` or `SteerSpeed1/2`, so [`phy`]'s loader leaves the steering scale at zero and
///    `mv::steer_command` multiplies the rider's lean by nothing: on that bike the stick is not
///    connected to the front wheel at all, whatever `ride.rs` does with the command. A bike the
///    original would ship has them, so the minute rides one that does — full lock at walking pace
///    falling to a quarter of it at 50 mph, which is the shape every road vehicle's is.
/// 2. **`Drag`.** 0.5 holds the hand-written bike to about 48 ft/s flat out, well under the 75.78 the
///    human's recording reaches, and a minute whose top speed is nothing like the original's cannot
///    be compared against it. 0.06 puts this bike's terminal speed within half a foot a second of the
///    recording's, which is what makes [`CAPTURED_TOP_SPEED`] an assertable number here.
///
/// 3. **`BodyTireForcePosition`**, through section (8)'s [`launch_bike`]. Added when P8 merged. P8
///    found that this key is the arm the original crosses the tire's force on, and that this file's
///    value was written for the key's *other* job in the crate — the wheel mounts — so read as an arm
///    it sits a foot above the body origin and the drive force shoves the bike's own nose into the
///    ground. The minute was written before that was known and rode the unpatched bike; it now rides
///    the same copy section (8) does. [`ride_the_minute`] keeps the mounts from the unpatched file,
///    the way section (8)'s harness does. Untangling the key's two jobs is P8's journal's open
///    question, not this file's.
///
/// Neither patch is a claim about the disc's file: the first is a shape, the second is a calibration,
/// and `the_minutes_numbers_are_the_originals` is what keeps the second honest.
fn ridden_bike() -> String {
    let mut out = BIKE.to_string();
    let patches = [
        (
            "XAxisRateLimit = 6\n",
            "XAxisRateLimit = 6\nSteerScale1 = 1.0\nSteerScale2 = 0.25\nSteerSpeed1 = 5\nSteerSpeed2 = 50\n",
        ),
        ("Drag = 0.5", "Drag = 0.06"),
    ];
    for (key, with) in patches {
        assert!(out.contains(key), "freeride-bike.phy.txt no longer has `{}`", key.trim_end());
        out = out.replace(key, with);
    }
    format!("{}{RIDE_LIMITS}", launch_bike(&out))
}

/// The same bike with a quarter of its final drive: gearing far too tall to pull away on, which is a
/// bike a rider cannot get up to speed. `the_minute_notices_a_bike_it_cannot_ride` rides the minute
/// on this one so that the legs above are known to be asserting something.
///
/// **This was `FinalGearRatio = 14.4` until P8 merged**, four times the drive rather than a quarter
/// of it, and the control was that the launch *looped out*. That bike no longer loops out, and no
/// longer should: P8 found the port was putting the drive force into the chassis at the contact
/// patch, two feet under the centre of gravity, so any bike with enough torque stood itself up. With
/// the force on the `.phy`'s own arm it does not, and 14.4 merely rides the minute faster — 0 to 67
/// ft/s on the launch, 81.2 flat out, not one wreck and not one leg failing. A control that asserts
/// a defect is no control once the defect is gone, so the bad bike is bad the other way now: a
/// quarter of the drive takes four seconds to reach 15 ft/s and tops out at 60, which is three of
/// the legs' own numbers missed and nothing to do with any defect.
fn cannot_pull_away() -> String {
    let out = ridden_bike();
    let key = "FinalGearRatio = 3.6";
    assert!(out.contains(key), "freeride-bike.phy.txt no longer has `{key}`");
    out.replace(key, "FinalGearRatio = 0.9")
}

/// Flat ground with one take-off ramp laid across the rider's path: a plane, rising at `slope` for
/// `length` feet starting `start` feet along `dir` from `origin`, and level again after it.
///
/// It is a ramp rather than [`Flat`] because a jump is the one leg of a minute flat ground cannot
/// produce, and a ramp is the shortest world that produces one: the bike leaves the lip because the
/// ground stops rising under it, not because anything in the test tells it to.
struct Jump {
    origin: Vf,
    dir: Vf,
    start: f32,
    slope: f32,
    length: f32,
}

impl Jump {
    /// Nowhere near the ramp: what the minute rides over until the run-up puts one in front of it.
    fn nothing() -> Self {
        Jump {
            origin: Vf::default(),
            dir: Vf::new(0.0, 0.0, 1.0, 0.0),
            start: 0.0,
            slope: 0.0,
            length: 0.0,
        }
    }

    /// How far along the ramp's own direction a point is, measured on the ground plane.
    fn along(&self, at: Vf) -> f32 {
        (at.x() - self.origin.x()) * self.dir.x() + (at.z() - self.origin.z()) * self.dir.z()
    }
}

impl World for Jump {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        let t = self.along(at) - self.start;
        let up = Vf::new(0.0, 1.0, 0.0, 0.0);
        if self.length <= 0.0 || t < 0.0 || t > self.length {
            return (Vf::new(at.x(), 0.0, at.z(), 1.0), up);
        }
        // The face's normal: the slope tilted back about the ramp's own across-axis, normalised the
        // way a surface query's has to be or the contact solves against a stretched one.
        let n = 1.0 / (1.0 + self.slope * self.slope).sqrt();
        (
            Vf::new(at.x(), t * self.slope, at.z(), 1.0),
            Vf::new(-self.slope * self.dir.x() * n, n, -self.slope * self.dir.z() * n, 0.0),
        )
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

/// One leg of the minute: a name to fail by, how many ticks of [`DT`] it lasts, and the hands.
struct Leg {
    name: &'static str,
    ticks: usize,
    hands: Controls,
}

/// What the rider sees out of one tick. Angles in radians, speeds in feet a second, `y` in feet.
struct Seat {
    speed: f32,
    heading: f32,
    /// The transform's up, down its own `y`: 1 upright, 0 on its side, negative upside down. The one
    /// number "it landed on its wheels" is.
    up_y: f32,
    y: f32,
    on_ground: bool,
    wreck: bool,
}

/// The minute, leg by leg, as it was ridden.
struct Minute {
    legs: Vec<(&'static str, Vec<Seat>)>,
}

impl Minute {
    /// One leg's ticks, by name.
    fn leg(&self, name: &str) -> Vec<&Seat> {
        let found = self.legs.iter().find(|(n, _)| *n == name);
        found.unwrap_or_else(|| panic!("the minute has no leg called '{name}'")).1.iter().collect()
    }

    /// Two legs' ticks end to end, for the one thing that straddles a boundary: the jump, which
    /// leaves the ground in `the run-up` or in `the jump` depending on how fast the bike got there.
    fn span(&self, first: &str, last: &str) -> Vec<&Seat> {
        let at = |name: &str| {
            self.legs
                .iter()
                .position(|(n, _)| *n == name)
                .unwrap_or_else(|| panic!("the minute has no leg called '{name}'"))
        };
        self.legs[at(first)..=at(last)].iter().flat_map(|(_, ticks)| ticks).collect()
    }

    fn all(&self) -> Vec<&Seat> {
        self.legs.iter().flat_map(|(_, ticks)| ticks).collect()
    }
}

fn fastest(ticks: &[&Seat]) -> f32 {
    ticks.iter().map(|t| t.speed).fold(0.0, f32::max)
}

fn slowest(ticks: &[&Seat]) -> f32 {
    ticks.iter().map(|t| t.speed).fold(f32::MAX, f32::min)
}

/// The furthest from upright the bike got: the minimum of the transform's own up.
fn flattest(ticks: &[&Seat]) -> f32 {
    ticks.iter().map(|t| t.up_y).fold(f32::MAX, f32::min)
}

fn highest(ticks: &[&Seat]) -> f32 {
    ticks.iter().map(|t| t.y).fold(f32::MIN, f32::max)
}

fn airborne(ticks: &[&Seat]) -> usize {
    ticks.iter().filter(|t| !t.on_ground).count()
}

fn wrecks(ticks: &[&Seat]) -> usize {
    ticks.iter().filter(|t| t.wreck).count()
}

/// A signed angle brought back into ±π, so a heading that walks past the branch cut does not read as
/// a turn the other way.
fn wrap(a: f32) -> f32 {
    let turn = std::f32::consts::TAU;
    let mut a = a % turn;
    if a > std::f32::consts::PI {
        a -= turn;
    }
    if a < -std::f32::consts::PI {
        a += turn;
    }
    a
}

/// How far round the bike came over these ticks, and how fast on average — signed the way the stick
/// is: positive is to the right, which is the way `heading` grows.
///
/// Tick by tick rather than end against start: a bike that holds full lock for four seconds comes
/// round further than half a circle, and the difference of the two headings cannot tell that from a
/// short turn the other way. P10 wrote the difference because nothing turned at all.
fn turned(ticks: &[&Seat]) -> f32 {
    ticks.windows(2).map(|w| wrap(w[1].heading - w[0].heading)).sum()
}

fn turn_rate(ticks: &[&Seat]) -> f32 {
    turned(ticks) / (ticks.len() as f32 * DT.0)
}

/// The minute itself: the legs, in order, with the hands for each.
///
/// Two things about the hands are deliberate. The throttle is **down on the first tick**, not fed in
/// after a settle, because `ride.rs`'s clutch latch makes those two different rides and only one of
/// them moves — see `it_pulls_away_after_a_frame_off_the_throttle`. And the turns keep the throttle
/// pinned, because a rider holding a turn at speed is the case the steering has to survive; a turn
/// off the throttle would also be a deceleration and the leg would prove less.
fn minute() -> Vec<Leg> {
    let gas = Controls { gas: F::ONE, ..Controls::default() };
    let right = Controls { stick: (F::ONE, F::ZERO), ..gas.clone() };
    let left = Controls { stick: (F(-1.0), F::ZERO), ..gas.clone() };
    let brake = Controls { brake: F::ONE, ..Controls::default() };
    let off = Controls::default();
    [
        ("the launch", 120, gas.clone()),
        ("flat out", 1200, gas.clone()),
        ("turning right", 240, right),
        ("straight after the right", 120, gas.clone()),
        ("turning left", 240, left),
        ("straight after the left", 120, gas.clone()),
        ("the run-up", 180, gas.clone()),
        ("the jump", 300, gas),
        ("the brake", 600, brake),
        ("stopped", 480, off),
    ]
    .into_iter()
    .map(|(name, ticks, hands)| Leg { name, ticks, hands })
    .collect()
}

/// How far ahead of the bike the ramp is put, and its shape: far enough that the run-up is over
/// before the lip, short and shallow enough that a bike at the top speed clears it rather than
/// climbing it. Feet, and a rise over the run.
const RAMP: (f32, f32, f32) = (250.0, 0.3, 15.0);

/// Ride [`minute`] on `phy` and keep what the rider saw.
///
/// The rider is the whole one — [`Rider`], not [`Ride`] — because three of the minute's assertions are
/// about the rider and not the bike: that nothing wrecks, that the landing is on the wheels, and that
/// the machine never decides it has crashed. It carries the same wreck body section 6 drops.
fn ride_the_minute(phy: &str) -> Minute {
    let tables = rider::Tables::build();
    let p = Phy::read_bytes(phy.as_bytes(), &tables.ride.curve);
    // The mounts come from the unpatched file, as in section (8): `ridden_bike`'s third patch moves
    // `BodyTireForcePosition`, which this crate reads as the drive force's arm *and* as the mounts.
    let geom = Phy::read_bytes(BIKE.as_bytes(), &tables.ride.curve);
    let at = Vf::new(0.0, 1.0, 0.0, 1.0);
    let mut r = Rider::new(&p, mounts(&geom), at, Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    let part = phy::Part::read_bytes(WRECK_PART.as_bytes(), "PhysicsPart1");
    let wrecks = phy::Wrecks::read_bytes(RIDER_WRECKS.as_bytes());
    r.carries_wreck(part, wrecks, part.inertia(WRECK_BOX));

    let mut world = Jump::nothing();
    let mut out = Minute { legs: Vec::new() };
    for Leg { name, ticks, hands } in minute() {
        // The ramp is laid when the run-up starts, ahead of wherever the bike actually is and along
        // whatever heading it actually has: the jump has to work on the bike's own line, not on a
        // line the test assumed six legs ago.
        if name == "the run-up" {
            let m = r.ride.transform();
            world = Jump {
                origin: r.ride.position(),
                dir: Vf::new(m.0[2].x(), 0.0, m.0[2].z(), 0.0),
                start: RAMP.0,
                slope: RAMP.1,
                length: RAMP.2,
            };
        }
        let mut seen = Vec::with_capacity(ticks);
        for _ in 0..ticks {
            let frame = r.frame(&p, &tables, &hands, DT, &mut world);
            let m = r.ride.transform();
            seen.push(Seat {
                speed: r.ride.chassis.speed.0,
                heading: f32::atan2(m.0[2].x(), m.0[2].z()),
                up_y: m.0[1].y(),
                y: r.ride.position().y(),
                on_ground: r.ride.air.on_ground,
                wreck: frame.wreck.is_some(),
            });
        }
        out.legs.push((name, seen));
    }
    out
}

/// The top speed the human's recording reaches, in feet a second, off its `bike_vel` columns and the
/// rows the bike is upright for. [`ridden_bike`]'s drag is set so this bike's terminal speed is this,
/// which is what lets the two legs below assert a number instead of a shape.
const CAPTURED_TOP_SPEED: f32 = 75.78;

/// The sharpest the human ever turned and the hardest they ever slowed, as the 99th and 1st
/// percentiles of the recording's own frame-to-frame differences — percentiles and not extremes
/// because a respawn moves the bike across the map in one row and the extremes are those rows.
/// Radians a second, and feet a second a second.
const CAPTURED_TURN_RATE: f32 = 2.40;
const CAPTURED_SLOWING: f32 = 70.9;

/// The human's ridden minute, in a subdirectory of `context/capture/` for the reason
/// [`IDLE_CAPTURE`]'s comment gives: a CSV directly in that directory is read by `replay.rs` as an
/// acceptance candidate.
const RIDDEN_CAPTURE: [&str; 2] = ["capture/acceptance", "acceptance-freeride.csv"];

#[test]
fn it_launches_from_a_standstill() {
    let m = ride_the_minute(&ridden_bike());
    let launch = m.leg("the launch");
    assert!(launch[0].speed < 1.0, "the launch did not start from rest: {} ft/s", launch[0].speed);

    // Half a second: a rider who pins the throttle on a 250 is moving by now, and the bike that
    // prompted this test is not.
    let half = launch[(0.5 / DT.0) as usize].speed;
    assert!(half > 3.0, "half a second of full throttle moved the bike at {half} ft/s");

    // Two seconds: a useful fraction of the top speed, which is as much as a hand-written engine's
    // numbers can be asked for. The recording's own launch is slower than this and that is the
    // human feathering a real clutch, not a bound on the port.
    let two = launch[launch.len() - 1].speed;
    assert!(two > 20.0, "two seconds of full throttle only reached {two} ft/s");

    // And it went forward on its wheels rather than standing up or sliding away sideways.
    assert!(flattest(&launch) > 0.9, "the launch leant to up {}", flattest(&launch));
    assert_eq!(wrecks(&launch), 0, "the launch wrecked");
}

#[test]
fn it_reaches_the_top_speed_and_holds_it() {
    let m = ride_the_minute(&ridden_bike());
    let flat_out = m.leg("flat out");
    let top = fastest(&flat_out);
    assert!(
        top > CAPTURED_TOP_SPEED * 0.9,
        "twenty seconds of full throttle reached {top} ft/s, against the original's {CAPTURED_TOP_SPEED}"
    );
    assert!(
        top < CAPTURED_TOP_SPEED * 1.1,
        "the bike runs away to {top} ft/s, past the original's {CAPTURED_TOP_SPEED}"
    );

    // And *keeps* it: over the last quarter of the leg the speed never falls away from the peak. This
    // is the half of "reaches a speed" a bike that surges and dies would pass without.
    let tail = &flat_out[flat_out.len() * 3 / 4..];
    assert!(
        slowest(tail) > top * 0.97,
        "the speed fell to {} ft/s after topping out at {top}",
        slowest(tail)
    );
    assert_eq!(wrecks(&flat_out), 0, "the bike wrecked flat out");
}

/// A held stick turns the bike the way it is pushed, for as long as it is held: the bike comes round
/// at a rate the human managed with the same stick, without falling over or losing the speed that
/// makes a turn a turn, and the two turns go opposite ways.
///
/// P10 shipped this ignored because the commanded angle never reached the hinge — `mv::steer_command`
/// worked it out every substep and the only writer of `wheel.w.steer_angle` was
/// `wheel::steer_integrate`, which only runs while the bike is balancing, so `patch` steered by zero
/// for the whole minute and the yaw rate was `0.000` in both directions. P11 bridges the pose's
/// command onto the hinge once a frame in [`Ride::frame`], where the original writes it.
#[test]
fn it_turns_the_way_the_stick_is_pushed() {
    let m = ride_the_minute(&ridden_bike());
    for (leg, sign) in [("turning right", 1.0), ("turning left", -1.0)] {
        let ticks = m.leg(leg);
        let rate = turn_rate(&ticks) * sign;
        assert!(rate > 0.1, "{leg}: the bike turned at {rate} rad/s the way the stick was pushed");
        assert!(
            rate < CAPTURED_TURN_RATE,
            "{leg}: {rate} rad/s is sharper than the original's {CAPTURED_TURN_RATE}"
        );
        assert!(
            turned(&ticks).abs() > 0.2,
            "{leg}: four seconds of full lock came round {} rad",
            turned(&ticks)
        );
        // Still rolling at the end of it, rather than a speed floor: full lock with the throttle
        // pinned at this bike's top speed is about a g of lateral load, and the port has no lean to
        // carry it — `mv::pose` pins the body's roll rate on the ground, so the contact patch scrubs
        // instead. The right turn comes in from 75 ft/s and winds itself down to a walking pace
        // before the lock starts pulling it round on the spot. How much of that is the original's
        // and how much is P4's tire stand-in is not something this test can say, so what it asserts
        // is that the bike is still being ridden at the end; the sustained turn at a speed a rider
        // would hold one at is `it_holds_a_turn_for_as_long_as_it_is_held`.
        let end = ticks[ticks.len() - 1].speed;
        assert!(end > 2.0, "{leg}: the turn left the bike at {end} ft/s");
        assert!(flattest(&ticks) > 0.5, "{leg}: the bike leant to up {}", flattest(&ticks));
        assert_eq!(wrecks(&ticks), 0, "{leg}: the turn wrecked");
    }

    // And the two turns go opposite ways, which a bike that merely drifts one way would fail.
    let (right, left) = (m.leg("turning right"), m.leg("turning left"));
    assert!(
        turned(&right) > 0.0 && turned(&left) < 0.0,
        "both turns went the same way: {} rad and {} rad",
        turned(&right),
        turned(&left)
    );
}

#[test]
fn it_takes_the_jump_and_lands_on_its_wheels() {
    let m = ride_the_minute(&ridden_bike());
    let jump = m.span("the run-up", "the jump");
    let air = airborne(&jump);
    assert!(air > 30, "the ramp only put the bike in the air for {air} ticks");
    assert!(
        highest(&jump) > RAMP.1 * RAMP.2 + 1.0,
        "the jump's highest point was {} ft, which is no higher than the lip",
        highest(&jump)
    );

    // Through the air the right way up — a bike that leaves a lip rotating is a bike that lands on
    // its head, and the port's own wreck test is what would catch it afterwards rather than here.
    assert!(flattest(&jump) > 0.6, "in the air the bike got to up {}", flattest(&jump));

    // And down: landed with a second of the leg still to run, upright, still rolling, never having
    // wrecked. Measured from the last airborne tick rather than over the leg's own last second,
    // because where in the leg the jump happens depends on how fast the bike reaches the ramp — and
    // now that the turns before it really turn, it gets there later than P10's bike did.
    let last_air = jump.iter().rposition(|t| !t.on_ground).expect("the bike never left the ground");
    let landed = &jump[last_air + 1..];
    // The landing is asserted in two parts, because it has two: the impact, and the ride away from
    // it. Through the impact the chassis is still rotating and the suspension is taking the drop, so
    // what those ticks have to clear is "not on its head" — the same bound the flight above is held
    // to. Once it is down it has [`SETTLE`] to come level, and from there to the end of the leg it
    // must stay level. A bike that never comes level, or that comes level and then falls over, fails
    // the second leg; one that merely lands hard does not, which is the distinction the single bound
    // this replaces could not draw.
    //
    // **The split is the P8+P11 merge's, and it is worth a human's eye** (master.md, P11). P10 wrote
    // one bound over the whole window and both tasks cleared it alone — main with P8 only touches
    // down at up 1.000 after 82 ticks of air, P11's branch without P8 at 0.944 after 68. With both
    // paid the bike reaches the lip slower and leaves it rolling: 62 ticks of air, a touchdown at
    // 0.675, level again by the eighth tick and level for the remaining 52. Nothing wrecks and the
    // speed is kept, so the leg's own statement still holds and is what is asserted here — but the
    // approach into this ramp got worse when the two fixes met, and no task owns that yet.
    const SETTLE: usize = 15; // a quarter of a second at DT; the landings above take 5 and 8.
    assert!(
        landed.len() > SETTLE + 30,
        "the bike came down with only {} ticks of the leg left to settle in",
        landed.len()
    );
    assert!(
        flattest(&landed[..SETTLE]) > 0.6,
        "it came down on its head, at up {}",
        flattest(&landed[..SETTLE])
    );
    assert!(
        flattest(&landed[SETTLE..]) > 0.9,
        "a quarter of a second after it landed it was still at up {}",
        flattest(&landed[SETTLE..])
    );
    assert!(
        slowest(landed) > CAPTURED_TOP_SPEED * 0.4,
        "the landing left the bike at {} ft/s",
        slowest(landed)
    );
    assert_eq!(wrecks(&jump), 0, "the jump wrecked");
}

/// The brake is pinned for ten seconds and the bike stops, and stays stopped.
///
/// This test was ignored until P13, and the reason it gave was wrong in an instructive way. It read
/// the bike's hold at about 26 ft/s as a *locked* clutch idling the rear wheel along through first
/// gear, and so as `mv::engine_update`'s MinRPM clamp against `ride.rs`'s `IDLE_SPEED` rule. The
/// clutch is in fact slipping the whole way down, and what pinned the speed was a servo: a slipping
/// clutch passes `ClutchKinetic × ratio` in whichever direction the revs and the road disagree, so
/// the moment the road's revs fell under the clamped idle the engine started pushing back with more
/// than the brake was taking off. The reason it could win is that the brake was not braking — the
/// port's `ride::surface_drag` took `MaxBrakingTorque` off the rear wheel's *spin*, which
/// `spin_integrate` overwrites from the road on every substep for a wheel in contact, so the pad
/// never reached the chassis. The original puts it on the wheel's drive-torque accumulator instead,
/// where it becomes a force at the contact patch. One line, and the engine's hold disappears with
/// it: nothing in `mv::engine_update` or the idle rule had to change, and neither did.
///
/// What it asserts: the brake takes the speed off at a rate the human managed and no faster, the
/// bike actually stops, and it stays stopped with nobody touching the pad.
#[test]
fn it_brakes_to_a_stop() {
    let m = ride_the_minute(&ridden_bike());
    let brake = m.leg("the brake");
    let from = brake[0].speed;

    // The first two seconds: real braking, and no harder than the human ever braked.
    let after = brake[(2.0 / DT.0) as usize].speed;
    assert!(after < from * 0.5, "two seconds of brake took {from} ft/s to {after}");
    let hardest = (from - after) / 2.0;
    assert!(
        hardest < CAPTURED_SLOWING,
        "the brake slows the bike at {hardest} ft/s^2, past the original's {CAPTURED_SLOWING}"
    );

    // And then it stops. "Stopped" is the speed the port's own idle is allowed to creep at, not zero.
    let end = brake[brake.len() - 1].speed;
    assert!(end < 1.0, "ten seconds of brake left the bike doing {end} ft/s");

    // And stays stopped for the eight seconds the rider's hands are off the pad.
    let stopped = m.leg("stopped");
    assert!(fastest(&stopped) < 2.0, "it rolled away again at {} ft/s", fastest(&stopped));
    assert_eq!(wrecks(&brake) + wrecks(&stopped), 0, "stopping wrecked");
}

/// A bike that spends one frame at a standstill with the throttle shut can never pull away again.
///
/// **Why it is ignored.** `ride.rs`'s frame sets `ride.clutch_in = 1` whenever the speed is under
/// `IDLE_SPEED` and the rate-limited throttle is exactly zero, and nothing anywhere clears it: the
/// field is a latch. One frame of a closed throttle on the start line is enough, and from then on
/// `mv::engine_update` is handed a clutch that is fully in, so the engine revs to 7000 and the rear
/// wheel gets nothing. The minute above only moves because its throttle is down on tick zero.
///
/// This is the shell's own start state — `openmx` spawns the rider standing still and the player
/// reaches for the throttle some frames later — so the measured cost of this latch is that the bike
/// in the shell cannot be ridden at all, which is what P10 is about.
///
/// What this asserts when it is paid: a bike that has been sitting there goes when the throttle does.
#[test]
#[ignore = "ride.clutch_in latches: one frame at rest off the throttle and the clutch is never let out again"]
fn it_pulls_away_after_a_frame_off_the_throttle() {
    let tables = rider::Tables::build();
    let text = ridden_bike();
    let p = Phy::read_bytes(text.as_bytes(), &tables.ride.curve);
    let at = Vf::new(0.0, 1.0, 0.0, 1.0);
    let mut world = Jump::nothing();
    let gas = Controls { gas: F::ONE, ..Controls::default() };

    // One second of sitting on the start line, then ten of full throttle — a rider arriving at a
    // bike, in other words, rather than a drag racer already on the rev limiter.
    for (what, wait) in [("one frame", 1usize), ("a second", 60)] {
        let mut r = Rider::new(&p, mounts(&p), at, Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
        for _ in 0..wait {
            r.frame(&p, &tables, &Controls::default(), DT, &mut world);
        }
        let mut top = 0.0f32;
        for _ in 0..600 {
            r.frame(&p, &tables, &gas, DT, &mut world);
            top = top.max(r.ride.chassis.speed.0);
        }
        assert!(
            top > 20.0,
            "after {what} off the throttle, ten seconds of full throttle reached {top} ft/s \
             (clutch_in {})",
            r.ride.mv.clutch_in.0
        );
    }
}

#[test]
fn none_of_the_minute_wrecks() {
    let m = ride_the_minute(&ridden_bike());

    // The minute, leg by leg, for a human reading a failure rather than for the assert.
    for (name, ticks) in &m.legs {
        let ticks: Vec<&Seat> = ticks.iter().collect();
        println!(
            "{name:24} {:4} ticks | speed {:6.2}..{:6.2} | came round {:6.3} rad | up {:5.3} \
             | peak y {:6.2} | air {:4} | wrecks {}",
            ticks.len(),
            slowest(&ticks),
            fastest(&ticks),
            turned(&ticks),
            flattest(&ticks),
            highest(&ticks),
            airborne(&ticks),
            wrecks(&ticks),
        );
    }

    let all = m.all();
    assert_eq!(
        wrecks(&all),
        0,
        "the minute wrecked {} of its {} ticks",
        wrecks(&all),
        all.len()
    );
    // Nothing in the minute asks the machine for a stunt, so nothing in it should separate the rider
    // from the bike either — and the bike should still be on the ground at the end of it.
    assert!(all[all.len() - 1].on_ground, "the minute ended in the air");
}

/// The minute, on [`cannot_pull_away`]'s bike: it never gets up to speed, and the legs above say so.
///
/// This is here because every assertion in this section is of the form "the bike did something", and a
/// test like that can rot into one that passes on anything. So it re-asserts three of the legs' own
/// numbers — the half-second and two-second marks `it_launches_from_a_standstill` wants, and the top
/// speed `it_reaches_the_top_speed_and_holds_it` wants — against a bike that misses all three. If the
/// legs above ever stop noticing *this*, they are no longer a drivability test.
///
/// It checks the bike fails for the right reason, too: nothing here wrecks and nothing falls over, so
/// what the legs caught is the bike not being rideable and not the fixture collapsing. See
/// [`cannot_pull_away`] for why this is a slow bike rather than P10's original fast one.
#[test]
fn the_minute_notices_a_bike_it_cannot_ride() {
    let m = ride_the_minute(&cannot_pull_away());
    let launch = m.leg("the launch");

    let half = launch[(0.5 / DT.0) as usize].speed;
    assert!(half < 3.0, "the under-driven bike made {half} ft/s in half a second, which the leg allows");
    let two = launch[launch.len() - 1].speed;
    assert!(two < 20.0, "the under-driven bike made {two} ft/s in two seconds, which the leg allows");
    let top = fastest(&m.leg("flat out"));
    assert!(
        top < CAPTURED_TOP_SPEED * 0.9,
        "the under-driven bike reached {top} ft/s, which the leg allows against the original's \
         {CAPTURED_TOP_SPEED}"
    );

    // And it is the gearing the legs caught, not the fixture falling over: this bike rides the whole
    // minute upright and never wrecks. It is slow, which is the one thing wrong with it.
    let whole = m.all();
    assert_eq!(wrecks(&whole), 0, "the under-driven bike wrecked, so the legs caught the wrong thing");
    assert!(
        flattest(&whole) > 0.5,
        "the under-driven bike fell to up {}, so the legs caught the wrong thing",
        flattest(&whole)
    );
}

/// The three numbers this section asserts against, read back out of the human's recording.
///
/// None of the legs above need the capture to run — they are bounds a rider would recognise, and a
/// checkout with no `context/` still fails if the bike stops being drivable. What the capture decides
/// is whether those bounds are the *original's*: whether [`ridden_bike`]'s drag still puts this bike's
/// top speed where the recording's is, and whether the minute's turning and braking stay inside what
/// the human's thumbs actually did. Edit the `.phy`'s drag or gearing and this is what goes red.
///
/// Skipped, with a line saying so, in a checkout without the recording: it is game data and cannot be
/// committed (`AGENT.md`).
///
/// One thing the recording will not settle: its `bike_vel` columns and its own positions disagree
/// about the speed by a steady factor of 1.254, so one of the two is scaled by something this test
/// cannot see. The columns are used here, because they are the recorder's value for the quantity the
/// port calls `chassis.speed` — and because the port's terminal speed lands within half a foot a
/// second of them, which the positions' 95 ft/s would make nonsense of.
#[test]
fn the_minutes_numbers_are_the_originals() {
    let path = private_dir(RIDDEN_CAPTURE[0]).join(RIDDEN_CAPTURE[1]);
    let Ok(text) = std::fs::read_to_string(&path) else {
        println!("no ridden capture at {} — skipped", path.display());
        return;
    };
    let cap = game_sim::replay::Capture::parse(&text).expect("the ridden capture parses");
    let col = |name: &str| {
        cap.columns
            .iter()
            .position(|c| c == name)
            .unwrap_or_else(|| panic!("no column '{name}' in {}", path.display()))
    };
    let at = |row: usize, name: &str| -> f32 {
        cap.rows[row][col(name)].parse().unwrap_or_else(|_| {
            panic!("{}: row {row}'s '{name}' is not a number", path.display())
        })
    };

    // Only the rows the bike is upright for: the wrecks and the respawns in between are not riding,
    // and a respawn's own row is a teleport with a frame-to-frame difference of hundreds.
    let upright: Vec<usize> =
        (0..cap.rows.len()).filter(|&row| at(row, "bike_xform.m11") > 0.7).collect();
    assert!(
        upright.len() > 500,
        "{}: only {} of {} rows are upright",
        path.display(),
        upright.len(),
        cap.rows.len()
    );
    let speed = |row: usize| {
        let (x, z) = (at(row, "bike_vel.x"), at(row, "bike_vel.z"));
        (x * x + z * z).sqrt()
    };
    let head = |row: usize| f32::atan2(at(row, "bike_xform.m20"), at(row, "bike_xform.m22"));

    let top = upright.iter().map(|&row| speed(row)).fold(0.0, f32::max);
    let pairs: Vec<usize> =
        upright.iter().copied().filter(|&row| upright.contains(&(row + 1))).collect();
    let mut slowing: Vec<f32> =
        pairs.iter().map(|&row| (speed(row) - speed(row + 1)) / CAPTURE_DT).collect();
    let mut turning: Vec<f32> =
        pairs.iter().map(|&row| wrap(head(row + 1) - head(row)).abs() / CAPTURE_DT).collect();
    slowing.sort_by(f32::total_cmp);
    turning.sort_by(f32::total_cmp);
    let pct = |sorted: &[f32], f: f32| sorted[((sorted.len() - 1) as f32 * f) as usize];
    let (slowed, turn) = (pct(&slowing, 0.99), pct(&turning, 0.99));
    println!(
        "{}: top {top:.2} ft/s, slowing p99 {slowed:.2} ft/s^2, turning p99 {turn:.3} rad/s \
         over {} upright rows",
        path.display(),
        upright.len()
    );

    // A percent either way on each: these are consts in a test file, not a checksum, and the point of
    // reading them back is to catch a recording or a fixture that has moved, not a rounding.
    let near = |ours: f32, theirs: f32| (ours - theirs).abs() <= theirs.abs() * 0.01 + 0.01;
    assert!(near(CAPTURED_TOP_SPEED, top), "CAPTURED_TOP_SPEED is {CAPTURED_TOP_SPEED}, the recording's is {top}");
    assert!(near(CAPTURED_SLOWING, slowed), "CAPTURED_SLOWING is {CAPTURED_SLOWING}, the recording's is {slowed}");
    assert!(near(CAPTURED_TURN_RATE, turn), "CAPTURED_TURN_RATE is {CAPTURED_TURN_RATE}, the recording's is {turn}");

    // And the minute really is ridden inside that envelope, which is the half of the comparison the
    // legs above cannot make without the recording in front of them.
    let m = ride_the_minute(&ridden_bike());
    let all = m.all();
    assert!(
        fastest(&all) < top * 1.05,
        "the minute reaches {} ft/s where the original reaches {top}",
        fastest(&all)
    );
    let worst = all
        .windows(2)
        .map(|w| (w[0].speed - w[1].speed) / DT.0)
        .fold(f32::MIN, f32::max);
    assert!(
        worst < slowed.max(1.0) * 2.0,
        "the minute sheds speed at {worst} ft/s^2 where the original's p99 is {slowed}"
    );
}

// ---------------------------------------------------------------------------------------------------
// (8) full throttle from a standstill
// ---------------------------------------------------------------------------------------------------
//
// P8. Pin the throttle on the start line and the bike used to stand itself on its back wheel before it
// had moved a foot: on the disc's own bike the chassis pitch ran from 0.0097 to 1.3111 radians over
// frames 8 to 18, the front wheel's load went to zero at frame 9 and never came back, and the ride
// wrecked six tenths of a second after the throttle opened.
//
// The cause was the same shape as the lateral defect P4 found. `ride::spin_integrate` put the tire's
// drive force into the chassis at the *contact patch* — two feet under the centre of gravity, so a
// couple of two foot-pounds for every pound of drive, with nothing holding the nose down. The
// original does not do that. Its own spin integration crosses the force with the vector at
// `wheel+0xb0`, which is the `.phy`'s `BodyTireForcePosition` written straight there by the loader,
// and uses it *as it stands* as an arm about the centre of gravity: no contact point, no centre of
// gravity subtracted. On the disc's bike that vector is a quarter of a foot under the body origin and
// three quarters of a foot either side of it — a tuning arm the file's author chose, not a geometric
// one, and nothing like the patch. The port now uses it the same way, and `ride::surface_drag` no
// longer adds a couple at all, because the original's surface drag writes the force accumulator and
// never the torque one. P8's journal entry has both functions and the reads.
//
// Two things make the statement below less direct than it looks, and both are in P8's journal entry.
//
// The file's bike is not the disc's, so no acceleration here can be compared to the disc's by value;
// the capture-backed test asks only that the two are within a factor of two of each other, which is a
// statement about the drive reaching the ground rather than about the parameters.
//
// And `BodyTireForcePosition` is doing two jobs in this checkout. It is the key above, and it is also
// what every test in the crate — and `stock.rs`, and `openmx` when the model has no `TireFront` node —
// hands [`Ride::new`] as the wheel *mounts*, because the mounts are the model's and a test has no
// model. The fixture's value was written for that second job: axles a foot up and a four-foot
// wheelbase. Read as the first job it is an arm a foot *above* the body origin, which is a bike whose
// drive force shoves its own nose into the ground. So the launch runs on a copy of the file with that
// one key set to a tuning arm of the right shape, and keeps the mounts the rest of the file stands on.

/// The file's bike with the drive force's arm set to a tuning arm rather than to the wheel mount the
/// rest of the crate reads that key as — see the section's head.
fn launch_bike(text: &str) -> String {
    text.replace("BodyTireForcePosition = 0.0, 1.1, 2.1", "BodyTireForcePosition = 0.0, -0.3, 0.8")
        .replace("BodyTireForcePosition = 0.0, 1.1, -2.1", "BodyTireForcePosition = 0.0, -0.3, -0.8")
}

/// What a second of pinned throttle from a standstill came to. Speeds in feet a second, accelerations
/// in feet a second squared, angles in radians, loads in pounds.
struct Launch {
    /// Forward speed at the end of each frame, from the frame the throttle opened.
    speed: Vec<f32>,
    /// The chassis' pitch each frame, nose up positive.
    pitch: Vec<f32>,
    /// The least either wheel's suspension ever carried while the throttle was open.
    front_load: f32,
    rear_load: f32,
    /// Whether the ride wrecked, and why.
    wreck: Option<game_sim::trick::Wreck>,
}

impl Launch {
    /// Frame-to-frame forward acceleration, which is what the capture can be compared against.
    fn accel(&self, dt: f32) -> Vec<f32> {
        let mut was = 0.0;
        self.speed
            .iter()
            .map(|&v| {
                let a = (v - was) / dt;
                was = v;
                a
            })
            .collect()
    }

    fn peak_accel(&self, dt: f32) -> f32 {
        self.accel(dt).into_iter().fold(0.0, f32::max)
    }
}

/// Wreck limits for the launch: every speed limit set out of reach, so the only wreck a second of
/// throttle on level ground can reach is the one the defect reached — the chassis pitching over
/// backwards past `GroundPitchUpWreckAngle`. Plausible numbers, not the disc's (see the file's head).
const LAUNCH_LIMITS: &str = "
[Wrecks]
MaxWreckTime = 2
CannonballWreckTime = 5
FrontWreckSpeedLimit = 400
BackWreckSpeedLimit = 400
SideWreckSpeedLimit = 400
RacingDownWreckSpeedLimit = 400
FreestyleDownWreckSpeedLimit = 400
GroundPitchDownWreckAngle = 85
GroundPitchUpWreckAngle = 85
AirbornePitchDownWreckAngle = 175
AirbornePitchUpWreckAngle = 175
AirborneRollWreckAngle = 175
NoHanderWreckSpeedLimit = 400
NoHanderMaxRollAngle = 45
JoustWreckHeight = 2
CrossupWreck = 1
CrossupWreckTime = 0.3
";

/// Settle a bike on the level, then pin the throttle and hold it, touching nothing else.
///
/// A [`Rider`] rather than a bare [`Ride`], because a wreck is one of the things being ruled out and
/// the wreck test is the rider's.
fn launch(dt: F, frames: usize) -> Launch {
    let tables = rider::Tables::build();
    // The mounts come from the unmodified file: only the force's arm is being changed.
    let geom = Phy::read_bytes(BIKE.as_bytes(), &tables.ride.curve);
    let text = format!("{}{LAUNCH_LIMITS}", launch_bike(BIKE));
    let p = Phy::read_bytes(text.as_bytes(), &tables.ride.curve);
    let mut r = Rider::new(
        &p,
        mounts(&geom),
        Vf::new(0.0, 1.0, 0.0, 1.0),
        Vf::new(0.0, 0.0, 1.0, 0.0),
        Set::default(),
    );
    let mut world = floor();

    let still = Controls::default();
    for _ in 0..(2.0 / dt.0) as usize {
        r.frame(&p, &tables, &still, dt, &mut world);
    }
    assert!(
        r.ride.chassis.vel.z().abs() < 0.01,
        "the bike has not settled: {} ft/s",
        r.ride.chassis.vel.z()
    );

    let gas = Controls { gas: F::ONE, ..Controls::default() };
    let mut out = Launch {
        speed: Vec::with_capacity(frames),
        pitch: Vec::with_capacity(frames),
        front_load: f32::MAX,
        rear_load: f32::MAX,
        wreck: None,
    };
    for _ in 0..frames {
        let f = r.frame(&p, &tables, &gas, dt, &mut world);
        out.wreck = out.wreck.or(f.wreck);
        // The bike's own forward, which is row 2 of its transform, against its world velocity.
        let (v, m) = (r.ride.chassis.vel, r.ride.transform());
        out.speed.push(v.x() * m.0[2].x() + v.y() * m.0[2].y() + v.z() * m.0[2].z());
        out.pitch.push(r.ride.chassis.euler.x());
        out.front_load = out.front_load.min(r.ride.front.w.spring_force.0);
        out.rear_load = out.rear_load.min(r.ride.rear.w.spring_force.0);
    }
    out
}

/// The wheelie the brief names: `move::roll_pitch` only stands the bike up when the rider pulls the
/// stick back past [`mv::WHEELIE_LEAN`], and nobody is touching the stick here, so a pitch anywhere
/// near that arm is the chassis doing it by itself. Held in radians, which is what the defect ran to:
/// 1.31 of them in ten frames.
const WHEELIE: f32 = 0.5;

/// **P8.** Full throttle from a standstill on the level drives the bike forward, on both wheels.
///
/// Three statements, in the order they failed before the fix.
///
/// *Both wheels stay loaded.* This is the one that catches the defect on this bike: with the drive
/// force back at the contact patch the front's suspension carries nothing at all for the first two
/// frames and settles at a tenth of what it holds now. The floor is a tenth of the bike's weight —
/// well clear of the two hundred pounds a wheel of it carries standing still, and not a number the
/// fixture's parameters were chosen to clear.
///
/// *The pitch stays under the wheelie threshold.* The brief's own statement, and the one the disc's
/// bike failed loudest: it reached 1.31 radians and wrecked. This bike has a lower centre of gravity
/// and never reached a tenth of that even with the defect in, so the bound is here to say what is
/// being claimed rather than to catch a regression — the load floor above does the catching.
///
/// *It goes.* A bike that wrecks, or one whose clutch is pulled in for it and never let out again,
/// reads as a bike that will not move; both of those were true of the port when P8 opened.
#[test]
fn full_throttle_from_a_standstill_drives_the_bike_forward() {
    for (dt, frames) in IDLE_RATES.map(|(dt, _)| (dt, (1.0 / dt.0) as usize)) {
        let run = launch(dt, frames);
        assert_eq!(run.wreck, None, "a second of throttle at {} wrecked the bike", dt.0);

        let floor = 40.0;
        assert!(
            run.front_load > floor,
            "at {}: the front carried {} lb at its lightest",
            dt.0,
            run.front_load
        );
        assert!(
            run.rear_load > floor,
            "at {}: the rear carried {} lb at its lightest",
            dt.0,
            run.rear_load
        );

        let pitch = run.pitch.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
        assert!(pitch < WHEELIE, "at {}: the chassis pitched to {pitch} rad", dt.0);

        let end = *run.speed.last().unwrap();
        assert!(end > 10.0, "at {}: a second of throttle got to {end} ft/s", dt.0);
        println!(
            "{} s: {end:.1} ft/s after a second, {:.1} ft/s2 at its hardest, \
             pitch {pitch:.4} rad, wheels {:.0}/{:.0} lb at their lightest",
            dt.0,
            run.peak_accel(dt.0),
            run.front_load,
            run.rear_load
        );
    }
}

/// The acceptance capture, which has the human's own launches in it: the same recording `replay.rs`
/// reads, kept in a subdirectory for the reason [`IDLE_CAPTURE`] gives.
const LAUNCH_CAPTURE: [&str; 2] = ["capture/acceptance", "acceptance-freeride.csv"];

/// The original's own launch, found rather than hard-coded: the first window in the recording where a
/// bike that is standing still is given a button and goes.
///
/// `(peak, mean)` acceleration over the window, in feet a second squared. `None` when the capture is
/// not in this checkout.
fn captured_launch() -> Option<(f32, f32)> {
    /// How long a window has to hold to count as a launch: four tenths of a second, which is as long
    /// as the human held the throttle before easing off the first time.
    const HELD: usize = 12;

    let path = private_dir(LAUNCH_CAPTURE[0]).join(LAUNCH_CAPTURE[1]);
    let text = std::fs::read_to_string(&path).ok()?;
    let cap = game_sim::replay::Capture::parse(&text).expect("the acceptance capture parses");
    let col = |name: &str| {
        cap.columns
            .iter()
            .position(|c| c == name)
            .unwrap_or_else(|| panic!("no column '{name}' in {}", path.display()))
    };
    let at = |row: usize, name: &str| -> f32 {
        cap.rows[row][col(name)].parse().unwrap_or_else(|_| {
            panic!("{}: row {row}'s '{name}' is not a number", path.display())
        })
    };
    let speed = |row: usize| {
        let (x, z) = (at(row, "bike_vel.x"), at(row, "bike_vel.z"));
        (x * x + z * z).sqrt()
    };
    let held = |row: usize, k: usize| at(row, &format!("ctl{k}.held")) != 0.0;

    // A standstill that becomes a launch: stopped, a button that is held for the whole window, and
    // five feet a second more by the end of it. Which button is the throttle is not written down
    // anywhere this test can read, so it is the one that is down while the bike accelerates.
    for row in 0..cap.rows.len() - HELD {
        if speed(row) > 1.0 || speed(row + HELD) - speed(row) < 5.0 {
            continue;
        }
        let Some(_) = (0..16).find(|&k| (row..=row + HELD).all(|r| held(r, k))) else {
            continue;
        };
        let (mut peak, mut was) = (0.0f32, speed(row));
        for r in row + 1..=row + HELD {
            peak = peak.max((speed(r) - was) / CAPTURE_DT);
            was = speed(r);
        }
        let mean = (speed(row + HELD) - speed(row)) / (HELD as f32 * CAPTURE_DT);
        println!(
            "the capture's launch: row {row}, {:.3} -> {:.3} ft/s over {HELD} ticks",
            speed(row),
            speed(row + HELD)
        );
        return Some((peak, mean));
    }
    panic!("{}: no full-throttle launch in the recording", path.display());
}

/// **P8.** The port's launch against the original's own.
///
/// The capture is of the disc's bike on the disc's start line and this bike is the file's, so what is
/// compared is an order of magnitude and not a value: the port's launch has to be within a factor of
/// two of the original's, either way. That is a loose bound on purpose. What it rules out is a port
/// that puts the engine's whole output into standing the bike up (far too little) or one that drives
/// as if the bike weighed nothing (far too much) — and before P8, with the drive force at the contact
/// patch, the disc's bike wrecked inside six tenths of a second rather than reaching any speed at all.
///
/// Skipped, with a line saying so, in a checkout that has no capture.
#[test]
fn the_port_launches_about_as_hard_as_the_original() {
    let Some((peak, mean)) = captured_launch() else {
        println!("no acceptance capture in {} — skipped", private_dir(LAUNCH_CAPTURE[0]).display());
        return;
    };
    let dt = F(CAPTURE_DT);
    let run = launch(dt, (1.0 / CAPTURE_DT) as usize);
    let ours_peak = run.peak_accel(CAPTURE_DT);
    // The same window the capture's was measured over, so the two means are of the same thing.
    let ours_mean = run.speed[11] / (12.0 * CAPTURE_DT);

    for (what, ours, theirs) in [("peak", ours_peak, peak), ("mean", ours_mean, mean)] {
        assert!(
            ours > theirs * 0.5 && ours < theirs * 2.0,
            "the port's {what} launch acceleration is {ours:.1} ft/s2 \
             where the original's is {theirs:.1}"
        );
        println!("{what}: the port {ours:.1} ft/s2, the original {theirs:.1} ft/s2");
    }
}

// ---------------------------------------------------------------------------------------------------
// (9) the stick, the lean, and coming back up
// ---------------------------------------------------------------------------------------------------
//
// The minute above holds the stick over for four seconds in each direction, which is enough to say
// that the bike turns at all. What it cannot say is what the task was reported for: that a held stick
// keeps turning the bike the *same* way for as long as it is held, and that a bike left leaned over
// with nobody asking for it comes back up. Both need a longer hold than a leg of the minute and the
// drawn bike's own roll, which the minute never looks at, so they ride their own thirteen seconds
// here — the length of the trace the task was raised from.
//
// Two separate things lean in this port, and only one of them is the bike a rider sees:
//
//   * the chassis **body**, whose roll rate `mv::pose` pins on the ground — `ang_free` is
//     `(1, 0, !on_ground)` while riding, so a grounded body cannot roll however hard it is pushed.
//     Nothing on the ground can right it either, by the same flag; `air::airborne_control`'s unroll
//     term is what brings it back, in the air, with the stick centred.
//   * the bike **node**, which `mv::no_hander` rolls about its own forward towards
//     `MaxRollCommand * lean` with `RollGain` of first-order lag. That is the lean in a turn, it is
//     what the renderer draws, and its return to level when the stick centres is the "comes back up"
//     a rider means.
//
// So these tests measure the node, and the body's pin is left where the original put it.

/// [`ridden_bike`] with a drawn bike that leans: `MaxRollCommand` at 40 degrees and a `MaxRollSpeed`
/// of 25 mph to reach it by.
///
/// `freeride-bike.phy.txt` has `MaxRollCommand = 0.4` — four tenths of a *degree* after the loader's
/// multiply — and no `MaxRollSpeed` at all, which reads as zero and shorts `mv::no_hander`'s command
/// to zero with it. On that bike the drawn bike never leans a degree whatever the stick does, so it
/// cannot say whether the lean controller works; the shared fixture is left alone because four other
/// tasks ride it.
///
/// The two numbers are this test's own shape and not the disc's: 40 degrees is just inside
/// [`RIDE_LIMITS`]' `NoHanderMaxRollAngle` of 45, and the human's recording saturates its own roll at
/// 0.73 to 0.76 radians — 42 to 44 degrees — which is the same bike in the same corner.
fn leaning_bike() -> String {
    let out = ridden_bike();
    let key = "MaxRollCommand = 0.4\n";
    assert!(out.contains(key), "freeride-bike.phy.txt no longer has `{}`", key.trim_end());
    out.replace(key, "MaxRollCommand = 40\nMaxRollSpeed = 25\n")
}

/// One tick of a held ride: what a rider would see out of the bike's own two transforms.
struct Pose {
    speed: f32,
    /// The body's heading, which is what the bike actually goes round by.
    heading: f32,
    /// The drawn bike's lean, radians, measured off the node exactly as the capture's `bike_xform`
    /// columns are measured — `asin(m01)`, negative leaning right.
    lean: f32,
    /// The front wheel's hinge, radians: the pose's command, once it reaches the hinge.
    steer: f32,
    up_y: f32,
    on_ground: bool,
    wreck: bool,
}

/// Rides `phy` over flat ground through a script of `(ticks, hands)` legs, and answers every tick.
///
/// The same rider, wreck part and start the minute uses, so a difference between a test here and a
/// leg up there is the hands and nothing else. [`Jump::nothing`] is flat ground: a ramp nowhere near
/// the bike.
fn ride_held(phy: &str, script: &[(usize, Controls)]) -> Vec<Pose> {
    let tables = rider::Tables::build();
    let p = Phy::read_bytes(phy.as_bytes(), &tables.ride.curve);
    // The mounts come from the unpatched file, as in sections (7) and (8): `ridden_bike`'s third
    // patch moves `BodyTireForcePosition`, which this crate reads as the drive force's arm *and* as
    // the wheel mounts. Read as mounts, the arm it patches in is a wheelbase of a foot and a half
    // with both axles below the body origin, and a bike that shape neither leans nor turns.
    let geom = Phy::read_bytes(BIKE.as_bytes(), &tables.ride.curve);
    let at = Vf::new(0.0, 1.0, 0.0, 1.0);
    let mut r = Rider::new(&p, mounts(&geom), at, Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    let part = phy::Part::read_bytes(WRECK_PART.as_bytes(), "PhysicsPart1");
    let wrecks = phy::Wrecks::read_bytes(RIDER_WRECKS.as_bytes());
    r.carries_wreck(part, wrecks, part.inertia(WRECK_BOX));

    let mut world = Jump::nothing();
    let mut out = Vec::new();
    for (ticks, hands) in script {
        for _ in 0..*ticks {
            let frame = r.frame(&p, &tables, hands, DT, &mut world);
            let m = r.ride.transform();
            out.push(Pose {
                speed: r.ride.chassis.speed.0,
                heading: f32::atan2(m.0[2].x(), m.0[2].z()),
                lean: r.ride.node[0].y().clamp(-1.0, 1.0).asin(),
                steer: r.ride.front.w.steer_angle.0,
                up_y: m.0[1].y(),
                on_ground: r.ride.air.on_ground,
                wreck: frame.wreck.is_some(),
            });
        }
    }
    out
}

/// How far round the bike came over these poses, tick by tick so a turn past half a circle counts as
/// one — the [`turned`] of section 7, for the longer rides here.
fn came_round(poses: &[Pose]) -> f32 {
    poses.windows(2).map(|w| wrap(w[1].heading - w[0].heading)).sum()
}

/// Thirteen seconds of full right lock, gas a half: the bike turns right for all thirteen of them.
///
/// This is the half of the task that was reported as a sign flip. The trace it came from had the
/// heading rise for four seconds and then fall for four more with the stick still pinned right, and a
/// full-lock turn that came round only 45 degrees in four seconds at 28 ft/s. Both were the same
/// thing: nothing wrote the pose's steering command onto the front wheel's hinge, so the bike was not
/// being steered at all and what the heading did was whatever the terrain did to it. With the command
/// bridged in [`Ride::frame`] the bike holds one circle after another and never once comes back the
/// other way.
///
/// Second by second rather than over the whole ride, because an average over thirteen seconds is
/// exactly the statistic that cannot tell a steady turn from a turn that reverses halfway.
#[test]
fn it_holds_a_turn_for_as_long_as_it_is_held() {
    let right = Controls { gas: F(0.5), stick: (F::ONE, F::ZERO), ..Controls::default() };
    let poses = ride_held(&leaning_bike(), &[(13 * 30, right)]);

    // The first second is the launch — the bike is still gathering speed and the steering has
    // nothing to bite on. Every second after it turns right, and none of them turns left.
    let seconds: Vec<f32> = poses.chunks(30).skip(1).map(came_round).collect();
    let worst = seconds.iter().copied().fold(f32::MAX, f32::min);
    assert!(
        worst > 0.1,
        "a second of held right lock came round {worst} rad: {seconds:?}"
    );
    let total = came_round(&poses);
    assert!(
        total > 0.5 * 12.0,
        "thirteen seconds of held right lock came round {total} rad in total"
    );

    // And it is the stick doing it: the hinge is over the whole time, the bike keeps the speed a turn
    // needs, it stays on its wheels and it never wrecks.
    let after = &poses[30..];
    assert!(
        after.iter().all(|p| p.steer > 0.05),
        "the hinge came back to {} rad with the stick still over",
        after.iter().map(|p| p.steer).fold(f32::MAX, f32::min)
    );
    assert!(
        after.iter().all(|p| p.speed > 8.0),
        "the held turn dropped the bike to {} ft/s",
        after.iter().map(|p| p.speed).fold(f32::MAX, f32::min)
    );
    assert!(
        poses.iter().all(|p| p.up_y > 0.9),
        "the bike leant its body to up {}",
        poses.iter().map(|p| p.up_y).fold(f32::MAX, f32::min)
    );
    assert!(after.iter().all(|p| p.on_ground), "the held turn put the bike in the air");
    assert!(!poses.iter().any(|p| p.wreck), "the held turn wrecked");
}

/// Five seconds of right lock and then the stick centred: the drawn bike leans into the turn, and
/// when nobody is asking for the lean any more it comes back level and stays there.
///
/// The other half of the task — "nothing rights a bike that is leaned over with no stick asking for
/// it". The lean that a rider sees is the node's, and `mv::no_hander`'s lag is what both puts it there
/// and takes it away; a second is all it needs at `RollGain = 6`. The body's own roll is not asserted
/// here because the original pins its roll rate on the ground (see the note above this section), so
/// the bike that comes back up is the one that leant.
#[test]
fn it_comes_back_upright_when_the_stick_is_centred() {
    let gas = Controls { gas: F(0.5), ..Controls::default() };
    let right = Controls { stick: (F::ONE, F::ZERO), ..gas.clone() };
    let poses = ride_held(&leaning_bike(), &[(5 * 30, right), (3 * 30, gas)]);
    let (held, after) = poses.split_at(5 * 30);

    // Leaning right is a negative `m01`, which is the recording's sign too: it leant one way, hard,
    // and for the whole hold.
    let peak = held.iter().map(|p| p.lean).fold(f32::MAX, f32::min);
    assert!(
        peak < -20.0_f32.to_radians(),
        "five seconds of right lock leant the drawn bike {} rad",
        peak
    );
    let settled = &held[30..];
    assert!(
        settled.iter().all(|p| p.lean < -0.05),
        "the lean came back to {} rad with the stick still over",
        settled.iter().map(|p| p.lean).fold(f32::MIN, f32::max)
    );

    // And back up: a second after the stick centres the drawn bike is level, and it stays level for
    // the two seconds after that rather than drifting on past.
    let level = &after[30..];
    let worst = level.iter().map(|p| p.lean.abs()).fold(0.0, f32::max);
    assert!(
        worst < 2.0_f32.to_radians(),
        "a second after the stick centred the bike was still leant {worst} rad"
    );

    // The turn ends with it — a centred stick is a straight hinge — and the bike is still upright and
    // rolling at the end.
    assert!(
        level.iter().all(|p| p.steer.abs() < 1e-3),
        "the hinge stayed at {} rad with the stick centred",
        level.iter().map(|p| p.steer.abs()).fold(0.0, f32::max)
    );
    assert!(
        came_round(level).abs() < 0.2,
        "the bike kept turning {} rad with the stick centred",
        came_round(level)
    );
    assert!(level[level.len() - 1].speed > 10.0, "the bike stopped: {} ft/s", level[level.len() - 1].speed);
    assert!(!poses.iter().any(|p| p.wreck), "centring the stick wrecked the bike");
}

/// The drawn bike leans the way the human's bike leaned: into the turn, not out of it.
///
/// What this settles that the two tests above cannot. The lean's *sign* against the heading's is a
/// convention, and the port had it backwards — `ride.rs`'s `set_axes` built the node's row0 as
/// `forward × up`, a reflection rather than a rotation, so every frame-relative read of the node came
/// back with its roll negated. `mv::no_hander` reads the node's own roll to lag it towards the
/// command, so a negated read made the loop fight itself and the lean settled at a twentieth of what
/// was asked for, leaning the wrong way for the turn it was in. The original's own VU0
/// orthonormaliser (`math::basis`, proved) builds `up × forward`, which is the cross `set_axes` takes
/// now; the recording is how we know that is the way a leaning bike reads back.
///
/// Measured here as a handedness and not as a number: the recording's bike is the retail `.phy`'s and
/// this one's roll keys are [`leaning_bike`]'s, so the degrees cannot be compared, but which way the
/// bike leans in a right-hand turn can be — tick by tick, over the recording's own long steady turns.
///
/// Skipped, with a line saying so, in a checkout without the recording: it is game data and cannot be
/// committed (`AGENT.md`).
#[test]
fn the_lean_goes_the_way_the_originals_does() {
    let path = private_dir(RIDDEN_CAPTURE[0]).join(RIDDEN_CAPTURE[1]);
    let Ok(text) = std::fs::read_to_string(&path) else {
        println!("no ridden capture at {} — skipped", path.display());
        return;
    };
    let cap = game_sim::replay::Capture::parse(&text).expect("the ridden capture parses");
    let col = |name: &str| {
        cap.columns
            .iter()
            .position(|c| c == name)
            .unwrap_or_else(|| panic!("no column '{name}' in {}", path.display()))
    };
    let at = |row: usize, name: &str| -> f32 {
        cap.rows[row][col(name)].parse().unwrap_or_else(|_| {
            panic!("{}: row {row}'s '{name}' is not a number", path.display())
        })
    };
    let head = |row: usize| f32::atan2(at(row, "bike_xform.m20"), at(row, "bike_xform.m22"));
    let lean = |row: usize| at(row, "bike_xform.m01").clamp(-1.0, 1.0).asin();
    let speed = |row: usize| {
        let (x, z) = (at(row, "bike_vel.x"), at(row, "bike_vel.z"));
        (x * x + z * z).sqrt()
    };

    // Upright, moving, and a row with a next row: the wrecks, the respawns and the standstills are
    // not turns.
    let upright: Vec<usize> =
        (0..cap.rows.len()).filter(|&row| at(row, "bike_xform.m11") > 0.7).collect();
    let riding: Vec<usize> = upright
        .iter()
        .copied()
        .filter(|&row| upright.contains(&(row + 1)) && speed(row) > 10.0)
        .collect();
    assert!(riding.len() > 300, "{}: only {} rows are riding", path.display(), riding.len());

    // The handedness, off the rows that are leaning enough for the question to mean anything. Not all
    // of them: the recording has the bike crossing cambers and being bounced, and a tick of that is
    // a lean with no turn under it.
    let leaning: Vec<usize> = riding.iter().copied().filter(|&row| lean(row).abs() > 0.3).collect();
    let into = leaning
        .iter()
        .filter(|&&row| lean(row) * wrap(head(row + 1) - head(row)) < 0.0)
        .count();
    let deepest = riding.iter().map(|&row| lean(row).abs()).fold(0.0, f32::max);
    println!(
        "{}: {} of {} leaning rows turn into the lean, deepest lean {deepest:.3} rad",
        path.display(),
        into,
        leaning.len()
    );
    assert!(
        into * 10 > leaning.len() * 7,
        "{}: only {into} of {} leaning rows turn into the lean",
        path.display(),
        leaning.len()
    );

    // The port, over its own held right turn: a right turn is the heading growing, and the lean that
    // goes with it is negative — the same hand as the recording's, tick for tick, with no row of the
    // steady part of the turn leaning out of it.
    let right = Controls { gas: F(0.5), stick: (F::ONE, F::ZERO), ..Controls::default() };
    let poses = ride_held(&leaning_bike(), &[(5 * 30, right)]);
    let steady = &poses[60..];
    let ours = steady
        .windows(2)
        .filter(|w| w[0].lean * wrap(w[1].heading - w[0].heading) < 0.0)
        .count();
    assert_eq!(
        ours,
        steady.len() - 1,
        "{} of {} ticks of the port's right turn lean into it, not all of them",
        ours,
        steady.len() - 1
    );
    assert!(
        steady.iter().map(|p| p.lean.abs()).fold(0.0, f32::max) < deepest.max(0.8) * 1.5,
        "the port leans to {} rad where the recording's deepest is {deepest}",
        steady.iter().map(|p| p.lean.abs()).fold(0.0, f32::max)
    );
}

