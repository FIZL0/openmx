//! The chassis under the bike: the rigid body the ground step pushes on, and the step that turns
//! what it pushed into motion.
//!
//! The ground step (`move.rs`, S3) spends a substep filling three accumulators on this object — a
//! force, a torque and a position impulse — and then calls one virtual, the body's own integrate,
//! which is most of this module. What that call does, in order:
//!
//! 1. the centre of gravity, through the transform, into the world;
//! 2. the transform's own orientation as a quaternion, and the turn it has taken since the body
//!    last looked, carried onto the body's orientation — this is how a bone or a parent frame
//!    moving the transform moves the body;
//! 3. the force through the inverse mass into a linear acceleration, and the torque through the
//!    inverse inertia tensor into an angular one;
//! 4. two integrations of each: acceleration into velocity into position, angular acceleration
//!    into angular velocity, and the orientation's own derivative into the orientation;
//! 5. the limits — a maximum speed, a maximum angular speed, and a per-axis window on the angular
//!    velocity — and the six freedom flags, each of which pins one axis of one of them;
//! 6. an orientation correction the contacts asked for, as an axis whose length is an angle;
//! 7. the transform written back from where the body now is, and the two frame-relative measures
//!    (the velocity in the body's frame, the angular velocity in the world) that the next frame's
//!    ground step reads;
//! 8. and, on every path including the early one, the three accumulators emptied and the contact
//!    count cleared — so a force is good for exactly one substep.
//!
//! Beside it: the per-frame measure that gives [`Chassis::speed`] its value, the transform's
//! rotate/unrotate pair that moves a vector between the body's frame and the world, the two-step
//! integrator itself, the three flags the ground step sets every frame, and the contact
//! distributor — the one call in the ground step's loop that is handed the *frame's* `dt` rather
//! than the substep's, and which never reads it.
//!
//! What is not here: the transform's own arithmetic (a matrix to a quaternion and back, the
//! quaternion derivative, a quaternion from an axis and an angle, a point through a transform).
//! Those belong to the transform and the maths module; this step calls them, so [`integrate`]
//! returns the list of calls it made, in order, with the vectors it handed over — see [`Call`].
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use ps2_float::{Acc, Dest, F, Field, Vf, vu};

/// The quadword the step leaves in the three accumulators and in the correction axis: `(0, 0, 0, 1)`,
/// which is the identity for the `w` lane a force accumulator never uses and a quaternion does.
///
/// It is one constant in the original's data, shared with the wheel's tire forces, and the
/// correction block tests against it word for word — so the port keeps it as a quadword and not as
/// four zeroes.
pub const CLEAR: Vf = Vf([0.0, 0.0, 0.0, 1.0]);

// --- the transform's rotation ----------------------------------------------------------------------

/// Three rows of a 3x3 matrix, each in the `xyz` of a quadword: a transform's rotation as it sits at
/// the transform's `+0x80`, `+0x90` and `+0xa0`, or the chassis' inverse inertia tensor at its
/// `+0xf0`, `+0x100` and `+0x110`. Both are read by the same three instructions ([`rotate`]).
///
/// The `w` lanes are part of the object and are kept: the packs [`unrotate`] builds its columns with
/// move two of them about, and the comparison against the original is word for word.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mat3 {
    pub rows: [Vf; 3],
}

impl Mat3 {
    pub fn new(r0: Vf, r1: Vf, r2: Vf) -> Mat3 {
        Mat3 { rows: [r0, r1, r2] }
    }
}

/// `m` times `v`: the rows scaled by `v`'s lanes and summed, `vmulax`/`vmadday`/`vmaddz`, with `w`
/// coming through from `v` untouched.
///
/// This is the body of the transform's "rotate a vector into the world" — without the lazy rebuild
/// of parent frames the original does first, which belongs to the transform — and it is also, to the
/// instruction, how the chassis puts a torque through its inverse inertia tensor.
pub fn rotate(m: &Mat3, v: Vf) -> Vf {
    let acc = m.rows[0].mul(v.bc(Field::X));
    let acc = Vf::madd(acc, m.rows[1], v.bc(Field::Y));
    v.merge(Dest::XYZ, Vf::madd(acc, m.rows[2], v.bc(Field::Z)))
}

