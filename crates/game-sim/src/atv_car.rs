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
    /// The update's ([`update`]): the vehicle's ghost step (vt `+0x3a4`, as the quad's); the
    /// vehicle's hazard-surface clock (vt `+0x45c`, the quad's [`crate::quad::Call::HazardSurface`]),
    /// handed the longest of the four tires' surface times and whether a tire is sunk; the class's own
    /// per-frame slot (vt `+0x488`); and last, always, the vehicle's base update.
    Ghost(F),
    HazardClock { dt: F, time: F, sunk: bool },
    ClassStep,
    VehicleUpdate(F),
    /// What SLUS-21104's own calls hand out in turn, when [`Own`] answers them (MD106b). The vehicle
    /// force's: the collision record told of the gravity (its vt `+0x88`, with 1). The vehicle post's:
    /// outside the world, the body's reset (vt `+0x13c`) and the car's respawn (vt `+0xe8`, which the
    /// unwreck makes too); the spatial update (vt `+0xe4`), the node's bounds (with bit 1 of the flag
    /// byte) and the collision sync (vt `+0x124`).
    GravityRecord,
    BodyReset,
    Respawn,
    SpatialUpdate,
    NodeBounds,
    CollisionSync,
    /// The wreck timer's: the map-edge test and throw (vt `+0x16c`, answered into
    /// [`Vehicle::out_of_bounds`]); the wreck's pad rumble, when the vehicle state is the player's and
    /// has a pad (the rumble's own switch is the call's to test); a sound cue posted with its kind (11
    /// the wreck, 12 the map-edge wreck's clock started, 13 the unwreck); the car's wrecked look put on
    /// (`true`, its vt `+0x94`) or taken off (its vt `+0x90`).
    OutOfBounds,
    Rumble,
    Cue(u32),
    WreckLook(bool),
    /// The landing's sound played with the shock (or velocity) in the node's frame.
    LandingSound(Vf),
    /// The car's node placed while current, with a subtree under it (its model's parts, past what
    /// [`Vehicle::scene`] holds): every current node of the subtree marked out of date.
    SubtreeOutOfDate,
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

/// The car's state word in the two wrecks its own wreck timer starts: thrown back off the map's edge,
/// and tipped over and still.
pub const WRECK_EDGE: u32 = 13;
pub const WRECK_TIPPED: u32 = 14;

/// Under this ground speed the car counts as still, for the tipped wreck and both unwrecks.
const STILL_SPEED: f32 = 10.0;
/// Below this (the node's up row on the world's up) the car is tipped over enough to wreck.
const TIPPED_UP: u32 = 0x3f4c_cccd; // 0.8
/// A tipped wreck's clock starts this far short of `MaxWreckTime`.
const TIPPED_HEAD_START: f32 = 2.0;
/// What a map-edge wreck keeps of the body's velocity as it unwrecks.
const EDGE_KEEP: u32 = 0x3d75_c28f; // 0.06
/// The wreck spins the body's rates up by this.
const WRECK_SPIN: u32 = 0x401a_e148; // 2.42
/// What the unwreck keeps of the body's velocity.
const UNWRECK_KEEP: u32 = 0x3df5_c28f; // 0.12
/// The map-edge throw halves the world's drag word; the unwreck from it divides it back by this.
const EDGE_DRAG: f32 = 0.5;
/// The rodata `(1, 1, 1, 1)` the unwreck puts both dampings back to.
const UNDAMPED: Vf = Vf([1.0, 1.0, 1.0, 1.0]);

