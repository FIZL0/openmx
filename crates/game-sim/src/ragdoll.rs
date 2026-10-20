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
//! And the step the ragdoll then takes each frame:
//!
//! * [`Pin::solve`]: a pin's constraint, the two-bone solve or the one-bone one against a target,
//!   and the joints' twist limits.
//! * [`Ragdoll::throw_clock`] and [`Ragdoll::throw_force`]: the throw's pull on its bone and the two
//!   clocks that end it and the hands' hold.
//! * [`Ragdoll::forces`]: gravity, the riding pose's spring, and the joints' torque springs against
//!   their critical damping.
//! * [`Ragdoll::step`] and [`Ragdoll::update`]: the world run at its iterations and rate, as the
//!   body actor's physics update runs it.
//!
//! Each is proved bit for bit against the original in `tests/ragdoll.rs`.
//!
//! # What is not done here
//!
//! Both of the original's entries first pose the bodies where the skeleton is and reset them (two
//! virtuals of the actor). The port has no skeleton world pose in this crate, so those two calls are
//! the caller's, made before. The wreck entry ends by asking the sound system for a wreck sound,
//! which is not simulation. The step takes the skeleton's pose and the bike as a [`Drive`] from its
//! caller. The bone collisions and the bike box keep their places in the world's constraint list
//! but do nothing yet (S52).

use crate::air::{AcosTable, acos, narrow, widen};
use crate::biplane::{Body, Dynamics, List, World, unturn};
use crate::camera::CurveTable;
use crate::chassis::{normalize, turn};
use crate::phy::{Ini, Section, Throw, Wrecks};
use crate::ragdoll_contact::{BikeBox, Contact, Scene, Sphere, Terrain};
use crate::rng::Rng;
use ps2_float::vu::{Dest, Field, Vf};
use ps2_float::{F, fpu, libm, vu};

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
    /// The orientation a pin with no second bone limits this bone's turn against. Not written by
    /// anything in this module.
    pub reference: Vf,
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
    /// The force is in the main bone's frame, turned by its orientation each frame, rather than in
    /// the bike's. Set for a front hit, a bottom hit and a hard hit.
    pub world_frame: bool,
    /// The force is pushed as it is, turned by nothing. It wins over `world_frame`. Every entry
    /// clears it.
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
    /// The ragdoll's pins: the four [`Ragdoll::holds`] first, then one per joint, in the joints'
    /// order.
    pub pins: Vec<Pin>,
    /// The force the throw pushes [`Launch::target`] with this frame, in the world:
    /// [`Ragdoll::throw_clock`] works it out of [`Launch::force`].
    pub pull: Vf,
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
        let pins = HOLD_BONES
            .iter()
            .map(|&b| Pin::new(b, None))
            .chain(records.joints.iter().map(|j| Pin::new(j.part1, Some(j.part2))))
            .collect();
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
            pins,
            pull: UNIT_W,
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

// --- the step ------------------------------------------------------------------------------------

/// The bones the four holds pin, in the holds' order: the two hands, then the two feet. The body
/// actor's constructor wires these, not the file.
pub const HOLD_BONES: [usize; 4] = [11, 7, 17, 14];

/// The tables the step reads: the sine and cosine curve, the arc cosine the joints measure their
/// twist with, and the finer arc cosine the joint limits use near the ends.
pub struct Tables {
    pub curve: CurveTable,
    pub acos: AcosTable,
    pub fine: FineAcos,
}

impl Tables {
    /// All three built the way the startup code builds them. The curve table's entries can sit a
    /// bit off the game's (see [`CurveTable::build`]); the two arc cosines are the game's.
    pub fn build() -> Tables {
        Tables { curve: CurveTable::build(), acos: AcosTable::build(), fine: FineAcos::build() }
    }
}

/// How many pieces [`FineAcos`] is cut into, and how many entries each piece has before its
/// duplicated last one.
const FINE_PIECES: usize = 25;
const FINE_ENTRIES: usize = 400;

/// The top of the finest piece, `2^-24`. Each piece above it is twice as wide as the one below.
const FINE_FIRST: f32 = f32::from_bits(0x3380_0000);

/// The exponent of the finest piece's top, less one: a lookup picks its piece by the argument's
/// exponent less this.
const FINE_EXPONENT: i32 = 102;

/// One piece of [`FineAcos`]: where it starts, how many entries per unit, and the entries.
#[derive(Clone, Debug, PartialEq)]
pub struct FinePiece {
    pub base: F,
    pub scale: F,
    pub entries: Box<[f32]>,
}

/// **The fine arc cosine**: `acos(1 - x)` for `x` in `0..1`, in twenty-five pieces that halve in
/// width toward zero, so that an angle near no turn at all (or a whole half turn) is still found
/// to the last bits. Each piece is 401 entries, the last a copy of the one before, built by the
/// game's double-precision `acos` at startup like [`AcosTable`].
#[derive(Clone, Debug, PartialEq)]
pub struct FineAcos(Box<[FinePiece]>);

