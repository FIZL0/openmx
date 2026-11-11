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
    /// The probe's and the contact sort's (MD127, [`Pieces`]), which the tire step reads too. Where the axle's top hangs (the car's
    /// frame) and the tire's radius; whether the tire has a ground to ask (without, the probe finds
    /// nothing).
    pub mount: Vf,
    pub radius: F,
    pub on_ground: bool,
    /// The last ground normal the probe found (world), and it in the car's frame; the axle's drop
    /// from the mount to the bottom of the tire along it (the car's frame), the bottom against the
    /// centre of mass, and the axle's top and bottom in the world.
    pub probe_normal: Vf,
    pub car_normal: Vf,
    pub drop: Vf,
    pub lever: Vf,
    pub upper: Vf,
    pub lower: Vf,
    /// How many times this frame's probe asked the ground.
    pub probes: i32,
    /// The contact: the ground's point and normal the probe found, or the sorted hit's.
    pub ground_point: Vf,
    pub ground_normal: Vf,
    /// The terrain holds the tire over its sorted contact (kept from frame to frame).
    pub held: bool,
    /// The surface byte the ground query leaves (the probe clears it under a sorted tire that the
    /// ground is below).
    pub surface: u8,
    /// The sorted hit's own record, as the sort handed it over.
    pub hit: u32,
    /// The ground query's answer to [`Call::TireGround`]: the point moved onto the ground, and the
    /// ground's normal.
    pub query: (Vf, Vf),
}

/// One of the contact sort's ray hits (MD127), as [`Call::SortSearch`] answers it: where the ray hangs
/// (the car's frame), the point and normal struck, the struck object (zero for none), the mover
/// under it with its kind word (kept only when that is 2), and the hit's own record.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SortHit {
    pub from: Vf,
    pub point: Vf,
    pub normal: Vf,
    pub object: u32,
    pub mover: u32,
    pub mover_kind: u32,
    pub record: u32,
}

/// The most hits the sort's search answers: one a ray, four rays.
pub const SORT_HITS: usize = 4;

/// The contact sort's search, as [`Call::SortSearch`] answers it: whether it struck anything, and the
/// hits.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SortAnswer {
    pub found: bool,
    pub count: usize,
    pub hits: [SortHit; SORT_HITS],
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
    /// The car has its sort rays (without, the contact sort does nothing); and the search's answer
    /// (MD127).
    pub sort_rays: bool,
    pub sort: SortAnswer,
    /// The shared smooth noise's answer to [`Call::Noise`] (MD125).
    pub noise: F,
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
    /// What the engine hands out (MD125, [`Pieces`]): its sound's wobble at the bottom of the revs
    /// asks the shared smooth noise at a time (into [`Car::noise`]); a shift, with the engine's sound,
    /// plays the shift on it.
    Noise(F),
    ShiftSound,
    /// What the contact sort and the probe hand out (MD127, [`Pieces`]): the sort's ray probe placed
    /// at the car's frame ([`Car::matrix`], its vt `+0xcc`) and searched (into [`Car::sort`]); a
    /// struck object's listeners told of the hit (each of them, vt `+0xc` with 0, 0), by the hit's
    /// index; the ground query under a tire's axle bottom (the point asked; into [`Tire::query`] and
    /// [`Tire::surface`]).
    SortPlace,
    SortSearch,
    Struck(usize),
    TireGround(usize, Vf),
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
    /// Each tire's keys (MD124).
    pub tires: [TireKeys; 4],
    /// The engine's (MD125): with them [`Pieces`] runs the engine's step, its shift and its reverse
    /// switch; without, those go to the rest.
    pub engine: Option<EngineKeys>,
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
    /// The steer after its rate limit; the front and rear steer angles (the words the tires' steer
    /// pointers read, MD124); the clutch the engine reads; what is left of the hand brake's fade; the
    /// two rear tires' friction scales (left, right).
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
    pub preload: PreloadState,
    /// The launch's timer and the scale it started from, and the engine's torque scale it sets.
    pub launch_timer: F,
    pub launch_start: F,
    pub launch_boost: F,
    /// Each tire's own state (MD124).
    pub tires: [TireState; 4],
    /// The engine's running words (MD125); its reverse word is [`State::reversing`], and the launch's
    /// torque scale [`State::launch_boost`].
    pub engine: EngineState,
}

/// **The car's [`Steps`] with MX Unleashed's pieces ported** (MD106a, MD126), as SLUS-21104 builds
/// them: the controls, the centre of mass's travel, the anti-slide, the preload, the launch and the
/// aero, and (MD124) each tire's step ([`tire_step`]), run on [`Keys`] and [`State`]; and (MD127) the
/// contact sort and each tire's probe, on the [`Car`]'s and [`Tire`]s' words; and (MD125), given the
/// engine's keys, the engine's step, its shift and its reverse switch. Every other call, the
/// controls' reverse switch without the engine's keys, the preload's cue, a tire step on or handed a
/// mover's contact, and what the sort, the probe and the engine hand out go to `rest`.
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
                    // The engine's reverse switch, the engine's piece's when it has one.
                    if !self.answer(Call::Reverse(r), car, body) {
                        self.rest.call(Call::Reverse(r), car, body);
                    }
                }
                // The rears' friction scales are the words their tire steps read (MD124's keys).
                self.keys.tires[2].friction_scale = self.state.rear_friction[0];
                self.keys.tires[3].friction_scale = self.state.rear_friction[1];
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
            Call::TireStep(i, h) => {
                // What stands on a mover (its velocity and its contact list) is the rest's.
                let t = car.tires[i];
                if t.stands_on != [0, 0] || (t.handed && t.handed_on != [0, 0]) {
                    return false;
                }
                let mut s = self.state.tires[i];
                tire_step(&self.keys.tires[i], &mut car.tires[i], &mut s, &self.state, car.cofg, body, h);
                self.state.tires[i] = s;
            }
            Call::SortContacts => sort_contacts(car, body, &mut self.rest),
            Call::Probe(k, _) => probe(k, car, body, &mut self.rest),
            Call::Engine(h) => {
                let Keys { engine: Some(e), tires, .. } = &mut self.keys else { return false };
                engine_step(e, tires, &mut self.state, car, body, h, &mut self.rest);
            }
            Call::Shift(dt) => {
                let Keys { engine: Some(e), tires, .. } = &mut self.keys else { return false };
                if shift(e, tires, &mut self.state, car, dt) && e.sound {
                    self.rest.call(Call::ShiftSound, car, body);
                }
            }
            Call::Reverse(r) => {
                let Keys { engine: Some(e), tires, .. } = &mut self.keys else { return false };
                reverse(e, tires, &mut self.state, r);
            }
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
    let (n, g) = (car.tires[FRONT_LEFT].ground_normal, s.gravity);
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

