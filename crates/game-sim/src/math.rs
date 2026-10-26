//! The matrix and quaternion helpers, in the original's own arithmetic and its own instruction order.
//!
//! Everything a bone or a camera is made of goes through these eight functions, and all eight are
//! inlined maths-header code: emitted once in the executable and called from everywhere, so getting one
//! of them wrong moves every bone in the game by a bit or two and the drift never shows up anywhere you
//! would think to look. They are ported here, ahead of the subsystems that call them, for the same
//! reason [`ps2_float`] exists at all.
//!
//! # Which model, and why it matters here
//!
//! Six of the eight are VU0 macro mode ([`ps2_float::vu`]): a chopping multiplier and a *full-width*
//! adder, with `vrsqrt` and `vdiv` coming out of a separate unit. Two — [`mat4_to_quat`] and
//! [`quat_axes`] — are plain COP1 scalar code ([`ps2_float::fpu`]): the same chopping, but the short
//! adder that drops everything past one guard bit, and the FPU's own accumulator for the `mula`/`madd`
//! and `mula`/`msub` pairs. Porting one of these with the other's model gives answers that are right to
//! six digits and wrong in the last bit, which is the worst kind of wrong, so each function below says
//! which model it is and the order it does its operations in.
//!
//! # The convention
//!
//! Matrices are four rows of four, row-vector: a point is transformed as `v · M`, the translation lives
//! in row 3, and `mul(a, b)` is "`a` then `b`". A quaternion is `(x, y, z, w)` in a [`Vf`]'s four lanes,
//! which is how the game stores one — the same layout as a vector, which is why the compiler could put
//! both through the same quadword loads.
//!
//! # The eight
//!
//! | | model | what it is |
//! |---|---|---|
//! | [`mul`] | VU0 | 4×4 by 4×4, one `vmulax`/`vmadday`/`vmaddaz`/`vmaddw` chain per row |
//! | [`transform_point`] | VU0 | a point through a matrix, with `w` forced to 1 |
//! | [`transform_points`] | VU0 | the same over a run of points — the loop the skinning code calls |
//! | [`basis`] | VU0 | the look-at: two axes, orthogonalised and normalised, plus a position |
//! | [`quat_to_mat4`] | VU0 | a rotation and a position into a matrix |
//! | [`quat_deriv`] | VU0 | ½·(q ⊗ ω): how a quaternion changes under an angular velocity |
//! | [`slerp`] | VU0 + FPU | the spherical interpolation every blended bone goes through |
//! | [`mat4_to_quat`] | FPU | the inverse of [`quat_to_mat4`]'s rotation, by largest component |
//! | [`quat_axes`] | FPU | two rows of a quaternion's matrix, without building the other two |
//!
//! [`slerp`] is the one with a dependency: its arc cosine is a 30001-entry table the game builds at
//! startup, which is already ported as [`crate::air::AcosTable`]. It is *not* read with that type's own
//! `at` — the two call sites interpolate the same two entries in a different order (see [`slerp`]) — so
//! the lookup is written out again here.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::air::AcosTable;
use ps2_float::fpu::Acc;
use ps2_float::vu::Dest;
use ps2_float::{F, Vf, fpu, vu};

/// Four rows of four, row-vector: `v · M`, translation in row 3.
///
/// A bare array of [`Vf`] rather than sixteen scalars, because that is how the original holds one: four
/// quadwords, loaded and stored whole, with every lane — including the `w` of the three rotation rows —
/// carrying a value the arithmetic depends on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4(pub [Vf; 4]);

impl Mat4 {
    /// The identity in this convention: rows 0–2 with their `w` at 0, row 3 the origin with `w` 1,
    /// which is what every function here produces for a null rotation at the origin.
    pub const IDENTITY: Mat4 = Mat4([
        Vf::new(1.0, 0.0, 0.0, 0.0),
        Vf::new(0.0, 1.0, 0.0, 0.0),
        Vf::new(0.0, 0.0, 1.0, 0.0),
        Vf::new(0.0, 0.0, 0.0, 1.0),
    ]);

    pub fn row(&self, i: usize) -> Vf {
        self.0[i]
    }

    /// The sixteen lanes in memory order, for a hash or a comparison.
    pub fn to_bits(&self) -> [u32; 16] {
        core::array::from_fn(|i| self.0[i / 4].to_bits()[i % 4])
    }
}

