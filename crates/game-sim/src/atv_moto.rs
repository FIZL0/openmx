//! **The quad's Moto, part 2** (MD75b), ported from MX vs. ATV Unleashed's PS2 ELF (SLUS-21104): the
//! calls the quad's three listeners make that had no port of their own, answered by [`crate::quad::Steps`]
//! alongside the bike steps MD75a ported.
//!
//! - The PreProcess's [`auto_shift`] (the gearbox, on the quad's speed and the left rear's radius)
//!   and [`head_tail`] (the centre of mass moved with the stick, the brake and the clutch ramp).
//! - The Force body's [`gravity`], [`engine_update`] (two driven wheels), [`air_drag`],
//!   [`wheel_clear`], [`rev_limit`] and [`peak_rates`].
//!
//! Each is MX Unleashed's step of the same name where SLUS-21104 kept it (`crate::r#move`), with the
//! differences written out where they fall: the engine drives one wheel *or two*, halving the torque
//! between them, and reads the road's revs off the first one's spin alone; the head and tail picks
//! one of three poses; the gravity lives on the vehicle, not the body, and it pokes the collision
//! record. The field map is research/oracle.txt's (`# MD75b`). Proof: `tests/atv_quad.rs`, each call
//! unhooked against SLUS-21104's own code.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::r#move::{Curve, Engine, MAX_RPM_POINTS, Params, set_gear};
use crate::quad::{Body, Quad};
use ps2_float::vu::Dest;
use ps2_float::{F, Vf, fpu, fpu::Acc};

/// Radians a second to RPM and back, as the engine loads them.
const RPM_PER_RAD: f32 = 9.549296;
const RAD_PER_RPM: f32 = 0.1047198;

/// The gearbox's crawl: under this speed with the throttle shut, the clutch lever goes in.
const CRAWL: u32 = 0x40ea_aa65; // 7.3333

/// The (0, 0, 0, 1) the wheel clear leaves in its vectors.
const NOTHING: Vf = Vf([0.0, 0.0, 0.0, 1.0]);

/// A wheel as the engine sees it: how fast it spins, the drive torque the engine adds into, whether
/// its spin is driven by torque (then the crank's load comes back from the tire), and the tire's
/// forward force that load is read from.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Drive {
    pub spin: F,
    pub torque: F,
    pub from_torque: bool,
    pub forward: F,
}

/// What [`crate::quad::Bike`] adds for these calls: the four wheels' [`Drive`]s and which the engine
/// drives (the first always, the second if there is one); the pose the head and tail takes with
/// control off; the body's last position (the head and tail moves it with the position) and its
/// enabled word; the quad's flag byte and whether it has a collision record.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parts {
    pub drive: [Drive; 4],
    pub driven: (usize, Option<usize>),
    pub cofg_off: Vf,
    pub last_position: Vf,
    pub body_on: bool,
    pub flags: u8,
    pub record: bool,
}

impl Default for Parts {
    fn default() -> Parts {
        Parts {
            drive: [Drive::default(); 4],
            driven: (crate::quad::LEFT_REAR, Some(crate::quad::RIGHT_REAR)),
            cofg_off: Vf::ZERO,
            last_position: Vf::ZERO,
            body_on: false,
            flags: 0,
            record: false,
        }
    }
}

fn add_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.add(b))
}

fn sub_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.sub(b))
}

fn with_z(v: Vf, z: F) -> Vf {
    v.merge(Dest::Z, Vf::splat(z.0))
}

/// **The gearbox**, once a PreProcess: under the crawl speed with the throttle shut the clutch lever
/// goes in; then up a gear past the up-shift point if there is one, else down one under the
/// down-shift point if above first. The revs are the road's through this gear off `inv_radius` (the
/// left rear's), against [`Quad::speed`]'s word.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn auto_shift(p: &Params, e: &mut Engine, q: &mut Quad, inv_radius: F) {
    let speed = F(q.contact_vel.z());
    if speed < F::from_bits(CRAWL) && q.gas_limited == F::ZERO {
        q.clutch_in = F::ONE;
    }
    let rpm = F(RPM_PER_RAD) * (e.ratio * (speed * inv_radius));
    if !(rpm <= p.up_shift_rpm) && e.gear < e.gears {
        set_gear(e, e.gear + 1);
    } else if rpm < p.down_shift_rpm && 2 <= e.gear {
        set_gear(e, e.gear - 1);
    }
}

