//! **The tire's grip**: the frame the tire pushes along, and how hard it pushes along each of its axes.
//!
//! The wheel's spin step (`wheel_spin_integrate` in `research/oracle.txt`) does three things before it
//! turns the wheel: it builds a frame at the contact patch out of the wheel's heading and the ground's
//! normal ([`contact_frame`]), takes the contact's velocity into that frame, and hands the result to the
//! tire's force law ([`force`]), which says how much the tire pushes sideways and along. The spin step
//! then turns that force back out through the frame and puts it into the chassis.
//!
//! The force law is a per-surface lookup and nothing else. Sideways, it is either linear in the slide
//! velocity — capped, and past the cap a flat "sliding" force — or, for a tire with
//! `UseParametricCurve`, a curve that rises with the slide and rounds off:
//!
//! ```text
//!   side = vx · P · (−Scale · load) / (1 + |vx · RoundOff · P| ^ DropOff)
//! ```
//!
//! with one of three key sets (braking, slipping or plain side) picked by the brake lever and whether
//! the wheel is spinning or locked. Every bike on the disc has the curve. Below 10 ft/s along the tire,
//! the slide velocity is multiplied by up to five, which is what holds a slow bike straight.
//!
//! The scalar code is COP1 ([`ps2_float::fpu`]) and the frame's two transforms are VU0, the same as
//! everything else the wheel does.
#![deny(clippy::float_arithmetic)]

use crate::camera::CurveTable;
use crate::chassis::{self, Mat3};
use crate::math::{self, Mat4};
use crate::phy::Surface;
use ps2_float::{Acc, Dest, F, Vf, libm, vu};

/// Below this speed along the tire (ft/s), the slide velocity the force law sees is scaled up.
const SLOW: f32 = 10.0;
/// The scale at a standstill; it falls linearly to 1 at [`SLOW`].
const SLOW_GAIN: f32 = 5.0;

/// The three force-model flags the law writes into the wheel and reads back the next time.
///
/// `spinning` and `locked` are rewritten on every call. `sliding` is only written by the linear law;
/// the curve leaves it as it was.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Slip {
    /// The drive torque asked for more than the surface can take: the tire spins and pushes with the
    /// skid drive force instead.
    pub spinning: bool,
    /// The brake asked for more than the surface can take the other way: the wheel locks.
    pub locked: bool,
    /// The sideways force went past its cap and is the flat skid force.
    pub sliding: bool,
}

/// What the force law reads off the wheel, besides the surface's table row.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Contact {
    /// The suspension's spring force: the tire's normal load.
    pub load: F,
    /// The torque the drive and brake put on the wheel this substep.
    pub drive_torque: F,
    /// One over the tire's radius.
    pub inv_radius: F,
    /// How far into a donut the rider is, 0 to 1.
    pub donut: F,
    /// The brake lever the wheel is wired to.
    pub brake: F,
    /// Whether the tire's file has `UseParametricCurve`.
    pub parametric: bool,
    /// The contact's velocity in the [`contact_frame`]: `x` across the tire, `z` along it.
    pub velocity: Vf,
}

/// The tire's force at the contact, in the [`contact_frame`]: `(side, forward)`. The frame's `y`
/// lane, along the normal, is always zero.
///
/// `wheel_tire_drive_force` in `research/oracle.txt`.
pub fn force(s: &Surface, c: &Contact, slip: &mut Slip) -> (F, F) {
    let load = c.load;
    let mut fwd = c.drive_torque * c.inv_radius;
    let cap = s.max_drive_force * load;
    if !(fwd <= cap) || (!(c.donut <= F::ZERO) && !(fwd <= F::ZERO)) {
        fwd = s.skid_drive_force * load;
        (slip.spinning, slip.locked) = (true, false);
    } else if !(fwd < -cap) || cap <= F::ZERO {
        (slip.spinning, slip.locked) = (false, false);
    } else {
        fwd = (-s.skid_drive_force) * load;
        (slip.spinning, slip.locked) = (false, true);
    }

    let mut vx = F(c.velocity.x());
    let along = F(c.velocity.z()).abs();
    if along < F(SLOW) {
        vx = vx * map_clamp(along, F::ZERO, F(SLOW), F(SLOW_GAIN), F::ONE);
    }

    let side = if c.brake != F::ZERO {
        if c.parametric {
            curve(vx, load, s.braking_side_force_speed_scale, s.braking_side_force_round_off,
                  s.braking_side_force_drop_off, s.braking_side_force_scale)
        } else {
            linear(vx, load, s.slipping_side_force, s.max_slipping_side_force,
                   s.skid_slipping_side_force, slip)
        }
    } else if slip.spinning || slip.locked {
        if c.parametric {
            let p = map_clamp(c.donut, F::ZERO, F::ONE, s.slipping_side_force_speed_scale,
                              s.donuts_side_force_speed_scale);
            curve(vx, load, p, s.slipping_side_force_round_off, s.slipping_side_force_drop_off,
                  s.slipping_side_force_scale)
        } else {
            linear(vx, load, s.slipping_side_force, s.max_slipping_side_force,
                   s.skid_slipping_side_force, slip)
        }
    } else if c.parametric {
        curve(vx, load, s.side_force_speed_scale, s.side_force_round_off, s.side_force_drop_off,
              s.side_force_scale)
    } else {
        linear(vx, load, s.side_force, s.max_side_force, s.skid_side_force, slip)
    };
    (side, fwd)
}

