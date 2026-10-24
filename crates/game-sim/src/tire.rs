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
use ps2_float::{Acc, Dest, F, Field, Vf, fpu, libm, vu};

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

// --- the spin step, whole (S58) ---------------------------------------------------------------------

/// `wheel_set_tire`'s two fixed arguments: every caller in the executable passes these.
const TIRE_WIDTH: f32 = 6.0;
const TIRE_PLY: f32 = 24.0;
/// The constant `wheel_set_tire` divides the width by.
const TIRE_WIDTH_DIV: f32 = 12.0;
/// The scale the spin step hands the steering hinge its tire force with.
const HINGE_SCALE: f32 = 0.25;
/// The surface drag's fade rate: the wheel's speed along the chassis, one unit per this many feet a
/// second.
const DRAG_FADE: f32 = 0.3;
const PI: f32 = 3.1415927;
const TWO_PI: f32 = 6.2831855;
const INV_TWO_PI: f32 = 0.15915494;

/// What `wheel_set_tire` works out from a tire's radius: `(1/r, inertia, 1/inertia)` — the words at
/// `+0x838`, `+0x60` and `+0x64`. A radius that is not positive leaves all three alone, which is
/// `None`.
///
/// The inertia is not the file's `TireInertia`: that goes to `+0x68` and only the gyroscopic torque
/// reads it. The spin's own inertia is a disc of the tire's radius and a mass the radius makes,
/// `(w/12) · 2πr · r` with `w` the width the loader passes (6): `r · (r · ((m/2) · w/12))`. The
/// disc's bike, at a radius of 1.2, has `1/I` = 0.614.
pub fn spin_inertia(radius: F) -> Option<(F, F, F)> {
    if radius <= F::ZERO {
        return None;
    }
    let share = F(TIRE_WIDTH) / F(TIRE_WIDTH_DIV);
    let inertia = radius * (radius * ((F(0.5) * tire_mass(radius)) * share));
    Some((F::ONE / radius, inertia, F::ONE / inertia))
}

/// `wheel_set_tire`'s `+0x220`, the tire's mass, written whatever the radius:
/// `(6/12) · ((2π · r) · r)`.
pub fn tire_mass(radius: F) -> F {
    let share = F(TIRE_WIDTH) / F(TIRE_WIDTH_DIV);
    share * ((F(TWO_PI) * radius) * radius)
}

/// `wheel_set_tire`'s `+0x8a8`, which nothing in the ride reads: `1 / (6 · (24 + 5))`.
pub fn tire_ply_scale() -> F {
    F::ONE / (F(TIRE_WIDTH) * (F(5.0) + F(TIRE_PLY)))
}

/// `a × b` as the VU0 writes it: `vopmula` then `vopmsub`. Only the `xyz` lanes are meant; every
/// caller merges or adds them.
fn cross(a: Vf, b: Vf) -> Vf {
    Vf::opmsub(Vf::opmula(a, b), b, a)
}

fn add_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.add(b))
}

fn sub_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.sub(b))
}

/// The EE's three-instruction dot product, `mula.s`/`madda.s`/`madd.s`, `a`'s lanes first.
fn dot3(a: Vf, b: Vf) -> F {
    Acc::mula(F(a.x()), F(b.x())).madda(F(a.y()), F(b.y())).madd(F(a.z()), F(b.z()))
}

/// `v.xyz · s` as `qmtc2` and `vmulx.xyz`; `w` is `v`'s.
fn scale(v: Vf, s: F) -> Vf {
    v.merge(Dest::XYZ, v.mul(Vf::new(s.0, s.0, s.0, s.0).bc(Field::X)))
}

/// What the spin step reads off the bike besides the wheel itself.
#[derive(Clone, Copy, Debug)]
pub struct Step {
    pub dt: F,
    /// The lever the wheel's brake pointer (`+0x30`) reads.
    pub brake: F,
    /// `UseParametricCurve` (`+0x40`).
    pub parametric: bool,
    /// The other wheel's contact flag, through `+0xcc`: a wheel in the air still runs the tire law
    /// while its partner is down.
    pub other_contact: bool,
    /// `BodyTireForcePosition`, the arm the tire's force turns the chassis on (`+0xb0`).
    pub arm: Vf,
    /// The chassis transform's row 2, its forward (`transform+0xa0`).
    pub forward: Vf,
    /// The chassis' angular velocity in its own frame (`chassis+0x290`).
    pub ang_vel: Vf,
}

