//! F7 — the ragdoll's bones against the knockable props at rest, on recording2's save state of free09
//! (its beach balls and the rest of its 62 props asleep where the scene put them) and the disc.
//!
//! A sleeping prop's `f.col` stands in the track's static tree (category `0x18`, `+0x116` `0x04`,
//! `+0x31` bit 0 set, `+0x7c` 0, `+0x80` its collision object), so the bones' search lists it, and
//! with flag bit `0x08` clear it lists it wherever the query's cells reach it, its distance not asked.
//!
//! 1. **the list**: the original's gather on a rider body actor's bones' search, round its first sphere
//!    stood near one of free09's props at rest (or a tree), lists the same scene meshes, plants and
//!    sleeping props in the same order with the same distances, round the same centre in the same
//!    bound, as the port's [`BikeSearch::gather_knocking`] with [`Statics::knock_keys`], fresh and kept;
//! 2. **the sphere test**: a bone's sphere through the original's search pass, its list a sleeping prop
//!    alone, twice, or after the scene mesh nearest it, is the port's [`KnockedProp::resting`] offered
//!    the record in the list's order, bit for bit;
//! 3. **the struck listener**: a sphere's record taken by a sleeping prop wakes its body (enabled) and
//!    pushes nothing (its position, velocity and spin keep their bits), as [`PropSet::struck`];
//! 4. **the ride**: a ragdoll thrown at a sleeping prop stops on it and wakes it, and goes through it
//!    without the props.
//!
//! Skipped with a line where the disc, the kit or the state are not there.

use std::collections::HashMap;

use game_sim::archive::{Psr, Res};
use game_sim::biplane::Area;
use game_sim::eco::{Trees, Wooded};
use game_sim::flight_search;
use game_sim::phy::{Ini, Wrecks};
use game_sim::prop_set::PropSet;
use game_sim::ragdoll_scene::{self, BikeSearch, Cells, KnockedProp, Statics, StillProp, Touch};
use game_sim::rng::Rng;
use game_sim::track::{Scene, Terrain};
use game_sim::world::{Ground, Place, Props};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

const STATE: &str = "recording2.p2m2_SaveState.eeMemory.bin";
const TRACK: &str = "Tracks\\free09.res";
const BUDGET: u64 = 50_000_000;
const DT: F = F(1.0 / 30.0);

/// free09's terrain: three root cells by three, nine feet a sample (tests/ragdoll_ball.rs).
const FREE09: (f32, f32, f32, f32) = (3.0, 3.0, 9.0, 9.0);

/// A collidable's fields, a sphere collider's and its record's (`research/oracle.txt` S105, S142).
mod at {
    pub const SPACE: u32 = 0x04;
    pub const CLASS: u32 = 0x02;
    pub const GATE: u32 = 0x31;
    pub const CENTRE: u32 = 0x40;
    pub const REACH: u32 = 0x60;
    pub const MOVER: u32 = 0x7c;
    pub const OBJECT: u32 = 0x80;
    pub const CATEGORY: u32 = 0x84;
    pub const FLAGS: u32 = 0x116;
    pub const CENTRE2: u32 = 0x120;
    pub const RADIUS: u32 = 0x170;
    pub const RADIUS2: u32 = 0x174;
    pub const RECORD: u32 = 0x6c;
    pub const COUNT: u32 = 0x4;
    pub const HELD: u32 = 0x8;
    pub const NORMAL: u32 = 0x30;
    pub const POINT: u32 = 0x60;
    pub const WITH: u32 = 0x90;
}

/// A placed group's record (S102): its cell and its two size bytes.
mod group_at {
    pub const CELL: u32 = 0x08;
    pub const HEIGHT: u32 = 0x10;
    pub const WIDTH: u32 = 0x11;
}

/// The space's table and its tree (`tests/ragdoll_props.rs`).
mod tree_at {
    pub const TABLE: u32 = 0x04;
    pub const FIXED: u32 = 0x34;
    pub const LO_X: u32 = 0x30;
    pub const HI_X: u32 = 0x34;
    pub const LO_Z: u32 = 0x38;
    pub const HI_Z: u32 = 0x3c;
    pub const EXTENT: u32 = 0x48;
    pub const CELL: u32 = 0x58;
    pub const SCALE: u32 = 0x5c;
    pub const ROOT: u32 = 0x94;
}

/// The bones' search's fields, and the body actor's that hold it.
mod search_at {
    pub const FOUND: u32 = 0x00;
    pub const COUNT: u32 = 0x04;
    pub const CENTRE: u32 = 0x10;
    pub const BOUND: u32 = 0x20;
    pub const LEAST: u32 = 0x24;
    pub const EXTRA: u32 = 0x28;
    pub const BUILT: u32 = 0x30;
    pub const CAPACITY: u32 = 0x34;
    pub const ACTOR_PARTS: u32 = 0x2e0;
    pub const ACTOR_COLLISIONS: u32 = 0x624;
    pub const ACTOR_SEARCH: u32 = 0x628;
    pub const SPHERE: u32 = 0x88;
}

/// A prop's physics object (tests/ragdoll_ball.rs): its body and its collision object.
mod obj {
    pub const BODY: u32 = 0x110;
    pub const COLLISION: u32 = 0x120;
}

/// The category of a prop's mesh collidable, and the flags of a sleeping one.
const PROP: u32 = 0x18;
const ASLEEP: u8 = 0x04;