/// The three columns of `m`, as the original's integer pipe packs them for [`unrotate`].
///
/// The transpose is built with five MMI moves (`pextlw`, `pexcw`, `pextuw`, `pcpyld`, `pcpyud`) and
/// then a sixth instruction that is not an MMI one at all — a plain 64-bit `or`, which writes only
/// the low half of its destination and so leaves row2's own upper half in place. That is what
/// finishes the third column: the pack supplies `x` and `y`, the register already held `z`.
///
/// The `w` lanes are each whatever the pack left there and nothing reads them; they are kept
/// because the comparison is word for word.
fn columns(m: &Mat3) -> [Vf; 3] {
    let r0 = m.rows[0].to_bits();
    let r1 = m.rows[1].to_bits();
    let r2 = m.rows[2].to_bits();
    [
        // pcpyld of the pextlw pair and row2's low half
        Vf::from_bits([r0[0], r1[0], r2[0], r2[1]]),
        // pcpyud of the same pair's high half and the pexcw'd row2's
        Vf::from_bits([r0[1], r1[1], r2[1], r2[3]]),
        // The pextuw pair moved onto row2 with a plain `or`, which is a 64-bit instruction: the
        // register keeps its own upper half, so the third column's `z` is row2's `z` and the two
        // words the pextuw put up there are thrown away. Reading that `move` as a quadword copy
        // gives a column whose `z` is row0's `w`, which is wrong and nearly plausible.
        Vf::from_bits([r0[2], r1[2], r2[2], r2[3]]),
    ]
}

/// A world vector back into the frame `m` describes: the transpose of `m` times `v`, by the same
/// three instructions [`rotate`] uses over the columns [`columns`] packs. `w` is `v`'s.
pub fn unrotate(m: &Mat3, v: Vf) -> Vf {
    let c = columns(m);
    let acc = c[0].mul(v.bc(Field::X));
    let acc = Vf::madd(acc, c[1], v.bc(Field::Y));
    v.merge(Dest::XYZ, Vf::madd(acc, c[2], v.bc(Field::Z)))
}

// --- the two-step integrator -----------------------------------------------------------------------

/// The derivative history the integrator keeps: this step's derivative and the one before it.
///
/// The original holds them as a single array of `2n` floats and finds the second half at `dx + 4n`,
/// which is why clearing an axis' acceleration (`+0x70`) also clears a word at `+0x2dc` — that word
/// is `prev[0]` of the pair whose `dx` starts at `+0x2d0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Deriv<const N: usize> {
    pub dx: [F; N],
    pub prev: [F; N],
}

impl<const N: usize> Default for Deriv<N> {
    fn default() -> Deriv<N> {
        Deriv { dx: [F(0.0); N], prev: [F(0.0); N] }
    }
}

/// The integrator the chassis reaches through a function pointer on itself, over `n` floats:
/// two-step Adams–Bashforth, `x += (dt/2) * (3 dx - prev)`, and then `prev = dx`.
///
/// Five operations per element in this order — `3 * dx`, less `prev`, times the half step, plus `x`
/// — with the half step multiplied out once before the loop. The original unrolls eight at a time
/// for `n > 8` and runs this loop otherwise; the chassis only ever asks for three or four.
pub fn advance(x: &mut [F], dx: &[F], prev: &mut [F], dt: F) {
    let h = F(0.5) * dt;
    for i in 0..x.len() {
        x[i] = x[i] + h * (F(3.0) * dx[i] - prev[i]);
        prev[i] = dx[i];
    }
}

/// [`advance`] over the `xyz` of a quadword: `w` is not the integrator's business and is left alone.
fn advance_xyz(x: Vf, d: &mut Deriv<3>, dt: F) -> Vf {
    let mut lanes = [F(x.x()), F(x.y()), F(x.z())];
    advance(&mut lanes, &d.dx, &mut d.prev, dt);
    Vf([lanes[0].0, lanes[1].0, lanes[2].0, x.w()])
}

