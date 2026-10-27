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
use ps2_float::{Dest, F, Vf};

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
    /// Where the plane is put back when a wreck recovers: the model's origin and its nose.
    pub spawn: (Vf, Vf),
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
        let (throttle, pad, spawn, flown) = (self.surfaces.throttle, self.pad, self.spawn, self.attitude.flown);
        self.put_back(other);
        (self.surfaces.throttle, self.pad, self.spawn, self.attitude.flown) = (throttle, pad, spawn, flown);
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
        let (throttle, spawn) = (self.surfaces.throttle, self.spawn);
        self.reset(origin, facing, acos);
        (self.surfaces.throttle, self.spawn) = (throttle, spawn);
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

    /// The plane stood at `origin` (the model's, which is where `machine.txt` parks it) with its nose
    /// along `facing` and its wings level, at rest; then measured, so that the first frame's wheels
    /// are placed by where it now is. Also where a recovered wreck is put back.
    pub fn place(&mut self, origin: Vf, facing: Vf, acos: &AcosTable) {
        self.spawn = (origin, facing);
        self.put(acos);
    }

    /// The plane unwrecked and put back at a new spawn: what the original's swap does to the plane it
    /// hands the controller (its unwreck, then its reset), with the throttle and the surfaces at zero.
    pub fn reset(&mut self, origin: Vf, facing: Vf, acos: &AcosTable) {
        self.spawn = (origin, facing);
        self.unwreck(acos);
    }

    /// **The reset chord** (S57), from the input slot, before the frame: `respawn` is
    /// [`crate::ride::Controls::respawn`] off the plane's pad — L1 pressed while R1 is held or the
    /// other way round, the two commands the original's chord reads ([`crate::biplane::Plane::reset_chord`]
    /// is S36's model of it). On the wheels or wrecked, the plane is unwrecked: back to its spawn at
    /// rest, on the press's own frame, as the waited-out wreck is. In the air it does nothing, and L1
    /// or R1 alone is the rudder. The original latches it once a frame because L1's and R1's press
    /// events are two calls; here they are one `respawn` a frame, so there is nothing to latch.
    /// Whether it put the plane back.
    pub fn reset_chord(&mut self, respawn: bool, acos: &AcosTable) -> bool {
        if !respawn || !(self.measure.grounded || self.crash.reason != 0) {
            return false;
        }
        self.unwreck(acos);
        true
    }

    /// The spawn into the body, and the measure after it.
    fn put(&mut self, acos: &AcosTable) {
        let (origin, facing) = self.spawn;
        let up = Vf::new(0.0, 1.0, 0.0, 0.0);
        let q = mat4_to_quat(&basis(origin, facing, up, true, false));
        let b = &mut self.body;
        let at = origin.merge(Dest::XYZ, origin.add(turn(q, b.cofg)));
        let at = Vf::new(at.x(), at.y(), at.z(), 1.0);
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
        let mesh = self.bike.clone();
        let bikes = mesh.as_deref().map(|mesh| Bikes { mesh, bikes });
        let (struck, movers) = self.search(dt, ground, bikes);
        let mut listeners = Listeners { plane: self, ground: Ground(ground), tables, bikes, movers: &movers, seam: [0, 0] };
        WORLD.step(dt, &mut listeners);
        let seam = listeners.seam;
        if self.attitude.flown {
            self.pad = pad;
        }
        (struck, seam)
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
    fn search(&mut self, dt: F, world: &mut dyn world::World, bikes: Option<Bikes<'_>>) -> (Vec<Option<u32>>, Vec<contact::Mover>) {
        let none = vec![None; bikes.map_or(0, |b| b.bikes.len())];
        let Some(rays) = self.rays.clone() else { return (none, Vec::new()) };
        self.record.node = node_of(&self.body);
        contact_search::clear(&mut self.record, Some(&mut self.body));
        let (field, props, trees) = world.wooded();
        let bounds = field.as_deref().map(field_box);
        let mut scene = Scene::flown(props, trees, bikes, bounds, &rays, &self.record.node, &mut self.nearby);
        contact_search::collide(std::slice::from_mut(&mut self.record), 0, Some(&mut self.body), field.as_deref(), &mut scene, dt);
        let movers = contact_search::Search::movers(&scene);
        (scene.struck, movers)
    }

    /// The search's refill (S99), first in the post listener: the original's body post sets the
    /// plane's pose from the stepped body and has the collision object gather its nearby list around
    /// the ray probe's reach there ([`flight_search::refresh`]), before the measure, the crash test
    /// and any unwreck — so the frame after a recovery searches with the list gathered where the
    /// plane came down. Once a world step, before the other bodies' posts move them.
    fn refresh(&mut self, world: &mut dyn world::World, bikes: Option<Bikes<'_>>) {
        let Some(rays) = self.rays.as_ref() else { return };
        let (field, props, trees) = world.wooded();
        let bounds = field.as_deref().map(field_box);
        flight_search::refresh_flown(props, trees, bikes.as_ref(), bounds, rays, &node_of(&self.body), &mut self.nearby);
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
    fn forces(&mut self, h: F, ground: &mut Ground<'_>, curve: &CurveTable, acos: &AcosTable, movers: &[contact::Mover]) {
        let (collider, contacts, mass) = (&mut self.collider, &mut self.record.contacts, self.mass);
        collision_force(&mut self.body, self.gravity, |body| {
            contact::resolve(collider, &mut Weighed { body, mass }, contacts, &mut [], movers, true);
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
        let (touchdown, true_speed) = self.measure.contacts(wheels, false);
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
    /// and the engine cut.
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
        self.engine.power = F::ZERO;
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
        self.put(acos);
    }

    /// Where the drawn plane is: the model's orientation and origin as the body stood at the start of
    /// the frame's last substep ([`Body::pose`]).
    pub fn pose(&self) -> (Vf, Vf) {
        self.body.pose()
    }
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
    /// The widths the post moved the plane over a seam (MD44).
    seam: [i8; 2],
}

impl Dynamics for Listeners<'_> {
    fn count(&self, list: List) -> i32 {
        match list {
            List::Constraints => 0,
            _ => 1,
        }
    }
    fn preforce(&mut self, _: i32, dt: F) {
        self.plane.preforce(dt);
    }
    fn force(&mut self, _: i32, h: F) {
        self.plane.forces(h, &mut self.ground, &self.tables.curve, &self.tables.acos, self.movers);
    }
    fn enabled(&self, _: i32) -> bool {
        self.plane.body.enabled
    }
    fn integrate(&mut self, _: i32, h: F) {
        self.plane.body.integrate(h);
    }
    fn end_substep(&mut self, _: i32) {
        self.plane.body.end_substep();
    }
    fn constrain(&mut self, _: i32, _: bool) {}
    fn post(&mut self, _: i32, dt: F) {
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
struct BodyWalk<'a>(&'a Body);

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
