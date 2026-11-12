//! MD118 — **the quad record's outside in RAM in Rust**: what the record's part search, sweep and
//! their place leave in RAM, written by the ports the bike's record glue drives
//! (`tests/mxvatv_bike_ride.rs`' F46a/F46c/F46d/F51/F54, the same record class in the same program),
//! copied here as they are: the spatial database move ([`spatial_move_in_ram`]), the sweep's set,
//! place and search ([`sweep_in_ram`]), the part search with its plant parts stood and its listeners
//! heard ([`part_search_in_ram`]).

#![allow(dead_code)]

use super::*;
use game_sim::atv_sphere;
use game_sim::camera::CurveTable;
use game_sim::collide::MeshRef;
use game_sim::sweep_probe::{Placed, Shape, SweepProbe, Target, Transform};

#[path = "../mxvatv_bike_ride/gather_in_ram.rs"]
mod gather_in_ram;
use gather_in_ram::*;

fn set_vf(ee: &mut Ee, a: u32, v: Vf) {
    put_v(ee, a, v);
}

// --- F46: the record's searches and the sync's database half in Rust ----------------------------------

use game_sim::atv_spatial::{self, Cell as DbCell, Database, Extra, Item, Link, LinkAt, List as DbList, Place, Pool, Slot, SlotAt, Sub};

/// The global holding the database's link pool, and the list sub-database's vtable (MD75c1's).
const DB_POOL_AT: u32 = 0x0040_c460;
const DB_LIST_VT: u32 = 0x003b_5510;

/// Where the database [`DbInRam::read`] read is in RAM: its cells', portals' and sub-databases'
/// addresses (in the model's order), each list's blocks, the entries by item, the link pool's
/// chunks and stride.
struct DbInRam {
    cells: Vec<u32>,
    portals: Vec<u32>,
    subs: Vec<u32>,
    blocks: Vec<Vec<u32>>,
    items: Vec<u32>,
    chunks: Vec<u32>,
    stride: u32,
    capacity: u32,
}

impl DbInRam {
    fn index(list: &[u32], a: u32, what: &str) -> usize {
        list.iter().position(|&x| x == a).unwrap_or_else(|| panic!("{what} {a:#x} is none of the database's"))
    }

    fn item(&mut self, a: u32) -> Item {
        self.items.iter().position(|&x| x == a).unwrap_or_else(|| {
            self.items.push(a);
            self.items.len() - 1
        })
    }

    fn slot_at(&self, sub: usize, a: u32, size: u32) -> SlotAt {
        for (b, &base) in self.blocks[sub].iter().enumerate() {
            if (base..base + 4 * size).contains(&a) {
                assert_eq!((a - base) % 4, 0, "a slot {a:#x} off its block's words");
                return SlotAt { block: b, index: ((a - base) / 4) as usize };
            }
        }
        panic!("slot {a:#x} is in none of sub-database {sub}'s blocks");
    }

    fn slot_addr(&self, sub: usize, s: SlotAt) -> u32 {
        self.blocks[sub][s.block] + 4 * s.index as u32
    }

    fn link_at(&self, a: u32) -> LinkAt {
        for (c, &base) in self.chunks.iter().enumerate() {
            let first = base + 4;
            if (first..first + self.capacity * self.stride).contains(&a) {
                assert_eq!((a - first) % self.stride, 0, "a link {a:#x} off its chunk's stride");
                return LinkAt { chunk: c, index: ((a - first) / self.stride) as usize };
            }
        }
        panic!("link {a:#x} is in none of the pool's chunks");
    }

    fn place(&self, ee: &Ee, sub: usize, a: u32) -> Place {
        assert_eq!(ee.u32_at(self.subs[sub] + 4), DB_LIST_VT, "a place {a:#x} in sub-database {:#x}, not a list", self.subs[sub]);
        Place::Slot(self.slot_at(sub, a, ee.u32_at(self.subs[sub] + 0x28)))
    }

    /// The link pool at `p`, as the move takes and gives its links.
    fn pool(&mut self, ee: &Ee, p: u32) -> Pool {
        let mut c = ee.u32_at(p + 0xc);
        while c != 0 {
            self.chunks.push(c);
            assert!(self.chunks.len() < 64, "the pool's chunk chain does not end");
            c = ee.u32_at(c);
        }
        (self.capacity, self.stride) = (ee.u32_at(p + 8), ee.u32_at(p + 0x1c));
        let mut free = vec![];
        let mut f = ee.u32_at(p + 0x24);
        while f != 0 {
            free.push(self.link_at(f));
            assert!(free.len() < 4096, "the pool's free chain does not end");
            f = ee.u32_at(f);
        }
        free.reverse();
        let mut spare = 0;
        let mut s = ee.u32_at(p + 0x14);
        while s != 0 {
            spare += 1;
            assert!(spare < 4096, "the pool's set-aside chain does not end");
            s = ee.u32_at(s);
        }
        Pool { capacity: ee.i32_at(p + 8), stride: self.stride, used: ee.i32_at(p + 0x18), chunks: self.chunks.len(), spare, free }
    }

    /// The database `entry` (a collidable's `+0x18`) is in, as [`atv_spatial`] models it, and the
    /// entry's record (its `+8`); the entry is item 0. The cells are the record's and those its links
    /// reach; their lists are read slot by slot, any other sub-database (the quadtree the gather
    /// reads) by its kind alone, which a move into it would refuse.
    fn read(ee: &Ee, entry: u32) -> (DbInRam, Database, atv_spatial::Record) {
        let mut m = DbInRam { cells: vec![], portals: vec![], subs: vec![], blocks: vec![], items: vec![entry], chunks: vec![], stride: 0, capacity: 0 };
        let pool = m.pool(ee, ee.u32_at(DB_POOL_AT));
        let rec = ee.u32_at(entry + 8);
        let home = ee.u32_at(rec + 4);
        let mut db = Database { pool, ..Default::default() };
        if home == 0 {
            return (m, db, atv_spatial::Record { last: vf_at(ee, rec + 0x10), ..Default::default() });
        }
        m.cells.push(home);
        let mut k = 0;
        while k < m.cells.len() {
            let c = m.cells[k];
            let mut l = ee.u32_at(c + 0x24);
            while l != 0 {
                let (to, portal) = (ee.u32_at(l + 4), ee.u32_at(l + 8));
                if !m.cells.contains(&to) {
                    m.cells.push(to);
                }
                if !m.portals.contains(&portal) {
                    m.portals.push(portal);
                }
                l = ee.u32_at(l);
            }
            for o in [0x28, 0x2c] {
                let s = ee.u32_at(c + o);
                if s != 0 && !m.subs.contains(&s) {
                    m.subs.push(s);
                }
            }
            k += 1;
        }
        for &c in &m.cells {
            let mut links = vec![];
            let mut l = ee.u32_at(c + 0x24);
            while l != 0 {
                links.push(Link { cell: DbInRam::index(&m.cells, ee.u32_at(l + 4), "a link's cell"), portal: DbInRam::index(&m.portals, ee.u32_at(l + 8), "a link's portal") });
                l = ee.u32_at(l);
            }
            let sub = |o: u32| match ee.u32_at(c + o) {
                0 => None,
                s => Some(DbInRam::index(&m.subs, s, "a cell's sub-database")),
            };
            db.cells.push(DbCell { links, subs: [sub(0x28), sub(0x2c)] });
        }
        db.portals = m
            .portals
            .iter()
            .map(|&a| atv_spatial::Portal {
                corners: std::array::from_fn(|i| vf_at(ee, a + 0x30 + 0x10 * i as u32)),
                normal: vf_at(ee, a + 0x70),
                plane: F(ee.f32_at(a + 0xc4)),
                edges: std::array::from_fn(|i| vf_at(ee, a + 0x80 + 0x10 * i as u32)),
            })
            .collect();
        for k in 0..m.subs.len() {
            let s = m.subs[k];
            let kind = ee.u32_at(s + 8);
            if ee.u32_at(s + 4) != DB_LIST_VT {
                m.blocks.push(vec![]);
                db.subs.push(Sub::Other { kind });
                continue;
            }
            let (size, last, arr) = (ee.i32_at(s + 0x28), ee.i32_at(s + 0x30), ee.u32_at(s + 0x34));
            m.blocks.push((0..ee.u32_at(s + 0x2c)).map(|b| ee.u32_at(arr + 4 * b)).collect());
            let n = m.blocks[k].len();
            let mut blocks = vec![];
            for b in 0..n {
                let base = m.blocks[k][b];
                let handed = if b + 1 == n { last } else { size } as usize;
                let mut slots = vec![None; size as usize];
                for (i, slot) in slots.iter_mut().enumerate().take(handed) {
                    let v = ee.u32_at(base + 4 * i as u32);
                    *slot = Some(if v & 2 != 0 {
                        let next = v & !3;
                        Slot::Free((next != 0).then(|| m.slot_at(k, next, size as u32)))
                    } else {
                        Slot::Item(m.item(v))
                    });
                }
                blocks.push(slots);
            }
            let free = match ee.u32_at(s + 0x38) {
                0 => None,
                f => Some(m.slot_at(k, f, size as u32)),
            };
            db.subs.push(Sub::List(DbList { kind, size, last, blocks, free }));
        }
        let sub = DbInRam::index(&m.subs, ee.u32_at(rec + 8), "the record's sub-database");
        let mut record = atv_spatial::Record { cell: Some(0), sub, slot: m.place(ee, sub, ee.u32_at(rec + 0xc)), last: vf_at(ee, rec + 0x10), extras: vec![] };
        let mut l = ee.u32_at(rec);
        while l != 0 {
            let sub = DbInRam::index(&m.subs, ee.u32_at(l + 8), "an extra's sub-database");
            record.extras.push(Extra { at: m.link_at(l), cell: DbInRam::index(&m.cells, ee.u32_at(l + 4), "an extra's cell"), sub, slot: m.place(ee, sub, ee.u32_at(l + 0xc)) });
            assert!(record.extras.len() < 64, "the record's extras do not end");
            l = ee.u32_at(l);
        }
        (m, db, record)
    }