fn state() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("states").join(STATE);
    if !path.exists() {
        skip(&format!("{} is not there: `./kit state ram` of recording2.p2m2_SaveState.p2s", path.display()));
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

/// free09: its scene, its terrain, its props read the way the shell reads them, and its plants on
/// the state's curve table (`tests/back_search.rs`).
struct Free09 {
    scene: Scene,
    terrain: Vec<u8>,
    props: Props,
    trees: Trees,
}

impl Free09 {
    fn terrain(&self) -> Terrain<'_> {
        Terrain::parse(&self.terrain).expect("free09's terrain")
    }
}

fn free09(ee: &Ee) -> Option<Free09> {
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
    let scene = Scene::parse(&read(".scn")).expect("free09's scene");
    let terrain = read(&scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn"));
    let parsed = Terrain::parse(&terrain).expect("free09's terrain");
    let ground = Ground::new(&parsed, &scene.terrain);
    let field = ground.field();
    let mut eco = game_sim::eco::Ecosystem::parse(&read(".esb")).expect("free09.esb parses");
    let layout = &scene.terrain;
    let extent = F::from(layout.width.max(layout.breadth) as i32 * game_sim::world::SAMPLES_PER_WIDTH as i32) * parsed.scale;
    eco.stand(extent, |p| F(game_sim::contact_search::ground(&field, p).point.y()));
    let (trees, missed) = Trees::new(&eco, game_sim::eco::cell_size(extent), &curve(ee));
    assert!(missed.is_empty(), "free09's solids the port does not build: {missed:?}");

    let second = member(&TRACK.replace(".res", "_2.res"));
    let mut parts = vec![(bytes.clone(), res)];
    if let Some(b) = second
        && let Ok(r) = Res::parse(&b)
    {
        parts.push((b, r));
    }
    let (props, _) = Props::load(&scene, |want| {
        parts
            .iter()
            .find_map(|(bytes, res)| by_suffix(res, want).and_then(|e| res.read(bytes, e).ok()))
            .ok_or_else(|| "no archive of this track has it".to_string())
    });
    drop(ground);
    drop(parsed);
    Some(Free09 { scene, terrain, props, trees })
}

fn curve(ee: &Ee) -> game_sim::camera::CurveTable {
    let base = ee.u32_at(addr("curve_table_ptr").unwrap());
    let entries: Vec<[f32; 2]> =
        (0..game_sim::camera::CURVE_ENTRIES as u32).map(|k| [ee.f32_at(base + 8 * k), ee.f32_at(base + 8 * k + 4)]).collect();
    game_sim::camera::CurveTable::from_entries(&entries)
}

fn vf(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

fn set_vf(ee: &mut Ee, a: u32, v: Vf) {
    for k in 0..4 {
        ee.set_f32(a + 4 * k as u32, v[k]);
    }
}

fn bits(v: Vf) -> [u32; 4] {
    v.to_bits()
}

/// Every entry in the tree, each once.
fn tree_entries(ee: &Ee, tree: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![ee.u32_at(tree + tree_at::ROOT)];
    while let Some(node) = stack.pop() {
        let mut c = ee.u32_at(node + 4);
        while c != 0 {
            for k in 1..=4 {
                let o = ee.u32_at(c + 4 * k);
                if o != 0 && seen.insert(o) {
                    out.push(o);
                }
            }
            c = ee.u32_at(c);
        }
        let children = ee.u32_at(node);
        if children != 0 {
            stack.extend((0..4).map(|i| ee.u32_at(children + 4 * i)).filter(|&k| k != 0));
        }
    }
    out
}

/// The tree's scene meshes and placed groups by their index in the port's [`Statics::props`]: a
/// mesh the prop whose box centre and reach have its bits, a group the plant on its cell with its
/// two size bytes, past the props.
fn listed_index(ee: &Ee, props: &Props, trees: &Trees, tree: u32) -> HashMap<u32, usize> {
    let mut by_place = HashMap::new();
    for (i, p) in props.props.iter().enumerate() {
        let (c, _) = flight_search::prop_box(p);
        by_place.insert((c.map(F::to_bits), p.radius.to_bits()), i);
    }
    let mut by_cell = HashMap::new();
    for (j, g) in trees.groups().iter().enumerate() {
        assert!(by_cell.insert((g.cell, g.height, g.blend), props.props.len() + j).is_none(), "two of free09's plants alike");
    }
    let (mut index, mut meshes, mut groups) = (HashMap::new(), 0, 0);
    for o in tree_entries(ee, tree) {
        match ee.u8_at(o + at::CLASS) {
            2 if ee.u32_at(o + at::CATEGORY) == flight_search::SCENERY => {
                // free09's one hierarchy (the utility shack) is none of the port's props.
                let key = ([0, 4, 8, 12].map(|k| ee.u32_at(o + at::CENTRE + k)), ee.u32_at(o + at::REACH));
                if let Some(&i) = by_place.get(&key) {
                    index.insert(o, i);
                    meshes += 1;
                }
            }
            3 => {
                let cell = [0, 2, 4].map(|k| ee.u16_at(o + group_at::CELL + k));
                let key = (cell, ee.u8_at(o + group_at::HEIGHT) as i8, ee.u8_at(o + group_at::WIDTH) as i8);
                let i = *by_cell.get(&key).unwrap_or_else(|| panic!("the state's group {o:#x} {key:?} is none of the port's plants"));
                index.insert(o, i);
                groups += 1;
            }
            _ => {}
        }
    }
    assert_eq!(meshes, props.props.len(), "the state's scene meshes and free09's props");
    assert_eq!(groups, trees.groups().len(), "the state's placed groups and free09's plants");
    index
}

/// A rider body actor in the state with its bones' search: the search and its first sphere.
fn bones(ee: &Ee) -> (u32, u32) {
    let ram = |a: u32| (0x10_0000..0x200_0000).contains(&a) && a % 4 == 0;
    for a in (0x10_0000u32..0x1ff_f000).step_by(4) {
        if ee.u32_at(a + search_at::ACTOR_PARTS) != 18 {
            continue;
        }
        let s = ee.u32_at(a + search_at::ACTOR_SEARCH);
        if !ram(s) {
            continue;
        }
        if ee.f32_at(s + search_at::LEAST) == 10.0 && ee.f32_at(s + search_at::EXTRA) == 30.0 && ee.i32_at(s + search_at::CAPACITY) == 128 {
            let first = ee.u32_at(ee.u32_at(a + search_at::ACTOR_COLLISIONS));
            if !ram(first) || !ram(ee.u32_at(first + search_at::SPHERE)) {
                continue;
            }
            return (s, ee.u32_at(first + search_at::SPHERE));
        }
    }
    panic!("no rider body actor in the state");
}

/// A small generator.
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
    fn upto(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn unit_vector(&mut self) -> Vf {
        let v = [self.range(-1.0, 1.0), self.range(-1.0, 1.0), self.range(-1.0, 1.0)];
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-3);
        Vf::new(v[0] / n, v[1] / n, v[2] / n, 0.0)
    }
}

/// The plants with a trunk, by their group index.
fn trunked(trees: &Trees) -> Vec<usize> {
    (0..trees.groups().len()).filter(|&j| trees.groups()[j].parts.iter().any(Option::is_some)).collect()
}

/// A plant's place on the ground plane, at the height of its trunk's centre.
fn plant_at(trees: &Trees, j: usize) -> Vf {
    let g = &trees.groups()[j];
    let trunk = &trees.props.props[g.parts.iter().flatten().next().copied().expect("a trunk")];
    let at = |c: u16| F::from(i32::from(c)).0 * trees.cell().0;
    Vf::new(at(g.cell[0]), trunk.centre.y(), at(g.cell[2]), 1.0)
}

// --- the props at rest ------------------------------------------------------------------------------

/// free09's knockable props off the disc, every one asleep where the scene put it (tests/ragdoll_ball.rs).
fn knockables() -> Option<PropSet> {
    let mut disc = game_render::disc::Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let name = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case(TRACK)).map(|e| e.name.clone())?;
    let res = disc.res(&name).expect("free09's archive");
    let toy = Ini::parse_bytes(&res.read_suffix("free09.toy").expect("free09.toy"));
    let scene = Scene::parse(&res.read_suffix("free09.scn").expect("free09.scn")).expect("free09.scn parses");
    let (x, z, sx, sz) = FREE09;
    let (set, said) = PropSet::load(
        &toy,
        "Tracks\\Free09\\",
        &scene.models,
        &mut |p| res.read(p).ok(),
        &mut |p| res.read_suffix(p).ok(),
        Some(Area::new(F(x), F(z), F(sx), F(sz))),
    );
    assert!(said.is_empty(), "free09 loads whole: {said:?}");
    Some(set)
}

