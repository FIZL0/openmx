//! S46 — the ray probe's narrow phase against the original.
//!
//! On the save state beside C7's recording, with the plane's own ray probe (its 36 segments and the tree
//! over them) and the 34 candidates its nearby search gathered there — 26 meshes and 8 placed groups of
//! two parts each, every one with its own tree, triangles and vertices as the game has them in memory.
//! Each case moves the probe so that one of its segments runs through a point on one of the candidate's
//! triangles (or, one case in eight, anywhere near it), turned any way, and rewrites the result record
//! the probe writes into: its nearest fields, the push and its bounds, the flags, whether it keeps a
//! list and one contact per segment, the list's capacity and the contacts already in it. Then:
//!
//! - `the_ray_probe_pair_matches_the_original`: the probe's pair answer (`ray_probe_pair`, the class's
//!   vt +0x94) on that candidate against [`pair`], one case in four after the probe's clear (vt +0x90)
//!   against [`clear`]. Compared: the return, the probe's flag byte, and every field of the record and
//!   every contact in its list, bit for bit.

use game_sim::ray_probe::{self, Contact, Facet, Hierarchy, Matrix, Mesh, Part, RayProbe, Record, Segment, Shape};
use game_sim::track::{Node, Points};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::collections::HashMap;

const STATE: &str = "biplane.p2m2_SaveState.eeMemory.bin";
const PLANE: u32 = 0x00c4_4a40;
const PLANE_COLLISION: u32 = 0x170;
const CASES: usize = 10_000;
const BUDGET: u64 = 50_000_000;

/// The collision object's fields: its nearby search and its ray probe.
const NEARBY: u32 = 0x10c;
const RAY_PROBE: u32 = 0x1f0;
/// The search's gathered list, `{object, distance}` pairs, and its count.
const FOUND: u32 = 0x00;
const FOUND_COUNT: u32 = 0x04;

/// A collidable's fields.
mod at {
    pub const KIND: u32 = 0x68;
    pub const RESULT: u32 = 0x6c;
    pub const FLAGS: u32 = 0x116;
    pub const TRANSFORM: u32 = 0x120;
    /// The probe's tree and segments.
    pub const PROBE_ROOT: u32 = 0x160;
    pub const PROBE_NODES: u32 = 0x16c;
    pub const PROBE_SEGMENTS: u32 = 0x170;
    pub const PROBE_FILTER: u32 = 0x174;
    pub const PROBE_EXTRA: u32 = 0x178;
    /// A mesh's vertex descriptor (array, smooth normals, packed), tree and triangles.
    pub const MESH_VERTICES: u32 = 0x220;
    pub const MESH_ROOT: u32 = 0x22c;
    pub const MESH_NODES: u32 = 0x234;
    pub const MESH_FACETS: u32 = 0x238;
    /// A placed group's box (centre at +0x120, half extents here), frame and parts.
    pub const GROUP_HALF: u32 = 0x130;
    pub const GROUP_FRAME: u32 = 0x1a0;
    pub const GROUP_COUNT: u32 = 0x200;
    pub const GROUP_ENABLED: u32 = 0x204;
    pub const GROUP_PARTS: u32 = 0x214;
}

/// The result record's fields.
mod rec_at {
    pub const FROM: u32 = 0x00;
    pub const TO: u32 = 0x10;
    pub const NORMAL: u32 = 0x20;
    pub const POINT: u32 = 0x30;
    pub const PUSH: u32 = 0x40;
    pub const LOW: u32 = 0x50;
    pub const HIGH: u32 = 0x60;
    pub const FIRST_POINT: u32 = 0x70;
    pub const FIRST: u32 = 0x80;
    pub const PART: u32 = 0x84;
    pub const CAPACITY: u32 = 0x88;
    pub const COUNT: u32 = 0x8c;
    pub const RAYS: u32 = 0x90;
    pub const NORMALS: u32 = 0x94;
    pub const POINTS: u32 = 0x98;
    pub const OBJECTS: u32 = 0x9c;
    pub const PLACED: u32 = 0xa0;
    pub const FRACTIONS: u32 = 0xa8;
    pub const SEGMENTS: u32 = 0xac;
    pub const NEAREST: u32 = 0xb0;
    pub const FLAGS: u32 = 0xb4;
    pub const ONE_PER_RAY: u32 = 0xb8;
}

/// The global the record writers find their record in, which the probe's clear sets.
const CURRENT_RECORD: u32 = 0x0046_8258;

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
    fn vector(&mut self, scale: f32, w: f32) -> Vf {
        Vf([self.range(-scale, scale), self.range(-scale, scale), self.range(-scale, scale), w])
    }
}

/// A quadword as its bits.
fn quad(ee: &Ee, a: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| f32::from_bits(ee.u32_at(a + k))))
}

