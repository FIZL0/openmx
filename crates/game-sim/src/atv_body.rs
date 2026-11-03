//! **The quad's rigid body** (MD75), ported from MX vs. ATV Unleashed's PS2 ELF (SLUS-21104): the
//! physics world's integrate and end-of-substep calls on the body the quad rides on.
//!
//! It is the engine's generic dynamics body, the same object as MX Unleashed's (the biplane's,
//! [`crate::biplane::Body`]), but SLUS-21104's build of it is a later one and steps differently:
//!
//! - the linear half keeps the acceleration it worked out (clamped to a top length) instead of the
//!   substep's `h`, takes `x += v h + (a/2 h) h` term by term, and clamps the speed afterwards;
//! - each linear and angular axis can be locked, which zeroes that lane of the acceleration and the
//!   velocity first;
//! - the angular half clamps the angular acceleration to a length and each rate to its own limit,
//!   and folds `|q'|^2` on the FPU (`x`, `y`, `z`, `w`) where MX Unleashed sums it on the VU.
//!
//! [`Body`] is described by what the two calls read and write; the offsets are research/oracle.txt's
//! (`# MD75`). Proof: `tests/atv_quad.rs`, against SLUS-21104's own integrate and end-of-substep.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::chassis::{CLEAR, normalize};
use ps2_float::vu::{self, Dest, Field};
use ps2_float::{F, Vf, fpu::Acc};

/// The quaternion derivative's scale as the spin helper loads it: `(1/2, 1/2, 1/2, -1/2)`.
const HALF: Vf = Vf([0.5, 0.5, 0.5, -0.5]);

/// The limits read as "no limit": a clamp is skipped while its limit is exactly this.
const NO_LIMIT: f32 = f32::MAX;

/// The quad's rigid body. Everything is in the world frame except the moment, the inverse inertia,
/// the rates and the angular acceleration, which are the body's own. `w` lanes are kept as the
/// original leaves them, because it stores whole quadwords.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Body {
    /// The world integrates the body only while this is set.
    pub enabled: bool,
    /// Which world axes the body may move along; a locked one has its acceleration and velocity
    /// zeroed each substep.
    pub linear_axes: [bool; 3],
    /// One over the mass. Zero pins the body in place: the linear step is skipped.
    pub inv_mass: F,
    /// What is kept of the velocity each substep, per axis.
    pub damping: Vf,
    /// Where the body was at the start of the last substep, and is now.
    pub last_position: Vf,
    pub position: Vf,
    pub velocity: Vf,
    /// The force of the last substep, and this one's.
    pub last_force: Vf,
    pub force: Vf,
    /// The linear acceleration the last substep moved by (clamped), `w` the force's.
    pub accel: Vf,
    /// A second force accumulator emptied each substep, and the largest magnitude per axis it
    /// reached since the quad's PreProcess last reset it ([`crate::quad::Body::reset_each_frame`]).
    pub correction_peak: Vf,
    pub correction: Vf,
    /// The top speed and the top linear acceleration ([`NO_LIMIT`] for none).
    pub max_speed: F,
    pub max_accel: F,
    /// Which body axes the body may turn about.
    pub angular_axes: [bool; 3],
    /// The inverse inertia, a diagonal in the body frame. Exactly `(0, 0, 0, 1)` pins the orientation.
    pub inv_inertia: Vf,
    /// What is kept of the rates each substep, per axis.
    pub ang_damping: Vf,
    /// Which way round the body was at the start of the last substep, and is now.
    pub last_quat: Vf,
    pub quat: Vf,
    /// The moment of this substep, and the last one's.
    pub moment: Vf,
    pub last_moment: Vf,
    /// The angular velocity.
    pub rates: Vf,
    /// The angular acceleration of the last substep (clamped), `w` the inverse inertia's.
    pub ang_accel: Vf,
    /// The moment's twin of [`Body::correction`] and its peak.
    pub twist_peak: Vf,
    pub twist: Vf,
    /// Each rate's limit either way, and the top angular acceleration ([`NO_LIMIT`] for none).
    pub max_rates: [F; 3],
    pub max_ang_accel: F,
}

/// `v.xyz * s`, `w` from `v`: a `vmulx.xyz` off a scalar moved in.
fn scale(v: Vf, s: F) -> Vf {
    v.merge(Dest::XYZ, v.mul(Vf::splat(s.0)))
}

/// `a.xyz + b.xyz`, `w` from `a`.
fn add_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.add(b))
}

/// `a.xyz * b.xyz`, `w` from `a`.
fn mul_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.mul(b))
}

/// `v` cut down to `max` long when it is longer (or its length is not a number): the length on the
/// FPU (`mula`, `madda`, `madd`, `sqrt`), the direction on the VU (`x + y`, then `+ 1·z`, `vrsqrt`).
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's `c.ole.s` branched on false
fn clamp_length(v: Vf, max: F) -> Vf {
    if max == NO_LIMIT {
        return v;
    }
    let len = Acc::mula(v.x(), v.x()).madda(v.y(), v.y()).madd(v.z(), v.z()).sqrt();
    if len <= max {
        return v;
    }
    let sq = v.mul(v);
    let len2 = vu::madd(vu::add(sq.x(), sq.y()), 1.0, sq.z());
    let unit = v.merge(Dest::XYZ, v.mul_q(vu::rsqrt(1.0, len2)));
    scale(unit, max)
}

