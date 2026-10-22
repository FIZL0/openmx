//! S86 — the riding bike's collidables against the original.
//!
//! On S93's split-screen RAM images (both players riding, P1 on a pro rider and P2 on another bike),
//! with each player's four collidables as the ride leaves them:
//!
//! - `both_bikes_load_the_disc_files`: each bike's model name, its body rays (segments and tree) and
//!   its frame mesh (tree, triangles, vertices) in memory against `Bike\Col\bikem.col` and `bikef.col`
//!   off the disc through [`BikeCol::load`]; the two reaches; and every mask, flag and list against
//!   [`bike_col::collidables`], the result record's actors word, and the wreck part's words beside.
//! - `body_rays_strike_the_other_frame_as_the_original`: P2's body rays (the probe's pair, vt +0x94)
//!   against P1's frame mesh, 10,000 cases, each with a segment run through a point on one of the
//!   frame's triangles (or near it), turned any way, and a rewritten result record; against
//!   [`BikeCol::strike`]. Compared: the return, the probe's flag byte and the record, bit for bit.

use game_render::disc::Disc;
use game_sim::bike_col::{self, BikeCol};
use game_sim::ray_probe::{self, Contact, Facet, Matrix, Record};
use game_sim::track::{Node, Points};
use game_sim::world_search::{Class, Collidable, flags};
use game_sim::wreck_bike;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::path::PathBuf;

const IMAGES: [&str; 3] = ["splitscreen", "s93-short-5997", "s93-long-3402"];
/// The two players' vehicles, the same in every image.
const P1: u32 = 0x00c1_f810;
const P2: u32 = 0x012e_3ad0;
const CASES: usize = 10_000;
const BUDGET: u64 = 50_000_000;

/// The vehicle's fields: its model name (inline), its collidables, its result record and wreck part.
mod bike {
    pub const MODEL: u32 = 0x28c;
    pub const ACTOR: u32 = 0x390;
    pub const SHAPE: u32 = 0x394;
    pub const WRECK_PART: u32 = 0x7a8;
    pub const COLLIDABLES: [u32; 4] = [0x7c0, 0x7c4, 0x7c8, 0x7cc];
    pub const RECORD: u32 = 0x7d0;
    /// The wreck part's collidable and collision object.
    pub const PART_OBJECT: u32 = 0x120;
    pub const PART_COLLIDABLE: u32 = 0x128;
}

/// A collidable's fields.
mod at {
    pub const CLASS: u32 = 0x02;
    pub const NEARBY: u32 = 0x31;
    pub const RADIUS: u32 = 0x60;
    pub const KIND: u32 = 0x68;
    pub const RESULT: u32 = 0x6c;
    pub const CATEGORY: u32 = 0x84;
    pub const COLLIDES_WITH: u32 = 0xcc;
    pub const OBJECTS: u32 = 0xd0;
    pub const MORE_OBJECTS: u32 = 0xdc;
    pub const CATEGORIES: u32 = 0xe8;
    pub const FLAGS: u32 = 0x116;
    pub const TRANSFORM: u32 = 0x120;
    pub const PROBE_ROOT: u32 = 0x160;
    pub const PROBE_NODES: u32 = 0x16c;
    pub const PROBE_SEGMENTS: u32 = 0x170;
    pub const PROBE_FILTER: u32 = 0x174;
    pub const PROBE_EXTRA: u32 = 0x178;
    pub const MESH_VERTICES: u32 = 0x220;
    pub const MESH_ROOT: u32 = 0x22c;
    pub const MESH_NODES: u32 = 0x234;
    pub const MESH_FACETS: u32 = 0x238;
    /// A collision object's actors and collides-with words.
    pub const ACTORS: u32 = 0x110;
    pub const ACTORS_WITH: u32 = 0x114;
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

/// The global the record writers find their record in.
const CURRENT_RECORD: u32 = 0x0046_8258;
/// The globals the masks come from: the rider and vehicle categories and the ride's two start words.
const RIDER_WORD: u32 = 0x0046_8f50;
const VEHICLE_WORD: u32 = 0x0046_8f68;
const START_WORDS: [u32; 2] = [0x004e_2128, 0x004e_2130];

fn ctx(rel: &str) -> Option<PathBuf> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let local = kit.root.join("context").join(rel);
    let path = if local.exists() { local } else { kit.ctx(rel) };
    path.exists().then_some(path)
}

