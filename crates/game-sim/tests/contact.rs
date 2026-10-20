//! S42 — the collision object's contact resolution against the original.
//!
//! The plane's own collision object (plane+0x170) in the save state beside C7's recording is handed a
//! generated contact list, its tuning, the plane's own body, and up to two other collision objects
//! (each on a dynamics body or on a vehicle's chassis) and a mover, all written into the original's
//! RAM (the others in scratch memory); then the object's resolution runs, through its own vtable, and
//! everything it writes is compared bit for bit with the port's: the object's three written fields,
//! each contact's two arms, every field of the plane's body (its entries run as the original's, not
//! hooked), the other objects' written centres, the other bodies and the chassis' four accumulators.
//!
//! The mover's two virtuals are the only hooks: they report a spin and a velocity stashed in the
//! mover. What is not proved, and why, is in the journal entry's last section.

use game_sim::biplane::Body;
use game_sim::contact::{Collider, Contact, Mover, Node, Other, Response, Rigid, Side, Vehicle, Weighed, resolve};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

/// The save state beside C7's recording, and the plane in it (MXBiplane, the player's).
const STATE: &str = "biplane.p2m2_SaveState.eeMemory.bin";
const PLANE: u32 = 0x00c4_4a40;
const PLANE_BODY: u32 = 0x160;
const PLANE_COLLISION: u32 = 0x170;
const CASES: usize = 10_000;
const BUDGET: u64 = 400_000;

/// The collision object's fields.
mod rec {
    pub const VTABLE: u32 = 0x08;
    pub const DAMP: u32 = 0x50;
    pub const DAMP_LINEAR: u32 = 0x58;
    pub const DAMP_ANGULAR: u32 = 0x60;
    pub const MASS: u32 = 0x64;
    pub const INV_MASS: u32 = 0x68;
    pub const INV_INERTIA: u32 = 0x80;
    pub const COFG: u32 = 0x90;
    pub const COFG_WORLD: u32 = 0xa0;
    pub const VEHICLE: u32 = 0xf0;
    pub const BODY: u32 = 0xf4;
    pub const NODE: u32 = 0xf8;
    pub const COUNT: u32 = 0x100;
    pub const LIST: u32 = 0x108;
    pub const ACTORS: u32 = 0x110;
    pub const COLLIDES: u32 = 0x114;
    pub const MAX_DEPTH: u32 = 0x118;
    /// Stiffness, damping, restitution: the scenery's and the actors'.
    pub const SCENERY: u32 = 0x124;
    pub const ACTOR: u32 = 0x13c;
    pub const ANGULAR: u32 = 0x148;
    pub const ACTOR_ANGULAR: u32 = 0x14c;
    pub const SPRING_MASK: u32 = 0x150;
    pub const FRICTION_MASK: u32 = 0x160;
    pub const ACTOR_SPRING_MASK: u32 = 0x170;
    pub const ACTOR_FRICTION_MASK: u32 = 0x180;
    pub const FRICTION: u32 = 0x190;
    pub const ACTOR_FRICTION: u32 = 0x194;
    pub const SURFACES: u32 = 0x198;
    pub const NORMAL_SPEED: u32 = 0x1d8;
    pub const RELATIVE: u32 = 0x1e0;
    pub const COLLIDABLE: u32 = 0x1f8;
    /// The collidable's flags.
    pub const FLAGS: u32 = 0x84;
    /// The resolution's vtable slot.
    pub const RESOLVE: u32 = 0x80;
}

/// A contact list entry's fields.
mod entry {
    pub const SIZE: u32 = 0x80;
    pub const KIND: u32 = 0x00;
    pub const FROM: u32 = 0x10;
    pub const ON_BODY: u32 = 0x20;
    pub const NORMAL: u32 = 0x30;
    pub const POINT: u32 = 0x40;
    pub const ARM: u32 = 0x50;
    pub const OTHER_ARM: u32 = 0x60;
    pub const SURFACE: u32 = 0x70;
    pub const OTHER: u32 = 0x74;
    pub const MOVER: u32 = 0x78;
}

