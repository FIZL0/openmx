//! **MX vs. ATV's cars' physics frame** (MD106), ported from MX vs. ATV Unleashed's PS2 ELF
//! (SLUS-21104): the physics world's step over MXVehicleCar's three listeners, as [`crate::quad::frame`]
//! is the quad's.
//!
//! The car is MX Unleashed's car class ([`crate::car::Machine`]) on SLUS-21104's vehicle base, and its
//! listeners run MX Unleashed's pieces in MX Unleashed's order, each through a virtual or a call:
//!
//! - [`pre_process`], once a frame before the substeps: the body's shock peak cleared, the controls,
//!   the centre of mass's travel, the four tires' contacts forgotten, the contact sort, each tire's
//!   probe, and the fronts' lone-front flag.
//! - [`force`], each substep: the vehicle's gravity, the anti-slide, then (only while the car is in
//!   its riding state, which MX Unleashed does not test) the preload, the launch and the engine; the
//!   two front tires, the fronts' load and ground handed to the rears, the two rears, the aero.
//! - [`post_process`], once a frame after: the vehicle's own post step, the yaw-rate hold, the gear
//!   change, the wreck timer (SLUS-21104's own, with its wreck states), the grounded flags, the
//!   landing sound, the speeds, angles and frame, the ground's tilt under the car, the latch, and
//!   the scene node's position.
//!
//! What they call is not ported here: each is a [`Call`] handed to a [`Steps`], in the original's
//! order. The car is described by what the listeners read and write ([`Car`], [`Tire`]), not by its
//! layout; the field map is research/oracle.txt's (`# MD39`, `# MD39b`, `# MD106`).
//!
//! [`update`], the car's once-a-frame update outside the world step (vt `+0x10c`), is the same: its own
//! code ported, its calls handed out.
//!
//! Proof: `tests/mxvatv_car_frame.rs`, SLUS-21104's world step and the car's update off MD39a's buggy
//! image with every call hooked.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::air::AcosTable;
use crate::atv_body::Body;
use ps2_float::{Acc, Dest, F, Vf};

/// The tires, in the car's order: the steered pair, then the rear pair.
pub const FRONT_LEFT: usize = 0;
pub const FRONT_RIGHT: usize = 1;
pub const REAR_LEFT: usize = 2;
pub const REAR_RIGHT: usize = 3;

/// The rodata quadword the shock peak is cleared to, and compared with whole: `(0, 0, 0, 1)`.
const CLEAR: Vf = Vf([0.0, 0.0, 0.0, 1.0]);

/// Above this (the up axis on the ground's normal, about 45 degrees) the car is not tipped.
const UPRIGHT: u32 = 0x3f34_fdf4;

/// The car's state word: in any other the car is wrecked, and the force listener skips the preload,
/// the launch and the engine.
pub const RIDING: u32 = 0;

/// One tire, as the car's listeners see it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tire {
    /// The probe found a solid contact, or the contact sort gave the tire one; both forgotten each
    /// frame before the sort.
    pub solid: bool,
    pub sorted: bool,
    /// What the contact is on (an object and a mover), forgotten with the two flags; a rear tire is
    /// handed the grounded front's.
    pub stands_on: [u32; 2],
    /// On the ground this substep.
    pub grounded: bool,
    /// A front tire's: a front is down and neither rear is.
    pub lone_front: bool,
    /// A rear tire's, handed each substep before it steps: a front is down, the larger front load,
    /// and what that front stands on; all clear when neither front is down.
    pub handed: bool,
    pub handed_load: F,
    pub handed_on: [u32; 2],
    /// The normal force.
    pub load: F,
    /// The contact's velocity in the car's frame, the post's ground velocity while the tire is down.
    pub local_velocity: Vf,
}

/// What the car's listeners read and write of the car, beside its body and [`Tire`]s.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Car {
    /// [`RIDING`], or one of the wrecked states.
    pub state: u32,
    /// `[Steering] YawRateLimit`: the post holds the body's yaw rate to it.
    pub yaw_rate_limit: F,
    /// A tire was down at the end of last frame, and is at the end of this one.
    pub was_grounded: bool,
    pub grounded: bool,
    /// Both left tires down and neither right, or the other way round; only measured while grounded.
    pub one_side: bool,
    /// The car has a landing sound (it plays it on touching down).
    pub landing_sound: bool,
    /// The body's speed, its velocity in its own frame, the rates turned into the world, the ground
    /// velocity (the first grounded tire's, else the body's own), alpha and side slip, the Euler
    /// angles, the frame and its forward and up rows (`w` kept).
    pub speed: F,
    pub velocity: Vf,
    pub world_rates: Vf,
    pub ground_velocity: Vf,
    pub alpha: F,
    pub side_slip: F,
    pub euler: Vf,
    pub matrix: [Vf; 4],
    pub forward: Vf,
    pub up: Vf,
    /// The body's centre of mass in its own frame, which the frame's origin is taken back by.
    pub cofg: Vf,
    /// The ground's normal under the body, as [`Call::GroundUnder`] answers it; and whether the up
    /// row is 45 degrees or more off it (the collision record's word).
    pub ground_normal: Vf,
    pub tipped: bool,
    /// A latch the post lets go a frame after it was set, and the counter it clears then.
    pub latch: bool,
    pub latched: bool,
    pub latch_count: i32,
    /// The scene node's position, as [`Call::NodePosition`] answers it.
    pub node_pos: Vf,
    pub tires: [Tire; 4],
}

