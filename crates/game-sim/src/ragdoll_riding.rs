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
//! animated pose. It draws only: nothing of the ride reads it back, and its generator is its own.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

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
const STAND_SPEED: F = F(f32::from_bits(0x40ea_aa65));

/// The speed under which the look-around may lift a foot.
const SLOW_SPEED: F = F(f32::from_bits(0x4292_aa7f));

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
/// which a carried repose measures the bones' velocity from.
#[derive(Clone, Debug)]
pub struct Riding {
    pub actor: Actor,
    drawn: Option<Vec<Mat4>>,
    rng: Rng,
}

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
        Riding { actor, drawn: None, rng: Rng::new() }
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
        tail
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

/// **What the bike hands the riding body**: the ride's orientation (as rows and as the quat), its
/// drawn transform for the bike box, its velocity and its rates in its own frame, its centre of
/// gravity for the spin, and the four holds' points in the world (`hands` left then right at the
/// grips, `feet` at the pegs). `pitch` is the vehicle animation's fore-and-aft mix, `bike` the bike's
/// collider.
pub fn request(ride: &Ride, pitch: F, hands: [Vf; 2], feet: [Vf; 2], bike: Option<(Vf, F)>) -> Request {
    let c = &ride.chassis;
    let m = math::quat_to_mat4(c.quat, Vf::ZERO);
    Request {
        rows: [m.0[0], m.0[1], m.0[2]],
        frame: ride.transform(),
        velocity: c.vel,
        rates: c.ang_vel,
        hands,
        feet,
        pitch,
        speed: F(c.local_vel.z()),
        centre: c.cofg_world,
        quat: c.quat,
        bike,
        ..Request::default()
    }
}

/// The actor's outside for the riding body: a pose puts every bone where the animated skeleton has
/// it, still on the entry, carried from last frame's drawn matrices on a repose.
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
        let before = self.before.filter(|_| carried);
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