/// [`advance`] over all four lanes, which is how an orientation is integrated.
fn advance_xyzw(x: Vf, d: &mut Deriv<4>, dt: F) -> Vf {
    let mut lanes = [F(x.x()), F(x.y()), F(x.z()), F(x.w())];
    advance(&mut lanes, &d.dx, &mut d.prev, dt);
    Vf([lanes[0].0, lanes[1].0, lanes[2].0, lanes[3].0])
}

// --- the arithmetic the step inlines ---------------------------------------------------------------

/// `v.xyz * s`, as a `qmtc2` into a register and a `vmulx.xyz` off its `x` lane. `w` is `v`'s.
fn scale_xyz(v: Vf, s: F) -> Vf {
    v.merge(Dest::XYZ, v.mul(Vf::new(s.0, s.0, s.0, s.0).bc(Field::X)))
}

/// `a.xyz + b.xyz`, `vadd.xyz`. `w` is `a`'s.
fn add_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.add(b))
}

/// `a.xyz - b.xyz`, `vsub.xyz`. `w` is `a`'s.
fn sub_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.sub(b))
}

/// `v.xyz`'s length, through the FPU's accumulator: `mula`, `madda`, `madd`, `sqrt`. The chassis
/// measures a speed and an angle with the same four instructions.
fn length_xyz(v: Vf) -> F {
    length2_xyz(v).sqrt()
}

/// The same accumulator chain without the square root: a length squared, which is what both of the
/// step's speed limits are stored as.
fn length2_xyz(v: Vf) -> F {
    Acc::mula(v.x(), v.x()).madda(v.y(), v.y()).madd(v.z(), v.z())
}

/// `v.xyz / s`, as `vdiv` into `Q` and a `vmulq.xyz` — a real divide, unlike the `vrsqrt` the
/// quaternion normalise uses. `w` is `v`'s.
fn div_xyz(v: Vf, s: F) -> Vf {
    v.merge(Dest::XYZ, v.mul_q(vu::div(1.0, s.0)))
}

/// A quaternion back onto the unit sphere: the sum of all four squares through the adder's `w` lane,
/// then `vrsqrt` — an approximate reciprocal square root, so it is not a square root and a divide,
/// and all four lanes are scaled by it.
pub(crate) fn normalize(q: Vf) -> Vf {
    let sq = q.mul(q);
    let len2 = vu::madd(vu::add(sq.w(), sq.x()), 1.0, sq.y());
    let len2 = vu::madd(len2, 1.0, sq.z());
    q.mul_q(vu::rsqrt(1.0, len2))
}

/// `-v`, written the way the original writes it: a `vmulax` off `vf0`'s `x` lane for a zero
/// accumulator, then `vmsubw` off `vf0`'s `w`, which is one. All four lanes.
fn negate(v: Vf) -> Vf {
    let acc = Vf::VF0.mul(Vf::VF0.bc(Field::X));
    Vf::msub(acc, v, Vf::VF0.bc(Field::W))
}

/// `a ⊗ b`, the quaternion product, in the chassis' own instruction order: the vector half as an
/// `vopmula`/`vopmsub` cross product with both scalar terms folded into the accumulator, and the
/// scalar half as `w - x - y - z` over the lane-by-lane product.
fn quat_mul(a: Vf, b: Vf) -> Vf {
    let p = a.mul(b);
    let acc = Vf::opmula(a, b);
    let acc = Vf::madd(acc, a, b.bc(Field::W));
    let acc = Vf::madd(acc, b, a.bc(Field::W));
    let w = vu::sub(p.w(), p.x());
    let w = vu::msub(w, 1.0, p.y());
    let w = vu::msub(w, 1.0, p.z());
    Vf::opmsub(acc, b, a).merge(Dest::W, Vf::splat(w))
}