// --- the matrix side --------------------------------------------------------------------------------

/// `a` then `b`: each row of `a` run through `b`.
///
/// VU0, four identical chains of `vmulax`/`vmadday`/`vmaddaz`/`vmaddw` — one per row of `a`, all four
/// lanes wide, so a row's `w` is multiplied and accumulated like everything else rather than being
/// assumed to be 0 or 1. The accumulation order is `((b0·a.x + b1·a.y) + b2·a.z) + b3·a.w`, and because
/// VU0's adder is full width that order is the whole of what makes the last bit come out right.
pub fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let rows = [b.0[0].0, b.0[1].0, b.0[2].0, b.0[3].0];
    Mat4(core::array::from_fn(|i| Vf(vu::transform(&rows, a.0[i].0))))
}

/// A point through a matrix: the same chain as [`mul`]'s rows, with the input's `w` lane *replaced* by
/// 1 before it is used.
///
/// The original really does overwrite it (with a `vaddx.w` off `vf0`, the cheapest way to materialise a
/// one), so whatever the caller left in `w` is ignored — this transforms points, never directions.
pub fn transform_point(m: &Mat4, v: Vf) -> Vf {
    let rows = [m.0[0].0, m.0[1].0, m.0[2].0, m.0[3].0];
    Vf(vu::transform(&rows, [v.x(), v.y(), v.z(), 1.0]))
}

/// [`transform_point`] over a run of points, which is the form the original actually provides: it loads
/// the four rows once and then loops, so this is the function the skinning and the bounding-box code
/// call rather than the single-point one.
///
/// The original takes a count and two byte strides and does nothing at all when the count is zero or
/// negative; a slice carries both, so the only thing worth keeping is that it writes exactly `n` points
/// and reads exactly `n`.
pub fn transform_points(out: &mut [Vf], src: &[Vf], m: &Mat4) {
    let rows = [m.0[0].0, m.0[1].0, m.0[2].0, m.0[3].0];
    for (o, v) in out.iter_mut().zip(src) {
        *o = Vf(vu::transform(&rows, [v.x(), v.y(), v.z(), 1.0]));
    }
}

/// The look-at: a position and two axes into a matrix whose rows are an orthonormal frame.
///
/// Row 1 is the normalised `up`, row 2 the normalised `forward`, row 0 their cross product, and row 3
/// the position with `w` 1; the three rotation rows get `w` 0. The two flags pick how much
/// orthogonalising happens first, and they are the original's own two arguments:
///
/// - `orthogonalise` false — `forward` and `up` are used as given, so a caller that already has a frame
///   pays for nothing but the two normalisations.
/// - `orthogonalise` true, `keep_forward` true — `up` is replaced by `forward × (up × forward)`, so the
///   forward axis survives exactly and the up axis is the one that moves. This is the camera's case.
/// - `orthogonalise` true, `keep_forward` false — the other way round: `forward` becomes
///   `(up × forward) × up`.
///
/// Both normalisations are VU0's `vrsqrt`, the approximate reciprocal square root, not a square root
/// and a divide — a different number in the last bits, and the reason this cannot be written with
/// [`ps2_float::vu::normalize`].
pub fn basis(pos: Vf, forward: Vf, up: Vf, orthogonalise: bool, keep_forward: bool) -> Mat4 {
    let (mut forward, mut up) = (forward, up);
    if orthogonalise {
        // `vopmula`/`vopmsub` with the operands swapped between the two, which is how a cross product
        // comes out of the outer-product pair. Port the pair, not the idiom: the operand order here is
        // the original's.
        let right = Vf::opmsub(Vf::opmula(up, forward), forward, up);
        if keep_forward {
            up = Vf::opmsub(Vf::opmula(forward, right), right, forward);
        } else {
            forward = Vf::opmsub(Vf::opmula(right, up), up, right);
        }
    }
    // Each axis: the sum of squares as `vmul` then `vaddy.x` then `vaddz.x`, then `Q = 1/sqrt(it)` and a
    // broadcast `vmulq.xyz`. The `w` lanes are cleared with a self-subtract, not with a constant.
    let nu = Vf(up.0).mul_q(vu::rsqrt(1.0, vu::dot3(up.0, up.0)));
    let nf = Vf(forward.0).mul_q(vu::rsqrt(1.0, vu::dot3(forward.0, forward.0)));
    let (nu, nf) = (nu.merge(Dest::W, Vf::ZERO), nf.merge(Dest::W, Vf::ZERO));
    let right = Vf::opmsub(Vf::opmula(nu, nf), nf, nu).merge(Dest::W, Vf::ZERO);
    Mat4([right, nu, nf, Vf::new(pos.x(), pos.y(), pos.z(), 1.0)])
}

