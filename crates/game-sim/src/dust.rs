//! **The bike's particle emitters' draws** (S328c): how many times each rider's emitters draw on the
//! game's one generator in a frame, and the Sim's own run of them.
//!
//! The particles are presentation, but their emitters are not, in one thing: every emitter that
//! places particles in a frame makes two `rand()`s on the shared generator first (the fraction of the
//! frame's rate it carries, and the pattern). The particles themselves draw on another generator. C33's
//! four racers show it: from tick 17 each moving rider's exhaust, rear-tyre dust and rear sprays make
//! 4-26 draws a tick, each rider's straight after its own AI's. The shell's particles
//! (`openmx/src/particles.rs`) run on dice of their own; the count lives here so the Sim's generator
//! can hold the original's.
//!
//! # The Sim's run
//!
//! [`Dust`] is a list of entries per player, given to a Sim that races ([`crate::Sim::dust`]); without
//! it nothing here draws, as before. In each rider's frame, after its brain has thought:
//!
//! - **The manager** reads the bike's snapshot, which the bike takes after the manager has run: what
//!   it reads is the frame before's. It sets the frame's flags — wrecked 8, else 0x10; each tyre's no contact 1, ground
//!   2, object 4 — and switches each entry on or off by its `Disable` mask against them. A live entry
//!   then runs its own update: the exhaust is on while `throttle · ½ · (MaxSpeed − speed) / MaxSpeed`
//!   is above 0, the generic is always on, and a spray is on while the rear wheel spins under throttle
//!   (or the side load passes its threshold) and for a quarter second after.
//! - **The emitter** takes its point: last frame's becomes the previous one and the new one is the
//!   bike's transform applied to the emitter's place on the bike ([`Emitter::at`]). An emitter switched
//!   on this frame only takes its point. One that is on and moved at least its `MinimumEmissionSpeed`
//!   draws twice.
//!
//! # The emitter's point (S421)
//!
//! An emitter's point is the place of the model node it hangs from, brought up to date down the
//! node tree (each node's world is its local in its parent's world, [`crate::atv_moto`]'s
//! `frame_mul`). The rear tyre's emitters hang from `RearTire_Particle01`, under `SwingArm`, under
//! `Bike`, under the bike's root, whose world is the chassis' transform ([`RearNode`]):
//!
//! - **`SwingArm`** is aimed at the rear axle every frame: its local is the look-at
//!   ([`crate::math::basis`], orthogonalised, the forward kept) from the rear mount lowered by the
//!   compression up to its pivot, `y` up, at the pivot. The compression is the bike's animation
//!   snapshot, taken after the emitters have run, so it is the frame before's, like [`Feed`].
//! - **`Bike`** is the chassis pose's, held at its seeded local: the bike's clips are not in the Sim.
//! - The emitter's point is the node's world position plus its offset (zero on this disc).
//!
//! # The front tyre's and the exhaust's points (S446)
//!
//! The front tyre's sprays hang from `FrontTire_Particle01`, under `Shocks`, under `Handlebars`,
//! under `Bike` ([`FrontNode`]); the exhaust from `250_Exhaust_Particle01` under the root of the
//! pipe's own model ([`ExhaustNode`]). The bike's drive sets those nodes after the emitters have run,
//! from the snapshot it has just taken and the root as the frame left it ([`RiderDust::frame`] runs
//! it last), so each emitter reads the frame before's:
//!
//! - **The bike's clip** poses `Bike` and `Handlebars` first (a turn of the bars of about a degree
//!   while riding). The clips are not in the Sim: both are held at their seeded locals
//!   ([`FrontNode::pose`]).
//! - **`Shocks`** slides along the steering axis: its place is the fork's rest place plus the axis
//!   times the front compression times the slide scale, plus a fixed nudge, taken into the world by
//!   `Bike`'s world and back into `Handlebars`' frame as the clip posed it. Its rotation is held.
//! - **`Handlebars`** is then turned by the front wheel's steer about the steering axis
//!   (`node_set_rotation`, [`crate::kiowa`]'s turn), the clip's place kept — only while a wheel is on
//!   the ground; in the air it stays as the clip posed it.
//! - **The pipe's root**'s local is a copy of `Bike`'s world.
//!
//! # The collision entries (S422)
//!
//! A `CollisionModel` ([`Kind::Collision`]) and a `ParticleCollisionConstraint` ([`Kind::Spheres`],
//! the foot dust among them) read no tyre: the manager switches them by the wreck flag alone. Their
//! emitters hang from nothing and are emitted from inside the entry's own update, in the manager's
//! pass, once per contact ([`Entry::touch_update`]); the emitter pass leaves them alone.
//!
//! - **A collision model** reads the wreck bike's collision object ([`Object`]): its contacts as the
//!   wreck bike's node left them this frame (the node runs before the manager).
//! - **A bone collision entry** reads the rider's bone collisions ([`Ball`]) as the body left them
//!   the frame before (the body runs after the manager): the wreck's ragdoll, or the riding one.
//! - **The emit** at a contact takes the contact as its point and the contact less the velocity
//!   times the frame as its previous one, and makes the speed test. An emitter just switched on only
//!   takes the point, as in the emitter pass; an emitter **culled** makes nothing at all.
//! - **The wreck flag** they switch by is the snapshot's copy of the wreck reason, which the state
//!   pack makes before the frame's verdict: a wreck's dust switches on two frames after its verdict
//!   and emits from the third (S483, [`RiderDust::wrecked`]).
//! - **The view test** runs once a frame after the manager, on the emitter's point
//!   ([`Emitter::view_test`]): culled past `FarCullDistance` from the camera, or behind it and past
//!   `DirectionalCullDistance`. An emitter switched on carries the last frame's answer, made at its
//!   stale point, so the first frame of a wreck far from the last one emits nothing. The camera is
//!   the shell's ([`Dust::view`]); with none it is the bike's point.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::atv_moto::frame_mul;
use crate::camera::CurveTable;
use crate::kiowa::{axis_turn, node_unpoint};
use crate::math::{Mat4, basis, transform_point};
use crate::rng::Rng;
use ps2_float::vu::{self, Dest};
use ps2_float::{F, Vf};