    fn place_word(&self, sub: usize, p: Place) -> u32 {
        match p {
            Place::Slot(s) => self.slot_addr(sub, s),
            Place::Key(k) => k,
        }
    }

    /// The move's outcome (`now`, read as `was`) written back where it changed: the record's words
    /// and each list's slots, count and free chain. What a move here never changes — the cells, a
    /// list's blocks, the link pool, the extras — must not have changed.
    fn write(&self, ee: &mut Ee, entry: u32, was: (&Database, &atv_spatial::Record), now: (&Database, &atv_spatial::Record)) {
        assert_eq!(was.0.cells, now.0.cells, "the cells changed");
        assert_eq!(was.0.pool, now.0.pool, "the link pool changed: not written back");
        assert_eq!(was.1.extras, now.1.extras, "the record's extras changed: not written back");
        for (k, (a, b)) in was.0.subs.iter().zip(&now.0.subs).enumerate() {
            let (Sub::List(a), Sub::List(b)) = (a, b) else {
                assert_eq!(a, b, "sub-database {:#x} is no list and changed", self.subs[k]);
                continue;
            };
            assert_eq!(a.blocks.len(), b.blocks.len(), "list {:#x} grew a block: not written back", self.subs[k]);
            for (bl, (x, y)) in a.blocks.iter().zip(&b.blocks).enumerate() {
                for (i, (x, y)) in x.iter().zip(y).enumerate() {
                    if x == y {
                        continue;
                    }
                    let w = match y.expect("a slot handed out is handed out still") {
                        Slot::Item(it) => self.items[it],
                        Slot::Free(n) => n.map_or(0, |n| self.slot_addr(k, n)) | 2,
                    };
                    ee.set_u32(self.slot_addr(k, SlotAt { block: bl, index: i }), w);
                }
            }
            let s = self.subs[k];
            ee.set_u32(s + 0x30, b.last as u32);
            ee.set_u32(s + 0x38, b.free.map_or(0, |f| self.slot_addr(k, f)));
        }
        let rec = ee.u32_at(entry + 8);
        let r = now.1;
        if r.cell.is_some() {
            ee.set_u32(rec + 4, self.cells[r.cell.unwrap()]);
            ee.set_u32(rec + 8, self.subs[r.sub]);
            ee.set_u32(rec + 0xc, self.place_word(r.sub, r.slot));
        }
        set_vf(ee, rec + 0x10, r.last);
    }
}

/// **F46a — a collidable's spatial-database move in Rust**: `entry`'s database read out of RAM,
/// moved by [`atv_spatial::spatial_move`], and written back.
pub(super) fn spatial_move_in_ram(ee: &mut Ee, entry: u32, centre: Vf, half: Vf, reach: F) {
    let (m, mut db, mut rec) = DbInRam::read(ee, entry);
    let (was_db, was_rec) = (db.clone(), rec.clone());
    atv_spatial::spatial_move(&mut db, &mut rec, 0, centre, half, reach);
    m.write(ee, entry, (&was_db, &was_rec), (&db, &rec));
}
use game_sim::broad_phase as bp;
use game_sim::world_search as ws;
use game_sim::{atv_part, ray_probe};


/// F46c: the sweep probe's clear's globals — the two words it zeroes, and where it notes the
/// probe's record.
const SWEEP_ZEROED: [u32; 2] = [0x003d_ce28, 0x003d_ce20];
const SWEEP_RECORD_AT: u32 = 0x003d_c618;

/// The nearby search's world round `probe` (a collidable): the probe as item 0, the nearby list at
/// `list`'s candidates, and the collidables their object lists name, with the filter's fields; and
/// the candidates by item. Answers the entries by item too.
fn sweep_world_in_ram(ee: &Ee, probe: u32, list: u32) -> (Vec<u32>, Vec<ws::Collidable>, Vec<ws::Candidate>) {
    let mut m = GatherInRam::default();
    m.item(probe + 0x18);
    let (arr, n) = (ee.u32_at(list), ee.u32_at(list + 4));
    let found = (0..n).map(|i| ws::Candidate { object: m.item(ee.u32_at(arr + 8 * i)), distance: word_f(ee, arr + 8 * i + 4) }).collect();
    let mut objects = vec![];
    let mut k = 0;
    while k < m.items.len() {
        let e = m.items[k];
        let mut o = ws::Collidable { class: ws::Class::from_byte(ee.u8_at(e + 2)), ..Default::default() };
        if o.class == ws::Class::Object {
            let c = e - 0x18;
            o.flags = ee.u8_at(c + 0x126);
            o.nearby = ee.u8_at(c + 0x45) & 1 != 0;
            o.radius = word_f(ee, c + 0x70);
            o.category = ee.u32_at(c + 0x94);
            o.collides_with = ee.u32_at(c + 0xdc);
            o.objects = m.objects(ee, c + 0xe0);
            o.more_objects = m.objects(ee, c + 0xec);
            let cs = c + 0xf8;
            o.categories = ws::Categories { mode: ee.u32_at(cs), list: (0..ee.u32_at(cs + 8)).map(|i| ee.u32_at(ee.u32_at(cs + 4) + 4 * i)).collect() };
        }
        objects.push(o);
        k += 1;
    }
    (m.items, objects, found)
}

/// **F46c — the record's sweep in Rust**: the segment from `from` to `to` set on the record `rec`'s
/// sweep probe `probe` ([`sweep_set_in_ram`]) and searched over the nearby list `list`
/// ([`atv_search::search`] with [`atv_sweep::Sweep`]: since F51 its pairs too, over
/// [`SweepWorldInRam`]); everything written back. Answers the hit bit.
pub(super) fn sweep_in_ram(ee: &mut Ee, rec: u32, probe: u32, list: u32, from: Vf, to: Vf, kinds: &mut BTreeMap<String, u32>) -> bool {
    let segments = sweep_set_in_ram(ee, rec, probe, from, to);
    sweep_search_in_ram(ee, probe, list, segments, kinds)
}