/// The rigid body's fields.
mod body_at {
    pub const ENABLED: u32 = 0x00;
    pub const MASS: u32 = 0x0c;
    pub const INV_MASS: u32 = 0x10;
    pub const DAMPING: u32 = 0x20;
    pub const LAST_POSITION: u32 = 0x50;
    pub const POSITION: u32 = 0x60;
    pub const VELOCITY: u32 = 0x70;
    pub const LAST_FORCE: u32 = 0x80;
    pub const FORCE: u32 = 0x90;
    pub const ACCEL: u32 = 0xa0;
    pub const SHOCK_PEAK: u32 = 0xb0;
    pub const SHOCK: u32 = 0xc0;
    pub const LISTENERS: u32 = 0xd0;
    pub const STEP: u32 = 0xf0;
    pub const INV_INERTIA: u32 = 0x120;
    pub const ANG_DAMPING: u32 = 0x130;
    pub const LAST_QUAT: u32 = 0x150;
    pub const QUAT: u32 = 0x170;
    pub const COFG: u32 = 0x180;
    pub const MOMENT: u32 = 0x190;
    pub const LAST_MOMENT: u32 = 0x1a0;
    pub const RATES: u32 = 0x1b0;
    pub const ANG_ACCEL: u32 = 0x1c0;
    pub const TWIST_PEAK: u32 = 0x1d0;
    pub const TWIST: u32 = 0x1e0;
    pub const SIZE: u32 = 0x200;
}

/// A vehicle chassis' fields, and its transform's.
mod veh {
    pub const INV_INERTIA: [u32; 3] = [0xf0, 0x104, 0x118];
    pub const SHOCK: u32 = 0x170;
    pub const TWIST: u32 = 0x180;
    pub const COFG: u32 = 0x190;
    pub const MASS: u32 = 0x210;
    pub const INV_MASS: u32 = 0x214;
    pub const VELOCITY: u32 = 0x260;
    pub const RATES: u32 = 0x290;
    pub const NODE_ROWS: u32 = 0x80;
    pub const NODE_ORIGIN: u32 = 0xb0;
    pub const NODE_CLEAN: u32 = 0x110;
}

/// A mover's fields: its vtable, the two slots the resolution calls, its transform and origin, and
/// two quadwords nothing reads, where the test stashes what the hooked slots report.
mod mov {
    pub const VTABLE: u32 = 0x08;
    pub const VELOCITY_SLOT: u32 = 0xb8;
    pub const SPIN_SLOT: u32 = 0xbc;
    pub const ORIGIN: u32 = 0x190;
    pub const NODE: u32 = 0x254;
    pub const SPIN: u32 = 0x1a0;
    pub const VELOCITY: u32 = 0x1b0;
}

/// The interpreter on the state's RAM, with `$gp` as the boot code leaves it, or why not.
fn plane() -> Option<Ee> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let path = kit.ctx("states").join(STATE);
    if !path.exists() {
        skip(&format!("{} is not there: C7's save state, `./kit state ram` of biplane.p2m2_SaveState.p2s", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

/// A deterministic generator for the cases.
struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    fn one_in(&mut self, n: u64) -> bool {
        self.next() % n == 0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    /// Mostly `x` scaled by up to half either way, sometimes zero, sometimes negated.
    fn around(&mut self, x: f32) -> f32 {
        match self.next() % 16 {
            0 => 0.0,
            1 => -x,
            2 => x,
            _ => x * self.range(0.5, 1.5),
        }
    }
    fn vector(&mut self, scale: f32, w: f32) -> Vf {
        Vf::new(self.range(-scale, scale), self.range(-scale, scale), self.range(-scale, scale), w)
    }
    /// A vector with any `w`: the lanes the original carries along without reading.
    fn loose(&mut self, scale: f32) -> Vf {
        let w = self.range(-2.0, 2.0);
        self.vector(scale, w)
    }
    /// A unit quaternion as the host rounds it, now and then a little off the sphere or an axis one.
    fn quat(&mut self) -> Vf {
        match self.next() % 12 {
            0 => Vf::new(0.0, 0.0, 0.0, 1.0),
            1 => Vf::new(0.0, 1.0, 0.0, 0.0),
            _ => {
                let q = [0; 4].map(|_| self.range(-1.0, 1.0));
                let n = q.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-3) * self.range(0.999, 1.001);
                Vf(q.map(|x| x / n))
            }
        }
    }
    /// A contact normal: unit, an axis, or now and then not unit at all; any `w`.
    fn normal(&mut self) -> Vf {
        let w = if self.one_in(3) { 0.0 } else { self.range(-2.0, 2.0) };
        match self.next() % 8 {
            0 => Vf::new(0.0, 1.0, 0.0, w),
            1 => Vf::new(-1.0, 0.0, -0.0, w),
            2 => self.loose(1.5),
            _ => {
                let v = [0; 3].map(|_| self.range(-1.0, 1.0));
                let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-3);
                Vf::new(v[0] / n, v[1] / n, v[2] / n, w)
            }
        }
    }
    /// A moment mask: the shipped all-ones, or lanes cut, or anything.
    fn mask(&mut self) -> Vf {
        match self.next() % 4 {
            0 | 1 => Vf::new(1.0, 1.0, 1.0, 1.0),
            2 => Vf([0; 4].map(|_| if self.one_in(2) { 0.0 } else { 1.0 })),
            _ => self.loose(2.0),
        }
    }
    /// Collidable flags: four bits, so masks overlap often.
    fn flags(&mut self) -> u32 {
        (self.next() % 16) as u32
    }
    /// A transform's three rows and origin: a rotation, scaled now and then, with the rows' `w`s
    /// anything (the first row's is read in place of a `z`).
    fn node(&mut self) -> Node {
        let q = self.quat();
        let (x, y, z, w) = (q.x(), q.y(), q.z(), q.w());
        let s = if self.one_in(4) { self.range(0.5, 2.0) } else { 1.0 };
        let mut w0 = || if self.one_in(2) { 0.0 } else { self.range(-1.0, 1.0) };
        let rows = [
            Vf::new(s * (1.0 - 2.0 * (y * y + z * z)), s * 2.0 * (x * y + z * w), s * 2.0 * (x * z - y * w), w0()),
            Vf::new(s * 2.0 * (x * y - z * w), s * (1.0 - 2.0 * (x * x + z * z)), s * 2.0 * (y * z + x * w), w0()),
            Vf::new(s * 2.0 * (x * z + y * w), s * 2.0 * (y * z - x * w), s * (1.0 - 2.0 * (x * x + y * y)), w0()),
        ];
        Node { rows, origin: self.vector(100.0, 1.0) }
    }
}

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| ee.f32_at(a + k)))
}

