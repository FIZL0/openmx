//! **The ragdoll in the ride** (S53): the rider's eighteen bones run while the rider is down, in
//! place of [`crate::ride::Crash`]'s one rider point.
//!
//! [`crate::ragdoll`] has the bones, the joints, the two entries and the step, each proved on its
//! own against the original. This module is what puts them in a ride:
//!
//! * [`Rig`]: what the ragdoll needs of the rider's skeleton, which this crate does not load. That
//!   is each part's node translation, its centre of mass, its node's pose in the bike's frame, and
//!   its rest orientation, plus the two hand dummies. A shell builds it once from the model and the
//!   riding clip and hands it over.
//! * [`load_pins`]: the joints' and the holds' pins, as the generic physics loader and the body
//!   actor's constructor build them. The anchors come from the skeleton and the centres of mass, and
//!   the twist limits are the file's angles, halved and taken through the engine's sine.
//! * [`Kit`]: the ragdoll as it is when the rider gets on: the records read, the pins loaded and
//!   the riding entry run. Plus the nine collision spheres and the tables the step reads.
//! * [`Down`]: one wreck's ragdoll. It is posed where the riding body is, reset, and handed to
//!   [`Ragdoll::wreck_enter`] with the game's generator. Then it is stepped each frame with its bone
//!   collisions against the ground, the hands held to the tumbling wreck bike until they let go.
//!   It hands the mode machine its sensors and its clocks.
//!
//! # What is the port's own
//!
//! The pose is fixed. The original poses the bones from the live animated skeleton at the frame
//! of the wreck. This port has no skeleton in the simulation, so the shell hands over one riding
//! pose ([`Rig`]), and the wreck starts from it wherever the bike is. The bones start with the
//! bike's velocity and no spin, where the original takes both from the skeleton's last frame. The
//! holds follow the points where the hands were on the wreck bike, carried with its body; the
//! original's follow the bike's own dummies. The rest of what is not 1:1 is listed on each item.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::biplane::{Body, World as Stepped};
use crate::camera::CurveTable;
use crate::chassis::{self, Mat3, turn};
use crate::math::{self, Mat4};
use crate::phy::{Ini, Throw, Wrecks};
use crate::ragdoll::{Among, Bike, Carrier, Drive, HOLD_BONES, Launch, MAIN, PARTS, Ragdoll, Records, Tables};
use crate::ragdoll_contact::{Contact, Open, Sphere};
use crate::ragdoll_pose::{self, Bone, Start};
use crate::rider_anim::{Clock, Pool, Thrown};
use crate::rng::Rng;
use crate::world::World;
use ps2_float::{Dest, F, Vf, fpu};

/// The degrees the file's angles are in, as the loader scales them: the same `0.01745329` as
/// [`crate::ragdoll`]'s `MaxJointTorqueAngle`.
const DEGREES: F = F(f32::from_bits(0x3c8e_fa34));

/// What a pin's twist limits are before the loader writes any: the pin constructor's.
const NO_LIMIT: f32 = -f32::MAX;

/// `(0, 0, 0, 1)`.
const UNIT_W: Vf = Vf([0.0, 0.0, 0.0, 1.0]);

/// The body actor's mode while wrecked, which the joint torques fade and share in.
const WRECK_MODE: i32 = 2;

/// The terrain's box the bone collisions ask the ground inside. The original's is the track's;
/// the port's world answers everywhere.
const TERRAIN_MIN: Vf = Vf([-1.0e30, -1.0e30, -1.0e30, 1.0]);
const TERRAIN_MAX: Vf = Vf([1.0e30, 1.0e30, 1.0e30, 1.0]);

/// **One part of the riding skeleton**, as the ragdoll reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RigPart {
    /// The part's node's translation in its parent node's frame: what a joint's first anchor is
    /// worked out from.
    pub local: Vf,
    /// The part's centre of mass in its node's frame. `DefaultCG = T` (every retail part but the
    /// arms', torso's and legs' that say `F`) is the model's bounding centre for the node; `F` is the
    /// file's `CG`.
    pub cofg: Vf,
    /// The node's world pose in the bike's frame, in the riding pose.
    pub bone: Bone,
    /// The node's rest orientation in its parent's frame, in the riding pose: what the joint
    /// torques pull each bone back towards.
    pub rest: Vf,
}

/// **The riding skeleton**, as much of it as the ragdoll reads. The shell builds it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rig {
    /// One per part, in the file's order.
    pub parts: Vec<RigPart>,
    /// The two hand dummies' translations in their hands' frames, left then right: the points the
    /// hand holds pin.
    pub hands: [Vf; 2],
}

