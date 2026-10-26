//! **The ragdoll while riding** (S109): the rider's eighteen bones run on every tick, not only in a
//! wreck.
//!
//! In the original the skeleton a riding rider is drawn with *is* the riding ragdoll (S70). Each
//! tick the body actor's update carries the riding pose round with the bike, the four holds pin the
//! hands to the grips and the feet to the pegs, the bike box keeps the pelvis round the seat, the
//! pose spring pulls each bone to where the animated skeleton has it, and after the step every
//! bone's node is written back from its body. The lean across a slope and the body's give on the
//! bike are that step's, not the clip's.
//!
//! [`crate::ragdoll_body::Actor`] is the update, proved tick for tick against the user's animandpump
//! ride from each captured row (1203 of 1203 riding ticks). This module is what runs it beside a
//! ride: [`Riding::new`] gives the actor the constructor's numbers, [`request`] is what the bike
//! hands it, and [`Riding::step`] is one tick with the skeleton's points and rests off the
//! animated pose. Its generator is its own.
//!
//! Since S110 it is the rider's ([`crate::rider::Rider::riding`]), stepped at the top of each frame for
//! the tick the shell last drew, and a wreck enters from its bodies rather than from the animated
//! skeleton ([`Riding::hand_over`]). The bike's ride reads nothing else of it, and the shell draws it
//! only when asked to (`OPENMX_RIDING_RAGDOLL=1`).

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::chassis::{self, Mat3};
use crate::math::{self, Mat4};
use crate::ragdoll::{MAIN, PARTS, Ragdoll};
use crate::ragdoll_body::{Actor, Outside, Request, Scenery, Skeleton, Tail};
use crate::ragdoll_contact::Open;
use crate::ragdoll_pose::{self, Bone, Start};
use crate::ragdoll_ride::{self, Kit, Posed};
use crate::ride::Ride;
use crate::rng::Rng;
use crate::world::World;
use ps2_float::{F, Vf};

/// The speed the riding pose's damper is scaled up to, under which the rider is standing:
/// the clutch's idle speed, as the vehicle animation's constructor sets it.
pub const STAND_SPEED: F = F(f32::from_bits(0x40ea_aa65));

/// The speed under which the look-around may lift a foot.
pub const SLOW_SPEED: F = F(f32::from_bits(0x4292_aa7f));

/// The feet's offsets from the request's points, in the bike's frame: left, then right.
const FOOT_POINTS: [Vf; 2] = [
    Vf([f32::from_bits(0xbca6_2990), f32::from_bits(0xbddc_9aff), f32::from_bits(0xbe29_8e53), 1.0]),
    Vf([f32::from_bits(0x3d04_0a69), f32::from_bits(0xbddc_9b3f), f32::from_bits(0xbe29_8e75), 1.0]),
];

/// The bike box the pelvis is kept round, in the bike's frame: its centre and half-size.
const BIKE_BOX: (Vf, Vf) = (
    Vf([0.0, f32::from_bits(0x4089_9999), -1.5, 1.0]),
    Vf([f32::from_bits(0x3ee6_6666), f32::from_bits(0x3f19_9999), 1.75, 1.0]),
);

/// `(0, 0, 0, 1)`.
const UNIT_W: Vf = Vf([0.0, 0.0, 0.0, 1.0]);

/// The ground's box the bones ask inside: everywhere.
const TERRAIN_MIN: Vf = Vf([-1.0e30, -1.0e30, -1.0e30, 1.0]);
const TERRAIN_MAX: Vf = Vf([1.0e30, 1.0e30, 1.0e30, 1.0]);

/// **The riding ragdoll**: the body actor, and the skeleton's drawn matrices from its last pose,
/// which a moving pose measures the bones' velocity from.
///
/// Since S110 it is the [`crate::rider::Rider`]'s, stepped by the rider's own frame and sent and
/// hashed with it: a wreck enters from its bodies ([`Riding::hand_over`]), as the original's does.
#[derive(Clone, Debug, PartialEq)]
pub struct Riding {
    pub actor: Actor,
    pub(crate) drawn: Option<Vec<Mat4>>,
    /// The bodies' nodes' drawn matrices before the last step (S110): what the wreck entry measures
    /// the bones' move and turn from. `None` until a placed body has been stepped.
    pub(crate) was: Option<Vec<Mat4>>,
    pub(crate) rng: Rng,
}

