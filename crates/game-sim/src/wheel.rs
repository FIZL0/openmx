//! **The wheel**: the three steps the ground frame drives it through, in the order it calls them.
//!
//! The bike's ground step ([`crate::r#move::step`]) does the chassis; everything that happens *under*
//! the chassis happens here, twice a substep — once for the front wheel and once for the rear:
//!
//! 1. [`suspension`] — the ray that looks for the ground under the wheel, and which of the two
//!    contacts (the ray's or the collision sweep's) the wheel ends up riding on.
//! 2. [`tire_forces`] — how far the suspension is compressed, the spring and damper force that comes
//!    out of that, and the drive, brake and side forces the tire puts into the ground.
//! 3. [`integrate`] — the wheel's own spin and steering.
//!
//! # The ten fields `wheel_clear` empties
//!
//! `move.rs`'s [`Fresh`](crate::r#move::Fresh) keeps the seven words and three quadwords the original
//! empties at the top of every substep by offset, because S3 did not port what writes them. This
//! module is what writes them, so here are their names — the table is the one thing to read if you are
//! holding a `Fresh` and wondering what is in it:
//!
//! | `Fresh` | offset | name here | written by |
//! |---|---|---|---|
//! | `scalars[0]` | `+0xd0` | [`Wheel::ray_hit`] | [`suspension`] |
//! | `scalars[1]` | `+0xd4` | [`Wheel::swept_contact`] | the contact distributor; [`suspension`] clears it |
//! | `scalars[2]` | `+0x1f0` | [`Wheel::hit_owner`] | the contact distributor (a pointer, not a float) |
//! | `scalars[3]` | `+0x1f4` | [`Wheel::hit_vehicle`] | the contact distributor (a pointer, not a float) |
//! | `scalars[4]` | `+0xf0` | [`Wheel::compression`] | [`tire_forces`] |
//! | `scalars[5]` | `+0xf8` | [`Wheel::spring_force`] | [`tire_forces`] |
//! | `scalars[6]` | `+0x1a0` | [`Wheel::velocity_done`] | the contact-velocity step |
//! | `vectors[0]` | `+0x170` | [`Wheel::contact_velocity`] | the contact-velocity step |
//! | `vectors[1]` | `+0x1b0` | [`Wheel::other_velocity`] | the contact-velocity step |
//! | `vectors[2]` | `+0x1c0` | [`Wheel::other_point_velocity`] | the contact-velocity step |
//!
//! Two of the seven "scalars" are object pointers and one more is a flag, which is why they are an
//! `i32` here and an `F` there: `wheel_clear` only ever writes zero into them, so both are the same
//! sixteen zero words, and only this side can tell you what a non-zero one means.
//!
//! # What is here and what is not
//!
//! Like `move.rs`, each step records what it called and does not port it: the ground query the
//! suspension ray asks (eighty-four callers across the game — it is the world's, not the wheel's), the
//! contact velocity, the tire's own force model and the two sides of the wheel's integration. [`Call`]
//! lists them, `tests/wheel.rs` proves the list against the original with every one hooked, and the
//! journal entry says why each was left.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::camera::CurveTable;
use ps2_float::{Acc, Dest, F, Field, Vf, vu};

/// `vf0`, the register that is always `(0, 0, 0, 1)`. The point transform adds its `x` lane — a zero —
/// to load the accumulator, which is not quite the same as not adding anything: a `-0.0` translation
/// comes back `+0.0`.
const VF0: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);

/// How far the suspension ray's answer may swing from the last one before the ray gives up and asks
/// again: five degrees, as the instruction stream's own literal, looked up in the engine's `{cos, sin}`
/// table rather than compared as an angle.
const NORMAL_TOLERANCE: f32 = 0.08726645;
/// The engine's curve table is indexed from `-pi` in steps of `2pi/5999`; a lookup finds its entry at
/// `(angle + pi) * INV_STEP`. Both constants are the global the table's builder writes.
#[allow(clippy::approx_constant)] // the game's own seven digits, not `f32::consts::PI` rounded
const PI: f32 = 3.1415927;
const CURVE_INV_STEP: f32 = 954.77045;
/// How many times the ray may re-aim itself in one substep.
const MAX_RAY_STEPS: i32 = 5;
/// How far from straight up a swept contact's normal may lean before the terrain under the wheel wins
/// the argument outright, and the slack either answer gets before it takes the contact over.
const SIDE_CONTACT_COSINE: f32 = 0.707;
const CONTACT_HYSTERESIS: f32 = 0.1;

