//! The biplane as a ride: the state a plane session carries from one tick to the next, the parameters
//! it is built from, and the frame [`crate::Sim`] steps when the plane has the controller.
//!
//! [`crate::biplane`] holds the plane's four listeners as functions over plain state; this is the
//! value they work on, held where the harness can see it. A [`crate::Sim`] with a [`Flight`] in it is
//! a plane ride: [`crate::Sim::step`] steps the plane and leaves the bike where it stands (the
//! original's Switch To Machine does the same, P19), and the state's walk carries the plane under
//! `plane.*` — so a fixture recorded off the original's plane names fields the port has, and a ride
//! with no plane in it walks, and hashes, exactly as it did before there was one.
//!
//! **A frame is the original's whole frame of the plane** (S41, P31): the plane's one body in the
//! engine's dynamics world ([`biplane::World`], 115 substeps a second at least), with the listeners
//! the plane hangs on it in the order the original calls them —
//!
//! 1. *preforce*, once: the body's shock peak cleared, the pad through the rate limits into the
//!    surfaces ([`controls`]), and the three tires' contacts forgotten ([`Tire::clear`]);
//! 2. *force*, every substep: gravity and the collision response ([`collision_force`]), then — while
//!    the plane is not wrecked — the autopilot, the engine and the aerodynamics, each force handed to
//!    the body in its own frame; then the three tires' ground probes and the three tires' forces;
//! 3. the body integrated and its accumulators emptied, every substep; the plane has no constraints;
//! 4. *post*, once: the search refilled around the stepped pose (S99), the
//!    measure's contact half (grounded off the wheels, the aero model's true-speed switch), the crash
//!    test ([`Crash`]) with the wreck and the recovery it asks for, then the measure's flight half,
//!    which is what next frame's listeners read ([`Measure`]).
//!
//! Before the world steps, the plane's collision object searches (S48, [`Flight::frame`]): the sweep
//! back, its ray probe against the terrain and against the track's collision meshes
//! ([`crate::flight_search`]), among the candidates last frame's post listener gathered. The force listener resolves the contacts it found, and the crash test
//! reads whether there were any. The pad it is handed is the recorded one, through I6's mapping ([`pad`]). The
//! acceptance run over C7's capture (`the_biplane_acceptance_run` in `tests/replay.rs`) says how far
//! it agrees, lane by lane, on every `./kit check` that has the capture.

#![deny(clippy::float_arithmetic)]

use crate::air::AcosTable;
use crate::biplane::{
    self, Aero, Air, Area, Hold, Attitude, Autopilot, Body, Crash, CrashContact, Dynamics, Engine, Joystick, List,
    Measure, Mount, Pad, Surfaces, THROWN, Tire, autopilot, collision_force, controls, gravity_force,
};
use crate::camera::CurveTable;
use crate::chassis::{CLEAR, turn};
use crate::collide::{Bundle, Record};
use crate::contact::{self, Collider, Weighed};
use crate::broad_phase;
use crate::contact_search;
use crate::flight_search::{self, Bikes, Rays, Scene, VEHICLE};
use crate::prop_set::PropSet;
use crate::wreck_bike::{self, node_of};
use std::sync::Arc;
use crate::hash::{State, Walk};
use crate::input::{self, PlaneInput};
use crate::math::{basis, mat4_to_quat};
use crate::noise::SmoothNoise;
use crate::phy;
use crate::ride;
use crate::rng::Rng;
use crate::track;
use crate::world;
use ps2_float::{Dest, F, Vf, vu};

/// The engine's constructor values the file does not set: the rpm the drawn prop turns at, the
/// power the file's `MaxPower` replaces, and the throttle's idle and top revolutions.
const PROP_RPM: f32 = f32::from_bits(0x4359_3fa2);
const IDLE_RPM: f32 = 500.0;
const MAX_RPM: f32 = 2500.0;

/// The aerodynamic model's constructor density, slugs per cubic foot: sea level.
const DENSITY: f32 = f32::from_bits(0x3b1b_c5c4);

/// The body's inverse inertia, as the original holds it once the plane is built (C7's save state).
///
/// The original works the tensor out of the plane model's bounding box and the file's
/// `InertiaScale`; the model's box is the renderer's side of the disc and not something this crate
/// reads, so the built value is carried (the precedent is the bike wreck's `ride::WRECK_INERTIA`).
/// `tests/flight.rs` holds it to the state.
const INV_INERTIA: Vf = Vf([f32::from_bits(0x39b9_06ac), f32::from_bits(0x3932_509d), f32::from_bits(0x3988_4ac4), 1.0]);

/// The body's inertia tensor, which the collision object's own is scaled from (C7's save state, the
/// body's `+0x110`): what [`INV_INERTIA`] is the inverse of, as the original keeps both.
const INERTIA: Vf = Vf([f32::from_bits(0x4531_1971), f32::from_bits(0x45b7_c3ca), f32::from_bits(0x4570_6cc5), 1.0]);

/// The plane's collidable's categories (the collision object's `+0x1f8`, its `+0x84`) and the object's
/// actor and collides-with masks (`+0x110`, `+0x114`), off C7's save state: not keys of the file.
pub const COLLIDABLE: u32 = 0x0004_2058;
pub const ACTORS: u32 = 8;
pub const COLLIDES_WITH: u32 = 0xffff_ffff;

/// How much a wreck spins the body up (S36's `Plane::wreck`).
const WRECK_SPIN: f32 = 4.0;

/// Substeps a second at least, and constraint passes a substep: the game's dynamics world.
const WORLD: biplane::World = biplane::World { iterations: 1, rate: F(115.0) };

/// No damping: what the body keeps of its velocity and rates when it is not wrecked.
const UNDAMPED: Vf = Vf([1.0, 1.0, 1.0, 1.0]);

/// A plane in a ride: its parameters off `biplane.phy`, and its state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Flight {
    /// `[Joystick]` and the surface limits.
    pub joystick: Joystick,
    /// `[Autopilot]`.
    pub autopilot: Autopilot,
    /// The engine: its `[Engine]` keys and the power, rpm, thrust and prop it keeps.
    pub engine: Engine,
    /// The aerodynamic model: its `[Aerodynamics]` keys and the coefficients it last worked out.
    pub aero: Aero,
    /// The rigid body the plane flies on.
    pub body: Body,
    /// What the measure listener worked out at the end of the last frame: the frame the wheels are
    /// placed by, the airspeed, the flow angles.
    pub measure: Measure,
    /// Front left, front right, and the tail wheel.
    pub tires: [Tire; 3],
    /// Each wheel's braking pushed again through the centre of gravity, so many times over (the
    /// gear's [`phy::Gear::cg_braking`]): zero but for the Biplane acceleration modifier (P116).
    pub cg_braking: [F; 3],
    /// The Biplane max G modifier's limit, in g (P137, the autopilot's [`phy::Autopilot::max_g`]):
    /// each substep the elevator command is held to what pulls no more and the ailerons eased above
    /// the corner speed ([`Aero::g_limit`]). Zero, off, as the file builds the plane.
    pub max_g: F,
    /// The least share of the aileron that hold eases a roll to (the autopilot's
    /// [`phy::Autopilot::max_g_roll`]): one, no easing, but for a plane faster than the file's.
    pub max_g_roll: F,
    /// The pad as the plane's input step left it.
    pub pad: Pad,
    pub surfaces: Surfaces,
    /// What the measure listener left for the controls and the autopilot.
    pub attitude: Attitude,
    /// And for the aerodynamics.
    pub air: Air,
    /// The crash test's state: the wreck reason (0 flying, 1 crashed, 13 thrown back from the map's
    /// edge) and the seconds in it.
    pub crash: Crash,
    /// The gravity force pushed on the body every substep, `(0, -scale·m·g, 0, 1)`: the file's scale,
    /// or `WreckGravity` while wrecked.
    pub gravity: Vf,
    /// The file's `GravityScale`, the mass, and the inverse inertia as built: what an unwreck puts
    /// back.
    pub gravity_scale: F,
    pub mass: F,
    pub inv_inertia: Vf,
    /// The scale the map-edge throw halves and its recovery doubles back (zero as built; nothing reads
    /// it).
    pub edge_scale: F,
    /// `[Wrecks]`.
    pub wrecks: phy::PlaneWrecks,
    /// Where the plane is put back when a wreck recovers or the reset chord is pressed: where its
    /// body (its centre of gravity) goes, and its nose — the original's spawn transform, which its
    /// reset stands the body on as it is (S164). [`Flight::place`] and [`Flight::reset`] are handed
    /// the model's origin and keep the body's spot it makes; [`Flight::place_body`] and
    /// [`Flight::reset_body`] the body's spot itself, as the original's placement gives one
    /// ([`bay_spot`]).
    pub spawn: (Vf, Vf),
    /// The spawn's up, the third of the original's spawn transform (S205): the ground's normal under
    /// a `machine.txt` bay as the original's placement queries it ([`Flight::place_body`]), the
    /// world's up at the port's own spots. Its reset re-crosses it and the nose against each other
    /// and keeps the two it made ([`spawn_axes`]).
    pub spawn_up: Vf,
    /// The terrain's extent, for the map-edge throw; `None` over a world with no terrain, which has
    /// no edge.
    pub area: Option<Area>,
    /// The engine's shared smooth noise, built off [`Flight::rng`] on its first sample as the
    /// original builds it off `rand()`.
    pub noise: Option<SmoothNoise>,
    /// The game's `rand()`, as far as the plane draws from it: the noise's tables and the throw's
    /// spins.
    pub rng: Rng,
    /// The collision object (S48): its response, its search's record with the contacts of the frame,
    /// and the ray probe out of `biplanem.col` it searches the world with. A plane built with no ray
    /// probe ([`Flight::new`] alone) has no collision at all but its wheels.
    pub collider: Collider,
    pub record: contact_search::Record,
    pub rays: Option<Arc<Rays>>,
    /// The collision object's nearby search (S96): what the broad phase last gathered around the ray
    /// probe, kept from frame to frame while the probe stays within its bound.
    pub nearby: broad_phase::Search,
    /// The ride's bikes' collision mesh (S95, `bike\col\bikef.col`): with it the other riders' bikes
    /// are in the plane's world ([`Flight::frame_among`]). Geometry off the disc, like the rays.
    pub bike: Option<Arc<track::Mesh>>,
    /// The plane's own collision mesh (N22a, [`HULL`]): with it a flown plane is in the riders'
    /// world, its hull struck by their bikes' body rays ([`crate::rider_contact::gather_met`]).
    /// Geometry off the disc, like the rays.
    pub hull: Option<Arc<track::Mesh>>,
    /// What the material table's listener heard the probe strike in the frame's search (F27a,
    /// [`crate::material_hit`]): a bike by player. Sound only.
    pub heard: crate::material_hit::Heard,
}

/// The plane's collision mesh in the machines' archive (N22a): its collidable's, kind 0, which the
/// riders' bikes strike.
pub const HULL: &str = "Biplane\\col\\biplanef.col";

/// **The riders' bikes as a flown plane meets them** (N22a): the mirrors the bikes' body rays put on
/// the plane's list when they struck its hull ([`crate::rider_contact::gather_met`]), each naming
/// its bike by player, and every player's collision object and chassis — the other sides of the
/// plane's resolve. A mirror is kind 1: it takes no impulse, and its spring is anchored on the
/// bike, which it does not move.
///
/// **And the bikes the plane strikes** (N22b): with [`Met::bikes`] and [`Met::named`] the plane's own
/// rays striking a bike's frame name the bike's collision object, as the original's do (the frame
/// collidable's owner is the bike's record): the plane's entry (kind 0) names the bike by player,
/// so its resolve impulses the bike's chassis as the plane's other ([`contact::resolve`]'s vehicle
/// side), and the mirror (kind 1) goes on the bike's list naming the plane, which the plane's frame
/// leaves in [`Met::struck`] for the bike's resolve. Which bikes it pushed is [`Met::pushed`].
#[derive(Clone, Debug, Default)]
pub struct Met {
    pub mirrors: Vec<contact::Contact>,
    pub colliders: Vec<Collider>,
    pub vehicles: Vec<contact::Vehicle>,
    /// The player each of the plane's [`flight_search::Bike`]s is, in their order: its index in
    /// [`Met::vehicles`]. Empty, the plane's rays strike the bikes as S95's static boxes.
    pub bikes: Vec<usize>,
    /// What the bikes' lists call this plane: the plane's index among the bikes' resolve's others
    /// ([`crate::rider_contact::Bump::planes`], after the vehicles).
    pub named: Option<usize>,
    /// Out: the mirrors the plane's search put on each player's bike list, by player.
    pub struck: Vec<Vec<contact::Contact>>,
    /// Out: the players whose bikes an entry of the plane's own named, whose chassis its resolve may
    /// have moved.
    pub pushed: Vec<bool>,
    /// The wreck bikes whose rays struck its hull this tick (S274, [`crate::wreck_plane`]): the bodies
    /// the mirrors in [`Met::mirrors`] naming [`WRECK_FROM`]` + j` push against, after the vehicles and
    /// the props among its resolve's other sides; written back as its resolve left them.
    pub wrecks: Vec<crate::prop_contact::Pushed>,
    /// The wreck bikes stepped in its world (F32, [`crate::wreck_plane::InWorld`]): each a body of the
    /// plane's dynamics world, [`Met::wrecks`]' body, its force listener after the plane's.
    pub downed: Vec<crate::wreck_plane::InWorld>,
}

