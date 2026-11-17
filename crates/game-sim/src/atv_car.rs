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
//! code ported, its calls handed out. [`class_update`] is the buggy's, golf cart's and truck's slot
//! around it (MD106e), and [`OwnUpdate`] answers the update's own calls: the ghost step, the hazard
//! clock and the base update (the bike's [`crate::handling::tail`]); the class's per-frame step stays
//! a call, which [`class_step`] answers on scene nodes (MD151), [`golf_class_step`] for the golf
//! cart (MD156a) and [`truck_class_step`] for the monster truck (MD156b).
//!
//! Proof: `tests/mxvatv_car_frame/`, SLUS-21104's world step and the car's update off MD39a's buggy
//! image with every call hooked; and (MD106c) the image's frame whole, with [`Pieces`] and [`Own`]
//! answering the calls and only the ground query left to the test.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::air::AcosTable;
use crate::atv_body::Body;
use crate::camera::CurveTable;
use crate::handling::node;
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
    /// The ground's normal under the body, as [`Call::GroundUnder`] answers it (and under the
    /// put-back's spot, as [`Call::BackGround`] does); and whether the up
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
    /// What a tire's stands-on words point at, as [`Call::StandsOn`] answers it (MD131).
    pub stood_on: Under,
}

/// What a tire's two stands-on words ([`Tire::stands_on`]) point at, as the tire step's contact
/// velocity reads it (MD131): a struck object (the first word), else a kept mover (the second).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Under {
    /// Both words zero, or nothing answered: the ground does not move.
    #[default]
    Still,
    /// A collision object: `cofg` its own centre of mass (`+0x80`), on what holds it.
    Object { cofg: Vf, on: Holder },
    /// A moving piece of scenery: its transform (`+0x244`, already up to date), the point it turns
    /// about in that frame (`+0x180`), and what its two virtuals answer (`+0xc4` the spin, asked
    /// first, then `+0xc0` the velocity).
    Mover { node: crate::contact::Node, origin: Vf, spin: Vf, velocity: Vf },
}

/// What a collision object under a tire belongs to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Holder {
    /// A rigid body (the object's `+0xe4`): its position, velocity, quaternion and rates (`+0x70`,
    /// `+0x80`, `+0x190`, `+0x1d0`).
    Body { position: Vf, velocity: Vf, quat: Vf, rates: Vf },
    /// With no body, a vehicle's chassis (`+0xe0`): its centre of mass in the transform's frame
    /// (`+0x180`), its velocity (`+0x250`) and its rates in the transform's frame (`+0x280`); placed by
    /// the object's transform (`+0xe8`, already up to date).
    Chassis { cofg: Vf, velocity: Vf, rates: Vf, node: crate::contact::Node },
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
    /// Untamed's hazard clock (MD81d, with [`Update::untamed`]): called straight, as the quad's
    /// ([`crate::quad::Call::UntamedHazardSurface`]), told also whether any tire is solidly down.
    UntamedHazardClock { dt: F, time: F, sunk: bool, grounded: bool },
    /// The class's per-frame step: [`class_step`] on the model's scene nodes (MD151).
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
    /// What the tire step's contact velocity hands out (MD131, [`Pieces`]): the thing a tire's
    /// stands-on words point at asked (into [`Car::stood_on`]); and a struck object's centre of mass
    /// turned into the world, which it writes back to the object (its `+0x90`).
    StandsOn([u32; 2]),
    ObjectCofg(u32, Vf),
    /// The car's node placed while current, with a subtree under it (its model's parts, past what
    /// [`Vehicle::scene`] holds): every current node of the subtree marked out of date. [`Own`]
    /// answers it when [`Vehicle::below`] holds the subtree (MD128).
    SubtreeOutOfDate,
    /// What the update's own calls hand out in turn, when [`OwnUpdate`] answers them (MD106e). The
    /// ghost step's: the collision probe's overlap search (answered into [`Updated::inside`]), the
    /// node's alpha, and the ghost over (vt `+0x39c`). The base update's ([`crate::handling::tail::base`]):
    /// its prepare (vt `+0x3e4`), the ride (vt `+0x35c`), the sync and snapshot (vt `+0x378`,
    /// `+0x37c`), the network's step and the vehicle's parts and listeners. The hazard clock's: the
    /// put-back (vt `+0x3a8` with 1, 1).
    GhostSearch,
    GhostAlpha(F),
    GhostOver,
    Prepare,
    Ride(F),
    Sync,
    Snapshot,
    Network(F),
    Parts(F),
    PutBack,
    /// What the car classes' own update slot ([`class_update`]) calls around [`update`]: the
    /// collision put on (vt `+0x138`), or taken off: the car taken off its list (with the list there)
    /// before [`Call::CollisionSync`]; without a player, the vehicle placed at its node's position
    /// and heading (vt `+0x3ac`, with 0 and 1); the pad's rumble at a level, when the vehicle state's
    /// rumble is on (the call's to ask), for [`RIDE_RUMBLE_TIME`] and not the soft one; and last
    /// [`Call::Cue`] 4, the frame's message.
    CollisionOn,
    Unlist,
    Place,
    RideRumble(i32),
    /// What [`Own`]'s answers to those hand out in turn (MD128), as the quad's
    /// ([`crate::quad::Call`]): the spatial database's move of the car's entry (`node` false; its
    /// node's box, reach 0) or the node's (its bounds' box and reach); the node's bounds gathered again
    /// (its vt `+0x94`, which takes in nodes past what [`Vehicle::scene`] holds); a collidable's box
    /// moved in the database, after its sync; the nearby list gathered round the active probe. The
    /// body's reset's: the body taken off the physics world's three lists (the bodies, the vehicle
    /// state's, the post elements), by index; with bit 7 of [`Vehicle::reset_flags`], the riding
    /// probe's owner cleared and the world's category word added to its list; else the record handed
    /// back to its owner.
    SpatialMove { node: bool, centre: Vf, half: Vf, reach: F },
    NodeRebound,
    PartMove { part: crate::quad::Part, centre: Vf, half: Vf, reach: F },
    Gather(crate::quad::Part),
    LeaveWorld(usize),
    RidingLetGo,
    ReleaseRecord,
    /// What the respawn hands out, when [`Own`] answers it (MD154). The put-back's: the ground query
    /// at the spot (answered into [`Car::ground_normal`]), the ghost started (vt `+0x3a0`), the trail
    /// reset. The ground reset's: the node taken out of the spatial database and hung back in its
    /// scene (with bit 1 of the flag byte); a collidable's database entry taken out; with the sync's
    /// switch, a collidable turned on and attached in the world. Then each tire's reset and the
    /// engine's, which [`Pieces`] answers ([`tire_reset`], [`engine_reset`]).
    BackGround(Vf),
    GhostStart,
    TrailReset,
    NodeUnhook,
    NodeRehang,
    PartUnhook(crate::quad::Part),
    PartOn(crate::quad::Part),
    PartAttach(crate::quad::Part),
    TireReset(usize),
    EngineReset,
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
    post_process_on(car, body, dt, table, s, false);
}

/// **Untamed's car PostProcess** (MD161, SLUS-21701's): [`post_process`] with the yaw rate held by
/// two compares ([`hold_yaw_rate_untamed`]).
pub fn post_process_untamed(car: &mut Car, body: &mut Body, dt: F, table: &AcosTable, s: &mut impl Steps) {
    post_process_on(car, body, dt, table, s, true);
}

#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's `c.ole.s` branched on false
fn post_process_on(car: &mut Car, body: &mut Body, dt: F, table: &AcosTable, s: &mut impl Steps, untamed: bool) {
    s.call(Call::VehiclePost(dt), car, body);
    if untamed {
        hold_yaw_rate_untamed(&mut body.rates, car.yaw_rate_limit);
    } else {
        hold_yaw_rate(&mut body.rates, car.yaw_rate_limit);
    }
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

/// Untamed's hold of the yaw rate (MD161): over `limit` (or not a number) it is `limit`, else under
/// `-limit` it is `-limit`, the limit's own bits either way. [`hold_yaw_rate`]'s product by the sign
/// gives the same for any normal positive limit; they part on a denormal or negative one.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's `c.ole.s` branched on false
pub fn hold_yaw_rate_untamed(rates: &mut Vf, limit: F) {
    // The compares read a denormal as 0 (as `F`'s do since F64); the stores keep the bits.
    let (y, l) = (F::from_bits(rates.y().to_bits()), F::from_bits(limit.to_bits()));
    if !(y <= l) {
        rates.0[1] = limit.0;
    } else if y < -l {
        rates.0[1] = (-limit).0;
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
    /// Untamed's update (MD81d): the hazard clock is [`Call::UntamedHazardClock`]. Off, SLUS-21104's.
    pub untamed: bool,
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
            let grounded = car.tires.iter().any(|t| t.solid);
            let sunk = u.on_hazard && grounded;
            let time = surface[REAR_LEFT].time.max(surface[REAR_RIGHT].time);
            let time = surface[FRONT_RIGHT].time.max(time);
            let time = surface[FRONT_LEFT].time.max(time);
            if u.untamed {
                s.call(Call::UntamedHazardClock { dt, time, sunk, grounded }, car, body);
            } else {
                s.call(Call::HazardClock { dt, time, sunk }, car, body);
            }
        }
        s.call(Call::ClassStep, car, body);
    }
    s.call(Call::VehicleUpdate(dt), car, body);
}

/// What the car's update's own calls and its class's update slot read and write beyond [`Car`],
/// [`Update`] and the body (research/oracle.txt `# MD106e`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Updated {
    /// The ghost's clock (counting down while it is on) and its length; whether the car has a
    /// collision probe, and the probe's answer to [`Call::GhostSearch`]: inside something.
    pub ghost: F,
    pub ghost_length: F,
    pub probe: bool,
    pub inside: bool,
    /// The vehicle base's words the base update and the hazard clock keep, as the bike's
    /// ([`crate::handling::tail`]; the vehicle base is the same class): the state (a pointer, which
    /// must be 0 exactly when [`Update::vehicle_state`] is none), the clocks, free and stuck, the
    /// body's position, the run's flags. The slot reads the node's two axes (`side` and `ahead`), the
    /// distance, the player and the rumbler there (on the ground, the speed and the wreck reason are
    /// [`Car::grounded`], [`Car::ground_velocity`]'s `z` and [`Car::state`]); `speed` is unused.
    pub vehicle: crate::handling::tail::Vehicle,
    /// The car is linked to an object whose node's flag word has bit 1: its collision stays on.
    pub carried: bool,
    /// The car's word the slot clears when it takes the collision off with the body in the world
    /// (the collision's put-on, [`Call::CollisionOn`], sets it); the car is on a list then (it is
    /// taken off it, [`Call::Unlist`]); and the byte whose bit 3 says the collision is off.
    pub collision_word: u32,
    pub listed: bool,
    pub collision_flags: u8,
}

/// The ghost's alpha floor, and its top.
const GHOST_FLOOR: f32 = 0.1;
/// The class slot's rumble: from this shock on, at 255 × shock over [`RIDE_RUMBLE_FULL`] and at least
/// [`RIDE_RUMBLE_LEAST`], for this long. The buggy's, the golf cart's and the truck's are the same.
const RIDE_RUMBLE_FROM: f32 = 5.0;
const RIDE_RUMBLE_FULL: f32 = 55.0;
pub const RIDE_RUMBLE_TIME: f32 = 0.3;
const RIDE_RUMBLE_LEAST: i32 = 125;
/// The vehicle state's mode the slot rides in: the collision on, the odometer's rumble measured.
const MODE_RIDING: u32 = 1;
/// The collision flags' bit: the collision off.
const COLLISION_OFF: u8 = 0x08;

/// **The update's own calls answered** (MD106e): the ghost step ([`ghost_step`]), the hazard clock
/// ([`crate::handling::tail::stuck`], the quad's and the bike's) and the base update
/// ([`crate::handling::tail::base`]); every other call, and what these hand out, goes on to `rest`.
pub struct OwnUpdate<'a, S> {
    pub w: &'a mut Updated,
    pub rest: &'a mut S,
}

impl<S: Steps> Steps for OwnUpdate<'_, S> {
    fn call(&mut self, c: Call, car: &mut Car, body: &mut Body) {
        // The vehicle base reads the body's position as its own (where it was last free).
        self.w.vehicle.body_position = body.position;
        match c {
            Call::Ghost(dt) => ghost_step(self.w, car, body, dt, self.rest),
            Call::HazardClock { dt, time, sunk } => {
                crate::handling::tail::stuck(&mut Base { w: &mut *self.w, rest: &mut *self.rest, car, body }, sunk, dt, time)
            }
            Call::UntamedHazardClock { dt, time, sunk, grounded } => {
                crate::handling::untamed::stuck(&mut Base { w: &mut *self.w, rest: &mut *self.rest, car, body }, sunk, grounded, dt, time)
            }
            Call::VehicleUpdate(dt) => crate::handling::tail::base(&mut Base { w: &mut *self.w, rest: &mut *self.rest, car, body }, dt),
            c => self.rest.call(c, car, body),
        }
    }
}

/// The vehicle base's outside world for the base update and the hazard clock: what they hand out
/// goes on as [`Call`]s.
struct Base<'a, S> {
    w: &'a mut Updated,
    rest: &'a mut S,
    car: &'a mut Car,
    body: &'a mut Body,
}

impl<S: Steps> crate::handling::tail::Vehicles for Base<'_, S> {
    fn vehicle(&mut self) -> &mut crate::handling::tail::Vehicle {
        &mut self.w.vehicle
    }
    fn skipped(&mut self) -> bool {
        unreachable!("the base update and the hazard clock do not ask for the skip")
    }
    fn paused(&mut self) -> bool {
        unreachable!("the base update and the hazard clock do not ask for the pause")
    }
    fn rumble_ready(&mut self) -> bool {
        unreachable!("the base update and the hazard clock do not rumble")
    }
    fn word(&mut self, _at: u32) -> u32 {
        unreachable!("the base update and the hazard clock read no word outside the vehicle")
    }
    fn out(&mut self, c: crate::handling::tail::Out, dt: F) {
        use crate::handling::tail::Out;
        let c = match c {
            Out::Prepare => Call::Prepare,
            Out::Ride => Call::Ride(dt),
            Out::Sync => Call::Sync,
            Out::Snapshot => Call::Snapshot,
            Out::Network => Call::Network(dt),
            Out::Parts => Call::Parts(dt),
            Out::PutBack => Call::PutBack,
            c => unreachable!("the base update and the hazard clock hand out no {c:?}"),
        };
        self.rest.call(c, self.car, self.body);
    }
}

