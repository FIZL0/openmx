//! **Where the ragdoll's bones start from**: each bone body's start state, and the reset that puts
//! the body back at it.
//!
//! A bone body keeps a start beside its state: a position and a velocity, set together, and an
//! orientation and its rates, set together. The pose ([`pose`], the first of the two virtuals both
//! ragdoll entries begin with) reads the skeleton's world pose and writes the start; the reset
//! ([`reset`], the second) copies it into every body of the ragdoll's world and empties everything
//! the last steps accumulated.
//!
//! The skeleton's world pose itself — each bone node's world position and its two axes — is not in
//! this crate yet, so [`pose`] takes it as arguments.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::air::AcosTable;
use crate::biplane::{Body, unturn};
use crate::camera::CurveTable;
use crate::chassis::turn;
use crate::math::{Mat4, basis, mat4_to_quat};
use ps2_float::{Dest, F, Field, Vf, fpu, vu};

/// What a reset leaves in every accumulator: the original copies its constant `(0, 0, 0, 1)`.
const EMPTY: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);

/// **A bone body's start**, as its two setters write it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Start {
    pub position: Vf,
    pub velocity: Vf,
    pub quat: Vf,
    pub rates: Vf,
}

impl Start {
    /// The first setter: the position and the velocity, whole quadwords.
    pub fn place(&mut self, position: Vf, velocity: Vf) {
        self.position = position;
        self.velocity = velocity;
    }

    /// The second: the orientation and the rates.
    pub fn turn(&mut self, quat: Vf, rates: Vf) {
        self.quat = quat;
        self.rates = rates;
    }

    /// **The reset of one body**: at its start, both this substep's and the last one's position and
    /// orientation, and every force, moment and acceleration emptied.
    pub fn reset(&self, body: &mut Body) {
        body.last_position = self.position;
        body.position = self.position;
        body.velocity = self.velocity;
        body.force = EMPTY;
        body.last_force = EMPTY;
        body.shock_peak = EMPTY;
        body.shock = EMPTY;
        body.accel = EMPTY;
        body.last_quat = self.quat;
        body.quat = self.quat;
        body.rates = self.rates;
        body.moment = EMPTY;
        body.last_moment = EMPTY;
        body.ang_accel = EMPTY;
        body.twist_peak = EMPTY;
        body.twist = EMPTY;
    }
}

/// **The ragdoll's reset**: every body of its world, in the world's order, back at its start.
pub fn reset<'a>(bodies: impl IntoIterator<Item = (&'a mut Body, &'a Start)>) {
    for (body, start) in bodies {
        start.reset(body);
    }
}

const PI: f32 = 3.1415927;
const TWO_PI: f32 = 6.2831855;
const INV_TWO_PI: f32 = 0.15915494;
const CURVE_INV_STEP: f32 = 954.77045;

/// **One bone, as the skeleton has it this frame**: its node's world position and its forward and up
/// axes, which is everything the pose reads of the skeleton.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bone {
    pub at: Vf,
    pub forward: Vf,
    pub up: Vf,
}

/// **The ragdoll's pose, for one bone**: the bone body's start put where the skeleton's bone is, and
/// the part's drawn matrix (`drawn`) rebuilt from it.
///
/// The orientation is the bone's two axes as a quaternion, on the `w >= 0` side. The start position is
/// the node's, plus the body's centre of mass turned by that orientation. Unless the bodies are to
/// start still (`still`, or a frame time that is not above zero), the velocity is how far the node
/// moved from the drawn matrix's position over the frame, and the rates are the turn from the drawn
/// matrix's orientation to this one over the frame, as an axis times an angle.
///
/// `drawn` is read before it is written, so it is last frame's matrix going in and this frame's coming
/// out.
#[allow(clippy::too_many_arguments)]
pub fn pose(bone: &Bone, cofg: Vf, drawn: &mut Mat4, start: &mut Start, inv_dt: Option<F>, acos: &AcosTable, curve: &CurveTable) {
    let q = upper(mat4_to_quat(&basis(EMPTY, bone.forward, bone.up, false, true)));
    let (velocity, rates) = match inv_dt {
        None => (EMPTY, EMPTY),
        Some(inv_dt) => {
            let last = drawn.0[3];
            let last = Vf::new(last.x(), last.y(), last.z(), 1.0);
            let prev = upper(mat4_to_quat(drawn));
            let moved = bone.at.merge(Dest::XYZ, bone.at.sub(last));
            let velocity = moved.merge(Dest::XYZ, moved.mul(Vf::splat(inv_dt.0)));
            let (angle, axis) = axis_angle(unturn(q, relative(q, prev)), acos, curve);
            let rate = angle * inv_dt;
            (velocity, axis.merge(Dest::XYZ, axis.mul(Vf::splat(rate.0))))
        }
    };
    let at = bone.at.merge(Dest::XYZ, bone.at.add(turn(q, cofg)));
    start.place(at, velocity);
    start.turn(q, rates);
    *drawn = basis(at, bone.forward, bone.up, false, true);
}

