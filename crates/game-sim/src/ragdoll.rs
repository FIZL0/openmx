//! **The rider's ragdoll**: eighteen rigid bodies pinned together by seventeen joints, which the
//! original rides on the bike and throws off it in a wreck.
//!
//! The bodies are S38's [`Body`], the engine's generic dynamics body. The plane flies on the same
//! class, and so do the rider's bones. The bones are also what the rider model is drawn from once it
//! is down. The rider file, `rider\anim\mrider.phy`, describes them as `[Part1]`..`[Part18]` and
//! `[Constraint1]`..`[Constraint26]`: two `Collision` sections, then the seventeen `PinToPart`
//! joints, then seven more `Collision`s.
//!
//! This module has what the body actor keeps beside its bones, and the two calls that switch the
//! ragdoll between its two lives:
//!
//! * [`Records`]: the per-bone and per-joint numbers for each life, as the rider loader stores them.
//! * [`Ragdoll::ride_enter`]: the riding mode's entry. Every hold on, the collisions off, the joints
//!   stiff, each bone back at its riding mass, inertia and damping.
//! * [`Ragdoll::wreck_enter`]: the wreck's. The world slowed down, the bike let go of, the joints
//!   changed to their wreck springs, the bones made heavy and floppy, and then a switch on the wreck
//!   reason. The switch hands the bones the bike's velocity, picks which hands stay on the bars, and
//!   draws the throw.
//!
//! Each is proved bit for bit against the original in `tests/ragdoll.rs`.
//!
//! # What the two entries do not do here
//!
//! Both of the original's entries first pose the bodies where the skeleton is and reset them (two
//! virtuals of the actor). The port has no skeleton world pose in this crate, so those two calls are
//! the caller's, made before. The wreck entry ends by asking the sound system for a wreck sound,
//! which is not simulation. The solvers the ragdoll is then stepped with are not ported yet: the
//! pins, the bone collisions, the bike box, the throw force and the joint forces. `plan/TODO.md`
//! has one task for each.

use crate::biplane::{Body, World, unturn};
use crate::phy::{Ini, Section, Throw, Wrecks};
use crate::rng::Rng;
use ps2_float::vu::{Dest, Vf};
use ps2_float::F;

/// The rider file's parts, and the bones built out of them.
pub const PARTS: usize = 18;

/// The main bone, `[Part1]` (the pelvis): the one the wreck measures and the throw pushes, but
/// for a back hit.
pub const MAIN: usize = 0;

/// The bone a back hit throws from instead, `[Part4]`.
pub const BACK: usize = 3;

/// The world the ragdoll is stepped in while riding: three passes of the constraints, at least 25
/// substeps a second. These are constants of the body actor's constructor, not keys of the file:
/// the file's own `NumberOfIterations = 4` and `SimulationRate = 25.0` are what the generic physics
/// loader reads, and both entries overwrite the world's with these.
pub const RIDE_WORLD: World = World { iterations: 3, rate: F(25.0) };

/// The wreck's world: two passes, at least 55 substeps a second.
pub const WRECK_WORLD: World = World { iterations: 2, rate: F(55.0) };

/// `(0, 0, 0, 1)`: the default of every vector key the rider loader reads for a ragdoll record.
const UNIT_W: Vf = Vf([0.0, 0.0, 0.0, 1.0]);

/// `(1, 1, 1, 1)`: the linear damping the riding entry hands every bone.
const ONES: Vf = Vf([1.0, 1.0, 1.0, 1.0]);

/// What `MaxJointTorqueAngle`'s degrees are scaled by into the radians a joint keeps: the
/// original's `0.01745329`, one unit in the last place under the nearest single to π/180.
const DEGREES: F = F(f32::from_bits(0x3c8e_fa34));

/// The chance that a hand stays on the bars after a bottom hit, per hand.
const HOLD_CHANCE: F = F(0.66);

/// The spin a cartwheel draws, at most, per axis.
const CARTWHEEL_SPIN: F = F(5.0);

