//! **The ATV's wheel suspension and tire forces** (MD37b), ported from MX vs. ATV Unleashed's PS2 ELF
//! (SLUS-21104): the two per-substep wheel steps the quad's Force body runs on each of its four
//! wheels. Every proof is against SLUS-21104's own code (`tests/atv_wheel.rs`), not MX Unleashed's;
//! the field map is research/oracle.txt's (`# MD37b (wheels)`).
//!
//! They are MX Unleashed's [`crate::wheel::suspension`] and [`crate::wheel::tire_forces`] rewritten
//! for MX vs. ATV's rigid body, and differ from them in more than the layout:
//!
//! - Points go between the body's frame and the world by its orientation quaternion and centre of
//!   mass (`to_world(p - cofg) + position`), not through a transform's matrix and callbacks.
//! - The ray's tolerance is a literal (cos 5°), not read off a curve table.
//! - The bottom-out shove is real here: the body's position moves by it, the closing speed comes out
//!   of the body's `+0xd0`, and what the body is pressing into the ground is handed back as a force.
//! - A steered wheel the quad balances on hands its force to the hinge; any other puts it into the
//!   body at the load point, as a force and the moment about the centre of mass.
//!
//! Both work on [`Wheel`], the MX Unleashed wheel, whose field docs give MX Unleashed's offsets.
//! SLUS-21104's wheel moves some of them:
//!
//! | field | MXU | ATV |
//! |---|---|---|
//! | `steered`, `balancing` | `+0x38`, `+0x3c` | `+0x2c`, `+0x30` |
//! | `max_compression` .. `damping_in` | `+0x48` .. `+0x5c` | `+0x3c` .. `+0x50` |
//! | `probe`, `mount` | `+0x90`, `+0xa0` | `+0x80`, `+0x90` |
//!
//! The rest (`+0xd0` .. `+0x180`, `+0x1e0`, `+0x200`, `+0x214`) sit where MX Unleashed has them. The
//! wheel's body pointer is `+0x1e4`; [`More`] has the two words of the wheel MX Unleashed's view has
//! no field for.
//!
//! What they call (the ground query, the contact-velocity step, the steering hinge's force) is handed
//! to closures as [`Call`]s, in the original's order.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::wheel::{Call, Wheel};
use ps2_float::vu::{self, Dest, Field};
use ps2_float::{F, Vf, fpu::Acc};

/// The ray re-aims itself until two answers' normals agree this well, `0x3f7f069e` (cos 5°).
const RAY_AGREEMENT: u32 = 0x3f7f069e;
/// How many times the ray may aim itself in one substep.
const RAY_STEPS: i32 = 5;
/// A swept contact whose suspension line is further than this (`0x3f34fdf4`, cos 45°) off the
/// contact's normal loses to the terrain at once.
const SWEPT_LEAN: u32 = 0x3f34fdf4;
/// How much higher one answer must be than the other to take the wheel off it (`0x3dcccccd`).
const SURFACE_MARGIN: f32 = 0.1;
/// Below this (`0x358637bd`) the suspension line is taken as parallel to the contact: no load.
const CONTACT_EPSILON: f32 = 1e-06;
/// The load scale's lean range, as MX Unleashed's (see [`crate::wheel`]).
const LEAN_LO: f32 = 0.5;
const LEAN_HI: f32 = 0.64;
/// How much of the overlap a bottomed-out suspension pushes the body back out (`0x3f666666`).
const BOTTOM_OUT_PUSH: f32 = 0.9;
/// How far along the suspension line a contact the ray did not find may be and still shove.
const BOTTOM_OUT_REACH: f32 = 2.0;
/// The body's own up, `0x397000`.
const UP: Vf = Vf::new(0.0, 1.0, 0.0, 1.0);
/// The scale the tire's moment is handed through the body's add-force-at, `0x396fd0`.
const MOMENT_SCALE: Vf = Vf::new(1.0, 1.0, 1.0, 1.0);
/// What an unloaded wheel's contact velocity is reset to, `0x396fc0`.
const NO_CONTACT_VELOCITY: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);

/// The ATV's rigid body, as far as its wheels read and write it (the wheel's `+0x1e4`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Body {
    /// `+0x70`: the centre of mass in the world. A bottomed-out wheel moves it.
    pub position: Vf,
    /// `+0xa0` and `+0x1b0`: the force and moment accumulators. Only `xyz` is ever added to.
    pub force: Vf,
    pub moment: Vf,
    /// `+0xd0`: what a bottomed-out wheel takes its closing speed out of, along the body's up. Read
    /// by the body's integrate, which is not ported.
    pub correction: Vf,
    /// `+0x190`: the orientation quaternion.
    pub orientation: Vf,
    /// `+0x1a0`: the centre of mass in the body's own frame.
    pub cofg: Vf,
}