/// How many bodies `set` has: the knockable props' indices run over them.
fn bodies(set: &PropSet) -> usize {
    set.collections.iter().map(|c| c.objects.len()).sum()
}

/// Every sleeping prop of `set` the bones could list, each by its index in [`Statics::props`] (`first`
/// on, [`Statics::knock_from`]), measured from `probe`.
fn resting_all(set: &PropSet, first: usize, probe: Vf) -> Vec<(usize, KnockedProp)> {
    let n = bodies(set);
    let all = Statics { props: (first..first + n).collect(), bike_at: n };
    KnockedProp::resting(set, &all, first, probe, F(f32::MAX)).into_iter().map(|p| (first + p.place.unwrap(), p)).collect()
}

/// Where a sleeping prop's collidable stands to the search: its centre (`+0x40`) and reach (`+0x60`).
fn posed(set: &PropSet, p: &KnockedProp) -> (Vf, F) {
    let (c, o) = (p.pushed.collection, p.pushed.object);
    let mesh = set.shapes[c][o].as_ref().unwrap();
    let (_, centre, radius) = game_sim::world::Prop::posed(mesh, &PropSet::matrix(&set.collections[c].objects[o].node));
    (centre, radius)
}

/// The tree's sleeping prop meshes (category `0x18`) by their index in the port's [`Statics::props`]:
/// the port's sleeping prop whose centre and reach have its bits. Each is checked for what the survey
/// found: flags `0x04`, the pair's gate open, no mover, its collision object set.
fn knock_index(ee: &Ee, set: &PropSet, tree: u32, first: usize) -> HashMap<u32, usize> {
    let mut by_place = HashMap::new();
    for (i, p) in resting_all(set, first, Vf::new(0.0, 0.0, 0.0, 1.0)) {
        let (c, r) = posed(set, &p);
        assert!(by_place.insert((bits(c), r.to_bits()), i).is_none(), "two of free09's props alike");
    }
    let mut index = HashMap::new();
    for o in tree_entries(ee, tree) {
        if ee.u8_at(o + at::CLASS) != 2 || ee.u32_at(o + at::CATEGORY) != PROP {
            continue;
        }
        // Can be struck, and listed wherever the query finds it (0x08 clear).
        let flags = ee.u8_at(o + at::FLAGS);
        assert_eq!(flags & (ASLEEP | 0x08), ASLEEP, "the tree's prop mesh {o:#x}'s flags {flags:#x}");
        assert!(ee.u8_at(o + at::GATE) & 1 == 1 && ee.u32_at(o + at::MOVER) == 0 && ee.u32_at(o + at::OBJECT) != 0, "the tree's prop mesh {o:#x}");
        let key = (bits(vf(ee, o + at::CENTRE)), ee.u32_at(o + at::REACH));
        let i = *by_place.get(&key).unwrap_or_else(|| panic!("the state's sleeping prop {o:#x} {key:x?} is none of the port's"));
        index.insert(o, i);
    }
    assert_eq!(index.len(), by_place.len(), "the state's sleeping props and free09's");
    index
}

