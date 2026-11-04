//! F31b4c — MX vs. ATV's `[Mass Properties]`, as its rigid-body part reader and the bike's loader
//! leave them: the body's mass, inertia and centre of gravity, the collision record's mass, springs,
//! sweep and frictions, the gravity block, and the part's flag bits.
//!
//! MX vs. ATV reads a vehicle's `[Mass Properties]` with the part reader every rigid body shares (MX
//! Unleashed's [`crate::phy::Part`] is its near-twin), and the bike's own loader then reads a handful
//! of keys again over it ([`Bike`]). Where SLUS-21104 differs from MX Unleashed:
//!
//! * **The body's mass** is `Weight * 0.0310559` (the reader's pounds-to-slugs), and its inertia is
//!   the model's box worked out as a solid block, times `InertiaScale` (default 2, not 1). The bike's
//!   loader then puts its own `Inertia` over the body's — but not over the collision record's, which
//!   keeps the box's.
//! * **The record's inertia** is the body's box inertia times `CollisionWeight`'s mass over the
//!   body's (`mass * inv_mass`, the vector unit's multiply), and its inverse is the FPU's for `x` and
//!   `y` and the vector unit's divide for `z`.
//! * **The sweep's facing** (`HighSpeedMinCollisionAngle`) is the curve table's cosine of the number
//!   *as radians*: MX Unleashed turns its degrees to radians first. `45` is `cos(45)`, `0.5253`.
//! * **`CheckForHighSpeedCollisions`** defaults to off.
//! * **A `-1` body spring key** (`BodyMaxPenetration`, `BodySpringStiffness`, `BodySpringDamping`,
//!   `BodyRestitution`) is the spring's, as a `-1` `BodyFriction` is the `Friction`.
//! * **A flag** is true for a value starting `T`, `ON` or `Y`, false for one starting `F`, `OFF` or
//!   `N`, and otherwise `strtol`'s; the whole word goes into the record's, bit 0 into the object's bits.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::camera::CurveTable;
use crate::phy::{MAX_SURFACES, Section, integer};
use ps2_float::{Acc, Dest, F, Vf, fpu, vu};

/// The reader's pounds to mass.
pub const MASS_PER_POUND: F = F(0.0310559);
/// A spring's stiffness is per pound of weight: mass times this.
const SPRING_GRAVITY: F = F(32.2);
const ZERO_W1: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);
const ONES: Vf = Vf::new(1.0, 1.0, 1.0, 1.0);
/// The moment scales' "same as the spring's": all four words.
const UNSET_SCALE: Vf = Vf::new(-1.0, -1.0, -1.0, 1.0);
/// The yaw rate limit's frame: `(20, YawRateLimit, 20)`.
const MAX_RATE: F = F(20.0);

const PI: F = F(core::f32::consts::PI);
const TWO_PI: F = F(core::f32::consts::TAU);
const ONE_OVER_TWO_PI: F = F(0.15915494);
const CURVE_STEP: F = F(0.0010473721);
const CURVE_INV_STEP: F = F(954.77045);

/// Where a part lives: its object (the flag bits), its rigid body, its collision record and its
/// gravity block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub object: u32,
    pub body: u32,
    pub record: u32,
    pub gravity: u32,
}

/// What the part reader writes into the gravity block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gravity {
    /// `GravityScale` (default 1), at the block's start.
    pub scale: F,
    /// `(0, -GravityScale * Weight, 0, 1)`.
    pub force: Vf,
    /// `SpeedThreshold`, `AngularRateThreshold` (0.3 each), then the acceleration, angular
    /// acceleration, speed impulse and body angular speed impulse thresholds (the largest float).
    pub thresholds: [F; 6],
    /// `MinimumResetHeight` (default 0).
    pub minimum_reset_height: F,
    /// `ResetTime` (default 5).
    pub reset_time: F,
}

