//! **The quad's Moto, part 2** (MD75b), ported from MX vs. ATV Unleashed's PS2 ELF (SLUS-21104): the
//! calls the quad's three listeners make that had no port of their own, answered by [`crate::quad::Steps`]
//! alongside the bike steps MD75a ported.
//!
//! - The PreProcess's [`auto_shift`] (the gearbox, on the quad's speed and the left rear's radius)
//!   and [`head_tail`] (the centre of mass moved with the stick, the brake and the clutch ramp).
//! - The Force body's [`gravity`], [`engine_update`] (two driven wheels), [`air_drag`],
//!   [`wheel_clear`], [`rev_limit`] and [`peak_rates`].
//! - Part 3 (MD75b1): the Force body's [`spin_air`] (the 360, off the scene node's heading), and the
//!   PostProcess's [`vehicle_post`] (the quad's node placed at the body, if it is in the world) and the
//!   node's position ([`Scene::position`]), on a small scene graph of the node's own ([`Scene`]).
//! - Part 4 (MD75c): the vehicle post's three calls — the spatial update and the node's bounds
//!   ([`Scene::bounds_stale`], [`Scene::world_box`], [`Scene::database_box`]: the box the world's spatial
//!   database is handed) and the [`collision_sync`].
//!
//! Each is MX Unleashed's step of the same name where SLUS-21104 kept it (`crate::r#move`), with the
//! differences written out where they fall: the engine drives one wheel *or two*, halving the torque
//! between them, and reads the road's revs off the first one's spin alone; the head and tail picks
//! one of three poses; the gravity lives on the vehicle, not the body, and it pokes the collision
//! record. The field map is research/oracle.txt's (`# MD75b`, `# MD75b1`, `# MD75c`). Proof: `tests/atv_quad.rs`, each call
//! unhooked against SLUS-21104's own code.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::r#move::{Curve, Engine, MAX_RPM_POINTS, Params, set_gear};
use crate::quad::{Bike, Body, Call, Quad};
use ps2_float::vu::{self, Dest, Field};
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
    /// MD75b1's: the 360's words ([`Spin`]), the scene graph the quad's node is in ([`Scene`], the
    /// quad's node first) and the world's bounds the vehicle post holds the quad in ([`Bounds`]).
    pub spin: Spin,
    pub scene: Scene,
    pub bounds: Bounds,
    /// MD75c's: whether the quad has an entry of its own in the world's spatial database (`+8`), and
    /// the collidables the collision sync keeps up to date ([`Collidables`]).
    pub spatial: bool,
    pub collidables: Collidables,
    /// MD86's: the collidable the last rear contact struck (`+0x143c`, [`crate::quad::wheel_contacts`]'s
    /// `rear_node`; 0 for none).
    pub rear_node: u32,
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
            spin: Spin::default(),
            scene: Scene::default(),
            bounds: Bounds::default(),
            spatial: false,
            collidables: Collidables::default(),
            rear_node: 0,
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

// --- MD75b1: the air spin, the vehicle's post step and the scene node's position ----------------------

/// π and 2π as the 360's heading test loads them, and the 35° its clip changes at.
const PI: u32 = 0x4049_0fdb;
const TWO_PI: u32 = 0x40c9_0fdb;
const SPIN_CLIP_ANGLE: u32 = 0x3f1c_61a9; // 0.6108652
/// How far the stick goes over to arm the 360 or fire it, and the grace it is given on landing.
const SPIN_STICK: u32 = 0x3f33_3333; // 0.7
const SPIN_GRACE: u32 = 0x3f66_6666; // 0.9
/// The lip: rising faster than this, and faster than this forward.
const LIP_RISE: f32 = 5.0;
const LIP_SPEED: f32 = 30.0;
/// The low nibble of the trick buttons that arms it: R1 alone.
const SPIN_BUTTONS: u8 = 4;

/// The 360's words on the quad (`+0x11dc..+0x1224`, `+0x12b4`, `+0x13bc`/`+0x13c0`, `+0x1758`):
/// armed, the stick flick's time range its torques are mapped over and the torques (pitch `x`, yaw
/// `y`) at either end, the grace after landing, the yaw's decay rate under an opposite stick, the
/// moment it pushes (`x` pitch, `y` yaw; `z` and `w` are not written), the heading it took off on,
/// its direction (`1` left, `-1` right), the stick's flick, the stunt ids for the two directions and
/// the 360's clip (0 none, 1 started, 2 past 35°). Its applied and active words are
/// [`Bike::spin_applied`] and [`Bike::spin_active`], its ramp [`Quad::spinning`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spin {
    pub armed: bool,
    pub min_time: F,
    pub max_time: F,
    pub grace: F,
    pub min_pitch: F,
    pub max_pitch: F,
    pub min_yaw: F,
    pub max_yaw: F,
    pub rate: F,
    pub moment: Vf,
    pub heading: F,
    pub dir: i32,
    pub flick: F,
    pub stunt_right: u32,
    pub stunt_left: u32,
    pub clip: i32,
}