/// The manager's frame flags, as the entries' `Disable` masks name them.
pub const NO_CONTACT: u8 = 1;
pub const GROUND: u8 = 2;
pub const OBJECT: u8 = 4;
pub const WRECKING: u8 = 8;
pub const NOT_WRECKING: u8 = 0x10;

/// What a tyre tells the manager this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tyre {
    pub ground: bool,
    pub object: bool,
    pub spinning: bool,
    /// The side load per unit of load, the spray's sideways push.
    pub side: F,
}

impl Tyre {
    /// The tyre's flags: its contacts, and the frame's wreck flag.
    fn flags(&self, wrecking: bool) -> u8 {
        let ground = if self.ground { GROUND } else { 0 };
        let object = if self.object { OBJECT } else { 0 };
        let none = if !self.ground && !self.object { NO_CONTACT } else { 0 };
        let wreck = if wrecking { WRECKING } else { NOT_WRECKING };
        wreck | ground | object | none
    }
}

/// What a rider's manager reads this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Feed {
    pub wrecking: bool,
    pub throttle: F,
    pub clutch: F,
    pub airspeed: F,
    /// Front, then rear.
    pub tyres: [Tyre; 2],
    /// The rear wheel's compression, which aims the swingarm ([`RearNode`]).
    pub rear_compression: F,
    /// The front wheel's compression, which slides the fork ([`FrontNode`]).
    pub front_compression: F,
    /// The front wheel's steer angle, which turns the bars ([`FrontNode`]).
    pub steer: F,
    /// Either wheel on the ground: the bars are turned only then.
    pub grounded: bool,
}

/// Where an emitter's point comes from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    /// A fixed place in the chassis' frame.
    Chassis(Vf),
    /// The rear tyre's node ([`RearNode`]).
    Rear,
    /// The front tyre's node ([`FrontNode`]).
    Front,
    /// The exhaust's node ([`ExhaustNode`]).
    Exhaust,
}

/// The nodes' points this frame, for the emitters that hang from them.
#[derive(Clone, Copy, Debug, Default)]
struct Points {
    rear: Option<Vf>,
    front: Option<Vf>,
    exhaust: Option<Vf>,
}

/// **The rear tyre's emitter node and the two above it** (S421, the module header).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RearNode {
    /// `Bike`'s local under the bike's root: the chassis pose's, held.
    pub bike: Mat4,
    /// `SwingArm`'s place in the `Bike` frame: its local's position, which the aim keeps.
    pub pivot: Vf,
    /// The rear wheel's mount.
    pub mount: Vf,
    /// The emitter node's local under `SwingArm`.
    pub node: Mat4,
}