/// The two words of SLUS-21104's wheel [`Wheel`] has no field for.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct More {
    /// `+0x110`, the first byte of the contact record the ground query fills (it is handed the
    /// record's address). A ray that misses under a swept contact zeroes it.
    pub record: u8,
    /// `+0xc0`: a vector the bottom-out replaces with a zero one when that is the longer. A zero
    /// vector never is, so it never changes; what reads it is not traced.
    pub kept: Vf,
}

fn rotations(b: &Body) -> crate::quad::Body {
    crate::quad::Body { orientation: b.orientation, ..Default::default() }
}

/// A point in the body's frame into the world: `to_world(p - cofg) + position`. `w` is `p`'s.
fn point_to_world(b: &Body, p: Vf) -> Vf {
    add_xyz(rotations(b).to_world(sub_xyz(p, b.cofg)), b.position)
}

/// Normalise `xyz` with VU0's `vrsqrt` (see [`crate::wheel`]'s). `w` comes through untouched.
fn normalize(v: Vf) -> Vf {
    let sq = v.mul(v);
    let len2 = vu::madd(vu::add(sq.x(), sq.y()), 1.0, sq.z());
    v.merge(Dest::XYZ, v.mul_q(vu::rsqrt(1.0, len2)))
}

fn scale(v: Vf, s: F) -> Vf {
    v.merge(Dest::XYZ, v.mul(Vf::splat(s.0).bc(Field::X)))
}

fn add_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.add(b))
}

fn sub_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.sub(b))
}

/// `a·b` over `xyz` on the FPU's accumulator: `mula`, `madda`, `madd`.
fn dot3(a: Vf, b: Vf) -> F {
    Acc::mula(a.x(), b.x()).madda(a.y(), b.y()).madd(a.z(), b.z())
}

/// `|v|`, the FPU's way: `v·v` then `sqrt.s`.
fn length(v: Vf) -> F {
    dot3(v, v).sqrt()
}

/// `v` with one lane stored over by `0.0` (an `swc1` of a zero register into the stacked vector).
fn zero_lane(v: Vf, d: Dest) -> Vf {
    v.merge(d, Vf::ZERO)
}

/// `atv_wheel_suspension`: the ray down the suspension, aimed along the ground's normal until two
/// answers agree, and the argument between its answer and the collision sweep's contact.
///
/// `ground` is the world's ground query, handed the probe's end in the world; it
/// answers with the point moved onto the ground and the ground's normal (not yet normalised).
pub fn suspension(w: &mut Wheel, x: &mut More, b: &Body,
    ground: &mut impl FnMut(Vf) -> (Vf, Vf)) -> Vec<Call> {
    let mut calls = Vec::new();
    w.ray_hit = 0;
    if !w.has_ground {
        return calls;
    }
    w.mount_world = point_to_world(b, w.mount);
    w.ray_steps = 0;
    let (mut point, mut normal) = (Vf::ZERO, Vf::ZERO);
    let mut agreement = F::ZERO;
    while agreement < F::from_bits(RAY_AGREEMENT) && w.ray_steps < RAY_STEPS {
        let aim = normalize(zero_lane(rotations(b).to_local(w.ground_normal), Dest::X));
        w.probe = scale(aim, -w.probe_length);
        w.probe_end = add_xyz(w.mount, w.probe);
        w.probe_end_world = point_to_world(b, w.probe_end);
        calls.push(Call::GroundQuery(w.probe_end_world));
        let (p, n) = ground(w.probe_end_world);
        point = p;
        normal = normalize(n);
        agreement = dot3(normal, w.ground_normal);
        w.ground_normal = normal;
        w.ray_steps += 1;
    }
    if F(point.y()) < F(w.probe_end_world.y()) {
        if w.swept_contact != 0 {
            x.record = 0;
        }
        return calls;
    }
    if w.swept_contact == 0 {
        w.ray_hit = 1;
        w.contact_point = point;
        w.contact_normal = normal;
        return calls;
    }
    let down = normalize(sub_xyz(w.mount_world, w.probe_end_world));
    if dot3(down, w.contact_normal) < F::from_bits(SWEPT_LEAN) {
        w.terrain_wins = 1;
    } else if w.terrain_wins != 0 {
        if !(F(w.contact_point.y()) <= F(SURFACE_MARGIN) + point.y()) {
            w.terrain_wins = 0;
        }
    } else if !(F(point.y()) <= F(SURFACE_MARGIN) + w.contact_point.y()) {
        w.terrain_wins = 1;
    }
    if w.terrain_wins != 0 {
        w.ray_hit = 1;
        w.swept_contact = 0;
        w.contact_point = point;
        w.contact_normal = normal;
    }
    calls
}

/// The load scale: MX Unleashed's remap of the contact's lean from `0.5..0.64` onto `0.5..1.0`.
fn load_scale(lean: F) -> F {
    let t = (lean - LEAN_LO) / (F(LEAN_HI) - LEAN_LO);
    Acc::adda(F::ZERO, F(LEAN_LO)).madd(t, F(1.0) - LEAN_LO).max(LEAN_LO).min(1.0)
}