/// **What the drawn bike hands the riding body** (S110): the chassis' root node ("Bike") and the four
/// holds' points (the grips, then the pegs: left before right) in the chassis' model frame, as the
/// shell last drew the bike. An input like [`crate::rider::Rider::skeleton`]: written by a shell that
/// draws the chassis, never sent, and with none the rider has no riding body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Holds {
    pub root: Mat4,
    pub hands: [Vf; 2],
    pub feet: [Vf; 2],
}

/// How far the riding ragdoll's main bone may be from the bike before it is started again, in feet.
pub const STRAY: F = F(10.0);

impl Riding {
    /// The actor over the kit's ragdoll, with the numbers the original's rider body is built with.
    /// It is not placed: its first tick poses the bones where the skeleton has them and runs the
    /// riding entry.
    pub fn new(kit: &Kit) -> Riding {
        let mut actor = Actor::new(kit.ragdoll.clone(), kit.spheres.len());
        actor.pose_spring = kit.pose_spring;
        actor.pose_damping = kit.pose_damping;
        actor.pose_damping_base = kit.pose_damping;
        actor.stand_speed = STAND_SPEED;
        actor.slow_speed = SLOW_SPEED;
        actor.foot_points = FOOT_POINTS;
        actor.bike_box = BIKE_BOX;
        Riding { actor, drawn: None, was: None, rng: Rng::new() }
    }

    /// **One tick**: the skeleton's points and rests off `posed` (or the rig's fixed riding pose)
    /// carried by the request's frame, then the actor's update. `clip` is the vehicle animation's
    /// state, which the look-around's feet switch reads.
    pub fn step(&mut self, kit: &Kit, posed: Option<&Posed>, r: &Request, clip: i32, dt: F, world: &mut dyn World) -> Tail {
        let now = ragdoll_ride::nodes(kit, posed, &r.frame);
        let mut points = [UNIT_W; PARTS];
        for ((p, b), part) in points.iter_mut().zip(&now).zip(&kit.rig.parts) {
            let (mut drawn, mut start) = (Mat4([Vf::ZERO; 4]), Start::default());
            ragdoll_pose::pose(b, part.cofg, &mut drawn, &mut start, None, &kit.tables.acos, &kit.tables.curve);
            *p = start.position;
        }
        let mut rests = [UNIT_W; PARTS];
        for (i, rest) in rests.iter_mut().enumerate() {
            *rest = posed.and_then(|p| p.rests.get(i)).or(kit.rig.parts.get(i).map(|p| &p.rest)).copied().unwrap_or(UNIT_W);
        }
        let skeleton = Skeleton { points, rests, main_rest: rests[MAIN] };
        self.actor.clip = clip;
        let was = self.history(kit);
        let mut pose = Pose { kit, now: &now, before: self.drawn.as_deref() };
        let tail = self.actor.tick(
            dt,
            r,
            &skeleton,
            &kit.tables,
            &kit.wrecks,
            Scenery { spheres: &kit.spheres, world, terrain: (TERRAIN_MIN, TERRAIN_MAX), scene: &mut Open },
            &mut self.rng,
            &mut pose,
        );
        self.drawn = Some(ragdoll_ride::drawn(&now));
        self.was = was;
        tail
    }

    /// **One tick on a rider** (S110): the request off the ride and `holds` (carried by the ride's
    /// transform, as the shell carried the drawn chassis' grips), the clip-table switch and the clip
    /// off the body's machine, then [`Riding::step`] with the skeleton the shell last posed.
    pub fn ride(&mut self, kit: &Kit, rider: &crate::rider::Rider, holds: &Holds, dt: F, world: &mut dyn World) -> Tail {
        let m = rider.ride.transform();
        let world_of = |p: Vf| math::transform_point(&m, p);
        let frame = math::mul(&holds.root, &m);
        let r = request(&rider.ride, rider.packed_pose, frame, holds.hands.map(world_of), holds.feet.map(world_of), kit.bike);
        self.posed_off_the_table(rider.body.machine.mode == crate::rider_anim::Mode::Posed);
        self.step(kit, rider.skeleton.as_deref(), &r, rider.body.veh.state, dt, world)
    }

    /// **One tick of the actor by another hand** (S110): `tick` runs it, and the bodies' nodes as they
    /// were drawn before it are kept for the wreck entry, as [`Riding::step`] keeps them. What a test
    /// steps the actor with the original's own skeleton and scene through.
    pub fn stepped<T>(&mut self, kit: &Kit, tick: impl FnOnce(&mut Actor) -> T) -> T {
        let was = self.history(kit);
        let out = tick(&mut self.actor);
        self.was = was;
        out
    }