/// What SLUS-21104's own calls in the car's frame read and write beyond [`Car`] and the body
/// (research/oracle.txt `# MD106b`): MXVehicle's wreck words, the world's two words the wreck and the
/// force touch, the flag byte, the scene node and the world's bounds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vehicle {
    /// Wrecks are on for this car; without, the wreck timer does nothing.
    pub wrecks_on: bool,
    /// The car is held to the map: the wreck timer asks [`Call::OutOfBounds`].
    pub bounded: bool,
    /// The wreck clock, `MaxWreckTime`, and whether the clock is counting to the unwreck.
    pub clock: F,
    pub max_wreck_time: F,
    pub counting: bool,
    /// Set by the tipped wreck, cleared by the unwreck.
    pub tipped_wreck: bool,
    /// The vehicle's kind: 0 and 1 keep their body's damping and inertia through a wreck.
    pub kind: u32,
    /// `WreckDamping`, `WreckInertia`, and the body's own inertia, the wreck's is a multiple of.
    pub wreck_damping: Vf,
    pub wreck_inertia: Vf,
    pub inertia: Vf,
    /// The inertia the body holds now (its inverse is the body's [`Body::inv_inertia`]).
    pub body_inertia: Vf,
    /// The vehicle state is the player's with a pad: the wreck asks for [`Call::Rumble`].
    pub rumble: bool,
    /// The car has a wrecked look ([`Call::WreckLook`]). The wreck uses it without asking.
    pub look: bool,
    /// The collision record is touching something; and there is a record.
    pub touching: bool,
    pub record: bool,
    /// The world's gravity force, added each substep, and its drag word.
    pub gravity: Vf,
    pub drag: F,
    /// The vehicle's flag byte: bit 0 holds it in the world's box, bit 6 over its floor; bit 1 asks
    /// the post for the node's bounds; the force clears bit 5, the post bit 2.
    pub flags: u8,
    /// The car's node (node 0) and its parents, and the world's bounds. Node 0's subtree is not held:
    /// `subtree` says it has one, which placing the node hands out ([`Call::SubtreeOutOfDate`]).
    pub scene: crate::atv_moto::Scene,
    pub subtree: bool,
    pub bounds: crate::atv_moto::Bounds,
    /// [`Call::OutOfBounds`]'s answer.
    pub out_of_bounds: bool,
}

/// **SLUS-21104's own calls answered** (MD106b): the vehicle force and post, the wreck timer, the
/// landing's node-local turn and the node's position, each ported; every other call, and what these
/// hand out, goes on to `rest`.
pub struct Own<'a, S> {
    pub v: &'a mut Vehicle,
    pub rest: &'a mut S,
}

impl<S: Steps> Steps for Own<'_, S> {
    fn call(&mut self, c: Call, car: &mut Car, body: &mut Body) {
        let v = &mut *self.v;
        let s = &mut *self.rest;
        match c {
            Call::VehicleForce => vehicle_force(v, body, car, s),
            Call::VehiclePost(_) => vehicle_post(v, car, body, s),
            Call::WreckTimer(dt) => wreck_timer(v, car, body, dt, s),
            Call::Landed(shock) => {
                let out = node_local(&mut v.scene, 0, shock);
                s.call(Call::LandingSound(out), car, body);
            }
            Call::NodePosition => car.node_pos = v.scene.position(0),
            c => s.call(c, car, body),
        }
    }
}

/// **The vehicle's force**, the Force body's first call: with the body enabled, the world's gravity
/// into its force (`xyz`), the collision record told ([`Call::GravityRecord`]) if there is one, and
/// bit 5 of the flag byte cleared. With the body off, nothing.
pub fn vehicle_force(v: &mut Vehicle, body: &mut Body, car: &mut Car, s: &mut impl Steps) {
    if !body.enabled {
        return;
    }
    body.force = body.force.merge(Dest::XYZ, body.force.add(v.gravity));
    if v.record {
        s.call(Call::GravityRecord, car, body);
    }
    v.flags &= !0x20;
}