/// **The head and tail**, the PreProcess's last call: the body's centre of mass posed for this frame.
///
/// The pose is the control-off one, else the balance one when the balance words agree, else the
/// riding one. Its `z` moves with the stick (`(0 + z) + -travel·y`, the forward travel for a stick
/// forward), forward by the brake's travel mapped off the lean while the brake is on, and back by
/// the clutch ramp's travel (faded out over the ramp's time, if it has one) times its weight. The
/// body's set-centre-of-mass then moves the position and the last position by the change, turned
/// into the world.
pub fn head_tail(p: &Params, parts: &mut Parts, cofg: &mut Vf, ride: &crate::r#move::Ride, stick_y: F, q: &Quad, body: &mut Body) {
    let pose = if q.control_off {
        parts.cofg_off
    } else if q.balancing {
        p.balance_cofg
    } else {
        p.cofg
    };
    let travel = if stick_y < F::ZERO { p.cofg_travel_forward } else { p.cofg_travel_back };
    let mut z = Acc::adda(F::ZERO, pose.z()).madd(F(fpu::neg(travel.0)), stick_y);
    if q.brake != F::ZERO {
        z = z + crate::air::map_clamp(q.lean.abs(), F::ZERO, F::ONE, F::ZERO, p.brake_cofg_travel_forward);
    }
    let back = if p.clutch_torque_scale_time == F::ZERO {
        p.clutch_cofg_travel_back
    } else {
        crate::air::map_clamp(ride.clutch_timer, F::ZERO, p.clutch_torque_scale_time, p.clutch_cofg_travel_back, F::ZERO)
    };
    let z = Acc::adda(F::ZERO, z).msub(ride.clutch_cofg_weight, back);
    let new = with_z(pose, z);
    // The body's set-centre-of-mass.
    let moved = body.to_world(sub_xyz(new, *cofg));
    body.position = add_xyz(body.position, moved);
    parts.last_position = add_xyz(parts.last_position, moved);
    *cofg = new;
}

/// **The vehicle's gravity**, the Force body's first call: with the body enabled, the world's gravity
/// added to its force as it is (no mass), and bit 5 of the quad's flag byte cleared; between the two
/// the collision record, if there is one, is told ([`crate::quad::Call::GravityRecord`], which this
/// returns `true` for). With the body off, nothing at all.
pub fn gravity(parts: &mut Parts, q: &Quad, body: &mut Body) -> bool {
    if !parts.body_on {
        return false;
    }
    body.add_force(q.gravity);
    parts.flags &= !0x20;
    parts.record
}

/// The curve's value at the cursor: `(0 + hi) - frac·(hi - lo)`, or the bottom point as it is.
fn curve_value(cur: &Curve, ys: &[F; MAX_RPM_POINTS]) -> F {
    if cur.index == 0 {
        ys[0]
    } else {
        let hi = ys[cur.index as usize];
        Acc::adda(F::ZERO, hi).msub(cur.frac(), hi - ys[cur.index as usize - 1])
    }
}

/// The road's revs through this gear: off one wheel, `(9.549·spin)·ratio`; with a second driven wheel,
/// `ratio·(9.549·(½·(s + s)))` — the first wheel's spin twice, not the two wheels' mean.
fn road_rpm(e: &Engine, w: &[Drive; 4], (a, b): (usize, Option<usize>)) -> F {
    let s = w[a].spin;
    match b {
        Some(_) => e.ratio * (F(RPM_PER_RAD) * (F(0.5) * (s + s))),
        None => (F(RPM_PER_RAD) * s) * e.ratio,
    }
}