// --- quaternion to matrix and back ------------------------------------------------------------------

/// A rotation and a position into a matrix.
///
/// VU0. The quaternion is doubled once (`q2 = q + q`) and then every term of the matrix is one
/// broadcast multiply against a lane of it, which is why the rows come out as
/// `2x² + 2w² − 1` on the diagonal rather than the `1 − 2y² − 2z²` a textbook writes: the same value for
/// a unit quaternion, a different one for anything else, and the original's choice.
///
/// The three rotation rows get `w` 0 (the hardware's cheapest zero: `vf0` rotated into the `w` lane),
/// row 3 is the position with `w` 1.
pub fn quat_to_mat4(q: Vf, pos: Vf) -> Mat4 {
    let q2 = q.add(q);
    // `vmulw`/`vmulx`/`vmuly`/`vmulz` against the four lanes of the doubled quaternion.
    let qw = q.mul(q2.bc(vu::Field::W));
    let rx = q.mul(q2.bc(vu::Field::X));
    let ry = q.mul(q2.bc(vu::Field::Y));
    let rz = q.mul(q2.bc(vu::Field::Z));
    // `2w² − 1`, which goes on all three diagonal entries.
    let diag = vu::sub(qw.w(), 1.0);
    let row0 = Vf::new(vu::add(rx.x(), diag), vu::add(rx.y(), qw.z()), vu::sub(rx.z(), qw.y()), 0.0);
    let row1 = Vf::new(vu::sub(ry.x(), qw.z()), vu::add(ry.y(), diag), vu::add(ry.z(), qw.x()), 0.0);
    let row2 = Vf::new(vu::add(rz.x(), qw.y()), vu::sub(rz.y(), qw.x()), vu::add(rz.z(), diag), 0.0);
    Mat4([row0, row1, row2, Vf::new(pos.x(), pos.y(), pos.z(), 1.0)])
}

/// Two rows of a quaternion's rotation matrix without building the other two: the forward axis (row 2)
/// and the up axis (row 1), which between them are everything an aimed object needs.
///
/// This one is scalar FPU, not VU0 — the short adder and the FPU accumulator — and its arithmetic is
/// *not* [`quat_to_mat4`]'s with two rows deleted. It writes the diagonal as `1 − (2x² + 2z²)` where the
/// matrix builder writes `2y² + 2w² − 1`, it shares the three doubled products `2x`, `2y`, `2z` across
/// both rows, and the two terms that need a sign change go through `mula`/`msub` and `mula`/`madd`
/// pairs. Returns `(forward, up)`, three floats each, in the order the original stores them.
pub fn quat_axes(q: Vf) -> ([F; 3], [F; 3]) {
    let (qx, qy, qz, qw) = (F(q.x()), F(q.y()), F(q.z()), F(q.w()));
    let (x2, y2, z2) = (qx * F(2.0), qy * F(2.0), qz * F(2.0));
    let zz = qz * z2;
    let xx = qx * x2;
    let wx = qw * x2;
    let xx_zz = xx + zz;
    let yz = qy * z2;
    let one_minus_xx_zz = F::ONE - xx_zz;
    let yy = qy * y2;
    let xx_yy = xx + yy;

    let up = [
        Acc::mula(qx, y2).msub(qw, z2), // 2xy − 2wz
        one_minus_xx_zz,                // 1 − (2x² + 2z²)
        yz + wx,                        // 2yz + 2wx
    ];
    let one_minus_xx_yy = F::ONE - xx_yy;
    let forward = [
        Acc::mula(qx, z2).madd(qw, y2), // 2xz + 2wy
        yz - wx,                        // 2yz − 2wx
        one_minus_xx_yy,                // 1 − (2x² + 2y²)
    ];
    (forward, up)
}