impl Rig {
    /// Where each part's body sits in the bike's frame: the node's position plus its centre of mass,
    /// turned by its orientation.
    fn place(&self, m: &Mat4, part: usize) -> Bone {
        let b = &self.parts[part].bone;
        Bone { at: math::transform_point(m, b.at), forward: rotate(m, b.forward), up: rotate(m, b.up) }
    }
}

/// `v` turned by `m`'s three rows, its translation left out.
fn rotate(m: &Mat4, v: Vf) -> Vf {
    chassis::rotate(&Mat3::new(m.0[0], m.0[1], m.0[2]), v)
}

/// `p` taken back into `m`'s frame.
fn unpoint(m: &Mat4, p: Vf) -> Vf {
    let d = p.merge(Dest::XYZ, p.sub(m.0[3]));
    let v = chassis::unrotate(&Mat3::new(m.0[0], m.0[1], m.0[2]), d);
    Vf::new(v.x(), v.y(), v.z(), 1.0)
}

/// **The pins**, as the generic physics loader builds a joint and the body actor's constructor
/// builds a hold.
///
/// A joint (`PinToPart`) with `DefaultPinPositions = T` pins its first bone at the second bone's
/// node translation, less the first bone's centre of mass, to the second bone at its centre of mass
/// negated. With `F` it reads `PinPosition1` and `PinPosition2` instead. The twist limits are
/// `MinAngles` and `MaxAngles`, in degrees, halved and taken through the engine's sine
/// ([`CurveTable::sine`]). That happens only when every lane's minimum is at or under its maximum,
/// and then the lower limit's `w` is zeroed. Otherwise both stay at the constructor's `-FLT_MAX`.
///
/// A hold pins a hand at its dummy's translation less the hand's centre of mass, and a foot at its
/// centre of mass, to a point in the world.
pub fn load_pins(ini: &Ini, ragdoll: &mut Ragdoll, rig: &Rig, curve: &CurveTable) {
    let none = Vf::splat(NO_LIMIT);
    let cofg = |part: usize| rig.parts.get(part).map_or(UNIT_W, |p| p.cofg);
    for (h, &bone) in HOLD_BONES.iter().enumerate() {
        let pin = &mut ragdoll.pins[h];
        let at = cofg(bone);
        pin.anchors = [if h < 2 { sub_w1(rig.hands[h], at) } else { at }, UNIT_W];
        pin.lower = none;
        pin.upper = none;
    }
    let constraints = ini.section("Physics").int("NumberOfConstraints").unwrap_or(0).max(0) as usize;
    let mut j = 0;
    for i in 1..=constraints {
        let s = ini.section(&format!("Constraint{i}"));
        if !s.text("Type").is_some_and(|t| t.eq_ignore_ascii_case("PinToPart")) {
            continue;
        }
        let Some(r) = ragdoll.records.joints.get(j).copied() else { break };
        let pin = &mut ragdoll.pins[4 + j];
        j += 1;
        let (c1, c2) = (cofg(r.part1), cofg(r.part2));
        pin.anchors = if s.flag("DefaultPinPositions", true) {
            let local = rig.parts.get(r.part2).map_or(UNIT_W, |p| p.local);
            [sub_w1(local, c1), Vf::new(fpu::neg(c2.x()), fpu::neg(c2.y()), fpu::neg(c2.z()), c2.w())]
        } else {
            [s.vector("PinPosition1", UNIT_W), s.vector("PinPosition2", UNIT_W)]
        };
        pin.lower = none;
        pin.upper = none;
        let min = s.vector("MinAngles", UNIT_W).mul(Vf::splat(DEGREES.0));
        let max = s.vector("MaxAngles", UNIT_W).mul(Vf::splat(DEGREES.0));
        if (0..3).all(|k| F(min[k]) <= F(max[k])) {
            pin.lower[3] = 0.0;
            for k in 0..3 {
                pin.lower[k] = curve.sine(F(min[k]) * F(0.5)).0;
                pin.upper[k] = curve.sine(F(max[k]) * F(0.5)).0;
            }
        }
    }
}

/// `a - b` in `x`, `y`, `z`, and a one in `w`.
fn sub_w1(a: Vf, b: Vf) -> Vf {
    let d = a.sub(b);
    Vf::new(d.x(), d.y(), d.z(), 1.0)
}

/// **The ragdoll as the rider gets on**: everything a wreck's ragdoll is copied from.
pub struct Kit {
    pub rig: Rig,
    /// Built from the file, with the pins loaded, each bone's centre of mass in its body, and the
    /// riding entry run.
    pub ragdoll: Ragdoll,
    /// The nine bone collisions, in the file's order.
    pub spheres: Vec<Sphere>,
    /// The riding pose's spring and damper, `[Joint Forces]`.
    pub pose_spring: Vf,
    pub pose_damping: Vf,
    pub tables: Tables,
}

