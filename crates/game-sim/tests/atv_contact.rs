//! MD75e1 — the quad's collision record resolving its contacts, against SLUS-21104's.
//!
//! The quad's own record in MD37a's `atv-r-spawn.ee.bin` is handed a generated contact list, its
//! tuning, the quad's own body (or, in the second test, a generated chassis in its place), and up to
//! two other records (each on a body or on a chassis) and a mover, all written into the original's RAM
//! (the others in scratch memory); then the record's vt `+0x88` (`atv_record_resolve`) runs, and
//! everything it writes is compared bit for bit with [`atv_contact::resolve`]'s: every word of every
//! body, the record's and the others' written fields, each entry's arms and relative velocity, the
//! chassis' accumulators. Every word the original wrote outside its stack is one of those.
//!
//! The mover's two virtuals are the only hooks: they report a spin and a velocity stashed in the
//! mover. The bodies' virtuals run as the original's. What is not proved is in the journal entry
//! (`research/journal/2026-11-04-md75e1-record-contacts/1-record-contacts-FINAL.md`).

use game_sim::atv_body;
use game_sim::atv_contact::{self, Entry, Record, Weighed};
use game_sim::contact::{Collider, Contact, Mover, Node, Other, Own, Response, Ridden, Rigid, Side, Vehicle};
use game_sim::handling::seed;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::collections::BTreeSet;

/// The quad's record in the image (`research/oracle.txt`, MD75d), and SLUS-21104's `$gp`.
const RECORD: u32 = 0x0162_86d0;
const GP: u64 = 0x003d_baf0;
const CASES: usize = 10_000;
const BUDGET: u64 = 400_000;

/// The record's fields.
mod rec {
    pub const VTABLE: u32 = 0x04;
    pub const DAMP: u32 = 0x44;
    pub const DAMP_LINEAR: u32 = 0x4c;
    pub const DAMP_ANGULAR: u32 = 0x54;
    pub const MASS: u32 = 0x58;
    pub const INV_MASS: u32 = 0x5c;
    pub const INV_INERTIA: u32 = 0x70;
    pub const COFG: u32 = 0x80;
    pub const COFG_WORLD: u32 = 0x90;
    pub const VEHICLE: u32 = 0xe0;
    pub const BODY: u32 = 0xe4;
    pub const NODE: u32 = 0xe8;
    pub const COUNT: u32 = 0xf0;
    pub const LIST: u32 = 0xf8;
    pub const ACTORS: u32 = 0x100;
    pub const COLLIDES: u32 = 0x104;
    pub const MAX_DEPTH: u32 = 0x10c;
    /// Stiffness, damping, restitution: the scenery's and the actors'.
    pub const SCENERY: u32 = 0x118;
    pub const ACTOR: u32 = 0x130;
    pub const ANGULAR: u32 = 0x13c;
    pub const ACTOR_ANGULAR: u32 = 0x140;
    pub const IMPULSE_MASK: u32 = 0x150;
    pub const SPRING_MASK: u32 = 0x160;
    pub const FRICTION_MASK: u32 = 0x170;
    pub const ACTOR_SPRING_MASK: u32 = 0x180;
    pub const ACTOR_FRICTION_MASK: u32 = 0x190;
    pub const FRICTION: u32 = 0x1a0;
    pub const ACTOR_FRICTION: u32 = 0x1a4;
    pub const SURFACES: u32 = 0x1a8;
    pub const NORMAL_SPEED: u32 = 0x1e8;
    pub const RELATIVE: u32 = 0x1f0;
    pub const COLLIDABLE: u32 = 0x208;
    pub const SIZE: u32 = 0x210;
    /// The collidable's flags.
    pub const FLAGS: u32 = 0x94;
    /// The resolution's vtable slot.
    pub const RESOLVE: u32 = 0x88;
}

/// A contact list entry's fields.
mod entry {
    pub const SIZE: u32 = 0x90;
    pub const KIND: u32 = 0x00;
    pub const FROM: u32 = 0x10;
    pub const ON_BODY: u32 = 0x20;
    pub const NORMAL: u32 = 0x30;
    pub const POINT: u32 = 0x40;
    pub const ARM: u32 = 0x50;
    pub const OTHER_ARM: u32 = 0x60;
    pub const RELATIVE: u32 = 0x70;
    pub const SURFACE: u32 = 0x80;
    pub const OTHER: u32 = 0x84;
    pub const MOVER: u32 = 0x88;
}

