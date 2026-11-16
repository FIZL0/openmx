//! F8 — the riding bike's kept nearby list with the knockable props in it, on recording2's save state
//! of free09 (its beach balls and the rest of its props asleep where the scene put them) and the disc.
//!
//! The original's wheel probe (`+0x7c8`) searches the bike's one list (`+0x7bc`, S189), gathered once
//! a tick round the body rays (`+0x7c4`). A sleeping prop's `f.col` stands in the track's static tree
//! (F7: category `0x18`, `+0x116` `0x04`), so the bike's gather lists it among the scene's meshes and
//! the plants, wherever the query's cells reach it. The port's riding gather with the props in its
//! world ([`flight_search::gather_knocked`], [`game_sim::prop_wheel::riding`]) lists the same scene
//! meshes, plants and sleeping props in the same order with the same distances, round the same centre
//! in the same bound, fresh and kept.
//!
//! Skipped with a line where the disc, the kit or the state are not there.

use std::collections::HashMap;

use game_sim::archive::{Psr, Res};
use game_sim::biplane::Area;
use game_sim::broad_phase;
use game_sim::contact_search::Heightfield;
use game_sim::eco::Trees;
use game_sim::flight_search;
use game_sim::phy::Ini;
use game_sim::prop_set::PropSet;
use game_sim::ragdoll_scene::{KnockedProp, Statics};
use game_sim::track::{Scene, Terrain};
use game_sim::world::{Ground, Props};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

const STATE: &str = "recording2.p2m2_SaveState.eeMemory.bin";
const TRACK: &str = "Tracks\\free09.res";
const BUDGET: u64 = 50_000_000;

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

/// The bike's nearby search's fields, and the rider's that hold it and the body rays (S189).
mod search_at {
    pub const FOUND: u32 = 0x00;
    pub const COUNT: u32 = 0x04;
    pub const CENTRE: u32 = 0x10;
    pub const BOUND: u32 = 0x20;
    pub const LEAST: u32 = 0x24;
    pub const EXTRA: u32 = 0x28;
    pub const BUILT: u32 = 0x30;
    pub const CAPACITY: u32 = 0x34;
    pub const RIDER_SEARCH: u32 = 0x7bc;
    pub const RIDER_RAYS: u32 = 0x7c4;
    pub const RIDER_RECORD: u32 = 0x7d0;
}