fn set_quad(ee: &mut Ee, a: u32, v: Vf) {
    for k in 0..4 {
        ee.set_f32(a + 4 * k as u32, v[k]);
    }
}

fn zero(ee: &mut Ee, a: u32, n: u32) {
    for k in (0..n).step_by(4) {
        ee.set_u32(a + k, 0);
    }
}

fn same_quad(case: usize, what: &str, got: Vf, want: Vf) {
    for k in 0..4 {
        assert_eq!(got[k].to_bits(), want[k].to_bits(),
                   "case {case}: {what}[{k}] is {:e} in the game, {:e} in the port ({want:?} / {got:?})", want[k], got[k]);
    }
}

// --- the objects, both ways ------------------------------------------------------------------------

fn response(ee: &Ee, r: u32, at: u32, angular: u32, spring: u32, friction_mask: u32, friction: u32) -> Response {
    Response {
        stiffness: F(ee.f32_at(r + at)),
        damping: F(ee.f32_at(r + at + 4)),
        restitution: F(ee.f32_at(r + at + 8)),
        angular: ee.u32_at(r + angular) != 0,
        friction: F(ee.f32_at(r + friction)),
        spring_mask: quad(ee, r + spring),
        friction_mask: quad(ee, r + friction_mask),
    }
}

fn collider_of(ee: &Ee, r: u32) -> Collider {
    use rec::*;
    let c = ee.u32_at(r + COLLIDABLE);
    Collider {
        damp: ee.u32_at(r + DAMP) != 0,
        damp_linear: F(ee.f32_at(r + DAMP_LINEAR)),
        damp_angular: F(ee.f32_at(r + DAMP_ANGULAR)),
        mass: F(ee.f32_at(r + MASS)),
        inv_mass: F(ee.f32_at(r + INV_MASS)),
        inv_inertia: quad(ee, r + INV_INERTIA),
        cofg: quad(ee, r + COFG),
        cofg_world: quad(ee, r + COFG_WORLD),
        actors: ee.u32_at(r + ACTORS),
        collides_with: ee.u32_at(r + COLLIDES),
        collidable: (c != 0).then(|| ee.u32_at(c + FLAGS)),
        max_depth: F(ee.f32_at(r + MAX_DEPTH)),
        scenery: response(ee, r, SCENERY, ANGULAR, SPRING_MASK, FRICTION_MASK, FRICTION),
        actor: response(ee, r, ACTOR, ACTOR_ANGULAR, ACTOR_SPRING_MASK, ACTOR_FRICTION_MASK, ACTOR_FRICTION),
        surfaces: core::array::from_fn(|k| F(ee.f32_at(r + SURFACES + 4 * k as u32))),
        normal_speed: F(ee.f32_at(r + NORMAL_SPEED)),
        relative: quad(ee, r + RELATIVE),
    }
}