/// The rotation part of a matrix back into a quaternion, by largest component.
///
/// Scalar FPU. Four candidates are formed first — a quarter of `1 ± m00 ± m11 ± m22` in its four sign
/// patterns, which are `w²`, `x²`, `y²` and `z²` for a rotation matrix — and the largest one is the
/// component taken as a square root, with the other three falling out of the off-diagonal pairs divided
/// by four times it. Taking the largest is what keeps the division away from zero.
///
/// The comparison chain is the original's, `<=` and in its order, so an exact tie goes the same way:
/// `w` wins only if it is strictly greater than all three others, then `x` only if strictly greater
/// than `y` and `z`, then `y` only if strictly greater than `z`.
// The negated comparisons below are the original's own shape: it tests `c.olt.s`/`c.ole.s` and
// branches on *false*, which is the branch an unordered pair takes too. Rewriting them the positive
// way would be the other branch on a pair that does not compare.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn mat4_to_quat(m: &Mat4) -> Vf {
    let (m00, m11, m22) = (F(m.0[0].x()), F(m.0[1].y()), F(m.0[2].z()));
    let (m01, m02) = (F(m.0[0].y()), F(m.0[0].z()));
    let (m10, m12) = (F(m.0[1].x()), F(m.0[1].z()));
    let (m20, m21) = (F(m.0[2].x()), F(m.0[2].y()));
    let quarter = F(0.25);

    // The four candidates, in the order and the association the original builds them:
    // `(1 + m00)` and `(1 − m00)` once each, then each combined with m11 and m22.
    let plus = F::ONE + m00;
    let minus = F::ONE - m00;
    let cw = quarter * (m22 + (plus + m11));
    let cx = quarter * ((plus - m11) - m22);
    let cy = quarter * ((minus + m11) - m22);
    let cz = quarter * (m22 + (minus - m11));

    if !(cw <= cx || cw <= cy || cw <= cz) {
        let w = cw.sqrt();
        let s = quarter / w;
        Vf::new((s * (m12 - m21)).0, (s * (m20 - m02)).0, (s * (m01 - m10)).0, w.0)
    } else if !(cx <= cy || cx <= cz) {
        let x = cx.sqrt();
        let s = quarter / x;
        Vf::new(x.0, (s * (m01 + m10)).0, (s * (m02 + m20)).0, (s * (m12 - m21)).0)
    } else if !(cy <= cz) {
        let y = cy.sqrt();
        let s = quarter / y;
        Vf::new((s * (m01 + m10)).0, y.0, (s * (m12 + m21)).0, (s * (m20 - m02)).0)
    } else {
        let z = cz.sqrt();
        let s = quarter / z;
        Vf::new((s * (m02 + m20)).0, (s * (m12 + m21)).0, z.0, (s * (m01 - m10)).0)
    }
}

// --- the two that move a quaternion -----------------------------------------------------------------

/// `½·(q ⊗ ω)`: the rate of change of a rotation under an angular velocity, which is what an integrator
/// adds to a quaternion each step.
///
/// VU0. `ω` is read as a pure quaternion — its `w` lane is never touched — and the half is not applied
/// at the end but carried in a constant quadword of `(0.5, 0.5, 0.5, −0.5)`, so the scalar lane comes
/// out as three separately-halved products summed, not as a dot product halved. That is a different
/// number, and it is the original's.
pub fn quat_deriv(q: Vf, omega: Vf) -> Vf {
    const HALF: Vf = Vf::new(0.5, 0.5, 0.5, -0.5);
    // The three lane products, kept for the scalar part.
    let prod = q.mul(omega);
    // `q × ω + ω·q.w`, as `vopmula` / `vmaddaw` / `vopmsub` — one accumulator, three instructions.
    let acc = Vf::opmula(q, omega);
    let acc = Vf::madd(acc, omega, q.bc(vu::Field::W));
    let v = Vf::opmsub(acc, omega, q);
    // The scalar part: `−0.5·x − 0.5·y − 0.5·z`, accumulated in that order.
    let w = vu::madd(
        vu::madd(vu::mul(HALF.w(), prod.x()), HALF.w(), prod.y()),
        HALF.w(),
        prod.z(),
    );
    // Only `xyz` are halved at the end — `w` was built halved, one term at a time, above.
    Vf::new(vu::mul(v.x(), HALF.x()), vu::mul(v.y(), HALF.y()), vu::mul(v.z(), HALF.z()), w)
}

