//! MD76a2 — the quad's wheel-ray search, proved against SLUS-21104.
//!
//! The oracle is MD37a's RAM image of Desert Oasis Free Ride with the quad at its spawn
//! (`context/mxvatv-ps2/md37a/atv-r-spawn.ee.bin`, as in `tests/atv_gather.rs`). It holds the quad's
//! trigger probe (quad `+0x1430`) with its four wheel rays, its record, and the world the gather fills
//! the quad's list (quad `+0xc4`) from. The tests run the original's own code, nothing hooked:
//!
//! - **the probe**: its tree is the port's tree of its four rays, its record is the port's, and the
//!   original's "stand the probe at this frame" gives the port's centre and reach;
//! - **the search**: the probe stood at frames that put a wheel's ray across the world's meshes —
//!   all 120 of them, the palms ahead of the spawn among them — the original's gather fills the list
//!   and its search runs over it; the record (every contact, the nearest, the low/high/push, the
//!   flags) and the hit bit must be the port's, bit for bit;
//! - **the ride**: the rows of MD37a's captures where a wheel's swept contact (wheel `+0xd4`) is set,
//!   the probe stood where the row before left the quad, must give the contact the wheel holds.
//!
//! Without the image, the files or the captures the tests skip (and say so).

use game_sim::archive::Res;
use game_sim::atv_search::{self, Plants, Wheels};
use game_sim::atv_world::{self, Nearby, PROBE, World};
use game_sim::chassis::WheelContact;
use game_sim::camera::CurveTable;
use game_sim::eco::{self, Ecosystem, Trees};
use game_sim::flight_search::Rays;
use game_sim::quad;
use game_sim::ray_probe::{self, Matrix, Record};
use game_sim::track::Scene;
use game_sim::world::{Prop, Props};
use game_sim::world_search::{Categories, Class, Collidable, Objects};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};
use std::collections::BTreeMap;
use std::path::Path;

/// SLUS-21104's `$gp`, which the calls need (crt0 sets it at `0x001001ac`).
const GP: u64 = 0x003d_baf0;
/// The gather, the search, the trigger probe's "stand at this frame" (its vtable's `+0xcc`).
const GATHER: u32 = 0x001d_6a60;
const SEARCH_FN: u32 = 0x0011_51c0;
const STAND: u32 = 0x001b_cfe0;
/// The quad, its list, the gather's probe (quad `+0xd8`) and the trigger probe (quad `+0x1430`).
const QUAD: u32 = 0x015e_0690;
const LIST: u32 = 0x0162_7a40;
const PROBE_AT: u32 = 0x0159_c500;
const SEARCH: u32 = 0x0162_cd30;
const ECO_AT: u32 = 0x0052_cd88;
const MESH_VTABLE: u32 = 0x003b_6d00;
/// MC02's species, as loaded (its file's slot 0 is empty).
const SPECIES: u32 = 11;
/// The ray probe class's vtable.
const RAY_VTABLE: u32 = 0x003b_6c20;

/// A collidable's fields (the entry the tree and the list hold is at `+0x18`).
mod at {
    pub const ENTRY: u32 = 0x18;
    pub const VTABLE: u32 = 0xc;
    pub const NEARBY: u32 = 0x45;
    pub const CENTRE: u32 = 0x50;
    pub const RADIUS: u32 = 0x70;
    pub const KIND: u32 = 0x78;
    pub const RECORD: u32 = 0x7c;
    pub const VEHICLE: u32 = 0x8c;
    pub const OWNER: u32 = 0x90;
    pub const CATEGORY: u32 = 0x94;
    pub const COLLIDES_WITH: u32 = 0xdc;
    pub const OBJECTS: u32 = 0xe0;
    pub const MORE_OBJECTS: u32 = 0xec;
    pub const CATEGORIES: u32 = 0xf8;
    pub const LISTENERS: u32 = 0x104;
    pub const ON_HIT: u32 = 0x11c;
    pub const ON_STRUCK: u32 = 0x120;
    pub const FLAGS: u32 = 0x126;
    pub const TRANSFORM: u32 = 0x130;
    /// The ray probe's: its tree's root, rays, nodes and segments, its smooth flag and filter.
    pub const ROOT: u32 = 0x170;
    pub const RAYS: u32 = 0x178;
    pub const NODES: u32 = 0x17c;
    pub const SEGMENTS: u32 = 0x180;
    pub const SMOOTH: u32 = 0x184;
    pub const FILTER: u32 = 0x188;
    /// A mesh's: its tree's root, nodes and facets.
    pub const MESH_ROOT: u32 = 0x23c;
    pub const MESH_NODES: u32 = 0x244;
}

/// The trigger probe's record's fields.
mod rec {
    pub const FROM: u32 = 0x0;
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
    pub const ZEROED: u32 = 0xac;
    pub const SEGMENTS: u32 = 0xb0;
    pub const NEAREST: u32 = 0xb4;
    pub const FLAGS: u32 = 0xb8;
    pub const ONE_PER_RAY: u32 = 0xbc;
}