/// `wheel_surface_drag`: what the surface under the wheel takes out of it.
///
/// On the ground, the fade is the chassis' own speed along itself (`+0x198`) at 0.3 per foot a
/// second, held to ±1, and the drag is `WheelDrag · v · |v|` against it, pushed along the chassis'
/// `z` through the transform and into the force accumulator: no couple. In the air the fade is the
/// spin, held to ±1. Then, on the ground or not, `−fade · WheelFriction` goes on the drive torque,
/// and so does `MaxBrakingTorque · (fade · −brake)` when the lever is off its stop.
///
/// `rotate` is `transform_rotate`, asked once on the ground.
pub fn surface_drag(wh: &mut crate::ride::Wheel, s: &Surface, brake: F, c: &mut crate::wheel::Chassis,
                    rotate: &mut impl FnMut(Vf) -> Vf) {
    let fade = if wh.w.contact {
        let vz = F(wh.chassis_local_velocity.z());
        let fade = (F(DRAG_FADE) * vz).max(F(-1.0)).min(F::ONE);
        // `-1` or `1`, negated and converted: the drag's sign, against the motion.
        let sign = F::from(-(if vz < F::ZERO { -1 } else { 1 }));
        let drag = s.wheel_drag * (vz * (vz * sign));
        let world = rotate(Vf::new(0.0, 0.0, drag.0, 1.0));
        c.force = add_xyz(c.force, world);
        fade
    } else {
        wh.mv.spin.max(F(-1.0)).min(F::ONE)
    };
    wh.mv.drive_torque = wh.mv.drive_torque + (-fade) * s.wheel_friction;
    if !(brake <= F::ZERO) {
        wh.mv.drive_torque = wh.mv.drive_torque + s.max_braking_torque * (fade * (-brake.max(F::ZERO)));
    }
}

/// `wheel_spin_integrate`: the tire's force through the contact patch, and the wheel's own spin.
///
/// The force law ([`force`]) runs whenever either wheel is down, on the [`contact_frame`]; with both in
/// the air the local velocity and force become `(0, 0, 0, 1)` and the frame is last substep's. The
/// force goes back out through the frame and into the chassis on the `.phy`'s arm — or, for a steered
/// wheel being balanced on, through the hinge at a quarter ([`hinge_force`]).
///
/// The spin is then the law's to decide: locked, it is zero; rolling on the ground, it is the patch's
/// speed along the tire over the radius; spinning, or in the air, the drive torque less the forward
/// force at the radius spins it up through `1/I`. The angle at `+0x1d0` turns with it and is wrapped
/// to ±π. The drive torque is spent. A wheel being balanced on gives the chassis its gyroscopic
/// torque, `ω × (−TireInertia · spin, 0, 0)`; a steered one adds the hinge's rate to `ω` and hands
/// the torque on through [`gyro_torque`]. Last, `+0x234` is the side force per pound of load.
///
/// `xf` is the chassis transform: the contact frame's rotate and the torque arm's unrotate. The
/// call [`HINGE_SCALE`] sends through [`hinge_force`] asks it twice more.
///
/// The share path (`+0xc4`, the other wheel's load copied over) is the caller's: `ride.rs`'s
/// `share_load` runs it before this, for a rear hanging while the front is down (S67). Not ported,
/// and not reached by any wheel on the disc (recording2's eight have it zero): the hinge-torque path
/// (`+0xc0`).
/// `+0x224`/`+0x228`, which the both-in-the-air case zeroes, are not kept: nothing in the port
/// reads them.
pub fn spin_integrate(wh: &mut crate::ride::Wheel, s: &Surface, st: &Step, c: &mut crate::wheel::Chassis,
                      curve: &CurveTable, xf: &mut impl FnMut(crate::wheel::Call) -> Vf) {
    use crate::wheel::Call;
    let unit_w = Vf::new(0.0, 0.0, 0.0, 1.0);
    if !wh.w.contact && !st.other_contact {
        wh.local_velocity = unit_w;
        wh.local_force = unit_w;
    } else {
        let (frame, local) = contact_frame(st.forward, wh.w.steered, wh.w.contact, wh.w.steer_angle,
                                           wh.steer_axis, wh.w.contact_normal, wh.w.contact_velocity,
                                           curve, &mut |v| xf(Call::Rotate(v)));
        wh.frame = frame.0;
        wh.local_velocity = local;
        let contact = Contact {
            load: wh.w.spring_force,
            drive_torque: wh.mv.drive_torque,
            inv_radius: wh.mv.inv_radius,
            donut: wh.mv.donut_scale,
            brake: st.brake,
            parametric: st.parametric,
            velocity: local,
        };
        let (side, fwd) = force(s, &contact, &mut wh.slip);
        wh.local_force = Vf::new(side.0, 0.0, fwd.0, wh.local_force.w());
    }

    // Out through the frame: `vmulax`, `vmadday`, `vmaddz`, the local force's own `w`.
    let lf = wh.local_force;
    let acc = wh.frame[0].mul(lf.bc(Field::X));
    let acc = Vf::madd(acc, wh.frame[1], lf.bc(Field::Y));
    let world = lf.merge(Dest::XYZ, Vf::madd(acc, wh.frame[2], lf.bc(Field::Z)));
    if wh.w.balancing && wh.w.steered {
        let at = wh.w.load_point;
        hinge_force(wh, c, F(HINGE_SCALE), world, at, xf);
    } else {
        c.force = add_xyz(c.force, world);
        let local = xf(Call::Unrotate(world));
        c.torque = add_xyz(c.torque, cross(st.arm, local));
    }

    let spin = if wh.slip.locked {
        F::ZERO
    } else if !wh.slip.spinning && wh.w.contact {
        F(wh.local_velocity.z()) * wh.mv.inv_radius
    } else {
        let net = wh.mv.drive_torque - F(wh.local_force.z()) * wh.mv.radius;
        wh.mv.spin + st.dt * (net * wh.inv_inertia)
    };
    wh.mv.spin = spin;

    let a = wh.angle + spin * st.dt;
    let turns = F(fpu::from_i32((F(INV_TWO_PI) * a).to_i32()));
    let r = a - F(TWO_PI) * turns;
    wh.angle = if !(r <= F(PI)) {
        r - F(TWO_PI)
    } else if r < F(-PI) {
        F(TWO_PI) + r
    } else {
        r
    };
    wh.mv.drive_torque = F::ZERO;

    if wh.w.balancing {
        let g = Vf::new(((-wh.gyro) * spin).0, 0.0, 0.0, 0.0);
        if wh.w.steered {
            let hinge = scale(wh.steer_axis, wh.w.steer_rate);
            let w = add_xyz(st.ang_vel, hinge);
            gyro_torque(wh, c, F::ONE, cross(w, g));
        } else {
            c.torque = add_xyz(c.torque, cross(st.ang_vel, g));
        }
    }

    let load = wh.w.spring_force;
    wh.side_per_load = if load <= F::ZERO { F::ZERO } else { F(wh.local_force.x()) / load };
    wh.mv.spin_from_torque = wh.slip.spinning;
    wh.mv.contact_force_fwd = F(wh.local_force.z());
}