// --- the contact sort and the tire probe (MD127) ---------------------------------------------------

/// The cosine (5°) two of the probe's normals must agree past before it stops asking the ground: a
/// literal here, where MX Unleashed's probe ([`crate::car::Wheel::probe`]) takes it off its curve table.
const PROBE_AGREE: u32 = 0x3f7f_069e;
/// The most times the probe asks.
const PROBE_TRIES: i32 = 5;
/// How far (ft) the terrain must rise past a sorted contact to hold the tire, and fall below it to
/// let go.
const TERRAIN_HOLD: u32 = 0x3dcc_cccd;

/// **The contact sort** (MX Unleashed's [`crate::car::Machine::sort_contacts`]), once a frame between
/// the tires' clear and their probes: with its rays, the sort's ray probe placed at the car's frame
/// and searched ([`Call::SortPlace`], [`Call::SortSearch`]); when it struck anything, each hit in
/// turn its object's listeners told ([`Call::Struck`], when it struck an object) and handed to its
/// tire by where its ray hangs: forward unless `z <= 0`, right unless `x <= 0` (the same as MX
/// Unleashed's `> 0`, with no NaN on the PS2). A hit's mover is kept only when its kind word is 2.
fn sort_contacts(car: &mut Car, body: &mut Body, s: &mut impl Steps) {
    if !car.sort_rays {
        return;
    }
    s.call(Call::SortPlace, car, body);
    s.call(Call::SortSearch, car, body);
    let a = car.sort;
    if !a.found {
        return;
    }
    for (i, h) in a.hits[..a.count.min(SORT_HITS)].iter().enumerate() {
        if h.object != 0 {
            s.call(Call::Struck(i), car, body);
        }
        let mover = if h.mover != 0 && h.mover_kind == 2 { h.mover } else { 0 };
        let k = match (F(h.from.z()) <= F::ZERO, F(h.from.x()) <= F::ZERO) {
            (false, false) => FRONT_RIGHT,
            (false, true) => FRONT_LEFT,
            (true, false) => REAR_RIGHT,
            (true, true) => REAR_LEFT,
        };
        set_contact(&mut car.tires[k], body, h, mover);
    }
}

/// The axle's drop along a normal in the car's frame: the radius down its `yz`, unit-scaled.
fn drop_along(car_normal: Vf, radius: F) -> Vf {
    let dir = crate::contact::unit(Vf::new(0.0, car_normal.y(), car_normal.z(), 1.0));
    scaled(dir, -radius)
}

/// The sort's hand-off (MX Unleashed's [`crate::car::Wheel::set_contact`]): the hit's point and normal
/// as the tire's contact, what it is on (the object and the kept mover), the hit's record, and the
/// normal in the car's frame with the drop along it, which MX Unleashed leaves to the probe.
fn set_contact(t: &mut Tire, body: &Body, h: &SortHit, mover: u32) {
    t.ground_point = h.point;
    t.ground_normal = h.normal;
    t.stands_on = [h.object, mover];
    t.hit = h.record;
    t.car_normal = crate::biplane::unturn(body.quat, h.normal);
    t.drop = drop_along(t.car_normal, t.radius);
    t.sorted = true;
}

/// **A tire's probe** (MX Unleashed's [`crate::car::Wheel::probe`]), once a frame: with a ground to
/// ask, the axle's top placed; then the axle's bottom along last frame's normal and the ground under
/// it ([`Call::TireGround`]), asked again from the new normal until two agree within 5° or it has
/// asked five times. A sorted tire keeps its contact unless its normal is 45° or more off the car's
/// up; with the ground below the axle's bottom the tire is not solid (and a sorted one's surface byte
/// is cleared); else a sorted tire takes the terrain only while the terrain holds it, and any other
/// is solid on it.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
fn probe(k: usize, car: &mut Car, body: &mut Body, s: &mut impl Steps) {
    car.tires[k].solid = false;
    if !car.tires[k].on_ground {
        return;
    }
    let q = body.quat;
    let mut t = car.tires[k];
    t.upper = plus(crate::chassis::turn(q, minus(t.mount, car.cofg)), body.position);
    t.probes = 0;
    let mut d = F::ZERO;
    let (mut point, mut normal) = (Vf::ZERO, Vf::ZERO);
    while d < F::from_bits(PROBE_AGREE) && t.probes < PROBE_TRIES {
        t.car_normal = crate::biplane::unturn(q, t.probe_normal);
        t.drop = drop_along(t.car_normal, t.radius);
        t.lever = minus(plus(t.mount, t.drop), car.cofg);
        t.lower = plus(crate::chassis::turn(q, t.lever), body.position);
        car.tires[k] = t;
        s.call(Call::TireGround(k, t.lower), car, body);
        t = car.tires[k];
        (point, normal) = t.query;
        normal = crate::contact::unit(normal);
        d = dot(normal, t.probe_normal);
        t.probe_normal = normal;
        t.probes += 1;
    }
    let facing = F::from_bits(UPRIGHT);
    if t.sorted && dot(car.up, t.ground_normal) < facing {
        t.sorted = false;
    }
    if F(point.y()) < F(t.lower.y()) {
        if t.sorted {
            t.surface = 0;
        }
        car.tires[k] = t;
        return;
    }
    if t.sorted {
        let (ground, sorted) = (F(point.y()), F(t.ground_point.y()));
        let hold = F::from_bits(TERRAIN_HOLD);
        let axle = crate::contact::unit(minus(t.upper, t.lower));
        if dot(axle, t.ground_normal) < facing {
            t.held = true;
        } else if !t.held {
            if !(ground <= hold + sorted) {
                t.held = true;
            }
        } else if !(sorted <= hold + ground) {
            t.held = false;
        }
        if !t.held {
            car.tires[k] = t;
            return;
        }
        t.sorted = false;
    }
    t.solid = true;
    t.ground_point = point;
    t.ground_normal = normal;
    car.tires[k] = t;
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

// --- MX Unleashed's tire step (MD124) ----------------------------------------------------------------

/// Which of the car's lever words a tire's brake or steer pointer reads ([`State`]'s `brake`,
/// `brake_force`, `steer` and `rear_steer`): the fronts' brake reads the brake and their steer the
/// steer, the rears' the brake force and the rear steer. `None` is a null pointer, which skips the
/// read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lever {
    Brake,
    BrakeForce,
    Steer,
    RearSteer,
}