/// A collision object's tuning around the plane's shipped values, written over `r`'s fields.
fn write_collider(ee: &mut Ee, p: &mut Pick, r: u32) {
    use rec::*;
    ee.set_u32(r + DAMP, u32::from(!p.one_in(4)));
    for at in [DAMP_LINEAR, DAMP_ANGULAR] {
        let k = match p.below(4) {
            0 => 1.0,
            1 => p.range(0.0, 1.0),
            2 => p.range(1.0, 3.0),
            _ => p.range(-0.5, 1.5),
        };
        ee.set_f32(r + at, k);
    }
    let m = p.around(310.559);
    ee.set_f32(r + MASS, m);
    ee.set_f32(r + INV_MASS, if p.one_in(8) { 0.0 } else { p.around(0.00322) });
    let iw = p.range(-2.0, 2.0);
    let ii = p.vector(0.01, iw);
    set_quad(ee, r + INV_INERTIA, ii);
    let c = if p.one_in(4) { Vf::new(0.0, 0.0, 0.0, 1.0) } else { p.loose(4.0) };
    set_quad(ee, r + COFG, c);
    let junk = p.loose(50.0);
    set_quad(ee, r + COFG_WORLD, junk);
    ee.set_u32(r + ACTORS, p.flags());
    ee.set_u32(r + COLLIDES, if p.one_in(4) { 0xffff_ffff } else { p.flags() });
    ee.set_f32(r + MAX_DEPTH, p.around(0.5));
    for (at, k, d) in [(SCENERY, 4500.0, 640.991), (ACTOR, 4500.0, 640.991)] {
        ee.set_f32(r + at, p.around(k));
        ee.set_f32(r + at + 4, p.around(d));
        ee.set_f32(r + at + 8, if p.one_in(3) { 0.0 } else { p.range(0.0, 1.0) });
    }
    ee.set_u32(r + ANGULAR, u32::from(!p.one_in(4)));
    ee.set_u32(r + ACTOR_ANGULAR, u32::from(!p.one_in(4)));
    for at in [SPRING_MASK, FRICTION_MASK, ACTOR_SPRING_MASK, ACTOR_FRICTION_MASK] {
        let m = p.mask();
        set_quad(ee, r + at, m);
    }
    for k in 0..18 {
        ee.set_f32(r + FRICTION + 4 * k, p.around(0.5));
    }
    ee.set_f32(r + NORMAL_SPEED, p.range(-50.0, 50.0));
    let junk = p.loose(50.0);
    set_quad(ee, r + RELATIVE, junk);
}

fn node_of(ee: &Ee, n: u32) -> Node {
    use veh::*;
    Node { rows: [0, 1, 2].map(|k| quad(ee, n + NODE_ROWS + 16 * k)), origin: quad(ee, n + NODE_ORIGIN) }
}

fn write_node(ee: &mut Ee, n: u32, node: &Node) {
    use veh::*;
    zero(ee, n, 0x120);
    for (k, r) in node.rows.iter().enumerate() {
        set_quad(ee, n + NODE_ROWS + 16 * k as u32, *r);
    }
    set_quad(ee, n + NODE_ORIGIN, node.origin);
    // Clean: the transform does not rebuild itself from its parents.
    ee.set_u32(n + NODE_CLEAN, 1);
}

fn vehicle_of(ee: &Ee, v: u32, node: u32) -> Vehicle {
    use veh::*;
    Vehicle {
        node: node_of(ee, node),
        cofg: quad(ee, v + COFG),
        mass: F(ee.f32_at(v + MASS)),
        inv_mass: F(ee.f32_at(v + INV_MASS)),
        inv_inertia: INV_INERTIA.map(|at| F(ee.f32_at(v + at))),
        velocity: quad(ee, v + VELOCITY),
        rates: quad(ee, v + RATES),
        shock: quad(ee, v + SHOCK),
        twist: quad(ee, v + TWIST),
    }
}

fn write_vehicle(ee: &mut Ee, p: &mut Pick, v: u32) {
    use veh::*;
    zero(ee, v, 0x2a0);
    for (at, s) in [(COFG, 3.0), (VELOCITY, 30.0), (RATES, 3.0), (SHOCK, 10.0), (TWIST, 10.0)] {
        let q = p.loose(s);
        set_quad(ee, v + at, q);
    }
    ee.set_f32(v + MASS, p.around(150.0));
    ee.set_f32(v + INV_MASS, p.around(1.0 / 150.0));
    for at in INV_INERTIA {
        ee.set_f32(v + at, p.around(0.02));
    }
}