/// `a ⊗ b*`: the turn from `b` to `a`. The same shape as [`quat_mul`] with two signs flipped — the
/// conjugate is folded into the instructions rather than taken first, so there is no negated
/// quaternion in between to round differently.
fn quat_mul_conj(a: Vf, b: Vf) -> Vf {
    let p = a.mul(b);
    let acc = Vf::opmula(b, a);
    let acc = Vf::madd(acc, a, b.bc(Field::W));
    let acc = Vf::msub(acc, b, a.bc(Field::W));
    let w = vu::add(p.w(), p.x());
    let w = vu::madd(w, 1.0, p.y());
    let w = vu::madd(w, 1.0, p.z());
    Vf::opmsub(acc, a, b).merge(Dest::W, Vf::splat(w))
}

/// `a`, turned by the quaternion `q`: ten VU0 operations, two cross products with the scalar terms
/// folded in, and not a matrix in sight. `w` is `a`'s, which is how the axis the correction block
/// hands on keeps the `w` of the vector it came from.
pub(crate) fn turn(q: Vf, a: Vf) -> Vf {
    let acc = Vf::opmula(q, a);
    let acc = Vf::madd(acc, a, q.bc(Field::W));
    let t = Vf::opmsub(acc, a, q);
    let p = q.mul(a);
    let acc = Vf::opmula(q, t);
    let acc = Vf::madd(acc, q, p.bc(Field::X));
    let acc = Vf::madd(acc, q, p.bc(Field::Y));
    let acc = Vf::madd(acc, q, p.bc(Field::Z));
    let acc = Vf::madd(acc, t, q.bc(Field::W));
    a.merge(Dest::XYZ, Vf::opmsub(acc, t, q))
}

// --- the body --------------------------------------------------------------------------------------

/// The chassis, as its own step reads and writes it. The offsets are the original's, and they are
/// named here because the ground step hands them about by offset: `move.rs`'s [`crate::r#move::Chassis`]
/// is the same object seen through the few fields that step touches.
///
/// Three groups: what the frame put in (the accumulators, the flags, the limits), what the body is
/// (where, how fast, which way round, how heavy), and the integrator's own history, which is state
/// as much as the velocity is — a body whose history is wrong moves differently for one step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Chassis {
    /// `+0x34` how fast the body is going, as a length. Written once a frame by [`measure`], not by
    /// the step, and read by the ground step's steering fade, drag and donut test.
    pub speed: F,
    /// `+0x60` where the centre of gravity is in the world. This, and not the transform's origin, is
    /// what the body integrates — the transform is written from it at the end of the step.
    pub cofg_world: Vf,
    /// `+0x70` the linear acceleration, from the force and the inverse mass.
    pub accel: Vf,
    /// `+0x80` the angular acceleration, from the torque and the inverse inertia tensor.
    pub ang_accel: Vf,
    /// `+0x90` the orientation as euler angles. Also [`measure`]'s, and nothing in the step reads it.
    pub euler: Vf,
    /// `+0xf0`, `+0x100`, `+0x110` the inverse inertia tensor.
    pub inv_inertia: Mat3,
    /// `+0x130` the force the substep accumulated, `+0x140` the torque, `+0x150` a straight push on
    /// the position that skips the velocity. All three are emptied at the end of every step.
    pub force: Vf,
    pub torque: Vf,
    pub impulse: Vf,
    /// `+0x160` an orientation correction, as an axis whose length is the angle to turn through.
    /// Emptied with the others, and tested against [`CLEAR`] word for word before it is used.
    pub correction: Vf,
    /// `+0x190` where the mass acts, in the body's own frame. The ground step moves this with the
    /// rider's weight.
    pub cofg: Vf,
    /// `+0x1a0` which way round the body is.
    pub quat: Vf,
    /// `+0x1b4` the body is awake. A sleeping body skips the whole step and only has its
    /// accumulators emptied.
    pub awake: i32,
    /// `+0x1e0` how many contacts the body has this frame, cleared at the end of every step by the
    /// one-word virtual the step finishes with.
    pub contacts: i32,
    /// `+0x214` one over the mass.
    pub inv_mass: F,
    /// `+0x220` the largest speed, and `+0x224` the largest angular speed, both squared: over them,
    /// the whole vector is scaled by `sqrt(limit / length²)`.
    pub max_speed_sq: F,
    pub max_ang_speed_sq: F,
    /// `+0x230..` and `+0x240..` a window on each axis of the angular velocity, applied after the
    /// length limit and in the body's own frame.
    pub ang_vel_max: [F; 3],
    pub ang_vel_min: [F; 3],
    /// `+0x260` the velocity, `+0x270` the same in the body's frame, `+0x280` the angular velocity in
    /// the world, `+0x290` the angular velocity in the body's frame. The two rotated ones are
    /// written at the end of the step and read by everything else.
    pub vel: Vf,
    pub local_vel: Vf,
    pub world_ang_vel: Vf,
    pub ang_vel: Vf,
    /// `+0x2a0..` may the body move along each world axis, `+0x2ac..` may it turn about each of its
    /// own. A zero pins the axis: the velocity, the acceleration and the integrator's memory of it
    /// are all cleared before anything is integrated. [`set_flags`] writes the second three.
    pub free: [i32; 3],
    pub ang_free: [i32; 3],
    /// `+0x2b8` the integrator's history for the position (whose derivative is the velocity),
    /// `+0x2d0` for the velocity, `+0x310` for the angular velocity, `+0x2f0` for the orientation —
    /// four lanes, that one.
    pub pos_hist: Deriv<3>,
    pub vel_hist: Deriv<3>,
    pub ang_hist: Deriv<3>,
    pub quat_hist: Deriv<4>,
}