/// **The vehicle's post step**, the PostProcess's first call: with the body enabled, the car's
/// origin (the body's position less its centre of mass turned into the world,
/// [`crate::atv_moto::origin`]) tested against the world ([`crate::atv_moto::in_world`]); inside, the
/// node placed there with the body's orientation (its subtree marked out of date by
/// [`Call::SubtreeOutOfDate`]); outside, [`Call::BodyReset`] and [`Call::Respawn`].
/// Then [`Call::SpatialUpdate`], [`Call::NodeBounds`] with bit 1 of the flag byte, and
/// [`Call::CollisionSync`]; bit 2 cleared. The quad's is the same code ([`crate::atv_moto::vehicle_post`]).
pub fn vehicle_post(v: &mut Vehicle, car: &mut Car, body: &mut Body, s: &mut impl Steps) {
    if !body.enabled {
        return;
    }
    let at = crate::atv_moto::origin(&body.view(), car.cofg);
    if crate::atv_moto::in_world(v.flags, &v.bounds, at) {
        let current = v.scene.nodes[0].flags & 1 != 0;
        v.scene.place(0, body.quat, at);
        if current && v.subtree {
            s.call(Call::SubtreeOutOfDate, car, body);
        }
    } else {
        s.call(Call::BodyReset, car, body);
        s.call(Call::Respawn, car, body);
    }
    s.call(Call::SpatialUpdate, car, body);
    if v.flags & 0x02 != 0 {
        s.call(Call::NodeBounds, car, body);
    }
    s.call(Call::CollisionSync, car, body);
    v.flags &= !0x04;
}

/// A world direction into node `n`'s frame (the landing's call): its world frame brought up to date,
/// then the transposed rows times the direction ([`crate::chassis::unrotate`]'s packing), `w` kept.
pub fn node_local(scene: &mut crate::atv_moto::Scene, n: usize, d: Vf) -> Vf {
    scene.update(n);
    let w = &scene.nodes[n].world;
    crate::chassis::unrotate(&crate::chassis::Mat3 { rows: [w[0], w[1], w[2]] }, d)
}

/// The ground velocity's length, as the wreck timer measures it (`mula`, `madda`, `madd`, `sqrt`).
fn ground_speed(car: &Car) -> F {
    let g = car.ground_velocity;
    Acc::mula(g.x(), g.x()).madda(g.y(), g.y()).madd(g.z(), g.z()).sqrt()
}

/// `v.xyz * k`, `w` kept (a `vmulx.xyz` of a scalar moved in).
fn scale_xyz(v: Vf, k: u32) -> Vf {
    v.merge(Dest::XYZ, v.mul(Vf::splat(f32::from_bits(k))))
}

/// The body's inertia set (its vt `+0x60`): `w` 1 (in the caller's copy too), kept, and each lane's
/// reciprocal into the inverse, or 0 for a lane not above 0; the inverse's `w` stays.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's `c.ole.s` branched on true
fn set_inertia(v: &mut Vehicle, body: &mut Body, mut i: Vf) -> Vf {
    i.0[3] = 1.0;
    v.body_inertia = i;
    for k in 0..3 {
        body.inv_inertia.0[k] = if F(i.0[k]) <= F::ZERO { 0.0 } else { (F::ONE / F(i.0[k])).0 };
    }
    i
}

/// **The car's wreck timer** (vt `+0x484`, SLUS-21104's own; MX Unleashed's car has none), once a
/// PostProcess, with wrecks on:
///
/// - held to the map and not already thrown off its edge: off it ([`Call::OutOfBounds`]), the
///   [`WRECK_EDGE`] wreck;
/// - riding: tipped over and still — under [`STILL_SPEED`] on the ground velocity, the record touching,
///   on one side or in the air, and the node's up row under [`TIPPED_UP`] on the world's up — the
///   [`WRECK_TIPPED`] wreck, its clock started [`TIPPED_HEAD_START`] short of `MaxWreckTime`;
/// - wrecked (the state read again, so a fresh map-edge wreck counts this frame): the clock runs. Off
///   the edge, past `MaxWreckTime` the clock starts again counting (cue 12); counting, past it again
///   or still, the body keeps [`EDGE_KEEP`] of its velocity and the car unwrecks. Tipped, not yet
///   counting, past it and still: the unwreck, and the clock starts again counting.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's `c.ole.s` / `c.olt.s` branched on false
pub fn wreck_timer(v: &mut Vehicle, car: &mut Car, body: &mut Body, dt: F, s: &mut impl Steps) {
    if !v.wrecks_on {
        return;
    }
    if v.bounded && car.state != WRECK_EDGE {
        s.call(Call::OutOfBounds, car, body);
        if v.out_of_bounds {
            wreck(v, car, body, WRECK_EDGE, s);
        }
    }
    if car.state == RIDING {
        // The node's own frame's up row, dotted with the rodata's (0, 1, 0).
        let up = v.scene.nodes[0].local[1];
        let level = Acc::mula(up.x(), F::ZERO).madda(up.y(), F::ONE).madd(up.z(), F::ZERO);
        if ground_speed(car) < F(STILL_SPEED)
            && v.touching
            && (car.one_side || !car.grounded)
            && level < F(f32::from_bits(TIPPED_UP))
        {
            wreck(v, car, body, WRECK_TIPPED, s);
            v.clock = v.max_wreck_time - F(TIPPED_HEAD_START);
            v.tipped_wreck = true;
        }
        return;
    }
    v.clock = v.clock + dt;
    if car.state == WRECK_EDGE {
        if !v.counting && !(v.clock <= v.max_wreck_time) {
            v.counting = true;
            v.clock = F::ZERO;
            s.call(Call::Cue(12), car, body);
        }
        if v.counting && (!(v.clock <= v.max_wreck_time) || ground_speed(car) < F(STILL_SPEED)) {
            body.velocity = scale_xyz(body.velocity, EDGE_KEEP);
            unwreck(v, car, body, s);
        }
    } else if car.state == WRECK_TIPPED
        && !v.counting
        && !(v.clock <= v.max_wreck_time)
        && ground_speed(car) < F(STILL_SPEED)
    {
        unwreck(v, car, body, s);
        v.counting = true;
        v.clock = F::ZERO;
    }
}