/// `wheel_hinge_force`: a force at a world point, put through the steering hinge. The chassis gets
/// it whole — the force, and the couple of its arm from the centre of gravity in the chassis' frame,
/// times `(1, 1, 1, 1)` — and the hinge gets `scale` times the moment about the fork's axis of the
/// force's arm from the wheel's mount (`+0x150`), taken into the chassis' frame.
///
/// `xf` answers `Unrotate` (twice) and `Unpoint` (once), in the order the original asks.
pub fn hinge_force(wh: &mut crate::ride::Wheel, c: &mut crate::wheel::Chassis, scale: F, force: Vf,
                   point: Vf, xf: &mut impl FnMut(crate::wheel::Call) -> Vf) {
    use crate::wheel::Call;
    let one = Vf::new(1.0, 1.0, 1.0, 1.0);
    let lf = xf(Call::Unrotate(force));
    let lp = xf(Call::Unpoint(point));
    let arm = sub_xyz(lp, c.cofg);
    c.force = add_xyz(c.force, force);
    let couple = arm.merge(Dest::XYZ, cross(arm, lf));
    c.torque = add_xyz(c.torque, one.merge(Dest::XYZ, one.mul(couple)));

    let reach = sub_xyz(point, wh.w.mount_world);
    let moment = reach.merge(Dest::XYZ, cross(reach, force));
    let local = xf(Call::Unrotate(moment));
    wh.w.steer_torque = wh.w.steer_torque + scale * dot3(local, wh.steer_axis);
}

/// The spin step's gyroscopic helper: a torque already in the chassis' frame, added to the chassis
/// whole and `scale` times its part about the fork's axis to the hinge.
pub fn gyro_torque(wh: &mut crate::ride::Wheel, c: &mut crate::wheel::Chassis, scale: F, t: Vf) {
    let t = Vf::new(t.x(), t.y(), t.z(), 1.0);
    c.torque = add_xyz(c.torque, t);
    wh.w.steer_torque = wh.w.steer_torque + scale * dot3(t, wh.steer_axis);
}
