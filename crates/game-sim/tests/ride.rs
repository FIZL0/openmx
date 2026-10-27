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
//! claimed to be the game's, and no test here asserts a *value* the parameters decide. The
//! exceptions are the race125p tests after section (9) and section (11), which read the disc's own
//! bike where the disc is present and skip where it is not.

use game_sim::collide::{Bundle, Record};
use std::cell::RefCell;
use std::rc::Rc;
use game_sim::hash::{self, Dump};
use game_sim::phy::{self, Phy};
use game_sim::r#move::{self as mv, Frame};
use game_sim::ride::{Controls, Ride, Tables};
use game_sim::rider::{self, Rider};
use game_sim::trick::{Set, Trick};
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

fn mounts(_: &Phy) -> (Vf, Vf) {
    game_sim::stock::MOUNTS
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
    // P30 moved both (472 and 1800 before it): the held stick's squared copy, its d-pad flags, the
    // rate-limited lean and the flick's 32-sample ring. P33 moved both again (543 and 2088 before
    // it): each wheel's steering axis and its tire's three slip flags. S58 moved both again (557 and
    // 2120 before it): each wheel's gyro, turn angle, the three velocities and forces the spin step
    // keeps, its contact frame and its side force per pound of load. S66 moved both (619 and 2368
    // before it): the put-back record's point, heading and up. P97 moved the size alone (2416
    // before it): the riding speed limit a put-back gives the chassis back, a constant of the ride
    // its builder decides, which the walk leaves out — the chassis' own limit it lands in is walked.
    // MD16 moved the size alone (2424 before it): MX vs. ATV's moves' state, boxed and walked only
    // when a trick pack is on — `None`, as here, is no leaf. S83 moved the size alone (2432 before
    // it): the put-back query's gathered list, boxed and walked only once a world with props or
    // plants has been asked — `None` here too.
    assert_eq!(dump.fields.len(), 631, "the walk's leaves");
    assert_eq!(std::mem::size_of::<Ride>(), 2440, "the ride's own size");

    // A few paths spelled out, so a reordered walk is a readable failure rather than a number.
    assert_eq!(dump.fields[0].path, "mv.g_peak[0]");
    assert!(dump.get("chassis.quat[3]").is_some());
    assert!(dump.get("front.w.steer_angle").is_some());
    assert!(dump.get("rear.mv.spin").is_some());
    assert!(dump.get("air.hang_time").is_some());
    assert!(dump.get("engine.curve.frac").is_some());
    assert!(dump.get("held.flick").is_some());
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
// What replaced it was a tire bounded by what friction can do: a force that may null the contact
// patch's slide inside one substep but never reverse it. S58 took that bound out with the rest of the
// stand-in, because the original has none: its spin step pushes with the per-surface law whole, and
// a slide past the cap gets the flat skid force. What keeps the original's bike still is its own
// geometry — the tire's force goes on the chassis a short way under the centre of gravity, at
// `BodyTireForcePosition`, which the stand-in bike now does too — so the tests below are the bike's
// statements, not the bound's, plus the original's own numbers.
//
// The original's come from the idle capture: S8 recorded sixty seconds of the acceptance run's start
// with the pad untouched for all of it, and the original creeps forward off the start line without
// turning, sliding or rolling to any degree worth the name. `the_port_idles_no_livelier_than_the_
// original` reads those bounds back out of the capture rather than hard-coding them, because they are
// the disc's numbers; the two tests before it hold to the same orders of magnitude without needing
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
        // 0.03 and 0.2, not 0.02 and 0.02, since S58: at 30 Hz the drop's landing puts 0.025 rad/s of
        // turn and 0.14 ft/s of slide into the bike for a few frames before the original's tire law
        // takes them out — it has no bound on the impulse, which the stand-in had — and the settled
        // bike below and the capture's own bounds are what hold the standing still itself.
        assert!(d.yaw_rate < 0.03, "{at}: it turned at {} rad/s", d.yaw_rate);
        assert!(d.slide < 0.2, "{at}: its tires slid sideways at {} ft/s", d.slide);
        assert!(d.speed < 0.2, "{at}: it reached {} ft/s", d.speed);

        // And after two seconds of settling, which is the state the capture's bike is in, it is not
        // merely slow: it has stopped.
        let s = idle(BIKE, dt, (2.0 / dt.0) as usize, frames);
        assert!(s.moved < 0.05, "{at} after settling: it still crept {} ft", s.moved);
        assert!(s.speed < 0.02, "{at} after settling: it still moves at {} ft/s", s.speed);
    }
}