/// What a struck collidable is, the same on both sides: a mesh by its centre's and reach's bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Id {
    Mesh([u32; 4]),
    /// A species' part: its model (loaded order) and its index.
    Part(usize, usize),
    Other(u32),
}

struct Data {
    ee: Ee,
    world: World,
    props: Vec<Prop>,
    eco: Ecosystem,
    trees: Trees,
    /// The trigger probe's collidable in the port's world, and each species' first part's.
    search: usize,
    slots: Vec<usize>,
    rays: Rays,
    rows: Vec<Row>,
}

/// One row of a capture: the quad's body and its four wheels' swept contacts.
#[derive(Clone, Debug)]
struct Row {
    name: String,
    frame: u32,
    pos: [f32; 3],
    quat: [f32; 4],
    /// Per wheel: `+0xd4`, and the point (`+0x120`) and normal (`+0x130`) when it is set.
    wheels: [(u32, [u32; 4], [u32; 4]); 4],
}

fn data() -> Option<Data> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let dir = kit.ctx("mxvatv-ps2/md37a");
    let image = dir.join(format!("atv-r-spawn.ee{}", ".bin"));
    let res = dir.join("mc02.res");
    let esb = dir.join("MC02.esb");
    for (p, what) in [
        (&image, "MD37a's eeMemory.bin of atv-r-spawn.p2s (memory: md37a ATV states live in s2)"),
        (&res, "Desert Oasis's mc02.res off MX vs. ATV's disc (Env/MC02)"),
        (&esb, "Env\\MC02\\MC02.esb out of MXVSATV.PSR on MX vs. ATV's disc"),
    ] {
        if !p.exists() {
            skip(&format!("{} is not there: {what}", p.display()));
            return None;
        }
    }
    let mut ee = Ee::from_ram(&image).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, GP);
    if ee.u32_at(QUAD + 0xc4) != LIST || ee.u32_at(QUAD + 0xd8) != PROBE_AT || ee.u32_at(QUAD + 0x1430) != SEARCH {
        panic!("{} is not the MX vs. ATV quad image these addresses are from", image.display());
    }
    let file = std::fs::read(&res).unwrap();
    let archive = Res::parse(&file).unwrap();
    let read = |name: &str| -> Result<Vec<u8>, String> {
        let e = archive.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name)).ok_or(format!("no {name}"))?;
        archive.read(&file, e)
    };
    let scene = Scene::parse(&read("Env\\mc02\\MC02.scn").unwrap()).unwrap();
    let (props, missed) = Props::load(&scene, |n| read(&format!("Col\\{n}")));
    assert!(missed.is_empty(), "MC02's meshes not read: {missed:?}");
    let eco = Ecosystem::parse(&std::fs::read(&esb).unwrap()).unwrap();
    let (groups, models) = atv_world::groups(&eco);
    let field = atv_world::terrain_box(&scene.terrain);
    let (trees, missed) = Trees::new(&eco, atv_world::plant_cell(field), &CurveTable::build());
    assert!(missed.is_empty(), "MC02's plant solids not built: {missed:?}");
    let mut world = World::new(collidable(&ee, PROBE_AT), &props.props, &groups, models, atv_world::plant_cell(field), field);
    let search = world.objects.len();
    world.objects.push(collidable(&ee, SEARCH));
    let first_part = world.objects.len();
    world.objects.extend(atv_search::part_collidables(&eco));
    let slots = Plants::new(&trees, &eco, &world, first_part).slots;
    let rows = capture_rows(&kit.ctx("capture/md37a"))?;
    let rays = image_rays(&ee);
    Some(Data { ee, world, props: props.props, eco, trees, search, slots, rays, rows })
}

/// The image's collidable at `c` as the port's: its reach, flags and filter. The collidables its
/// lists name (the quad's own) are not in the port's world; each stands for a number past its end.
fn collidable(ee: &Ee, c: u32) -> Collidable {
    let mut named = BTreeMap::new();
    let mut objects = |off: u32| {
        let (mode, at, n) = (ee.u32_at(c + off), ee.u32_at(c + off + 4), ee.u32_at(c + off + 8));
        let list = (0..n)
            .map(|i| {
                let p = ee.u32_at(at + 4 * i);
                let next = usize::MAX - named.len();
                *named.entry(p).or_insert(next)
            })
            .collect();
        Objects { mode, list }
    };
    let (o, m) = (objects(at::OBJECTS), objects(at::MORE_OBJECTS));
    let k = c + at::CATEGORIES;
    let categories = Categories { mode: ee.u32_at(k), list: (0..ee.u32_at(k + 8)).map(|i| ee.u32_at(ee.u32_at(k + 4) + 4 * i)).collect() };
    Collidable {
        class: Class::Object,
        flags: ee.u8_at(c + at::FLAGS),
        nearby: ee.u8_at(c + at::NEARBY) & 1 != 0,
        radius: F(ee.f32_at(c + at::RADIUS)),
        category: ee.u32_at(c + at::CATEGORY),
        collides_with: ee.u32_at(c + at::COLLIDES_WITH),
        objects: o,
        more_objects: m,
        categories,
        ..Collidable::default()
    }
}