/// How far the suspension line has to lean out of the contact's plane before the two are called
/// parallel and the wheel gets no load at all: the instruction stream's own millionth.
const CONTACT_EPSILON: f32 = 1e-06;
/// The load scale: a contact whose normal, in the chassis' frame and flattened sideways, points less
/// than `0.5` up carries half the compression; one past `0.64` carries all of it. The low end is both
/// the input's and the output's, which is the tuning's own coincidence and not an identity.
const LEAN_LO: f32 = 0.5;
const LEAN_HI: f32 = 0.64;
/// How much of the overlap a bottomed-out suspension pushes back out. The original computes the push
/// vector, stores it in a stack slot and never reads it again, so this constant reaches nothing —
/// see the journal entry.
const BOTTOM_OUT_PUSH: f32 = 0.9;
/// How far into the suspension's travel the ray's answer may be before a wheel with no ray hit still
/// gets the bottom-out push.
const BOTTOM_OUT_REACH: f32 = 2.0;

/// Two of the engine's constant quadwords: `(1, 1, 1, 1)`, which the torque is multiplied by
/// component-wise on its way to the chassis, and the `+y` axis the bottom-out push rotates into the
/// world. Their addresses are in `research/oracle.txt`; the proof reads the second one out of the
/// original's own memory rather than trusting this copy.
const ONE: Vf = Vf::new(1.0, 1.0, 1.0, 1.0);
const UP: Vf = Vf::new(0.0, 1.0, 0.0, 1.0);