impl Flight {
    /// A plane built off its file, at rest at the origin: the parameters are the file's and the
    /// constructors', the state is the constructors' zeros. Being flown is the caller's to say
    /// ([`Attitude::flown`]), as is where it stands.
    pub fn new(plane: &phy::Plane) -> Flight {
        let a = &plane.aero;
        let s = &plane.surfaces;
        let j = &plane.joystick;
        let p = &plane.autopilot;
        let e = &plane.engine;
        let mut f = Flight {
            joystick: Joystick {
                throttle_increment: j.throttle_increment,
                reverse_throttle: j.reverse_throttle,
                aileron_rate: j.aileron_control_rate_limit,
                elevator_rate: j.elevator_control_rate_limit,
                rudder_rate: j.rudder_control_rate_limit,
                max_elevator: s.max_elevator,
                max_aileron: s.max_aileron,
                max_rudder: s.max_rudder,
                ramp_off_roll: s.elevator_ramp_off_roll_angle,
                off_roll: s.elevator_off_roll_angle,
                min_elevator: s.min_elevator,
            },
            autopilot: Autopilot {
                aileron_dead_zone: p.aileron_dead_zone,
                max_pitch_rate: p.max_pitch_rate,
                max_roll: p.max_roll_angle,
                feed_forward: p.pitch_rate_feed_forward_scale,
                gain: p.pitch_rate_gain,
                integrator_gain: p.pitch_rate_integrator_gain,
            },
            engine: Engine {
                min_efficiency: e.min_prop_efficiency,
                full_efficiency_speed: e.min_prop_efficiency_speed,
                prop_rpm: F(PROP_RPM),
                max_power: e.max_power,
                max_altitude: e.max_altitude,
                idle_rpm: F(IDLE_RPM),
                max_rpm: F(MAX_RPM),
                ..Engine::default()
            },
            aero: Aero {
                // Set on every plane the original has built: the live one in C7's save state carries
                // it, though the model's constructor clears it and the store that sets it is not in
                // the plane's constructors, its init or its loader (the journal entry says where
                // else it was looked for).
                true_speed: true,
                density: F(DENSITY),
                chord: a.chord,
                span: a.span,
                area: a.surface_area,
                default_alpha: a.default_angle_of_attack,
                cm_break_alpha: a.cm_break_angle_of_attack,
                lift_round_off: a.cl_round_off,
                lift_drop_off: a.cl_drop_off,
                lift_scale: a.cl_scale,
                lift_angle_scale: a.cl_angle_scale,
                lift_q: a.cl_q,
                lift_de: a.cl_de,
                drag0: a.cd0,
                drag1: a.cd1,
                drag_de: a.cd_de,
                side_beta: a.cy_beta,
                side_da: a.cy_da,
                side_dr: a.cy_dr,
                side_p: a.cy_p,
                side_r: a.cy_r,
                cl_beta: a.cl_beta,
                cl_p: a.cl_p,
                cl_r: a.cl_r,
                cl_da: a.cl_da,
                cl_dr: a.cl_dr,
                cm0: a.cm0,
                cm_alpha: a.cm_alpha,
                cm_q: a.cm_q,
                cm_de: a.cm_de,
                cm_stall: a.cm_stall,
                cn_beta: a.cn_beta,
                cn_p: a.cn_p,
                cn_r: a.cn_r,
                cn_dr: a.cn_dr,
                cn_da: a.cn_da,
                ..Aero::default()
            },
            spawn_up: WORLD_UP,
            ..Flight::default()
        };
        let m = &plane.mass_properties;
        f.mass = m.mass;
        f.gravity_scale = m.gravity_scale;
        f.gravity = gravity_force(m.gravity_scale, m.mass);
        f.inv_inertia = INV_INERTIA;
        f.wrecks = plane.wrecks;
        f.crash.min_wreck_speed = plane.wrecks.min_wreck_speed;
        f.crash.wreck_time = plane.wrecks.wreck_time;
        f.body = Body {
            enabled: true,
            inv_mass: F::ONE / m.mass,
            damping: m.velocity_damping,
            ang_damping: m.angular_rate_damping,
            inv_inertia: INV_INERTIA,
            cofg: m.cofg,
            quat: Vf::new(0.0, 0.0, 0.0, 1.0),
            last_quat: Vf::new(0.0, 0.0, 0.0, 1.0),
            position: m.cofg,
            last_position: m.cofg,
            velocity: CLEAR,
            ..Body::default()
        };
        // The two front legs take the brake and the tail wheel the steer: the pointers the original's
        // gear keeps to the plane's brake and steer words.
        f.tires = [0, 1, 2].map(|k| tire(&plane.gear[k], k < 2, k == 2));
        f.cg_braking = plane.gear.map(|g| g.cg_braking);
        f.max_g = plane.autopilot.max_g;
        f.max_g_roll = plane.autopilot.max_g_roll;
        f.collider = Collider {
            collidable: Some(COLLIDABLE),
            actors: ACTORS,
            collides_with: COLLIDES_WITH,
            ..wreck_bike::collider(m, m.mass, f.body.inv_mass, INERTIA)
        };
        f.record = contact_search::Record { rays: None, capacity: 0, ..wreck_bike::record(m, node_of(&f.body)) };
        f
    }

    /// **The plane's numbers changed under a flight going on** (P98): what [`Flight::new`] took off
    /// the file and the game modifiers ([`crate::modifiers::Mods::plane`]) reach — the engine's power
    /// and the speed its prop is fully efficient from, the zero-lift drag, the throttle's rate and the
    /// wheels' braking force with the elevator's drag (P116), the gravity scale, the turn's
    /// lift, induced drag, pitch moments and pitch-rate target (P108), the engine's altitude fade
    /// (P136) — taken again from `plane`, the
    /// flight left where and how it is. The gravity force is the file's scale's only while flying: a
    /// wreck's is `WreckGravity`'s, and its recovery builds the new scale's.
    pub fn retune(&mut self, plane: &phy::Plane) {
        let fresh = Flight::new(plane);
        self.engine.max_power = fresh.engine.max_power;
        self.engine.full_efficiency_speed = fresh.engine.full_efficiency_speed;
        self.aero.drag0 = fresh.aero.drag0;
        // The acceleration's brakes (P116): how fast the throttle moves, and each wheel's braking
        // force; and the elevator's drag, which the acceleration scales with the others.
        self.joystick.throttle_increment = fresh.joystick.throttle_increment;
        self.aero.drag_de = fresh.aero.drag_de;
        for (t, n) in self.tires.iter_mut().zip(&fresh.tires) {
            t.brake_friction = n.brake_friction;
        }
        self.cg_braking = fresh.cg_braking;
        // The turn's (P108).
        self.aero.lift_scale = fresh.aero.lift_scale;
        self.aero.drag1 = fresh.aero.drag1;
        self.aero.cm_alpha = fresh.aero.cm_alpha;
        self.aero.cm_q = fresh.aero.cm_q;
        self.aero.cm_de = fresh.aero.cm_de;
        self.aero.cm_stall = fresh.aero.cm_stall;
        self.autopilot.max_pitch_rate = fresh.autopilot.max_pitch_rate;
        // The ceiling's (P136): where the engine's altitude fade starts and ends.
        self.engine.fade_altitude = fresh.engine.fade_altitude;
        self.engine.max_altitude = fresh.engine.max_altitude;
        // The g limit (P137).
        self.max_g = fresh.max_g;
        self.max_g_roll = fresh.max_g_roll;
        self.gravity_scale = fresh.gravity_scale;
        if self.crash.reason == 0 {
            self.gravity = gravity_force(self.gravity_scale, self.mass);
        }
    }

    /// **Back into a flight it kept** (P107, a plane's session marker): the whole of `mark` —
    /// where the plane was and its attitude, its speed and spin, the throttle and surfaces, the
    /// engine, the wheels, the wreck counter — out of whatever this flight is in, wreck or not. What
    /// is this flight's own stays: the numbers the modifiers set ([`Flight::retune`]'s, so a marker
    /// kept before a change flies on the plane as it is now), and the ray probe and the bike mesh,
    /// which are the disc's. With the numbers unchanged it is `mark` bit for bit, and the next tick
    /// is the tick after the placement flown again.
    pub fn put_back(&mut self, mark: &Flight) {
        let mut back = mark.clone();
        back.engine.max_power = self.engine.max_power;
        back.engine.full_efficiency_speed = self.engine.full_efficiency_speed;
        back.aero.drag0 = self.aero.drag0;
        // The acceleration's brakes (P116).
        back.joystick.throttle_increment = self.joystick.throttle_increment;
        back.aero.drag_de = self.aero.drag_de;
        for (t, n) in back.tires.iter_mut().zip(&self.tires) {
            t.brake_friction = n.brake_friction;
        }
        back.cg_braking = self.cg_braking;
        // The ceiling's (P136).
        back.engine.fade_altitude = self.engine.fade_altitude;
        back.engine.max_altitude = self.engine.max_altitude;
        back.max_g = self.max_g;
        back.max_g_roll = self.max_g_roll;
        back.gravity_scale = self.gravity_scale;
        if back.crash.reason == 0 {
            back.gravity = gravity_force(back.gravity_scale, back.mass);
        }
        back.rays = self.rays.clone();
        back.bike = self.bike.clone();
        *self = back;
    }

    /// **Fly on as `other` flies** (P141, a teleport onto a player flying the plane): [`Flight::put_back`]
    /// of `other`'s whole motion — where it is and its attitude, its speed and spin, the surfaces,
    /// the engine, the wheels — moved `beside` feet along its right wing (its orientation's first
    /// row), so the two planes fly on side by side. What stays this flight's: its throttle, the pad
    /// its last input step stored, its spawn (where a wreck's recovery puts it back) and whether it
    /// is flown; the modifiers' numbers as [`Flight::put_back`] keeps them. `other` is flying and not
    /// wrecked, so this plane comes out of any wreck it was in.
    pub fn fly_as(&mut self, other: &Flight, beside: F, acos: &AcosTable) {
        let (throttle, pad, spawn, spawn_up, flown) = (self.surfaces.throttle, self.pad, self.spawn, self.spawn_up, self.attitude.flown);
        self.put_back(other);
        (self.surfaces.throttle, self.pad, self.spawn, self.spawn_up, self.attitude.flown) = (throttle, pad, spawn, spawn_up, flown);
        let side = self.measure.matrix.0[0];
        let shift = CLEAR.merge(Dest::XYZ, side.mul(Vf::splat(beside.to_f32())));
        let b = &mut self.body;
        b.position = b.position.merge(Dest::XYZ, b.position.add(shift));
        b.last_position = b.last_position.merge(Dest::XYZ, b.last_position.add(shift));
        self.remeasure(acos);
    }

    /// **Put in the air at a speed** (P141, a teleport onto a rider who is moving): unwrecked and
    /// stood at `origin` along `facing`, wings level, as [`Flight::reset`] stands it, and then moving
    /// at `vel` (`xyz`, feet a second) — at a rider's speed, which is far under the plane's stall,
    /// so it drops until the throttle and the dive give it flying speed. This flight's throttle and
    /// its spawn (where a wreck's recovery puts it back) stay its own.
    pub fn launch(&mut self, origin: Vf, facing: Vf, vel: Vf, acos: &AcosTable) {
        let (throttle, spawn, spawn_up) = (self.surfaces.throttle, self.spawn, self.spawn_up);
        self.reset(origin, facing, acos);
        (self.surfaces.throttle, self.spawn, self.spawn_up) = (throttle, spawn, spawn_up);
        self.body.velocity = CLEAR.merge(Dest::XYZ, vel);
        self.remeasure(acos);
    }

    /// The plane with its collision object's ray probe: the rays of `Biplane\col\biplanem.col`
    /// ([`Rays::of`]), which is what the original's plane loader builds the probe from.
    pub fn with_rays(mut self, rays: Rays) -> Flight {
        self.record.rays = Some(rays.pairs());
        self.record.capacity = rays.segments.len();
        self.rays = Some(Arc::new(rays));
        self.nearby = flight_search::search();
        self
    }

    /// The plane with the ride's bikes' collision mesh ([`flight_search::BIKE_COLLISION`]), so the
    /// other riders' bikes are in its world.
    pub fn with_bike(mut self, mesh: track::Mesh) -> Flight {
        self.bike = Some(Arc::new(mesh));
        self
    }

    /// The plane with its own collision mesh ([`HULL`]), so the riders' bikes meet it (N22a).
    pub fn with_hull(mut self, mesh: track::Mesh) -> Flight {
        self.hull = Some(Arc::new(mesh));
        self
    }