fn quad_at(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

fn bits(v: Vf) -> [u32; 4] {
    v.0.map(f32::to_bits)
}

fn words(ee: &Ee, a: u32) -> [u32; 4] {
    [0, 4, 8, 12].map(|o| ee.u32_at(a + o))
}

/// The trigger probe's four rays as the quad hands them over (left front, right front, left rear,
/// right rear: `x` below zero the left, `z` above it the front), out of the image's tree order, and
/// the port's tree of them.
fn image_rays(ee: &Ee) -> Rays {
    let n = ee.u32_at(SEARCH + at::RAYS);
    assert_eq!(n, 4, "the trigger probe has a ray a wheel");
    let segs = ee.u32_at(SEARCH + at::SEGMENTS);
    let mut by_wheel = [None; 4];
    for k in 0..n {
        let s = ray_probe::Segment { from: quad_at(ee, segs + 0x20 * k), to: quad_at(ee, segs + 0x20 * k + 0x10) };
        let wheel = match (s.from.x() > 0.0, s.from.z() > 0.0) {
            (false, true) => 0,
            (true, true) => 1,
            (false, false) => 2,
            (true, false) => 3,
        };
        assert!(by_wheel[wheel].replace(s).is_none(), "one ray a wheel");
    }
    game_sim::kiowa::ray_tree(&by_wheel.map(Option::unwrap))
}

/// The rows of every capture with the body's place and the wheels' swept contacts.
fn capture_rows(dir: &Path) -> Option<Vec<Row>> {
    let mut names: Vec<_> = std::fs::read_dir(dir).ok().into_iter().flatten().flatten().map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "csv")).collect();
    names.sort();
    if names.len() < 15 {
        skip(&format!("{} does not hold MD37a's fifteen captures", dir.display()));
        return None;
    }
    let mut out = Vec::new();
    for name in names {
        let text = std::fs::read_to_string(&name).unwrap();
        let mut lines = text.lines();
        let head: Vec<&str> = lines.next().unwrap().split(',').collect();
        let col = |h: &str| head.iter().position(|c| *c == h).unwrap_or_else(|| panic!("{}: no {h}", name.display()));
        let stem = name.file_stem().unwrap().to_string_lossy().into_owned();
        let pos = ["pos.x", "pos.y", "pos.z"].map(col);
        let quat = ["quat.x", "quat.y", "quat.z", "quat.w"].map(col);
        let raw = [0, 1, 2, 3].map(|k| col(&format!("wheel{k}.raw")));
        let frame = col("frame");
        for line in lines {
            let f: Vec<&str> = line.split(',').collect();
            let wheels = raw.map(|c| {
                let b: Vec<u8> = (0..f[c].len() / 2).map(|i| u8::from_str_radix(&f[c][2 * i..2 * i + 2], 16).unwrap()).collect();
                let w = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
                (w(0xd4), [0, 4, 8, 12].map(|k| w(0x120 + k)), [0, 4, 8, 12].map(|k| w(0x130 + k)))
            });
            out.push(Row {
                name: stem.clone(),
                frame: f[frame].parse().unwrap(),
                pos: pos.map(|c| f[c].parse().unwrap()),
                quat: quat.map(|c| f[c].parse().unwrap()),
                wheels,
            });
        }
    }
    Some(out)
}

/// A unit quaternion's rows (row-vector convention, as the body's orientation turns the quad).
fn rows_of(q: [f32; 4]) -> [[f32; 3]; 3] {
    let [x, y, z, w] = q;
    [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + w * z), 2.0 * (x * z - w * y)],
        [2.0 * (x * y - w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + w * x)],
        [2.0 * (x * z + w * y), 2.0 * (y * z - w * x), 1.0 - 2.0 * (x * x + y * y)],
    ]
}

fn frame_of(r: [[f32; 3]; 3], t: [f32; 3]) -> Matrix {
    [
        Vf::new(r[0][0], r[0][1], r[0][2], 0.0),
        Vf::new(r[1][0], r[1][1], r[1][2], 0.0),
        Vf::new(r[2][0], r[2][1], r[2][2], 0.0),
        Vf::new(t[0], t[1], t[2], 1.0),
    ]
}

/// The quad's scene node where the body is: the body's frame, less its centre of gravity (the image's
/// body `+0x1a0`, `(0, 1, 0)`) along its rows.
fn node_of(pos: [f32; 3], quat: [f32; 4]) -> Matrix {
    let r = rows_of(quat);
    frame_of(r, [pos[0] - r[1][0], pos[1] - r[1][1], pos[2] - r[1][2]])
}

/// The image's struck collidable `c` (a record's object word) as an [`Id`].
fn image_id(ee: &Ee, c: u32) -> Id {
    for (m, parts) in image_parts(ee).iter().enumerate() {
        if let Some(k) = parts.iter().position(|&p| p == c) {
            return Id::Part(m, k);
        }
    }
    if ee.u32_at(c + at::VTABLE) == MESH_VTABLE {
        let w = |o: u32| ee.u32_at(c + o);
        Id::Mesh([w(at::CENTRE), w(at::CENTRE + 4), w(at::CENTRE + 8), w(at::RADIUS)])
    } else {
        Id::Other(c)
    }
}