/// **The vehicle's ghost step** (the car's vt `+0x3a4`): while the ghost's clock runs, the car's
/// collision probe searched for an overlap ([`Call::GhostSearch`]), and the clock counted down by
/// `dt` unless it is inside something (with no probe, no search and no count). Still running, the
/// node drawn at an alpha that rises from [`GHOST_FLOOR`] to 1 as the clock runs out (the floor with
/// a zero length); run out, the ghost is over ([`Call::GhostOver`]). The clock is not put back to
/// zero here. MX Unleashed's is [`crate::ghost::Ghost::step`], with a fixed length.
pub fn ghost_step(w: &mut Updated, car: &mut Car, body: &mut Body, dt: F, s: &mut impl Steps) {
    if w.ghost <= F::ZERO {
        return;
    }
    if w.probe {
        s.call(Call::GhostSearch, car, body);
        if !w.inside {
            w.ghost = w.ghost - dt;
        }
    }
    if w.ghost <= F::ZERO {
        s.call(Call::GhostOver, car, body);
        return;
    }
    let len = w.ghost_length;
    let alpha = if len == F::ZERO {
        F(GHOST_FLOOR)
    } else {
        let t = (w.ghost - len) / (F::ZERO - len);
        let (lo, hi) = (F(GHOST_FLOOR), F::ONE);
        Acc::adda(F::ZERO, lo).madd(t, hi - lo).max(lo.min(hi)).min(lo.max(hi))
    };
    s.call(Call::GhostAlpha(alpha), car, body);
}

/// `xyz` normalised with VU0's `vrsqrt`; `w` kept.
fn normalize(v: Vf) -> Vf {
    let sq = v.mul(v);
    let len2 = ps2_float::vu::madd(ps2_float::vu::add(sq.x(), sq.y()), 1.0, sq.z());
    v.merge(Dest::XYZ, v.mul_q(ps2_float::vu::rsqrt(1.0, len2)))
}

/// **The car classes' own update slot** (the buggy's, golf cart's and truck's vt `+0x10c`, one code
/// with the same constants), around the class's [`update`]. With no vehicle state, the update alone.
/// Otherwise:
///
/// 1. the odometer: the distance grows by `dt` times the speed, never below 0: on the ground the
///    ground velocity's `z` (the vehicle base's signed speed word), off it the body's velocity along
///    the node's heading flattened (its ahead axis, or its side axis when the ahead axis points
///    straight up or down, `y` dropped, normalised);
/// 2. the collision: on ([`Call::CollisionOn`], the off bit cleared) while carried or riding (the
///    vehicle state's mode 1); else off: with the body in the world its word cleared and, on a list,
///    [`Call::Unlist`]; then [`Call::CollisionSync`] and the off bit set;
/// 3. with no player, [`Call::Place`];
/// 4. riding and not wrecked ([`RIDING`]), the pad's rumble off the body's shock peak past [`RIDE_RUMBLE_FROM`]
///    ([`Call::RideRumble`], with a rumbler);
/// 5. the [`update`], its own calls answered by [`OwnUpdate`]; and [`Call::Cue`] 4.
pub fn class_update(u: &mut Update, w: &mut Updated, car: &mut Car, body: &mut Body, dt: F, s: &mut impl Steps) {
    if u.vehicle_state.is_none() {
        update(u, car, body, dt, &mut OwnUpdate { w, rest: s });
        return;
    }
    let v = &mut w.vehicle;
    let mut speed = if car.grounded {
        F(car.ground_velocity.z())
    } else {
        let y = F(v.ahead.y());
        let mut d = if y <= F(-1.0) || !(y < F::ONE) { v.side } else { v.ahead };
        d.0[1] = 0.0;
        let d = normalize(d);
        Acc::adda(F::ZERO, F(body.velocity.z()) * F(d.z())).madd(body.velocity.x(), d.x())
    };
    if speed < F::ZERO {
        speed = F::ZERO;
    }
    v.distance = Acc::adda(F::ZERO, v.distance).madd(speed, dt);

    let riding = u.vehicle_state == Some(MODE_RIDING);
    if w.carried || riding {
        s.call(Call::CollisionOn, car, body);
        w.collision_flags &= !COLLISION_OFF;
    } else {
        if body.enabled {
            w.collision_word = 0;
            if w.listed {
                s.call(Call::Unlist, car, body);
            }
        }
        s.call(Call::CollisionSync, car, body);
        w.collision_flags |= COLLISION_OFF;
    }
    if w.vehicle.player == 0 {
        s.call(Call::Place, car, body);
    }
    if riding && car.state == RIDING {
        let p = body.correction_peak;
        let shock = Acc::mula(p.x(), p.x()).madda(p.y(), p.y()).madd(p.z(), p.z()).sqrt();
        let level = if shock <= F(RIDE_RUMBLE_FROM) {
            0
        } else {
            let l = (F(255.0) * (shock / F(RIDE_RUMBLE_FULL))).to_i32();
            l.max(RIDE_RUMBLE_LEAST)
        };
        if level > 0 && w.vehicle.rumbler {
            s.call(Call::RideRumble(level), car, body);
        }
    }
    update(u, car, body, dt, &mut OwnUpdate { w, rest: s });
    s.call(Call::Cue(4), car, body);
}

/// One front corner's posed parts (MD151), scene nodes in the model's graph: the upright (steered,
/// then tilted to its two ends), the arm (swung by the tire's compression), the knuckle (tilted to
/// the two ends round the corner's pivot), the two ends, and the top the upright's tilt is measured
/// from.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrontParts {
    pub upright: u32,
    pub arm: u32,
    pub knuckle: u32,
    pub ends: [u32; 2],
    pub top: u32,
}

/// **The buggy's posed parts** (MD151): the car's own node (the frame the parts are measured and
/// aimed in), the four wheels (in the tires' order), the two front corners (in the tires' order),
/// the rear axle, the four links (aimed in pairs, all or none), the two spinners (turned by the
/// engine, both or neither), and the eight rods, each pair aimed at each other (every second pair
/// both or neither). Research/oracle.txt `# MD151` has where the car keeps them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Parts {
    pub car: u32,
    pub wheels: [u32; 4],
    pub fronts: [FrontParts; 2],
    pub axle: u32,
    pub links: [u32; 4],
    pub spinners: [u32; 2],
    pub rods: [[u32; 2]; 8],
}

/// What the class step reads off the car (MD151): the front tires' steer, every tire's angle,
/// compression, maximum compression and stretch (the wheel's `y` scale at full compression), the
/// left rear's mount, the arms' swing per unit of compression, the two front pivots (in the tires'
/// order) and the rear's, and the engine's rpm.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Posing {
    pub steer: [F; 2],
    pub angle: [F; 4],
    pub compression: [F; 4],
    pub max_compression: [F; 4],
    pub stretch: [F; 4],
    pub rear_mount: Vf,
    pub swing: F,
    pub pivots: [Vf; 2],
    pub rear_pivot: Vf,
    pub rpm: F,
}

/// The class step's axes (rodata): up, side and ahead, each `w` 1.
const POSE_UP: [F; 3] = [F::ZERO, F::ONE, F::ZERO];
const POSE_SIDE: [F; 3] = [F::ONE, F::ZERO, F::ZERO];
const POSE_AHEAD: [F; 3] = [F::ZERO, F::ZERO, F::ONE];
/// Ahead negated lane by lane (`-0`, `-0`, `-1`): the axis the left arm alone is spun about.
const POSE_BEHIND: [F; 3] = [F(f32::from_bits(0x8000_0000)), F(f32::from_bits(0x8000_0000)), F(f32::from_bits(0xbf80_0000))];
const POSE_UP_QUAD: [u32; 4] = [0, 0x3f80_0000, 0, 0x3f80_0000];
const POSE_AHEAD_QUAD: [u32; 4] = [0, 0, 0x3f80_0000, 0x3f80_0000];
/// The arms' swing's own factor on the compression (a data word, 1.25 in the buggy's image), the
/// spinners' turn per rpm, and a quarter turn.
const SWING_FACTOR: F = F(1.25);
const SPIN_PER_RPM: u32 = 0x3a03_126f;
const QUARTER_TURN: u32 = 0x3fc9_0fdb;

/// The angle a tilt's sine-like cross product gives: clamped to −1..1 (the original's three clamps
/// in a row are the one), a quarter turn less its arc cosine off the table.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
fn tilt(c: F, acos: &AcosTable) -> F {
    let c = if !(c <= F::ONE) {
        F::ONE
    } else if c < F(-1.0) {
        F(-1.0)
    } else {
        c
    };
    F::from_bits(QUARTER_TURN) - acos.at(c)
}

/// `a × b` (`vopmula a, b`, `vopmsub b, a`), its `z`.
fn cross_z(a: Vf, b: Vf) -> F {
    F(Vf::opmsub(Vf::opmula(a, b), b, a).z())
}

/// `xyz` of `a - b`, `w` `a`'s.
fn less(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.sub(b))
}

/// **The buggy's per-frame class step** (MD151; its vt `+0x488`, the update's [`Call::ClassStep`]):
/// the model's parts posed off the tires and the engine, on the scene nodes in `m`:
///
/// 1. the front uprights spun about up by their tire's steer, the front wheels about the side by
///    their tire's angle;
/// 2. each front corner, the right first: the arm spun about ahead by the swing times
///    [`SWING_FACTOR`] times its tire's compression (the left arm about ahead negated); the
///    knuckle spun to 0, then about ahead by the [`tilt`] of the two ends as seen from the corner's
///    pivot (each end's position in the car's frame less it, normalised; the second × the first);
///    the upright turned further about ahead by the tilt of the two ends as seen from the top (the
///    first × the second);
/// 3. the rear axle aimed (with up) from the left rear's mount, raised by the larger rear
///    compression and on the centre line, to the rear pivot; the rear wheels spun by their angles;
/// 4. each wheel, the rears first, stretched along `y` ([`node::stretch`]): with no maximum
///    compression by 1, else from 1 at none towards its stretch at full, kept between the two;
/// 5. with all four links, each pair (the second and fourth, then the third and first) aimed along
///    the direction from the second of the pair to the first, in the car's frame, the first of the
///    pair first; with both spinners, each turned about ahead by the rpm times [`SPIN_PER_RPM`];
/// 6. each rod pair (skipping every second pair unless it has both) aimed the same way, the second
///    of the pair first, along the direction from it to the first.
///
/// Nothing in the car's physics reads the parts back.
pub fn class_step(m: &mut impl node::Ram, parts: &Parts, p: &Posing, curve: &CurveTable, acos: &AcosTable) {
    let car = parts.car;
    fn at(m: &mut impl node::Ram, n: u32, car: u32) -> Vf {
        Vf::from_bits(node::position_in(m, n, car))
    }
    /// The pair aimed along the direction from its second to its first, in the car's frame.
    fn aim_pair(m: &mut impl node::Ram, [first, second]: [u32; 2], car: u32, second_first: bool) {
        let d = normalize(less(at(m, first, car), at(m, second, car))).to_bits();
        let order = if second_first { [second, first] } else { [first, second] };
        for n in order {
            node::aim_along(m, n, car, POSE_AHEAD_QUAD, d);
        }
    }
    let [left, right] = parts.fronts;
    node::spin_about(m, left.upright, POSE_UP, p.steer[FRONT_LEFT], curve);
    node::spin_about(m, parts.wheels[FRONT_LEFT], POSE_SIDE, p.angle[FRONT_LEFT], curve);
    node::spin_about(m, right.upright, POSE_UP, p.steer[FRONT_RIGHT], curve);
    node::spin_about(m, parts.wheels[FRONT_RIGHT], POSE_SIDE, p.angle[FRONT_RIGHT], curve);

    for (k, f, swing_axis) in [(FRONT_RIGHT, right, POSE_AHEAD), (FRONT_LEFT, left, POSE_BEHIND)] {
        node::spin_about(m, f.arm, swing_axis, p.swing * (SWING_FACTOR * p.compression[k]), curve);
        node::spin_about(m, f.knuckle, POSE_AHEAD, F::ZERO, curve);
        let a = normalize(less(at(m, f.ends[0], car), p.pivots[k]));
        let b = normalize(less(at(m, f.ends[1], car), p.pivots[k]));
        node::spin_about(m, f.knuckle, POSE_AHEAD, tilt(cross_z(b, a), acos), curve);
        let top = at(m, f.top, car);
        let a = normalize(less(at(m, f.ends[0], car), top));
        let b = normalize(less(at(m, f.ends[1], car), top));
        node::turn_about(m, f.upright, POSE_AHEAD, tilt(cross_z(a, b), acos), curve);
    }

    let lift = p.compression[REAR_LEFT].max(p.compression[REAR_RIGHT]);
    let mount = Vf::new(0.0, (F(p.rear_mount.y()) + lift).0, p.rear_mount.z(), 1.0);
    let d = normalize(less(p.rear_pivot, mount));
    node::aim(m, parts.axle, d.to_bits(), POSE_UP_QUAD);
    node::spin_about(m, parts.wheels[REAR_LEFT], POSE_SIDE, p.angle[REAR_LEFT], curve);
    node::spin_about(m, parts.wheels[REAR_RIGHT], POSE_SIDE, p.angle[REAR_RIGHT], curve);

    for k in [REAR_LEFT, REAR_RIGHT, FRONT_LEFT, FRONT_RIGHT] {
        let full = p.max_compression[k];
        let s = if full == F::ZERO {
            F::ONE
        } else {
            let t = (p.compression[k] - F::ZERO) / (full - F::ZERO);
            let k = p.stretch[k];
            Acc::adda(F::ZERO, F::ONE).madd(t, k - F::ONE).max(F::ONE.min(k)).min(F::ONE.max(k))
        };
        node::stretch(m, parts.wheels[k], s);
    }

    let l = parts.links;
    if l.iter().all(|&n| n != 0) {
        aim_pair(m, [l[3], l[1]], car, false);
        aim_pair(m, [l[2], l[0]], car, false);
    }
    if parts.spinners.iter().all(|&n| n != 0) {
        let a = F::from_bits(SPIN_PER_RPM) * p.rpm;
        for n in parts.spinners {
            node::turn_about(m, n, POSE_AHEAD, a, curve);
        }
    }
    for (k, &pair) in parts.rods.iter().enumerate() {
        if k % 2 == 0 || pair.iter().all(|&n| n != 0) {
            aim_pair(m, pair, car, true);
        }
    }
}

/// One of the golf cart's front arms (MD156a), as the car keeps it: the direction it is aimed along
/// and the up it is squared to before the tire's compression turns it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GolfArm {
    pub ahead: Vf,
    pub up: Vf,
}

/// One of the golf cart's front corners' posed parts (MD156a): the three arms (each squared to its
/// up turned by the tire's compression), the knuckle (aimed along its ahead turned by the tire's
/// steer, squared to the line between the two ends) and the two ends (the line from the second to
/// the first).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GolfCorner {
    pub arms: [u32; 3],
    pub knuckle: u32,
    pub ends: [u32; 2],
}

/// **The golf cart's posed parts** (MD156a): the car's own node (the frame the parts are measured
/// and aimed in), the steering wheel, the two front wheels (in the tires' order), the two front
/// corners (in the tires' order), two pairs of links aimed at each other, the rear axle, the rear
/// wheel, the hub (set where the rear rides) and the two shocks (each the node aimed and its foot).
/// Research/oracle.txt `# MD156a` has where the car keeps them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GolfParts {
    pub car: u32,
    pub steering: u32,
    pub wheels: [u32; 2],
    pub corners: [GolfCorner; 2],
    pub links: [[u32; 2]; 2],
    pub axle: u32,
    pub rear_wheel: u32,
    pub hub: u32,
    pub shocks: [[u32; 2]; 2],
}