/// How far listed entry `i` is from `probe`, as the port measures it.
fn distance(props: &[game_sim::world::Prop], trees: &Trees, set: &PropSet, first: usize, i: usize, probe: Vf) -> F {
    if i >= first {
        let one = Statics { props: vec![i], bike_at: 1 };
        return KnockedProp::resting(set, &one, first, probe, F(f32::MAX))[0].distance;
    }
    match props.get(i) {
        Some(p) => StillProp::of(p, probe).distance,
        None => ragdoll_scene::group_distance(&trees.groups()[i - props.len()], trees.cell(), probe),
    }
}

/// How many probes [`the_bones_search_lists_the_props_at_rest_as_the_originals`] stands.
const PROBES: usize = 3_000;

/// **The bones' search lists the props at rest as the original's** (F7): the original's gather on a
/// rider body actor's bones' search, its first sphere stood within 40 ft of one of free09's sleeping
/// props (three probes in four) or one of its trees, and the port's with the props' keys after the
/// plants', list the same scene meshes, plants and sleeping props in the same order with the same
/// distances, round the same centre in the same bound; then, the sphere moved within the bound, both
/// keep the list and measure it again. A sleeping prop is listed wherever the cells reach it: many
/// are listed further off than the bound, where a scene mesh would not be. What else the original
/// lists (the trigger boxes, the bikes, the shack's hierarchy) is left out of the comparison.
#[test]
fn the_bones_search_lists_the_props_at_rest_as_the_originals() {
    let Some(mut ee) = state() else { return };
    let Some(free) = free09(&ee) else { return };
    let Some(set) = knockables() else { return };
    let (props, trees) = (&free.props.props, &free.trees);
    let (search, sphere) = bones(&ee);
    let tree = ee.u32_at(ee.u32_at(ee.u32_at(sphere + at::SPACE) + tree_at::TABLE) + tree_at::FIXED);
    let first = Statics::knock_from(props, Some(trees));
    let mut index = listed_index(&ee, &free.props, trees, tree);
    let knocks = knock_index(&ee, &set, tree, first);
    eprintln!("{} sleeping props in the state's tree", knocks.len());
    index.extend(knocks);
    let resting: Vec<Vf> = resting_all(&set, first, Vf::new(0.0, 0.0, 0.0, 1.0)).iter().map(|(_, p)| posed(&set, p).0).collect();

    let terrain = free.terrain();
    let ground = Ground::new(&terrain, &free.scene.terrain);
    let cells = Cells::of(&ground.field()).expect("free09's field has edges");
    let t = &cells.tree;
    assert_eq!(
        (t.origin, cells.hi, t.extent, t.cell, t.scale),
        (
            [F(ee.f32_at(tree + tree_at::LO_X)), F(ee.f32_at(tree + tree_at::LO_Z))],
            [F(ee.f32_at(tree + tree_at::HI_X)), F(ee.f32_at(tree + tree_at::HI_Z))],
            [F(ee.f32_at(tree + tree_at::EXTENT)), F(ee.f32_at(tree + tree_at::EXTENT + 4))],
            ee.i32_at(tree + tree_at::CELL),
            F(ee.f32_at(tree + tree_at::SCALE))
        ),
        "the tree"
    );
    let prop_keys = Statics::keys(&cells, props);
    let knock_keys = Statics::knock_keys(&cells, &set, first);
    let gather = addr("collision_search_gather").unwrap();
    let reach = F(ee.f32_at(sphere + at::REACH));
    let far = (Vf::new(-1.0e6, 0.0, -1.0e6, 1.0), F::ONE);
    let found = |ee: &Ee| -> (Vec<(usize, u32)>, u32) {
        let (a, n) = (ee.u32_at(search + search_at::FOUND), ee.u32_at(search + search_at::COUNT));
        let list = (0..n).filter_map(|k| index.get(&ee.u32_at(a + 8 * k)).map(|&i| (i, ee.u32_at(a + 8 * k + 4)))).collect();
        (list, n)
    };
    let ours = |s: &Statics, probe: Vf| -> Vec<(usize, u32)> { s.props.iter().map(|&i| (i, distance(props, trees, &set, first, i, probe).to_bits())).collect() };

    let woods = trunked(trees);
    let mut pick = Pick(0xf007_f007_ba11_5eed);
    let (mut compared, mut kept, mut full, mut most, mut listed, mut past_bound, mut mixed) = (0, 0, 0, 0, 0, 0, 0);
    for case in 0..PROBES {
        let around = if case % 4 == 3 { plant_at(trees, woods[pick.upto(woods.len() as u64) as usize]) } else { resting[pick.upto(resting.len() as u64) as usize] };
        let probe = Vf::new(around.x() + pick.range(-40.0, 40.0), around.y() + pick.range(-10.0, 10.0), around.z() + pick.range(-40.0, 40.0), 1.0);
        set_vf(&mut ee, sphere + at::CENTRE, probe);
        ee.set_u32(search + search_at::BUILT, 0);
        let mut port = BikeSearch { stood: Some(probe), ..BikeSearch::default() };
        let mut statics = Statics::default();
        let mut keys = prop_keys.clone();
        keys.extend(Statics::group_keys(&cells, trees, props.len(), probe, reach));
        keys.extend(knock_keys.iter().copied());
        for step in 0..2 {
            let mut now = probe;
            if step == 1 {
                // Moved so the sphere's reach stays inside the bound: the list is kept.
                let room = port.bound.0 - reach.0;
                if !(room > 0.5) {
                    break;
                }
                let d = room * 0.5;
                now = Vf::new(probe.x() + pick.range(-d, d) * 0.7, probe.y(), probe.z() + pick.range(-d, d) * 0.7, 1.0);
                set_vf(&mut ee, sphere + at::CENTRE, now);
                port.stood = Some(now);
                assert!(!port.regathers(reach), "case {case}: the port regathers a list it should keep");
            }
            ee.call(gather, &[u64::from(search), u64::from(sphere), 3], &[], BUDGET);
            port.gather_knocking(Some(&cells), reach, far, 0, false, props, Some(trees), &keys, first, &mut statics);
            let (theirs, n) = found(&ee);
            if n as usize >= 128 {
                full += 1;
                break;
            }
            let what = format!("case {case} step {step}: round {now:?}");
            assert_eq!(ours(&statics, now), theirs, "{what}: the props, plants and props at rest, in order, with their distances");
            assert_eq!(bits(port.centre), bits(vf(&ee, search + search_at::CENTRE)), "{what}: the centre");
            assert_eq!(port.bound.to_bits(), ee.u32_at(search + search_at::BOUND), "{what}: the bound");
            compared += 1;
            kept += usize::from(step == 1);
            most = most.max(theirs.len());
            if step == 0 {
                let knocked: Vec<usize> = statics.props.iter().copied().filter(|&i| i >= first).collect();
                listed += knocked.len();
                past_bound += knocked.iter().filter(|&&i| !(distance(props, trees, &set, first, i, now) < port.bound)).count();
                mixed += usize::from(!knocked.is_empty() && statics.props.iter().any(|&i| i < first));
            }
        }
    }
    eprintln!(
        "{compared} lists compared ({kept} kept), {full} full, at most {most} entries in one; {listed} props at rest listed, {past_bound} past the bound; {mixed} lists mix them with scenery or plants"
    );
    assert!(compared > PROBES && kept > PROBES / 3 && listed > PROBES && past_bound > 100 && mixed > 100, "the cases did not reach far enough");
}