impl RearNode {
    /// `SwingArm`'s local at a rear compression of `c`: the direction from the lowered mount up to
    /// the pivot (`vsub.xyz`; its length squared `vmul`, `vadday.x`, `vmaddz.x`; `vrsqrt`, `vmulq`),
    /// then the look-at at the pivot, orthogonalised with the forward kept.
    pub fn swingarm(&self, c: F) -> Mat4 {
        let m = self.mount;
        let axle = Vf::new(m.x(), (F(m.y()) + c).0, m.z(), 0.0);
        let d = self.pivot.merge(Dest::XYZ, self.pivot.sub(axle));
        let s = d.mul(d);
        let q = vu::rsqrt(1.0, vu::madd(vu::add(s.x(), s.y()), 1.0, s.z()));
        let dir = d.merge(Dest::XYZ, d.mul_q(q));
        basis(self.pivot, dir, Vf::new(0.0, 1.0, 0.0, 1.0), true, true)
    }

    /// The emitter's point with the bike's root at `root` and a rear compression of `c`: the node's
    /// world down the tree, its position with `w` 1, plus the (zero) offset (`vadd.xyz`).
    pub fn point(&self, root: &Mat4, c: F) -> Vf {
        let bike = frame_mul(&self.bike.0, &root.0);
        let arm = frame_mul(&self.swingarm(c).0, &bike);
        emitter_point(&frame_mul(&self.node.0, &arm))
    }
}

/// `Shocks`' nudge, added to its place in the `Bike` frame after the slide.
const FORK_NUDGE: Vf = Vf([-0.03, 0.0, 0.02, 1.0]);

/// A node's world position with `w` 1, plus the (zero) emitter offset (`vadd.xyz`): an emitter's point.
fn emitter_point(world: &[Vf; 4]) -> Vf {
    Vf::new(0.0, 0.0, 0.0, 1.0).merge(Dest::XYZ, world[3].add(Vf::ZERO))
}

/// `transform_point`'s node form: a point in a node's frame into the world (`vaddax.xyz` row 3,
/// `vmaddax`/`vmadday`/`vmaddz.xyz` the axes), `w` kept.
fn node_point(m: &[Vf; 4], v: Vf) -> Vf {
    let acc = m[3].add(Vf::ZERO);
    let acc = Vf::madd(acc, m[0], v.bc(vu::Field::X));
    let acc = Vf::madd(acc, m[1], v.bc(vu::Field::Y));
    v.merge(Dest::XYZ, Vf::madd(acc, m[2], v.bc(vu::Field::Z)))
}

/// **The front tyre's emitter node and the three above it** (S446, the module header). `bars` and
/// `shocks` are the nodes' locals as the last drive left them; `pose` is the clip's `Handlebars`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrontNode {
    /// `Bike`'s local under the bike's root: the chassis pose's, held.
    pub bike: Mat4,
    /// `Handlebars`' local under `Bike`.
    pub bars: Mat4,
    /// `Handlebars`' local as the bike's clip poses it before the drive: held.
    pub pose: Mat4,
    /// `Shocks`' local under `Handlebars`.
    pub shocks: Mat4,
    /// The emitter node's local under `Shocks`.
    pub node: Mat4,
    /// The steering axis, in the `Bike` frame.
    pub axis: Vf,
    /// The fork's rest place, in the `Bike` frame.
    pub rest: Vf,
    /// The fork's slide per unit of front compression.
    pub slide: F,
}

impl FrontNode {
    /// The emitter's point with the bike's root at `root`: the node's world down the tree.
    pub fn point(&self, root: &Mat4) -> Vf {
        let bike = frame_mul(&self.bike.0, &root.0);
        let bars = frame_mul(&self.bars.0, &bike);
        let shocks = frame_mul(&self.shocks.0, &bars);
        emitter_point(&frame_mul(&self.node.0, &shocks))
    }

