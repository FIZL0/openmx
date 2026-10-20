//! The tire: what one wheel's contact patch pushes back with, given the load on it, the torque the
//! engine and the brake have put on the wheel, and how fast the patch is sliding.
//!
//! This is the original's own tire force, `tire_patch_force` in `research/oracle.txt`, which the
//! wheel's spin integration calls once a substep with the patch's velocity already turned into the
//! contact's own frame (`x` across the wheel, `z` along it). It is the function P4 stood in for with a
//! linear side force capped by the load; P24 found that the stand-in's shape is what scrubbed half the
//! speed off a held turn, and ported this in its place.
//!
//! Two forces come out of it, both per unit load times the load:
//!
//! - **along the wheel**, the drive: the torque over the radius, held to `MaxDriveForce`. Past that
//!   limit the tire breaks loose and the force drops to `SkidDriveForce` — forwards with the wheel
//!   *spinning*, backwards with it *locked*, and those two flags are what the spin integration and the
//!   engine read back. A donut command breaks a driving tire loose at any torque.
//! - **across the wheel**, the side force, picked out of three families by the brake and by whether
//!   the tire is already spinning or locked: plain cornering, slipping and braking. Each family is
//!   either a straight line saturated at a `Max…`/`Skid…` pair (the old key set) or, when the tire
//!   says `UseParametricCurve`, a curve that rises with the slide and then falls away as
//!   `1 / (1 + |k·slide|^DropOff)` — which is what the game's own bikes use, and what lets a tire that
//!   is past its peak keep most of its grip rather than pinning at a cap.
//!
//! Below ten units a second of rolling speed the slide is magnified, up to five times at a standstill,
//! so that a slow bike still steers.
//!
//! Every operation is the original's, in its order: proved bit-exact over the oracle in
//! `tests/tire.rs`.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::camera::CurveTable;
use crate::motion::remap;
use crate::phy::Surface;
use ps2_float::fpu::Acc;
use ps2_float::{F, Vf, libm};

/// What the tire force reads off the wheel.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Grip {
    /// The torque accumulated on the wheel this substep: the engine's and the surface drag's.
    pub torque: F,
    /// One over the tire's radius.
    pub inv_radius: F,
    /// The load on the patch: the suspension's spring force.
    pub load: F,
    /// The donut command's share, `0..1`, which the rear tire lerps its slipping curve by.
    pub donut: F,
    /// The tire's `UseParametricCurve`.
    pub parametric: bool,
    /// The brake lever this wheel reads. Only whether it is exactly zero matters here.
    pub brake: F,
    /// The patch's velocity in the contact's frame: across the wheel, and along it.
    pub slide: F,
    pub roll: F,
}

/// The three flags the tire force leaves on the wheel. `sliding` is only written by the linear
/// families: the parametric curves leave whatever was there.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flags {
    pub spinning: bool,
    pub locked: bool,
    pub sliding: bool,
}

/// The two forces, in the contact's frame: `side` across the wheel, `drive` along it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Force {
    pub side: F,
    pub drive: F,
}

/// Rolling speeds under this magnify the slide.
const SLOW: f32 = 10.0;
/// How much they magnify it: this at a standstill, falling to one at [`SLOW`].
const SLOW_GAIN: f32 = 5.0;

/// The tire force, `tire_patch_force`.
pub fn force(g: &Grip, s: &Surface, flags: &mut Flags) -> Force {
    let load = g.load;
    let mut drive = g.torque * g.inv_radius;
    let cap = s.max_drive_force * load;
    let skid = if !(drive <= cap) || (!(g.donut <= F::ZERO) && !(drive <= F::ZERO)) {
        Some(true)
    } else if !(drive < -cap) || cap <= F::ZERO {
        None
    } else {
        Some(false)
    };
    match skid {
        Some(true) => {
            drive = s.skid_drive_force * load;
            *flags = Flags { spinning: true, locked: false, ..*flags };
        }
        Some(false) => {
            drive = -s.skid_drive_force * load;
            *flags = Flags { spinning: false, locked: true, ..*flags };
        }
        None => *flags = Flags { spinning: false, locked: false, ..*flags },
    }

    let mut slide = g.slide;
    let roll = g.roll.abs();
    if roll < F(SLOW) {
        slide = slide * remap(roll, F::ZERO, F(SLOW), F(SLOW_GAIN), F::ONE);
    }

    let side = if !(g.brake == F::ZERO) {
        if g.parametric {
            curve(slide, load, s.braking_side_force_round_off, s.braking_side_force_drop_off,
                  s.braking_side_force_scale, s.braking_side_force_speed_scale)
        } else {
            line(slide, load, s.slipping_side_force, s.max_slipping_side_force, s.skid_slipping_side_force,
                 flags)
        }
    } else if !flags.spinning && !flags.locked {
        if g.parametric {
            curve(slide, load, s.side_force_round_off, s.side_force_drop_off, s.side_force_scale,
                  s.side_force_speed_scale)
        } else {
            line(slide, load, s.side_force, s.max_side_force, s.skid_side_force, flags)
        }
    } else if g.parametric {
        let k = remap(g.donut, F::ZERO, F::ONE, s.slipping_side_force_speed_scale,
                      s.donuts_side_force_speed_scale);
        curve(slide, load, s.slipping_side_force_round_off, s.slipping_side_force_drop_off,
              s.slipping_side_force_scale, k)
    } else {
        line(slide, load, s.slipping_side_force, s.max_slipping_side_force, s.skid_slipping_side_force,
             flags)
    };
    Force { side, drive }
}