/// What the car's three listeners call, and this module does not port, in the order the original
/// calls them. `F`s are the `dt` or `h` handed over; `usize`s the tire. The names are MX Unleashed's
/// pieces ([`crate::car::Machine`]) in the same places.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    /// The PreProcess's: the controls' step (vt `+0x34c`; [`crate::car::Controls::step`] and the rear
    /// friction), the centre of mass's travel (vt `+0x470`; [`crate::car::travel_cofg`]), the contact
    /// sort, and each tire's probe.
    Controls(F),
    /// The controls' own call (MD126, [`Pieces`]): the engine's reverse switch, into reverse with both
    /// pedals down, out of it otherwise (and always without a driver).
    Reverse(crate::car::Reverse),
    TravelCofg,
    SortContacts,
    Probe(usize, F),
    /// The Force body's: the vehicle's gravity and collision record (first, always), the anti-slide
    /// (vt `+0x478`); riding, the preload (vt `+0x47c`), the launch (vt `+0x480`) and the engine's
    /// step; each tire's step (its suspension, roll and forces); the aero (vt `+0x474`).
    VehicleForce,
    AntiSlide(F),
    Preload(F),
    /// The preload's own call (MD106a, [`Pieces`]): its cue, played once as the push starts winding
    /// down from a set timer, and not again until the timer has run out.
    PreloadCue,
    Launch(F),
    Engine(F),
    TireStep(usize, F),
    Aero(F),
    /// The PostProcess's: the vehicle's own post step; the gearbox's shift; the wreck timer (vt
    /// `+0x484`); on touching down, the landing sound handed the shock (or, with none, the velocity)
    /// taken into the scene node's frame; the world's ground query under the body's position (into
    /// [`Car::ground_normal`]); the scene node asked where it is (into [`Car::node_pos`]).
    VehiclePost(F),
    Shift(F),
    WreckTimer(F),
    Landed(Vf),
    GroundUnder(Vf),
    NodePosition,
    /// The update's ([`update`]): the vehicle's ghost step (vt `+0x3a4`, as the quad's); the
    /// vehicle's hazard-surface clock (vt `+0x45c`, the quad's [`crate::quad::Call::HazardSurface`]),
    /// handed the longest of the four tires' surface times and whether a tire is sunk; the class's own
    /// per-frame slot (vt `+0x488`); and last, always, the vehicle's base update.
    Ghost(F),
    HazardClock { dt: F, time: F, sunk: bool },
    ClassStep,
    VehicleUpdate(F),
}

/// Whoever runs the calls the car's listeners make. A `Vec<Call>` just lists them.
pub trait Steps {
    fn call(&mut self, c: Call, car: &mut Car, body: &mut Body);
}

impl Steps for Vec<Call> {
    fn call(&mut self, c: Call, _car: &mut Car, _body: &mut Body) {
        self.push(c);
    }
}

/// **The car's PreProcess**: once a frame, before the substeps.
pub fn pre_process(car: &mut Car, body: &mut Body, dt: F, s: &mut impl Steps) {
    body.correction_peak = CLEAR;
    s.call(Call::Controls(dt), car, body);
    s.call(Call::TravelCofg, car, body);
    for t in &mut car.tires {
        t.solid = false;
        t.sorted = false;
        t.stands_on = [0, 0];
    }
    s.call(Call::SortContacts, car, body);
    for k in 0..4 {
        s.call(Call::Probe(k, dt), car, body);
    }
    let down = |t: &Tire| t.solid || t.sorted;
    let t = &car.tires;
    let lone = (down(&t[FRONT_LEFT]) || down(&t[FRONT_RIGHT])) && !down(&t[REAR_LEFT]) && !down(&t[REAR_RIGHT]);
    for t in &mut car.tires[..REAR_LEFT] {
        t.lone_front = lone;
    }
}

/// **The car's Force body**: each substep.
pub fn force(car: &mut Car, body: &mut Body, h: F, s: &mut impl Steps) {
    s.call(Call::VehicleForce, car, body);
    s.call(Call::AntiSlide(h), car, body);
    if car.state == RIDING {
        s.call(Call::Preload(h), car, body);
        s.call(Call::Launch(h), car, body);
        s.call(Call::Engine(h), car, body);
    }
    s.call(Call::TireStep(FRONT_LEFT, h), car, body);
    s.call(Call::TireStep(FRONT_RIGHT, h), car, body);
    hand_off(&mut car.tires);
    s.call(Call::TireStep(REAR_LEFT, h), car, body);
    s.call(Call::TireStep(REAR_RIGHT, h), car, body);
    s.call(Call::Aero(h), car, body);
}

/// What the Force body hands each rear tire before stepping it: with a front down, the larger front
/// load (a `max.s`, so `+0` against `-0` is `+0`) and what the left front stands on, or the right's
/// when the left is not down; with neither, nothing.
fn hand_off(t: &mut [Tire; 4]) {
    let (fl, fr) = (t[FRONT_LEFT], t[FRONT_RIGHT]);
    let (handed, load, on) = if !fl.grounded && !fr.grounded {
        (false, F::ZERO, [0, 0])
    } else {
        let load = fl.load.max(fr.load);
        (true, load, if fl.grounded { fl.stands_on } else { fr.stands_on })
    };
    for r in &mut t[REAR_LEFT..] {
        r.handed = handed;
        r.handed_load = load;
        r.handed_on = on;
    }
}