    /// Its hull as the riders' bikes meet it (N22a): where its collision node stands it, the
    /// model's origin as its search last placed it, and the room its record has. `None` for a plane
    /// with no hull or no ray probe, whose collision object is in no list.
    pub fn hull(&self, seat: usize) -> Option<crate::rider_contact::Hull<'_>> {
        let mesh = self.hull.as_deref().filter(|_| self.rays.is_some())?;
        let node = node_of(&self.body);
        let at = crate::math::Mat4([node.rows[0], node.rows[1], node.rows[2], node.origin]);
        Some(crate::rider_contact::Hull { seat, mesh, at, capacity: self.record.capacity, category: COLLIDABLE })
    }

    /// Its body as a bike's resolve pushes it (N22a, [`crate::rider_contact::Bump::planes`]): the copy
    /// nearest `from` (the bike) on a world that wraps.
    pub fn flown(&self, seat: usize, from: Vf, world: &dyn world::World) -> crate::rider_contact::Flown {
        let mut body = self.body;
        let near = crate::nearest_copy(world, from, body.position);
        if near != body.position {
            let by = near.sub(body.position);
            body.position = near;
            body.last_position = body.last_position.add(by);
        }
        crate::rider_contact::Flown { seat, collider: self.collider, body, mass: self.mass }
    }

    /// The body a bike's resolve pushed, written back (N22a): its speeds, spins and what its resolve
    /// wrote, where the plane really is.
    pub fn pushed(&mut self, p: &crate::rider_contact::Flown) {
        let (position, last_position) = (self.body.position, self.body.last_position);
        let by = p.body.position.sub(position);
        self.body = p.body;
        (self.body.position, self.body.last_position) = (position, last_position);
        self.collider = p.collider;
        if p.body.position != position {
            self.collider.cofg_world = self.collider.cofg_world.sub(by);
        }
    }

    /// The plane stood with its model's origin at `origin` (the port's own spots: a switch's landing,
    /// a start line) with its nose along `facing` and its wings level, at rest; then measured, so that
    /// the first frame's wheels are placed by where it now is. Also where a recovered wreck is put
    /// back: the spawn keeps the body's spot this makes, the origin turned on by the centre of gravity.
    pub fn place(&mut self, origin: Vf, facing: Vf, acos: &AcosTable) {
        self.spawn_up = WORLD_UP;
        self.spawn = (self.body_at(origin, facing), facing);
        self.put(false, acos);
    }

    /// [`Flight::place`] with the body's spot given, not the model's: what the original's placement
    /// does with a track's `machine.txt` spot ([`bay_spot`]), the body's centre of gravity put on
    /// `body` (S164), and `up` the ground query's normal there, which tilts the wings to the ground
    /// across the nose (S205, [`spawn_axes`]).
    pub fn place_body(&mut self, body: Vf, facing: Vf, up: Vf, acos: &AcosTable) {
        (self.spawn, self.spawn_up) = ((body, facing), up);
        self.put(false, acos);
    }

    /// The plane unwrecked and put back at a new spawn, its model's origin at `origin` as
    /// [`Flight::place`] stands it: what the original's swap does to the plane it hands the controller
    /// (its unwreck, then its reset), with the throttle and the surfaces at zero.
    pub fn reset(&mut self, origin: Vf, facing: Vf, acos: &AcosTable) {
        self.spawn_up = WORLD_UP;
        self.spawn = (self.body_at(origin, facing), facing);
        self.unwreck(acos);
    }

    /// [`Flight::reset`] onto the body's spot `body` with the ground's normal `up`, as
    /// [`Flight::place_body`] stands it.
    pub fn reset_body(&mut self, body: Vf, facing: Vf, up: Vf, acos: &AcosTable) {
        (self.spawn, self.spawn_up) = ((body, facing), up);
        self.unwreck(acos);
    }

    /// The model's origin of the spawn: the body's spot with the centre of gravity, turned along the
    /// spawn's nose, taken back off. What the drawn plane's spawn is; a rounding off the origin a
    /// [`Flight::place`] was handed.
    pub fn spawn_origin(&self) -> Vf {
        let (at, facing) = self.spawn;
        let q = spawn_turn(facing, self.spawn_up);
        let o = at.merge(Dest::XYZ, at.sub(turn(q, self.body.cofg)));
        Vf::new(o.x(), o.y(), o.z(), 1.0)
    }

    /// Where the body goes for the model's origin at `origin` along `facing`: the centre of gravity
    /// turned with the plane and added on.
    fn body_at(&self, origin: Vf, facing: Vf) -> Vf {
        let q = spawn_turn(facing, self.spawn_up);
        let at = origin.merge(Dest::XYZ, origin.add(turn(q, self.body.cofg)));
        Vf::new(at.x(), at.y(), at.z(), 1.0)
    }

    /// **The reset chord** (S57), from the input slot, before the frame: `respawn` is
    /// [`crate::ride::Controls::respawn`] off the plane's pad — L1 pressed while R1 is held or the
    /// other way round, the two commands the original's chord reads ([`crate::biplane::Plane::reset_chord`]
    /// is S36's model of it). On the wheels or wrecked, the plane is unwrecked: back to its spawn at
    /// rest, on the press's own frame, as the waited-out wreck is. In the air it does nothing, and L1
    /// or R1 alone is the rudder. The original latches it once a frame because L1's and R1's press
    /// events are two calls; here they are one `respawn` a frame, so there is nothing to latch.
    /// Whether it put the plane back.
    ///
    /// Unlike the waited-out wreck's recovery, which the post listener's measure follows, nothing
    /// measures the plane between the chord and the frame (S193, C7's chord at tick 775): the
    /// original's reset writes only the measure's matrix, the spawn's orientation over the *body's*
    /// spot rather than the model's origin, and the rest of the measure is the wreck's last. So the
    /// first frame back flies on the wreck's speed, angles and Euler, and its tires probe a frame
    /// lifted by the centre of gravity: they miss, and the plane falls a frame before they take it.
    pub fn reset_chord(&mut self, respawn: bool, acos: &AcosTable) -> bool {
        if !respawn || !(self.measure.grounded || self.crash.reason != 0) {
            return false;
        }
        let last = self.measure;
        self.unwreck(acos);
        let mut matrix = self.measure.matrix;
        matrix.0[3] = self.spawn.0;
        self.measure = Measure { matrix, ..last };
        self.attitude = self.measure.attitude(self.attitude.flown, &self.body);
        self.air = self.measure.air(&self.surfaces, &self.body);
        true
    }

    /// The spawn into the body, and the measure after it: the body on the spawn's spot as it is, as
    /// the original's reset places it (S164), turned along its nose with its wings square to the
    /// spawn's up ([`spawn_axes`]). `keep`: the nose and the up it made are the spawn's from now on —
    /// the reset hands the place its spawn transform itself, which it squares in place (S205), where
    /// the factory's first placement hands it copies.
    fn put(&mut self, keep: bool, acos: &AcosTable) {
        let (at, facing) = self.spawn;
        let (facing, up) = spawn_axes(facing, self.spawn_up);
        if keep {
            (self.spawn.1, self.spawn_up) = (facing, up);
        }
        let q = axes_to_quat(facing, up);
        let b = &mut self.body;
        (b.quat, b.last_quat, b.position, b.last_position) = (q, q, at, at);
        (b.velocity, b.rates, b.force, b.moment) = (CLEAR, CLEAR, CLEAR, CLEAR);
        self.remeasure(acos);
    }

    /// The measure's flight half off the body as it stands, and the attitude and the air off that:
    /// what a frame's listeners read. A caller that has moved the body by hand (a fixture's start, a
    /// spawn) calls this before the next frame.
    pub fn remeasure(&mut self, acos: &AcosTable) {
        self.measure.flight(&self.body, acos);
        self.attitude = self.measure.attitude(self.attitude.flown, &self.body);
        self.air = self.measure.air(&self.surfaces, &self.body);
    }

    /// One frame of the plane, `pad` being what its input step made of the controller, over
    /// `world`'s ground.
    ///
    /// The collision object's search first (S48), then the world step and its
    /// listeners as the module's docs list them; the force listener resolves the search's contacts
    /// inside [`collision_force`], and the crash test is told whether there were any. A plane with no
    /// ray probe ([`Flight::new`] without [`Flight::with_rays`]) has no search: it meets the world
    /// only with its wheels, and can only crash by being thrown back from the map's edge.
    ///
    /// Not here: **what is not simulation**, the touchdown sound, the surfaces' and the pilot's animation
    ///   (S35's, the renderer's), the engine sound the prop rpm is written from, the pad's rumble.
    ///
    /// The step reads the pad the *last* frame's input step stored, and `pad` is stored after it
    /// (S94): the game steps the world before the player record's input dispatch reaches the plane,
    /// so a press is in the plane's commands on the tick it is made and in its controls on the next
    /// (C7's capture: the first gas press is in `+0x338` a row before the throttle moves). Only while
    /// the plane is flown: the dispatch reaches only the actor a player controls, and an unflown
    /// plane's controls keep the pad zeroed.
    pub fn frame(&mut self, dt: F, pad: Pad, ground: &mut dyn world::World, tables: &ride::Tables) {
        self.frame_among(dt, pad, ground, tables, &[]);
    }

    /// [`Flight::frame`] in a ride with other riders' `bikes` in it, in player order (S95): with the
    /// bikes' mesh ([`Flight::with_bike`]) they are collidables in the plane's search and its post's
    /// gather, and a plane that strikes one is spared by its crash test as by any vehicle. Answers
    /// what each bike's struck listener kept this frame ([`flight_search::bike_struck`]): the
    /// category of the probe that struck it, for the bike's wreck test ([`flight_search::struck_wreck`]).
    pub fn frame_among(
        &mut self,
        dt: F,
        pad: Pad,
        ground: &mut dyn world::World,
        tables: &ride::Tables,
        bikes: &[flight_search::Bike],
    ) -> Vec<Option<u32>> {
        self.frame_seamed(dt, pad, ground, tables, bikes).0
    }

    /// [`Flight::frame_among`], and the widths the plane was moved over a seam this frame, `[x, z]`
    /// ([`Flight::wrap_round`]): what the shell moves its camera by. `[0, 0]` on every world that
    /// does not wrap.
    pub fn frame_seamed(
        &mut self,
        dt: F,
        pad: Pad,
        ground: &mut dyn world::World,
        tables: &ride::Tables,
        bikes: &[flight_search::Bike],
    ) -> (Vec<Option<u32>>, [i8; 2]) {
        self.frame_met(dt, pad, ground, tables, bikes, None)
    }

    /// [`Flight::frame_seamed`] with the riders' bikes meeting the plane (N22a, [`Met`]): after its
    /// own search the mirrors the bikes' rays put on its list follow its own entries, as the
    /// original's node lists the plane's object ahead of the bikes', with the record's flags set as
    /// a struck object's are; its force listener resolves them against the bikes. With the bikes
    /// named ([`Met::bikes`], N22b) its own rays' hits on them are the bikes' too: its resolve pushes
    /// their chassis, and their mirrors are left in [`Met::struck`]. `None` is
    /// [`Flight::frame_seamed`].
    pub fn frame_met(
        &mut self,
        dt: F,
        pad: Pad,
        ground: &mut dyn world::World,
        tables: &ride::Tables,
        bikes: &[flight_search::Bike],
        met: Option<&mut Met>,
    ) -> (Vec<Option<u32>>, [i8; 2]) {
        self.frame_struck(dt, pad, ground, tables, bikes, met, None)
    }

    /// **[`Flight::frame_met`] among the knockable props** (S241): the plane's rays strike a prop's
    /// `f.col` as they strike a bike's frame, and the props' rays strike its hull as they strike a
    /// wreck bike's ([`crate::prop_wreck::struck_on`]), as the original's world node files the plane's
    /// collision object ahead of the props' and searches them in that order.
    ///
    /// 1. its search ([`Flight::search`]) has every prop with a collidable mesh in it after the world
    ///    ([`Scene::knocking`]): its entries (kind 0) name the prop's record, and the mirrors (kind 1,
    ///    naming the plane as `me`, its place among the props' step's strikers) go on the prop's list;
    /// 2. each struck prop woken and its mirrors filed at the head of its list ([`PropSet::strike`]);
    /// 3. the props' rays against the hull where the plane's node stands ([`Flight::hull`]): their
    ///    entries after their own, naming the plane as `me`, the mirrors after the plane's own entries
    ///    and the bikes' mirrors;
    /// 4. the plane's world steps, its resolve pushing the props its list names (their bodies after
    ///    [`Met::vehicles`] among its other sides, as a wreck bike's body: [`contact::Side::Body`]);
    ///    the pushed bodies are written back ([`PropSet::pushed`]).
    ///
    /// The props' own step, which resolves the mirrors against the plane, is the caller's, after this.
    #[allow(clippy::too_many_arguments)]
    pub fn frame_props(
        &mut self,
        dt: F,
        pad: Pad,
        ground: &mut dyn world::World,
        tables: &ride::Tables,
        bikes: &[flight_search::Bike],
        met: Option<&mut Met>,
        props: &mut PropSet,
        me: usize,
    ) -> (Vec<Option<u32>>, [i8; 2]) {
        self.frame_struck(dt, pad, ground, tables, bikes, met, Some((props, me)))
    }

    #[allow(clippy::too_many_arguments)]
    fn frame_struck(
        &mut self,
        dt: F,
        pad: Pad,
        ground: &mut dyn world::World,
        tables: &ride::Tables,
        bikes: &[flight_search::Bike],
        met: Option<&mut Met>,
        props: Option<(&mut PropSet, usize)>,
    ) -> (Vec<Option<u32>>, [i8; 2]) {
        let mut met = met;
        let shapes = props.as_ref().map(|(set, _)| Arc::clone(&set.shapes));
        let posed = match (&props, &shapes) {
            (Some((set, _)), Some(shapes)) => knocks(set, shapes),
            _ => Vec::new(),
        };
        let knocks: Vec<flight_search::Knock<'_>> = posed.iter().map(|p| p.knock).collect();
        let mesh = self.bike.clone();
        let (struck, movers, knocked) = self.search(dt, ground, mesh.as_deref().map(|mesh| Bikes { mesh, bikes }), met.as_deref_mut(), &knocks);
        let bikes = mesh.as_deref().map(|mesh| Bikes { mesh, bikes });
        if let Some(m) = met.as_deref().filter(|m| !m.mirrors.is_empty() && self.rays.is_some()) {
            let r = &mut self.record;
            (r.touching, r.on_object, r.touching_other) = (true, true, true);
            let room = r.capacity.saturating_sub(r.contacts.len());
            r.contacts.extend(m.mirrors.iter().copied().take(room));
        }
        let mut bodies: Vec<crate::prop_contact::Pushed> = Vec::new();
        let mut props = props;
        if let Some((set, me)) = props.as_mut() {
            let first = met.as_deref().map_or(0, |m| m.vehicles.len());
            self.knocked(set, *me, first, &posed, knocked, &mut bodies);
        }
        let wrecks = met.as_deref_mut().map(|m| std::mem::take(&mut m.wrecks)).unwrap_or_default();
        let at = bodies.len();
        if !wrecks.is_empty() {
            let first = met.as_deref().map_or(0, |m| m.vehicles.len()) + at;
            for e in &mut self.record.contacts {
                if let Some(j) = e.other.filter(|&o| e.kind == 1 && o >= WRECK_FROM).map(|o| o - WRECK_FROM) {
                    e.other = Some(first + j);
                }
            }
            bodies.extend(wrecks);
        }
        // The wreck bikes stepped in its world (F32), their bodies those after the props.
        let mut downed = met.as_deref_mut().map(|m| std::mem::take(&mut m.downed)).unwrap_or_default();
        for d in &mut downed {
            d.place += at;
        }
        let mut listeners = Listeners { plane: self, ground: Ground(ground), tables, bikes, movers: &movers, met, seam: [0, 0], props: &mut bodies, downed };
        WORLD.step(dt, &mut listeners);
        let seam = listeners.seam;
        let pushed = listeners.props[at..].to_vec();
        let mut downed = std::mem::take(&mut listeners.downed);
        if let Some(m) = listeners.met {
            m.wrecks = pushed;
            for d in &mut downed {
                d.place -= at;
            }
            m.downed = downed;
        }
        bodies.truncate(at);
        if let Some((set, _)) = props {
            set.pushed(&bodies);
        }
        if self.attitude.flown {
            self.pad = pad;
        }
        (struck, seam)
    }

    /// Steps 2 and 3 of [`Flight::frame_props`]: the props the search struck (`knocked`, the mirrors
    /// on each of `posed`) woken and their mirrors filed, the props' rays on the hull, and the plane's
    /// entries and the mirrors renamed to the struck bodies' places after the `first` vehicles of
    /// its resolve. The bodies are added to `bodies`, in that order, but a body already there (one
    /// another plane of [`frames_together`] struck) keeps its place, so the planes push one copy.
    fn knocked(
        &mut self,
        set: &mut PropSet,
        me: usize,
        first: usize,
        posed: &[Posed<'_>],
        knocked: Vec<Vec<contact::Contact>>,
        bodies: &mut Vec<crate::prop_contact::Pushed>,
    ) {
        // Names past [`WRECK_FROM`] are the wreck bikes' mirrors (S276), not props.
        let from = self.record.contacts.iter().filter_map(|e| e.other).filter(|o| (KNOCK_FROM..WRECK_FROM).contains(o)).collect::<Vec<_>>();
        // The struck props in the order the plane's entries first name them.
        let mut order: Vec<usize> = Vec::new();
        for o in from {
            let k = o - KNOCK_FROM;
            if !order.contains(&k) {
                order.push(k);
            }
        }
        for (k, m) in knocked.iter().enumerate() {
            if !m.is_empty() && !order.contains(&k) {
                order.push(k);
            }
        }
        let struck = order
            .iter()
            .map(|&k| {
                let mirrors = knocked[k].iter().map(|e| contact::Contact { other: Some(me), ..*e }).collect();
                (posed[k].collection, posed[k].object, mirrors)
            })
            .collect();
        let mut places = Vec::with_capacity(order.len());
        for p in set.strike(&crate::prop_contact::Strike { contacts: Vec::new(), struck }) {
            match bodies.iter().position(|b| (b.collection, b.object) == (p.collection, p.object)) {
                Some(i) => places.push(i),
                None => {
                    places.push(bodies.len());
                    bodies.push(p);
                }
            }
        }
        for e in &mut self.record.contacts {
            if let Some(k) = e.other.filter(|&o| o < WRECK_FROM).and_then(|o| o.checked_sub(KNOCK_FROM)) {
                e.other = order.iter().position(|&j| j == k).map(|i| first + places[i]);
            }
        }
        let Some(hull) = self.hull.clone().filter(|_| self.rays.is_some()) else { return };
        let node = node_of(&self.body);
        let at = [node.rows[0], node.rows[1], node.rows[2], node.origin];
        let on = crate::prop_wreck::struck_on(set, &hull, at, COLLIDABLE, COLLIDES_WITH, me);
        if on.is_empty() {
            return;
        }
        let places: Vec<usize> = on
            .struck
            .iter()
            .map(|p| match bodies.iter().position(|b| (b.collection, b.object) == (p.collection, p.object)) {
                Some(i) => i,
                None => {
                    bodies.push(p.clone());
                    bodies.len() - 1
                }
            })
            .collect();
        let r = &mut self.record;
        (r.touching, r.on_object, r.touching_other) = (true, true, true);
        let room = r.capacity.saturating_sub(r.contacts.len());
        r.contacts.extend(on.mirrors.iter().map(|e| contact::Contact { other: e.other.map(|i| first + places[i]), ..*e }).take(room));
    }

    /// **Over a seam** (MD44, the port's own; [`crate::rider::Rider::wrap_round`]'s for the plane): on
    /// a world that wraps, a plane whose body has crossed a seam is moved back one width on that
    /// axis — its body, its collision record and its collider ([`crate::seam`]) — and the answer is
    /// the widths it was past, `[x, z]`. Its speed, spin and attitude are not places and do not
    /// change, so it flies on as it was flying; the measure is taken again, for its matrix's origin.
    /// The spawn a wreck's recovery puts it back on is a place in the world already.
    pub fn wrap_round(&mut self, world: &dyn world::World, acos: &AcosTable) -> [i8; 2] {
        let wrap = world.wrap();
        let Some(area) = world.area().filter(|_| wrap != [false, false]) else { return [0, 0] };
        let at = self.body.position;
        let laps = [crate::seam::lap(wrap[0], at.x(), area.width), crate::seam::lap(wrap[1], at.z(), area.depth)];
        if laps == [0, 0] {
            return laps;
        }
        let by = [crate::seam::by(laps[0], area.width), crate::seam::by(laps[1], area.depth)];
        crate::seam::body(&mut self.body, by);
        crate::seam::record(&mut self.record, by);
        self.collider.cofg_world = crate::seam::moved(self.collider.cofg_world, by);
        self.remeasure(acos);
        laps
    }

    /// The collision object's search, before the world steps (the plane's world node, as the wreck
    /// bike's runs it): its contacts emptied, then the sweep back, the rays against the terrain and
    /// the rays against the track's collision meshes ([`contact_search::collide`] over
    /// [`flight_search::Scene`]). The transform is where the body stood after last frame's post
    /// listener, which is where the original's post listener leaves it. With what the bikes' struck
    /// listeners kept, it gives the movers its contacts name ([`Search::movers`], S145).
    ///
    /// With a [`Met`] that names the bikes (N22b), each bike's collision object is in the search
    /// after the plane's, empty, as the original's node empties every list before any searches and
    /// searches the planes first: the plane's entry on a bike names it by player, and the mirrors
    /// the bikes' records took are left in [`Met::struck`] naming the plane as [`Met::named`].
    ///
    /// With `knocks` (S241, [`Scene::knocking`]) the knockable props' meshes are in the search after
    /// the world, each prop's record after the bikes': the plane's entry on prop `k` names
    /// [`KNOCK_FROM`]` + k`, and the answer's third list is the mirrors each prop's record took.
    #[allow(clippy::type_complexity)]
    fn search(
        &mut self,
        dt: F,
        world: &mut dyn world::World,
        bikes: Option<Bikes<'_>>,
        met: Option<&mut Met>,
        knocks: &[flight_search::Knock<'_>],
    ) -> (Vec<Option<u32>>, Vec<contact::Mover>, Vec<Vec<contact::Contact>>) {
        let none = vec![None; bikes.map_or(0, |b| b.bikes.len())];
        self.heard = crate::material_hit::Heard::default();
        let Some(rays) = self.rays.clone() else { return (none, Vec::new(), Vec::new()) };
        self.record.node = node_of(&self.body);
        contact_search::clear(&mut self.record, Some(&mut self.body));
        let (field, props, trees) = flight_search::near_world(world, &rays, &self.record.node, &mut self.nearby);
        let bounds = field.as_deref().map(field_box);
        let met = met.filter(|m| m.named.is_some() && bikes.is_some_and(|b| b.bikes.len() == m.bikes.len()));
        let velocities: Vec<Vf> = met.as_deref().map_or(Vec::new(), |m| m.bikes.iter().map(|&p| m.vehicles[p].velocity).collect());
        let mut scene = Scene::flown(props, trees, bikes, bounds, &rays, &self.record.node, &mut self.nearby);
        if met.is_some() {
            scene = scene.owned(&velocities);
        }
        if !knocks.is_empty() {
            scene = scene.knocking(knocks, 1 + velocities.len());
        }
        // The node's file put the material table's listener on the plane's probe (F27a).
        scene.listen_material();
        let mut records = vec![std::mem::take(&mut self.record)];
        records.extend(velocities.iter().map(|_| contact_search::Record { capacity: crate::rider_contact::CAPACITY, ..contact_search::Record::default() }));
        records.extend(knocks.iter().map(|k| contact_search::Record { capacity: k.capacity, ..contact_search::Record::default() }));
        contact_search::collide(&mut records, 0, Some(&mut self.body), field.as_deref(), &mut scene, dt);
        let movers = contact_search::Search::movers(&scene);
        let players = met.as_deref().map(|m| m.bikes.clone());
        self.heard = crate::material_hit::Heard(
            scene
                .sounded()
                .iter()
                .map(|&s| match s {
                    crate::material_hit::Struck::Bike(k) => crate::material_hit::Struck::Bike(players.as_ref().and_then(|b| b.get(k).copied()).unwrap_or(k)),
                    s => s,
                })
                .collect(),
        );
        let mut records = records.into_iter();
        self.record = records.next().unwrap_or_default();
        let owned = velocities.len();
        for e in &mut self.record.contacts {
            if let Some(k) = e.other.filter(|&o| o > owned && !knocks.is_empty()) {
                e.other = Some(KNOCK_FROM + k - 1 - owned);
            }
        }
        if let Some(m) = met {
            let n = m.vehicles.len();
            m.struck = vec![Vec::new(); n];
            m.pushed = vec![false; n];
            for e in &mut self.record.contacts {
                if let Some(k) = e.other.filter(|&o| o < KNOCK_FROM) {
                    let p = m.bikes[k - 1];
                    e.other = Some(p);
                    m.pushed[p] = true;
                }
            }
            for (bike, &p) in records.by_ref().take(owned).zip(&m.bikes) {
                m.struck[p] = bike.contacts.into_iter().map(|e| contact::Contact { other: m.named, ..e }).collect();
            }
        }
        let knocked = records.map(|r| r.contacts).collect();
        (scene.struck, movers, knocked)
    }

    /// The collision object's search alone ([`Flight::search`], what [`Flight::frame_met`] runs before
    /// the world steps) on the bikes' collision `mesh`, for a test to set against the original's.
    pub fn search_alone(
        &mut self,
        dt: F,
        world: &mut dyn world::World,
        mesh: &track::Mesh,
        bikes: &[flight_search::Bike],
        met: Option<&mut Met>,
    ) -> Vec<Option<u32>> {
        self.search(dt, world, Some(Bikes { mesh, bikes }), met, &[]).0
    }

    /// The collision object's search alone among the knockable props `set` (S241, what
    /// [`Flight::frame_props`] runs before the world steps), for a test to set against the original's:
    /// the plane's entries on a prop name [`KNOCK_FROM`] plus its place in the answer, which is each
    /// struck prop's collection and object and the mirrors its record took (naming the plane as
    /// nothing), in the props' order.
    pub fn search_props(&mut self, dt: F, world: &mut dyn world::World, set: &PropSet) -> Vec<(usize, usize, Vec<contact::Contact>)> {
        let posed = knocks(set, &set.shapes);
        let knocks: Vec<flight_search::Knock<'_>> = posed.iter().map(|p| p.knock).collect();
        let (_, _, knocked) = self.search(dt, world, None, None, &knocks);
        posed.iter().zip(knocked).map(|(p, m)| (p.collection, p.object, m)).collect()
    }

    /// The search's refill (S99), first in the post listener: the original's body post sets the
    /// plane's pose from the stepped body and has the collision object gather its nearby list around
    /// the ray probe's reach there ([`flight_search::refresh`]), before the measure, the crash test
    /// and any unwreck — so the frame after a recovery searches with the list gathered where the
    /// plane came down. Once a world step, before the other bodies' posts move them.
    fn refresh(&mut self, world: &mut dyn world::World, bikes: Option<Bikes<'_>>) {
        let Some(rays) = self.rays.as_ref() else { return };
        let node = node_of(&self.body);
        let (field, props, trees) = flight_search::near_world(world, rays, &node, &mut self.nearby);
        let bounds = field.as_deref().map(field_box);
        flight_search::refresh_flown(props, trees, bikes.as_ref(), bounds, rays, &node, &mut self.nearby);
    }

    /// The preforce listener.
    fn preforce(&mut self, dt: F) {
        self.body.clear_shock_peak();
        controls(&mut self.pad, &mut self.surfaces, &self.joystick, &self.attitude, dt);
        for t in &mut self.tires {
            t.clear();
        }
    }

    /// The force listener, one substep of `h`.
    fn forces(
        &mut self,
        h: F,
        ground: &mut Ground<'_>,
        curve: &CurveTable,
        acos: &AcosTable,
        movers: &[contact::Mover],
        others: &mut [contact::Other<'_>],
    ) {
        let (collider, contacts, mass) = (&mut self.collider, &mut self.record.contacts, self.mass);
        collision_force(&mut self.body, self.gravity, |body| {
            contact::resolve(collider, &mut Weighed { body, mass }, contacts, others, movers, true);
        });
        if self.crash.reason == 0 {
            self.attitude = self.measure.attitude(self.attitude.flown, &self.body);
            autopilot(&self.pad, &mut self.surfaces, &self.joystick, &self.autopilot, &self.attitude, curve, h);
            let (noise, rng) = (&mut self.noise, &mut self.rng);
            let mut sample = |t: F| noise.get_or_insert_with(|| SmoothNoise::build(rng)).at(t);
            let b = &self.body;
            let thrust = self.engine.step(self.surfaces.throttle, b.velocity, F(b.position.y()), F(self.measure.forward.y()), h, &mut sample);
            self.body.add_force_local(thrust);
            self.air = self.measure.air(&self.surfaces, &self.body);
            if self.max_g != F::ZERO {
                self.hold_g(F(thrust.z()), h, acos);
            }
            let (force, moment) = self.aero.forces(&self.air);
            self.body.add_force_local(force);
            self.body.add_moment_local(moment);
        }
        let m = &self.measure;
        // The wheels' speed is the measure's signed one along the nose (the word the autopilot reads),
        // not the airspeed: rolling backward, the rolling friction turns round with it.
        let speed = F(m.velocity.z());
        let mount = Mount { frame: m.matrix.0, rates: m.world_rates, speed, brake: self.surfaces.brake, steer: self.surfaces.steer };
        for t in &mut self.tires {
            t.probe(ground, &mount);
        }
        for t in &mut self.tires {
            t.force(&mut self.body, &mount, h);
        }
        // The Biplane acceleration modifier's harder braking (P116), through the centre of gravity:
        // k times the force at the contacts would pitch the plane onto its nose against a gravity that
        // is the file's. Nothing at all as the file builds the plane.
        for (t, c) in self.tires.iter().zip(self.cg_braking) {
            if c != F::ZERO {
                self.body.add_force(t.braking(c));
            }
        }
    }

    /// The Biplane max G modifier's hold (P137), between the autopilot and the aerodynamics: the
    /// elevator command (the open-loop stick's and the pitch-rate loop's alike, and the loop's
    /// integral with it, so it does not wind up against the hold) kept to what pulls no more than
    /// [`Flight::max_g`], and the ailerons the aerodynamics fly eased above the corner speed.
    fn hold_g(&mut self, thrust: F, h: F, acos: &AcosTable) {
        let weight = self.gravity_scale * F(self.measure.up.y());
        // How fast the speed is picking up: the thrust less the last substep's drag, and the weight
        // along the nose.
        let pressure = self.aero.area * (F(0.5) * self.aero.density * self.air.speed * self.air.speed);
        let speeding = (thrust - self.aero.drag * pressure + F(self.gravity.y()) * F(self.measure.forward.y())) / self.mass;
        // The angle of attack this substep's, not the frame's measure (in a pull the wing's angle
        // runs on a good way between two frames), but found as the measure finds it: off the arc
        // cosine table, whose first entry is coarse beside the small angles a fast wing pulls at.
        let along = biplane::unturn(self.body.quat, self.body.velocity);
        let alpha = biplane::flow_angle(F(along.z()), F(along.y()), acos);
        let limit = self.aero.g_limit(
            &Air { alpha, ..self.air },
            &Hold {
                max_g: self.max_g,
                mass: self.mass,
                weight,
                speeding,
                max_elevator: self.joystick.max_elevator,
                pitch_inertia: F(self.body.inv_inertia.x()),
                h,
            },
        );
        let s = &mut self.surfaces;
        for v in [&mut s.elevator_command, &mut s.pitch_integral] {
            if let Some(lo) = limit.elevator.0 {
                *v = v.max(lo);
            }
            if let Some(hi) = limit.elevator.1 {
                *v = v.min(hi);
            }
        }
        self.air.elevator = s.elevator_command;
        let ease = limit.aileron.max(self.max_g_roll);
        if ease < F::ONE {
            self.air.aileron = self.air.aileron * ease;
        }
    }

    /// The post listener: the measure, with the crash test between its halves. `holed` and `bare` are
    /// [`CrashContact`]'s, which only a world of several terrains sets.
    ///
    /// On a world that wraps (MD29), the map edge is asked only on the axes that do not: the plane
    /// crosses a seam as the bike does, and the rest of the crash test is the same.
    fn post(&mut self, dt: F, acos: &AcosTable, holed: bool, bare: bool, wrap: [bool; 2]) {
        let wheels = self.tires.map(|t| t.touching);
        // The aero model's true speed is on while a wheel is down or the collision record touched
        // anything (S193: a wreck tumbling on its body rays has it on, which the chord's first frame
        // back flies on).
        let (touchdown, true_speed) = self.measure.contacts(wheels, self.record.touching);
        // The touchdown sound's pick (P146) draws one `rand()` even from its list of one, before the
        // crash test's: the plane's draws include it.
        if touchdown {
            self.rng.rand();
        }
        self.aero.true_speed = true_speed;
        let m = &self.measure;
        // What the world rays last struck, in the plane's mask of categories the speed rule spares.
        let ignored = self.record.struck.is_some_and(|o| flight_search::category(o) & VEHICLE != 0);
        let c = CrashContact { touching: self.record.touching, ignored, grounded: m.grounded, speed: m.airspeed, up_y: F(m.up.y()), holed, bare };
        let b = &mut self.body;
        let edge = self.area.filter(|a| wrap == [false, false] || holed || crate::off_map::outside_axes(a, wrap, b.position, F(crate::biplane::EDGE)));
        let out = self.crash.step_in(&c, edge.as_ref(), b.position, &mut b.velocity, &mut b.rates, &mut self.edge_scale, &mut self.rng, dt);
        if out.wreck.is_some() {
            self.wreck();
        }
        if out.recover {
            self.unwreck(acos);
        }
        self.remeasure(acos);
    }

    /// The wreck's effect on the plane (S36's `Plane::wreck`): the body spun up and given `[Wrecks]`'
    /// damping, its inertia scaled by `WreckInertia` and its gravity by `WreckGravity`, the throttle
    /// and the engine's note cut (P187: the original zeroes the note the engine sound reads, not the
    /// power, which keeps its last value through the wreck — the engine is not stepped while wrecked).
    fn wreck(&mut self) {
        let w = &self.wrecks;
        let b = &mut self.body;
        b.rates = b.rates.merge(Dest::XYZ, b.rates.mul(Vf::splat(WRECK_SPIN)));
        b.damping = w.wreck_damping;
        b.ang_damping = w.wreck_damping;
        let mut inv = self.inv_inertia;
        for k in 0..3 {
            inv[k] = (F(inv[k]) / F(w.wreck_inertia[k])).to_f32();
        }
        b.inv_inertia = inv;
        self.gravity = gravity_force(w.wreck_gravity, self.mass);
        self.surfaces.throttle = F::ZERO;
        self.engine.note = F::ZERO;
    }

    /// The recovery: everything [`Flight::wreck`] changed put back, and the plane reset to its spawn
    /// with the throttle and the surfaces at zero.
    fn unwreck(&mut self, acos: &AcosTable) {
        if self.crash.reason == THROWN {
            self.edge_scale = self.edge_scale / F(0.5);
        }
        self.crash.reason = 0;
        self.crash.landed = false;
        self.crash.clock = F::ZERO;
        self.body.damping = UNDAMPED;
        self.body.ang_damping = UNDAMPED;
        self.body.inv_inertia = self.inv_inertia;
        self.gravity = gravity_force(self.gravity_scale, self.mass);
        let s = &mut self.surfaces;
        (s.throttle, s.elevator, s.elevator_command, s.aileron, s.rudder) = (F::ZERO, F::ZERO, F::ZERO, F::ZERO, F::ZERO);
        self.put(true, acos);
    }

    /// Where the drawn plane is: the model's orientation and origin as the body stood at the start of
    /// the frame's last substep ([`Body::pose`]).
    pub fn pose(&self) -> (Vf, Vf) {
        self.body.pose()
    }
}

/// The world's up: the spawn's at every spot the port finds itself.
const WORLD_UP: Vf = Vf([0.0, 1.0, 0.0, 0.0]);

/// The spawn's orientation: nose along `facing`, squared to `up` ([`spawn_axes`]).
pub fn spawn_turn(facing: Vf, up: Vf) -> Vf {
    let (facing, up) = spawn_axes(facing, up);
    axes_to_quat(facing, up)
}

/// **How the original's place squares a spawn** (S205, the physics object's place): the side is
/// the nose crossed with the up, the up the side crossed with the nose, and both are normalised by
/// VU0's reciprocal square root. The nose is kept and the up moves: a bay's ground normal that
/// leans across the nose rolls the wings onto the slope, one that leans along it is taken off, so
/// the plane never pitches with the ground. Each cross is the instruction pair with its operands
/// swapped, and its `w` is the nose's (the `.xyz` writes leave the register's).
pub fn spawn_axes(facing: Vf, up: Vf) -> (Vf, Vf) {
    let side = facing.merge(Dest::XYZ, Vf::opmsub(Vf::opmula(facing, up), up, facing));
    let up = side.merge(Dest::XYZ, Vf::opmsub(Vf::opmula(side, facing), facing, side));
    let unit = |v: Vf| v.merge(Dest::XYZ, Vf(v.0).mul_q(vu::rsqrt(1.0, vu::dot3(v.0, v.0))));
    (unit(facing), unit(up))
}

/// The original's axes to a turn: the matrix of a nose and an up (normalised again, not re-crossed)
/// into a quaternion.
fn axes_to_quat(facing: Vf, up: Vf) -> Vf {
    mat4_to_quat(&basis(Vf::new(0.0, 0.0, 0.0, 1.0), facing, up, false, true))
}

/// The plane's model in `machine.res` (the factory's livery 0), whose bounds lift a spawn
/// ([`spawn_lift`]).
pub const BIPLANE_MODEL: &str = "biplane\\anim\\biplane.psg";

/// **Where the original's placement puts a parked plane's body** (S164), `ground` being the ground
/// query's answer at the track's `machine.txt` spot (with `SetGroundHeight`, on by default, the spot
/// moved onto the ground there; its `x` and `z` are the file's) and `lift` [`spawn_lift`] of the
/// plane's model: the spot raised by the height of the model's bounds' centre. The machine factory
/// adds that height to the position after it builds the plane, and the position is the body's — its
/// centre of gravity's — from then on: its spawn transform keeps it and its reset stands the body on
/// it as it is. On free06's `[Biplane1]` the body is at (2304, 138.61406, 2281) with its model's
/// origin 3.5 ft under it and 2.5 ft along, where S143's put-backs stand.
pub fn bay_spot(ground: Vf, lift: F) -> Vf {
    Vf::new(ground.x(), (F(ground.y()) + lift).to_f32(), ground.z(), 1.0)
}

/// How far the machine factory raises a plane's spot (S164): the `y` of its model's bounds' centre
/// as the scene node answers it ([`crate::prop_world::node_bounds`]) — the model's root box widened
/// by every node under it, each placed by its local matrix under its parent's — out of the model's
/// `.psg` bytes ([`BIPLANE_MODEL`]). The rudder's box reaches over the root's, so the answer is
/// not the file's own centre: 3.6154737 ft on the disc's plane, against the root's 3.6148021.
/// The root is taken where the model has it, unplaced: the centre comes back in its frame.
pub fn spawn_lift(psg: &[u8]) -> Result<F, String> {
    use crate::math::{Mat4, mul};
    use crate::prop_world::BoundsNode;
    use crate::props::NodeBox;
    const NODES: usize = 12;
    const RECORD: usize = 224;
    const MATRIX: usize = 128;
    const CENTRE: usize = 192;
    const HALF: usize = 204;
    const PARENT: usize = 220;
    let word = |at: usize| -> Result<u32, String> {
        psg.get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| format!(".psg ends at {} before byte {at}", psg.len()))
    };
    if psg.get(..4) != Some(b"psg\0".as_slice()) {
        return Err("not a .psg: no psg\\0 at the front".into());
    }
    let count = word(8)? as usize;
    let at = |i: usize| NODES + i * RECORD;
    let mut parents = Vec::with_capacity(count);
    for i in 0..count {
        parents.push(word(at(i) + PARENT)? as i32);
    }
    let three = |at: usize| -> Result<Vf, String> { Ok(Vf::from_bits([word(at)?, word(at + 4)?, word(at + 8)?, 1f32.to_bits()])) };
    let local = |i: usize| -> Result<Mat4, String> {
        let row = |r: usize| -> Result<Vf, String> {
            let b = at(i) + MATRIX + r * 16;
            Ok(Vf::from_bits([word(b)?, word(b + 4)?, word(b + 8)?, word(b + 12)?]))
        };
        Ok(Mat4([row(0)?, row(1)?, row(2)?, row(3)?]))
    };
    fn node(
        i: usize,
        parent: Option<&Mat4>,
        parents: &[i32],
        local: &dyn Fn(usize) -> Result<Mat4, String>,
        three: &dyn Fn(usize) -> Result<Vf, String>,
        at: &dyn Fn(usize) -> usize,
    ) -> Result<BoundsNode, String> {
        let world = match parent {
            Some(p) => mul(&local(i)?, p),
            None => local(i)?,
        };
        let mut children = Vec::new();
        for (k, &p) in parents.iter().enumerate() {
            // A node comes after its parent in the file; a later index cannot loop back.
            if p == i as i32 && k > i {
                children.push(node(k, Some(&world), parents, local, three, at)?);
            }
        }
        // The node reader marks every node as having a box, the dummies' empty ones too.
        let own = Some(NodeBox { centre: three(at(i) + CENTRE)?, half_extent: three(at(i) + HALF)? });
        Ok(BoundsNode { own, world, children })
    }
    if count == 0 || parents[0] != -1 {
        return Err("a .psg whose first node is not its root".into());
    }
    let root = node(0, None, &parents, &local, &three, &at)?;
    Ok(F(crate::prop_world::node_bounds(&root).centre.y()))
}

/// Where the plane's entries on the knockable props are named from between its search and the
/// struck bodies' places ([`Flight::frame_props`]): above any vehicle's index.
pub const KNOCK_FROM: usize = 1 << 20;

/// What a wreck bike's mirror on the plane's list names until the plane's frame puts the wreck
/// bike's body among its resolve's other sides (S274, [`Met::wrecks`]): `WRECK_FROM + j`.
pub const WRECK_FROM: usize = 1 << 21;

/// A knockable prop in the plane's search (S241): its collection and object, and its mesh where its
/// node stands it.
struct Posed<'a> {
    collection: usize,
    object: usize,
    knock: flight_search::Knock<'a>,
}