/// One of S93's images, from `states/` or `states/s93/`.
fn image(name: &str) -> Option<Ee> {
    let Some(path) = ctx(&format!("states/s93/{name}.ram")).or_else(|| ctx(&format!("states/{name}.ram"))) else {
        skip(&format!("{name}.ram is not there: S93's split-screen RAM images (context/states, or states/s93)"));
        return None;
    };
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

/// The two files off the disc, out of the track archive the split ride loads.
fn disc_files() -> Option<(Vec<u8>, Vec<u8>)> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let res = disc.res("Tracks\\free09_ss_2.res").expect("the split track's archive");
    let read = |name: String| res.read_suffix(&name).unwrap_or_else(|e| panic!("{e}"));
    Some((read(bike_col::rays_file(bike_col::MODEL)), read(bike_col::shape_file(bike_col::MODEL))))
}

fn load() -> Option<BikeCol> {
    let (rays, shape) = disc_files()?;
    Some(BikeCol::load(&rays, &shape).expect("the bike's collision files"))
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

/// A tree as it sits in memory, 36 bytes a node, read from the root down; with one past the highest
/// leaf.
fn tree(ee: &Ee, nodes: u32, root: u16) -> (Vec<Node>, usize) {
    let blank = Node { volume: F(0.0), centre: [F(0.0); 3], half_extent: [F(0.0); 3], children: [0, 0] };
    let (mut out, mut leaves, mut todo) = (Vec::new(), 0, vec![root]);
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
        todo.extend(children);
    }
    (out, leaves)
}

/// Node bits, so that a `-0.0` or a NaN shows.
fn node_bits(nodes: &[Node]) -> Vec<(u32, [u32; 3], [u32; 3], [u16; 2])> {
    nodes
        .iter()
        .map(|n| (n.volume.to_bits(), n.centre.map(|x| x.to_bits()), n.half_extent.map(|x| x.to_bits()), n.children))
        .collect()
}

fn point_bits(p: &Points) -> Vec<[u32; 3]> {
    match p {
        Points::Wide(p) => p.iter().map(|v| v.map(|x| x.to_bits())).collect(),
        Points::Packed(p) => p.iter().map(|v| v.map(|x| x as u16 as u32 | 0xffff_0000)).collect(),
    }
}

/// A bike's frame mesh as memory has it: tree, triangles and vertices.
struct Frame {
    root: u16,
    nodes: Vec<Node>,
    facets: Vec<Facet>,
    vertices: Points,
}

fn frame(ee: &Ee, o: u32, vertex_count: usize) -> Frame {
    assert_eq!(ee.u32_at(o + at::KIND), 0, "{o:#x}: the frame is not a mesh");
    let root = ee.u16_at(o + at::MESH_ROOT);
    let (nodes, count) = tree(ee, ee.u32_at(o + at::MESH_NODES), root);
    let f = ee.u32_at(o + at::MESH_FACETS);
    let facets = (0..count as u32)
        .map(|i| Facet {
            vertices: [0, 2, 4].map(|k| ee.u16_at(f + 0x14 * i + k)),
            normal: [0, 2, 4].map(|k| ee.u16_at(f + 0x14 * i + 0xc + k) as i16),
        })
        .collect();
    let d = o + at::MESH_VERTICES;
    let array = ee.u32_at(d);
    let n = vertex_count as u32;
    let vertices = if ee.u32_at(d + 8) != 0 {
        Points::Packed((0..n).map(|i| [0, 2, 4].map(|k| ee.u16_at(array + 6 * i + k) as i16)).collect())
    } else {
        Points::Wide((0..n).map(|i| [0, 4, 8].map(|k| single(ee, array + 12 * i + k))).collect())
    };
    Frame { root, nodes, facets, vertices }
}

fn vertex_count(p: &Points) -> usize {
    match p {
        Points::Wide(p) => p.len(),
        Points::Packed(p) => p.len(),
    }
}

/// A collidable's masks, flags and lists as memory has them; the lists name the bike's four by their
/// place in [`bike::COLLIDABLES`] (offset by `first`). The searched bit is the search's, not the
/// builder's, and is dropped; the radius is left 0 here and compared on its own.
fn collidable(ee: &Ee, c: u32, mine: &[u32; 4], first: usize) -> Collidable {
    let index = |o: u32| first + mine.iter().position(|&m| m == o).unwrap_or_else(|| panic!("{o:#x} is not one of the bike's"));
    let objects = |a: u32| game_sim::world_search::Objects {
        mode: ee.u32_at(a),
        list: (0..ee.u32_at(a + 8)).map(|i| index(ee.u32_at(ee.u32_at(a + 4) + 4 * i))).collect(),
    };
    let a = c + at::CATEGORIES;
    Collidable {
        class: Class::from_byte(ee.u8_at(c + at::CLASS)),
        flags: ee.u8_at(c + at::FLAGS) & !flags::SEARCHED,
        nearby: ee.u8_at(c + at::NEARBY) & 1 != 0,
        category: ee.u32_at(c + at::CATEGORY),
        collides_with: ee.u32_at(c + at::COLLIDES_WITH),
        objects: objects(c + at::OBJECTS),
        more_objects: objects(c + at::MORE_OBJECTS),
        categories: game_sim::world_search::Categories {
            mode: ee.u32_at(a),
            list: (0..ee.u32_at(a + 8)).map(|i| ee.u32_at(ee.u32_at(a + 4) + 4 * i)).collect(),
        },
        ..Collidable::default()
    }
}