/// `q ⊗ (w, 0)` scaled by [`HALF`]: the vector half as a `vopmula`/`vopmsub` cross product with the
/// scalar term folded in, the scalar half `-1/2` times each of the three products, accumulated.
fn spin(q: Vf, w: Vf) -> Vf {
    let p = q.mul(w);
    let acc = Vf::opmula(q, w);
    let acc = Vf::madd(acc, w, q.bc(Field::W));
    let v = Vf::opmsub(acc, w, q).mul(HALF);
    let s = vu::madd(vu::madd(vu::mul(HALF.w(), p.x()), HALF.w(), p.y()), HALF.w(), p.z());
    Vf::new(v.x(), v.y(), v.z(), s)
}

impl Body {
    /// The linear half of a substep. The position is remembered first, even when the body is pinned.
    ///
    /// `a = force * inv_mass` (stored with the force's `w`), the locked axes zeroed in it and in the
    /// velocity, `a` clamped to [`Body::max_accel`]; then `x += v h + ((a/2) h) h`, `v = damping * v +
    /// a h` (`w` the damping's), and the speed clamped to [`Body::max_speed`].
    pub fn advance(&mut self, h: F) {
        self.last_position = self.position;
        if self.inv_mass == 0.0 {
            return;
        }
        self.accel = scale(self.force, self.inv_mass);
        for k in 0..3 {
            if !self.linear_axes[k] {
                self.accel[k] = 0.0;
                self.velocity[k] = 0.0;
            }
        }
        self.accel = clamp_length(self.accel, self.max_accel);
        let a = self.accel;
        let curve = scale(scale(scale(a, F(0.5)), h), h);
        let slide = scale(self.velocity, h);
        self.position = add_xyz(add_xyz(self.position, slide), curve);
        let push = scale(a, h);
        let kept = mul_xyz(self.damping, self.velocity);
        self.velocity = clamp_length(add_xyz(kept, push), self.max_speed);
    }

    /// One substep of motion, the world's integrate call: [`Body::advance`], then the orientation.
    ///
    /// `alpha = inv_inertia * moment` (stored with the inverse inertia's `w`), the locked axes zeroed
    /// in it and in the rates, `alpha` clamped to [`Body::max_ang_accel`]. The quaternion takes the
    /// second-order step `q' = q ⊗ (rates, 0) / 2`, `q'' = q ⊗ (alpha, 0) / 2 - q |q'|^2`,
    /// `q += q' h + ((q''/2) h) h` over all four lanes, then back onto the unit sphere. The rates take
    /// `ang_damping * rates + alpha h`, each then held within its [`Body::max_rates`] either way.
    #[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's `c.ole.s` branched on false
    pub fn integrate(&mut self, h: F) {
        self.advance(h);
        self.last_quat = self.quat;
        if self.inv_inertia.to_bits() == CLEAR.to_bits() {
            return;
        }
        self.ang_accel = mul_xyz(self.inv_inertia, self.moment);
        for k in 0..3 {
            if !self.angular_axes[k] {
                self.ang_accel[k] = 0.0;
                self.rates[k] = 0.0;
            }
        }
        self.ang_accel = clamp_length(self.ang_accel, self.max_ang_accel);
        let (q, alpha) = (self.quat, self.ang_accel);
        let d = spin(q, self.rates);
        let n2 = Acc::mula(d.x(), d.x()).madda(d.y(), d.y()).madda(d.z(), d.z()).madd(d.w(), d.w());
        let dd = spin(q, alpha).sub(q.mul(Vf::splat(n2.0)));
        let curve = dd.mul(Vf::splat(0.5)).mul(Vf::splat(h.0)).mul(Vf::splat(h.0));
        let turn = d.mul(Vf::splat(h.0));
        self.quat = normalize(q.add(turn).add(curve));
        let push = scale(alpha, h);
        let kept = mul_xyz(self.ang_damping, self.rates);
        self.rates = add_xyz(kept, push);
        for k in 0..3 {
            let (r, m) = (F(self.rates[k]), self.max_rates[k]);
            if !(r <= m) {
                self.rates[k] = m.0;
            } else if r < -m {
                self.rates[k] = (-m).0;
            }
        }
    }

    /// The world's second call per body per substep: each correction lane's peak kept by magnitude,
    /// the force remembered and emptied with the correction; the same for the twist and the moment.
    #[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's `c.ole.s` branched on false
    pub fn end_substep(&mut self) {
        for k in 0..3 {
            if !(F(self.correction[k]).abs() <= F(self.correction_peak[k]).abs()) {
                self.correction_peak[k] = self.correction[k];
            }
        }
        self.last_force = self.force;
        self.force = CLEAR;
        self.correction = CLEAR;
        for k in 0..3 {
            if !(F(self.twist[k]).abs() <= F(self.twist_peak[k]).abs()) {
                self.twist_peak[k] = self.twist[k];
            }
        }
        self.last_moment = self.moment;
        self.moment = CLEAR;
        self.twist = CLEAR;
    }

    /// What the quad's listeners read and write of the body.
    pub fn view(&self) -> crate::quad::Body {
        crate::quad::Body {
            force: self.force,
            moment: self.moment,
            orientation: self.quat,
            ang_vel: self.rates,
            velocity: self.velocity,
            reset_each_frame: self.correction_peak,
        }
    }

    /// The listeners' writes put back.
    pub fn set_view(&mut self, b: &crate::quad::Body) {
        self.force = b.force;
        self.moment = b.moment;
        self.quat = b.orientation;
        self.rates = b.ang_vel;
        self.velocity = b.velocity;
        self.correction_peak = b.reset_each_frame;
    }
}