/// How many spheres [`the_sphere_test_on_a_prop_at_rest_answers_as_the_originals`] stands.
const SPHERES: usize = 20_000;

/// A point near a prop's mesh: one of its vertices through its frame.
fn near_vertex(prop: &KnockedProp, pick: &mut Pick) -> Vf {
    let mesh = prop.mesh().expect("a prop has a mesh");
    let n = match &mesh.vertices {
        game_sim::track::Points::Wide(v) => v.len(),
        game_sim::track::Points::Packed(v) => v.len(),
    };
    let v = game_sim::collide::MeshRef::of(mesh).vertex(pick.upto(n as u64) as u16);
    prop.frame.place(Vf::new(v.x(), v.y(), v.z(), 1.0))
}

/// A sphere stood near a vertex of `prop`: the bones' radii and random ones from a tenth to a metre.
fn sphere_near(prop: &KnockedProp, case: usize, pick: &mut Pick) -> (Vf, f32) {
    let p = near_vertex(prop, pick);
    let radius = match case % 5 {
        0 => 0.4f32,
        1 => 0.75,
        2 => 0.5,
        _ => pick.range(0.1, 1.0),
    };
    let off = pick.unit_vector();
    let reach = 2.0 * radius * pick.unit();
    (Vf::new(p.x() + off.x() * reach, p.y() + off.y() * reach, p.z() + off.z() * reach, 1.0), radius)
}

/// The port's answer to a sphere on a list, as [`ragdoll_scene::WreckBikeScene`]'s walk offers it:
/// each entry in the list's order, a scene mesh as a [`StillProp`] and a sleeping prop as a
/// [`KnockedProp::resting`], when its distance is within the radius, on the record's hit so far.
fn port_pass(props: &[game_sim::world::Prop], set: &PropSet, first: usize, order: &[usize], centre: Vf, radius: F) -> Option<(usize, Touch)> {
    let statics = Statics { props: order.to_vec(), bike_at: order.len() };
    let still = StillProp::near_wooded(props, None, &statics, centre, radius);
    let rest = KnockedProp::resting(set, &statics, first, centre, radius);
    let mut best: Option<(usize, Touch)> = None;
    for (n, &i) in order.iter().enumerate() {
        let held = best.map(|(_, b)| b.distance);
        let t = match (still.iter().find(|(k, _)| *k == n), rest.iter().find(|p| p.place == Some(n))) {
            (Some((_, s)), _) => s.touch(centre, radius, held).filter(|_| s.distance <= radius),
            (_, Some(p)) => p.touch(centre, radius, held).filter(|_| p.distance <= radius),
            _ => None,
        };
        if let Some(t) = t {
            best = Some((i, t));
        }
    }
    best
}