/// How many scene nodes [`Scene`] holds: the quad's and enough around it for a parent, a child and
/// a sibling.
pub const NODES: usize = 4;

/// A scene node's frame words (`+0x30..+0xac`, `+0xf0..+0xf8`, `+0x100`): its frame in its parent's
/// and in the world (rows `x`, `y`, `z`, then the position), its parent, first child and next
/// sibling (indices into [`Scene::nodes`]), and its flag byte — bit 0 the world frame is current,
/// bit 7 the bounds are stale.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Node {
    pub local: [Vf; 4],
    pub world: [Vf; 4],
    pub parent: Option<usize>,
    pub child: Option<usize>,
    pub sibling: Option<usize>,
    pub flags: u8,
    /// MD75c's: whether the node has an entry in the world's spatial database (`+8`); its box in its
    /// own frame, the centre (`+0xd0`) and the half extent (`+0xe0`); its second flag byte (`+0x101`,
    /// bit 2: the database is handed a box round the node's origin); and its reach (`+0x108`).
    pub spatial: bool,
    pub centre: Vf,
    pub half: Vf,
    pub more: u8,
    pub reach: F,
    /// MD75c3's: the node's own bounds in its frame, a box (`+0xb0` centre, `+0xc0` half extent) or a
    /// sphere (the same centre, `+0x104` the radius), there with bit 2 of the flag byte; and the nodes
    /// its gather takes in (the `+0x144` list, `+0x10c` long), as places in [`Scene::nodes`].
    pub own_centre: Vf,
    pub own_half: Vf,
    pub radius: F,
    pub list: [usize; NODES],
    pub listed: usize,
}

/// The scene graph around the quad: node 0 is the quad's (`+0xe4`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Scene {
    pub nodes: [Node; NODES],
}

/// The world's bounds (quad `+0xe8`): its floor (`+0x98`) and the box (`+0xa8`, `None` for none; its
/// `+0x30` low corner and `+0x40` high, of which `x` and `z` are read).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bounds {
    pub floor: F,
    pub area: Option<(Vf, Vf)>,
}

/// The quad's collidables as the collision sync sees them (MD75c): the flag that turns it on (bit 0
/// of the byte before the flag byte, `+0x44`), the riding probe (`+0xcc`), the frame mesh (`+0xd0`)
/// and the active probe (`+0xd8`, the one the nearby list is gathered round). The riding and the
/// active probe are the same one but for a moment during a swap. MD75c2's: what each collidable's
/// sync reads and writes, by [`crate::atv_sync::slot`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Collidables {
    pub sync: bool,
    pub riding: Option<crate::quad::Part>,
    pub frame: Option<crate::quad::Part>,
    pub active: Option<crate::quad::Part>,
    pub states: [crate::atv_sync::Collidable; 5],
}

const CLEAN: u8 = 0x01;
const OWN_BOUNDS: u8 = 0x08;
const BOUNDED: u8 = 0x40;
const STALE_BOUNDS: u8 = 0x80;
/// The second flag byte's bit for a database box round the node's origin.
const ORIGIN_REACH: u8 = 0x04;
/// The flag byte's bits for the node's own bounds and for bounds gathered as a sphere (MD75c3).
const OWN_BOX: u8 = 0x04;
const SPHERE: u8 = 0x10;

/// A point of a node's frame carried into the world (`vaddax` of the position row and 0, then
/// `vmaddax`, `vmadday`, `vmaddz` of the rows): `xyz`, the `w` the point's.
fn frame_point(m: &[Vf; 4], p: Vf) -> Vf {
    let acc = Vf::madd(Vf::madd(m[3].add(Vf::ZERO), m[0], p.bc(Field::X)), m[1], p.bc(Field::Y));
    p.merge(Dest::XYZ, Vf::madd(acc, m[2], p.bc(Field::Z)))
}

