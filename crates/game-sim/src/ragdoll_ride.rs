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
//! The pose is the shell's. The original poses the bones from the live animated skeleton at the
//! frame of the wreck. This port has no skeleton in the simulation, so the shell hands over one
//! riding pose ([`Rig`]) and, where it draws the body, the pose it last drew ([`Posed`], S65); the
//! wreck starts from that wherever the bike is. The bones take their velocity and spin from how
//! far that pose's nodes moved since the tick before (S70), as the original's do from its
//! skeleton's; the original's skeleton is its riding ragdoll's bodies, which the port does not
//! run while riding, so the motion is the shell's pose's and the bike's, not the original's. The
//! holds follow the points where the hands were on the wreck bike, carried with its body; the
//! original's follow the bike's own dummies. The rest of what is not 1:1 is listed on each item.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::biplane::{Body, World as Stepped};
use crate::camera::CurveTable;
use crate::chassis::{self, Mat3, normalize, turn};
use crate::math::{self, Mat4};
use crate::phy::{Ini, Throw, Wrecks};
use crate::ragdoll::{Among, Bike, Carrier, Drive, HOLD_BONES, Launch, MAIN, PARTS, Ragdoll, Records, Tables};
use crate::ragdoll_body::{self, Fade};
use crate::ragdoll_contact::{Contact, Open, Sphere};
use crate::ragdoll_scene::{BikeSearch, Cells, WreckBikeScene};
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
    /// `[Wrecks]`, for the wreck's settle test: the flail height, the roll's time and speed, and
    /// the main bone's damping at rest.
    pub wrecks: Wrecks,
    /// The wreck bike's collision sphere, its centre in the bike's frame and its radius, which the
    /// bones are off the bike once the main bone is a foot past (S65). [`Kit::new`] leaves it
    /// `None`, and the bones never leave the bike; a shell sets it from [`bike_sphere`].
    pub bike: Option<(Vf, F)>,
    /// The wreck bike's collision mesh, the scene the bones' spheres are tested against (S105).
    /// [`Kit::new`] leaves it `None`, and the scene is open; a shell sets it from
    /// [`crate::ragdoll_scene::Bike::parse`].
    pub bike_mesh: Option<std::sync::Arc<crate::ragdoll_scene::Bike>>,
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
            wrecks: Wrecks::read(&ini),
            bike: None,
            bike_mesh: None,
            rig,
            ragdoll,
            tables,
        }
    }
}

/// **The wreck bike's collision sphere** out of its collision file (`wreckbikef.col`), as the
/// collision object the body's request points at holds it (S65): the root node's centre, and its
/// box's half extent's length for the radius, through the FPU. `None` when the file does not read
/// as a mesh or has no nodes.
pub fn bike_sphere(col: &[u8]) -> Option<(Vf, F)> {
    let mesh = crate::track::Mesh::parse(col).ok()?;
    let root = mesh.nodes.first()?;
    let [x, y, z] = root.centre;
    let [hx, hy, hz] = root.half_extent;
    let acc = fpu::mula(hx.0, hx.0);
    let acc = fpu::madda(acc, hy.0, hy.0);
    let radius = F(fpu::sqrt(fpu::madd(acc, hz.0, hz.0)));
    Some((Vf::new(x.0, y.0, z.0, 1.0), radius))
}

/// **The bike sphere's centre in the world**: `centre` (in the bike's frame) through the wreck
/// bike's frame `m`, as the collision object places it. The translation goes in first and then the
/// three rows, which is not [`math::transform_point`]'s order and differs from it in the last bit.
pub fn bike_centre(m: &Mat4, centre: Vf) -> Vf {
    let acc = m.0[3].mul(Vf::splat(1.0));
    let acc = Vf::madd(acc, m.0[0], Vf::splat(centre.x()));
    let acc = Vf::madd(acc, m.0[1], Vf::splat(centre.y()));
    let p = Vf::madd(acc, m.0[2], Vf::splat(centre.z()));
    Vf::new(p.x(), p.y(), p.z(), 1.0)
}

/// **A riding part's node, off its bone body** (S70): where the original's riding ragdoll puts the
/// skeleton after each step. The body's orientation is normalised in place (through the reciprocal
/// square root), and the node sits at the body's position less its centre of mass turned by it.
/// Returns the node's position and the normalised orientation.
pub fn node_of(position: Vf, quat: Vf, cofg: Vf) -> (Vf, Vf) {
    let q = normalize(quat);
    (position.sub(turn(q, cofg)), q)
}