/// Stand the sphere collider at `centre` of `radius` with `order` on the bones' search at the port's
/// distances, and run the original's pass (`collide_against_search`); the record's contact.
#[allow(clippy::too_many_arguments)]
fn original_pass(ee: &mut Ee, search: u32, sphere: u32, of: &HashMap<usize, u32>, order: &[usize], distances: &[F], centre: Vf, radius: f32) -> Option<(u32, u32, [u32; 4], [u32; 4])> {
    let list_at = ee.u32_at(search + search_at::FOUND);
    set_vf(ee, sphere + at::CENTRE, centre);
    set_vf(ee, sphere + at::CENTRE2, centre);
    ee.set_f32(sphere + at::REACH, radius);
    ee.set_f32(sphere + at::RADIUS, radius);
    ee.set_f32(sphere + at::RADIUS2, ps2_float::fpu::mul(radius, radius));
    for (k, (&i, d)) in order.iter().zip(distances).enumerate() {
        ee.set_u32(list_at + 8 * k as u32, of[&i]);
        ee.set_u32(list_at + 8 * k as u32 + 4, d.to_bits());
    }
    ee.set_u32(search + search_at::COUNT, order.len() as u32);
    ee.call(addr("collide_against_search").unwrap(), &[u64::from(sphere), u64::from(search)], &[], BUDGET);
    let rec = ee.u32_at(sphere + at::RECORD);
    let count = ee.u32_at(rec + at::COUNT);
    assert!(count <= 1, "{count} contacts");
    (count > 0).then(|| (ee.u32_at(rec + at::WITH), ee.u32_at(rec + at::HELD), bits(vf(ee, rec + at::POINT)), bits(vf(ee, rec + at::NORMAL))))
}

/// **The sphere test on a prop at rest answers as the original's** (F7): 20,000 spheres stood near a
/// vertex of one of free09's sleeping props, each through the original's search pass on a bone's
/// sphere collider with a list of the prop alone, the prop twice, the scene mesh nearest it and then
/// the prop, or the prop and then the sleeping prop nearest it, each at the distance the port
/// measures; and through the port's walk ([`port_pass`]). The record's count, what it is with, the
/// plane distance it holds, and the contact's point and normal in the world, bit for bit.
#[test]
fn the_sphere_test_on_a_prop_at_rest_answers_as_the_originals() {
    let Some(mut ee) = state() else { return };
    let Some(free) = free09(&ee) else { return };
    let Some(set) = knockables() else { return };
    let (props, trees) = (&free.props.props, &free.trees);
    let (search, sphere) = bones(&ee);
    let tree = ee.u32_at(ee.u32_at(ee.u32_at(sphere + at::SPACE) + tree_at::TABLE) + tree_at::FIXED);
    let first = Statics::knock_from(props, Some(trees));
    let mut index = listed_index(&ee, &free.props, trees, tree);
    index.extend(knock_index(&ee, &set, tree, first));
    let of: HashMap<usize, u32> = index.iter().map(|(&o, &i)| (i, o)).collect();
    let resting = resting_all(&set, first, Vf::new(0.0, 0.0, 0.0, 1.0));
    let nearest_prop: Vec<usize> = resting
        .iter()
        .map(|(_, p)| {
            let a = posed(&set, p).0;
            (0..props.len()).min_by(|&x, &y| StillProp::of(&props[x], a).distance.0.total_cmp(&StillProp::of(&props[y], a).distance.0)).unwrap()
        })
        .collect();
    let nearest_rest: Vec<usize> = resting
        .iter()
        .map(|(i, p)| {
            let a = posed(&set, p).0;
            let d = |j: usize| distance(props, trees, &set, first, j, a).0;
            resting.iter().map(|(j, _)| *j).filter(|j| j != i).min_by(|&x, &y| d(x).total_cmp(&d(y))).unwrap()
        })
        .collect();
    let mut pick = Pick(0xf007_0000_5ea7_ed07);
    let (mut hits, mut second, mut kept, mut gated) = (0, 0, 0, 0);
    for case in 0..SPHERES {
        let w = pick.upto(resting.len() as u64) as usize;
        let (k, prop) = (resting[w].0, &resting[w].1);
        let order: Vec<usize> = match pick.upto(4) {
            0 => vec![k],
            1 => vec![k, k],
            2 => vec![nearest_prop[w], k],
            _ => vec![k, nearest_rest[w]],
        };
        let (centre, radius) = sphere_near(prop, case, &mut pick);
        let distances: Vec<F> = order.iter().map(|&i| distance(props, trees, &set, first, i, centre)).collect();
        gated += distances.iter().filter(|d| !(d.0 <= radius)).count();
        let best = port_pass(props, &set, first, &order, centre, F(radius));
        let port = best.map(|(i, t)| (of[&i], t.distance.to_bits(), bits(t.world.0), bits(t.world.1)));
        let original = original_pass(&mut ee, search, sphere, &of, &order, &distances, centre, radius);
        assert_eq!(port, original, "case {case}: list {order:?} at {distances:?}, sphere at {centre:?} r {radius}");
        if let Some((i, _)) = best {
            hits += 1;
            second += usize::from(order.len() == 2 && i == order[1] && order[0] != order[1]);
            kept += usize::from(order.len() == 2 && i == order[0]);
        }
    }
    eprintln!("{hits} of {SPHERES} hit; the second took {second}, the first kept {kept}; {gated} entries out of a sphere's reach");
    assert!(hits > SPHERES / 5 && hits < SPHERES * 9 / 10, "{hits} hits");
    assert!(second > 500 && kept > 500 && gated > 1000, "the second took {second}, the first kept {kept}, {gated} gated");
}