/// What the part reader writes into the rigid body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub mass: F,
    pub inv_mass: F,
    /// `Radius` (default 0.5), made positive.
    pub radius: F,
    /// `DefaultVelocityDamping`, `DefaultAngularRateDamping` (default ones).
    pub damping: Vf,
    pub angular_damping: Vf,
    /// `InertiaScale` (default `(2, 2, 2, 1)`), which the box's inertia is times.
    pub inertia_scale: Vf,
    /// The box's inertia (`w` 1) and its inverse: 0 for a lane at or under 0. The inverse's `w` is
    /// left as it was.
    pub inertia: Vf,
    pub inv_inertia: [F; 3],
    /// `CofG` (default the origin). The setter also moves the body by the change turned into the
    /// world, which is the body's own business and not written here.
    pub cofg: Vf,
}

/// What the part reader writes into the collision record.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Record {
    /// `CheckForCollisions` (default 1) and `CheckForHighSpeedCollisions` (default 0), whole words.
    pub check_for_collisions: u32,
    pub check_high_speed: u32,
    /// `CollisionWeight`'s mass and its inverse, the inertia and its inverse: none of them written
    /// for a mass at or under 0.
    pub mass: Option<(F, F)>,
    pub inertia: Option<Vf>,
    pub inv_inertia: Option<Vf>,
    /// `CollisionCofGOffset` and `HighSpeedRayOffset` (default the origin).
    pub cofg_offset: Vf,
    pub ray_offset: Vf,
    /// `HighSpeedRaySpeedScale`, `HighSpeedUnpenetrationDistance` (default 2 each), and the facing
    /// cosine of `HighSpeedMinCollisionAngle` (default 0).
    pub ray_speed_scale: F,
    pub unpenetration: F,
    pub facing: F,
    /// The springs, as the record keeps them: penetration, stiffness, damping, the stiffness per
    /// pound, the damping per mass, restitution; then the body spring's six.
    pub springs: [F; 12],
    /// `AngularImpulseEnabled`, `BodyAngularImpulseEnabled` (default 1 each), and
    /// `BodyAngularImpulseScale` (default ones).
    pub angular_impulse: [u32; 2],
    pub angular_impulse_scale: Vf,
    /// `SpringMomentScale`, `FrictionMomentScale`, `BodySpringMomentScale`,
    /// `BodyFrictionMomentScale` (default ones; the body's two are the others for `-1, -1, -1`).
    pub moment_scales: [Vf; 4],
    pub friction: F,
    pub body_friction: F,
    pub surface_friction: [F; MAX_SURFACES],
}

/// The part reader's flags, bit 0 of each: `CollisionEnabled`, `DetectCollisionsWhenDisabled`,
/// `ResetOffEdgeOfWorld`, `ResetBelowMinimum`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flags {
    pub collision_enabled: bool,
    pub detect_when_disabled: bool,
    pub reset_off_edge: bool,
    pub reset_below_minimum: bool,
}

/// MX vs. ATV's `[Mass Properties]` through its part reader.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub flags: Flags,
    pub gravity: Gravity,
    pub body: Body,
    pub record: Record,
}

/// A flag as the game's reads it: `T`, `ON`, `Y` (any case, the start of the value) are 1, then
/// `F`, `OFF`, `N` are 0, and anything else is `strtol`'s base-0 number.
pub fn flag(text: &str) -> u32 {
    let starts = |p: &str| text.len() >= p.len() && text.as_bytes()[..p.len()].eq_ignore_ascii_case(p.as_bytes());
    if starts("T") || starts("ON") || starts("Y") {
        1
    } else if starts("F") || starts("OFF") || starts("N") {
        0
    } else {
        integer(text) as u32
    }
}

fn flag_or(s: &Section, key: &str, default: u32) -> u32 {
    s.text(key).map_or(default, flag)
}

/// The record setter's angle wrap, into `-pi..=pi`.
fn wrap(a: F) -> F {
    let turns = F(fpu::from_i32((ONE_OVER_TWO_PI * a).to_i32()));
    let w = Acc::adda(F::ZERO, a).msub(TWO_PI, turns);
    if !(w <= PI) {
        w - TWO_PI
    } else if w < -PI {
        TWO_PI + w
    } else {
        w
    }
}