/// A file slot's species in loaded order (the empty slots left out).
fn loaded(eco: &Ecosystem, slot: u8) -> usize {
    eco.species[..usize::from(slot)].iter().flatten().count()
}

/// Each species' part collidables, in loaded order.
fn image_parts(ee: &Ee) -> Vec<Vec<u32>> {
    let eco = ee.u32_at(ECO_AT);
    let models = ee.u32_at(eco + 0x84);
    (0..SPECIES)
        .map(|m| {
            let a = ee.u32_at(models + 4 * m);
            (0..ee.u32_at(a + 0x194)).map(|k| ee.u32_at(ee.u32_at(a + 0x19c) + 4 * k)).collect()
        })
        .collect()
}

fn port_id(w: &World, slots: &[usize], o: u32) -> Id {
    let o = o as usize;
    if let Some(m) = slots.iter().rposition(|&s| s <= o) {
        return Id::Part(m, o - slots[m]);
    }
    if w.meshes.contains(&o) {
        let p = &w.broad.places[o];
        Id::Mesh([p.centre[0].0.to_bits(), p.centre[1].0.to_bits(), p.centre[2].0.to_bits(), w.objects[o].radius.0.to_bits()])
    } else {
        Id::Other(o as u32)
    }
}

/// A record, comparable: the nearest, the low/high/push, the first, the part, the flags, and each
/// contact (its ray, normal, point, what it struck, placed, fraction, ray word).
#[derive(Debug, PartialEq)]
struct Seen {
    nearest: [[u32; 4]; 4],
    spread: [[u32; 4]; 3],
    first: ([u32; 4], u32),
    part: u32,
    near: (u32, u8),
    contacts: Vec<([u32; 4], [u32; 4], [u32; 4], [u32; 4], Id, u32, u32, u16)>,
}

fn image_record(ee: &Ee, r: u32) -> Seen {
    let n = ee.u32_at(r + rec::COUNT);
    let arr = |o: u32| ee.u32_at(r + o);
    for i in 0..ee.u32_at(r + rec::CAPACITY) {
        assert_eq!(ee.u32_at(arr(rec::ZEROED) + 4 * i), 0, "the per-contact words the pair zeroes stay zero");
    }
    Seen {
        nearest: [rec::FROM, rec::TO, rec::NORMAL, rec::POINT].map(|o| words(ee, r + o)),
        spread: [rec::PUSH, rec::LOW, rec::HIGH].map(|o| words(ee, r + o)),
        first: (words(ee, r + rec::FIRST_POINT), ee.u32_at(r + rec::FIRST)),
        part: ee.u32_at(r + rec::PART),
        near: (ee.u32_at(r + rec::NEAREST), ee.u8_at(r + rec::FLAGS)),
        contacts: (0..n)
            .map(|i| {
                (
                    words(ee, arr(rec::RAYS) + 0x20 * i),
                    words(ee, arr(rec::RAYS) + 0x20 * i + 0x10),
                    words(ee, arr(rec::NORMALS) + 0x10 * i),
                    words(ee, arr(rec::POINTS) + 0x10 * i),
                    image_id(ee, ee.u32_at(arr(rec::OBJECTS) + 4 * i)),
                    ee.u32_at(arr(rec::PLACED) + 4 * i),
                    ee.u32_at(arr(rec::FRACTIONS) + 4 * i),
                    ee.u16_at(arr(rec::SEGMENTS) + 2 * i),
                )
            })
            .collect(),
    }
}

fn port_record(w: &World, slots: &[usize], r: &Record, part: u32) -> Seen {
    Seen {
        nearest: [r.from, r.to, r.normal, r.point].map(bits),
        spread: [r.push, r.low, r.high].map(bits),
        first: (bits(r.first_point), r.first.0.to_bits()),
        part,
        near: (r.nearest.0.to_bits(), r.flags),
        contacts: r
            .contacts
            .iter()
            .map(|c| (bits(c.from), bits(c.to), bits(c.normal), bits(c.point), port_id(w, slots, c.object), u32::from(c.placed), c.fraction.0.to_bits(), c.segment))
            .collect(),
    }
}

/// The port's record as the image holds it: the fields the search's clear leaves as they were.
fn record_from_image(ee: &Ee, r: u32) -> Record {
    let mut out = atv_search::record(4);
    out.normal = quad_at(ee, r + rec::NORMAL);
    out.point = quad_at(ee, r + rec::POINT);
    out.first_point = quad_at(ee, r + rec::FIRST_POINT);
    out.flags = ee.u8_at(r + rec::FLAGS);
    out
}