fn set_quad(ee: &mut Ee, a: u32, v: Vf) {
    for k in 0..4 {
        ee.set_u32(a + 4 * k as u32, v.0[k].to_bits());
    }
}

fn matrix(ee: &Ee, a: u32) -> Matrix {
    [0, 1, 2, 3].map(|k| quad(ee, a + 16 * k))
}

fn single(ee: &Ee, a: u32) -> F {
    F(ee.f32_at(a))
}

/// A tree as it sits in memory, 36 bytes a node: the key, the two child words at +8, the centre at
/// +12 and the half extents at +24. Read from the root down, so only what the descent can reach; with
/// it, one past the highest leaf, and each child word's parent node.
fn tree(ee: &Ee, nodes: u32, root: u16) -> (Vec<Node>, usize, HashMap<u16, u16>) {
    let blank = Node { volume: F(0.0), centre: [F(0.0); 3], half_extent: [F(0.0); 3], children: [0, 0] };
    let (mut out, mut leaves, mut todo, mut parents) = (Vec::new(), 0, vec![root], HashMap::new());
    while let Some(w) = todo.pop() {
        if w & 0x8000 != 0 {
            leaves = leaves.max((w & 0x7fff) as usize + 1);
            continue;
        }
        let n = nodes + 36 * u32::from(w);
        if out.len() <= w as usize {
            out.resize(w as usize + 1, blank);
        }
        let three = |a: u32| [0, 4, 8].map(|k| single(ee, a + k));
        let children = [ee.u16_at(n + 8), ee.u16_at(n + 10)];
        out[w as usize] = Node { volume: single(ee, n), centre: three(n + 12), half_extent: three(n + 24), children };
        for c in children {
            parents.insert(c, w);
        }
        todo.extend(children);
    }
    (out, leaves, parents)
}

/// One mesh's collision data, read once.
struct MeshData {
    id: u32,
    transform: Matrix,
    root: u16,
    nodes: Vec<Node>,
    facets: Vec<Facet>,
    vertices: Points,
    parents: HashMap<u16, u16>,
}

impl MeshData {
    fn read(ee: &Ee, o: u32) -> MeshData {
        assert_eq!(ee.u32_at(o + at::KIND), 0, "{o:#x} is not a mesh");
        let root = ee.u16_at(o + at::MESH_ROOT);
        let (nodes, count, parents) = tree(ee, ee.u32_at(o + at::MESH_NODES), root);
        let at = ee.u32_at(o + at::MESH_FACETS);
        let facets: Vec<Facet> = (0..count as u32)
            .map(|i| {
                let f = at + 0x14 * i;
                Facet {
                    vertices: [0, 2, 4].map(|k| ee.u16_at(f + k)),
                    normal: [0, 2, 4].map(|k| ee.u16_at(f + 0xc + k) as i16),
                }
            })
            .collect();
        let corners = facets.iter().flat_map(|f| f.vertices).max().map_or(0, |m| m as u32 + 1);
        let d = o + at::MESH_VERTICES;
        let array = ee.u32_at(d);
        let vertices = if ee.u32_at(d + 8) != 0 {
            Points::Packed((0..corners).map(|i| [0, 2, 4].map(|k| ee.u16_at(array + 6 * i + k) as i16)).collect())
        } else {
            Points::Wide((0..corners).map(|i| [0, 4, 8].map(|k| single(ee, array + 12 * i + k))).collect())
        };
        MeshData { id: o, transform: matrix(ee, o + at::TRANSFORM), root, nodes, facets, vertices, parents }
    }

    /// The nodes above triangle `i`, nearest first.
    fn ancestors(&self, i: usize) -> Vec<u16> {
        let mut out = vec![];
        let mut w = 0x8000 | i as u16;
        while let Some(&p) = self.parents.get(&w) {
            out.push(p);
            w = p;
        }
        out
    }

    fn mesh(&self) -> Mesh<'_> {
        Mesh {
            id: self.id,
            transform: self.transform,
            root: self.root,
            nodes: &self.nodes,
            facets: &self.facets,
            vertices: &self.vertices,
        }
    }

    /// A point on triangle `i`, in the world.
    fn world_point(&self, i: usize, pick: &mut Pick) -> [f32; 3] {
        let v = self.facets[i].vertices.map(|k| match &self.vertices {
            Points::Wide(p) => p[k as usize].map(|x| x.0),
            Points::Packed(p) => p[k as usize].map(f32::from),
        });
        let (mut a, mut b) = (pick.unit(), pick.unit());
        if a + b > 1.0 {
            (a, b) = (1.0 - a, 1.0 - b);
        }
        let local: [f32; 3] = core::array::from_fn(|l| v[0][l] + a * (v[1][l] - v[0][l]) + b * (v[2][l] - v[0][l]));
        let t = &self.transform;
        core::array::from_fn(|l| t[3].0[l] + local[0] * t[0].0[l] + local[1] * t[1].0[l] + local[2] * t[2].0[l])
    }
}

