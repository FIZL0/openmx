//! The put-back's mesh query (S83), against the original's own `put_back_solid` on S66's bike state
//! `context/s66/free06.eeMemory.bin` and the disc. S66 named it free06, but its world is free09's: every
//! one of free09's props stands in it on the port's matrix, and every plant on the port's cell.
//!
//! The original sweeps the bundle against the bike's own gathered search list, gathered again only
//! when the bundle has moved ten feet; the port's [`back_search::query`] gathers over the plane's
//! broad-phase world of free09's props and plants. Two proofs, each over generated bundles walked
//! about the track (short steps that keep the list, jumps that gather it again):
//!
//! 1. **the query** — every bundle asked of both: the refusal, every object that met the bundle in the
//!    order it was swept (which prop, probe 0's point and normal, every probe's flag), the query's own
//!    highest point and its normal, the move it measured, where it gathered, and the list it kept, bit
//!    for bit. The state's world also holds what the port's has not got — the toys, the bikes, the
//!    hierarchies: their entries are left out of the lists compared, and the hooked sweep answers
//!    "nothing met" for them, so the original asks its query of the port's world.
//! 2. **the race** — the same with the bike in a race of made-up checkpoints, and the race object
//!    asking for a static-only gather or not, case by case.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use game_sim::archive::{Psr, Res};
use game_sim::back_search::{self, Cache, Checkpoint, Race, Scene};
use game_sim::collide::{Bundle, MAX_PROBES, Probe, Record};
use game_sim::eco::Trees;
use game_sim::flight_search::{self, FIRST};
use game_sim::track::{self, Terrain};
use game_sim::world::{Ground, Props};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

const TRACK: &str = "Tracks\\free09.res";
/// The bike in S66's "free06" state, as its captures' header names it.
const BIKE: u32 = 0x00da_d810;

/// The bike's bundle, search list, the query's cache, its race object and its mode object.
const BIKE_BUNDLE: u32 = 0x1484;
const BIKE_LIST: u32 = 0x1488;
const BIKE_SEARCH: u32 = 0x148c;
const BIKE_AT: u32 = 0x1390;
const BIKE_MOVED: u32 = 0x13a8;
const BIKE_RACE: u32 = 0xf9c;
const BIKE_MODE: u32 = 0xfa0;

/// The bundle's fields (S66): probe count, the three arrays, centre and radius, and its space.
mod bundle_at {
    pub const COUNT: u32 = 0x120;
    pub const LENGTHS: u32 = 0x134;
    pub const ORIGINS: u32 = 0x138;
    pub const DIRS: u32 = 0x13c;
    pub const CENTRE: u32 = 0x150;
    pub const RADIUS: u32 = 0x160;
    pub const SPACE: u32 = 0x04;
}

/// A collidable's fields.
mod at {
    pub const CLASS: u32 = 0x02;
    pub const CENTRE: u32 = 0x40;
    pub const RADIUS: u32 = 0x60;
    pub const CATEGORY: u32 = 0x84;
    pub const MATRIX: u32 = 0x120;
}

/// A placed group's cell and size bytes.
mod group_at {
    pub const CELL: u32 = 0x08;
    pub const HEIGHT: u32 = 0x10;
    pub const WIDTH: u32 = 0x11;
}

/// The race object's static-only word; the mode object's mode, flags and checkpoint list; a
/// checkpoint's centre, radius and next.
const RACE_STATIC: u32 = 0x40;
const MODE: u32 = 0x64;
const MODE_FLAGS: u32 = 0x5c;
const MODE_CHECKPOINTS: u32 = 0x70;
const CP_X: u32 = 0x40;
const CP_Z: u32 = 0x48;
const CP_RADIUS: u32 = 0x70;
const CP_NEXT: u32 = 0x8c;

/// The search list's fields.
const LIST_FOUND: u32 = 0x00;
const LIST_COUNT: u32 = 0x04;
const LIST_BUILT: u32 = 0x30;

const CASES: usize = 12_000;
const RACE_CASES: usize = 10_000;

/// A space link's table; a table's static tree; the tree's root.
const TABLE: u32 = 0x04;
const FIXED: u32 = 0x34;
const ROOT: u32 = 0x94;