/// **The engine, once a substep**: MX Unleashed's [`crate::r#move::engine_update`] on one or two
/// driven wheels. The curve, the clutch's lock and slip and the revs are its; the differences:
///
/// - the road's revs are [`road_rpm`]'s;
/// - the wheel torque goes to the first wheel whole, or half to each of two;
/// - with two wheels the slipping crank's load comes back from both tires only when both are driven
///   by torque (`½·(f₂r₂ + f₁r₁)`, accumulated), else it is the clutch's share, as with one;
/// - the curve is read as `(0 + hi) - frac·(hi - lo)`, and the revs as `(0 + rpm) + 9.549·(…)`.
///
/// `gas` and `clutch` are the two words the engine points at ([`Quad::gas_limited`] and
/// [`Quad::clutch_in`]); `radius` is each wheel's `+0x214`.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn engine_update(e: &mut Engine, w: &mut [Drive; 4], radius: [F; 4], driven: (usize, Option<usize>), gas: F, clutch: F, dt: F) {
    crate::r#move::curve_seek(&mut e.curve, &e.curve_rpm, e.rpm);
    let curve = curve_value(&e.curve, &e.curve_torque);
    let brake = e.rpm * (e.engine_braking * e.rpm);
    let thr = (gas + e.idle_throttle).min(F::ONE);
    e.torque = e.clutch_torque_scale * ((curve + brake) * thr - brake);

    let was = e.road_rpm;
    e.road_rpm = road_rpm(e, w, driven);

    // Every path below writes it before it is read.
    let mut shaft = F::ZERO;
    if e.clutch_locked {
        if clutch == F::ONE || !(e.torque.abs() <= e.clutch_static * (F::ONE - clutch)) {
            e.clutch_locked = false;
        } else {
            shaft = e.torque;
        }
    }
    if !e.clutch_locked {
        let sign = if e.rpm - e.road_rpm < F::ZERO { -1 } else { 1 };
        shaft = (e.clutch_kinetic * (F::ONE - clutch)) * F(fpu::from_i32(sign));
        let (rpm, road) = (e.rpm, e.road_rpm);
        if clutch < F::ONE && ((!(rpm <= was) && rpm < road) || (rpm < was && !(rpm <= road))) {
            e.clutch_locked = true;
            shaft = e.torque;
        }
    }

    e.wheel_torque = e.torque_scale * (shaft * e.ratio);
    let (a, b) = driven;
    match b {
        Some(b) => {
            w[a].torque = w[a].torque + F(0.5) * e.wheel_torque;
            w[b].torque = w[b].torque + F(0.5) * e.wheel_torque;
        }
        None => w[a].torque = w[a].torque + e.wheel_torque,
    }

    if e.clutch_locked {
        e.rpm = e.road_rpm;
    } else {
        let load = match b {
            Some(b) if w[a].from_torque && w[b].from_torque => {
                let both = Acc::mula(w[b].forward, radius[b]).madd(w[a].forward, radius[a]);
                e.inv_ratio * ((F::ONE - clutch) * (F(0.5) * both))
            }
            None if w[a].from_torque => e.inv_ratio * ((F::ONE - clutch) * (w[a].forward * radius[a])),
            _ => shaft,
        };
        e.rpm = Acc::adda(F::ZERO, e.rpm).madd(F(RPM_PER_RAD), (e.inv_inertia * (e.torque - load)) * dt);
    }
    if e.rpm < e.min_rpm {
        e.rpm = e.min_rpm;
    } else if !(e.rpm <= e.rev_limit) {
        e.rpm = e.rev_limit;
    }
}

/// **The rev limiter**, the Force body's last but one: with the clutch locked and the road's revs
/// ([`road_rpm`]) past the limit, the first driven wheel's spin is set to the limit's
/// (`(0.1047·limit)·inv_ratio`) and the second's copied from it.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn rev_limit(e: &Engine, w: &mut [Drive; 4], driven: (usize, Option<usize>)) {
    let rpm = road_rpm(e, w, driven);
    if e.clutch_locked && !(rpm <= e.rev_limit) {
        let (a, b) = driven;
        w[a].spin = (F(RAD_PER_RPM) * e.rev_limit) * e.inv_ratio;
        if let Some(b) = b {
            w[b].spin = w[a].spin;
        }
    }
}

/// **The air drag**: MX Unleashed's [`crate::r#move::air_drag`] on the quad, into the body's force.
/// The direction is the velocity normalised on VU0 (its `w` kept), the size `-drag·((½·|v|)·|v|)` off
/// [`Quad::speed_len`].
pub fn air_drag(drag: F, q: &Quad, body: &mut Body) {
    let k = (F(0.5) * q.speed_len) * q.speed_len;
    let f = F(fpu::neg(drag.0)) * k;
    let dir = crate::air::normalize(body.velocity);
    body.add_force(dir.merge(Dest::XYZ, dir.mul(Vf::splat(f.0))));
}

/// **The peak rates**: the body's correction peak brought into its own frame, and each of its `xyz`
/// kept in [`Quad::attitude`] when it is not smaller by magnitude (a NaN is kept).
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn peak_rates(q: &mut Quad, body: &Body) {
    let local = body.to_local(body.reset_each_frame);
    for (i, dest) in [Dest::X, Dest::Y, Dest::Z].into_iter().enumerate() {
        if !(F(local.0[i]).abs() <= F(q.attitude.0[i]).abs()) {
            q.attitude = q.attitude.merge(dest, local);
        }
    }
}

/// **A wheel cleared**, each substep before the contacts are spread: MX Unleashed's
/// [`crate::r#move::wheel_clear`]'s words on the quad's wheel — the ray and sweep flags, the contact,
/// what it stands on (still ground), the compression and spring force, and the three velocities the
/// contact velocity writes, (0, 0, 0, 1).
pub fn wheel_clear(w: &mut crate::wheel::Wheel, more: &mut crate::atv_wheel::More, ground: &mut crate::atv_wheel::Ground) {
    w.ray_hit = 0;
    w.swept_contact = 0;
    w.contact = false;
    w.compression = F::ZERO;
    w.spring_force = F::ZERO;
    w.contact_velocity = NOTHING;
    more.ground_known = 0;
    more.ground_velocity = NOTHING;
    more.ground_point_velocity = NOTHING;
    *ground = crate::atv_wheel::Ground::Still;
}