/// The body as the port holds it, off the original's fields.
fn body_of(ee: &Ee, b: u32) -> Body {
    use body_at::*;
    Body {
        enabled: ee.u32_at(b + ENABLED) != 0,
        inv_mass: F(ee.f32_at(b + INV_MASS)),
        damping: quad(ee, b + DAMPING),
        last_position: quad(ee, b + LAST_POSITION),
        position: quad(ee, b + POSITION),
        velocity: quad(ee, b + VELOCITY),
        last_force: quad(ee, b + LAST_FORCE),
        force: quad(ee, b + FORCE),
        accel: quad(ee, b + ACCEL),
        shock_peak: quad(ee, b + SHOCK_PEAK),
        shock: quad(ee, b + SHOCK),
        step: quad(ee, b + STEP),
        inv_inertia: quad(ee, b + INV_INERTIA),
        ang_damping: quad(ee, b + ANG_DAMPING),
        last_quat: quad(ee, b + LAST_QUAT),
        quat: quad(ee, b + QUAT),
        cofg: quad(ee, b + COFG),
        moment: quad(ee, b + MOMENT),
        last_moment: quad(ee, b + LAST_MOMENT),
        rates: quad(ee, b + RATES),
        ang_accel: quad(ee, b + ANG_ACCEL),
        twist_peak: quad(ee, b + TWIST_PEAK),
        twist: quad(ee, b + TWIST),
    }
}

/// A body with every field the resolution reads or writes generated: positions near the contacts,
/// speeds that close on them both gently and hard, and damping both sides of what a contact lowers
/// it to.
fn write_body(ee: &mut Ee, p: &mut Pick, b: u32) {
    use body_at::*;
    ee.set_f32(b + MASS, p.around(46.5838));
    ee.set_f32(b + INV_MASS, if p.one_in(8) { 0.0 } else { p.around(0.0214667) });
    for (at, s) in [
        (POSITION, 20.0),
        (VELOCITY, 30.0),
        (FORCE, 1000.0),
        (SHOCK, 10.0),
        (MOMENT, 1000.0),
        (RATES, 3.0),
        (TWIST, 10.0),
    ] {
        let v = p.loose(s);
        set_quad(ee, b + at, v);
    }
    let iw = p.range(-2.0, 2.0);
    let ii = p.vector(0.05, iw);
    set_quad(ee, b + INV_INERTIA, ii);
    for at in [DAMPING, ANG_DAMPING] {
        let d = if p.one_in(3) { Vf::new(1.0, 1.0, 1.0, 1.0) } else { Vf([0; 4].map(|_| p.range(0.0, 2.0))) };
        set_quad(ee, b + at, d);
    }
    let q = p.quat();
    set_quad(ee, b + QUAT, q);
    let c = if p.one_in(4) { Vf::new(0.0, 3.5, -2.5, 1.0) } else { p.loose(4.0) };
    set_quad(ee, b + COFG, c);
}

fn check_body(case: usize, what: &str, ee: &Ee, b: u32, got: &Body) {
    let want = body_of(ee, b);
    assert_eq!(got.enabled, want.enabled, "case {case}: {what}: enabled");
    assert_eq!(got.inv_mass.to_bits(), want.inv_mass.to_bits(), "case {case}: {what}: the inverse mass");
    for (name, g, w) in [
        ("the damping", got.damping, want.damping),
        ("the last position", got.last_position, want.last_position),
        ("the position", got.position, want.position),
        ("the velocity", got.velocity, want.velocity),
        ("the last force", got.last_force, want.last_force),
        ("the force", got.force, want.force),
        ("the acceleration", got.accel, want.accel),
        ("the shock peak", got.shock_peak, want.shock_peak),
        ("the shock", got.shock, want.shock),
        ("the step", got.step, want.step),
        ("the inverse inertia", got.inv_inertia, want.inv_inertia),
        ("the angular damping", got.ang_damping, want.ang_damping),
        ("the last orientation", got.last_quat, want.last_quat),
        ("the orientation", got.quat, want.quat),
        ("the centre of mass", got.cofg, want.cofg),
        ("the moment", got.moment, want.moment),
        ("the last moment", got.last_moment, want.last_moment),
        ("the rates", got.rates, want.rates),
        ("the angular acceleration", got.ang_accel, want.ang_accel),
        ("the twist peak", got.twist_peak, want.twist_peak),
        ("the twist", got.twist, want.twist),
    ] {
        same_quad(case, &format!("{what}: {name}"), g, w);
    }
}

fn contact_of(ee: &Ee, e: u32, others: &[u32], mover: u32) -> Contact {
    use entry::*;
    let other = ee.u32_at(e + OTHER);
    let m = ee.u32_at(e + MOVER);
    Contact {
        kind: ee.u32_at(e + KIND),
        on_body: quad(ee, e + ON_BODY),
        normal: quad(ee, e + NORMAL),
        point: quad(ee, e + POINT),
        arm: quad(ee, e + ARM),
        other_arm: quad(ee, e + OTHER_ARM),
        surface: ee.u32_at(e + SURFACE) as i32,
        other: (other != 0).then(|| others.iter().position(|&o| o == other).unwrap()),
        mover: (m != 0).then(|| {
            assert_eq!(m, mover);
            0
        }),
    }
}