/// **The ragdoll's pose**: [`pose`] for every bone, in order. The frame time is turned into its
/// reciprocal once, and only if it is above zero; at zero or below the bodies start still whatever
/// `still` says.
pub fn pose_all<'a>(
    bones: impl IntoIterator<Item = (&'a Bone, Vf, &'a mut Mat4, &'a mut Start)>,
    still: bool,
    dt: F,
    acos: &AcosTable,
    curve: &CurveTable,
) {
    // `c.ole.s dt, 0` taken is the still branch, so a frame time that does not compare divides.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    let inv_dt = if dt <= 0.0 { None } else { Some(F(fpu::div(1.0, dt.0))) };
    let inv_dt = if still { None } else { inv_dt };
    for (bone, cofg, drawn, start) in bones {
        pose(bone, cofg, drawn, start, inv_dt, acos, curve);
    }
}

/// `q`, or `0 - q` if its `w` is below zero: the original negates on VU0, as an accumulator of zeros
/// less `q` times one.
fn upper(q: Vf) -> Vf {
    if F(q.w()) < 0.0 { Vf::msub(Vf::splat(0.0), q, Vf::splat(1.0)) } else { q }
}

/// The turn from `prev` to `q`, as the pose's one block of VU0 has it: the vector half a cross
/// product with both scalar terms folded in, the scalar half the four products summed from `w`.
fn relative(q: Vf, prev: Vf) -> Vf {
    let p = q.mul(prev);
    let acc = Vf::opmula(prev, q);
    let acc = Vf::madd(acc, q, prev.bc(Field::W));
    let acc = Vf::msub(acc, prev, q.bc(Field::W));
    let v = Vf::opmsub(acc, q, prev);
    let w = vu::madd(vu::madd(vu::add(p.w(), p.x()), 1.0, p.y()), 1.0, p.z());
    Vf::new(v.x(), v.y(), v.z(), w)
}

/// **A quaternion as an angle and an axis**, the engine's helper: the arc cosine of `w` from the
/// table, the axis the vector half over the curve table's sine of it (the entry below, not
/// interpolated), and the angle twice the arc cosine. An arc cosine of exactly zero is no turn, and
/// its axis is the constant `(1, 0, 0, 1)`.
fn axis_angle(q: Vf, acos: &AcosTable, curve: &CurveTable) -> (F, Vf) {
    let w = F(q.w());
    // The original's pair of clamps: the high test first and on *false*, so a `w` that does not compare
    // is held at one; the second pass over the clamped value changes nothing and is left out.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    let w = if !(w <= 1.0) {
        F::ONE
    } else if w < -1.0 {
        F(-1.0)
    } else {
        w
    };
    let a = acos.at(w);
    if a == 0.0 {
        return (a, Vf::new(1.0, 0.0, 0.0, 1.0));
    }
    let wrapped = wrap(wrap(a));
    let i = ((F(PI) + wrapped) * CURVE_INV_STEP).to_i32();
    let sin = F(curve.entries()[i as usize][1]);
    let inv = F(fpu::div(1.0, sin.0));
    let axis = Vf::new((F(q.x()) * inv).0, (F(q.y()) * inv).0, (F(q.z()) * inv).0, 1.0);
    (a * F(2.0), axis)
}

/// An angle into `-pi..pi`: whole turns off by truncation, then the one correction.
fn wrap(angle: F) -> F {
    let a = angle - F(TWO_PI) * F(fpu::from_i32((F(INV_TWO_PI) * angle).to_i32()));
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    if !(a <= PI) {
        a - TWO_PI
    } else if a < -PI {
        F(TWO_PI) + a
    } else {
        a
    }
}