/// Spherical linear interpolation, the one every blended bone goes through.
///
/// `t` is how far from `a` towards `b`. With `shortest` set, a negative dot product flips `b`'s
/// contribution so the interpolation takes the short way round; with it clear the quaternions are taken
/// as given, which is what the keyframe decoders want because their data is already consistent.
///
/// The shape is the textbook one — `(a·sin((1−t)θ) + b·sin(tθ)) / sin θ` — but three details are not:
///
/// 1. **The guard is on `|cos θ| < 1`, and it returns `a` unchanged.** Not a lerp fallback, not a
///    normalisation: the original branches straight over its own store, so whatever the caller had in
///    the destination for the nearly-parallel case is `a`, the quaternion it loaded first.
/// 2. **θ comes out of a table.** The game builds a 30001-entry arc cosine table at startup and
///    interpolates two entries of it; `acos` itself is never called at runtime. The index and the weight
///    both come from `(|cos θ| + 1)·14999.5`. The interpolation is `lo − t·lo + t·hi` through the FPU
///    accumulator (`adda`, `msuba`, `madd`) — [`crate::air::AcosTable::at`] is the *same* two entries
///    weighted the same way but written `(1 − t)·lo + t·hi` with a `mula`, which is a different last
///    bit, so this reads the table itself rather than calling that.
/// 3. **The three sines are a Taylor polynomial, on VU0, three lanes at once.** `sin(x) ≈ x·(1 + s²·(−⅙
///    + s²·(¹⁄₁₂₀ + s²·(−¹⁄₅₀₄₀))))` with `s = x`, evaluated for `θ`, `t·θ` and `(1 − t)·θ` in the x, y
///    and z lanes of one register. `1/sin θ` is a `vdiv`.
///
/// The one place this does not follow the instruction stream to the letter: the original applies the
/// `shortest` sign to the `t·θ` lane *between* squaring that lane and the final `x·p` multiply, and
/// this applies it to the lane up front. A chopping multiply's sign is the exclusive-or of its
/// operands' signs and its magnitude does not depend on them, so the square is the same quadword
/// either way and so is the product; doing it up front keeps the polynomial a function of one vector.
// The negated comparisons below are the original's own shape: it tests `c.olt.s`/`c.ole.s` and
// branches on *false*, which is the branch an unordered pair takes too. Rewriting them the positive
// way would be the other branch on a pair that does not compare.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn slerp(a: Vf, b: Vf, t: F, shortest: bool, table: &AcosTable) -> Vf {
    // The four-lane dot product, accumulated `((x + y) + z) + w` on VU0's full-width adder.
    let p = a.mul(b);
    let dot = vu::madd(vu::madd(vu::add(p.x(), p.y()), 1.0, p.z()), 1.0, p.w());

    let cos = F(dot).abs();
    if !(cos < F::ONE) {
        return a;
    }
    // ±1, built out of the dot's sign bit and the flag with integer ops, then used as a multiplier.
    let sign = if shortest { dot.to_bits() & 0x8000_0000 } else { 0 };
    let signed_one = F::from_bits(sign | 0x3f80_0000);

    let theta = acos_from_table(table, cos);
    // `t` reaches the angles through a broadcast add with zero, which is where a denormal argument is
    // flushed and a minus zero becomes a plus zero; both lanes below are that one sum, not the
    // argument.
    let t = vu::add(0.0, t.0);
    // The three angles, in the lanes the polynomial runs over: θ, ±t·θ, (1 − t)·θ.
    let ta = vu::mul(t, theta.0);
    let ua = vu::msub(vu::add(0.0, theta.0), t, theta.0);
    let ang = Vf::new(theta.0, vu::mul(ta, signed_one.0), ua, 0.0);
    let sin = sin_poly(ang);

    let q = vu::div(1.0, sin.x());
    let out = Vf::madd(a.mul(sin.bc(vu::Field::Z)), b, sin.bc(vu::Field::Y));
    out.mul_q(q)
}