fn state() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("s66").join(format!("free06.ee{}", "Memory.bin"));
    if !path.exists() {
        skip(&format!("{} is not there: copy S66's context/s66 from the main checkout", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

fn by_suffix<'a>(res: &'a Res, want: &str) -> Option<&'a game_sim::archive::Entry> {
    let want = want.to_ascii_lowercase();
    res.entries.iter().find(|e| {
        let name = e.name.to_ascii_lowercase();
        name == want || name.ends_with(&format!("\\{want}"))
    })
}

/// free09's props, its field's box on the ground plane, and its plants on the state's curve table.
fn free09(ee: &Ee) -> Option<(Props, ([F; 2], [F; 2]), Trees)> {
    let kit = Kit::load().ok()?;
    let at = kit.ctx("iso").join("MX.PSR");
    let Ok(mut file) = std::fs::File::open(&at) else {
        skip(&format!("no {} — `./kit extract MX.PSR`", at.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    std::io::Read::read_exact(&mut file, &mut front).expect("the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let mut member = |name: &str| -> Option<Vec<u8>> {
        let e = psr.find(name)?;
        std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(e.at)).expect("seek");
        let mut bytes = vec![0u8; e.size as usize];
        std::io::Read::read_exact(&mut file, &mut bytes).expect("read");
        Some(bytes)
    };
    let bytes = member(TRACK)?;
    let res = Res::parse(&bytes).ok()?;
    let read = |suffix: &str| {
        let e = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(suffix)).unwrap_or_else(|| panic!("free09 has no {suffix}"));
        res.read(&bytes, e).expect("a free09 member")
    };
    let scene = track::Scene::parse(&read(".scn")).expect("free09's scene");
    let terrain_bytes = read(&scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn"));
    let terrain = Terrain::parse(&terrain_bytes).expect("free09's terrain");
    let ground = Ground::new(&terrain, &scene.terrain);
    let field = ground.field();
    let bounds = ([F(field.min.x()), F(field.min.z())], [F(field.max.x()), F(field.max.z())]);

    let mut eco = game_sim::eco::Ecosystem::parse(&read(".esb")).expect("free09.esb parses");
    let layout = &scene.terrain;
    let extent = F::from(layout.width.max(layout.breadth) as i32 * game_sim::world::SAMPLES_PER_WIDTH as i32) * terrain.scale;
    eco.stand(extent, |p| F(game_sim::contact_search::ground(&field, p).point.y()));
    let table = ee.u32_at(addr("curve_table_ptr").unwrap());
    let entries: Vec<[f32; 2]> = (0..game_sim::camera::CURVE_ENTRIES as u32)
        .map(|i| [f32::from_bits(ee.u32_at(table + 8 * i)), f32::from_bits(ee.u32_at(table + 8 * i + 4))])
        .collect();
    let curve = game_sim::camera::CurveTable::from_entries(&entries);
    let (trees, missed) = Trees::new(&eco, game_sim::eco::cell_size(extent), &curve);
    assert!(missed.is_empty(), "free09's solids the port does not build: {missed:?}");

    let second = member(&TRACK.replace(".res", "_2.res"));
    let mut parts = vec![(bytes.clone(), res)];
    if let Some(b) = second
        && let Ok(r) = Res::parse(&b)
    {
        parts.push((b, r));
    }
    let (props, missing) = Props::load(&scene, |want| {
        parts
            .iter()
            .find_map(|(bytes, res)| by_suffix(res, want).and_then(|e| res.read(bytes, e).ok()))
            .ok_or_else(|| "no archive of this track has it".to_string())
    });
    // A hierarchy (free09's utility shack) is not in the port's world; the hooked sweep answers
    // nothing for it.
    let missing: Vec<_> = missing.into_iter().filter(|m| !m.contains("Hierarchy")).collect();
    assert!(missing.is_empty(), "the track's props the archives lack: {missing:?}");
    Some((props, bounds, trees))
}

/// The original's tree's slots, every node's.
fn tree_slots(ee: &Ee, root: u32) -> Vec<u32> {
    fn walk(ee: &Ee, node: u32, out: &mut Vec<u32>) {
        let mut c = ee.u32_at(node + 4);
        while c != 0 {
            out.extend((1..=4).map(|k| ee.u32_at(c + 4 * k)).filter(|&o| o != 0));
            c = ee.u32_at(c);
        }
        let children = ee.u32_at(node);
        if children != 0 {
            for i in 0..4 {
                let k = ee.u32_at(children + 4 * i);
                if k != 0 {
                    walk(ee, k, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(ee, root, &mut out);
    out
}

fn quad_at(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

fn set_quad(ee: &mut Ee, a: u32, v: Vf) {
    for k in 0..4 {
        ee.set_u32(a + 4 * k, v[k as usize].to_bits());
    }
}

fn bits(v: Vf) -> [u32; 4] {
    [0, 1, 2, 3].map(|k| v[k].to_bits())
}

/// The state's scene meshes and plants by their port collidable: a mesh by its place and reach, a
/// plant by its cell and size bytes (the first not yet named, where two stand on one spot). Each
/// mesh's matrix is the port prop's, word for word.
fn index(ee: &Ee, props: &Props, trees: &Trees, field: ([F; 2], [F; 2])) -> HashMap<u32, usize> {
    let (broad, objects) = flight_search::world_wooded(&props.props, Some(trees), None, Some(field), Vf::new(0.0, 0.0, 0.0, 1.0), F::ZERO);
    let groups = FIRST + props.props.len();
    let mut by_place: HashMap<([u32; 4], u32), usize> = HashMap::new();
    for i in FIRST..groups {
        let c = broad.places[i].centre.map(F::to_bits);
        assert!(by_place.insert((c, objects[i].radius.to_bits()), i).is_none(), "two props stand at one place");
    }
    let mut plants: HashMap<(u16, u16, i8, i8), Vec<usize>> = HashMap::new();
    for (g, p) in trees.groups().iter().enumerate() {
        plants.entry((p.cell[0], p.cell[2], p.height, p.blend)).or_default().push(groups + g);
    }
    let bundle = ee.u32_at(BIKE + BIKE_BUNDLE);
    let table = ee.u32_at(ee.u32_at(bundle + bundle_at::SPACE) + TABLE);
    let tree = ee.u32_at(table + FIXED);
    let mut index = HashMap::new();
    let (mut taken, mut extra) = (std::collections::HashSet::new(), std::collections::HashSet::new());
    let mut matrices = 0;
    for o in tree_slots(ee, ee.u32_at(tree + ROOT)) {
        if index.contains_key(&o) {
            continue;
        }
        match ee.u8_at(o + at::CLASS) {
            2 if ee.u32_at(o + at::CATEGORY) == flight_search::SCENERY => {
                let key = ([0, 4, 8, 12].map(|k| ee.u32_at(o + at::CENTRE + k)), ee.u32_at(o + at::RADIUS));
                let Some(&i) = by_place.get(&key) else {
                    if extra.insert(o) {
                        eprintln!("the state's mesh {o:#x} at {:?} is none of the port's props: left out", quad_at(ee, o + at::CENTRE));
                    }
                    continue;
                };
                let ours = &props.props[i - FIRST].to_world;
                for r in 0..4 {
                    assert_eq!(bits(quad_at(ee, o + at::MATRIX + 16 * r)), bits(ours.0[r as usize]), "{o:#x}'s matrix row {r}");
                }
                matrices += 1;
                index.insert(o, i);
            }
            3 => {
                let cell = [0, 2, 4].map(|k| ee.u16_at(o + group_at::CELL + k));
                let key = (cell[0], cell[2], ee.u8_at(o + group_at::HEIGHT) as i8, ee.u8_at(o + group_at::WIDTH) as i8);
                let found = plants.get(&key).unwrap_or_else(|| panic!("the state's group {o:#x} at {key:?} is none of the port's plants"));
                let g = *found.iter().find(|g| !taken.contains(*g)).unwrap_or_else(|| panic!("{o:#x}: every plant at {key:?} is taken"));
                taken.insert(g);
                index.insert(o, g);
            }
            _ => {}
        }
    }
    assert_eq!(matrices, props.props.len(), "the state's scene meshes and free09's props");
    assert!(extra.len() <= 1, "the state's meshes the port has not got: {extra:x?}");
    assert_eq!(taken.len(), trees.groups().len(), "the state's groups and free09's plants");
    index
}

struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * ((self.next() >> 40) as f32 / (1u64 << 24) as f32)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn unit(&mut self) -> Vf {
        loop {
            let v = [self.range(-1.0, 1.0), self.range(-1.0, 1.0), self.range(-1.0, 1.0)];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if l > 0.1 && l <= 1.0 {
                return Vf::new(v[0] / l, v[1] / l, v[2] / l, 0.0);
            }
        }
    }
}

/// What one object that met the bundle left: which collidable, probe 0's point and normal, every
/// probe's flag.
type Answer = (usize, [u32; 4], [u32; 4], Vec<bool>);

/// The original's query, its hooked sweep, and what it says about the port's collidables.
struct Original {
    ee: Ee,
    solid: u32,
    sweep: u32,
    index: Rc<HashMap<u32, usize>>,
}

/// What the original's query did with a bundle.
#[derive(Debug, PartialEq)]
struct Asked {
    refused: bool,
    answers: Vec<Answer>,
    y: Option<(u32, [u32; 4])>,
    moved: u32,
    at: [u32; 4],
    list: Vec<(usize, u32)>,
}

impl Original {
    fn ask(&mut self, bundle: &Bundle) -> (Asked, usize) {
        let ee = &mut self.ee;
        let b = ee.u32_at(BIKE + BIKE_BUNDLE);
        ee.set_u32(b + bundle_at::COUNT, bundle.count as u32);
        let (lengths, origins, dirs) = (ee.u32_at(b + bundle_at::LENGTHS), ee.u32_at(b + bundle_at::ORIGINS), ee.u32_at(b + bundle_at::DIRS));
        for (i, p) in bundle.probes[..bundle.count].iter().enumerate() {
            let i = i as u32;
            set_quad(ee, origins + 16 * i, p.origin);
            set_quad(ee, dirs + 16 * i, p.dir);
            ee.set_u32(lengths + 4 * i, p.length.0.to_bits());
        }
        set_quad(ee, b + bundle_at::CENTRE, bundle.centre);
        ee.set_u32(b + bundle_at::RADIUS, bundle.radius.0.to_bits());

        let answers: Rc<RefCell<Vec<Answer>>> = Rc::default();
        let (sweep, heard, index) = (self.sweep, answers.clone(), self.index.clone());
        ee.hooks.insert(
            sweep,
            Box::new(move |ee: &mut Ee| {
                let (args, ra, sp, stack) = ([ee.r(4), ee.r(5), ee.r(6)], ee.r(31), ee.r(29), ee.stack);
                // What the port's world has not got met nothing.
                let met = match index.get(&(args[1] as u32)) {
                    Some(_) => {
                        ee.stack = (sp as u32 - 0x40) & !0xf;
                        ee.call(sweep, &args, &[], 100_000_000);
                        ee.v0()
                    }
                    None => 0,
                };
                (ee.stack, ee.pc) = (stack, sweep);
                ee.set(29, sp);
                ee.set(31, ra);
                ee.set(2, met);
                if met != 0 {
                    let (bundle, record) = (args[0] as u32, args[2] as u32);
                    let flags = ee.u32_at(record + 0xa4);
                    let hit = (0..ee.u32_at(bundle + bundle_at::COUNT)).map(|k| ee.u32_at(flags + 4 * k) != 0).collect();
                    let o = index[&(args[1] as u32)];
                    heard.borrow_mut().push((o, bits(quad_at(ee, ee.u32_at(record + 0x98))), bits(quad_at(ee, ee.u32_at(record + 0x94))), hit));
                }
            }),
        );
        let (y, normal) = (ee.scratch(2), ee.scratch(3));
        ee.set_f32(y, -1.0e30);
        set_quad(ee, normal, Vf::ZERO);
        let args = [BIKE, ee.u32_at(BIKE + BIKE_SEARCH), 0, b, normal, y].map(u64::from);
        ee.call(self.solid, &args, &[], 2_000_000_000);
        ee.hooks.remove(&sweep);
        let refused = ee.v0() != 0;
        let answers = answers.take();
        let y = (!answers.is_empty()).then(|| (ee.f32_at(y).to_bits(), bits(quad_at(ee, normal))));

        let list = ee.u32_at(BIKE + BIKE_LIST);
        let (n, found) = (ee.i32_at(list + LIST_COUNT).max(0) as u32, ee.u32_at(list + LIST_FOUND));
        let entries: Vec<(usize, u32)> = (0..n)
            .filter_map(|k| self.index.get(&ee.u32_at(found + 8 * k)).map(|&o| (o, ee.u32_at(found + 8 * k + 4))))
            .collect();
        let asked = Asked {
            refused,
            answers,
            y,
            moved: ee.u32_at(BIKE + BIKE_MOVED),
            at: bits(quad_at(ee, BIKE + BIKE_AT)),
            list: entries,
        };
        (asked, n as usize)
    }
}

/// The port's answer to the same bundle, in [`Asked`]'s terms.
fn port(scene: Scene<'_>, bundle: &Bundle, cache: &mut Cache, race: Race<'_>) -> Asked {
    let mut answers = Vec::new();
    let mut found = Record::new();
    let refused = back_search::query_each(scene, bundle, cache, race, &mut found, |(i, r)| {
        answers.push((FIRST + i, bits(r.point[0]), bits(r.normal[0]), r.hit[..bundle.count].to_vec()));
    });
    let y = found.hit[0].then(|| (found.point[0].y().to_bits(), bits(found.normal[0])));
    Asked {
        refused,
        answers,
        y,
        moved: cache.moved.to_bits(),
        at: bits(cache.at),
        list: cache.list.nearby.found.iter().map(|c| (c.object, c.distance.to_bits())).collect(),
    }
}

/// Where the walk goes: the props' and plants' centres.
fn targets(props: &Props, trees: &Trees) -> Vec<Vf> {
    let mut out: Vec<Vf> = props.props.iter().map(|p| {
        let (c, _) = flight_search::prop_box(p);
        Vf::new(c[0].0, c[1].0, c[2].0, 1.0)
    }).collect();
    for g in trees.groups() {
        let (c, _) = g.bounds(trees.cell());
        out.push(Vf::new(c[0].0, c[1].0, c[2].0, 1.0));
    }
    out
}

/// One generated bundle about `centre`, aimed at `target` some of the time: the overhead test's one
/// probe a thousand feet wide, the clearance's ring, none at all, or a handful anywhere.
fn bundle(pick: &mut Pick, centre: Vf, target: Vf) -> Bundle {
    let mut b = Bundle { centre, ..Bundle::default() };
    match pick.below(4) {
        0 => {
            b.count = 1;
            b.radius = F(1000.0);
            b.probes[0] = Probe {
                origin: Vf::new(centre.x(), centre.y() + 10.0, centre.z(), 1.0),
                dir: Vf::new(0.0, -1.0, 0.0, 0.0),
                length: F(pick.range(5.0, 120.0)),
            };
        }
        1 => {
            b.count = 14;
            b.radius = F(40.0);
            b.probes[0] = Probe { origin: Vf::new(centre.x(), centre.y() + 10.0, centre.z(), 1.0), dir: Vf::new(0.0, -1.0, 0.0, 0.0), length: F(20.0) };
            let turn = pick.range(0.0, 6.283);
            for i in 1..13 {
                let a = turn + i as f32 * 0.5236;
                b.probes[i] = Probe { origin: Vf::new(centre.x(), centre.y() + 3.0, centre.z(), 1.0), dir: Vf::new(a.sin(), 0.0, a.cos(), 0.0), length: F(20.0) };
            }
            let (s, c) = turn.sin_cos();
            b.probes[13] = Probe {
                origin: Vf::new(centre.x() - 20.0 * s, centre.y() + 6.5, centre.z() - 20.0 * c, 1.0),
                dir: Vf::new(s, 0.0, c, 0.0),
                length: F(40.0),
            };
        }
        2 => {
            b.count = 0;
            b.radius = F(pick.range(1.0, 120.0));
        }
        _ => {
            b.count = 1 + pick.below(MAX_PROBES.min(14) as u64) as usize;
            b.radius = F(pick.range(5.0, 200.0));
            for i in 0..b.count {
                let origin = Vf::new(centre.x() + pick.range(-15.0, 15.0), centre.y() + pick.range(-5.0, 25.0), centre.z() + pick.range(-15.0, 15.0), 1.0);
                let dir = if pick.below(2) == 0 {
                    let d = [target.x() - origin.x() + pick.range(-8.0, 8.0), target.y() - origin.y() + pick.range(-8.0, 8.0), target.z() - origin.z() + pick.range(-8.0, 8.0)];
                    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-3);
                    Vf::new(d[0] / l, d[1] / l, d[2] / l, 0.0)
                } else {
                    pick.unit()
                };
                b.probes[i] = Probe { origin, dir, length: F(pick.range(0.5, 60.0)) };
            }
        }
    }
    b
}

/// The walk's next centre: a short step most of the time, else a jump to beside a prop or plant (a
/// prop half the time: the first `props` targets).
fn step(pick: &mut Pick, at: Vf, targets: &[Vf], props: usize) -> (Vf, Vf) {
    let n = if pick.below(2) == 0 { props } else { targets.len() };
    let target = targets[pick.below(n as u64) as usize];
    if pick.below(4) == 0 {
        let c = Vf::new(target.x() + pick.range(-40.0, 40.0), target.y() + pick.range(-20.0, 30.0), target.z() + pick.range(-40.0, 40.0), 1.0);
        (c, target)
    } else {
        let c = Vf::new(at.x() + pick.range(-5.0, 5.0), at.y() + pick.range(-3.0, 3.0), at.z() + pick.range(-5.0, 5.0), 1.0);
        (c, target)
    }
}

/// Everything both proofs start from: the state, free09, the index, the original's query with a
/// fresh list, and the port's cache in step with it.
fn setup() -> Option<(Original, Props, ([F; 2], [F; 2]), Trees, Cache)> {
    let mut ee = state()?;
    let (props, field, trees) = free09(&ee)?;
    let index = Rc::new(index(&ee, &props, &trees, field));
    // Both gather fresh the first time: the state's list forgotten, its centre far off.
    let list = ee.u32_at(BIKE + BIKE_LIST);
    ee.set_u32(list + LIST_BUILT, 0);
    let far = Vf::new(1.0e6, 0.0, 1.0e6, 1.0);
    set_quad(&mut ee, BIKE + BIKE_AT, far);
    let cache = Cache { at: far, ..Cache::default() };
    let original = Original { ee, solid: addr("put_back_solid").unwrap(), sweep: addr("bundle_sweep_object").unwrap(), index };
    Some((original, props, field, trees, cache))
}

/// What a run met, to say how much of the query it exercised.
#[derive(Default, Debug)]
struct Tally {
    refused: usize,
    met: usize,
    answers: usize,
    gathered: usize,
    full: usize,
    by_groups: usize,
}

/// **The query**: the original's and the port's over 12 000 generated bundles walked about free09.
#[test]
fn the_put_back_query_is_the_originals() {
    let Some((mut original, props, field, trees, mut cache)) = setup() else { return };
    let scene = Scene { props: &props.props, trees: Some(&trees), field: Some(field) };
    let targets = targets(&props, &trees);
    let mut pick = Pick(0x5383_0001_c0ff_ee11);
    let (mut at, mut tally, mut apart) = (targets[0], Tally::default(), Vec::new());
    for case in 0..CASES {
        let (next, target) = step(&mut pick, at, &targets, props.props.len());
        at = next;
        let b = bundle(&mut pick, at, target);
        let before = cache.at;
        let (theirs, listed) = original.ask(&b);
        let ours = port(scene, &b, &mut cache, Race::default());
        tally.gathered += usize::from(bits(before) != ours.at);
        if listed >= 128 {
            tally.full += 1;
            continue;
        }
        tally.refused += usize::from(theirs.refused);
        tally.met += usize::from(theirs.y.is_some());
        tally.answers += theirs.answers.len();
        tally.by_groups += usize::from(theirs.refused && theirs.answers.iter().all(|a| !a.3.iter().skip(1).any(|&h| h)));
        if theirs != ours {
            apart.push((case, b.count, theirs, ours));
        }
    }
    eprintln!("{CASES} bundles: {tally:?}");
    assert!(apart.is_empty(), "{} of {CASES} apart, first: {:#?}", apart.len(), &apart[..apart.len().min(2)]);
    assert!(tally.full * 100 < CASES, "the list was full in {} cases", tally.full);
    assert!(tally.met > 500 && tally.refused > 500 && tally.by_groups > 50 && tally.gathered > 500, "too little exercised: {tally:?}");
}

/// **The race**: the same walk with the bike in a race — the mode object's mode 1 with its flag up,
/// a list of one to six made-up checkpoints about each bundle — and the race object asking for a
/// static-only gather in a random half of the cases.
#[test]
fn the_put_back_query_in_a_race_is_the_originals() {
    let Some((mut original, props, field, trees, mut cache)) = setup() else { return };
    let scene = Scene { props: &props.props, trees: Some(&trees), field: Some(field) };
    let targets = targets(&props, &trees);
    let (race, mode) = (original.ee.u32_at(BIKE + BIKE_RACE), original.ee.u32_at(BIKE + BIKE_MODE));
    assert!(race != 0 && mode != 0, "the state's bike has its race and mode objects");
    original.ee.set_u32(mode + MODE, 1);
    let flags = original.ee.u32_at(mode + MODE_FLAGS);
    original.ee.set_u32(mode + MODE_FLAGS, flags | 1);
    let mut pick = Pick(0x5383_0002_dead_beef);
    let (mut at, mut apart, mut by_checkpoint, mut static_only) = (targets[0], Vec::new(), 0, 0);
    for case in 0..RACE_CASES {
        let (next, target) = step(&mut pick, at, &targets, props.props.len());
        at = next;
        let b = bundle(&mut pick, at, target);
        let n = 1 + pick.below(6) as usize;
        let checkpoints: Vec<Checkpoint> = (0..n)
            .map(|_| Checkpoint {
                at: Vf::new(at.x() + pick.range(-60.0, 60.0), pick.range(-100.0, 100.0), at.z() + pick.range(-60.0, 60.0), 1.0),
                radius: F(pick.range(0.0, 50.0)),
            })
            .collect();
        let ee = &mut original.ee;
        for (k, c) in checkpoints.iter().enumerate() {
            let node = ee.scratch(10 + 3 * k as u32);
            ee.set_f32(node + CP_X, c.at.x());
            ee.set_f32(node + CP_Z, c.at.z());
            ee.set_f32(node + CP_RADIUS, c.radius.0);
            let next = if k + 1 < n { ee.scratch(10 + 3 * (k as u32 + 1)) } else { 0 };
            ee.set_u32(node + CP_NEXT, next);
        }
        let on = pick.below(5) != 0;
        ee.set_u32(mode + MODE_CHECKPOINTS, if on { ee.scratch(10) } else { 0 });
        let stat = pick.below(2) == 0;
        ee.set_u32(race + RACE_STATIC, u32::from(stat));
        static_only += usize::from(stat);
        let (theirs, listed) = original.ask(&b);
        let r = Race { static_only: stat, checkpoints: on.then_some(&checkpoints[..]) };
        let ours = port(scene, &b, &mut cache, r);
        if listed >= 128 {
            continue;
        }
        let near = on && checkpoints.iter().any(|c| back_search::near(c, b.centre));
        by_checkpoint += usize::from(near);
        if theirs != ours {
            apart.push((case, b.count, theirs, ours));
        }
    }
    eprintln!("{RACE_CASES} bundles in a race: {by_checkpoint} near a checkpoint, {static_only} static-only");
    assert!(apart.is_empty(), "{} of {RACE_CASES} apart, first: {:#?}", apart.len(), &apart[..apart.len().min(2)]);
    assert!(by_checkpoint > 1000 && by_checkpoint < RACE_CASES - 1000, "the checkpoints refused {by_checkpoint} times");
}