#[test]
fn the_trigger_probe_is_the_images() {
    let Some(d) = data() else { return };
    let ee = &d.ee;
    let p = SEARCH;
    assert_eq!(ee.u32_at(p + at::VTABLE), RAY_VTABLE, "the trigger probe is a ray probe");
    assert_eq!([ee.u32_at(p + at::SMOOTH), ee.u32_at(p + at::FILTER)], [0, 0], "no smooth normals, no per-ray filter");
    assert_eq!([ee.u32_at(p + at::LISTENERS), ee.u32_at(p + at::ON_HIT), ee.u32_at(p + at::ON_STRUCK), ee.u32_at(p)], [0; 4]);
    assert_eq!(ee.u32_at(LIST + 0x40), 0, "the quad's list carries no terrain grid");
    // The tree, node for node and ray for ray.
    assert_eq!(ee.u16_at(p + at::ROOT), d.rays.root, "the root");
    let nodes = ee.u32_at(p + at::NODES);
    assert_eq!(d.rays.nodes.len(), 3);
    for (k, node) in d.rays.nodes.iter().enumerate() {
        let a = nodes + 0x24 * k as u32;
        let got: Vec<u32> = [0, 12, 16, 20, 24, 28, 32].iter().map(|&o| ee.u32_at(a + o)).collect();
        let want: Vec<u32> = [node.volume].iter().chain(&node.centre).chain(&node.half_extent).map(|v| v.0.to_bits()).collect();
        assert_eq!(got, want, "node {k}'s volume, centre and half extent");
        assert_eq!([ee.u16_at(a + 8), ee.u16_at(a + 10)], node.children, "node {k}'s children");
    }
    let segs = ee.u32_at(p + at::SEGMENTS);
    for (k, s) in d.rays.segments.iter().enumerate() {
        let a = segs + 0x20 * k as u32;
        assert_eq!((words(ee, a), words(ee, a + 0x10)), (bits(s.from), bits(s.to)), "ray {k}");
    }
    // The record.
    let r = ee.u32_at(p + at::RECORD);
    let want = atv_search::record(4);
    assert_eq!(ee.u32_at(r + rec::CAPACITY) as usize, want.capacity);
    assert_eq!(ee.u32_at(r + rec::ONE_PER_RAY) != 0, want.one_per_ray);
    assert!(ee.u32_at(r + rec::RAYS) != 0 && want.listed, "the record keeps a contact list");
    // The probe stood where the image has it: the centre and the reach.
    let t: Matrix = [0, 1, 2, 3].map(|i| quad_at(ee, p + at::TRANSFORM + 0x10 * i));
    let (c, radius) = atv_search::reach(&d.rays, &t);
    assert_eq!(bits(c)[..3], words(ee, p + at::CENTRE)[..3], "the probe's centre");
    assert_eq!(radius.0.to_bits(), ee.u32_at(p + at::RADIUS), "the probe's reach");
    // The species' parts: a sphere where the port has none, a cylinder where it built a trunk — and
    // the cylinder's tree, node for node, the port's. Each part is stood at the plant the search
    // asks for, at the plant's cell; the cells the image holds are the file's.
    let w = &d.world;
    let image = image_parts(ee);
    let solids: Vec<&eco::Species> = d.eco.species.iter().flatten().collect();
    assert_eq!((image.len(), d.slots.len()), (solids.len(), solids.len()), "the species");
    let mut cylinders = 0;
    for (m, parts) in image.iter().enumerate() {
        assert_eq!(parts.len(), solids[m].solids.len(), "species {m}'s part count");
        // A plant of the species, for its trunks.
        let g = d.trees.groups().iter().find(|g| loaded(&d.eco, g.model) == m);
        for (k, &c) in parts.iter().enumerate() {
            let kind = ee.u32_at(c + at::KIND);
            let sphere = solids[m].solids[k].kind == eco::SPHERE;
            assert_eq!((kind == 5, ee.u32_at(c + at::CATEGORY)), (sphere, if sphere { 0x80 } else { 0x40 }), "species {m}'s part {k}");
            assert_eq!(
                [ee.u32_at(c), ee.u32_at(c + at::LISTENERS), ee.u32_at(c + at::ON_STRUCK), ee.u32_at(c + at::OWNER), ee.u32_at(c + at::VEHICLE)],
                [0; 5],
                "species {m}'s part {k}: not retired, nobody listens, no body, no vehicle"
            );
            if sphere {
                continue;
            }
            let Some(j) = g.and_then(|g| g.parts[k]) else { panic!("species {m}'s part {k}: no trunk built") };
            let mesh = &d.trees.props.props[j].mesh;
            assert_eq!(ee.u16_at(c + at::MESH_ROOT), 0, "species {m}'s part {k}'s tree starts at node 0");
            let nodes = ee.u32_at(c + at::MESH_NODES);
            for (n, node) in mesh.nodes.iter().enumerate() {
                let a = nodes + 0x24 * n as u32;
                let want: Vec<u32> = [0, 8, 0xc, 0x10, 0x14, 0x18, 0x1c, 0x20].iter().map(|&o| ee.u32_at(a + o)).collect();
                let mut got = vec![node.volume.0.to_bits(), u32::from(node.children[0]) | u32::from(node.children[1]) << 16];
                got.extend(node.centre.iter().chain(&node.half_extent).map(|v| v.0.to_bits()));
                assert_eq!(got, want, "species {m}'s part {k}'s node {n}");
            }
            cylinders += 1;
        }
    }
    assert!(cylinders > 0, "MC02 has cylinders");
    // MC02's cylinders are rings with no height (the file's height is 0, the image's trees are flat):
    // no ray strikes one, here or in the original.
    let flat = solids.iter().flat_map(|s| &s.solids).filter(|s| s.kind == eco::CYLINDER).all(|s| s.height == F::ZERO);
    assert!(flat, "MC02's cylinders have no height");
    let mut checked = 0;
    for_each_image_mesh(ee, &mut |c| {
        let id = image_id(ee, c);
        let o = w.meshes.clone().find(|&o| port_id(w, &d.slots, o as u32) == id).expect("every image mesh is a port mesh");
        let prop = &d.props[o - w.meshes.start];
        assert_eq!(ee.u32_at(c + at::KIND), 0, "mesh {c:#x} is a plain mesh");
        assert_eq!(ee.u16_at(c + at::MESH_ROOT), 0, "mesh {c:#x}'s tree starts at node 0");
        assert_eq!(
            [ee.u32_at(c), ee.u32_at(c + at::LISTENERS), ee.u32_at(c + at::ON_STRUCK), ee.u32_at(c + at::OWNER), ee.u32_at(c + at::VEHICLE)],
            [0; 5],
            "mesh {c:#x}: not retired, nobody listens, no body, no vehicle"
        );
        let t: Vec<[u32; 4]> = (0..4).map(|i| words(ee, c + at::TRANSFORM + 0x10 * i)).collect();
        assert_eq!(t, prop.to_world.0.map(bits).to_vec(), "mesh {c:#x}'s frame");
        checked += 1;
    });
    assert_eq!(checked, w.meshes.len(), "every port mesh is one of the image's");
    println!(
        "the trigger probe: {} rays, {} nodes, word for word; {checked} meshes' frames; {cylinders} cylinders' trees",
        d.rays.segments.len(),
        d.rays.nodes.len()
    );
}