/// **The car's PostProcess**: once a frame, after the substeps.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's `c.ole.s` branched on false
pub fn post_process(car: &mut Car, body: &mut Body, dt: F, table: &AcosTable, s: &mut impl Steps) {
    s.call(Call::VehiclePost(dt), car, body);
    hold_yaw_rate(&mut body.rates, car.yaw_rate_limit);
    s.call(Call::Shift(dt), car, body);
    s.call(Call::WreckTimer(dt), car, body);
    car.was_grounded = car.grounded;
    let down = car.tires.map(|t| t.grounded);
    car.grounded = down.iter().any(|&d| d);
    if car.grounded {
        let [fl, fr, rl, rr] = down;
        car.one_side = (fl && rl && !fr && !rr) || (!fl && !rl && fr && rr);
    }
    if !car.was_grounded && car.grounded && car.landing_sound {
        let shock = body.correction_peak;
        let v = if shock.to_bits() == CLEAR.to_bits() { body.velocity } else { shock };
        s.call(Call::Landed(v), car, body);
    }
    let v = body.velocity;
    car.speed = Acc::mula(v.x(), v.x()).madda(v.y(), v.y()).madd(v.z(), v.z()).sqrt();
    let q = body.quat;
    car.velocity = crate::biplane::unturn(q, v);
    car.ground_velocity = car.tires.iter().find(|t| t.grounded).map_or(car.velocity, |t| t.local_velocity);
    car.world_rates = crate::chassis::turn(q, body.rates);
    let along = F(car.velocity.z());
    car.alpha = crate::biplane::flow_angle(along, F(car.velocity.y()), table);
    car.side_slip = crate::biplane::flow_angle(along, F(car.velocity.x()), table);
    car.euler = crate::math::quat_to_euler(q, table);
    let p = body.position;
    let origin = p.merge(Dest::XYZ, p.sub(crate::chassis::turn(q, car.cofg)));
    car.matrix = crate::atv_moto::quat_frame(q, origin);
    car.forward = car.forward.merge(Dest::XYZ, car.matrix[2]);
    car.up = car.up.merge(Dest::XYZ, car.matrix[1]);
    s.call(Call::GroundUnder(p), car, body);
    let (u, n) = (car.up, car.ground_normal);
    let tilt = Acc::mula(u.x(), n.x()).madda(u.y(), n.y()).madd(u.z(), n.z());
    car.tipped = tilt <= F(f32::from_bits(UPRIGHT));
    if car.latch && car.latched {
        car.latch = false;
        car.latch_count = 0;
    }
    car.latched = car.latch;
    s.call(Call::NodePosition, car, body);
}

/// The body's yaw rate (its body-frame `y`) held to `limit`, keeping its sign: MX Unleashed's
/// [`crate::car::hold_yaw_rate`], but tested as `!(|y| <= limit)`, so a rate that is not a number
/// becomes `limit`.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's `c.ole.s` branched on false
fn hold_yaw_rate(rates: &mut Vf, limit: F) {
    let y = F(rates.y());
    if !(y.abs() <= limit) {
        let sign = if y < 0.0 { -1 } else { 1 };
        rates.0[1] = (limit * F::from(sign)).0;
    }
}

/// What the car's [`update`] reads and keeps beyond [`Car`] (research/oracle.txt `# MD106d`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Update {
    /// The game's still-camera switch: set, the update is the vehicle's base update alone.
    pub still_camera: bool,
    /// The vehicle state's mode, or none. With none, or in mode 3 or 4, the update counts the latch
    /// instead of measuring the hazard surfaces.
    pub vehicle_state: Option<u32>,
    /// The vehicle's surface hazard table (the quad's), and each tire's surface, an index into it.
    pub surfaces: [crate::quad::Surface; 16],
    pub tire_surface: [u8; 4],
    /// Some tire is on a hazard surface; kept from the last frame it was measured.
    pub on_hazard: bool,
}

/// Counted past this many frames, the latch ([`Car::latch`]) is let go.
const LATCH_FRAMES: i32 = 3;

/// **The car's update**, once a frame outside the world step. Unless the still camera is on (when only
/// the vehicle's base update runs): the ghost step; then, with no vehicle state or in its mode 3 or 4,
/// the latch counted (a set latch counts frames up to [`LATCH_FRAMES`], and on the next is let go with
/// its count cleared); else the hazard surfaces: on one when any tire's surface is a hazard, sunk when
/// also any tire is solidly down, and the vehicle's hazard clock handed the longest of the four tires'
/// surface times. Then the class's own step and the vehicle's base update.
pub fn update(u: &mut Update, car: &mut Car, body: &mut Body, dt: F, s: &mut impl Steps) {
    if !u.still_camera {
        s.call(Call::Ghost(dt), car, body);
        if matches!(u.vehicle_state, None | Some(3 | 4)) {
            if car.latch {
                if car.latch_count < LATCH_FRAMES {
                    car.latch_count += 1;
                } else {
                    car.latch = false;
                    car.latch_count = 0;
                }
            }
        } else {
            let surface = u.tire_surface.map(|k| u.surfaces[usize::from(k)]);
            u.on_hazard = surface.iter().any(|s| s.hazard);
            let sunk = u.on_hazard && car.tires.iter().any(|t| t.solid);
            let time = surface[REAR_LEFT].time.max(surface[REAR_RIGHT].time);
            let time = surface[FRONT_RIGHT].time.max(time);
            let time = surface[FRONT_LEFT].time.max(time);
            s.call(Call::HazardClock { dt, time, sunk }, car, body);
        }
        s.call(Call::ClassStep, car, body);
    }
    s.call(Call::VehicleUpdate(dt), car, body);
}

