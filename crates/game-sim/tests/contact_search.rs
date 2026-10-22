//! S43 — the collision object's contact search against the original.
//!
//! Two proofs, both on the save state beside C7's recording:
//!
//! - the heightfield query, on the state's own terrain (the track the plane flies over): generated
//!   points in and around it, both cell widths and both ways of splitting the cells, the original's
//!   query against [`ground`] with the original's own cell reader behind the port's [`Heightfield`];
//! - the whole search of the plane's collision object, through its vtable (the clear, then the
//!   collide): generated rays, poses, transforms, tuning and flags, on and off its body, with the
//!   world searches' results and two other collision objects, three struck objects and two movers
//!   written into the original's RAM (the others in scratch memory). Everything the search writes is
//!   compared: every flag of the three objects, their lists entry by entry, the last struck object,
//!   the sweep's segment, the body, and the transform's position.
//!
//! The hooks are the world searches, the sweep probe's segment setter and the probes' "moved"
//! refresh, which only log their calls (the test writes what the searches would have found before the
//! call), and the movers' velocity, which reports what the test stashed. The heightfield query, the
//! cell reader, the ground result, the transform and the body's damping entries run as the
//! original's. What is not proved, and why, is in the journal entry's last section.

use game_sim::biplane::Body;
use game_sim::contact::{Contact, Node};
use game_sim::contact_search::{
    Cell, Heightfield, Hit, MOVER_KIND, Record, Search, Struck, Swept, clear, collide, ground,
};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::cell::RefCell;
use std::rc::Rc;

const STATE: &str = "biplane.p2m2_SaveState.eeMemory.bin";
const PLANE: u32 = 0x00c4_4a40;
const PLANE_COLLISION: u32 = 0x170;
const CASES: usize = 10_000;
const BUDGET: u64 = 2_000_000;

/// The collision object's fields.
mod rec {
    pub const VTABLE: u32 = 0x08;
    pub const SEARCH_GROUND: u32 = 0x30;
    pub const SEARCH: u32 = 0x34;
    pub const SWEEP_BACK: u32 = 0x3c;
    pub const TOUCHING: u32 = 0x40;
    pub const TOUCHING_OTHER: u32 = 0x44;
    pub const ON_GROUND: u32 = 0x48;
    pub const ON_OBJECT: u32 = 0x4c;
    pub const DAMP: u32 = 0x50;
    pub const REST_LINEAR: u32 = 0x54;
    pub const REST_ANGULAR: u32 = 0x5c;
    pub const OFFSET: u32 = 0xb0;
    pub const LEAD: u32 = 0xc0;
    pub const PUSH_OUT: u32 = 0xc4;
    pub const FACING: u32 = 0xc8;
    pub const SWEPT_FROM: u32 = 0xd0;
    pub const SWEPT_TO: u32 = 0xe0;
    pub const VEHICLE: u32 = 0xf0;
    pub const BODY: u32 = 0xf4;
    pub const NODE: u32 = 0xf8;
    pub const TERRAIN: u32 = 0xfc;
    pub const COUNT: u32 = 0x100;
    pub const CAPACITY: u32 = 0x104;
    pub const LIST: u32 = 0x108;
    pub const NEARBY: u32 = 0x10c;
    pub const RAY_PROBE: u32 = 0x1f0;
    pub const SWEEP_PROBE: u32 = 0x1f4;
    pub const STRUCK: u32 = 0x1fc;
    pub const SIZE: u32 = 0x200;
    /// The vtable slots: the clear and the collide.
    pub const CLEAR: u32 = 0x74;
    pub const COLLIDE: u32 = 0x7c;
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

/// A probe's fields, and its result record's.
mod probe {
    pub const RESULT: u32 = 0x6c;
    pub const HIT_FLAGS: u32 = 0x116;
    pub const HIT: u8 = 0x20;
    pub const RAY_COUNT: u32 = 0x168;
    pub const RAYS: u32 = 0x170;
    /// The result record: the push-out direction and point, the hit count and the hits' rays,
    /// normals, points and objects.
    pub const PUSH: u32 = 0x20;
    pub const POINT: u32 = 0x30;
    pub const HITS: u32 = 0x8c;
    pub const HIT_RAYS: u32 = 0x90;
    pub const HIT_NORMALS: u32 = 0x94;
    pub const HIT_POINTS: u32 = 0x98;
    pub const HIT_OBJECTS: u32 = 0x9c;
}

/// A struck object's fields.
mod obj {
    pub const MOVER: u32 = 0x7c;
    pub const RECORD: u32 = 0x80;
}

/// A mover's fields, and the quadword nothing reads where the test stashes its velocity.
mod mov {
    pub const VTABLE: u32 = 0x08;
    pub const VELOCITY_SLOT: u32 = 0xb8;
    pub const KIND: u32 = 0x1b0;
    pub const VELOCITY: u32 = 0x1c0;
}

/// The body's and the chassis' fields.
mod body_at {
    pub const DAMPING: u32 = 0x20;
    pub const POSITION: u32 = 0x60;
    pub const VELOCITY: u32 = 0x70;
    pub const ANG_DAMPING: u32 = 0x130;
    pub const QUAT: u32 = 0x170;
    pub const CHASSIS_VELOCITY: u32 = 0x260;
}

/// A transform's fields: the local rows and position, the world ones, and the flags.
mod node_at {
    pub const LOCAL: u32 = 0x40;
    pub const POSITION: u32 = 0x70;
    pub const WORLD: u32 = 0x80;
    pub const ORIGIN: u32 = 0xb0;
    pub const FLAGS: u32 = 0x110;
}

/// The terrain object's fields.
mod ter {
    pub const MIN: u32 = 0x30;
    pub const MAX: u32 = 0x40;
    pub const HEIGHT_SCALE: u32 = 0x80;
    pub const TREE: u32 = 0x84;
    pub const SPLIT: u32 = 0x760;
    pub const PER_SAMPLE: u32 = 0x778;
    pub const COARSE: u32 = 0x8a8;
    pub const CACHE_READY: u32 = 0x9d0;
}

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
            1 => Vf::new(1.0, 0.0, 0.0, 0.0),
            _ => {
                let q = [0; 4].map(|_| self.range(-1.0, 1.0));
                let n = q.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-3) * self.range(0.999, 1.001);
                Vf(q.map(|x| x / n))
            }
        }
    }
    /// A rotation's rows, scaled now and then, with any `w`s; the origin is the caller's.
    fn rows(&mut self) -> [Vf; 3] {
        let q = self.quat();
        let (x, y, z, w) = (q.x(), q.y(), q.z(), q.w());
        let s = if self.one_in(4) { self.range(0.5, 2.0) } else { 1.0 };
        let mut w0 = || if self.one_in(2) { 0.0 } else { self.range(-1.0, 1.0) };
        [
            Vf::new(s * (1.0 - 2.0 * (y * y + z * z)), s * 2.0 * (x * y + z * w), s * 2.0 * (x * z - y * w), w0()),
            Vf::new(s * 2.0 * (x * y - z * w), s * (1.0 - 2.0 * (x * x + z * z)), s * 2.0 * (y * z + x * w), w0()),
            Vf::new(s * 2.0 * (x * z + y * w), s * 2.0 * (y * z - x * w), s * (1.0 - 2.0 * (x * x + y * y)), w0()),
        ]
    }
    fn flag(&mut self, one_in: u64) -> u32 {
        u32::from(!self.one_in(one_in))
    }
}