// --- the layout ------------------------------------------------------------------------------------

/// Where one other collision object and everything it owns sit in scratch memory.
#[derive(Clone, Copy)]
struct OtherAt {
    record: u32,
    collidable: u32,
    body: u32,
    vehicle: u32,
    node: u32,
}

struct Layout {
    list: u32,
    collidable: u32,
    others: [OtherAt; 2],
    mover: u32,
    mover_node: u32,
    mover_vtable: u32,
    spin_hook: u32,
    velocity_hook: u32,
}

fn layout(ee: &Ee) -> Layout {
    // Slots of 0x40: a list of up to four entries, the plane's collidable, then per other a record
    // (0x200), a collidable, a body (0x200), a chassis (0x2a0) and a transform (0x120); then the mover.
    let other = |k: u32| {
        let s = 11 + 35 * k;
        OtherAt {
            record: ee.scratch(s),
            collidable: ee.scratch(s + 8),
            body: ee.scratch(s + 11),
            vehicle: ee.scratch(s + 19),
            node: ee.scratch(s + 30),
        }
    };
    Layout {
        list: ee.scratch(0),
        collidable: ee.scratch(8),
        others: [other(0), other(1)],
        mover: ee.scratch(81),
        mover_node: ee.scratch(92),
        mover_vtable: ee.scratch(97),
        spin_hook: ee.scratch(110),
        velocity_hook: ee.scratch(111),
    }
}

/// The two virtuals of the mover: each writes what the test stashed in the mover to its out pointer.
fn hook_mover(ee: &mut Ee, l: &Layout) {
    ee.hooks.insert(l.spin_hook, Box::new(|ee| {
        let (out, m) = (ee.r(4) as u32, ee.r(5) as u32);
        let v = quad(ee, m + mov::SPIN);
        set_quad(ee, out, v);
    }));
    ee.hooks.insert(l.velocity_hook, Box::new(|ee| {
        let (out, m) = (ee.r(4) as u32, ee.r(5) as u32);
        let v = quad(ee, m + mov::VELOCITY);
        set_quad(ee, out, v);
    }));
}

/// Counts of what the cases exercised, so a generator that stops reaching a path shows.
#[derive(Default, Debug)]
struct Seen {
    contacts: usize,
    hard: usize,
    impulses: usize,
    springs: usize,
    mutual: usize,
    actors: usize,
    vehicle_impulses: usize,
    vehicles: usize,
    bodies: usize,
    movers: usize,
    theirs: usize,
    damped: usize,
}

// --- the proof -------------------------------------------------------------------------------------