/// The three flags the ground step sets on the chassis every frame, `chassis_set_flags`: which of
/// its own axes the body may turn about. The ground step passes one for the first two and "the
/// wheels are on the ground" for the third.
pub fn set_flags(ch: &mut Chassis, x: i32, y: i32, z: i32) {
    ch.ang_free = [x, y, z];
}

/// Everything [`measure`] and [`integrate`] call and this module does not port, in the order they
/// are called and with the vector or `dt` handed over. Both return them as a list, which is how the
/// order gets proved even where the callee does not.
///
/// Each variant names the original's function; `research/oracle.txt` has the addresses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    /// `quat_to_euler` — the orientation as three angles, for the measure's own records.
    QuatToEuler(Vf),
    /// The base class' per-frame update, the measure's tail call, which gets the frame's `dt`.
    FrameUpdate(F),
    /// `orientation_transform_point` — the centre of gravity, through the transform, into the world.
    PointCofg(Vf),
    /// `mat4_to_quat` — the transform's own rotation as a quaternion.
    MatrixQuat,
    /// `quat_deriv` — how fast the orientation is turning, from the orientation and the angular
    /// velocity.
    QuatDeriv(Vf, Vf),
    /// `quat_from_axis_angle` — the correction turn, from the axis (already normalised and already
    /// turned into the world) and the angle, which is the axis' own length.
    CorrectionQuat(Vf, F),
    /// `transform_rotate` — the centre of gravity rotated, with no translation, to find where the
    /// transform's origin has to go.
    RotateCofg(Vf),
    /// The transform set from the body's orientation and that origin, and then marked as moved.
    SetTransform(Vf, Vf),
    TouchTransform,
    /// `transform_unrotate` and `transform_rotate` again, through two one-line virtuals on the
    /// chassis: the velocity into the body's frame, and the angular velocity into the world.
    Unrotate(Vf),
    Rotate(Vf),
}

