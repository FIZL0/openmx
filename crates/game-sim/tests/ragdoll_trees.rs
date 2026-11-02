//! The ragdoll's bones against the ecosystem's plants (S206), on recording2's save state of free09
//! (630 of its 726 placed groups are trees with a trunk) and the disc's `free09.esb`.
//!
//! 1. **the list**: the original's gather on a rider body actor's bones' search, round its first
//!    sphere stood among free09's trees, lists the same placed groups and scene meshes in the same
//!    order with the same distances, round the same centre in the same bound, as the port's
//!    [`BikeSearch::gather_wooded`], gathered fresh and then kept and measured again;
//! 2. **the sphere test**: a bone's sphere collider through the original's search pass, its list a
//!    plant, a plant and its neighbour, a plant twice, or a prop then a plant, is the port's trunks
//!    ([`StillProp::near_wooded`]) offered the record in the list's order, bit for bit;
//! 3. **the trunk**: a ragdoll thrown at a tree stops on its trunk, and goes through it with the
//!    plants out of the world.

use std::collections::HashMap;

use game_sim::archive::{Psr, Res};
use game_sim::eco::{Trees, Wooded};
use game_sim::flight_search;
use game_sim::phy::Wrecks;
use game_sim::ragdoll_scene::{self, BikeSearch, Cells, Statics, StillProp, Touch};
use game_sim::rng::Rng;
use game_sim::track::{Scene, Terrain};
use game_sim::world::{Ground, Place, Props};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

const STATE: &str = "recording2.p2m2_SaveState.eeMemory.bin";
const TRACK: &str = "Tracks\\free09.res";
const BUDGET: u64 = 50_000_000;
const DT: F = F(1.0 / 30.0);

/// A collidable's fields, a sphere collider's and its record's (`research/oracle.txt` S105, S142).
mod at {
    pub const SPACE: u32 = 0x04;
    pub const CLASS: u32 = 0x02;
    pub const CENTRE: u32 = 0x40;
    pub const REACH: u32 = 0x60;
    pub const CATEGORY: u32 = 0x84;
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

/// How many probes [`the_bones_search_lists_the_plants_as_the_originals`] stands.
const PROBES: usize = 3_000;

/// **The bones' search lists the plants as the original's** (S206): the original's gather on a rider
/// body actor's bones' search, its first sphere stood round one of free09's trees (within 40 ft on
/// the ground plane) or one of its props, and the port's, list the same placed groups and scene
/// meshes in the same order with the same distances, round the same centre in the same bound; then,
/// the sphere moved within the bound, both keep the list and measure it again. What else the
/// original lists (the trigger boxes, the knockable props, the bikes, the shack's hierarchy) is not
/// in the port's list and is left out of the comparison.
#[test]
fn the_bones_search_lists_the_plants_as_the_originals() {
    let Some(mut ee) = state() else { return };
    let Some(free) = free09(&ee) else { return };
    let (props, trees) = (&free.props.props, &free.trees);
    let (search, sphere) = bones(&ee);
    let tree = ee.u32_at(ee.u32_at(ee.u32_at(sphere + at::SPACE) + tree_at::TABLE) + tree_at::FIXED);
    let index = listed_index(&ee, &free.props, trees, tree);

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
    let gather = addr("collision_search_gather").unwrap();
    let reach = F(ee.f32_at(sphere + at::REACH));
    let far = (Vf::new(-1.0e6, 0.0, -1.0e6, 1.0), F::ONE);
    let found = |ee: &Ee| -> (Vec<(usize, u32)>, u32) {
        let (a, n) = (ee.u32_at(search + search_at::FOUND), ee.u32_at(search + search_at::COUNT));
        let list = (0..n).filter_map(|k| index.get(&ee.u32_at(a + 8 * k)).map(|&i| (i, ee.u32_at(a + 8 * k + 4)))).collect();
        (list, n)
    };
    let ours = |s: &Statics, probe: Vf| -> Vec<(usize, u32)> {
        s.props
            .iter()
            .map(|&i| {
                let d = match props.get(i) {
                    Some(p) => StillProp::of(p, probe).distance,
                    None => ragdoll_scene::group_distance(&trees.groups()[i - props.len()], trees.cell(), probe),
                };
                (i, d.to_bits())
            })
            .collect()
    };

    let woods = trunked(trees);
    let mut pick = Pick(0x5206_5206_f00d_d00d);
    let (mut compared, mut kept, mut full, mut most, mut plants, mut with_trunks) = (0, 0, 0, 0, 0, 0);
    for case in 0..PROBES {
        let around = if case % 4 == 3 {
            let k = pick.upto(props.len() as u64) as usize;
            let (c, _) = flight_search::prop_box(&props[k]);
            Vf::new(c[0].0, c[1].0, c[2].0, 1.0)
        } else {
            plant_at(trees, woods[pick.upto(woods.len() as u64) as usize])
        };
        let probe = Vf::new(around.x() + pick.range(-40.0, 40.0), around.y() + pick.range(-10.0, 10.0), around.z() + pick.range(-40.0, 40.0), 1.0);
        set_vf(&mut ee, sphere + at::CENTRE, probe);
        ee.set_u32(search + search_at::BUILT, 0);
        let mut port = BikeSearch { stood: Some(probe), ..BikeSearch::default() };
        let mut statics = Statics::default();
        let mut keys = prop_keys.clone();
        keys.extend(Statics::group_keys(&cells, trees, props.len(), probe, reach));
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
            port.gather_wooded(Some(&cells), reach, far, 0, false, props, Some(trees), &keys, &mut statics);
            let (theirs, n) = found(&ee);
            if n as usize >= 128 {
                full += 1;
                break;
            }
            let what = format!("case {case} step {step}: round {now:?}");
            assert_eq!(ours(&statics, now), theirs, "{what}: the props and plants, in order, with their distances");
            assert_eq!(bits(port.centre), bits(vf(&ee, search + search_at::CENTRE)), "{what}: the centre");
            assert_eq!(port.bound.to_bits(), ee.u32_at(search + search_at::BOUND), "{what}: the bound");
            compared += 1;
            kept += usize::from(step == 1);
            most = most.max(theirs.len());
            let listed: Vec<usize> = statics.props.iter().filter(|&&i| i >= props.len()).map(|&i| i - props.len()).collect();
            plants += listed.len();
            with_trunks += listed.iter().filter(|&&j| woods.binary_search(&j).is_ok()).count();
        }
    }
    eprintln!("{compared} lists compared ({kept} kept), {full} full, at most {most} entries in one, {plants} plants listed ({with_trunks} with a trunk)");
    assert!(compared > PROBES && kept > PROBES / 3 && most > 3 && with_trunks > PROBES, "the cases did not reach far enough");
}