/// SLUS-21104's physics world as the car's frame sees it: one rigid body, the car's three
/// listeners, no constraints.
struct Frame<'a, S> {
    car: &'a mut Car,
    body: &'a mut Body,
    table: &'a AcosTable,
    s: &'a mut S,
}

impl<S: Steps> crate::biplane::Dynamics for Frame<'_, S> {
    fn count(&self, list: crate::biplane::List) -> i32 {
        i32::from(list != crate::biplane::List::Constraints)
    }
    fn preforce(&mut self, _: i32, dt: F) {
        pre_process(self.car, self.body, dt, self.s);
    }
    fn force(&mut self, _: i32, h: F) {
        force(self.car, self.body, h, self.s);
    }
    fn enabled(&self, _: i32) -> bool {
        self.body.enabled
    }
    fn integrate(&mut self, _: i32, h: F) {
        self.body.integrate(h);
    }
    fn end_substep(&mut self, _: i32) {
        self.body.end_substep();
    }
    fn constrain(&mut self, _: i32, _: bool) {}
    fn post(&mut self, _: i32, dt: F) {
        post_process(self.car, self.body, dt, self.table, self.s);
    }
}

/// **The car's physics frame**: SLUS-21104's world step over the car alone ([`crate::biplane::World::step`];
/// the game's world has 1 iteration at 115 a second, so four substeps at 30 frames a second):
/// [`pre_process`] with the frame's `dt`, then each substep [`force`], the body's
/// [`Body::integrate`] and [`Body::end_substep`] (while the body is enabled), then
/// [`post_process`] with the frame's `dt`.
///
/// In the game the world's PreProcess list holds the other vehicles' listeners too (the second car of
/// the class and two more), run after the car's own; they are not part of this.
pub fn frame(world: &crate::biplane::World, car: &mut Car, body: &mut Body, dt: F, table: &AcosTable, s: &mut impl Steps) {
    world.step(dt, &mut Frame { car, body, table, s });
}

// --- MX Unleashed's pieces (MD106a) ----------------------------------------------------------------

/// One wing, as the aero reads it (MX Unleashed's [`crate::car::Wing`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wing {
    pub lift_slope: F,
    pub angle_of_attack: F,
    pub basic_drag: F,
    pub induced_drag: F,
    /// Where it pushes, in the car's frame.
    pub position: Vf,
}

/// The loader's words the ported pieces read (research/oracle.txt `# MD39b`, `# MD106a`, `# MD126`): MX
/// Unleashed's `[Joystick]` (its rate limits, steer tables and squared steering), `[Steering]`'s yaw
/// rate limits and the hand brake's keys, `[Mass Properties]` `CofG` and the car's `CofGAI` and
/// travel, `[Preload]`, the clutch's launch keys and `[Aerodynamics]` with its wings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Keys {
    pub square_steering: bool,
    pub brake_rate_limit: F,
    pub gas_rate_limit: F,
    pub x_dpad_rate_limit_on: F,
    pub x_dpad_rate_limit_off: F,
    pub x_axis_rate_limit: F,
    /// Three `(speed, scale)` points each: the stick's, a wheel's and the d-pad's steer tables.
    pub steer: [[F; 2]; 3],
    pub wheel_steer: [[F; 2]; 3],
    pub dpad_steer: [[F; 2]; 3],
    /// Two `(speed, scale)` points: the rear tires' share of the steer.
    pub rear_steer: [[F; 2]; 2],
    pub yaw_rate_limit: F,
    pub hand_brake_yaw_rate_limit: F,
    pub hand_brake_friction_scale: F,
    pub hand_brake_brake_scale: F,
    pub hand_brake_fade_time: F,
    pub cofg: Vf,
    pub cofg_ai: Vf,
    pub cofg_travel_forward: F,
    pub cofg_travel_back: F,
    pub preload_time: F,
    pub min_preload_time: F,
    pub max_preload_time: F,
    pub max_preload: F,
    pub clutch_torque_scale_time: F,
    pub max_clutch_torque_scale: F,
    pub max_clutch_torque_scale_speed: F,
    pub aero_surface_area: F,
    pub airborne_damping: F,
    pub unroll_gain: F,
    pub unroll_torque: F,
    pub unyaw_gain: F,
    pub unyaw_torque: F,
    pub airborne_control_moment: Vf,
    pub brake_gyro_torque: F,
    /// The wings the aero steps: `NumberOfWings` of them (none when the count is not positive).
    pub wings: Vec<Wing>,
}

/// The preload's state: it may wind up, it is pushing, it is armed by a forward lean; the wind-up
/// while armed and the push's time left while active; the push (world, its `y` set as it fires);
/// and whether the cue has played for this push.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PreloadState {
    pub ready: bool,
    pub active: bool,
    pub armed: bool,
    pub timer: F,
    pub force: Vf,
    pub cued: bool,
}