    /// The bodies' nodes' drawn matrices before a step, while the body is placed.
    fn history(&self, kit: &Kit) -> Option<Vec<Mat4>> {
        self.placed().then(|| ragdoll_ride::drawn(&self.bones(kit)))
    }

    /// The main bone has been placed more than [`STRAY`] from `at`: the bike went where the bones
    /// did not (a respawn), and the body is started again.
    pub fn strayed(&self, at: Vf) -> bool {
        let d = self.main().sub(at);
        let (x, y, z) = (F(d.x()), F(d.y()), F(d.z()));
        self.placed() && x * x + y * y + z * z > STRAY * STRAY
    }

    /// **Each part's node as the skeleton's bone** (S70's hand-over): [`Riding::nodes`]' position and
    /// the normalised orientation's forward and up rows, which is what the original's wreck entry
    /// reads off the riding skeleton after a step wrote it back.
    pub fn bones(&self, kit: &Kit) -> Vec<Bone> {
        bones(&self.actor.ragdoll, kit)
    }

    /// **What a wreck enters from** (S110): the bodies' nodes now, and their drawn matrices before
    /// the last step, which the entry's pose measures each bone's velocity and spin from. `None`
    /// while the body is not placed, and the history `None` until it has been stepped placed.
    pub fn hand_over(&self, kit: &Kit) -> Option<(Vec<Bone>, Option<&[Mat4]>)> {
        self.placed().then(|| (self.bones(kit), self.was.as_deref()))
    }

    /// **A pose off the clip table is in charge** (V61): the two words the actor's mode reads for
    /// it ([`Actor::posed`], [`Actor::latched`]) are the vehicle animation's `from_table` and
    /// `holding`, which the body's own machine reads at the same point of the tick
    /// ([`crate::rider_anim::Body::mode`]). So `posed` is whether that machine chose
    /// [`crate::rider_anim::Mode::Posed`] this tick, and while it is the ragdoll is not placed: a
    /// gesture, a no-hander or a backflip is drawn as its clip wrote it, with nothing held.
    pub fn posed_off_the_table(&mut self, posed: bool) {
        self.actor.posed = posed;
        self.actor.latched = false;
    }

    /// The bones have been placed and stepped: [`Riding::nodes`] is the rider.
    pub fn placed(&self) -> bool {
        self.actor.placed
    }

    /// **Each part's node in the world**, as the original writes the skeleton back after the step:
    /// the body's orientation, normalised, at its position less its centre of mass turned by it.
    pub fn nodes(&self, kit: &Kit) -> Vec<Mat4> {
        self.actor
            .ragdoll
            .bones
            .iter()
            .zip(&kit.rig.parts)
            .map(|(b, p)| {
                let (at, q) = ragdoll_ride::node_of(b.body.position, b.body.quat, p.cofg);
                math::quat_to_mat4(q, at)
            })
            .collect()
    }

    /// The main bone's position.
    pub fn main(&self) -> Vf {
        self.actor.ragdoll.bones[MAIN].body.position
    }
}

/// **The riding bodies' nodes as skeleton bones** (S110): each body's node ([`ragdoll_ride::node_of`])
/// with the forward and up rows of its normalised orientation, the form the wreck entry's pose reads.
pub fn bones(ragdoll: &Ragdoll, kit: &Kit) -> Vec<Bone> {
    ragdoll
        .bones
        .iter()
        .zip(&kit.rig.parts)
        .map(|(b, p)| {
            let (at, q) = ragdoll_ride::node_of(b.body.position, b.body.quat, p.cofg);
            let m = math::quat_to_mat4(q, at);
            Bone { at, forward: m.0[2], up: m.0[1] }
        })
        .collect()
}

impl crate::hash::State for Riding {
    fn walk(&self, w: &mut crate::hash::Walk<'_>) {
        w.bool("placed", self.actor.placed);
        w.bool("repose", self.actor.repose);
        w.bool("touching", self.actor.touching);
        for (i, h) in self.actor.ragdoll.holds.iter().enumerate() {
            w.bool(["hold0", "hold1", "hold2", "hold3"][i], *h);
        }
        for b in &self.actor.ragdoll.bones {
            w.vf("position", b.body.position);
            w.vf("velocity", b.body.velocity);
            w.vf("quat", b.body.quat);
            w.vf("rates", b.body.rates);
        }
    }
}

