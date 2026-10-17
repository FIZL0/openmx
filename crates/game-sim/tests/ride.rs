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
// (5) the wreck, and getting back up
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
const CAPTURE_DT: F = F(0.03333);

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
        let f = r.frame(&p, &tables, &hands, CAPTURE_DT, &mut world);
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
    let floor_ticks = ((0.3 + 2.0) / CAPTURE_DT.0).floor() as usize;
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