/// One tire's keys, as MD39b maps them: its suspension, wheel and tire model, with its sixteen
/// surfaces.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TireKeys {
    pub brake: Option<Lever>,
    pub steer: Option<Lever>,
    pub max_compression: F,
    pub max_compression_rate: F,
    pub max_decompression_rate: F,
    pub spring: F,
    /// The damping while the contact moves away from the ground (`v > 0`), and towards it.
    pub bounce_damping: F,
    pub rebound_damping: F,
    pub inv_inertia: F,
    pub max_steer_angle: F,
    pub top_wheel_speed: F,
    pub max_tire_angle_rate: F,
    /// The ABS word, non-zero.
    pub abs: bool,
    pub min_braking_slip: F,
    pub max_braking_slip: F,
    pub friction_scale: F,
    pub camber_roll_ratio: F,
    pub surfaces: [crate::car::Surface; 16],
}

/// What one tire's step reads and writes besides [`Tire`]. The words the probe and the step share
/// (the mount, drop and radius, the surface byte, the ground point and normal, the car normal, whose
/// `x` and `y` are the camber's lean, the lever and the axle's top and bottom) are the [`Tire`]'s.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TireState {
    pub compression: F,
    pub last_compression: F,
    pub contact: Vf,
    pub point_velocity: Vf,
    pub body_velocity: Vf,
    pub angle: F,
    pub spin: F,
    pub low_speed_grip: bool,
    pub slipping: bool,
    pub sliding: bool,
    pub locked: bool,
    pub torque: F,
    pub last_torque: F,
    pub slip_ratio: F,
    pub slip_angle: F,
    pub slip_tan: F,
    /// The contact's velocity in the tire's frame, and the last one the low-speed grip compared.
    pub velocity: Vf,
    pub last_velocity: Vf,
    /// The tire's force in its own frame, and the last.
    pub force: Vf,
    pub last_force: Vf,
    pub steer: F,
    pub frame: [Vf; 4],
    pub side_ratio: F,
    pub camber: F,
}

/// The engine's sine and arc cosine tables, built once as the startup code builds them.
fn tables() -> &'static (crate::camera::CurveTable, AcosTable) {
    static TABLES: std::sync::OnceLock<(crate::camera::CurveTable, AcosTable)> = std::sync::OnceLock::new();
    TABLES.get_or_init(|| (crate::camera::CurveTable::build(), AcosTable::build()))
}

const INV_TWO_PI: u32 = 0x3e22_f983;
const TWO_PI: u32 = 0x40c9_0fdb;
const PI: u32 = 0x4049_0fdb;
const HALF_PI: u32 = 0x3fc9_0fdb;
/// The straight wheel's heading, `(0, 0, 1, 1)`.
const AHEAD: Vf = Vf([0.0, 0.0, 1.0, 1.0]);
/// A tire bottomed out past its travel only pushes the body back up when it stands on something
/// solid or its contact is within this many travels.
const BOTTOM_RATIO: f32 = 2.0;
/// Braking past this slip locks the wheel.
const LOCK_SLIP: u32 = 0x3f73_3333; // 0.95
/// Past this slip ratio the wheel is slipping, and past this slip angle sliding.
const SLIP: u32 = 0x3e4c_cccd; // 0.2
/// The slip's own model below this speed along the tire; the low-speed grip below the second.
const FAST_SLIP_SPEED: f32 = 20.0;
const GRIP_SPEED: f32 = 5.0;
const GRIP_SCALE: u32 = 0x3fb3_3333; // 1.4
const GRIP_LOAD: f32 = 100.0;
/// The slip ratio's relaxation rate, and the arc tangent's fit.
const CREEP_RATE: f32 = 5.0;
const ATAN_FIT: u32 = 0x3e8f_5c29; // 0.28
const MAX_SLIP_RATIO: u32 = 0x3f7a_e148; // 0.98
const MAX_CAMBER: u32 = 0x3db8_51ec; // 0.09
/// `4 / pi`, the friction curve's linear term.
const FOUR_OVER_PI: u32 = 0x3fa2_f983;

fn bits(b: u32) -> F {
    F(f32::from_bits(b))
}

/// `x` held inside −1..1 by the original's pair of selects, the high test first.
fn clamp_unit(x: F) -> F {
    if x <= 1.0 {
        if x >= -1.0 { x } else { F(-1.0) }
    } else {
        F::ONE
    }
}

/// An angle wrapped once into `-pi..pi` as the inlined wraps do it: the whole turns off through
/// `adda.s`/`msub.s`, then one subtraction or addition.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's `c.ole.s` branched on false
fn wrap(a: F) -> F {
    let turns = F::from((bits(INV_TWO_PI) * a).to_i32());
    let a = Acc::adda(F::ZERO, a).msub(bits(TWO_PI), turns);
    if !(a <= bits(PI)) {
        a - bits(TWO_PI)
    } else if a < -bits(PI) {
        bits(TWO_PI) + a
    } else {
        a
    }
}

/// The body's add-force-at (its vt `+0x84`): the force, and its moment about the centre of mass
/// turned into the body's frame and scaled lane by lane.
fn add_force_at(body: &mut Body, f: Vf, p: Vf, scale: Vf) {
    let r = minus(p, body.position);
    let c = r.merge(Dest::XYZ, Vf::opmsub(Vf::opmula(r, f), f, r));
    let m = crate::biplane::unturn(body.quat, c);
    body.moment = plus(body.moment, m.merge(Dest::XYZ, m.mul(scale)));
    body.force = plus(body.force, f);
}

/// The body's add-velocity (its vt `+0x4c`): into the velocity and the correction both.
fn add_velocity(body: &mut Body, v: Vf) {
    body.velocity = plus(body.velocity, v);
    body.correction = plus(body.correction, v);
}

/// The contact's velocity: the body's, with its rates' spin about the centre of mass at the contact;
/// and the body's alone, in the world and in the body's frame.
fn contact_velocity(t: &mut Tire, s: &mut TireState, body: &Body) {
    let q = body.quat;
    let local = crate::biplane::unturn(q, minus(s.contact, body.position));
    let r = body.rates;
    let spin = r.merge(Dest::XYZ, Vf::opmsub(Vf::opmula(r, local), local, r));
    s.point_velocity = minus(plus(body.velocity, crate::chassis::turn(q, spin)), CLEAR);
    s.body_velocity = minus(body.velocity, CLEAR);
    t.local_velocity = crate::biplane::unturn(q, s.body_velocity);
}