/// The body's add-force-at (its vtable's `+0x84`): the moment of `f` at `point` about
/// the centre of mass, into the body's frame and scaled lane by lane, then the force itself.
pub fn add_force_at(b: &mut Body, f: Vf, point: Vf, moment_scale: Vf) {
    let arm = sub_xyz(point, b.position);
    let cross = arm.merge(Dest::XYZ, Vf::opmsub(Vf::opmula(arm, f), f, arm));
    let local = rotations(b).to_local(cross);
    b.moment = add_xyz(b.moment, local.merge(Dest::XYZ, local.mul(moment_scale)));
    b.force = add_xyz(b.force, f);
}

/// `atv_wheel_tire_forces`: the suspension's compression along the line from the mount to the
/// probe's end, the bottom-out, the spring and damper, and the force they make, then the extension
/// limit.
///
/// `callee` runs what it calls: [`Call::ContactVelocity`] (which writes the wheel's contact and
/// chassis velocities) and [`Call::HingeForce`] (the hinge is handed `1.0` in `f12` besides).
pub fn tire_forces(w: &mut Wheel, b: &mut Body, dt: F,
    callee: &mut impl FnMut(Call, &mut Wheel, &mut Body), x: &mut More) -> Vec<Call> {
    let mut calls = Vec::new();
    let mut call = |c: Call, w: &mut Wheel, b: &mut Body| {
        calls.push(c);
        callee(c, w, b);
    };
    w.compression = F::ZERO;
    w.contact = false;
    if w.ray_hit != 0 || w.swept_contact != 0 {
        w.mount_world = point_to_world(b, w.mount);
        w.probe_end = add_xyz(w.mount, w.probe);
        w.probe_end_world = point_to_world(b, w.probe_end);
        let down = sub_xyz(w.mount_world, w.probe_end_world);
        let denom = dot3(down, w.contact_normal);
        let mut along = F::ZERO;
        if !(denom <= CONTACT_EPSILON) {
            along = dot3(sub_xyz(w.contact_point, w.probe_end_world), w.contact_normal) / denom;
            if !(along <= 0.0) {
                w.load_point = add_xyz(w.probe_end_world, scale(down, along));
                w.compression = length(sub_xyz(w.load_point, w.probe_end_world));
            }
        }
        if !(w.compression <= 0.0) {
            w.contact = true;
            let lean = rotations(b).to_local(w.contact_normal);
            let lean = normalize(zero_lane(lean, Dest::Z));
            w.compression = w.compression * load_scale(F(lean.y()));
            call(Call::ContactVelocity, w, b);
            if !(w.compression <= w.max_compression) {
                if w.ray_hit != 0 || along < BOTTOM_OUT_REACH {
                    bottom_out(w, x, b);
                }
                w.compression = w.max_compression;
                call(Call::ContactVelocity, w, b);
            }
            let closing = dot3(w.contact_velocity, w.contact_normal)
                .max(-w.max_damper_speed)
                .min(w.max_damper_speed);
            let damping = if !(closing <= 0.0) { w.damping_out } else { w.damping_in };
            w.spring_force = F::ZERO.max(Acc::mula(w.spring_rate, w.compression).msub(damping, closing));
            let force = scale(w.contact_normal, w.spring_force);
            if w.steered && w.balancing {
                call(Call::HingeForce(force, w.load_point), w, b);
            } else {
                add_force_at(b, force, w.load_point, MOMENT_SCALE);
            }
        }
    }
    if !w.contact {
        w.spring_force = F::ZERO;
        w.contact_velocity = NO_CONTACT_VELOCITY;
    }
    if w.compression < w.last_compression {
        w.compression = w.last_compression
            - (w.last_compression - w.compression).min(w.extension_rate * dt);
    }
    w.last_compression = w.compression.min(w.max_compression);
    calls
}

/// The bottom-out: the body pushed back out along the contact's normal, the closing speed taken out
/// along its up, and what it is pressing into the ground handed back as a force.
fn bottom_out(w: &Wheel, x: &mut More, b: &mut Body) {
    let push = scale(w.contact_normal, F(BOTTOM_OUT_PUSH) * (w.compression - w.max_compression));
    b.position = add_xyz(b.position, push);
    let into = dot3(w.chassis_velocity, w.contact_normal).min(0.0);
    let zero = scale(w.contact_normal, F::ZERO);
    if !(length(zero) <= length(x.kept)) {
        x.kept = zero;
    }
    let up = rotations(b).to_world(UP);
    b.correction = sub_xyz(b.correction, scale(up, into));
    let pressing = -dot3(b.force, w.contact_normal).min(0.0);
    b.force = add_xyz(b.force, scale(w.contact_normal, pressing));
}