/// One wheel, as the three steps in this module see it. The original's object is 0x8c0 bytes and the
/// ground step's own view of it is [`crate::r#move::Wheel`]; this is the rest of it, by the name of
/// what writes it, with the offset it lives at so the two views can be lined up.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wheel {
    /// `+0x1e0`: the world the suspension ray asks for the ground. Null in the original — a wheel with
    /// no world does not probe at all.
    pub has_ground: bool,
    /// `+0xa0`: where the wheel hangs off the chassis, in the chassis' own frame, and `+0x150` the same
    /// point in the world.
    pub mount: Vf,
    pub mount_world: Vf,
    /// `+0x90`: the probe, in the chassis' frame: the suspension's travel along the ground's normal.
    /// `+0x100` is where it ends, and `+0x160` that in the world.
    pub probe: Vf,
    pub probe_end: Vf,
    pub probe_end_world: Vf,
    /// `+0x214`: how far the probe reaches.
    pub probe_length: F,
    /// `+0x200`: the ground's normal where the ray last found it. The ray aims itself along this, so it
    /// survives the substep — [`crate::r#move::wheel_clear`] does not touch it.
    pub ground_normal: Vf,
    /// `+0xec`: how many times the ray re-aimed itself, this substep.
    pub ray_steps: i32,
    /// `+0xd0`: the ray found ground under the wheel this substep.
    pub ray_hit: i32,
    /// `+0xd4`: the collision sweep put a contact on this wheel this substep. [`suspension`] clears it
    /// when it decides the terrain is the better answer.
    pub swept_contact: i32,
    /// `+0xd8`: the terrain won the last argument. Not cleared per substep, so the wheel keeps riding
    /// whichever surface it chose until the other one is a tenth of a unit better.
    pub terrain_wins: i32,
    /// `+0x120` and `+0x130`: the contact the tire forces then work on — where it is and which way the
    /// ground faces there.
    pub contact_point: Vf,
    pub contact_normal: Vf,

    // --- the suspension's tuning and what [`tire_forces`] makes of it ---
    /// `+0x48`: how far the suspension may be compressed. Past this it is bottomed out: the chassis
    /// gets a shove instead of a spring, and the compression is held here.
    pub max_compression: F,
    /// `+0x4c`: the largest closing speed the damper will answer to, both ways.
    pub max_damper_speed: F,
    /// `+0x50`: how fast the suspension may extend, in compression per second. Compressing is not
    /// limited — only letting go is.
    pub extension_rate: F,
    /// `+0x54`: the spring rate, against the compression.
    pub spring_rate: F,
    /// `+0x58` and `+0x5c`: the damper, which has a rate for each direction. The sign of the contact
    /// velocity along the contact's normal picks between them; positive — the wheel is leaving the
    /// surface — takes `+0x58`.
    pub damping_out: F,
    pub damping_in: F,
    /// `+0xdc`: this tire is loaded — [`crate::r#move::Wheel::contact`], the same word, from the
    /// other side. Set here, and only here, by a compression above zero.
    pub contact: bool,
    /// `+0xf0`: how far the suspension is compressed, this substep.
    pub compression: F,
    /// `+0xf4`: what it was last substep, clamped to the travel — the only thing here that survives
    /// [`crate::r#move::wheel_clear`], and what the extension rate limit is measured against.
    pub last_compression: F,
    /// `+0xf8`: the spring and damper force, along the contact's normal.
    pub spring_force: F,
    /// `+0x140`: where the suspension line crosses the contact's plane, which is where the tire's
    /// force is applied.
    pub load_point: Vf,
    /// `+0x170`: how fast the contact point is moving, relative to whatever the wheel is touching,
    /// and `+0x180` the same for the chassis as a whole. Both belong to the contact-velocity step;
    /// this one reads them.
    pub contact_velocity: Vf,
    pub chassis_velocity: Vf,

    // --- the steering, which only [`integrate`] touches ---
    /// `+0x38`: this wheel is steered — it has a hinge of its own, with the axis at `+0x8c0`, the
    /// frame at `+0x8d0`, and the inertia and limits below. The rear wheel's is zero and its steering
    /// does not integrate at all.
    pub steered: bool,
    /// `+0x3c`: the bike is being balanced on this wheel, which is what hands the steering over to
    /// the physical hinge — [`crate::r#move::Wheel::balancing`], the same word, from the other side.
    /// Steered and not balancing, the angle the ground step commanded stands and only the rate is
    /// dumped.
    pub balancing: bool,
    /// `+0x6c`: the steering torque, accumulated over the substep by everything that pushes on the
    /// bars and spent — down to zero — here.
    pub steer_torque: F,
    /// `+0x70` and `+0x78`: how much the hinge resists turning, and one over its inertia.
    pub steer_damping: F,
    pub steer_inv_inertia: F,
    /// `+0x7c` and `+0x80`: the steering lock, both ways. A wheel is built with `+-pi/4`, and
    /// `+0x7c` is [`crate::r#move::Wheel::max_steer_angle`] from the other side.
    pub max_steer_angle: F,
    pub min_steer_angle: F,
    /// `+0x8ac` and `+0x8b0`: where the bars are and how fast they are turning. The ground step writes
    /// the angle directly as a command ([`crate::r#move::Wheel::steer_command`]); the hinge, when it
    /// runs, writes over it.
    pub steer_angle: F,
    pub steer_rate: F,
}

/// The chassis the wheel hangs off, as [`tire_forces`] sees it: the five accumulators it pushes the
/// tire's force into. `+0x1e4` on the wheel is the pointer to it, and
/// [`crate::r#move::Chassis`] is the ground step's own view of the same object — the offsets line up.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Chassis {
    /// `+0x130` and `+0x140`: the force and torque accumulators the substep adds into.
    pub force: Vf,
    pub torque: Vf,
    /// `+0x170`: the acceleration accumulator, which the bottom-out push writes rather than the
    /// force, so the shove does not scale with the mass.
    pub accel: Vf,
    /// `+0x190`: where the body's mass acts. The tire's torque arm is measured from here.
    pub cofg: Vf,
    /// `+0x260`: the body's velocity, which the bottom-out push takes the into-the-ground part out of.
    pub vel: Vf,
}

