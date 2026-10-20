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
//! **What a frame does today is the plane's preforce listener and nothing else.** The original steps
//! the plane inside the engine's generic dynamics world: the controls once a frame, then a handful of
//! substeps of the autopilot, the engine, the aerodynamics and the three tires, with the rigid body
//! integrated between them and the measure listener deriving the next frame's attitude at the end.
//! The world step, the body and the measure are S38's and the tires and the crash test S39's; until
//! they land [`Flight::frame`] cannot run the substeps, and the body does not move. The pad it is
//! handed is the recorded one, through I6's mapping ([`pad`]). The acceptance run over C7's capture (`the_biplane_acceptance_run` in
//! `tests/replay.rs`) says so, lane by lane, on every `./kit check`.

#![deny(clippy::float_arithmetic)]

use crate::biplane::{Aero, Air, Attitude, Autopilot, Engine, Joystick, Pad, Surfaces, controls};
use crate::hash::{State, Walk};
use crate::input::{self, PlaneInput};
use crate::phy;
use ps2_float::{F, Vf};

/// The engine's constructor values the file does not set: the rpm the drawn prop turns at, the
/// power the file's `MaxPower` replaces, and the throttle's idle and top revolutions.
const PROP_RPM: f32 = f32::from_bits(0x4359_3fa2);
const IDLE_RPM: f32 = 500.0;
const MAX_RPM: f32 = 2500.0;

/// The aerodynamic model's constructor density, slugs per cubic foot: sea level.
const DENSITY: f32 = f32::from_bits(0x3b1b_c5c4);

/// The rigid body the plane flies on, as far as a capture of the original sees it: four quads, each
/// as the body keeps it (world frame, `w` lanes included).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Body {
    /// The centre of mass.
    pub pos: Vf,
    pub vel: Vf,
    /// The orientation, `x y z w`.
    pub quat: Vf,
    /// The angular velocity, radians a second; `x` is what the pitch autopilot holds.
    pub rate: Vf,
}

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
    pub body: Body,
    /// The pad as the plane's input step left it.
    pub pad: Pad,
    pub surfaces: Surfaces,
    /// What the measure listener left for the controls and the autopilot.
    pub attitude: Attitude,
    /// And for the aerodynamics.
    pub air: Air,
    /// The wreck reason: 0 flying, 1 crashed, 13 thrown back from the map's edge.
    pub crash: u32,
    /// Seconds in the crash.
    pub crash_time: F,
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
        Flight {
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
        }
    }

    /// One frame of the plane, `pad` being what its input step made of the controller.
    ///
    /// The preforce half of the original's frame: the pad through the rate limits into the surfaces
    /// ([`controls`]), once, with the frame's `dt`. The substeps after it — the autopilot, the
    /// engine, the aerodynamics and the tires, each handing the body a force, and the body integrated
    /// between them — and the measure that refreshes [`Flight::attitude`] and [`Flight::air`] are
    /// not run: see the module's own docs for whose they are.
    pub fn frame(&mut self, dt: F, pad: Pad) {
        self.pad = pad;
        controls(&mut self.pad, &mut self.surfaces, &self.joystick, &self.attitude, dt);
    }
}

/// The plane's input step on the player's pad: I6's [`PlaneInput::from_pad`], into the five fields
/// the plane keeps it in (the two throttle commands, then the elevator, the aileron and the rudder,
/// in the order the original stores them).
pub fn pad(pad: &input::Pad) -> Pad {
    let p = PlaneInput::from_pad(pad);
    Pad { forward: p.throttle_up, back: p.throttle_down, elevator: p.elevator, aileron: p.aileron, rudder: p.rudder }
}

impl State for Body {
    fn walk(&self, w: &mut Walk<'_>) {
        w.vf("pos", self.pos);
        w.vf("vel", self.vel);
        w.vf("quat", self.quat);
        w.vf("rate", self.rate);
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
        w.field("body", &self.body);
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
        w.u32("crash", self.crash);
        w.f("crash_time", self.crash_time);
    }
}