    /// The drive's step, after the emitters, on this frame's snapshot `now` and the root at `root`:
    /// `Handlebars` posed, `Shocks` slid (`vmulx.xyz`, `vadd.xyz`, the nudge `vadd.xyz`; into the world
    /// by `Bike`'s, back by the posed `Handlebars`', the place's `xyz` written), then on the ground
    /// `Handlebars` turned by the steer, its place kept with `w` 1.
    pub fn drive(&mut self, root: &Mat4, now: &Feed, curve: &CurveTable) {
        let bike = frame_mul(&self.bike.0, &root.0);
        let along = self.axis.merge(Dest::XYZ, self.axis.mul(Vf::splat((self.slide * now.front_compression).0)));
        let at = self.rest.merge(Dest::XYZ, self.rest.add(along));
        let at = at.merge(Dest::XYZ, at.add(FORK_NUDGE));
        let world = node_point(&bike, at);
        self.bars = self.pose;
        let local = node_unpoint(&Mat4(frame_mul(&self.bars.0, &bike)), world);
        self.shocks.0[3] = self.shocks.0[3].merge(Dest::XYZ, local);
        if now.grounded {
            let (cos, sin) = curve.at(now.steer);
            let place = self.bars.0[3];
            self.bars = axis_turn([self.axis.x(), self.axis.y(), self.axis.z()], cos, sin);
            self.bars.0[3] = Vf::new(place.x(), place.y(), place.z(), 1.0);
        }
    }
}

/// **The exhaust's emitter node and the pipe's root above it** (S446, the module header).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExhaustNode {
    /// `Bike`'s local under the bike's root: the chassis pose's, held.
    pub bike: Mat4,
    /// The pipe's root's local: `Bike`'s world as the last drive copied it.
    pub pipe: Mat4,
    /// The emitter node's local under the pipe's root.
    pub node: Mat4,
}

impl ExhaustNode {
    /// The emitter's point: the node's world under the pipe's root, whose world is its local.
    pub fn point(&self) -> Vf {
        emitter_point(&frame_mul(&self.node.0, &self.pipe.0))
    }

    /// The drive's step, after the emitters: the pipe's root takes `Bike`'s world with the root at `root`.
    pub fn drive(&mut self, root: &Mat4) {
        self.pipe = Mat4(frame_mul(&self.bike.0, &root.0));
    }
}

/// An entry's own update.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// On while the throttle pushes below `max_speed`.
    Exhaust { max_speed: F },
    /// On whenever it is live.
    Generic,
    /// The tyre's spray: on while its wheel spins under throttle or the side load passes `threshold`,
    /// and for `clock` seconds more.
    Spray { threshold: F, clock: F },
    /// A `CollisionModel`: one emit per contact of the wreck bike's collision object ([`Object`]),
    /// in a zone with particles (S422). `zones` is the entry's sixteen zone groups, none = 0xff.
    Collision { zones: [u8; 16] },
    /// A `ParticleCollisionConstraint` (the foot dust among them): one emit per bone collision
    /// ([`Ball`]) whose bit is in `mask`, touching as `Disable` allows, in a zone with particles (S422).
    Spheres { mask: u32, zones: [u8; 16] },
    /// An entry whose emitter does not draw on the shared generator here: the tyre tracks (the
    /// module header).
    Quiet,
}

/// The wreck bike's collision object as a [`Kind::Collision`] reads it (S422): its record's ground
/// and object contact, the collider's last relative velocity, and each contact's point and surface
/// (-1 for none).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Object {
    pub ground: bool,
    pub object: bool,
    pub relative: Vf,
    pub contacts: Vec<(Vf, i32)>,
}

/// A bone collision as a [`Kind::Spheres`] reads it (S422): its contact's ground and scene flags,
/// the surface the ground query last wrote, the last contact point, and its bone's velocity.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ball {
    pub ground: bool,
    pub object: bool,
    pub surface: u8,
    pub point: Vf,
    pub velocity: Vf,
}

/// What the collision entries read in a frame (S422). The wreck bike's node runs before the manager,
/// so its object is this frame's; the rider's body runs after it, so the balls are the frame
/// before's. `None` for a ball the feed has no collision for.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Touches {
    pub object: Option<Object>,
    pub balls: Vec<Option<Ball>>,
}

/// A zone group, by index, as the updates read it: an index past the sixteen reads the entry's
/// neighbouring bytes, which are never 0xff on the discs (S422's journal), so it counts as a zone with
/// particles.
fn zone_on(zones: &[u8; 16], index: i64) -> bool {
    usize::try_from(index).ok().and_then(|i| zones.get(i)).is_none_or(|&z| z != 0xff)
}