/// What the golf cart's class step reads off the car and keeps on it (MD156a): the front tires'
/// steer, every tire's angle and compression, the right rear's mount; the car's up (its `w` set to
/// 1 by the step) and the steering frame the step builds off it; each front corner's arms (in the
/// tires' order, the arms in [`GolfCorner::arms`]' order) and its knuckle's ahead; the point the
/// rear axle is aimed at, the hub's point and the two shocks' tops.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GolfPosing {
    pub steer: [F; 2],
    pub angle: [F; 4],
    pub compression: [F; 4],
    pub rear_mount: Vf,
    pub up: Vf,
    pub steer_frame: [Vf; 4],
    pub arms: [[GolfArm; 3]; 2],
    pub knuckle_ahead: [Vf; 2],
    pub axle_to: Vf,
    pub hub: Vf,
    pub shock_tops: [Vf; 2],
}

/// The steering frame's origin (rodata, `(0, 0, 0, 1)`).
const GOLF_ORIGIN: [u32; 4] = [0, 0, 0, 0x3f80_0000];

/// `v`'s `x` and `y` turned about `z` by `angle` (the curve table's [`node::sin_cos`]): `x·cos − y·sin`
/// and `y·cos + x·sin` (`mula`, then `msub` or `madd`); `z` and `w` its words as they are.
fn turn_xy(v: [u32; 4], angle: F, curve: &CurveTable) -> [u32; 4] {
    let (sin, cos) = node::sin_cos(angle, curve);
    let (x, y) = (F::from_bits(v[0]), F::from_bits(v[1]));
    [Acc::mula(x, cos).msub(y, sin).to_bits(), Acc::mula(y, cos).madd(x, sin).to_bits(), v[2], v[3]]
}

/// `v`'s `x` and `z` turned about `y` by `angle`: `x·cos + z·sin` and `z·cos − x·sin`; `y` and `w`
/// its words as they are.
fn turn_xz(v: [u32; 4], angle: F, curve: &CurveTable) -> [u32; 4] {
    let (sin, cos) = node::sin_cos(angle, curve);
    let (x, z) = (F::from_bits(v[0]), F::from_bits(v[2]));
    [Acc::mula(x, cos).madd(z, sin).to_bits(), v[1], Acc::mula(z, cos).msub(x, sin).to_bits(), v[3]]
}

/// **The golf cart's per-frame class step** (MD156a; its vt `+0x488`, the update's
/// [`Call::ClassStep`]): the model's parts posed off the tires, on the scene nodes in `m`, and the
/// steering frame kept in `p`:
///
/// 1. the up's `w` set to 1, and the steering frame built off it ([`crate::math::basis`] at the
///    origin, ahead kept and the up squared to it); the steering wheel aimed ([`node::aim_as_is`],
///    with the up) at the point of that frame the left front's steer points to (its sine and cosine
///    off the curve table's entry below it, `(sin, 0, cos)`);
/// 2. the front wheels spun about the side by their tire's angle;
/// 3. each front corner, the right first: each arm aimed along its ahead squared to its up turned
///    about `z` by the tire's compression (the left's negated) ([`node::aim_squared`] in the car's
///    frame); the knuckle aimed along its ahead turned about `y` by the tire's steer, squared to the
///    direction from the second end to the first;
/// 4. each link pair aimed (the second first) along the direction from the second to the first;
/// 5. the rear axle aimed (with up) at its point from one on the centre line, as high as the point
///    raised by the larger rear compression and as far ahead as the right rear's mount; the rear
///    wheel spun by the right rear's angle; the hub set at its point raised by that compression;
/// 6. each shock aimed along the direction from its foot to its top, squared to up.
///
/// Nothing in the car's physics reads the parts back.
pub fn golf_class_step(m: &mut impl node::Ram, parts: &GolfParts, p: &mut GolfPosing, curve: &CurveTable) {
    let car = parts.car;
    fn at(m: &mut impl node::Ram, n: u32, car: u32) -> Vf {
        Vf::from_bits(node::position_in(m, n, car))
    }
    p.up.0[3] = 1.0;
    let up = p.up.to_bits();
    p.steer_frame = crate::math::basis(Vf::from_bits(GOLF_ORIGIN), Vf::from_bits(POSE_AHEAD_QUAD), p.up, true, false).0;
    let (cos, sin) = curve.coarse(p.steer[FRONT_LEFT]);
    let dir = crate::atv_moto::frame_point(&p.steer_frame, Vf::new(sin.0, 0.0, cos.0, 0.0));
    node::aim_as_is(m, parts.steering, dir.to_bits(), up);
    node::spin_about(m, parts.wheels[FRONT_LEFT], POSE_SIDE, p.angle[FRONT_LEFT], curve);
    node::spin_about(m, parts.wheels[FRONT_RIGHT], POSE_SIDE, p.angle[FRONT_RIGHT], curve);

    for (k, swing) in [(FRONT_RIGHT, p.compression[FRONT_RIGHT]), (FRONT_LEFT, -p.compression[FRONT_LEFT])] {
        let c = parts.corners[k];
        for (&n, arm) in c.arms.iter().zip(p.arms[k]) {
            let up = turn_xy(arm.up.to_bits(), swing, curve);
            node::aim_squared(m, n, car, arm.ahead.to_bits(), up);
        }
        let a = at(m, c.ends[0], car);
        let b = at(m, c.ends[1], car);
        let toward = normalize(less(a, b)).to_bits();
        let ahead = turn_xz(p.knuckle_ahead[k].to_bits(), p.steer[k], curve);
        node::aim_squared(m, c.knuckle, car, ahead, toward);
    }

    for [first, second] in parts.links {
        let d = normalize(less(at(m, first, car), at(m, second, car))).to_bits();
        for n in [second, first] {
            node::aim_along(m, n, car, POSE_AHEAD_QUAD, d);
        }
    }

    let lift = p.compression[REAR_LEFT].max(p.compression[REAR_RIGHT]);
    let raised = p.rear_mount.merge(Dest::XYZ, p.rear_mount.add(Vf::from_bits(POSE_UP_QUAD).mul(Vf::splat(lift.0))));
    let from = Vf::from_bits([0, (F(p.axle_to.y()) + lift).to_bits(), raised.z().to_bits(), F::ONE.to_bits()]);
    let d = normalize(less(p.axle_to, from));
    node::aim(m, parts.axle, d.to_bits(), POSE_UP_QUAD);
    node::spin_about(m, parts.rear_wheel, POSE_SIDE, p.angle[REAR_RIGHT], curve);
    let hub = p.hub.to_bits();
    node::set_position(m, parts.hub, [hub[0], (F(p.hub.y()) + lift).to_bits(), hub[2], hub[3]]);

    for ([shock, foot], top) in parts.shocks.into_iter().zip(p.shock_tops) {
        let d = normalize(less(top, at(m, foot, car)));
        node::aim_squared(m, shock, car, d.to_bits(), POSE_UP_QUAD);
    }
}

/// **The monster truck's posed parts** (MD156b): the car's own node (the frame the parts are
/// measured and aimed in), the four wheels, the four uprights (each set where its tire rides and
/// steered), the four mounts (set where their tire rides), the two axles (set between their pair of
/// tires and leaned to them, the front first), the two shocks aimed from themselves, and the eight
/// rods, each the node aimed and the node it is measured from. Wheels, uprights and mounts in the
/// tires' order. Research/oracle.txt `# MD156b` has where the car keeps them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TruckParts {
    pub car: u32,
    pub wheels: [u32; 4],
    pub uprights: [u32; 4],
    pub mounts: [u32; 4],
    pub axles: [u32; 2],
    pub shocks: [u32; 2],
    pub rods: [[u32; 2]; 8],
}

/// What the monster truck's class step reads off the car (MD156b): every tire's steer, angle,
/// compression, maximum compression and stretch (as [`Posing`]'s); the uprights' and mounts' points
/// (each raised by its tire's compression), the axles' points (the front's, then the rear's), the
/// track (the axle's half width the lean is measured over), the rear tires' share of the steer, and
/// the points the shocks and rods are aimed from (the two shocks', then the eight rods').
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TruckPosing {
    pub steer: [F; 4],
    pub angle: [F; 4],
    pub compression: [F; 4],
    pub max_compression: [F; 4],
    pub stretch: [F; 4],
    pub upright_at: [Vf; 4],
    pub mount_at: [Vf; 4],
    pub axle_at: [Vf; 2],
    pub track: F,
    pub rear_steer: F,
    pub tops: [Vf; 10],
}

/// A point's `xyz` words with its `y` raised by `by` (`add.s`).
fn raised(p: Vf, by: F) -> [u32; 4] {
    let q = p.to_bits();
    [q[0], (F(p.y()) + by).to_bits(), q[2], q[3]]
}

/// An axle's lean off its two tires' compressions (MD156b): the arc cosine (the table's) of the
/// track over the length of (`track`, the second's compression less the first's) (`rsqrt`), clamped
/// to −1..1 (twice in the original, the same as once), negated when the second tire is the less
/// compressed.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
fn truck_lean(track: F, first: F, second: F, acos: &AcosTable) -> F {
    let d = second - first;
    let c = track * F::ONE.rsqrt(Acc::mula(track, track).madd(d, d));
    let c = if !(c <= F::ONE) {
        F::ONE
    } else if c < F(-1.0) {
        F(-1.0)
    } else {
        c
    };
    let a = acos.at(c);
    if d < F::ZERO { -a } else { a }
}

/// **The monster truck's per-frame class step** (MD156b; its vt `+0x488`, the update's
/// [`Call::ClassStep`]): the model's parts posed off the tires, on the scene nodes in `m`:
///
/// 1. each corner (left front, right front, right rear, left rear): the upright set at its point
///    raised by the tire's compression and spun about up by the tire's steer (a rear's times the
///    rear share); the wheel spun about the side by the tire's angle;
/// 2. each wheel, the rears first, stretched along `y` as the buggy's ([`class_step`]'s step 4);
/// 3. each mount set at its point raised by its tire's compression;
/// 4. each axle, the front first: set at its point raised by half its tires' summed compressions,
///    and spun about ahead by their [`truck_lean`] (the left tire first);
/// 5. each shock, then each rod, aimed (squared to up, in the car's frame) along the direction from
///    its top to where it (a rod: the node it is measured from) is in the car's frame.
///
/// Nothing in the car's physics reads the parts back; the step keeps nothing on the car.
pub fn truck_class_step(m: &mut impl node::Ram, parts: &TruckParts, p: &TruckPosing, curve: &CurveTable, acos: &AcosTable) {
    let car = parts.car;
    for k in [FRONT_LEFT, FRONT_RIGHT, REAR_RIGHT, REAR_LEFT] {
        node::set_position(m, parts.uprights[k], raised(p.upright_at[k], p.compression[k]));
        let steer = if k < REAR_LEFT { p.steer[k] } else { p.rear_steer * p.steer[k] };
        node::spin_about(m, parts.uprights[k], POSE_UP, steer, curve);
        node::spin_about(m, parts.wheels[k], POSE_SIDE, p.angle[k], curve);
    }

    for k in [REAR_LEFT, REAR_RIGHT, FRONT_LEFT, FRONT_RIGHT] {
        let full = p.max_compression[k];
        let s = if full == F::ZERO {
            F::ONE
        } else {
            let t = (p.compression[k] - F::ZERO) / (full - F::ZERO);
            let k = p.stretch[k];
            Acc::adda(F::ZERO, F::ONE).madd(t, k - F::ONE).max(F::ONE.min(k)).min(F::ONE.max(k))
        };
        node::stretch(m, parts.wheels[k], s);
    }

    for k in [FRONT_LEFT, FRONT_RIGHT, REAR_LEFT, REAR_RIGHT] {
        node::set_position(m, parts.mounts[k], raised(p.mount_at[k], p.compression[k]));
    }

    for (axle, at, [l, r]) in [(0, p.axle_at[0], [FRONT_LEFT, FRONT_RIGHT]), (1, p.axle_at[1], [REAR_LEFT, REAR_RIGHT])] {
        let (cl, cr) = (p.compression[l], p.compression[r]);
        let y = Acc::adda(F::ZERO, F(at.y())).madd(F(0.5), cl + cr);
        let q = at.to_bits();
        node::set_position(m, parts.axles[axle], [q[0], y.to_bits(), q[2], q[3]]);
        node::spin_about(m, parts.axles[axle], POSE_AHEAD, truck_lean(p.track, cl, cr, acos), curve);
    }

    let aimed = parts.shocks.map(|n| [n, n]).into_iter().chain(parts.rods);
    for ([n, from], top) in aimed.zip(p.tops) {
        let d = normalize(less(Vf::from_bits(node::position_in(m, from, car)), top));
        node::aim_squared(m, n, car, d.to_bits(), POSE_UP_QUAD);
    }
}

/// SLUS-21104's physics world as the car's frame sees it: one rigid body, the car's three
/// listeners, no constraints.
struct Frame<'a, S> {
    car: &'a mut Car,
    body: &'a mut Body,
    table: &'a AcosTable,
    s: &'a mut S,
    /// The body's integrate is Untamed's ([`Body::integrate_untamed`]).
    untamed: bool,
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
        if self.untamed {
            self.body.integrate_untamed(h);
        } else {
            self.body.integrate(h);
        }
    }
    fn end_substep(&mut self, _: i32) {
        self.body.end_substep();
    }
    fn constrain(&mut self, _: i32, _: bool) {}
    fn post(&mut self, _: i32, dt: F) {
        post_process_on(self.car, self.body, dt, self.table, self.s, self.untamed);
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
    world.step(dt, &mut Frame { car, body, table, s, untamed: false });
}

/// **MX vs. ATV Untamed's car frame** (MD161): [`frame`] with the body's integrate Untamed's
/// ([`Body::integrate_untamed`], the angular damping per time). The listeners are [`frame`]'s: what
/// Untamed changes in them is switched by [`Keys::untamed`], [`Vehicle::untamed`] and
/// [`Update::untamed`]; the PostProcess's yaw hold is [`post_process_untamed`]'s.
pub fn frame_untamed(world: &crate::biplane::World, car: &mut Car, body: &mut Body, dt: F, table: &AcosTable, s: &mut impl Steps) {
    world.step(dt, &mut Frame { car, body, table, s, untamed: true });
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
    /// Untamed's preload cue (MD81d): played while the push is on rather than while its timer is
    /// above 0 (they differ with `PreloadTime` 0, Untamed's baja, and while armed). Off, SLUS-21104's.
    pub untamed: bool,
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
/// controls' reverse switch without the engine's keys, the preload's cue, and what the sort, the
/// probe, the engine and the tire step (MD131: what a tire stands on) hand out go to `rest`. (MD154)
/// The respawn's tire and engine resets ([`tire_reset`], [`engine_reset`]).
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
                // What a tire stands on is the rest's to say (MD131), asked into the car's word.
                let (mut t, mut s, cofg) = (car.tires[i], self.state.tires[i], car.cofg);
                let rest = &mut self.rest;
                let mut ask = |c: Call, body: &mut Body| {
                    car.stood_on = Under::Still;
                    rest.call(c, car, body);
                    car.stood_on
                };
                tire_step(&self.keys.tires[i], &mut t, &mut s, &self.state, cofg, body, h, &mut ask);
                (car.tires[i], self.state.tires[i]) = (t, s);
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
            Call::TireReset(k) => tire_reset(&mut car.tires[k], &mut self.state.tires[k]),
            Call::EngineReset => engine_reset(&mut self.state.engine),
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

/// The `[Joystick]` keys the car's input step reads: the gas button's full pressure, and the speed
/// and sideways angle past which (fast and not sliding) any press is the whole pedal (MD107).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InputKeys {
    pub max_analog_throttle: F,
    pub digital_throttle_speed: F,
    pub digital_side_slip_angle: F,
}