/// **What the bike hands the riding body** (S111): the original's request writer copies every field
/// out of the rider's state pack, which runs after this tick's physics and before the animation
/// header. So the bike's numbers are this tick's and `pitch` is the pose the header showed last tick.
///
/// `frame` is the drawn bike's root node ("Bike", the chassis node's child) in the world, which is
/// the frame the rates are turned into and the bike box sits in. `pitch` is the rider's pose axis
/// ([`crate::motion::Driver::pose`]) as it was before this tick's header, `hands` and `feet` the
/// four holds' points in the world (left then right, the grips and the pegs), `bike` the collider.
pub fn request(ride: &Ride, pitch: F, frame: Mat4, hands: [Vf; 2], feet: [Vf; 2], bike: Option<(Vf, F)>) -> Request {
    let sources = Sources {
        chassis: ride.transform(),
        frame,
        velocity: ride.chassis.vel,
        world_rates: ride.chassis.world_ang_vel,
        speed: F(ride.air.contact_vel.z()),
        on_ground: ride.air.on_ground,
        flipped: ride.air.flip_torque_applied,
        pitch,
    };
    sources.request(hands, feet, bike)
}

/// The rider's numbers the request is built out of, each the state pack's copy of one field.
#[derive(Clone, Copy, Debug)]
pub struct Sources {
    /// The chassis node's transform in the world: what the rows, the centre and the quat come from.
    pub chassis: Mat4,
    /// The drawn bike's root node in the world.
    pub frame: Mat4,
    /// The chassis' velocity in the world.
    pub velocity: Vf,
    /// The chassis' angular velocity in the world, which the request carries in `frame`'s axes.
    pub world_rates: Vf,
    /// The contact velocity's forward lane: the touching wheel's, or the chassis' own when neither
    /// touches ([`crate::air::Air::contact_vel`]). Not the speed as a length.
    pub speed: F,
    pub on_ground: bool,
    /// A backflip's torque went in on the tick's last substep. The actor latches its pose off the clip
    /// table on it, so a backflip is drawn as its clip wrote it, with nothing held.
    pub flipped: bool,
    pub pitch: F,
}

impl Sources {
    /// The request, field by field as the original's writer fills it. The reason is 0 (a wrecked
    /// rider has no riding body), so the hit velocity is the velocity. The reverse flag stays off: the
    /// field it copies is the reverse command, which no pad of the original's sets (P73).
    pub fn request(&self, hands: [Vf; 2], feet: [Vf; 2], bike: Option<(Vf, F)>) -> Request {
        let m = &self.chassis;
        let f = &self.frame;
        let at = m.0[3];
        Request {
            rows: [m.0[0], m.0[1], m.0[2]],
            frame: *f,
            velocity: self.velocity,
            rates: chassis::unrotate(&Mat3::new(f.0[0], f.0[1], f.0[2]), self.world_rates),
            hands,
            feet,
            looking: !self.on_ground,
            posed: self.flipped,
            hit_velocity: self.velocity,
            pitch: self.pitch,
            speed: self.speed,
            centre: Vf::new(at.x(), at.y(), at.z(), 1.0),
            quat: math::mat4_to_quat(m),
            bike,
            ..Request::default()
        }
    }
}

/// The actor's outside for the riding body: a pose puts every bone where the animated skeleton has
/// it, moving from last frame's drawn matrices on an entry, still on a repose (`carried` is the
/// original's `a1`, which stills the pose: S110), and still with no last frame.
struct Pose<'a> {
    kit: &'a Kit,
    now: &'a [Bone],
    before: Option<&'a [Mat4]>,
}

impl Outside for Pose<'_> {
    fn pose(&mut self, ragdoll: &mut Ragdoll, dt: F, carried: bool) {
        let t = &self.kit.tables;
        let mut starts = [Start::default(); PARTS];
        let mut drawn = [Mat4([Vf::ZERO; 4]); PARTS];
        let before = self.before.filter(|_| !carried);
        if let Some(before) = before {
            for (d, b) in drawn.iter_mut().zip(before) {
                *d = *b;
            }
        }
        ragdoll_pose::pose_all(
            self.now.iter().zip(&self.kit.rig.parts).zip(drawn.iter_mut().zip(starts.iter_mut())).map(|((b, p), (d, s))| (b, p.cofg, d, s)),
            before.is_none(),
            dt,
            &t.acos,
            &t.curve,
        );
        ragdoll_pose::reset(ragdoll.bones.iter_mut().map(|b| &mut b.body).zip(starts.iter()));
    }
}