/// How much a cartwheel adds to the bike's velocity before handing it to the bones: 2 % across,
/// 1 % up. These are the literal bit patterns: the nearest floats to 1.02 and 1.01.
const CARTWHEEL_ACROSS: F = F(f32::from_bits(0x3f82_8f5c));
const CARTWHEEL_UP: F = F(f32::from_bits(0x3f81_47ae));

/// **One bone's three sets of numbers**: one for riding, one for the wreck and one for the limp
/// body.
///
/// The riding mass, inertia and angular damping are the part's own (`Mass`, `Inertia`,
/// `AngularRateDamping`, which the generic loader puts in the body). The loader copies them out of
/// the body before reading the rest. A missing `RagdollMass` or `RagdollInertia` keeps the riding
/// value, and a missing damping is `(0, 0, 0, 1)`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PartRecord {
    pub ride_mass: F,
    /// `RagdollMass`.
    pub wreck_mass: F,
    pub ride_inertia: Vf,
    /// `RagdollInertia`.
    pub wreck_inertia: Vf,
    pub ride_angular_damping: Vf,
    /// `WreckAngularRateDamping`: what the bones keep of their rates once the wreck has *settled*
    /// ([`Ragdoll::damping`] with `stiff`).
    pub settled_angular_damping: Vf,
    /// `RagdollAngularRateDamping`: what they keep while they are being thrown about. `1` in the
    /// retail file, so nothing.
    pub limp_angular_damping: Vf,
}

/// **One joint's numbers**: which two bones it pins, and its torque spring both ways.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct JointRecord {
    /// `Part1` and `Part2`, as bone indices. The critical damping is worked out from the second
    /// one's inertia.
    pub part1: usize,
    pub part2: usize,
    /// `MaxJointTorqueAngle`, in radians.
    pub max_angle: F,
    /// `JointTorqueSpring` and `JointTorqueDamping`.
    pub ride_spring: Vf,
    pub ride_damping: Vf,
    /// `WreckJointTorqueSpring` and `WreckJointTorqueDamping`, **read from the wrong section**.
    /// The loader reads joint `i`'s from `[Constraint{i+1}]`, counting the joints and not the
    /// constraints. The joints start at `[Constraint3]`, so every joint gets the wreck spring of the
    /// joint two before it. The first two joints read the two `Collision` sections, which have no
    /// such keys, and get `(0, 0, 0, 1)`. In the retail file that makes the pelvis-to-lower-torso
    /// and lower-to-upper-torso joints go fully limp in a wreck. This port keeps the bug.
    pub wreck_spring: Vf,
    pub wreck_damping: Vf,
}

/// **Every record the two entries read**, as the rider loader leaves them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Records {
    pub parts: Vec<PartRecord>,
    pub joints: Vec<JointRecord>,
    /// The `Collision` sections, in file order: the bone each one is for.
    pub collisions: Vec<usize>,
}

impl Records {
    /// Read a rider file's bytes.
    pub fn read_bytes(bytes: &[u8]) -> Records {
        Records::read(&Ini::parse_bytes(bytes))
    }