/// The car's pad words as its input step leaves them for the controls ([`State`]'s of the same
/// names), and whether the gas pedal has come on yet: until it does the brake is held on.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    pub stick: F,
    pub lean: F,
    pub pad_word: F,
    pub gas_pedal: F,
    pub vehicle_brake: F,
    pub hand_brake: F,
    pub clutch: F,
    pub dpad: bool,
    pub gassed: bool,
}

/// The hand brake's command on SLUS-21104's car (MX Unleashed's is [`crate::car`]'s `0x16`).
const HAND_BRAKE: crate::bindings::Command = crate::bindings::Command(0x18);

/// **The car's input step** (MD107; MX Unleashed's [`crate::car::Controls::input`]), once a frame
/// for a car a person drives, before the sim frame: the stick `(steer, lean)` and the pad word as
/// they come, then each pedal from its command's strength (`value`). `speed` is the car's +0x32c and
/// `side_slip` its +0x330: slow, or sliding past the angle, the gas is the button's pressure over
/// `MaxAnalogThrottle` (none when that is 0), else any press is the whole pedal. The first gas holds
/// the brake no longer. The d-pad's left or right is the whole stick; with neither, a stick to the
/// right says the d-pad no longer steers. Up or down is the whole lean.
///
/// SLUS-21104 adds to MX Unleashed's the held brake; the remap is inline (`adda.s`/`madd.s`).
pub fn input(k: &InputKeys, p: &mut Input, speed: F, side_slip: F, stick: (F, F), pad_word: F, value: impl Fn(crate::bindings::Command) -> F) {
    input_on(false, k, p, speed, side_slip, stick, pad_word, value);
}

/// **Untamed's input step** (MD81d, SLUS-21701's): [`input`] with the clutch any strength over 0
/// (SLUS-21104's any but 0; they part on a negative strength). Untamed also reads each command but the
/// gas as not pressed when it is unbound, reads the stick and the strengths off the controls block and
/// the pad word as player 0's preload command: the caller's `stick`, `pad_word` and `value`.
pub fn input_untamed(k: &InputKeys, p: &mut Input, speed: F, side_slip: F, stick: (F, F), pad_word: F, value: impl Fn(crate::bindings::Command) -> F) {
    input_on(true, k, p, speed, side_slip, stick, pad_word, value);
}

#[allow(clippy::too_many_arguments)]
fn input_on(untamed: bool, k: &InputKeys, p: &mut Input, speed: F, side_slip: F, stick: (F, F), pad_word: F, value: impl Fn(crate::bindings::Command) -> F) {
    use crate::bindings::Command;
    (p.stick, p.lean) = stick;
    p.pad_word = pad_word;
    p.gas_pedal = if speed <= k.digital_throttle_speed || !(side_slip.abs() < k.digital_side_slip_angle) {
        let (s, r) = (value(Command::GAS), k.max_analog_throttle);
        if F::ZERO == r {
            F::ZERO - F::ZERO
        } else {
            let t = (s - F::ZERO) / (r - F::ZERO);
            let lo = F::ZERO.min(F::ONE);
            let hi = F::ZERO.max(F::ONE);
            Acc::adda(F::ZERO, F::ZERO).madd(t, F::ONE - F::ZERO).max(lo).min(hi)
        }
    } else if value(Command::GAS) <= 0.0 {
        F::ZERO
    } else {
        F::ONE
    };
    if !(p.gas_pedal <= 0.0) {
        p.gassed = true;
    }
    p.vehicle_brake = if !(value(Command::BRAKE) <= 0.0) || !p.gassed { F::ONE } else { F::ZERO };
    p.hand_brake = if value(HAND_BRAKE) <= 0.0 { F::ZERO } else { F::ONE };
    p.clutch = if untamed {
        if value(Command::CLUTCH) <= 0.0 { F::ZERO } else { F::ONE }
    } else if value(Command::CLUTCH) == 0.0 {
        F::ZERO
    } else {
        F::ONE
    };
    if value(Command::LEFT) != 0.0 {
        p.stick = F(-1.0);
        p.dpad = true;
    } else if value(Command::RIGHT) != 0.0 {
        p.stick = F::ONE;
        p.dpad = true;
    } else if !(p.stick <= 0.0) {
        p.dpad = false;
    }
    if value(Command::UP) != 0.0 {
        p.lean = F(-1.0);
    } else if value(Command::DOWN) != 0.0 {
        p.lean = F::ONE;
    }
}

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
        if if k.untamed { p.active } else { !(p.timer <= 0.0) } {
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

/// The map-edge test's margin: a point this near an edge of the map's area is off it.
const EDGE_MARGIN: f32 = 150.0;

/// What SLUS-21104's own calls in the car's frame read and write beyond [`Car`] and the body
/// (research/oracle.txt `# MD106b`, `# MD128`): MXVehicle's wreck words, the world's two words the
/// wreck and the force touch, the flag bytes, the scene node and its subtree, the world's bounds, the
/// map's area, the collidables and the collision record.
#[derive(Clone, Debug, Default, PartialEq)]
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
    /// The car's node (node 0) and its parents, and the world's bounds. Node 0's subtree is
    /// [`Vehicle::below`]; `subtree` says it has one.
    pub scene: crate::atv_moto::Scene,
    pub subtree: bool,
    pub bounds: crate::atv_moto::Bounds,
    /// [`Call::OutOfBounds`]'s answer.
    pub out_of_bounds: bool,
    /// MD128's. The map's area the edge test reads (the terrain's extent; `None`, never off it), and
    /// the random numbers its throw draws.
    pub area: Option<crate::biplane::Area>,
    pub rng: crate::rng::Rng,
    /// The car has an entry in the world's spatial database (`+8`): the spatial update moves it.
    pub spatial: bool,
    /// The car's collidables as the collision sync sees them (the sync's switch is bit 0 of the byte
    /// before the flag byte).
    pub collidables: crate::atv_moto::Collidables,
    /// The rest of that byte (`+0x44`), which the body's reset reads: bit 7 lets the riding probe go
    /// rather than hand the record back, bit 6 clears bit 1 of `flag_word` (`+0x24`).
    pub reset_flags: u8,
    pub flag_word: u32,
    /// The record has an owner (`+0xdc`) to be handed back to.
    pub owned: bool,
    /// The record's contacts, which the gravity's call resolves ([`crate::atv_contact::resolve`] on
    /// the car's body; the held body is not read). `None`, and [`Call::GravityRecord`] goes on.
    pub held: Option<crate::atv_contact::Held>,
    /// Node 0's subtree, its first child first; empty, and [`Call::SubtreeOutOfDate`] goes on.
    pub below: Vec<Below>,
    /// Untamed's vehicle force (MD81d): after the gravity it adds a zero force as well (`xyz`), which
    /// turns a -0 into +0; and Untamed's put-back (MD183): its ground reset takes no collidable out
    /// of the database before the orient ([`respawn_ground`]). Off, SLUS-21104's.
    pub untamed: bool,
    /// The respawn's words (MD154); `None`, and [`Call::Respawn`] goes on.
    pub back: Option<Back>,
}

/// What the car's respawn reads and writes beyond [`Car`], the body and the rest of [`Vehicle`]
/// (MD154).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Back {
    /// The put-back spot: its position, forward and up (`+0xd40`, `+0xd50`, `+0xd60`); and the
    /// put-back's copy of it (`+0x50`, `+0x60`, `+0x70`), which the orient squares and normalizes.
    pub spot: [Vf; 3],
    pub at: [Vf; 3],
    /// The run's spot words (`+0xd70`, `+0xd74`, `+0xd78`), and the run's own three (its `+0x94`);
    /// `None`, the car has no run.
    pub run_spot: [u32; 3],
    pub run: Option<[u32; 3]>,
    /// The car has a terrain (`+0x3b4`): the put-back stands the spot on its ground
    /// ([`Call::BackGround`]).
    pub terrain: bool,
    /// The ghost's clock (`+0xde8`) and how long a ghost lasts (`+0xdec`).
    pub ghost: F,
    pub ghost_time: F,
    /// The car has a trail (`+0x10dc`), which the put-back resets ([`Call::TrailReset`]).
    pub trail: bool,
    /// The car has its engine and all four tires: the respawn resets them ([`Call::TireReset`],
    /// [`Call::EngineReset`]).
    pub parts: bool,
    /// The body's staged words, which its commit makes its own: the position (`+0x40`), velocity
    /// (`+0x50`), spin (`+0x160`) and orientation (`+0x180`).
    pub staged_position: Vf,
    pub staged_velocity: Vf,
    pub staged_spin: Vf,
    pub staged_quat: Vf,
}

/// A node of the car node's subtree as marking it out of date sees it: its first child and next
/// sibling (places in [`Vehicle::below`]) and its flag byte (bit 0 current).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Below {
    pub child: Option<usize>,
    pub sibling: Option<usize>,
    pub flags: u8,
}

/// **SLUS-21104's own calls answered** (MD106b): the vehicle force and post, the wreck timer, the
/// landing's node-local turn and the node's position, each ported; and (MD128) what those hand out:
/// the map-edge test and throw, the body's reset, the spatial update, the node's bounds, the collision
/// sync and each collidable's, the record's gravity call and the subtree's marking; and (MD154), given
/// [`Vehicle::back`] and a node with no parent, the respawn ([`respawn`]). Every other call, and what
/// these hand out in turn, goes on to `rest`.
pub struct Own<'a, S> {
    pub v: &'a mut Vehicle,
    pub rest: &'a mut S,
}

impl<S: Steps> Steps for Own<'_, S> {
    fn call(&mut self, c: Call, car: &mut Car, body: &mut Body) {
        match c {
            Call::VehicleForce => vehicle_force(self, body, car),
            Call::VehiclePost(_) => vehicle_post(self, car, body),
            Call::WreckTimer(dt) => wreck_timer(self, car, body, dt),
            Call::Landed(shock) => {
                let out = node_local(&mut self.v.scene, 0, shock);
                self.rest.call(Call::LandingSound(out), car, body);
            }
            Call::NodePosition => car.node_pos = self.v.scene.position(0),
            Call::OutOfBounds => out_of_bounds(self.v, body),
            Call::BodyReset => body_reset(self, car, body),
            Call::GravityRecord if self.v.held.as_ref().is_some_and(|h| h.entries.iter().all(|e| e.contact.other.is_none())) => {
                let h = self.v.held.as_mut().expect("tested");
                let mut me = crate::atv_contact::Weighed { body, mass: h.mass, cofg: car.cofg };
                let side = &mut crate::contact::Own::Body(&mut me);
                crate::atv_contact::resolve(&mut h.record, side, &mut h.entries, &mut [], &h.movers, true);
            }
            Call::SubtreeOutOfDate if !self.v.below.is_empty() => out_of_date(&mut self.v.below, 0),
            Call::Respawn if self.v.back.is_some() && self.v.scene.nodes[0].parent.is_none() => respawn(self, car, body),
            Call::SpatialUpdate => {
                if self.v.spatial {
                    let (centre, half) = node_box(self, car, body);
                    self.rest.call(Call::SpatialMove { node: false, centre, half, reach: F::ZERO }, car, body);
                }
            }
            Call::NodeBounds => {
                if self.v.scene.nodes[0].spatial {
                    let (centre, half) = node_box(self, car, body);
                    let (centre, half, reach) = self.v.scene.database_box(0, centre, half);
                    self.rest.call(Call::SpatialMove { node: true, centre, half, reach }, car, body);
                }
            }
            Call::CollisionSync => {
                for c in crate::atv_moto::collision_sync(&self.v.collidables) {
                    match c {
                        crate::quad::Call::PartSync(p) => part_sync(self, p, car, body),
                        crate::quad::Call::Gather(p) => self.rest.call(Call::Gather(p), car, body),
                        c => unreachable!("the collision sync's {c:?}"),
                    }
                }
            }
            c => self.rest.call(c, car, body),
        }
    }
}

/// **The vehicle's force**, the Force body's first call: with the body enabled, the world's gravity
/// into its force (`xyz`), the collision record told ([`Call::GravityRecord`]) if there is one, and
/// bit 5 of the flag byte cleared. With the body off, nothing.
pub fn vehicle_force<S: Steps>(o: &mut Own<'_, S>, body: &mut Body, car: &mut Car) {
    if !body.enabled {
        return;
    }
    body.force = body.force.merge(Dest::XYZ, body.force.add(o.v.gravity));
    if o.v.untamed {
        body.force = body.force.merge(Dest::XYZ, body.force.add(Vf::ZERO));
    }
    if o.v.record {
        o.call(Call::GravityRecord, car, body);
    }
    o.v.flags &= !0x20;
}

/// **The vehicle's post step**, the PostProcess's first call: with the body enabled, the car's
/// origin (the body's position less its centre of mass turned into the world,
/// [`crate::atv_moto::origin`]) tested against the world ([`crate::atv_moto::in_world`]); inside, the
/// node placed there with the body's orientation (its subtree marked out of date by
/// [`Call::SubtreeOutOfDate`]); outside, [`Call::BodyReset`] and [`Call::Respawn`].
/// Then [`Call::SpatialUpdate`], [`Call::NodeBounds`] with bit 1 of the flag byte, and
/// [`Call::CollisionSync`]; bit 2 cleared. The quad's is the same code ([`crate::atv_moto::vehicle_post`]).
pub fn vehicle_post<S: Steps>(o: &mut Own<'_, S>, car: &mut Car, body: &mut Body) {
    if !body.enabled {
        return;
    }
    let at = crate::atv_moto::origin(&body.view(), car.cofg);
    if crate::atv_moto::in_world(o.v.flags, &o.v.bounds, at) {
        let current = o.v.scene.nodes[0].flags & 1 != 0;
        o.v.scene.place(0, body.quat, at);
        if current && o.v.subtree {
            o.call(Call::SubtreeOutOfDate, car, body);
        }
    } else {
        o.call(Call::BodyReset, car, body);
        o.call(Call::Respawn, car, body);
    }
    o.call(Call::SpatialUpdate, car, body);
    if o.v.flags & 0x02 != 0 {
        o.call(Call::NodeBounds, car, body);
    }
    o.call(Call::CollisionSync, car, body);
    o.v.flags &= !0x04;
}