/// `vmini` / `vmax` (`want_max`) lane by lane, `-0` below `+0`. The ps2-oracle's VU compares the bit
/// patterns in sign-magnitude order, while `ps2_float`'s `Vf::mini`/`max` hand back the first operand
/// on a ±0 tie. A box corner at `-0` against the `+0` of `vf0` shows the difference (MD75c3).
fn pick(a: Vf, b: Vf, want_max: bool) -> Vf {
    let key = |x: f32| {
        let i = x.to_bits() as i32;
        i ^ (i >> 31 & 0x7fff_ffff)
    };
    Vf(core::array::from_fn(|i| {
        let (x, y) = (vu::operand(a.0[i].to_bits()), vu::operand(b.0[i].to_bits()));
        if (key(x) > key(y)) == want_max { x } else { y }
    }))
}

/// A world point brought into a node's frame (MD75c3, the inverse of [`frame_point`]): the position
/// row taken off (`xyz`), then each lane the row's dot with it, over the 3×3 transposed (`vmulax`,
/// `vmadday`, `vmaddz`); the `w` the point's.
fn point_in_frame(m: &[Vf; 4], p: Vf) -> Vf {
    let d = sub_xyz(p, m[3]);
    let col = |k: usize| Vf::new(m[0].0[k], m[1].0[k], m[2].0[k], 0.0);
    let v = Vf::madd(Vf::madd(col(0).mul(d.bc(Field::X)), col(1), d.bc(Field::Y)), col(2), d.bc(Field::Z));
    d.merge(Dest::XYZ, v)
}

/// `a · b` as the scene graph multiplies frames (`vmulax`, `vmadday`, `vmaddz` a row; the position
/// row starts from `b`'s with `vaddax` of 0): `a`'s rows in `b`'s space.
pub(crate) fn frame_mul(a: &[Vf; 4], b: &[Vf; 4]) -> [Vf; 4] {
    let row = |r: Vf, acc: Vf| Vf::madd(Vf::madd(Vf::madd(acc, b[0], r.bc(Field::X)), b[1], r.bc(Field::Y)), b[2], r.bc(Field::Z));
    let first = |r: Vf| Vf::madd(Vf::madd(b[0].mul(r.bc(Field::X)), b[1], r.bc(Field::Y)), b[2], r.bc(Field::Z));
    [first(a[0]), first(a[1]), first(a[2]), row(a[3], b[3].add(Vf::ZERO))]
}

/// A rigid frame's inverse, as the frame set takes it of the parent's: the 3×3 transposed (each
/// moved lane `0 + x`, the `w`s left), the position `0 - x·rx - y·ry - z·rz` over the transposed rows.
fn frame_inverse(m: &[Vf; 4]) -> [Vf; 4] {
    let lane = |v: Vf, d: Dest, x: f32| v.merge(d, Vf::splat(vu::add(0.0, x)));
    let r0 = lane(lane(m[0], Dest::Z, m[2].x()), Dest::Y, m[1].x());
    let r1 = lane(lane(m[1], Dest::Z, m[2].y()), Dest::X, m[0].y());
    let r2 = lane(lane(m[2], Dest::Y, m[1].z()), Dest::X, m[0].z());
    let p = m[3];
    let acc = Vf::msub(Vf::msub(Vf::ZERO, r0, p.bc(Field::X)), r1, p.bc(Field::Y));
    [r0, r1, r2, p.merge(Dest::XYZ, Vf::msub(acc, r2, p.bc(Field::Z)))]
}

/// The quaternion's frame at `pos` (`atv_quat_frame`): `q2 = q + q`, the rows `q·q2.x`, `q·q2.y`,
/// `q·q2.z` with `w` 0, the off-diagonals `±` `q·q2.w`'s lanes, the diagonal `0 - ((a + b) - 1)`;
/// the position row `pos` with `w` 1.
pub fn quat_frame(q: Vf, pos: Vf) -> [Vf; 4] {
    let q2 = q.add(q);
    let w3 = q.mul(q2.bc(Field::W));
    let row = |f: Field| q.mul(q2.bc(f)).merge(Dest::W, Vf::ZERO);
    let (mut r0, mut r1, mut r2) = (row(Field::X), row(Field::Y), row(Field::Z));
    let d = Vf::new(vu::add(r0.x(), r2.z()), vu::add(r1.y(), r0.x()), vu::add(r2.z(), r1.y()), 0.0);
    let d = Vf::new(vu::sub(d.x(), 1.0), vu::sub(d.y(), 1.0), vu::sub(d.z(), 1.0), 0.0);
    r0.0[1] = vu::add(r0.y(), w3.z());
    r1.0[0] = vu::sub(r1.x(), w3.z());
    r2.0[0] = vu::add(r2.x(), w3.y());
    r0.0[2] = vu::sub(r0.z(), w3.y());
    r1.0[2] = vu::add(r1.z(), w3.x());
    r2.0[1] = vu::sub(r2.y(), w3.x());
    r0.0[0] = vu::sub(0.0, d.z());
    r1.0[1] = vu::sub(0.0, d.x());
    r2.0[2] = vu::sub(0.0, d.y());
    [r0, r1, r2, pos.merge(Dest::W, Vf::new(0.0, 0.0, 0.0, 1.0))]
}