    /// Read the parts, then the constraints in file order: a `PinToPart` is a joint, a `Collision`
    /// is a bone's collision. Then read each joint's wreck spring and damping from the section the
    /// loader actually picks (see [`JointRecord::wreck_spring`]).
    pub fn read(ini: &Ini) -> Records {
        let parts = ini.section("Physics").int("NumberOfParts").unwrap_or(0).max(0) as usize;
        let constraints = ini.section("Physics").int("NumberOfConstraints").unwrap_or(0).max(0) as usize;
        let parts = (1..=parts)
            .map(|i| {
                let s = ini.section(&format!("Part{i}"));
                let ride_mass = s.float_or("Mass", F::ZERO);
                // The inertia setter writes a one into the source quadword's `w` before it copies it.
                // The riding inertia is copied back out of the body, so it has that `w`.
                let ride_inertia = with_w_one(s.vector("Inertia", UNIT_W));
                PartRecord {
                    ride_mass,
                    wreck_mass: s.float_or("RagdollMass", ride_mass),
                    ride_inertia,
                    wreck_inertia: s.vector("RagdollInertia", ride_inertia),
                    ride_angular_damping: s.vector("AngularRateDamping", UNIT_W),
                    settled_angular_damping: s.vector("WreckAngularRateDamping", UNIT_W),
                    limp_angular_damping: s.vector("RagdollAngularRateDamping", UNIT_W),
                }
            })
            .collect();
        let bone = |s: &Section, key: &str| (s.int(key).unwrap_or(1) - 1).max(0) as usize;
        let mut joints = Vec::new();
        let mut collisions = Vec::new();
        for i in 1..=constraints {
            let s = ini.section(&format!("Constraint{i}"));
            match s.text("Type") {
                Some(t) if t.eq_ignore_ascii_case("PinToPart") => joints.push(JointRecord {
                    part1: bone(&s, "Part1"),
                    part2: bone(&s, "Part2"),
                    max_angle: s.float_or("MaxJointTorqueAngle", F::ZERO) * DEGREES,
                    ride_spring: s.vector("JointTorqueSpring", UNIT_W),
                    ride_damping: s.vector("JointTorqueDamping", UNIT_W),
                    ..JointRecord::default()
                }),
                Some(t) if t.eq_ignore_ascii_case("Collision") => collisions.push(bone(&s, "Part")),
                _ => {}
            }
        }
        for (i, j) in joints.iter_mut().enumerate() {
            let s = ini.section(&format!("Constraint{}", i + 1));
            j.wreck_spring = s.vector("WreckJointTorqueSpring", UNIT_W);
            j.wreck_damping = s.vector("WreckJointTorqueDamping", UNIT_W);
        }
        Records { parts, joints, collisions }
    }
}

fn with_w_one(v: Vf) -> Vf {
    Vf::new(v.x(), v.y(), v.z(), 1.0)
}

/// **One bone**: its body, and the two numbers of the body the plane's port did not need, its
/// mass and its inertia. The body holds only their inverses.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bone {
    pub body: Body,
    pub mass: F,
    pub inertia: Vf,
}

impl Bone {
    /// The body's mass setter: the mass, and one over it, **unguarded**. A zero mass is an
    /// infinite inverse mass, not a pinned body.
    pub fn set_mass(&mut self, m: F) {
        self.mass = m;
        self.body.inv_mass = F::ONE / m;
    }

    /// The inertia setter: the diagonal with a one written into its `w`, then one over each lane,
    /// or zero for a lane that is not positive. The inverse's `w` is left alone.
    pub fn set_inertia(&mut self, v: Vf) {
        let v = with_w_one(v);
        self.inertia = v;
        let inv = |x: f32| if F(x) <= F::ZERO { 0.0 } else { (F::ONE / F(x)).0 };
        self.body.inv_inertia = Vf::new(inv(v.x()), inv(v.y()), inv(v.z()), self.body.inv_inertia.w());
    }
}

/// **One joint as it is now**: the record's spring and damping for the current life, and the
/// critical damping worked out from them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Joint {
    /// The joint is in its wreck setting. The pin solve reads it.
    pub wrecked: bool,
    pub max_angle: F,
    pub spring: Vf,
    pub damping: Vf,
    /// `damping * 2 * sqrt(inertia * spring)` per lane, with the inertia taken from the joint's
    /// second bone *as it is when the entry runs*. Both entries set the joints before the bones, so
    /// the wreck's joints are worked out with the riding inertia and the riding entry's with the
    /// wreck's. `w` is untouched.
    pub critical: Vf,
}

impl Joint {
    fn set(&mut self, wrecked: bool, spring: Vf, damping: Vf, max_angle: F, inertia: Vf) {
        self.wrecked = wrecked;
        self.spring = spring;
        self.damping = damping;
        let lane = |k: usize| (F(damping[k]) * F(2.0) * (F(inertia[k]) * F(spring[k])).sqrt()).0;
        self.critical = Vf::new(lane(0), lane(1), lane(2), self.critical.w());
        self.max_angle = max_angle;
    }
}