impl FineAcos {
    /// Build it the way the startup code does: piece `k` runs from the last one's top to twice
    /// that, `399 / width` entries per unit, entry `i` the arc cosine of `1 - (base + i / scale)`.
    pub fn build() -> FineAcos {
        let (mut lo, mut hi) = (0.0f32, FINE_FIRST);
        let mut pieces = Vec::with_capacity(FINE_PIECES);
        for _ in 0..FINE_PIECES {
            let scale = fpu::div(399.0, fpu::sub(hi, lo));
            let mut t = vec![0.0f32; FINE_ENTRIES + 1];
            for (i, e) in t.iter_mut().take(FINE_ENTRIES).enumerate() {
                let x = fpu::sub(1.0, fpu::add(lo, fpu::div(fpu::from_i32(i as i32), scale)));
                *e = narrow(acos(widen(x)));
            }
            t[FINE_ENTRIES] = t[FINE_ENTRIES - 1];
            pieces.push(FinePiece { base: F(lo), scale: F(scale), entries: t.into_boxed_slice() });
            lo = hi;
            hi = fpu::mul(hi, 2.0);
        }
        FineAcos(pieces.into_boxed_slice())
    }

    /// The pieces, for the test that compares them with the game's.
    pub fn pieces(&self) -> &[FinePiece] {
        &self.0
    }

    /// `acos(1 - x)`: the piece by `x`'s exponent (clamped to the first and the last), then a linear
    /// interpolation between two of its entries.
    pub fn at(&self, x: F) -> F {
        let k = (((x.0.to_bits() & 0x7f80_0000) >> 23) as i32 - FINE_EXPONENT).clamp(0, FINE_PIECES as i32 - 1);
        let p = &self.0[k as usize];
        let s = F(fpu::mul(fpu::sub(x.0, p.base.0), p.scale.0));
        let i = s.to_i32();
        let frac = fpu::sub(s.0, fpu::from_i32(i));
        let (lo, hi) = (p.entries[i as usize], p.entries[i as usize + 1]);
        F(fpu::madd(fpu::mula(fpu::sub(1.0, frac), lo), frac, hi))
    }
}

/// `a × b`, as a `vopmula`/`vopmsub` pair. `w` is zero.
fn cross(a: Vf, b: Vf) -> Vf {
    Vf::opmsub(Vf::opmula(a, b), b, a)
}

/// `c + x + y + z`, through the adder's `w` lane: how every length and dot product in the pin's
/// solve is summed. `c` is the pin's [`Pin::gains`] `w`, zero in the game.
fn sumw(c: f32, v: Vf) -> f32 {
    vu::madd(vu::madd(vu::add(c, v.x()), 1.0, v.y()), 1.0, v.z())
}

/// `-v`, all four lanes, off `vf0`.
fn negate(v: Vf) -> Vf {
    Vf::msub(Vf::VF0.mul(Vf::VF0.bc(Field::X)), v, Vf::VF0.bc(Field::W))
}

/// `a ⊗ b`.
fn prod(a: Vf, b: Vf) -> Vf {
    let p = a.mul(b);
    let acc = Vf::opmula(a, b);
    let acc = Vf::madd(acc, a, b.bc(Field::W));
    let acc = Vf::madd(acc, b, a.bc(Field::W));
    let w = vu::msub(vu::msub(vu::sub(p.w(), p.x()), 1.0, p.y()), 1.0, p.z());
    Vf::opmsub(acc, b, a).merge(Dest::W, Vf::splat(w))
}

/// `a ⊗ b*`: the turn from `b` to `a`.
fn rel(a: Vf, b: Vf) -> Vf {
    let p = a.mul(b);
    let acc = Vf::opmula(b, a);
    let acc = Vf::madd(acc, a, b.bc(Field::W));
    let acc = Vf::msub(acc, b, a.bc(Field::W));
    let w = vu::madd(vu::madd(vu::add(p.w(), p.x()), 1.0, p.y()), 1.0, p.z());
    Vf::opmsub(acc, a, b).merge(Dest::W, Vf::splat(w))
}

/// `a* ⊗ b`, the other way round from [`rel`]: `b` turned back by `a`.
fn rel_back(a: Vf, b: Vf) -> Vf {
    let p = a.mul(b);
    let acc = Vf::opmula(b, a);
    let acc = Vf::madd(acc, b, a.bc(Field::W));
    let acc = Vf::msub(acc, a, b.bc(Field::W));
    let w = vu::madd(vu::madd(vu::add(p.w(), p.x()), 1.0, p.y()), 1.0, p.z());
    Vf::opmsub(acc, a, b).merge(Dest::W, Vf::splat(w))
}

fn add_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.add(b))
}

fn sub_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.sub(b))
}