/// **The suspension**: the wheel's travel from its mount down to its drop, in the world; where the
/// ground's plane cuts it, the contact and the compression. Compressed, the tire is down: bottomed
/// past its travel, the body is pushed out along the normal and its velocity into the ground taken
/// off (on something solid, or near), and the compression held at the travel; then the spring less
/// the damper (clamped rate) is the load, pushed at the contact. Not down, the rears take what the
/// fronts handed them. Last, the compression eases back no faster than the decompression rate.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's `c.ole.s` branched on false
fn suspension(k: &TireKeys, t: &mut Tire, s: &mut TireState, cofg: Vf, body: &mut Body, h: F) {
    s.compression = F::ZERO;
    t.grounded = false;
    let mut ratio = F::ZERO;
    let q = body.quat;
    let n = t.ground_normal;
    if t.solid || t.sorted {
        t.upper = plus(crate::chassis::turn(q, minus(t.mount, cofg)), body.position);
        t.lever = minus(plus(t.mount, t.drop), cofg);
        t.lower = plus(crate::chassis::turn(q, t.lever), body.position);
        let travel = minus(t.upper, t.lower);
        let along = dot(travel, n);
        if !(along <= 0.0) {
            ratio = dot(minus(t.ground_point, t.lower), n) / along;
            if !(ratio <= 0.0) {
                s.contact = plus(scaled(travel, ratio), t.lower);
                let d = minus(s.contact, t.lower);
                s.compression = dot(d, d).sqrt();
            }
        }
        if !(s.compression <= 0.0) {
            t.grounded = true;
            contact_velocity(t, s, body);
            let max = k.max_compression;
            if !(s.compression <= max) {
                if t.solid || ratio < BOTTOM_RATIO {
                    let push = (s.compression - max).max(F::ZERO).min(max);
                    body.position = plus(body.position, scaled(n, push));
                    let into = dot(s.body_velocity, n).min(F::ZERO);
                    add_velocity(body, scaled(n, -into));
                }
                s.compression = max;
                contact_velocity(t, s, body);
            }
            let rate = k.max_compression_rate;
            let v = dot(s.point_velocity, n).max(-rate).min(rate);
            let damper = if v <= 0.0 { k.rebound_damping } else { k.bounce_damping };
            t.load = F::ZERO.max(Acc::mula(k.spring, s.compression).msub(damper, v));
            add_force_at(body, scaled(n, t.load), s.contact, ONES);
        }
    }
    if !t.grounded {
        if t.handed {
            t.load = t.handed_load;
            t.stands_on = t.handed_on;
            t.lever = plus(t.mount, t.drop);
            t.lower = crate::chassis::turn(q, t.lever);
            contact_velocity(t, s, body);
        } else {
            t.load = F::ZERO;
            s.point_velocity = CLEAR;
        }
    }
    let last = s.last_compression;
    if s.compression < last {
        s.compression = last - (last - s.compression).min(k.max_decompression_rate * h);
    }
    s.last_compression = s.compression.min(k.max_compression);
}

/// **The roll**: the surface's wheel friction against half the spin (clamped to ±1), and braked, the
/// surface's braking torque against it too, eased off by the ABS between the braking slips.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's `c.ole.s` branched on false
fn roll(k: &TireKeys, t: &Tire, s: &mut TireState, brake: Option<F>) {
    let sf = &k.surfaces[usize::from(t.surface) & 15];
    let half = (F(0.5) * s.spin).max(F(-1.0)).min(F::ONE);
    s.torque = Acc::adda(F::ZERO, s.torque).madd(-half, sf.wheel_friction);
    if let Some(b) = brake
        && !(b <= 0.0)
    {
        let mut scale = F::ONE;
        let (lo, hi) = (k.min_braking_slip, k.max_braking_slip);
        if k.abs && t.grounded && lo != hi {
            let along = (s.slip_ratio.abs() - lo) / (hi - lo);
            scale = Acc::adda(F::ZERO, F::ONE).madd(along, F(-1.0)).max(F::ZERO).min(F::ONE);
        }
        s.torque = Acc::adda(F::ZERO, s.torque).madd(sf.max_braking_torque, half * (scale * -b.max(F::ZERO)));
    }
}

/// **The slip**: braked hard past [`LOCK_SLIP`] the wheel locks, else it spins up by the torque less
/// the ground's pull (capped at its top speed). Fast along the tire, the slip angle and ratio come
/// straight from the velocity; slow, the low-speed grip holds a crawling tire still, and both relax
/// towards the velocity's instead. Then the camber off the car's lean, and the clamps.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's `c.ole.s` branched on false
fn slip(k: &TireKeys, t: &Tire, s: &mut TireState, brake: Option<F>, h: F) {
    let (curve, acos) = tables();
    let (lat, fwd) = (F(s.velocity.x()), F(s.velocity.z()));
    let speed = fwd.abs();
    if brake.is_some_and(|b| !(b <= 0.0)) && !(s.slip_ratio <= bits(LOCK_SLIP)) {
        s.slipping = false;
        s.locked = true;
        s.spin = F::ZERO;
    } else {
        s.slipping = !(s.slip_ratio.abs() <= bits(SLIP));
        s.locked = false;
        let top = k.top_wheel_speed;
        let pull = k.inv_inertia * (s.torque - F(s.last_force.z()) * t.radius);
        s.spin = Acc::adda(F::ZERO, s.spin).madd(pull, h).max(-top).min(top);
    }
    if !(speed <= FAST_SLIP_SPEED) {
        let cos = speed * F::ONE.rsqrt(Acc::mula(speed, speed).madd(lat, lat));
        let angle = acos.at(clamp_unit(clamp_unit(cos)));
        s.slip_angle = if lat < 0.0 { -angle } else { angle };
        s.slip_ratio = Acc::adda(F::ZERO, fwd).msub(s.spin, t.radius) / speed;
        let (c, sn) = curve.coarse(wrap(wrap(s.slip_angle)));
        s.slip_tan = sn / c;
    } else {
        if speed < GRIP_SPEED && F(s.last_velocity.x()) * lat < 0.0 {
            s.low_speed_grip = true;
        } else if !(speed <= GRIP_SPEED) {
            s.low_speed_grip = false;
        }
        s.last_velocity = s.velocity;
        if s.low_speed_grip {
            let grip = (F(GRIP_LOAD) * t.load).sqrt();
            s.force.0[0] = Acc::adda(F::ZERO, s.force.x()).msub(bits(GRIP_SCALE) * lat, grip).0;
        }
        s.slip_tan = Acc::adda(F::ZERO, s.slip_tan).madd(h, Acc::adda(F::ZERO, lat).msub(speed, s.slip_tan));
        let x = s.slip_tan.abs();
        let atan = if x <= 1.0 {
            x / Acc::adda(F::ZERO, F::ONE).madd(bits(ATAN_FIT) * x, x)
        } else {
            bits(HALF_PI) - x / Acc::adda(F::ZERO, bits(ATAN_FIT)).madd(x, x)
        };
        s.slip_angle = if s.slip_tan < 0.0 { -atan } else { atan };
        let creep = Acc::adda(F::ZERO, fwd).msuba(speed, s.slip_ratio).msub(t.radius, s.spin);
        s.slip_ratio = Acc::adda(F::ZERO, s.slip_ratio).madd(h, F(CREEP_RATE) * creep);
    }
    s.sliding = !(s.slip_angle.abs() <= bits(SLIP));
    let (x, y) = (F(t.car_normal.x()), F(t.car_normal.y()));
    let c = y * F::ONE.rsqrt(Acc::mula(y, y).madd(x, x));
    let lean = acos.at(clamp_unit(clamp_unit(c)));
    let lean = if x < 0.0 { -lean } else { lean };
    s.camber = -k.camber_roll_ratio * lean;
    s.slip_angle = s.slip_angle.max(-bits(HALF_PI)).min(bits(HALF_PI));
    s.slip_ratio = s.slip_ratio.max(-bits(MAX_SLIP_RATIO)).min(bits(MAX_SLIP_RATIO));
    s.camber = s.camber.max(-bits(MAX_CAMBER)).min(bits(MAX_CAMBER));
}