impl Scene {
    /// **The node's world frame brought up to date** (`atv_node_world_update`): unless it is current, the
    /// parents up to the first current one (or the root), then each from the top down, its frame
    /// times its parent's world frame (the root's own frame as it is), marked current.
    pub fn update(&mut self, n: usize) {
        if self.nodes[n].flags & CLEAN != 0 {
            return;
        }
        let mut chain = vec![n];
        let mut up = self.nodes[n].parent;
        while let Some(p) = up {
            if self.nodes[p].flags & CLEAN != 0 {
                break;
            }
            chain.push(p);
            up = self.nodes[p].parent;
        }
        for &i in chain.iter().rev() {
            let node = self.nodes[i];
            self.nodes[i].world = match node.parent {
                Some(p) => frame_mul(&node.local, &self.nodes[p].world),
                None => node.local,
            };
            self.nodes[i].flags |= CLEAN;
        }
    }

    /// **A frame changed** (`atv_node_moved`): if the node was current, it and every parent get their
    /// bounds marked stale (`atv_node_stale_bounds`), it is marked out of date, and so is each current node
    /// under it (`atv_node_out_of_date`: down a sibling list, into a node's children only if it was current).
    fn moved(&mut self, n: usize) {
        if self.nodes[n].flags & CLEAN == 0 {
            return;
        }
        let mut up = Some(n);
        while let Some(p) = up {
            self.nodes[p].flags |= STALE_BOUNDS;
            up = self.nodes[p].parent;
        }
        self.nodes[n].flags &= !CLEAN;
        if let Some(c) = self.nodes[n].child {
            self.out_of_date(c);
        }
    }

    fn out_of_date(&mut self, first: usize) {
        let mut at = Some(first);
        while let Some(n) = at {
            if self.nodes[n].flags & CLEAN != 0 {
                self.nodes[n].flags &= !CLEAN;
                if let Some(c) = self.nodes[n].child {
                    self.out_of_date(c);
                }
            }
            at = self.nodes[n].sibling;
        }
    }

    /// **The node placed in the world** (`atv_node_orientation` with an orientation and a position): the
    /// quaternion's frame ([`quat_frame`]), taken into the parent's (its world frame brought up to
    /// date first, then inverted) if there is one, as the node's own; then [`Scene::moved`].
    pub fn place(&mut self, n: usize, q: Vf, pos: Vf) {
        let m = quat_frame(q, pos);
        self.nodes[n].local = match self.nodes[n].parent {
            Some(p) => {
                self.update(p);
                frame_mul(&m, &frame_inverse(&self.nodes[p].world))
            }
            None => m,
        };
        self.moved(n);
    }

    /// **Where the node is** (`atv_node_position` in the world): its world frame brought up to date, the
    /// position row with `w` 1.
    pub fn position(&mut self, n: usize) -> Vf {
        self.update(n);
        self.nodes[n].world[3].merge(Dest::W, Vf::new(0.0, 0.0, 0.0, 1.0))
    }

    /// **The node's bounds are to be gathered again** (MD75c), after its world frame is brought up to
    /// date: they are stale (bit 7), and the node has its own (bit 3) or none yet (bit 6 clear). The
    /// caller then makes the node's gather (its vt `+0x94`) and clears bit 7 ([`Scene::rebounded`]).
    pub fn bounds_stale(&mut self, n: usize) -> bool {
        self.update(n);
        let f = self.nodes[n].flags;
        f & STALE_BOUNDS != 0 && (f & OWN_BOUNDS != 0 || f & BOUNDED == 0)
    }