/// `-1`, as the solve builds it: `0 - 1` in the VU.
fn minus_one() -> Vf {
    Vf::splat(vu::sub(0.0, 1.0))
}

/// `a` clamped into `-1..1` the way the original writes it, so that an unordered `a` comes out as
/// one.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn clamp_unit(a: F) -> F {
    if a <= F::ONE {
        if a < F(-1.0) { F(-1.0) } else { a }
    } else {
        F::ONE
    }
}

/// **A quaternion as an angle and an axis**: the half angle off the arc cosine of `w`, then twice
/// it, and the vector half over the sine of the half angle (the curve table's entry, with no
/// correction). No turn at all is the `x` axis and the half angle as it came out, which may be
/// `-0`. The axis's `w` is one.
pub fn axis_angle(q: Vf, tables: &Tables) -> (F, Vf) {
    let half = tables.acos.at(clamp_unit(clamp_unit(F(q.w()))));
    if half == F::ZERO {
        return (half, Vf::new(1.0, 0.0, 0.0, 1.0));
    }
    let inv = F::ONE / tables.curve.coarse_sin(half);
    let axis = Vf::new((F(q.x()) * inv).0, (F(q.y()) * inv).0, (F(q.z()) * inv).0, 1.0);
    (half * F(2.0), axis)
}

/// The two bones of a pin, mutably: `(first, second)`.
fn pair(bones: &mut [Bone], a: usize, b: usize) -> (&mut Body, &mut Body) {
    assert_ne!(a, b, "a pin between a bone and itself");
    if a < b {
        let (lo, hi) = bones.split_at_mut(b);
        (&mut lo[a].body, &mut hi[0].body)
    } else {
        let (lo, hi) = bones.split_at_mut(a);
        (&mut hi[0].body, &mut lo[b].body)
    }
}

/// The smallest gap between a lane's two limits that counts as the lane being locked: `0.001`, as
/// its bit pattern.
const LOCKED: F = F(f32::from_bits(0x3a83_126f));

/// **One pin**: an anchor on one bone held to an anchor on another, or to a point in the world
/// that the pin follows.
///
/// A joint's pin also limits how far its second bone turns against its first, lane by lane, while
/// its joint is [`Joint::wrecked`]: a lane past a limit is put back on it, and it is remembered
/// in [`Pin::hit`] (this step) and [`Pin::sticky`] (until the joint torque reads it).
///
/// The anchors, the limits and [`Pin::gains`]'s `z` and `w` are the pin's own, from the generic
/// physics loader. This module does not read them out of the file.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pin {
    /// The bone that is pinned, and the bone it is pinned to (none for a hold).
    pub first: usize,
    pub second: Option<usize>,
    /// The two anchors, each in its bone's frame. A hold's second anchor is the world point it is
    /// held at when it follows nothing.
    pub anchors: [Vf; 2],
    /// The point and velocity a hold is held at.
    pub target: Vf,
    pub target_velocity: Vf,
    /// The limits of the twist, per lane, as the vector half of a quaternion.
    pub lower: Vf,
    pub upper: Vf,
    /// `1` for a lane put back on its upper limit, `-1` on its lower one, else `0`; `w` is one.
    pub hit: Vf,
    pub sticky: Vf,
    /// `(first's inverse mass, second's inverse mass, gain, c)`: the solve writes the first two
    /// before every use. The gain scales the error. `c` is summed into every length.
    pub gains: Vf,
}

impl Pin {
    fn new(first: usize, second: Option<usize>) -> Pin {
        Pin {
            first,
            second,
            hit: UNIT_W,
            sticky: UNIT_W,
            gains: Vf::new(0.0, 0.0, 1.0, 0.0),
            ..Pin::default()
        }
    }