/// **The map-edge test and throw** (MXVehicle's out-of-bounds slot, vt `+0x16c`; MD128): held to the
/// map, the body's position within [`EDGE_MARGIN`] of the area's edge or past it
/// ([`crate::biplane::Area::outside_mxvatv`]) throws the body back towards the middle
/// ([`crate::biplane::throw_mxvatv`]: its velocity and rates, the world's drag word halved) and
/// answers yes, into [`Vehicle::out_of_bounds`]. The biplane's slot is the same function.
fn out_of_bounds(v: &mut Vehicle, body: &mut Body) {
    v.out_of_bounds = false;
    let Some(area) = v.area.filter(|_| v.bounded) else { return };
    if area.outside_mxvatv(body.position, F(EDGE_MARGIN)) {
        crate::biplane::throw_mxvatv(&area, body.position, &mut body.velocity, &mut body.rates, &mut v.drag, &mut v.rng);
        v.out_of_bounds = true;
    }
}

/// **The body's reset** (MXVehicle's vt `+0x13c`, out of the world; MD128), as the bike's
/// ([`crate::handling::reset::body_reset`]) on the car's words: with the body enabled, the body taken
/// off the world's three lists ([`Call::LeaveWorld`]) and turned off; with a record and its owner,
/// the riding probe let go ([`Call::RidingLetGo`], with bit 7 of [`Vehicle::reset_flags`] and a riding
/// probe) or the record handed back ([`Call::ReleaseRecord`]); with bit 6, bit 1 of the flag word
/// cleared.
fn body_reset<S: Steps>(o: &mut Own<'_, S>, car: &mut Car, body: &mut Body) {
    if !body.enabled {
        return;
    }
    for k in 0..3 {
        o.rest.call(Call::LeaveWorld(k), car, body);
    }
    body.enabled = false;
    if o.v.owned && o.v.record {
        if o.v.reset_flags & 0x80 != 0 {
            if o.v.collidables.riding.is_some() {
                o.rest.call(Call::RidingLetGo, car, body);
            }
        } else {
            o.rest.call(Call::ReleaseRecord, car, body);
        }
    }
    if o.v.reset_flags & 0x40 != 0 {
        o.v.flag_word &= !2;
    }
}

/// The car's node's box in the world (as the quad's): its frame brought up to date, its bounds
/// gathered again if they are stale ([`Call::NodeRebound`], then bit 7 cleared), then
/// [`crate::atv_moto::Scene::world_box`].
fn node_box<S: Steps>(o: &mut Own<'_, S>, car: &mut Car, body: &mut Body) -> (Vf, Vf) {
    if o.v.scene.bounds_stale(0) {
        o.rest.call(Call::NodeRebound, car, body);
        o.v.scene.rebounded(0);
    }
    o.v.scene.world_box(0)
}

/// A collidable's sync (as the quad's): the car's node's frame brought up to date, the part stood at
/// it, and its box moved in the database ([`Call::PartMove`]) if it has an entry.
fn part_sync<S: Steps>(o: &mut Own<'_, S>, p: crate::quad::Part, car: &mut Car, body: &mut Body) {
    let v = &mut *o.v;
    v.scene.update(0);
    let frame = v.scene.nodes[0].world;
    let part = &mut v.collidables.states[crate::atv_sync::slot(p)];
    if let Some((centre, half, reach)) = crate::atv_sync::part_sync(part, &frame) {
        o.rest.call(Call::PartMove { part: p, centre, half, reach }, car, body);
    }
}

/// The quadword the respawn's resets and the orient read for "none" (`0, 0, 0, 1`), and the tire
/// reset's up (`0, 1, 0, 1`).
const BACK_ORIGIN: Vf = Vf([0.0, 0.0, 0.0, 1.0]);
const BACK_UP: Vf = Vf([0.0, 1.0, 0.0, 1.0]);
/// How far the put-back lifts the spot along the ground's normal (0.3).
const BACK_LIFT: u32 = 0x3e99_999a;

/// **The car's respawn** (MXVehicle's vt `+0xe8`; MD154): the put-back, snapped to the ground and
/// starting the ghost; then, with the engine and all four tires, each tire's reset and the engine's
/// ([`Call::TireReset`], [`Call::EngineReset`]).
///
/// Untamed's put-back (MD183) is the same code at its own offsets, with [`Vehicle::untamed`]'s ground
/// reset.
///
/// The put-back: the spot's position copied in; with a run whose spot's first two words are both
/// set, the run's three words the spot's; with a terrain, the ground queried at the position
/// ([`Call::BackGround`]), the position lifted along its normal by 0.3 (`xyz`), the forward the
/// spot's and the up the normal; without, the forward and up the spot's. Then the ground reset
/// ([`respawn_ground`]) and the latch set; the ghost's clock its length and the ghost started
/// ([`Call::GhostStart`]); with a trail, [`Call::TrailReset`]; and the put-back's cue
/// ([`Call::Cue`] 21).
fn respawn<S: Steps>(o: &mut Own<'_, S>, car: &mut Car, body: &mut Body) {
    let b = o.v.back.as_mut().expect("tested");
    b.at[0] = b.spot[0];
    if let Some(run) = b.run.as_mut().filter(|_| b.run_spot[0] != 0 && b.run_spot[1] != 0) {
        *run = b.run_spot;
    }
    if b.terrain {
        let at = b.at[0];
        o.rest.call(Call::BackGround(at), car, body);
        let n = car.ground_normal;
        let b = o.v.back.as_mut().expect("tested");
        b.at[0] = b.at[0].merge(Dest::XYZ, b.at[0].add(scale_xyz(n, BACK_LIFT)));
        (b.at[1], b.at[2]) = (b.spot[1], n);
    } else {
        (b.at[1], b.at[2]) = (b.spot[1], b.spot[2]);
    }
    respawn_ground(o, car, body);
    car.latch = true;
    let b = o.v.back.as_mut().expect("tested");
    b.ghost = b.ghost_time;
    let trail = b.trail;
    o.rest.call(Call::GhostStart, car, body);
    if trail {
        o.rest.call(Call::TrailReset, car, body);
    }
    o.rest.call(Call::Cue(21), car, body);
    if o.v.back.as_ref().expect("tested").parts {
        for k in 0..4 {
            o.rest.call(Call::TireReset(k), car, body);
        }
        o.rest.call(Call::EngineReset, car, body);
    }
}

/// **The ground reset** (the put-back's, as the bike's [`crate::handling::wreck::ground`]): with bit 1
/// of the flag byte the node taken out of the database ([`Call::NodeUnhook`]); the frame's entry
/// taken out if there is a frame, the riding probe's and the sweep's if there is a riding probe
/// ([`Call::PartUnhook`]; Untamed's, [`Vehicle::untamed`], takes neither out); the body's staged velocity [`BACK_ORIGIN`]; the orient ([`respawn_orient`]);
/// with that bit the node hung back ([`Call::NodeRehang`]); and with the sync's switch, the frame,
/// then the riding probe and the sweep (with a riding probe), each turned on and attached
/// ([`Call::PartOn`], [`Call::PartAttach`]).
fn respawn_ground<S: Steps>(o: &mut Own<'_, S>, car: &mut Car, body: &mut Body) {
    use crate::quad::Part;
    let bit = o.v.flags & 0x02 != 0;
    if bit {
        o.rest.call(Call::NodeUnhook, car, body);
    }
    let c = o.v.collidables;
    if !o.v.untamed {
        if let Some(p) = c.frame {
            o.rest.call(Call::PartUnhook(p), car, body);
        }
        if let Some(p) = c.riding {
            o.rest.call(Call::PartUnhook(p), car, body);
            o.rest.call(Call::PartUnhook(Part::Sweep), car, body);
        }
    }
    o.v.back.as_mut().expect("tested").staged_velocity = BACK_ORIGIN;
    respawn_orient(o, car, body);
    if bit {
        o.rest.call(Call::NodeRehang, car, body);
    }
    if !c.sync {
        return;
    }
    let mut on = |p: Part, o: &mut Own<'_, S>| {
        o.rest.call(Call::PartOn(p), car, body);
        o.rest.call(Call::PartAttach(p), car, body);
    };
    if let Some(p) = c.frame {
        on(p, o);
    }
    if let Some(p) = c.riding {
        on(p, o);
        on(Part::Sweep, o);
    }
}

/// **The car's orient** (its vt `+0xf0`, the bike's [`crate::handling::wreck::orient`]) on the
/// put-back's position, forward and up: the up made square to the forward (`(forward × up) ×
/// forward`, its `w` the forward's) and both normalized, the position's `w` 1; the body staged there,
/// turned to the frame's quaternion with no spin, and made so ([`respawn_commit`]); the node set at the
/// body's centre less its turned centre of mass (its subtree marked out of date,
/// [`Call::SubtreeOutOfDate`], if it was current), and aimed along the forward and up; then
/// [`Call::SpatialUpdate`], [`Call::NodeBounds`] with bit 1 of the flag byte, and with the sync's
/// switch the frame's and the riding probe's syncs.
fn respawn_orient<S: Steps>(o: &mut Own<'_, S>, car: &mut Car, body: &mut Body) {
    use crate::air::{cross, normalize};
    let b = o.v.back.as_mut().expect("tested");
    let (f, u) = (b.at[1], b.at[2]);
    let t = f.merge(Dest::XYZ, cross(f, u));
    b.at[2] = t.merge(Dest::XYZ, cross(t, f));
    b.at[1] = normalize(b.at[1]);
    b.at[2] = normalize(b.at[2]);
    b.at[0].0[3] = 1.0;
    let (f, u) = (b.at[1], b.at[2]);
    let q = crate::math::mat4_to_quat(&crate::math::basis(BACK_ORIGIN, f, u, false, true));
    (b.staged_position, b.staged_quat, b.staged_spin) = (b.at[0], q, BACK_ORIGIN);
    respawn_commit(b, body, car.cofg);
    let r = crate::handling::wreck::turned(body.quat, car.cofg);
    let c = body.position;
    let current = o.v.scene.nodes[0].flags & 1 != 0;
    o.v.scene.set_position(0, c.merge(Dest::XYZ, c.sub(r))).expect("tested: no parent");
    if current && o.v.subtree {
        o.call(Call::SubtreeOutOfDate, car, body);
    }
    o.v.scene.aim(0, f, u);
    o.call(Call::SpatialUpdate, car, body);
    if o.v.flags & 0x02 != 0 {
        o.call(Call::NodeBounds, car, body);
    }
    let c = o.v.collidables;
    if c.sync {
        for p in [c.frame, c.riding].into_iter().flatten() {
            part_sync(o, p, car, body);
        }
    }
}

/// **The body's staged state made its own** (its vt `+0x3c`, the bike's
/// [`crate::handling::wreck::body_commit`]): the centre at the staged position, the velocity the
/// staged one, the accumulators cleared; the orientation the staged one, the rates the staged spin;
/// the centre moved by the centre of mass turned by it (`xyz`), the last centre the same; the moments
/// and twists cleared.
fn respawn_commit(b: &Back, body: &mut Body, cofg: Vf) {
    (body.last_position, body.position, body.velocity) = (b.staged_position, b.staged_position, b.staged_velocity);
    (body.last_force, body.force, body.accel) = (BACK_ORIGIN, BACK_ORIGIN, BACK_ORIGIN);
    (body.correction_peak, body.correction) = (BACK_ORIGIN, BACK_ORIGIN);
    (body.last_quat, body.quat, body.rates) = (b.staged_quat, b.staged_quat, b.staged_spin);
    let r = crate::handling::wreck::turned(body.quat, cofg);
    body.position = body.position.merge(Dest::XYZ, body.position.add(r));
    body.last_position = body.position;
    (body.moment, body.last_moment, body.ang_accel) = (BACK_ORIGIN, BACK_ORIGIN, BACK_ORIGIN);
    (body.twist_peak, body.twist) = (BACK_ORIGIN, BACK_ORIGIN);
}

/// **A tire's reset** (the respawn's, MD154): its contact flags, spin, slips, torque, compression,
/// load and what it stands on cleared; its contact, point and local velocities, velocity and force
/// to `0, 0, 0, 1`; its ground, car and probe normals to `0, 1, 0, 1`.
pub fn tire_reset(t: &mut Tire, s: &mut TireState) {
    (t.solid, t.sorted, t.stands_on) = (false, false, [0, 0]);
    (s.spin, s.slip_ratio, s.slip_angle, s.slip_tan) = (F::ZERO, F::ZERO, F::ZERO, F::ZERO);
    (s.torque, s.compression, t.load) = (F::ZERO, F::ZERO, F::ZERO);
    (s.contact, s.point_velocity, t.local_velocity) = (BACK_ORIGIN, BACK_ORIGIN, BACK_ORIGIN);
    (s.velocity, s.force) = (BACK_ORIGIN, BACK_ORIGIN);
    (t.ground_normal, t.car_normal, t.probe_normal) = (BACK_UP, BACK_UP, BACK_UP);
}

/// **The engine's reset** (the respawn's, MD154): its rpm and the wheels' cleared, first gear.
pub fn engine_reset(e: &mut EngineState) {
    (e.rpm, e.wheel_rpm, e.gear) = (F::ZERO, F::ZERO, 1);
}