/// Every prop of `set` with a collidable mesh (`shapes`, the set's) and a collision object, in
/// collection and object order, as [`crate::prop_contact::strike`] poses them.
fn knocks<'a>(set: &PropSet, shapes: &'a [Vec<Option<track::Mesh>>]) -> Vec<Posed<'a>> {
    let mut out = Vec::new();
    for (c, (coll, meshes)) in set.collections.iter().zip(shapes).enumerate() {
        for (o, (object, mesh)) in coll.objects.iter().zip(meshes).enumerate() {
            let (Some(mesh), Some(col)) = (mesh.as_ref(), object.collision.as_ref()) else { continue };
            let (to_world, centre, radius) = world::Prop::posed(mesh, &PropSet::matrix(&object.node));
            let velocity = coll.bones[o].body.velocity;
            let capacity = col.record.capacity;
            out.push(Posed { collection: c, object: o, knock: flight_search::Knock { mesh, to_world, centre, radius, velocity, capacity } });
        }
    }
    out
}

/// One wheel off its `biplane.phy` section, at rest.
fn tire(g: &phy::Gear, braked: bool, steered: bool) -> Tire {
    Tire {
        max_compression: g.strut_compression,
        max_decompression_rate: g.max_decompression_rate,
        spring: g.spring_rate,
        bounce: g.bounce_damping,
        rebound: g.rebound_damping,
        center: g.body_tire_center,
        radius: g.tire_radius,
        inv_radius: g.inv_tire_radius,
        // The gear's two braking words land the other way round in the tire: its friction scale
        // in the first and the force in the second (the state holds 0.4 and -500).
        max_braking: g.brake_friction,
        side_force: g.side_force,
        brake_friction: g.max_braking_force,
        steering_moment: g.steering_moment,
        braked,
        steered,
        last_normal: Vf::new(0.0, 1.0, 0.0, 1.0),
        ..Tire::default()
    }
}