/// **The surface's force**: the tire model's combined slip on the surface the tire stands on, its
/// load capped — the friction curve's shape off the slips, scaled by the load and the peak friction
/// (falling off toward the end point), sideways less the camber's thrust — added into the force.
fn surface_force(k: &TireKeys, t: &Tire, s: &mut TireState) {
    let (curve, _) = tables();
    let sf = &k.surfaces[usize::from(t.surface) & 15];
    let mu = k.friction_scale * sf.friction_peak;
    let (cos, sin) = curve.coarse(wrap(wrap(s.slip_angle)));
    let sr = s.slip_ratio;
    let sr2 = sr * sr;
    let sin2 = sin * sin;
    let tan = sin / cos;
    let cos2 = cos * cos;
    let tan2 = tan * tan;
    let load = t.load.min(sf.max_normal_load);
    let (lat, fwd) = if load == 0.0 {
        (F::ZERO, F::ZERO)
    } else {
        let load2 = load * load;
        let a1a2 = sf.a1 * sf.inv_a2;
        let d = F(2.0) * Acc::adda(F::ZERO, Acc::adda(F::ZERO, sf.a0).madd(sf.a1, load)).msub(load2, a1a2);
        let comb = Acc::adda(F::ZERO, sin2).madd(sr2, cos2).sqrt();
        let cs = (F(2.0) * load) * sf.cornering_stiffness;
        let e8 = (F(8.0) * mu) * load;
        let a3a4 = sf.a3 * sf.inv_a4;
        let a3l = sf.a3 * load;
        let cx = Acc::adda(F::ZERO, cs).madd(comb, d - cs);
        let mue = mu * Acc::adda(F::ZERO, F::ONE).msub(sf.friction_end_point, comb);
        let q = (cs * sr) / (F::ONE - sr);
        let d2t2 = (d * d) * tan2;
        let sigma = (bits(PI) * Acc::adda(F::ZERO, d2t2).madd(q, q).sqrt()) / e8;
        let s2 = sigma * sigma;
        let s3 = s2 * sigma;
        let c1s = sf.c1 * s3;
        let num = Acc::adda(F::ZERO, Acc::adda(F::ZERO, c1s).madd(sf.c2, s2)).madd(bits(FOUR_OVER_PI), sigma);
        let tt = Acc::adda(F::ZERO, Acc::adda(F::ZERO, c1s).madd(sf.c3, s2)).madd(sf.c4, sigma);
        let ck = Acc::adda(F::ZERO, a3l).msub(load2, a3a4);
        let shape = num / (F::ONE + tt);
        let norm = Acc::adda(F::ZERO, d2t2).madd(cx * cx, sr2).sqrt();
        let (x, y) = if norm.abs() <= 0.0 {
            (F::ZERO, F::ZERO)
        } else {
            let inv = F::ONE / norm;
            (inv * (sr * (shape * cx)), inv * (tan * (-shape * d)))
        };
        (Acc::mula(load, y * mue).msub(ck, s.camber), load * (-x * mue))
    };
    s.force.0[0] = (F(s.force.x()) + lat).0;
    s.force.0[2] = (F(s.force.z()) + fwd).0;
}

/// `v` taken into the tire's frame: each row dotted with it, by the transposed rows' VU0
/// multiply-adds; `w` is `v`'s.
fn into_frame(m: &[Vf; 4], v: Vf) -> Vf {
    let col = |i: usize| Vf::new(m[0].0[i], m[1].0[i], m[2].0[i], 0.0);
    let acc = col(0).mul(v.bc(ps2_float::Field::X));
    let acc = Vf::madd(acc, col(1), v.bc(ps2_float::Field::Y));
    v.merge(Dest::XYZ, Vf::madd(acc, col(2), v.bc(ps2_float::Field::Z)))
}

/// `v` out of the tire's frame into the world: the rows by its lanes; `w` is `v`'s.
fn out_of_frame(m: &[Vf; 4], v: Vf) -> Vf {
    let acc = m[0].mul(v.bc(ps2_float::Field::X));
    let acc = Vf::madd(acc, m[1], v.bc(ps2_float::Field::Y));
    v.merge(Dest::XYZ, Vf::madd(acc, m[2], v.bc(ps2_float::Field::Z)))
}