/// **One bone's collision** with the world.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Collision {
    /// The bone it is for.
    pub bone: usize,
    /// The collision is tested. Off while riding: the bike's own collision carries the rider.
    pub enabled: bool,
    /// The bone is on the ground. The riding entry clears it.
    pub touching: bool,
}

/// The three of the bike's vectors the wreck entry reads off its request.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bike {
    /// The body's velocity and its rates, as the request's first pair.
    pub velocity: Vf,
    pub rates: Vf,
    /// The bike's velocity as the request's second copy has it: what a hard hit and a cartwheel
    /// hand the bones.
    pub hit_velocity: Vf,
}

/// **What a thrown body is thrown with**: the throw the reason picked, drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Launch {
    /// `Force`, give or take `ForceDelta` lane by lane, with `x` mirrored for half the reasons.
    pub force: Vf,
    /// What is left of `ForceTime` and of `HandHoldTime`, each give or take its delta.
    pub force_time: F,
    pub hand_hold: F,
    /// The bone the throw pushes.
    pub target: usize,
    /// The force is in the world's frame rather than the bike's. Set for a front hit, a bottom hit
    /// and a hard hit.
    pub world_frame: bool,
    /// The second of the pair of frame flags. Every entry clears it.
    pub other_frame: bool,
}

/// **The ragdoll**: the bones, the joints between them, and the switches the two entries throw.
#[derive(Clone, Debug, PartialEq)]
pub struct Ragdoll {
    pub records: Records,
    pub bones: Vec<Bone>,
    pub joints: Vec<Joint>,
    pub collisions: Vec<Collision>,
    /// The world the bones are stepped in.
    pub world: World,
    /// The four pins that hold the rider to the bike: the two hands, then two more. The wreck takes
    /// the last two off at once. The reason decides the hands.
    pub holds: [bool; 4],
    /// The box the riding body is kept inside, round the bike's seat.
    pub bike_box: bool,
    /// The throw's force is being pushed.
    pub throwing: bool,
    pub launch: Launch,
    /// The bones' collisions ignore the bike's two colliders. Both entries add the pair to every
    /// bone's ignore list. The riding entry does so only when there is a bike.
    pub ignores_bike: bool,
    /// Set by both entries.
    pub entered: bool,
    /// The bones hold the settled damping rather than the limp one. Riding counts as stiff.
    pub stiff: bool,
    /// The riding pose is driving the bones.
    pub riding: bool,
    /// The wreck's own four, all cleared by its entry: the bones are off the bike, the body was
    /// flung, it has come to rest, and how long the roll has lasted.
    pub off_bike: bool,
    pub flung: bool,
    pub settled: bool,
    pub roll: F,
}

impl Ragdoll {
    /// A ragdoll built out of the records, every body at rest at the origin with the riding numbers.
    pub fn new(records: Records) -> Ragdoll {
        let bones = records
            .parts
            .iter()
            .map(|p| {
                let mut b = Bone { body: Body { enabled: true, ..Body::default() }, ..Bone::default() };
                b.set_mass(p.ride_mass);
                b.set_inertia(p.ride_inertia);
                b.body.damping = ONES;
                b.body.ang_damping = p.ride_angular_damping;
                b.body.quat = UNIT_W;
                b.body.last_quat = UNIT_W;
                b
            })
            .collect();
        let joints = vec![Joint::default(); records.joints.len()];
        let collisions = records.collisions.iter().map(|&bone| Collision { bone, ..Collision::default() }).collect();
        Ragdoll {
            records,
            bones,
            joints,
            collisions,
            world: RIDE_WORLD,
            holds: [false; 4],
            bike_box: false,
            throwing: false,
            launch: Launch::default(),
            ignores_bike: false,
            entered: false,
            stiff: false,
            riding: false,
            off_bike: false,
            flung: false,
            settled: false,
            roll: F::ZERO,
        }
    }

    /// Each joint set for one life, in order, from its second bone's inertia as it is now.
    fn set_joints(&mut self, wrecked: bool) {
        for (j, r) in self.joints.iter_mut().zip(&self.records.joints) {
            let (spring, damping) = if wrecked { (r.wreck_spring, r.wreck_damping) } else { (r.ride_spring, r.ride_damping) };
            j.set(wrecked, spring, damping, r.max_angle, self.bones[r.part2].inertia);
        }
    }