enum Candidate {
    Mesh(MeshData),
    Group { centre: Vf, half: Vf, frame: Matrix, enabled_at: u32, parts: Vec<(bool, MeshData)> },
}

struct Scene {
    probe: u32,
    record: u32,
    transform: Matrix,
    root: u16,
    nodes: Vec<Node>,
    segments: Vec<Segment>,
    candidates: Vec<(u32, Candidate)>,
}

fn scene(ee: &Ee) -> Scene {
    let rec = ee.u32_at(PLANE + PLANE_COLLISION);
    let probe = ee.u32_at(rec + RAY_PROBE);
    assert_eq!(ee.u32_at(probe + at::KIND), 2, "the plane's ray probe is not a ray probe");
    assert_eq!(ee.u32_at(probe + at::PROBE_FILTER), 0, "the probe has a filter: not the case S46 ports");
    assert_eq!(ee.u32_at(probe + at::PROBE_EXTRA), 0, "the probe has its extra object: not the case S46 ports");
    let root = ee.u16_at(probe + at::PROBE_ROOT);
    let (nodes, count, _) = tree(ee, ee.u32_at(probe + at::PROBE_NODES), root);
    assert_eq!(count, 36, "the plane's probe has 36 rays");
    let seg = ee.u32_at(probe + at::PROBE_SEGMENTS);
    let segments = (0..count as u32).map(|i| Segment { from: quad(ee, seg + 32 * i), to: quad(ee, seg + 32 * i + 16) }).collect();
    let near = ee.u32_at(rec + NEARBY);
    let found = ee.u32_at(near + FOUND);
    let candidates = (0..ee.u32_at(near + FOUND_COUNT))
        .map(|i| {
            let o = ee.u32_at(found + 8 * i);
            let c = match ee.u32_at(o + at::KIND) {
                0 => Candidate::Mesh(MeshData::read(ee, o)),
                1 => {
                    let n = ee.u32_at(o + at::GROUP_COUNT);
                    let (en, list) = (ee.u32_at(o + at::GROUP_ENABLED), ee.u32_at(o + at::GROUP_PARTS));
                    Candidate::Group {
                        centre: quad(ee, o + at::TRANSFORM),
                        half: quad(ee, o + at::GROUP_HALF),
                        frame: matrix(ee, o + at::GROUP_FRAME),
                        enabled_at: en,
                        parts: (0..n).map(|j| (ee.u32_at(en + 4 * j) != 0, MeshData::read(ee, ee.u32_at(list + 4 * j)))).collect(),
                    }
                }
                k => panic!("candidate {o:#x} is of kind {k}: the plane's search was meshes and groups"),
            };
            (o, c)
        })
        .collect();
    Scene { probe, record: ee.u32_at(probe + at::RESULT), transform: matrix(ee, probe + at::TRANSFORM), root, nodes, segments, candidates }
}

/// A turn: a random unit quaternion's three rows.
fn turn(pick: &mut Pick) -> [[f32; 3]; 3] {
    let q = loop {
        let q = [pick.range(-1.0, 1.0), pick.range(-1.0, 1.0), pick.range(-1.0, 1.0), pick.range(-1.0, 1.0)];
        let n = q.iter().map(|x| x * x).sum::<f32>();
        if n > 0.01 && n <= 1.0 {
            break q.map(|x| x / n.sqrt());
        }
    };
    let [x, y, z, w] = q;
    [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + w * z), 2.0 * (x * z - w * y)],
        [2.0 * (x * y - w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + w * x)],
        [2.0 * (x * z + w * y), 2.0 * (y * z - w * x), 1.0 - 2.0 * (x * x + y * y)],
    ]
}

/// The probe moved so that a point `along` one of its segments lands on `target`.
fn place_probe(s: &Scene, target: [f32; 3], pick: &mut Pick) -> Matrix {
    let r = if pick.one_in(16) { [0, 1, 2].map(|k| s.transform[k].0[..3].try_into().unwrap()) } else { turn(pick) };
    let seg = s.segments[pick.below(s.segments.len() as u64) as usize];
    let u = pick.range(-0.05, 1.05);
    let q: [f32; 3] = core::array::from_fn(|l| seg.from.0[l] + u * (seg.to.0[l] - seg.from.0[l]));
    let t: [f32; 3] = core::array::from_fn(|l| target[l] - (q[0] * r[0][l] + q[1] * r[1][l] + q[2] * r[2][l]));
    let row = |v: [f32; 3], w: f32| Vf([v[0], v[1], v[2], w]);
    [row(r[0], s.transform[0].w()), row(r[1], s.transform[1].w()), row(r[2], s.transform[2].w()), row(t, s.transform[3].w())]
}