/// The rigid body's fields (`research/oracle.txt` MD75) the cases generate, and its size.
mod body_at {
    pub const MASS: u32 = 0x18;
    pub const INV_MASS: u32 = 0x1c;
    pub const DAMPING: u32 = 0x30;
    pub const POSITION: u32 = 0x70;
    pub const VELOCITY: u32 = 0x80;
    pub const FORCE: u32 = 0xa0;
    pub const CORRECTION: u32 = 0xd0;
    pub const LISTENERS: u32 = 0xe8;
    pub const INV_INERTIA: u32 = 0x140;
    pub const ANG_DAMPING: u32 = 0x150;
    pub const QUAT: u32 = 0x190;
    pub const COFG: u32 = 0x1a0;
    pub const MOMENT: u32 = 0x1b0;
    pub const RATES: u32 = 0x1d0;
    pub const TWIST: u32 = 0x200;
    pub const SIZE: u32 = 0x230;
}

/// A chassis' fields, and its scene node's (rows, origin, the up-to-date byte).
mod veh {
    pub const INV_INERTIA: [u32; 3] = [0xe0, 0xf4, 0x108];
    pub const SHOCK: u32 = 0x160;
    pub const TWIST: u32 = 0x170;
    pub const COFG: u32 = 0x180;
    pub const MASS: u32 = 0x200;
    pub const INV_MASS: u32 = 0x204;
    pub const VELOCITY: u32 = 0x250;
    pub const RATES: u32 = 0x280;
    pub const SIZE: u32 = 0x290;
    /// The own side's: its centre in the world, and the force and moment the substep gathers.
    pub const POSITION: u32 = 0x50;
    pub const FORCE: u32 = 0x120;
    pub const TORQUE: u32 = 0x130;
    pub const NODE_ROWS: u32 = 0x70;
    pub const NODE_ORIGIN: u32 = 0xa0;
    pub const NODE_CURRENT: u32 = 0x100;
    pub const NODE_SIZE: u32 = 0x110;
}

/// A mover's fields: its vtable, the two slots the resolution calls, its node and origin, and two
/// quadwords nothing reads, where the test stashes what the hooked slots report.
mod mov {
    pub const VTABLE: u32 = 0x04;
    pub const VELOCITY_SLOT: u32 = 0xc0;
    pub const SPIN_SLOT: u32 = 0xc4;
    pub const ORIGIN: u32 = 0x180;
    pub const NODE: u32 = 0x244;
    pub const SPIN: u32 = 0x1c0;
    pub const VELOCITY: u32 = 0x1d0;
    pub const SIZE: u32 = 0x250;
}

/// The interpreter on the image's RAM, with SLUS-21104's `$gp`, or why not.
fn image() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2/md37a").join(format!("atv-r-spawn.ee{}", ".bin"));
    if !path.exists() {
        skip(&format!(
            "{} is not there: it is MD37a's (eeMemory.bin of atv-r-spawn.p2s; memory: md37a ATV states live in s2)",
            path.display()
        ));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, GP);
    Some(ee)
}

/// A deterministic generator for the cases (tests/contact.rs's).
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
    fn loose(&mut self, scale: f32) -> Vf {
        let w = self.range(-2.0, 2.0);
        self.vector(scale, w)
    }
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
    fn flags(&mut self) -> u32 {
        (self.next() % 16) as u32
    }
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

/// The record as [`atv_contact`] reads it.
fn record_of(ee: &Ee, r: u32) -> Record {
    use rec::*;
    let c = ee.u32_at(r + COLLIDABLE);
    let collider = Collider {
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
    };
    Record { collider, impulse_mask: quad(ee, r + IMPULSE_MASK) }
}