/// A quadword as its bits: what the original stores is what is compared, denormals and all.
fn quad(ee: &Ee, a: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| f32::from_bits(ee.u32_at(a + k))))
}

fn set_quad(ee: &mut Ee, a: u32, v: Vf) {
    for k in 0..4 {
        ee.set_u32(a + 4 * k as u32, v[k].to_bits());
    }
}

fn single(ee: &Ee, a: u32) -> F {
    F(f32::from_bits(ee.u32_at(a)))
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

// --- the terrain -----------------------------------------------------------------------------------

/// The state's terrain object, its own cell reader behind the port's trait.
struct EeField<'a> {
    ee: &'a RefCell<&'a mut Ee>,
    terrain: u32,
    reader: u32,
    out: u32,
}

impl Heightfield for EeField<'_> {
    fn min(&self) -> Vf {
        quad(&self.ee.borrow(), self.terrain + ter::MIN)
    }
    fn max(&self) -> Vf {
        quad(&self.ee.borrow(), self.terrain + ter::MAX)
    }
    fn per_sample(&self) -> F {
        single(&self.ee.borrow(), self.terrain + ter::PER_SAMPLE)
    }
    fn height_scale(&self) -> F {
        single(&self.ee.borrow(), self.terrain + ter::HEIGHT_SCALE)
    }
    fn coarse(&self) -> bool {
        self.ee.borrow().u32_at(self.terrain + ter::COARSE) != 0
    }
    fn split_one_way(&self) -> bool {
        self.ee.borrow().u32_at(self.terrain + ter::SPLIT) == 1
    }
    fn cell(&self, ix: i32, iz: i32, step: i32) -> Option<Cell> {
        let mut ee = self.ee.borrow_mut();
        let tree = ee.u32_at(self.terrain + ter::TREE);
        let o = self.out;
        zero(&mut ee, o, 0x80);
        let args = [tree, ix as u32, iz as u32, o + 0x40, o, o + 0x5c, step as u32].map(u64::from);
        ee.call(self.reader, &args, &[], BUDGET);
        if ee.v0() as u32 == 0 {
            return None;
        }
        Some(Cell {
            heights: quad(&ee, o + 0x40),
            normals: [0, 1, 2, 3].map(|k| quad(&ee, o + 0x10 * k)),
            surfaces: ee.u32_at(o + 0x5c).to_le_bytes(),
        })
    }
}