    /// The limit test on one twist `e`: each lane past a limit set onto it and marked. Says whether
    /// any was.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn limit(&mut self, e: &mut Vf) -> bool {
        let mut any = false;
        for k in 0..3 {
            if !(F(e[k]) <= F(self.upper[k])) {
                self.hit[k] = 1.0;
                self.sticky[k] = 1.0;
                e[k] = self.upper[k];
                any = true;
            } else if F(e[k]) < F(self.lower[k]) {
                self.hit[k] = -1.0;
                self.sticky[k] = -1.0;
                e[k] = self.lower[k];
                any = true;
            }
        }
        any
    }

    /// **The solve**, once per constraint pass. `on` is the pin's switch; `limited` its joint's
    /// [`Joint::wrecked`]; `follow` the point (and velocity) a hold follows this frame.
    ///
    /// With both bones there and enabled, the two-bone solve, after the twist limit if `limited`.
    /// Otherwise the first bone alone against the target, its turn limited against its
    /// [`Bone::reference`].
    pub fn solve(&mut self, on: bool, limited: bool, follow: Option<(Vf, Option<Vf>)>, bones: &mut [Bone], tables: &Tables) {
        if !on {
            return;
        }
        if let Some(second) = self.second
            && bones[self.first].body.enabled
            && bones[second].body.enabled
        {
            self.hit = UNIT_W;
            let (b1, b2) = pair(bones, self.first, second);
            if limited {
                self.twist_limit(b1, b2, tables);
            }
            self.two_body(b1, b2, &tables.curve);
            return;
        }
        if !bones[self.first].body.enabled {
            return;
        }
        self.hit = UNIT_W;
        let bone = &mut bones[self.first];
        if limited {
            self.turn_limit(bone);
        }
        match follow {
            Some((target, velocity)) => {
                self.target = target;
                if let Some(v) = velocity {
                    self.target_velocity = v;
                }
            }
            None => self.target = self.anchors[1],
        }
        self.one_body(&mut bone.body, &tables.curve);
    }

    /// The two-bone twist limit: the second bone's turn against the first in the first's frame,
    /// on the near hemisphere, limited. A lane past its limit turns both bones back onto it, the
    /// first by the share of its inverse inertia, and takes the relative rate out of the lanes
    /// still moving outward (or of every lane locked between two equal limits).
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn twist_limit(&mut self, b1: &mut Body, b2: &mut Body, tables: &Tables) {
        let d = rel(b2.quat, b1.quat);
        let mut e = unturn(b1.quat, d);
        if F(e.w()) < F::ZERO {
            e = negate(e);
        }
        if !self.limit(&mut e) {
            return;
        }
        e[3] = fpu::sqrt(fpu::sub(
            fpu::sub(fpu::sub(1.0, fpu::mul(e.x(), e.x())), fpu::mul(e.y(), e.y())),
            fpu::mul(e.z(), e.z()),
        ));
        let c = rel(d, turn(b1.quat, e));
        let norm = |i: Vf| fpu::sqrt(fpu::madd(fpu::madda(fpu::mula(i.x(), i.x()), i.y(), i.y()), i.z(), i.z()));
        let (n1, n2) = (norm(b1.inv_inertia), norm(b2.inv_inertia));
        let s = fpu::div(1.0, fpu::add(n1, n2));
        let (f1, f2) = (fpu::mul(n1, s), fpu::mul(n2, s));
        if F(c.w()).abs() < F::ONE {
            let v = clamp_unit(clamp_unit(F(c.w())));
            let (x, base, sign) = if v < F::ZERO {
                (v + F::ONE, F(core::f32::consts::PI), F(-1.0))
            } else {
                (F::ONE - v, F::ZERO, F::ONE)
            };
            let theta = base + sign * tables.fine.at(x);
            let inv = F::ONE / F(libm::sinf(theta.0));
            let turn_by = |f: f32| {
                let (cos, sin) = tables.curve.at(F(f) * theta);
                let si = sin * inv;
                Vf::new((si * F(c.x())).0, (si * F(c.y())).0, (si * F(c.z())).0, cos.0)
            };
            let (r1, r2) = (turn_by(f1), turn_by(f2));
            b1.quat = prod(r1, b1.quat);
            b2.quat = rel_back(r2, b2.quat);
        }
        let d = b2.rates.sub(unturn(b2.quat, turn(b1.quat, b1.rates)));
        let mut out = Vf::default();
        for k in 0..3 {
            let (h, r) = (F(self.hit[k]), F(d[k]));
            let keep = (!(h <= F::ZERO) && !(r <= F::ZERO))
                || (h < F::ZERO && r < F::ZERO)
                || F(fpu::sub(self.lower[k], self.upper[k])).abs() < LOCKED;
            out[k] = if keep { r.0 } else { 0.0 };
        }
        b2.rates = sub_xyz(b2.rates, out.mul(Vf::splat(f2)));
        b1.rates = add_xyz(b1.rates, unturn(b1.quat, turn(b2.quat, out)).mul(Vf::splat(f1)));
    }

    /// The one-bone turn limit: the bone's turn against its reference, limited the same way. A
    /// lane past its limit turns the bone back onto it outright and zeroes the rate still moving
    /// outward.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn turn_limit(&mut self, bone: &mut Bone) {
        let r = bone.reference;
        let b = &mut bone.body;
        let d = rel(b.quat, r);
        let mut e = unturn(r, d);
        if F(e.w()) < F::ZERO {
            e = negate(e);
        }
        if !self.limit(&mut e) {
            return;
        }
        e[3] = fpu::sqrt(fpu::sub(
            fpu::sub(fpu::sub(1.0, fpu::mul(e.x(), e.x())), fpu::mul(e.y(), e.y())),
            fpu::mul(e.z(), e.z()),
        ));
        let c = rel(d, turn(r, e));
        if F(c.w()).abs() < F::ONE {
            b.quat = normalize(rel_back(c, b.quat));
        }
        for k in 0..3 {
            let (h, w) = (F(self.hit[k]), F(b.rates[k]));
            if (!(h <= F::ZERO) && !(w <= F::ZERO)) || (h < F::ZERO && w < F::ZERO) {
                b.rates[k] = 0.0;
            }
        }
    }

    /// What one bone's share of an impulse does: `j` through the anchor `r` into the bone's frame,
    /// across the inverse inertia and back out, as an axis and the angle to turn it by.
    fn swing(b: &Body, r: Vf, j: Vf, c: f32) -> (Vf, f32) {
        let a = turn(b.quat, cross(r, unturn(b.quat, j)).mul(b.inv_inertia));
        let angle = vu::add(c, vu::sqrt(sumw(c, a.mul(a))));
        (a.mul(Vf::splat(vu::div(1.0, angle))), angle)
    }

    /// The effective inverse mass of one bone along the direction of `e`.
    fn reach(b: &Body, r: Vf, n: Vf, c: f32) -> f32 {
        let nb = unturn(b.quat, n);
        sumw(c, nb.mul(cross(cross(r, nb).mul(b.inv_inertia), r)))
    }

    /// The unit direction of an error.
    fn direction(e: Vf, c: f32) -> Vf {
        e.mul(Vf::splat(vu::rsqrt(1.0, sumw(c, e.mul(e)))))
    }

    /// `(axis·sin, cos)` of half of `angle`: the turn [`Pin::swing`]'s angle makes.
    fn half_turn(axis: Vf, angle: f32, curve: &CurveTable) -> Vf {
        let (cos, sin) = curve.at(F(vu::mul(0.5, angle)));
        axis.mul(Vf::splat(sin.0)).merge(Dest::W, Vf::splat(vu::mul(1.0, cos.0)))
    }

    /// **The two-bone solve**: the two anchors' gap and their velocities' difference each taken
    /// out by one impulse along it, shared by the inverse masses and the inverse inertias. The
    /// position impulse moves the bones and turns them. The velocity impulse changes their
    /// velocities and rates.
    pub fn two_body(&mut self, b1: &mut Body, b2: &mut Body, curve: &CurveTable) {
        self.gains = Vf::new(b1.inv_mass.0, b2.inv_mass.0, self.gains.z(), self.gains.w());
        let g = self.gains;
        let (gx, gy, c) = (g.x(), g.y(), g.w());
        let gain = g.bc(Field::Z);
        let [r1, r2] = self.anchors;
        let e = turn(b1.quat, r1).add(b1.position).sub(turn(b2.quat, r2).add(b2.position)).mul(gain);
        let v1 = turn(b1.quat, cross(b1.rates, r1)).add(b1.velocity);
        let v2 = turn(b2.quat, cross(b2.rates, r2)).add(b2.velocity);
        let ve = v1.sub(v2).mul(gain);
        let impulse = |e: Vf, b1: &Body, b2: &Body| {
            let n = Pin::direction(e, c);
            let denom = vu::add(c, gx);
            let denom = vu::madd(denom, 1.0, gy);
            let denom = vu::madd(denom, 1.0, Pin::reach(b1, r1, n, c));
            let denom = vu::madd(denom, 1.0, Pin::reach(b2, r2, n, c));
            e.mul(minus_one()).mul(Vf::splat(vu::div(1.0, denom)))
        };

        let j = impulse(e, b1, b2);
        b1.position = add_xyz(b1.position, j.mul(Vf::splat(gx)));
        let (axis1, angle1) = Pin::swing(b1, r1, j, c);
        b2.position = sub_xyz(b2.position, j.mul(Vf::splat(gy)));
        let (axis2, angle2) = Pin::swing(b2, r2, j, c);
        let angle2 = vu::sub(c, angle2);

        let jv = impulse(ve, b1, b2);
        b1.velocity = add_xyz(b1.velocity, jv.mul(Vf::splat(gx)));
        b1.rates = add_xyz(b1.rates, cross(r1, unturn(b1.quat, jv)).mul(b1.inv_inertia));
        b2.velocity = sub_xyz(b2.velocity, jv.mul(Vf::splat(gy)));
        b2.rates = sub_xyz(b2.rates, cross(r2, unturn(b2.quat, jv)).mul(b2.inv_inertia));

        b1.quat = prod(Pin::half_turn(axis1, angle1, curve), b1.quat);
        b2.quat = prod(Pin::half_turn(axis2, angle2, curve), b2.quat);
    }

    /// **The one-bone solve**: the same, with the target in place of the second bone.
    pub fn one_body(&mut self, b1: &mut Body, curve: &CurveTable) {
        self.gains = Vf::new(b1.inv_mass.0, self.gains.y(), self.gains.z(), self.gains.w());
        let g = self.gains;
        let (gx, c) = (g.x(), g.w());
        let gain = g.bc(Field::Z);
        let r1 = self.anchors[0];
        let e = turn(b1.quat, r1).add(b1.position).sub(self.target).mul(gain);
        let v1 = turn(b1.quat, cross(b1.rates, r1)).add(b1.velocity);
        let ve = v1.sub(self.target_velocity).mul(gain);
        let impulse = |e: Vf, b1: &Body| {
            let n = Pin::direction(e, c);
            let denom = vu::add(Pin::reach(b1, r1, n, c), gx);
            e.mul(minus_one()).mul(Vf::splat(vu::div(1.0, denom)))
        };

        let j = impulse(e, b1);
        b1.position = add_xyz(b1.position, j.mul(Vf::splat(gx)));
        let (axis1, angle1) = Pin::swing(b1, r1, j, c);

        let jv = impulse(ve, b1);
        b1.velocity = add_xyz(b1.velocity, jv.mul(Vf::splat(gx)));
        b1.rates = add_xyz(b1.rates, cross(r1, unturn(b1.quat, jv)).mul(b1.inv_inertia));

        b1.quat = prod(Pin::half_turn(axis1, angle1, curve), b1.quat);
    }
}