/// The physics object whose collision object is `collision`, scanned for (tests/ragdoll_ball.rs's
/// layout): its body.
fn body_of(ee: &Ee, collision: u32) -> u32 {
    let found: Vec<u32> = (0x10_0000u32..0x1ff_fe00).step_by(16).filter(|&o| ee.u32_at(o + obj::COLLISION) == collision && ee.u32_at(o + obj::BODY) != 0).collect();
    assert_eq!(found.len(), 1, "the physics objects on {collision:#x}: {found:x?}");
    ee.u32_at(found[0] + obj::BODY)
}

/// **A bone's strike wakes a prop at rest and pushes nothing** (F7): for each of a dozen of free09's
/// sleeping props, on a fresh state, a sphere the port says the prop takes goes through the original's
/// search pass with the prop alone on the list. The record is the prop's, and after the pass its body
/// is enabled (it was not) while its position, velocity, orientation and spin keep their bits: the
/// struck listener is the wake ([`PropSet::struck`]), and nothing pushes the prop. The port's
/// [`PropSet::struck`] enables the same body.
#[test]
fn a_bones_strike_wakes_a_prop_at_rest_and_pushes_nothing() {
    let Some(ee) = state() else { return };
    let Some(free) = free09(&ee) else { return };
    let Some(mut set) = knockables() else { return };
    let (props, trees) = (&free.props.props, &free.trees);
    let (search, sphere) = bones(&ee);
    let tree = ee.u32_at(ee.u32_at(ee.u32_at(sphere + at::SPACE) + tree_at::TABLE) + tree_at::FIXED);
    let first = Statics::knock_from(props, Some(trees));
    let knocks = knock_index(&ee, &set, tree, first);
    let of: HashMap<usize, u32> = knocks.iter().map(|(&o, &i)| (i, o)).collect();
    drop(ee);
    let resting = resting_all(&set, first, Vf::new(0.0, 0.0, 0.0, 1.0));
    let mut pick = Pick(0xf007_0000_0000_0003);
    let mut woken = 0;
    for w in (0..resting.len()).step_by((resting.len() / 12).max(1)) {
        let (k, prop) = (resting[w].0, &resting[w].1);
        let (centre, radius) = loop {
            let (c, r) = sphere_near(prop, 2, &mut pick);
            if port_pass(props, &set, first, &[k], c, F(r)).is_some() {
                break (c, r);
            }
        };
        let mut ee = state().expect("the state");
        let body = body_of(&ee, ee.u32_at(of[&k] + at::OBJECT));
        let words = |ee: &Ee| [0x60u32, 0x70, 0x170, 0x1b0].map(|a| bits(vf(ee, body + a)));
        let before = words(&ee);
        assert_eq!(ee.u32_at(body), 0, "prop {k}'s body is asleep in the state");
        let d = distance(props, trees, &set, first, k, centre);
        let hit = original_pass(&mut ee, search, sphere, &of, &[k], &[d], centre, radius);
        assert_eq!(hit.map(|h| h.0), Some(of[&k]), "prop {k}: the original's record");
        assert_ne!(ee.u32_at(body), 0, "prop {k}: the strike did not wake its body");
        assert_eq!(words(&ee), before, "prop {k}: the strike moved its body");
        eprintln!("prop {k} ({:?}): woken, flags now {:#x}", (prop.pushed.collection, prop.pushed.object), ee.u8_at(of[&k] + at::FLAGS));
        let (c, o) = (prop.pushed.collection, prop.pushed.object);
        set.struck(c, o);
        assert!(set.collections[c].bones[o].body.enabled, "prop {k}: the port's strike");
        woken += 1;
    }
    assert!(woken >= 10, "{woken} props struck");
}

// --- the ride ---------------------------------------------------------------------------------------

/// The ragdoll's kit as the shell builds it (`tests/ragdoll_props.rs`'s `ride_kit`).
fn ride_kit() -> Option<(game_sim::ragdoll_ride::Kit, game_sim::phy::Part, Wrecks)> {
    use game_render::disc::Disc;
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let names: Vec<String> =
        disc.entries().iter().filter(|e| e.name.to_ascii_lowercase().ends_with(".res")).map(|e| e.name.clone()).collect();
    let rider_name = names.iter().filter(|n| n.to_ascii_lowercase().ends_with("thq1.res")).min_by_key(|n| n.len())?.clone();
    let res = disc.res(&rider_name).expect("the rider archive opens");
    let rider_file = res.read_suffix("mrider.phy").expect("the rider's archive carries mrider.phy");
    let mut body = game_render::rider::Body::load(
        vec![disc.res(&rider_name).expect("the rider archive opens")],
        "thq1",
        game_render::rider::Sex::Male,
        "",
        &[],
        game_sim::motion::Params::default(),
    )
    .expect("the rider's body loads");
    let model = game_render::psg::Model::parse(&res.read_suffix(&format!("{}.psg", body.geometry())).expect("its model"))
        .expect("the model parses");
    let doll = body.ragdoll_rig(&model, &rider_file).expect("every part names a node");
    let wreck_bike = names
        .iter()
        .find_map(|n| disc.res(n).ok().and_then(|r| r.read_suffix("wreckbike.phy").ok()))
        .expect("an archive carries wreckbike.phy");
    let mut kit = game_sim::ragdoll_ride::Kit::new(&rider_file, doll.rig);
    let col = names.iter().find_map(|n| disc.res(n).ok().and_then(|r| r.read_suffix("wreckbikef.col").ok()));
    kit.bike = col.as_deref().and_then(game_sim::ragdoll_ride::bike_sphere);
    kit.bike_mesh = col.as_deref().and_then(game_sim::ragdoll_scene::Bike::parse).map(std::sync::Arc::new);
    assert!(kit.bike.is_some() && kit.bike_mesh.is_some(), "an archive carries wreckbikef.col");
    Some((kit, game_sim::phy::Part::read_bytes(&wreck_bike, "PhysicsPart1"), Wrecks::read_bytes(&rider_file)))
}