/// The world's ground for the tires, which ask a sized one.
struct Ground<'a>(&'a mut dyn world::World);

impl world::World for Ground<'_> {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        self.0.ground(at)
    }
    fn surface(&mut self, at: Vf) -> u8 {
        self.0.surface(at)
    }
    fn solid(&mut self, bundle: &Bundle, record: &mut Record) -> bool {
        self.0.solid(bundle, record)
    }
}

/// A heightfield's box on the ground plane, (x, z) low and high.
fn field_box(f: &(dyn contact_search::Heightfield + '_)) -> ([F; 2], [F; 2]) {
    let (lo, hi) = (f.min(), f.max());
    ([F(lo.x()), F(lo.z())], [F(hi.x()), F(hi.z())])
}

/// The plane's lists in the dynamics world: one body, one listener of each kind, no constraints.
struct Listeners<'a> {
    plane: &'a mut Flight,
    ground: Ground<'a>,
    tables: &'a ride::Tables,
    bikes: Option<Bikes<'a>>,
    /// The movers the search's contacts name (S145).
    movers: &'a [contact::Mover],
    /// The riders' bikes the mirrors on its list name (N22a).
    met: Option<&'a mut Met>,
    /// The widths the post moved the plane over a seam (MD44).
    seam: [i8; 2],
    /// The knockable props' bodies its list names (S241), after the vehicles among its other sides.
    props: &'a mut [crate::prop_contact::Pushed],
    /// The wreck bikes in its world (F32), bodies `1..` of its lists, their bodies among `props`.
    downed: Vec<crate::wreck_plane::InWorld>,
}