/// A record's tuning around the quad's values, written over `r`'s fields.
fn write_record(ee: &mut Ee, p: &mut Pick, r: u32) {
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
    ee.set_f32(r + MASS, p.around(13.0435));
    ee.set_f32(r + INV_MASS, if p.one_in(8) { 0.0 } else { p.around(0.0767) });
    let iw = p.range(-2.0, 2.0);
    let ii = p.vector(0.1, iw);
    set_quad(ee, r + INV_INERTIA, ii);
    let c = if p.one_in(4) { Vf::new(0.0, 0.0, 0.0, 1.0) } else { p.loose(4.0) };
    set_quad(ee, r + COFG, c);
    let junk = p.loose(50.0);
    set_quad(ee, r + COFG_WORLD, junk);
    ee.set_u32(r + ACTORS, p.flags());
    ee.set_u32(r + COLLIDES, if p.one_in(4) { 0xffff_ffff } else { p.flags() });
    ee.set_f32(r + MAX_DEPTH, p.around(0.5));
    for at in [SCENERY, ACTOR] {
        ee.set_f32(r + at, p.around(1500.0));
        ee.set_f32(r + at + 4, p.around(200.0));
        ee.set_f32(r + at + 8, if p.one_in(3) { 0.0 } else { p.range(0.0, 1.0) });
    }
    ee.set_u32(r + ANGULAR, u32::from(!p.one_in(4)));
    ee.set_u32(r + ACTOR_ANGULAR, u32::from(!p.one_in(4)));
    for at in [IMPULSE_MASK, SPRING_MASK, FRICTION_MASK, ACTOR_SPRING_MASK, ACTOR_FRICTION_MASK] {
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
    zero(ee, n, NODE_SIZE);
    for (k, r) in node.rows.iter().enumerate() {
        set_quad(ee, n + NODE_ROWS + 16 * k as u32, *r);
    }
    set_quad(ee, n + NODE_ORIGIN, node.origin);
    // Up to date: the node does not rebuild its world frame from its parents.
    ee.set_u32(n + NODE_CURRENT, 1);
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
    zero(ee, v, SIZE);
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

fn body_of(ee: &Ee, b: u32) -> atv_body::Body {
    seed::body(&|a| ee.u32_at(a), b)
}

/// A body with every field the resolution reads or writes generated, over a copy of the quad's.
fn write_body(ee: &mut Ee, p: &mut Pick, b: u32) {
    use body_at::*;
    ee.set_f32(b + MASS, p.around(13.0435));
    ee.set_f32(b + INV_MASS, if p.one_in(8) { 0.0 } else { p.around(0.0767) });
    for (at, s) in [
        (POSITION, 20.0),
        (VELOCITY, 30.0),
        (FORCE, 1000.0),
        (CORRECTION, 10.0),
        (MOMENT, 1000.0),
        (RATES, 3.0),
        (TWIST, 10.0),
    ] {
        let v = p.loose(s);
        set_quad(ee, b + at, v);
    }
    let iw = p.range(-2.0, 2.0);
    let ii = p.vector(0.1, iw);
    set_quad(ee, b + INV_INERTIA, ii);
    for at in [DAMPING, ANG_DAMPING] {
        let d = if p.one_in(3) { Vf::new(1.0, 1.0, 1.0, 1.0) } else { Vf([0; 4].map(|_| p.range(0.0, 2.0))) };
        set_quad(ee, b + at, d);
    }
    let q = p.quat();
    set_quad(ee, b + QUAT, q);
    let c = if p.one_in(4) { Vf::new(0.0, 0.5, -0.5, 1.0) } else { p.loose(4.0) };
    set_quad(ee, b + COFG, c);
}

/// Every word of the body [`seed::body`] reads, and its mass and centre of mass, as the original left
/// them. Returns the addresses compared.
fn check_body(case: usize, what: &str, ee: &Ee, b: u32, got: &atv_body::Body, mass: F, cofg: Vf, read: &mut BTreeSet<u32>) {
    for (a, v) in seed::body_words(got, b) {
        assert_eq!(v, ee.u32_at(a), "case {case}: {what} +{:#x}: port {v:#010x}, original {:#010x}", a - b, ee.u32_at(a));
        read.insert(a);
    }
    assert_eq!(mass.to_bits(), ee.u32_at(b + body_at::MASS), "case {case}: {what}: the mass");
    same_quad(case, &format!("{what}: the centre of mass"), cofg, quad(ee, b + body_at::COFG));
}

fn entry_of(ee: &Ee, e: u32, others: &[u32], mover: u32) -> Entry {
    use entry::*;
    let other = ee.u32_at(e + OTHER);
    let m = ee.u32_at(e + MOVER);
    let contact = Contact {
        kind: ee.u32_at(e + KIND),
        from: quad(ee, e + FROM),
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
    };
    Entry { contact, relative: quad(ee, e + RELATIVE) }
}

// --- the layout ------------------------------------------------------------------------------------

/// Where one other record and everything it owns sit in scratch memory.
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
    chassis: u32,
    node: u32,
}

fn layout(ee: &Ee) -> Layout {
    // Slots of 0x40: a list of up to four entries (0x240), the quad's collidable (0xc0), then per
    // other a record (0x240), a collidable, a body (0x240), a chassis (0x2c0) and a node (0x140); then
    // the mover (0x280), its node, its vtable (0x100), the two hooks; the own chassis and node.
    let other = |k: u32| {
        let s = 12 + 37 * k;
        OtherAt {
            record: ee.scratch(s),
            collidable: ee.scratch(s + 9),
            body: ee.scratch(s + 12),
            vehicle: ee.scratch(s + 21),
            node: ee.scratch(s + 32),
        }
    };
    Layout {
        list: ee.scratch(0),
        collidable: ee.scratch(9),
        others: [other(0), other(1)],
        mover: ee.scratch(86),
        mover_node: ee.scratch(96),
        mover_vtable: ee.scratch(101),
        spin_hook: ee.scratch(105),
        velocity_hook: ee.scratch(106),
        chassis: ee.scratch(107),
        node: ee.scratch(118),
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
    masked: usize,
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

/// The quad's record resolves its contacts as SLUS-21104's does, on the quad's body (`on_chassis`
/// false) or on a chassis in the body's place.
fn run(on_chassis: bool, seed: u64) {
    let Some(mut ee) = image() else { return };
    let record = RECORD;
    let quad_body = ee.u32_at(record + rec::BODY);
    let resolve_at = ee.u32_at(ee.u32_at(record + rec::VTABLE) + rec::RESOLVE);
    assert_eq!(resolve_at, addr("atv_record_resolve").unwrap(), "the record's slot is not the resolution");
    assert_eq!(ee.u32_at(quad_body + body_at::LISTENERS), 0, "the quad's body has listeners");
    let l = layout(&ee);
    hook_mover(&mut ee, &l);
    // The others' bodies are copies of the quad's (its vtable, no listeners).
    let template: Vec<u32> = (0..body_at::SIZE).step_by(4).map(|k| ee.u32_at(quad_body + k)).collect();
    let mut p = Pick(seed);
    let mut seen = Seen::default();
    for case in 0..CASES {
        // The quad's own record, and its body or a chassis.
        write_record(&mut ee, &mut p, record);
        let centre = if on_chassis {
            write_vehicle(&mut ee, &mut p, l.chassis);
            for (at, s) in [(veh::POSITION, 100.0), (veh::FORCE, 1000.0), (veh::TORQUE, 1000.0)] {
                let v = p.loose(s);
                set_quad(&mut ee, l.chassis + at, v);
            }
            let n = p.node();
            write_node(&mut ee, l.node, &n);
            ee.set_u32(record + rec::VEHICLE, l.chassis);
            ee.set_u32(record + rec::BODY, 0);
            ee.set_u32(record + rec::NODE, l.node);
            n.origin
        } else {
            write_body(&mut ee, &mut p, quad_body);
            ee.set_u32(record + rec::VEHICLE, 0);
            ee.set_u32(record + rec::BODY, quad_body);
            quad(&ee, quad_body + body_at::POSITION)
        };
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
            zero(&mut ee, o.record, rec::SIZE);
            write_record(&mut ee, &mut p, o.record);
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
        zero(&mut ee, l.mover, mov::SIZE);
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

        // The contacts, around the own side.
        let count = match p.below(10) {
            0 => 0,
            1..=5 => 1,
            6 | 7 => 2,
            _ => 4,
        };
        ee.set_u32(record + rec::COUNT, count);
        ee.set_u32(record + rec::LIST, l.list);
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
            for (at, s) in [(FROM, 5.0), (ON_BODY, 5.0), (ARM, 5.0), (OTHER_ARM, 5.0), (RELATIVE, 30.0)] {
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
        let mut own = record_of(&ee, record);
        let mut body = body_of(&ee, quad_body);
        let (mass, cofg) = (F(ee.f32_at(quad_body + body_at::MASS)), quad(&ee, quad_body + body_at::COFG));
        let mut ridden = Ridden {
            chassis: vehicle_of(&ee, l.chassis, l.node),
            position: quad(&ee, l.chassis + veh::POSITION),
            force: quad(&ee, l.chassis + veh::FORCE),
            torque: quad(&ee, l.chassis + veh::TORQUE),
        };
        let other_records = l.others.map(|o| o.record);
        let mut entries: Vec<Entry> = (0..count).map(|k| entry_of(&ee, l.list + entry::SIZE * k, &other_records, l.mover)).collect();
        let mut records = l.others.map(|o| record_of(&ee, o.record).collider);
        let mut bodies = l.others.map(|o| body_of(&ee, o.body));
        let mut vehicles = l.others.map(|o| vehicle_of(&ee, o.vehicle, o.node));
        let weights = l.others.map(|o| (F(ee.f32_at(o.body + body_at::MASS)), quad(&ee, o.body + body_at::COFG)));
        let movers = [Mover {
            spin: quad(&ee, l.mover + mov::SPIN),
            velocity: quad(&ee, l.mover + mov::VELOCITY),
            node: node_of(&ee, l.mover_node),
            origin: quad(&ee, l.mover + mov::ORIGIN),
        }];
        for e in &entries {
            let c = &e.contact;
            seen.contacts += 1;
            seen.movers += usize::from(c.mover.is_some());
            if let Some(k) = c.other {
                if on_vehicle[k] { seen.vehicles += 1 } else { seen.bodies += 1 }
                seen.theirs += usize::from(c.kind != 0);
            }
        }
        {
            let [c0, c1] = &mut records;
            let [b0, b1] = &mut bodies;
            let [v0, v1] = &mut vehicles;
            let mut w0 = Weighed { body: b0, mass: weights[0].0, cofg: weights[0].1 };
            let mut w1 = Weighed { body: b1, mass: weights[1].0, cofg: weights[1].1 };
            let side0 = if on_vehicle[0] { Side::Vehicle(v0) } else { Side::Body(&mut w0 as &mut dyn Rigid) };
            let side1 = if on_vehicle[1] { Side::Vehicle(v1) } else { Side::Body(&mut w1 as &mut dyn Rigid) };
            let mut others = [Other { collider: c0, side: side0 }, Other { collider: c1, side: side1 }];
            let mut me = Weighed { body: &mut body, mass, cofg };
            let mut side = if on_chassis { Own::Vehicle(&mut ridden) } else { Own::Body(&mut me) };
            atv_contact::resolve(&mut own, &mut side, &mut entries, &mut others, &movers, impulses);
        }

        // The original.
        ee.written.clear();
        ee.call(resolve_at, &[u64::from(record), u64::from(impulses)], &[], BUDGET);

        // What the port accounts for, compared; then nothing else may have been written.
        let mut read = BTreeSet::new();
        let mut quads = |what: &str, got: Vf, at: u32, read: &mut BTreeSet<u32>| {
            same_quad(case, what, got, quad(&ee, at));
            read.extend((0..16).map(|k| at + k));
        };
        let own_c = &own.collider;
        quads("the record's centre in the world", own_c.cofg_world, record + rec::COFG_WORLD, &mut read);
        quads("the relative velocity", own_c.relative, record + rec::RELATIVE, &mut read);
        assert_eq!(own_c.normal_speed.to_bits(), ee.u32_at(record + rec::NORMAL_SPEED), "case {case}: the normal speed");
        read.extend((0..4).map(|k| record + rec::NORMAL_SPEED + k));
        for (k, e) in entries.iter().enumerate() {
            let at = l.list + entry::SIZE * k as u32;
            quads(&format!("entry {k}'s arm"), e.contact.arm, at + entry::ARM, &mut read);
            quads(&format!("entry {k}'s other arm"), e.contact.other_arm, at + entry::OTHER_ARM, &mut read);
            quads(&format!("entry {k}'s relative velocity"), e.relative, at + entry::RELATIVE, &mut read);
        }
        if on_chassis {
            let want = vehicle_of(&ee, l.chassis, l.node);
            let got = ridden.chassis;
            for (name, g, at) in [
                ("velocity", got.velocity, veh::VELOCITY),
                ("rates", got.rates, veh::RATES),
                ("shock", got.shock, veh::SHOCK),
                ("twist", got.twist, veh::TWIST),
                ("force", ridden.force, veh::FORCE),
                ("torque", ridden.torque, veh::TORQUE),
            ] {
                quads(&format!("the own chassis' {name}"), g, l.chassis + at, &mut read);
            }
            let _ = want;
        } else {
            let mut words = BTreeSet::new();
            check_body(case, "the quad's body", &ee, quad_body, &body, mass, cofg, &mut words);
            read.extend(words.iter().flat_map(|&a| a..a + 4));
        }
        for (k, o) in l.others.iter().enumerate() {
            quads(&format!("other {k}'s centre in the world"), records[k].cofg_world, o.record + rec::COFG_WORLD, &mut read);
            if on_vehicle[k] {
                let got = vehicles[k];
                for (name, g, at) in [
                    ("velocity", got.velocity, veh::VELOCITY),
                    ("rates", got.rates, veh::RATES),
                    ("shock", got.shock, veh::SHOCK),
                    ("twist", got.twist, veh::TWIST),
                ] {
                    quads(&format!("other {k}'s chassis {name}"), g, o.vehicle + at, &mut read);
                }
            } else {
                let mut words = BTreeSet::new();
                check_body(case, &format!("other {k}'s body"), &ee, o.body, &bodies[k], weights[k].0, weights[k].1, &mut words);
                read.extend(words.iter().flat_map(|&a| a..a + 4));
            }
        }
        let stray: Vec<u32> = ee.written.iter().copied().filter(|&a| !(ee.stack - 0x4000..ee.stack).contains(&a) && !read.contains(&a)).collect();
        assert!(stray.is_empty(), "case {case}: the original wrote what the port does not: {:x?}", &stray[..stray.len().min(16)]);

        // Which paths ran, from what the original left: with one contact, its normal speed decides.
        if count == 1 {
            let c = &entries[0].contact;
            let hard = ee.f32_at(record + rec::NORMAL_SPEED) < -10.0;
            seen.hard += usize::from(hard);
            let impulse = hard && impulses && c.kind == 0;
            seen.impulses += usize::from(impulse);
            let mask = own.impulse_mask;
            seen.masked += usize::from(impulse && !on_chassis && own_c.scenery.angular && (0..3).any(|k| mask[k] != 1.0));
            seen.springs += usize::from(!hard);
            if let Some(k) = c.other {
                let mine = ee.u32_at(record + rec::COLLIDABLE);
                let theirs = ee.u32_at(l.others[k].record + rec::COLLIDABLE);
                let flags = |c: u32| ee.u32_at(c + rec::FLAGS);
                let shared = theirs != 0
                    && if mine != 0 {
                        flags(theirs) & own_c.collides_with != 0 && flags(mine) & records[k].collides_with != 0
                    } else {
                        flags(theirs) & own_c.collides_with != 0
                    };
                seen.mutual += usize::from(shared);
                seen.actors += usize::from(theirs != 0 && flags(theirs) & own_c.actors != 0);
                seen.vehicle_impulses += usize::from(on_vehicle[k] && impulse);
            }
        }
        seen.damped += usize::from(own_c.damp && count > 0 && !on_chassis);
    }
    eprintln!("{seen:?}");
    let Seen { contacts, hard, impulses, masked, springs, mutual, actors, vehicle_impulses, vehicles, bodies, movers, theirs, damped } = seen;
    let mut paths = vec![
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
    ];
    if !on_chassis {
        paths.extend([("masked impulse turns", masked), ("damped cases", damped)]);
    }
    for (what, n) in paths {
        assert!(n >= 100, "only {n} {what}: the generator stopped reaching a path");
    }
}

/// **The quad's record resolves its contacts as the original's**, on the quad's body: every contact
/// kind and every kind of other, mutual and not, actors and scenery, hard and gentle, with and without
/// impulses, the impulse turn masked.
#[test]
fn the_quads_record_resolves_its_contacts_as_the_originals() {
    run(false, 0x0075_e1c0_11_1de5);
}

/// The same with the record on a chassis (its body pointer null), as a vehicle's record without a
/// dynamics body has it.
#[test]
fn the_record_on_a_chassis_resolves_as_the_originals() {
    run(true, 0x0075_e1b1_4e_b1_4e);
}