/// The sweep's facing: the curve table's cosine of `angle` (radians), with the entry's slope.
pub fn facing(angle: F, curve: &CurveTable) -> F {
    let s = (PI + wrap(wrap(angle))) * CURVE_INV_STEP;
    let i = s.to_i32();
    let frac = s - F(fpu::from_i32(i));
    let [cos, sin] = curve.entries()[i as usize];
    Acc::adda(F::ZERO, F(cos)).msub(CURVE_STEP, F(sin) * frac)
}

/// A spring's damping per mass: `damping * 2 * sqrt(mass * stiffness)`, 0 for no mass or no
/// stiffness.
fn spring_damping(mass: F, damping: F, stiffness: F) -> F {
    if mass <= F::ZERO || stiffness <= F::ZERO {
        F::ZERO
    } else {
        F(2.0) * damping * (mass * stiffness).sqrt()
    }
}

/// The model's box as a solid block of `mass`: each lane the mass over 12 times the squares of the
/// other two sides, times the lane's scale; `w` 1.
pub fn block_inertia(mass: F, scale: Vf, half_box: Vf) -> Vf {
    let dims = half_box.merge(Dest::XYZ, half_box.mul(Vf::splat(2.0)));
    let m12 = mass / F(12.0);
    let (x2, y2, z2) = (F(dims.x()) * F(dims.x()), F(dims.y()) * F(dims.y()), F(dims.z()) * F(dims.z()));
    Vf::new(
        (F(scale.x()) * m12 * (y2 + z2)).0,
        (F(scale.y()) * m12 * (x2 + z2)).0,
        (F(scale.z()) * m12 * (x2 + y2)).0,
        1.0,
    )
}

/// The record's inertia off the body's (and its inverse): the lanes times the record's mass over
/// the body's.
pub fn record_inertia(inertia: Vf, mass: F, inv_mass: F) -> (Vf, Vf) {
    let k = (mass * inv_mass).0;
    let i = Vf::new(vu::mul(inertia.x(), k), vu::mul(inertia.y(), k), vu::mul(inertia.z(), k), inertia.w());
    let inv = if F(i.x()) <= F::ZERO || F(i.y()) <= F::ZERO || F(i.z()) <= F::ZERO {
        ZERO_W1
    } else {
        Vf::new((F::ONE / F(i.x())).0, (F::ONE / F(i.y())).0, vu::div(1.0, i.z()), i.w())
    };
    (i, inv)
}

/// The rigid body's inertia setter: the vector with `w` 1, and the inverse of each lane over 0.
fn inverse_inertia(inertia: Vf) -> [F; 3] {
    let inv = |v: f32| if F(v) <= F::ZERO { F::ZERO } else { F::ONE / F(v) };
    [inv(inertia.x()), inv(inertia.y()), inv(inertia.z())]
}