thread_local! {
    static SUBSTEPS: std::cell::RefCell<Option<Vec<Body>>> = const { std::cell::RefCell::new(None) };
}

/// **A lone plane's body at the end of each substep** of whatever `run` flies on this thread (S274),
/// in order, with `run`'s answer: where a capture row the original's PINE read inside a frame's step
/// stands (C16's row 137911 holds the plane after the first of 137910's two). Play never asks, and
/// keeps nothing. Planes flown together ([`frames_together`]) are not kept.
pub fn substeps<R>(run: impl FnOnce() -> R) -> (R, Vec<Body>) {
    let was = SUBSTEPS.replace(Some(Vec::new()));
    let r = run();
    let kept = SUBSTEPS.replace(was).unwrap_or_default();
    (r, kept)
}

impl Dynamics for Listeners<'_> {
    fn count(&self, list: List) -> i32 {
        match list {
            List::Constraints => 0,
            _ => 1 + self.downed.len() as i32,
        }
    }
    fn preforce(&mut self, i: i32, dt: F) {
        if i > 0 {
            let d = &mut self.downed[i as usize - 1];
            d.preforce(&mut self.props[d.place]);
            return;
        }
        self.plane.preforce(dt);
    }
    fn force(&mut self, i: i32, h: F) {
        if i > 0 {
            // A wreck bike's, after the plane's: the plane's body on its resolve.
            let d = &mut self.downed[i as usize - 1];
            let from = d.bike.origin();
            let mut flown: Vec<_> = d.planes.iter().map(|&k| self.plane.flown(k, from, &*self.ground.0)).collect();
            d.force(&mut self.props[d.place], &mut flown);
            for f in &flown {
                self.plane.pushed(f);
            }
            return;
        }
        let mut bodies: Vec<(&mut Collider, Weighed<'_>)> = self.props.iter_mut().map(|p| p.weighed()).collect();
        let mut others: Vec<contact::Other<'_>> = match self.met.as_deref_mut() {
            Some(m) => m
                .colliders
                .iter_mut()
                .zip(m.vehicles.iter_mut())
                .map(|(collider, v)| contact::Other { collider, side: contact::Side::Vehicle(v) })
                .collect(),
            None => Vec::new(),
        };
        others.extend(bodies.iter_mut().map(|(collider, body)| contact::Other { collider: &mut **collider, side: contact::Side::Body(body) }));
        self.plane.forces(h, &mut self.ground, &self.tables.curve, &self.tables.acos, self.movers, &mut others);
    }
    fn enabled(&self, i: i32) -> bool {
        match i {
            0 => self.plane.body.enabled,
            _ => self.props[self.downed[i as usize - 1].place].body.enabled,
        }
    }
    fn integrate(&mut self, i: i32, h: F) {
        if i > 0 {
            self.props[self.downed[i as usize - 1].place].body.integrate(h);
            return;
        }
        self.plane.body.integrate(h);
    }
    fn end_substep(&mut self, i: i32) {
        if i > 0 {
            self.props[self.downed[i as usize - 1].place].body.end_substep();
            return;
        }
        self.plane.body.end_substep();
        SUBSTEPS.with_borrow_mut(|kept| {
            if let Some(kept) = kept {
                kept.push(self.plane.body);
            }
        });
    }
    fn constrain(&mut self, _: i32, _: bool) {}
    fn post(&mut self, i: i32, dt: F) {
        if i > 0 {
            let d = &mut self.downed[i as usize - 1];
            d.post(&self.props[d.place]);
            return;
        }
        // Over a seam (MD44), first: the refill gathers where the plane now is.
        let seam = self.plane.wrap_round(&*self.ground.0, &self.tables.acos);
        self.seam = [self.seam[0] + seam[0], self.seam[1] + seam[1]];
        self.plane.refresh(&mut *self.ground.0, self.bikes);
        // A world of several terrains' holes and bare sky (MD14), where the plane stands now.
        let at = self.plane.body.position;
        let holed = self.plane.area.is_some() && self.ground.0.off_world(at, F(crate::biplane::EDGE));
        let bare = !self.ground.0.keeps(at) && at.y() < 0.0;
        let wrap = self.ground.0.wrap();
        self.plane.post(dt, &self.tables.acos, holed, bare, wrap);
    }
}

/// The plane's input step on the player's pad: I6's [`PlaneInput::from_pad`], into the five fields
/// the plane keeps it in (the two throttle commands, then the elevator, the aileron and the rudder,
/// in the order the original stores them).
pub fn pad(pad: &input::Pad) -> Pad {
    let p = PlaneInput::from_pad(pad);
    Pad { forward: p.throttle_up, back: p.throttle_down, elevator: p.elevator, aileron: p.aileron, rudder: p.rudder }
}

/// The body under the names a capture of the original gives its four quads (`pos`, `vel`, `quat`,
/// `rate`), then the rest in declaration order.
pub(crate) struct BodyWalk<'a>(pub(crate) &'a Body);

impl State for BodyWalk<'_> {
    fn walk(&self, w: &mut Walk<'_>) {
        let b = self.0;
        w.vf("pos", b.position);
        w.vf("vel", b.velocity);
        w.vf("quat", b.quat);
        w.vf("rate", b.rates);
        w.bool("enabled", b.enabled);
        w.f("inv_mass", b.inv_mass);
        for (name, v) in [
            ("damping", b.damping),
            ("last_position", b.last_position),
            ("last_force", b.last_force),
            ("force", b.force),
            ("accel", b.accel),
            ("shock_peak", b.shock_peak),
            ("shock", b.shock),
            ("step", b.step),
            ("inv_inertia", b.inv_inertia),
            ("ang_damping", b.ang_damping),
            ("last_quat", b.last_quat),
            ("cofg", b.cofg),
            ("moment", b.moment),
            ("last_moment", b.last_moment),
            ("ang_accel", b.ang_accel),
            ("twist_peak", b.twist_peak),
            ("twist", b.twist),
        ] {
            w.vf(name, v);
        }
    }
}

/// What the measure keeps between frames.
struct MeasureWalk<'a>(&'a Measure);

impl State for MeasureWalk<'_> {
    fn walk(&self, w: &mut Walk<'_>) {
        let m = self.0;
        w.bool("grounded", m.grounded);
        w.bool("was_grounded", m.was_grounded);
        w.f("airspeed", m.airspeed);
        w.f("alpha", m.alpha);
        w.f("beta", m.beta);
        for (name, v) in [
            ("world_rates", m.world_rates),
            ("velocity", m.velocity),
            ("forward", m.forward),
            ("up", m.up),
            ("euler", m.euler),
            ("matrix0", m.matrix.0[0]),
            ("matrix1", m.matrix.0[1]),
            ("matrix2", m.matrix.0[2]),
            ("matrix3", m.matrix.0[3]),
        ] {
            w.vf(name, v);
        }
    }
}

/// One wheel's state (its parameters are the file's and are not walked).
struct TireWalk<'a>(&'a Tire);

impl State for TireWalk<'_> {
    fn walk(&self, w: &mut Walk<'_>) {
        let t = self.0;
        w.bool("below", t.below);
        w.bool("held", t.held);
        w.bool("touching", t.touching);
        w.u32("surface", u32::from(t.surface));
        for (name, v) in [
            ("compression", t.compression),
            ("last_compression", t.last_compression),
            ("spring_force", t.spring_force),
            ("angle", t.angle),
            ("spin", t.spin),
        ] {
            w.f(name, v);
        }
        for (name, v) in [
            ("offset", t.offset),
            ("bottom", t.bottom),
            ("low", t.low),
            ("ground", t.ground),
            ("normal", t.normal),
            ("contact", t.contact),
            ("hub", t.hub),
            ("contact_velocity", t.contact_velocity),
            ("last_normal", t.last_normal),
            ("slip", t.slip),
            ("friction", t.friction),
            ("frame0", t.frame[0]),
            ("frame1", t.frame[1]),
            ("frame2", t.frame[2]),
        ] {
            w.vf(name, v);
        }
    }
}