/// A record state: what the original and the port both start from.
fn some_record(s: &Scene, ee: &Ee, pick: &mut Pick) -> Record {
    let listed = !pick.one_in(6);
    let capacity = if pick.one_in(4) { pick.below(6) as usize } else { ee.u32_at(s.record + rec_at::CAPACITY) as usize };
    let held = if pick.one_in(2) { 0 } else { pick.below(capacity.min(6) as u64 + 1) as usize };
    let small = |pick: &mut Pick| if pick.one_in(3) { Vf::VF0 } else {
        let w = pick.range(-1.0, 2.0);
        pick.vector(3.0, w)
    };
    let contacts = (0..held)
        .map(|_| {
            let k = pick.below(s.segments.len() as u64) as usize;
            Contact {
                from: s.segments[k].from,
                to: s.segments[k].to,
                normal: pick.vector(1.0, 1.0),
                point: pick.vector(100.0, 1.0),
                object: pick.next() as u32,
                placed: pick.one_in(2),
                fraction: F(pick.range(-0.1, 1.2)),
                segment: 0x8000 | k as u16,
            }
        })
        .collect();
    Record {
        from: pick.vector(5.0, 1.0),
        to: pick.vector(5.0, 1.0),
        normal: pick.vector(1.0, 1.0),
        point: pick.vector(100.0, 1.0),
        push: small(pick),
        low: small(pick),
        high: small(pick),
        first_point: pick.vector(100.0, 1.0),
        first: F(pick.range(0.0, 1.5)),
        part: pick.next() as u32,
        listed,
        capacity,
        contacts,
        one_per_ray: !pick.one_in(5),
        nearest: if pick.one_in(3) { F::ONE } else { F(pick.range(-0.1, 1.5)) },
        flags: pick.next() as u8,
    }
}

/// The record's lists, as the save state has them (the port's `listed` clears the rays' pointer).
struct Lists {
    rays: u32,
}

fn write_record(ee: &mut Ee, s: &Scene, lists: &Lists, r: &Record) {
    let a = s.record;
    for (off, v) in [
        (rec_at::FROM, r.from),
        (rec_at::TO, r.to),
        (rec_at::NORMAL, r.normal),
        (rec_at::POINT, r.point),
        (rec_at::PUSH, r.push),
        (rec_at::LOW, r.low),
        (rec_at::HIGH, r.high),
        (rec_at::FIRST_POINT, r.first_point),
    ] {
        set_quad(ee, a + off, v);
    }
    ee.set_f32(a + rec_at::FIRST, r.first.0);
    ee.set_u32(a + rec_at::PART, r.part);
    ee.set_u32(a + rec_at::CAPACITY, r.capacity as u32);
    ee.set_u32(a + rec_at::COUNT, r.contacts.len() as u32);
    ee.set_u32(a + rec_at::RAYS, if r.listed { lists.rays } else { 0 });
    let at = |ee: &Ee, off: u32| ee.u32_at(a + off);
    for (i, c) in r.contacts.iter().enumerate() {
        let i = i as u32;
        set_quad(ee, lists.rays + 32 * i, c.from);
        set_quad(ee, lists.rays + 32 * i + 16, c.to);
        set_quad(ee, at(ee, rec_at::NORMALS) + 16 * i, c.normal);
        set_quad(ee, at(ee, rec_at::POINTS) + 16 * i, c.point);
        ee.set_u32(at(ee, rec_at::OBJECTS) + 4 * i, c.object);
        ee.set_u32(at(ee, rec_at::PLACED) + 4 * i, u32::from(c.placed));
        ee.set_f32(at(ee, rec_at::FRACTIONS) + 4 * i, c.fraction.0);
        ee.write(at(ee, rec_at::SEGMENTS) + 2 * i, 2, u128::from(c.segment));
    }
    ee.set_f32(a + rec_at::NEAREST, r.nearest.0);
    ee.write(a + rec_at::FLAGS, 1, u128::from(r.flags));
    ee.set_u32(a + rec_at::ONE_PER_RAY, u32::from(r.one_per_ray));
}