/// The flags of a contact: its ground and object contact, none when neither.
fn contact_flags(ground: bool, object: bool) -> u8 {
    let none = if !ground && !object { NO_CONTACT } else { 0 };
    (if ground { GROUND } else { 0 }) | (if object { OBJECT } else { 0 }) | none
}

/// An emitter's switch and its two points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Emitter {
    /// Switched on (the emitter's on bit).
    pub on: bool,
    /// Switched on since its last point (the just-on bit): its next point only takes the place.
    pub just_on: bool,
    /// `MinimumEmissionSpeed`.
    pub min_speed: F,
    /// Where its point comes from.
    pub place: Place,
    pub pos: Vf,
    pub prev: Vf,
    /// Its view cull, if it has one (`UseViewDependentCull`, S422).
    pub cull: Option<Cull>,
    /// Culled at the last frame's view test: a collision entry's emits make nothing until the next
    /// test ([`Emitter::view_test`]).
    pub culled: bool,
}

/// An emitter's view cull distances (S422): `FarCullDistance`, and `DirectionalCullDistance` for a
/// point behind the camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cull {
    pub far: F,
    pub directional: F,
}

/// The camera an emitter's view test measures from (S422): its point and its forward row.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct View {
    pub at: Vf,
    pub forward: Vf,
}

impl Emitter {
    /// **The view test** (S422), once a frame after the emitters have run: an emitter with a cull is
    /// culled when its point is farther than `far` from the camera, or behind it (its offset
    /// against the forward row below zero) and farther than `directional`.
    pub fn view_test(&mut self, view: &View) {
        let Some(c) = self.cull else { return };
        let d = self.pos.sub(view.at);
        let (x, y, z) = (F(d.x()), F(d.y()), F(d.z()));
        let dist = F::dot([x, y, z], [x, y, z]).sqrt();
        self.culled = if dist <= c.far {
            let f = view.forward;
            let behind = F::dot([F(f.x()), F(f.y()), F(f.z())], [x, y, z]) < F(0.0);
            behind && !(dist <= c.directional)
        } else {
            true
        };
    }

    fn switch_on(&mut self) {
        if !self.on {
            self.on = true;
            self.just_on = true;
        }
    }

    fn switch_off(&mut self) {
        self.on = false;
        self.prev = self.pos;
    }

    /// The emitter's frame: its new point, the node's for one on a node, then the draws it makes
    /// (0 or 2).
    fn frame(&mut self, bike: &Mat4, points: Points, dt: F) -> u8 {
        self.prev = self.pos;
        let node = match self.place {
            Place::Chassis(at) => Some(transform_point(bike, at)),
            Place::Rear => points.rear,
            Place::Front => points.front,
            Place::Exhaust => points.exhaust,
        };
        // A rider seeded without the node: the point stays where it was.
        self.pos = node.unwrap_or(self.pos);
        if self.just_on {
            self.just_on = false;
            self.prev = self.pos;
            return 0;
        }
        if !self.on {
            self.prev = self.pos;
            return 0;
        }
        self.gate(dt)
    }

    /// The emit's speed test on the two points: two draws if the point moved at least
    /// `MinimumEmissionSpeed` over the frame, else none.
    fn gate(&self, dt: F) -> u8 {
        let d = self.pos.sub(self.prev);
        let (x, y, z) = (F(d.x()), F(d.y()), F(d.z()));
        let speed = F::dot([x, y, z], [x, y, z]).sqrt() / dt;
        if speed < self.min_speed { 0 } else { 2 }
    }

    /// An emit at points the update set (S422, a collision entry's): with no node the emitter keeps
    /// them. An emitter just switched on or off takes `pos` for both and draws nothing.
    fn emit_at(&mut self, pos: Vf, prev: Vf, dt: F) -> u8 {
        self.pos = pos;
        self.prev = prev;
        if self.just_on {
            self.just_on = false;
            self.prev = self.pos;
            return 0;
        }
        if self.culled {
            return 0;
        }
        if !self.on {
            self.prev = self.pos;
            return 0;
        }
        self.gate(dt)
    }
}