/// F46c's search in Rust: the sweep probe `probe` on `segments` (just set) searched over the nearby
/// list `list`, as [`sweep_in_ram`] has it. Answers the hit bit.
fn sweep_search_in_ram(ee: &mut Ee, probe: u32, list: u32, segments: Vec<game_sim::sweep_probe::Segment>, kinds: &mut BTreeMap<String, u32>) -> bool {
    use game_sim::atv_search;
    use game_sim::atv_sweep;
    // The search.
    assert_eq!(ee.u32_at(list + 0x40), 0, "the nearby list has a grid: the sweep would prepare it");
    let (mut items, mut objects, found) = sweep_world_in_ram(ee, probe, list);
    // The plant groups' parts, as collidables of their own.
    for k in 0..objects.len() {
        if objects[k].class != ws::Class::Group {
            continue;
        }
        for p in PartWorldInRam::species_parts(ee, items[k]) {
            if p != 0 && !items.contains(&(p + 0x18)) {
                items.push(p + 0x18);
                objects.push(ws::Collidable { class: ws::Class::from_byte(ee.u8_at(p + 0x18 + 2)), flags: ee.u8_at(p + 0x126), ..Default::default() });
            }
        }
    }
    let retired: Vec<bool> = items.iter().map(|&e| ee.u32_at(e - 0x18) == 1).collect();
    let count = segments.len();
    let r = ee.u32_at(probe + 0x7c);
    let (answer, more) = sweep_answer_in_ram(ee, r, count, &mut items);
    let mut port = SweepProbe { segments: segments.clone(), answer, ..SweepProbe::default() };
    sweep_local_in_ram(ee, probe, count, &mut port);
    let mut stood = port.clone();
    sweep_probe_clear(&mut stood);
    let mut world = PartWorldInRam { ee: &mut *ee, items, mesh: None, kinds: BTreeMap::new(), listeners: vec![], callbacks: vec![] };
    for (k, o) in objects.iter_mut().enumerate() {
        if o.class == ws::Class::Object {
            let e = world.items[k];
            world.hear(o, e - 0x18);
        }
    }
    let heard: Vec<ws::Collidable> = objects.clone();
    let mut outside = SweepWorldInRam { world: &mut world, centre: stood.world.centre, radius: stood.world.radius, pairs: 0, descriptor: None };
    let mut sweep = atv_sweep::Sweep::new(segments, more, &mut outside);
    sweep.probe = port;
    let hit = atv_search::search(&mut objects, 0, &found, &|o: usize| retired.get(o).copied().unwrap_or(false), &mut sweep);
    let (port, more) = (sweep.probe, sweep.more);
    let (pairs, descriptor) = (outside.pairs, outside.descriptor);
    for (k, v) in std::mem::take(&mut world.kinds) {
        *kinds.entry(k).or_default() += v;
    }
    let items = world.items;
    for (k, (a, b)) in heard.iter().zip(&objects).enumerate() {
        assert_eq!(ws::Collidable { flags: b.flags, radius: b.radius, group: b.group, ..a.clone() }, *b, "item {k} changed beyond its flags and reach");
        if a.flags != b.flags {
            assert_eq!(a.class, ws::Class::Object, "item {k}'s flags");
            set_u8(ee, items[k] - 0x18 + 0x126, b.flags);
        }
        if let Some(g) = b.group {
            ee.set_u32(items[k] - 0x18 + 0x88, items[g]);
        }
    }
    // The clear's world form and the record's reset.
    let w = &port.world;
    let [lengths, starts, dirs] = [0x144, 0x148, 0x14c].map(|o| ee.u32_at(probe + o));
    for (i, p) in w.probes[..w.count].iter().enumerate() {
        let i = i as u32;
        ee.set_u32(lengths + 4 * i, p.length.0.to_bits());
        set_vf(ee, starts + 0x10 * i, p.origin);
        set_vf(ee, dirs + 0x10 * i, p.dir);
    }
    for o in [0x160, 0x50] {
        set_vf(ee, probe + o, w.centre);
    }
    assert_eq!(objects[0].radius, w.radius);
    for o in [0x170, 0x70] {
        ee.set_u32(probe + o, w.radius.0.to_bits());
    }
    for a in SWEEP_ZEROED {
        ee.set_u32(a, 0);
    }
    ee.set_u32(SWEEP_RECORD_AT, r);
    sweep_pairs_write(ee, probe, count, &port, pairs, descriptor);
    write_sweep_answer(ee, r, count, &items, &port.answer, &more);
    hit
}

/// The probe's local forms (each segment's origin and direction in the last paired object's
/// frame, and the centre) into `port`, as RAM has them.
fn sweep_local_in_ram(ee: &Ee, probe: u32, count: usize, port: &mut SweepProbe) {
    use f51_probe as pa;
    assert_eq!(ee.u32_at(probe + pa::SMOOTH), 0, "the probe asks for smoothed normals: not ported");
    let (locals, ldirs) = (ee.u32_at(probe + pa::LOCAL_ORIGINS), ee.u32_at(probe + pa::LOCAL_DIRS));
    for i in 0..count {
        port.local.probes[i].origin = vf_at(ee, locals + 0x10 * i as u32);
        port.local.probes[i].dir = vf_at(ee, ldirs + 0x10 * i as u32);
    }
    port.local.centre = vf_at(ee, probe + pa::LOCAL_CENTRE);
}

/// What the pairs write beyond the answer, where any ran: the smoothing word noted in its global,
/// the local forms, and the last swept mesh's descriptor in its global.
fn sweep_pairs_write(ee: &mut Ee, probe: u32, count: usize, port: &SweepProbe, pairs: u32, descriptor: Option<u32>) {
    use f51_probe as pa;
    if pairs > 0 {
        let (locals, ldirs) = (ee.u32_at(probe + pa::LOCAL_ORIGINS), ee.u32_at(probe + pa::LOCAL_DIRS));
        ee.set_u32(SWEEP_SMOOTH_AT, ee.u32_at(probe + pa::SMOOTH));
        for i in 0..count as u32 {
            set_vf(ee, locals + 0x10 * i, port.local.probes[i as usize].origin);
            set_vf(ee, ldirs + 0x10 * i, port.local.probes[i as usize].dir);
        }
        set_vf(ee, probe + pa::LOCAL_CENTRE, port.local.centre);
    }
    if let Some(d) = descriptor {
        ee.set_u32(SWEEP_DESCRIPTOR_AT, d);
    }
}

/// F46c's set and place in Rust: the segment from `from` to `to` (the record `rec`'s own, written at
/// `+0xc0`) set on its sweep probe `probe` ([`atv_sweep::set`]), and the probe placed (its box, and
/// its database entry moved, [`spatial_move_in_ram`]). Answers the segments.
fn sweep_set_in_ram(ee: &mut Ee, rec: u32, probe: u32, from: Vf, to: Vf) -> Vec<game_sim::sweep_probe::Segment> {
    set_vf(ee, rec + 0xc0, from);
    set_vf(ee, rec + 0xd0, to);
    // The set: the record's segment the probe's, no nodes.
    let n = ee.u32_at(probe + 0x130);
    let points: Vec<[Vf; 2]> = (0..n).map(|i| [vf_at(ee, rec + 0xc0 + 0x20 * i), vf_at(ee, rec + 0xd0 + 0x20 * i)]).collect();
    sweep_set_points_in_ram(ee, probe, &points)
}

/// F46c's set and place on the sweep probe `probe`, from its segments' `points` with no nodes.
fn sweep_set_points_in_ram(ee: &mut Ee, probe: u32, points: &[[Vf; 2]]) -> Vec<game_sim::sweep_probe::Segment> {
    use game_sim::atv_sweep;
    let (segments, b) = atv_sweep::set(points, None);
    let (nodes, quads) = (ee.u32_at(probe + 0x134), ee.u32_at(probe + 0x138));
    for (i, s) in segments.iter().enumerate() {
        let i = i as u32;
        for k in 0..2 {
            ee.set_u32(nodes + 4 * (2 * i + k), 0);
            set_vf(ee, quads + 0x10 * (2 * i + k), s.ends[k as usize]);
        }
    }
    set_vf(ee, probe + 0x50, b.centre);
    for o in [0x170, 0x70] {
        ee.set_u32(probe + o, b.reach.0.to_bits());
    }
    // The place: its box again, and its entry moved.
    if ee.u32_at(probe + 0x20) != 0 {
        let b = atv_sweep::bounds(&segments);
        set_vf(ee, probe + 0x50, b.centre);
        ee.set_u32(probe + 0x70, b.reach.0.to_bits());
        spatial_move_in_ram(ee, probe + 0x18, b.centre, b.half, b.reach);
    }
    segments
}

/// A ray probe's or a mesh's tree as it sits in RAM, 36 bytes a node (the key, the two child words
/// at `+8`, the centre at `+0xc`, the half extents at `+0x18`), read from `root` down; and one past
/// its highest leaf.
fn tree_in_ram(ee: &Ee, nodes: u32, root: u16) -> (Vec<track::Node>, usize) {
    let blank = track::Node { volume: F::ZERO, centre: [F::ZERO; 3], half_extent: [F::ZERO; 3], children: [0, 0] };
    let (mut out, mut seen, mut leaves, mut todo) = (Vec::new(), Vec::new(), 0, vec![root]);
    while let Some(w) = todo.pop() {
        if w & 0x8000 != 0 {
            leaves = leaves.max(usize::from(w & 0x7fff) + 1);
            continue;
        }
        let n = nodes + 36 * u32::from(w);
        if out.len() <= usize::from(w) {
            out.resize(usize::from(w) + 1, blank);
            seen.resize(usize::from(w) + 1, false);
        }
        // A tree reaches each node once; a second visit is a loop, which would grow `todo` forever.
        assert!(!std::mem::replace(&mut seen[usize::from(w)], true), "node {w} at {nodes:#x} reached twice: not a tree");
        let three = |a: u32| [0, 4, 8].map(|k| word_f(ee, a + k));
        let children = [ee.u16_at(n + 8), ee.u16_at(n + 10)];
        out[usize::from(w)] = track::Node { volume: word_f(ee, n), centre: three(n + 12), half_extent: three(n + 24), children };
        todo.extend(children);
    }
    (out, leaves)
}

fn matrix_at(ee: &Ee, a: u32) -> ray_probe::Matrix {
    [0, 1, 2, 3].map(|k| vf_at(ee, a + 0x10 * k))
}