fn read_record(ee: &Ee, s: &Scene, lists: &Lists) -> Record {
    let a = s.record;
    let at = |off: u32| ee.u32_at(a + off);
    let count = at(rec_at::COUNT);
    let contacts = (0..count)
        .map(|i| Contact {
            from: quad(ee, lists.rays + 32 * i),
            to: quad(ee, lists.rays + 32 * i + 16),
            normal: quad(ee, at(rec_at::NORMALS) + 16 * i),
            point: quad(ee, at(rec_at::POINTS) + 16 * i),
            object: ee.u32_at(at(rec_at::OBJECTS) + 4 * i),
            placed: ee.u32_at(at(rec_at::PLACED) + 4 * i) != 0,
            fraction: single(ee, at(rec_at::FRACTIONS) + 4 * i),
            segment: ee.u16_at(at(rec_at::SEGMENTS) + 2 * i),
        })
        .collect();
    Record {
        from: quad(ee, a + rec_at::FROM),
        to: quad(ee, a + rec_at::TO),
        normal: quad(ee, a + rec_at::NORMAL),
        point: quad(ee, a + rec_at::POINT),
        push: quad(ee, a + rec_at::PUSH),
        low: quad(ee, a + rec_at::LOW),
        high: quad(ee, a + rec_at::HIGH),
        first_point: quad(ee, a + rec_at::FIRST_POINT),
        first: single(ee, a + rec_at::FIRST),
        part: at(rec_at::PART),
        listed: at(rec_at::RAYS) != 0,
        capacity: at(rec_at::CAPACITY) as usize,
        contacts,
        one_per_ray: at(rec_at::ONE_PER_RAY) != 0,
        nearest: single(ee, a + rec_at::NEAREST),
        flags: ee.u8_at(a + rec_at::FLAGS),
    }
}

/// Bits, not floats: `-0.0 == 0.0` and `NaN != NaN` would both hide a difference.
fn bits(r: &Record) -> String {
    let v = |v: Vf| v.0.map(f32::to_bits);
    let mut out = format!(
        "from {:08x?} to {:08x?} normal {:08x?} point {:08x?} push {:08x?} low {:08x?} high {:08x?} first_point {:08x?} \
         first {:08x} part {:08x} listed {} capacity {} one_per_ray {} nearest {:08x} flags {:02x}",
        v(r.from), v(r.to), v(r.normal), v(r.point), v(r.push), v(r.low), v(r.high), v(r.first_point),
        r.first.to_bits(), r.part, r.listed, r.capacity, r.one_per_ray, r.nearest.to_bits(), r.flags
    );
    for c in &r.contacts {
        out += &format!(
            "\n  contact {:04x} from {:08x?} to {:08x?} normal {:08x?} point {:08x?} object {:08x} placed {} fraction {:08x}",
            c.segment, v(c.from), v(c.to), v(c.normal), v(c.point), c.object, c.placed, c.fraction.to_bits()
        );
    }
    out
}