/// The answers the port takes as given, one per call it makes that it does not port. A field here is
/// the value the hooked callee returned; the [`Call`] beside it is the argument it was given, so
/// both halves of each seam are checked.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Given {
    /// `orientation_transform_point` of the centre of gravity.
    pub cofg_world: Vf,
    /// `mat4_to_quat` of the transform's rotation.
    pub matrix_quat: Vf,
    /// `quat_deriv` of the orientation and the angular velocity.
    pub quat_deriv: Vf,
    /// `quat_from_axis_angle` of the correction's axis and angle.
    pub correction_quat: Vf,
    /// `transform_rotate` of the centre of gravity.
    pub rotated_cofg: Vf,
    /// `transform_unrotate` of the velocity.
    pub local_vel: Vf,
    /// `transform_rotate` of the angular velocity.
    pub world_ang_vel: Vf,
}

/// The per-frame measure, `Chassis::speed`: the orientation as euler angles, the speed as the
/// velocity's length, and then the base class' own frame update with the frame's `dt`.
///
/// It is a virtual on the chassis and it is called once a frame, before the ground step — so the
/// speed the steering fade reads is the one the *previous* frame's integration left.
pub fn measure(ch: &mut Chassis, dt: F, g: Vf) -> Vec<Call> {
    let calls = vec![Call::QuatToEuler(ch.quat), Call::FrameUpdate(dt)];
    ch.euler = g;
    ch.speed = length_xyz(ch.vel);
    calls
}

/// The chassis' own integrate, the virtual the ground step ends every substep with: the force and
/// the torque the substep accumulated, turned into motion, and the transform brought along.
///
/// Returns the calls it made, in order (see [`Call`]); the original returns a constant 1, which no
/// caller reads.
pub fn integrate(ch: &mut Chassis, dt: F, g: &Given) -> Vec<Call> {
    let mut calls = Vec::new();
    if ch.awake == 0 {
        // A sleeping body still has its accumulators emptied — a force put on it this frame is gone
        // by the next one whether it moved or not.
        clear(ch);
        return calls;
    }

    // Where the mass is, in the world. The transform may have been moved by something else since
    // the last step (a parent frame, an animation), so the body takes its orientation from the
    // transform too and carries the difference onto its own.
    calls.push(Call::PointCofg(ch.cofg));
    ch.cofg_world = g.cofg_world;
    calls.push(Call::MatrixQuat);
    let mut delta = quat_mul_conj(g.matrix_quat, ch.quat);
    if delta.w() < 0.0 {
        // The same turn, the short way round: a quaternion and its negation are the same rotation,
        // and the integrator must not be handed the one that goes the long way.
        delta = negate(delta);
    }
    ch.quat = normalize(quat_mul(delta, ch.quat));

    // The force, spent. Note the order: the acceleration is written before the freedom flags are
    // read, so a pinned axis is cleared after it is filled in and not instead of it.
    ch.accel = scale_xyz(ch.force, ch.inv_mass);
    for i in 0..3 {
        if ch.free[i] == 0 {
            ch.vel.0[i] = 0.0;
            ch.accel.0[i] = 0.0;
            ch.vel_hist.prev[i] = F(0.0);
        }
    }
    ch.vel_hist.dx = [F(ch.accel.x()), F(ch.accel.y()), F(ch.accel.z())];
    ch.vel = advance_xyz(ch.vel, &mut ch.vel_hist, dt);
    ch.vel = clamp_length(ch.vel, ch.max_speed_sq);

    // The velocity, spent. The position's derivative is a copy of the velocity *after* the limit,
    // so a body held at its top speed integrates the speed it was held to.
    ch.pos_hist.dx = [F(ch.vel.x()), F(ch.vel.y()), F(ch.vel.z())];
    ch.cofg_world = advance_xyz(ch.cofg_world, &mut ch.pos_hist, dt);

    // The torque, spent, through the inverse inertia tensor — the same three instructions a
    // transform rotates a vector with.
    ch.ang_accel = rotate(&ch.inv_inertia, ch.torque);
    for i in 0..3 {
        if ch.ang_free[i] == 0 {
            ch.ang_vel.0[i] = 0.0;
            ch.ang_accel.0[i] = 0.0;
            ch.ang_hist.prev[i] = F(0.0);
        }
    }
    ch.ang_hist.dx = [F(ch.ang_accel.x()), F(ch.ang_accel.y()), F(ch.ang_accel.z())];
    ch.ang_vel = advance_xyz(ch.ang_vel, &mut ch.ang_hist, dt);
    ch.ang_vel = clamp_length(ch.ang_vel, ch.max_ang_speed_sq);
    for i in 0..3 {
        // The window, one axis at a time, against the value as it was before this loop touched it:
        // over the top it is pinned to the top, under the bottom to the bottom, and the two tests
        // are not the same shape — the first is "not less or equal", the second "less than".
        let w = ch.ang_vel.0[i];
        if w <= ch.ang_vel_max[i].0 {
            if w < ch.ang_vel_min[i].0 {
                ch.ang_vel.0[i] = ch.ang_vel_min[i].0;
            }
        } else {
            ch.ang_vel.0[i] = ch.ang_vel_max[i].0;
        }
    }

    // The orientation, integrated the same way as everything else — four lanes of a quaternion
    // through the same linear integrator, and then put back onto the unit sphere.
    calls.push(Call::QuatDeriv(ch.quat, ch.ang_vel));
    ch.quat_hist.dx = [F(g.quat_deriv.x()), F(g.quat_deriv.y()), F(g.quat_deriv.z()), F(g.quat_deriv.w())];
    ch.quat = advance_xyzw(ch.quat, &mut ch.quat_hist, dt);
    ch.quat = normalize(ch.quat);

    // The correction the contacts asked for, if they asked for one. The test is a quadword compare
    // against the constant the last step left behind, so any word at all being different — down to a
    // zero with the wrong sign — counts as a request.
    if ch.correction.to_bits() != CLEAR.to_bits() {
        let angle = length_xyz(ch.correction);
        ch.correction = div_xyz(ch.correction, angle);
        calls.push(Call::CorrectionQuat(turn(ch.quat, ch.correction), angle));
        ch.quat = quat_mul(g.correction_quat, ch.quat);
    }

    // The push on the position that skips the velocity, and then the transform follows the body:
    // its origin is where the mass is, less where the mass sits in the body's own frame.
    ch.cofg_world = add_xyz(ch.cofg_world, ch.impulse);
    calls.push(Call::RotateCofg(ch.cofg));
    let origin = sub_xyz(ch.cofg_world, g.rotated_cofg);
    calls.push(Call::SetTransform(ch.quat, origin));
    calls.push(Call::TouchTransform);

    // The two measures the next frame reads, each through a one-line virtual that does nothing but
    // pick the vector and call the transform.
    calls.push(Call::Unrotate(ch.vel));
    ch.local_vel = g.local_vel;
    calls.push(Call::Rotate(ch.ang_vel));
    ch.world_ang_vel = g.world_ang_vel;

    clear(ch);
    calls
}