/// The linear law: `−k · load` per unit of slide, and past `max · load` the flat `skid · load` in the
/// slide's direction.
fn linear(vx: F, load: F, k: F, max: F, skid: F, slip: &mut Slip) -> F {
    let side = vx * ((-k) * load);
    if side.abs() <= max * load {
        slip.sliding = false;
        return side;
    }
    let sign = if side < F::ZERO { F::from(-1) } else { F::ONE };
    slip.sliding = true;
    load * (skid * sign)
}

/// The parametric curve, with `p` its speed scale, `r` the round-off, `d` the drop-off and `scale`
/// the gain. `powf` is the C library's, through the executable's one thunk to it.
fn curve(vx: F, load: F, p: F, r: F, d: F, scale: F) -> F {
    let shape = F(libm::powf((vx * (r * p)).abs().0, d.0));
    (vx * (p * ((-scale) * load))) / (F::ONE + shape)
}

/// `x` mapped from `a..b` onto `out0..out1` and held inside it. `move.rs` has the same five lines.
fn map_clamp(x: F, a: F, b: F, out0: F, out1: F) -> F {
    let u = (x - a) / (b - a);
    let v = out0.lerp(out1, u);
    v.max(out0.min(out1)).min(out0.max(out1))
}

/// The rotation by `angle` about the unit `axis`, as the engine's general axis-angle routine builds it:
/// the nine rotation entries of a row-major matrix, with the third row rebuilt as the cross product of
/// the first two. The three `w`s and the fourth row are not written; they are zero here.
///
/// `axis_rotation_matrix` in `research/oracle.txt`.
pub fn axis_rotation(angle: F, axis: Vf, curve: &CurveTable) -> [Vf; 3] {
    let (c, s) = curve.at(angle);
    let (x, y, z) = (F(axis.x()), F(axis.y()), F(axis.z()));
    let xx = x * x;
    let xz = x * z;
    let yz = y * z;
    let sz = s * z;
    let m0 = xx + c * (F::ONE - xx);
    let yy = y * y;
    let xy = x * y;
    let m5 = yy + c * (F::ONE - yy);
    let cxy = -(c * xy);
    let m1 = xy + (cxy + sz);
    let m4 = xy + (cxy - sz);
    let m2 = xz + Acc::mula(s, -y).madd(c, -xz);
    let m6 = yz + Acc::mula(c, -yz).madd(s, x);
    let m8 = Acc::mula(m1, m6).msub(m2, m5);
    let m9 = Acc::mula(m2, m4).msub(m0, m6);
    let m10 = Acc::mula(m0, m5).msub(m1, m4);
    [
        Vf::new(m0.0, m1.0, m2.0, 0.0),
        Vf::new(m4.0, m5.0, m6.0, 0.0),
        Vf::new(m8.0, m9.0, m10.0, 0.0),
    ]
}

/// The axis a fork steers about: from the shocks' node to the handlebars', normalised the VU0 way.
/// `w` is the handlebars'.
pub fn steering_axis(handlebars: Vf, shocks: Vf) -> Vf {
    let d = handlebars.merge(Dest::XYZ, handlebars.sub(shocks));
    let sq = d.mul(d);
    let len2 = vu::madd(vu::add(sq.x(), sq.y()), 1.0, sq.z());
    d.merge(Dest::XYZ, d.mul_q(vu::rsqrt(1.0, len2)))
}

/// The frame the tire pushes along, and the contact's velocity in it.
///
/// The frame's forward is the chassis' own forward — or, for a steered wheel on the ground, the chassis
/// forward turned by the steer angle about the fork's [`steering_axis`]. Its up is the ground's normal,
/// and [`math::basis`] squares the forward to it, so row 0 is across the tire, row 1 the normal and
/// row 2 along it. The velocity comes back through the frame's transpose.
///
/// `rotate` is the chassis transform turning one of its own directions into the world (the original
/// calls the transform for it, which may rebuild it first); it is only asked for a steered wheel on the
/// ground.
///
/// `wheel_contact_frame` in `research/oracle.txt`.
pub fn contact_frame(chassis_forward: Vf, steered: bool, in_contact: bool, steer: F, axis: Vf,
                     normal: Vf, velocity: Vf, curve: &CurveTable,
                     rotate: &mut impl FnMut(Vf) -> Vf) -> (Mat4, Vf) {
    let forward = if in_contact && steered {
        let r = axis_rotation(steer, axis, curve)[2];
        rotate(Vf::new(r.x(), r.y(), r.z(), 1.0))
    } else {
        Vf::new(chassis_forward.x(), chassis_forward.y(), chassis_forward.z(), 1.0)
    };
    let frame = math::basis(Vf::new(0.0, 0.0, 0.0, 1.0), forward, normal, true, false);
    let local = chassis::unrotate(&Mat3::new(frame.0[0], frame.0[1], frame.0[2]), velocity);
    (frame, local)
}