/// **The forces**: the steer word to the steer angle; in the air only the slip. Down, the tire's frame
/// off its steered heading and the ground's normal, the contact's velocity into it, the slip and the
/// surface's force, and that force turned out into the world on the body (with its moment, unless only
/// the fronts are down). Then the wheel turns by its spin (clamped by the angle rate) and wraps, the
/// torque is spent, and the side ratio is the sideways force on the load.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's `c.ole.s` branched on false
fn forces(k: &TireKeys, t: &Tire, s: &mut TireState, body: &mut Body, steer: Option<F>, brake: Option<F>, h: F) {
    if let Some(w) = steer {
        s.steer = w * k.max_steer_angle;
    }
    s.force = CLEAR;
    if !t.grounded {
        s.velocity = CLEAR;
        slip(k, t, s, brake, h);
    } else {
        let q = body.quat;
        let forward = if !(s.steer.abs() <= 0.0) {
            // The frame's lookups wrap with plain `mul.s`/`sub.s`, as the table's own lookup does.
            let (cos, sin) = tables().0.coarse(s.steer);
            crate::chassis::turn(q, Vf::new(sin.0, 0.0, cos.0, 0.0))
        } else {
            crate::chassis::turn(q, AHEAD)
        };
        s.frame = crate::math::basis(CLEAR, forward, t.ground_normal, true, false).0;
        s.velocity = into_frame(&s.frame, s.point_velocity);
        slip(k, t, s, brake, h);
        surface_force(k, t, s);
        let world = out_of_frame(&s.frame, s.force);
        body.force = plus(body.force, world);
        if !t.lone_front {
            let r = minus(s.contact, body.position);
            let arm = r.merge(Dest::XYZ, Vf::opmsub(Vf::opmula(r, world), world, r));
            body.moment = plus(body.moment, crate::biplane::unturn(q, arm));
        }
    }
    s.last_force = s.force;
    let rate = k.max_tire_angle_rate;
    let a = Acc::adda(F::ZERO, s.angle).madd(h, s.spin.max(-rate).min(rate));
    s.angle = wrap(a);
    s.last_torque = s.torque;
    s.torque = F::ZERO;
    s.side_ratio = if t.load <= 0.0 { F::ZERO } else { F(s.force.x()) / t.load };
}

/// **One tire's step** (MX Unleashed's): its suspension, roll and forces, reading the brake and steer
/// words its pointers name, with `cofg` the body's centre of mass ([`Car::cofg`]).
pub fn tire_step(k: &TireKeys, t: &mut Tire, s: &mut TireState, levers: &State, cofg: Vf, body: &mut Body, h: F) {
    let read = |l: Option<Lever>| {
        l.map(|l| match l {
            Lever::Brake => levers.brake,
            Lever::BrakeForce => levers.brake_force,
            Lever::Steer => levers.steer,
            Lever::RearSteer => levers.rear_steer,
        })
    };
    let (brake, steer) = (read(k.brake), read(k.steer));
    suspension(k, t, s, cofg, body, h);
    roll(k, t, s, brake);
    forces(k, t, s, body, steer, brake, h);
}

// --- the engine and its gearbox (MD125) ---------------------------------------------------------------

/// The engine's loader words (research/oracle.txt `# MD125`; MD39b's `[Engine]`), as SLUS-21104's
/// engine keeps them: MX Unleashed's [`crate::car::Engine`] on the car's tires.
#[derive(Clone, Debug, PartialEq)]
pub struct EngineKeys {
    /// The tires the engine drives (its first two slots), and on a four-wheel drive the other two.
    pub drive: [usize; 2],
    pub drive_more: Option<[usize; 2]>,
    /// The torque table: its RPM axis and the torque at each point; its last RPM, the redline.
    pub rpm: Vec<F>,
    pub torque: Vec<F>,
    pub redline: F,
    /// `GearRatio1..` and `FinalGearRatio`.
    pub gear_ratios: Vec<F>,
    pub final_gear_ratio: F,
    pub idle_throttle: F,
    /// `EngineBraking * 1e-6`.
    pub engine_braking: F,
    /// The fraction of the drive the box loses.
    pub transmission_loss: F,
    pub inv_inertia: F,
    pub min_rpm: F,
    /// The torque converter: `PumpTorqueScale * 0.001`, `MaxPumpTorque`, `maxK`.
    pub pump_torque_scale: F,
    pub max_pump_torque: F,
    pub max_k: F,
    /// Traction control.
    pub tcs: bool,
    pub min_traction_slip: F,
    pub max_traction_slip: F,
    /// Nitrous: how fast it burns and the horsepower it adds.
    pub nox_burn_rate: F,
    pub nox_hp_boost: F,
    /// The shift points and the shift's throttle (`high_load_rpm` read as zero when `None`).
    pub shift: crate::car::Shift,
    /// The engine has its sound, which a shift plays.
    pub sound: bool,
}

/// The engine's running words: MX Unleashed's [`crate::car::Drivetrain`]'s, but its reverse word
/// ([`State::reversing`]) and the launch's torque scale ([`State::launch_boost`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EngineState {
    /// Nitrous on (the car's word), and what is left in the bottle.
    pub nox_on: bool,
    pub nox_left: F,
    /// Time until the box may shift again; the last shift went up; the shift's own throttle's time.
    pub shift_timer: F,
    pub shifting_up: bool,
    pub shift_throttle_timer: F,
    /// The gear, from 1, and the overall ratio and its reciprocal.
    pub gear: i32,
    pub ratio: F,
    pub inv_ratio: F,
    /// The driven wheels' speed in engine revs; the converter's pump torque, multiplication and
    /// output.
    pub wheel_rpm: F,
    pub converter_torque: F,
    pub multiplication: F,
    pub output_torque: F,
    /// The torque curve's cursor: its point and the weight back to the one below.
    pub cursor_index: u32,
    pub cursor_frac: F,
    /// The net torque and the engine braking taken off it; each drive slot's torque.
    pub engine_torque: F,
    pub braking_torque: F,
    pub drive_torque: [F; 4],
    /// The revs, the revs the converter slips by, and the sound's clock and pitch.
    pub rpm: F,
    pub slip_rpm: F,
    pub sound_clock: F,
    pub pitch: F,
}

/// Engine revs to radians a second, and back; horsepower over revs to torque.
const RPM_TO_RAD: u32 = 0x3dd6_7756; // 0.10471976
const RAD_TO_RPM: u32 = 0x4118_c9eb; // 9.549296
const HP_TORQUE: f32 = 5252.0;
/// The sound's pitch: its floor (also the squat's), the top past which it wobbles on the curve
/// table's sine, the bottom under which on the smooth noise.
const PITCH_FLOOR: u32 = 0x3dcc_cccd; // 0.1
const PITCH_HIGH: u32 = 0x3f73_3333; // 0.95
const PITCH_LOW: u32 = 0x3e4c_cccd; // 0.2
const PITCH_NOISE: u32 = 0x3de1_47ae; // 0.11
/// The curve table's index scale (camera's), for the sound's top wobble's bare lookup.
const CURVE_INV_STEP: f32 = 954.77045;