/// The main bone over `ticks` ticks of a ragdoll entered from the riding pose, its hands let go and
/// every bone moved so the main bone is `back` ft past a lone sleeping prop's reach (`-x` of its
/// centre) at its centre's height, all thrown at it at `speed` ft/s; stepped among free09's knockable
/// props or without them. The wreck bike stays 200 ft off. Answers the main bone each tick, the prop's
/// centre and reach, and whether the prop woke.
fn throw(with_props: bool, back: f32, speed: f32, ticks: usize) -> Option<(Vec<Vf>, Vf, F, bool)> {
    let ee = state()?;
    let free = free09(&ee)?;
    let mut set = knockables()?;
    let (kit, part, wrecks) = ride_kit()?;
    let first = Statics::knock_from(&free.props.props, Some(&free.trees));
    let resting = resting_all(&set, first, Vf::new(0.0, 0.0, 0.0, 1.0));
    // The largest sleeping prop with no plant, scene mesh or other prop within 30 ft of its reach.
    let (_, prop) = resting
        .iter()
        .filter(|(i, p)| {
            let (a, r) = posed(&set, p);
            let lone = |d: F| !(d.0 - r.0 < 30.0);
            free.trees.groups().iter().all(|g| lone(ragdoll_scene::group_distance(g, free.trees.cell(), a)))
                && free.props.props.iter().all(|s| lone(StillProp::of(s, a).distance))
                && resting.iter().filter(|(j, _)| j != i).all(|(_, q)| lone(KnockedProp::resting(&set, &Statics { props: vec![first + q.place.unwrap()], bike_at: 1 }, first, a, F(f32::MAX))[0].distance))
        })
        .max_by(|(_, a), (_, b)| posed(&set, a).1.0.total_cmp(&posed(&set, b).1.0))
        .expect("a lone prop at rest");
    let (c, o) = (prop.pushed.collection, prop.pushed.object);
    let (centre, radius) = posed(&set, prop);
    eprintln!("the prop: collection {c} object {o}, model {:?}, at {centre:?}", set.models[c].get(o));
    let Free09 { scene, terrain, props, trees } = free;
    let terrain = Terrain::parse(&terrain).expect("free09's terrain");
    let mut world = Wooded { world: Place { ground: Ground::new(&terrain, &scene.terrain), props }, trees };

    let mut bike = game_sim::ride::Ride::default();
    bike.chassis.quat = Vf::new(0.0, 0.0, 0.0, 1.0);
    bike.origin = Vf::new(centre.x() + 200.0, centre.y() + 40.0, centre.z() + 200.0, 1.0);
    let crash = game_sim::ride::Crash::begin(&bike, game_sim::trick::Wreck(5), &part, game_sim::ride::WRECK_INERTIA, &wrecks, None);
    let mut doll = game_sim::ragdoll_ride::Down::enter(&kit, &bike.transform(), &crash, crash.throw, 5, &wrecks, &mut Rng::default(), &mut world);
    doll.ragdoll.holds = [false; 4];
    let main = doll.main().position;
    let start = centre.x() - radius.0 - back;
    for b in &mut doll.ragdoll.bones {
        let p = b.body.position;
        b.body.position = Vf::new(p.x() - main.x() + start, p.y() - main.y() + centre.y(), p.z() - main.z() + centre.z(), p.w());
        b.body.velocity = Vf::new(speed, 0.0, 0.0, 0.0);
        b.body.rates = Vf::ZERO;
    }
    let mut seen = Vec::new();
    for _ in 0..ticks {
        let knockable = if with_props { Some(&mut set) } else { None };
        doll.step_posed_among(&kit, None, DT, &crash, &mut world, false, knockable);
        seen.push(doll.main().position);
    }
    Some((seen, centre, radius, set.collections[c].bones[o].body.enabled))
}

/// **A ragdoll thrown at a prop at rest stops on it and wakes it** (F7): thrown at 25 ft/s from 3 ft
/// off the reach of free09's largest lone sleeping prop, at its centre's height, the main bone stops
/// short of the prop's centre and slides down it, never a foot past, and the prop is woken; stepped
/// without the props (the port before this task) the bone flies through, 5 ft past the centre, and
/// the prop sleeps on.
#[test]
fn a_ragdoll_thrown_at_a_prop_at_rest_stops_on_it_and_wakes_it() {
    let Some((on, centre, radius, woke)) = throw(true, 3.0, 25.0, 30) else { return };
    let (through, _, _, slept) = throw(false, 3.0, 25.0, 30).expect("the same track");
    for (t, (a, b)) in on.iter().zip(&through).enumerate() {
        println!("tick {t}: at ({:.2}, {:.2}, {:.2}) of the prop; without the props x {:.2}", a.x() - centre.x(), a.y() - centre.y(), a.z() - centre.z(), b.x() - centre.x());
    }
    eprintln!("the prop's reach {}; woken {woke}, without the props {slept}", radius.0);
    assert!(woke && !slept, "the prop woke {woke}, without the props {slept}");
    assert!(on.iter().all(|p| p.x() < centre.x() + 1.0), "the main bone went past the prop's centre");
    assert!(through.last().unwrap().x() > centre.x() + 5.0, "without the props the bone stayed at x {}", through.last().unwrap().x() - centre.x());
}