/// The heightfield query: the point with the ground's height, the normal, the surfaces and the
/// weights, as the original leaves its buffer — over the whole terrain and past its edges, a sample
/// apart and two, with the cells split one way and by parity.
#[test]
fn the_heightfield_query_matches_the_original() {
    let Some(mut ee) = plane() else { return };
    let record = ee.u32_at(PLANE + PLANE_COLLISION);
    let terrain = ee.u32_at(record + rec::TERRAIN);
    assert_ne!(terrain, 0, "the plane's collision object has no terrain");
    let query = addr("terrain_point_query").unwrap();
    let reader = addr("terrain_cell_read").unwrap();
    let (buf, out) = (ee.scratch(0), ee.scratch(4));
    let max = quad(&ee, terrain + ter::MAX);
    let mut p = Pick(0x7e44_a1_5eed);
    let (mut coarse, mut misses, mut cells, mut splits) = (0usize, 0usize, 0usize, [0usize; 2]);
    let mut was_coarse = ee.u32_at(terrain + ter::COARSE);
    for case in 0..CASES {
        // A cell width; the original's cell cache is keyed by the sample alone, so it is emptied when
        // the width changes, as a terrain never does under it.
        let c = u32::from(p.one_in(3));
        ee.set_u32(terrain + ter::COARSE, c);
        if c != was_coarse {
            ee.set_u32(terrain + ter::CACHE_READY, 0);
            was_coarse = c;
        }
        let split = if p.one_in(2) { 1 } else { p.below(3) as u32 * 2 };
        ee.set_u32(terrain + ter::SPLIT, split);
        let x = if p.one_in(16) { p.range(-300.0, 0.0) } else if p.one_in(16) { p.range(max.x(), max.x() + 300.0) } else { p.range(0.0, max.x()) };
        let z = if p.one_in(16) { p.range(-300.0, 0.0) } else if p.one_in(16) { p.range(max.z(), max.z() + 300.0) } else { p.range(0.0, max.z()) };
        let at = Vf::new(x, p.range(-50.0, 500.0), z, p.range(-2.0, 2.0));
        set_quad(&mut ee, buf, at);
        let junk = p.loose(3.0);
        set_quad(&mut ee, buf + 0x10, junk);
        ee.set_u32(buf + 0x20, p.next() as u32);
        set_quad(&mut ee, buf + 0x30, Vf::splat(1.0));

        let got = {
            let shared = RefCell::new(&mut ee);
            let field = EeField { ee: &shared, terrain, reader, out };
            ground(&field, at)
        };
        coarse += c as usize;
        ee.call(query, &[u64::from(terrain), u64::from(buf)], &[], BUDGET);

        same_quad(case, "the point", got.point, quad(&ee, buf));
        same_quad(case, "the normal", got.normal, quad(&ee, buf + 0x10));
        assert_eq!(u32::from_le_bytes(got.surfaces), ee.u32_at(buf + 0x20), "case {case}: the surfaces");
        same_quad(case, "the weights", got.weights, quad(&ee, buf + 0x30));
        if got.point.y() == at.y() && got.normal == Vf::new(0.0, 1.0, 0.0, 1.0) {
            misses += 1;
        } else {
            cells += 1;
            splits[usize::from(split == 1)] += 1;
        }
    }
    println!("heightfield query: {CASES} cases, {cells} on a cell ({} split by parity, {} one way), {misses} \
              off the terrain, {coarse} two samples wide", splits[0], splits[1]);
    assert!(cells > 5_000 && misses > 100 && coarse > 1_000 && splits.iter().all(|&n| n > 1_000));
}

// --- the search ------------------------------------------------------------------------------------

/// Which probe a logged call was on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Probe {
    Rays,
    Sweep,
}

/// A call into the world the search made, in order.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Call {
    Search { probe: Probe, nearby: bool },
    Segment { from: Vf, to: Vf },
    Moved(Probe),
}

/// The port's world: what the test wrote into the original's probes, and the log.
struct Fake {
    hits: Vec<Hit>,
    swept: Option<Swept>,
    /// Each record's body (or chassis) velocity, by index, and each mover's.
    velocities: [Vf; 3],
    movers: [Vf; 2],
    log: Vec<Call>,
}