/// **The skeleton's nodes in the world**, as the wreck entry's pose reads them: `posed`'s (or,
/// without one, the [`Rig`]'s fixed riding pose) carried by the bike's transform `m`.
pub fn nodes(kit: &Kit, posed: Option<&Posed>, m: &Mat4) -> Vec<Bone> {
    (0..kit.rig.parts.len().min(PARTS))
        .map(|i| match posed.and_then(|p| p.bones.get(i)) {
            Some(b) => Bone { at: math::transform_point(m, b.at), forward: rotate(m, b.forward), up: rotate(m, b.up) },
            None => kit.rig.place(m, i),
        })
        .collect()
}

/// **Each node's drawn matrix**: the world matrix the wreck entry's pose measures the node's move
/// and turn from, built from the node's position and two axes as the pose builds its own.
pub fn drawn(nodes: &[Bone]) -> Vec<Mat4> {
    nodes.iter().map(|b| math::basis(b.at, b.forward, b.up, false, true)).collect()
}

/// **The skeleton as the shell last posed it** (S65): each part's node in the bike's frame and its
/// rest orientation, the same two things [`RigPart::bone`] and [`RigPart::rest`] hold for the fixed
/// riding pose. A wreck that has one is posed from it and its step pulls towards its rests, as the
/// original's are from the live animated skeleton; one without falls back on the [`Rig`]'s.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Posed {
    /// One per part, in the file's order.
    pub bones: Vec<Bone>,
    pub rests: Vec<Vf>,
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
    /// The torques' fade, which every stiffening restarts, and whether a bone was touching when the
    /// settle test last looked (S61).
    pub fade: Fade,
    pub touching: bool,
    /// What the bones' search holds of the wreck bike (S105).
    pub search: BikeSearch,
}