/// **The car's wreck** (vt `+0x368`): MXVehicle's — the state set, the clock stopped at 0, the
/// rumble ([`Call::Rumble`]) if it is the player's; unless the vehicle is of kind 0 or 1, both the
/// body's dampings `WreckDamping` and its inertia `WreckInertia` times its own (`xyz`); cue 11 — then
/// the car's: the body's rates spun up by [`WRECK_SPIN`], and the wrecked look on.
pub fn wreck(v: &mut Vehicle, car: &mut Car, body: &mut Body, state: u32, s: &mut impl Steps) {
    car.state = state;
    v.clock = F::ZERO;
    v.counting = false;
    if v.rumble {
        s.call(Call::Rumble, car, body);
    }
    if v.kind != 1 && v.kind != 0 {
        let i = v.wreck_inertia.merge(Dest::XYZ, v.wreck_inertia.mul(v.inertia));
        body.damping = v.wreck_damping;
        body.ang_damping = v.wreck_damping;
        set_inertia(v, body, i);
    }
    s.call(Call::Cue(11), car, body);
    body.rates = scale_xyz(body.rates, WRECK_SPIN);
    s.call(Call::WreckLook(true), car, body);
}

/// **The car's unwreck** (vt `+0x36c`, with 1): the car's — the latch set, the tipped wreck's flag
/// cleared, the body keeping [`UNWRECK_KEEP`] of its velocity, the wrecked look off if there is one,
/// the respawn ([`Call::Respawn`]) — then MXVehicle's: out of a map-edge wreck the world's drag
/// divided back by [`EDGE_DRAG`]; riding, the clock at 0 (counting is left as it is); unless the
/// vehicle is of kind 0 or 1, both dampings back to [`UNDAMPED`] and the body's own inertia back
/// (its `w` set to 1 where it is kept); cue 13.
pub fn unwreck(v: &mut Vehicle, car: &mut Car, body: &mut Body, s: &mut impl Steps) {
    car.latch = true;
    v.tipped_wreck = false;
    body.velocity = scale_xyz(body.velocity, UNWRECK_KEEP);
    if v.look {
        s.call(Call::WreckLook(false), car, body);
    }
    s.call(Call::Respawn, car, body);
    if car.state == WRECK_EDGE {
        v.drag = v.drag / F(EDGE_DRAG);
    }
    car.state = RIDING;
    v.clock = F::ZERO;
    if v.kind != 1 && v.kind != 0 {
        body.damping = UNDAMPED;
        body.ang_damping = UNDAMPED;
        let own = v.inertia;
        v.inertia = set_inertia(v, body, own);
    }
    s.call(Call::Cue(13), car, body);
}