impl std::fmt::Debug for Kit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Kit").field("rig", &self.rig).field("spheres", &self.spheres).finish_non_exhaustive()
    }
}

impl Kit {
    /// **The main bone's centre of mass in the world**, in the riding pose on the bike at `m` (the
    /// ride's transform): where [`Down::enter`] would put it. The empty vector for a rig without one.
    pub fn main_bone(&self, m: &Mat4) -> Vf {
        let Some(part) = self.rig.parts.get(MAIN) else {
            return UNIT_W;
        };
        let mut drawn = Mat4([Vf::ZERO; 4]);
        let mut start = Start::default();
        ragdoll_pose::pose(&self.rig.place(m, MAIN), part.cofg, &mut drawn, &mut start, None, &self.tables.acos, &self.tables.curve);
        start.position
    }

    /// The kit out of the rider file's bytes (`rider\anim\mrider.phy`) and the skeleton's riding
    /// pose.
    pub fn new(rider_file: &[u8], rig: Rig) -> Kit {
        let ini = Ini::parse_bytes(rider_file);
        let tables = Tables::build();
        let mut ragdoll = Ragdoll::new(Records::read(&ini));
        for (b, p) in ragdoll.bones.iter_mut().zip(&rig.parts) {
            b.body.cofg = p.cofg;
        }
        load_pins(&ini, &mut ragdoll, &rig, &tables.curve);
        ragdoll.ride_enter(true);
        let forces = ini.section("Joint Forces");
        Kit {
            spheres: Sphere::read(&ini),
            pose_spring: forces.vector("SpringForce", UNIT_W),
            pose_damping: forces.vector("DampingForce", UNIT_W),
            rig,
            ragdoll,
            tables,
        }
    }
}

/// The game's generator as the wreck entry draws from it: the one the body's idle pool keeps,
/// seeded first if nothing has seeded it yet. The pool's seed is the wall clock the shell hands
/// the body; this has none, and seeds a pool nothing has drawn from with the zero clock.
pub fn generator(pool: &mut Pool) -> &mut Rng {
    if !pool.seeded {
        pool.rng.seed(Clock::default().seed());
        pool.seeded = true;
    }
    &mut pool.rng
}

/// **One wreck's ragdoll.**
#[derive(Clone, Debug, PartialEq)]
pub struct Down {
    /// The reason the wreck entered with: what the damping switch reads.
    pub reason: i32,
    pub ragdoll: Ragdoll,
    /// One per bone collision. Each frame starts with none touched.
    pub contacts: Vec<Contact>,
    /// Where each hold holds, in the wreck bike's frame: where it was when the wreck entered.
    pub held_at: [Vf; 4],
    /// What the mode machine's settle test reads, as the last step left it.
    pub sensed: Thrown,
}

impl Down {
    /// **The wreck entered**: the bones put where the riding pose has them on the bike at `m` (the
    /// ride's transform), reset, and handed to the wreck entry for `reason`. The bike's numbers are
    /// the wreck body's, which `crash` has just been entered with.
    ///
    /// Every bone starts with the bike's velocity and no rates. The entry then gives some reasons
    /// their own.
    #[allow(clippy::too_many_arguments)]
    pub fn enter(
        kit: &Kit,
        m: &Mat4,
        crash: &crate::ride::Crash,
        throw: Throw,
        reason: i32,
        wrecks: &Wrecks,
        rng: &mut Rng,
        world: &mut dyn World,
    ) -> Down {
        let mut ragdoll = kit.ragdoll.clone();
        let t = &kit.tables;
        let mut starts = [Start::default(); PARTS];
        let mut drawn = [Mat4([Vf::ZERO; 4]); PARTS];
        let places: Vec<Bone> = (0..kit.rig.parts.len().min(PARTS)).map(|i| kit.rig.place(m, i)).collect();
        ragdoll_pose::pose_all(
            places.iter().zip(&kit.rig.parts).zip(drawn.iter_mut().zip(starts.iter_mut())).map(|((b, p), (d, s))| (b, p.cofg, d, s)),
            true,
            F::ZERO,
            &t.acos,
            &t.curve,
        );
        ragdoll_pose::reset(ragdoll.bones.iter_mut().map(|b| &mut b.body).zip(starts.iter()));
        for b in &mut ragdoll.bones {
            b.body.velocity = crash.body.vel;
        }
        let bike = Bike { velocity: crash.body.vel, rates: crash.body.ang_vel, hit_velocity: crash.body.vel };
        ragdoll.wreck_enter(reason, &bike, wrecks, rng);
        let frame = math::quat_to_mat4(crash.body.quat, crash.origin);
        let held_at = std::array::from_fn(|h| {
            let pin = &ragdoll.pins[h];
            let b = &ragdoll.bones[pin.first].body;
            unpoint(&frame, add_xyz(b.position, turn(b.quat, pin.anchors[0])))
        });
        let mut down = Down {
            reason,
            contacts: vec![Contact::default(); kit.spheres.len()],
            ragdoll,
            held_at,
            sensed: Thrown { throw, ..Thrown::default() },
        };
        down.sense(world);
        down
    }