impl Down {
    /// **The wreck entered**: the bones put where the riding pose has them on the bike at `m` (the
    /// ride's transform), reset, and handed to the wreck entry for `reason`. The bike's numbers are
    /// the wreck body's, which `crash` has just been entered with.
    ///
    /// With no skeleton history every bone starts with the bike's velocity and no rates (the
    /// port's own; [`Down::enter_from`] is the original's hand-over). The entry then gives some
    /// reasons their own.
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
        Down::enter_posed(kit, None, m, crash, throw, reason, wrecks, rng, world)
    }

    /// [`Down::enter`], the bones put where `posed` has them when there is one (S65). With no
    /// previous frame, the bones start with the bike's velocity and no rates: the port's own, for a
    /// wreck with no skeleton history (see [`Down::enter_from`]).
    #[allow(clippy::too_many_arguments)]
    pub fn enter_posed(
        kit: &Kit,
        posed: Option<&Posed>,
        m: &Mat4,
        crash: &crate::ride::Crash,
        throw: Throw,
        reason: i32,
        wrecks: &Wrecks,
        rng: &mut Rng,
        world: &mut dyn World,
    ) -> Down {
        Down::enter_from(kit, &nodes(kit, posed, m), None, F::ZERO, crash, throw, reason, wrecks, rng, world)
    }

    /// **The wreck entered from the riding skeleton** (S70): the original's wreck entry, which
    /// poses every bone where the skeleton's node is now (`now`, in the world) and takes its
    /// velocity and rates from how far the node moved and turned since the node's matrix was last
    /// drawn (`before`, the same nodes one tick earlier), over the tick's `dt`. That is the
    /// riding pose's motion, not the bike's: the original hands no bone the bike's velocity
    /// here, and only some reasons' throws do afterwards.
    ///
    /// Without `before` the bones start still, then take the bike's velocity, which is what the
    /// port did before it kept the skeleton's last frame.
    #[allow(clippy::too_many_arguments)]
    pub fn enter_from(
        kit: &Kit,
        now: &[Bone],
        before: Option<&[Mat4]>,
        dt: F,
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
        if let Some(before) = before {
            for (d, b) in drawn.iter_mut().zip(before) {
                *d = *b;
            }
        }
        ragdoll_pose::pose_all(
            now.iter().zip(&kit.rig.parts).zip(drawn.iter_mut().zip(starts.iter_mut())).map(|((b, p), (d, s))| (b, p.cofg, d, s)),
            before.is_none(),
            dt,
            &t.acos,
            &t.curve,
        );
        ragdoll_pose::reset(ragdoll.bones.iter_mut().map(|b| &mut b.body).zip(starts.iter()));
        if before.is_none() {
            for b in &mut ragdoll.bones {
                b.body.velocity = crash.bike.body.velocity;
            }
        }
        let bike = Bike { velocity: crash.bike.body.velocity, rates: crash.bike.body.rates, hit_velocity: crash.bike.body.velocity };
        ragdoll.wreck_enter(reason, &bike, wrecks, rng);
        let frame = math::quat_to_mat4(crash.bike.body.quat, crash.origin);
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
            fade: Fade { length: F::ONE, left: F::ZERO },
            touching: false,
            search: BikeSearch::default(),
        };
        down.sense(world);
        down
    }

    /// **One frame of it**: the wreck's arm of the original's mode machine
    /// ([`ragdoll_body::wreck_arm`]: the throw's clock, the roll, the settle test and its damping
    /// switch), then the world stepped with the bone collisions on the ground, the hands that still
    /// hold following the wreck bike, and every bone's orientation normalised.
    ///
    /// The bones are off the bike once the main bone is a foot past the kit's bike sphere
    /// ([`Kit::bike`]), its centre carried with the wreck body; the step has no bike box.
    pub fn step(&mut self, kit: &Kit, dt: F, crash: &crate::ride::Crash, world: &mut dyn World) {
        self.step_posed(kit, None, dt, crash, world);
    }

    /// [`Down::step`], the joint torques pulling towards `posed`'s rests when there is one (S65).
    pub fn step_posed(&mut self, kit: &Kit, posed: Option<&Posed>, dt: F, crash: &crate::ride::Crash, world: &mut dyn World) {
        self.step_posed_with(kit, posed, dt, crash, world, true);
    }

    /// [`Down::step_posed`] with whether the wreck bike is still in the broad phase (S108). The
    /// original takes it out at the put-back: the bike's unwreck stops the wreck part, which takes
    /// its collidable out of the track's tree and lowers the pair's own gate on it, and the body's
    /// update runs after the bike's in the tick, so on the put-back's tick the bones gather once
    /// more with the bike gone. A list gathered anew does not hold it, and a list kept from before
    /// still does, but no sphere is paired with it.
    pub fn step_posed_with(&mut self, kit: &Kit, posed: Option<&Posed>, dt: F, crash: &crate::ride::Crash, world: &mut dyn World, bike_in: bool) {
        let frame = math::quat_to_mat4(crash.bike.body.quat, crash.origin);
        let bike = kit.bike.map(|(centre, radius)| (bike_centre(&frame, centre), radius));
        // The bones' search, once a frame before the off-bike test, round the first sphere.
        if let (Some(b), Some(s)) = (bike, kit.spheres.first()) {
            // The bike is put in the track's tree where it stands, every frame.
            let cells = world.heightfield().and_then(|f| Cells::of(&*f));
            let key = cells.as_ref().zip(kit.bike_mesh.as_deref()).map_or(0, |(c, mesh)| c.key(mesh, &frame));
            self.search.gather(cells.as_ref(), s.radius, b, key, bike_in && !self.ragdoll.ignores_bike);
        }
        ragdoll_body::wreck_arm(
            &mut self.ragdoll,
            &self.contacts,
            &mut self.touching,
            &mut self.fade,
            dt,
            crash.bike.body.quat,
            bike,
            self.reason,
            &kit.wrecks,
            world,
        );
        let v = crash.bike.body.velocity;
        let drive = Drive {
            bike: Carrier {
                rows: [frame.0[0], frame.0[1], frame.0[2]],
                spin: F::ZERO,
                velocity: crash.bike.body.velocity,
                centre: crash.bike.body.position,
                quat: crash.bike.body.quat,
            },
            rests: std::array::from_fn(|i| match posed.and_then(|p| p.rests.get(i)) {
                Some(&r) => r,
                None => kit.rig.parts.get(i).map_or(UNIT_W, |p| p.rest),
            }),
            hold_targets: self.held_at.map(|p| math::transform_point(&frame, p)),
            hold_velocity: v.merge(Dest::XYZ, v.mul(Vf::splat(0.8))),
            gravity_scale: F::ONE,
            pose_spring: kit.pose_spring,
            pose_damping: kit.pose_damping,
            torques: true,
            mode: WRECK_MODE,
            fade_length: self.fade.length,
            fade_left: self.fade.left,
            ..Drive::default()
        };
        for c in &mut self.contacts {
            *c = Contact { touched: false, ground: false, scene: false, swept: false, ..*c };
        }
        for c in &mut self.ragdoll.collisions {
            c.touching = false;
        }
        // The bones' scene is the wreck bike (S105), a sphere paired with it as the search says: the
        // wreck's entry puts the bike on every bone's ignore list, the arm above takes it off again
        // when the main bone is off the bike, and the search lists it from its next gather.
        let mut open = Open;
        let paired = BikeSearch { listed: bike_in && self.search.listed, ..self.search };
        let mut bike_scene = kit.bike_mesh.as_deref().map(|mesh| WreckBikeScene::new(mesh, &frame, &crash.bike, paired));
        // And the path cars (S142): the search lists every moving collidable, measured from where
        // the first sphere last stood, and the cars have moved this tick before the bones do.
        if let (Some(b), Some(probe), true) = (&mut bike_scene, self.search.stood, self.search.built) {
            let reach = kit.spheres.iter().fold(F::ZERO, |r, s| r.max(s.radius));
            b.cars = world.moving().map_or_else(Vec::new, |p| crate::ragdoll_scene::PathCar::near(&p.props, probe, reach));
        }
        // The bones' segments are offered the bike through its collidable's sphere (S166).
        if let Some(b) = &mut bike_scene {
            b.sphere = bike;
        }
        let scene: &mut dyn crate::ragdoll_contact::Scene = match &mut bike_scene {
            Some(b) => b,
            None => &mut open,
        };
        self.ragdoll.step_among(
            dt,
            &drive,
            &kit.tables,
            Among { spheres: &kit.spheres, contacts: &mut self.contacts, terrain: Some((&mut *world, TERRAIN_MIN, TERRAIN_MAX)), scene, bike_box: None },
        );
        // Where the first sphere stood, if it stood: its collision on and its bone enabled.
        if let (Some(b), Some(s), Some(c)) = (&bike_scene, kit.spheres.first(), self.ragdoll.collisions.first())
            && c.enabled
            && self.ragdoll.bones.get(s.bone).is_some_and(|b| b.body.enabled)
        {
            self.search.stood = b.stood.first().copied();
        }
        for b in &mut self.ragdoll.bones {
            b.body.quat = normalize(b.body.quat);
        }
        self.sense(world);
    }

    /// The sensors the mode machine reads off the bones, and the ragdoll's own clocks.
    fn sense(&mut self, world: &mut dyn World) {
        let t = Thrown::from_bones(self.sensed.throw, &self.contacts, &self.ragdoll.bones[MAIN].body, world);
        self.sensed = Thrown { launch: Some(self.ragdoll.launch), ..t };
    }

    /// **The settle gate**: the body's touching flag, as the last step's wreck arm left it (S65).
    ///
    /// The original's body actor ends its tick by copying its touching flag into the request it
    /// shares with the bike, and that word is the gate the ride machine reads. The bike's update runs
    /// before the rider's, so the machine sees the flag the previous tick's step left: a caller reads
    /// this before the step. The hands may still hold; S26's "hands off first" was the clock's half
    /// second, not the gate.
    pub fn settles(&self) -> bool {
        self.touching
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
        w.bool("flung", self.ragdoll.flung);
        w.bool("settled", self.ragdoll.settled);
        w.f("roll", self.ragdoll.roll);
        w.f("fade_left", self.fade.left);
        w.bool("touching", self.touching);
        // The bones' search for the wreck bike (S105), which decides whether a bone meets it.
        w.bool("ignores_bike", self.ragdoll.ignores_bike);
        w.vf("search_stood", self.search.stood.unwrap_or_default());
        w.vf("search_centre", self.search.centre);
        w.f("search_bound", self.search.bound);
        w.bool("search_built", self.search.built);
        w.bool("search_listed", self.search.listed);
        w.f("search_distance", self.search.distance);
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