/// The bike, as the ragdoll's step reads it off the request.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Carrier {
    /// The bike's orientation as three rows, and the one rate of the request's the riding pose is
    /// carried round by: about the third row.
    pub rows: [Vf; 3],
    pub spin: F,
    pub velocity: Vf,
    /// The point the spin is about.
    pub centre: Vf,
    /// The orientation a throw in neither frame is turned by.
    pub quat: Vf,
}

/// **What the rest of the body actor hands the step each frame**: the bike, the skeleton's pose,
/// the holds' targets, the mode and the fade, and the actor's numbers the forces read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drive {
    pub bike: Carrier,
    /// Each bone's centre of mass where the skeleton's riding pose has it, in the world.
    pub points: [Vf; PARTS],
    /// Each bone's rest orientation in its skeleton bone's frame.
    pub rests: [Vf; PARTS],
    /// Where the four holds hold, and the velocity all four follow.
    pub hold_targets: [Vf; 4],
    pub hold_velocity: Vf,
    /// What gravity is scaled by.
    pub gravity_scale: F,
    /// The riding pose's spring and damper, per lane of a bone's frame.
    pub pose_spring: Vf,
    pub pose_damping: Vf,
    /// The joints' torque springs run (with [`Ragdoll::stiff`]).
    pub torques: bool,
    /// The body actor's mode: in the wreck's (2), the torques are faded and pushed on both bones.
    pub mode: i32,
    /// The fade's length and what is left of it: the torques fade in as the clock runs down.
    pub fade_length: F,
    pub fade_left: F,
}