/// A vector held to a length limit, which the step does twice: the length squared through the FPU's
/// accumulator, and over the limit the whole thing scaled by `sqrt(limit / length²)` — a square root
/// of a ratio, not a divide by a length, which is one rounding and not two.
fn clamp_length(v: Vf, max_sq: F) -> Vf {
    let len2 = length2_xyz(v);
    if len2 <= max_sq {
        return v;
    }
    scale_xyz(v, (max_sq / len2).sqrt())
}

/// The end of every step, on both paths: the three accumulators and the correction axis back to
/// [`CLEAR`], and the contact count to zero through the chassis' own one-word virtual.
fn clear(ch: &mut Chassis) {
    ch.force = CLEAR;
    ch.torque = CLEAR;
    ch.impulse = CLEAR;
    ch.correction = CLEAR;
    ch.contacts = 0;
}

// --- the contact distributor -----------------------------------------------------------------------

/// The five fields of a wheel a contact lands in, `wheel_set_contact`. `wheel.rs` names them too,
/// from the other side — the wheel's own step reads what this writes, so each module keeps its own
/// view and neither has to know the other's.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WheelContact {
    /// `+0x120` where the contact is and `+0x130` which way the surface faces, both whole quadwords.
    pub point: Vf,
    pub normal: Vf,
    /// `+0x1f0` the body the surface belongs to, and `+0x1f4` the vehicle, zero for either when the
    /// contact is with the world. Opaque here: the distributor only moves them.
    pub owner: u32,
    pub vehicle: u32,
    /// `+0xd4` this wheel is touching something. Cleared at the top of every substep by the ground
    /// step and set again here, once a frame.
    pub touching: i32,
}

