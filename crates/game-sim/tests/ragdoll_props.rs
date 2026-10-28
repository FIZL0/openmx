//! The ragdoll's bones against the track's static props (S165), on S89's s89-along save state of
//! free06 and the disc's scene meshes.
//!
//! 1. **the list**: the original's gather on a rider body actor's bones' search, round its first
//!    sphere stood near one of free06's props, lists the same scene meshes in the same order with
//!    the same distances, round the same centre in the same bound, as the port's
//!    [`BikeSearch::gather_among`], gathered fresh and then kept and measured again; and the pair
//!    filter passes the bone sphere to every scene mesh in the tree;
//! 2. **the sphere test**: a bone's sphere collider paired with the state's scene meshes, one or two
//!    on one record, is [`StillProp::touch`] on the port's props, bit for bit;
//! 3. **the push-out**: a bone pushed out of a contact on a scene mesh is the port's against the
//!    still scenery, bit for bit;
//! 4. **the roof**: a ragdoll dropped on a building's roof lands on it, and falls through it with
//!    the props out of the world.

use std::collections::HashMap;

use game_sim::archive::{Psr, Res};
use game_sim::biplane::Body;
use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::flight_search;
use game_sim::phy::Wrecks;
use game_sim::ragdoll_contact::{Against, Contact, Sphere, moving};
use game_sim::ragdoll_scene::{BikeSearch, Cells, Statics, StillProp, Touch};
use game_sim::rng::Rng;
use game_sim::track::{Scene, Terrain};
use game_sim::world::{Ground, Place, Props, World};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

const STATE: &str = "s89-along.eeMemory.bin";
const TRACK: &str = "Tracks\\free06.res";
const BUDGET: u64 = 50_000_000;
const DT: F = F(1.0 / 30.0);

/// A collidable's fields, a sphere collider's and its record's (`research/oracle.txt` S105, S142).
mod at {
    pub const SPACE: u32 = 0x04;
    pub const CLASS: u32 = 0x02;
    pub const CENTRE: u32 = 0x40;
    pub const REACH: u32 = 0x60;
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

/// The space's table and its tree (`tests/flight_candidates.rs`).
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
}

/// A bone collision's fields and its body's (`tests/ragdoll_contact.rs`).
mod coll {
    pub const ON: u32 = 0x04;
    pub const TOUCHED: u32 = 0x14;
    pub const SWEPT: u32 = 0x20;
    pub const UNPENETRATION: u32 = 0x2c;
    pub const ROTATION: u32 = 0x30;
    pub const RESTITUTION: u32 = 0x34;
    pub const FRICTION: u32 = 0x38;
    pub const POINT: u32 = 0x40;
    pub const NORMAL: u32 = 0x50;
    pub const DV: u32 = 0x60;
    pub const OTHER: u32 = 0x70;
    pub const BODY: u32 = 0x7c;
    pub const SPHERE: u32 = 0x88;
    pub const ENABLED: u32 = 0x00;
    pub const INV_MASS: u32 = 0x10;
    pub const B_RADIUS: u32 = 0x14;
    pub const POSITION: u32 = 0x60;
    pub const VELOCITY: u32 = 0x70;
    pub const INV_INERTIA: u32 = 0x120;
    pub const QUAT: u32 = 0x170;
    pub const RATES: u32 = 0x1b0;
}