/// The arc cosine lookup as the interpolation code does it: the index and the weight out of one
/// `(x + 1)·scale`, the integer part by `cvt.w.s`, and the two entries combined through the FPU
/// accumulator as `lo − weight·lo + weight·hi`.
fn acos_from_table(table: &AcosTable, x: F) -> F {
    let s = Acc::adda(F::ZERO, F(ACOS_SCALE)).madd(x, F(ACOS_SCALE));
    let i = s.to_i32();
    let w = s - F(fpu::from_i32(i));
    let (lo, hi) = (F(table.entries()[i as usize]), F(table.entries()[i as usize + 1]));
    Acc::adda(F::ZERO, lo).msuba(w, lo).madd(w, hi)
}

/// What the startup code scales an arc cosine lookup's argument by: 30000 entries spanning −1 to 1.
const ACOS_SCALE: f32 = 14999.5;

/// `sin` of three angles at once, as the seventh-order Taylor polynomial the interpolation carries in
/// one constant quadword, evaluated innermost-first on VU0's accumulator: each step is a `vadda` of the
/// next coefficient followed by a `vmadd` of the squared angle.
fn sin_poly(x: Vf) -> Vf {
    // The quadword as it is written in the executable, not as a decimal: `-0.1666667` spelled out is
    // two of the last bits away from the word the game actually loads, and two of the last bits in a
    // coefficient is two of the last bits in every interpolated bone.
    const C: Vf = Vf::new(
        f32::from_bits(0x3f80_0000),
        f32::from_bits(0xbe2a_aaab),
        f32::from_bits(0x3c08_8889),
        f32::from_bits(0xb950_0d01),
    );
    let sq = x.mul(x);
    let p = Vf::madd(Vf::splat(C.z()), sq, Vf::splat(C.w()));
    let p = Vf::madd(Vf::splat(C.y()), sq, p);
    let p = Vf::madd(Vf::splat(C.x()), sq, p);
    x.mul(p)
}

/// A quaternion as three Euler angles, the way the engine's own `quat_to_euler` does it — the
/// conversion the chassis measures its pitch and roll with every frame, and so the one that feeds
/// `air.rs`'s whole preload-and-flip machine.
///
/// The three angles come out as `(x, y, z)` with `w` of one, and they are all **negated**: the
/// original computes an arc cosine and writes `-v`, which makes `y` a heading that grows clockwise.
/// Each one is the same four steps — two terms of the rotation matrix, the first divided by the
/// length of the pair through the *approximate* reciprocal square root, clamped to `[-1, 1]` twice
/// (the clamp is written out twice, not called twice, and the second pass can only matter for a NaN),
/// and read off the arc cosine table — with the sign taken from the second term.
///
/// `z` and `y` share nothing, but `x` reuses `z`'s sum: its first term is `sqrt(a² + c²)`, the length
/// of the pair `z` normalised. A quaternion whose `a` and `c` are both zero therefore hands `rsqrt` a
/// zero, which on the FPU is an infinity the clamp catches rather than a NaN.
pub fn quat_to_euler(q: Vf, table: &AcosTable) -> Vf {
    let (qx, qy, qz, qw) = (F(q.x()), F(q.y()), F(q.z()), F(q.w()));
    let two = F(2.0);
    let xx = qx * qx;
    // The two matrix diagonals, each `1 - 2(…)`, and the three off-diagonal pairs, each through the
    // accumulator with the *product* of the two imaginary parts first.
    let a = F::ONE - two * (xx + qz * qz);
    let b = F::ONE - two * (xx + qy * qy);
    let c = two * Acc::mula(qx, qy).madd(qw, qz);
    let d = two * Acc::mula(qx, qz).madd(qw, qy);
    let e = two * Acc::mula(qy, qz).msub(qw, qx);

    // `y`: the pair is `(b, d)`, and the accumulator takes the *sign* term's square first.
    let angle = |first: F, sum: F, sign: F| {
        let t = first * F::ONE.rsqrt(sum);
        let v = table.at(clamp_unit(clamp_unit(t)));
        if sign < F::ZERO { v } else { -v }
    };
    let y = angle(b, Acc::mula(d, d).madd(b, b), d);
    let zs = Acc::mula(c, c).madd(a, a);
    let z = angle(a, zs, c);
    // `x`'s first term is the length of `z`'s pair, by a real square root this time.
    let r = zs.sqrt();
    let x = angle(r, Acc::mula(e, e).madd(r, r), -e);
    Vf::new(x.0, y.0, z.0, 1.0)
}

