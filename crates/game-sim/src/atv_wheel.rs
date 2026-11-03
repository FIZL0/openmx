//! **The ATV's wheel suspension and tire forces** (MD37b), ported from MX vs. ATV Unleashed's PS2 ELF
//! (SLUS-21104): the two per-substep wheel steps the quad's Force body runs on each of its four
//! wheels, and the two the tire forces call (MD74): the contact velocity and the steering hinge's
//! force. Every proof is against SLUS-21104's own code (`tests/atv_wheel.rs`), not MX Unleashed's;
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
//! | `steer_torque` | `+0x6c` | `+0x60` |
//!
//! The rest (`+0xd0` .. `+0x180`, `+0x1e0`, `+0x200`, `+0x214`) sit where MX Unleashed has them. The
//! wheel's body pointer is `+0x1e4`; [`More`] has the words of the wheel MX Unleashed's view has no
//! field for.
//!
//! What the two steps call (the ground query, the contact-velocity step, the steering hinge's force)
//! is handed to closures as [`Call`]s, in the original's order; [`run`] answers the last two with
//! [`contact_velocity`] and [`hinge_force`].

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::contact::Node;
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
/// What an unloaded wheel's contact velocity is reset to, `0x396fc0`; also what the ground's own
/// velocity is taken to be when the wheel stands on nothing that moves.
const NO_CONTACT_VELOCITY: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);

/// The ATV's rigid body, as far as its wheels read and write it (the wheel's `+0x1e4`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Body {
    /// `+0x70`: the centre of mass in the world. A bottomed-out wheel moves it.
    pub position: Vf,
    /// `+0x80`: the velocity in the world.
    pub velocity: Vf,
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
    /// `+0x1d0`: the angular velocity in the body's own frame.
    pub spin: Vf,
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
    /// `+0x190`: [`Wheel::chassis_velocity`] in the body's frame, as the contact velocity leaves it.
    pub chassis_velocity_local: Vf,
    /// `+0x1a0`: the ground's velocities below are already known. The contact velocity sets it and
    /// then reuses them; what clears it is not traced.
    pub ground_known: i32,
    /// `+0x1b0` and `+0x1c0`: the velocity of what the wheel stands on, and that thing's velocity at
    /// the contact point.
    pub ground_velocity: Vf,
    pub ground_point_velocity: Vf,
    /// `+0x8c0`: the steering hinge's axis, in the body's frame.
    pub steer_axis: Vf,
}

/// What the wheel stands on, as the contact velocity reads it: the wheel's `+0x1f0` (a collision
/// object) and, without one, its `+0x1f4` (a moving piece of scenery).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Ground {
    /// Neither: the ground does not move.
    #[default]
    Still,
    /// A collision object on a body or a vehicle. `cofg` is its own centre of mass (`+0x80`) and
    /// `cofg_world` (`+0x90`) is that turned into the world, which the contact velocity writes.
    Object { cofg: Vf, cofg_world: Vf, on: Holder },
    /// A moving piece of scenery.
    Mover(Mover),
}

/// What a collision object under the wheel belongs to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Holder {
    /// A rigid body of the quad's own kind (the object's `+0xe4`).
    Body(Body),
    /// A vehicle's chassis (`+0xe0`, read only when there is no body), placed by the object's
    /// transform (`+0xe8`).
    Chassis(Chassis),
}

/// A vehicle's chassis under the wheel, as far as the contact velocity reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Chassis {
    /// `+0x180`: the centre of mass in the transform's frame.
    pub cofg: Vf,
    /// `+0x250`: the velocity in the world.
    pub velocity: Vf,
    /// `+0x280`: the angular velocity in the transform's frame.
    pub spin: Vf,
    /// The collision object's transform, already up to date.
    pub node: Node,
}

/// A moving piece of scenery under the wheel.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mover {
    /// `+0x244`: its transform, already up to date.
    pub node: Node,
    /// `+0x180`: the point it turns about, in the transform's frame.
    pub origin: Vf,
    /// What its two virtuals answer (`+0xc0` the velocity, `+0xc4` the spin, asked spin first).
    pub velocity: Vf,
    pub spin: Vf,
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