/// What the ported pieces read and keep of the car beyond [`Car`]: its pad's words as the controls
/// left them, the vehicle's, the world's and the pieces' own.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct State {
    /// A person drives: the vehicle state's mode is 1 (no vehicle state, or any other mode, is the
    /// AI's `CofGAI`).
    pub human: bool,
    /// Someone drives at all (without, the controls clear every pedal); the vehicle state says the
    /// driver's controller is a wheel; the d-pad steered last.
    pub driven: bool,
    pub wheel: bool,
    pub dpad: bool,
    /// The race's start holds the gas off (the vehicle's start-penalty clock is running).
    pub start_penalty: bool,
    /// The stick's lean (back is negative) and steer, the preload's pad word, the clutch pedal.
    pub lean: F,
    pub stick: F,
    pub pad_word: F,
    pub clutch: F,
    /// The vehicle's gas and brake pedals as pressed, and its gas after the rate limit; the car's
    /// brake and brake force (the front and the rear tires' brakes) and hand brake.
    pub gas_pedal: F,
    pub gas: F,
    pub vehicle_brake: F,
    pub brake: F,
    pub brake_force: F,
    pub hand_brake: F,
    /// The steer after its rate limit; the front and rear steer angles; the clutch the engine reads;
    /// what is left of the hand brake's fade; the two rear tires' friction scales (left, right).
    pub steer_input: F,
    pub steer: F,
    pub rear_steer: F,
    pub engaged_clutch: F,
    pub hand_brake_fade: F,
    pub rear_friction: [F; 2],
    /// The car is held braked, off the gas, until the gas pedal first comes on; the vehicle lets both
    /// pedals reverse it, and lets the gas through at all; the engine is in reverse.
    pub held: bool,
    pub reverse_enabled: bool,
    pub throttle_enabled: bool,
    pub reversing: bool,
    /// The body's weight (world), from the vehicle's gravity block.
    pub gravity: Vf,
    /// The collision record says the car touches something.
    pub touching: bool,
    /// The left front tire's ground normal (world).
    pub front_normal: Vf,
    pub preload: PreloadState,
    /// The launch's timer and the scale it started from, and the engine's torque scale it sets.
    pub launch_timer: F,
    pub launch_start: F,
    pub launch_boost: F,
}

/// **The car's [`Steps`] with MX Unleashed's pieces ported** (MD106a, MD126), as SLUS-21104 builds
/// them: the controls, the centre of mass's travel, the anti-slide, the preload, the launch and the
/// aero, run on [`Keys`] and [`State`]. Every other call, the controls' reverse switch and the
/// preload's cue go to `rest`.
pub struct Pieces<R> {
    pub keys: Keys,
    pub state: State,
    pub rest: R,
}

impl<R: Steps> Steps for Pieces<R> {
    fn call(&mut self, c: Call, car: &mut Car, body: &mut Body) {
        if !self.answer(c, car, body) {
            self.rest.call(c, car, body);
        }
    }
}

impl<R: Steps> Pieces<R> {
    /// Runs `c` when it is one of the ported pieces; `false`, and nothing done, when it is not.
    pub fn answer(&mut self, c: Call, car: &mut Car, body: &mut Body) -> bool {
        match c {
            Call::Controls(dt) => {
                if let Some(r) = controls(&self.keys, &mut self.state, car, dt) {
                    self.rest.call(Call::Reverse(r), car, body);
                }
            }
            Call::TravelCofg => travel_cofg(&self.keys, &self.state, car, body),
            Call::AntiSlide(_) => anti_slide(&self.state, car, body),
            Call::Preload(h) => {
                if preload(&self.keys, &mut self.state, car, body, h) {
                    self.rest.call(Call::PreloadCue, car, body);
                }
            }
            Call::Launch(h) => launch(&self.keys, &mut self.state, car, h),
            Call::Aero(_) => aero(&self.keys, &self.state, car, body),
            _ => return false,
        }
        true
    }
}

/// The ones vector the aero hands the body's add-force-at as its moment scale.
const ONES: Vf = Vf([1.0, 1.0, 1.0, 1.0]);

/// `a.xyz + b.xyz`, `w` from `a`; and `-`.
fn plus(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.add(b))
}
fn minus(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.sub(b))
}
/// `v.xyz · s` (`vmulx.xyz` by a broadcast single), `w` kept.
fn scaled(v: Vf, s: F) -> Vf {
    v.merge(Dest::XYZ, v.mul(Vf::splat(s.0)))
}
/// The FPU's dot product, `mula.s`/`madda.s`/`madd.s`, `a`'s lanes first.
fn dot(a: Vf, b: Vf) -> F {
    Acc::mula(a.x(), b.x()).madda(a.y(), b.y()).madd(a.z(), b.z())
}

/// The listeners' inline remap, MX Unleashed's ([`crate::motion`]'s) but for a degenerate input
/// range: the branch past the lerp carries `sub.s` in its delay slot, so that answers the low end
/// less the input's low end, not the low end.
fn remap(x: F, in_lo: F, in_hi: F, out_lo: F, out_hi: F) -> F {
    if in_lo == in_hi {
        return out_lo - in_lo;
    }
    let t = (x - in_lo) / (in_hi - in_lo);
    out_lo.lerp(out_hi, t).max(out_lo.min(out_hi)).min(out_lo.max(out_hi))
}