fn state() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("states").join(STATE);
    if !path.exists() {
        skip(&format!("{} is not there: `./kit state ram` of S89's s89-along save state", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

/// free06's archives, whole: the track's and its second.
fn archives() -> Option<Vec<(Vec<u8>, Res)>> {
    let kit = Kit::load().ok()?;
    let at = kit.ctx("iso").join("MX.PSR");
    let Ok(mut file) = std::fs::File::open(&at) else {
        skip(&format!("no {} — `./kit extract MX.PSR`", at.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    std::io::Read::read_exact(&mut file, &mut front).expect("the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let mut parts = Vec::new();
    for name in [TRACK.to_string(), TRACK.replace(".res", "_2.res")] {
        let Some(e) = psr.find(&name) else { continue };
        std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(e.at)).expect("seek");
        let mut bytes = vec![0u8; e.size as usize];
        std::io::Read::read_exact(&mut file, &mut bytes).expect("read");
        if let Ok(res) = Res::parse(&bytes) {
            parts.push((bytes, res));
        }
    }
    (!parts.is_empty()).then_some(parts)
}

fn member(parts: &[(Vec<u8>, Res)], want: &str) -> Option<Vec<u8>> {
    let want = want.to_ascii_lowercase();
    parts.iter().find_map(|(bytes, res)| {
        let e = res.entries.iter().find(|e| {
            let name = e.name.to_ascii_lowercase();
            name == want || name.ends_with(&format!("\\{want}")) || name.ends_with(&format!("/{want}"))
        })?;
        res.read(bytes, e).ok()
    })
}

/// free06: its scene, its terrain and its props read the way the shell reads them.
struct Free06 {
    scene: Scene,
    terrain: Vec<u8>,
    props: Props,
}

impl Free06 {
    fn terrain(&self) -> Terrain<'_> {
        Terrain::parse(&self.terrain).expect("free06's terrain")
    }
}

fn free06() -> Option<Free06> {
    let parts = archives()?;
    let scene = Scene::parse(&member(&parts, "free06.scn")?).ok()?;
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let terrain = member(&parts, &leaf)?;
    Terrain::parse(&terrain).ok()?;
    let (props, missing) = Props::load(&scene, |want| member(&parts, want).ok_or_else(|| "no archive of this track has it".to_string()));
    assert!(missing.is_empty(), "free06's props the archives lack: {missing:?}");
    Some(Free06 { scene, terrain, props })
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

/// The tree of the space `sphere` is in.
fn tree_of(ee: &Ee, sphere: u32) -> u32 {
    ee.u32_at(ee.u32_at(ee.u32_at(sphere + at::SPACE) + tree_at::TABLE) + tree_at::FIXED)
}

/// Every collidable in the tree, each once.
fn tree_collidables(ee: &Ee, tree: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let mut stack = vec![ee.u32_at(tree + tree_at::ROOT)];
    while let Some(node) = stack.pop() {
        let mut c = ee.u32_at(node + 4);
        while c != 0 {
            for k in 1..=4 {
                let o = ee.u32_at(c + 4 * k);
                if o != 0 && !out.contains(&o) {
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

/// Each of the tree's scene meshes (class 2, category [`flight_search::SCENERY`]) by its port index:
/// the prop whose box centre and reach have its bits (`tests/flight_candidates.rs`).
fn scenery_index(ee: &Ee, props: &Props, tree: u32) -> HashMap<u32, usize> {
    let mut by_place = HashMap::new();
    for (i, p) in props.props.iter().enumerate() {
        let (c, _) = flight_search::prop_box(p);
        assert!(by_place.insert((c.map(F::to_bits), p.radius.to_bits()), i).is_none(), "two of free06's props at one place");
    }
    let mut index = HashMap::new();
    for o in tree_collidables(ee, tree) {
        if ee.u8_at(o + at::CLASS) != 2 || ee.u32_at(o + at::CATEGORY) != flight_search::SCENERY {
            continue;
        }
        let key = ([0, 4, 8, 12].map(|k| ee.u32_at(o + at::CENTRE + k)), ee.u32_at(o + at::REACH));
        let i = *by_place.get(&key).unwrap_or_else(|| panic!("the state's mesh {o:#x} at {key:x?} is none of the port's props"));
        index.insert(o, i);
    }
    assert_eq!(index.len(), props.props.len(), "the state's scene meshes and free06's props");
    index
}

/// A rider body actor in the state (18 parts, the bones' search at `+0x628` with its least, extra
/// and capacity): its bones' search and its first collision's sphere.
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
            let collisions = ee.u32_at(a + search_at::ACTOR_COLLISIONS);
            let first = ee.u32_at(collisions);
            if !ram(first) || !ram(ee.u32_at(first + coll::SPHERE)) {
                continue;
            }
            return (s, ee.u32_at(first + coll::SPHERE));
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
    fn chance(&mut self, n: u64) -> bool {
        self.upto(n) == 0
    }
    fn float(&mut self, x: f32) -> f32 {
        match self.upto(16) {
            0 => 0.0,
            1 => -0.0,
            2 => -x,
            _ => self.range(-x, x),
        }
    }
    fn vector(&mut self, x: f32) -> Vf {
        Vf::new(self.float(x), self.float(x), self.float(x), self.float(2.0))
    }
    fn unit_vector(&mut self) -> Vf {
        let v = [self.range(-1.0, 1.0), self.range(-1.0, 1.0), self.range(-1.0, 1.0)];
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-3);
        Vf::new(v[0] / n, v[1] / n, v[2] / n, 0.0)
    }
    fn quat(&mut self) -> Vf {
        let v = [self.range(-1.0, 1.0), self.range(-1.0, 1.0), self.range(-1.0, 1.0), self.range(-1.0, 1.0)];
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-3);
        Vf::new(v[0] / n, v[1] / n, v[2] / n, v[3] / n)
    }
}

/// How many probes [`the_bones_search_lists_the_props_as_the_originals`] stands.
const PROBES: usize = 3_000;

/// **The bones' search lists the props as the original's** (S165): the original's gather on a rider
/// body actor's bones' search, its first sphere stood round one of free06's props (within its reach
/// and 30 ft past), and the port's, list the same scene meshes in the same order with the same
/// distances, round the same centre in the same bound; then, the sphere moved within the bound, both
/// keep the list and measure it again. The tree the port lays over free06's field is the state's,
/// and the pair filter passes the sphere to every scene mesh in it, which asks for the bound.
#[test]
fn the_bones_search_lists_the_props_as_the_originals() {
    let Some(mut ee) = state() else { return };
    let Some(free) = free06() else { return };
    let props = &free.props.props;
    let (search, sphere) = bones(&ee);
    let tree = tree_of(&ee, sphere);
    let index = scenery_index(&ee, &free.props, tree);
    let filter = addr("collision_pair_filter").unwrap();
    for &o in index.keys() {
        ee.call(filter, &[u64::from(sphere), u64::from(o), 0], &[], BUDGET);
        assert_ne!(ee.v0(), 0, "the filter refuses the bone sphere {sphere:#x} the scene mesh {o:#x}");
        assert_ne!(ee.u8_at(o + at::FLAGS) & 0x08, 0, "the scene mesh {o:#x} is listed outside the bound");
    }

    let terrain = free.terrain();
    let ground = Ground::new(&terrain, &free.scene.terrain);
    let cells = Cells::of(&ground.field()).expect("free06's field has edges");
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
    let keys = Statics::keys(&cells, props);
    let gather = addr("collision_search_gather").unwrap();
    let reach = F(ee.f32_at(sphere + at::REACH));
    let far = (Vf::new(-1.0e6, 0.0, -1.0e6, 1.0), F::ONE);
    let found = |ee: &Ee| -> (Vec<(usize, u32)>, u32) {
        let (a, n) = (ee.u32_at(search + search_at::FOUND), ee.u32_at(search + search_at::COUNT));
        let list = (0..n).filter_map(|k| index.get(&ee.u32_at(a + 8 * k)).map(|&i| (i, ee.u32_at(a + 8 * k + 4)))).collect();
        (list, n)
    };
    let ours = |s: &Statics, probe: Vf| -> Vec<(usize, u32)> {
        s.props.iter().map(|&i| (i, StillProp::of(&props[i], probe).distance.to_bits())).collect()
    };

    let mut pick = Pick(0x5165_5165_f00d_d00d);
    let (mut compared, mut kept, mut full, mut most, mut none) = (0, 0, 0, 0, 0);
    for case in 0..PROBES {
        let k = pick.upto(props.len() as u64) as usize;
        let (c, _) = flight_search::prop_box(&props[k]);
        let r = props[k].radius.0 + 30.0;
        let probe = Vf::new(c[0].0 + pick.range(-r, r), c[1].0 + pick.range(-20.0, 20.0), c[2].0 + pick.range(-r, r), 1.0);
        set_vf(&mut ee, sphere + at::CENTRE, probe);
        ee.set_u32(search + search_at::BUILT, 0);
        let mut port = BikeSearch { stood: Some(probe), ..BikeSearch::default() };
        let mut statics = Statics::default();
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
            port.gather_among(Some(&cells), reach, far, 0, false, props, &keys, &mut statics);
            let (theirs, n) = found(&ee);
            if n as usize >= 128 {
                full += 1;
                break;
            }
            let what = format!("case {case} step {step}: round {now:?} by prop {k}");
            assert_eq!(ours(&statics, now), theirs, "{what}: the props, in order, with their distances");
            assert_eq!(bits(port.centre), bits(vf(&ee, search + search_at::CENTRE)), "{what}: the centre");
            assert_eq!(port.bound.to_bits(), ee.u32_at(search + search_at::BOUND), "{what}: the bound");
            compared += 1;
            kept += usize::from(step == 1);
            most = most.max(theirs.len());
            none += usize::from(theirs.is_empty());
        }
    }
    eprintln!("{compared} lists compared ({kept} kept and measured again, {none} empty), {full} full, at most {most} props in one");
    assert!(compared > PROBES && kept > PROBES / 3 && most > 3 && none < compared / 2, "the cases did not reach far enough");
}

/// A point near the prop's mesh: one of its vertices through its frame.
fn near_vertex(prop: &StillProp, pick: &mut Pick) -> Vf {
    let n = match &prop.mesh.vertices {
        game_sim::track::Points::Wide(v) => v.len(),
        game_sim::track::Points::Packed(v) => v.len(),
    };
    let i = pick.upto(n as u64) as u16;
    let v = game_sim::collide::MeshRef::of(&prop.mesh).vertex(i);
    prop.frame.place(Vf::new(v.x(), v.y(), v.z(), 1.0))
}

/// How many spheres [`the_sphere_test_on_a_prop_answers_as_the_originals`] stands.
const SPHERES: usize = 20_000;

/// **The sphere test on the props answers as the original's** (S165): 20,000 spheres stood near a
/// vertex of one of free06's props (the bones' radii and random ones from a tenth to a metre), each
/// through the original's pair on a bone's sphere collider with an emptied record, with the prop
/// alone, with it and then the prop nearest it, or with the same prop twice; and through the port's
/// props, each offered the record's hit so far. The record's count, what it is with, the plane
/// distance it holds, and the contact's point and normal in the world, bit for bit.
#[test]
fn the_sphere_test_on_a_prop_answers_as_the_originals() {
    let Some(mut ee) = state() else { return };
    let Some(free) = free06() else { return };
    let props = &free.props.props;
    let (_, sphere) = bones(&ee);
    let index = scenery_index(&ee, &free.props, tree_of(&ee, sphere));
    let of: HashMap<usize, u32> = index.iter().map(|(&o, &i)| (i, o)).collect();
    let (clear, pair) = (addr("sphere_collider_clear").unwrap(), addr("sphere_collider_pair").unwrap());
    let still: Vec<StillProp> = props.iter().map(|p| StillProp::of(p, Vf::new(0.0, 0.0, 0.0, 1.0))).collect();
    // Each prop's nearest other: the one whose sphere's reach is nearest its centre.
    let nearest: Vec<usize> = (0..props.len())
        .map(|i| {
            let d = |j: usize| {
                let (a, b) = (props[i].centre, props[j].centre);
                let v = [a.x() - b.x(), a.y() - b.y(), a.z() - b.z()];
                (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() - props[j].radius.0
            };
            (0..props.len()).filter(|&j| j != i).min_by(|&a, &b| d(a).total_cmp(&d(b))).unwrap()
        })
        .collect();
    let mut pick = Pick(0x5165_0000_5ea7_ed00);
    let (mut hits, mut second, mut kept) = (0, 0, 0);
    for case in 0..SPHERES {
        let k = pick.upto(props.len() as u64) as usize;
        let order: Vec<usize> = match pick.upto(3) {
            0 => vec![k],
            1 => vec![k, nearest[k]],
            _ => vec![k, k],
        };
        let p = near_vertex(&still[k], &mut pick);
        let radius = match case % 5 {
            0 => 0.4f32,
            1 => 0.75,
            2 => 0.5,
            _ => pick.range(0.1, 1.0),
        };
        let off = pick.unit_vector();
        let reach = 2.0 * radius * pick.unit();
        let centre = Vf::new(p.x() + off.x() * reach, p.y() + off.y() * reach, p.z() + off.z() * reach, 1.0);
        set_vf(&mut ee, sphere + at::CENTRE, centre);
        set_vf(&mut ee, sphere + at::CENTRE2, centre);
        ee.set_f32(sphere + at::RADIUS, radius);
        ee.set_f32(sphere + at::RADIUS2, ps2_float::fpu::mul(radius, radius));
        ee.call(clear, &[u64::from(sphere)], &[], 1_000_000);
        for &c in &order {
            ee.call(pair, &[u64::from(sphere), u64::from(of[&c])], &[], BUDGET);
        }
        let rec = ee.u32_at(sphere + at::RECORD);
        let count = ee.u32_at(rec + at::COUNT);
        let original = (count > 0).then(|| {
            (ee.u32_at(rec + at::WITH), ee.u32_at(rec + at::HELD), bits(vf(&ee, rec + at::POINT)), bits(vf(&ee, rec + at::NORMAL)))
        });

        let mut best: Option<(usize, Touch)> = None;
        for &c in &order {
            if let Some(t) = still[c].touch(centre, F(radius), best.map(|(_, b)| b.distance)) {
                best = Some((c, t));
            }
        }
        let port = best.map(|(c, t)| (of[&c], t.distance.to_bits(), bits(t.world.0), bits(t.world.1)));
        let what = format!("case {case}: props {order:?}, sphere at {centre:?} r {radius}");
        assert!(count <= 1, "{what}: {count} contacts");
        assert_eq!(port, original, "{what}");
        if let Some((c, _)) = best {
            hits += 1;
            second += usize::from(order.len() == 2 && c == order[1] && order[0] != order[1]);
            kept += usize::from(order.len() == 2 && c == order[0]);
        }
    }
    eprintln!("{hits} of {SPHERES} hit; the second prop took {second}, the first kept {kept}");
    assert!(hits > SPHERES / 5 && hits < SPHERES * 9 / 10, "{hits} hits");
    assert!(second > 20 && kept > 1000, "the second prop took {second} records, the first kept {kept}");
}

/// The bone collisions in the state: each one whose sphere (`+0x88`) is a sphere collider of the
/// bones' sphere's class and whose body (`+0x7c`) has the sphere's radius.
fn collisions(ee: &Ee, sphere: u32) -> Vec<u32> {
    let class = ee.u32_at(sphere + 0x38);
    let ram = |a: u32| (0x10_0000..0x1ff_fe00).contains(&a) && a % 4 == 0;
    (0x10_0000u32..0x1ff_fe00)
        .step_by(4)
        .filter(|&a| {
            let (s, b) = (ee.u32_at(a + coll::SPHERE), ee.u32_at(a + coll::BODY));
            ram(s) && ram(b) && ee.u32_at(s + 0x38) == class && ee.u32_at(b + coll::B_RADIUS) == ee.u32_at(s + at::RADIUS)
        })
        .collect()
}

fn curve(ee: &Ee) -> CurveTable {
    let base = ee.u32_at(addr("curve_table_ptr").unwrap());
    let entries: Vec<[f32; 2]> =
        (0..CURVE_ENTRIES as u32).map(|k| [ee.f32_at(base + 8 * k), ee.f32_at(base + 8 * k + 4)]).collect();
    CurveTable::from_entries(&entries)
}

/// **The push-out against a prop is the original's against the scenery** (S165): 10,000 bones
/// generated on the state's bone collisions, each pushed out of a contact near one of free06's props
/// with the contact's other the prop's scene mesh. The original asks the mesh how it moves and finds
/// it still; the port pushes against the still scenery, as a contact on a [`StillProp`] is. The
/// bone's position, velocity, orientation and rates, and the contact's change of velocity, bit for
/// bit.
#[test]
fn the_push_out_against_a_prop_is_the_originals() {
    let Some(mut ee) = state() else { return };
    let Some(free) = free06() else { return };
    let props = &free.props.props;
    let (_, sphere) = bones(&ee);
    let index = scenery_index(&ee, &free.props, tree_of(&ee, sphere));
    let of: HashMap<usize, u32> = index.iter().map(|(&o, &i)| (i, o)).collect();
    let table = curve(&ee);
    let push_out = addr("bone_collision_push_out").unwrap();
    let found = collisions(&ee, sphere);
    assert!(found.len() >= 9, "{} bone collisions in the state", found.len());
    let (point_at, normal_at) = (0x01f8_0000u32, 0x01f8_0010u32);
    let mut pick = Pick(0x5165_0f0f_5165);
    for case in 0..10_000 {
        let c = found[pick.upto(found.len() as u64) as usize];
        let b = ee.u32_at(c + coll::BODY);
        let k = pick.upto(props.len() as u64) as usize;
        let around = props[k].centre;
        let pos = Vf::new(around.x() + pick.range(-20.0, 20.0), around.y() + pick.range(-5.0, 10.0), around.z() + pick.range(-20.0, 20.0), pick.float(2.0));
        let radius = pick.range(0.2, 1.0);
        ee.set_u32(b + coll::ENABLED, 1);
        ee.set_f32(b + coll::INV_MASS, pick.range(0.05, 6.0));
        ee.set_f32(b + coll::B_RADIUS, radius);
        set_vf(&mut ee, b + coll::POSITION, pos);
        set_vf(&mut ee, b + coll::VELOCITY, pick.vector(60.0));
        set_vf(&mut ee, b + coll::INV_INERTIA, Vf::new(pick.range(0.01, 6.0), pick.range(0.01, 6.0), pick.range(0.01, 6.0), 0.0));
        set_vf(&mut ee, b + coll::QUAT, pick.quat());
        set_vf(&mut ee, b + coll::RATES, pick.vector(10.0));
        ee.set_u32(c + coll::ON, 1);
        ee.set_f32(c + coll::UNPENETRATION, if pick.chance(3) { 1.0 } else { pick.range(0.0, 1.5) });
        ee.set_f32(c + coll::ROTATION, if pick.chance(3) { 1.0 } else { pick.range(0.0, 1.5) });
        ee.set_f32(c + coll::RESTITUTION, if pick.chance(2) { 0.0 } else { pick.range(0.0, 1.0) });
        ee.set_f32(c + coll::FRICTION, if pick.chance(2) { 1.0 } else { pick.range(0.0, 1.2) });
        set_vf(&mut ee, c + coll::DV, pick.vector(9.0));
        let swept = pick.chance(4);
        ee.set_u32(c + coll::SWEPT, u32::from(swept));
        let d = pick.unit_vector();
        let r = radius * pick.range(-0.5, 1.5);
        let point = Vf::new(pos.x() + d.x() * r, pos.y() + d.y() * r, pos.z() + d.z() * r, 1.0);
        let normal = Vf::new(-d.x(), -d.y(), -d.z(), 1.0);
        set_vf(&mut ee, point_at, point);
        set_vf(&mut ee, normal_at, normal);
        set_vf(&mut ee, c + coll::POINT, point);
        set_vf(&mut ee, c + coll::NORMAL, normal);
        ee.set_u32(c + coll::OTHER, of[&k]);

        let f = |o: u32| F(ee.f32_at(c + o));
        let sphere = Sphere {
            bone: 0,
            radius: F(radius),
            unpenetration: f(coll::UNPENETRATION),
            rotation: f(coll::ROTATION),
            restitution: f(coll::RESTITUTION),
            friction: f(coll::FRICTION),
            sweep: false,
            sweep_back: F::ZERO,
        };
        let mut body = Body {
            enabled: true,
            inv_mass: F(ee.f32_at(b + coll::INV_MASS)),
            position: vf(&ee, b + coll::POSITION),
            velocity: vf(&ee, b + coll::VELOCITY),
            inv_inertia: vf(&ee, b + coll::INV_INERTIA),
            quat: vf(&ee, b + coll::QUAT),
            rates: vf(&ee, b + coll::RATES),
            ..Body::default()
        };
        let mut contact = Contact {
            touched: ee.u32_at(c + coll::TOUCHED) != 0,
            ground: false,
            scene: false,
            swept,
            point,
            normal,
            dv: vf(&ee, c + coll::DV),
        };
        contact.push_out_against(&sphere, &mut body, point, normal, moving(point, Against::Still), &table);
        ee.call(push_out, &[u64::from(c), u64::from(point_at), u64::from(normal_at)], &[], BUDGET);

        let what = format!("case {case}: prop {k} ({:#x}), swept {swept}", of[&k]);
        for (field, got, want) in [
            ("position", body.position, vf(&ee, b + coll::POSITION)),
            ("velocity", body.velocity, vf(&ee, b + coll::VELOCITY)),
            ("quat", body.quat, vf(&ee, b + coll::QUAT)),
            ("rates", body.rates, vf(&ee, b + coll::RATES)),
            ("dv", contact.dv, vf(&ee, c + coll::DV)),
        ] {
            assert_eq!(bits(got), bits(want), "{what}: {field} is {want:?} in the game, {got:?} in the port");
        }
    }
}

// --- the ride ---------------------------------------------------------------------------------------

/// The ragdoll's kit as the shell builds it (`tests/ragdoll_cars.rs`'s `ride_kit`).
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

/// **A roof to drop on**: of free06's props, the one with the widest box whose face straight down
/// its box's centre faces up and stands more than 10 ft over the ground there. The prop and the
/// face's point.
fn a_roof(free: &Free06) -> (usize, Vf) {
    let terrain = free.terrain();
    let mut ground = Place { ground: Ground::new(&terrain, &free.scene.terrain), props: Props::default() };
    let mut best: Option<(f32, usize, Vf)> = None;
    for (i, p) in free.props.props.iter().enumerate() {
        let (c, h) = flight_search::prop_box(p);
        let width = h[0].0.min(h[2].0);
        if width < 6.0 || best.is_some_and(|b| width <= b.0) {
            continue;
        }
        let (x, z) = (c[0].0, c[2].0);
        let s = StillProp::of(p, Vf::new(x, 0.0, z, 1.0));
        // The highest face under (x, z): a foot's sphere let down the box half a foot at a time.
        let mut y = c[1].0 + h[1].0 + 2.0;
        let roof = loop {
            if y < c[1].0 - h[1].0 - 2.0 {
                break None;
            }
            if let Some(t) = s.touch(Vf::new(x, y, z, 1.0), F::ONE, None) {
                break Some(t.world);
            }
            y -= 0.5;
        };
        let Some((point, normal)) = roof else { continue };
        let floor = ground.ground(Vf::new(x, point.y(), z, 1.0)).0;
        if normal.y() > 0.95 && point.y() - floor.y() > 10.0 {
            best = Some((width, i, point));
        }
    }
    let (_, i, point) = best.expect("a prop with a roof");
    (i, point)
}

/// The main bone over `ticks` ticks of a ragdoll entered from the riding pose, its hands let go and
/// every bone moved so the main bone is `above` the roof's point, still; with free06's props in the
/// world or without them. The wreck bike stays 200 ft off. Answers the main bone each tick, the roof's
/// point and the prop.
fn drop(with_props: bool, above: f32, ticks: usize) -> Option<(Vec<Vf>, Vf, usize)> {
    let mut free = free06()?;
    let (kit, part, wrecks) = ride_kit()?;
    let (prop, roof) = a_roof(&free);
    let props = if with_props { std::mem::take(&mut free.props) } else { Props::default() };
    let terrain = free.terrain();
    let mut world = Place { ground: Ground::new(&terrain, &free.scene.terrain), props };

    let mut bike = game_sim::ride::Ride::default();
    bike.chassis.quat = Vf::new(0.0, 0.0, 0.0, 1.0);
    bike.origin = Vf::new(roof.x() + 200.0, roof.y() + 40.0, roof.z() + 200.0, 1.0);
    let crash = game_sim::ride::Crash::begin(&bike, game_sim::trick::Wreck(5), &part, game_sim::ride::WRECK_INERTIA, &wrecks, None);
    let mut doll = game_sim::ragdoll_ride::Down::enter(&kit, &bike.transform(), &crash, crash.throw, 5, &wrecks, &mut Rng::default(), &mut world);
    doll.ragdoll.holds = [false; 4];
    let main = doll.main().position;
    for b in &mut doll.ragdoll.bones {
        let p = b.body.position;
        b.body.position = Vf::new(p.x() - main.x() + roof.x(), p.y() - main.y() + roof.y() + above, p.z() - main.z() + roof.z(), p.w());
        b.body.velocity = Vf::ZERO;
        b.body.rates = Vf::ZERO;
    }
    let mut seen = Vec::new();
    for _ in 0..ticks {
        doll.step_posed_with(&kit, None, DT, &crash, &mut world, false);
        seen.push(doll.main().position);
    }
    Some((seen, roof, prop))
}

/// **A ragdoll dropped on a roof lands on it** (S165): dropped from 4.5 ft over the middle of the
/// widest roof free06's scene has, it stops on the roof and stays there for two seconds; with the
/// props out of the world (the port before this task) it falls through.
#[test]
fn a_ragdoll_dropped_on_a_roof_lands_on_it() {
    let Some((on, roof, prop)) = drop(true, 4.5, 60) else { return };
    let (through, ..) = drop(false, 4.5, 60).expect("the same track");
    for (t, (a, b)) in on.iter().zip(&through).enumerate() {
        println!(
            "tick {t}: at ({:.2}, {:.2}, {:.2}); without the props y {:.2}",
            a.x() - roof.x(),
            a.y() - roof.y(),
            a.z() - roof.z(),
            b.y() - roof.y()
        );
    }
    println!("prop {prop}'s roof at {roof:?}");
    let last = on.last().unwrap();
    assert!(on.iter().all(|p| p.y() > roof.y()), "the main bone went under the roof");
    assert!(last.y() - roof.y() < 3.0, "the main bone stands {} ft over the roof", last.y() - roof.y());
    assert!(through.last().unwrap().y() < roof.y() - 5.0, "without the props the bone stayed at {}", through.last().unwrap().y() - roof.y());
}