#[test]
fn the_ray_probe_pair_matches_the_original() {
    let Some(mut ee) = plane() else { return };
    let s = scene(&ee);
    let lists = Lists { rays: ee.u32_at(s.record + rec_at::RAYS) };
    assert_ne!(lists.rays, 0, "the plane's record keeps no list");
    let pair_at = addr("ray_probe_pair").unwrap();
    let clear_at = addr("ray_probe_clear").unwrap();
    let probe_flags = ee.u8_at(s.probe + at::FLAGS);
    let mut pick = Pick(0x5346_7261_7973_0046);
    let (mut struck, mut contacts, mut groups, mut cleared, mut full, mut ties, mut twice) = (0, 0, 0, 0, 0, 0, 0);

    for case in 0..CASES {
        let (o, cand) = &s.candidates[pick.below(s.candidates.len() as u64) as usize];
        // The target: a triangle of the mesh, or of one of the group's parts.
        let (enabled, target) = match cand {
            Candidate::Mesh(m) => (vec![], m),
            Candidate::Group { parts, .. } => {
                let en = parts.iter().map(|(e, _)| if pick.one_in(8) { !e } else { *e }).collect();
                (en, &parts[pick.below(parts.len() as u64) as usize].1)
            }
        };
        let mut point = target.world_point(pick.below(target.facets.len() as u64) as usize, &mut pick);
        if pick.one_in(8) {
            let far = pick.range(0.0, 20.0);
            point = point.map(|x| x + pick.range(-far, far));
        }
        let transform = place_probe(&s, point, &mut pick);
        for (k, row) in transform.iter().enumerate() {
            set_quad(&mut ee, s.probe + at::TRANSFORM + 16 * k as u32, *row);
        }
        if let Candidate::Group { enabled_at, .. } = cand {
            for (j, e) in enabled.iter().enumerate() {
                ee.set_u32(enabled_at + 4 * j as u32, u32::from(*e));
            }
        }
        let start = some_record(&s, &ee, &mut pick);
        write_record(&mut ee, &s, &lists, &start);
        let flags = probe_flags | if pick.one_in(2) { 0x20 } else { 0 };
        ee.write(s.probe + at::FLAGS, 1, u128::from(flags));
        ee.set_u32(CURRENT_RECORD, s.record);

        // One case in five ties the probe's root key with the target's, so the descent's `<=` decides.
        let mut nodes = s.nodes.clone();
        let root_key = ee.u32_at(s.probe + at::PROBE_NODES) + 36 * u32::from(s.root);
        let tied = pick.one_in(5);
        if tied {
            nodes[s.root as usize].volume = target.nodes[target.root as usize].volume;
            ee.set_f32(root_key, nodes[s.root as usize].volume.0);
            ties += 1;
        }

        let mut mine = start.clone();
        let mut my_flags = flags;
        let clearing = pick.one_in(4);
        if clearing {
            cleared += 1;
            ee.call(clear_at, &[u64::from(s.probe)], &[], BUDGET);
            ray_probe::clear(&mut my_flags, &mut mine);
            assert_eq!(ee.u32_at(CURRENT_RECORD), s.record, "case {case}: the clear did not make the record current");
        }
        let probe = RayProbe { transform, root: s.root, nodes: &nodes, segments: &s.segments };
        let parts: Vec<Part>;
        let shape = match cand {
            Candidate::Mesh(m) => Shape::Mesh(m.mesh()),
            Candidate::Group { centre, half, frame, parts: p, .. } => {
                parts = p.iter().zip(&enabled).map(|((_, m), e)| Part { enabled: *e, mesh: m.mesh() }).collect();
                Shape::Hierarchy(Hierarchy { centre: *centre, half: *half, frame: *frame, parts: &parts })
            }
        };
        // One case in six pairs twice: the second finds its own fractions already nearest.
        let rounds = if pick.one_in(6) { 2 } else { 1 };
        for round in 0..rounds {
            ee.call(pair_at, &[u64::from(s.probe), u64::from(*o)], &[], BUDGET);
            let theirs = ee.v0() as u32 != 0;
            let ours = ray_probe::pair(&probe, &shape, &mut mine);
            let got = read_record(&ee, &s, &lists);
            let (want, have) = (bits(&got), bits(&mine));
            assert!(
                ours == theirs && want == have && ee.u8_at(s.probe + at::FLAGS) == my_flags,
                "case {case} round {round} against {o:#x} (clear first: {clearing}, tied: {tied}): returned {ours}, the \
                 original {theirs}; probe flags {my_flags:02x}, the original {:02x}\n port:     {have}\n original: {want}\n \
                 start:    {}",
                ee.u8_at(s.probe + at::FLAGS),
                bits(&start)
            );
            if theirs {
                struck += 1;
                contacts += got.contacts.iter().filter(|c| c.object != 0 && start.contacts.iter().all(|d| d != *c)).count();
                groups += usize::from(matches!(cand, Candidate::Group { .. }));
                full += usize::from(got.capacity > 0 && got.contacts.len() == got.capacity && got.listed);
                twice += usize::from(round == 1);
            }
        }
        if tied {
            ee.set_f32(root_key, s.nodes[s.root as usize].volume.0);
        }
    }
    eprintln!(
        "{CASES} cases: {struck} pairs struck ({groups} of them groups, {twice} on a second pairing), {contacts} \
         contacts written, {cleared} after a clear, {ties} with the root keys tied, {full} filled their list"
    );
    assert!(struck > CASES / 4, "only {struck} of {CASES} pairs struck anything: the cases are not reaching the triangles");
}

/// Every mesh the plane could pair with: the candidates and the groups' parts.
fn meshes(s: &Scene) -> Vec<&MeshData> {
    s.candidates
        .iter()
        .flat_map(|(_, c)| match c {
            Candidate::Mesh(m) => vec![m],
            Candidate::Group { parts, .. } => parts.iter().map(|(_, m)| m).collect(),
        })
        .collect()
}

/// The original's three frames for the probe against a mesh, through the pointers it keeps them
/// behind: mesh to probe, probe to mesh, and the second's magnitudes.
const FRAMES: [u32; 3] = [0x0041_da30, 0x0041_da40, 0x0041_da50];
/// The tree globals: pointers to `{nodes, segments}` and `{nodes, facets}`, which the pair keeps on its
/// own stack — dead once it returns, so the test points them at copies of its own.
const TREES: [u32; 2] = [0x0046_8a80, 0x0046_8a88];

/// How often each answer came up, so a test that only ever says one thing fails.
#[derive(Default)]
struct Tally([usize; 2]);

impl Tally {
    fn add(&mut self, b: bool) {
        self.0[usize::from(b)] += 1;
    }
    fn check(&self, what: &str) {
        eprintln!("{what}: {} reach, {} do not", self.0[1], self.0[0]);
        let n = self.0[0] + self.0[1];
        assert!(self.0[0] > n / 10 && self.0[1] > n / 10, "{what}: the cases are lopsided ({:?})", self.0);
    }
}

