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
//! 4. *post*, once: the measure's contact half (grounded off the wheels, the aero model's true-speed
//!    switch), the crash test ([`Crash`]) with the wreck and the recovery it asks for, then the
//!    measure's flight half, which is what next frame's listeners read ([`Measure`]).
//!
//! What it does not have, and why, is in [`Flight::frame`]'s own docs: no contact with anything but
//! the ground under the three wheels, because the collision search the response would read is
//! S45/S46/S48's. The pad it is handed is the recorded one, through I6's mapping ([`pad`]). The
//! acceptance run over C7's capture (`the_biplane_acceptance_run` in `tests/replay.rs`) says how far
//! it agrees, lane by lane, on every `./kit check` that has the capture.

#![deny(clippy::float_arithmetic)]

use crate::air::AcosTable;
use crate::biplane::{
    self, Aero, Air, Area, Attitude, Autopilot, Body, Crash, CrashContact, Dynamics, Engine, Joystick, List,
    Measure, Mount, Pad, Surfaces, THROWN, Tire, autopilot, collision_force, controls, gravity_force,
};
use crate::camera::CurveTable;
use crate::chassis::{CLEAR, turn};
use crate::collide::{Bundle, Record};
use crate::hash::{State, Walk};
use crate::input::{self, PlaneInput};
use crate::math::{basis, mat4_to_quat};
use crate::noise::SmoothNoise;
use crate::phy;
use crate::ride;
use crate::rng::Rng;
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
        f
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
    /// The world step and its listeners as the module's docs list them. Two things the original's
    /// frame has are not here:
    ///
    /// * **The collision response is empty.** The original resolves its collision object's contacts
    ///   inside the force listener ([`collision_force`]); the search that would fill that list is
    ///   S45/S46/S48's and not ported, so the plane touches the world only through its three wheels,
    ///   and the crash test never sees a contact — a plane cannot crash into anything, only be thrown
    ///   back from the map's edge.
    /// * **What is not simulation**: the touchdown sound, the surfaces' and the pilot's animation
    ///   (S35's, the renderer's), the engine sound the prop rpm is written from, the pad's rumble.
    pub fn frame(&mut self, dt: F, pad: Pad, ground: &mut dyn world::World, tables: &ride::Tables) {
        self.pad = pad;
        WORLD.step(dt, &mut Listeners { plane: self, ground: Ground(ground), tables });
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
    fn forces(&mut self, h: F, ground: &mut Ground<'_>, curve: &CurveTable) {
        collision_force(&mut self.body, self.gravity, |_| {});
        if self.crash.reason == 0 {
            self.attitude = self.measure.attitude(self.attitude.flown, &self.body);
            autopilot(&self.pad, &mut self.surfaces, &self.joystick, &self.autopilot, &self.attitude, curve, h);
            let (noise, rng) = (&mut self.noise, &mut self.rng);
            let mut sample = |t: F| noise.get_or_insert_with(|| SmoothNoise::build(rng)).at(t);
            let b = &self.body;
            let thrust = self.engine.step(self.surfaces.throttle, b.velocity, F(b.position.y()), F(self.measure.forward.y()), h, &mut sample);
            self.body.add_force_local(thrust);
            self.air = self.measure.air(&self.surfaces, &self.body);
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
    }

    /// The post listener: the measure, with the crash test between its halves.
    fn post(&mut self, dt: F, acos: &AcosTable) {
        let wheels = self.tires.map(|t| t.touching);
        let (_touchdown, true_speed) = self.measure.contacts(wheels, false);
        self.aero.true_speed = true_speed;
        let m = &self.measure;
        let c = CrashContact { touching: false, ignored: false, grounded: m.grounded, speed: m.airspeed, up_y: F(m.up.y()) };
        let b = &mut self.body;
        let out = self.crash.step_in(&c, self.area.as_ref(), b.position, &mut b.velocity, &mut b.rates, &mut self.edge_scale, &mut self.rng, dt);
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

/// The plane's lists in the dynamics world: one body, one listener of each kind, no constraints.
struct Listeners<'a> {
    plane: &'a mut Flight,
    ground: Ground<'a>,
    tables: &'a ride::Tables,
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
        self.plane.forces(h, &mut self.ground, &self.tables.curve);
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
        self.plane.post(dt, &self.tables.acos);
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
    }
}