/// A mesh collidable (kind 0) as the part's pair reads it: its frame (`+0x130`), its tree's root
/// (`+0x23c`), nodes (`+0x244`) and triangles (`+0x248`, 0x14 bytes each: three vertex shorts, the
/// packed normal at `+0xc`), and its vertices through the descriptor at `+0x230` (the array, and at
/// `+8` whether they are packed shorts).
struct MeshInRam {
    id: u32,
    transform: ray_probe::Matrix,
    root: u16,
    nodes: Vec<track::Node>,
    facets: Vec<ray_probe::Facet>,
    vertices: track::Points,
}

impl MeshInRam {
    fn read(ee: &Ee, c: u32) -> MeshInRam {
        assert_eq!(ee.u32_at(c + 0x78), 0, "{c:#x} is not a mesh");
        let root = ee.u16_at(c + 0x23c);
        let (nodes, count) = tree_in_ram(ee, ee.u32_at(c + 0x244), root);
        let at = ee.u32_at(c + 0x248);
        let facets: Vec<ray_probe::Facet> = (0..count as u32)
            .map(|i| {
                let f = at + 0x14 * i;
                ray_probe::Facet { vertices: [0, 2, 4].map(|k| ee.u16_at(f + k)), normal: [0, 2, 4].map(|k| ee.u16_at(f + 0xc + k) as i16) }
            })
            .collect();
        let corners = facets.iter().flat_map(|f| f.vertices).max().map_or(0, |m| u32::from(m) + 1);
        let d = c + 0x230;
        let array = ee.u32_at(d);
        let vertices = if ee.u32_at(d + 8) != 0 {
            track::Points::Packed((0..corners).map(|i| [0, 2, 4].map(|k| ee.u16_at(array + 6 * i + k) as i16)).collect())
        } else {
            track::Points::Wide((0..corners).map(|i| [0, 4, 8].map(|k| word_f(ee, array + 12 * i + k))).collect())
        };
        MeshInRam { id: c, transform: matrix_at(ee, c + 0x130), root, nodes, facets, vertices }
    }

    fn mesh(&self) -> ray_probe::Mesh<'_> {
        ray_probe::Mesh { id: self.id, transform: self.transform, root: self.root, nodes: &self.nodes, facets: &self.facets, vertices: &self.vertices }
    }
}

/// A plant group's part `i` stood at the plant (`(group entry, i)`, answers the part or 0).
const ECO_PART: u32 = 0x0046_d3c0;
/// F54: the riding part's two listeners, by their vt `+8` (thunks to the bike, `this - 0x38`, and to
/// the pair log's owner, `this - 0x28`); the bike's hit slot (vt `+0x138`); the file and line the pair
/// log's push passes the heap's resize.
const BIKE_HEARD: u32 = 0x0049_56a0;
const PAIR_LOG_HEARD: u32 = 0x0025_b950;
const BIKE_HIT_SLOT: u32 = 0x138;
const PAIR_LOG_FILE: u32 = 0x003a_1c30;
const PAIR_LOG_LINE: u32 = 0xd1;
const HEAP_RESIZE_F54: u32 = 0x001b_7470;

use game_sim::{atv_hear, atv_plant};

/// The tables a turned plant mesh reads, built once.
fn plant_tables() -> atv_plant::Tables<'static> {
    static T: std::sync::OnceLock<(AcosTable, CurveTable)> = std::sync::OnceLock::new();
    let (acos, curve) = T.get_or_init(|| (AcosTable::build(), CurveTable::build()));
    atv_plant::Tables { acos, curve }
}

/// The plant (group entry `g`) and its species' scale words as RAM has them, the eco's unit, and its
/// part `i` (0 for none).
fn plant_in_ram(ee: &Ee, g: u32, i: usize) -> (atv_plant::Plant, atv_plant::Species, F, u32) {
    let eco = ee.u32_at(ECO_AT);
    let sp = ee.u32_at(ee.u32_at(eco + 0x84) + 4 * u32::from(ee.u8_at(g + 0x12)));
    let flags = ee.u8_at(g + 0x16);
    let turn = if flags & atv_plant::flags::TURNED != 0 { raw_quad(ee, ee.u32_at(eco + 0x70) + 0x50 * u32::from(ee.u8_at(g + 0x17)) + 0x10) } else { Vf::ZERO };
    let plant = atv_plant::Plant { position: [0xc, 0xe, 0x10].map(|o| ee.u16_at(g + o)), lean: ee.u8_at(g + 0x14) as i8, size: ee.u8_at(g + 0x15) as i8, flags, turn };
    let species = atv_plant::Species { lean: [word_f(ee, sp + 0x100), word_f(ee, sp + 0x104)], size: [word_f(ee, sp + 0x10c), word_f(ee, sp + 0x114)] };
    let part = if flags & atv_plant::flags::HIDDEN != 0 { 0 } else { ee.u32_at(ee.u32_at(sp + 0x19c) + 4 * i as u32) };
    (plant, species, word_f(ee, eco + 0xc4), part)
}

/// **F54 — a plant group's part stood at its plant in Rust** ([`atv_plant::stand`]): the plant at
/// `g`, its part `i`; a sphere's scales, frame and refresh written back (its stand slot asserted to
/// be the sphere's frame), a mesh's stood frame's position written and its stand slot run at the
/// port's frame: MD118, a mesh part's slot (the frame mesh's vtable's) is [`atv_sync::stand`], its
/// words written back (`frame_at` is not read). Answers the part or 0, as the original does.
fn plant_stand_with(ee: &mut Ee, g: u32, i: usize, frame_at: Option<u32>) -> u32 {
    let (plant, species, unit, p) = plant_in_ram(ee, g, i);
    let tables = plant_tables();
    if p == 0 {
        assert_eq!(atv_plant::stand(&plant, &species, unit, None, Some(&tables)), atv_plant::Stood::Nothing);
        return 0;
    }
    let vt = ee.u32_at(p + 0xc);
    match ee.u32_at(p + 0x78) {
        5 => {
            assert_eq!(ee.u32_at(vt + 0xcc), SPHERE_FRAME, "a plant sphere's stand slot ({p:#x})");
            let mut s = sphere_in_ram(ee, p);
            let mut scales = [word_f(ee, p + 0x188), word_f(ee, p + 0x18c)];
            let r = atv_plant::stand(&plant, &species, unit, Some(atv_plant::Part::Sphere { sphere: &mut s, scales: &mut scales }), Some(&tables));
            assert_eq!(r, atv_plant::Stood::Done);
            write_sphere(ee, p, &s);
            ee.set_u32(p + 0x188, scales[0].0.to_bits());
            ee.set_u32(p + 0x18c, scales[1].0.to_bits());
        }
        0 => {
            let mut stood = [0, 1, 2, 3].map(|k| raw_quad(ee, p + 0x1b0 + 0x10 * k));
            let r = atv_plant::stand(&plant, &species, unit, Some(atv_plant::Part::Mesh { stood: &mut stood }), Some(&tables));
            let atv_plant::Stood::Mesh(frame) = r else { panic!("a mesh stood as {r:?}") };
            for k in 0..3 {
                ee.set_u32(p + 0x1e0 + 4 * k, stood[3].0[k as usize].to_bits());
            }
            let _ = frame_at;
            assert_eq!(ee.u32_at(p + 0xc), crate::MESH_VTABLE, "a plant mesh's vtable ({p:#x}): its stand slot is not the frame mesh's");
            let mut gaps = vec![];
            let mut c = crate::image::collidable(ee, p, &mut gaps);
            assert!(gaps.is_empty(), "{gaps:?}");
            game_sim::atv_sync::stand(&mut c, &frame);
            let game_sim::atv_sync::Kind::Mesh { stood, before, .. } = c.kind else { unreachable!() };
            for (o, rows) in [(0x130, c.frame), (0x1b0, stood), (0x1f0, before)] {
                for (k, row) in rows.iter().enumerate() {
                    set_vf(ee, p + o + 0x10 * k as u32, *row);
                }
            }
            set_vf(ee, p + 0x50, c.centre);
            ee.set_u32(p + 0x70, c.reach.0.to_bits());
        }
        _ => assert_eq!(atv_plant::stand(&plant, &species, unit, Some(atv_plant::Part::Other), Some(&tables)), atv_plant::Stood::Done),
    }
    p
}

/// [`plant_stand_with`] in a search: no plant mesh is stood on C15 (its stand slot is not Rust's).
fn plant_stand_in_ram(ee: &mut Ee, g: u32, i: usize) -> u32 {
    plant_stand_with(ee, g, i, None)
}

/// **F54 — the bike's own listener in Rust** ([`atv_hear::bike_hears`]): the bike at `bike`, the struck
/// collidable at `struck`.
fn bike_heard_in_ram(ee: &Ee, bike: u32, struck: u32) -> atv_hear::Heard {
    let (flags, mask) = (ee.u8_at(bike + 0x44), ee.u32_at(ee.u32_at(bike + 0xe8) + 0xa4));
    if atv_hear::bike_hears(flags, mask, ee.u32_at(struck + 0x94)) { atv_hear::Heard::Bike } else { atv_hear::Heard::Nothing }
}