impl Default for Drive {
    fn default() -> Drive {
        Drive {
            bike: Carrier::default(),
            points: [UNIT_W; PARTS],
            rests: [UNIT_W; PARTS],
            hold_targets: [UNIT_W; 4],
            hold_velocity: UNIT_W,
            gravity_scale: F::ONE,
            pose_spring: UNIT_W,
            pose_damping: UNIT_W,
            torques: false,
            mode: 0,
            fade_length: F::ZERO,
            fade_left: F::ZERO,
        }
    }
}

/// The gravity the bones fall at, per unit mass and gravity scale: `32.2`, as its bit pattern.
const GRAVITY: F = F(f32::from_bits(0x4200_cccd));

/// The body actor's mode while wrecked.
const WRECK_MODE: i32 = 2;

/// Gravity's force on a bone.
fn weight(scale: F, mass: F) -> F {
    GRAVITY * (scale * mass)
}

/// The constraints the world holds in front of the joints: the four holds and the first two bone
/// collisions.
const BEFORE_JOINTS: usize = 6;

/// The bone collisions in front of the joints.
const FIRST_COLLISIONS: usize = 2;

/// The world's lists, for [`World::step`]: the bones, the two force listeners (the actor's, then
/// the throw), and the constraints in the order the world holds them.
struct Stepper<'a, 'b, 'w> {
    ragdoll: &'a mut Ragdoll,
    drive: &'a Drive,
    tables: &'a Tables,
    among: Option<Among<'b, 'w>>,
}

/// **What the bone collisions and the bike box solve against** (S53): the nine spheres in the
/// `Collision` sections' order, a contact record for each, the terrain and its box, the scene, and
/// the box the riding body is held in. [`Ragdoll::step_among`] takes it; [`Ragdoll::step`] has
/// none, and their places in the list do nothing.
pub struct Among<'a, 'w> {
    pub spheres: &'a [Sphere],
    pub contacts: &'a mut [Contact],
    /// The ground query and the terrain's box (`min`, `max`).
    pub terrain: Option<(&'a mut (dyn crate::world::World + 'w), Vf, Vf)>,
    pub scene: &'a mut dyn Scene,
    pub bike_box: Option<BikeBox>,
}

/// The constraint at `i` is which bone collision: the first two sit in front of the joints, the
/// other seven after them.
fn collision_at(i: usize, joints: usize, collisions: usize) -> Option<usize> {
    if (4..BEFORE_JOINTS).contains(&i) {
        Some(i - 4)
    } else if (BEFORE_JOINTS + joints..BEFORE_JOINTS + joints + collisions - FIRST_COLLISIONS).contains(&i) {
        Some(i - joints - 4)
    } else {
        None
    }
}