/// The plane's collision object resolves its contacts as the original's does: every contact kind and
/// every kind of other, mutual and not, actors and scenery, hard and gentle, with and without impulses.
#[test]
fn the_contact_resolution_matches_the_original() {
    let Some(mut ee) = plane() else { return };
    let record = ee.u32_at(PLANE + PLANE_COLLISION);
    let plane_body = ee.u32_at(PLANE + PLANE_BODY);
    assert_eq!(ee.u32_at(record + rec::BODY), plane_body, "the plane's collision object is not on its body");
    let resolve_at = ee.u32_at(ee.u32_at(record + rec::VTABLE) + rec::RESOLVE);
    assert_eq!(resolve_at, addr("collision_resolve").unwrap(), "the collision object's slot is not the resolution");
    assert_eq!(ee.u32_at(plane_body + body_at::LISTENERS), 0, "the plane's body has listeners");
    let l = layout(&ee);
    hook_mover(&mut ee, &l);
    // The others' bodies are copies of the plane's (its vtable, no listeners).
    let template: Vec<u32> = (0..body_at::SIZE).step_by(4).map(|k| ee.u32_at(plane_body + k)).collect();
    let mut p = Pick(0x5eed_c0_11_1de5);
    let mut seen = Seen::default();
    for case in 0..CASES {
        // The plane's own object and body.
        write_collider(&mut ee, &mut p, record);
        write_body(&mut ee, &mut p, plane_body);
        let own_collidable = !p.one_in(4);
        if own_collidable {
            let f = p.flags();
            zero(&mut ee, l.collidable, 0xc0);
            ee.set_u32(l.collidable + rec::FLAGS, f);
        }
        ee.set_u32(record + rec::COLLIDABLE, if own_collidable { l.collidable } else { 0 });

        // Two others.
        let mut on_vehicle = [false; 2];
        for (k, o) in l.others.iter().enumerate() {
            zero(&mut ee, o.record, 0x200);
            write_collider(&mut ee, &mut p, o.record);
            let has = !p.one_in(4);
            if has {
                let f = p.flags();
                zero(&mut ee, o.collidable, 0xc0);
                ee.set_u32(o.collidable + rec::FLAGS, f);
            }
            ee.set_u32(o.record + rec::COLLIDABLE, if has { o.collidable } else { 0 });
            on_vehicle[k] = p.one_in(3);
            if on_vehicle[k] {
                write_vehicle(&mut ee, &mut p, o.vehicle);
                let n = p.node();
                write_node(&mut ee, o.node, &n);
                ee.set_u32(o.record + rec::VEHICLE, o.vehicle);
                ee.set_u32(o.record + rec::BODY, 0);
                ee.set_u32(o.record + rec::NODE, o.node);
            } else {
                for (j, w) in template.iter().enumerate() {
                    ee.set_u32(o.body + 4 * j as u32, *w);
                }
                write_body(&mut ee, &mut p, o.body);
                ee.set_u32(o.record + rec::VEHICLE, 0);
                ee.set_u32(o.record + rec::BODY, o.body);
                ee.set_u32(o.record + rec::NODE, 0);
            }
        }

        // The mover.
        zero(&mut ee, l.mover, 0x260);
        ee.set_u32(l.mover + mov::VTABLE, l.mover_vtable);
        ee.set_u32(l.mover_vtable + mov::SPIN_SLOT, l.spin_hook);
        ee.set_u32(l.mover_vtable + mov::VELOCITY_SLOT, l.velocity_hook);
        let n = p.node();
        write_node(&mut ee, l.mover_node, &n);
        ee.set_u32(l.mover + mov::NODE, l.mover_node);
        for (at, s) in [(mov::ORIGIN, 3.0), (mov::SPIN, 3.0), (mov::VELOCITY, 30.0)] {
            let v = p.loose(s);
            set_quad(&mut ee, l.mover + at, v);
        }

        // The contacts, around the plane's body.
        let count = match p.below(10) {
            0 => 0,
            1..=5 => 1,
            6 | 7 => 2,
            _ => 4,
        };
        ee.set_u32(record + rec::COUNT, count);
        ee.set_u32(record + rec::LIST, l.list);
        let centre = quad(&ee, plane_body + body_at::POSITION);
        for k in 0..count {
            use entry::*;
            let e = l.list + SIZE * k;
            zero(&mut ee, e, SIZE);
            let other = match p.below(3) {
                0 => None,
                k => Some(k as usize - 1),
            };
            let kind = if other.is_some() && p.one_in(4) { 1 } else { 0 };
            let mover = other.is_none() && p.one_in(3) || p.one_in(8);
            ee.set_u32(e + KIND, kind);
            for (at, s) in [(FROM, 5.0), (ON_BODY, 5.0), (ARM, 5.0), (OTHER_ARM, 5.0)] {
                let v = p.loose(s);
                set_quad(&mut ee, e + at, v);
            }
            let nv = p.normal();
            set_quad(&mut ee, e + NORMAL, nv);
            let off = p.loose(8.0);
            let point = Vf::new(centre.x() + off.x(), centre.y() + off.y(), centre.z() + off.z(), off.w());
            set_quad(&mut ee, e + POINT, point);
            ee.set_u32(e + SURFACE, (p.below(17) as i32 - 1) as u32);
            ee.set_u32(e + OTHER, other.map_or(0, |k| l.others[k].record));
            ee.set_u32(e + MOVER, if mover { l.mover } else { 0 });
        }
        let impulses = !p.one_in(4);

        // The port, on the same inputs.
        let mut own = collider_of(&ee, record);
        let mut body = body_of(&ee, plane_body);
        let mass = F(ee.f32_at(plane_body + body_at::MASS));
        let other_records = l.others.map(|o| o.record);
        let mut contacts: Vec<Contact> =
            (0..count).map(|k| contact_of(&ee, l.list + entry::SIZE * k, &other_records, l.mover)).collect();
        let mut colliders = l.others.map(|o| collider_of(&ee, o.record));
        let mut bodies = l.others.map(|o| body_of(&ee, o.body));
        let mut vehicles = l.others.map(|o| vehicle_of(&ee, o.vehicle, o.node));
        let masses = l.others.map(|o| F(ee.f32_at(o.body + body_at::MASS)));
        let movers = [Mover {
            spin: quad(&ee, l.mover + mov::SPIN),
            velocity: quad(&ee, l.mover + mov::VELOCITY),
            node: node_of(&ee, l.mover_node),
            origin: quad(&ee, l.mover + mov::ORIGIN),
        }];
        // What the generated case reaches, read the way the original decides it.
        for c in &contacts {
            seen.contacts += 1;
            seen.movers += usize::from(c.mover.is_some());
            if let Some(k) = c.other {
                if on_vehicle[k] { seen.vehicles += 1 } else { seen.bodies += 1 }
                seen.theirs += usize::from(c.kind != 0);
            }
        }
        {
            let [c0, c1] = &mut colliders;
            let [b0, b1] = &mut bodies;
            let [v0, v1] = &mut vehicles;
            let mut w0 = Weighed { body: b0, mass: masses[0] };
            let mut w1 = Weighed { body: b1, mass: masses[1] };
            let side0 = if on_vehicle[0] { Side::Vehicle(v0) } else { Side::Body(&mut w0 as &mut dyn Rigid) };
            let side1 = if on_vehicle[1] { Side::Vehicle(v1) } else { Side::Body(&mut w1 as &mut dyn Rigid) };
            let mut others = [Other { collider: c0, side: side0 }, Other { collider: c1, side: side1 }];
            let mut me = Weighed { body: &mut body, mass };
            resolve(&mut own, &mut me, &mut contacts, &mut others, &movers, impulses);
        }

        // The original.
        ee.call(resolve_at, &[u64::from(record), u64::from(impulses)], &[], BUDGET);

        same_quad(case, "the object's centre in the world", own.cofg_world, quad(&ee, record + rec::COFG_WORLD));
        same_quad(case, "the relative velocity", own.relative, quad(&ee, record + rec::RELATIVE));
        assert_eq!(own.normal_speed.to_bits(), ee.f32_at(record + rec::NORMAL_SPEED).to_bits(), "case {case}: the normal speed");
        for (k, c) in contacts.iter().enumerate() {
            let e = l.list + entry::SIZE * k as u32;
            same_quad(case, &format!("contact {k}'s arm"), c.arm, quad(&ee, e + entry::ARM));
            same_quad(case, &format!("contact {k}'s other arm"), c.other_arm, quad(&ee, e + entry::OTHER_ARM));
        }
        check_body(case, "the plane's body", &ee, plane_body, &body);
        for (k, o) in l.others.iter().enumerate() {
            same_quad(case, &format!("other {k}'s centre in the world"), colliders[k].cofg_world, quad(&ee, o.record + rec::COFG_WORLD));
            if on_vehicle[k] {
                let want = vehicle_of(&ee, o.vehicle, o.node);
                let got = vehicles[k];
                for (name, g, w) in [
                    ("velocity", got.velocity, want.velocity),
                    ("rates", got.rates, want.rates),
                    ("shock", got.shock, want.shock),
                    ("twist", got.twist, want.twist),
                ] {
                    same_quad(case, &format!("other {k}'s chassis {name}"), g, w);
                }
            } else {
                check_body(case, &format!("other {k}'s body"), &ee, o.body, &bodies[k]);
            }
        }
        // Which paths ran, from what the original left: with one contact, its normal speed decides.
        if count == 1 {
            let c = &contacts[0];
            let hard = ee.f32_at(record + rec::NORMAL_SPEED) < -10.0;
            seen.hard += usize::from(hard);
            seen.impulses += usize::from(hard && impulses && c.kind == 0);
            seen.springs += usize::from(!hard);
            if let Some(k) = c.other {
                let mine = ee.u32_at(record + rec::COLLIDABLE);
                let theirs = ee.u32_at(l.others[k].record + rec::COLLIDABLE);
                let flags = |c: u32| ee.u32_at(c + rec::FLAGS);
                let shared = theirs != 0
                    && if mine != 0 {
                        flags(theirs) & own.collides_with != 0 && flags(mine) & colliders[k].collides_with != 0
                    } else {
                        flags(theirs) & own.collides_with != 0
                    };
                seen.mutual += usize::from(shared);
                seen.actors += usize::from(theirs != 0 && flags(theirs) & own.actors != 0);
                seen.vehicle_impulses += usize::from(on_vehicle[k] && hard && impulses && c.kind == 0);
            }
        }
        seen.damped += usize::from(own.damp && count > 0);
    }
    eprintln!("{seen:?}");
    let Seen { contacts, hard, impulses, springs, mutual, actors, vehicle_impulses, vehicles, bodies, movers, theirs, damped } = seen;
    for (what, n) in [
        ("contacts", contacts),
        ("hard contacts", hard),
        ("impulses", impulses),
        ("springs", springs),
        ("mutual contacts", mutual),
        ("actor contacts", actors),
        ("impulses on a chassis", vehicle_impulses),
        ("contacts with a chassis", vehicles),
        ("contacts with a body", bodies),
        ("contacts with a mover", movers),
        ("contacts of the other's kind", theirs),
        ("damped cases", damped),
    ] {
        assert!(n >= 100, "only {n} {what}: the generator stopped reaching a path");
    }
}