/// **F54 — the pair log's listener in Rust** ([`atv_hear::push`]): the owner at `o` logs `{probe,
/// struck, 0, kind}` on its log (`+0x3c8`), the log's words written back; a grow is the heap's resize
/// (the original's, `grown` told the bytes).
fn pair_log_heard_in_ram(ee: &mut Ee, o: u32, probe: u32, struck: u32, grown: &mut dyn FnMut(u32)) {
    let l = o + 0x3c8;
    let kind = atv_hear::kind(ee.u32_at(struck + 0x94), [ee.u32_at(o + 0x2c), ee.u32_at(o + 0x30)]);
    let i = |ee: &Ee, k: u32| ee.u32_at(l + k) as i32;
    let mut log = atv_hear::Log { count: i(ee, 0), entries: ee.u32_at(l + 8), step: i(ee, 0xc), capacity: i(ee, 0x10), most: i(ee, 0x14) };
    let slot = atv_hear::push(&mut log, &mut |at, bytes| {
        grown(bytes);
        ee.set(28, GP);
        ee.call(HEAP_RESIZE_F54, &[u64::from(at), u64::from(bytes), u64::from(PAIR_LOG_FILE), u64::from(PAIR_LOG_LINE)], &[], BUDGET);
        ee.v0() as u32
    });
    ee.set_u32(l + 8, log.entries);
    ee.set_u32(l + 0x10, log.capacity as u32);
    if let Some(slot) = slot {
        for (k, w) in atv_hear::entry(probe, struck, kind).into_iter().enumerate() {
            ee.set_u32(log.entries.wrapping_add(slot << 4) + 4 * k as u32, w);
        }
        ee.set_u32(l, log.count as u32);
    }
}

/// The world of a part search in RAM: what each item is to the pair, read from RAM at each pair (a
/// mesh, kind 0; a hierarchy, kind 1, not read: C15 never pairs one; anything else nothing struck),
/// and a plant group's parts, each stood at the plant by the port ([`plant_stand_in_ram`], F54).
/// Counts the pairs by kind, the stands and the listeners heard.
struct PartWorldInRam<'e> {
    ee: &'e mut Ee,
    items: Vec<u32>,
    mesh: Option<MeshInRam>,
    kinds: BTreeMap<String, u32>,
    /// The listeners and the callbacks the collidables name ([`ws::Collidable::listeners`], `on_hit`,
    /// `on_struck` index these), each called in the original (handed on).
    listeners: Vec<u32>,
    callbacks: Vec<u32>,
}

impl PartWorldInRam<'_> {
    /// Calls the original's `f` with `args` (collidables by item, as their bases), counted as `what`,
    /// the probe's and the other's flags written first (the hit bit is up when they run). A listener
    /// must leave both listener lists as they were: the search's are read once.
    fn hand_on(&mut self, objects: &[ws::Collidable], what: &str, f: u32, this: u32, a: usize, b: usize) {
        *self.kinds.entry(format!("> record search {what}")).or_default() += 1;
        let [ca, cb] = [a, b].map(|k| self.items[k] - 0x18);
        for (k, c) in [(a, ca), (b, cb)] {
            set_u8(self.ee, c + 0x126, objects[k].flags);
        }
        let counts = [ca, cb].map(|c| self.ee.u32_at(c + 0x104));
        self.ee.set(28, GP);
        let args: Vec<u64> = if this == 0 { vec![u64::from(ca), u64::from(cb)] } else { vec![u64::from(this), u64::from(ca), u64::from(cb)] };
        self.ee.call(f, &args, &[], BUDGET);
        assert_eq!([ca, cb].map(|c| self.ee.u32_at(c + 0x104)), counts, "{what} changed a listener list");
    }

    /// The listeners (`+0x104` the count, `+0x10c` the array) and the callbacks (`+0x11c` on a hit,
    /// `+0x120` on being struck) of the collidable at `c`, into `o`.
    fn hear(&mut self, o: &mut ws::Collidable, c: u32) {
        let (n, arr) = (self.ee.u32_at(c + 0x104), self.ee.u32_at(c + 0x10c));
        for j in 0..n {
            o.listeners.push(self.listeners.len());
            self.listeners.push(self.ee.u32_at(arr + 4 * j));
        }
        for (slot, at) in [(&mut o.on_hit, 0x11c), (&mut o.on_struck, 0x120)] {
            let f = self.ee.u32_at(c + at);
            if f != 0 {
                *slot = Some(self.callbacks.len());
                self.callbacks.push(f);
            }
        }
    }

    /// The species' parts of the plant group at entry `g` (0 where it has none).
    fn species_parts(ee: &Ee, g: u32) -> Vec<u32> {
        let sp = ee.u32_at(ee.u32_at(ee.u32_at(ECO_AT) + 0x84) + 4 * u32::from(ee.u8_at(g + 0x12)));
        (0..ee.u32_at(sp + 0x194)).map(|i| ee.u32_at(ee.u32_at(sp + 0x19c) + 4 * i)).collect()
    }
}

impl atv_part::Outside for PartWorldInRam<'_> {
    fn shape(&mut self, other: usize) -> ray_probe::Shape<'_> {
        let c = self.items[other] - 0x18;
        let kind = self.ee.u32_at(c + 0x78);
        *self.kinds.entry(format!("record search pair, kind {kind}")).or_default() += 1;
        match kind {
            0 => {
                // MD118: the pair's tree walk's setup (the part's pair always runs it on a mesh)
                // notes in its globals the walk's mode (1), the mesh's descriptor, and the two tree
                // arrays it keeps in its own frame, gone once the pair returns: any stack address.
                self.ee.set_u32(TREE_WALK_MODE_AT, 1);
                self.ee.set_u32(SWEEP_DESCRIPTOR_AT, c + 0x230);
                let sp = self.ee.stack;
                self.ee.set_u32(TREE_WALK_TREES_AT[0], sp - 8);
                self.ee.set_u32(TREE_WALK_TREES_AT[1], sp - 0x10);
                ray_probe::Shape::Mesh(self.mesh.insert(MeshInRam::read(self.ee, c)).mesh())
            }
            1 => panic!("the part search paired a hierarchy ({c:#x}): its parts are not read"),
            5 => unreachable!("a sphere is the part's own test"),
            6 => panic!("the part search paired a kind 6 ({c:#x}): its test is not Rust's"),
            _ => ray_probe::Shape::Never,
        }
    }
    fn sphere(&mut self, other: usize) -> Option<atv_part::Sphere> {
        let c = self.items[other] - 0x18;
        (self.ee.u32_at(c + 0x78) == 5).then(|| {
            *self.kinds.entry("record search pair, kind 5".into()).or_default() += 1;
            atv_part::Sphere { centre: vf_at(self.ee, c + 0x50), radius: word_f(self.ee, c + 0x180), radius2: word_f(self.ee, c + 0x184) }
        })
    }
}