impl Search for Fake {
    fn rays(&mut self, nearby: bool) -> Vec<Hit> {
        self.log.push(Call::Search { probe: Probe::Rays, nearby });
        self.hits.clone()
    }
    fn sweep(&mut self, from: Vf, to: Vf, nearby: bool) -> Option<Swept> {
        self.log.push(Call::Segment { from, to });
        self.log.push(Call::Moved(Probe::Sweep));
        self.log.push(Call::Search { probe: Probe::Sweep, nearby });
        self.swept
    }
    fn moved(&mut self, _node: &game_sim::contact::Node) {
        self.log.push(Call::Moved(Probe::Rays));
    }
    fn velocity(&self, struck: &Struck) -> Vf {
        match struck.other {
            Some(o) => self.velocities[o],
            None => self.movers[struck.mover.expect("a struck object with neither").0],
        }
    }
}

/// Where everything the test makes sits in scratch memory (slots of 0x40).
struct Layout {
    others: [(u32, u32, u32, u32); 2],
    objects: [u32; 3],
    movers: [u32; 2],
    mover_vtable: u32,
    velocity_hook: u32,
    rays: u32,
    probe_buf: u32,
    cell_out: u32,
}

fn layout(ee: &Ee) -> Layout {
    // Per other: a record (8 slots), a list of eight entries (16), a body (8), a chassis (11).
    let other = |k: u32| {
        let s = 8 + 48 * k;
        (ee.scratch(s), ee.scratch(s + 8), ee.scratch(s + 24), ee.scratch(s + 32))
    };
    Layout {
        others: [other(0), other(1)],
        objects: [0, 1, 2].map(|j| ee.scratch(104 + 4 * j)),
        movers: [0, 1].map(|m| ee.scratch(116 + 8 * m)),
        mover_vtable: ee.scratch(132),
        velocity_hook: ee.scratch(136),
        rays: ee.scratch(140),
        probe_buf: ee.scratch(0),
        cell_out: ee.scratch(4),
    }
}

/// The index of `a` among `all`, or `None` for null.
fn index_of(a: u32, all: &[u32]) -> Option<usize> {
    (a != 0).then(|| all.iter().position(|&x| x == a).unwrap_or_else(|| panic!("{a:#x} is not one of {all:x?}")))
}

fn contact_of(ee: &Ee, e: u32, records: &[u32], movers: &[u32]) -> Contact {
    use entry::*;
    Contact {
        kind: ee.u32_at(e + KIND),
        from: quad(ee, e + FROM),
        on_body: quad(ee, e + ON_BODY),
        normal: quad(ee, e + NORMAL),
        point: quad(ee, e + POINT),
        arm: quad(ee, e + ARM),
        other_arm: quad(ee, e + OTHER_ARM),
        surface: ee.u32_at(e + SURFACE) as i32,
        other: index_of(ee.u32_at(e + OTHER), records),
        mover: index_of(ee.u32_at(e + MOVER), movers),
    }
}

fn node_of(ee: &Ee, n: u32) -> Node {
    use node_at::*;
    if n == 0 {
        return Node::default();
    }
    Node { rows: [0, 1, 2].map(|k| quad(ee, n + WORLD + 16 * k)), origin: quad(ee, n + ORIGIN) }
}

fn record_of(ee: &Ee, r: u32, records: &[u32], movers: &[u32], objects: &[u32]) -> Record {
    use rec::*;
    let on = |at: u32| ee.u32_at(r + at) != 0;
    let list = ee.u32_at(r + LIST);
    let rays = match ee.u32_at(r + RAY_PROBE) {
        0 => None,
        p => {
            let (n, at) = (ee.u32_at(p + probe::RAY_COUNT), ee.u32_at(p + probe::RAYS));
            Some((0..n).map(|i| [quad(ee, at + 0x20 * i), quad(ee, at + 0x20 * i + 0x10)]).collect())
        }
    };
    Record {
        search_ground: on(SEARCH_GROUND),
        search: on(SEARCH),
        sweep_back: on(SWEEP_BACK),
        touching: on(TOUCHING),
        touching_other: on(TOUCHING_OTHER),
        on_ground: on(ON_GROUND),
        on_object: on(ON_OBJECT),
        damp: on(DAMP),
        rest_linear: single(ee, r + REST_LINEAR),
        rest_angular: single(ee, r + REST_ANGULAR),
        offset: quad(ee, r + OFFSET),
        lead: single(ee, r + LEAD),
        push_out: single(ee, r + PUSH_OUT),
        facing: single(ee, r + FACING),
        swept_from: quad(ee, r + SWEPT_FROM),
        swept_to: quad(ee, r + SWEPT_TO),
        node: node_of(ee, ee.u32_at(r + NODE)),
        contacts: (0..ee.u32_at(r + COUNT)).map(|k| contact_of(ee, list + entry::SIZE * k, records, movers)).collect(),
        capacity: ee.u32_at(r + CAPACITY) as usize,
        nearby: on(NEARBY),
        rays,
        sweeps: on(SWEEP_PROBE),
        struck: index_of(ee.u32_at(r + STRUCK), objects),
    }
}