impl Part {
    /// Read `[Mass Properties]` in the reader's order. `half_box` is the model's bounding box's half
    /// size, which the inertia is worked out from.
    pub fn read(s: &Section, half_box: Vf, curve: &CurveTable) -> Part {
        let inertia_default = ONES.merge(Dest::XYZ, ONES.mul(Vf::splat(2.0)));
        let weight = s.float_or("Weight", F(500.0));
        let cofg = s.vector("CofG", ZERO_W1);
        let scale = s.float_or("GravityScale", F::ONE);
        let inertia_scale = s.vector("InertiaScale", inertia_default);
        let radius = s.float_or("Radius", F(0.5));
        let damping = s.vector("DefaultVelocityDamping", ONES);
        let angular_damping = s.vector("DefaultAngularRateDamping", ONES);
        let collision_enabled = flag_or(s, "CollisionEnabled", 1);
        let check_for_collisions = flag_or(s, "CheckForCollisions", 1);
        let detect_when_disabled = flag_or(s, "DetectCollisionsWhenDisabled", 0);
        let collision_weight = s.float_or("CollisionWeight", weight);
        let cofg_offset = s.vector("CollisionCofGOffset", ZERO_W1);
        let check_high_speed = flag_or(s, "CheckForHighSpeedCollisions", 0);
        let ray_offset = s.vector("HighSpeedRayOffset", ZERO_W1);
        let ray_speed_scale = s.float_or("HighSpeedRaySpeedScale", F(2.0));
        let unpenetration = s.float_or("HighSpeedUnpenetrationDistance", F(2.0));
        let angle = s.float_or("HighSpeedMinCollisionAngle", F::ZERO);
        let spring = [
            s.float_or("SpringStiffness", F(10.0)),
            s.float_or("SpringDamping", F::from_bits(0x3f33_3333)),
            s.float_or("MaxPenetration", F(0.25)),
            s.float_or("Restitution", F::ZERO),
        ];
        let body_spring = [
            s.float_or("BodySpringStiffness", F(10.0)),
            s.float_or("BodySpringDamping", F::from_bits(0x3f33_3333)),
            s.float_or("BodyMaxPenetration", F(0.25)),
            s.float_or("BodyRestitution", F::ZERO),
        ];
        let surface_friction = core::array::from_fn(|k| s.float_or(&format!("FrictionZone{}", k + 1), F::ONE));
        let friction = s.float_or("Friction", F::ONE);
        let body_friction = s.float_or("BodyFriction", friction);
        let moment_scales = [
            s.vector("SpringMomentScale", ONES),
            s.vector("FrictionMomentScale", ONES),
            s.vector("BodySpringMomentScale", ONES),
            s.vector("BodyFrictionMomentScale", ONES),
        ];
        let angular_impulse = [flag_or(s, "AngularImpulseEnabled", 1), flag_or(s, "BodyAngularImpulseEnabled", 1)];
        let angular_impulse_scale = s.vector("BodyAngularImpulseScale", ONES);
        let reset_off_edge = flag_or(s, "ResetOffEdgeOfWorld", 1);
        let reset_time = s.float_or("ResetTime", F(5.0));
        let reset_below_minimum = flag_or(s, "ResetBelowMinimum", 0);
        let minimum_reset_height = s.float_or("MinimumResetHeight", F::ZERO);
        let thresholds = [
            s.float_or("SpeedThreshold", F::from_bits(0x3e99_999a)),
            s.float_or("AngularRateThreshold", F::from_bits(0x3e99_999a)),
            s.float_or("AccelerationThreshold", F(f32::MAX)),
            s.float_or("AngularAccelerationThreshold", F(f32::MAX)),
            s.float_or("SpeedImpulseThreshold", F(f32::MAX)),
            s.float_or("BodyAngularSpeedImpulseThreshold", F(f32::MAX)),
        ];

        let mass = MASS_PER_POUND * weight;
        let collision_mass = MASS_PER_POUND * collision_weight;
        let inertia = block_inertia(mass, inertia_scale, half_box);
        let body = Body {
            mass,
            inv_mass: F::ONE / mass,
            radius: radius.abs(),
            damping,
            angular_damping,
            inertia_scale,
            inertia,
            inv_inertia: inverse_inertia(inertia),
            cofg,
        };
        let (mass_words, inertia_words) = if collision_mass <= F::ZERO {
            (None, None)
        } else {
            (Some((collision_mass, F::ONE / collision_mass)), Some(record_inertia(inertia, collision_mass, body.inv_mass)))
        };

        let [stiffness, damp, penetration, restitution] = spring;
        let [b_stiffness, b_damp, b_penetration, b_restitution] = body_spring;
        let unset = F(-1.0);
        let per_pound = (SPRING_GRAVITY * (stiffness * mass)).abs();
        let b_penetration = if b_penetration == unset { penetration } else { b_penetration };
        let (b_stiffness, b_per_pound) = if b_stiffness == unset {
            (stiffness, per_pound)
        } else {
            (b_stiffness, (SPRING_GRAVITY * (b_stiffness * mass)).abs())
        };
        let b_damp = if b_damp == unset { damp } else { b_damp };
        let b_per_mass = spring_damping(mass, b_damp, b_per_pound);
        let b_restitution = if b_restitution == unset { restitution } else { b_restitution };
        let springs = [
            penetration,
            stiffness,
            damp,
            per_pound,
            spring_damping(mass, damp, per_pound),
            restitution,
            b_penetration,
            b_stiffness,
            b_damp,
            b_per_pound,
            b_per_mass,
            b_restitution,
        ];
        let [sms, fms, bsms, bfms] = moment_scales;
        let same = |v: Vf| v.to_bits() == UNSET_SCALE.to_bits();

        Part {
            flags: Flags {
                collision_enabled: collision_enabled & 1 != 0,
                detect_when_disabled: detect_when_disabled & 1 != 0,
                reset_off_edge: reset_off_edge & 1 != 0,
                reset_below_minimum: reset_below_minimum & 1 != 0,
            },
            gravity: Gravity {
                scale,
                force: Vf::new(0.0, (-scale * weight).0, 0.0, 1.0),
                thresholds,
                minimum_reset_height,
                reset_time,
            },
            body,
            record: Record {
                check_for_collisions,
                check_high_speed,
                mass: mass_words,
                inertia: inertia_words.map(|(i, _)| i),
                inv_inertia: inertia_words.map(|(_, v)| v),
                cofg_offset,
                ray_offset,
                ray_speed_scale,
                unpenetration,
                facing: facing(angle, curve),
                springs,
                angular_impulse,
                angular_impulse_scale,
                moment_scales: [sms, fms, if same(bsms) { sms } else { bsms }, if same(bfms) { fms } else { bfms }],
                friction,
                body_friction: if body_friction == unset { friction } else { body_friction },
                surface_friction,
            },
        }
    }