    /// **The node's bounds gathered again** (`atv_node_rebound`, its vt `+0x94`; MD75c3), its world frame
    /// brought up to date first. Each listed node with bounds of its own (bit 2) has its world frame
    /// brought up to date as it is taken in; one without is passed over.
    ///
    /// - **As a sphere** (bit 4): the node's own centre in the world with its radius, if it has them
    ///   (else the radius 0). A listed node's centre in the world ([`frame_point`]) with its radius
    ///   replaces a radius not above 0; else the radius grows to the centre's farthest lane from it
    ///   (`vabs`, `vmaxy.x`, `vmaxz.x`) plus that radius, if that is more (`add.s`, `c.ole.s`). The
    ///   centre goes back into the node's frame ([`point_in_frame`], `w` the centre's); the half
    ///   extent's `xyz` (its `w` kept) and the reach are the radius.
    /// - **As a box**: low and high corners from `(0, 0, 0, 1)`, or the node's own box (`xyz`, both
    ///   `w`s the centre's). A listed node's box is taken through its world frame times the inverse of
    ///   the node's ([`frame_mul`], `frame_inverse`) as [`Scene::world_box`] takes one; its corners keep
    ///   the `w`s of that frame's rows `x` and `y`, and after the first box in, they are `vmini` and
    ///   `vmax` with the corners so far, every lane. The centre is half the corners' sum (`xyz`, `w` the
    ///   low's), the half extent half their difference (`w` the high's), the reach that half extent's
    ///   length (`mula`, `madda`, `madd`, `sqrt`).
    ///
    /// The reach goes to `+0x108`, and a reach above 0 sets bit 6. A sphere with neither its own
    /// bounds nor a listed node with them has no centre: the original's is a stack word never written.
    /// The port takes `(0, 0, 0, 0)`, and the oracle does not make that case.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub fn rebound(&mut self, n: usize) {
        self.update(n);
        let me = self.nodes[n];
        let listed = me.list.into_iter().take(me.listed);
        let reach = if me.flags & SPHERE != 0 {
            let (mut centre, mut r) = (Vf::ZERO, F::ZERO);
            if me.flags & OWN_BOX != 0 {
                (centre, r) = (frame_point(&me.world, me.own_centre), me.radius);
            }
            for m in listed {
                if self.nodes[m].flags & OWN_BOX == 0 {
                    continue;
                }
                self.update(m);
                let o = &self.nodes[m];
                let p = frame_point(&o.world, o.own_centre);
                if r <= F::ZERO {
                    (centre, r) = (p, o.radius);
                } else {
                    let d = sub_xyz(p, centre).abs();
                    let far = F(vu::max(vu::max(d.x(), d.y()), d.z())) + o.radius;
                    if !(far <= r) {
                        r = far;
                    }
                }
            }
            self.update(n);
            let node = &mut self.nodes[n];
            node.centre = point_in_frame(&node.world, centre);
            node.half = node.half.merge(Dest::XYZ, Vf::splat(r.0));
            r
        } else {
            let (mut lo, mut hi, mut had) = (Vf::VF0, Vf::VF0, false);
            if me.flags & OWN_BOX != 0 {
                (lo, hi, had) = (sub_xyz(me.own_centre, me.own_half), add_xyz(me.own_centre, me.own_half), true);
            }
            for m in listed {
                if self.nodes[m].flags & OWN_BOX == 0 {
                    continue;
                }
                self.update(m);
                let o = &self.nodes[m];
                let rel = frame_mul(&o.world, &frame_inverse(&self.nodes[n].world));
                let (c, e) = (frame_point(&rel, o.own_centre), o.own_half);
                let (hx, hy, hz) = (rel[0].mul(e.bc(Field::X)).abs(), rel[1].mul(e.bc(Field::Y)).abs(), rel[2].mul(e.bc(Field::Z)).abs());
                let e = Vf::madd(hy.add(hx), hz, Vf::splat(1.0));
                let (l, h) = (rel[0].merge(Dest::XYZ, c.sub(e)), rel[1].merge(Dest::XYZ, c.add(e)));
                (lo, hi) = if had { (pick(l, lo, false), pick(h, hi, true)) } else { (l, h) };
                had = true;
            }
            let half_of = |v: Vf, keep: Vf| keep.merge(Dest::XYZ, v.mul(Vf::splat(0.5)));
            let node = &mut self.nodes[n];
            node.centre = half_of(lo.add(hi), lo);
            node.half = half_of(hi.sub(lo), hi);
            let h = node.half;
            Acc::mula(F(h.x()), F(h.x())).madda(F(h.y()), F(h.y())).madd(F(h.z()), F(h.z())).sqrt()
        };
        let node = &mut self.nodes[n];
        node.reach = reach;
        if !(reach <= F::ZERO) {
            node.flags |= BOUNDED;
        }
    }

    /// The node's bounds gathered: bit 7 cleared.
    pub fn rebounded(&mut self, n: usize) {
        self.nodes[n].flags &= !STALE_BOUNDS;
    }

    /// **The node's box in the world** (`atv_node_world_box`, after [`Scene::bounds_stale`]): the centre
    /// carried through the world frame (`vaddax` of the position row and 0, then `vmaddax`, `vmadday`,
    /// `vmaddz` of the rows), the half extent the sum of each row times its lane, made positive
    /// (`|y| + |x|` into the accumulator, then `+ |z|·1`). `xyz` only: the `w`s are the box's own.
    pub fn world_box(&self, n: usize) -> (Vf, Vf) {
        let node = &self.nodes[n];
        let (m, c, e) = (&node.world, node.centre, node.half);
        let (hx, hy, hz) = (m[0].mul(e.bc(Field::X)).abs(), m[1].mul(e.bc(Field::Y)).abs(), m[2].mul(e.bc(Field::Z)).abs());
        let acc = Vf::madd(Vf::madd(m[3].add(Vf::ZERO), m[0], c.bc(Field::X)), m[1], c.bc(Field::Y));
        let centre = c.merge(Dest::XYZ, Vf::madd(acc, m[2], c.bc(Field::Z)));
        let half = e.merge(Dest::XYZ, Vf::madd(hy.add(hx), hz, Vf::splat(1.0)));
        (centre, half)
    }

    /// **The box the node's bounds hand the spatial database** (`atv_node_bounds`, after its box): with
    /// bit 2 of the second flag byte, a box round the node's origin (its world position, `w` 1) that
    /// still holds the node's — the half extent grown by `|centre - origin|` (the sign bits masked off
    /// all four lanes) — reaching the length of that half extent (`mula`, `madda`, `madd`, `sqrt`);
    /// else the box as it is, reaching the node's reach word. Returns the centre, the half extent and
    /// the reach.
    pub fn database_box(&mut self, n: usize, centre: Vf, half: Vf) -> (Vf, Vf, F) {
        if self.nodes[n].more & ORIGIN_REACH == 0 {
            return (centre, half, self.nodes[n].reach);
        }
        self.update(n);
        let origin = self.nodes[n].world[3].merge(Dest::W, Vf::splat(1.0));
        let d = sub_xyz(centre, origin);
        let d = Vf(d.0.map(|x| f32::from_bits(x.to_bits() & 0x7fff_ffff)));
        let half = add_xyz(half, d);
        let reach = Acc::mula(F(half.x()), F(half.x())).madda(F(half.y()), F(half.y())).madd(F(half.z()), F(half.z())).sqrt();
        (origin, half, reach)
    }

    /// **The node's forward and up axes** (`atv_node_axes` in the world): the world frame brought up to
    /// date, rows `z` and `y` with `w` 0.
    pub fn axes(&mut self, n: usize) -> (Vf, Vf) {
        self.update(n);
        let w = self.nodes[n].world;
        (w[2].merge(Dest::W, Vf::ZERO), w[1].merge(Dest::W, Vf::ZERO))
    }
}