fn struck_of(ee: &Ee, o: u32, l: &Layout, records: &[u32]) -> Struck {
    let m = ee.u32_at(o + obj::MOVER);
    Struck {
        object: index_of(o, &l.objects).unwrap(),
        other: index_of(ee.u32_at(o + obj::RECORD), records),
        mover: index_of(m, &l.movers).map(|k| (k, ee.u32_at(m + mov::KIND))),
    }
}

/// Counts of what the cases exercised, so a generator that stops reaching a path shows.
#[derive(Default, Debug)]
struct Seen {
    ground: usize,
    objects: usize,
    others: usize,
    movers: usize,
    full: usize,
    swept_back: usize,
    flying_apart: usize,
    no_body: usize,
    no_rays: usize,
    damped: usize,
}

/// The plane's collision object empties its list and searches as the original's does: the sweep back
/// (moving the body or refusing to), the terrain under every downward ray, and the world's hits with
/// their contacts handed back to other collision objects, with lists that fill up.
#[test]
fn the_contact_search_matches_the_original() {
    let Some(mut ee) = plane() else { return };
    let record = ee.u32_at(PLANE + PLANE_COLLISION);
    let vtable = ee.u32_at(record + rec::VTABLE);
    let (clear_at, collide_at) = (ee.u32_at(vtable + rec::CLEAR), ee.u32_at(vtable + rec::COLLIDE));
    assert_eq!(clear_at, addr("collision_record_clear").unwrap(), "the object's slot is not the clear");
    assert_eq!(collide_at, addr("collision_record_collide").unwrap(), "the object's slot is not the collide");
    let plane_body = ee.u32_at(record + rec::BODY);
    let node = ee.u32_at(record + rec::NODE);
    let terrain = ee.u32_at(record + rec::TERRAIN);
    let (rays_probe, sweep_probe) = (ee.u32_at(record + rec::RAY_PROBE), ee.u32_at(record + rec::SWEEP_PROBE));
    let nearby = ee.u32_at(record + rec::NEARBY);
    assert!(plane_body != 0 && node != 0 && terrain != 0 && rays_probe != 0 && sweep_probe != 0 && nearby != 0);
    assert_eq!(ee.u32_at(node + 0x100), 0, "the plane's transform has a parent");
    let node_flags = ee.u32_at(node + node_at::FLAGS);
    let (ray_result, sweep_result) = (ee.u32_at(rays_probe + probe::RESULT), ee.u32_at(sweep_probe + probe::RESULT));
    let sweep_flags = ee.u8_at(sweep_probe + probe::HIT_FLAGS);
    let l = layout(&ee);
    let records = [record, l.others[0].0, l.others[1].0];

    // The hooks: the world searches, the segment setter and the refresh log; the movers report.
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let which = move |p: u32| if p == rays_probe { Probe::Rays } else { assert_eq!(p, sweep_probe); Probe::Sweep };
    for (name, nearby) in [("collide_against_search", true), ("probe_collide_world", false)] {
        let log = log.clone();
        ee.hooks.insert(addr(name).unwrap(), Box::new(move |ee| {
            let probe = which(ee.r(4) as u32);
            log.borrow_mut().push(Call::Search { probe, nearby });
        }));
    }
    {
        let log = log.clone();
        ee.hooks.insert(addr("probe_bundle_set_points").unwrap(), Box::new(move |ee| {
            assert_eq!(which(ee.r(4) as u32), Probe::Sweep);
            let (at, n) = (ee.r(5) as u32, ee.r(6));
            assert_eq!(n, 0, "the segment is not handed in as a pair");
            log.borrow_mut().push(Call::Segment { from: quad(ee, at), to: quad(ee, at + 0x10) });
        }));
    }
    {
        let log = log.clone();
        ee.hooks.insert(addr("probe_object_moved").unwrap(), Box::new(move |ee| {
            let probe = which(ee.r(4) as u32);
            log.borrow_mut().push(Call::Moved(probe));
        }));
    }
    ee.hooks.insert(l.velocity_hook, Box::new(|ee| {
        let (out, m) = (ee.r(4) as u32, ee.r(5) as u32);
        let v = quad(ee, m + mov::VELOCITY);
        set_quad(ee, out, v);
    }));
    let query = addr("terrain_point_query").unwrap();
    let reader = addr("terrain_cell_read").unwrap();
    let max = quad(&ee, terrain + ter::MAX);

    let mut p = Pick(0x5eed_5ea2_c4ed);
    let mut seen = Seen::default();
    for case in 0..CASES {
        // Where: over the terrain (now and then past its edge), near its surface.
        let edge = p.one_in(10);
        let x = if edge { p.range(-60.0, 0.0) } else { p.range(30.0, max.x() - 30.0) };
        let z = if edge && p.one_in(2) { p.range(max.z(), max.z() + 60.0) } else { p.range(30.0, max.z() - 30.0) };
        set_quad(&mut ee, l.probe_buf, Vf::new(x, 0.0, z, 1.0));
        set_quad(&mut ee, l.probe_buf + 0x30, Vf::splat(1.0));
        ee.call(query, &[u64::from(terrain), u64::from(l.probe_buf)], &[], BUDGET);
        let y = ee.f32_at(l.probe_buf + 4) + p.range(-12.0, 12.0);
        let origin = Vf::new(x, y, z, if p.one_in(3) { p.range(-2.0, 2.0) } else { 1.0 });

        // The transform, local and world alike (it has no parent).
        let rows = p.rows();
        for (k, r) in rows.iter().enumerate() {
            set_quad(&mut ee, node + node_at::LOCAL + 16 * k as u32, *r);
            set_quad(&mut ee, node + node_at::WORLD + 16 * k as u32, *r);
        }
        set_quad(&mut ee, node + node_at::POSITION, origin);
        set_quad(&mut ee, node + node_at::ORIGIN, origin);
        ee.set_u32(node + node_at::FLAGS, node_flags);

        // The body.
        let with_body = !p.one_in(6);
        let off = p.loose(2.0);
        let pos = Vf::new(origin.x() + off.x(), origin.y() + off.y(), origin.z() + off.z(), off.w());
        set_quad(&mut ee, plane_body + body_at::POSITION, pos);
        let vel = if p.one_in(6) { p.loose(0.8) } else { p.loose(40.0) };
        set_quad(&mut ee, plane_body + body_at::VELOCITY, vel);
        let q = p.quat();
        set_quad(&mut ee, plane_body + body_at::QUAT, q);
        for at in [body_at::DAMPING, body_at::ANG_DAMPING] {
            let d = p.loose(2.0);
            set_quad(&mut ee, plane_body + at, d);
        }

        // The plane's object.
        {
            use rec::*;
            let r = record;
            ee.set_u32(r + BODY, if with_body { plane_body } else { 0 });
            ee.set_u32(r + SEARCH_GROUND, p.flag(6));
            ee.set_u32(r + SEARCH, p.flag(8));
            ee.set_u32(r + SWEEP_BACK, if with_body { p.flag(4) } else { 0 });
            for at in [TOUCHING, TOUCHING_OTHER, ON_GROUND, ON_OBJECT] {
                ee.set_u32(r + at, p.below(2) as u32);
            }
            ee.set_u32(r + DAMP, p.flag(2));
            ee.set_f32(r + REST_LINEAR, p.range(0.0, 2.0));
            ee.set_f32(r + REST_ANGULAR, p.range(0.0, 2.0));
            let o = p.loose(3.0);
            set_quad(&mut ee, r + OFFSET, o);
            ee.set_f32(r + LEAD, p.around(2.0));
            ee.set_f32(r + PUSH_OUT, p.around(4.0));
            ee.set_f32(r + FACING, if p.one_in(2) { -1.0 } else { p.range(-1.0, 1.0) });
            for at in [SWEPT_FROM, SWEPT_TO] {
                let v = p.loose(100.0);
                set_quad(&mut ee, r + at, v);
            }
            ee.set_u32(r + CAPACITY, if p.one_in(3) { 0x24 } else { p.below(6) as u32 });
            let count = p.below(4) as u32;
            ee.set_u32(r + COUNT, count);
            let list = ee.u32_at(r + LIST);
            zero(&mut ee, list, entry::SIZE * count);
            ee.set_u32(r + NEARBY, if p.one_in(2) { nearby } else { 0 });
            ee.set_u32(r + RAY_PROBE, if p.one_in(10) { 0 } else { rays_probe });
            ee.set_u32(r + SWEEP_PROBE, if p.one_in(6) { 0 } else { sweep_probe });
            ee.set_u32(r + STRUCK, if p.one_in(2) { 0 } else { l.objects[p.below(3) as usize] });
        }

        // Its rays.
        let n = p.below(13) as u32;
        ee.set_u32(rays_probe + probe::RAY_COUNT, n);
        ee.set_u32(rays_probe + probe::RAYS, l.rays);
        for i in 0..n {
            let start = p.loose(3.0);
            let d = Vf::new(p.range(-6.0, 6.0), p.range(-14.0, 6.0), p.range(-6.0, 6.0), 0.0);
            let end = Vf::new(start.x() + d.x(), start.y() + d.y(), start.z() + d.z(), p.range(-2.0, 2.0));
            set_quad(&mut ee, l.rays + 0x20 * i, start);
            set_quad(&mut ee, l.rays + 0x20 * i + 0x10, end);
        }

        // Two other objects, each on a body or a chassis, with lists of their own.
        let mut velocities = [Vf::ZERO; 3];
        for (k, &(r, list, body, chassis)) in l.others.iter().enumerate() {
            use rec::*;
            zero(&mut ee, r, SIZE);
            let cap = p.below(6) as u32;
            let count = p.below(u64::from(cap) + 1) as u32;
            ee.set_u32(r + CAPACITY, cap);
            ee.set_u32(r + COUNT, count);
            ee.set_u32(r + LIST, list);
            for at in [TOUCHING, TOUCHING_OTHER, ON_GROUND, ON_OBJECT] {
                ee.set_u32(r + at, p.below(2) as u32);
            }
            for e in 0..count {
                let e = list + entry::SIZE * e;
                zero(&mut ee, e, entry::SIZE);
                for at in [entry::FROM, entry::ON_BODY, entry::NORMAL, entry::POINT] {
                    let v = p.loose(10.0);
                    set_quad(&mut ee, e + at, v);
                }
                ee.set_u32(e + entry::SURFACE, u32::MAX);
            }
            let v = if p.one_in(6) { p.loose(0.8) } else { p.loose(40.0) };
            velocities[k + 1] = v;
            if p.one_in(3) {
                zero(&mut ee, chassis, 0x2c0);
                set_quad(&mut ee, chassis + body_at::CHASSIS_VELOCITY, v);
                ee.set_u32(r + VEHICLE, chassis);
            } else {
                zero(&mut ee, body, 0x200);
                set_quad(&mut ee, body + body_at::VELOCITY, v);
                ee.set_u32(r + BODY, body);
            }
        }

        // The movers and the struck objects.
        let mut mover_velocities = [Vf::ZERO; 2];
        ee.set_u32(l.mover_vtable + mov::VELOCITY_SLOT, l.velocity_hook);
        for (m, &at) in l.movers.iter().enumerate() {
            zero(&mut ee, at, 0x200);
            ee.set_u32(at + mov::VTABLE, l.mover_vtable);
            ee.set_u32(at + mov::KIND, if m == 0 { MOVER_KIND } else { p.below(4) as u32 });
            let v = if p.one_in(6) { p.loose(0.8) } else { p.loose(40.0) };
            set_quad(&mut ee, at + mov::VELOCITY, v);
            mover_velocities[m] = v;
        }
        for &o in &l.objects {
            zero(&mut ee, o, 0x100);
            ee.set_u32(o + obj::RECORD, match p.below(3) { 0 => 0, k => records[k as usize] });
            ee.set_u32(o + obj::MOVER, match p.below(3) { 0 => 0, k => l.movers[k as usize - 1] });
        }

        // What the world searches found: the ray probe's hits, and the sweep's.
        let hits = if p.one_in(3) { 0 } else { p.below(5) as u32 };
        ee.set_u32(ray_result + probe::HITS, hits);
        let (hr, hn, hp, ho) = [probe::HIT_RAYS, probe::HIT_NORMALS, probe::HIT_POINTS, probe::HIT_OBJECTS]
            .map(|at| ee.u32_at(ray_result + at))
            .into();
        for i in 0..hits {
            for at in [hr + 0x20 * i, hr + 0x20 * i + 0x10, hp + 0x10 * i] {
                let v = p.loose(50.0);
                set_quad(&mut ee, at, v);
            }
            let nv = p.loose(1.5);
            set_quad(&mut ee, hn + 0x10 * i, nv);
            ee.set_u32(ho + 4 * i, l.objects[p.below(3) as usize]);
        }
        let swept = p.one_in(2);
        ee.write(sweep_probe + probe::HIT_FLAGS, 1, u128::from(if swept { sweep_flags | probe::HIT } else { sweep_flags & !probe::HIT }));
        let push = p.loose(1.5);
        set_quad(&mut ee, sweep_result + probe::PUSH, push);
        let at = p.loose(4.0);
        let at = Vf::new(pos.x() + at.x(), pos.y() + at.y(), pos.z() + at.z(), at.w());
        set_quad(&mut ee, sweep_result + probe::POINT, at);
        let first = ee.u32_at(sweep_result + probe::HIT_OBJECTS);
        ee.set_u32(first, l.objects[p.below(3) as usize]);
        let dt = match p.below(4) {
            0 => 1.0 / 60.0,
            1 => 1.0 / 30.0,
            _ => p.range(0.0, 0.1),
        };

        // The port, on the same inputs.
        let mut ours: Vec<Record> = records.iter().map(|&r| record_of(&ee, r, &records, &l.movers, &l.objects)).collect();
        let mut body = Body {
            position: quad(&ee, plane_body + body_at::POSITION),
            velocity: quad(&ee, plane_body + body_at::VELOCITY),
            quat: quad(&ee, plane_body + body_at::QUAT),
            damping: quad(&ee, plane_body + body_at::DAMPING),
            ang_damping: quad(&ee, plane_body + body_at::ANG_DAMPING),
            ..Body::default()
        };
        let mut world = Fake {
            hits: (0..hits)
                .map(|i| Hit {
                    from: quad(&ee, hr + 0x20 * i),
                    to: quad(&ee, hr + 0x20 * i + 0x10),
                    point: quad(&ee, hp + 0x10 * i),
                    normal: quad(&ee, hn + 0x10 * i),
                    struck: struck_of(&ee, ee.u32_at(ho + 4 * i), &l, &records),
                })
                .collect(),
            swept: swept.then(|| Swept {
                push: quad(&ee, sweep_result + probe::PUSH),
                point: quad(&ee, sweep_result + probe::POINT),
                struck: struck_of(&ee, ee.u32_at(first), &l, &records),
            }),
            velocities,
            movers: mover_velocities,
            log: Vec::new(),
        };
        let before = (ours[0].contacts.len(), body.position);
        {
            let shared = RefCell::new(&mut ee);
            let field = EeField { ee: &shared, terrain, reader, out: l.cell_out };
            clear(&mut ours[0], with_body.then_some(&mut body));
            collide(&mut ours, 0, with_body.then_some(&mut body), Some(&field), &mut world, F(dt));
        }

        // The original.
        log.borrow_mut().clear();
        ee.call(clear_at, &[u64::from(record)], &[], BUDGET);
        ee.call(collide_at, &[u64::from(record)], &[dt], BUDGET);

        assert_eq!(world.log, *log.borrow(), "case {case}: the calls into the world");
        for (k, (&r, got)) in records.iter().zip(&ours).enumerate() {
            use rec::*;
            let flag = |at: u32| ee.u32_at(r + at);
            assert_eq!(
                [got.touching, got.touching_other, got.on_ground, got.on_object].map(u32::from),
                [TOUCHING, TOUCHING_OTHER, ON_GROUND, ON_OBJECT].map(flag),
                "case {case}: object {k}'s flags"
            );
            assert_eq!(got.contacts.len() as u32, flag(COUNT), "case {case}: object {k}'s count");
            let list = ee.u32_at(r + LIST);
            for (j, c) in got.contacts.iter().enumerate() {
                let want = contact_of(&ee, list + entry::SIZE * j as u32, &records, &l.movers);
                let what = format!("object {k}'s contact {j}");
                assert_eq!((c.kind, c.surface, c.other, c.mover), (want.kind, want.surface, want.other, want.mover),
                           "case {case}: {what}: kind, surface, other, mover");
                for (name, g, w) in [
                    ("from", c.from, want.from),
                    ("to", c.on_body, want.on_body),
                    ("normal", c.normal, want.normal),
                    ("point", c.point, want.point),
                    ("arm", c.arm, want.arm),
                    ("other arm", c.other_arm, want.other_arm),
                ] {
                    same_quad(case, &format!("{what}: {name}"), g, w);
                }
                if k == 0 {
                    if c.surface >= 0 { seen.ground += 1 } else { seen.objects += 1 }
                    seen.movers += usize::from(c.mover.is_some());
                } else {
                    seen.others += usize::from(c.kind == 1);
                }
            }
        }
        let got = &ours[0];
        assert_eq!(got.struck, index_of(ee.u32_at(record + rec::STRUCK), &l.objects), "case {case}: the last struck object");
        same_quad(case, "the sweep's start", got.swept_from, quad(&ee, record + rec::SWEPT_FROM));
        same_quad(case, "the sweep's end", got.swept_to, quad(&ee, record + rec::SWEPT_TO));
        same_quad(case, "the transform's position", got.node.origin, quad(&ee, node + node_at::POSITION));
        same_quad(case, "the body's position", body.position, quad(&ee, plane_body + body_at::POSITION));
        same_quad(case, "the body's damping", body.damping, quad(&ee, plane_body + body_at::DAMPING));
        same_quad(case, "the body's angular damping", body.ang_damping, quad(&ee, plane_body + body_at::ANG_DAMPING));

        seen.full += usize::from(got.contacts.len() == got.capacity && got.capacity < 0x24);
        let swept_now = world.log.contains(&Call::Search { probe: Probe::Sweep, nearby: got.nearby });
        if swept_now && swept {
            if body.position != before.1 { seen.swept_back += 1 } else { seen.flying_apart += 1 }
        }
        seen.no_body += usize::from(!with_body);
        seen.no_rays += usize::from(got.rays.is_none());
        seen.damped += usize::from(with_body && got.damp);
        let _ = before.0;
    }
    println!("contact search: {CASES} cases, {seen:?}");
    assert!(seen.ground > 1_000 && seen.objects > 1_000 && seen.others > 500 && seen.movers > 300);
    assert!(seen.full > 500 && seen.swept_back > 500 && seen.flying_apart > 100);
    assert!(seen.no_body > 500 && seen.no_rays > 300 && seen.damped > 1_000);
}