#[test]
fn the_shape_tests_match_the_original() {
    let Some(mut ee) = plane() else { return };
    let s = scene(&ee);
    let all = meshes(&s);
    let pair_at = addr("ray_probe_pair").unwrap();
    let boxes_at = addr("ray_box_box").unwrap();
    let segment_at = addr("ray_segment_box").unwrap();
    let step_at = addr("ray_leaf_step").unwrap();
    // The pair sets the frames and the tree globals up, then descends: the descent answers nothing.
    ee.hooks.insert(addr("ray_tree_descend").unwrap(), Box::new(|ee: &mut Ee| ee.set(2, 0)));
    ee.set_u32(CURRENT_RECORD, s.record);
    let mut pick = Pick(0x5346_7368_6170_6573);
    let (mut boxes, mut segments, mut facets, mut leaves) = (Tally::default(), Tally::default(), Tally::default(), Tally::default());
    let sc: [u32; 4] = [0, 1, 2, 3].map(|k| ee.scratch(k));

    for case in 0..CASES {
        let m = all[pick.below(all.len() as u64) as usize];
        let f = pick.below(m.facets.len() as u64) as usize;
        let mut point = m.world_point(f, &mut pick);
        if pick.one_in(8) {
            let far = pick.range(0.0, 20.0);
            point = point.map(|x| x + pick.range(-far, far));
        }
        let transform = place_probe(&s, point, &mut pick);
        for (k, row) in transform.iter().enumerate() {
            set_quad(&mut ee, s.probe + at::TRANSFORM + 16 * k as u32, *row);
        }
        ee.call(pair_at, &[u64::from(s.probe), u64::from(m.id)], &[], BUDGET);
        for (k, (nodes, list)) in [(s.probe + at::PROBE_NODES, s.probe + at::PROBE_SEGMENTS), (m.id + at::MESH_NODES, m.id + at::MESH_FACETS)]
            .into_iter()
            .enumerate()
        {
            let copy = ee.scratch(6 + k as u32);
            ee.set_u32(copy, ee.u32_at(nodes));
            ee.set_u32(copy + 4, ee.u32_at(list));
            ee.set_u32(TREES[k], copy);
        }

        let to_probe = ray_probe::compose(&ray_probe::invert(&transform), &m.transform);
        let to_mesh = ray_probe::invert(&to_probe);
        let to_mesh_abs = ray_probe::abs_rows(&to_mesh);
        let theirs: [Matrix; 3] = FRAMES.map(|g| matrix(&ee, ee.u32_at(g)));
        let rows = |m: &[Vf]| m.iter().map(|v| v.0.map(f32::to_bits)).collect::<Vec<_>>();
        assert_eq!(rows(&theirs[0]), rows(&to_probe), "case {case}: the mesh-to-probe frame");
        assert_eq!(rows(&theirs[1]), rows(&to_mesh), "case {case}: the probe-to-mesh frame");
        assert_eq!(rows(&theirs[2][..3]), rows(&to_mesh_abs), "case {case}: the frame's magnitudes");

        let probe = RayProbe { transform, root: s.root, nodes: &s.nodes, segments: &s.segments };
        let mesh = m.mesh();
        let above = m.ancestors(f);
        let near_node = |pick: &mut Pick| &m.nodes[above[pick.below(above.len() as u64) as usize] as usize];
        let v = |c: [F; 3], w: f32| Vf([c[0].0, c[1].0, c[2].0, w]);
        let place = |t: &Matrix, p: [f32; 3]| -> Vf {
            Vf(core::array::from_fn(|l| if l == 3 { 1.0 } else { t[3].0[l] + p[0] * t[0].0[l] + p[1] * t[1].0[l] + p[2] * t[2].0[l] }))
        };
        for _ in 0..4 {
            // A mesh node over the target against a probe node, real or moved about it.
            let a = near_node(&mut pick);
            let p = &s.nodes[pick.below(s.nodes.len() as u64) as usize];
            let (ac, ah) = (v(a.centre, pick.range(-1.0, 1.0)), v(a.half_extent, pick.range(-1.0, 1.0)));
            let (bc, bh) = if pick.one_in(2) {
                (v(p.centre, 0.0), v(p.half_extent, 0.0))
            } else {
                let reach = a.half_extent.iter().chain(&p.half_extent).map(|x| x.0).fold(0.0, f32::max) * 1.5;
                let home = place(&to_probe, ac.0[..3].try_into().unwrap());
                let k = pick.range(0.2, 2.0);
                (Vf(core::array::from_fn(|l| if l == 3 { 0.0 } else { home.0[l] + pick.range(-reach, reach) })),
                 Vf(p.half_extent.map(|x| x.0 * k).iter().copied().chain([0.0]).collect::<Vec<_>>().try_into().unwrap()))
            };
            for (k, q) in [ac, ah, bc, bh].into_iter().enumerate() {
                set_quad(&mut ee, sc[k], q);
            }
            ee.call(boxes_at, &sc.map(u64::from), &[], BUDGET);
            let ours = ray_probe::boxes(ac, ah, bc, bh, &to_mesh, &to_mesh_abs);
            assert_eq!(ours, ee.v0() != 0, "case {case}: boxes {ac:?} {ah:?} against {bc:?} {bh:?}");
            boxes.add(ours);

            // A segment against a mesh node over the target: the probe's own, or one near the box.
            let n = near_node(&mut pick);
            let (from, to) = if pick.one_in(2) {
                let seg = s.segments[pick.below(s.segments.len() as u64) as usize];
                (seg.from, seg.to)
            } else {
                let h = n.half_extent.map(|x| x.0);
                let size = h.iter().copied().fold(0.0, f32::max);
                let p: [f32; 3] = core::array::from_fn(|l| n.centre[l].0 + pick.range(-1.3, 1.3) * h[l]);
                let len = pick.range(0.1, 2.0) * size;
                let q: [f32; 3] = core::array::from_fn(|_| pick.range(-len, len));
                let q: [f32; 3] = core::array::from_fn(|l| p[l] + q[l]);
                (place(&to_probe, p), place(&to_probe, q))
            };
            let c = v(n.centre, 0.0);
            for (k, q) in [c, v(n.half_extent, 0.0), from, to].into_iter().enumerate() {
                set_quad(&mut ee, sc[k], q);
            }
            let back = ee.u32_at(FRAMES[1]);
            ee.call(segment_at, &[sc[0], sc[1], sc[2], sc[3], back].map(u64::from), &[], BUDGET);
            let ours = ray_probe::segment_reaches(c, n.half_extent, from, to, &to_mesh);
            assert_eq!(ours, ee.v0() != 0, "case {case}: segment {from:?} {to:?} against {c:?} {:?}", n.half_extent);
            segments.add(ours);

            // The descent's mixed step, both ways round, on the real trees.
            let (a, b, tally) = if pick.one_in(2) {
                let near = (f + pick.below(5) as usize).saturating_sub(2).min(m.facets.len() - 1);
                (pick.below(s.nodes.len() as u64) as u16, 0x8000 | near as u16, &mut facets)
            } else {
                (0x8000 | pick.below(s.segments.len() as u64) as u16, above[pick.below(above.len() as u64) as usize], &mut leaves)
            };
            ee.call(step_at, &[u64::from(a), u64::from(b)], &[], BUDGET);
            let ours = ray_probe::leaf_reaches(&probe, &mesh, a, b);
            assert_eq!(ours, ee.v0() != 0, "case {case}: the step {a:04x} against {b:04x} in {:#x}", m.id);
            tally.add(ours);
        }
    }
    boxes.check("box against box");
    segments.check("segment against box");
    facets.check("box against triangle");
    leaves.check("segment against node");
}