/// A linear family: the slide times the stiffness, until that passes the limit, and then the skid
/// force with the slide's sign.
fn line(slide: F, load: F, stiffness: F, max: F, skid: F, flags: &mut Flags) -> F {
    let f = slide * (-stiffness * load);
    if f.abs() <= max * load {
        flags.sliding = false;
        return f;
    }
    let sign = if f < F::ZERO { F(-1.0) } else { F::ONE };
    flags.sliding = true;
    load * (skid * sign)
}

/// A parametric family: `slide·k·(−scale·load) / (1 + |slide·round_off·k|^drop_off)`, with `k` the
/// family's speed scale.
fn curve(slide: F, load: F, round_off: F, drop_off: F, scale: F, k: F) -> F {
    let fall = F(libm::powf((slide * (round_off * k)).abs().0, drop_off.0));
    (slide * (k * (-scale * load))) / (F::ONE + fall)
}

/// What the surface takes out of a wheel, `wheel_surface_drag`: a torque onto the wheel's torque
/// accumulator, and on the ground a drag along the chassis' own `z`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Drag {
    /// The accumulator afterwards.
    pub torque: F,
    /// The drag, in the chassis' own frame, as the `z` of `(0, 0, z)`: on the ground only. The caller
    /// turns it into the world and adds it to the chassis' force.
    pub force_z: Option<F>,
}

/// How fast a rolling wheel's torques fade in: `0.3 ×` the chassis' forward speed, clamped to ±1.
const FADE_RATE: f32 = 0.3;

/// `wheel_surface_drag`. `torque` is the wheel's accumulator as it stands, `forward` the `z` of the
/// chassis' velocity in its own frame (`+0x198`), `spin` the wheel's (`+0x1d4`), and `brake` the
/// lever the wheel reads, or `None` for a wheel with no lever wired.
///
/// One signed fade is made first — on the ground out of the forward speed, in the air out of the
/// spin — and both torques are that fade times a number of the surface's: `WheelFriction` always,
/// and `MaxBrakingTorque` times the lever when the lever is off its stop. The drag is
/// `-WheelDrag · v·|v|` and is not scaled by the load.
pub fn surface_drag(torque: F, contact: bool, forward: F, spin: F, brake: Option<F>, s: &Surface) -> Drag {
    let (fade, force_z) = if contact {
        let fade = (F(FADE_RATE) * forward).max(F(-1.0)).min(F::ONE);
        let sign = if forward < F::ZERO { F::ONE } else { F(-1.0) };
        (fade, Some(s.wheel_drag * (forward * (forward * sign))))
    } else {
        (spin.max(F(-1.0)).min(F::ONE), None)
    };
    let mut torque = torque + -fade * s.wheel_friction;
    if let Some(brake) = brake {
        if !(brake <= F::ZERO) {
            torque = torque + s.max_braking_torque * (fade * -brake.max(F::ZERO));
        }
    }
    Drag { torque, force_z }
}

/// The wheel's own inverse spin inertia, `+0x64`, as `wheel_set_tire` works it out from the radius
/// alone: a disc of the wheel's radius with the two literals the loader passes beside the file's
/// numbers. `TireInertia` is *not* in it — that one only feeds the gyroscopic term. A radius of zero
/// or less leaves the field unwritten in the original; here it is zero.
pub fn spin_inv_inertia(radius: F) -> F {
    // `wheel_set_tire`'s third argument over twelve, and its own 2π.
    let half = F(6.0) / F(12.0);
    let area = half * ((F(core::f32::consts::TAU) * radius) * radius);
    if radius <= F::ZERO {
        return F::ZERO;
    }
    F::ONE / (radius * (radius * ((F(0.5) * area) * half)))
}

/// Which way a steered wheel points, in the chassis' own frame: the chassis' `z` turned by `angle`
/// about the hinge `axis` (a unit vector, the wheel's `+0x8c0`). This is the third row of the
/// original's axis-angle matrix (`axis_angle_matrix` in `research/oracle.txt`), the only row the
/// wheel's contact frame reads — the cosine and sine come off the same table and wrap as
/// [`CurveTable::at`]'s, and the row is the cross of the first two, in the original's order. `w` is
/// zero, a direction.
///
/// The hinge is the fork, raked back off the chassis' `y`, so a turned bar swings the wheel's
/// forward less than its own angle across the ground and tips it down a little: the frame the tire
/// force is measured in flattens that back onto the surface.
pub fn steered_forward(angle: F, axis: Vf, curve: &CurveTable) -> Vf {
    let (c, s) = curve.at(angle);
    let (x, y, z) = (F(axis.x()), F(axis.y()), F(axis.z()));
    let (xx, yy, xy) = (x * x, y * y, x * y);
    let (xz, yz) = (x * z, y * z);
    let m0 = xx + c * (F::ONE - xx);
    let m5 = yy + c * (F::ONE - yy);
    let cxy = -(c * xy);
    let m1 = xy + (cxy + s * z);
    let m2 = xz + Acc::mula(s, -y).madd(c, -xz);
    let m4 = xy + (cxy - s * z);
    let m6 = yz + Acc::mula(c, -yz).madd(s, x);
    Vf::new(
        Acc::mula(m1, m6).msub(m2, m5).0,
        Acc::mula(m2, m4).msub(m0, m6).0,
        Acc::mula(m0, m5).msub(m1, m4).0,
        0.0,
    )
}
