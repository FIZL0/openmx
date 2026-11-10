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
//! Proof: `tests/mxvatv_car_frame.rs`, SLUS-21104's world step off MD39a's buggy image with every
//! call hooked.

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
    TravelCofg,
    SortContacts,
    Probe(usize, F),
    /// The Force body's: the vehicle's gravity and collision record (first, always), the anti-slide
    /// (vt `+0x478`); riding, the preload (vt `+0x47c`), the launch (vt `+0x480`) and the engine's
    /// step; each tire's step (its suspension, roll and forces); the aero (vt `+0x474`).
    VehicleForce,
    AntiSlide(F),
    Preload(F),
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