/// One of the manager's entries.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    pub kind: Kind,
    /// The tyre it reads, 0 front or 1 rear, if any.
    pub tyre: Option<usize>,
    /// The frame flags that switch it off.
    pub disable: u8,
    /// Live (the entry's bit 0).
    pub live: bool,
    /// Its tyre is in a zone with no particles (bit 1): live, but its emitter stays off.
    pub zone_off: bool,
    pub emitter: Emitter,
    /// The draws its emitter made last frame.
    pub draws: u8,
}

impl Entry {
    fn enable(&mut self) {
        self.live = true;
        if !self.zone_off {
            self.emitter.switch_on();
        }
    }

    fn disable(&mut self) {
        self.live = false;
        if let Kind::Spray { clock, .. } = &mut self.kind {
            *clock = F(0.0);
        }
        self.emitter.switch_off();
    }

    /// The entry's own update, while it is live.
    fn update(&mut self, feed: &Feed, dt: F) {
        match &mut self.kind {
            Kind::Exhaust { max_speed } => {
                let max = *max_speed;
                let f = feed.throttle * F(0.5) * (max - feed.airspeed) / max;
                if f <= F(0.0) { self.emitter.switch_off() } else { self.emitter.switch_on() }
            }
            Kind::Generic => self.emitter.switch_on(),
            Kind::Spray { threshold, clock } => {
                let tyre = feed.tyres[self.tyre.unwrap_or(1)];
                let idle = feed.throttle == F(0.0) || feed.clutch != F(0.0) || !tyre.spinning;
                let level = -*threshold <= tyre.side && tyre.side <= *threshold;
                if idle && level {
                    if F(0.0) < *clock {
                        *clock = *clock - dt;
                        if F(0.0) < *clock {
                            self.emitter.switch_on();
                            return;
                        }
                    }
                    *clock = F(0.0);
                    self.emitter.switch_off();
                } else {
                    *clock = F(0.25);
                    self.emitter.switch_on();
                }
            }
            Kind::Collision { .. } | Kind::Spheres { .. } | Kind::Quiet => {}
        }
    }

    /// **A collision entry's update** (S422): its emitter's switch and its emits, on `touches`; the
    /// draws they make. Nothing for the other kinds, whose emitters draw in the emitter pass.
    ///
    /// - A collision model's flags are the frame's wreck flag and the object's contacts. While its
    ///   `Disable` lets them through the emitter is on, and each contact in a zone with particles
    ///   emits from the contact point, its previous point the point less the relative velocity times
    ///   the frame (`xyz`). Otherwise the emitter is off.
    /// - A bone collision entry's emitter is on while `Disable` lets the wreck flag through. Then
    ///   each ball in `mask`, in a zone with particles and with contact flags `Disable` lets through,
    ///   emits from its point, its previous point the point less its bone's velocity times the frame.
    pub fn touch_update(&mut self, feed: &Feed, touches: &Touches, dt: F) -> u32 {
        let wreck = if feed.wrecking { WRECKING } else { NOT_WRECKING };
        let mut draws = 0;
        match self.kind {
            Kind::Collision { zones } => {
                let none = Object::default();
                let o = touches.object.as_ref().unwrap_or(&none);
                if self.disable & (wreck | contact_flags(o.ground, o.object)) != 0 {
                    self.emitter.switch_off();
                    return 0;
                }
                self.emitter.switch_on();
                let v = o.relative.mul_q(dt.0);
                for &(point, surface) in &o.contacts {
                    if zone_on(&zones, i64::from(surface)) {
                        let prev = point.merge(Dest::XYZ, point.sub(v));
                        draws += u32::from(self.emitter.emit_at(point, prev, dt));
                    }
                }
            }
            Kind::Spheres { mask, zones } => {
                if self.disable & wreck == 0 { self.emitter.switch_on() } else { self.emitter.switch_off() }
                for (k, b) in touches.balls.iter().enumerate() {
                    let Some(b) = b else { continue };
                    if k >= 32 || mask >> k & 1 == 0 {
                        continue;
                    }
                    if zone_on(&zones, i64::from(b.surface)) && self.disable & contact_flags(b.ground, b.object) == 0 {
                        let prev = b.point.merge(Dest::XYZ, b.point.sub(b.velocity.mul_q(dt.0)));
                        draws += u32::from(self.emitter.emit_at(b.point, prev, dt));
                    }
                }
            }
            _ => {}
        }
        draws
    }
}