impl Dynamics for Stepper<'_, '_, '_> {
    fn count(&self, list: List) -> i32 {
        match list {
            List::Bodies => self.ragdoll.bones.len() as i32,
            List::Force => 2,
            List::Constraints => (4 + self.ragdoll.collisions.len() + self.ragdoll.joints.len() + 1) as i32,
            List::Preforce | List::Post => 0,
        }
    }
    fn preforce(&mut self, _: i32, _: F) {}
    fn force(&mut self, i: i32, _: F) {
        match i {
            0 => self.ragdoll.forces(self.drive, self.tables),
            _ => self.ragdoll.throw_force(),
        }
    }
    fn enabled(&self, body: i32) -> bool {
        self.ragdoll.bones[body as usize].body.enabled
    }
    fn integrate(&mut self, body: i32, h: F) {
        self.ragdoll.bones[body as usize].body.integrate(h);
    }
    fn end_substep(&mut self, body: i32) {
        self.ragdoll.bones[body as usize].body.end_substep();
    }
    fn constrain(&mut self, i: i32, _: bool) {
        let i = i as usize;
        let r = &mut *self.ragdoll;
        if i < 4 {
            let follow = Some((self.drive.hold_targets[i], Some(self.drive.hold_velocity)));
            r.pins[i].solve(r.holds[i], false, follow, &mut r.bones, self.tables);
        } else if (BEFORE_JOINTS..BEFORE_JOINTS + r.joints.len()).contains(&i) {
            let j = i - BEFORE_JOINTS;
            r.pins[4 + j].solve(true, r.joints[j].wrecked, None, &mut r.bones, self.tables);
        }
        let (joints, collisions) = (r.joints.len(), r.collisions.len());
        let Some(among) = self.among.as_mut() else { return };
        if let Some(k) = collision_at(i, joints, collisions) {
            let (Some(s), Some(c)) = (among.spheres.get(k), among.contacts.get_mut(k)) else { return };
            let terrain = among.terrain.as_mut().map(|(world, min, max)| Terrain { world: &mut **world, min: *min, max: *max });
            c.solve(r.collisions[k].enabled, s, &mut r.bones[s.bone].body, terrain, &mut *among.scene, &self.tables.curve);
            r.collisions[k].touching = c.touched;
        } else if i == BEFORE_JOINTS + joints + collisions - FIRST_COLLISIONS {
            if let Some(b) = among.bike_box {
                b.hold(r.bike_box, &mut r.bones[MAIN].body);
            }
        }
    }
    fn post(&mut self, _: i32, _: F) {}
}