/// One contact, as the distributor reads it out of the collision record's parallel arrays — all four
/// indexed by the same `i`, with strides of 0x20, 0x10, 0x10 and 4.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Contact {
    /// `record+0x90`'s entry, the word at its `+8`: above zero the contact is the front wheel's,
    /// zero or below the rear's. It is a distance along the bike, not a flag.
    pub select: F,
    /// `record+0x94`'s quadword: the surface normal.
    pub normal: Vf,
    /// `record+0x98`'s: the contact point.
    pub point: Vf,
    /// `record+0x9c`'s: the collision node that was hit. Only ever stored, never read through.
    pub node: u32,
    /// `node+0x80`: the body the node belongs to, zero for the world.
    pub owner: u32,
    /// `node+0x7c`: the vehicle it belongs to.
    pub vehicle: u32,
    /// `*(node+0x7c + 0x1b0)`: what kind of thing that vehicle is. It counts as a vehicle only when
    /// this is 2; anything else and the wheel is told the contact has none.
    pub vehicle_kind: i32,
    /// `*(*(owner+0x1f8) + 0xf4)`: how many listeners the owner has, each of which is told that
    /// something has touched it.
    pub listeners: i32,
}

/// What the distributor calls and this module does not port.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ContactCall {
    /// The collision record emptied, through a virtual on the search object.
    ClearRecord,
    /// `collide_against_search` — the sweep that fills the record, handed the search and the actor
    /// list.
    Collide,
    /// One listener of one contact's owner told that it has been touched: the owner, and the index
    /// of the listener in its list. The element itself comes from a list getter that is not ported.
    Notify(u32, i32),
}

/// `bike_wheel_contacts`: the frame's collision contacts spread onto the two wheels.
///
/// This is the one call in the ground step's substep loop that is handed the *frame's* `dt` rather
/// than the substep's — and it never reads it, which is why the ground step gets away with it.
///
/// `look` is the flag on the search object (`+0x116`, bit 5) that the sweep sets when there is
/// anything to look at: without it the record is not walked at all, and both wheels keep whatever
/// the substep's clear left them.
pub fn wheel_contacts(
    front: &mut WheelContact,
    rear: &mut WheelContact,
    rear_node: &mut u32,
    look: bool,
    cs: &[Contact],
    _dt: F,
) -> Vec<ContactCall> {
    *rear_node = 0;
    let mut calls = vec![ContactCall::ClearRecord, ContactCall::Collide];
    if !look {
        return calls;
    }
    for c in cs {
        // Whatever owns the surface hears about it first, once per listener and before either wheel
        // is written — a listener that moves the body is moving it before the contact is handed on.
        if c.owner != 0 {
            for i in 0..c.listeners {
                calls.push(ContactCall::Notify(c.owner, i));
            }
        }
        let vehicle = if c.vehicle != 0 && c.vehicle_kind == 2 { c.vehicle } else { 0 };
        if c.select > 0.0 {
            set_contact(front, c, vehicle);
        } else {
            set_contact(rear, c, vehicle);
            // Only the rear wheel's node is kept, and only the last one: the bike reads it to find
            // out what it is standing on.
            *rear_node = c.node;
        }
    }
    calls
}

/// `wheel_set_contact`: a contact written onto a wheel, two quadwords and three words.
pub fn set_contact(w: &mut WheelContact, c: &Contact, vehicle: u32) {
    w.point = c.point;
    w.normal = c.normal;
    w.owner = c.owner;
    w.vehicle = vehicle;
    w.touching = 1;
}