/// What this module's steps call and do not port, in the order they call it.
///
/// Each variant names the original's function; `research/oracle.txt` has the address and the journal
/// entry says why it was left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    /// The world's "where is the ground under this point" query, with the point handed to it. It
    /// answers with the point moved onto the surface and the surface's normal, which is what the
    /// closure [`suspension`] takes hands back.
    GroundQuery(Vf),
    /// The rolling resistance the surface under the wheel puts on it: three per-surface tables, picked
    /// out by the surface id the ground query wrote into the wheel's contact record, turned into a
    /// force on the chassis and a torque on the wheel. Handed `dt` in `f12` — the original leaves it
    /// there in the branch's delay slot — which its own code does not read.
    SurfaceDrag(F),
    /// The wheel's spin and the steering hinge's contribution to the chassis' angular velocity: the
    /// larger half of the integration, and the one that reads the steering frame at `+0x8d0`.
    SpinIntegrate(F),
    /// The chassis' transform, asked to turn a world direction back into its own frame, with the
    /// direction handed to it. The transform is lazy — asking it may rebuild a chain of parent
    /// frames — so it belongs to the transform and not to the wheel.
    Unrotate(Vf),
    /// The same transform, asked to turn one of its own directions out into the world.
    Rotate(Vf),
    /// The same transform, asked to turn a world *point* back into its own frame.
    Unpoint(Vf),
    /// The contact-velocity step: how fast the contact point and the chassis are moving relative to
    /// whatever the wheel is touching, into [`Wheel::contact_velocity`] and
    /// [`Wheel::chassis_velocity`]. [`tire_forces`] asks for it twice when the suspension bottoms
    /// out, because the shove it just gave the chassis changed the answer.
    ContactVelocity,
    /// The transform, told to bring itself up to date before the push reads the chassis' velocity.
    /// Takes no answer and is called for what it does to the transform, not to the wheel.
    TransformSync,
    /// The steering hinge's own force application, handed the force and the point it acts at: a
    /// steered wheel the bike is balancing on puts its tire force through the hinge instead of
    /// straight into the chassis.
    HingeForce(Vf, Vf),
}

// --- the arithmetic the original inlines everywhere ------------------------------------------------

/// Normalise `xyz` the way this code's VU0 does: the sum of squares with `vmul`/`vadday`/`vmaddz`, then
/// `vrsqrt` — an *approximate* reciprocal square root, so not the same number as [`vu::normalize`]'s.
/// `w` comes through untouched. (`move.rs` has the same four lines, for the same reason.)
fn normalize(v: Vf) -> Vf {
    let sq = v.mul(v);
    let len2 = vu::madd(vu::add(sq.x(), sq.y()), 1.0, sq.z());
    v.merge(Dest::XYZ, v.mul_q(vu::rsqrt(1.0, len2)))
}

/// `v.xyz * s`, as `qmtc2` into a register and `vmulx.xyz` off its `x` lane. `w` is `v`'s.
fn scale(v: Vf, s: F) -> Vf {
    v.merge(Dest::XYZ, v.mul(Vf::new(s.0, s.0, s.0, s.0).bc(Field::X)))
}

/// `a.xyz + b.xyz` with `a`'s `w`, and the same for the subtraction: the `vadd.xyz`/`vsub.xyz` every
/// vector in here is combined with.
fn add_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.add(b))
}
fn sub_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.sub(b))
}

/// The FPU's three-instruction dot product of two vectors' `xyz`: `mula.s`, `madda.s`, `madd.s`, in
/// that order. Not the VU0 one — these are `lwc1`'d lanes through the EE's own accumulator.
fn dot3(a: Vf, b: Vf) -> F {
    Acc::mula(a.x(), b.x()).madda(a.y(), b.y()).madd(a.z(), b.z())
}