/// Every scene mesh in the image's tree, once.
fn for_each_image_mesh(ee: &Ee, f: &mut dyn FnMut(u32)) {
    let tree = 0x0065_47c0;
    let mut seen = std::collections::BTreeSet::new();
    let mut stack = vec![ee.u32_at(tree + 0x8c)];
    while let Some(node) = stack.pop() {
        let mut chunk = ee.u32_at(node + 4);
        while chunk != 0 {
            for s in 0..4 {
                let e = ee.u32_at(chunk + 4 + 4 * s);
                if e != 0 && ee.u8_at(e + 2) == 2 {
                    let c = e - at::ENTRY;
                    if ee.u32_at(c + at::VTABLE) == MESH_VTABLE && ee.u32_at(c + at::CATEGORY) == atv_world::MESH && seen.insert(c) {
                        f(c);
                    }
                }
            }
            chunk = ee.u32_at(chunk);
        }
        let children = ee.u32_at(node);
        if children != 0 {
            for i in 0..4 {
                let c = ee.u32_at(children + 4 * i);
                if c != 0 {
                    stack.push(c);
                }
            }
        }
    }
}

/// A deterministic generator: inputs only.
struct Gen(u64);

impl Gen {
    fn unit(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
    fn signed(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }
}

/// What one search came to on both sides.
struct Outcome {
    want: Seen,
    got: Seen,
    hit: (bool, bool),
    /// The original's list held a placed group.
    groups: bool,
}

/// The original and the port, the probe stood at `t` and the list gathered round its node.
fn run(d: &mut Data, record: &mut Record, t: &Matrix) -> Outcome {
    let ee = &mut d.ee;
    for (i, row) in t.iter().enumerate() {
        for k in 0..4 {
            ee.set_f32(SEARCH + at::TRANSFORM + 0x10 * i as u32 + 4 * k, row.0[k as usize]);
        }
    }
    ee.call(STAND, &[u64::from(SEARCH), u64::from(SEARCH + at::TRANSFORM)], &[], 1_000_000);
    for k in 0..3 {
        ee.set_f32(PROBE_AT + at::CENTRE + 4 * k, t[3].0[k as usize]);
    }
    ee.call(GATHER, &[u64::from(LIST), u64::from(PROBE_AT), 3], &[], 50_000_000);
    let groups = (0..ee.u32_at(LIST + 4)).any(|i| ee.u8_at(ee.u32_at(ee.u32_at(LIST) + 8 * i) + 2) == 3);
    ee.call(SEARCH_FN, &[u64::from(SEARCH), u64::from(LIST), 1], &[], 200_000_000);
    let want_hit = ee.v0() != 0;
    assert_eq!(want_hit, ee.u8_at(SEARCH + at::FLAGS) & 0x20 != 0);
    let r = ee.u32_at(SEARCH + at::RECORD);
    let want = image_record(ee, r);
    let want_part = ee.u32_at(r + rec::PART);

    let w = &mut d.world;
    let mut list = Nearby::quad();
    w.place_probe([t[3].x(), t[3].y(), t[3].z()].map(F));
    atv_world::gather(&mut w.objects, &mut w.broad, &mut list, PROBE, 3);
    let plants = Plants::new(&d.trees, &d.eco, w, d.slots[0]);
    let mut wheels = Wheels::new(&d.rays, *t, &d.props, w.meshes.start, plants);
    wheels.record = std::mem::take(record);
    let hit = atv_search::collide(&mut w.objects, d.search, &list.found, &mut wheels);
    assert_eq!(w.objects[d.search].radius.0.to_bits(), d.ee.u32_at(SEARCH + at::RADIUS), "the probe's reach");
    *record = wheels.record;
    // The record's part is only written by a hierarchy's pair, which Desert Oasis has none of.
    let got = port_record(&d.world, &d.slots, record, want_part);
    Outcome { want, got, hit: (want_hit, hit), groups }
}

#[test]
fn the_quads_search_is_the_originals() {
    let Some(mut d) = data() else { return };
    let mut record = record_from_image(&d.ee, d.ee.u32_at(SEARCH + at::RECORD));
    let mut g = Gen(0x4d44_3736_6132);
    // Frames that put one wheel's ray across a mesh: a triangle's centre, a wheel picked, the probe
    // moved so the ray's middle is there (give or take 0.6 ft), turned any way about up and tipped a
    // little — and one in eight tipped anywhere. Forty at each scene mesh, and four at each of 300
    // trunks (prickly pears and weeds, spread over the file) tipped anywhere.
    let mut frames: Vec<(String, Matrix)> = Vec::new();
    let aim = |g: &mut Gen, p: &Prop, i: usize| -> Matrix {
        let verts = game_sim::collide::MeshRef::of(&p.mesh);
        let tris = &p.mesh.triangles;
        let tri = &tris[(g.unit() * tris.len() as f32) as usize % tris.len()];
        let m = p.to_world.0;
        let world: Vec<[f32; 3]> = tri
            .vertices
            .map(|v| verts.vertex(v))
            .iter()
            .map(|v| [0, 1, 2].map(|k| v.x() * m[0].0[k] + v.y() * m[1].0[k] + v.z() * m[2].0[k] + m[3].0[k]))
            .collect();
        let target = [0, 1, 2].map(|k| (world[0][k] + world[1][k] + world[2][k]) / 3.0 + g.signed() * 0.6);
        let tilt = if i % 8 == 7 { 3.0 } else { 0.35 };
        let (yaw, pitch, roll) = (g.unit() * std::f32::consts::TAU, g.signed() * tilt, g.signed() * tilt);
        let r = rows_of(euler_quat(yaw, pitch, roll));
        let s = d.rays.segments[(g.unit() * 4.0) as usize % 4];
        let mid = [0, 1, 2].map(|k| (s.from.0[k] + s.to.0[k]) * 0.5);
        frame_of(r, [0, 1, 2].map(|k| target[k] - (mid[0] * r[0][k] + mid[1] * r[1][k] + mid[2] * r[2][k])))
    };
    for (o, p) in d.props.iter().enumerate() {
        for i in 0..40 {
            frames.push((format!("mesh {o} #{i}"), aim(&mut g, p, i)));
        }
    }
    let trunks = &d.trees.props.props;
    for n in 0..300.min(trunks.len()) {
        let j = n * trunks.len() / 300.min(trunks.len());
        // A wheel's ray runs along a trunk's side stood up: these are tipped anywhere.
        for i in 0..4 {
            frames.push((format!("trunk {j} #{i}"), aim(&mut g, &trunks[j], 7)));
        }
    }
    // And the image's own frame, at the spawn.
    frames.push(("spawn".into(), [0, 1, 2, 3].map(|i| quad_at(&d.ee, SEARCH + at::TRANSFORM + 0x10 * i))));

    let (mut hits, mut contacts, mut with_groups, mut hit_only) = (0, 0, 0, 0);
    let mut per_mesh: BTreeMap<Id, usize> = BTreeMap::new();
    let mut by_count = [0usize; 13];
    for (name, t) in &frames {
        let o = run(&mut d, &mut record, t);
        assert_eq!(o.got, o.want, "{name}: the record");
        if o.hit.0 != o.hit.1 {
            // Only a sphere the port does not test can tell them apart: no contact, a group in the list.
            assert!(o.hit.0 && o.want.contacts.is_empty() && o.groups, "{name}: the hit bit {:?}", o.hit);
            hit_only += 1;
        }
        hits += usize::from(o.hit.1);
        contacts += o.want.contacts.len();
        with_groups += usize::from(o.groups);
        by_count[o.want.contacts.len()] += 1;
        for c in &o.want.contacts {
            *per_mesh.entry(c.4).or_insert(0) += 1;
        }
    }
    // MC02's trunks are flat (see the probe's test): the frames at them strike nothing on either side.
    let on_parts: usize = per_mesh.iter().filter(|(id, _)| matches!(id, Id::Part(..))).map(|(_, n)| n).sum();
    let meshes = per_mesh.keys().filter(|id| matches!(id, Id::Mesh(_))).count();
    println!(
        "{} frames: {hits} struck, {contacts} contacts ({on_parts} on trunks) on {meshes} meshes, by count {by_count:?}; \
         {with_groups} lists held plants; {hit_only} struck only a plant's sphere",
        frames.len(),
    );
    assert!(contacts > 1000 && meshes == d.props.len(), "the rays struck every mesh");
}

/// A quaternion turning by `yaw` about up, then `pitch` about across and `roll` about along.
fn euler_quat(yaw: f32, pitch: f32, roll: f32) -> [f32; 4] {
    let (sy, cy) = (yaw * 0.5).sin_cos();
    let (sp, cp) = (pitch * 0.5).sin_cos();
    let (sr, cr) = (roll * 0.5).sin_cos();
    let mul = |a: [f32; 4], b: [f32; 4]| {
        [
            a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
            a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
            a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
            a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
        ]
    };
    mul(mul([0.0, sy, 0.0, cy], [sp, 0.0, 0.0, cp]), [0.0, 0.0, sr, cr])
}

#[test]
fn the_captured_swept_contacts_are_the_searchs() {
    let Some(mut d) = data() else { return };
    let mut record = record_from_image(&d.ee, d.ee.u32_at(SEARCH + at::RECORD));
    let rows = d.rows.clone();
    let (mut checked, mut reproduced, mut searched) = (0, 0, 0);
    for (k, row) in rows.iter().enumerate() {
        if row.wheels.iter().all(|w| w.0 == 0) {
            continue;
        }
        let before = &rows[k - 1];
        assert_eq!(before.name, row.name);
        let mut best: Option<(f32, String)> = None;
        // The rows are two frames apart and the search runs inside the frame between: the node
        // where the row before left the body, eighths of the way to this row's, and this row's (the
        // turn the quaternions' straight-line blend, normalised).
        let blend = |a: &Row, b: &Row, f: f32| {
            let dot: f32 = (0..4).map(|j| a.quat[j] * b.quat[j]).sum();
            let sign = if dot < 0.0 { -1.0 } else { 1.0 };
            let q = [0, 1, 2, 3].map(|j| a.quat[j] + (sign * b.quat[j] - a.quat[j]) * f);
            let n = q.iter().map(|v| v * v).sum::<f32>().sqrt();
            node_of([0, 1, 2].map(|j| a.pos[j] + (b.pos[j] - a.pos[j]) * f), q.map(|v| v / n))
        };
        let earlier = &rows[k.saturating_sub(2)];
        let mut frames: Vec<(String, Matrix)> = Vec::new();
        if earlier.name == row.name && k >= 2 {
            frames.extend((0..8).map(|e| (format!("{e}/8 of the way from the row two back"), blend(earlier, before, e as f32 / 8.0))));
        }
        frames.extend((0..=8).map(|e| (format!("{e}/8 of the way from the row before"), blend(before, row, e as f32 / 8.0))));
        for (when, t) in frames {
            let o = run(&mut d, &mut record, &t);
            assert_eq!(o.got, o.want, "{}@{} at {when}: the record", row.name, row.frame);
            searched += 1;
            let cs = atv_search::contacts(&record);
            let mut wheels = [WheelContact::default(); 4];
            let mut rear = 0;
            quad::wheel_contacts(&mut wheels, &mut rear, o.hit.1, &cs);
            let mut far = 0.0f32;
            let mut text = format!("{when}:");
            for (i, (w, c)) in wheels.iter().zip(&row.wheels).enumerate() {
                let set = c.0 != 0;
                if (w.touching != 0) != set {
                    far = f32::INFINITY;
                    text += &format!(" wheel {i} touching {} (captured {set})", w.touching);
                    continue;
                }
                if !set {
                    continue;
                }
                let gap = (0..3).map(|j| (w.point.0[j] - f32::from_bits(c.1[j])).powi(2)).sum::<f32>().sqrt();
                let same_face = bits(w.normal) == c.2;
                far = far.max(if same_face { gap } else { f32::INFINITY });
                text += &format!(" wheel {i} {gap:.4} ft off, normal {}", if same_face { "the same" } else { "differs" });
            }
            if std::env::var_os("ATV_SEARCH_ROWS").is_some() {
                println!("{}@{} {text}", row.name, row.frame);
            }
            if best.as_ref().is_none_or(|b| far < b.0) {
                best = Some((far, text));
            }
        }
        let (far, text) = best.unwrap();
        if far < 0.25 {
            println!("{}@{} reproduced at {text}", row.name, row.frame);
            reproduced += 1;
        } else {
            println!("{}@{} not reproduced; nearest {text}", row.name, row.frame);
        }
        checked += 1;
    }
    println!("{checked} captured rows with a swept contact, {} frames searched on both sides, {reproduced} rows reproduced", searched);
    assert!(checked >= 8, "the palm rows of race-jump and race-wheelie-launch");
    // The capture does not say which frame between its rows the search ran at; the rows where no
    // blended frame gives the contact are in the journal's not-verified list.
    assert!(reproduced >= 3, "the captured contacts reproduced");
}