/// A prop's physics object (tests/ragdoll_ball.rs): its body and its collision object.
mod obj {
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

/// The riders (actor type 2) in the game manager's actor list, each with a collision object
/// (tests/traffic.rs).
fn riders_of(ee: &Ee) -> Vec<u32> {
    let mut riders = Vec::new();
    let mut a = ee.u32_at(ee.u32_at(ee.u32_at(addr("game_manager").unwrap()) + 0x34) + 0x34);
    while a != 0 && riders.len() < 16 {
        let actor = ee.u32_at(a);
        if actor != 0 && ee.u32_at(actor) == 2 && ee.u32_at(actor + search_at::RIDER_RECORD) != 0 {
            riders.push(actor);
        }
        a = ee.u32_at(a + 0x8);
    }
    riders
}

/// Body `n`'s place among every collection's objects, in order (F7's index past `first`).
fn body_index(set: &PropSet, (c, o): (usize, usize)) -> usize {
    set.collections[..c].iter().map(|c| c.objects.len()).sum::<usize>() + o
}

/// How many probes [`the_bikes_list_holds_the_props_at_rest_as_the_originals`] stands.
const PROBES: usize = 2_000;

/// **The bike's list holds the props at rest as the original's** (F8): the original's gather on the
/// rider's bike search (`+0x7bc`) round its body rays (`+0x7c4`), stood within 40 ft of one of
/// free09's sleeping props (three probes in four) or one of its trees, and the port's riding gather
/// with the props in its world ([`flight_search::gather_knocked`]), list the same scene meshes, plants
/// and sleeping props in the same order with the same distances, round the same centre in the same
/// bound; then, the rays moved within the bound, both keep the list and measure it again. What else
/// the original lists (the trigger boxes, the bikes, the shack's hierarchy, the moving list's) is
/// left out of the comparison, as is what else the port's lists.
#[test]
fn the_bikes_list_holds_the_props_at_rest_as_the_originals() {
    let Some(mut ee) = state() else { return };
    let Some(free) = free09(&ee) else { return };
    let Some(set) = knockables() else { return };
    let (props, trees) = (&free.props.props, &free.trees);
    let rider = *riders_of(&ee).first().expect("a rider in the state");
    let search = ee.u32_at(rider + search_at::RIDER_SEARCH);
    let rays = ee.u32_at(rider + search_at::RIDER_RAYS);
    assert_eq!(
        (ee.f32_at(search + search_at::LEAST), ee.f32_at(search + search_at::EXTRA), ee.i32_at(search + search_at::CAPACITY)),
        (30.0, 50.0, 256),
        "the bike's search"
    );
    let tree = ee.u32_at(ee.u32_at(ee.u32_at(rays + at::SPACE) + tree_at::TABLE) + tree_at::FIXED);
    let first = Statics::knock_from(props, Some(trees));
    let mut index = listed_index(&ee, &free.props, trees, tree);
    let sleeping = knock_index(&ee, &set, tree, first);
    eprintln!("{} sleeping props in the state's tree", sleeping.len());
    index.extend(sleeping);
    let resting: Vec<Vf> = resting_all(&set, first, Vf::new(0.0, 0.0, 0.0, 1.0)).iter().map(|(_, p)| posed(&set, p).0).collect();

    let terrain = free.terrain();
    let ground = Ground::new(&terrain, &free.scene.terrain);
    let field = ground.field();
    let (lo, hi) = (field.min(), field.max());
    let bounds = Some(([F(lo.x()), F(lo.z())], [F(hi.x()), F(hi.z())]));
    let reach = F(ee.f32_at(rays + at::REACH));
    let gather = addr("collision_search_gather").unwrap();
    let found = |ee: &Ee| -> (Vec<(usize, u32)>, u32) {
        let (a, n) = (ee.u32_at(search + search_at::FOUND), ee.u32_at(search + search_at::COUNT));
        let list = (0..n).filter_map(|k| index.get(&ee.u32_at(a + 8 * k)).map(|&i| (i, ee.u32_at(a + 8 * k + 4)))).collect();
        (list, n)
    };

    game_sim::prop_wheel::riding(Some(&set), |knocks, named| {
        assert!(knocks.asleep.iter().all(|&a| a), "free09's props all asleep");
        let groups_at = flight_search::FIRST + props.len();
        let knocks_at = groups_at + trees.groups().len() + trees.props.props.len();
        // A port collidable by the index the original's entries map to: a scene mesh, a plant, a
        // sleeping prop past `first`.
        let mapped = |o: usize| -> Option<usize> {
            if o >= knocks_at {
                Some(first + body_index(&set, named[o - knocks_at]))
            } else if (groups_at..groups_at + trees.groups().len()).contains(&o) {
                Some(props.len() + o - groups_at)
            } else {
                (flight_search::FIRST..groups_at).contains(&o).then(|| o - flight_search::FIRST)
            }
        };
        let ours = |s: &broad_phase::Search| -> Vec<(usize, u32)> {
            s.nearby.found.iter().filter_map(|c| Some((mapped(c.object)?, c.distance.to_bits()))).collect()
        };
        let woods = trunked(trees);
        let mut pick = Pick(0xf008_f008_ba11_5eed);
        let (mut compared, mut kept, mut full, mut most, mut listed, mut mixed) = (0, 0, 0, 0, 0, 0);
        for case in 0..PROBES {
            let around = if case % 4 == 3 { plant_at(trees, woods[pick.upto(woods.len() as u64) as usize]) } else { resting[pick.upto(resting.len() as u64) as usize] };
            let probe = Vf::new(around.x() + pick.range(-40.0, 40.0), around.y() + pick.range(-10.0, 10.0), around.z() + pick.range(-40.0, 40.0), 1.0);
            set_vf(&mut ee, rays + at::CENTRE, probe);
            ee.set_u32(search + search_at::BUILT, 0);
            let mut port = broad_phase::Search { least: F(30.0), extra: F(50.0), capacity: 256, ..broad_phase::Search::default() };
            for step in 0..2 {
                let mut now = probe;
                if step == 1 {
                    // Moved so the rays' reach stays inside the bound: the list is kept.
                    let room = port.bound.0 - reach.0;
                    if !(room > 0.5) {
                        break;
                    }
                    let d = room * 0.5;
                    now = Vf::new(probe.x() + pick.range(-d, d) * 0.7, probe.y(), probe.z() + pick.range(-d, d) * 0.7, 1.0);
                    set_vf(&mut ee, rays + at::CENTRE, now);
                }
                ee.call(gather, &[u64::from(search), u64::from(rays), 3], &[], BUDGET);
                flight_search::gather_knocked(Some(&free.props), Some(trees), knocks, bounds, now, reach, &mut port, &[]);
                let (theirs, n) = found(&ee);
                if n as usize >= 256 {
                    full += 1;
                    break;
                }
                let what = format!("case {case} step {step}: round {now:?}");
                let mine = ours(&port);
                assert_eq!(mine, theirs, "{what}: the props, plants and props at rest, in order, with their distances");
                assert_eq!(port.centre.map(F::to_bits), bits(vf(&ee, search + search_at::CENTRE)), "{what}: the centre");
                assert_eq!(port.bound.to_bits(), ee.u32_at(search + search_at::BOUND), "{what}: the bound");
                compared += 1;
                kept += usize::from(step == 1);
                most = most.max(theirs.len());
                if step == 0 {
                    let knocked = mine.iter().filter(|&&(i, _)| i >= first).count();
                    listed += knocked;
                    mixed += usize::from(knocked > 0 && mine.iter().any(|&(i, _)| i < first));
                }
            }
        }
        eprintln!("{compared} lists compared ({kept} kept), {full} full, at most {most} entries in one; {listed} props at rest listed; {mixed} lists mix them with scenery or plants");
        assert!(compared > PROBES && kept > PROBES / 3 && listed > PROBES && mixed > 100, "the cases did not reach far enough");
    });
}