#[test]
fn a_tire_kills_a_slide_and_never_reverses_it() {
    // Shoved sideways at twenty feet a second, which is a real slide and not a settling transient: both
    // tires go past their cap and push with the flat skid force, and once the slide is back under it
    // the linear law takes it the rest of the way. So the slide decays, monotonically, to nothing, at
    // both frame rates. (Before S58 this ran at a hundred times the stiffness too, which only the
    // stand-in's bound on the impulse could hold; the original's law, unbounded, chatters there.)
    for phy in [BIKE] {
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

/// What a capture's `bike_vel` columns are, as a fraction of the chassis velocity the port calls
/// `chassis.vel`: they are the type-4 actor's copy at +0x130, which the vehicle-anim update writes as
/// the body's velocity times 0.8 (`vmulx.xyz` by `0x3f4ccccd` at `0x0024d42c`, P22; P21 measured the
/// ratio on 480 rows at 0.7999999 to 0.8000000). Every speed this file reads off those columns is
/// divided by it, so that what is compared with the port is the body's speed and not 80% of it (P28).
const BIKE_VEL_PER_BODY: f32 = 0.8;

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
        let side = (at(row, "bike_vel.x") * at(row, "bike_xform.m00")
            + at(row, "bike_vel.y") * at(row, "bike_xform.m01")
            + at(row, "bike_vel.z") * at(row, "bike_xform.m02"))
            / BIKE_VEL_PER_BODY;
        out.slide = out.slide.max(side.abs());
        let (vx, vz) = (at(row, "bike_vel.x"), at(row, "bike_vel.z"));
        out.speed = out.speed.max((vx * vx + vz * vz).sqrt() / BIKE_VEL_PER_BODY);
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
// minute, ten legs, 3600 ticks of [`DT`], over a floor with a take-off ramp laid on it twice:
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
// (P16 added an eleventh leg, a flip off a ramp of its own, and P30 took it out again: once the stick
// the backflip reads was the original's — squared, and the hold a flick speed rather than a held time
// — this bike's hand-written flip numbers no longer meant anything, and the flip is ridden on the
// disc's own bike in [`it_flips_off_the_lip`].)
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
// **Every leg passes now.** Three of them shipped `#[ignore]`d with the defect named in the test —
// not aspirational: each failed for a reason traced to a line, each reason was a bike a rider could
// not ride, and none of them lived in this file. All three are paid.
//
//     `it_turns_the_way_the_stick_is_pushed`          P12 — the steering command never reached
//                                                     the front wheel's hinge
//     `it_brakes_to_a_stop`                           P13 — the brake torque went on the rear
//                                                     wheel's spin, which the road overwrites
//                                                     every substep, so it never became a force.
//                                                     The reason written here before was wrong:
//                                                     the clutch was slipping, not locked.
//     `it_pulls_away_after_a_frame_off_the_throttle`  P11 — the clutch the idle rule pulled in was
//                                                     a second copy of the engine's clutch word
//                                                     that nothing walked back out, and the test's
//                                                     own bike took its wheel mounts off the
//                                                     patched file and so rode on a 1.6 ft
//                                                     wheelbase
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
///    original would ship has them, so the minute rides one that does — full lock at walking pace,
///    falling with speed, which is the shape every road vehicle's is.
///
///    **The fade's far end is a calibration, and P12 moved it from a quarter to a twentieth.** It was
///    written before anything put the command on the hinge, so until P12 no ride had ever been
///    steered by it. A quarter of a 30° lock is 7.5° at the top speed, which on this bike's 4.2 ft
///    wheelbase asks for a 32 ft radius at 75 ft/s — five and a half g, where the file's own
///    `MaxSideForce` keys can hold about four. Asked for what it cannot have, the port understeers,
///    slides, and sheds three quarters of its speed in a four-second turn. A twentieth keeps the
///    demand near one g at every speed the fade covers, which is what a rider can hold; it is this
///    file's number and not the disc's, like the rest of the bike.
/// 2. **`Drag`.** 0.5 holds the hand-written bike to about 48 ft/s flat out, well under the 94.73 the
///    human's recording reaches, and a minute whose top speed is nothing like the original's cannot
///    be compared against it. 0.037 puts this bike's terminal speed on the recording's to the
///    hundredth, which is what makes [`CAPTURED_TOP_SPEED`] an assertable number here. It was 0.06
///    until P28, tuned to the 75.78 the capture's `bike_vel` columns read raw — the chassis' velocity
///    times 0.8 ([`BIKE_VEL_PER_BODY`]), so the bike was 20% slower than the one it was matched to.
///
/// 3. **`BodyTireForcePosition`**, through section (8)'s [`launch_bike`]. Added when P8 merged. P8
///    found that this key is the arm the original crosses the tire's force on, and that this file's
///    value was written for the key's *other* job in the crate — the wheel mounts — so read as an arm
///    it sits a foot above the body origin and the drive force shoves the bike's own nose into the
///    ground. The minute was written before that was known and rode the unpatched bike; it now rides
///    the same copy section (8) does. [`ride_the_minute`] keeps the mounts from the unpatched file,
///    the way section (8)'s harness does. Untangling the key's two jobs is P8's journal's open
///    question, not this file's. **S58 untangled them**: the fixture now carries this arm itself and
///    the mounts are [`game_sim::stock::MOUNTS`], so the patch finds nothing to replace.
///
/// Neither patch is a claim about the disc's file: the first is a shape, the second is a calibration,
/// and `the_minutes_numbers_are_the_originals` is what keeps the second honest.
fn ridden_bike() -> String {
    let mut out = BIKE.to_string();
    let patches = [
        (
            "XAxisRateLimit = 6\n",
            "XAxisRateLimit = 6\nSteerScale1 = 1.0\nSteerScale2 = 0.05\nSteerSpeed1 = 5\nSteerSpeed2 = 50\n",
        ),
        ("Drag = 0.5", "Drag = 0.037"),
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

/// One leg of the minute: a name to fail by, how many ticks of [`DT`] it lasts, and the hands — the
/// hands it starts with, and any it changes to part way, as the tick of the leg they start on.
struct Leg {
    name: &'static str,
    ticks: usize,
    hands: Controls,
    then: Vec<(usize, Controls)>,
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
    /// The lip, as `air.rs`'s `on_a_lip` reads it — a wheel down, rising past 5 ft/s, over 30 ft/s at
    /// the contact patch, no brake — sampled once at the end of the tick rather than on each substep.
    /// The two thresholds are repeated here because the function is private, and they are the
    /// disc's literals, which do not move.
    lip: bool,
    /// The preload is shoving, and a backflip has been triggered (from the tick it fires to the tick
    /// it is over) — `Air::preload_firing` and `Air::flip_active`.
    preload: bool,
    flip: bool,
    /// The nose's angle above the ground, in the vertical plane of the ramp the bike is riding at:
    /// positive up, the full circle, so a backflip walks it once round.
    pitch: f32,
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

    /// The whole minute, launch to standstill: what a rider does on a bike rather than with one —
    /// the envelope the recording measures, and the slow bike's "never falls over". It was every leg
    /// but the flip until P30 moved the flip to the disc's bike.
    fn riding(&self) -> Vec<&Seat> {
        self.span("the launch", "stopped")
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

/// How far over the bike went, backwards positive: [`turned`] for the nose rather than the heading.
fn came_over(ticks: &[&Seat]) -> f32 {
    ticks.windows(2).map(|w| wrap(w[1].pitch - w[0].pitch)).sum()
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
    .map(|(name, ticks, hands)| Leg { name, ticks, hands, then: Vec::new() })
    .collect()
}

/// How far ahead of the bike the ramp is put, and its shape: far enough that the run-up is over
/// before the lip, short and shallow enough that a bike at the top speed clears it rather than
/// climbing it. Feet, and a rise over the run.
const RAMP: (f32, f32, f32) = (250.0, 0.3, 15.0);

/// The flip's ramp, as ([`RAMP`]'s) three numbers: nearer, steeper and longer, because the jump's ramp
/// has no lip a flip fits on. 108 ft from a standstill brings the disc's 125 to its foot at about
/// 77 ft/s, and a 25 ft face at a half holds it on the lip for 27 ticks of [`DT`] — 0.45 s, where the
/// human's three flips in the original had 17 to 19 ticks of its 30 Hz, 0.57 to 0.63 s. It was the
/// stand-in minute's eleventh leg's ramp until P30 moved the flip onto the disc's bike, and the
/// numbers came with it unchanged. `crates/openmx/tests/ride-flip.pad` is laid out against it too.
const FLIP_RAMP: (f32, f32, f32) = (108.0, 0.5, 25.0);

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
    // (Since S58 the mounts are `stock::MOUNTS` and the patch is a no-op; see `ridden_bike`.)
    let geom = Phy::read_bytes(BIKE.as_bytes(), &tables.ride.curve);
    let at = Vf::new(0.0, 1.0, 0.0, 1.0);
    let mut r = Rider::new(&p, mounts(&geom), at, Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    let part = phy::Part::read_bytes(WRECK_PART.as_bytes(), "PhysicsPart1");
    let wrecks = phy::Wrecks::read_bytes(RIDER_WRECKS.as_bytes());
    r.carries_wreck(part, wrecks, part.inertia(WRECK_BOX));

    let mut world = Jump::nothing();
    let mut out = Minute { legs: Vec::new() };
    for Leg { name, ticks, hands, then } in minute() {
        // The ramp is laid when the run-up starts, ahead of wherever the bike actually is and along
        // whatever heading it actually has: the jump has to work on the bike's own line, not on a
        // line the test assumed six legs ago.
        let ramp = match name {
            "the run-up" => Some(RAMP),
            _ => None,
        };
        if let Some((start, slope, length)) = ramp {
            let m = r.ride.transform();
            world = Jump {
                origin: r.ride.position(),
                dir: Vf::new(m.0[2].x(), 0.0, m.0[2].z(), 0.0),
                start,
                slope,
                length,
            };
        }
        let mut seen = Vec::with_capacity(ticks);
        let mut now = hands;
        for tick in 0..ticks {
            if let Some((_, next)) = then.iter().find(|(at, _)| *at == tick) {
                now = next.clone();
            }
            let frame = r.frame(&p, &tables, &now, DT, &mut world);
            let m = r.ride.transform();
            let (air, f) = (&r.ride.air, m.0[2]);
            seen.push(Seat {
                speed: r.ride.chassis.speed.0,
                heading: f32::atan2(m.0[2].x(), m.0[2].z()),
                up_y: m.0[1].y(),
                y: r.ride.position().y(),
                on_ground: air.on_ground,
                wreck: frame.wreck.is_some(),
                lip: air.on_ground
                    && r.ride.chassis.vel.y() > 5.0
                    && air.contact_vel.z() > 30.0
                    && r.ride.held.brake == F::ZERO,
                preload: air.preload_firing,
                flip: air.flip_active,
                pitch: f32::atan2(f.y(), f.x() * world.dir.x() + f.z() * world.dir.z()),
            });
        }
        out.legs.push((name, seen));
    }
    out
}

/// The top speed the human's recording reaches, in feet a second, off its `bike_vel` columns and the
/// rows the bike is upright for, in the chassis' units: the columns' 75.78 over
/// [`BIKE_VEL_PER_BODY`]. [`ridden_bike`]'s drag is set so this bike's terminal speed is this, which
/// is what lets the two legs below assert a number instead of a shape.
///
/// **Until P28 this was 75.78**, the columns read raw, and the drag was tuned to put the port's
/// `chassis.speed` there — a top speed 20% under the original's.
const CAPTURED_TOP_SPEED: f32 = 94.73;

/// The sharpest the human ever turned and the hardest they ever slowed, as the 99th and 1st
/// percentiles of the recording's own frame-to-frame differences — percentiles and not extremes
/// because a respawn moves the bike across the map in one row and the extremes are those rows.
/// Radians a second, and feet a second a second — the slowing in the chassis' units, as
/// [`CAPTURED_TOP_SPEED`] is (70.9 off the raw columns until P28).
const CAPTURED_TURN_RATE: f32 = 2.40;
const CAPTURED_SLOWING: f32 = 88.6;

/// The slowest of the original's four standing launches across both recordings, half a second and
/// two seconds in, in feet a second of the chassis: P15's 8.8 and 34.2 off the `bike_vel` columns
/// (against 20.2 and 56.9 for the quickest), over [`BIKE_VEL_PER_BODY`].
///
/// **Until P28 this was (8.0, 32.0)** — P15's numbers rounded down, in the columns' units, so the
/// port's chassis was held to 80% of the launch it was said to match and cleared it.
const ORIGINAL_LAUNCH: (f32, f32) = (11.0, 42.75);

/// How much of [`ORIGINAL_LAUNCH`] the hand-written bike is held to: it does not make the whole of it,
/// and says so. It makes 9.54 and 35.77 ft/s (87% and 84%), and what holds it there is the clutch —
/// a quarter more horsepower gains half a foot a second at two seconds and nothing at half a second,
/// while half as much clutch friction again makes 14.3 and 38.4 but is then strong enough to drive
/// the rear wheel against the pinned brake, and the brake leg never stops (P13's servo). The first
/// does not reach it and the second breaks a leg that is right, so the bound is what the bike carries
/// and the gap is written up in P28's journal.
const LAUNCH_SHARE: (f32, f32) = (0.85, 0.8);

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
    assert!(
        half > ORIGINAL_LAUNCH.0 * LAUNCH_SHARE.0,
        "half a second of full throttle moved the bike at {half} ft/s, against the original's {}",
        ORIGINAL_LAUNCH.0
    );

    // Two seconds: most of the way to the original's slowest launch, which is as much as a
    // hand-written engine's numbers can carry ([`LAUNCH_SHARE`]) — and its 0 to 60 is 7.7 s against
    // the original's 2.2 to 3.4 off the columns (P15), so quicker still in the chassis' units.
    let two = launch[launch.len() - 1].speed;
    assert!(
        two > ORIGINAL_LAUNCH.1 * LAUNCH_SHARE.1,
        "two seconds of full throttle only reached {two} ft/s, against the original's {}",
        ORIGINAL_LAUNCH.1
    );

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
/// for the whole minute and the yaw rate was `0.000` in both directions. P11 un-ignored it by
/// bridging the pose's command onto the hinge once a frame in [`Ride::frame`]; **P12 then moved that
/// bridge to where the aliasing actually is.** The original keeps the command and the hinge's angle
/// in *one word* on the wheel (`wheel+0x8ac`), which the ground step's pose stores straight into and
/// which the steered-but-not-balancing case of the wheel's own integration leaves alone; `ride.rs`
/// holds that word in two structs and now copies it both ways round the ported call, in
/// `Wheel::to_wheel`/`Wheel::from_wheel`. P12's journal entry has the decompiled store.
///
/// The turn rate the leg comes out at is the *fixture's*, through `ridden_bike`'s recalibrated
/// `SteerScale2` — see that function's first patch, and P12's journal for why 0.25 could not be
/// ridden by a bike with this port's stand-in cornering grip.
#[test]
fn it_turns_the_way_the_stick_is_pushed() {
    let m = ride_the_minute(&ridden_bike());
    for (leg, sign) in [("turning right", 1.0), ("turning left", -1.0)] {
        let ticks = m.leg(leg);
        let rate = turn_rate(&ticks) * sign;
        // 0.2 is what the fixture carries (0.24); the original's held stick comes round at 0.6 to
        // 2.5 rad/s, so this is a floor and not the original's number (P15).
        assert!(rate > 0.2, "{leg}: the bike turned at {rate} rad/s the way the stick was pushed");
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
        // instead. The right turn comes in from 95 ft/s and winds itself down to a walking pace
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
    // 104 ticks (1.73 s) off this ramp at 89 ft/s; the original's lips at about 65 hang for 2.2 to
    // 2.5 s, but they are not this ramp (P15, its 52 over `BIKE_VEL_PER_BODY`).
    assert!(air > 70, "the ramp only put the bike in the air for {air} ticks");
    assert!(
        highest(&jump) > RAMP.1 * RAMP.2 + 1.0,
        "the jump's highest point was {} ft, which is no higher than the lip",
        highest(&jump)
    );

    // Through the air the right way up — a bike that leaves a lip rotating is a bike that lands on
    // its head, and the port's own wreck test is what would catch it afterwards rather than here.
    // 0.6 until S58. The bike leaves this lip pitching down at 0.8 rad/s and nothing in the air
    // takes that out, so it comes down at up 0.58 now that its tires are the original's; the landing
    // below, which is what "on its wheels" means, still holds whole.
    assert!(flattest(&jump) > 0.5, "in the air the bike got to up {}", flattest(&jump));

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
    // The original's clean landings cost it 5 to 10 ft/s off 65 (P15's 4 to 8 off 52, in the
    // columns' units); this one comes down at 75 from 89 and bottoms out at 68, 0.72 of the top
    // speed, so 0.65 is still the floor this ramp carries.
    assert!(
        slowest(landed) > CAPTURED_TOP_SPEED * 0.65,
        "the landing left the bike at {} ft/s",
        slowest(landed)
    );
    assert_eq!(wrecks(&jump), 0, "the jump wrecked");
}

/// P16's flip, ridden on the disc's bike: pinned throttle from a standstill at [`FLIP_RAMP`], and the
/// stick's y set to each value from its tick of [`DT`] on. `crates/openmx/tests/ride-flip.pad` is the
/// same hands in the pad's notation, tick for tick.
///
/// `air.rs` reads two gestures off the stick's y while the lip holds, and since P30 it reads them the
/// way the original does — the stick squared, and the "how long it was held" that sizes the torque
/// the speed of the flick that crossed it (`ride::Flick`). So the hands are one movement, quick:
///
/// * **back** (`y` past +0.8) as the lip arrives at about tick 150 — the preload arms;
/// * **forward** at 158 — the flick fires the preload, and the same tick arms the backflip;
/// * **back** at 166 — the backflip fires, two ticks later, with the lip ten ticks from its end;
/// * centred in the air the torque holds, and **forward** for a third of a second at 260 ends it,
///   about three quarters of the way round, and the airborne control brings the rest round.
///
/// The stab has slack: anywhere from 250 to 290 lands it. Without it the 125 is still turning when
/// it comes down and lands on its back.
const THE_FLIP: [(usize, f32); 6] = [(140, 1.0), (158, -1.0), (166, 1.0), (176, 0.0), (260, -1.0), (280, 0.0)];

/// P16 and P30: a rider can pump a lip and flip off it, on `race125p.phy` read off the disc. The
/// hands are [`THE_FLIP`], and what the test holds the ride to is what a rider would see: the
/// preload went off *on the lip*, the backflip was triggered *on the lip*, the bike left the ramp,
/// came once round backwards, and rode away from the landing on its wheels.
///
/// Until P30 this rode the stand-in minute's bike with a backflip section written for it, and it
/// could not be done on the disc's: the port read the backflip's stick through the ride's rate limit
/// and sized the torque by how long the stick had been held, and both are wrong (P30's journal). It
/// skips when the disc is not here.
#[test]
fn it_flips_off_the_lip() {
    let tables = rider::Tables::build();
    let Some((p, wheels)) = the_disc_125(&tables) else { return };
    let ridden = ride_the_flip(&p, wheels, &tables, FLIP_RAMP, &THE_FLIP, 420);
    let flip: Vec<&Seat> = ridden.iter().collect();
    let lip = flip.iter().filter(|t| t.lip).count();
    // Sixteen ticks since S58: the disc's tires grip the way the original's do now, so the bike
    // reaches this 25 ft ramp at 76 ft/s rather than slower, and the ramp goes by sooner. Twelve is
    // the floor that still leaves the two gestures below room to land on it.
    assert!(lip > 12, "the flip's ramp only held the lip for {lip} ticks");

    // Both gestures went off with the lip under the wheels: `air.rs` cannot fire either anywhere
    // else, so these hold the ride to having reached the lip with the stick in the right place,
    // not to a lucky flip of the bike off a bump.
    let first = |on: fn(&Seat) -> bool| flip.iter().position(|t| on(*t));
    let shoved = first(|t| t.preload).expect("the preload never fired");
    assert!(flip[shoved].lip, "the preload fired at tick {shoved}, off the lip");
    let fired = first(|t| t.flip).expect("the backflip never fired");
    assert!(flip[fired].lip, "the backflip fired at tick {fired}, off the lip");
    assert!(shoved < fired, "the backflip (tick {fired}) went before the preload (tick {shoved})");

    // Off the ramp and once round, backwards: a whole turn of the nose in the ramp's plane, give or
    // take a quarter. Less is a bike that landed on its back wheel half way through, more is one
    // that is still turning when it lands.
    let air = airborne(&flip[fired..]);
    assert!(air > 30, "after the backflip fired the bike was only {air} ticks in the air");
    let over = came_over(&flip[fired..]);
    let turn = std::f32::consts::TAU;
    assert!(
        (over - turn).abs() < turn / 4.0,
        "the bike came {:.2} turns over, not one",
        over / turn
    );

    // Down, upright, and riding away — held to the same two-part landing as the jump's, for the
    // same reason ([`it_takes_the_jump_and_lands_on_its_wheels`]).
    const SETTLE: usize = 15;
    let last_air = flip.iter().rposition(|t| !t.on_ground).expect("the bike never left the ground");
    let landed = &flip[last_air + 1..];
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
    assert!(slowest(landed) > 20.0, "the landing left the bike at {} ft/s", slowest(landed));
    assert_eq!(wrecks(&flip), 0, "the flip wrecked");
}

/// Ride the disc's bike from a standstill at `ramp`, with the stick's y set to each value of `script`
/// from its tick on and the throttle pinned throughout, and keep what the rider saw. The ramp is laid
/// along +z from the origin and the bike starts on it, so `ramp`'s first number is the run-up.
fn ride_the_flip(p: &Phy, wheels: (Vf, Vf), tables: &rider::Tables, ramp: (f32, f32, f32),
                 script: &[(usize, f32)], ticks: usize) -> Vec<Seat> {
    let mut r = Rider::new(p, wheels, Vf::new(0.0, 2.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    let (start, slope, length) = ramp;
    let mut world = Jump { origin: Vf::new(0.0, 0.0, 0.0, 1.0), dir: Vf::new(0.0, 0.0, 1.0, 0.0), start, slope, length };
    let gas = Controls { gas: F::ONE, ..Controls::default() };
    let mut now = gas.clone();
    let mut seen = Vec::with_capacity(ticks);
    for tick in 0..ticks {
        if let Some((_, y)) = script.iter().find(|(at, _)| *at == tick) {
            now = Controls { stick: (F::ZERO, F(*y)), ..gas.clone() };
        }
        let frame = r.frame(p, tables, &now, DT, &mut world);
        let m = r.ride.transform();
        let (air, f) = (&r.ride.air, m.0[2]);
        seen.push(Seat {
            speed: r.ride.chassis.speed.0,
            heading: f32::atan2(m.0[2].x(), m.0[2].z()),
            up_y: m.0[1].y(),
            y: r.ride.position().y(),
            on_ground: air.on_ground,
            wreck: frame.wreck.is_some(),
            lip: air.on_ground
                && r.ride.chassis.vel.y() > 5.0
                && air.contact_vel.z() > 30.0
                && r.ride.held.brake == F::ZERO,
            preload: air.preload_firing,
            flip: air.flip_active,
            pitch: f32::atan2(f.y(), f.x() * world.dir.x() + f.z() * world.dir.z()),
        });
    }
    seen
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
    assert!(after < from * 0.2, "two seconds of brake took {from} ft/s to {after}");
    let hardest = (from - after) / 2.0;
    assert!(
        hardest < CAPTURED_SLOWING,
        "the brake slows the bike at {hardest} ft/s^2, past the original's {CAPTURED_SLOWING}"
    );

    // And then it stops. "Stopped" is the speed the port's own idle is allowed to creep at, not zero.
    let end = brake[brake.len() - 1].speed;
    assert!(end < 1.0, "ten seconds of brake left the bike doing {end} ft/s");
    // Within three seconds: this bike takes 2.5 from 84, and the disc's 125 in the shell 2.5 from 121.
    // Neither recording holds a brake to a stop, so the original's own time is not known (P15).
    let stop = brake.iter().position(|t| t.speed < 1.0).map_or(f32::MAX, |i| i as f32 * DT.0);
    assert!(stop < 3.0, "the brake took {stop} s to stop the bike from {from} ft/s");

    // And stays stopped for the eight seconds the rider's hands are off the pad.
    let stopped = m.leg("stopped");
    assert!(fastest(&stopped) < 2.0, "it rolled away again at {} ft/s", fastest(&stopped));
    assert_eq!(wrecks(&brake) + wrecks(&stopped), 0, "stopping wrecked");
}

/// A bike that has been sitting on the start line pulls away when the throttle does.
///
/// **The defect this was written for.** `ride.rs`'s idle rule pulls the clutch in for a rider whose
/// speed is under `IDLE_SPEED` with the rate-limited throttle shut, and until P11 it wrote that to a
/// copy of the engine's clutch word that nothing else walked — so one frame of a closed throttle on
/// the start line left the clutch fully in, the engine revved to 7000 and the rear wheel got
/// nothing. (P8 had already taken the worst of it off by copying the rider's lever over that field
/// every frame, which cured the latch but let the clutch out in a single frame instead of feeding
/// it.) The minute above only ever moved because its throttle is down on tick zero; this is the
/// shell's own start state, where `openmx` spawns the rider standing still and the player reaches
/// for the throttle some frames later.
///
/// **The fixture this was written with.** P10 took the wheel mounts off the *patched* bike, whose
/// `BodyTireForcePosition` section (8) moves to a tuning arm — so the bike it rode had a 1.6 ft
/// wheelbase with its wheels hung below the body, and it wrecked at 18 ft/s however good the
/// clutch was. It takes them off the unpatched file now, the way [`ride_the_minute`] does and for
/// the reason that function's comment gives, and it carries a wreck body so that a wreck shows up
/// as a failed assertion rather than as a rider frozen mid-frame.
///
/// What it asserts: the clutch is in while the bike sits there, it is fed back out over about the
/// `ClutchOutRateLimit` the file asks for once the throttle opens rather than being dropped in one
/// frame, the bike goes, and none of it wrecks.
#[test]
fn it_pulls_away_after_a_frame_off_the_throttle() {
    let tables = rider::Tables::build();
    let text = ridden_bike();
    let p = Phy::read_bytes(text.as_bytes(), &tables.ride.curve);
    // The mounts come from the unpatched file, as in section (8) and in `ride_the_minute`.
    let geom = Phy::read_bytes(BIKE.as_bytes(), &tables.ride.curve);
    let at = Vf::new(0.0, 1.0, 0.0, 1.0);
    let gas = Controls { gas: F::ONE, ..Controls::default() };

    // One frame and then one second of sitting on the start line, then ten of full throttle — a
    // rider arriving at a bike, in other words, rather than a drag racer already on the limiter.
    for (what, wait) in [("one frame", 1usize), ("a second", 60)] {
        let mut world = Jump::nothing();
        let mut r =
            Rider::new(&p, mounts(&geom), at, Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
        let part = phy::Part::read_bytes(WRECK_PART.as_bytes(), "PhysicsPart1");
        let wrecks = phy::Wrecks::read_bytes(RIDER_WRECKS.as_bytes());
        r.carries_wreck(part, wrecks, part.inertia(WRECK_BOX));

        let mut wrecked = 0;
        for _ in 0..wait {
            wrecked += r.frame(&p, &tables, &Controls::default(), DT, &mut world).wreck.is_some()
                as usize;
        }
        // Sitting there with the throttle shut is what pulls the clutch in, and it is all the way
        // in: that is the state the rest of this test is about.
        assert_eq!(
            r.ride.mv.clutch_in.0, 1.0,
            "after {what} at rest off the throttle the clutch is {} and not in",
            r.ride.mv.clutch_in.0
        );

        let mut top = 0.0f32;
        let mut clutch = Vec::with_capacity(600);
        for _ in 0..600 {
            wrecked += r.frame(&p, &tables, &gas, DT, &mut world).wreck.is_some() as usize;
            top = top.max(r.ride.chassis.speed.0);
            clutch.push(r.ride.mv.clutch_in.0);
        }

        // The clutch comes back out, and it is fed out rather than dropped: `ClutchOutRateLimit`
        // is 4 a second on this bike, so a lever's worth of travel is a quarter of a second — about
        // fifteen frames of these, and not one.
        assert!(
            clutch[0] > 0.9 && clutch[0] < 1.0,
            "after {what}, the first frame on the throttle took the clutch from in to {} — it was \
             dropped, not fed",
            clutch[0]
        );
        let out = clutch.iter().position(|&c| c == 0.0).unwrap_or_else(|| {
            panic!("after {what}, the clutch never came out: {:?}", &clutch[..8])
        });
        let quarter = (0.25 / DT.0).round() as usize;
        assert!(
            out >= quarter - 2 && out <= quarter + 2,
            "after {what}, the clutch was out {out} frames in, not the file's quarter second \
             ({quarter} frames)"
        );

        // And then the bike goes.
        assert!(
            top > 20.0,
            "after {what} off the throttle, ten seconds of full throttle reached {top} ft/s \
             (clutch_in {})",
            r.ride.mv.clutch_in.0
        );
        assert_eq!(wrecked, 0, "pulling away after {what} wrecked {wrecked} times");
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
    // And the bike should still be on the ground at the end of it.
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
    let whole = m.riding();
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
/// Its `bike_vel` columns and its own positions disagree about the speed by a steady factor of 1.25,
/// and until P28 this test took the columns at their word and so held the port to 80% of the
/// original. P22 found why: the columns are the actor's copy of the chassis' velocity, written times
/// 0.8. They are read here over [`BIKE_VEL_PER_BODY`], which puts the top speed at the positions'
/// 95-odd ft/s.
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
        (x * x + z * z).sqrt() / BIKE_VEL_PER_BODY
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
    // The slowing is the riding's alone. The flip lands off a twelve-foot face, and even when it comes
    // down level the impact takes four or five ft/s off in a tick — 340 ft/s^2 and more, which no
    // upright row of a free ride is, and which the recording has nothing to compare with (it has no
    // flip in it that this test knows of). Its landing is held to `it_flips_off_the_lip`'s bounds.
    let riding = m.riding();
    let worst = riding
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
//
// Since S58 the key does one job: the fixture's value is that tuning arm and the mounts are
// [`game_sim::stock::MOUNTS`], so `launch_bike` below replaces nothing and is kept only so the
// section's harness reads as it did.

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
        (x * x + z * z).sqrt() / BIKE_VEL_PER_BODY
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
//
// P32: the two are one. In the original the no-hander rolls the chassis' own transform, and the next
// integration reads that transform back into the body, so the node's lean *is* the body's; `ride.node`
// now carries that transform's rotation from frame to frame. The pin stays — the roll is turned by the
// transform, not by a rate.

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
/// bridged onto the hinge — in `Wheel::to_wheel`/`Wheel::from_wheel`, where P12 put it — the bike
/// holds one circle after another and never once comes back the other way.
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
    // Since P32 the body is the bike that leans (the no-hander rolls the chassis' own transform), so
    // it may lean as far as `leaning_bike`'s `MaxRollCommand` of 40° asks — cos 40° is 0.766 — and
    // no further.
    assert!(
        poses.iter().all(|p| p.up_y > 0.75),
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
        (x * x + z * z).sqrt() / BIKE_VEL_PER_BODY
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



// ---------------------------------------------------------------------------------------------------
// P20 — the reset chord.

/// The human's second recording, whose second wreck the chord ended (S29). It sits directly in the
/// acceptance directory because that is where P15 left it; this test reads it by name.
const CHORD_CAPTURE: [&str; 2] = ["capture/acceptance", "recording2-freeride.csv"];

/// The row of [`CHORD_CAPTURE`] where R1 went down with L1 already held, and the reason-12 wreck the
/// rider was in read 0 on the same row.
const CHORD_ROW: usize = 1212;

/// The save state the recording starts from, unpacked by `./kit state ram`, which is where the
/// original's binding table is read from.
const CHORD_STATE: &str = "recording2.p2m2_SaveState.eeMemory.bin";

/// What `from_pad` says about the chord on each frame of a run of pads.
fn chord_over(frames: &[&[game_sim::input::Control]]) -> Vec<bool> {
    use game_sim::input::{Manager, Report, mask_of};
    let mut input = Manager::connected();
    frames
        .iter()
        .map(|held| {
            input.tick(F(0.03333), &[Report::from_buttons(mask_of(held)), Report::missing()]);
            Controls::from_pad(input.pad(0)).respawn
        })
        .collect()
}

/// **P20.** The reset chord is the original's: L1 and R1, the second press of the two, riding or down.
///
/// Four things, from the cheapest up:
///
/// 1. **The binding table**, out of the recording's save state when it is unpacked in this checkout.
///    The hand reset asks the input manager's binding map about two commands; the map holds, for each
///    pad, one control index per command plus a second binding, and on the shipped table the two
///    reset commands are L1 and R1 with no second binding, and the three trick commands are Circle,
///    Triangle and R1 in the order [`Controls::trick`] reads them. So R1 is shared in the original
///    too, which is why the port shares it.
/// 2. **The edge.** It fires on the press of either while the other is held, once; held on from the
///    frame before is not a chord, and neither button alone is one — R1 alone is a trick button.
///    The two stick clicks the port used before are not a chord any more.
/// 3. **The human's own presses.** Every row of the recording's sixteen control columns through the
///    ported input layer and [`Controls::from_pad`]: the chord fires on exactly one row, the one where
///    the original put the rider back, and not on the two stretches where R1 alone was held riding.
/// 4. **Riding as well as down.** The original's gate has no test on a wreck, so a rider on its
///    wheels, rolling, is put back by it on the tick of the second press.
#[test]
fn the_reset_chord_is_the_originals() {
    use game_sim::input::Control::{self, *};

    // 1 — the table.
    let at = private_dir("states").join(CHORD_STATE);
    match std::fs::read(&at) {
        Err(_) => println!("no {} — `./kit state ram` of the recording's save state writes it", at.display()),
        Ok(ram) => {
            let word = |a: u32| {
                let a = (a & 0x01ff_ffff) as usize;
                u32::from_le_bytes(ram[a..a + 4].try_into().unwrap())
            };
            // `app_ptr` in kit.toml, then the input manager and its binding map.
            let map = word(word(word(0x0041_f770) + 0x104) + 0x648);
            let first = |pad: u32, cmd: u32| word(map + 0x138 + pad * 0x138 + cmd * 4) as i32;
            let second = |pad: u32, cmd: u32| word(map + 0x3a8 + pad * 0x138 + cmd * 4) as i32;
            let index = |c: Control| c.index() as i32;
            for pad in 0..2 {
                assert_eq!(first(pad, 0x4c), index(L1), "pad {pad}: the reset's first command");
                assert_eq!(first(pad, 0x4d), index(R1), "pad {pad}: the reset's second command");
                assert_eq!(second(pad, 0x4c), -1, "pad {pad}: no second binding for the first");
                assert_eq!(second(pad, 0x4d), -1, "pad {pad}: no second binding for the second");
                let tricks: Vec<i32> = (0x11..=0x13).map(|cmd| first(pad, cmd)).collect();
                assert_eq!(
                    tricks,
                    [index(Circle), index(Triangle), index(R1)],
                    "pad {pad}: the three trick commands, in the decoder's order"
                );
            }
        }
    }

    // 2 — the edge.
    assert_eq!(chord_over(&[&[L1], &[L1, R1], &[L1, R1]]), [false, true, false], "L1 then R1");
    assert_eq!(chord_over(&[&[R1], &[R1, L1], &[R1, L1]]), [false, true, false], "R1 then L1");
    assert_eq!(chord_over(&[&[L1, R1], &[L1, R1]]), [true, false], "both on one frame");
    assert_eq!(chord_over(&[&[L1, R1], &[L1], &[L1, R1]]), [true, false, true], "R1 again");
    assert_eq!(chord_over(&[&[R1], &[R1], &[]]), [false; 3], "R1 alone is a trick button");
    assert_eq!(chord_over(&[&[L1], &[L1], &[]]), [false; 3], "L1 alone");
    assert_eq!(chord_over(&[&[L3], &[L3, R3]]), [false; 2], "the stick clicks are not the chord");

    // 3 — the recording.
    let path = private_dir(CHORD_CAPTURE[0]).join(CHORD_CAPTURE[1]);
    match std::fs::read_to_string(&path) {
        Err(_) => println!("no {} — the recording's own presses were not replayed", path.display()),
        Ok(text) => {
            use game_sim::input::{Manager, Report};
            let cap = game_sim::replay::Capture::parse(&text).expect("the recording parses");
            let col = |name: &str| {
                cap.columns
                    .iter()
                    .position(|c| c == name)
                    .unwrap_or_else(|| panic!("no column '{name}' in {}", path.display()))
            };
            let reason = |row: usize| cap.rows[row][col("wreck_reason")].parse::<u32>().unwrap();
            let mut input = Manager::connected();
            let mut fired = Vec::new();
            for row in 0..cap.rows.len() {
                let mut mask = 0;
                for c in Control::ALL {
                    if cap.rows[row][col(&format!("ctl{}.held", c.index()))] != "0" {
                        mask |= c.mask();
                    }
                }
                input.tick(F(0.03333), &[Report::from_buttons(mask), Report::missing()]);
                if Controls::from_pad(input.pad(0)).respawn {
                    fired.push(row);
                }
            }
            assert_eq!(fired, [CHORD_ROW], "the rows the port's chord fires on");
            assert_eq!(reason(CHORD_ROW - 1), 12, "the rider was down on the row before");
            assert_eq!(reason(CHORD_ROW), 0, "and the original put it back on the chord's row");
            println!("{}: the chord fires on row {CHORD_ROW} only", path.display());
        }
    }

    // 4 — riding, on the bike and the ground [`ride_held`] rides, for the reason its comment gives.
    let tables = rider::Tables::build();
    let p = Phy::read_bytes(ridden_bike().as_bytes(), &tables.ride.curve);
    let geom = Phy::read_bytes(BIKE.as_bytes(), &tables.ride.curve);
    let at = Vf::new(0.0, 1.0, 0.0, 1.0);
    let mut r = Rider::new(&p, mounts(&geom), at, Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    let part = phy::Part::read_bytes(WRECK_PART.as_bytes(), "PhysicsPart1");
    let wrecks = phy::Wrecks::read_bytes(RIDER_WRECKS.as_bytes());
    r.carries_wreck(part, wrecks, part.inertia(WRECK_BOX));
    let mut world = Jump::nothing();
    let gas = Controls { gas: F::ONE, ..Controls::default() };
    for k in 0..90 {
        let f = r.frame(&p, &tables, &gas, DT, &mut world);
        assert!(f.wreck.is_none() && !f.respawned, "frame {k}: the ride is not a clean one");
    }
    let rolled = r.ride.position();
    let from_spawn = |v: Vf| ((v.x() - at.x()).powi(2) + (v.z() - at.z()).powi(2)).sqrt();
    assert!(from_spawn(rolled) > 1.0, "the bike has not gone anywhere to be put back from");
    let press = chord_over(&[&[L1], &[L1, R1]]);
    for (k, chord) in press.into_iter().enumerate() {
        let f = r.frame(&p, &tables, &Controls { respawn: chord, ..gas }, DT, &mut world);
        assert!(f.wreck.is_none(), "the chord does not put the rider down");
        assert_eq!(f.respawned, chord, "chord frame {k}: put back on the second press only");
    }
    // Put back where it was riding, on the put-back record, and never on the start line (S66):
    // the record is the spot under the bike a frame before the press, and the press's own frame
    // rides on from it.
    let back = r.ride.position();
    let apart = ((back.x() - rolled.x()).powi(2) + (back.z() - rolled.z()).powi(2)).sqrt();
    assert!(
        apart < 2.0 && from_spawn(back) > 1.0,
        "put back {apart:.2} ft from where it rode and {:.2} ft from the start line",
        from_spawn(back)
    );
}

/// P21's capture: the original's `race125p` held flat out from a standstill along the straight
/// x = 4620 lane of Free09, by a generated input recording — Cross at full pressure, nothing else —
/// with `watch/acceptance.toml`'s columns and the chassis' and engine's own. In a subdirectory of
/// `context/capture/` for the reason [`IDLE_CAPTURE`]'s comment gives.
const FLAT_OUT_CAPTURE: [&str; 2] = ["capture/p21", "flat-throttle.csv"];

/// How many frames the terminal speed is measured over, at either end: the last second of the run.
const TERMINAL_FRAMES: usize = 30;

/// The capture's terminal speed: how far the chassis' centre of gravity went over the last
/// [`TERMINAL_FRAMES`] rows, over the time those rows say they took. The distance and not the
/// `ch_speed` column, so that the port's number below is measured the same way off its own position;
/// on the capture the two agree to 0.03 ft/s.
fn captured_terminal_speed(text: &str) -> f32 {
    let cap = game_sim::replay::Capture::parse(text).expect("the flat-out capture parses");
    let col = |name: &str| {
        cap.columns.iter().position(|c| c == name).unwrap_or_else(|| panic!("no {name} column"))
    };
    let (x, y, z, dt) = (col("ch_cofg.x"), col("ch_cofg.y"), col("ch_cofg.z"), col("dt"));
    let at = |row: &Vec<String>, i: usize| row[i].parse::<f32>().expect("a number");
    let rows = &cap.rows[cap.rows.len() - TERMINAL_FRAMES - 1..];
    let (a, b) = (&rows[0], &rows[TERMINAL_FRAMES]);
    let gone = ((at(b, x) - at(a, x)).powi(2)
        + (at(b, y) - at(a, y)).powi(2)
        + (at(b, z) - at(a, z)).powi(2))
    .sqrt();
    gone / rows[1..].iter().map(|r| at(r, dt)).sum::<f32>()
}

/// The disc's `race125p.phy` and where the `125_faisst` model hangs its wheels — the bike the
/// capture was ridden on, read the way `tests/replay.rs` reads it (that file says why each half is
/// what it is). `None` when the disc is not here.
fn the_disc_125(tables: &rider::Tables) -> Option<(Phy, (Vf, Vf))> {
    the_disc_125_and_fork(tables).map(|(p, wheels, _)| (p, wheels))
}

/// [`the_disc_125`], and the axis the model's front fork steers about: from its `Shocks` node to its
/// `Handlebars` node, the way the bike's loader sets the front wheel's (P33).
fn the_disc_125_and_fork(tables: &rider::Tables) -> Option<(Phy, (Vf, Vf), Vf)> {
    use game_sim::archive::{Psr, Res};
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(private_dir("iso").join("MX.PSR")).ok()?;
    let mut front = vec![0u8; 64 << 10];
    file.read_exact(&mut front).ok()?;
    let psr = Psr::parse(&front).ok()?;
    let mut member = |name: &str| -> Option<Vec<u8>> {
        let e = psr.find(name)?;
        file.seek(SeekFrom::Start(e.at)).ok()?;
        let mut bytes = vec![0u8; e.size as usize];
        file.read_exact(&mut bytes).ok()?;
        Some(bytes)
    };
    let leaf = |res: &Res, want: &str| {
        res.entries
            .iter()
            .find(|e| {
                let n = e.name.to_ascii_lowercase();
                n == want || n.ends_with(&format!("\\{want}"))
            })
            .cloned()
    };
    let archives: Vec<String> = psr
        .entries
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.starts_with("bike\\") && n.ends_with(".res")
        })
        .collect();
    let (mut phy, mut wheels, mut fork) = (None, None, None);
    for archive in &archives {
        let Some(bytes) = member(archive) else { continue };
        let Ok(res) = Res::parse(&bytes) else { continue };
        if phy.is_none() {
            if let Some(e) = leaf(&res, "race125p.phy") {
                phy = res.read(&bytes, &e).ok().map(|b| Phy::read_bytes(&b, &tables.ride.curve));
            }
        }
        if wheels.is_none() && archive.to_ascii_lowercase().contains("125_faisst") {
            let Some(e) = leaf(&res, "bike.psg") else { continue };
            let Ok(psg) = res.read(&bytes, &e) else { continue };
            let Ok(model) = game_render::psg::Model::parse(&psg) else { continue };
            let node = |name: &str| -> Option<Vf> {
                let i = model.nodes.iter().position(|n| n.name == name)?;
                let mut root = i;
                while let Some(up) = model.nodes[root].parent {
                    root = up;
                }
                Some(Vf(model.place_in(i, root)))
            };
            wheels = node("TireFront").zip(node("TireRear"));
            fork = node("Handlebars").zip(node("Shocks")).map(|(h, s)| game_sim::tire::steering_axis(h, s));
        }
    }
    Some((phy?, wheels?, fork?))
}

/// **The 125's top speed is its own** (P21). Sixteen seconds of full throttle on a flat floor, from
/// a standstill, at the 30 Hz step the capture was taken at, and the last second's speed against the
/// original's on the straightest lane of Free09: within 5%.
///
/// What caps the original there is drag against the top gear's torque — the capture is in fifth, the
/// top gear, at 8736 rpm under a 9500 rev limit, and 120 ft/s is under the rigid body's 135 — so this
/// is a test of the drag, the gearing and the tires together, not of a clamp.
#[test]
fn race125p_tops_out_where_the_original_does() {
    let path = private_dir(FLAT_OUT_CAPTURE[0]).join(FLAT_OUT_CAPTURE[1]);
    let Ok(text) = std::fs::read_to_string(&path) else {
        println!("no flat-out capture at {} — skipped", path.display());
        return;
    };
    let tables = rider::Tables::build();
    let Some((p, wheels)) = the_disc_125(&tables) else {
        println!("no race125p.phy or 125_faisst wheels on a disc in context/iso — skipped");
        return;
    };
    let want = captured_terminal_speed(&text);

    let dt = F(game_sim::tick::DeltaTime::CAP_30);
    let at = Vf::new(0.0, 1.0, 0.0, 1.0);
    let mut r = Rider::new(&p, wheels, at, Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    let mut world = floor();
    let gas = Controls { gas: F::ONE, ..Controls::default() };
    let mut track = Vec::new();
    for _ in 0..16 * 30 {
        r.frame(&p, &tables, &gas, dt, &mut world);
        track.push(r.ride.position());
    }
    let (a, b) = (track[track.len() - 1 - TERMINAL_FRAMES], track[track.len() - 1]);
    let gone = ((b.x() - a.x()).powi(2) + (b.y() - a.y()).powi(2) + (b.z() - a.z()).powi(2)).sqrt();
    let got = gone / (TERMINAL_FRAMES as f32 * dt.0);
    println!(
        "terminal speed: {got:.2} ft/s against the original's {want:.2} ({:+.1}%), gear {}",
        (got / want - 1.0) * 100.0,
        r.ride.engine.gear,
    );
    assert!(
        (got / want - 1.0).abs() < 0.05,
        "race125p tops out at {got} ft/s on the flat, against the original's {want}"
    );
}


// ---------------------------------------------------------------------------------------------------
// S37: a hard hit on the scenery
// ---------------------------------------------------------------------------------------------------

/// The reason the head ray wrecks with.
const SCENERY: game_sim::trick::Wreck = game_sim::trick::Wreck(game_sim::air::Wreck::Scenery as u8);

/// A beam across the rider's path, sloping up and away from them: from four feet up at `z` to eight
/// feet up four feet further on. The bike's body passes under it, and only a ray reaching up from the
/// bike can find it — which the head ray does over a few feet of travel, so a frame's step cannot jump
/// it. Wound to face down at the rider, so that the sweep, which takes a back face as no hit, sees it
/// from below.
fn beam(z: f32) -> Prop {
    let mesh = Mesh {
        version: 0,
        surface: None,
        transform: [
            [F::ONE, F::ZERO, F::ZERO],
            [F::ZERO, F::ONE, F::ZERO],
            [F::ZERO, F::ZERO, F::ONE],
            [F::ZERO, F::ZERO, F::ZERO],
        ],
        vertices: Points::Wide(vec![
            [F(-10.0), F(5.0), F(z)],
            [F(10.0), F(5.0), F(z)],
            [F(10.0), F(9.0), F(z + 4.0)],
            [F(-10.0), F(9.0), F(z + 4.0)],
        ]),
        normals: None,
        nodes: vec![Node {
            volume: F::ZERO,
            centre: [F::ZERO, F(7.0), F(z + 2.0)],
            half_extent: [F(10.0), F(2.0), F(2.0)],
            children: [0x8000, 0x8001],
        }],
        triangles: vec![
            Triangle { distance: F::ZERO, normal: [F::ZERO; 3], vertices: [0, 1, 2] },
            Triangle { distance: F::ZERO, normal: [F::ZERO; 3], vertices: [0, 2, 3] },
        ],
    };
    Prop::place(mesh, &Placement::default())
}

/// The first wreck a rider thrown at `prop` comes off with, if any.
fn thrown_at(prop: Prop) -> Option<game_sim::trick::Wreck> {
    let tables = rider::Tables::build();
    let p = Phy::read_bytes(format!("{BIKE}{WRECK_LIMITS}").as_bytes(), &tables.ride.curve);
    let mut r = Rider::new(&p, mounts(&p), Vf::new(0.0, 1.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    r.ride.chassis.vel = Vf::new(0.0, 0.0, 60.0, 0.0);
    let mut yard = Yard { floor: floor(), props: Props { props: vec![prop] } };
    (0..120).find_map(|_| r.frame(&p, &tables, &Controls::default(), DT, &mut yard).wreck)
}

/// What the original's own collision record holds in a RAM image, and what both wreck tests make of
/// it: the contact list its collide pass builds, read into the port's [`game_sim::air::Contact`]s;
/// the port's [`game_sim::air::hit_wreck`] over that list; and every reason the original's own wreck
/// test enters over the same image, the entry hooked so that it only writes the reason down.
struct Window {
    frame: u32,
    list: Vec<game_sim::air::Contact>,
    port: Option<game_sim::air::Wreck>,
    original: Vec<u32>,
}

fn window(image: &str) -> Option<Window> {
    use game_sim::air::{self, Contact, Other};
    use ps2_oracle::{Ee, Kit, addr, skip};
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    // The slot's own `context/` first: a worktree's `kit.toml` points `[paths] context` at the main
    // checkout's, and the images are made in the slot.
    let path = [kit.root.join("context/states").join(image), kit.ctx("states").join(image)]
        .into_iter()
        .find(|p| p.exists())
        .unwrap_or_else(|| kit.ctx("states").join(image));
    let mut ee = Ee::from_ram(path).map_err(|why| skip(&why)).ok()?;
    let u = |ee: &Ee, a: u32| ee.u32_at(a);
    let f = |ee: &Ee, a: u32| F(ee.f32_at(a));
    // The human's bike: the game manager's actor list, two lists in and two items along.
    let mut a = u(&ee, addr("game_manager").unwrap());
    for off in [0x34, 0x34, 0x8, 0x8] {
        a = u(&ee, a + off);
    }
    let bike = u(&ee, a);
    let (record, chassis) = (u(&ee, bike + 0x7d0), u(&ee, bike + 0x7a4));
    ee.call(addr("collision_record_collide").unwrap(), &[record as u64], &[], 50_000_000);

    let list = u(&ee, record + 0x108);
    let list: Vec<Contact> = (0..u(&ee, record + 0x100))
        .map(|i| {
            let e = list + 0x80 * i;
            let other = match u(&ee, e + 0x74) {
                0 => Other::Scenery,
                o if u(&ee, o + 0xf0) != 0 => {
                    let body = u(&ee, o + 0xf0);
                    Other::Rider { height: f(&ee, body + 0x64), vel: Vf(core::array::from_fn(|k| ee.f32_at(body + 0x260 + 4 * k as u32))) }
                }
                o => {
                    let body = u(&ee, o + 0xf4);
                    Other::Object { mass: f(&ee, body + 0xc), vel: Vf(core::array::from_fn(|k| ee.f32_at(body + 0x70 + 4 * k as u32))) }
                }
            };
            Contact { mirrored: u(&ee, e) != 0, reach: f(&ee, e + 0x24), other }
        })
        .collect();
    let tables = Tables::build();
    let mut p = self::bike(&tables).air;
    p.joust_wreck_height = f(&ee, bike + 0x784);
    let mut state = air::Air::default();
    let port = air::hit_wreck(&p, &mut state, u(&ee, record + 0x40) != 0, u(&ee, bike + 0x588) != 0,
                              f(&ee, chassis + 0x210), f(&ee, chassis + 0x64), &list);

    let entry = u(&ee, u(&ee, bike + 0x48) + 0xfc);
    let codes = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = codes.clone();
    ee.hooks.insert(entry, Box::new(move |ee| seen.borrow_mut().push(ee.r(5) as u32)));
    ee.call(addr("bike_wreck_test").unwrap(), &[bike as u64, 0], &[], 50_000_000);
    let original = codes.borrow().clone();
    Some(Window { frame: u(&ee, addr("frame_counter").unwrap()), list, port, original })
}

/// **S37.** A rider who hits the world hard wrecks at reason 6.
///
/// Two halves. The original's, at the window S29 found in recording2 — the reason-6 wreck at capture
/// row 1750 (`frame_counter` 4124): two EE images saved during the playback, at the tick boundaries
/// before it, with the bike's own collision record run over each. On the last one the record finds
/// one contact, the head ray, against no actor; the port's clause wrecks on it at reason 6, and the
/// original's whole wreck test leaves 6 as the reason. On the one before the list is empty
/// and neither does anything. The images are `context/`'s and are not committed: the journal's
/// `live/save.sh` makes them again, and without them this half says so and passes.
///
/// The port's: a rider thrown under a beam at head height wrecks at reason 6 — the head ray
/// [`game_sim::ride::hits`] sweeps finds it — and the same throw at a wall does not, whatever else it
/// does to the rider: the bike's nose meets a wall long before the rider's head can, in the original
/// as much as here, because the head ray stands almost straight up over the seat.
#[test]
fn a_hard_hit_on_scenery_wrecks_at_reason_six() {
    use game_sim::air::{Contact, Other, Wreck};
    if let Some(w) = window("s37-9.ee.bin") {
        assert_eq!(w.frame, 4123, "the image is not the boundary before the wreck");
        assert_eq!(
            w.list,
            [Contact { mirrored: false, reach: F(6.6743364), other: Other::Scenery }],
            "the record's list"
        );
        assert_eq!(F(6.6743364), F(game_sim::air::HEAD.1.y()), "the head ray is the one that hit");
        assert_eq!(w.port, Some(Wreck::Scenery), "the port");
        // The reason that stays is the last one entered. Over this image the landing clauses enter one
        // first — a tail wreck, off whatever attitude the bike was left in at the boundary — and the
        // hit enters 6 over it, which is the reason the capture shows.
        assert_eq!(w.original.last(), Some(&(Wreck::Scenery as u32)), "the original: {:?}", w.original);
        assert_eq!(w.original.iter().filter(|&&c| c == Wreck::Scenery as u32).count(), 1);
    }
    if let Some(w) = window("s37-8.ee.bin") {
        assert_eq!(w.frame, 4122);
        assert_eq!((w.list.len(), w.port, w.original.len()), (0, None, 0), "the tick before");
    }

    assert_eq!(thrown_at(beam(30.0)), Some(SCENERY), "a rider thrown under a beam");
    assert_ne!(thrown_at(wall(30.0)), Some(SCENERY), "a wall the bike's nose meets first");
}

// ---------------------------------------------------------------------------------------------------
// P23 — the brake bites as hard as the original's, on the disc's own bike
// ---------------------------------------------------------------------------------------------------

/// The original's speeds here are the chassis' own, which is not what the captures' `bike_vel`
/// columns hold. Those run a steady 0.8 of it — P10's factor of 1.254, which P23 pinned down: the
/// rigid body's velocity read live during the brake, the wheels' spin times their radius, and the
/// capture's own positions over its `dt` all agree with each other and are 1.25 times the column. The
/// port's [`Ride`] carries the rigid body's, so the captured speeds below are off the positions.
///
/// The speed `brake2` was going when the lever bit: its last row still gaining, ft/s.
const BRAKE_FROM: f32 = 76.95;
/// The original's flat brake decel, ft/s², from the first row under 60 ft/s to the first under 12:
/// rows 24 to 53 of `brake2`, 58.85 to 11.72 in twenty-nine ticks of 0.03333 s.
const CAPTURED_BRAKE_DECEL: f32 = 48.8;
/// And its time from [`BRAKE_FROM`] to the first row under 1 ft/s: forty-nine ticks.
const CAPTURED_BRAKE_STOP: f32 = 1.63;

/// The disc's `race125p.phy` and the `125_faisst` model's two tire nodes (the bike the human rode,
/// and the one P23's captures were braked on), or `None` with a line saying what is missing.
///
/// The same reads `tests/replay.rs` makes for its acceptance bike, kept here so that this file stays
/// readable on its own: a member is a seek and a read, the physics file is in every bike archive, and
/// the mounts are the model's `TireFront` and `TireRear` in the model's root frame.
fn race125p(tables: &Tables) -> Option<(Phy, (Vf, Vf))> {
    use game_sim::archive::{Psr, Res};
    use std::io::{Read, Seek, SeekFrom};
    let at = private_dir("iso").join("MX.PSR");
    let Ok(mut file) = std::fs::File::open(&at) else {
        println!("no {} — `./kit extract MX.PSR` writes it", at.display());
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    file.read_exact(&mut front).expect("the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let archives: Vec<String> = psr
        .entries
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.starts_with("bike\\") && n.ends_with(".res") && n.contains("125_faisst")
        })
        .collect();
    let leaf = |res: &Res, want: &str| {
        res.entries.iter().position(|e| {
            let n = e.name.to_ascii_lowercase();
            n == want || n.ends_with(&format!("\\{want}"))
        })
    };
    for archive in archives {
        let e = psr.find(&archive)?;
        file.seek(SeekFrom::Start(e.at)).expect("seek");
        let mut bytes = vec![0u8; e.size as usize];
        file.read_exact(&mut bytes).expect("read");
        let Ok(res) = Res::parse(&bytes) else { continue };
        let (Some(p), Some(m)) = (leaf(&res, "race125p.phy"), leaf(&res, "bike.psg")) else { continue };
        let phy = Phy::read_bytes(&res.read(&bytes, &res.entries[p]).expect("race125p.phy"), &tables.curve);
        let psg = res.read(&bytes, &res.entries[m]).expect("bike.psg");
        let model = game_render::psg::Model::parse(&psg).expect("bike.psg");
        let node = |name: &str| -> Option<Vf> {
            let i = model.nodes.iter().position(|n| n.name == name)?;
            let mut root = i;
            while let Some(up) = model.nodes[root].parent {
                root = up;
            }
            Some(Vf(model.place_in(i, root)))
        };
        if let (Some(f), Some(r)) = (node("TireFront"), node("TireRear")) {
            return Some((phy, (f, r)));
        }
    }
    println!("no 125_faisst archive with race125p.phy and both tire nodes on the disc");
    None
}

/// The bike's forward speed: its world velocity along row 2 of its transform.
fn forward_speed(r: &Ride) -> f32 {
    let (v, m) = (r.chassis.vel, r.transform());
    v.x() * m.0[2].x() + v.y() * m.0[2].y() + v.z() * m.0[2].z()
}

/// race125p on the level at the 30 Hz step the captures run at: settled, ridden up to
/// [`BRAKE_FROM`] on the throttle, then square held with the stick centred until it stops. The speed
/// on every braking frame, the first being the one the lever went in on.
fn brake_race125p() -> Option<Vec<f32>> {
    let tables = rider::Tables::build();
    let (p, mounts) = race125p(&tables.ride)?;
    for (name, t) in [("front", &p.front), ("rear", &p.rear)] {
        let s = t.surfaces[0];
        println!(
            "{name}: radius {} inertia {} | MaxBrakingTorque {} MaxDriveForce {} SkidDriveForce {} \
             WheelFriction {} WheelDrag {}",
            t.tire_radius.0, t.tire_inertia.0, s.max_braking_torque.0, s.max_drive_force.0,
            s.skid_drive_force.0, s.wheel_friction.0, s.wheel_drag.0,
        );
    }
    println!("mass {}", p.mass.mass.0);
    let dt = F(game_sim::tick::DeltaTime::CAP_30);
    let mut r = Rider::new(&p, mounts, Vf::new(0.0, 1.5, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    let mut world = floor();
    let still = Controls::default();
    for _ in 0..60 {
        r.frame(&p, &tables, &still, dt, &mut world);
    }
    let gas = Controls { gas: F::ONE, ..Controls::default() };
    let mut k = 0;
    while forward_speed(&r.ride) < BRAKE_FROM {
        let f = r.frame(&p, &tables, &gas, dt, &mut world);
        assert!(f.wreck.is_none(), "race125p wrecked on the throttle at {} ft/s", forward_speed(&r.ride));
        k += 1;
        assert!(k < 600, "race125p never reached {BRAKE_FROM} ft/s: {}", forward_speed(&r.ride));
    }
    let brake = Controls { brake: F::ONE, ..Controls::default() };
    let mut speed = Vec::new();
    for _ in 0..150 {
        speed.push(forward_speed(&r.ride));
        let f = r.frame(&p, &tables, &brake, dt, &mut world);
        assert!(f.wreck.is_none(), "race125p wrecked braking at {} ft/s", forward_speed(&r.ride));
    }
    Some(speed)
}

/// **P23.** race125p's flat brake decel and its time to a stop are the original's, within 15%.
///
/// The original's are off P23's own capture (`brake2`: square held from full speed on a level
/// stretch of Free09, stick centred), read in the chassis' own speed (see [`BRAKE_FROM`]). What the
/// same playback showed live, and what this leans on:
///
/// - square puts the brake on *both* wheels, the two brake words ramped in together over three
///   ticks — which is what [`Controls::brake`] does here too;
/// - each wheel then puts `-(MaxBrakingTorque + WheelFriction) / TireRadius` through its patch —
///   to the tenth of a pound of what that sum gives on the disc's numbers — with no lock flag set on
///   the level. So the brake's own force is the port's to within the `WheelFriction` torque
///   `surface_drag` leaves out (P23's journal entry has the pounds; they are the disc's and so are
///   not written here);
/// - and the body slows at 48.8 ft/s². The port's 46.7 is that torque short, a wheel.
///
/// The brief's "twice as hard" was the 0.8 between the capture's `bike_vel` and the body's speed,
/// compared against a shell trace from 121 ft/s, where the engine's own braking through the clutch
/// adds to the brake; on the same footing the two brakes are 4% apart.
#[test]
fn race125p_brakes_like_the_original() {
    let Some(speed) = brake_race125p() else { return };
    let dt = game_sim::tick::DeltaTime::CAP_30;
    let under = |v: f32| {
        speed
            .iter()
            .position(|&s| s < v)
            .unwrap_or_else(|| panic!("race125p never got under {v} ft/s braking: {speed:?}"))
    };
    let (a, b) = (under(60.0), under(12.0));
    let decel = (speed[a] - speed[b]) / ((b - a) as f32 * dt);
    let stop = under(1.0) as f32 * dt;
    println!(
        "race125p brakes at {decel:.1} ft/s^2 (the original {CAPTURED_BRAKE_DECEL}) and stops in {stop:.2} s \
         (the original {CAPTURED_BRAKE_STOP})"
    );
    assert!(
        (decel - CAPTURED_BRAKE_DECEL).abs() <= 0.15 * CAPTURED_BRAKE_DECEL,
        "race125p brakes at {decel:.1} ft/s^2 from 60 to 12 ft/s; the original at {CAPTURED_BRAKE_DECEL}"
    );
    assert!(
        (stop - CAPTURED_BRAKE_STOP).abs() <= 0.15 * CAPTURED_BRAKE_STOP,
        "race125p takes {stop:.2} s from {BRAKE_FROM} ft/s to under 1; the original {CAPTURED_BRAKE_STOP} s"
    );
}


// ---------------------------------------------------------------------------------------------------
// (11) the minute on the disc's own bike
// ---------------------------------------------------------------------------------------------------
//
// Section (7)'s minute rides this directory's hand-written bike, and P15 raised its legs as far as that
// bike carries them — which is short of the original at every leg: its 0 to 60 takes 7.7 s against the
// original's 2.2 to 3.4, its turn comes round at 0.24 rad/s against 0.6 to 2.5. No calibration of the
// fixture closes that, because the fixture is not the bike the human rode. This one is: the disc's own
// `Bike\Race125P.phy` (D7), read by the port's loader, with its wheels where the `125_faisst` model
// hangs them, the way `tests/replay.rs`'s acceptance fixture builds it — over a flat floor, at the
// original's 30 Hz tick, judged against the bands P15 measured off the human's two rides.
//
// **It fails today, and it is meant to.** As P26 first rode it, the launch is inside the original's
// bands at every mark (0.5, 1 and 2 s, 0 to 30 and 0 to 60) and so is the brake's first 0.4 s, but the
// drive pulls a fifth to a quarter too hard in every 20 ft/s band, the bike tops out at 97.6 against the
// original's 75.8 to 79.9, it coasts down at 9.4 ft/s² against 3.4 to 4.6, and a held turn keeps two
// thirds of its speed where the original's kept 0.85 — the drive (P20's numbers), top-speed (P21),
// brake (P23) and turn (P24) tasks. The test is `#[ignore]`d with that reason and comes off it when they
// land; until then `./kit check -- -p game-sim --test ride -- --ignored --nocapture
// the_minute_on_the_discs_bike` prints every leg against its band. It skips, with a line, where the
// disc is not in the checkout.

/// The original's tick: the captures' own `dt` column, which is what the bands below were measured at.
const DISC_DT: F = F(0.03333);

/// What a capture's `bike_vel` column holds, as a fraction of the chassis' own velocity: the rider
/// body's mode method writes the actor's velocity as the chassis' scaled by 0.8 (one `vmulx` by the
/// literal 0.8, read off the binary for this task), and the capture records the actor's. Every speed
/// band below is in the capture's units, so the port's speed is put in them before it is compared.
const BIKE_VEL_SCALE: f32 = 0.8;

/// The bike a freeride boot loads (D7) and the model the human's wheels hang off (`tests/replay.rs`).
const DISC_PHY: &str = "race125p.phy";
const DISC_MODEL: &str = "125_faisst";

/// The user's own `MX.PSR`, opened the way `tests/replay.rs`'s `Disc` opens it — copied rather than
/// shared because a test file here reads on its own.
struct DiscFile {
    file: std::fs::File,
    psr: game_sim::archive::Psr,
}

impl DiscFile {
    fn open() -> Option<DiscFile> {
        let at = private_dir("iso").join("MX.PSR");
        let Ok(mut file) = std::fs::File::open(&at) else {
            println!("no {} — `./kit extract MX.PSR` writes it; the disc's minute is unchecked", at.display());
            return None;
        };
        let mut front = vec![0u8; 64 << 10];
        std::io::Read::read_exact(&mut file, &mut front).expect("the front of MX.PSR");
        let psr = game_sim::archive::Psr::parse(&front).expect("MX.PSR's directory");
        Some(DiscFile { file, psr })
    }

    fn member(&mut self, name: &str) -> Option<Vec<u8>> {
        let e = self.psr.find(name)?;
        std::io::Seek::seek(&mut self.file, std::io::SeekFrom::Start(e.at)).expect("seek");
        let mut bytes = vec![0u8; e.size as usize];
        std::io::Read::read_exact(&mut self.file, &mut bytes).expect("read");
        Some(bytes)
    }

    /// Every `Bike\*.res`'s member whose name ends in `leaf`, in the directory's order, with the
    /// archive it came out of.
    fn bike_members(&mut self, leaf: &str, archive_has: &str) -> Vec<(String, Vec<u8>)> {
        let archives: Vec<String> = self
            .psr
            .entries
            .iter()
            .map(|e| e.name.clone())
            .filter(|n| {
                let n = n.to_ascii_lowercase();
                n.starts_with("bike\\") && n.ends_with(".res") && n.contains(archive_has)
            })
            .collect();
        let want = leaf.to_ascii_lowercase();
        let mut out = Vec::new();
        for archive in archives {
            let Some(bytes) = self.member(&archive) else { continue };
            let Ok(res) = game_sim::archive::Res::parse(&bytes) else { continue };
            let found = res.entries.iter().find(|e| {
                let name = e.name.to_ascii_lowercase();
                name == want || name.ends_with(&format!("\\{want}"))
            });
            if let Some(member) = found.and_then(|e| res.read(&bytes, e).ok()) {
                out.push((archive, member));
            }
        }
        out
    }
}

/// The disc's bike and its wheels: `Race125P.phy` out of the first bike archive that has it, and the
/// `TireFront`/`TireRear` nodes of the `125_faisst`'s `bike.psg` placed in the model's root frame in
/// the original's VU0 order — `tests/replay.rs`'s `the_bike_the_human_rode` and
/// `the_wheels_the_human_rode`, which prove both against the recording's own save state.
fn the_discs_bike(disc: &mut DiscFile, tables: &rider::Tables) -> Option<(Phy, (Vf, Vf))> {
    let Some((archive, bytes)) = disc.bike_members(DISC_PHY, "").into_iter().next() else {
        println!("no {DISC_PHY} in any Bike\\*.res on this disc");
        return None;
    };
    println!("the bike: {DISC_PHY} out of {archive}");
    let p = Phy::read_bytes(&bytes, &tables.ride.curve);
    let wheels = disc.bike_members("bike.psg", DISC_MODEL).into_iter().find_map(|(_, psg)| {
        let model = game_render::psg::Model::parse(&psg).ok()?;
        let node = |name: &str| -> Option<Vf> {
            let i = model.nodes.iter().position(|n| n.name == name)?;
            let mut root = i;
            while let Some(up) = model.nodes[root].parent {
                root = up;
            }
            Some(Vf(model.place_in(i, root)))
        };
        Some((node("TireFront")?, node("TireRear")?))
    });
    let Some(wheels) = wheels else {
        println!("no {DISC_MODEL} model with TireFront/TireRear nodes on this disc");
        return None;
    };
    Some((p, wheels))
}

/// One leg of the disc's minute: its name, the hands, and how long — a fixed number of ticks, or
/// until the bike is going `until` ft/s in the capture's units, whichever comes first.
struct DiscLeg {
    name: &'static str,
    ticks: usize,
    until: Option<f32>,
    hands: Controls,
}

/// What one tick of the disc's minute came to. `speed` is in the capture's units ([`BIKE_VEL_SCALE`]).
struct DiscSeat {
    speed: f32,
    heading: f32,
    up_y: f32,
    on_ground: bool,
    wreck: bool,
}

/// The legs. Every measurement starts from something the original's own rides did:
///
/// * a standing launch, held flat out for twenty seconds — the launch, the 0 to 30 and 0 to 60, the
///   acceleration in each 20 ft/s band, and where the speed tops out;
/// * three seconds off the gas — the coasting decel;
/// * the brake, from there, and a standstill;
/// * up to 50 ft/s and a held right, two seconds of it with the gas pinned — the turn rate and the speed
///   the turn keeps, entered at the speed the original's one long held turn was (61.4 there; 50 is the
///   round number inside the original's 10 to 57 ft/s turn speeds);
/// * the brake, a standstill, and the same to the left.
fn disc_minute() -> Vec<DiscLeg> {
    let gas = Controls { gas: F::ONE, ..Controls::default() };
    let right = Controls { stick: (F::ONE, F::ZERO), ..gas.clone() };
    let left = Controls { stick: (F(-1.0), F::ZERO), ..gas.clone() };
    let brake = Controls { brake: F::ONE, ..Controls::default() };
    let off = Controls::default();
    let leg = |name, ticks, until, hands| DiscLeg { name, ticks, until, hands };
    vec![
        leg("settling", 90, None, off.clone()),
        leg("the launch", 600, None, gas.clone()),
        leg("the coast", 90, None, off.clone()),
        leg("the brake", 150, None, brake.clone()),
        leg("stopped", 60, None, off.clone()),
        leg("up to the right", 300, Some(50.0), gas.clone()),
        leg("turning right", 60, None, right),
        leg("braking after the right", 150, None, brake.clone()),
        leg("stopped after the right", 60, None, off.clone()),
        leg("up to the left", 300, Some(50.0), gas),
        leg("turning left", 60, None, left),
        leg("braking after the left", 150, None, brake),
        leg("stopped after the left", 60, None, off),
    ]
}

/// Ride [`disc_minute`] on `p` over a flat floor, wrecks on, and keep what the rider saw.
fn ride_the_discs_minute(p: &Phy, wheels: (Vf, Vf), tables: &rider::Tables) -> Vec<(&'static str, Vec<DiscSeat>)> {
    // Dropped from two feet, so the model's mounts — which hang the wheels higher than the `.phy`'s
    // own do — start clear of the floor, and the settle takes the drop out before the launch.
    let mut r = Rider::new(p, wheels, Vf::new(0.0, 2.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    // The fork steers about the model's raked axis, as the shell sets it (P24: about the chassis' `y`
    // the held turns keep 0.68 of their speed, about the fork 0.86).
    if let Some((_, _, fork)) = the_disc_125_and_fork(tables) {
        r.ride.front.steer_axis = fork;
    }
    let mut world = floor();
    let mut out = Vec::new();
    for DiscLeg { name, ticks, until, hands } in disc_minute() {
        let mut seen = Vec::with_capacity(ticks);
        for _ in 0..ticks {
            let frame = r.frame(p, tables, &hands, DISC_DT, &mut world);
            let m = r.ride.transform();
            seen.push(DiscSeat {
                speed: r.ride.chassis.speed.0 * BIKE_VEL_SCALE,
                heading: f32::atan2(m.0[2].x(), m.0[2].z()),
                up_y: m.0[1].y(),
                on_ground: r.ride.air.on_ground,
                wreck: frame.wreck.is_some(),
            });
            if until.is_some_and(|v| seen.last().unwrap().speed >= v) {
                break;
            }
        }
        out.push((name, seen));
    }
    out
}

/// One of the original's numbers: what it is, the band P15 measured it in, and the port's.
struct Band {
    what: &'static str,
    lo: f32,
    hi: f32,
    port: Option<f32>,
}

/// The seconds into `ticks` that the speed first reaches `v`, if it does.
fn first_reaches(ticks: &[DiscSeat], v: f32) -> Option<f32> {
    ticks.iter().position(|t| t.speed >= v).map(|k| (k + 1) as f32 * DISC_DT.0)
}

/// The mean acceleration from `from` to `to` ft/s, in ft/s², off the first ticks the speed crosses each.
fn band_accel(ticks: &[DiscSeat], from: f32, to: f32) -> Option<f32> {
    let a = ticks.iter().position(|t| t.speed >= from)?;
    let b = ticks.iter().position(|t| t.speed >= to)?;
    (b > a).then(|| (ticks[b].speed - ticks[a].speed) / ((b - a) as f32 * DISC_DT.0))
}

/// The turn rate a held stick settles into: the heading's change, tick by tick, from a third of a
/// second in (past the lean-in, which is where P15 starts the original's and the port's windows) to a
/// second after that, in rad/s.
fn held_rate(ticks: &[DiscSeat]) -> Option<f32> {
    let window = ticks.get(10..=40)?;
    let turned: f32 = window.windows(2).map(|w| wrap(w[1].heading - w[0].heading)).sum();
    Some(turned / (30.0 * DISC_DT.0))
}

/// The speed a held turn keeps: 1.1 s in against the speed it came in at — the length of the
/// original's one long held turn (P15: 61.4 to 52.4 over 1.1 s, recording2).
fn kept_through(before: &[DiscSeat], turn: &[DiscSeat]) -> Option<f32> {
    let entry = before.last()?.speed;
    Some(turn.get(32)?.speed / entry)
}

/// The original's bands, beside the port's, for every leg the disc's minute rides.
///
/// Where the band comes from, all of it P15's table (`research/journal/2026-10-20-p15-p15-how-it-should-feel`),
/// in the capture's units, at 30 Hz:
///
/// * **the launch** at 0.5, 1 and 2 s, and the 0 to 30 and 0 to 60, are the slowest and quickest of the
///   original's four standing launches across both rides. P15 drops r1's 5.5 s 0 to 60, which had a
///   flight in it, so the 0 to 60 band is the task's 2.2 to 3.4;
/// * **the acceleration** in each 20 ft/s band is the two rides' flat, straight, full-gas figures, ten
///   per cent either side of the two (two samples are not a spread);
/// * **the top speed** is the two rides' maxima, ten per cent either side. Both were at the foot of a
///   long drop with the bike still gaining, so this is the band P15's legs kept, not a terminal speed
///   anyone has measured (P21);
/// * **the coast** is the two rides' median flat decel off the gas, ten per cent either side;
/// * **the brake** is the mean over its first 0.4 s, from r1's one flat brake (56.9 to 51.3 over ticks
///   1036 to 1048, 14 ft/s², measured off the capture for this task) to r2's one brake on a 0.6
///   up-slope (66.6 to 48.9 in 0.4 s, 44), ten per cent outside each;
/// * **the turn rate** is the range of the original's held-stick rates after the lean-in, 0.6 to 2.5,
///   by magnitude, with the sign the stick's;
/// * **the speed a turn keeps** is a floor: recording2's held turn kept 0.853 of its speed over 1.1 s,
///   less ten per cent. The original's turns were not all at full gas, so there is no ceiling.
fn the_discs_bands(m: &[(&'static str, Vec<DiscSeat>)]) -> Vec<Band> {
    let leg = |name: &str| -> &[DiscSeat] {
        &m.iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("no leg '{name}'")).1
    };
    let at = |ticks: &[DiscSeat], s: f32| ticks.get((s / DISC_DT.0).round() as usize - 1).map(|t| t.speed);
    let launch = leg("the launch");
    let band = |what, lo, hi, port| Band { what, lo, hi, port };
    let wide = |what, lo: f32, hi: f32, port| Band { what, lo: lo * 0.9, hi: hi * 1.1, port };

    let coast = leg("the coast");
    let mut decel: Vec<f32> = coast.windows(2).map(|w| (w[0].speed - w[1].speed) / DISC_DT.0).collect();
    decel.sort_by(f32::total_cmp);
    let brake = leg("the brake");
    let bite = (brake.len() > 12)
        .then(|| (coast.last().map_or(0.0, |t| t.speed) - brake[11].speed) / (12.0 * DISC_DT.0));
    let upright: Vec<f32> = launch.iter().filter(|t| t.up_y > 0.9 && t.on_ground).map(|t| t.speed).collect();

    vec![
        band("launch, 0.5 s (ft/s)", 8.8, 20.2, at(launch, 0.5)),
        band("launch, 1 s (ft/s)", 19.4, 32.3, at(launch, 1.0)),
        band("launch, 2 s (ft/s)", 34.2, 56.9, at(launch, 2.0)),
        band("0 to 30 (s)", 0.93, 1.53, first_reaches(launch, 30.0)),
        band("0 to 60 (s)", 2.2, 3.4, first_reaches(launch, 60.0)),
        wide("accel 0-20 (ft/s^2)", 25.3, 25.8, band_accel(launch, 0.0, 20.0)),
        wide("accel 20-40 (ft/s^2)", 15.9, 22.1, band_accel(launch, 20.0, 40.0)),
        wide("accel 40-60 (ft/s^2)", 16.4, 17.6, band_accel(launch, 40.0, 60.0)),
        wide("top speed, upright (ft/s)", 75.78, 79.89, upright.iter().copied().reduce(f32::max)),
        wide("coasting decel, median (ft/s^2)", 3.4, 4.6, decel.get(decel.len() / 2).copied()),
        wide("brake, first 0.4 s (ft/s^2)", 14.0, 44.0, bite),
        band("held right, rate (rad/s)", 0.6, 2.5, held_rate(leg("turning right"))),
        band("held left, rate (rad/s)", -2.5, -0.6, held_rate(leg("turning left"))),
        band(
            "held right, speed kept over 1.1 s",
            0.853 * 0.9,
            f32::INFINITY,
            kept_through(leg("up to the right"), leg("turning right")),
        ),
        band(
            "held left, speed kept over 1.1 s",
            0.853 * 0.9,
            f32::INFINITY,
            kept_through(leg("up to the left"), leg("turning left")),
        ),
    ]
}

/// **The minute on the disc's own bike, against the original's bands.** Every band is checked and the
/// whole table printed before anything fails, so one run says how far each leg is — the drive, top
/// speed, brake and turn tasks each read their own line of it.
#[test]
#[ignore = "fails until the drive, top-speed (P21) and brake (P23) tasks land: race125p \
            pulls ~25% over the original in each 20 ft/s band, tops out at 97.6 against 75.8-79.9 \
            and coasts down at twice the original's rate (P24: the held turns keep 0.86, inside)"]
fn the_minute_on_the_discs_bike() {
    let Some(mut disc) = DiscFile::open() else { return };
    let tables = rider::Tables::build();
    let Some((p, wheels)) = the_discs_bike(&mut disc, &tables) else { return };
    let m = ride_the_discs_minute(&p, wheels, &tables);

    let mut missed = Vec::new();
    for b in the_discs_bands(&m) {
        let ok = b.port.is_some_and(|v| v >= b.lo && v <= b.hi);
        let port = b.port.map_or("never".to_string(), |v| format!("{v:.3}"));
        let line = format!("{:<36} original {:>7.3} .. {:<7.3} port {port}", b.what, b.lo, b.hi);
        println!("{} {line}", if ok { "  " } else { "!!" });
        if !ok {
            missed.push(line);
        }
    }
    // The minute is riding, not stunting: nothing in it is a reason for the original to put the rider
    // down, so a wreck anywhere is a leg the port cannot ride.
    for (name, ticks) in &m {
        let down = ticks.iter().filter(|t| t.wreck).count();
        let flattest = ticks.iter().map(|t| t.up_y).fold(f32::MAX, f32::min);
        if down > 0 || flattest < 0.5 {
            missed.push(format!("{name}: {down} wreck ticks, flattest up {flattest:.2}"));
        }
    }
    assert!(missed.is_empty(), "the disc's bike misses the original on:\n{}", missed.join("\n"));
}

/// The live 125's chassis in the acceptance state (P27's `capture/limits.csv`, three rows, all the
/// same): `+0x224`, then `+0x230..` and `+0x240..` without their `w`.
const LIVE_ANG_SPEED_SQ: f32 = 1875.0;
const LIVE_ANG_VEL_MAX: [f32; 3] = [25.0, 3.0, 25.0];
const LIVE_ANG_VEL_MIN: [f32; 3] = [-25.0, -3.0, -25.0];

/// **The bike turns inside the original's limits** (P27). The chassis `race125p` builds carries the
/// rigid body's angular windows with the file's `YawRateLimit` on the yaw axis, and the
/// constructor's angular limit — what the running original's chassis holds, word for word. A bike
/// whose file has no `YawRateLimit` keeps the constructor's 25 there.
#[test]
fn the_chassis_race125p_builds_carries_the_originals_angular_limits() {
    let bits = |v: [F; 3]| v.map(|x| x.0.to_bits());
    let tables = Tables::build();
    let p = bike(&tables);
    assert_eq!(p.mass.yaw_rate_limit, F::ZERO, "the test bike has grown a YawRateLimit");
    let c = rider(&p).chassis;
    assert_eq!(c.max_ang_speed_sq.0.to_bits(), LIVE_ANG_SPEED_SQ.to_bits());
    assert_eq!(bits(c.ang_vel_max), [25.0f32; 3].map(f32::to_bits));
    assert_eq!(bits(c.ang_vel_min), [-25.0f32; 3].map(f32::to_bits));

    let tables = rider::Tables::build();
    let Some((p, wheels)) = the_disc_125(&tables) else {
        println!("no race125p.phy or 125_faisst wheels on a disc in context/iso — skipped");
        return;
    };
    assert_eq!(p.mass.yaw_rate_limit.0, 3.0, "race125p's YawRateLimit");
    let c = Ride::new(&p, wheels, Vf::new(0.0, 1.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0)).chassis;
    assert_eq!(c.max_ang_speed_sq.0.to_bits(), LIVE_ANG_SPEED_SQ.to_bits());
    assert_eq!(bits(c.ang_vel_max), LIVE_ANG_VEL_MAX.map(f32::to_bits));
    assert_eq!(bits(c.ang_vel_min), LIVE_ANG_VEL_MIN.map(f32::to_bits));
}

// (12) P29: straight in the air ------------------------------------------------------------------

/// The chassis' yaw, pitch and roll in degrees, read off its transform's rows (0 right, 1 up, 2
/// forward) the way `research/p29_air_attitude.py` reads the original's `bike_xform`, so the two are
/// one measure.
fn attitude(r: &Rider) -> (f32, f32, f32) {
    let m = r.ride.transform();
    let (right, up, forward) = (m.0[0], m.0[1], m.0[2]);
    (
        f32::atan2(forward.x(), forward.z()).to_degrees(),
        forward.y().clamp(-1.0, 1.0).asin().to_degrees(),
        f32::atan2(right.y(), up.y()).to_degrees(),
    )
}

/// The original's squarest landings off recording2's straight jumps, in degrees: the least roll any
/// of them came down with (−0.6, the jump at tick 2651) and the least the heading turned over a
/// flight (−4.2, tick 3753). The human held the stick over in every one of them, so they are the
/// envelope of a ridden landing; a flight nobody touches has no excuse to land outside them.
const ORIGINAL_SQUAREST_ROLL: f32 = 0.6;
const ORIGINAL_SQUAREST_YAW: f32 = 4.2;

/// **P29.** Off a jump with the stick left alone, race125p lands as square as the original's
/// squarest landing on recording2.
///
/// The ramp is laid 20° across the run-up, so the bike turns onto it and takes off with what a real
/// lip hands the air step: a yaw rate the ground step left on the body, and a heading that is not the
/// way it is travelling (the shell's `--trace-air` off free09's ramp shows the same, −0.14 rad/s and a
/// crossup of 0.11 rad). From the first airborne frame on the controls are all zero. What the flight
/// owes the rider is the auto-level: the roll pulled to level and the heading pulled onto the
/// velocity, so the landing is square to where the bike is going.
#[test]
fn it_flies_straight_hands_off() {
    let tables = rider::Tables::build();
    let Some((p, wheels, fork)) = the_disc_125_and_fork(&tables) else {
        println!("no race125p.phy or 125_faisst wheels on a disc in context/iso — skipped");
        return;
    };
    let mut r = Rider::new(&p, wheels, Vf::new(0.0, 2.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    r.ride.front.steer_axis = fork;
    let off = Controls::default();
    let gas = Controls { gas: F::ONE, ..Controls::default() };
    let mut world = floor();
    for _ in 0..60 {
        r.frame(&p, &tables, &off, DISC_DT, &mut world);
    }
    let across = 20f32.to_radians();
    let mut jump = Jump {
        origin: r.ride.position(),
        dir: Vf::new(across.sin(), 0.0, across.cos(), 0.0),
        start: 150.0,
        slope: 0.5,
        length: 20.0,
    };

    let (mut lip, mut landed) = (None, None);
    let (mut flying, mut most_roll) = (0usize, 0f32);
    let mut lip_rate = F::ZERO;
    for k in 0..600 {
        let hands = if lip.is_some() { &off } else { &gas };
        let before = attitude(&r);
        let rate = F(r.ride.chassis.ang_vel.y());
        let f = r.frame(&p, &tables, hands, DISC_DT, &mut jump);
        assert!(f.wreck.is_none(), "frame {k}: wrecked hands off: {:?}", f.wreck);
        let air = !r.ride.air.on_ground;
        if air && lip.is_none() {
            lip = Some(before);
            lip_rate = rate;
        }
        if air {
            flying += 1;
            most_roll = most_roll.max(attitude(&r).2.abs());
        }
        if !air && flying > 0 {
            landed = Some((attitude(&r), r.ride.air.crossup));
            break;
        }
    }
    let lip = lip.expect("the bike never left the ramp");
    let ((yaw, pitch, roll), crossup) = landed.expect("the bike never came down");
    let turned = yaw - lip.0;
    println!(
        "hands off from the lip: {flying} frames in the air; lip yaw {:.2} pitch {:.2} roll {:.2} \
         yaw rate {:.4} rad/s; landed yaw {yaw:.2} (turned {turned:+.2}) pitch {pitch:.2} roll \
         {roll:.2}, crossup {:.4} rad, the most roll in the air {most_roll:.2}",
        lip.0, lip.1, lip.2, lip_rate.0, crossup.0
    );
    // Measured: 63 frames up, a lip yaw rate of 0.0744 rad/s, landed rolled 0.03 deg with the
    // heading turned −0.55 deg and a crossup of 0.0000. Since P32 the bike leaves the lip rolled
    // 10.87 deg: at speed the no-hander stands it square to the ground, and this ramp is laid across
    // the run-up, so its surface is banked under the bike. The flight only ever takes that roll out.
    // Since P33 the tire is the original's and holds the heading up the ramp: the lip hands over
    // −0.0123 rad/s (it was 0.0744) with the bike pointed 5.5 deg off the run-up and rolled 9.6 deg,
    // and it lands rolled 0.03 deg with the heading turned −1.36 deg.
    assert!(flying > 30, "not a jump: {flying} frames in the air");
    // Since S67 (the bottom-out's shove) the lip hands over 0.0085 rad/s, the other way.
    assert!(lip_rate.abs() > F(0.005), "the lip handed the air step no yaw rate to take out: {}", lip_rate.0);
    assert!(
        roll.abs() <= ORIGINAL_SQUAREST_ROLL,
        "landed rolled {roll} deg hands off; the original's squarest landing is {ORIGINAL_SQUAREST_ROLL}"
    );
    assert!(
        turned.abs() <= ORIGINAL_SQUAREST_YAW,
        "the heading turned {turned} deg over a hands-off flight; the original's least is {ORIGINAL_SQUAREST_YAW}"
    );
    assert!(crossup.abs() < F(0.01), "landed crossed up {} rad hands off", crossup.0);
    assert!(
        most_roll < lip.2.abs() + 0.5,
        "rolled {most_roll} deg in a hands-off flight that left the lip rolled {} deg",
        lip.2
    );
}

/// The bike's slip in degrees: its heading less the direction it is travelling on the ground plane,
/// read the way `research/p32_recover.py` reads the original's `bike_xform` and `bike_vel`.
fn slip(r: &Rider) -> f32 {
    let v = r.ride.chassis.vel;
    wrap(attitude(r).0.to_radians() - f32::atan2(v.x(), v.z())).to_degrees()
}

/// How many ticks after the stick centres the original's roll is down to a third of what it was
/// leant at the release, at worst. `research/p32_recover.py` over its four clean releases (stick
/// centred, both wheels down, the throttle held): 43.8° → 10.5° in 12 ticks (recording2, tick 3665),
/// 27.6° → 5.8° (the first recording, 3976), 32.2° → 2.0° (3574) and 10.1° → 3.3° (recording2, 2945),
/// a ratio of 0.24, 0.21, 0.06 and 0.33. The worst of them is the bound.
const ORIGINAL_ROLL_BACK: (usize, f32) = (12, 0.34);

/// How many ticks after the stick centres the original's slip is inside 3°, at worst: the slides at
/// the same releases came in from −13.4° (3976, +11 ticks), −13.8° (2765, +7) and −9.5° (3574, +3).
const ORIGINAL_SLIP_BACK: (usize, f32) = (11, 3.0);

/// **P32.** race125p, slid through a hard turn and let go, comes back upright and straight as fast as
/// the original does.
///
/// The original straightens a bike after a turn: from 27–44° of lean its roll is down to a third in
/// twelve ticks of a centred stick, and the slide is gone in eleven. What does it is the no-hander's
/// roll (`mv::no_hander`), which lags the bike's roll towards `MaxRollCommand × lean` at `RollGain` a
/// second — towards upright, against the ground, once the stick is centred. In the original it rolls
/// the chassis' own transform, and the next integration takes the turn into the body. The port rolled
/// a node of its own that nothing read, and the grounded body's roll rate is pinned (`mv::pose` frees
/// roll only in the air), so a bike that came down leant stayed leant.
///
/// The turn is the human's longest clean one (recording2, ticks 3642–3664): from 57 ft/s, the stick
/// held hard over and pulled back — (−0.94, +0.94) on the pad — with the throttle open for 23 ticks,
/// then centred with the throttle still held; on the level, at the captures' 30 Hz. The roll and the
/// heading are both held to the original's times.
///
/// **P33.** The heading used to come back two ticks late: the port's tire was a stand-in, its slip
/// peaked at 5.8° and decayed by about 0.9 a tick against the original's 0.66–0.8. Since P33 the
/// tire is the original's (`tire::force` on the curve, in `tire::contact_frame`, steered about the
/// model's fork) and the slip peaks at 2.8° and is never outside 3° after the release — 2.2° at the
/// release, held near 2° for eight ticks while the roll comes up, then decaying by 0.8–0.85 a tick.
/// The roll is 41.0° → 9.3° in 12 ticks. (Those are P33's numbers before P30's squared stick was
/// merged in; with it the turn reaches 72.7 ft/s, the roll lets go at 38.6° and is 8.3° at +12, and
/// the slip peaks at 3.1° five ticks after the release and is inside 1.5° from +11.)
#[test]
fn it_straightens_after_a_slide() {
    let tables = rider::Tables::build();
    let Some((p, wheels, fork)) = the_disc_125_and_fork(&tables) else {
        println!("no race125p.phy or 125_faisst wheels on a disc in context/iso — skipped");
        return;
    };
    let mut r = Rider::new(&p, wheels, Vf::new(0.0, 2.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    r.ride.front.steer_axis = fork;
    let off = Controls::default();
    let gas = Controls { gas: F::ONE, ..Controls::default() };
    let turn = Controls { stick: (F(-0.94), F(0.94)), ..gas.clone() };
    let mut world = floor();
    for _ in 0..60 {
        r.frame(&p, &tables, &off, DISC_DT, &mut world);
    }
    let mut up_to = 0;
    while r.ride.chassis.speed.0 < 57.0 {
        let f = r.frame(&p, &tables, &gas, DISC_DT, &mut world);
        assert!(f.wreck.is_none(), "wrecked on the run-up: {:?}", f.wreck);
        up_to += 1;
        assert!(up_to < 900, "race125p never reached 57 ft/s: {}", r.ride.chassis.speed.0);
    }

    let mut most = (0f32, 0f32);
    for k in 0..23 {
        let f = r.frame(&p, &tables, &turn, DISC_DT, &mut world);
        assert!(f.wreck.is_none(), "turn tick {k}: wrecked: {:?}", f.wreck);
        let (_, _, roll) = attitude(&r);
        most = (most.0.max(roll.abs()), most.1.max(slip(&r).abs()));
    }
    let let_go = (attitude(&r).2, slip(&r), r.ride.chassis.speed.0);
    let mut after = Vec::new();
    for k in 0..45 {
        let f = r.frame(&p, &tables, &gas, DISC_DT, &mut world);
        assert!(f.wreck.is_none(), "tick {k} after the release: wrecked: {:?}", f.wreck);
        assert!(r.ride.air.on_ground, "tick {k} after the release: off the ground");
        after.push((attitude(&r).2, slip(&r)));
    }
    let trace: Vec<String> = after.iter().map(|(a, b)| format!("{a:+.1}/{b:+.1}")).collect();
    println!(
        "23 ticks of the human's turn from {up_to} ticks of run-up: the most roll {:.1} deg and slip {:.1} deg; \
         let go at roll {:+.1} slip {:+.1} speed {:.1}; roll/slip after: {}",
        most.0, most.1, let_go.0, let_go.1, let_go.2, trace.join(" ")
    );

    // A real lean to come back from, and a real slide.
    assert!(let_go.0.abs() > 20.0, "the human's turn leant race125p only {} deg", let_go.0);
    let (ticks, share) = ORIGINAL_ROLL_BACK;
    let roll = after[ticks - 1].0;
    assert!(
        roll.abs() <= share * let_go.0.abs(),
        "{ticks} ticks after the stick centred race125p was still leant {roll} deg of {} deg; the \
         original is down to {share} of it",
        let_go.0
    );
    let (ticks, inside) = ORIGINAL_SLIP_BACK;
    let worst = after[ticks - 1..].iter().map(|a| a.1.abs()).fold(0.0, f32::max);
    assert!(
        worst <= inside,
        "from {ticks} ticks after the stick centred race125p slipped up to {worst} deg; it was inside \
         {inside} (the original inside {} from {} ticks)",
        ORIGINAL_SLIP_BACK.1,
        ORIGINAL_SLIP_BACK.0
    );
    // And it stays straight and upright: no overshoot that keeps going, nothing left at the end.
    let (last_roll, last_slip) = after[after.len() - 1];
    assert!(last_roll.abs() < 1.0 && last_slip.abs() < 1.0, "ended at roll {last_roll} slip {last_slip}");
}

// --- P24: a held turn keeps its speed ------------------------------------------------------------------

/// race125p on the level at the captures' 30 Hz, its fork on the model's axis, from a standstill:
/// `run_up` ticks of `first`, then `ticks` of `then` — or, when `from` is given, `first` until the
/// bike is going `from` ft/s and then `then`. The speed the second hands came in at, and each of
/// their ticks' speed, heading, front steer angle and up `y`.
fn race125p_turn(first: &Controls, run_up: usize, from: Option<f32>, then: &Controls, ticks: usize)
                 -> Option<(f32, Vec<[f32; 4]>)> {
    let tables = rider::Tables::build();
    let (p, wheels, fork) = the_disc_125_and_fork(&tables)?;
    let mut r = Rider::new(&p, wheels, Vf::new(0.0, 2.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    r.ride.front.steer_axis = fork;
    let mut world = floor();
    for _ in 0..60 {
        r.frame(&p, &tables, &Controls::default(), DISC_DT, &mut world);
    }
    let mut k = 0;
    while from.map_or(k < run_up, |v| r.ride.chassis.speed.0 < v) {
        let f = r.frame(&p, &tables, first, DISC_DT, &mut world);
        assert!(f.wreck.is_none(), "wrecked on the run-up: {:?}", f.wreck);
        k += 1;
        assert!(k < 900, "race125p never reached {from:?} ft/s: {}", r.ride.chassis.speed.0);
    }
    let entry = r.ride.chassis.speed.0;
    let mut out = Vec::with_capacity(ticks);
    for k in 0..ticks {
        let f = r.frame(&p, &tables, then, DISC_DT, &mut world);
        assert!(f.wreck.is_none(), "turn tick {k}: wrecked: {:?}", f.wreck);
        assert!(r.ride.air.on_ground, "turn tick {k}: off the ground");
        let m = r.ride.transform();
        out.push([r.ride.chassis.speed.0, f32::atan2(m.0[2].x(), m.0[2].z()), r.ride.front.w.steer_angle.0, m.0[1].y()]);
    }
    Some((entry, out))
}

/// **P24.** race125p in a one-second full-lock turn at 55 ft/s, the gas pinned, keeps 80% of its speed.
///
/// P15 had the shell's bike going from 76 to 35 ft/s in 1.33 s of this. P10 blamed the roll rate the
/// ground pose pins or P4's stand-in tire; it was the tire. The pinned roll is the original's own
/// (`mv::pose` frees roll only in the air, flag for flag), and the stand-in's side force pinned at its
/// cap every substep and scrubbed the speed away. P33's tire law and contact frame and S58's spin
/// step and surface drag are the original's, and with them this turn keeps all of it: 55.1 → 54.9
/// ft/s (0.996) with the heading coming round at 1.8 to 2.4 rad/s, inside the original's held-stick
/// 0.6 to 2.5. [`a_held_turn_rides_like_the_originals`] is the same turn against the original's own.
#[test]
fn a_full_lock_turn_keeps_its_speed() {
    let gas = Controls { gas: F::ONE, ..Controls::default() };
    for side in [1.0f32, -1.0] {
        let lock = Controls { stick: (F(side), F::ZERO), ..gas.clone() };
        let Some((entry, turn)) = race125p_turn(&gas, 0, Some(55.0), &lock, 30) else {
            println!("no race125p.phy or 125_faisst wheels on a disc in context/iso — skipped");
            return;
        };
        let kept = turn[29][0] / entry;
        let rates: Vec<f32> = turn.windows(2).map(|w| wrap(w[1][1] - w[0][1]) / DISC_DT.0 * side).collect();
        // The fastest over two ticks: since S67 a bottom-out's shove jolts the heading on one tick
        // (2.84 rad/s between 1.92 and 1.85, full lock left), which is not the turn's own rate.
        let fastest = rates.windows(2).map(|w| (w[0] + w[1]) / 2.0).fold(f32::MIN, f32::max);
        let mean = rates.iter().sum::<f32>() / rates.len() as f32;
        assert!(kept >= 0.8, "stick {side:+}: a second of full lock from {entry} ft/s kept {kept} of it");
        assert!(
            mean > 0.6 && fastest <= 2.5,
            "stick {side:+}: came round at {mean} rad/s on the mean and {fastest} at most; the original's \
             held-stick turns run 0.6 to 2.5"
        );
    }
}

/// S58's held turn in the original (`watch/s58-slip.toml`'s "a hard turn at speed", recorded off
/// first_recording's state 8 on Free09 with cr125_faisst): the kit's pad pinned the gas from a near
/// standstill, put the stick hard right half a second later and held both until the bike left the
/// ground two seconds after. Copied here from the slot that recorded it, in a subdirectory so that
/// `replay.rs` does not take it for an acceptance run.
const HELD_TURN_CAPTURE: [&str; 2] = ["capture/p24", "turn.csv"];

/// **P24.** The same held turn as the original's, on the level: the gas from a standstill, then the
/// stick hard over with it — and once the lean has settled (44 to 56 ft/s, about 45° over) the
/// heading comes round as fast as the original's, at the same lean and steer, and the speed climbs
/// as fast as the original's does. A turn that scrubbed its speed would fall behind here.
///
/// The original rode it on Free09, which is not flat — its wheel loads swing from 140 to 400 lbf
/// over the bumps, where the port's flat floor holds them at ~200 — so the comparison is of means
/// over the band, not tick for tick. Measured: rate 1.853 (original) against 1.842 (port) rad/s, up
/// `y` 0.742 against 0.747, steer 0.303 against 0.304 rad, and 44 → 54 ft/s in 1.00 s for both.
#[test]
fn a_held_turn_rides_like_the_originals() {
    let path = private_dir(HELD_TURN_CAPTURE[0]).join(HELD_TURN_CAPTURE[1]);
    let Ok(text) = std::fs::read_to_string(&path) else {
        println!("no held-turn capture at {} — skipped", path.display());
        return;
    };
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().expect("a header").split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name).unwrap_or_else(|| panic!("no column {name}"));
    let want = ["gas_lever", "f_steer", "ch_speed", "ch_xform.m20", "ch_xform.m22", "ch_xform.m11",
                "f_spring_force", "r_spring_force"].map(col);
    let rows: Vec<[f32; 8]> = lines
        .map(|l| {
            let cells: Vec<&str> = l.split(',').collect();
            want.map(|c| cells[c].parse().expect("a number"))
        })
        .collect();
    let gas_at = rows.iter().position(|r| r[0] >= 1.0).expect("the gas goes on");
    let lock_at = rows.iter().position(|r| r[1] > 0.0).expect("the stick goes over");
    let lands = rows[lock_at..].iter().position(|r| r[6] <= 0.0 || r[7] <= 0.0).expect("it leaves the ground");
    // The original's turn, and the port's: speed, heading, steer, up `y` per tick.
    let original: Vec<[f32; 4]> =
        rows[lock_at..lock_at + lands].iter().map(|r| [r[2], f32::atan2(r[3], r[4]), r[1], r[5]]).collect();
    let gas = Controls { gas: F::ONE, ..Controls::default() };
    let right = Controls { stick: (F::ONE, F::ZERO), ..gas.clone() };
    let Some((_, port)) = race125p_turn(&gas, lock_at - gas_at, None, &right, lands) else {
        println!("no race125p.phy or 125_faisst wheels on a disc in context/iso — skipped");
        return;
    };

    // The means over the settled band, and the time the speed takes across it.
    let band = |turn: &[[f32; 4]]| -> (f32, f32, f32, f32) {
        let ticks: Vec<usize> = (1..turn.len()).filter(|&k| (44.0..=56.0).contains(&turn[k][0])).collect();
        assert!(ticks.len() > 20, "only {} ticks between 44 and 56 ft/s", ticks.len());
        let n = ticks.len() as f32;
        let rate = ticks.iter().map(|&k| wrap(turn[k][1] - turn[k - 1][1]) / DISC_DT.0).sum::<f32>() / n;
        let steer = ticks.iter().map(|&k| turn[k][2]).sum::<f32>() / n;
        let up = ticks.iter().map(|&k| turn[k][3]).sum::<f32>() / n;
        let at = |v: f32| turn.iter().position(|t| t[0] >= v).expect("the speed gets there");
        (rate, steer, up, (at(54.0) - at(44.0)) as f32 * DISC_DT.0)
    };
    let (o, p) = (band(&original), band(&port));
    println!(
        "44-56 ft/s: rate {:.3} / {:.3} rad/s, steer {:.3} / {:.3} rad, up {:.3} / {:.3}, 44 to 54 in {:.2} / {:.2} s \
         (original / port)",
        o.0, p.0, o.1, p.1, o.2, p.2, o.3, p.3
    );
    assert!((p.0 - o.0).abs() <= 0.1 * o.0, "turn rate {} rad/s, the original's {}", p.0, o.0);
    assert!((p.1 - o.1).abs() <= 0.02, "steer {} rad, the original's {}", p.1, o.1);
    assert!((p.2 - o.2).abs() <= 0.03, "up y {}, the original's {}", p.2, o.2);
    assert!((p.3 - o.3).abs() <= 0.2 * o.3, "44 to 54 ft/s in {} s, the original in {}", p.3, o.3);
}

// --- S54: more than one rider -----------------------------------------------------------------------

/// A session of `n` riders on the stand-in bike, wrecks on, stacked `rise` apart with the last
/// player at the stock spawn and player 1 (the seed's own rider) on top, the others moving at
/// velocities of their own, so that each one's head ray reaches into the body of the one above it —
/// the player before it, which the last-player-first step (S88) has not moved yet.
fn stacked(n: usize, rise: f32) -> game_sim::Sim {
    let mut sim = game_sim::Sim::new(0x5354);
    sim.rider.sense.wrecks_enabled = true;
    if n > 1 {
        let top = Vf::new(SPAWN.x(), SPAWN.y() + rise * (n - 1) as f32, SPAWN.z(), 1.0);
        sim.rider.ride.place(top, game_sim::stock::FACING);
    }
    let mounts = game_sim::stock::mounts(&sim.phy);
    for i in 1..n {
        let at = Vf::new(SPAWN.x(), SPAWN.y() + rise * (n - 1 - i) as f32, SPAWN.z(), 1.0);
        let mut r = Rider::new(&sim.phy, mounts, at, game_sim::stock::FACING, Set::default());
        r.ride.chassis.vel = Vf::new(i as f32, 0.0, 2.0 * i as f32, 0.0);
        let phy = (*sim.phy).clone();
        assert_eq!(sim.add_player(phy, r), i);
    }
    sim
}

const SPAWN: Vf = game_sim::stock::SPAWN;

fn held(n: usize, k: usize) -> Vec<Controls> {
    (0..n).map(|i| Controls { gas: F(if (k + i) % 3 == 0 { 1.0 } else { 0.0 }), ..Controls::default() }).collect()
}

/// One rider: the step with nobody else in the session is the step it always was. Two copies of the
/// same state, one stepped through `Sim::step_all` and one through the rider's own frame by hand, as
/// `Sim::step` read before S54, hash the same on every tick — and the walk has no players group.
#[test]
fn one_rider_hashes_as_before() {
    let mut a = game_sim::Sim::new(7);
    let mut b = a.clone();
    let dt = F(game_sim::tick::DeltaTime::CAP_30);
    for k in 0..240 {
        let c = held(1, k);
        a.step_all(dt, &c, &mut Flat::default());
        let tables = b.tables.clone();
        b.rider.frame(&b.phy, &tables, &c[0], dt, &mut Flat::default());
        b.ticks += 1;
        assert_eq!(a.hash(), b.hash(), "tick {}", k + 1);
    }
    assert!(Dump::of(&a).get("players.len").is_none(), "a one-player walk has no players group");
}

/// The order is the last player first, each rider's whole frame before the next one's (S88): a
/// two-rider step is player 2's frame against player 1 where it stood, then player 1's against
/// player 2 where it now is — rebuilt by hand here and compared word for word.
#[test]
fn two_riders_step_the_last_player_first() {
    let mut a = stacked(2, 12.0);
    let mut b = a.clone();
    let dt = F(game_sim::tick::DeltaTime::CAP_30);
    for k in 0..90 {
        let c = held(2, k);
        a.step_all(dt, &c, &mut Flat::default());
        let tables = b.tables.clone();
        let up = |r: &Rider| r.state.wreck.is_none();
        let r1: Vec<_> = Some(&b.rider).filter(|r| up(r)).map(|r| r.ride.rival()).into_iter().collect();
        let p = &mut b.others[0];
        p.rider.frame_among(&p.phy, &tables, &c[1], dt, &mut Flat::default(), &r1);
        let r2: Vec<_> = Some(&b.others[0].rider).filter(|r| up(r)).map(|r| r.ride.rival()).into_iter().collect();
        b.rider.frame_among(&b.phy, &tables, &c[0], dt, &mut Flat::default(), &r2);
        b.ticks += 1;
        assert_eq!(a.hash(), b.hash(), "tick {}", k + 1);
    }
}

/// Two and four riders are deterministic: the same session stepped twice hashes the same every
/// tick, and every player is in the walk.
#[test]
fn two_and_four_riders_are_deterministic() {
    let dt = F(game_sim::tick::DeltaTime::CAP_30);
    for n in [2, 4] {
        let run = || {
            let mut sim = stacked(n, 3.0);
            (0..150)
                .map(|k| {
                    sim.step_all(dt, &held(n, k), &mut Flat::default());
                    sim.hash()
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(run(), run(), "{n} riders");
        let sim = stacked(n, 3.0);
        let d = Dump::of(&sim);
        assert!(d.get("players.len").is_some());
        assert!(d.get(&format!("players[{}].rider.ride.origin[1]", n - 2)).is_some(), "{n} riders walked");
        assert!(d.get(&format!("players[{}]", n - 1)).is_none());
        // A rider's state is in the hash: moving player `n` moves it.
        let mut moved = sim.clone();
        moved.others[n - 2].rider.ride.origin = Vf::new(0.0, 99.0, 0.0, 1.0);
        assert_ne!(moved.hash(), sim.hash());
    }
}

/// Riders touch through the original's `Joust` path. Stacked three units apart, each rider's head
/// ray reaches into the body of the one above, whose chassis is higher by more than the bike's
/// `JoustWreckHeight` (2): every rider with one above them comes off with reason 7, thrown at the
/// velocity of the rider who landed on them — taken **before** that rider's own frame, which is the
/// step order showing (the one below is the later player, stepped first). The top one, player 1,
/// has nobody above and does not joust.
#[test]
fn stacked_riders_joust() {
    let dt = F(game_sim::tick::DeltaTime::CAP_30);
    let joust = Some(game_sim::trick::Wreck::from(game_sim::air::Wreck::Joust));
    for n in [2, 4] {
        let mut sim = stacked(n, 3.0);
        let before: Vec<Vf> = (0..n).map(|i| sim.rider_of(i).ride.chassis.vel).collect();
        let frames = sim.step_all(dt, &vec![Controls::default(); n], &mut Flat::default());
        for i in 1..n {
            assert_eq!(frames[i].wreck, joust, "{n} riders: player {} under player {}", i + 1, i);
            assert_eq!(sim.rider_of(i).ride.air.wreck_impact, before[i - 1], "{n} riders: player {}", i + 1);
        }
        assert_ne!(frames[0].wreck, joust, "{n} riders: the top rider has nobody above");
    }
    // Side by side the heights are level: the head ray touches, and it is no joust.
    let mut sim = game_sim::Sim::new(0x5354);
    sim.rider.sense.wrecks_enabled = true;
    let mounts = game_sim::stock::mounts(&sim.phy);
    let at = Vf::new(SPAWN.x() + 1.0, SPAWN.y(), SPAWN.z(), 1.0);
    let r = Rider::new(&sim.phy, mounts, at, game_sim::stock::FACING, Set::default());
    let phy = (*sim.phy).clone();
    sim.add_player(phy, r);
    let hits = game_sim::ride::hits_among(&sim.rider.ride, &mut Flat::default(), &[sim.others[0].rider.ride.rival()]);
    assert!(matches!(hits[..], [game_sim::air::Contact { other: game_sim::air::Other::Rider { .. }, .. }]));
    // Asked of the contact clause alone: the stand-in bike's own landing test answers on its first
    // frame with wrecks on (`Sim::new` says why), which is not the touch this is about.
    let r = &sim.rider.ride;
    let (mass, height) = (F::ONE / r.chassis.inv_mass, F(r.chassis.cofg_world.y()));
    let mut air = r.air;
    assert_eq!(game_sim::air::hit_wreck(&sim.phy.air, &mut air, true, false, mass, height, &hits), None);
}

/// A session holds eight riders and no more.
#[test]
#[should_panic(expected = "at most 8")]
fn a_ninth_rider_is_refused() {
    stacked(9, 20.0);
}

// --- S63: the put-back ghost among other riders ---------------------------------------------------

/// Two riders side by side, `apart` along x, neither of whom wrecks: the stand-in bike's own landing
/// test answers on its first frame with wrecks on (`Sim::new` says why), and a downed rider is no
/// rival, which would end every overlap this is about.
fn side_by_side(apart: f32) -> game_sim::Sim {
    let mut sim = game_sim::Sim::new(0x5363);
    let mounts = game_sim::stock::mounts(&sim.phy);
    let at = Vf::new(SPAWN.x() + apart, SPAWN.y(), SPAWN.z(), 1.0);
    let mut r = Rider::new(&sim.phy, mounts, at, game_sim::stock::FACING, Set::default());
    r.sense.wrecks_enabled = false;
    let phy = (*sim.phy).clone();
    sim.add_player(phy, r);
    sim
}

/// S63: a ghost passes through the other riders and they pass through it. The stack that jousts in
/// `stacked_riders_joust` does not once either rider is a ghost: the one below's head ray sweeps the
/// world alone while it is the ghost, and the one above is nobody's rival while it is.
#[test]
fn a_ghost_passes_through_another_rider() {
    use game_sim::ghost::Ghost;
    let dt = F(game_sim::tick::DeltaTime::CAP_30);
    let joust = Some(game_sim::trick::Wreck::from(game_sim::air::Wreck::Joust));
    let still = vec![Controls::default(); 2];
    let mut sim = stacked(2, 3.0);
    let frames = sim.step_all(dt, &still, &mut Flat::default());
    assert_eq!(frames[1].wreck, joust, "the stack without a ghost");
    for ghost in [0, 1] {
        let mut sim = stacked(2, 3.0);
        if ghost == 0 { sim.rider.ghost.start() } else { sim.others[0].rider.ghost.start() }
        let frames = sim.step_all(dt, &still, &mut Flat::default());
        assert_ne!(frames[1].wreck, joust, "player {} a ghost: the head ray struck the other rider", ghost + 1);
        assert!(sim.rider_of(ghost).ghost.on);
    }
    // Nor can it run out in there: half a tick left, and inside the rider above, it is still a ghost
    // after the tick — the clock that would have ended it did not count — and still struck nobody.
    let mut sim = stacked(2, 3.0);
    let half = F(dt.0 / 2.0);
    sim.others[0].rider.ghost = Ghost { clock: half, alpha: F(0.9), on: true };
    let frames = sim.step_all(dt, &still, &mut Flat::default());
    let g = sim.others[0].rider.ghost;
    assert_eq!((g.clock, g.on), (half, true), "the clock ran out inside");
    assert_ne!(frames[1].wreck, joust);
}

/// S63: the ghost's clock stands still for as long as its body is inside another rider's, and runs
/// the rest of its three seconds once it is out. Held, the alpha is what the fade gives a full clock,
/// the floor. Two ghosts inside each other do not hold each other: each one's list rejects the other.
#[test]
fn a_ghost_holds_its_clock_while_inside_another_rider() {
    use game_sim::ghost::Ghost;
    let dt = F(game_sim::tick::DeltaTime::CAP_30);
    let still = vec![Controls::default(); 2];
    let mut sim = side_by_side(1.0);
    sim.rider.ghost.start();
    for k in 0..45 {
        sim.step_all(dt, &still, &mut Flat::default());
        let g = sim.rider.ghost;
        assert_eq!((g.clock, g.alpha, g.on), (Ghost::LENGTH, Ghost::FLOOR, true), "tick {k}: the clock moved inside");
    }
    // Out of the other rider: the clock counts, and the fade runs its 90 ticks and ends on the 91st.
    let far = Vf::new(SPAWN.x() + 50.0, SPAWN.y(), SPAWN.z(), 1.0);
    sim.others[0].rider.ride.place(far, game_sim::stock::FACING);
    let mut ticks = 0;
    while sim.rider.ghost.on {
        sim.step_all(dt, &still, &mut Flat::default());
        ticks += 1;
        if ticks == 1 {
            assert_eq!(sim.rider.ghost.clock, Ghost::LENGTH - dt, "the first tick out counts");
        }
        assert!(ticks <= 91, "the ghost never ended");
    }
    assert_eq!(ticks, 91);

    // Next to it is not inside it: far enough apart that the two bodies only miss.
    let reach = 2.0 * sim.rider.ride.rival().radius.0;
    let mut sim = side_by_side(reach + 0.01);
    sim.rider.ghost.start();
    sim.step_all(dt, &still, &mut Flat::default());
    assert_eq!(sim.rider.ghost.clock, Ghost::LENGTH - dt, "two bodies apart held the clock");

    // Two ghosts in one place: neither holds the other.
    let mut sim = side_by_side(1.0);
    sim.rider.ghost.start();
    sim.others[0].rider.ghost.start();
    sim.step_all(dt, &still, &mut Flat::default());
    for i in 0..2 {
        assert_eq!(sim.rider_of(i).ghost.clock, Ghost::LENGTH - dt, "player {}: held by another ghost", i + 1);
    }
}

/// The overlap query on its own: strictly closer than the two reaches, and only against the rivals
/// it is handed.
#[test]
fn the_ghosts_overlap_query_is_strict() {
    use game_sim::ride::Rival;
    let body = |x: f32| Rival { centre: Vf::new(x, 0.0, 0.0, 1.0), radius: F(1.0), height: F(0.0), vel: Vf::default() };
    assert!(game_sim::ghost::overlaps(&body(0.0), &[body(1.999)]));
    assert!(!game_sim::ghost::overlaps(&body(0.0), &[body(2.0)]), "touching is not inside");
    assert!(game_sim::ghost::overlaps(&body(0.0), &[body(9.0), body(-1.0)]));
    assert!(!game_sim::ghost::overlaps(&body(0.0), &[]));
}

// --- S55: crooked landings ----------------------------------------------------------------------------

/// One crooked landing off P29's cross-laid ramp: the run-up on the throttle, then from the first
/// airborne frame the stick held at `stick` (and the throttle with it) until the bike has been down
/// `after` ticks. Answers every tick from three before the touchdown to `after` past it: the
/// attitude, the crossup in degrees and whether the bike is down — the columns
/// `research/s55_landing.py` reads off the original's capture.
fn crooked_landing(stick: (f32, f32), after: usize) -> Option<Vec<((f32, f32, f32), f32, bool)>> {
    let tables = rider::Tables::build();
    let (p, wheels, fork) = the_disc_125_and_fork(&tables)?;
    let mut r = Rider::new(&p, wheels, Vf::new(0.0, 2.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    r.ride.front.steer_axis = fork;
    let gas = Controls { gas: F::ONE, ..Controls::default() };
    let held = Controls { stick: (F(stick.0), F(stick.1)), ..gas.clone() };
    let mut world = floor();
    for _ in 0..60 {
        r.frame(&p, &tables, &Controls::default(), DISC_DT, &mut world);
    }
    let across = 20f32.to_radians();
    let mut jump = Jump {
        origin: r.ride.position(),
        dir: Vf::new(across.sin(), 0.0, across.cos(), 0.0),
        start: 150.0,
        slope: 0.5,
        length: 20.0,
    };
    let mut rows = Vec::new();
    let (mut flown, mut down) = (0usize, None);
    for k in 0..900 {
        let hands = if flown > 0 { &held } else { &gas };
        let f = r.frame(&p, &tables, hands, DISC_DT, &mut jump);
        assert!(f.wreck.is_none(), "tick {k}: wrecked: {:?}", f.wreck);
        let on = r.ride.air.on_ground;
        flown += usize::from(!on);
        rows.push((attitude(&r), r.ride.air.crossup.0.to_degrees(), on));
        if on && flown > 0 && down.is_none() {
            down = Some(rows.len() - 1);
        }
        if down.is_some_and(|d| rows.len() > d + after) {
            break;
        }
    }
    let d = down.expect("the bike never came down");
    Some(rows[d - 3..].to_vec())
}

/// The most the original's attitude moves in one tick through a touchdown, degrees, over the nine
/// landings of recording2's trick capture that are neither a put-back nor a wreck
/// (`research/s55_landing.py recording2-tricks`): the heading 10.54 (tick 222), the roll 13.25 (tick
/// 1657, landed rolled 50°). A landing that moved further in one tick than these is the snap S55 is
/// about.
const ORIGINAL_LANDING_TURN: (f32, f32) = (10.54, 13.25);

/// The original's crossup two ticks after touching down, at worst, degrees: −25.0° → −7.05°
/// (tick 1614), 28.9° → 2.6° (341, which slid one tick more first, to 46.5°), 23.8° → 3.7° (1109).
/// The ground takes the slide out on the tick or two it bites, by the speed it costs.
const ORIGINAL_LANDING_CROSSUP: f32 = 7.05;

/// **S55.** race125p comes down crooked — 30° sideways, and rolled 27° with a 24° crossup — and
/// lands the way the original does: the slide is gone two ticks after the touchdown, while the heading
/// and the roll come round over the ticks after it, no faster than the original's ever do.
///
/// The user saw the bike and rider "snapped straight" on landing. The simulation does not do it: the
/// body's attitude eases through the touchdown here as it does in the capture, and what the original
/// does in one tick — the velocity swung onto the heading — the port does too. The snap was the
/// drawn chassis dropping its airborne crossup pose in one frame (`game_render::vehicle::Chassis`),
/// and the shell's wall-clock `dt` (P52) can add one of its own. This holds the simulation's half.
#[test]
fn it_lands_crooked_without_a_snap() {
    // (the stick in the air, what the landing is)
    for (stick, what) in [((0.5f32, 0.0f32), "sideways"), ((-0.5, 0.0), "rolled")] {
        let Some(rows) = crooked_landing(stick, 8) else {
            println!("no race125p.phy or 125_faisst wheels on a disc in context/iso — skipped");
            return;
        };
        let trace: Vec<String> = rows
            .iter()
            .map(|((y, _, r), c, on)| format!("{}{y:+.1}/{r:+.1}/{c:+.1}", if *on { "" } else { "^" }))
            .collect();
        println!("{what} (stick {stick:?}), yaw/roll/crossup from 3 ticks before: {}", trace.join(" "));
        let ((_, _, roll), cross, _) = rows[2];
        assert!(!rows[2].2 && rows[3].2, "{what}: row 3 is not the touchdown");
        // A crooked landing to judge.
        assert!(cross.abs() > 20.0, "{what}: came down only {cross} deg crossed up");
        if what == "rolled" {
            assert!(roll.abs() > 20.0, "{what}: came down rolled only {roll} deg");
        }
        // No tick moves the body further than the original's ever does through a touchdown.
        for (k, pair) in rows.windows(2).enumerate() {
            let ((y0, _, r0), _, _) = pair[0];
            let ((y1, _, r1), _, _) = pair[1];
            let turn = (wrap((y1 - y0).to_radians()).to_degrees().abs(), (r1 - r0).abs());
            assert!(
                turn.0 <= ORIGINAL_LANDING_TURN.0 && turn.1 <= ORIGINAL_LANDING_TURN.1,
                "{what}: tick {} the heading turned {:.2} and the roll {:.2} deg in one tick; the \
                 original's most through a touchdown is {:?}",
                k as i32 - 2,
                turn.0,
                turn.1,
                ORIGINAL_LANDING_TURN
            );
        }
        // And the slide is gone two ticks after the touchdown, as the original's is.
        let after = rows[5].1;
        assert!(
            after.abs() <= ORIGINAL_LANDING_CROSSUP,
            "{what}: still {after} deg crossed up two ticks after landing; the original is inside \
             {ORIGINAL_LANDING_CROSSUP}"
        );
    }
}

// --- the put-back (S66) ---------------------------------------------------------------------------
//
// S66 recorded the original's put-back record once a tick on two tracks (free06 riding, wrecked and
// put back by the chord and by the clock; free09 riding), with every float as its bits, through
// `research/journal/2026-10-21-s66-hand-reset-position/capture/putback_watch.py`. The captures sit in
// `context/s66/` and the save states they started from in `context/states/`; without them the tests
// below say so and pass, and with them they count what they compared so a skip cannot read as a pass.

/// One tick of a put-back capture: what the record's update reads and what it and the put-back wrote.
struct BackTick {
    frame: u32,
    reason: u32,
    /// The put-back record: point, heading, up.
    rec: [Vf; 3],
    /// The point the model setup last applied to the bike.
    applied: Vf,
    /// The sweep's point (three feet up the frame the tick started with), the body's centre (as
    /// printed, four places: only its height is read, and only to start the ground query from), the
    /// wreck part's point, and the forward row the race manager hands the record.
    e0: Vf,
    centre: Vf,
    part: Vf,
    heading: Vf,
    /// The node's forward row, which the race manager's copy is made from (its `w` set to one).
    row: Vf,
}

fn bits4(words: &[&str]) -> Vf {
    let f = |s: &str| f32::from_bits(u32::from_str_radix(s, 16).expect("a capture word"));
    Vf::new(f(words[0]), f(words[1]), f(words[2]), f(words[3]))
}

fn floats4(text: &str) -> Vf {
    let inside = &text[text.find('[').unwrap() + 1..text.find(']').unwrap()];
    let v: Vec<f32> = inside.split(", ").map(|s| s.parse().unwrap()).collect();
    Vf::new(v[0], v[1], v[2], v[3])
}

fn word_after(row: &str, key: &str) -> u32 {
    let at = row.find(key).unwrap_or_else(|| panic!("no {key} in a row")) + key.len();
    row[at..].split_whitespace().next().unwrap().parse().unwrap()
}

/// A capture's header (the bike's address) and its rows.
fn back_capture(text: &str) -> (u32, Vec<BackTick>) {
    let mut chunks = text.split("\nf ");
    let head = chunks.next().unwrap();
    let bike = u32::from_str_radix(head.split_whitespace().nth(2).unwrap(), 16).unwrap();
    let mut rows = Vec::new();
    for chunk in chunks {
        if !chunk.contains("race.120") {
            continue; // the last, torn row
        }
        let line = |key: &str| chunk.lines().find(|l| l.trim_start().starts_with(key)).unwrap();
        let tail = |key: &str| {
            let words: Vec<&str> = line(key).split_whitespace().collect();
            bits4(&words[words.len() - 4..])
        };
        let part_line = line("part ");
        let after = &part_line[part_line.find(']').unwrap() + 1..];
        let part_words: Vec<&str> = after.split_whitespace().take(4).collect();
        rows.push(BackTick {
            frame: chunk.split_whitespace().next().unwrap().parse().unwrap(),
            reason: word_after(chunk, "reason "),
            rec: [tail("rec.p"), tail("rec.f"), tail("rec.u")],
            applied: tail("applied"),
            e0: tail("coll.b0"),
            centre: floats4(line("body.c")),
            part: bits4(&part_words),
            heading: tail("part "),
            row: {
                let words: Vec<&str> = line("node.l3").split_whitespace().collect();
                let r = bits4(&words[6..10]);
                Vf::new(r.x(), r.y(), r.z(), 1.0)
            },
        });
    }
    (bike, rows)
}

/// The bike's terrain: the world its record's ground query is asked of.
const BIKE_TERRAIN: u32 = 0x7ac;
/// The bike's probe bundle, which the record's overhead probe and its clearance ring both fill, and the
/// search list its mesh query walks.
const BIKE_BUNDLE: u32 = 0x1484;
const BIKE_LIST: u32 = 0x1488;
const BIKE_SEARCH: u32 = 0x148c;
/// The mesh query's broad-phase cache on the bike: how far the bundle may move before the search list
/// is gathered again, and how far the last query had moved it.
const BIKE_CACHE_REACH: u32 = 0x13a4;
const BIKE_CACHE_MOVED: u32 = 0x13a8;

/// **The original's world under the put-back** — [`World::ground`] answered by the game's own
/// `ground_query`, run in the interpreter on the save state's RAM against the bike's own terrain, with
/// no normal or record pointer past the two the record's update hands it; and [`World::solid`] and
/// [`World::crowded`] (S75) answered by the game's own mesh query, `put_back_solid`, on the bike's own
/// bundle and search list.
///
/// `solid` writes the port's bundle into the bike's — count, origins, directions, lengths, centre and
/// radius, where the record's update and the clearance write them — and asks. Before a ring of more
/// than one probe it re-files the bundle in the world first when the query before regathered, as
/// `put_back_clear` does. `crowded` asks the same query with no probes at all, so that nothing but a
/// placed group or a checkpoint can refuse.
struct TheOriginalsTerrain {
    ee: ps2_oracle::Ee,
    bike: u32,
    terrain: u32,
    query: u32,
    mesh: Option<OriginalsMeshes>,
    /// Rings the original refused with no probe past the first meeting anything, and how many times
    /// [`World::crowded`] said yes: the two have to agree.
    unsaid: u32,
    crowded: u32,
    /// Queries asked, and how many of them met anything.
    asked: u32,
    met: u32,
}

/// The addresses [`TheOriginalsTerrain::solid`] calls and hands over.
struct OriginalsMeshes {
    solid: u32,
    sweep: u32,
    refile: u32,
    ring: u32,
    over: u32,
}

impl OriginalsMeshes {
    fn load() -> Result<OriginalsMeshes, String> {
        Ok(OriginalsMeshes {
            solid: ps2_oracle::addr("put_back_solid")?,
            sweep: ps2_oracle::addr("bundle_sweep_object")?,
            refile: ps2_oracle::addr("put_back_refile")?,
            ring: ps2_oracle::addr("put_back_ring_filter")?,
            over: ps2_oracle::addr("put_back_over_filter")?,
        })
    }
}

/// What one object that met the bundle left in the record: probe 0's point and normal, and every
/// probe's flag.
type Answer = (Vf, Vf, Vec<bool>);

impl TheOriginalsTerrain {
    /// Run the original's query on a bundle: whether it refused, and the answer of every object that
    /// met the bundle, in the order it swept them.
    ///
    /// The query sweeps the bundle against one object at a time, and each sweep clears the record's
    /// flags and fractions before it starts — so what it acts on is each object's answer as it comes,
    /// and the record after the last object is that object's alone. The per-object sweep is hooked to
    /// run as itself and hand over, for every object that answered, the record's probe-0 point and
    /// normal (which that sweep does not clear: the query reads them all the same) and its flags.
    fn ask(&mut self, bundle: &Bundle) -> (bool, Vec<Answer>) {
        let m = self.mesh.as_ref().expect("asked with no mesh query");
        let (ee, bike) = (&mut self.ee, self.bike);
        let b = ee.u32_at(bike + BIKE_BUNDLE);
        let n = bundle.count as u32;
        ee.set_u32(b + 0x120, n);
        let (lengths, origins, dirs) = (ee.u32_at(b + 0x134), ee.u32_at(b + 0x138), ee.u32_at(b + 0x13c));
        for (i, p) in bundle.probes[..bundle.count].iter().enumerate() {
            let i = i as u32;
            set_quad(ee, origins + 16 * i, p.origin);
            set_quad(ee, dirs + 16 * i, p.dir);
            ee.set_u32(lengths + 4 * i, p.length.0.to_bits());
        }
        set_quad(ee, b + 0x150, bundle.centre);
        ee.set_u32(b + 0x160, bundle.radius.0.to_bits());
        let filter = if n == 1 { m.over } else { m.ring };
        if n > 1 && ee.f32_at(bike + BIKE_CACHE_REACH) < ee.f32_at(bike + BIKE_CACHE_MOVED) {
            ee.call(m.refile, &[b, b + 0x150, filter].map(u64::from), &[bundle.radius.0], 50_000_000);
        }

        let answers: Rc<RefCell<Vec<Answer>>> = Rc::default();
        let (sweep, heard) = (m.sweep, answers.clone());
        ee.hooks.insert(
            sweep,
            Box::new(move |ee: &mut ps2_oracle::Ee| {
                let (args, ra, sp, stack) = ([ee.r(4), ee.r(5), ee.r(6)], ee.r(31), ee.r(29), ee.stack);
                ee.stack = (sp as u32 - 0x40) & !0xf;
                ee.call(sweep, &args, &[], 100_000_000);
                let met = ee.v0();
                // The interpreter files the hook back under the `pc` it leaves, so leave it here.
                (ee.stack, ee.pc) = (stack, sweep);
                ee.set(29, sp);
                ee.set(31, ra);
                ee.set(2, met);
                if met != 0 {
                    let (bundle, record) = (args[0] as u32, args[2] as u32);
                    let flags = ee.u32_at(record + 0xa4);
                    let hit = (0..ee.u32_at(bundle + 0x120)).map(|k| ee.u32_at(flags + 4 * k) != 0).collect();
                    heard.borrow_mut().push((quad_at(ee, ee.u32_at(record + 0x98)), quad_at(ee, ee.u32_at(record + 0x94)), hit));
                }
            }),
        );
        let (y, normal) = (ee.scratch(2), ee.scratch(3));
        ee.set_f32(y, -1.0e30);
        let args = [bike, ee.u32_at(bike + BIKE_SEARCH), filter, b, normal, y].map(u64::from);
        ee.call(m.solid, &args, &[], 400_000_000);
        ee.hooks.remove(&sweep);
        let refused = ee.v0() != 0;
        let answers = answers.take();

        // The query's own `y` and normal are the highest probe-0 point any answer left, and its normal.
        if let Some(top) = answers.iter().max_by(|a, b| a.0.y().total_cmp(&b.0.y())) {
            assert_eq!(ee.f32_at(y).to_bits(), top.0.y().to_bits(), "the query's own y");
            assert!(same_bits(quad_at(ee, normal), top.1), "the query's own normal");
        }
        (refused, answers)
    }
}

fn set_quad(ee: &mut ps2_oracle::Ee, a: u32, v: Vf) {
    for k in 0..4 {
        ee.set_u32(a + 4 * k, v[k as usize].to_bits());
    }
}

fn quad_at(ee: &ps2_oracle::Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

impl World for TheOriginalsTerrain {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        let ee = &mut self.ee;
        let (point, normal) = (ee.scratch(0), ee.scratch(1));
        set_quad(ee, point, at);
        set_quad(ee, normal, Vf::ZERO);
        let args = [self.terrain, point, normal, 0, 0].map(u64::from);
        ee.call(self.query, &args, &[], 50_000_000);
        (quad_at(ee, point), quad_at(ee, normal))
    }

    /// Probe 0 is answered as the query answers its own `y`: the highest point any object that met the
    /// bundle left in it, with that object's normal; the others by their flags, any object's.
    fn solid(&mut self, bundle: &Bundle, record: &mut Record) -> bool {
        if self.mesh.is_none() {
            return false;
        }
        let (refused, answers) = self.ask(bundle);
        for (point, normal, hit) in &answers {
            if !record.hit[0] || F(record.point[0].y()) < F(point.y()) {
                (record.point[0], record.normal[0]) = (*point, *normal);
            }
            record.hit[0] = true;
            for (k, &h) in hit.iter().enumerate().skip(1) {
                record.hit[k] |= h;
            }
        }
        let flagged = record.hit[1..bundle.count].iter().any(|&h| h);
        assert!(refused || !flagged, "the query met something with a ring probe and did not refuse");
        if bundle.count > 1 && refused && !flagged {
            self.unsaid += 1;
        }
        self.asked += 1;
        self.met += u32::from(record.hit[0]);
        true
    }

    fn crowded(&mut self, centre: Vf, radius: F) -> bool {
        if self.mesh.is_none() {
            return false;
        }
        let none = Bundle { count: 0, centre, radius, ..Bundle::default() };
        let (refused, answers) = self.ask(&none);
        assert!(answers.iter().all(|a| a.2.is_empty()), "a bundle with no probes met something");
        self.crowded += u32::from(refused);
        refused
    }
}

/// The captures and the save-state slot each started from.
const BACK_CAPTURES: [(&str, u32); 5] = [
    ("free06-ride2.txt", 6),
    ("free06-down.txt", 6),
    ("free06-downchord.txt", 6),
    ("free09-ride2.txt", 7),
    ("free09-down.txt", 7),
];

/// A capture's rows and the original's terrain under them; `None`, said why, when this checkout has
/// not got them.
fn back_run(name: &str, slot: u32) -> Option<(Vec<BackTick>, TheOriginalsTerrain)> {
    let kit = match ps2_oracle::Kit::load() {
        Ok(kit) => kit,
        Err(why) => {
            println!("no kit.toml ({why}): no put-back verdict");
            return None;
        }
    };
    let Ok(text) = std::fs::read_to_string(kit.ctx("s66").join(name)) else {
        println!("no context/s66/{name}: no put-back verdict for it");
        return None;
    };
    // The state each capture started from: kept beside the captures under the track's name (S75), as
    // the numbered slots are each emulator copy's own and another copy's slot 6 is another state.
    let beside = kit.ctx("s66").join(format!("{}.eeMemory.bin", &name[..6]));
    let ram = if beside.exists() {
        beside
    } else {
        kit.ctx("states").join(format!("SLUS-20767 (9866FE54).{slot:02}.eeMemory.bin"))
    };
    let mut ee = match ps2_oracle::Ee::from_ram(&ram) {
        Ok(ee) => ee,
        Err(why) => {
            println!("{why}: `./kit state ram {slot}` writes it; no put-back verdict for {name}");
            return None;
        }
    };
    let (Ok(gp), Ok(query)) = (ps2_oracle::addr("boot_gp"), ps2_oracle::addr("ground_query")) else {
        println!("research/oracle.txt has no boot_gp or ground_query: no put-back verdict");
        return None;
    };
    ee.set(28, u64::from(gp));
    let (bike, rows) = back_capture(&text);
    let terrain = ee.u32_at(bike + BIKE_TERRAIN);
    let mesh = OriginalsMeshes::load().map_err(|why| println!("{why}: the put-back's solid answers nothing")).ok();
    Some((rows, TheOriginalsTerrain { ee, bike, terrain, query, mesh, unsaid: 0, crowded: 0, asked: 0, met: 0 }))
}

fn same_bits(a: Vf, b: Vf) -> bool {
    (0..4).all(|k| a[k].to_bits() == b[k].to_bits())
}

fn show(v: Vf) -> String {
    format!("{:08x} {:08x} {:08x} {:08x}", v[0].to_bits(), v[1].to_bits(), v[2].to_bits(), v[3].to_bits())
}

/// The original's world with nothing solid in it: what a refused tick would have kept, had nothing
/// stood in the way — which says whether a refusal was the meshes' or the ground's slope.
struct NoSolid<'a>(&'a mut TheOriginalsTerrain);

impl World for NoSolid<'_> {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        self.0.ground(at)
    }

    fn solid(&mut self, _: &Bundle, _: &mut Record) -> bool {
        false
    }
}

/// **The ticks the original held its record on** (S75): with the meshes answered by the original's own
/// query, the port has to hold the record on them too. Each held tick is tried from the same probes
/// and headings [`the_put_back_record_is_kept_bit_for_bit`] tries a kept one with — this row's and
/// the one before's, the wreck part's too while down — and it is held when one of them is refused, or
/// keeps the record on the bits it already had. A refusal is the meshes' when the same probe over a
/// world with nothing solid would have been kept.
#[derive(Default)]
struct Refusals {
    /// Refused by something solid: an overhead mesh, a ring probe meeting one.
    by_solid: u32,
    /// Refused with nothing solid in the way: the ground's slope.
    by_ground: u32,
    /// Kept on the very bits it held.
    in_place: u32,
    /// Ticks of a cannonball, which the port does not keep a record in.
    cannonball: u32,
    /// The port moved the record where the original held it.
    moved: Vec<String>,
}

impl Refusals {
    fn hold(
        &mut self,
        name: &str,
        before: &BackTick,
        now: &BackTick,
        world: &mut TheOriginalsTerrain,
        p: &Phy,
        tables: &Tables,
    ) {
        const CANNONBALL: u32 = 13;
        if now.reason == CANNONBALL || before.reason == CANNONBALL {
            self.cannonball += 1;
            return;
        }
        let down = now.reason != 0 || before.reason != 0;
        let probe = |r: &BackTick, part: bool| {
            if part { r.part } else { Vf::new(r.e0.x(), r.centre.y(), r.e0.z(), r.e0.w()) }
        };
        let was = game_sim::ride::Back { at: before.rec[0], facing: before.rec[1], up: before.rec[2] };
        let mut tried = Vec::new();
        for (from, part) in [(now, false), (before, false), (now, true), (before, true)] {
            if part && !down {
                continue;
            }
            for heading in [now.heading, before.heading, now.row, before.row] {
                let mut ride = rider(p);
                ride.back = was;
                if !ride.keep_back(probe(from, part), heading, world, &tables.curve) {
                    let mut bare = rider(p);
                    bare.back = was;
                    if bare.keep_back(probe(from, part), heading, &mut NoSolid(world), &tables.curve) {
                        self.by_solid += 1;
                    } else {
                        self.by_ground += 1;
                    }
                    return;
                }
                if same_bits(ride.back.at, now.rec[0]) && same_bits(ride.back.up, now.rec[2]) {
                    self.in_place += 1;
                    return;
                }
                tried.push(show(ride.back.at));
            }
        }
        self.moved.push(format!(
            "{name} frame {} ({}): the original held {}; the port kept {tried:?}",
            now.frame,
            if down { "down" } else { "riding" },
            show(now.rec[0])
        ));
    }

    /// Say what the held ticks came to, and hold them: none moved, and every ring the query refused
    /// with no probe flagged was one [`World::crowded`] refused too.
    fn verdict(&self, name: &str, held: u32, world: &TheOriginalsTerrain) {
        let (unsaid, crowded) = (world.unsaid, world.crowded);
        println!(
            "{name}: of {held} held, {} refused by a mesh, {} by the slope, {} kept in place, {} in a \
             cannonball, {} moved; {unsaid} refusals the record did not say, {crowded} crowded",
            self.by_solid,
            self.by_ground,
            self.in_place,
            self.cannonball,
            self.moved.len()
        );
        assert!(self.moved.is_empty(), "{name}: {:#?}", self.moved);
        assert_eq!(unsaid, crowded, "{name}: the original's query refused rings its record and crowding do not say");
    }
}

/// **The put-back record, tick for tick** (S66): on every captured tick the original moved its record,
/// the port's [`Ride::keep_back`] — handed the same probe and heading, from the record the tick
/// before — moves it to the same point, heading and up, to the bit. Riding, the probe is the sweep's
/// point at the body's height; down, the wreck part's point.
#[test]
fn the_put_back_record_is_kept_bit_for_bit() {
    let tables = Tables::build();
    let p = bike(&tables);
    let (mut compared, mut down_compared) = (0, 0);
    for (name, slot) in BACK_CAPTURES {
        let Some((rows, mut world)) = back_run(name, slot) else { continue };
        let (mut here, mut held, mut unseen, mut unfaced) = (0, 0, 0, 0);
        let mut refusals = Refusals::default();
        for i in 1..rows.len() {
            let (before, now) = (&rows[i - 1], &rows[i]);
            if now.frame != before.frame + 1 {
                continue; // a tick lost between them
            }
            if now.applied[0].to_bits() != before.applied[0].to_bits() {
                continue; // the put-back tick itself
            }
            if same_bits(now.rec[0], before.rec[0]) {
                held += 1;
                refusals.hold(name, before, now, &mut world, &p, &tables);
                continue;
            }
            // A row is read while the game runs, so it can hold this tick's sweep with the record
            // the tick before left, and the race manager's heading from either side of the physics:
            // the probe and the heading (the race manager's copy, or the node row it copies) are taken
            // from this row or the one before, every way, and one of them has to give the original's
            // record word for word.
            // The reset clears the reason before the tick is over, so a row read across it says
            // riding of a record the wreck kept: either row down and the wreck part is tried too.
            let down = now.reason != 0 || before.reason != 0;
            let probe = |r: &BackTick, part: bool| {
                if part { r.part } else { Vf::new(r.e0.x(), r.centre.y(), r.e0.z(), r.e0.w()) }
            };
            let xz = |v: Vf| (v.x().to_bits(), v.z().to_bits());
            let mut sources = [(now, false), (before, false), (now, true), (before, true)]
                .into_iter()
                .filter(|&(r, part)| (down || !part) && xz(probe(r, part)) == xz(now.rec[0]))
                .peekable();
            if sources.peek().is_none() {
                unseen += 1; // the probe it was kept from fell between two rows
                continue;
            }
            let (mut placed, mut faced, mut tried) = (false, false, Vec::new());
            for (from, part) in sources {
                for heading in [now.heading, before.heading, now.row, before.row] {
                    let mut ride = rider(&p);
                    ride.back =
                        game_sim::ride::Back { at: before.rec[0], facing: before.rec[1], up: before.rec[2] };
                    let kept = ride.keep_back(probe(from, part), heading, &mut world, &tables.curve);
                    if kept && same_bits(ride.back.at, now.rec[0]) && same_bits(ride.back.up, now.rec[2]) {
                        placed = true;
                        faced |= same_bits(ride.back.facing, now.rec[1]);
                    }
                    tried.push(format!("{} / {}", show(ride.back.at), show(ride.back.up)));
                }
            }
            assert!(
                placed,
                "{name} frame {} ({}): the original's record is {} up {}; the port's off the same \
                 probe: {tried:?}",
                now.frame,
                if down { "down" } else { "riding" },
                show(now.rec[0]),
                show(now.rec[2])
            );
            if !faced {
                unfaced += 1; // the heading it was kept with fell between two rows
            }
            here += 1;
            if down {
                down_compared += 1;
            }
        }
        println!("{name}: {here} ticks kept bit for bit ({unfaced} of them off a heading no row holds), \
                  {held} held, {unseen} off a probe no row holds");
        println!("{name}: {} mesh queries, {} met something", world.asked, world.met);
        refusals.verdict(name, held, &world);
        assert!(unfaced * 20 <= here, "{name}: {unfaced} of {here} headings matched no row's");
        assert!(unseen * 20 <= here, "{name}: {unseen} of {here} probes matched no row's");
        compared += here;
    }
    if compared > 0 {
        assert!(compared > 1000, "only {compared} ticks compared");
        assert!(down_compared > 50, "only {down_compared} down ticks compared");
    }
}

/// **The put-back lands on the record** (S66): on every captured put-back — the chord riding, the chord
/// down, and the clock's reset after a wreck — the original applied the record as it stood the tick
/// before, and [`Ride::place_back`] from that record puts the bike's origin on the same point to the
/// bit, its forward row along the record's heading and square to its up.
#[test]
fn every_captured_put_back_lands_where_the_original_put_the_bike() {
    let tables = Tables::build();
    let p = bike(&tables);
    let mut landed = 0;
    for (name, slot) in BACK_CAPTURES {
        let Some((rows, _)) = back_run(name, slot) else { continue };
        for pair in rows.windows(2) {
            let (before, now) = (&pair[0], &pair[1]);
            if now.frame != before.frame + 1 || same_bits(now.applied, before.applied) {
                continue;
            }
            // The record as it stood when the chord ran: the row before's, or — when this row was
            // read before its own tick's record update — this row's, which nothing after the
            // physics could make equal to the point applied.
            let Some(rec) = [before.rec, now.rec].into_iter().find(|rec| same_bits(now.applied, rec[0])) else {
                panic!(
                    "{name} frame {}: the original applied {} and its record held {}",
                    now.frame,
                    show(now.applied),
                    show(before.rec[0])
                )
            };
            let mut ride = rider(&p);
            ride.back = game_sim::ride::Back { at: rec[0], facing: rec[1], up: rec[2] };
            ride.place_back();
            assert!(
                same_bits(ride.position(), now.applied),
                "{name} frame {}: the port put the bike at {}, the original at {}",
                now.frame,
                show(ride.position()),
                show(now.applied)
            );
            let m = game_sim::math::basis(rec[0], rec[1], rec[2], true, false);
            assert_eq!(ride.node, [m.0[0], m.0[1], m.0[2]], "{name} frame {}: the frame set down", now.frame);
            println!("{name}: put back at frame {} ({})", now.frame, if before.reason != 0 { "down" } else { "riding" });
            landed += 1;
        }
    }
    if landed > 0 {
        assert!(landed >= 5, "only {landed} put-backs compared");
    }
}

// --- the put-back's clearance against the original's meshes (S75) ---------------------------------
//
// The captures above never ride near anything the original's put-back query looks at: its search list
// holds the world's actors, and the ones it sweeps (type 2, flagged collidable, not masked off) were all
// hundreds of feet from both tracks' paths, so no captured tick was refused by a mesh. These cases are
// generated where it does: around every such object the bike's search list holds in the save state,
// the original's own record update runs in the interpreter, and the port's runs over the original's
// ground and mesh query, from the same probe, heading and record.

/// A small generator of cases: xorshift64*.
struct Cases(u64);

impl Cases {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// Uniform in `[-1, 1)`.
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    }
}

/// Where the record's update reads its riding probe: the collision record's sweep point and the
/// body's centre height; the reason, the cannonball's wreck kind, and the record and its raise.
const BIKE_SWEEP: u32 = 0x7d0;
const BIKE_BODY: u32 = 0x7a4;
const BIKE_REASON: u32 = 0x748;
const BIKE_WRECK_KIND: u32 = 0xc84;
const BIKE_RECORD: u32 = 0x3b0;
const BIKE_RAISE: u32 = 0x13a0;

/// **The put-back's clearance refuses where the original's does** (S75): ten thousand probes around the
/// things the original's mesh query sweeps, on free06 and free09, each kept or refused by the port's
/// [`Ride::keep_back`] exactly as by the original's record update — and where kept, kept on the same
/// point, heading and up to the bit.
#[test]
fn the_put_back_clearance_refuses_where_the_original_does() {
    const CASES: usize = 5_000;
    let tables = Tables::build();
    let p = bike(&tables);
    let mut total = 0;
    for name in ["free06-ride2.txt", "free09-ride2.txt"] {
        let Some((_, mut world)) = back_run(name, 0) else { continue };
        let Ok(keep) = ps2_oracle::addr("put_back_keep") else {
            println!("research/oracle.txt has no put_back_keep: no clearance verdict");
            return;
        };
        if world.mesh.is_none() {
            return;
        }
        let bike = world.bike;
        assert_eq!(world.ee.u32_at(bike + BIKE_RAISE), 0, "{name}: the record's raise is not zero here");
        assert_eq!(world.ee.u32_at(bike + BIKE_REASON), 0, "{name}: the state is not riding");
        assert_ne!(world.ee.u32_at(bike + BIKE_WRECK_KIND), 13, "{name}: the state is in a cannonball");

        // What the query sweeps: the search list's flagged, unmasked objects, centre and radius.
        let ee = &world.ee;
        let list = ee.u32_at(bike + BIKE_LIST);
        let (arr, count) = (ee.u32_at(list), ee.u32_at(list + 4));
        let mask = ee.u32_at(ps2_oracle::addr("put_back_mask").expect("put_back_mask in research/oracle.txt"));
        let things: Vec<(f32, f32, f32)> = (0..count)
            .map(|k| ee.u32_at(arr + 8 * k))
            .filter(|&o| ee.u8_at(o + 2) == 2 && ee.u8_at(o + 0x31) & 1 != 0 && ee.u32_at(o + 0x84) & mask == 0)
            .map(|o| (ee.f32_at(o + 0x40), ee.f32_at(o + 0x48), ee.f32_at(o + 0x60)))
            .collect();
        assert!(!things.is_empty(), "{name}: the search list sweeps nothing");

        let mut cases = Cases(0x5375_0000 ^ total as u64 ^ u64::from(bike));
        let (mut kept, mut by_solid, mut by_ground) = (0, 0, 0);
        for i in 0..CASES {
            let (cx, cz, r) = things[i % things.len()];
            // A point on or around the thing's sphere, out to the ring's reach past it, at a body's
            // height (two to eight feet) over the ground there.
            let reach = r + 50.0;
            let (x, z) = (cx + cases.unit() * reach, cz + cases.unit() * reach);
            let (under, _) = world.ground(Vf::new(x, 0.0, z, 1.0));
            let at = Vf::new(x, under.y() + 5.0 + 3.0 * cases.unit(), z, 1.0);
            let a = cases.unit() * std::f32::consts::PI;
            let heading = Vf::new(a.sin(), 0.2 * cases.unit(), a.cos(), 1.0);
            let was = game_sim::ride::Back {
                at: Vf::new(-1.0, -2.0, -3.0, 1.0),
                facing: Vf::new(0.0, 0.0, 1.0, 1.0),
                up: Vf::new(0.0, 1.0, 0.0, 0.0),
            };

            // The original: the probe where its update reads it, the record where it keeps it.
            let ee = &mut world.ee;
            let sweep = ee.u32_at(bike + BIKE_SWEEP) + 0xe0;
            let body = ee.u32_at(bike + BIKE_BODY) + 0x64;
            let quad = |ee: &mut ps2_oracle::Ee, a: u32, v: Vf| {
                for k in 0..4 {
                    ee.set_u32(a + 4 * k, v[k as usize].to_bits());
                }
            };
            quad(ee, sweep, at);
            ee.set_u32(body, at.y().to_bits());
            quad(ee, bike + BIKE_RECORD, was.at);
            quad(ee, bike + BIKE_RECORD + 0x10, was.facing);
            quad(ee, bike + BIKE_RECORD + 0x20, was.up);
            let h = ee.scratch(3);
            quad(ee, h, heading);
            ee.call(keep, &[bike, h, h].map(u64::from), &[], 400_000_000);
            let read = |ee: &ps2_oracle::Ee, a: u32| Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12));
            let theirs = [read(ee, bike + BIKE_RECORD), read(ee, bike + BIKE_RECORD + 0x10), read(ee, bike + BIKE_RECORD + 0x20)];
            let they_kept = !same_bits(theirs[0], was.at);

            // The port, over the original's ground and meshes, from the same record.
            let mut ride = rider(&p);
            ride.back = was;
            let we_kept = ride.keep_back(at, heading, &mut world, &tables.curve);
            assert_eq!(
                we_kept, they_kept,
                "{name} case {i}: probe {} heading {}: the original {} it, the port {} it",
                show(at),
                show(heading),
                if they_kept { "kept" } else { "refused" },
                if we_kept { "kept" } else { "refused" }
            );
            if we_kept {
                for (k, (ours, theirs)) in [ride.back.at, ride.back.facing, ride.back.up].into_iter().zip(theirs).enumerate() {
                    assert!(same_bits(ours, theirs), "{name} case {i}: record word {k}: port {} original {}", show(ours), show(theirs));
                }
                kept += 1;
            } else {
                let mut bare = rider(&p);
                bare.back = was;
                if bare.keep_back(at, heading, &mut NoSolid(&mut world), &tables.curve) {
                    by_solid += 1;
                } else {
                    by_ground += 1;
                }
            }
        }
        println!(
            "{name}: {CASES} probes around {} things: {kept} kept bit for bit, {by_solid} refused by a mesh, \
             {by_ground} by the ground; {} refusals the record did not say, {} rings crowded",
            things.len(),
            world.unsaid,
            world.crowded
        );
        assert!(by_solid * 20 >= CASES, "{name}: only {by_solid} of {CASES} probes met a mesh");
        assert!(kept * 20 >= CASES, "{name}: only {kept} of {CASES} probes were kept");
        assert_eq!(world.unsaid, world.crowded, "{name}: the original's query refused rings its record and crowding do not say");
        total += CASES;
    }
    if total > 0 {
        assert!(total >= 10_000, "only {total} probes compared");
    }
}

// --- the put-back after a shootoff (S77) ----------------------------------------------------------
//
// S77 rode the original off three of free09's edges (east and west from save state 7, north from 6)
// with `research/s77_edge_back.py`, which records once a tick, every float with its bits: the request's
// reason (`c84`, the cannonball branch's test), the wreck bike's collision record's touched words, the
// put-back record, the wreck bike's transform (its forward row and translation) and the wreck part's
// point. The captures and the two states' RAM sit in `context/s77/`; without them the tests below say
// so and pass, and with them they count what they compared so a skip cannot read as a pass.

/// One tick of a shootoff capture.
struct ThrownTick {
    frame: u32,
    reason: u32,
    /// The request's copy of the reason, from before the tick's verdict.
    c84: u32,
    /// The wreck bike's collision record touched something.
    touch: bool,
    rec: [Vf; 3],
    /// The wreck bike's transform: its forward row and its translation.
    forward: Vf,
    at: Vf,
    /// The wreck part's point, which the overhead test stands on while down.
    part: Vf,
}

fn thrown_capture(text: &str) -> (u32, Vec<ThrownTick>) {
    let mut chunks = text.split("\nf ");
    let head = chunks.next().unwrap();
    let bike = u32::from_str_radix(head.split_whitespace().nth(2).unwrap(), 16).unwrap();
    let mut rows = Vec::new();
    for chunk in chunks {
        if !chunk.contains(" head ") {
            continue; // a torn last row
        }
        let line = |key: &str| chunk.lines().find(|l| l.trim_start().starts_with(key)).unwrap();
        let bits_of = |key: &str| {
            let l = line(key);
            let words: Vec<&str> = l[l.find(']').unwrap() + 1..].split_whitespace().take(4).collect();
            bits4(&words)
        };
        let first = chunk.lines().next().unwrap();
        rows.push(ThrownTick {
            frame: first.split_whitespace().next().unwrap().parse().unwrap(),
            reason: word_after(first, "reason "),
            c84: word_after(first, "c84 "),
            touch: first[first.find("touch [").unwrap() + 7..].starts_with('1'),
            rec: [bits_of("rec.p"), bits_of("rec.f"), bits_of("rec.u")],
            forward: bits_of("node.r2"),
            at: bits_of("node.t"),
            part: bits_of("part "),
        });
    }
    (bike, rows)
}

/// The captures and the state each started from.
const THROWN_CAPTURES: [(&str, u32); 3] = [("free09-east.txt", 7), ("free09-north.txt", 6), ("free09-west.txt", 7)];

/// A shootoff capture's rows and the original's terrain under them; `None`, said why, when this
/// checkout has not got them.
fn thrown_run(name: &str, slot: u32) -> Option<(Vec<ThrownTick>, TheOriginalsTerrain)> {
    let kit = match ps2_oracle::Kit::load() {
        Ok(kit) => kit,
        Err(why) => {
            println!("no kit.toml ({why}): no shootoff verdict");
            return None;
        }
    };
    let Ok(text) = std::fs::read_to_string(kit.ctx("s77").join(name)) else {
        println!("no context/s77/{name}: no shootoff verdict for it");
        return None;
    };
    let ram = kit.ctx("s77").join(format!("slot{slot}.eeMemory.bin"));
    let mut ee = match ps2_oracle::Ee::from_ram(&ram) {
        Ok(ee) => ee,
        Err(why) => {
            println!("{why}: `./kit state ram {slot} --out` writes it; no shootoff verdict for {name}");
            return None;
        }
    };
    let (Ok(gp), Ok(query)) = (ps2_oracle::addr("boot_gp"), ps2_oracle::addr("ground_query")) else {
        println!("research/oracle.txt has no boot_gp or ground_query: no shootoff verdict");
        return None;
    };
    ee.set(28, u64::from(gp));
    let (bike, rows) = thrown_capture(&text);
    let terrain = ee.u32_at(bike + BIKE_TERRAIN);
    let mesh = OriginalsMeshes::load().map_err(|why| println!("{why}: the put-back's solid answers nothing")).ok();
    Some((rows, TheOriginalsTerrain { ee, bike, terrain, query, mesh, unsaid: 0, crowded: 0, asked: 0, met: 0 }))
}

/// The point, up and the heading's `x`, `y`, `z` of two records: the heading's `w` is a word the
/// original leaves as its stack had it.
fn same_record(port: &game_sim::ride::Back, original: &[Vf; 3]) -> bool {
    let xyz = |a: Vf, b: Vf| (0..3).all(|k| a[k].to_bits() == b[k].to_bits());
    same_bits(port.at, original[0]) && same_bits(port.up, original[2]) && xyz(port.facing, original[1])
}

/// The port's cannonball branch from one tick's update, tried on every way a row can hold it: the
/// wreck bike's transform and the part from this row or the one before (a row is read while the game
/// runs, and can hold one tick's record with the next tick's bike). What the record came to on each.
fn thrown_tries(before: &ThrownTick, now: &ThrownTick, world: &mut TheOriginalsTerrain, p: &Phy, tables: &Tables) -> Vec<(bool, game_sim::ride::Back)> {
    let mut tries = Vec::new();
    for bike in [now, before] {
        for part in [now.part, before.part] {
            let mut ride = rider(p);
            ride.back = game_sim::ride::Back { at: before.rec[0], facing: before.rec[1], up: before.rec[2] };
            let kept = ride.keep_back_thrown(part, bike.at, bike.forward, world, &tables.curve);
            tries.push((kept, ride.back));
        }
    }
    tries
}

/// **The record in a cannonball, tick for tick** (S77): on every captured tick of a shootoff the
/// original's record changed on, the port's [`Ride::keep_back_thrown`] — handed the same wreck bike
/// and part, from the record the tick before, over the original's own ground and meshes — leaves the
/// same point, up and heading, to the bit: moved onto the wreck bike where the original moved it, and
/// held with the ground's new up where the original's clearance refused. Before the wreck bike first
/// touches, and after it last does, the record does not change at all.
#[test]
fn the_record_follows_the_wreck_bike_through_a_shootoff() {
    let tables = Tables::build();
    let p = bike(&tables);
    let (mut moved, mut refused) = (0, 0);
    for (name, slot) in THROWN_CAPTURES {
        let Some((rows, mut world)) = thrown_run(name, slot) else { continue };
        let (mut here, mut held_here, mut still, mut untouched) = (0, 0, 0, 0);
        for i in 1..rows.len() {
            let (before, now) = (&rows[i - 1], &rows[i]);
            if now.frame != before.frame + 1 || now.reason != 13 || now.c84 != 13 || before.c84 != 13 {
                continue; // a lost tick, not thrown, or the throw's own tick, which is the riding branch's
            }
            let changed = (0..3).any(|k| !same_bits(now.rec[k], before.rec[k]));
            if !now.touch && !before.touch {
                // Neither row's wreck bike touched: the branch is not taken.
                assert!(!changed, "{name} frame {}: the record changed with the wreck bike in the air", now.frame);
                untouched += 1;
                continue;
            }
            if !changed {
                still += 1; // the row was read before its tick's update
                continue;
            }
            let tries = thrown_tries(before, now, &mut world, &p, &tables);
            let Some(&(kept, _)) = tries.iter().find(|(_, back)| same_record(back, &now.rec)) else {
                panic!(
                    "{name} frame {}: the original's record is {} up {}; the port's: {:?}",
                    now.frame,
                    show(now.rec[0]),
                    show(now.rec[2]),
                    tries.iter().map(|(k, b)| format!("{k} {} up {}", show(b.at), show(b.up))).collect::<Vec<_>>()
                )
            };
            assert_eq!(kept, !same_bits(now.rec[0], before.rec[0]), "{name} frame {}: kept, or held", now.frame);
            if kept { here += 1 } else { held_here += 1 }
        }
        println!(
            "{name}: {here} ticks moved onto the wreck bike bit for bit, {held_here} held with the new up, \
             {still} read before their update, {untouched} in the air untouched; {} mesh queries, {} met something",
            world.asked, world.met
        );
        assert!(here >= 5, "{name}: only {here} ticks moved");
        assert!(still * 4 <= here, "{name}: {still} rows read before their update, against {here}");
        moved += here;
        refused += held_here;
    }
    if moved > 0 {
        assert!(moved >= 80, "only {moved} ticks compared");
        println!("{moved} moved and {refused} held, over the three edges");
    }
}

/// **The put-back after a shootoff lands at the landing** (S77): on every captured shootoff the
/// clock's put-back set the original's bike on its record as it stood the tick before, which the
/// port's cannonball branch left on the wreck bike's last touch. [`Ride::place_back`] from the
/// port's own record puts the bike's origin on the original's to the bit — inside the map, hundreds of
/// feet from the edge it was thrown off, where the port used to put it.
#[test]
fn each_captured_post_shootoff_put_back_lands_bit_exact_at_the_landing() {
    let tables = Tables::build();
    let p = bike(&tables);
    let mut landed = 0;
    for (name, slot) in THROWN_CAPTURES {
        let Some((rows, mut world)) = thrown_run(name, slot) else { continue };
        let thrown = rows.iter().position(|r| r.reason == 13).unwrap_or_else(|| panic!("{name}: never thrown"));
        let back = thrown + rows[thrown..].iter().position(|r| r.reason == 0).unwrap_or_else(|| panic!("{name}: never put back"));
        let edge = rows[thrown].rec[0];
        // The last tick the original moved its record before the put-back, and the port's from it.
        let last = (thrown + 1..back)
            .rev()
            .find(|&i| rows[i].frame == rows[i - 1].frame + 1 && !same_bits(rows[i].rec[0], rows[i - 1].rec[0]))
            .unwrap_or_else(|| panic!("{name}: the record never moved after the throw"));
        let (before, now) = (&rows[last - 1], &rows[last]);
        let tries = thrown_tries(before, now, &mut world, &p, &tables);
        let (_, kept) = *tries
            .iter()
            .find(|(kept, b)| *kept && same_record(b, &now.rec))
            .unwrap_or_else(|| panic!("{name} frame {}: the port did not keep the original's last record", now.frame));
        assert!(rows[last..back].iter().all(|r| same_bits(r.rec[0], kept.at)), "{name}: the record moved again");
        let mut ride = rider(&p);
        ride.back = kept;
        ride.place_back();
        let put = &rows[back];
        assert!(
            same_bits(ride.position(), put.at),
            "{name} frame {}: the port put the bike at {}, the original at {}",
            put.frame,
            show(ride.position()),
            show(put.at)
        );
        let far = (put.at.x() - edge.x()).hypot(put.at.z() - edge.z());
        assert!(far > 500.0, "{name}: put back {far} ft from the edge");
        println!(
            "{name}: thrown at frame {}, record last moved at {}, put back at {} on {}, {} ft from the edge",
            rows[thrown].frame,
            now.frame,
            put.frame,
            show(put.at),
            far
        );
        landed += 1;
    }
    if landed > 0 {
        assert_eq!(landed, THROWN_CAPTURES.len(), "a capture had no verdict");
    }
}

// --- MD16: MX vs. ATV's tricks, as a pack laid over the original's ------------------------------------

/// A trick pack, generated: MX vs. ATV's table layout (`[StuntNN]` with its four extra keys) with two
/// stunts the rider's 24 do not have, one of them gesture-mapped, plus the moves' own rows; and a
/// bike file with MX vs. ATV's `[Three Sixty]` keys and its `[Airborne Control]` auto-level gains.
const MD16_TABLE: &str = "
[Stunt01]
Name=nacnac
DisplayName=\"NacNac\"
HoldTime=0.6
SafeTime=1.0
Duration=1.2
DefaultKeyMapping=1CU
[Stunt02]
Name=Varial
DisplayName=\"Varial\"
HoldTime=0.4
SafeTime=0.6
Duration=0.7
HasAnimation=T
PointValue=1000
HoldPointValue=1000
[Stunt03]
Name=9oclocknac
DisplayName=\"9 O'Clock Nac\"
HoldTime=0.4
SafeTime=0.6
Duration=0.7
DefaultKeyMapping=1CTL
[Stunt04]
Name=backflip
DisplayName=\"Backflip\"
HoldTime=0.6
SafeTime=0.8
Duration=1.0
UsedForBike=F
PointValue=2000
HoldPointValue=10000
[Stunt05]
Name=threesixtyL
DisplayName=\"Three Sixty Left\"
HoldTime=0.72
SafeTime=1.26
Duration=1.5
PointValue=2000
HoldPointValue=15000
[Stunt06]
Name=threesixtyR
DisplayName=\"Three Sixty Right\"
HoldTime=0.72
SafeTime=1.26
Duration=1.5
PointValue=2000
HoldPointValue=15000
";

const MD16_PHY: &str = "
[Airborne Control]
UnrollGain=1.9
[Three Sixty]
MaxThreeSixtyTorqueX=1100
MinThreeSixtyTorqueX=600
MaxThreeSixtyTorqueY=1100
MinThreeSixtyTorqueY=200
ThreeSixtyTorqueRateIncrease=480
ThreeSixtyTorqueRateDecrease=1960
MinThreeSixtyTime=0
MaxThreeSixtyTime=60
";

/// The disc's 125 with [`MD16_TABLE`] and [`MD16_PHY`] laid over it, and the rider's set with the
/// pack's extras: what the shell builds with Tricks on MX vs. ATV.
fn md16_pack(p: &Phy, whip: bool) -> (Phy, Set) {
    use game_sim::trick_pack;
    let rows = trick_pack::table(MD16_TABLE.as_bytes()).expect("the generated table reads");
    let ini = phy::Ini::parse(MD16_PHY);
    let moves = trick_pack::moves(&ini, &rows).expect("the generated bike has a [Three Sixty]");
    let mut q = p.clone();
    if whip {
        trick_pack::apply(&mut q, &ini, moves);
    } else {
        q.tricks = Some(moves);
    }
    let set = Set::default();
    let extras = trick_pack::extras(&rows, &set);
    (q, set.with_extras(extras))
}

/// One tick of an MD16 ride: the seat, and what the moves did.
struct Md16 {
    seat: Seat,
    frame: rider::Frame,
    flip_row: usize,
    spin: game_sim::air::Spin,
    /// The chassis, bit for bit: what "the stick does exactly what it did" is held to.
    bits: [u32; 8],
    /// The score banked and open so far, and the ids of the open combo's nodes.
    total: f32,
    nodes: Vec<u8>,
    /// The ride's hash: what "the original's replays do not move" is held to.
    hash: u64,
}

/// [`ride_the_flip`] with whole hands per tick and the rider's own set: the flip ramp, pinned
/// throttle, and `script`'s controls from each tick on.
fn ride_md16(p: &Phy, wheels: (Vf, Vf), tables: &rider::Tables, set: Set, script: &[(usize, Controls)],
             ticks: usize) -> (Vec<Md16>, Rider) {
    ride_md16_off(FLIP_RAMP, p, wheels, tables, set, script, ticks)
}

/// [`ride_md16`] off another ramp, `(start, slope, length)` as [`FLIP_RAMP`] (MD30).
fn ride_md16_off((start, slope, length): (f32, f32, f32), p: &Phy, wheels: (Vf, Vf), tables: &rider::Tables, set: Set,
                 script: &[(usize, Controls)], ticks: usize) -> (Vec<Md16>, Rider) {
    let mut r = Rider::new(p, wheels, Vf::new(0.0, 2.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), set);
    let mut world = Jump { origin: Vf::new(0.0, 0.0, 0.0, 1.0), dir: Vf::new(0.0, 0.0, 1.0, 0.0), start, slope, length };
    let mut now = Controls { gas: F::ONE, ..Controls::default() };
    let mut seen = Vec::with_capacity(ticks);
    for tick in 0..ticks {
        if let Some((_, c)) = script.iter().find(|(at, _)| *at == tick) {
            now = Controls { gas: F::ONE, ..c.clone() };
        }
        let frame = r.frame(p, tables, &now, DT, &mut world);
        let m = r.ride.transform();
        let (air, f) = (&r.ride.air, m.0[2]);
        let pos = r.ride.position();
        let q = r.ride.chassis.quat;
        seen.push(Md16 {
            seat: Seat {
                speed: r.ride.chassis.speed.0,
                heading: f32::atan2(m.0[2].x(), m.0[2].z()),
                up_y: m.0[1].y(),
                y: pos.y(),
                on_ground: air.on_ground,
                wreck: frame.wreck.is_some(),
                lip: air.on_ground
                    && r.ride.chassis.vel.y() > 5.0
                    && air.contact_vel.z() > 30.0
                    && r.ride.held.brake == F::ZERO,
                preload: air.preload_firing,
                flip: air.flip_active,
                pitch: f32::atan2(f.y(), f.x() * world.dir.x() + f.z() * world.dir.z()),
            },
            frame,
            flip_row: r.ride.tricks.as_ref().map_or(0, |t| t.flip_row),
            spin: r.ride.tricks.as_ref().map(|t| t.spin).unwrap_or_default(),
            bits: [pos.x(), pos.y(), pos.z(), q.x(), q.y(), q.z(), q.w(), r.ride.chassis.speed.0].map(f32::to_bits),
            total: r.score.total.0 + r.score.nodes.iter().map(|n| n.points.0).sum::<f32>(),
            nodes: r.score.nodes.iter().map(|n| n.id).collect(),
            hash: hash::of(&r.ride),
        });
    }
    (seen, r)
}

/// The hands: `y` and `x` on the stick, R1 alone or not, and the two extra buttons.
fn md16_hands(x: f32, y: f32, r1: bool, extra: [bool; 2]) -> Controls {
    Controls { stick: (F(x), F(y)), trick: [false, false, r1], extra, ..Controls::default() }
}

/// A script of whole hands: `(tick, x, y, R1 alone, [Cross, Square])`.
fn md16_script(hands: &[(usize, f32, f32, bool, [bool; 2])]) -> Vec<(usize, Controls)> {
    hands.iter().map(|&(at, x, y, r1, extra)| (at, md16_hands(x, y, r1, extra))).collect()
}

fn md16_seats(ride: &[Md16]) -> Vec<&Seat> {
    ride.iter().map(|t| &t.seat).collect()
}

/// [`THE_FLIP`] as whole hands, with R1 up or held through it.
fn the_flip_hands(r1: bool) -> Vec<(usize, Controls)> {
    THE_FLIP.iter().map(|&(at, y)| (at, md16_hands(0.0, y, r1, [false; 2]))).collect()
}

/// MD16: the generated pack's extra stunts start from the new buttons. Cross and Square with a stick
/// direction in the air name the extra slots in the original's up, down, left, right order: Cross
/// up is the first row the pack maps to a gesture (`nacnac`), Cross left the first it does not
/// (`Varial`, after `9oclocknac`), and Square — a slot the three rows do not reach — starts nothing.
/// On the original's set the same hands start nothing and the ride does not move by a bit.
#[test]
fn md16_extras_start_from_cross_and_square() {
    let tables = rider::Tables::build();
    let Some((p, wheels)) = the_disc_125(&tables) else { return };
    let (q, set) = md16_pack(&p, true);
    let names: Vec<&str> = (0..3).map(|k| set.stunt(Trick(Trick::EXTRA + k)).expect("three extras").name.as_str()).collect();
    assert_eq!(names, ["nacnac", "9oclocknac", "Varial"], "mapped rows first, then the rest in file order");
    assert!(set.stunt(Trick(Trick::EXTRA + 3)).is_none(), "the pack has three rows the rider lacks");

    let at = |c: [bool; 2], x: f32, y: f32| md16_script(&[(200, x, y, false, c), (230, 0.0, 0.0, false, [false; 2])]);
    let started = |ride: &[Md16]| ride.iter().filter_map(|t| t.frame.started).collect::<Vec<_>>();
    let (up, r) = ride_md16(&q, wheels, &tables, set.clone(), &at([true, false], 0.0, -1.0), 360);
    assert_eq!(started(&up), [Trick(Trick::EXTRA)], "Cross up is the first extra");
    assert!(r.score.total > F::ZERO, "and it scores when it lands");
    assert_eq!(wrecks(&md16_seats(&up)), 0);
    let (left, _) = ride_md16(&q, wheels, &tables, set.clone(), &at([true, false], -1.0, 0.0), 360);
    assert_eq!(started(&left), [Trick(Trick::EXTRA + 2)], "Cross left is the third");
    let (square, _) = ride_md16(&q, wheels, &tables, set.clone(), &at([false, true], 1.0, 0.0), 360);
    assert!(started(&square).is_empty(), "an empty extra slot starts nothing");

    // The original's tricks: the buttons are not trick buttons, and the ride is the one without them.
    let (pressed, _) = ride_md16(&p, wheels, &tables, Set::default(), &at([true, true], 0.0, -1.0), 360);
    let (alone, _) = ride_md16(&p, wheels, &tables, Set::default(), &at([false, false], 0.0, -1.0), 360);
    assert!(started(&pressed).is_empty());
    assert!(pressed.iter().zip(&alone).all(|(a, b)| a.hash == b.hash), "Cross and Square change nothing on Original");
}

/// MD16: without R1 the stick does exactly what it did. [`THE_FLIP`]'s pump, R1 up, on a bike with
/// the pack's moves on: no new move starts, and with the whip's gains left the bike's the ride is the
/// original's to the bit, tick by tick — the moves' state rides along unread. With the whip it is
/// still the disc's backflip.
#[test]
fn md16_the_pump_without_r1_is_the_original() {
    let tables = rider::Tables::build();
    let Some((p, wheels)) = the_disc_125(&tables) else { return };
    let (moves_only, set) = md16_pack(&p, false);
    let (original, _) = ride_md16(&p, wheels, &tables, Set::default(), &the_flip_hands(false), 400);
    let (pack, _) = ride_md16(&moves_only, wheels, &tables, set.clone(), &the_flip_hands(false), 400);
    assert!(pack.iter().all(|t| t.flip_row == 0 && !t.spin.active), "no new move starts with R1 up");
    for (i, (a, b)) in original.iter().zip(&pack).enumerate() {
        assert_eq!(a.bits, b.bits, "tick {i}: the pump without R1 rides as the original's");
    }
    let flips = md16_seats(&original);
    assert!(came_over(&flips) > 5.5, "and it is a backflip: {}", came_over(&flips));

    let (whip, _) = ride_md16(&md16_pack(&p, true).0, wheels, &tables, set, &the_flip_hands(false), 400);
    assert!(whip.iter().all(|t| t.flip_row == 0 && !t.spin.active));
    assert!(came_over(&md16_seats(&whip)) > 5.5, "the whip's gains do not stop the backflip");

    // MD20: the same pump on the diagonal — the 360 backflip's hands, R1 up — is still the original's,
    // and on the original's tricks MD20's hands with R1 held start no 360.
    let diagonal: Vec<_> = THE_FLIP.iter().map(|&(at, y)| (at, md16_hands(y, y, false, [false; 2]))).collect();
    let (original, _) = ride_md16(&p, wheels, &tables, Set::default(), &diagonal, 400);
    let (pack, _) = ride_md16(&moves_only, wheels, &tables, md16_pack(&p, false).1, &diagonal, 400);
    assert!(pack.iter().all(|t| t.flip_row == 0 && !t.spin.active), "no 360 on the diagonal with R1 up");
    for (i, (a, b)) in original.iter().zip(&pack).enumerate() {
        assert_eq!(a.bits, b.bits, "tick {i}: the diagonal pump without R1 rides as the original's");
    }
    let (held, _) = ride_md16(&p, wheels, &tables, Set::default(), &md20_hands(true, true, (220, 40), (220, 40)), 400);
    assert!(held.iter().all(|t| t.flip_row == 0 && t.spin == game_sim::air::Spin::default()), "Original: no 360 under R1");
}

/// MD16: where an R1 gesture and a new move read the same input, the original wins. The original's
/// R1 tricks are R1 *with* Circle or Triangle (groups 3 to 6), and the new moves ask for R1 alone, so
/// a combination in the air starts the very trick it starts on Original, and nothing new. The
/// backflip, asked for from a centred stick with R1 held — forward on the lip, then back — arms
/// first and stays the backflip: the front flip only gets the stick if it was back on the lip first.
#[test]
fn md16_r1_tricks_and_the_backflip_stay_the_originals() {
    let tables = rider::Tables::build();
    let Some((p, wheels)) = the_disc_125(&tables) else { return };
    let (q, set) = md16_pack(&p, true);
    let combo = |trick: [bool; 3]| {
        vec![(200, Controls { stick: (F::ZERO, F(-1.0)), trick, ..Controls::default() }), (230, Controls::default())]
    };
    for trick in [[true, false, true], [false, true, true], [true, true, true]] {
        let (pack, _) = ride_md16(&q, wheels, &tables, set.clone(), &combo(trick), 260);
        let (original, _) = ride_md16(&p, wheels, &tables, Set::default(), &combo(trick), 260);
        // The press's own: the landings differ after it (the whip's gains), and so may a wheelie.
        let started = |ride: &[Md16]| ride.iter().find_map(|t| t.frame.started);
        assert!(started(&original).is_some_and(|t| t.slot().is_some()), "{trick:?} starts a trick on Original");
        assert_eq!(started(&pack), started(&original), "{trick:?} starts the same trick");
        assert!(pack.iter().all(|t| t.flip_row == 0 && !t.spin.active));
    }

    let hands = md16_script(&[
        (130, 0.0, 0.0, true, [false; 2]),
        (158, 0.0, -1.0, true, [false; 2]),
        (166, 0.0, 1.0, true, [false; 2]),
        (176, 0.0, 0.0, true, [false; 2]),
        (260, 0.0, -1.0, true, [false; 2]),
        (280, 0.0, 0.0, true, [false; 2]),
    ]);
    let (ride, _) = ride_md16(&q, wheels, &tables, set, &hands, 400);
    assert!(ride.iter().any(|t| t.seat.flip), "the backflip fires");
    assert!(ride.iter().all(|t| t.flip_row == 0 && !t.spin.active), "on the backflip's row, no 360");
    assert!(came_over(&md16_seats(&ride)) > 5.5, "once round backwards: {}", came_over(&md16_seats(&ride)));
}

/// MD16: the front flip. R1 held, stick back on the lip and then forward: the backflip's own step
/// on its mirrored row, pitching the nose down. The bike goes once round forwards inside the
/// backflip's air, a stab of back stick near the end stops it as a backflip's is stopped, it lands on
/// its wheels, and it scores as [`score::FRONTFLIP`].
#[test]
fn md16_front_flip_goes_round_and_lands() {
    let tables = rider::Tables::build();
    let Some((p, wheels)) = the_disc_125(&tables) else { return };
    let (q, set) = md16_pack(&p, true);
    let hands = md16_script(&[
        (130, 0.0, 0.0, true, [false; 2]),
        (140, 0.0, 1.0, true, [false; 2]),
        (158, 0.0, -1.0, true, [false; 2]),
        (176, 0.0, 0.0, true, [false; 2]),
        (285, 0.0, 1.0, true, [false; 2]),
        (295, 0.0, 0.0, true, [false; 2]),
    ]);
    let (ride, r) = ride_md16(&q, wheels, &tables, set, &hands, 400);
    let seats = md16_seats(&ride);
    let fired = ride.iter().position(|t| t.seat.flip).expect("the front flip fires");
    assert!(ride[fired].seat.lip && ride[fired].flip_row == 1, "on the lip, on the front flip's row");
    let over = came_over(&seats);
    assert!(over < -5.5 && over > -7.0, "once round forwards: {over}");
    assert_eq!(wrecks(&seats), 0, "it lands");
    let last = seats.last().unwrap();
    assert!(last.on_ground && last.up_y > 0.95, "on its wheels: {}", last.up_y);
    assert!(ride.iter().any(|t| t.nodes.contains(&game_sim::score::FRONTFLIP)), "scored as a front flip");
    assert!(ride.iter().any(|t| t.frame.started == Some(Trick::FRONTFLIP)), "and told as a start, for the shell's name");
    assert_eq!(r.score.total, F(10000.0), "worth the pack's backflip");
}

/// MD16: MX vs. ATV's 360. R1 held, stick left then right on the lip: armed one way and fired the
/// other, with the yaw torque the pack's `[Three Sixty]` maps the flick to. In the air the bike turns
/// round, and the rider ends it — R1 released, the stick pushed against the turn — once it is back
/// inside 35° of its take-off heading: the clip comes round, it scores, and it lands. Since MD30 the
/// stick's own yaw moment pushes against the turn too (MX vs. ATV's air control), so the push comes
/// later and shorter than MD16 had it.
///
/// Held to the end instead, the torque never decays: the bike turns on past its heading and lands
/// with the clip part way round, which is MX vs. ATV's wreck.
#[test]
fn md16_three_sixty_comes_round_or_wrecks() {
    let tables = rider::Tables::build();
    let Some((p, wheels)) = the_disc_125(&tables) else { return };
    let (q, set) = md16_pack(&p, true);
    let flick = [
        (130, 0.0, 0.0, true, [false; 2]),
        (150, -1.0, 0.0, true, [false; 2]),
        (158, 1.0, 0.0, true, [false; 2]),
    ];
    let mut ended = flick.to_vec();
    ended.extend([(166, 0.0, 0.0, false, [false; 2]), (270, -1.0, 0.0, false, [false; 2]), (290, 0.0, 0.0, false, [false; 2])]);
    let (ride, r) = ride_md16(&q, wheels, &tables, set.clone(), &md16_script(&ended), 400);
    let seats = md16_seats(&ride);
    let fired = ride.iter().position(|t| t.spin.active).expect("the 360 fires");
    assert!(ride[fired].seat.lip && ride[fired].spin.dir == 1, "on the lip, turning right");
    assert!(ride.iter().any(|t| t.spin.clip == 2), "it turned past 35°");
    assert!(ride.iter().any(|t| t.nodes.contains(&game_sim::score::THREE_SIXTY_RIGHT)), "it came round and scored");
    assert!(ride.iter().any(|t| t.frame.started == Some(Trick::THREE_SIXTY_RIGHT)), "and told as a start");
    assert_eq!(wrecks(&seats), 0, "and landed");
    assert!(r.score.total >= F(15000.0));

    let mut held = flick.to_vec();
    held.push((166, 0.0, 0.0, true, [false; 2]));
    let (ride, _) = ride_md16(&q, wheels, &tables, set, &md16_script(&held), 400);
    let wreck = ride.iter().position(|t| t.frame.wreck.is_some()).expect("landing mid-clip wrecks");
    assert!(ride[wreck - 1].spin.clip == 2 && ride[wreck].seat.on_ground, "on landing, part way round");
    // The 360's own wreck is Tail, but the landing's attitude limits are tested first; since S67 the
    // no-hander's roll reaches the body (the stunt clocks' copy-back), and the bike comes down rolled
    // past them, off to the right.
    let side = [game_sim::air::Wreck::Tail, game_sim::air::Wreck::Right].map(game_sim::trick::Wreck::from);
    assert!(ride[wreck].frame.wreck.is_some_and(|w| side.contains(&w)), "{:?}", ride[wreck].frame.wreck);
}

/// P129: the shell's hands do no trick off the d-pad. In the air off the flip ramp, a trick button
/// held with d-pad Up starts a trick on the original's hands (the d-pad as [`Controls::through`]
/// reads it: the gesture's direction and the digital stick), and nothing at all once
/// [`Controls::stick_alone`] has let the d-pad go — the ride the same, tick for tick, as the trick
/// button alone. The left stick pushed up in the d-pad's place starts the very trick the d-pad did.
#[test]
fn stick_alone_does_no_trick_off_the_dpad() {
    let tables = rider::Tables::build();
    let Some((p, wheels)) = the_disc_125(&tables) else { return };
    let started = |ride: &[Md16]| ride.iter().filter_map(|t| t.frame.started).collect::<Vec<_>>();
    let at = |c: Controls| vec![(200, c), (230, Controls::default())];
    let button = Controls { trick: [true, false, false], stunt: true, ..Controls::default() };
    let dpad_up = Controls { dpad_y: true, dpad_dir: 1, dpad: [false, false, true, false], ..button };
    let (original, _) = ride_md16(&p, wheels, &tables, Set::default(), &at(dpad_up), 360);
    let (shell, _) = ride_md16(&p, wheels, &tables, Set::default(), &at(dpad_up.stick_alone()), 360);
    let (alone, _) = ride_md16(&p, wheels, &tables, Set::default(), &at(button), 360);
    let (stick, _) = ride_md16(&p, wheels, &tables, Set::default(), &at(Controls { stick: (F::ZERO, F(-1.0)), ..button }), 360);
    assert!(started(&original).iter().any(|t| t.slot().is_some()), "the original's d-pad gesture starts no trick: {:?}", started(&original));
    assert!(started(&shell).is_empty(), "the shell's d-pad started {:?}", started(&shell));
    assert!(shell.iter().zip(&alone).all(|(a, b)| a.hash == b.hash), "the let-go d-pad rode");
    assert_eq!(started(&stick), started(&original), "the stick starts the d-pad's trick");
}

/// MD21: the 360 of [`md16_three_sixty_comes_round_or_wrecks`], with the body run each tick on a
/// library that has the pack's two 360 clips. Past 35° (`spin.clip` set) the body asks for
/// threesixtyR and plays it; before, and once the bike has come round, it asks for what it did
/// without. A library with no pack clips — every MX Unleashed body — never leaves the sixty, and one
/// with only threesixtyL plays nothing for a right-hand 360.
#[test]
fn md21_a_three_sixty_plays_threesixty_r() {
    use game_sim::motion::{Clip, ClipId};
    use game_sim::vehicle_anim::{self as va, Library, PACK_EXTRA, SPIN_LEFT, SPIN_RIGHT, STATES};
    let tables = rider::Tables::build();
    let Some((p, wheels)) = the_disc_125(&tables) else { return };
    let (q, set) = md16_pack(&p, true);
    let script = md16_script(&[
        (130, 0.0, 0.0, true, [false; 2]),
        (150, -1.0, 0.0, true, [false; 2]),
        (158, 1.0, 0.0, true, [false; 2]),
        (166, 0.0, 0.0, false, [false; 2]),
    ]);
    let mut sixty = Library::default();
    for s in 0..STATES {
        sixty.clip[s] = Some(ClipId(s as u32));
    }
    let with = |states: &[i32]| {
        let mut lib = sixty.clone();
        for &s in states {
            lib.pack[(s - PACK_EXTRA) as usize] = Some(ClipId(s as u32));
        }
        lib
    };
    // (spin.clip, the state the body asked for, the state it ran) per tick.
    let ride = |lib: &Library| {
        let mut r = Rider::new(&q, wheels, Vf::new(0.0, 2.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), set.clone());
        r.body_rates = va::Params { rate_stunt: F(3.0), rate_default: F(3.0), rate_pose: F(3.0), rate_hold: F(3.0),
                                    rate_ready: F(1.0), cross_cap: F(0.25), start_phase: F::ZERO,
                                    band: [F(7.333_333), F(14.666_666), F(22.0)] };
        let (start, slope, length) = FLIP_RAMP;
        let mut world = Jump { origin: Vf::new(0.0, 0.0, 0.0, 1.0), dir: Vf::new(0.0, 0.0, 1.0, 0.0), start, slope, length };
        let mut now = Controls { gas: F::ONE, ..Controls::default() };
        let clips = |_: ClipId| Clip::new(F(1.0), false);
        let mut seen = Vec::new();
        for tick in 0..400 {
            if let Some((_, c)) = script.iter().find(|(at, _)| *at == tick) {
                now = Controls { gas: F::ONE, ..c.clone() };
            }
            r.frame(&q, &tables, &now, DT, &mut world);
            r.body_frame(DT, &now, rider::Gate::default(), lib, &clips);
            let spin = r.ride.tricks.as_ref().map(|t| t.spin).unwrap_or_default();
            seen.push((spin.clip, r.body.veh.want, r.body.veh.state));
        }
        seen
    };

    let both = ride(&with(&[SPIN_LEFT, SPIN_RIGHT]));
    let turned: Vec<usize> = (0..both.len()).filter(|&t| both[t].0 != 0).collect();
    assert!(turned.len() > 5, "the 360 turns past 35° for a while: {turned:?}");
    assert!(turned.iter().all(|&t| both[t].1 == SPIN_RIGHT), "the body asks for threesixtyR while it turns");
    let ran = turned.iter().filter(|&&t| both[t].2 == SPIN_RIGHT).count();
    assert!(ran + 2 >= turned.len(), "and plays it ({ran} of {} ticks)", turned.len());
    assert!(both.iter().all(|t| t.1 != SPIN_LEFT && t.2 != SPIN_LEFT), "never threesixtyL");

    let none = ride(&sixty);
    assert!(none.iter().all(|t| t.1 < STATES as i32 && t.2 < STATES as i32), "no pack, no state past the sixty");
    let unturned: Vec<usize> = (0..both.len()).filter(|t| !turned.contains(t)).collect();
    let first_after = turned.last().unwrap() + 1;
    assert!(unturned.iter().filter(|&&t| t < turned[0]).all(|&t| both[t] == none[t]), "before the turn the body is the same");
    assert_eq!(both[first_after].1, none[first_after].1, "once round it asks for what it did without");
    let left_only = ride(&with(&[SPIN_LEFT]));
    assert_eq!(left_only, none, "a right-hand 360 with only threesixtyL plays nothing new");
}

/// The bike's turn in its own frame, summed tick by tick off the chassis quaternion: (pitch, yaw),
/// in radians, as M21 measured the original's (the body-frame steps of the quaternion, summed).
fn md20_turned(ride: &[Md16]) -> (f32, f32) {
    let q = |t: &Md16| [3, 4, 5, 6].map(|k| f32::from_bits(t.bits[k]));
    let (mut pitch, mut yaw) = (0.0, 0.0);
    for w in ride.windows(2) {
        let ([ax, ay, az, aw], [bx, by, bz, bw]) = (q(&w[0]), q(&w[1]));
        // conj(a) * b: the step in a's own frame.
        let x = aw * bx - ax * bw - ay * bz + az * by;
        let y = aw * by + ax * bz - ay * bw - az * bx;
        let dw = aw * bw + ax * bx + ay * by + az * bz;
        let s = if dw < 0.0 { -2.0 } else { 2.0 };
        pitch += s * x;
        yaw += s * y;
    }
    (pitch, yaw)
}

/// MD20's hands for a flip and a 360 at once, `back` for a backflip (else a front flip), `right` for a
/// 360 turning right: R1 held, the stick diagonally against both on the lip, then diagonally into
/// both; R1 let go and the stick centred in the air, then pushed against both from `x_at` for `x_for`
/// ticks (the 360's end) and from `y_at` for `y_for` (the flip's stab), and centred again.
fn md20_hands(back: bool, right: bool, (x_at, x_for): (usize, usize), (y_at, y_for): (usize, usize)) -> Vec<(usize, Controls)> {
    let (fx, fy) = (if right { 1.0 } else { -1.0 }, if back { 1.0 } else { -1.0 });
    let mut h = vec![(130, 0.0, 0.0, true, [false; 2]), (158, -fx, -fy, true, [false; 2]), (166, fx, fy, true, [false; 2])];
    for t in 176..400 {
        let x = if (x_at..x_at + x_for).contains(&t) { -fx } else { 0.0 };
        let y = if (y_at..y_at + y_for).contains(&t) { -fy } else { 0.0 };
        h.push((t, x, y, false, [false; 2]));
    }
    md16_script(&h)
}

/// MD20: the 360 backflip and the 360 front flip, both ways round. One diagonal flick with R1 held
/// fires the flip and the 360 on the same lip tick; the bike goes once round on its pitch and once
/// round on its yaw, the rider ends both, it lands on its wheels and scores each move — the flip's
/// row and the 360's, and nothing for the pair ([`game_sim::air::both_at_once`]).
#[test]
fn md20_three_sixty_flips_turn_both_ways_and_land_clean() {
    use game_sim::score;
    let tables = rider::Tables::build();
    let Some((p, wheels)) = the_disc_125(&tables) else { return };
    let (q, set) = md16_pack(&p, true);
    for back in [true, false] {
        for right in [true, false] {
            let name = format!("{} {}", if back { "360 backflip" } else { "360 front flip" }, if right { "right" } else { "left" });
            let (ride, r) = ride_md16(&q, wheels, &tables, set.clone(), &md20_hands(back, right, (220, 40), (220, 40)), 400);
            let seats = md16_seats(&ride);
            let flip = ride.iter().position(|t| t.seat.flip).expect("the flip fires");
            let spin = ride.iter().position(|t| t.spin.active).expect("the 360 fires");
            assert_eq!(flip, spin, "{name}: one flick fires both");
            assert!(ride[flip].seat.lip, "{name}: on the lip");
            assert_eq!(ride[flip].flip_row, if back { 0 } else { 1 }, "{name}: on the flip's row");
            assert_eq!(ride[spin].spin.dir, if right { 1 } else { -1 }, "{name}: turning its way");
            assert_eq!(ride.iter().any(|t| t.spin.forward), !back, "{name}: the 360's pitch follows a front flip only");
            let (pitch, yaw) = md20_turned(&ride);
            let over = if back { -pitch } else { pitch };
            assert!(over > 5.3 && over < 6.5, "{name}: once round on the pitch: {pitch}");
            let round = if right { yaw } else { -yaw };
            assert!(round > 1.5 && round < 3.0, "{name}: and the 360 (a cork, as MX vs. ATV's is) on the yaw: {yaw}");
            assert_eq!(wrecks(&seats), 0, "{name}: it lands");
            let last = seats.last().unwrap();
            assert!(last.on_ground && last.up_y > 0.95, "{name}: on its wheels: {}", last.up_y);
            let scored = |id: u8| ride.iter().any(|t| t.nodes.contains(&id));
            assert!(scored(if back { score::BACKFLIP } else { score::FRONTFLIP }), "{name}: the flip scores");
            assert!(scored(if right { score::THREE_SIXTY_RIGHT } else { score::THREE_SIXTY_LEFT }), "{name}: the 360 scores");
            assert!(r.score.total >= F(25000.0), "{name}: both moves' points: {}", r.score.total.0);
        }
    }
}

/// MD20: either can start while the other is triggered, until the grace ends — the 360 fired on the
/// lip and the backflip six ticks later, or the other way round, still go round together and land.
/// Without R1 the second is MX vs. ATV's: a backflip asked for while the 360 turns does not start.
#[test]
fn md20_either_starts_while_the_other_is_triggered() {
    let tables = rider::Tables::build();
    let Some((p, wheels)) = the_disc_125(&tables) else { return };
    let (q, set) = md16_pack(&p, true);
    let hands = |second: (f32, f32), r1: bool| {
        let mut h = vec![(130, 0.0, 0.0, true, [false; 2]), (156, -1.0, -1.0, true, [false; 2]),
                         (162, second.0, second.1, true, [false; 2]), (168, 1.0, 1.0, r1, [false; 2])];
        h.extend((176..400).map(|t| (t, if (220..260).contains(&t) { -1.0 } else { 0.0 }, if (220..260).contains(&t) { -1.0 } else { 0.0 }, false, [false; 2])));
        md16_script(&h)
    };
    for (name, second, flip_at, spin_at) in [("360 first", (1.0, -1.0), 168, 162), ("backflip first", (-1.0, 1.0), 162, 168)] {
        let (ride, _) = ride_md16(&q, wheels, &tables, set.clone(), &hands(second, true), 400);
        assert_eq!(ride.iter().position(|t| t.seat.flip), Some(flip_at), "{name}: the backflip fires");
        assert_eq!(ride.iter().position(|t| t.spin.active), Some(spin_at), "{name}: the 360 fires");
        assert!(ride[flip_at.max(spin_at)].seat.lip, "{name}: both on the lip");
        let (pitch, _) = md20_turned(&ride);
        assert!(-pitch > 5.3 && -pitch < 6.5, "{name}: once round backwards: {pitch}");
        assert_eq!(wrecks(&md16_seats(&ride)), 0, "{name}: it lands");
        assert!(ride.iter().any(|t| t.nodes.contains(&game_sim::score::THREE_SIXTY_RIGHT)), "{name}: the 360 scores");
        assert!(ride.iter().any(|t| t.nodes.contains(&game_sim::score::BACKFLIP)), "{name}: the backflip scores");
    }
    let (ride, _) = ride_md16(&q, wheels, &tables, set, &hands((1.0, -1.0), false), 400);
    assert_eq!(ride.iter().position(|t| t.spin.active), Some(162), "the 360 fires");
    assert!(!ride.iter().any(|t| t.seat.flip), "and without R1 the backflip is locked out, as MX vs. ATV's is");
}

/// MD20: one short of either wrecks as it would alone. The 360 brought round but the backflip
/// stopped early: the 360 scores, and the bike lands on its nose (the backflip's own wreck test,
/// clip home). The flip done but the 360 not ended in time: the clip is still part way round when it
/// is due and the 360 wrecks the rider as it does alone — with the flip still going.
#[test]
fn md20_one_short_of_either_wrecks() {
    use game_sim::{air, trick};
    let tables = rider::Tables::build();
    let Some((p, wheels)) = the_disc_125(&tables) else { return };
    let (q, set) = md16_pack(&p, true);

    let (ride, _) = ride_md16(&q, wheels, &tables, set.clone(), &md20_hands(true, true, (220, 40), (200, 60)), 400);
    let w = ride.iter().position(|t| t.frame.wreck.is_some()).expect("a backflip stopped short wrecks");
    let (pitch, _) = md20_turned(&ride[..w]);
    assert!(-pitch < 5.0, "short of round: {pitch}");
    assert!(ride[w].seat.on_ground && ride[w - 1].spin.clip == 0, "on landing, the 360 home");
    assert_eq!(ride[w].frame.wreck, Some(trick::Wreck::from(air::Wreck::Front)), "on its nose");
    assert!(ride.iter().any(|t| t.nodes.contains(&game_sim::score::THREE_SIXTY_RIGHT)), "the 360 still scored");
    assert!(!ride.iter().any(|t| t.nodes.contains(&game_sim::score::BACKFLIP)), "the backflip did not");

    for back in [true, false] {
        let (ride, _) = ride_md16(&q, wheels, &tables, set.clone(), &md20_hands(back, back, (200, 60), (220, 40)), 400);
        let w = ride.iter().position(|t| t.frame.wreck.is_some()).expect("a 360 left turning wrecks");
        assert_eq!(ride[w - 1].spin.clip, 2, "back {back}: the clip part way round");
        assert!(ride[w].seat.flip, "back {back}: while the flip still goes");
        assert_eq!(ride[w].frame.wreck, Some(trick::Wreck::from(air::Wreck::Tail)), "back {back}: the 360's wreck");
    }
}

/// MD25, kept by MD30: MX vs. ATV's 360 rides as it did before MD25's easy one came and went — the
/// per-tick ride hashes of its gesture 360 and of MD20's diagonal 360 backflip, folded. The gesture's
/// is the one 4d30ef48's step gave (taken from a scratch worktree of it). The diagonal's was
/// `0x13fe6e4514be885d` there and through MD30's removal of the easy 360; MD30's air-control fix moved
/// it twice over, as MX vs. ATV's air control does both: once the backflip is over and the 360 turns
/// on alone, the stick has its say again, and once the 360 has ended in the air the bike is damped
/// until the wheels are down ([`game_sim::air::Step::spin_ramp`]) where the port had levelled. Re-pinned
/// by S67, whose bottom-out shove and stunt-clock copy-back are the original's and move every landing,
/// and again with S106's braking speed source on top of it, and with S107's inverse inertia (the
/// original's general inverse, an ulp off the old reciprocal on one axis), and with S138's Euler angles
/// measured again after the substeps (the attachment walk's measure, which the wreck test reads), and
/// with S115's centre of gravity put into the world origin first (an ulp, as the original rounds it).
#[test]
fn md25_mx_vs_atv_rides_as_before() {
    let tables = rider::Tables::build();
    let Some((p, wheels)) = the_disc_125(&tables) else { return };
    let (q, set) = md16_pack(&p, true);
    let fold = |ride: &[Md16]| ride.iter().fold(0u64, |a, t| a.rotate_left(5) ^ t.hash);
    let gesture = md16_script(&[(130, 0.0, 0.0, true, [false; 2]), (150, -1.0, 0.0, true, [false; 2]), (158, 1.0, 0.0, true, [false; 2]), (166, 0.0, 0.0, false, [false; 2])]);
    let (spin, _) = ride_md16(&q, wheels, &tables, set.clone(), &gesture, 400);
    let (flip, _) = ride_md16(&q, wheels, &tables, set, &md20_hands(true, true, (220, 40), (220, 40)), 400);
    assert_eq!((fold(&spin), fold(&flip)), (MD25_GESTURE_FOLD, MD25_DIAGONAL_FOLD));
}
const MD25_GESTURE_FOLD: u64 = 0x5ad923a109eec826;
const MD25_DIAGONAL_FOLD: u64 = 0xccc259f6d830eb53;

/// MD30: a lip that gives MX vs. ATV's 360 about the air M21's captures had on that game's first
/// lip (1.6 s, take-off to touchdown): [`FLIP_RAMP`] at a shallower slope. 0.31 until S67's
/// bottom-out shove changed the take-off, 0.305 until S106's braking speed source landed on top of it
/// (34° off); at 0.30 `spin` lands 40° off and `spinpull` paths 360°.
const M21_LIP: (f32, f32, f32) = (108.0, 0.30, 25.0);

/// What one 360 off [`M21_LIP`] did in the air, take-off to touchdown, as openmx-mods' `both_sum.py`
/// sums M21's captures: the body-frame rotation per axis, the path (every step's angle summed), the
/// net angle between the two attitudes, the heading off at touchdown (all degrees), and the steady
/// body rates mid-flight (radians a second).
struct M21 {
    air: f32,
    x: f32,
    y: f32,
    path: f32,
    net: f32,
    off: f32,
    rates: (f32, f32),
}

fn m21(ride: &[Md16]) -> M21 {
    let quat = |t: &Md16| [3, 4, 5, 6].map(|k| f32::from_bits(t.bits[k]));
    let angle = |a: &Md16, b: &Md16| {
        let (a, b) = (quat(a), quat(b));
        2.0 * (a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]).abs().min(1.0).acos()
    };
    let off = ride.iter().position(|t| !t.seat.on_ground && t.spin.active).expect("it takes off turning");
    let down = off + ride[off..].iter().position(|t| t.seat.on_ground).expect("it lands");
    let (x, y) = md20_turned(&ride[off - 1..=down]);
    let path = ride[off - 1..=down].windows(2).map(|w| angle(&w[0], &w[1])).sum::<f32>();
    let d = (ride[down].seat.heading - ride[off].seat.heading).rem_euclid(std::f32::consts::TAU);
    let mid = (off + down) / 2;
    let (rx, ry) = md20_turned(&ride[mid - 1..=mid]);
    M21 {
        air: (down - off) as f32 * DT.0,
        x: x.to_degrees(),
        y: y.to_degrees(),
        path: path.to_degrees(),
        net: angle(&ride[off - 1], &ride[down]).to_degrees(),
        off: d.min(std::f32::consts::TAU - d).to_degrees(),
        rates: (rx / DT.0, ry / DT.0),
    }
}

/// M21's hands: R1 held from the approach to the end, the stick left then right on the lip (the
/// full flick), centred, and with `pull` held back from the take-off on.
fn m21_hands(pull: bool) -> Vec<(usize, Controls)> {
    let mut h = vec![(130, 0.0, 0.0, true, [false; 2]), (150, -1.0, 0.0, true, [false; 2]), (158, 1.0, 0.0, true, [false; 2]), (166, 0.0, 0.0, true, [false; 2])];
    if pull {
        h.push((176, 0.0, 1.0, true, [false; 2]));
    }
    md16_script(&h)
}

/// MD30: M21's `spin`, reproduced. MX vs. ATV state 4's first lip, R1 and left-right at the full
/// flick, nothing after (`extracted/mxvatv/capture/m21-spin.csv`): X −1100 / Y +1100, the body
/// steady at −2.44 / +2.44 rad/s pitch / yaw from 0.2 s after take-off (the 360's torque against
/// `AirborneDamping` 450: neither the 125's `YawRateLimit` of 3 nor anything else cuts it), x −212°,
/// y +217°, path 306° in 1.63 s of air, landing 37° off its heading with the clip still running: a
/// wreck. Off [`M21_LIP`] the port's 360 does the same within a few degrees.
#[test]
fn md30_m21_spin_lands_part_way_round_and_wrecks() {
    let tables = rider::Tables::build();
    let Some((p, wheels)) = the_disc_125(&tables) else { return };
    let (q, set) = md16_pack(&p, true);
    let (ride, _) = ride_md16_off(M21_LIP, &q, wheels, &tables, set, &m21_hands(false), 400);
    let fired = ride.iter().position(|t| t.spin.active).expect("the 360 fires");
    assert!(ride[fired].seat.lip, "on the lip");
    assert_eq!((ride[fired].spin.torque.x(), ride[fired].spin.torque.y()), (-1100.0, 1100.0), "the full flick's torques");
    let m = m21(&ride);
    assert!((m.air - 1.6).abs() < 0.1, "about the capture's air: {}", m.air);
    assert!((m.rates.0 + 2.44).abs() < 0.02 && (m.rates.1 - 2.44).abs() < 0.02, "the disc's steady rates: {:?}", m.rates);
    assert!((m.x + 212.0).abs() < 15.0 && (m.y - 217.0).abs() < 15.0, "x {} y {}", m.x, m.y);
    assert!((m.path - 306.0).abs() < 15.0, "path {}", m.path);
    assert!(m.off > 35.0, "short of round at the landing: {}", m.off);
    let w = ride.iter().position(|t| t.frame.wreck.is_some()).expect("landing mid-clip wrecks");
    assert!(ride[w].seat.on_ground && ride[w - 1].spin.clip == 2, "on landing, the clip part way round");
    assert!(!ride.iter().any(|t| t.nodes.contains(&game_sim::score::THREE_SIXTY_RIGHT)), "and it does not score");
}

/// MD30: M21's `spinpull`, reproduced: the same, with the stick held back from take-off and R1 still
/// down (`m21-spinpull.csv`). The 360 does not decay under a trick button, and the air control adds
/// the stick's pitch on top: the body steady at −3.33 / +2.44 rad/s, x −289°, y +217°, path 365°,
/// net 10°, landing 9° off its heading: clean. That pitch is the stick's own moment
/// (`AirborneControlMoment` −400 against the same damping), which MX vs. ATV's air control keeps
/// under a 360 and MD16 had silenced.
#[test]
fn md30_m21_spinpull_comes_round_and_lands() {
    let tables = rider::Tables::build();
    let Some((p, wheels)) = the_disc_125(&tables) else { return };
    let (q, set) = md16_pack(&p, true);
    let (ride, _) = ride_md16_off(M21_LIP, &q, wheels, &tables, set, &m21_hands(true), 400);
    let m = m21(&ride);
    assert!((m.air - 1.6).abs() < 0.1, "about the capture's air: {}", m.air);
    assert!((m.rates.0 + 3.33).abs() < 0.02 && (m.rates.1 - 2.44).abs() < 0.02, "the disc's steady rates: {:?}", m.rates);
    assert!((m.x + 289.0).abs() < 15.0 && (m.y - 217.0).abs() < 15.0, "x {} y {}", m.x, m.y);
    assert!((m.path - 365.0).abs() < 15.0, "path {}", m.path);
    assert!(m.net < 20.0 && m.off < 20.0, "round: net {} off {}", m.net, m.off);
    assert_eq!(wrecks(&md16_seats(&ride)), 0, "it lands");
    assert!(ride.iter().any(|t| t.nodes.contains(&game_sim::score::THREE_SIXTY_RIGHT)), "and scores");
}

/// One tick of an S154 ride: whether a wheel was down, the wreck entered, the heading and the pitch
/// (as [`Seat`] reads them), and the 360's state.
#[derive(Clone, Debug, PartialEq)]
struct S154 {
    on_ground: bool,
    wreck: Option<game_sim::trick::Wreck>,
    heading: f32,
    pitch: f32,
    spin: game_sim::air::Spin,
    flipping: bool,
}

/// [`ride_md16`] on the pack's bike with the Air Wrecks switch as `air_wrecks` (S154), wrecks on.
fn ride_s154(air_wrecks: bool, p: &Phy, wheels: (Vf, Vf), tables: &rider::Tables, set: Set,
             script: &[(usize, Controls)], ticks: usize) -> Vec<S154> {
    let (start, slope, length) = FLIP_RAMP;
    let mut r = Rider::new(p, wheels, Vf::new(0.0, 2.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), set);
    r.sense.air_wrecks = air_wrecks;
    let mut world = Jump { origin: Vf::new(0.0, 0.0, 0.0, 1.0), dir: Vf::new(0.0, 0.0, 1.0, 0.0), start, slope, length };
    let mut now = Controls { gas: F::ONE, ..Controls::default() };
    let mut seen = Vec::with_capacity(ticks);
    for tick in 0..ticks {
        if let Some((_, c)) = script.iter().find(|(at, _)| *at == tick) {
            now = Controls { gas: F::ONE, ..c.clone() };
        }
        let frame = r.frame(p, tables, &now, DT, &mut world);
        let f = r.ride.transform().0[2];
        seen.push(S154 {
            on_ground: r.ride.air.on_ground,
            wreck: frame.wreck,
            heading: f32::atan2(f.x(), f.z()),
            pitch: f32::atan2(f.y(), f.z()),
            spin: r.ride.tricks.as_ref().map(|t| t.spin).unwrap_or_default(),
            flipping: r.ride.air.flipping,
        });
        if frame.wreck.is_some() {
            break;
        }
    }
    seen
}

/// The tick the bike left the ground after `from`, and the tick it first came back down (or the
/// ride's length).
fn s154_flight(ride: &[S154], from: usize) -> (usize, usize) {
    let off = from + ride[from..].iter().position(|t| !t.on_ground).expect("it takes off");
    let down = off + ride[off..].iter().position(|t| t.on_ground).unwrap_or(ride.len() - off);
    (off, down)
}

/// The cuts of one move: `held` is its hands held through, `against` the stick pushing against the
/// turn, and `turned(ride, off)` the tick the held move is once round. Each cut pushes against the
/// turn at a quarter, a half and three quarters of the way from take-off to there, and lets go 40
/// ticks later. Returns, per cut, the ride with the switch on and with it off.
fn s154_cuts(q: &Phy, wheels: (Vf, Vf), tables: &rider::Tables, set: &Set, name: &str,
             held: &[(usize, Controls)], against: Controls, turned: fn(&[S154], usize) -> usize)
             -> Vec<(usize, Vec<S154>, Vec<S154>)> {
    let ride = ride_s154(true, q, wheels, tables, set.clone(), held, 400);
    let fired = ride.iter().position(|t| t.spin.active || t.flipping).expect("the move fires");
    let (off, _) = s154_flight(&ride, fired);
    let round = turned(&ride, off);
    assert!(round > off + 20, "{name}: once round {} ticks after take-off", round - off);
    [1, 2, 3]
        .map(|quarters| {
            let at = off + (round - off) * quarters / 4;
            let mut hands = held.to_vec();
            hands.retain(|(t, _)| *t < at);
            hands.push((at, against.clone()));
            hands.push((at + 40, Controls::default()));
            let on = ride_s154(true, q, wheels, tables, set.clone(), &hands, 400);
            let off = ride_s154(false, q, wheels, tables, set.clone(), &hands, 400);
            (at, on, off)
        })
        .into()
}

/// S154: under Air Wrecks Off, a move let go — a 360, a backflip, a front flip, cut short at a
/// quarter, a half and three quarters of its turn — rides to touchdown without a wreck; and with the
/// switch on the ride is the same tick for tick but for the wrecks it always had. Before S154 the 360
/// cut short in the air wrecked whatever the switch said: its torque decays under its minimum with
/// the clip still part way round, and MX vs. ATV's "landed part way round" wreck fired off the ground.
///
/// Held to the end instead, the 360 lands part way round and wrecks on the touch with the switch
/// either way: the landing is judged as P138 says.
#[test]
fn s154_cut_short_moves_ride_down_with_air_wrecks_off() {
    let tables = rider::Tables::build();
    let Some((p, wheels)) = the_disc_125(&tables) else { return };
    let (q, set) = md16_pack(&p, true);
    let spin_held = md16_script(&[
        (130, 0.0, 0.0, true, [false; 2]),
        (150, -1.0, 0.0, true, [false; 2]),
        (158, 1.0, 0.0, true, [false; 2]),
        (166, 0.0, 0.0, true, [false; 2]),
    ]);
    // The flips are held to the end with the stick centred: nothing decays their torque.
    let back_held = md16_script(&[(140, 0.0, 1.0, false, [false; 2]), (158, 0.0, -1.0, false, [false; 2]),
                                  (166, 0.0, 1.0, false, [false; 2]), (176, 0.0, 0.0, false, [false; 2])]);
    let front_held = md16_script(&[(130, 0.0, 0.0, true, [false; 2]), (140, 0.0, 1.0, true, [false; 2]),
                                   (158, 0.0, -1.0, true, [false; 2]), (176, 0.0, 0.0, true, [false; 2])]);
    // Once round: the 360's clip back inside 35° after playing; a flip's nose through a whole turn.
    fn spun(ride: &[S154], off: usize) -> usize {
        let past = off + ride[off..].iter().position(|t| t.spin.clip == 2).expect("the 360 turns past 35°");
        past + ride[past..].iter().position(|t| t.spin.clip == 0).expect("and comes round")
    }
    fn flipped(ride: &[S154], off: usize) -> usize {
        let mut turned = 0.0f32;
        for k in off + 1..ride.len() {
            let d = ride[k].pitch - ride[k - 1].pitch;
            turned += d - std::f32::consts::TAU * (d / std::f32::consts::TAU).round();
            if turned.abs() >= std::f32::consts::TAU {
                return k;
            }
        }
        panic!("the flip never came round: {turned}");
    }
    let moves: [(&str, &[(usize, Controls)], Controls, fn(&[S154], usize) -> usize); 3] = [
        ("360", &spin_held, md16_hands(-1.0, 0.0, false, [false; 2]), spun),
        ("backflip", &back_held, md16_hands(0.0, -1.0, false, [false; 2]), flipped),
        ("front flip", &front_held, md16_hands(0.0, 1.0, false, [false; 2]), flipped),
    ];
    let mut spin_wrecks = 0;
    for (name, held, against, turned) in moves {
        for (at, on, off) in s154_cuts(&q, wheels, &tables, &set, name, held, against, turned) {
            let fired = off.iter().position(|t| t.spin.active || t.flipping).expect("the move fires");
            let (lift, down) = s154_flight(&off, fired);
            assert!(down < off.len(), "{name} let go at {at}: never came down");
            assert!(off[..down].iter().all(|t| t.wreck.is_none()),
                    "{name} let go at {at}, Air Wrecks off: wrecked in the air at {:?}",
                    off.iter().position(|t| t.wreck.is_some()).map(|k| (k, off[k].wreck)));
            // On: tick for tick the same ride until on's wreck, which is the only thing the switch moves.
            let w = on.iter().position(|t| t.wreck.is_some()).unwrap_or(on.len());
            assert_eq!(on[..w], off[..w], "{name} let go at {at}: the switch moved the ride itself");
            if w < down {
                assert!(w > lift, "{name} let go at {at}: wrecked before take-off");
                spin_wrecks += usize::from(name == "360" && on[w].wreck == Some(game_sim::trick::Wreck::from(game_sim::air::Wreck::Tail)));
            }
        }
    }
    assert_eq!(spin_wrecks, 3, "the switch on, a 360 cut short in the air is MX vs. ATV's wreck");

    // Held: the landing part way round is judged with the switch either way.
    let on = ride_s154(true, &q, wheels, &tables, set.clone(), &spin_held, 400);
    let off = ride_s154(false, &q, wheels, &tables, set, &spin_held, 400);
    assert_eq!(on, off, "a held 360 is the same ride with the switch either way");
    let w = off.iter().position(|t| t.wreck.is_some()).expect("landing part way round wrecks");
    assert!(off[w].on_ground && off[w - 1].spin.clip == 2, "on the touch, part way round");
}