/// A sibling list of the subtree marked out of date, as the original recurses: the rest of the list
/// first, then the node, and if it was current, its children.
fn out_of_date(below: &mut [Below], n: usize) {
    if let Some(next) = below[n].sibling {
        out_of_date(below, next);
    }
    if below[n].flags & 1 != 0 {
        below[n].flags &= !1;
        if let Some(c) = below[n].child {
            out_of_date(below, c);
        }
    }
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
pub fn wreck_timer<S: Steps>(o: &mut Own<'_, S>, car: &mut Car, body: &mut Body, dt: F) {
    if !o.v.wrecks_on {
        return;
    }
    if o.v.bounded && car.state != WRECK_EDGE {
        o.call(Call::OutOfBounds, car, body);
        if o.v.out_of_bounds {
            wreck(o, car, body, WRECK_EDGE);
        }
    }
    let v = &mut *o.v;
    if car.state == RIDING {
        // The node's own frame's up row, dotted with the rodata's (0, 1, 0).
        let up = v.scene.nodes[0].local[1];
        let level = Acc::mula(up.x(), F::ZERO).madda(up.y(), F::ONE).madd(up.z(), F::ZERO);
        if ground_speed(car) < F(STILL_SPEED)
            && v.touching
            && (car.one_side || !car.grounded)
            && level < F(f32::from_bits(TIPPED_UP))
        {
            wreck(o, car, body, WRECK_TIPPED);
            o.v.clock = o.v.max_wreck_time - F(TIPPED_HEAD_START);
            o.v.tipped_wreck = true;
        }
        return;
    }
    v.clock = v.clock + dt;
    if car.state == WRECK_EDGE {
        if !v.counting && !(v.clock <= v.max_wreck_time) {
            v.counting = true;
            v.clock = F::ZERO;
            o.call(Call::Cue(12), car, body);
        }
        if o.v.counting && (!(o.v.clock <= o.v.max_wreck_time) || ground_speed(car) < F(STILL_SPEED)) {
            body.velocity = scale_xyz(body.velocity, EDGE_KEEP);
            unwreck(o, car, body);
        }
    } else if car.state == WRECK_TIPPED
        && !v.counting
        && !(v.clock <= v.max_wreck_time)
        && ground_speed(car) < F(STILL_SPEED)
    {
        unwreck(o, car, body);
        o.v.counting = true;
        o.v.clock = F::ZERO;
    }
}

/// **The car's wreck** (vt `+0x368`): MXVehicle's — the state set, the clock stopped at 0, the
/// rumble ([`Call::Rumble`]) if it is the player's; unless the vehicle is of kind 0 or 1, both the
/// body's dampings `WreckDamping` and its inertia `WreckInertia` times its own (`xyz`); cue 11 — then
/// the car's: the body's rates spun up by [`WRECK_SPIN`], and the wrecked look on.
pub fn wreck<S: Steps>(o: &mut Own<'_, S>, car: &mut Car, body: &mut Body, state: u32) {
    car.state = state;
    o.v.clock = F::ZERO;
    o.v.counting = false;
    if o.v.rumble {
        o.call(Call::Rumble, car, body);
    }
    let v = &mut *o.v;
    if v.kind != 1 && v.kind != 0 {
        let i = v.wreck_inertia.merge(Dest::XYZ, v.wreck_inertia.mul(v.inertia));
        body.damping = v.wreck_damping;
        body.ang_damping = v.wreck_damping;
        set_inertia(v, body, i);
    }
    o.call(Call::Cue(11), car, body);
    body.rates = scale_xyz(body.rates, WRECK_SPIN);
    o.call(Call::WreckLook(true), car, body);
}

/// **The car's unwreck** (vt `+0x36c`, with 1): the car's — the latch set, the tipped wreck's flag
/// cleared, the body keeping [`UNWRECK_KEEP`] of its velocity, the wrecked look off if there is one,
/// the respawn ([`Call::Respawn`]) — then MXVehicle's: out of a map-edge wreck the world's drag
/// divided back by [`EDGE_DRAG`]; riding, the clock at 0 (counting is left as it is); unless the
/// vehicle is of kind 0 or 1, both dampings back to [`UNDAMPED`] and the body's own inertia back
/// (its `w` set to 1 where it is kept); cue 13.
pub fn unwreck<S: Steps>(o: &mut Own<'_, S>, car: &mut Car, body: &mut Body) {
    car.latch = true;
    o.v.tipped_wreck = false;
    body.velocity = scale_xyz(body.velocity, UNWRECK_KEEP);
    if o.v.look {
        o.call(Call::WreckLook(false), car, body);
    }
    o.call(Call::Respawn, car, body);
    let v = &mut *o.v;
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
    o.call(Call::Cue(13), car, body);
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

/// How the tire step asks about what a tire stands on (MD131): handed [`Call::StandsOn`] or
/// [`Call::ObjectCofg`] with the body, it makes the call and answers [`Car::stood_on`].
pub type Ask<'a> = &'a mut dyn FnMut(Call, &mut Body) -> Under;

/// `a × b` as VU0 takes it (`vopmula`, `vopmsub`); `w` from `a`.
fn cross(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, Vf::opmsub(Vf::opmula(a, b), b, a))
}

/// The contact's velocity: the body's, with its rates' spin about the centre of mass at the contact;
/// and the body's alone, in the world and in the body's frame. Each is taken against the ground's
/// (MD131): still unless the tire's stands-on words name something, which `ask` answers. A struck
/// object's ground velocity is its holder's velocity, and its point's that plus the holder's spin
/// about the object's centre of mass at the tire's ground point: a rigid body's by its quaternion
/// (the centre of mass turned into the world is written back to the object), a chassis's in its
/// transform's frame; a mover's, its velocity plus its spin about its origin, in its transform's.
fn contact_velocity(t: &mut Tire, s: &mut TireState, body: &mut Body, ask: Ask) {
    use crate::biplane::unturn;
    use crate::chassis::turn;
    let q = body.quat;
    let local = unturn(q, minus(s.contact, body.position));
    let at = plus(body.velocity, turn(q, cross(body.rates, local)));
    let (velocity, point) = if t.stands_on == [0, 0] {
        (CLEAR, CLEAR)
    } else {
        match ask(Call::StandsOn(t.stands_on), body) {
            Under::Still => (CLEAR, CLEAR),
            Under::Object { cofg, on: Holder::Body { position, velocity, quat, rates } } => {
                let cofg_world = turn(quat, cofg);
                ask(Call::ObjectCofg(t.stands_on[0], cofg_world), body);
                let arm = unturn(quat, minus(t.ground_point, plus(position, cofg_world)));
                (velocity, plus(turn(quat, cross(rates, arm)), velocity))
            }
            Under::Object { cofg, on: Holder::Chassis { cofg: centre, velocity, rates, node } } => {
                let cofg_world = node.rotate(cofg);
                ask(Call::ObjectCofg(t.stands_on[0], cofg_world), body);
                let arm = minus(node.unplace(t.ground_point), plus(centre, cofg_world));
                (velocity, plus(node.rotate(cross(rates, arm)), velocity))
            }
            Under::Mover { node, origin, spin, velocity } => {
                let arm = minus(node.unplace(t.ground_point), origin);
                (velocity, plus(node.rotate(cross(spin, arm)), velocity))
            }
        }
    };
    s.point_velocity = minus(at, point);
    s.body_velocity = minus(body.velocity, velocity);
    t.local_velocity = unturn(q, s.body_velocity);
}

/// **The suspension**: the wheel's travel from its mount down to its drop, in the world; where the
/// ground's plane cuts it, the contact and the compression. Compressed, the tire is down: bottomed
/// past its travel, the body is pushed out along the normal and its velocity into the ground taken
/// off (on something solid, or near), and the compression held at the travel; then the spring less
/// the damper (clamped rate) is the load, pushed at the contact. Not down, the rears take what the
/// fronts handed them. Last, the compression eases back no faster than the decompression rate.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's `c.ole.s` branched on false
fn suspension(k: &TireKeys, t: &mut Tire, s: &mut TireState, cofg: Vf, body: &mut Body, h: F, ask: Ask) {
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
            contact_velocity(t, s, body, ask);
            let max = k.max_compression;
            if !(s.compression <= max) {
                if t.solid || ratio < BOTTOM_RATIO {
                    let push = (s.compression - max).max(F::ZERO).min(max);
                    body.position = plus(body.position, scaled(n, push));
                    let into = dot(s.body_velocity, n).min(F::ZERO);
                    add_velocity(body, scaled(n, -into));
                }
                s.compression = max;
                contact_velocity(t, s, body, ask);
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
            contact_velocity(t, s, body, ask);
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
/// words its pointers name, with `cofg` the body's centre of mass ([`Car::cofg`]). A tire standing on
/// something asks `ask` what (MD131, [`Ask`]).
pub fn tire_step(k: &TireKeys, t: &mut Tire, s: &mut TireState, levers: &State, cofg: Vf, body: &mut Body, h: F, ask: Ask) {
    let read = |l: Option<Lever>| {
        l.map(|l| match l {
            Lever::Brake => levers.brake,
            Lever::BrakeForce => levers.brake_force,
            Lever::Steer => levers.steer,
            Lever::RearSteer => levers.rear_steer,
        })
    };
    let (brake, steer) = (read(k.brake), read(k.steer));
    suspension(k, t, s, cofg, body, h, ask);
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

// --- MD150: the car made from its files ----------------------------------------------------------

/// The spawned car's words no file key gives, as the constructor and the loaders leave them on all
/// five of MD39a's spawn images: the body's top speed and rate limits, the tires' friction scale, and
/// the flag bits the part reader does not write.
const SPAWN_MAX_SPEED: f32 = 300.0;
const SPAWN_MAX_RATE: f32 = 12.0;
const SPAWN_FRICTION_SCALE: F = F::ONE;
/// The inverse inertia's `w`, which the part reader leaves alone.
const SPAWN_INV_INERTIA_W: f32 = 1.0;
/// The flag byte's bits 1 and 2 (the post asks for the node's bounds; the post's own bit), set on the
/// spawned car; bits 0 and 6 are the part reader's.
const SPAWN_FLAGS: u8 = 0x06;

/// Each tire's brake and steer levers, as the car's tire setup hands them out: the fronts the brake
/// and the steer, the rears the brake force and the rear steer.
pub const TIRE_LEVERS: [(Lever, Lever); 4] = [
    (Lever::Brake, Lever::Steer),
    (Lever::Brake, Lever::Steer),
    (Lever::BrakeForce, Lever::RearSteer),
    (Lever::BrakeForce, Lever::RearSteer),
];

/// **MX vs. ATV's car made from its files** (MD150), with no RAM image: a `Veh\<car>.phy` as
/// SLUS-21104 loads it ([`crate::car::Car::read_mxvatv`], MD39b), its `[Mass Properties]` again
/// through the rigid-body part reader ([`crate::handling::mass::Part`], MXVehicle's base loader), and
/// what the car's setup takes from the chassis model: the part reader's half box and the four tire
/// nodes' points ([`tire_points`]), handed in as for the quad ([`crate::quad::Load::read`]).
///
/// It builds what the ported frame reads ([`Keys`], [`InputKeys`], [`Car`], [`Body`], [`Vehicle`]) as
/// the car stands after its load: riding, still, no wreck clock, with no pose. Where the car stands,
/// its scene node and the world's bounds are the ride's to set.
#[derive(Clone, Debug, PartialEq)]
pub struct Load {
    pub file: crate::car::MxvatvCar,
    pub part: crate::handling::mass::Part,
    /// The tire nodes' points in the car's frame (before `TireWidth`), in [`FRONT_LEFT`]..[`REAR_RIGHT`]
    /// order.
    pub points: [Vf; 4],
    /// Driven by MX vs. ATV Untamed's (SLUS-21701) car step (MD81d): the keys' preload cue, the
    /// vehicle's force and the update's hazard clock are Untamed's ([`Keys::untamed`],
    /// [`Vehicle::untamed`], [`Update::untamed`]), and so is the scene's node update (MD145,
    /// [`crate::atv_moto::Scene::untamed`]); with it the ride's frame is [`frame_untamed`] (MD161:
    /// the body's per-time angular damping, the PostProcess's yaw hold by compares). Off (as read),
    /// SLUS-21104's.
    pub untamed: bool,
}

impl Load {
    /// Read a parsed file for one of the five cars.
    pub fn read(ini: &crate::phy::Ini, machine: crate::car::MxvatvMachine, half_box: Vf, points: [Vf; 4], curve: &crate::camera::CurveTable) -> Load {
        let part = crate::handling::mass::Part::read(&ini.section("Mass Properties"), half_box, curve);
        Load { file: crate::car::Car::read_mxvatv(ini, machine), part, points, untamed: false }
    }

    /// Read the file's bytes.
    pub fn read_bytes(bytes: &[u8], machine: crate::car::MxvatvMachine, half_box: Vf, points: [Vf; 4], curve: &crate::camera::CurveTable) -> Load {
        Load::read(&crate::phy::Ini::parse_bytes(bytes), machine, half_box, points, curve)
    }

    /// Each tire's mount: its node's point slid out along `x` by `TireWidth`, `w` 1.
    pub fn mounts(&self) -> [Vf; 4] {
        core::array::from_fn(|k| {
            let p = self.points[k];
            Vf::new((F(p.x()) + self.file.car.tires[k].width).0, p.y(), p.z(), 1.0)
        })
    }

    /// The pieces' keys: `[Joystick]`, `[Steering]`, the centres of mass and travel, `[Preload]`, the
    /// clutch's launch, `[Aerodynamics]` and its wings, each tire's and the engine's.
    ///
    /// The keys read with the leave-alone getter (`[Preload]`, the clutch's) fall back to zero, which
    /// no retail file needs. An undriven tire's top speed is MX Unleashed's constant
    /// ([`crate::car::UNDRIVEN_TOP_WHEEL_SPEED`]); each tire has sixteen surfaces, the ones past the
    /// file's zero.
    pub fn keys(&self) -> Keys {
        let c = &self.file.car;
        let (j, a, p) = (&c.joystick, &c.aero, &c.preload);
        let or0 = |v: Option<F>| v.unwrap_or(F::ZERO);
        Keys {
            square_steering: j.square_steering_input != 0,
            brake_rate_limit: j.brake_rate_limit,
            gas_rate_limit: j.gas_rate_limit,
            x_dpad_rate_limit_on: j.x_dpad_rate_limit_on,
            x_dpad_rate_limit_off: j.x_dpad_rate_limit_off,
            x_axis_rate_limit: j.x_axis_rate_limit,
            steer: j.steer,
            wheel_steer: j.wheel_steer,
            dpad_steer: j.dpad_steer,
            rear_steer: j.rear_steer,
            yaw_rate_limit: c.yaw_rate_limit,
            hand_brake_yaw_rate_limit: c.hand_brake_yaw_rate_limit,
            hand_brake_friction_scale: c.hand_brake_friction_scale,
            hand_brake_brake_scale: c.hand_brake_brake_scale,
            hand_brake_fade_time: c.hand_brake_fade_time,
            cofg: c.mass_properties.cofg,
            cofg_ai: c.cofg_ai,
            cofg_travel_forward: c.cofg_travel_forward,
            cofg_travel_back: c.cofg_travel_back,
            preload_time: or0(p.preload_time),
            min_preload_time: or0(p.min_preload_time),
            max_preload_time: or0(p.max_preload_time),
            max_preload: or0(p.max_preload),
            clutch_torque_scale_time: or0(c.clutch_torque_scale_time),
            max_clutch_torque_scale: or0(c.max_clutch_torque_scale),
            max_clutch_torque_scale_speed: or0(c.max_clutch_torque_scale_speed),
            aero_surface_area: a.aero_surface_area,
            airborne_damping: a.airborne_damping,
            unroll_gain: a.unroll_gain,
            unroll_torque: a.unroll_torque,
            unyaw_gain: a.unyaw_gain,
            unyaw_torque: a.unyaw_torque,
            airborne_control_moment: a.airborne_control_moment,
            brake_gyro_torque: a.brake_gyro_torque,
            wings: if a.number_of_wings > 0 {
                a.wings
                    .iter()
                    .map(|w| Wing { lift_slope: w.lift_slope, angle_of_attack: w.angle_of_attack, basic_drag: w.basic_drag, induced_drag: w.induced_drag, position: w.position })
                    .collect()
            } else {
                Vec::new()
            },
            tires: core::array::from_fn(|k| self.tire_keys(k)),
            engine: self.engine_keys(),
            untamed: self.untamed,
        }
    }

    /// One tire's keys, with its levers ([`TIRE_LEVERS`]).
    pub fn tire_keys(&self, k: usize) -> TireKeys {
        let c = &self.file.car;
        let t = &c.tires[k];
        let mut surfaces = [crate::car::Surface::default(); 16];
        for (s, f) in surfaces.iter_mut().zip(&t.surfaces) {
            *s = *f;
        }
        TireKeys {
            brake: Some(TIRE_LEVERS[k].0),
            steer: Some(TIRE_LEVERS[k].1),
            max_compression: t.max_compression,
            max_compression_rate: t.max_compression_rate,
            max_decompression_rate: t.max_decompression_rate,
            spring: t.spring,
            bounce_damping: t.bounce_damping,
            rebound_damping: t.rebound_damping,
            inv_inertia: t.inv_inertia,
            max_steer_angle: t.max_steer_angle,
            top_wheel_speed: c.top_wheel_speed(k).unwrap_or(crate::car::UNDRIVEN_TOP_WHEEL_SPEED),
            max_tire_angle_rate: t.max_tire_angle_rate,
            abs: t.abs_enabled != 0,
            min_braking_slip: t.min_braking_slip,
            max_braking_slip: t.max_braking_slip,
            friction_scale: SPAWN_FRICTION_SCALE,
            camber_roll_ratio: t.camber_roll_ratio,
            surfaces,
        }
    }

    /// The engine's keys, with its sound. `None` when the file drives no tire, which no retail file
    /// does. A missing inverse inertia or RPM table reads as zero.
    pub fn engine_keys(&self) -> Option<EngineKeys> {
        let e = &self.file.car.engine;
        let [Some(a), Some(b), c, d] = e.driven else { return None };
        Some(EngineKeys {
            drive: [a, b],
            drive_more: c.zip(d).map(|(c, d)| [c, d]),
            rpm: e.rpm.clone(),
            torque: e.torque.clone(),
            redline: e.max_rpm.unwrap_or(F::ZERO),
            gear_ratios: e.gear_ratios.clone(),
            final_gear_ratio: e.final_gear_ratio,
            idle_throttle: e.idle_throttle,
            engine_braking: e.engine_braking,
            transmission_loss: e.transmission_loss,
            inv_inertia: e.inv_inertia.unwrap_or(F::ZERO),
            min_rpm: e.min_rpm,
            pump_torque_scale: e.pump_torque_scale,
            max_pump_torque: e.max_pump_torque,
            max_k: e.max_k,
            tcs: e.tcs_enabled != 0,
            min_traction_slip: e.min_traction_slip,
            max_traction_slip: e.max_traction_slip,
            nox_burn_rate: e.nox_burn_rate,
            nox_hp_boost: e.nox_hp_boost,
            shift: e.shift,
            sound: true,
        })
    }

    /// The input step's `[Joystick]` keys.
    pub fn input_keys(&self) -> InputKeys {
        let j = &self.file.car.joystick;
        InputKeys { max_analog_throttle: j.max_analog_throttle, digital_throttle_speed: j.digital_throttle_speed, digital_side_slip_angle: j.digital_side_slip_angle }
    }

    /// The car as loaded: riding, its yaw limit `YawRateLimit`, a landing sound and sort rays, the
    /// body's centre of mass, each tire's mount ([`Load::mounts`]) and radius; nothing measured yet.
    pub fn car(&self) -> Car {
        let mounts = self.mounts();
        let mut car = Car {
            state: RIDING,
            yaw_rate_limit: self.file.car.yaw_rate_limit,
            landing_sound: true,
            cofg: self.part.body.cofg,
            sort_rays: true,
            ..Car::default()
        };
        for (k, t) in car.tires.iter_mut().enumerate() {
            (t.mount, t.radius) = (mounts[k], self.file.car.tires[k].radius);
        }
        car
    }

    /// The rigid body as loaded: enabled on every axis, the part's mass, damping and inertia, the top
    /// speed and rates the constructor gives, no acceleration limits, and no pose (all zero).
    pub fn body(&self) -> Body {
        let b = &self.part.body;
        let [ix, iy, iz] = b.inv_inertia;
        Body {
            enabled: true,
            linear_axes: [true; 3],
            inv_mass: b.inv_mass,
            damping: b.damping,
            max_speed: F(SPAWN_MAX_SPEED),
            max_accel: F(f32::MAX),
            angular_axes: [true; 3],
            inv_inertia: Vf::new(ix.0, iy.0, iz.0, SPAWN_INV_INERTIA_W),
            ang_damping: b.angular_damping,
            max_rates: [F(SPAWN_MAX_RATE); 3],
            max_ang_accel: F(f32::MAX),
            ..Body::default()
        }
    }

    /// MXVehicle's words as loaded: wrecks on, held to the map, no wreck clock, the car's kind word,
    /// `[Wrecks]`, the part's inertia (the body's and the one the wreck scales), a wrecked look and a
    /// collision record, the world's gravity force, and the flag byte. `drag` is the world's own
    /// word. The scene and the bounds are the ride's.
    pub fn vehicle(&self, drag: F) -> Vehicle {
        let (w, f) = (&self.file.wrecks, self.part.flags);
        Vehicle {
            wrecks_on: true,
            bounded: true,
            max_wreck_time: self.file.car.max_wreck_time,
            kind: self.file.machine.kind_word(),
            wreck_damping: w.wreck_damping,
            wreck_inertia: w.wreck_inertia,
            inertia: self.part.body.inertia,
            body_inertia: self.part.body.inertia,
            look: true,
            record: true,
            gravity: self.part.gravity.force,
            drag,
            flags: SPAWN_FLAGS | u8::from(f.reset_off_edge) | (u8::from(f.reset_below_minimum) << 6),
            untamed: self.untamed,
            scene: crate::atv_moto::Scene { untamed: self.untamed, ..crate::atv_moto::Scene::default() },
            ..Vehicle::default()
        }
    }

    /// **The car's collision record as loaded** (MD149b), on `riding` (its `<car>m.col`, a ray
    /// probe): what the part reader writes ([`crate::handling::mass::Part::words`], laid at scratch
    /// places and read back as the record and its resolution's fields) over the record's constructor,
    /// as each car's image holds it (MD39a): the world rays, the rays and the sweep on as
    /// `CheckForCollisions` and `CheckForHighSpeedCollisions` say, rests and damping of one, a body,
    /// the terrain, the near-field list, one contact a riding ray, the two probes, and the car's
    /// class word [`RECORD_CLASS`]. The heightfield rays are off: the frame turns them on with the
    /// car tipped ([`Car::tipped`]). The held list has no body (the car's resolution reads the ride's).
    pub fn record(&self, riding: &crate::track::Rays) -> (crate::handling::record::Record, crate::atv_contact::Held, Vec<[Vf; 2]>) {
        use crate::handling::record;
        let riding = crate::flight_search::Rays::of(riding).pairs();
        let r = &self.part.record;
        let mut words = std::collections::BTreeMap::new();
        for (a, mask, v) in self.part.words(RECORD_AT) {
            let w = words.entry(a).or_insert(0u32);
            *w = (*w & !mask) | (v & mask);
        }
        let at = RECORD_AT.record;
        let one = F::ONE.to_bits();
        for (o, v) in [
            (0x28, 1),
            (0x2c, u32::from(r.check_for_collisions != 0)),
            (0x30, u32::from(r.check_high_speed != 0)),
            (0x48, one),
            (0x4c, one),
            (0x50, one),
            (0x54, one),
            (0xe4, RECORD_AT.body),
            (0xec, 1),
            (0xf4, riding.len() as u32),
            (0xfc, 1),
            (0x100, RECORD_ACTORS),
            (0x104, RECORD_COLLIDES),
            (0x200, 1),
            (0x204, 1),
            (0x208, RECORD_CLASS_AT),
        ] {
            words.insert(at + o, v);
        }
        // The `w` lanes of the centre of mass in the world and the relative velocity.
        for o in [0x9c, 0x1fc] {
            words.insert(at + o, one);
        }
        words.insert(RECORD_CLASS_AT + 0x94, RECORD_CLASS);
        let w = |a: u32| words.get(&a).copied().unwrap_or(0);
        let held = record::held(&w, at).expect("a record with no contacts is held");
        (record::read(&w, at), held, riding)
    }
}

/// Where [`Load::record`] lays the part reader's words to read the collision record back: scratch
/// places, nobody's RAM.
const RECORD_AT: crate::handling::mass::Layout = crate::handling::mass::Layout { object: 0x1000, body: 0x2000, record: 0x3000, gravity: 0x4000 };
const RECORD_CLASS_AT: u32 = 0x5000;
/// The record's constructor words no `.phy` key writes, as each car's image holds them (MD39a's five):
/// the actor bit it is, what it collides with (everything), and its class word (the record's `+0x208`
/// object's `+0x94`).
const RECORD_ACTORS: u32 = 0x1000;
const RECORD_COLLIDES: u32 = !0;
pub const RECORD_CLASS: u32 = 0x0004_3806;

/// **The car's tire nodes' points** off its chassis model (`<car>.psg`) placed at `frame`: each
/// [`crate::car::TIRE_NODES`] node's world origin carried back into `frame`, as the quad's wheel
/// setup does it ([`crate::quad::Model::mounts`]), `w` 1. The frame the car's setup sees is its
/// placement at that time, which a spawn image does not keep (research/journal MD150).
pub fn tire_points(model: &crate::quad::Model, frame: &crate::math::Mat4) -> Result<[Vf; 4], String> {
    use ps2_float::Field;
    let worlds = model.worlds(frame);
    let [r0, r1, r2, r3] = frame.0;
    let column = |k: usize| Vf::new(r0[k], r1[k], r2[k], 0.0);
    let mut out = [Vf::ZERO; 4];
    for (slot, name) in out.iter_mut().zip(crate::car::TIRE_NODES) {
        let i = model.nodes.iter().position(|n| n.name == name).ok_or_else(|| format!("the model has no {name}"))?;
        let d = worlds[i].0[3].sub(r3);
        let acc = column(0).mul(d.bc(Field::X));
        let acc = Vf::madd(acc, column(1), d.bc(Field::Y));
        let p = Vf::madd(acc, column(2), d.bc(Field::Z));
        *slot = Vf::new(p.x(), p.y(), p.z(), 1.0);
    }
    Ok(out)
}

// --- MD149: the car on MX Unleashed's worlds -------------------------------------------------------

/// The car's physics world (SLUS-21104's: one iteration a substep, 115 substeps a second).
pub const WORLD: crate::biplane::World = crate::biplane::World { iterations: 1, rate: F(115.0) };

/// The quadword a fresh tire's normals stand on (`w` 0; the respawn's [`tire_reset`] puts `w` 1).
const TIRE_UP: Vf = Vf([0.0, 1.0, 0.0, 0.0]);

/// The probe the put-back spot is kept from, in the car node's frame (the quad's).
const BACK_PROBE: Vf = Vf([0.0, 3.0, 0.0, 1.0]);
/// How far over the ground the put-back stands the car's lowest tire bottom, ft.
const BACK_CLEAR: f32 = 1.0;
/// How far under the spot it is put down at the world's floor lies, ft: an MX Unleashed world has no
/// floor word, so below this the car is out of the world (bit 6 of [`Vehicle::flags`]).
const FLOOR_BELOW: f32 = 1000.0;

impl Load {
    /// **The pieces' running words as the car is put in the world** (MD149), as MD39a's five spawn
    /// images hold them: driven by a person (the vehicle state's mode 1, a pad, no wheel), not held,
    /// with the gas and both-pedal reverse let through; the vehicle's gravity; and the engine in first
    /// ([`EngineKeys`]' first gear times the final drive), its revs and wheel revs at 0 (the
    /// respawn's engine reset), a full tank of nitrous (`NOXVolume`) and its torque curve's cursor on
    /// its second point. Everything else is 0.
    pub fn state(&self) -> State {
        let mut s = State {
            human: true,
            driven: true,
            reverse_enabled: true,
            throttle_enabled: true,
            gravity: self.part.gravity.force,
            ..State::default()
        };
        if let Some(e) = self.engine_keys() {
            s.engine.gear = 1;
            s.engine.ratio = e.final_gear_ratio * e.gear_ratios[0];
            s.engine.inv_ratio = F::ONE / s.engine.ratio;
        }
        (s.engine.nox_left, s.engine.cursor_index) = (self.file.car.engine.nox_volume, 1);
        s.rear_friction = [SPAWN_FRICTION_SCALE; 2];
        s
    }

    /// **The car as a shell steps it** (MD149), stood with its node at `at` (`w` 1) turned to the
    /// quaternion `quat`, still, on `world` (its drag word `drag`; [`Ride::new`]).
    pub fn ride(&self, at: Vf, quat: Vf, drag: F) -> Ride {
        let mut car = self.car();
        for t in &mut car.tires {
            // Every tire has the world's ground to ask, and stands on its up until it has asked.
            t.on_ground = true;
            (t.probe_normal, t.ground_normal, t.car_normal) = (TIRE_UP, TIRE_UP, TIRE_UP);
        }
        let mut r = Ride {
            keys: self.keys(),
            state: self.state(),
            input_keys: self.input_keys(),
            input: Input::default(),
            car,
            body: self.body(),
            vehicle: self.vehicle(drag),
            update: Update { vehicle_state: Some(MODE_RIDING), untamed: self.untamed, ..Update::default() },
            updated: Updated::default(),
            table: AcosTable::build(),
            noise: crate::noise::SmoothNoise::build(&mut crate::rng::Rng::default()),
            back: None,
            cache: None,
            spot: (at, quat),
            recorded: None,
            rest: Vec::new(),
        };
        // The vehicle base is a player's: a vehicle state, a player, no rumbler.
        (r.updated.vehicle.state, r.updated.vehicle.player) = (1, 1);
        r.vehicle.bounds.floor = F(at.y()) - F(FLOOR_BELOW);
        r.put(at, quat);
        r
    }
}

/// **MX vs. ATV's car as a shell steps it on an MX Unleashed world** (MD149): the pieces' keys and
/// running words, the input step's, the car, its rigid body, MXVehicle's words, the update's, and
/// whoever answers what nobody here does (`rest`, a list its owner clears), one [`Ride::step`] a
/// tick: the pad through the car's input step ([`input`]), the collision record's pass when the car
/// has one ([`Ride::record_on`], MD149b), [`frame`] with [`Own`] over [`Pieces`] over [`OnWorld`],
/// then [`class_update`] the same way, then the put-back spot kept.
///
/// What the frame and the update call, and who answers on an MX Unleashed world:
///
/// - the tires' ground ([`Call::TireGround`]) and the ground under the body ([`Call::GroundUnder`]):
///   [`crate::world::World::ground`] (and the wheel's surface, [`crate::quad::world_ground`]);
/// - the shared smooth noise ([`Call::Noise`]): [`crate::noise::SmoothNoise`] built as the original
///   builds it, off a fresh generator;
/// - the contact sort's search ([`Call::SortSearch`]): nothing struck. MX Unleashed's props are not
///   SLUS-21104 objects, so the sort ray probe has nothing to strike; the tires find the ground by
///   their probes alone. [`Call::SortPlace`] goes to `rest`;
/// - the node's position, the map-edge test and throw, the body's reset, the spatial update, the
///   node's bounds and the collision sync: [`Own`], on [`Vehicle`] with the world's box
///   ([`crate::quad::world_bounds`]) and map area ([`crate::world::World::area`]);
/// - the wreck and the unwreck: [`Own`]'s [`wreck_timer`]; the respawn ([`Call::Respawn`]) and the
///   hazard clock's put-back ([`Call::PutBack`]): [`Ride`]'s put-back, which stands the car at the
///   spot MX Unleashed's put-back record keeps ([`crate::ride::keep_back_on`]), still and enabled;
///   the respawn also runs the car's tire and engine resets;
/// - the ghost, the hazard clock and the base update: [`OwnUpdate`] (no ghost runs; the hazard
///   table is empty, so the clock frees the car each frame);
/// - the collision record's gravity call ([`Call::GravityRecord`]), with the car rigged
///   ([`Ride::rig`]): [`Own`], resolving the contacts [`Ride::record_on`] filed against the world.
///
/// Dropped (`rest`): [`Call::GravityRecord`] on a car not rigged (no record runs:
/// [`Vehicle::touching`] stays false, so the tipped wreck never starts), [`Call::SpatialMove`],
/// [`Call::PartMove`], [`Call::Gather`] (SLUS-21104's spatial database), [`Call::LeaveWorld`]
/// (its physics lists), [`Call::RidingLetGo`] and [`Call::ReleaseRecord`] (the record), the sound
/// cues, the landing and shift sounds, the preload's cue, the rumbles, the wrecked look,
/// [`Call::SortPlace`], [`Call::CollisionOn`], [`Call::Unlist`], [`Call::Place`], the class's own
/// step ([`Call::ClassStep`]: posing the model's parts, ported as [`class_step`] over scene nodes,
/// which a ride has none of), and the base update's prepare, ride, sync, snapshot, network and
/// parts.
#[derive(Clone)]
pub struct Ride {
    pub keys: Keys,
    pub state: State,
    pub input_keys: InputKeys,
    pub input: Input,
    pub car: Car,
    pub body: Body,
    pub vehicle: Vehicle,
    pub update: Update,
    pub updated: Updated,
    pub table: AcosTable,
    pub noise: crate::noise::SmoothNoise,
    /// MX Unleashed's put-back record, and its search's cache.
    pub back: Option<crate::ride::Back>,
    pub cache: Option<Box<crate::back_search::Cache>>,
    /// Where the put-back stands the car's node, and its turn.
    pub spot: (Vf, Vf),
    /// MD149b's: the collision record run before each frame ([`Ride::record_on`]); `None`, none runs.
    pub recorded: Option<Recorded>,
    pub rest: Vec<Call>,
}

/// **The car's collision record on an MX Unleashed world** (MD149b), as [`Ride`] keeps it: the
/// record ([`crate::handling::record::Record`]) and its riding rays (the `<car>m.col` ray probe, in
/// the scene node's frame). Its resolution's fields and list are [`Vehicle::held`], where the
/// gravity's call reads them.
#[derive(Clone, Debug)]
pub struct Recorded {
    pub record: crate::handling::record::Record,
    pub riding: Vec<[Vf; 2]>,
}

/// The curve table the put-back record's search reads, built once.
fn back_curve() -> &'static crate::camera::CurveTable {
    static CURVE: std::sync::OnceLock<crate::camera::CurveTable> = std::sync::OnceLock::new();
    CURVE.get_or_init(crate::camera::CurveTable::build)
}

impl Ride {
    /// The car's node stood at `at` turned to `quat`: the body there (its centre of mass turned
    /// out from the node), still, enabled, with its last pose the same; the scene node placed.
    pub fn put(&mut self, at: Vf, quat: Vf) {
        let b = &mut self.body;
        let at = at.merge(Dest::W, Vf::new(0.0, 0.0, 0.0, 1.0));
        b.position = plus(at, crate::chassis::turn(quat, self.car.cofg));
        b.quat = quat;
        (b.last_position, b.last_quat) = (b.position, b.quat);
        (b.velocity, b.rates, b.force, b.moment) = (Vf::ZERO, Vf::ZERO, Vf::ZERO, Vf::ZERO);
        (b.last_force, b.last_moment, b.accel, b.ang_accel) = (Vf::ZERO, Vf::ZERO, Vf::ZERO, Vf::ZERO);
        b.enabled = true;
        self.vehicle.scene.place(0, quat, at);
    }

    /// The car's input step on `pad` through `table` (the ride's bindings), the stick read through
    /// `stick`'s range, and its words laid where the controls read them.
    pub fn pad(&mut self, pad: &crate::input::Pad, stick: &crate::input::AxisRange, table: &crate::bindings::Table) {
        use crate::input::Axis;
        let s = (pad.axis_through(Axis::LeftX, stick), pad.axis_through(Axis::LeftY, stick));
        let (speed, side_slip) = (self.car.speed, self.car.side_slip);
        if self.update.untamed {
            input_untamed(&self.input_keys, &mut self.input, speed, side_slip, s, F::ZERO, |c| table.value(pad, c));
        } else {
            input(&self.input_keys, &mut self.input, speed, side_slip, s, F::ZERO, |c| table.value(pad, c));
        }
        self.controls();
    }

    /// The input step's words laid into the pieces' state, as the controls read them.
    pub fn controls(&mut self) {
        let (p, s) = (&self.input, &mut self.state);
        (s.stick, s.lean, s.pad_word, s.clutch) = (p.stick, p.lean, p.pad_word, p.clutch);
        (s.gas_pedal, s.vehicle_brake, s.hand_brake, s.dpad) = (p.gas_pedal, p.vehicle_brake, p.hand_brake, p.dpad);
    }

    /// One tick as the shell steps it: the pad's input step ([`Ride::pad`]), then [`Ride::frame_on`].
    pub fn step(&mut self, pad: &crate::input::Pad, table: &crate::bindings::Table, dt: F, world: &mut dyn crate::world::World) {
        self.pad(pad, &crate::input::AxisRange::PLAYER, table);
        self.frame_on(dt, world);
    }

    /// **The car's collision record rigged** (MD149b) on `riding`, its `<car>m.col`
    /// ([`Load::record`]): the record kept in [`Ride::recorded`], its resolution's fields and list in
    /// [`Vehicle::held`] (with the ride's body).
    pub fn rig(&mut self, load: &Load, riding: &crate::track::Rays) {
        let (record, mut held, riding) = load.record(riding);
        held.body = Some(self.body);
        self.vehicle.held = Some(held);
        self.recorded = Some(Recorded { record, riding });
    }

    /// **The collision record's clear and pass** (MD149b, [`crate::handling::record::clear`] and
    /// [`crate::handling::record::pass`]), before the frame, as the world runs its records before its
    /// step: on [`crate::quad::RecordOn`] (the riding rays stood at the car's node and swept against
    /// [`crate::world::World::solid`] and [`crate::world::World::groups`], the heightfield rays against
    /// [`crate::world::World::ground`]), with the heightfield rays on as the last frame left
    /// [`Car::tipped`]. The pass's contacts refill [`Vehicle::held`] (none is ever with another record
    /// or a mover on an MX Unleashed world), and its touch is [`Vehicle::touching`] and [`State::touching`].
    pub fn record_on(&mut self, dt: F, world: &mut dyn crate::world::World) {
        let Some(r) = self.recorded.as_mut() else { return };
        r.record.heightfield = self.car.tipped;
        let mut on = crate::quad::RecordOn { world, scene: &mut self.vehicle.scene, rays: &r.riding, damped: None, category: 0, nobody: Default::default() };
        crate::handling::record::clear(&mut r.record, &mut on);
        let mut view = self.body.view();
        let body = if r.record.body { Some(&mut view) } else { None };
        // `me` names the record to another one's contact: there is no other on an MX Unleashed world.
        crate::handling::record::pass(&mut r.record, 1, body, dt, &mut on);
        if let Some((linear, angular)) = on.damped {
            (self.body.damping, self.body.ang_damping) = (linear, angular);
        }
        self.body.set_view(&view);
        if let Some(h) = self.vehicle.held.as_mut() {
            h.refill(&r.record.contacts);
        }
        // The original reads the record's touching word in both: the vehicle's tipped wreck and the
        // pieces' air control.
        (self.vehicle.touching, self.state.touching) = (r.record.touching, r.record.touching);
    }

    /// One tick on an MX Unleashed world, its controls laid already: the collision record's pass
    /// ([`Ride::record_on`]), the world's box and map area into [`Vehicle`], the car's [`frame`], its
    /// [`class_update`], then the put-back spot kept ([`Ride::keep_back`]).
    pub fn frame_on(&mut self, dt: F, world: &mut dyn crate::world::World) {
        self.record_on(dt, world);
        let v = &mut self.vehicle;
        v.bounds = crate::quad::world_bounds(world, v.bounds.floor);
        v.area = world.area();
        let Ride { keys, state, car, body, vehicle, update, updated, table, noise, spot, rest, .. } = self;
        let on = OnWorld { world: &mut *world, noise, rest };
        chain(Chain { keys, state, car, body, vehicle, update, updated, table, spot: *spot }, dt, on, true);
        self.keep_back(world);
    }

    /// **The car's [`frame`] alone through the Ride's own chain** (MD149a): [`Own`] over the
    /// put-back over [`Pieces`] (the keys and state taken out and put back), as [`Ride::frame_on`]
    /// steps it, with `rest` answering in place of [`OnWorld`] what the pieces do not; `rest` back.
    /// No world box or area is laid, no [`class_update`] runs and no put-back spot is kept.
    pub fn frame_with<R: Steps>(&mut self, dt: F, rest: R) -> R {
        let Ride { keys, state, car, body, vehicle, update, updated, table, spot, .. } = self;
        chain(Chain { keys, state, car, body, vehicle, update, updated, table, spot: *spot }, dt, rest, false)
    }

    /// **The put-back spot kept** (MX Unleashed's put-back record, [`crate::ride::keep_back_on`]),
    /// while the car rides with its body in the world: probed from [`BACK_PROBE`] in the node's frame
    /// at the body's height, facing the node's forward row. A spot kept stands the node over it, on
    /// its ground's up, high enough that the lowest tire bottom clears the ground by [`BACK_CLEAR`].
    pub fn keep_back(&mut self, world: &mut dyn crate::world::World) {
        if self.car.state != RIDING || !self.body.enabled {
            return;
        }
        let v = &mut self.vehicle;
        v.scene.update(0);
        let frame = v.scene.nodes[0].world;
        let probe = crate::math::transform_point(&crate::math::Mat4(frame), BACK_PROBE);
        let probe = Vf::new(probe.x(), self.body.position.y(), probe.z(), probe.w());
        let mut back = self.back.unwrap_or_default();
        if !crate::ride::keep_back_on(&mut back, &mut self.cache, probe, frame[2], world, back_curve()) {
            return;
        }
        self.back = Some(back);
        let m = crate::math::basis(back.at, back.facing, back.up, true, false);
        let lift = self.car.tires.iter().map(|t| t.radius - F(t.mount.y())).fold(F::ZERO, F::max) + F(BACK_CLEAR);
        let up = m.0[1];
        let node = Vf::new(back.at.x(), back.at.y(), back.at.z(), 1.0);
        self.spot = (node.merge(Dest::XYZ, node.add(scaled(up, lift))), crate::math::mat4_to_quat(&m));
    }

    /// The car is down: wrecked ([`Car::state`] not [`RIDING`]).
    pub fn down(&self) -> bool {
        self.car.state != RIDING
    }
}

/// **The car's calls answered by an MX Unleashed world** (MD149, [`Ride`]): the tires' ground and the
/// ground under the body, the smooth noise, and the sort's search (nothing struck); every other call
/// goes on to `rest`.
pub struct OnWorld<'a, 'w> {
    pub world: &'a mut (dyn crate::world::World + 'w),
    pub noise: &'a crate::noise::SmoothNoise,
    pub rest: &'a mut Vec<Call>,
}

impl Steps for OnWorld<'_, '_> {
    fn call(&mut self, c: Call, car: &mut Car, _body: &mut Body) {
        match c {
            Call::TireGround(k, at) => {
                let t = &mut car.tires[k];
                let mut surface = t.surface;
                t.query = crate::quad::world_ground(self.world, at, &mut surface);
                t.surface = surface;
            }
            Call::GroundUnder(at) => car.ground_normal = self.world.ground(at).1,
            Call::Noise(t) => car.noise = self.noise.at_accumulated(t),
            Call::SortSearch => car.sort = SortAnswer::default(),
            c => self.rest.push(c),
        }
    }
}

/// What [`Ride`]'s chain steps, borrowed apart from the world's answers.
struct Chain<'a> {
    keys: &'a mut Keys,
    state: &'a mut State,
    car: &'a mut Car,
    body: &'a mut Body,
    vehicle: &'a mut Vehicle,
    update: &'a mut Update,
    updated: &'a mut Updated,
    table: &'a AcosTable,
    spot: (Vf, Vf),
}

/// [`Ride`]'s chain: [`Own`] over the put-back over [`Pieces`] over `rest`, the keys and state taken
/// out and put back; the car's [`frame`], then with `with_update` its [`class_update`] the same way.
fn chain<R: Steps>(c: Chain<'_>, dt: F, rest: R, with_update: bool) -> R {
    let mut pieces = Pieces { keys: std::mem::take(c.keys), state: *c.state, rest };
    {
        let mut back = PutBack { pieces: &mut pieces, spot: c.spot };
        let mut own = Own { v: &mut *c.vehicle, rest: &mut back };
        if c.update.untamed {
            frame_untamed(&WORLD, c.car, c.body, dt, c.table, &mut own);
        } else {
            frame(&WORLD, c.car, c.body, dt, c.table, &mut own);
        }
        if with_update {
            // The update reads the tires' surfaces and the node's axes as the frame left them.
            c.update.tire_surface = c.car.tires.map(|t| t.surface & 15);
            let node = own.v.scene.nodes[0].world;
            (c.updated.vehicle.side, c.updated.vehicle.ahead) = (node[0], node[2]);
            class_update(c.update, c.updated, c.car, c.body, dt, &mut own);
        }
    }
    (*c.keys, *c.state) = (pieces.keys, pieces.state);
    pieces.rest
}

/// The respawn and the put-back answered for [`Ride`] in front of the pieces (whose tire and engine
/// words the respawn resets); every other call goes on to them.
struct PutBack<'a, R> {
    pieces: &'a mut Pieces<R>,
    spot: (Vf, Vf),
}