    /// **One frame of it**: the throw's clock, then the world stepped with the bone collisions on the
    /// ground, the hands that still hold following the wreck bike.
    pub fn step(&mut self, kit: &Kit, dt: F, crash: &crate::ride::Crash, world: &mut dyn World) {
        let frame = math::quat_to_mat4(crash.body.quat, crash.origin);
        self.ragdoll.throw_clock(dt, crash.body.quat);
        let drive = Drive {
            bike: Carrier {
                rows: [frame.0[0], frame.0[1], frame.0[2]],
                spin: F::ZERO,
                velocity: crash.body.vel,
                centre: crash.body.cofg_world,
                quat: crash.body.quat,
            },
            rests: std::array::from_fn(|i| kit.rig.parts.get(i).map_or(UNIT_W, |p| p.rest)),
            hold_targets: self.held_at.map(|p| math::transform_point(&frame, p)),
            hold_velocity: crash.body.vel,
            gravity_scale: F::ONE,
            pose_spring: kit.pose_spring,
            pose_damping: kit.pose_damping,
            torques: true,
            mode: WRECK_MODE,
            fade_length: F::ONE,
            fade_left: F::ZERO,
            ..Drive::default()
        };
        for c in &mut self.contacts {
            c.touched = false;
        }
        self.ragdoll.step_among(
            dt,
            &drive,
            &kit.tables,
            Among { spheres: &kit.spheres, contacts: &mut self.contacts, terrain: Some((&mut *world, TERRAIN_MIN, TERRAIN_MAX)), scene: &mut Open, bike_box: None },
        );
        self.sense(world);
    }

    /// The sensors the mode machine reads off the bones, and the ragdoll's own clocks.
    fn sense(&mut self, world: &mut dyn World) {
        let t = Thrown::from_bones(self.sensed.throw, &self.contacts, &self.ragdoll.bones[MAIN].body, world);
        self.sensed = Thrown { launch: Some(self.ragdoll.launch), ..t };
    }

    /// **The settle gate**: both hands have let go of the bars and a bone is on the ground.
    ///
    /// **Inferred.** The gate's writer in the original was not found. This keeps what S26 measured
    /// it to follow, the hands off and then the rider on the ground, read off the bones in place of
    /// the one rider point.
    pub fn settles(&self) -> bool {
        !self.ragdoll.holds[0] && !self.ragdoll.holds[1] && self.contacts.iter().any(|c| c.touched)
    }

    /// The main bone.
    pub fn main(&self) -> &Body {
        &self.ragdoll.bones[MAIN].body
    }

    /// **Each part's node in the world**, for drawing the rider from the bones: the body's
    /// orientation, at its position less its centre of mass turned by it.
    pub fn nodes(&self, kit: &Kit) -> Vec<Mat4> {
        self.ragdoll
            .bones
            .iter()
            .zip(&kit.rig.parts)
            .map(|(b, p)| {
                let q = b.body.quat;
                let at = b.body.position.sub(turn(q, p.cofg));
                math::quat_to_mat4(q, at)
            })
            .collect()
    }

    /// The world the step runs in, for a caller that wants to know.
    pub fn world(&self) -> Stepped {
        self.ragdoll.world
    }

    /// What the throw was drawn as.
    pub fn launch(&self) -> Launch {
        self.ragdoll.launch
    }
}

fn add_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.add(b))
}

impl crate::hash::State for Down {
    fn walk(&self, w: &mut crate::hash::Walk<'_>) {
        w.i32("reason", self.reason);
        for (i, h) in self.ragdoll.holds.iter().enumerate() {
            w.bool(["hold0", "hold1", "hold2", "hold3"][i], *h);
        }
        w.bool("throwing", self.ragdoll.throwing);
        w.f("force_time", self.ragdoll.launch.force_time);
        w.f("hand_hold", self.ragdoll.launch.hand_hold);
        w.bool("stiff", self.ragdoll.stiff);
        for b in &self.ragdoll.bones {
            w.vf("position", b.body.position);
            w.vf("velocity", b.body.velocity);
            w.vf("quat", b.body.quat);
            w.vf("rates", b.body.rates);
        }
        for c in &self.contacts {
            w.bool("touched", c.touched);
        }
    }
}