/// A rider's entries, in the manager's order, and the snapshot of the bike they read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RiderDust {
    pub entries: Vec<Entry>,
    /// What the manager reads: the bike's snapshot, which the bike takes after the manager has run,
    /// so each frame's is the frame before's.
    pub feed: Feed,
    /// The rear tyre's node, for the entries at [`Place::Rear`].
    pub rear: Option<RearNode>,
    /// The front tyre's node, for the entries at [`Place::Front`].
    pub front: Option<FrontNode>,
    /// The exhaust's node, for the entries at [`Place::Exhaust`].
    pub exhaust: Option<ExhaustNode>,
    /// The bone collisions the [`Kind::Spheres`] entries read: the body's, as the frame before left
    /// them (S422, [`Touches`]).
    pub balls: Vec<Option<Ball>>,
    /// The rider's wreck as the frame before left it, which this frame's snapshot carries (S483):
    /// the snapshot's wreck word is the state pack's copy of the reason, made before the frame's
    /// verdict, so a wreck reaches the manager two frames after its verdict.
    pub wrecked: bool,
}

impl RiderDust {
    /// **The rider's frame** (the module header): the manager's switches and updates on the last
    /// snapshot, then each emitter's point and draws, then this frame's snapshot, `now`, and the
    /// drive's step on it. Answers the draws, which the caller makes on the game's generator.
    pub fn frame(&mut self, now: Feed, bike: &Mat4, curve: &CurveTable, dt: F) -> u32 {
        self.frame_with(now, None, Vec::new(), bike, curve, dt)
    }

    /// [`RiderDust::frame`] with the collision entries' inputs (S422): the wreck bike's `object` as
    /// this frame left it, and the body's `balls` as this frame left them, which the next frame reads.
    pub fn frame_with(
        &mut self,
        now: Feed,
        object: Option<Object>,
        balls: Vec<Option<Ball>>,
        bike: &Mat4,
        curve: &CurveTable,
        dt: F,
    ) -> u32 {
        let feed = &std::mem::replace(&mut self.feed, now);
        let touches = Touches { object, balls: std::mem::replace(&mut self.balls, balls) };
        let mut made = vec![0; self.entries.len()];
        let wreck = if feed.wrecking { WRECKING } else { NOT_WRECKING };
        let tyres = feed.tyres.map(|t| t.flags(feed.wrecking));
        for (e, made) in self.entries.iter_mut().zip(&mut made) {
            let tyre = e.tyre.map_or(0, |t| tyres[t]);
            if e.disable & tyre == 0 && e.disable & wreck == 0 {
                if !e.live {
                    e.enable();
                }
            } else if e.live {
                e.disable();
            }
            if e.live && !e.zone_off {
                e.update(feed, dt);
                // A collision entry emits in its update, in the manager's pass (S422).
                *made = e.touch_update(feed, &touches, dt);
            }
        }
        let points = Points {
            rear: self.rear.map(|n| n.point(bike, feed.rear_compression)),
            front: self.front.map(|n| n.point(bike)),
            exhaust: self.exhaust.map(|n| n.point()),
        };
        let mut draws = 0;
        for (e, made) in self.entries.iter_mut().zip(made) {
            e.draws = match e.kind {
                Kind::Quiet => 0,
                Kind::Collision { .. } | Kind::Spheres { .. } => {
                    draws += made;
                    e.draws = u8::try_from(made).unwrap_or(u8::MAX);
                    continue;
                }
                _ => e.emitter.frame(bike, points, dt),
            };
            draws += u32::from(e.draws);
        }
        if let Some(n) = &mut self.front {
            n.drive(bike, &self.feed, curve);
        }
        if let Some(n) = &mut self.exhaust {
            n.drive(bike);
        }
        draws
    }
}

/// Every player's entries, by player.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dust {
    pub riders: Vec<RiderDust>,
    /// The camera the emitters' view tests measure from (S422), an input like the controls: the
    /// shell's, as it drew the frame. `None` measures from each rider's bike, with no forward row.
    pub view: Option<View>,
}

/// An emitter's place in the chassis' frame, from where it is in the world with the bike at `bike`:
/// what a seed from an image puts in [`Place::Chassis`].
pub fn place_on(bike: &Mat4, world: Vf) -> Option<Vf> {
    crate::math::inverse(bike).map(|inv| transform_point(&inv, world))
}