impl<R: Steps> Steps for PutBack<'_, R> {
    fn call(&mut self, c: Call, car: &mut Car, body: &mut Body) {
        match c {
            Call::Respawn => {
                put_back(self.spot, car, body);
                respawn_reset(car, &mut self.pieces.state);
            }
            Call::PutBack => put_back(self.spot, car, body),
            c => self.pieces.call(c, car, body),
        }
    }
}

/// The car's put-back on an MX Unleashed world: its body stood still at `spot` (the node's place and
/// turn, the centre of mass turned out from it), enabled.
fn put_back(spot: (Vf, Vf), car: &Car, body: &mut Body) {
    let (at, quat) = spot;
    body.position = plus(at, crate::chassis::turn(quat, car.cofg));
    body.quat = quat;
    (body.velocity, body.rates, body.force, body.moment) = (Vf::ZERO, Vf::ZERO, Vf::ZERO, Vf::ZERO);
    body.enabled = true;
}

/// **The car's respawn resets** (its respawn's, after the put-back, when it has all four tires and
/// its engine): each tire's contact forgotten (solid, sorted, what it stands on), its spin, slips,
/// torque, compression and load at 0, its contact, point and local velocities, velocity and force
/// cleared, its ground, car and probe normals stood up; the engine's revs and wheel revs at 0 and
/// its gear word 1 (the ratio is left as it was): [`tire_reset`] and [`engine_reset`], so the
/// vectors' `w` is 1 as SLUS-21104 puts it (MD180).
fn respawn_reset(car: &mut Car, s: &mut State) {
    for (t, ts) in car.tires.iter_mut().zip(&mut s.tires) {
        tire_reset(t, ts);
    }
    engine_reset(&mut s.engine);
}