/// The controls' inline remap for the steer tables: as [`remap`], but a degenerate input range is
/// the low end itself (the `sub.s` in that branch's delay slot writes a register it drops).
fn table_remap(x: F, in_lo: F, in_hi: F, out_lo: F, out_hi: F) -> F {
    if in_lo == in_hi { out_lo } else { remap(x, in_lo, in_hi, out_lo, out_hi) }
}

/// A three-point steer table at `speed`: the first segment below the middle point's speed, the
/// second at or above it.
fn steer_scale(t: &[[F; 2]; 3], speed: F) -> F {
    let [a, b, c] = *t;
    if speed < b[0] { table_remap(speed, a[0], b[0], a[1], b[1]) } else { table_remap(speed, b[0], c[0], b[1], c[1]) }
}

/// `now` moved toward `want` by at most `rate`: `max.s`/`min.s`, then the add.
fn slew(now: F, want: F, rate: F) -> F {
    now + (want - now).max(-rate).min(rate)
}

/// A pedal through its rate limit: it rises no faster than `rate`, and drops at once.
fn pedal(now: F, want: F, rate: F) -> F {
    if want <= now { want } else { slew(now, want, rate) }
}

/// The throttle both pedals together hold the car to while it reverses.
const REVERSE_THROTTLE: u32 = 0x3ecc_cccd;
/// The speed over which the hand brake's brake scale comes in.
const HAND_BRAKE_SPEED: f32 = 10.0;

/// **The controls** (vt `+0x34c`, MX Unleashed's [`crate::car::Controls::step`] on SLUS-21104's
/// vehicle): first a running start penalty takes the gas pedal off. Without a driver every pedal and
/// the stick are cleared, the brakes are full and the yaw limit is the key's, and the engine is put
/// out of reverse. Else the steer: the d-pad's through its on or off rate (off with the stick
/// centred), a wheel's as it is, or the stick's (squared, keeping its sign, with `SquareSteeringInput`)
/// through its rate, each times its table at the car's forward speed; the rear's share by its two
/// points. Then the pedals, unless the car is held (the gas pedal lets it go): both pedals down (when
/// the vehicle allows it) is reverse, the brakes off and the gas pedal pressed to 0.4; otherwise out
/// of reverse (when in it) and the brake through its rate, the rears' the same; the gas through its
/// rate (none while the vehicle holds the throttle off). The clutch passes through. Last the hand
/// brake: held, the rears' friction and the yaw limit are its, the rears' brake its scale with speed
/// up to 10, and the fade is wound; let go, the friction comes back over the fade, and with the fade
/// run out the yaw limit is the key's. Returns the engine's reverse switch, where it calls it.
///
/// SLUS-21104 adds to MX Unleashed's the start penalty, the hold and the vehicle's reverse and
/// throttle switches; the remaps are inline (`adda.s`/`madd.s`), the steer tables' answering a flat
/// range with its low end, the fade's with the low end less the input's (both 1 for its range).
fn controls(k: &Keys, s: &mut State, car: &mut Car, dt: F) -> Option<crate::car::Reverse> {
    use crate::car::Reverse;
    if s.start_penalty {
        s.gas_pedal = F::ZERO;
    }
    if !s.driven {
        s.stick = F::ZERO;
        s.lean = F::ZERO;
        s.pad_word = F::ZERO;
        s.gas_pedal = F::ZERO;
        s.vehicle_brake = F::ZERO;
        s.clutch = F::ZERO;
        s.steer_input = F::ZERO;
        s.gas = F::ZERO;
        s.steer = F::ZERO;
        s.hand_brake = F::ZERO;
        s.brake = F::ONE;
        s.brake_force = F::ONE;
        car.yaw_rate_limit = k.yaw_rate_limit;
        return Some(Reverse::Out);
    }

    let speed = F(car.velocity.z());
    let scale = if s.dpad {
        let rate = if s.stick.abs() == 0.0 { k.x_dpad_rate_limit_off } else { k.x_dpad_rate_limit_on };
        s.steer_input = slew(s.steer_input, s.stick, rate * dt);
        steer_scale(&k.dpad_steer, speed)
    } else if s.wheel {
        s.steer_input = s.stick;
        steer_scale(&k.wheel_steer, speed)
    } else {
        if k.square_steering {
            let sign = if s.stick < 0.0 { -1 } else { 1 };
            s.stick = s.stick * (s.stick * F::from(sign));
        }
        s.steer_input = slew(s.steer_input, s.stick, k.x_axis_rate_limit * dt);
        steer_scale(&k.steer, speed)
    };
    s.steer = scale * s.steer_input;
    let [r1, r2] = k.rear_steer;
    s.rear_steer = table_remap(speed, r1[0], r2[0], r1[1], r2[1]) * s.steer;

    let mut reverse = None;
    if s.gas_pedal > 0.0 {
        s.held = false;
    }
    if s.held {
        s.brake = F::ONE;
        s.brake_force = F::ONE;
        s.gas = F::ZERO;
    } else {
        if s.gas_pedal == 0.0 || s.vehicle_brake == 0.0 || !s.reverse_enabled {
            if s.reversing {
                reverse = Some(Reverse::Out);
            }
            s.brake = pedal(s.brake, s.vehicle_brake, k.brake_rate_limit * dt);
            s.brake_force = s.brake;
        } else {
            reverse = Some(Reverse::In);
            s.brake = F::ZERO;
            s.brake_force = F::ZERO;
            s.gas_pedal = F(f32::from_bits(REVERSE_THROTTLE));
        }
        s.gas = if s.throttle_enabled { pedal(s.gas, s.gas_pedal, k.gas_rate_limit * dt) } else { F::ZERO };
    }
    s.engaged_clutch = s.clutch;

    if s.hand_brake == 0.0 {
        if s.hand_brake_fade > 0.0 {
            let f = remap(s.hand_brake_fade, F::ZERO, k.hand_brake_fade_time, F::ONE, k.hand_brake_friction_scale);
            s.rear_friction = [f; 2];
            s.hand_brake_fade = s.hand_brake_fade - dt;
        } else {
            s.rear_friction = [F::ONE; 2];
            car.yaw_rate_limit = k.yaw_rate_limit;
        }
    } else {
        s.rear_friction = [k.hand_brake_friction_scale; 2];
        car.yaw_rate_limit = k.hand_brake_yaw_rate_limit;
        s.brake_force = remap(car.speed, F::ZERO, F(HAND_BRAKE_SPEED), F::ONE, k.hand_brake_brake_scale);
        s.hand_brake_fade = k.hand_brake_fade_time;
    }
    reverse
}