/// How many spheres [`the_sphere_test_on_a_trunk_answers_as_the_originals`] stands.
const SPHERES: usize = 20_000;

/// **The sphere test on the trunks answers as the original's** (S206): 20,000 spheres stood near a
/// vertex of one of free09's trunks (the bones' radii and random ones from a tenth to a metre), each
/// through the original's search pass on a bone's sphere collider (`collide_against_search`) with a
/// list of the plant alone, the plant and then its nearest, the plant twice, or the scene mesh
/// nearest it and then the plant, each at the distance the port measures it from the sphere; and
/// through the port's listed trunks ([`StillProp::near_wooded`]), each offered the record's hit so
/// far when its distance is within the radius, as the scene does. The record's count, what it is
/// with, the plane distance it holds, and the contact's point and normal in the world, bit for bit.
#[test]
fn the_sphere_test_on_a_trunk_answers_as_the_originals() {
    let Some(mut ee) = state() else { return };
    let Some(free) = free09(&ee) else { return };
    let (props, trees) = (&free.props.props, &free.trees);
    let (search, sphere) = bones(&ee);
    let tree = ee.u32_at(ee.u32_at(ee.u32_at(sphere + at::SPACE) + tree_at::TABLE) + tree_at::FIXED);
    let index = listed_index(&ee, &free.props, trees, tree);
    let of: HashMap<usize, u32> = index.iter().map(|(&o, &i)| (i, o)).collect();
    let pass = addr("collide_against_search").unwrap();
    let part = addr("group_part").unwrap();
    // The collidable a contact names: a mesh itself, a plant its species' part.
    let names = |ee: &mut Ee, i: usize| -> u32 {
        if i < props.len() {
            return of[&i];
        }
        ee.call(part, &[u64::from(of[&i]), 0], &[], BUDGET);
        ee.v0() as u32
    };
    let woods = trunked(trees);
    let flat = |a: Vf, b: Vf| ((a.x() - b.x()).powi(2) + (a.z() - b.z()).powi(2)).sqrt();
    let nearest_plant: Vec<usize> = woods
        .iter()
        .map(|&j| {
            let a = plant_at(trees, j);
            *woods.iter().filter(|&&k| k != j).min_by(|&&x, &&y| flat(a, plant_at(trees, x)).total_cmp(&flat(a, plant_at(trees, y)))).unwrap()
        })
        .collect();
    let nearest_prop: Vec<usize> = woods
        .iter()
        .map(|&j| {
            let a = plant_at(trees, j);
            (0..props.len()).min_by(|&x, &y| StillProp::of(&props[x], a).distance.0.total_cmp(&StillProp::of(&props[y], a).distance.0)).unwrap()
        })
        .collect();
    let list_at = ee.u32_at(search + search_at::FOUND);
    let mut pick = Pick(0x5206_0000_5ea7_ed00);
    let (mut hits, mut second, mut kept, mut gated) = (0, 0, 0, 0);
    for case in 0..SPHERES {
        let w = pick.upto(woods.len() as u64) as usize;
        let j = props.len() + woods[w];
        let order: Vec<usize> = match pick.upto(4) {
            0 => vec![j],
            1 => vec![j, props.len() + nearest_plant[w]],
            2 => vec![j, j],
            _ => vec![nearest_prop[w], j],
        };
        let g = &trees.groups()[woods[w]];
        let trunk = StillProp::of(&trees.props.props[g.parts.iter().flatten().next().copied().unwrap()], Vf::new(0.0, 0.0, 0.0, 1.0));
        let n = match &trunk.mesh.vertices {
            game_sim::track::Points::Wide(v) => v.len(),
            game_sim::track::Points::Packed(v) => v.len(),
        };
        let v = game_sim::collide::MeshRef::of(&trunk.mesh).vertex(pick.upto(n as u64) as u16);
        let p = trunk.frame.place(Vf::new(v.x(), v.y(), v.z(), 1.0));
        let radius = match case % 5 {
            0 => 0.4f32,
            1 => 0.75,
            2 => 0.5,
            _ => pick.range(0.1, 1.0),
        };
        let off = pick.unit_vector();
        let reach = 2.0 * radius * pick.unit();
        let centre = Vf::new(p.x() + off.x() * reach, p.y() + off.y() * reach, p.z() + off.z() * reach, 1.0);

        // The port: the list, measured from the sphere, its trunks offered the record in order.
        let statics = Statics { props: order.clone(), bike_at: order.len() };
        let listed = StillProp::near_wooded(props, Some(trees), &statics, centre, F(radius));
        let mut best: Option<(usize, Touch)> = None;
        for (k, s) in &listed {
            if !(s.distance <= F(radius)) {
                continue;
            }
            if let Some(t) = s.touch(centre, F(radius), best.map(|(_, b)| b.distance)) {
                best = Some((order[*k], t));
            }
        }
        let distances: Vec<F> = order
            .iter()
            .map(|&i| match props.get(i) {
                Some(p) => StillProp::of(p, centre).distance,
                None => ragdoll_scene::group_distance(&trees.groups()[i - props.len()], trees.cell(), centre),
            })
            .collect();
        gated += distances.iter().filter(|d| !(d.0 <= radius)).count();

        // The original: the same list on the bones' search, the sphere's pass over it.
        set_vf(&mut ee, sphere + at::CENTRE, centre);
        set_vf(&mut ee, sphere + at::CENTRE2, centre);
        ee.set_f32(sphere + at::REACH, radius);
        ee.set_f32(sphere + at::RADIUS, radius);
        ee.set_f32(sphere + at::RADIUS2, ps2_float::fpu::mul(radius, radius));
        for (k, (&i, d)) in order.iter().zip(&distances).enumerate() {
            ee.set_u32(list_at + 8 * k as u32, of[&i]);
            ee.set_u32(list_at + 8 * k as u32 + 4, d.to_bits());
        }
        ee.set_u32(search + search_at::COUNT, order.len() as u32);
        ee.call(pass, &[u64::from(sphere), u64::from(search)], &[], BUDGET);
        let rec = ee.u32_at(sphere + at::RECORD);
        let count = ee.u32_at(rec + at::COUNT);
        let original = (count > 0).then(|| {
            (ee.u32_at(rec + at::WITH), ee.u32_at(rec + at::HELD), bits(vf(&ee, rec + at::POINT)), bits(vf(&ee, rec + at::NORMAL)))
        });
        let port = best.map(|(i, t)| (names(&mut ee, i), t.distance.to_bits(), bits(t.world.0), bits(t.world.1)));
        let what = format!("case {case}: list {order:?} at {distances:?}, sphere at {centre:?} r {radius}");
        assert!(count <= 1, "{what}: {count} contacts");
        assert_eq!(port, original, "{what}");
        if let Some((i, _)) = best {
            hits += 1;
            second += usize::from(order.len() == 2 && i == order[1] && order[0] != order[1]);
            kept += usize::from(order.len() == 2 && i == order[0]);
        }
    }
    eprintln!("{hits} of {SPHERES} hit; the second took {second}, the first kept {kept}; {gated} entries out of a sphere's reach");
    assert!(hits > SPHERES / 5 && hits < SPHERES * 9 / 10, "{hits} hits");
    assert!(kept > 1000 && gated > 1000, "the first kept {kept} records, {gated} entries gated");
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
/// every bone moved so the main bone is `back` ft from a lone tree's trunk axis (`-x` of it) at
/// `up` ft over its foot, all thrown at it at `speed` ft/s; with free09's plants in the world or
/// without them. The wreck bike stays 200 ft off. Answers the main bone each tick and the trunk's
/// axis.
fn throw(with_trees: bool, back: f32, up: f32, speed: f32, ticks: usize) -> Option<(Vec<Vf>, Vf)> {
    let ee = state()?;
    let free = free09(&ee)?;
    let (kit, part, wrecks) = ride_kit()?;
    let trees = &free.trees;
    // A tree with no other plant or prop within 30 ft, and a trunk tall enough.
    let woods = trunked(trees);
    let lone = *woods
        .iter()
        .find(|&&j| {
            let a = plant_at(trees, j);
            let others = trees.groups().iter().enumerate().any(|(k, g)| k != j && ragdoll_scene::group_distance(g, trees.cell(), a) < 30.0);
            let props = free.props.props.iter().any(|p| StillProp::of(p, a).distance < 30.0);
            !others && !props
        })
        .expect("a lone tree");
    let g = &trees.groups()[lone];
    let foot = {
        let at = |c: u16| F::from(i32::from(c)).0 * trees.cell().0;
        Vf::new(at(g.cell[0]), at(g.cell[1]), at(g.cell[2]), 1.0)
    };
    let Free09 { scene, terrain, props, trees } = free;
    let terrain = Terrain::parse(&terrain).expect("free09's terrain");
    let place = Place { ground: Ground::new(&terrain, &scene.terrain), props };
    let trees = if with_trees { trees } else { Trees::default() };
    let mut world = Wooded { world: place, trees };

    let mut bike = game_sim::ride::Ride::default();
    bike.chassis.quat = Vf::new(0.0, 0.0, 0.0, 1.0);
    bike.origin = Vf::new(foot.x() + 200.0, foot.y() + 40.0, foot.z() + 200.0, 1.0);
    let crash = game_sim::ride::Crash::begin(&bike, game_sim::trick::Wreck(5), &part, game_sim::ride::WRECK_INERTIA, &wrecks, None);
    let mut doll = game_sim::ragdoll_ride::Down::enter(&kit, &bike.transform(), &crash, crash.throw, 5, &wrecks, &mut Rng::default(), &mut world);
    doll.ragdoll.holds = [false; 4];
    let main = doll.main().position;
    for b in &mut doll.ragdoll.bones {
        let p = b.body.position;
        b.body.position = Vf::new(p.x() - main.x() + foot.x() - back, p.y() - main.y() + foot.y() + up, p.z() - main.z() + foot.z(), p.w());
        b.body.velocity = Vf::new(speed, 0.0, 0.0, 0.0);
        b.body.rates = Vf::ZERO;
    }
    let mut seen = Vec::new();
    for _ in 0..ticks {
        doll.step_posed_with(&kit, None, DT, &crash, &mut world, false);
        seen.push(doll.main().position);
    }
    Some((seen, foot))
}

/// **A ragdoll thrown at a tree stops on its trunk** (S206): thrown at 25 ft/s from 6 ft off a lone
/// tree of free09, 4 ft up, the main bone never comes nearer the trunk's axis than the trunk's
/// radius less a few tenths and stays on its near side; with the plants out of the world (the port
/// before this task) it flies through to the far side.
#[test]
fn a_ragdoll_thrown_at_a_tree_stops_on_its_trunk() {
    let Some((on, foot)) = throw(true, 6.0, 4.0, 25.0, 30) else { return };
    let (through, _) = throw(false, 6.0, 4.0, 25.0, 30).expect("the same track");
    for (t, (a, b)) in on.iter().zip(&through).enumerate() {
        println!("tick {t}: at ({:.2}, {:.2}, {:.2}) of the trunk; without the plants x {:.2}", a.x() - foot.x(), a.y() - foot.y(), a.z() - foot.z(), b.x() - foot.x());
    }
    let axis = |p: &Vf| ((p.x() - foot.x()).powi(2) + (p.z() - foot.z()).powi(2)).sqrt();
    assert!(on.iter().all(|p| p.x() < foot.x()), "the main bone went past the trunk");
    let nearest = on.iter().map(axis).fold(f32::MAX, f32::min);
    assert!(nearest > 1.5, "the main bone came {nearest} ft from the trunk's axis");
    assert!(on.iter().any(|p| axis(p) < 3.0), "the main bone never reached the trunk");
    assert!(through.last().unwrap().x() > foot.x() + 5.0, "without the plants the bone stayed at x {}", through.last().unwrap().x() - foot.x());
}