    /// **The riding mode's entry**: the ragdoll goes back on the bike. `bike` is whether there is
    /// one to ignore.
    pub fn ride_enter(&mut self, bike: bool) {
        self.world = RIDE_WORLD;
        self.holds = [true; 4];
        self.bike_box = true;
        self.throwing = false;
        for c in &mut self.collisions {
            c.enabled = false;
            c.touching = false;
        }
        if bike {
            self.ignores_bike = true;
        }
        self.set_joints(false);
        for (b, p) in self.bones.iter_mut().zip(&mut self.records.parts) {
            b.set_mass(p.ride_mass);
            // The setter writes its one into the record it is handed, not only into the body.
            p.ride_inertia = with_w_one(p.ride_inertia);
            b.set_inertia(p.ride_inertia);
            b.body.ang_damping = p.ride_angular_damping;
            b.body.damping = ONES;
        }
        self.entered = true;
        self.stiff = true;
        self.riding = true;
    }

    /// **The wreck's entry**: the ragdoll comes off the bike, for `reason`.
    ///
    /// The reasons the switch knows are 1 to 8 and 11 to 13. Every other reason, 0 included, does
    /// only the common part: the world, the holds, the joints, the bones and the flags.
    ///
    /// | reason | hands on | bones get | throw |
    /// |---|---|---|---|
    /// | 1 | both | - | front, world frame |
    /// | 2 | right | - | side |
    /// | 3 | left | - | side, mirrored |
    /// | 4 | neither | - | back, from bone 4, mirrored |
    /// | 5 | 66 % each | the bike's velocity and rates | bottom, world frame |
    /// | 6, 7 | neither | the hit velocity | none, world frame, force off |
    /// | 8 | both | - | fall off the front, mirrored |
    /// | 11 | neither | - | fall off the back, mirrored |
    /// | 12 | neither | the bike's velocity, flattened | stunt, mirrored |
    /// | 13 | neither | a cartwheel: a random spin | - |
    ///
    /// "Mirrored" is `x` of the force times −1. The table names the hands as the first and second
    /// hold.
    pub fn wreck_enter(&mut self, reason: i32, bike: &Bike, wrecks: &Wrecks, rng: &mut Rng) {
        self.world = WRECK_WORLD;
        self.holds[2] = false;
        self.holds[3] = false;
        self.bike_box = false;
        self.throwing = true;
        for c in &mut self.collisions {
            c.enabled = true;
        }
        self.ignores_bike = true;
        self.set_joints(true);
        for (b, p) in self.bones.iter_mut().zip(&mut self.records.parts) {
            b.set_mass(p.wreck_mass);
            p.wreck_inertia = with_w_one(p.wreck_inertia);
            b.set_inertia(p.wreck_inertia);
            b.body.ang_damping = p.limp_angular_damping;
        }
        self.entered = true;
        self.stiff = false;
        self.riding = false;
        self.off_bike = false;
        self.settled = false;
        self.flung = false;
        self.roll = F::ZERO;

        match reason {
            1 => self.throw(wrecks.hit_front, [true, true], MAIN, true, false, rng),
            2 => self.throw(wrecks.hit_side, [false, true], MAIN, false, false, rng),
            3 => self.throw(wrecks.hit_side, [true, false], MAIN, false, true, rng),
            4 => self.throw(wrecks.hit_back, [false, false], BACK, false, true, rng),
            5 => {
                for b in &mut self.bones {
                    b.body.velocity = bike.velocity;
                    b.body.rates = bike.rates;
                }
                let first = rng.unit() < HOLD_CHANCE;
                let second = rng.unit() < HOLD_CHANCE;
                self.throw(wrecks.hit_bottom, [first, second], MAIN, true, false, rng);
            }
            6 | 7 => {
                for b in &mut self.bones {
                    b.body.velocity = bike.hit_velocity;
                }
                self.holds[0] = false;
                self.holds[1] = false;
                self.throwing = false;
                self.launch.target = MAIN;
                self.launch.world_frame = true;
                self.launch.other_frame = false;
            }
            8 => self.throw(wrecks.fall_off_front, [true, true], MAIN, false, true, rng),
            11 => self.throw(wrecks.fall_off_back, [false, false], MAIN, false, true, rng),
            12 => {
                for b in &mut self.bones {
                    let v = bike.velocity;
                    b.body.velocity = Vf::new(v.x(), 0.0, v.z(), v.w());
                }
                self.throw(wrecks.stunt, [false, false], MAIN, false, true, rng);
            }
            13 => {
                self.holds[0] = false;
                self.holds[1] = false;
                self.cartwheel(bike, rng);
            }
            _ => {}
        }
    }