/// The body's set-centre-of-mass (its vtable's `+0x5c`): the position and the last position move by
/// the change, turned into the world, so the car stays put; then the centre is `cofg`, whole.
fn set_cofg(car: &mut Car, body: &mut Body, cofg: Vf) {
    let moved = crate::chassis::turn(body.quat, cofg.merge(Dest::XYZ, cofg.sub(car.cofg)));
    body.position = plus(body.position, moved);
    body.last_position = plus(body.last_position, moved);
    car.cofg = cofg;
}

/// **The centre of mass's travel** (vt `+0x470`, MX Unleashed's [`crate::car::travel_cofg`]): the
/// class's `CofG` (the AI's `CofGAI` unless a person drives) slid back along `z` by the forward travel
/// times a forward (negative) lean, or the back travel times any other, through `adda.s`/`madd.s`
/// where MX Unleashed multiplies and adds; the body handed it through its own setter.
fn travel_cofg(k: &Keys, s: &State, car: &mut Car, body: &mut Body) {
    let mut cofg = if s.human { k.cofg } else { k.cofg_ai };
    let travel = if s.lean < 0.0 { k.cofg_travel_forward } else { k.cofg_travel_back };
    cofg.0[2] = Acc::adda(F::ZERO, cofg.z()).madd(-travel, s.lean).0;
    set_cofg(car, body, cofg);
}

/// Under this speed (along the car) the anti-slide holds a car off the gas.
const ANTI_SLIDE_SPEED: f32 = 10.0;

/// **The anti-slide** (vt `+0x478`, MX Unleashed's): grounded, off the gas and slower than
/// [`ANTI_SLIDE_SPEED`] along the car, gravity's pull along the left front's ground is pushed back
/// against (by a remap from 1 to 1, so wholly), and while either brake word is on, the share along the
/// car's heading on that ground too.
fn anti_slide(s: &State, car: &Car, body: &mut Body) {
    let speed = F(car.velocity.z());
    if !car.grounded || s.gas != 0.0 || !(speed.abs() < ANTI_SLIDE_SPEED) {
        return;
    }
    let (n, g) = (s.front_normal, s.gravity);
    let along = minus(g, scaled(n, dot(n, g)));
    let v = car.forward;
    let mut heading = minus(v, scaled(n, dot(v, n)));
    let len2 = dot(heading, heading);
    if len2 == 0.0 {
        heading = CLEAR;
    } else {
        let r = F::ONE.rsqrt(len2);
        for k in 0..3 {
            heading.0[k] = (F(heading.0[k]) * r).0;
        }
    }
    let ahead = scaled(heading, dot(heading, along));
    let rest = minus(along, ahead);
    let mut force = scaled(rest, -remap(speed, F::ZERO, F(ANTI_SLIDE_SPEED), F::ONE, F::ONE));
    if s.vehicle_brake != 0.0 || s.hand_brake != 0.0 {
        force = minus(force, ahead);
    }
    body.force = plus(body.force, force);
}

/// The preload's thresholds: the body rising faster than this, the car faster than this along
/// itself, a lean past this either way.
const PRELOAD_RISE: f32 = 5.0;
const PRELOAD_SPEED: f32 = 30.0;
const PRELOAD_LEAN: u32 = 0x3f4c_cccd;