impl ws::Narrow for PartWorldInRam<'_> {
    fn clear(&mut self, _: &mut [ws::Collidable], _: usize) {
        unreachable!("the part's own clear")
    }
    fn prepare(&mut self, _: &mut [ws::Collidable], _: usize) {
        panic!("the part search prepared a grid")
    }
    fn pair(&mut self, _: &mut [ws::Collidable], _: usize, _: usize) -> bool {
        unreachable!("the part's own pair")
    }
    fn parts(&mut self, _: &mut [ws::Collidable], group: usize) -> usize {
        Self::species_parts(self.ee, self.items[group]).len()
    }
    fn part(&mut self, _: &mut [ws::Collidable], group: usize, i: usize) -> Option<usize> {
        *self.kinds.entry("record search part stand".into()).or_default() += 1;
        let p = plant_stand_in_ram(self.ee, self.items[group], i);
        (p != 0).then(|| self.items.iter().position(|&e| e == p + 0x18).expect("a stood part the search did not list"))
    }
    /// F54: the riding part's two listeners Rust's ([`atv_hear`]); any other handed on.
    fn heard_hit(&mut self, objects: &mut [ws::Collidable], l: usize, probe: usize, other: usize) {
        let l = self.listeners[l];
        let f = self.ee.u32_at(self.ee.u32_at(l) + 8);
        let [ca, cb] = [probe, other].map(|k| self.items[k] - 0x18);
        match f {
            BIKE_HEARD => {
                *self.kinds.entry("record search heard_hit, the bike's".into()).or_default() += 1;
                let bike = l - 0x38;
                if bike_heard_in_ram(self.ee, bike, cb) == atv_hear::Heard::Bike {
                    // Never on C15; handed on as the original calls it, the category in `a1`.
                    *self.kinds.entry("> record search the bike's hit slot".into()).or_default() += 1;
                    for (k, c) in [(probe, ca), (other, cb)] {
                        set_u8(self.ee, c + 0x126, objects[k].flags);
                    }
                    let f = self.ee.u32_at(self.ee.u32_at(bike + 4) + BIKE_HIT_SLOT);
                    let category = self.ee.u32_at(cb + 0x94);
                    self.ee.set(28, GP);
                    self.ee.call(f, &[u64::from(bike), u64::from(category), u64::from(cb)], &[], BUDGET);
                }
            }
            PAIR_LOG_HEARD => {
                *self.kinds.entry("record search heard_hit, the pair log".into()).or_default() += 1;
                let kinds = &mut self.kinds;
                pair_log_heard_in_ram(self.ee, l - 0x28, ca, cb, &mut |_| *kinds.entry("heap: the pair log grown".into()).or_default() += 1);
            }
            _ => self.hand_on(objects, "heard_hit", f, l, probe, other),
        }
    }
    fn heard_struck(&mut self, objects: &mut [ws::Collidable], l: usize, struck: usize, probe: usize) {
        let l = self.listeners[l];
        let f = self.ee.u32_at(self.ee.u32_at(l) + 0xc);
        self.hand_on(objects, "heard_struck", f, l, struck, probe);
    }
    fn on_hit(&mut self, objects: &mut [ws::Collidable], c: usize, probe: usize, other: usize) {
        let f = self.callbacks[c];
        self.hand_on(objects, "on_hit", f, 0, probe, other);
    }
    fn on_struck(&mut self, objects: &mut [ws::Collidable], c: usize, struck: usize, probe: usize) {
        let f = self.callbacks[c];
        self.hand_on(objects, "on_struck", f, 0, struck, probe);
    }
}

/// The part's record (`+0x7c`) in RAM: what the clear leaves (the nearest normal and point, the
/// pair's first point, the struck part, the flags) and the list's shape (capacity `+0x88`, the rays
/// `+0x90` or none, one contact a ray `+0xbc`).
fn part_record_in_ram(ee: &Ee, r: u32) -> ray_probe::Record {
    ray_probe::Record {
        normal: vf_at(ee, r + 0x20),
        point: vf_at(ee, r + 0x30),
        first_point: vf_at(ee, r + 0x70),
        part: ee.u32_at(r + 0x84),
        listed: ee.u32_at(r + 0x90) != 0,
        capacity: ee.u32_at(r + 0x88) as usize,
        one_per_ray: ee.u8_at(r + 0xbc) != 0,
        flags: ee.u8_at(r + 0xb8),
        nearest: F::ONE,
        ..ray_probe::Record::default()
    }
}

/// MD118: the globals the part's pair's tree walk setup writes on a mesh: the walk's mode, and the
/// two tree arrays (pointers into its frame). The descriptor goes in [`SWEEP_DESCRIPTOR_AT`].
const TREE_WALK_MODE_AT: u32 = 0x003d_ce30;
const TREE_WALK_TREES_AT: [u32; 2] = [0x003d_ce40, 0x003d_ce48];

/// The smooth-flag global the part's pair sets from the probe's `+0x184`.
const PART_SMOOTH_AT: u32 = 0x003d_c610;

/// **F46d — the record's part search in Rust**: the ray probe `part` (the riding or wreck rays) searched
/// over the nearby list `list` ([`atv_search::search`] with [`atv_part::PartRays`]: the clear and the
/// pair [`ray_probe`]'s, the meshes read from RAM), and everything written back: the flags, the
/// globals, the record. Answers the record's contacts; counts the pairs by kind into `kinds`.
pub(super) fn part_search_in_ram(ee: &mut Ee, part: u32, list: u32, kinds: &mut BTreeMap<String, u32>) -> Vec<ray_probe::Contact> {
    use game_sim::atv_search;
    assert_eq!(ee.u32_at(part + 0x188), 0, "the part has a ray filter: its reset and test are not Rust's");
    assert_eq!(ee.u32_at(list + 0x40), 0, "the nearby list has a grid: the search would prepare it");
    let (mut items, mut objects, found) = sweep_world_in_ram(ee, part, list);
    // The plant groups' parts, as collidables of their own (the search tests them unfiltered).
    for k in 0..objects.len() {
        if objects[k].class != ws::Class::Group {
            continue;
        }
        for p in PartWorldInRam::species_parts(ee, items[k]) {
            if p != 0 && !items.contains(&(p + 0x18)) {
                items.push(p + 0x18);
                objects.push(ws::Collidable { class: ws::Class::from_byte(ee.u8_at(p + 0x18 + 2)), flags: ee.u8_at(p + 0x126), ..Default::default() });
            }
        }
    }
    let root = ee.u16_at(part + 0x170);
    let (nodes, _) = tree_in_ram(ee, ee.u32_at(part + 0x17c), root);
    let segs = ee.u32_at(part + 0x180);
    let segments: Vec<ray_probe::Segment> =
        (0..ee.u32_at(part + 0x178)).map(|i| ray_probe::Segment { from: vf_at(ee, segs + 0x20 * i), to: vf_at(ee, segs + 0x20 * i + 0x10) }).collect();
    let probe = ray_probe::RayProbe { transform: matrix_at(ee, part + 0x130), root, nodes: &nodes, segments: &segments };
    let r = ee.u32_at(part + 0x7c);
    let record = part_record_in_ram(ee, r);
    let retired: Vec<bool> = items.iter().map(|&e| ee.u32_at(e - 0x18) == 1).collect();
    let mut world = PartWorldInRam { ee: &mut *ee, items: items.clone(), mesh: None, kinds: BTreeMap::new(), listeners: vec![], callbacks: vec![] };
    for (o, &e) in objects.iter_mut().zip(&items) {
        if o.class == ws::Class::Object {
            world.hear(o, e - 0x18);
        }
    }
    let was = objects.clone();
    let mut n = atv_part::PartRays { probe, record, world: &mut world, pairs: 0 };
    let hit = atv_search::search(&mut objects, 0, &found, &|o| retired[o], &mut n);
    let (record, pairs) = (n.record, n.pairs);
    for (k, v) in std::mem::take(&mut world.kinds) {
        *kinds.entry(k).or_default() += v;
    }
    for (k, (a, b)) in was.iter().zip(&objects).enumerate() {
        assert_eq!(ws::Collidable { flags: b.flags, group: b.group, ..a.clone() }, *b, "item {k} changed beyond its flags and group");
        if let Some(g) = b.group {
            ee.set_u32(items[k] - 0x18 + 0x88, items[g]);
        }
        if a.flags != b.flags {
            assert_eq!(a.class, ws::Class::Object, "item {k}'s flags");
            set_u8(ee, items[k] - 0x18 + 0x126, b.flags);
        }
    }
    write_part_record(ee, part, r, &record, pairs);
    // A sphere struck leaves no contact: the hit bit alone says it.
    assert!(hit || record.contacts.is_empty(), "contacts without the hit bit");
    record.contacts
}

/// The part search's outcome past the flags, written back: the clear's globals, the pairs' smooth
/// flag and per-contact words, and the part's record at `r`.
fn write_part_record(ee: &mut Ee, part: u32, r: u32, record: &ray_probe::Record, pairs: u32) {
    // The clear's globals, and the pairs' smooth flag and per-contact words.
    for a in SWEEP_ZEROED {
        ee.set_u32(a, 0);
    }
    ee.set_u32(SWEEP_RECORD_AT, r);
    if pairs > 0 {
        ee.set_u32(PART_SMOOTH_AT, ee.u32_at(part + 0x184));
        let zeroed = ee.u32_at(r + 0xac);
        for i in 0..record.capacity as u32 {
            ee.set_u32(zeroed + 4 * i, 0);
        }
    }
    // The record.
    let quads = [record.from, record.to, record.normal, record.point, record.push, record.low, record.high, record.first_point];
    for (i, q) in quads.into_iter().enumerate() {
        set_vf(ee, r + 0x10 * i as u32, q);
    }
    ee.set_u32(r + 0x80, record.first.0.to_bits());
    ee.set_u32(r + 0x84, record.part);
    ee.set_u32(r + 0x8c, record.contacts.len() as u32);
    ee.set_u32(r + 0xb4, record.nearest.0.to_bits());
    set_u8(ee, r + 0xb8, record.flags);
    let [rays, normals, points, objs, placed, fractions, words] = [0x90, 0x94, 0x98, 0x9c, 0xa0, 0xa8, 0xb0].map(|o| ee.u32_at(r + o));
    for (i, c) in record.contacts.iter().enumerate() {
        let i = i as u32;
        set_vf(ee, rays + 0x20 * i, c.from);
        set_vf(ee, rays + 0x20 * i + 0x10, c.to);
        set_vf(ee, normals + 0x10 * i, c.normal);
        set_vf(ee, points + 0x10 * i, c.point);
        ee.set_u32(objs + 4 * i, c.object);
        ee.set_u32(placed + 4 * i, u32::from(c.placed));
        ee.set_u32(fractions + 4 * i, c.fraction.0.to_bits());
        let w = words + 2 * i;
        let word = ee.u32_at(w & !3);
        let shift = (w & 2) * 8;
        ee.set_u32(w & !3, word & !(0xffff << shift) | u32::from(c.segment) << shift);
    }
}