/// A point through a transform: `t + x·row0 + y·row1 + z·row2`, with the rows of `m` as the frame's
/// three axes and `m[3]` its origin. `w` is the point's own, untouched.
///
/// The accumulator is loaded with `vaddax` against `vf0`'s `x` lane rather than moved, which is where
/// the stray `+ 0.0` comes from.
fn transform_point(m: &[Vf; 4], v: Vf) -> Vf {
    let acc = m[3].add(VF0.bc(Field::X));
    let acc = Vf::madd(acc, m[0], v.bc(Field::X));
    let acc = Vf::madd(acc, m[1], v.bc(Field::Y));
    v.merge(Dest::XYZ, Vf::madd(acc, m[2], v.bc(Field::Z)))
}

/// A direction back through the same transform, which drops the translation and turns by the
/// rotation's transpose. `w` is `v`'s.
///
/// The original does not write this as three dot products: it builds the transposed matrix out of the
/// rows with the R5900's quadword shuffles — `pextlw`, `pexcw`, `pextuw`, `pcpyld`, `pcpyud` — and
/// then runs the same three-step `madd` chain the forward transform does. Moving words about is exact,
/// so the transpose can be written here as a transpose and the arithmetic still lands on the same
/// bits; what matters is that each lane accumulates `x`, then `y`, then `z`.
fn unrotate(m: &[Vf; 4], v: Vf) -> Vf {
    let t = [
        Vf::new(m[0].x(), m[1].x(), m[2].x(), 0.0),
        Vf::new(m[0].y(), m[1].y(), m[2].y(), 0.0),
        Vf::new(m[0].z(), m[1].z(), m[2].z(), 0.0),
    ];
    let acc = t[0].mul(v.bc(Field::X));
    let acc = Vf::madd(acc, t[1], v.bc(Field::Y));
    v.merge(Dest::XYZ, Vf::madd(acc, t[2], v.bc(Field::Z)))
}

// --- the suspension ray ---------------------------------------------------------------------------

/// How far the ray's answer may turn before it is asked again, as the original reads it: one entry of
/// the engine's `{cos, sin}` table, taken by the index five degrees lands on.
///
/// The original inlines its generic "wrap into `-pi..pi`" around the constant angle first, which for
/// five degrees does nothing at all, and then indexes the table without the derivative correction a
/// real cosine goes through — so this is the table's entry, not the cosine of five degrees.
pub fn normal_tolerance(curve: &CurveTable) -> F {
    let i = ((F(PI) + NORMAL_TOLERANCE) * CURVE_INV_STEP).to_i32();
    F(curve.entries()[i as usize][0])
}