#[test]
fn the_group_gate_matches_the_original() {
    let Some(mut ee) = plane() else { return };
    let s = scene(&ee);
    let gate_at = addr("ray_group_gate").unwrap();
    let groups: Vec<_> = s.candidates.iter().filter(|(_, c)| matches!(c, Candidate::Group { .. })).collect();
    let mut pick = Pick(0x5346_6761_7465_0046);
    let mut tally = Tally::default();
    for case in 0..CASES {
        let (o, Candidate::Group { centre, half, frame, parts, .. }) = groups[pick.below(groups.len() as u64) as usize] else {
            unreachable!()
        };
        let m = &parts[pick.below(parts.len() as u64) as usize].1;
        let mut point = m.world_point(pick.below(m.facets.len() as u64) as usize, &mut pick);
        let far = pick.range(0.0, 60.0);
        point = point.map(|x| x + pick.range(-far, far));
        let transform = place_probe(&s, point, &mut pick);
        for (k, row) in transform.iter().enumerate() {
            set_quad(&mut ee, s.probe + at::TRANSFORM + 16 * k as u32, *row);
        }
        // One case in four without the probe's tree: the gate boxes its first segment instead.
        let treeless = pick.one_in(4);
        let tree_at = ee.u32_at(s.probe + at::PROBE_NODES);
        if treeless {
            ee.set_u32(s.probe + at::PROBE_NODES, 0);
        }
        ee.call(gate_at, &[u64::from(s.probe), u64::from(*o)], &[], BUDGET);
        ee.set_u32(s.probe + at::PROBE_NODES, tree_at);
        let nodes: &[Node] = if treeless { &[] } else { &s.nodes };
        let probe = RayProbe { transform, root: s.root, nodes, segments: &s.segments };
        let group = Hierarchy { centre: *centre, half: *half, frame: *frame, parts: &[] };
        let ours = ray_probe::group_reaches(&probe, &group);
        assert_eq!(ours, ee.v0() != 0, "case {case}: the probe at {:?} against group {o:#x}", transform[3]);
        tally.add(ours);
    }
    tally.check("the group gate");
}