impl crate::Sim {
    /// **Player `i`'s dust frame** (S328c, the module header), on the game's generator — the race's,
    /// or the AI lane's when there is no race (S387). A Sim with no dust, or no generator, draws nothing here.
    pub(crate) fn dust_frame(&mut self, i: usize, dt: F) {
        let Some(dust) = self.dust.as_deref() else { return };
        if dust.riders.len() <= i {
            return;
        }
        let rider = self.rider_of(i);
        let r = &rider.ride;
        let tyre = |w: &crate::ride::Wheel| Tyre {
            ground: w.w.ray_hit != 0,
            object: w.w.swept_contact != 0,
            spinning: w.slip.spinning,
            side: w.side_per_load,
        };
        // The snapshot's wreck is the frame before's (S483, [`RiderDust::wrecked`]).
        let wrecked = rider.crash.is_some();
        let feed = Feed {
            wrecking: dust.riders[i].wrecked,
            throttle: r.held.gas,
            clutch: r.mv.clutch_in,
            airspeed: r.chassis.speed,
            tyres: [tyre(&r.front), tyre(&r.rear)],
            rear_compression: r.rear.w.compression,
            front_compression: r.front.w.compression,
            steer: r.front.w.steer_angle,
            grounded: r.air.on_ground,
        };
        let bike = r.transform();
        // The wreck bike's collision object, and the body's bone collisions (S422).
        let object = rider.crash.as_ref().map(|c| Object {
            ground: c.bike.record.on_ground,
            object: c.bike.record.on_object,
            relative: c.bike.collider.relative,
            contacts: c.bike.record.contacts.iter().map(|k| (k.point, k.surface)).collect(),
        });
        let balls = match (&rider.ragdoll, &rider.riding) {
            (Some(down), _) => balls_of(&down.ragdoll, &down.contacts),
            (None, Some(riding)) => balls_of(&riding.actor.ragdoll, &riding.actor.contacts),
            (None, None) => Vec::new(),
        };
        let tables = std::sync::Arc::clone(&self.tables);
        let Some(dust) = self.dust.as_deref_mut() else { return };
        let view = dust.view.unwrap_or(View { at: bike.0[3], forward: Vf([0.0; 4]) });
        let rider = &mut dust.riders[i];
        rider.wrecked = wrecked;
        let draws = rider.frame_with(feed, object, balls, &bike, &tables.ride.curve, dt);
        // The emitters' view tests, after the manager has run (S422).
        for e in &mut rider.entries {
            e.emitter.view_test(&view);
        }
        let rng: &mut Rng = match (self.ai.as_deref_mut(), self.race.as_deref_mut()) {
            // In a race, the race's: the lane's own is for a ride with no race (S387).
            (_, Some(race)) => &mut race.rng,
            (Some(lane), None) => &mut lane.rng,
            (None, None) => return,
        };
        for _ in 0..draws {
            rng.rand();
        }
    }
}

/// The bones of the feed's nine spheres, in its order (S476): the rider body keeps one collision per
/// named bone (the two feet, then the pelvis, the spine, the head, the arms and the legs), filled by
/// bone as the rider file's collisions are built, and the body hands those nine to the feed every
/// frame. So the order is the slots', not the file's, and a slot no collision fills is empty.
pub const FEED_BONES: [usize; 9] = [17, 14, 0, 2, 3, 10, 6, 16, 13];

/// A ragdoll's bone collisions as the collision entries read them (S422), in the feed's order
/// ([`FEED_BONES`], S476): each contact's flags, zone and point, and its bone's velocity. A collision
/// that is off (a held foot) is handed over all the same: its flags are cleared every tick, so it
/// touches nothing, and its zone and point are the last solve's (S476).
pub fn balls_of(doll: &crate::ragdoll::Ragdoll, contacts: &[crate::ragdoll_contact::Contact]) -> Vec<Option<Ball>> {
    FEED_BONES
        .iter()
        .map(|&bone| {
            let k = doll.collisions.iter().position(|k| k.bone == bone)?;
            let c = contacts.get(k)?;
            Some(Ball {
                ground: c.ground,
                object: c.scene,
                surface: c.surface,
                point: c.point,
                velocity: doll.bones[bone].body.velocity,
            })
        })
        .collect()
}