// --- F51: the sweep's pairs ------------------------------------------------------------------

/// A collidable's fields as the pair reads them.
mod f51_at {
    pub const NEARBY: u32 = 0x45;
    pub const CENTRE: u32 = 0x50;
    pub const RADIUS: u32 = 0x70;
    pub const KIND: u32 = 0x78;
    pub const FLAGS: u32 = 0x126;
    pub const TRANSFORM: u32 = 0x130;
    pub const PARTS: u32 = 0x210;
    pub const PART_LIST: u32 = 0x224;
    pub const VERTICES: u32 = 0x230;
    pub const PACKED: u32 = 0x238;
    pub const NODES: u32 = 0x244;
    pub const TRIANGLES: u32 = 0x248;
    /// How much of a mesh collidable a copy takes.
    pub const MESH_LEN: u32 = 0x250;
}

/// The sweep probe's own fields (MX Unleashed's moved `+0x10`).
mod f51_probe {
    pub const RECORD: u32 = 0x7c;
    pub const COUNT: u32 = 0x130;
    pub const NODES: u32 = 0x134;
    pub const ENDS: u32 = 0x138;
    pub const LOCAL_ORIGINS: u32 = 0x13c;
    pub const LOCAL_DIRS: u32 = 0x140;
    pub const LENGTHS: u32 = 0x144;
    pub const ORIGINS: u32 = 0x148;
    pub const DIRS: u32 = 0x14c;
    pub const LOCAL_CENTRE: u32 = 0x150;
    pub const CENTRE: u32 = 0x160;
    pub const RADIUS: u32 = 0x170;
    pub const SMOOTH: u32 = 0x174;
}

/// The record's fields as the sweep probe uses them.
mod f51_rec {
    pub const NORMAL: u32 = 0x20;
    pub const POINT: u32 = 0x30;
    pub const PUSH: u32 = 0x40;
    pub const PART: u32 = 0x84;
    pub const HITS: u32 = 0x8c;
    pub const NORMALS: u32 = 0x94;
    pub const POINTS: u32 = 0x98;
    pub const STRUCK: u32 = 0x9c;
    pub const OWNERS: u32 = 0xa0;
    pub const FLAGS: u32 = 0xa4;
    pub const FRACTIONS: u32 = 0xa8;
    pub const WORDS: u32 = 0xac;
    pub const NEAREST: u32 = 0xb4;
}

/// A mesh collidable's mesh as the sweep reads it: everything reachable from node 0, the triangles'
/// three vertex shorts, and the vertices wide or packed.
struct SweptMesh {
    nodes: Vec<track::Node>,
    triangles: Vec<track::Triangle>,
    vertices: track::Points,
}

impl SweptMesh {
    fn read(ee: &Ee, c: u32) -> SweptMesh {
        use f51_at as at;
        let (nodes_at, tris_at) = (ee.u32_at(c + at::NODES), ee.u32_at(c + at::TRIANGLES));
        let node = |i: u32| {
            let a = nodes_at + 0x24 * i;
            let v3 = |o: u32| [0, 4, 8].map(|k| word_f(ee, a + o + k));
            track::Node { volume: word_f(ee, a + 4), centre: v3(0xc), half_extent: v3(0x18), children: [ee.u16_at(a + 8), ee.u16_at(a + 0xa)] }
        };
        let (mut n_nodes, mut n_tris) = (1u32, 0u32);
        let mut todo = vec![0u32];
        let mut seen = std::collections::BTreeSet::new();
        while let Some(i) = todo.pop() {
            if !seen.insert(i) {
                continue;
            }
            let n = node(i);
            for c in 0..2 {
                match n.child(c) {
                    track::Child::Node(j) => {
                        n_nodes = n_nodes.max(u32::from(j) + 1);
                        todo.push(u32::from(j));
                    }
                    track::Child::Leaf(t) => n_tris = n_tris.max(u32::from(t) + 1),
                }
            }
        }
        let nodes = (0..n_nodes).map(node).collect();
        let triangles: Vec<track::Triangle> = (0..n_tris)
            .map(|i| track::Triangle { distance: F::ZERO, normal: [F::ZERO; 3], vertices: [0, 2, 4].map(|k| ee.u16_at(tris_at + 0x14 * i + k)) })
            .collect();
        let corners = triangles.iter().flat_map(|t| t.vertices).max().map_or(0, |m| u32::from(m) + 1);
        let va = ee.u32_at(c + at::VERTICES);
        let vertices = if ee.u32_at(c + at::PACKED) != 0 {
            track::Points::Packed((0..corners).map(|i| [0, 2, 4].map(|k| ee.u16_at(va + 6 * i + k) as i16)).collect())
        } else {
            track::Points::Wide((0..corners).map(|i| [0, 4, 8].map(|k| word_f(ee, va + 12 * i + k))).collect())
        };
        SweptMesh { nodes, triangles, vertices }
    }

    fn mesh(&self) -> MeshRef<'_> {
        MeshRef { nodes: &self.nodes, triangles: &self.triangles, vertices: &self.vertices }
    }
}

fn f51_transform(ee: &Ee, c: u32) -> Transform {
    [0, 1, 2, 3].map(|k| vf_at(ee, c + f51_at::TRANSFORM + 0x10 * k))
}

fn sweep_probe_clear(probe: &mut SweepProbe) {
    game_sim::sweep_probe::clear(probe);
}

/// The global search's sweep descriptor global: the last mesh the pair swept (its `+0x230`).
const SWEEP_DESCRIPTOR_AT: u32 = 0x003d_ce38;
/// The pair's smooth-normals global (the probe's `+0x174`).
const SWEEP_SMOOTH_AT: u32 = 0x003d_c610;

/// The sweep's world in RAM: what each item is to the pair, read from RAM at each pair (a mesh, a
/// hierarchy and its parts, or nothing struck), and everything else as F46d's part search has it
/// ([`PartWorldInRam`]: a group's parts stood by the original, the listeners and callbacks handed
/// on). Keeps what the pairs leave in the two globals only the pair writes.
struct SweepWorldInRam<'w, 'e> {
    world: &'w mut PartWorldInRam<'e>,
    /// The probe's sphere as the clear stands it, for telling which pairs sweep.
    centre: Vf,
    radius: F,
    pairs: u32,
    descriptor: Option<u32>,
}

impl atv_sweep::Outside for SweepWorldInRam<'_, '_> {
    fn target(&mut self, other: usize, pair: &mut dyn FnMut(Option<&Target>) -> bool) -> bool {
        use f51_at as at;
        let ee = &*self.world.ee;
        let c = self.world.items[other] - 0x18;
        let kind = ee.u32_at(c + at::KIND);
        *self.world.kinds.entry(format!("sweep pair, kind {kind}")).or_default() += 1;
        self.pairs += 1;
        let (nearby, centre, radius) = (ee.u8_at(c + at::NEARBY) & 1 != 0, vf_at(ee, c + at::CENTRE), word_f(ee, c + at::RADIUS));
        let parts: Vec<u32> = match kind {
            0 => vec![c],
            1 => (0..ee.u32_at(c + at::PARTS)).map(|j| ee.u32_at(ee.u32_at(c + at::PART_LIST) + 4 * j)).collect(),
            8 => panic!("the sweep paired a finite plane ({c:#x}): its pair is not ported"),
            _ => vec![],
        };
        let meshes: Vec<SweptMesh> = parts.iter().map(|&p| SweptMesh::read(ee, p)).collect();
        let ids: Vec<usize> = parts
            .iter()
            .map(|&p| match self.world.items.iter().position(|&e| e == p + 0x18) {
                Some(k) => k,
                None => {
                    self.world.items.push(p + 0x18);
                    self.world.items.len() - 1
                }
            })
            .collect();
        let ee = &*self.world.ee;
        let placed: Vec<Placed> = meshes.iter().zip(&parts).zip(&ids).map(|((m, &p), &id)| Placed { transform: f51_transform(ee, p), mesh: m.mesh(), object: id }).collect();
        let shape = match kind {
            0 => Shape::Mesh(placed[0]),
            1 => Shape::Hierarchy(&placed),
            _ => Shape::Other,
        };
        // Whether the pair gets to its sweep: the spheres as the pair tests them.
        let d = self.centre.sub(centre);
        let reach = radius + self.radius;
        if nearby && F(ps2_float::fpu::dot([d.x(), d.y(), d.z()], [d.x(), d.y(), d.z()])) < reach * reach {
            if let Some(&last) = parts.last() {
                self.descriptor = Some(last + 0x230);
            }
        }
        pair(Some(&Target { nearby, centre, radius, shape }))
    }
}