/// `[-1, 1]` as the original's two compares and two moves, which is not `min`/`max`: a branch on an
/// ordered compare and `min.s` disagree about a NaN, and anything unordered comes back as `1.0`.
/// `move.rs` has the same four lines for the same reason.
fn clamp_unit(c: F) -> F {
    if c <= F::ONE {
        if c < F(-1.0) { F(-1.0) } else { c }
    } else {
        F::ONE
    }
}

// --- the inverse ------------------------------------------------------------------------------------

/// The inverse of an affine matrix, as the original's general inverse computes it: the adjugate of
/// the 3×3 rotation part and of the translation (column 3 is taken to be `(0, 0, 0, 1)` and never
/// read), transposed, then every lane of every row times one reciprocal of the determinant. `None`
/// where the determinant is zero, and then the original writes nothing.
///
/// FPU for the cofactors and the determinant, each one an accumulator pair (`mula` then `msub`, or a
/// `suba` then a `madd`) in the order below, and `div.s` for the reciprocal; VU0 for the scale, a
/// `vmulx.xyzw` per row. So a diagonal matrix does not come back as each lane's own reciprocal: the
/// middle one is `(a·c)·(1/(a·(b·c)))`, an ulp off `1/b` as often as not, and every zero cofactor that
/// is a negated `0 − 0` comes out `−0`. This is how a rigid body is handed its inertia tensor, and the
/// inverse is what its torque goes through (S107).
pub fn inverse(m: &Mat4) -> Option<Mat4> {
    let e = |r: usize, c: usize| F(m.0[r].0[c]);
    let (m0, m1, m2) = (e(0, 0), e(0, 1), e(0, 2));
    let (m4, m5, m6) = (e(1, 0), e(1, 1), e(1, 2));
    let (m8, m9, m10) = (e(2, 0), e(2, 1), e(2, 2));
    let (m12, m13, m14) = (e(3, 0), e(3, 1), e(3, 2));

    // The determinant, the inverse's own arithmetic — not the adjugate's, which works it out again
    // in another order for its corner lane.
    let minor = Acc::mula(m5, m10).msub(m6, m9);
    let a = m0 * minor;
    let b = m4 * Acc::mula(m1, m10).msub(m2, m9);
    let c = m1 * m6 - m2 * m5;
    let det = Acc::suba(a, b).madd(m8, c);
    if det == F::ZERO {
        return None;
    }
    let s = F::ONE / det;

    // The adjugate, row by row as it lands on the stack before the transpose.
    let c00 = Acc::mula(m5, m10).msub(m6, m9);
    let c01 = -Acc::mula(m4, m10).msub(m6, m8);
    let c02 = Acc::mula(m4, m9).msub(m5, m8);
    let t0 = Acc::mula(m9, m14).msub(m10, m13);
    let t1 = Acc::mula(m5, m14).msub(m6, m13);
    let c03 = -Acc::suba(m4 * t0, m8 * t1).madd(m12, c00);
    let u0 = Acc::mula(m1, m10).msub(m2, m9);
    let c10 = -u0;
    let c11 = Acc::mula(m0, m10).msub(m2, m8);
    let c12 = -Acc::mula(m0, m9).msub(m1, m8);
    let u1 = Acc::mula(m1, m14).msub(m2, m13);
    let c13 = Acc::suba(m0 * t0, m8 * u1).madd(m12, u0);
    let u2 = Acc::mula(m1, m6).msub(m2, m5);
    let c21 = -Acc::mula(m0, m6).msub(m2, m4);
    let c22 = Acc::mula(m0, m5).msub(m1, m4);
    let c23 = -Acc::suba(m0 * t1, m4 * u1).madd(m12, u2);
    let c33 = Acc::suba(m0 * c00, m4 * u0).madd(m8, u2);
    let adj = [
        [c00, c01, c02, c03],
        [c10, c11, c12, c13],
        [u2, c21, c22, c23],
        [F::ZERO, F::ZERO, F::ZERO, c33],
    ];
    Some(Mat4(core::array::from_fn(|r| {
        Vf(core::array::from_fn(|c| vu::mul(adj[c][r].0, s.0)))
    })))
}