/// `vopmula a, b` / `vopmsub a ← b, a`, `.xyz`: `a × b`, `w` from `a`.
fn cross(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, Vf::opmsub(Vf::opmula(a, b), b, a))
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
/// `ground` is the world's ground query, handed the probe's end in the world and the wheel's
/// contact record ([`More::record`], the original's `t0`), whose surface byte it may write; it
/// answers with the point moved onto the ground and the ground's normal (not yet normalised).
pub fn suspension(w: &mut Wheel, x: &mut More, b: &Body,
    ground: &mut impl FnMut(Vf, &mut u8) -> (Vf, Vf)) -> Vec<Call> {
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
        let (p, n) = ground(w.probe_end_world, &mut x.record);
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
/// chassis velocities) and [`Call::HingeForce`] (the hinge is handed `1.0` in `f12` besides). [`run`]
/// is the original's.
pub fn tire_forces(w: &mut Wheel, b: &mut Body, dt: F,
    callee: &mut impl FnMut(Call, &mut Wheel, &mut Body, &mut More), x: &mut More) -> Vec<Call> {
    let mut calls = Vec::new();
    let mut call = |c: Call, w: &mut Wheel, b: &mut Body, x: &mut More| {
        calls.push(c);
        callee(c, w, b, x);
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
            call(Call::ContactVelocity, w, b, x);
            if !(w.compression <= w.max_compression) {
                if w.ray_hit != 0 || along < BOTTOM_OUT_REACH {
                    bottom_out(w, x, b);
                }
                w.compression = w.max_compression;
                call(Call::ContactVelocity, w, b, x);
            }
            let closing = dot3(w.contact_velocity, w.contact_normal)
                .max(-w.max_damper_speed)
                .min(w.max_damper_speed);
            let damping = if !(closing <= 0.0) { w.damping_out } else { w.damping_in };
            w.spring_force = F::ZERO.max(Acc::mula(w.spring_rate, w.compression).msub(damping, closing));
            let force = scale(w.contact_normal, w.spring_force);
            if w.steered && w.balancing {
                call(Call::HingeForce(force, w.load_point), w, b, x);
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

/// What [`tire_forces`] calls, run as SLUS-21104 runs it: the contact velocity on `ground`, and the
/// hinge with `1.0` for its scale.
pub fn run(c: Call, w: &mut Wheel, b: &mut Body, x: &mut More, ground: &mut Ground) {
    match c {
        Call::ContactVelocity => contact_velocity(w, x, b, ground),
        Call::HingeForce(force, point) => hinge_force(w, x, b, force, point, F::ONE),
        _ => unreachable!("the tire forces call nothing else"),
    }
}

/// `atv_wheel_contact_velocity`: how fast the wheel's load point moves against the ground under it
/// ([`Wheel::contact_velocity`]), and the body as a whole against it ([`Wheel::chassis_velocity`],
/// and [`More::chassis_velocity_local`] in the body's frame).
///
/// The body's point velocity is `velocity + to_world(spin) × (load point - position)`. The ground's
/// is worked out once ([`More::ground_known`]) and kept; the ways it is worked out are MX Unleashed's
/// owner and mover branches (see [`crate::contact::other_at`]) at SLUS-21104's offsets, except that a
/// rigid body's arm is taken from the object's centre of mass on it and its spin is turned into the
/// world after the cross product. The ground's point is the contact point, not the load point.
pub fn contact_velocity(w: &mut Wheel, x: &mut More, b: &Body, ground: &mut Ground) {
    let r = rotations(b);
    let arm = sub_xyz(w.load_point, b.position);
    let at = add_xyz(b.velocity, cross(r.to_world(b.spin), arm));
    if x.ground_known == 0 {
        let (velocity, point) = match ground {
            Ground::Object { cofg, cofg_world, on: Holder::Body(o) } => {
                let q = rotations(o);
                *cofg_world = q.to_world(*cofg);
                let centre = add_xyz(o.position, *cofg_world);
                let arm = q.to_local(sub_xyz(w.contact_point, centre));
                (o.velocity, q.to_world(cross(o.spin, arm)))
            }
            Ground::Object { cofg, cofg_world, on: Holder::Chassis(c) } => {
                *cofg_world = c.node.rotate(*cofg);
                let local = c.node.unplace(w.contact_point);
                let arm = sub_xyz(local, add_xyz(c.cofg, *cofg_world));
                (c.velocity, c.node.rotate(cross(c.spin, arm)))
            }
            Ground::Mover(m) => {
                let arm = sub_xyz(m.node.unplace(w.contact_point), m.origin);
                (m.velocity, m.node.rotate(cross(m.spin, arm)))
            }
            Ground::Still => (NO_CONTACT_VELOCITY, NO_CONTACT_VELOCITY),
        };
        x.ground_velocity = velocity;
        x.ground_point_velocity = match ground {
            Ground::Still => point,
            _ => add_xyz(point, velocity),
        };
    }
    x.ground_known = 1;
    w.contact_velocity = sub_xyz(at, x.ground_point_velocity);
    w.chassis_velocity = sub_xyz(b.velocity, x.ground_velocity);
    x.chassis_velocity_local = r.to_local(w.chassis_velocity);
}

/// `atv_wheel_hinge_force`: a steered wheel's tire force on a quad balanced on it. The force goes into
/// the body at `point` as any other wheel's does ([`add_force_at`]), and its moment about the wheel's
/// mount, turned into the body's frame, is added to [`Wheel::steer_torque`] along the hinge's axis
/// ([`More::steer_axis`]), times `scale` (the tire forces hand it `1.0`).
pub fn hinge_force(w: &mut Wheel, x: &More, b: &mut Body, force: Vf, point: Vf, scale: F) {
    add_force_at(b, force, point, MOMENT_SCALE);
    let moment = rotations(b).to_local(cross(sub_xyz(point, w.mount_world), force));
    w.steer_torque = Acc::adda(F::ZERO, w.steer_torque).madd(scale, dot3(moment, x.steer_axis));
}