impl State for Flight {
    /// The state, in declaration order, parameters included: they are a handful of words, and a
    /// plane built off a different file is a different ride from its first tick.
    fn walk(&self, w: &mut Walk<'_>) {
        let j = &self.joystick;
        w.group("joystick", |w| {
            for (name, v) in [
                ("throttle_increment", j.throttle_increment),
                ("reverse_throttle", j.reverse_throttle),
                ("aileron_rate", j.aileron_rate),
                ("elevator_rate", j.elevator_rate),
                ("rudder_rate", j.rudder_rate),
                ("max_elevator", j.max_elevator),
                ("max_aileron", j.max_aileron),
                ("max_rudder", j.max_rudder),
                ("ramp_off_roll", j.ramp_off_roll),
                ("off_roll", j.off_roll),
                ("min_elevator", j.min_elevator),
            ] {
                w.f(name, v);
            }
        });
        let p = &self.autopilot;
        w.group("autopilot", |w| {
            for (name, v) in [
                ("aileron_dead_zone", p.aileron_dead_zone),
                ("max_pitch_rate", p.max_pitch_rate),
                ("max_roll", p.max_roll),
                ("feed_forward", p.feed_forward),
                ("gain", p.gain),
                ("integrator_gain", p.integrator_gain),
            ] {
                w.f(name, v);
            }
        });
        let e = &self.engine;
        w.group("engine", |w| {
            for (name, v) in [
                ("min_efficiency", e.min_efficiency),
                ("full_efficiency_speed", e.full_efficiency_speed),
                ("prop_rpm", e.prop_rpm),
                ("prop_angle", e.prop_angle),
                ("max_power", e.max_power),
                ("fade_altitude", e.fade_altitude),
                ("max_altitude", e.max_altitude),
                ("power", e.power),
                ("rpm", e.rpm),
                ("idle_rpm", e.idle_rpm),
                ("max_rpm", e.max_rpm),
                ("thrust", e.thrust),
                ("note", e.note),
                ("clock", e.clock),
            ] {
                w.f(name, v);
            }
        });
        let a = &self.aero;
        w.group("aero", |w| {
            w.bool("true_speed", a.true_speed);
            for (name, v) in [
                ("density", a.density),
                ("chord", a.chord),
                ("span", a.span),
                ("area", a.area),
                ("default_alpha", a.default_alpha),
                ("cm_break_alpha", a.cm_break_alpha),
                ("lift_alpha", a.lift_alpha),
                ("lift_round_off", a.lift_round_off),
                ("lift_drop_off", a.lift_drop_off),
                ("lift_scale", a.lift_scale),
                ("lift_angle_scale", a.lift_angle_scale),
                ("lift_q", a.lift_q),
                ("lift_de", a.lift_de),
                ("drag0", a.drag0),
                ("drag1", a.drag1),
                ("drag_de", a.drag_de),
                ("side_beta", a.side_beta),
                ("side_da", a.side_da),
                ("side_dr", a.side_dr),
                ("side_p", a.side_p),
                ("side_r", a.side_r),
                ("cl_beta", a.cl_beta),
                ("cl_p", a.cl_p),
                ("cl_r", a.cl_r),
                ("cl_da", a.cl_da),
                ("cl_dr", a.cl_dr),
                ("cm0", a.cm0),
                ("cm_alpha", a.cm_alpha),
                ("cm_q", a.cm_q),
                ("cm_de", a.cm_de),
                ("cm_stall", a.cm_stall),
                ("cn_beta", a.cn_beta),
                ("cn_p", a.cn_p),
                ("cn_r", a.cn_r),
                ("cn_dr", a.cn_dr),
                ("cn_da", a.cn_da),
                ("lift", a.lift),
                ("drag", a.drag),
                ("side", a.side),
                ("roll", a.roll),
                ("pitch", a.pitch),
            ] {
                w.f(name, v);
            }
        });
        w.field("body", &BodyWalk(&self.body));
        w.field("measure", &MeasureWalk(&self.measure));
        for (name, t) in ["tire0", "tire1", "tire2"].into_iter().zip(&self.tires) {
            w.field(name, &TireWalk(t));
        }
        // Only a plane the acceleration modifier brakes harder has these (P116), so the file's plane
        // walks, and hashes, as before.
        if self.cg_braking.iter().any(|&c| c != F::ZERO) {
            w.group("cg_braking", |w| {
                for (name, c) in ["tire0", "tire1", "tire2"].into_iter().zip(self.cg_braking) {
                    w.f(name, c);
                }
            });
        }
        // And only a plane the max G modifier holds has this (P137).
        if self.max_g != F::ZERO {
            w.f("max_g", self.max_g);
            w.f("max_g_roll", self.max_g_roll);
        }
        let pad = &self.pad;
        w.group("pad", |w| {
            w.f("forward", pad.forward);
            w.f("back", pad.back);
            w.f("elevator", pad.elevator);
            w.f("aileron", pad.aileron);
            w.f("rudder", pad.rudder);
        });
        let s = &self.surfaces;
        w.group("surfaces", |w| {
            w.f("throttle", s.throttle);
            w.f("brake", s.brake);
            w.f("elevator", s.elevator);
            w.f("elevator_command", s.elevator_command);
            w.f("aileron", s.aileron);
            w.f("rudder", s.rudder);
            w.f("steer", s.steer);
            w.f("pitch_integral", s.pitch_integral);
        });
        let at = &self.attitude;
        w.group("attitude", |w| {
            w.bool("flown", at.flown);
            w.bool("grounded", at.grounded);
            w.f("nose_y", at.nose_y);
            w.f("roll", at.roll);
            w.f("speed", at.speed);
            w.f("pitch_rate", at.pitch_rate);
        });
        let air = &self.air;
        w.group("air", |w| {
            w.f("alpha", air.alpha);
            w.f("beta", air.beta);
            w.f("speed", air.speed);
            w.f("elevator", air.elevator);
            w.f("aileron", air.aileron);
            w.f("rudder", air.rudder);
            w.vf("rates", air.rates);
        });
        w.u32("crash", self.crash.reason);
        w.f("crash_time", self.crash.clock);
        w.bool("crash_landed", self.crash.landed);
        w.vf("gravity", self.gravity);
        w.f("edge_scale", self.edge_scale);
        w.bool("noise", self.noise.is_some());
        w.u64("rng", self.rng.state());
        // The collision object (S48), only on a plane that has one, so a plane without a ray probe
        // walks as it did: what the resolve and the search leave from one frame to the next. Its
        // tuning is the file's, and the ray probe's geometry the disc's.
        if self.rays.is_some() {
            let (c, r) = (&self.collider, &self.record);
            w.group("collision", |w| {
                w.f("normal_speed", c.normal_speed);
                w.vf("relative", c.relative);
                w.vf("cofg_world", c.cofg_world);
                w.bool("touching", r.touching);
                w.bool("on_ground", r.on_ground);
                w.bool("on_object", r.on_object);
                w.bool("touching_other", r.touching_other);
                w.vf("swept_from", r.swept_from);
                w.vf("swept_to", r.swept_to);
                for (k, row) in r.node.rows.iter().enumerate() {
                    w.vf(["row0", "row1", "row2"][k], *row);
                }
                w.vf("origin", r.node.origin);
                w.u32("contacts", r.contacts.len() as u32);
                w.bool("struck", r.struck.is_some());
                w.u32("struck_object", r.struck.map_or(0, |o| o as u32));
                let n = &self.nearby;
                w.bool("nearby_built", n.built);
                for (k, c) in n.centre.iter().enumerate() {
                    w.f(["nearby_x", "nearby_y", "nearby_z", "nearby_w"][k], *c);
                }
                w.f("nearby_bound", n.bound);
                w.u32("nearby_found", n.nearby.found.len() as u32);
            });
        }
    }
}

/// **One of the ride's planes in a joint frame** (N22a1, [`frames_together`]): the plane, the pad
/// its input step stores after the step, and the riders' bikes it meets ([`Met`]).
pub struct Fly<'a> {
    pub flight: &'a mut Flight,
    pub pad: Pad,
    pub met: Option<&'a mut Met>,
    /// The other players' bikes in its world ([`Flight::frame_among`]): its own seat's not among them.
    pub bikes: &'a [flight_search::Bike],
}