/// The inline remap's clamped lerp, `adda.s 0`/`madd.s`, for a range known not to be flat.
fn lerp_held(t: F, lo: F, hi: F) -> F {
    Acc::adda(F::ZERO, lo).madd(t, hi - lo).max(lo.min(hi)).min(lo.max(hi))
}

/// **Put the box in `gear`** (from 1; SLUS-21104's set-gear, MX Unleashed's
/// [`crate::car::Drivetrain::set_gear`]): nothing in reverse or off the ends. The ratio sets the
/// driven tires' top wheel speed, the redline's (the second pair's only on a four-wheel drive).
fn set_gear(e: &EngineKeys, tires: &mut [TireKeys; 4], s: &mut State, gear: i32) {
    if gear <= 0 || e.gear_ratios.len() < gear as usize || s.reversing {
        return;
    }
    let g = &mut s.engine;
    g.gear = gear;
    g.ratio = e.final_gear_ratio * e.gear_ratios[gear as usize - 1];
    g.inv_ratio = F::ONE / g.ratio;
    let top = e.redline * bits(RPM_TO_RAD) * g.inv_ratio;
    for k in e.drive.into_iter().chain(e.drive_more.into_iter().flatten()) {
        tires[k].top_wheel_speed = top;
    }
}

/// **The engine's reverse switch** (MX Unleashed's [`crate::car::Drivetrain::set_reverse`]): into
/// reverse, first's ratio turned round with the tires' top speeds left alone; out of it, into first.
fn reverse(e: &EngineKeys, tires: &mut [TireKeys; 4], s: &mut State, r: crate::car::Reverse) {
    if r == crate::car::Reverse::In {
        s.reversing = true;
        let g = &mut s.engine;
        g.gear = 1;
        g.ratio = e.final_gear_ratio * -e.gear_ratios[0];
        g.inv_ratio = F::ONE / g.ratio;
    } else {
        s.reversing = false;
        set_gear(e, tires, s, 1);
    }
}

/// A shift point between its low-load and high-load RPMs by where the converter's slip sits between
/// the two loads; a flat load range is the low-load point.
fn shift_point(e: &EngineKeys, s: &EngineState, low: F, high: F) -> F {
    let (lo, hi) = (e.shift.low_load_rpm, e.shift.high_load_rpm.unwrap_or(F::ZERO));
    if lo == hi {
        return low;
    }
    lerp_held((s.slip_rpm - lo) / (hi - lo), low, high)
}

/// **The automatic box** (SLUS-21104's, once a frame from the PostProcess; MX Unleashed's
/// [`crate::car::Drivetrain::shift`]): nothing while neither of the first two drive slots' tires is
/// on the ground, or in reverse. A running timer counts down and that is all; else, with the clutch
/// up, revs over the up-shift point go up a gear (none past the top, the timer restarted all the
/// same) and revs under the down-shift point down one (none from first), each starting the shift's
/// throttle and the timer. `true` on a shift, where the original plays the shift sound (before the
/// gear changes). The box's own way out of reverse on an up-shift, and into it on a shift down from
/// first, are never reached: in reverse it returns first, and it never shifts down from first.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's `c.ole.s` branched on false
fn shift(e: &EngineKeys, tires: &mut [TireKeys; 4], s: &mut State, car: &Car, dt: F) -> bool {
    let [a, b] = e.drive;
    if (!car.tires[a].grounded && !car.tires[b].grounded) || s.reversing {
        return false;
    }
    let g = &mut s.engine;
    if !(g.shift_timer <= 0.0) {
        g.shift_timer = g.shift_timer - dt;
        return false;
    }
    if !(s.engaged_clutch <= 0.0) {
        return false;
    }
    let p = e.shift;
    let up = shift_point(e, g, p.low_load_up_shift_rpm, p.high_load_up_shift_rpm);
    let down = shift_point(e, g, p.low_load_down_shift_rpm, p.high_load_down_shift_rpm);
    let mut shifted = false;
    if !(g.rpm <= up) {
        let next = g.gear + 1;
        if next as usize <= e.gear_ratios.len() {
            g.shifting_up = true;
            g.shift_throttle_timer = p.shift_up_duration;
            shifted = true;
            set_gear(e, tires, s, next);
        }
    } else if g.rpm < down {
        if g.gear < 2 {
            return false;
        }
        g.shifting_up = false;
        g.shift_throttle_timer = p.shift_down_duration;
        shifted = true;
        let next = g.gear - 1;
        set_gear(e, tires, s, next);
    } else {
        return false;
    }
    s.engine.shift_timer = p.shift_time;
    shifted
}

/// The torque curve's cursor walked to `x` (the shared curve seek): up past every point at or under
/// it, back below every point over it; the weight back to the point below, or zero at either end.
fn seek(xs: &[F], g: &mut EngineState, x: F) {
    let n = xs.len() as u32;
    let mut i = g.cursor_index;
    while i < n && xs[i as usize] <= x {
        i += 1;
    }
    while i != 0 && x < xs[i as usize - 1] {
        i -= 1;
    }
    if i < n {
        g.cursor_frac = if i == 0 {
            F::ZERO
        } else {
            let hi = xs[i as usize];
            (hi - x) / (hi - xs[i as usize - 1])
        };
    } else {
        g.cursor_frac = F::ZERO;
        i = n.wrapping_sub(1);
    }
    g.cursor_index = i;
}

/// Traction control's cut for a tire's slip ratio: 1 under the minimum slip falling to 0 at the
/// maximum. A flat slip range is `flat`: the inline remap's own branch answers 1 for the first tire
/// of a pair, and for the second the `sub.s` its delay slot carries, `1 - min`.
fn traction(e: &EngineKeys, slip_ratio: F, flat: F) -> F {
    let (lo, hi) = (e.min_traction_slip, e.max_traction_slip);
    if lo == hi {
        return flat;
    }
    let t = (slip_ratio.abs() - lo) / (hi - lo);
    Acc::adda(F::ZERO, F::ONE).madd(t, F::ZERO - F::ONE).max(F::ONE.min(F::ZERO)).min(F::ONE.max(F::ZERO))
}