    /// The five draws of a throw, in the original's order: the force's three lanes, then the two
    /// clocks, each `base + delta * (2u - 1)`.
    fn throw(&mut self, t: Throw, hands: [bool; 2], target: usize, world_frame: bool, mirrored: bool, rng: &mut Rng) {
        self.holds[0] = hands[0];
        self.holds[1] = hands[1];
        self.launch.target = target;
        self.launch.world_frame = world_frame;
        self.launch.other_frame = false;
        let mut spread = |delta: f32| (F(delta) * (F(2.0) * rng.unit() - F::ONE)).0;
        let x = spread(t.force_delta.x());
        let y = spread(t.force_delta.y());
        let z = spread(t.force_delta.z());
        let force = t.force.merge(Dest::XYZ, t.force.add(Vf::new(x, y, z, 1.0)));
        self.launch.force = if mirrored { Vf::new((F(force.x()) * F(-1.0)).0, force.y(), force.z(), force.w()) } else { force };
        self.launch.force_time = t.force_time + t.force_time_delta * (F(2.0) * rng.unit() - F::ONE);
        self.launch.hand_hold = t.hand_hold_time + t.hand_hold_time_delta * (F(2.0) * rng.unit() - F::ONE);
    }

    /// A cartwheel: a random spin of up to five radians a second about each axis, handed to every
    /// bone as the velocity the spin gives it about the main bone, on top of the bike's hit velocity
    /// a little enlarged, and as its rates in its own frame.
    fn cartwheel(&mut self, bike: &Bike, rng: &mut Rng) {
        let sx = (rng.unit() * CARTWHEEL_SPIN).0;
        let sy = (rng.unit() * CARTWHEEL_SPIN).0;
        let sz = (rng.unit() * CARTWHEEL_SPIN).0;
        let spin = Vf::new(sx, sy, sz, 1.0);
        let v = bike.hit_velocity;
        let base = Vf::new(
            (F(v.x()) * CARTWHEEL_ACROSS).0,
            (F(v.y()) * CARTWHEEL_UP).0,
            (F(v.z()) * CARTWHEEL_ACROSS).0,
            v.w(),
        );
        let centre = self.bones[MAIN].body.position;
        for b in &mut self.bones {
            let p = b.body.position;
            let arm = p.merge(Dest::XYZ, p.sub(centre));
            let around = spin.merge(Dest::XYZ, Vf::opmsub(Vf::opmula(spin, arm), arm, spin));
            b.body.velocity = base.merge(Dest::XYZ, base.add(around));
            b.body.rates = unturn(b.body.quat, spin);
        }
    }

    /// **The bones' damping switch** the wreck's settle test throws. `stiff` hands every bone its
    /// settled angular damping, except after a cartwheel (`reason` 13), which keeps the limp
    /// one. Otherwise the bones get the limp damping. The original also copies a clock of the
    /// actor's when it goes stiff. That clock is not this module's.
    pub fn damping(&mut self, stiff: bool, reason: i32) {
        self.stiff = stiff;
        let settled = stiff && reason != 13;
        for (b, p) in self.bones.iter_mut().zip(&self.records.parts) {
            b.body.ang_damping = if settled { p.settled_angular_damping } else { p.limp_angular_damping };
        }
    }
}