#[test]
fn both_bikes_load_the_disc_files() {
    let Some(col) = load() else { return };
    assert_eq!(col.rays.segments.len(), 11, "bikem.col: eleven body rays");
    let (mut seen, mut ghosted) = (0, 0);
    for name in IMAGES {
        let Some(ee) = image(name) else { continue };
        assert_eq!(ee.u32_at(RIDER_WORD), bike_col::RIDER, "{name}: the rider word");
        assert_eq!(ee.u32_at(VEHICLE_WORD), bike_col::VEHICLE, "{name}: the vehicle word");
        assert_eq!(START_WORDS.map(|a| ee.u32_at(a)), bike_col::RIDE_START, "{name}: the ride's start words");
        for (player, v) in [("P1", P1), ("P2", P2)] {
            let at_ = |k: u32| ee.u32_at(v + k);
            let model: Vec<u8> = (0..8).map(|k| ee.u8_at(v + bike::MODEL + k)).take_while(|&b| b != 0).collect();
            assert_eq!(model, bike_col::MODEL.as_bytes(), "{name} {player}: the model name");
            assert_eq!((at_(bike::ACTOR), at_(bike::SHAPE)), (bike_col::ACTOR, bike_col::SHAPE), "{name} {player}: +0x390/+0x394");
            let mine = bike::COLLIDABLES.map(at_);

            // The body rays.
            let r = mine[bike_col::BODY_RAYS];
            assert_eq!(ee.u32_at(r + at::KIND), 2, "{name} {player}: the body rays are a ray probe");
            let root = ee.u16_at(r + at::PROBE_ROOT);
            let (nodes, count) = tree(&ee, ee.u32_at(r + at::PROBE_NODES), root);
            let seg = ee.u32_at(r + at::PROBE_SEGMENTS);
            let segments: Vec<_> = (0..count as u32).map(|i| [quad(&ee, seg + 32 * i), quad(&ee, seg + 32 * i + 16)]).collect();
            let vb = |p: &[[Vf; 2]]| p.iter().map(|s| s.map(|v| v.0.map(f32::to_bits))).collect::<Vec<_>>();
            assert_eq!(root, col.rays.root, "{name} {player}: the rays' root");
            assert_eq!(node_bits(&nodes), node_bits(&col.rays.nodes), "{name} {player}: the rays' tree");
            assert_eq!(vb(&segments), vb(&col.rays.pairs()), "{name} {player}: the body rays");

            // The frame mesh.
            let f = mine[bike_col::FRAME];
            let m = frame(&ee, f, vertex_count(&col.frame.vertices));
            assert_eq!(m.root, 0, "{name} {player}: the frame's root");
            assert_eq!(node_bits(&m.nodes), node_bits(&col.frame.nodes), "{name} {player}: the frame's tree");
            assert_eq!(m.facets, col.facets, "{name} {player}: the frame's triangles");
            assert_eq!(point_bits(&m.vertices), point_bits(&col.frame.vertices), "{name} {player}: the frame's vertices");

            // The reaches the files give, and the masks and lists.
            assert_eq!(single(&ee, f + at::RADIUS).to_bits(), col.frame_radius.to_bits(), "{name} {player}: the frame's reach");
            assert_eq!(single(&ee, r + at::RADIUS).to_bits(), col.rays_radius().to_bits(), "{name} {player}: the rays' reach");
            let want = bike_col::collidables(&col, 0);
            for (k, c) in mine.iter().enumerate() {
                let have = collidable(&ee, *c, &mine, 0);
                let mut want = Collidable { radius: F::ZERO, ..want[k].clone() };
                // A bike that has been a ghost: the put-back added the vehicle category and took it out.
                if (k == bike_col::BODY_RAYS || k == bike_col::SWEEP) && have.categories.list == [0] {
                    bike_col::ghost(&mut want.categories, bike_col::VEHICLE, true);
                    bike_col::ghost(&mut want.categories, bike_col::VEHICLE, false);
                    ghosted += 1;
                }
                assert_eq!(have, want, "{name} {player}: collidable +{:#x}", bike::COLLIDABLES[k]);
            }
            let rec = at_(bike::RECORD);
            assert_eq!(ee.u32_at(rec + at::ACTORS), bike_col::RECORD_ACTORS, "{name} {player}: the record's actors");
            assert_eq!(ee.u32_at(rec + at::ACTORS_WITH), u32::MAX, "{name} {player}: the record collides with all");

            // The wreck part beside them: S69's words, read off the riding bike.
            let part = at_(bike::WRECK_PART);
            let pc = ee.u32_at(part + bike::PART_COLLIDABLE);
            let po = ee.u32_at(part + bike::PART_OBJECT);
            assert_eq!(ee.u32_at(pc + at::CATEGORY), wreck_bike::COLLIDABLE, "{name} {player}: the wreck part's categories");
            assert_eq!(
                (ee.u32_at(po + at::ACTORS), ee.u32_at(po + at::ACTORS_WITH)),
                (wreck_bike::ACTORS, wreck_bike::COLLIDES_WITH),
                "{name} {player}: the wreck part's object"
            );
            seen += 1;
        }
    }
    eprintln!("{seen} bikes match the disc ({ghosted} lists left by a ghost)");
    if seen == 0 {
        skip("no S93 image here");
    }
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

/// A point on triangle `i` of the frame at `t`, in the world.
fn world_point(col: &BikeCol, t: &Matrix, i: usize, pick: &mut Pick) -> [f32; 3] {
    let v = col.facets[i].vertices.map(|k| match &col.frame.vertices {
        Points::Wide(p) => p[k as usize].map(|x| x.0),
        Points::Packed(p) => p[k as usize].map(f32::from),
    });
    let (mut a, mut b) = (pick.unit(), pick.unit());
    if a + b > 1.0 {
        (a, b) = (1.0 - a, 1.0 - b);
    }
    let local: [f32; 3] = core::array::from_fn(|l| v[0][l] + a * (v[1][l] - v[0][l]) + b * (v[2][l] - v[0][l]));
    core::array::from_fn(|l| t[3].0[l] + local[0] * t[0].0[l] + local[1] * t[1].0[l] + local[2] * t[2].0[l])
}

/// The probe moved so that a point along one of its segments lands on `target`; the rows' `w` kept
/// from `like`.
fn place_probe(col: &BikeCol, like: &Matrix, target: [f32; 3], pick: &mut Pick) -> Matrix {
    let r = if pick.one_in(16) { [0, 1, 2].map(|k| like[k].0[..3].try_into().unwrap()) } else { turn(pick) };
    let seg = col.rays.segments[pick.below(col.rays.segments.len() as u64) as usize];
    let u = pick.range(-0.05, 1.05);
    let q: [f32; 3] = core::array::from_fn(|l| seg.from.0[l] + u * (seg.to.0[l] - seg.from.0[l]));
    let t: [f32; 3] = core::array::from_fn(|l| target[l] - (q[0] * r[0][l] + q[1] * r[1][l] + q[2] * r[2][l]));
    let row = |v: [f32; 3], w: f32| Vf([v[0], v[1], v[2], w]);
    [row(r[0], like[0].w()), row(r[1], like[1].w()), row(r[2], like[2].w()), row(t, like[3].w())]
}

/// A record state both sides start from.
fn some_record(col: &BikeCol, capacity: u32, pick: &mut Pick) -> Record {
    let n = col.rays.segments.len() as u64;
    let listed = !pick.one_in(6);
    let capacity = if pick.one_in(4) { pick.below(6) as usize } else { capacity as usize };
    let held = if pick.one_in(2) { 0 } else { pick.below(capacity.min(6) as u64 + 1) as usize };
    let small = |pick: &mut Pick| if pick.one_in(3) { Vf::VF0 } else {
        let w = pick.range(-1.0, 2.0);
        pick.vector(3.0, w)
    };
    let contacts = (0..held)
        .map(|_| {
            let k = pick.below(n) as usize;
            Contact {
                from: col.rays.segments[k].from,
                to: col.rays.segments[k].to,
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

fn write_record(ee: &mut Ee, a: u32, rays: u32, r: &Record) {
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
    ee.set_u32(a + rec_at::RAYS, if r.listed { rays } else { 0 });
    let at = |ee: &Ee, off: u32| ee.u32_at(a + off);
    for (i, c) in r.contacts.iter().enumerate() {
        let i = i as u32;
        set_quad(ee, rays + 32 * i, c.from);
        set_quad(ee, rays + 32 * i + 16, c.to);
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

fn read_record(ee: &Ee, a: u32, rays: u32) -> Record {
    let at = |off: u32| ee.u32_at(a + off);
    let contacts = (0..at(rec_at::COUNT))
        .map(|i| Contact {
            from: quad(ee, rays + 32 * i),
            to: quad(ee, rays + 32 * i + 16),
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
fn body_rays_strike_the_other_frame_as_the_original() {
    let Some(col) = load() else { return };
    let Some(mut ee) = image("splitscreen") else { return };
    let probe = ee.u32_at(P2 + bike::COLLIDABLES[bike_col::BODY_RAYS]);
    let other = ee.u32_at(P1 + bike::COLLIDABLES[bike_col::FRAME]);
    assert_eq!(ee.u32_at(probe + at::KIND), 2, "P2's body rays are not a ray probe");
    assert_eq!(ee.u32_at(other + at::KIND), 0, "P1's frame is not a mesh");
    assert_eq!(ee.u32_at(probe + at::PROBE_FILTER), 0, "the rays have a filter: not the case S46 ports");
    assert_eq!(ee.u32_at(probe + at::PROBE_EXTRA), 0, "the rays have their extra object: not the case S46 ports");
    let record = ee.u32_at(probe + at::RESULT);
    let rays = ee.u32_at(record + rec_at::RAYS);
    assert_ne!(rays, 0, "P2's rays keep no list");
    let capacity = ee.u32_at(record + rec_at::CAPACITY);
    let like = matrix(&ee, probe + at::TRANSFORM);
    let frame_at = matrix(&ee, other + at::TRANSFORM);
    let pair_at = addr("ray_probe_pair").unwrap();
    let clear_at = addr("ray_probe_clear").unwrap();
    let probe_flags = ee.u8_at(probe + at::FLAGS);
    let mut pick = Pick(0x5338_3662_696b_6573);
    let (mut struck, mut contacts, mut cleared, mut twice) = (0, 0, 0, 0);

    for case in 0..CASES {
        let mut point = world_point(&col, &frame_at, pick.below(col.facets.len() as u64) as usize, &mut pick);
        if pick.one_in(8) {
            let far = pick.range(0.0, 6.0);
            point = point.map(|x| x + pick.range(-far, far));
        }
        let transform = place_probe(&col, &like, point, &mut pick);
        for (k, row) in transform.iter().enumerate() {
            set_quad(&mut ee, probe + at::TRANSFORM + 16 * k as u32, *row);
        }
        let start = some_record(&col, capacity, &mut pick);
        write_record(&mut ee, record, rays, &start);
        let flags = probe_flags | if pick.one_in(2) { flags::HIT } else { 0 };
        ee.write(probe + at::FLAGS, 1, u128::from(flags));
        ee.set_u32(CURRENT_RECORD, record);

        let mut mine = start.clone();
        let mut my_flags = flags;
        let clearing = pick.one_in(4);
        if clearing {
            cleared += 1;
            ee.call(clear_at, &[u64::from(probe)], &[], BUDGET);
            ray_probe::clear(&mut my_flags, &mut mine);
        }
        let rounds = if pick.one_in(6) { 2 } else { 1 };
        for round in 0..rounds {
            ee.call(pair_at, &[u64::from(probe), u64::from(other)], &[], BUDGET);
            let theirs = ee.v0() as u32 != 0;
            let ours = col.strike(transform, &col, other, frame_at, &mut mine);
            let got = read_record(&ee, record, rays);
            let (want, have) = (bits(&got), bits(&mine));
            assert!(
                ours == theirs && want == have && ee.u8_at(probe + at::FLAGS) == my_flags,
                "case {case} round {round} (clear first: {clearing}): returned {ours}, the original {theirs}; probe flags \
                 {my_flags:02x}, the original {:02x}\n port:     {have}\n original: {want}\n start:    {}",
                ee.u8_at(probe + at::FLAGS),
                bits(&start)
            );
            if theirs {
                struck += 1;
                contacts += got.contacts.iter().filter(|c| c.object == other && start.contacts.iter().all(|d| d != *c)).count();
                twice += usize::from(round == 1);
            }
        }
    }
    eprintln!("{CASES} cases: {struck} pairs struck ({twice} on a second pairing), {contacts} contacts written, {cleared} after a clear");
    assert!(struck > CASES / 4, "only {struck} of {CASES} pairs struck: the cases are not reaching the frame");
    assert!(contacts > CASES / 8, "only {contacts} contacts written");
}