/// **The engine, once a substep** (SLUS-21104's, from the Force body while riding; MX Unleashed's
/// [`crate::car::Drivetrain::step`], whose doc walks it), on the car's throttle ([`State::gas`]),
/// clutch ([`State::engaged_clutch`]) and brake ([`State::brake_force`]): the drive torque added to
/// the driven tires' ([`TireState::torque`]). SLUS-21104 fuses (`adda.s 0`/`madd.s`, `msub.s`) what
/// MX Unleashed multiplies and adds: the torque curve's value, the nitrous left, the converter's
/// multiplication, the revs' damping and step, and the sound's wobbles and squat; and its traction
/// control on a flat slip range cuts the second tire of a pair to `1 - min` ([`traction`]). The
/// smooth noise the sound's bottom wobble asks is handed out ([`Call::Noise`]).
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's `c.ole.s` branched on false
fn engine_step(e: &EngineKeys, tires: &[TireKeys; 4], s: &mut State, car: &mut Car, body: &mut Body, h: F, rest: &mut impl Steps) {
    let (gas, engaged_clutch, brake_force) = (s.gas, s.engaged_clutch, s.brake_force);
    let g = &mut s.engine;
    let (clutch, throttle) = if !(g.shift_throttle_timer <= 0.0) {
        let p = e.shift;
        let throttle = if g.shifting_up { p.shift_up_throttle } else { p.shift_down_throttle };
        g.shift_throttle_timer = g.shift_throttle_timer - h;
        (F::ONE, throttle)
    } else {
        (engaged_clutch, gas)
    };

    seek(&e.rpm, g, g.rpm);
    let i = g.cursor_index as usize;
    let mut torque = if i == 0 {
        e.torque[0]
    } else {
        let hi = e.torque[i];
        Acc::adda(F::ZERO, hi).msub(g.cursor_frac, hi - e.torque[i - 1])
    };
    if g.nox_on && !(g.rpm <= 0.0) && !(g.nox_left <= 0.0) {
        let boost = F(HP_TORQUE) * e.nox_hp_boost / g.rpm;
        torque = torque + boost;
        g.nox_left = Acc::adda(F::ZERO, g.nox_left).msub(e.nox_burn_rate, h).max(F::ZERO);
    }
    g.engine_torque = torque * (throttle + e.idle_throttle).min(F::ONE);
    g.braking_torque = g.rpm * (e.engine_braking * g.rpm);
    g.engine_torque = g.engine_torque - g.braking_torque;

    let ts = &s.tires;
    let [a, b] = e.drive;
    let (share, spin) = if clutch <= 0.0 {
        let mut spin = ts[a].spin + ts[b].spin;
        let share = if let Some([c, d]) = e.drive_more {
            spin = spin + (ts[c].spin + ts[d].spin);
            F(0.25)
        } else {
            F(0.5)
        };
        g.wheel_rpm = bits(RAD_TO_RPM) * (g.ratio * (share * spin));
        g.slip_rpm = if g.wheel_rpm < 0.0 { g.rpm } else { g.rpm - g.wheel_rpm };
        (share, spin)
    } else {
        g.wheel_rpm = g.rpm;
        g.slip_rpm = F::ZERO;
        (F::ZERO, F::ZERO)
    };

    let slip = g.slip_rpm;
    let sign = F::from(if slip < 0.0 { -1 } else { 1 });
    g.converter_torque = slip * (slip * (e.pump_torque_scale * sign));
    g.converter_torque = g.converter_torque.max(-e.max_pump_torque).min(e.max_pump_torque);
    let (wheel_rpm, rpm) = (g.wheel_rpm, g.rpm);
    g.multiplication = if rpm <= wheel_rpm || rpm <= 0.0 || wheel_rpm <= 0.0 {
        e.max_k
    } else {
        Acc::adda(F::ZERO, e.max_k).madd(F::ONE - e.max_k, wheel_rpm / rpm)
    };
    let boost = if g.ratio <= 0.0 || spin <= 0.0 { F::ONE } else { s.launch_boost };
    g.output_torque = g.converter_torque * (boost * g.multiplication);

    // The pump torque's slope in the revs, for the implicit step.
    let slope = F(0.5) * (slip * (e.pump_torque_scale * (F(2.0) * sign)));
    let net = e.inv_inertia * (g.engine_torque - g.converter_torque);
    let damp = Acc::adda(F::ZERO, F::ONE).madd(h, slope * e.inv_inertia);
    let accel = if damp <= 0.0 { F::ONE * net } else { (F::ONE / damp) * net };
    g.rpm = Acc::adda(F::ZERO, g.rpm).madd(bits(RAD_TO_RPM), h * accel);
    g.rpm = g.rpm.max(e.min_rpm).min(e.redline);

    let drive = if brake_force <= 0.0 { g.ratio * (share * (F::ONE - e.transmission_loss)) } else { F::ZERO };
    for (n, [a, b]) in [Some(e.drive), e.drive_more].into_iter().flatten().enumerate() {
        let (cut_a, cut_b) = if e.tcs && car.tires[a].grounded && car.tires[b].grounded {
            (traction(e, s.tires[a].slip_ratio, F::ONE), traction(e, s.tires[b].slip_ratio, F::ONE - e.min_traction_slip))
        } else {
            (F::ONE, F::ONE)
        };
        let g = &mut s.engine;
        g.drive_torque[2 * n] = g.output_torque * (cut_a * drive);
        g.drive_torque[2 * n + 1] = g.output_torque * (cut_b * drive);
        s.tires[a].torque = s.tires[a].torque + g.drive_torque[2 * n];
        s.tires[b].torque = s.tires[b].torque + g.drive_torque[2 * n + 1];
    }

    // The sound's pitch.
    let g = &mut s.engine;
    let (lo, hi) = (e.min_rpm, e.redline);
    let floor = bits(PITCH_FLOOR);
    g.pitch = if lo == hi { floor } else { lerp_held((g.rpm - lo) / (hi - lo), floor, F::ONE) };
    if !(g.pitch <= bits(PITCH_HIGH)) {
        let a = wrap(wrap(F(35.0) * g.sound_clock));
        let k = ((bits(PI) + a) * CURVE_INV_STEP).to_i32();
        let sin = F(tables().0.entries()[k as usize][1]);
        g.pitch = Acc::adda(F::ZERO, g.pitch).madd(F(0.0125), sin);
    } else if g.pitch < bits(PITCH_LOW) {
        let at = F(0.25) * g.sound_clock;
        rest.call(Call::Noise(at), car, body);
        let g = &mut s.engine;
        g.pitch = Acc::adda(F::ZERO, g.pitch).madd(bits(PITCH_NOISE), car.noise);
    }
    let g = &mut s.engine;
    let squat = F(0.5) * (s.tires[a].compression + s.tires[b].compression) / tires[a].max_compression;
    if !(squat < floor) {
        g.pitch = Acc::adda(F::ZERO, g.pitch).msub(floor, squat - floor);
    }
    g.sound_clock = g.sound_clock + h;
    g.pitch = lerp_held((g.pitch - F::ZERO) / (F::ONE - F::ZERO), lo, hi);
}