/// `[-1, 1]` as two compares and two moves: anything unordered comes back as `1.0`.
fn clamp_unit(c: F) -> F {
    if c <= F::ONE {
        if c < F(-1.0) { F(-1.0) } else { c }
    } else {
        F::ONE
    }
}

/// The heading of the forward axis flattened (`y` 0, normalised through `vrsqrt`), off the arc
/// cosine table: `acos(z)` (clamped twice; `mula(frac, hi)`, `madd(1 - frac, lo)`), and `(π - a) + π`
/// for an `x` below 0.
fn spin_heading(forward: Vf) -> F {
    let v = crate::air::normalize(forward.merge(Dest::Y, Vf::ZERO));
    let z = clamp_unit(clamp_unit(F(v.z())));
    let t = acos_table().entries();
    let s = (F::ONE + z) * F(ACOS_SCALE);
    let i = s.to_i32();
    let frac = s - F(fpu::from_i32(i));
    let a = F(fpu::madd(fpu::mula(frac.0, t[i as usize + 1]), (F::ONE - frac).0, t[i as usize]));
    if F(v.x()) < F::ZERO { (F::from_bits(PI) - a) + F::from_bits(PI) } else { a }
}

const ACOS_SCALE: f32 = 14999.5;

fn acos_table() -> &'static crate::air::AcosTable {
    static TABLE: std::sync::OnceLock<crate::air::AcosTable> = std::sync::OnceLock::new();
    TABLE.get_or_init(crate::air::AcosTable::build)
}