/// **The ride's planes stepped together** (N22a1): what the original's level node does with both
/// players' planes, whose collision objects are records on it and whose bodies are in one dynamics
/// world. Every plane's record is emptied first; then each plane searches in `fleet`'s order with
/// the others' hulls in its world ([`flight_search::Scene::among_planes`]), so a hit on another plane
/// names it and files the mirror on its record — after its own entries if it searched already, ahead
/// of them if it has yet to; then the bikes' mirrors follow on each ([`Met::mirrors`], as
/// [`Flight::frame_met`] puts them). One world step steps every body: each substep each plane's
/// force listener resolves its list against the bikes and against the other planes' bodies, which
/// it pushes ([`contact::Side::Body`]), in turn, so their resolves interleave substep by substep;
/// then each plane's post in turn. An entry on plane `i`'s list names plane `j` as its met's
/// vehicles' count plus `j`'s place among the planes but `i`.
///
/// With one plane it is [`Flight::frame_met`]. Answers each plane's [`Flight::frame_seamed`].
pub fn frames_together(
    dt: F,
    fleet: &mut [Fly<'_>],
    ground: &mut dyn world::World,
    tables: &ride::Tables,
) -> Vec<(Vec<Option<u32>>, [i8; 2])> {
    frames_together_props(dt, fleet, ground, tables, None)
}

/// **[`frames_together`] among the knockable props** (S259a): each plane's frame as
/// [`Flight::frame_props`] has a lone one's, plane `k` named `me[k]` in its mirrors (its seat, its
/// place among the props' step's strikers).
///
/// 1. every plane's search has the props after the world and after the other planes' hulls
///    ([`Scene::knocking`]), each prop's record after the planes': its entries name the prop's
///    record, its mirrors go on the prop's list;
/// 2. then, plane by plane in `fleet`'s order after the bikes' mirrors, each prop it struck woken
///    and its mirrors filed at the head of its list ([`PropSet::strike`]), and the props' rays on its
///    hull (step 3 of [`Flight::frame_props`]);
/// 3. one world step, each plane's resolve pushing the props its list names after the bikes and the
///    other planes among its sides. The struck bodies are one list for the fleet, so a prop two
///    planes strike is one body both push, substep by substep; written back after
///    ([`PropSet::pushed`]).
///
/// The props' own step is the caller's, after this. `None` is [`frames_together`].
pub fn frames_together_props(
    dt: F,
    fleet: &mut [Fly<'_>],
    ground: &mut dyn world::World,
    tables: &ride::Tables,
    props: Option<(&mut PropSet, &[usize])>,
) -> Vec<(Vec<Option<u32>>, [i8; 2])> {
    let shapes = props.as_ref().map(|(set, _)| Arc::clone(&set.shapes));
    let posed = match (&props, &shapes) {
        (Some((set, _)), Some(shapes)) => knocks(set, shapes),
        _ => Vec::new(),
    };
    let knocks: Vec<flight_search::Knock<'_>> = posed.iter().map(|p| p.knock).collect();
    let searched = search_together(dt, fleet, ground, &knocks);
    for f in fleet.iter_mut() {
        let p = &mut *f.flight;
        if let Some(m) = f.met.as_deref().filter(|m| !m.mirrors.is_empty() && p.rays.is_some()) {
            let r = &mut p.record;
            (r.touching, r.on_object, r.touching_other) = (true, true, true);
            let room = r.capacity.saturating_sub(r.contacts.len());
            r.contacts.extend(m.mirrors.iter().copied().take(room));
        }
    }
    let n = fleet.len();
    let (mut struck, mut movers, mut knocked) = (Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n));
    for (s, m, k) in searched {
        struck.push(s);
        movers.push(m);
        knocked.push(k);
    }
    let mut bodies: Vec<crate::prop_contact::Pushed> = Vec::new();
    let mut props = props;
    if let Some((set, me)) = props.as_mut() {
        for (i, (f, k)) in fleet.iter_mut().zip(knocked).enumerate() {
            // Its resolve's sides: its met's vehicles, the other planes, then the struck bodies.
            let first = f.met.as_deref().map_or(0, |m| m.vehicles.len()) + n - 1;
            f.flight.knocked(set, me[i], first, &posed, k, &mut bodies);
        }
    }
    // The downed riders' wreck bikes (S276a, [`Met::wrecks`]) after the props: one body each for the
    // fleet, named by its `object`, so a wreck bike under two planes is one body both push, substep
    // by substep; each plane's mirrors naming [`WRECK_FROM`]` + j` renamed to its place.
    let at = bodies.len();
    let mut wrecked: Vec<(usize, Vec<usize>)> = Vec::new();
    for (i, f) in fleet.iter_mut().enumerate() {
        let Some(m) = f.met.as_deref_mut().filter(|m| !m.wrecks.is_empty()) else { continue };
        let first = m.vehicles.len() + n - 1;
        let places: Vec<usize> = std::mem::take(&mut m.wrecks)
            .into_iter()
            .map(|w| match bodies[at..].iter().position(|b| b.object == w.object) {
                Some(p) => at + p,
                None => {
                    bodies.push(w);
                    bodies.len() - 1
                }
            })
            .collect();
        for e in &mut f.flight.record.contacts {
            if let Some(j) = e.other.filter(|&o| e.kind == 1 && o >= WRECK_FROM).map(|o| o - WRECK_FROM) {
                e.other = places.get(j).map(|&p| first + p);
            }
        }
        wrecked.push((i, places));
    }
    // The wreck bikes stepped in the planes' world (F32): each its body's place among the bodies, and
    // its planes by their place in the fleet (the search's planes are the fleet's with a met).
    let with_met: Vec<usize> = (0..n).filter(|&i| fleet[i].met.is_some()).collect();
    let mut downed = Vec::new();
    let mut kept = Vec::new();
    for (i, places) in &wrecked {
        let Some(m) = fleet[*i].met.as_deref_mut() else { continue };
        for mut d in std::mem::take(&mut m.downed) {
            kept.push((*i, d.place, std::mem::take(&mut d.planes)));
            d.place = places.get(d.place).copied().unwrap_or(usize::MAX);
            d.planes = kept.last().map(|k| k.2.iter().filter_map(|&k| with_met.get(k).copied()).collect()).unwrap_or_default();
            downed.push(d);
        }
    }
    let hulls = fleet.iter().map(|f| f.flight.hull.as_ref().filter(|_| f.flight.rays.is_some()).map(|_| hull_at(f.flight))).collect();
    let mut listeners = Together {
        fleet: &mut *fleet,
        ground: Ground(ground),
        tables,
        movers: &movers,
        hulls,
        seams: Vec::new(),
        props: &mut bodies,
        downed: &mut downed,
    };
    listeners.seams = vec![[0, 0]; listeners.fleet.len()];
    WORLD.step(dt, &mut listeners);
    let seams = listeners.seams;
    // Each plane's met has its wreck bikes' bodies back, as the joint resolve left them, and the
    // wreck bikes its search filed there, stepped.
    for (i, places) in wrecked {
        if let Some(m) = fleet[i].met.as_deref_mut() {
            m.wrecks = places.iter().map(|&p| bodies[p].clone()).collect();
        }
    }
    for (mut d, (i, place, planes)) in downed.into_iter().zip(kept) {
        (d.place, d.planes) = (place, planes);
        if let Some(m) = fleet[i].met.as_deref_mut() {
            m.downed.push(d);
        }
    }
    bodies.truncate(at);
    if let Some((set, _)) = props {
        set.pushed(&bodies);
    }
    for f in fleet.iter_mut() {
        if f.flight.attitude.flown {
            f.flight.pad = f.pad;
        }
    }
    struck.into_iter().zip(seams).collect()
}

/// [`frames_together`]' searches alone, for a test to set against the original's: each plane's list
/// as its force listener would resolve it, but for the bikes' mirrors ([`Met::mirrors`]). Answers
/// what each plane's bikes' struck listeners kept.
pub fn searches_together(
    dt: F,
    fleet: &mut [Fly<'_>],
    world: &mut dyn world::World,
) -> Vec<Vec<Option<u32>>> {
    search_together(dt, fleet, world, &[]).into_iter().map(|(struck, ..)| struck).collect()
}

/// Where a plane's hull stands: its collision node's matrix ([`Flight::hull`]).
fn hull_at(p: &Flight) -> crate::math::Mat4 {
    let node = node_of(&p.body);
    crate::math::Mat4([node.rows[0], node.rows[1], node.rows[2], node.origin])
}

/// What plane `i`'s list calls plane `j` among its resolve's other planes: its place among the
/// planes but `i`.
fn rank(i: usize, j: usize) -> usize {
    if j < i { j } else { j - 1 }
}

/// [`frames_together`]' searches ([`Flight::search`] for each plane, the others' hulls in its world),
/// with `knocks` (S259a, [`frames_together_props`]) the knockable props after the world, each one's
/// record after the other planes': a plane's entry on prop `k` names [`KNOCK_FROM`]` + k`, and the
/// answer's third list is, for each prop, the mirrors that plane's search filed on its record.
#[allow(clippy::type_complexity)]
fn search_together(
    dt: F,
    fleet: &mut [Fly<'_>],
    world: &mut dyn world::World,
    knocks: &[flight_search::Knock<'_>],
) -> Vec<(Vec<Option<u32>>, Vec<contact::Mover>, Vec<Vec<contact::Contact>>)> {
    let n = fleet.len();
    for f in fleet.iter_mut() {
        let p = &mut *f.flight;
        if p.rays.is_some() {
            p.record.node = node_of(&p.body);
            contact_search::clear(&mut p.record, Some(&mut p.body));
        }
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mesh = fleet[i].flight.bike.clone();
        let bikes = fleet[i].bikes;
        let near_bikes = mesh.as_deref().map(|mesh| Bikes { mesh, bikes });
        let none = vec![None; near_bikes.map_or(0, |b| b.bikes.len())];
        let Some(rays) = fleet[i].flight.rays.clone() else {
            out.push((none, Vec::new(), Vec::new()));
            continue;
        };
        let from = fleet[i].flight.body.position;
        let seen: Vec<usize> = (0..n).filter(|&j| j != i && fleet[j].flight.rays.is_some() && fleet[j].flight.hull.is_some()).collect();
        let hulls: Vec<(Arc<track::Mesh>, crate::math::Mat4, Vf)> = seen
            .iter()
            .map(|&j| {
                let p = &*fleet[j].flight;
                let mut at = hull_at(p);
                at.0[3] = crate::nearest_copy(&*world, from, at.0[3]);
                (p.hull.clone().expect("seen planes have hulls"), at, p.body.velocity)
            })
            .collect();
        let planes: Vec<flight_search::Plane<'_>> = hulls.iter().map(|(mesh, at, velocity)| flight_search::Plane { mesh, at: *at, velocity: *velocity }).collect();
        let bases: Vec<usize> = (0..n).map(|j| fleet[j].met.as_deref().map_or(0, |m| m.vehicles.len())).collect();
        let mut theirs: Vec<contact_search::Record> = seen.iter().map(|&j| std::mem::take(&mut fleet[j].flight.record)).collect();
        let Fly { flight: me, met, .. } = &mut fleet[i];
        let me = &mut **me;
        let (field, props, trees) = flight_search::near_world(world, &rays, &me.record.node, &mut me.nearby);
        let bounds = field.as_deref().map(field_box);
        let met = met.as_deref_mut().filter(|m| m.named.is_some() && near_bikes.is_some_and(|b| b.bikes.len() == m.bikes.len()));
        let velocities: Vec<Vf> = met.as_deref().map_or(Vec::new(), |m| m.bikes.iter().map(|&p| m.vehicles[p].velocity).collect());
        let first = 1 + velocities.len();
        let mut scene = Scene::among_planes(props, trees, near_bikes, &planes, first, bounds, &rays, &me.record.node, &mut me.nearby);
        if met.is_some() {
            scene = scene.owned(&velocities);
        }
        let knocks_from = first + seen.len();
        if !knocks.is_empty() {
            scene = scene.knocking(knocks, knocks_from);
        }
        let mine = me.record.contacts.len();
        let mut records = vec![std::mem::take(&mut me.record)];
        records.extend(velocities.iter().map(|_| contact_search::Record { capacity: crate::rider_contact::CAPACITY, ..contact_search::Record::default() }));
        let before: Vec<usize> = theirs.iter().map(|r| r.contacts.len()).collect();
        records.append(&mut theirs);
        records.extend(knocks.iter().map(|k| contact_search::Record { capacity: k.capacity, ..contact_search::Record::default() }));
        contact_search::collide(&mut records, 0, Some(&mut me.body), field.as_deref(), &mut scene, dt);
        let movers = contact_search::Search::movers(&scene);
        let struck = std::mem::take(&mut scene.struck);
        drop(scene);
        let knocked: Vec<Vec<contact::Contact>> = records.split_off(knocks_from).into_iter().map(|r| r.contacts).collect();
        let mut theirs = records.split_off(first);
        let mut records = records.into_iter();
        me.record = records.next().unwrap_or_default();
        let mut met = met;
        if let Some(m) = met.as_deref_mut() {
            let k = m.vehicles.len();
            m.struck = vec![Vec::new(); k];
            m.pushed = vec![false; k];
        }
        // The plane's own entries (after the mirrors the planes searched before it left) name the
        // bikes by player and the planes after them; its mirrors on the others name it as their
        // lists do.
        for e in &mut me.record.contacts[mine..] {
            match e.other {
                Some(o) if o >= knocks_from => e.other = Some(KNOCK_FROM + o - knocks_from),
                Some(o) if o >= first => e.other = Some(bases[i] + rank(i, seen[o - first])),
                Some(o) => {
                    if let Some(m) = met.as_deref_mut() {
                        let p = m.bikes[o - 1];
                        e.other = Some(p);
                        m.pushed[p] = true;
                    }
                }
                None => {}
            }
        }
        if let Some(m) = met {
            for (bike, &p) in records.zip(&m.bikes) {
                m.struck[p] = bike.contacts.into_iter().map(|e| contact::Contact { other: m.named, ..e }).collect();
            }
        }
        for ((r, &j), &b) in theirs.iter_mut().zip(&seen).zip(&before) {
            for e in &mut r.contacts[b..] {
                e.other = Some(bases[j] + rank(j, i));
            }
        }
        for (r, &j) in theirs.drain(..).zip(&seen) {
            fleet[j].flight.record = r;
        }
        out.push((struck, movers, knocked));
    }
    out
}

/// [`frames_together`]' planes in one dynamics world: a body and a listener of each kind apiece.
struct Together<'a, 'b> {
    fleet: &'a mut [Fly<'b>],
    ground: Ground<'a>,
    tables: &'a ride::Tables,
    movers: &'a [Vec<contact::Mover>],
    /// Where each plane's hull stands for the others' refills: as the frame's search placed it, until
    /// its own post moves it.
    hulls: Vec<Option<crate::math::Mat4>>,
    seams: Vec<[i8; 2]>,
    /// The knockable props' bodies the planes struck (S259a), after the other planes among each
    /// plane's sides.
    props: &'a mut [crate::prop_contact::Pushed],
    /// The wreck bikes in the planes' world (F32), bodies after the fleet's, their bodies among
    /// `props`.
    downed: &'a mut [crate::wreck_plane::InWorld],
}

impl Dynamics for Together<'_, '_> {
    fn count(&self, list: List) -> i32 {
        match list {
            List::Constraints => 0,
            _ => (self.fleet.len() + self.downed.len()) as i32,
        }
    }
    fn preforce(&mut self, i: i32, dt: F) {
        if let Some(d) = self.downed.get_mut((i as usize).wrapping_sub(self.fleet.len())) {
            d.preforce(&mut self.props[d.place]);
            return;
        }
        self.fleet[i as usize].flight.preforce(dt);
    }
    fn force(&mut self, i: i32, h: F) {
        let i = i as usize;
        if let Some(d) = self.downed.get_mut(i.wrapping_sub(self.fleet.len())) {
            // A wreck bike's, after every plane's: their bodies on its resolve.
            let from = d.bike.origin();
            let world = &*self.ground.0;
            let mut flown: Vec<_> = d.planes.iter().map(|&k| self.fleet[k].flight.flown(k, from, world)).collect();
            d.force(&mut self.props[d.place], &mut flown);
            for (&k, f) in d.planes.iter().zip(&flown) {
                self.fleet[k].flight.pushed(f);
            }
            return;
        }
        let (before, rest) = self.fleet.split_at_mut(i);
        let (me, after) = rest.split_first_mut().expect("a plane of the fleet");
        let from = me.flight.body.position;
        // Another plane's body where the copy nearest this one is, on a world that wraps; put back
        // where it is after the resolve.
        let mut kept = Vec::new();
        for f in before.iter_mut().chain(after.iter_mut()) {
            let b = &mut f.flight.body;
            kept.push((b.position, b.last_position, f.flight.collider.cofg_world));
            let near = crate::nearest_copy(&*self.ground.0, from, b.position);
            if near != b.position {
                let by = near.sub(b.position);
                b.position = near;
                b.last_position = b.last_position.add(by);
                f.flight.collider.cofg_world = f.flight.collider.cofg_world.add(by);
            }
        }
        let mut planes: Vec<(&mut Collider, Weighed<'_>)> = before
            .iter_mut()
            .chain(after.iter_mut())
            .map(|f| {
                let p = &mut *f.flight;
                (&mut p.collider, Weighed { body: &mut p.body, mass: p.mass })
            })
            .collect();
        let Fly { flight, met, .. } = me;
        let mut others: Vec<contact::Other<'_>> = match met.as_deref_mut() {
            Some(m) => m
                .colliders
                .iter_mut()
                .zip(m.vehicles.iter_mut())
                .map(|(collider, v)| contact::Other { collider, side: contact::Side::Vehicle(v) })
                .collect(),
            None => Vec::new(),
        };
        others.extend(planes.iter_mut().map(|(collider, body)| contact::Other { collider: &mut **collider, side: contact::Side::Body(body) }));
        let mut bodies: Vec<(&mut Collider, Weighed<'_>)> = self.props.iter_mut().map(|p| p.weighed()).collect();
        others.extend(bodies.iter_mut().map(|(collider, body)| contact::Other { collider: &mut **collider, side: contact::Side::Body(body) }));
        flight.forces(h, &mut self.ground, &self.tables.curve, &self.tables.acos, &self.movers[i], &mut others);
        drop(others);
        drop(bodies);
        drop(planes);
        for (f, (position, last, cofg)) in before.iter_mut().chain(after.iter_mut()).zip(kept) {
            let b = &mut f.flight.body;
            if b.position != position {
                (b.position, b.last_position) = (position, last);
                f.flight.collider.cofg_world = cofg;
            }
        }
    }
    fn enabled(&self, i: i32) -> bool {
        match self.downed.get((i as usize).wrapping_sub(self.fleet.len())) {
            Some(d) => self.props[d.place].body.enabled,
            None => self.fleet[i as usize].flight.body.enabled,
        }
    }
    fn integrate(&mut self, i: i32, h: F) {
        match self.downed.get((i as usize).wrapping_sub(self.fleet.len())) {
            Some(d) => self.props[d.place].body.integrate(h),
            None => self.fleet[i as usize].flight.body.integrate(h),
        }
    }
    fn end_substep(&mut self, i: i32) {
        match self.downed.get((i as usize).wrapping_sub(self.fleet.len())) {
            Some(d) => self.props[d.place].body.end_substep(),
            None => self.fleet[i as usize].flight.body.end_substep(),
        }
    }
    fn constrain(&mut self, _: i32, _: bool) {}
    fn post(&mut self, i: i32, dt: F) {
        let i = i as usize;
        if let Some(d) = self.downed.get_mut(i.wrapping_sub(self.fleet.len())) {
            d.post(&self.props[d.place]);
            return;
        }
        let plane = &mut *self.fleet[i].flight;
        let seam = plane.wrap_round(&*self.ground.0, &self.tables.acos);
        self.seams[i] = [self.seams[i][0] + seam[0], self.seams[i][1] + seam[1]];
        // The refill, with the other planes' hulls where they stand now.
        let fleet = &*self.fleet;
        let from = fleet[i].flight.body.position;
        let world = &*self.ground.0;
        let hulls: Vec<(Arc<track::Mesh>, crate::math::Mat4, Vf)> = (0..fleet.len())
            .filter(|&j| j != i)
            .filter_map(|j| {
                let p = &*fleet[j].flight;
                let mut at = self.hulls[j]?;
                at.0[3] = crate::nearest_copy(world, from, at.0[3]);
                Some((p.hull.clone()?, at, p.body.velocity))
            })
            .collect();
        let planes: Vec<flight_search::Plane<'_>> = hulls.iter().map(|(mesh, at, velocity)| flight_search::Plane { mesh, at: *at, velocity: *velocity }).collect();
        let fly = &mut self.fleet[i];
        let plane = &mut *fly.flight;
        if let Some(rays) = plane.rays.clone() {
            let mesh = plane.bike.clone();
            let bikes = mesh.as_deref().map(|mesh| Bikes { mesh, bikes: fly.bikes });
            let node = node_of(&plane.body);
            let (field, props, trees) = flight_search::near_world(&mut *self.ground.0, &rays, &node, &mut plane.nearby);
            let bounds = field.as_deref().map(field_box);
            flight_search::refresh_planes(props, trees, bikes.as_ref(), &planes, bounds, &rays, &node, &mut plane.nearby);
        }
        let at = plane.body.position;
        let holed = plane.area.is_some() && self.ground.0.off_world(at, F(crate::biplane::EDGE));
        let bare = !self.ground.0.keeps(at) && at.y() < 0.0;
        let wrap = self.ground.0.wrap();
        plane.post(dt, &self.tables.acos, holed, bare, wrap);
        if self.hulls[i].is_some() {
            self.hulls[i] = Some(hull_at(plane));
        }
    }
}
