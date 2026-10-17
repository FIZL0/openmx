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