/// The fired 360's torque off the flick: the low end with no time range, else the flick mapped
/// from the time range onto the two torques and clamped between them.
fn spin_torque(s: &Spin, lo: F, hi: F) -> F {
    if s.min_time == s.max_time { lo } else { crate::air::map_clamp(s.flick.abs(), s.min_time, s.max_time, lo, hi) }
}

/// **The air spin**, the Force body's third bike step (`atv_bike_spin_air`): the 360. Its applied word is
/// cleared first.
///
/// Not on: on a lip (on the ground, no backflip applied or on, rising faster than 5 and faster than
/// 30 forward, the brake off) an armed 360 whose stick is over past 0.7 its own way fires — the yaw
/// torque its direction times [`spin_torque`] of the yaw pair, the pitch minus that of the pitch pair,
/// on, the grace 0.9 — and otherwise R1 alone with the stick past 0.7 arms it, to the left for a stick
/// left. Off a lip it is disarmed.
///
/// On: the node's forward axis is read ([`Scene::axes`]). Not yet armed, on the ground the grace runs
/// down and ends it at 0; in the air it arms with the heading it took off on ([`spin_heading`]). Armed,
/// it ends on the ground, touching something or with the yaw under the low torque — a stunt id past
/// 40 cleared to 255 and, mid-clip, the clip stopped and the quad wrecked (15 if `steep`, else 11).
/// Otherwise, with no trick button the stick held the other way decays the yaw by `rate·dir·|x|` and
/// the pitch by `x·rate` over `dt`, the moment goes into the body, applied and the ramp go on, and
/// the clip starts past 35° from the take-off heading (the stunt start answers yes), stops back
/// under it, and is marked past it.
///
/// Last, a ramp on the ground is let go and the authority restarts. Returns the stunt or wreck call.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn spin_air(b: &mut Bike, q: &mut Quad, body: &mut Body, dt: F, steep: bool) -> Option<Call> {
    let x = b.stick.x;
    let side = if x < F::ZERO { -1 } else { 1 };
    let buttons = b.buttons & 0xf;
    let s = &mut b.parts.spin;
    let mut call = None;
    b.spin_applied = false;
    if !b.spin_active {
        let lip = q.on_ground
            && !b.air.flip_torque_applied
            && !b.air.flip_active
            && !(F(body.velocity.y()) <= F(LIP_RISE))
            && !(F(q.contact_vel.z()) <= F(LIP_SPEED))
            && q.brake == F::ZERO;
        if !lip {
            b.spin_active = false;
            s.armed = false;
        } else if s.armed && !(x.abs() <= F::from_bits(SPIN_STICK)) && s.dir == side {
            let yaw = spin_torque(s, s.min_yaw, s.max_yaw);
            let pitch = spin_torque(s, s.min_pitch, s.max_pitch);
            s.moment.0[1] = (F(fpu::from_i32(s.dir)) * yaw).0;
            s.moment.0[0] = (-pitch).0;
            s.armed = false;
            b.spin_active = true;
            s.grace = F::from_bits(SPIN_GRACE);
        } else if buttons == SPIN_BUTTONS {
            if x < -F::from_bits(SPIN_STICK) {
                (s.armed, s.dir) = (true, 1);
            } else if !(x <= F::from_bits(SPIN_STICK)) {
                (s.armed, s.dir) = (true, -1);
            }
        }
    } else {
        let (forward, _) = b.parts.scene.axes(0);
        if !s.armed {
            if q.on_ground {
                s.grace = s.grace - dt;
                if s.grace <= F::ZERO {
                    b.spin_active = false;
                    s.armed = false;
                }
            } else {
                s.armed = true;
                s.heading = spin_heading(forward);
            }
        } else {
            let now = spin_heading(forward);
            let off = (s.heading - now).abs().min(((F::from_bits(TWO_PI) + s.heading) - now).abs());
            if q.on_ground || b.touching || F(s.moment.y()).abs() < s.min_yaw {
                b.spin_active = false;
                s.armed = false;
                if b.stunt_id >= 41 {
                    b.stunt_id = 255;
                }
                if s.clip != 0 {
                    s.clip = 0;
                    call = Some(Call::Wreck(if steep { 15 } else { 11 }));
                }
            } else {
                if buttons == 0 && s.dir != side {
                    let by = s.rate * (F(fpu::from_i32(s.dir)) * x.abs());
                    s.moment.0[1] = Acc::adda(F::ZERO, F(s.moment.y())).msub(dt, by).0;
                    let by = x * s.rate;
                    s.moment.0[0] = Acc::adda(F::ZERO, F(s.moment.x())).msub(dt, by).0;
                }
                body.add_moment(s.moment);
                b.spin_applied = true;
                q.spinning = true;
                let past = F::from_bits(SPIN_CLIP_ANGLE);
                if s.clip == 0 {
                    if !(off <= past) {
                        call = Some(Call::StartStunt(if s.dir > 0 { s.stunt_left } else { s.stunt_right }));
                        s.clip = 1;
                    }
                } else if off < past {
                    if s.clip == 2 {
                        call = Some(Call::EndStunt);
                        s.clip = 0;
                    }
                } else if s.clip == 1 {
                    s.clip = 2;
                }
            }
        }
    }
    if q.spinning && q.on_ground {
        q.spinning = false;
        q.authority = q.authority_time;
    }
    call
}