    /// The words the reader writes, as `(address, mask, value)`: a mask of all ones is a whole word,
    /// the object's flag bits share one.
    pub fn words(&self, at: Layout) -> Vec<(u32, u32, u32)> {
        let mut w = Vec::new();
        let whole = |w: &mut Vec<(u32, u32, u32)>, a: u32, v: F| w.push((a, !0, v.to_bits()));
        let quad = |w: &mut Vec<(u32, u32, u32)>, a: u32, v: Vf| {
            for (k, b) in v.to_bits().into_iter().enumerate() {
                w.push((a + 4 * k as u32, !0, b));
            }
        };
        let f = self.flags;
        let bit = |on: bool, b: u32| if on { b } else { 0 };
        w.push((
            at.object + 0x44,
            0x0000_4181,
            bit(f.collision_enabled, 1) | bit(f.detect_when_disabled, 0x80) | bit(f.reset_off_edge, 0x100)
                | bit(f.reset_below_minimum, 0x4000),
        ));

        let g = self.gravity;
        whole(&mut w, at.gravity, g.scale);
        quad(&mut w, at.gravity + 0x40, g.force);
        for (k, t) in g.thresholds.into_iter().enumerate() {
            whole(&mut w, at.gravity + 0x80 + 4 * k as u32, t);
        }
        whole(&mut w, at.gravity + 0x98, g.minimum_reset_height);
        whole(&mut w, at.gravity + 0xa0, g.reset_time);

        let b = self.body;
        whole(&mut w, at.body + 0x18, b.mass);
        whole(&mut w, at.body + 0x1c, b.inv_mass);
        whole(&mut w, at.body + 0x20, b.radius);
        quad(&mut w, at.body + 0x30, b.damping);
        quad(&mut w, at.body + 0x130, b.inertia);
        for (k, v) in b.inv_inertia.into_iter().enumerate() {
            whole(&mut w, at.body + 0x140 + 4 * k as u32, v);
        }
        quad(&mut w, at.body + 0x150, b.angular_damping);
        quad(&mut w, at.body + 0x1a0, b.cofg);

        let r = &self.record;
        w.push((at.record + 0x2c, !0, r.check_for_collisions));
        w.push((at.record + 0x30, !0, r.check_high_speed));
        w.push((at.record + 0x44, !0, 0));
        if let Some((m, inv)) = r.mass {
            whole(&mut w, at.record + 0x58, m);
            whole(&mut w, at.record + 0x5c, inv);
        }
        if let (Some(i), Some(inv)) = (r.inertia, r.inv_inertia) {
            quad(&mut w, at.record + 0x60, i);
            quad(&mut w, at.record + 0x70, inv);
        }
        quad(&mut w, at.record + 0x80, r.cofg_offset);
        quad(&mut w, at.record + 0xa0, r.ray_offset);
        whole(&mut w, at.record + 0xb0, r.ray_speed_scale);
        whole(&mut w, at.record + 0xb4, r.unpenetration);
        whole(&mut w, at.record + 0xb8, r.facing);
        for (k, s) in r.springs.into_iter().enumerate() {
            whole(&mut w, at.record + 0x10c + 4 * k as u32, s);
        }
        w.push((at.record + 0x13c, !0, r.angular_impulse[0]));
        w.push((at.record + 0x140, !0, r.angular_impulse[1]));
        quad(&mut w, at.record + 0x150, r.angular_impulse_scale);
        for (k, v) in r.moment_scales.into_iter().enumerate() {
            quad(&mut w, at.record + 0x160 + 0x10 * k as u32, v);
        }
        whole(&mut w, at.record + 0x1a0, r.friction);
        whole(&mut w, at.record + 0x1a4, r.body_friction);
        for (k, z) in r.surface_friction.into_iter().enumerate() {
            whole(&mut w, at.record + 0x1a8 + 4 * k as u32, z);
        }
        w
    }
}