/// **The preload** (vt `+0x47c`, MX Unleashed's, with SLUS-21104's cue): nothing in a wrecked state.
/// Pushing, the timer runs down and the push is added while time is left. Else, grounded, rising,
/// fast and off the brake, it is ready: a forward lean past 0.8 arms it (the timer cleared), the timer
/// winds while armed and the lean is within 0.8, and an armed back lean past 0.8 fires it, the push's
/// `y` the pad word's size remapped from the preload times onto up to `MaxPreload`, for
/// `PreloadTime`. Then the cue: played (`true`) when the timer is not run out and the cue has not
/// played; forgotten once the timer has run out.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's `c.ole.s` branched on false
fn preload(k: &Keys, s: &mut State, car: &Car, body: &mut Body, h: F) -> bool {
    if car.state != RIDING {
        return false;
    }
    let lean = F(f32::from_bits(PRELOAD_LEAN));
    let p = &mut s.preload;
    if p.active {
        p.timer = p.timer - h;
        if p.timer <= 0.0 {
            p.active = false;
        } else {
            body.force = plus(body.force, p.force);
        }
    } else if !car.grounded
        || F(body.velocity.y()) <= PRELOAD_RISE
        || F(car.velocity.z()) <= PRELOAD_SPEED
        || s.vehicle_brake != 0.0
    {
        p.ready = false;
    } else {
        p.ready = true;
        if !(s.lean <= lean) {
            p.armed = true;
            p.timer = F::ZERO;
        } else if !(s.lean < -lean) {
            if p.armed {
                p.timer = p.timer + h;
            }
        } else if p.armed {
            p.force.0[1] = remap(s.pad_word.abs(), k.min_preload_time, k.max_preload_time, F::ZERO, k.max_preload).0;
            p.timer = k.preload_time;
            p.armed = false;
            p.active = true;
        }
    }
    if !p.cued {
        if !(p.timer <= 0.0) {
            p.cued = true;
            return true;
        }
    } else if p.timer <= 0.0 {
        p.cued = false;
    }
    false
}

/// **The launch** (vt `+0x480`, MX Unleashed's): the clutch in clears the timer; let out from
/// standing (the clutch word 0) and within `ClutchTorqueScaleTime`, the scale starts (at the timer's
/// first step) from `MaxClutchTorqueScale`, less with speed up to `MaxClutchTorqueScaleSpeed`, and
/// eases back to 1 over the time: the engine's torque scale.
fn launch(k: &Keys, s: &mut State, car: &Car, h: F) {
    if s.clutch == 1.0 {
        s.launch_timer = F::ZERO;
        return;
    }
    if s.clutch != 0.0 || !(s.launch_timer <= k.clutch_torque_scale_time) {
        return;
    }
    if s.launch_timer == 0.0 {
        let speed = F(car.velocity.z());
        s.launch_start = remap(speed, F::ZERO, k.max_clutch_torque_scale_speed, k.max_clutch_torque_scale, F::ONE);
    }
    s.launch_timer = s.launch_timer + h;
    s.launch_boost = remap(s.launch_timer, F::ZERO, k.clutch_torque_scale_time, s.launch_start, F::ONE);
}

/// The air's density over two, times the speed squared, makes the wings' pressure.
const AIR: u32 = 0x3a9b_c5c4;
/// The air control needs the car faster than this, its stick within this of centre to unroll, and
/// its up row's height above this.
const AIR_CONTROL_SPEED: f32 = 10.0;
const UNROLL_STICK: u32 = 0x3dcc_cccd;
const UNROLL_UP: u32 = 0x3eae_147b;

/// **The aero** (vt `+0x474`, MX Unleashed's): nothing wrecked. Moving forward, each wing's lift and
/// drag (its lift slope times alpha plus its angle; the drag its basic plus the lift coefficient
/// squared times the induced, through `adda.s`/`madd.s`) pushed at the wing through the body's
/// add-force-at. Then off the ground, touching nothing and faster than [`AIR_CONTROL_SPEED`], the air
/// control in the car's frame: the stick's moment (lean on `x`, steer on `y` and `z`) less the rates'
/// damping; near the stick's centre and not upside down the unroll and unyaw (`adda.s`/`madd.s`);
/// with either brake word on, the brakes' gyro on `x`.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's `c.ole.s` branched on false
fn aero(k: &Keys, s: &State, car: &Car, body: &mut Body) {
    if car.state != RIDING {
        return;
    }
    if !(F(car.velocity.z()) <= 0.0) {
        let q = F(f32::from_bits(AIR)) * car.speed * car.speed;
        for w in &k.wings {
            let cl = w.lift_slope * (car.alpha + w.angle_of_attack);
            let cd = Acc::adda(F::ZERO, w.basic_drag).madd(cl, cl * w.induced_drag);
            let force = Vf::new(0.0, (k.aero_surface_area * (q * cl)).0, (k.aero_surface_area * (-q * cd)).0, 1.0);
            let p = w.position;
            let m = p.merge(Dest::XYZ, Vf::opmsub(Vf::opmula(p, force), force, p));
            body.moment = plus(body.moment, m.merge(Dest::XYZ, m.mul(ONES)));
            body.force = plus(body.force, crate::chassis::turn(body.quat, force));
        }
    }
    if car.grounded || s.touching || car.speed <= AIR_CONTROL_SPEED {
        return;
    }
    let cm = k.airborne_control_moment;
    let m = Vf::new((F(cm.x()) * s.lean).0, (F(cm.y()) * s.stick).0, (F(cm.z()) * s.stick).0, 1.0);
    let r = body.rates;
    let mut m = minus(m, scaled(r, k.airborne_damping));
    if s.stick.abs() < F(f32::from_bits(UNROLL_STICK)) && !(F(car.up.y()) <= F(f32::from_bits(UNROLL_UP))) {
        m.0[2] = Acc::adda(F::ZERO, m.z()).madd(k.unroll_torque, k.unroll_gain * F(car.euler.z()) - F(r.z())).0;
        m.0[1] = Acc::adda(F::ZERO, m.y()).madd(k.unyaw_torque, k.unyaw_gain * car.side_slip - F(r.y())).0;
    }
    if s.brake != 0.0 || s.brake_force != 0.0 {
        m.0[0] = (F(m.x()) + k.brake_gyro_torque).0;
    }
    body.moment = plus(body.moment, m);
}