/// The quad's origin as the vehicle post sets the node to it: the body's position less its centre of
/// mass turned into the world (`q c q*` as two `vopmula`/`vmadd*`/`vopmsub` rounds), `w` the position's.
pub fn origin(body: &Body, cofg: Vf) -> Vf {
    let q = body.orientation;
    let acc = Vf::madd(Vf::opmula(q, cofg), cofg, q.bc(Field::W));
    let t = Vf::opmsub(acc, cofg, q);
    let p = q.mul(cofg);
    let acc = Vf::madd(Vf::madd(Vf::madd(Vf::opmula(q, t), q, p.bc(Field::X)), q, p.bc(Field::Y)), q, p.bc(Field::Z));
    let acc = Vf::madd(acc, t, q.bc(Field::W));
    let turned = Vf::opmsub(acc, t, q);
    body.position.merge(Dest::XYZ, body.position.sub(turned))
}

/// The vehicle's in-world test (quad vt `+0x120`, `atv_vehicle_in_world`): with bit 0 of the flag byte, inside
/// the box in `x` and `z` (no box, inside); else with bit 6, not under the floor; else inside.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn in_world(flags: u8, bounds: &Bounds, at: Vf) -> bool {
    let (x, y, z) = (F(at.x()), F(at.y()), F(at.z()));
    if flags & 0x01 != 0 {
        match bounds.area {
            None => true,
            Some((lo, hi)) => x < F(hi.x()) && !(x <= F(lo.x())) && z < F(hi.z()) && !(z <= F(lo.z())),
        }
    } else if flags & 0x40 != 0 {
        !(y < bounds.floor)
    } else {
        true
    }
}

/// **The vehicle's post step**, the PostProcess's first call (`atv_vehicle_post`), up to its calls: with the
/// body on, the quad's origin ([`origin`]) is tested against the world ([`in_world`]); inside, the
/// quad's node is placed there with the body's orientation ([`Scene::place`]). Returns `None` with the
/// body off (the step does nothing), else whether it was inside; the caller makes the calls that
/// follow — outside, the body's reset and the respawn (vt `+0x13c`, `+0xe8`); then the spatial
/// update (vt `+0xe4`), the node's bounds if bit 1 of the flag byte is set (`atv_node_bounds`), the
/// collision sync (vt `+0x124`) — and clears bit 2 of the flag byte.
pub fn vehicle_post(parts: &mut Parts, body: &Body, cofg: Vf) -> Option<bool> {
    if !parts.body_on {
        return None;
    }
    let at = origin(body, cofg);
    let inside = in_world(parts.flags, &parts.bounds, at);
    if inside {
        parts.scene.place(0, body.orientation, at);
    }
    Some(inside)
}

/// **The collision sync** (`atv_vehicle_collision_sync`, vt `+0x124`), the vehicle post's last call: with the sync on,
/// the riding probe brought up to date unless it is the active one, then the frame mesh, then the
/// active probe and the nearby list gathered round it — each one there. Returns the calls in order:
/// [`Call::PartSync`] for each collidable (`atv_part_sync`: stood at its node's frame, then its box into
/// the database; [`crate::atv_sync`]), then [`Call::Gather`].
pub fn collision_sync(c: &Collidables) -> Vec<Call> {
    let mut calls = Vec::new();
    if !c.sync {
        return calls;
    }
    if let Some(p) = c.riding.filter(|&p| Some(p) != c.active) {
        calls.push(Call::PartSync(p));
    }
    if let Some(p) = c.frame {
        calls.push(Call::PartSync(p));
    }
    if let Some(p) = c.active {
        calls.push(Call::PartSync(p));
        calls.push(Call::Gather(p));
    }
    calls
}