impl ws::Narrow for SweepWorldInRam<'_, '_> {
    fn clear(&mut self, _: &mut [ws::Collidable], _: usize) {
        unreachable!("the sweep's own clear")
    }
    fn prepare(&mut self, _: &mut [ws::Collidable], _: usize) {
        panic!("the sweep prepared a grid")
    }
    fn pair(&mut self, _: &mut [ws::Collidable], _: usize, _: usize) -> bool {
        unreachable!("the sweep's own pair")
    }
    fn parts(&mut self, objects: &mut [ws::Collidable], group: usize) -> usize {
        self.world.parts(objects, group)
    }
    fn part(&mut self, objects: &mut [ws::Collidable], group: usize, i: usize) -> Option<usize> {
        self.world.part(objects, group, i)
    }
    fn heard_hit(&mut self, objects: &mut [ws::Collidable], l: usize, probe: usize, other: usize) {
        self.world.heard_hit(objects, l, probe, other)
    }
    fn heard_struck(&mut self, objects: &mut [ws::Collidable], l: usize, struck: usize, probe: usize) {
        self.world.heard_struck(objects, l, struck, probe)
    }
    fn on_hit(&mut self, objects: &mut [ws::Collidable], c: usize, probe: usize, other: usize) {
        self.world.on_hit(objects, c, probe, other)
    }
    fn on_struck(&mut self, objects: &mut [ws::Collidable], c: usize, struck: usize, probe: usize) {
        self.world.on_struck(objects, c, struck, probe)
    }
}

/// The sweep probe's segments in RAM: its count, its two node pointers (a node's world transform at
/// its `+0x70`) and its two points each.
fn sweep_segments_in_ram(ee: &Ee, probe: u32) -> Vec<game_sim::sweep_probe::Segment> {
    use f51_probe as pa;
    let (nodes, ends) = (ee.u32_at(probe + pa::NODES), ee.u32_at(probe + pa::ENDS));
    (0..ee.u32_at(probe + pa::COUNT))
        .map(|i| {
            let mut s = game_sim::sweep_probe::Segment::default();
            for k in 0..2 {
                s.ends[k as usize] = vf_at(ee, ends + 0x20 * i + 0x10 * k);
                let n = ee.u32_at(nodes + 8 * i + 4 * k);
                s.nodes[k as usize] = (n != 0).then(|| [0, 1, 2, 3].map(|r| vf_at(ee, n + 0x70 + 0x10 * r)));
            }
            s
        })
        .collect()
}

/// The sweep probe's answer in RAM (`+0x7c`), its first `count` segments' entries, and the record's
/// [`atv_sweep::More`]; collidables by `items` (one not listed is pushed).
fn sweep_answer_in_ram(ee: &Ee, rec: u32, count: usize, items: &mut Vec<u32>) -> (game_sim::sweep_probe::Answer, atv_sweep::More) {
    use f51_rec as ra;
    let mut item = |c: u32| -> Option<usize> {
        (c != 0).then(|| match items.iter().position(|&e| e == c + 0x18) {
            Some(k) => k,
            None => {
                items.push(c + 0x18);
                items.len() - 1
            }
        })
    };
    let mut a = game_sim::sweep_probe::Answer { head: [vf_at(ee, rec), vf_at(ee, rec + 0x10)], ..Default::default() };
    let mut more = atv_sweep::More { part: item(ee.u32_at(rec + ra::PART)), ..Default::default() };
    let r = &mut a.record;
    r.nearest_normal = vf_at(ee, rec + ra::NORMAL);
    r.nearest_point = vf_at(ee, rec + ra::POINT);
    r.push = vf_at(ee, rec + ra::PUSH);
    r.hits = ee.u32_at(rec + ra::HITS) as i32;
    r.nearest = word_f(ee, rec + ra::NEAREST);
    let [normals, points, struck, owners, hits, fractions, words] = [ra::NORMALS, ra::POINTS, ra::STRUCK, ra::OWNERS, ra::FLAGS, ra::FRACTIONS, ra::WORDS].map(|o| ee.u32_at(rec + o));
    for i in 0..count {
        let iu = i as u32;
        r.normal[i] = vf_at(ee, normals + 0x10 * iu);
        r.point[i] = vf_at(ee, points + 0x10 * iu);
        r.owner[i] = ee.u32_at(owners + 4 * iu) as i32;
        let h = ee.u32_at(hits + 4 * iu);
        assert!(h <= 1, "segment {i}'s hit word is {h:#x}");
        r.hit[i] = h != 0;
        r.fraction[i] = word_f(ee, fractions + 4 * iu);
        more.words[i] = ee.u32_at(words + 4 * iu);
        a.struck[i] = item(ee.u32_at(struck + 4 * iu));
    }
    (a, more)
}

/// [`sweep_answer_in_ram`]'s answer written back.
fn write_sweep_answer(ee: &mut Ee, rec: u32, count: usize, items: &[u32], a: &game_sim::sweep_probe::Answer, more: &atv_sweep::More) {
    use f51_rec as ra;
    let base = |k: Option<usize>| k.map_or(0, |k| items[k] - 0x18);
    set_vf(ee, rec, a.head[0]);
    set_vf(ee, rec + 0x10, a.head[1]);
    let r = &a.record;
    set_vf(ee, rec + ra::NORMAL, r.nearest_normal);
    set_vf(ee, rec + ra::POINT, r.nearest_point);
    set_vf(ee, rec + ra::PUSH, r.push);
    ee.set_u32(rec + ra::HITS, r.hits as u32);
    ee.set_u32(rec + ra::NEAREST, r.nearest.0.to_bits());
    ee.set_u32(rec + ra::PART, base(more.part));
    let [normals, points, struck, owners, hits, fractions, words] = [ra::NORMALS, ra::POINTS, ra::STRUCK, ra::OWNERS, ra::FLAGS, ra::FRACTIONS, ra::WORDS].map(|o| ee.u32_at(rec + o));
    for i in 0..count {
        let iu = i as u32;
        set_vf(ee, normals + 0x10 * iu, r.normal[i]);
        set_vf(ee, points + 0x10 * iu, r.point[i]);
        ee.set_u32(owners + 4 * iu, r.owner[i] as u32);
        ee.set_u32(hits + 4 * iu, u32::from(r.hit[i]));
        ee.set_u32(fractions + 4 * iu, r.fraction[i].0.to_bits());
        ee.set_u32(words + 4 * iu, more.words[i]);
        ee.set_u32(struck + 4 * iu, base(a.struck[i]));
    }
}


// --- S279/F54: a plant sphere's words -------------------------------------------------------

/// S279: the frame a sphere's stand ends in (its vt `+0xcc`).
const SPHERE_FRAME: u32 = 0x001c_3340;
/// The sphere's words: its point, frame, radius and square, node, record; as a collidable, its
/// centre and reach.
mod s279_at {
    pub const LOCAL: u32 = 0x130;
    pub const FRAME: u32 = 0x140;
    pub const RADIUS: u32 = 0x180;
    pub const RADIUS2: u32 = 0x184;
    pub const NODE: u32 = 0x190;
    pub const RECORD: u32 = 0x7c;
    pub const CENTRE: u32 = 0x50;
    pub const REACH: u32 = 0x70;
}

/// A quadword's four words as they are (no normalising).
fn raw_quad(ee: &Ee, a: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| f32::from_bits(ee.u32_at(a + k))))
}

fn sphere_in_ram(ee: &Ee, s: u32) -> atv_sphere::Sphere {
    use s279_at as at;
    atv_sphere::Sphere {
        local: raw_quad(ee, s + at::LOCAL),
        frame: [0, 1, 2, 3].map(|k| raw_quad(ee, s + at::FRAME + 0x10 * k)),
        radius: word_f(ee, s + at::RADIUS),
        radius2: word_f(ee, s + at::RADIUS2),
        centre: raw_quad(ee, s + at::CENTRE),
        reach: word_f(ee, s + at::REACH),
    }
}

fn write_sphere(ee: &mut Ee, s: u32, p: &atv_sphere::Sphere) {
    use s279_at as at;
    set_vf(ee, s + at::LOCAL, p.local);
    for (k, r) in p.frame.iter().enumerate() {
        set_vf(ee, s + at::FRAME + 0x10 * k as u32, *r);
    }
    ee.set_u32(s + at::RADIUS, p.radius.0.to_bits());
    ee.set_u32(s + at::RADIUS2, p.radius2.0.to_bits());
    set_vf(ee, s + at::CENTRE, p.centre);
    ee.set_u32(s + at::REACH, p.reach.0.to_bits());
}
