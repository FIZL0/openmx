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
    /// An entry whose emitter does not draw on the shared generator here: the collision models, the
    /// rider's foot dust, the tyre tracks (the module header).
    Quiet,
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
}

impl Emitter {
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
        let d = self.pos.sub(self.prev);
        let (x, y, z) = (F(d.x()), F(d.y()), F(d.z()));
        let speed = F::dot([x, y, z], [x, y, z]).sqrt() / dt;
        if speed < self.min_speed { 0 } else { 2 }
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
            Kind::Quiet => {}
        }
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
}

impl RiderDust {
    /// **The rider's frame** (the module header): the manager's switches and updates on the last
    /// snapshot, then each emitter's point and draws, then this frame's snapshot, `now`, and the
    /// drive's step on it. Answers the draws, which the caller makes on the game's generator.
    pub fn frame(&mut self, now: Feed, bike: &Mat4, curve: &CurveTable, dt: F) -> u32 {
        let feed = &std::mem::replace(&mut self.feed, now);
        let wreck = if feed.wrecking { WRECKING } else { NOT_WRECKING };
        let tyres = feed.tyres.map(|t| t.flags(feed.wrecking));
        for e in &mut self.entries {
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
            }
        }
        let points = Points {
            rear: self.rear.map(|n| n.point(bike, feed.rear_compression)),
            front: self.front.map(|n| n.point(bike)),
            exhaust: self.exhaust.map(|n| n.point()),
        };
        let mut draws = 0;
        for e in &mut self.entries {
            e.draws = match e.kind {
                Kind::Quiet => 0,
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
        let feed = Feed {
            wrecking: rider.crash.is_some(),
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
        let tables = std::sync::Arc::clone(&self.tables);
        let Some(dust) = self.dust.as_deref_mut() else { return };
        let draws = dust.riders[i].frame(feed, &bike, &tables.ride.curve, dt);
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