impl Ragdoll {
    /// **The throw's clock**, once a frame in the wreck mode. While the force has time left the
    /// pull is the launch force (as it is, turned by the main bone's orientation, or turned by
    /// the bike's) and the time runs down. Once it is out the throw stops, the pull back at
    /// nothing. Then the hands: they hold on until their own clock runs out, then both let go.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub fn throw_clock(&mut self, dt: F, bike_quat: Vf) {
        let l = &mut self.launch;
        if !(l.force_time <= F::ZERO) {
            self.pull = if l.other_frame {
                l.force
            } else if l.world_frame {
                turn(self.bones[MAIN].body.quat, l.force)
            } else {
                turn(bike_quat, l.force)
            };
            l.force_time = l.force_time - dt;
        } else if self.throwing {
            self.pull = UNIT_W;
            self.throwing = false;
        }
        if !(l.hand_hold <= F::ZERO) {
            l.hand_hold = l.hand_hold - dt;
        } else if self.holds[0] || self.holds[1] {
            self.holds[0] = false;
            self.holds[1] = false;
        }
    }

    /// **The throw force**, the world's second force listener: the pull onto the target bone while
    /// throwing and the bone is enabled.
    pub fn throw_force(&mut self) {
        let b = &mut self.bones[self.launch.target].body;
        if self.throwing && b.enabled {
            b.add_force(self.pull);
        }
    }

    /// **The actor's force listener**, the world's first: gravity on every bone, the riding pose's
    /// spring and damper pulling each bone at its point of the pose (carried round with the
    /// bike), then each joint's torque spring.
    pub fn forces(&mut self, drive: &Drive, tables: &Tables) {
        let bike = &drive.bike;
        let s = Vf::new(0.0, 0.0, bike.spin.0, 0.0);
        let spin = Vf::madd(Vf::madd(bike.rows[0].mul(s.bc(Field::X)), bike.rows[1], s.bc(Field::Y)), bike.rows[2], s.bc(Field::Z));
        for (i, bone) in self.bones.iter_mut().enumerate() {
            let b = &mut bone.body;
            b.force[1] = (F(b.force[1]) - weight(drive.gravity_scale, bone.mass)).0;
            if self.riding {
                let x = drive.points[i];
                let arm = sub_xyz(x, bike.centre);
                let carried = add_xyz(bike.velocity, cross(spin, arm));
                let dv = sub_xyz(carried, b.velocity);
                let dp = sub_xyz(x, b.position);
                let f = drive.pose_spring.mul(unturn(b.quat, dp)).add(drive.pose_damping.mul(unturn(b.quat, dv)));
                b.force = add_xyz(b.force, turn(b.quat, f));
            }
        }
        if self.stiff && drive.torques {
            for j in 0..self.joints.len() {
                self.torque(j, drive, tables);
            }
        }
    }

    /// One joint's torque spring: the second bone's turn away from where its rest orientation on
    /// the first would have it, as an angle about an axis in its own frame, clamped to the
    /// joint's max angle (a lane its pin has just stopped at its limit, and still pushing that
    /// way, is let go), against the critically damped rates. The second bone takes the torque.
    /// In the wreck the torque is faded in and halved, and the first bone takes it back.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn torque(&mut self, j: usize, drive: &Drive, tables: &Tables) {
        let pin = self.pins[4 + j];
        let joint = self.joints[j];
        let Some(second) = pin.second else { return };
        let (q1, q2) = (self.bones[pin.first].body.quat, self.bones[second].body.quat);
        let rates = self.bones[second].body.rates;
        let clamp = |v: f32| fpu::min(fpu::max(v, -1.0), 1.0);
        let wc = Vf::new(clamp(rates.x()), clamp(rates.y()), clamp(rates.z()), rates.w());
        let qa = turn(q1, drive.rests[second]);
        let qb = prod(qa, q1);
        let mut qr = rel(qb, q2);
        if F(qr.w()) < F::ZERO {
            qr = negate(qr);
        }
        let (angle, axis) = axis_angle(normalize(qr), tables);
        let a = unturn(q2, axis.merge(Dest::XYZ, axis.mul(Vf::splat(angle.0))));
        let m = joint.max_angle.0;
        let mut a = a;
        for k in 0..3 {
            let (s, v) = (F(pin.sticky[k]), F(a[k]));
            a[k] = if (!(s <= F::ZERO) && !(v <= F::ZERO)) || (s < F::ZERO && v < F::ZERO) {
                0.0
            } else {
                fpu::min(fpu::max(a[k], fpu::neg(m)), m)
            };
        }
        let t = Vf::msub(joint.spring.mul(a), joint.critical, wc);
        let t = t.merge(Dest::W, joint.spring);
        if drive.mode == WRECK_MODE {
            let fade = if drive.fade_length == F::ZERO {
                F::ONE
            } else {
                let ratio = fpu::div(fpu::sub(drive.fade_left.0, 0.0), fpu::sub(drive.fade_length.0, 0.0));
                let f = fpu::madd(fpu::adda(0.0, 1.0), ratio, fpu::sub(0.0, 1.0));
                F(fpu::min(fpu::max(f, 0.0), 1.0))
            };
            let t = t.merge(Dest::XYZ, t.mul(Vf::splat(fpu::mul(0.5, fade.0))));
            self.bones[second].body.add_moment_local(t);
            let back = unturn(q1, turn(q2, t).mul(minus_one()));
            self.bones[pin.first].body.add_moment_local(back);
        } else {
            self.bones[second].body.add_moment_local(t);
        }
        self.pins[4 + j].sticky = UNIT_W;
    }

    /// **The ragdoll's step**: one frame of the world at its iterations and rate, the force
    /// listeners and the constraints as the world holds them. The bone collisions and the bike
    /// box are not ported (S52): their places in the list do nothing.
    pub fn step(&mut self, dt: F, drive: &Drive, tables: &Tables) {
        let world = self.world;
        world.step(dt, &mut Stepper { ragdoll: self, drive, tables, among: None });
    }

    /// [`Ragdoll::step`] with the bone collisions and the bike box solved in their places (S53).
    pub fn step_among(&mut self, dt: F, drive: &Drive, tables: &Tables, among: Among<'_, '_>) {
        let world = self.world;
        world.step(dt, &mut Stepper { ragdoll: self, drive, tables, among: Some(among) });
    }

    /// **The body actor's physics update**, the part of it that is the ragdoll's: the world is
    /// stepped while the body is `placed` and `active` and not waiting to be `reposed`. Says whether
    /// the actor counts as driven by the skeleton this frame (the pair of flags the update sets).
    /// The pose, reset, skeleton and logic calls round the step are the caller's.
    pub fn update(&mut self, dt: F, placed: bool, active: bool, repose: &mut bool, drive: &Drive, tables: &Tables) -> bool {
        let driven = !(placed && active && !self.riding && !*repose);
        if placed && active && !*repose {
            self.step(dt, drive, tables);
        }
        *repose = false;
        driven
    }
}