/// **The suspension ray**: where the ground is under this wheel, and which contact the wheel rides.
///
/// The ray starts at the wheel's mount and reaches `probe_length` *along the ground's own normal* — not
/// straight down, and not along the bike's suspension axis. Since the normal is only known from the
/// last answer, the ray re-aims and asks again until two answers agree to within five degrees, up to
/// five times a substep. The direction is flattened into the chassis' `yz` plane before it is used, so
/// a wheel never probes sideways.
///
/// Then the argument: the wheel may already have a contact from the collision sweep (`swept_contact`),
/// and the terrain's answer only takes it over if the sweep's normal leans more than 45 degrees off
/// the ray's direction, or if the terrain is a tenth of a unit higher than the contact the wheel is
/// riding — with the same tenth of slack in the other direction, so the choice does not flutter.
///
/// `ground` is the world's query: it is handed the point to look under and answers with that point
/// moved onto the surface and the surface's normal. That function belongs to the world and is not
/// ported here, so the calls to it come back in the returned list.
pub fn suspension(w: &mut Wheel, m: &[Vf; 4], curve: &CurveTable,
                  ground: &mut impl FnMut(Vf) -> (Vf, Vf)) -> Vec<Call> {
    let mut calls = Vec::new();
    w.ray_hit = 0;
    if !w.has_ground {
        return calls;
    }
    w.mount_world = transform_point(m, w.mount);

    let tolerance = normal_tolerance(curve);
    // The query's two answers, which the loop carries out to the contact test below. The original
    // leaves them in a stack slot the loop fills in, so a loop that never ran would read it
    // uninitialised — it cannot happen while the tolerance is a cosine, which is below one.
    let (mut point, mut normal) = (Vf::default(), Vf::default());
    let mut agreement = F::ZERO;
    w.ray_steps = 0;
    while agreement < tolerance && w.ray_steps < MAX_RAY_STEPS {
        // Aim: the last normal, in the chassis' frame, flattened into its `yz` plane and normalised.
        let aim = unrotate(m, w.ground_normal);
        let aim = normalize(Vf::new(0.0, aim.y(), aim.z(), aim.w()));
        w.probe = scale(aim, -w.probe_length);
        w.probe_end = add_xyz(w.mount, w.probe);
        w.probe_end_world = transform_point(m, w.probe_end);
        calls.push(Call::GroundQuery(w.probe_end_world));
        (point, normal) = ground(w.probe_end_world);
        normal = normalize(normal);
        agreement = dot3(normal, w.ground_normal);
        w.ground_normal = normal;
        w.ray_steps += 1;
    }

    // Did the ray reach the ground at all?
    if point.y() < w.probe_end_world.y() {
        return calls;
    }
    if w.swept_contact == 0 {
        w.ray_hit = 1;
        w.contact_point = point;
        w.contact_normal = normal;
        return calls;
    }
    // There is a swept contact as well, so the two are compared. `lean` is how squarely the contact's
    // normal faces back along the suspension.
    let down = normalize(sub_xyz(w.mount_world, w.probe_end_world));
    let lean = dot3(down, w.contact_normal);
    // The three comparisons are written the way the original branches on them — `c.le.s` and a branch
    // taken when it is *false* — rather than the way they read, because the two differ on a NaN, and a
    // suspension whose normal came out zero-length has one.
    if lean < SIDE_CONTACT_COSINE {
        w.terrain_wins = 1;
    } else if w.terrain_wins != 0 {
        if !(w.contact_point.y() <= F(CONTACT_HYSTERESIS) + point.y()) {
            w.terrain_wins = 0;
        }
    } else if !(point.y() <= F(CONTACT_HYSTERESIS) + w.contact_point.y()) {
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

// --- the tire's forces ----------------------------------------------------------------------------

/// Record a call and take its answer. The answers to the two that have none are thrown away.
fn ask(calls: &mut Vec<Call>, world: &mut impl FnMut(Call) -> Vf, call: Call) -> Vf {
    calls.push(call);
    world(call)
}

/// How much of the load the contact's lean earns: the engine's generic remap, inlined, taking the
/// lean from `0.5..0.64` onto `0.5..1.0` — a contact facing up the chassis' own `y` carries the whole
/// compression, one leaning sixty degrees off it carries half.
///
/// The original inlines the remap's guard against a zero-width input range too — "if `lo == hi`,
/// leave the scale at `lo`" — on two constants that are not equal, so that arm is dead and the
/// division cannot be by zero. It also takes a `min` and a `max` of the two output ends before
/// clamping to them; on these constants that is `0.5` and `1.0`, which is what the clamp here uses.
fn load_scale(lean: F) -> F {
    let t = (lean - LEAN_LO) / (F(LEAN_HI) - LEAN_LO);
    Acc::adda(F::ZERO, F(LEAN_LO)).madd(t, F(1.0) - LEAN_LO).max(LEAN_LO).min(1.0)
}

/// **The tire's forces**: how far the suspension is compressed by the contact the ray settled on, and
/// the spring and damper force that comes out of it.
///
/// The compression is geometry, not a distance along the probe: the suspension line runs from the
/// probe's end back up to the mount, and the compression is how far along that line the contact's
/// *plane* is. A contact whose normal leans away from the chassis' own up carries less of the load
/// than a flat floor under the wheel, which is [`load_scale`].
///
/// Past [`Wheel::max_compression`] the suspension has bottomed out. Then the spring is not asked to
/// do the work: the chassis is shoved directly — the part of its velocity going into the ground is
/// taken out, the same amount is taken off its acceleration along its own up, and whatever of its
/// accumulated force is still pressing into the ground is cancelled — and the compression is held at
/// the travel. The contact velocity is asked for again afterwards, because that shove changed it.
///
/// What comes out, along the contact's normal, is the spring against the compression less the damper
/// against the closing speed, never pulling. A steered wheel the bike is balancing on hands that to
/// its steering hinge; every other wheel puts it into the chassis as a force at the centre of mass
/// and a torque about it.
///
/// `m` is the chassis' transform, as four rows — the same matrix [`suspension`] takes. `world` is
/// everything this does not port: each [`Call`] goes to it in order and it answers with a vector,
/// which is what the original's callee would have written. The calls come back in the returned list
/// as well, so a caller that only wants the list can ignore the answers.
pub fn tire_forces(w: &mut Wheel, c: &mut Chassis, m: &[Vf; 4], dt: F,
                   world: &mut impl FnMut(Call) -> Vf) -> Vec<Call> {
    let mut calls = Vec::new();
    w.compression = F::ZERO;
    w.contact = false;
    if w.ray_hit != 0 || w.swept_contact != 0 {
        // The suspension line. [`suspension`] has these, but a contact can come from the collision
        // sweep with no ray behind it, so they are rebuilt here rather than relied on.
        w.mount_world = transform_point(m, w.mount);
        w.probe_end = add_xyz(w.mount, w.probe);
        w.probe_end_world = transform_point(m, w.probe_end);
        let down = sub_xyz(w.mount_world, w.probe_end_world);

        // Where that line crosses the contact's plane, as a fraction of the line. The line lying in
        // the plane leaves the wheel unloaded, and so does a plane the wheel has not reached.
        let denom = dot3(down, w.contact_normal);
        // The original's own `along` is a register the division writes, and the bottom-out test below
        // reads it even on the paths where the division never happened. Zero stands in for that;
        // reaching it needs a negative travel, which no wheel is built with.
        let mut along = F::ZERO;
        if !(denom <= F(CONTACT_EPSILON)) {
            along = dot3(sub_xyz(w.contact_point, w.probe_end_world), w.contact_normal) / denom;
            if !(along <= F::ZERO) {
                w.load_point = add_xyz(w.probe_end_world, scale(down, along));
                let arm = sub_xyz(w.load_point, w.probe_end_world);
                let len2 = Acc::mula(arm.x(), arm.x()).madda(arm.y(), arm.y()).madd(arm.z(), arm.z());
                w.compression = len2.sqrt();
            }
        }

        if !(w.compression <= F::ZERO) {
            w.contact = true;
            // How far the contact leans, in the chassis' own frame: its normal brought back there,
            // its `z` lane dropped *before* the normalise — so only the lean within the chassis'
            // `xy` plane counts, and a contact facing straight along the chassis' `z` leaves the
            // normalise with nothing to divide by.
            let lean = ask(&mut calls, world, Call::Unrotate(w.contact_normal));
            let lean = normalize(lean.merge(Dest::Z, Vf::ZERO));
            w.compression = w.compression * load_scale(F(lean.y()));
            ask(&mut calls, world, Call::ContactVelocity);

            if !(w.compression <= w.max_compression) {
                // Bottomed out. The ray's own answer always gets the shove; a wheel riding a swept
                // contact only gets it while that contact is within reach of the suspension line.
                if w.ray_hit != 0 || along < F(BOTTOM_OUT_REACH) {
                    // The original computes this shove as a vector, stores it in a stack slot and
                    // never reads it again — the three pushes below are what actually moves the
                    // chassis. It is kept here because it is what the original computes.
                    let _push = scale(w.contact_normal,
                                      F(BOTTOM_OUT_PUSH) * (w.compression - w.max_compression));
                    ask(&mut calls, world, Call::TransformSync);
                    let into = dot3(w.chassis_velocity, w.contact_normal).min(F::ZERO);
                    c.vel = sub_xyz(c.vel, scale(w.contact_normal, into));
                    let up = ask(&mut calls, world, Call::Rotate(UP));
                    c.accel = sub_xyz(c.accel, scale(up, into));
                    let pressing = -dot3(c.force, w.contact_normal).min(F::ZERO);
                    c.force = add_xyz(c.force, scale(w.contact_normal, pressing));
                }
                w.compression = w.max_compression;
                ask(&mut calls, world, Call::ContactVelocity);
            }

            // The spring against the compression, less the damper against the closing speed, never
            // pulling the wheel back down onto the ground.
            let closing = dot3(w.contact_velocity, w.contact_normal);
            let closing = closing.max(-w.max_damper_speed).min(w.max_damper_speed);
            let damping = if F::ZERO < closing { w.damping_out } else { w.damping_in };
            w.spring_force = (w.spring_rate * w.compression - damping * closing).max(F::ZERO);
            let force = scale(w.contact_normal, w.spring_force);

            if w.steered && w.balancing {
                ask(&mut calls, world, Call::HingeForce(force, w.load_point));
            } else {
                // Into the chassis, in the chassis' own frame: the force at the centre of mass and
                // the torque the arm from it puts on the body. The component-wise multiply by
                // `(1, 1, 1, 1)` on the way is the original's, and is left in.
                let local_force = ask(&mut calls, world, Call::Unrotate(force));
                let local_point = ask(&mut calls, world, Call::Unpoint(w.load_point));
                let arm = sub_xyz(local_point, c.cofg);
                c.force = add_xyz(c.force, force);
                let cross = Vf::opmsub(Vf::opmula(arm, local_force), local_force, arm);
                let torque = ONE.merge(Dest::XYZ, ONE.mul(arm.merge(Dest::XYZ, cross)));
                c.torque = add_xyz(c.torque, torque);
            }
        }
    }
    if !w.contact {
        w.spring_force = F::ZERO;
        w.contact_velocity = VF0;
    }
    // Letting go is rate limited, taking load up is not: a wheel dropping into a hole extends at
    // most `extension_rate` a second, so the tire does not snap after the ground.
    if w.compression < w.last_compression {
        let step = (w.last_compression - w.compression).min(w.extension_rate * dt);
        w.compression = w.last_compression - step;
    }
    w.last_compression = w.compression.min(w.max_compression);
    calls
}

// --- the wheel's own integration ------------------------------------------------------------------

/// **The wheel's spin and steering**, the last of the three steps.
///
/// Its own arithmetic is the dispatch and the steering hinge; the spin and the surface's rolling
/// resistance are two functions of their own, and are recorded rather than ported.
///
/// The steering has three cases, and [`Wheel::steered`] and [`Wheel::balancing`] choose between them:
/// a wheel that does not steer is left alone, a steered wheel that is not being balanced on keeps the
/// angle the ground step commanded and has its rate thrown away, and a steered wheel the bike is
/// balancing on turns under its own torque.
pub fn integrate(w: &mut Wheel, dt: F) -> Vec<Call> {
    let calls = vec![Call::SurfaceDrag(dt), Call::SpinIntegrate(dt)];
    if w.steered {
        if w.balancing {
            steer_integrate(w, dt);
        } else {
            // The commanded angle stands; what the substep's torques did to the hinge does not.
            w.steer_rate = F::ZERO;
            w.steer_torque = F::ZERO;
        }
    }
    calls
}

/// The steering hinge, integrated: the accumulated torque less its damping, into the rate, into the
/// angle, held inside the lock. Hitting either end of the lock stops the bars dead rather than letting
/// them wind up against it, and the torque is spent either way.
fn steer_integrate(w: &mut Wheel, dt: F) {
    let torque = w.steer_torque - w.steer_damping * w.steer_rate;
    let mut rate = w.steer_rate + dt * (torque * w.steer_inv_inertia);
    let mut angle = w.steer_angle + rate * dt;
    // `c.ole.s` and a branch taken when it is false, so a NaN angle clamps to the top of the lock.
    if !(angle <= w.max_steer_angle) {
        angle = w.max_steer_angle;
        rate = F::ZERO;
    } else if angle < w.min_steer_angle {
        angle = w.min_steer_angle;
        rate = F::ZERO;
    }
    w.steer_angle = angle;
    w.steer_rate = rate;
    w.steer_torque = F::ZERO;
}