/// What the bike's own loader reads again from `[Mass Properties]`, after the part reader, into the
/// bike and over its body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bike {
    /// `Inertia` (default the origin, `w` made 1), which becomes the body's, and its inverse.
    pub inertia: Vf,
    pub inv_inertia: [F; 3],
    /// `CofG`, `WreckCofG`, `BalanceCofG` (default the origin): the bike's, not the body's.
    pub cofgs: [Vf; 3],
    /// `CofGTravelForward`, `CofGTravelBack`, `BrakeCofGTravelForward` (default 0).
    pub travel_forward: F,
    pub travel_back: F,
    pub brake_travel_forward: F,
    /// `YawRateLimit`, the body's largest rates' `y` between two 20s. A file without it leaves
    /// whatever was on the loader's stack: `None`, and the rates are not written.
    pub yaw_rate_limit: Option<F>,
}

impl Bike {
    pub fn read(s: &Section) -> Bike {
        let inertia = s.vector("Inertia", ZERO_W1).merge(Dest::W, Vf::splat(1.0));
        Bike {
            inertia,
            inv_inertia: inverse_inertia(inertia),
            cofgs: [s.vector("CofG", ZERO_W1), s.vector("WreckCofG", ZERO_W1), s.vector("BalanceCofG", ZERO_W1)],
            travel_forward: s.float_or("CofGTravelForward", F::ZERO),
            travel_back: s.float_or("CofGTravelBack", F::ZERO),
            brake_travel_forward: s.float_or("BrakeCofGTravelForward", F::ZERO),
            yaw_rate_limit: s.float("YawRateLimit"),
        }
    }

    /// The words the loader writes over the part's, as [`Part::words`]: `bike` is the bike's own
    /// address (its inertia and centres of gravity are at `+0x1120..`, the travels at `+0x1320..`).
    pub fn words(&self, bike: u32, body: u32) -> Vec<(u32, u32, u32)> {
        let mut w = Vec::new();
        let quad = |w: &mut Vec<(u32, u32, u32)>, a: u32, v: Vf| {
            for (k, b) in v.to_bits().into_iter().enumerate() {
                w.push((a + 4 * k as u32, !0, b));
            }
        };
        quad(&mut w, bike + 0x1120, self.inertia);
        for (k, v) in self.cofgs.into_iter().enumerate() {
            quad(&mut w, bike + 0x1130 + 0x10 * k as u32, v);
        }
        w.push((bike + 0x1320, !0, self.travel_forward.to_bits()));
        w.push((bike + 0x1324, !0, self.travel_back.to_bits()));
        w.push((bike + 0x132c, !0, self.brake_travel_forward.to_bits()));
        quad(&mut w, body + 0x130, self.inertia);
        for (k, v) in self.inv_inertia.into_iter().enumerate() {
            w.push((body + 0x140 + 4 * k as u32, !0, v.to_bits()));
        }
        if let Some(y) = self.yaw_rate_limit {
            for (k, v) in [MAX_RATE, y, MAX_RATE].into_iter().enumerate() {
                w.push((body + 0x210 + 4 * k as u32, !0, v.to_bits()));
            }
        }
        w
    }
}
