//! S97 — the ecosystem's trunks, against the original.
//!
//! - **The builder** ([`eco::build`]) is the engine's collision-tree builder: every node and facet it
//!   writes, and the vertices it moves in place, bit for bit over thousands of made-up meshes and
//!   trunks, by calling the original's.
//! - **The trunks** of free09 are the ones the original built: each tree species' mesh in the
//!   recording2 save state — vertices, nodes and facets — against [`eco::cylinder`] off the state's
//!   own `{cos, sin}` table and the solid record the state holds, which is the one the disc's
//!   `free09.esb` gives ([`eco::Ecosystem::parse`]), plants and all.
//! - **The ride** stops at a trunk: the disc's 125 ridden flat out at a free09 tree on a flat floor
//!   comes to rest against it instead of riding through.
//!
//! Skipped with a line where the disc, the kit or the state is not there.

use game_render::disc::Disc;
use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::eco::{self, Built, Ecosystem, Solid, Trees, Wooded};
use game_sim::world::{Flat, World};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, game_or_skip, skip};

const STATE: &str = "recording2.p2m2_SaveState.eeMemory.bin";

/// Where the builder test puts its inputs and hands out its heap.
const VERTS: u32 = 0x0100_0000;
const TRIS: u32 = 0x0108_0000;
const MATRIX: u32 = 0x010f_0000;
const OUT: u32 = 0x010f_0100;
const HEAP: u32 = 0x0110_0000;
const HEAP_END: u32 = 0x0170_0000;

/// The original's layouts: a mesh collidable's tree, a species' solids, the ecosystem's plants.
mod at {
    /// A pointer to the corners, three words each.
    pub const MESH_VERTS: u32 = 0x220;
    pub const MESH_NODES: u32 = 0x234;
    pub const MESH_FACETS: u32 = 0x238;
    pub const MESH_VERT_COUNT: u32 = 0x23c;
    pub const MESH_TRI_COUNT: u32 = 0x23e;
    pub const SPECIES: u32 = 0x94;
    pub const CELL: u32 = 0xd4;
    pub const PLANTS: u32 = 0x68;
    pub const PLANT_COUNT: u32 = 0x64;
    pub const PLANT_SIZE: u32 = 0x1c;
    pub const SOLID_COUNT: u32 = 0x194;
    pub const SOLIDS: u32 = 0x198;
    pub const SOLID_SIZE: u32 = 0x30;
    pub const COLLIDABLES: u32 = 0x19c;
    pub const NODE_SIZE: u32 = 0x24;
    pub const FACET_SIZE: u32 = 0x14;
}

/// A deterministic generator. Plain host arithmetic: these are only inputs.
struct Gen(u64);

#[allow(clippy::float_arithmetic)]
impl Gen {
    fn bits(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }
    fn unit(&mut self) -> f32 {
        self.bits() as f32 / 4294967296.0
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    fn below(&mut self, n: u32) -> u32 {
        self.bits() % n
    }
}

/// A made-up mesh: a few dozen corners, some of them shared, some triangles flat or repeated, at a
/// scale anywhere from an inch to a mile; or now and then a cylinder of any size.
#[allow(clippy::float_arithmetic)]
fn made_up(g: &mut Gen, curve: &CurveTable) -> (Vec<Vf>, Vec<[i32; 3]>) {
    if g.below(5) == 0 {
        let solid = Solid {
            kind: eco::CYLINDER,
            at: [F(g.range(-50.0, 50.0)), F(g.range(-5.0, 5.0)), F(g.range(-50.0, 50.0))],
            radius: F(g.range(0.1, 10.0)),
            height: F(g.range(1.0, 80.0)),
        };
        return eco::cylinder(&solid, curve);
    }
    let scale = [0.1, 1.0, 10.0, 1000.0][g.below(4) as usize];
    let n = 3 + g.below(30) as usize;
    let grid = g.below(3) == 0;
    let verts: Vec<Vf> = (0..n)
        .map(|_| {
            let mut c = || if grid { g.below(5) as f32 * scale } else { g.range(-scale, scale) };
            let (x, y, z) = (c(), c(), c());
            Vf::new(x, y, z, 1.0)
        })
        .collect();
    let t = 2 + g.below(40) as usize;
    let tris = (0..t)
        .map(|_| {
            let a = g.below(n as u32) as i32;
            let b = g.below(n as u32) as i32;
            let c = if g.below(20) == 0 { b } else { g.below(n as u32) as i32 };
            [a, b, c]
        })
        .collect();
    (verts, tris)
}

/// The original's builder over one mesh: what it wrote, read back in the port's shapes.
fn theirs(ee: &mut Ee, verts: &[Vf], tris: &[[i32; 3]]) -> Built {
    for (i, v) in verts.iter().enumerate() {
        for (l, b) in v.to_bits().iter().enumerate() {
            ee.set_u32(VERTS + 16 * i as u32 + 4 * l as u32, *b);
        }
    }
    for (i, t) in tris.iter().enumerate() {
        for (l, k) in t.iter().enumerate() {
            ee.set_u32(TRIS + 12 * i as u32 + 4 * l as u32, *k as u32);
        }
    }
    ee.set_u32(HEAP_END, HEAP);
    let args = [OUT, OUT + 4, MATRIX, VERTS, TRIS, tris.len() as u32, verts.len() as u32].map(u64::from);
    ee.call(addr("collision_tree_build").unwrap(), &args, &[], 2_000_000_000);
    let (nodes, facets) = (ee.u32_at(OUT), ee.u32_at(OUT + 4));
    read_tree(ee, nodes, facets, tris.len(), (0..verts.len() as u32).map(|i| VERTS + 16 * i))
}

/// A tree as the original lays it out in memory: `count - 1` nodes, `count` facets, and the vertices
/// at `verts` (three words each).
fn read_tree(ee: &Ee, nodes: u32, facets: u32, count: usize, verts: impl Iterator<Item = u32>) -> Built {
    let f = |a: u32| F(f32::from_bits(ee.u32_at(a)));
    let v3 = |a: u32| [f(a), f(a + 4), f(a + 8)];
    Built {
        nodes: (0..count as u32 - 1)
            .map(|i| {
                let n = nodes + at::NODE_SIZE * i;
                eco::BuiltNode {
                    volume: f(n),
                    radius: f(n + 4),
                    children: [ee.u16_at(n + 8), ee.u16_at(n + 0xa)],
                    centre: v3(n + 0xc),
                    half: v3(n + 0x18),
                }
            })
            .collect(),
        facets: (0..count as u32)
            .map(|i| {
                let t = facets + at::FACET_SIZE * i;
                let packed = [ee.u16_at(t + 0xc), ee.u16_at(t + 0xe), ee.u16_at(t + 0x10)];
                // Unpacked the way the port's facet packs: a sixteen-thousandth per step.
                let normal = packed.map(|p| F(p as i16 as f32 / 16384.0));
                eco::Facet { vertices: [ee.u16_at(t), ee.u16_at(t + 2), ee.u16_at(t + 4)], distance: f(t + 8), normal }
            })
            .collect(),
        vertices: verts.map(|a| { let [x, y, z] = v3(a); Vf::new(x.0, y.0, z.0, 0.0) }).collect(),
    }
}

/// Bit by bit, with the first difference named: nodes, facets (packed normals) and vertices' `xyz`.
fn same(what: &str, ours: &Built, theirs: &Built) -> Result<(), String> {
    let bits3 = |v: [F; 3]| v.map(|c| c.to_bits());
    if ours.nodes.len() != theirs.nodes.len() {
        return Err(format!("{what}: {} nodes, theirs {}", ours.nodes.len(), theirs.nodes.len()));
    }
    for (i, (a, b)) in ours.nodes.iter().zip(&theirs.nodes).enumerate() {
        let ka = (a.volume.to_bits(), a.radius.to_bits(), a.children, bits3(a.centre), bits3(a.half));
        let kb = (b.volume.to_bits(), b.radius.to_bits(), b.children, bits3(b.centre), bits3(b.half));
        if ka != kb {
            return Err(format!("{what}: node {i}: ours {a:?}, theirs {b:?}"));
        }
    }
    for (i, (a, b)) in ours.facets.iter().zip(&theirs.facets).enumerate() {
        let pa = a.packed().map(|p| p as u16);
        let pb = b.normal.map(|n| (n.0 * 16384.0) as i16 as u16);
        if (a.vertices, a.distance.to_bits(), pa) != (b.vertices, b.distance.to_bits(), pb) {
            return Err(format!("{what}: facet {i}: ours {a:?} {pa:x?}, theirs {b:?} {pb:x?}"));
        }
    }
    for (i, (a, b)) in ours.vertices.iter().zip(&theirs.vertices).enumerate() {
        if a.to_bits()[..3] != b.to_bits()[..3] {
            return Err(format!("{what}: vertex {i}: ours {a:?}, theirs {b:?}"));
        }
    }
    Ok(())
}

/// The game with a heap of its own: the builder's two allocators hand out a bump of `HEAP`, whose
/// next free byte is kept at `HEAP_END`, and the free does nothing.
fn builder() -> Option<Ee> {
    let mut ee = game_or_skip()?;
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    for name in ["malloc", "operator_new_array"] {
        ee.hooks.insert(addr(name).unwrap(), Box::new(|ee| {
            let size = (ee.r(4) as u32 + 15) & !15;
            let at = ee.u32_at(HEAP_END);
            assert!(at + size < HEAP_END, "the test's heap is full");
            ee.set_u32(HEAP_END, at + size);
            ee.set(2, u64::from(at));
        }));
    }
    ee.hooks.insert(addr("free").unwrap(), Box::new(|_| {}));
    Some(ee)
}

/// `cases` made-up meshes through both builders, from `seed`.
fn builds_alike(seed: u64, cases: usize) {
    let Some(mut ee) = builder() else { return };
    let curve = CurveTable::build();
    let mut g = Gen(seed);
    let mut cylinders = 0;
    for case in 0..cases {
        let (verts, tris) = made_up(&mut g, &curve);
        cylinders += usize::from(tris.len() == 32 && verts.len() == 32);
        let ours = eco::build(&verts, &tris);
        let theirs = theirs(&mut ee, &verts, &tris);
        if let Err(e) = same(&format!("case {case} ({} corners, {} triangles)", verts.len(), tris.len()), &ours, &theirs) {
            panic!("{e}");
        }
    }
    println!("{cases} meshes built alike, {cylinders} of them trunks");
    assert!(cylinders > cases / 10);
}

/// Six hundred meshes, a fifth of them trunks: about forty seconds of the original.
#[test]
fn the_tree_builder_is_the_engines() {
    builds_alike(0x5397, 600);
}

/// Ten thousand more, for a run by hand (`--ignored`): ten minutes of the original.
#[test]
#[ignore]
fn the_tree_builder_is_the_engines_ten_thousand_times() {
    builds_alike(0x5397_0001, 10_000);
}

/// The recording2 state's RAM, or a line saying why not.
fn free09() -> Option<Ee> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let path = kit.ctx("states").join(STATE);
    if !path.exists() {
        skip(&format!("{} is not there: `./kit state ram` of recording2.p2m2_SaveState.p2s", path.display()));
        return None;
    }
    Some(Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}")))
}

/// The disc's free09.esb, read.
fn free09_esb() -> Option<Ecosystem> {
    let disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let member = disc.entries().iter().find(|e| e.name.to_ascii_lowercase().ends_with("\\free09.res"))?.name.clone();
    let mut disc = disc;
    let res = disc.res(&member).expect("free09's archive");
    let bytes = res.read_suffix("free09.esb").expect("free09.esb");
    Some(Ecosystem::parse(&bytes).expect("free09.esb parses"))
}

/// The curve table the state's game built.
fn curve_of(ee: &Ee) -> CurveTable {
    let table = ee.u32_at(addr("curve_table_ptr").unwrap());
    let entries: Vec<[f32; 2]> = (0..CURVE_ENTRIES as u32)
        .map(|i| [f32::from_bits(ee.u32_at(table + 8 * i)), f32::from_bits(ee.u32_at(table + 8 * i + 4))])
        .collect();
    CurveTable::from_entries(&entries)
}

#[test]
fn free09s_trunks_are_the_ones_the_original_built() {
    let Some(ee) = free09() else { return };
    assert_eq!(ee.f32_at(addr("curve_table_invstep").unwrap()), 954.77045, "the table's index scale");
    let curve = curve_of(&ee);
    let world = ee.u32_at(addr("world_manager").unwrap());
    let species = ee.u32_at(world + at::SPECIES);
    let esb = free09_esb();
    let mut trunks = 0;
    // The game keeps the file's slot n in its table's n - 1 (the file's slot 0 is empty on every
    // track this has seen), and a plant's species byte likewise.
    let file_slot = |game: u32| game as usize + 1;
    for slot in 0..255u32 {
        let sp = ee.u32_at(species + 4 * slot);
        if sp == 0 {
            if let Some(esb) = &esb {
                assert!(esb.species[file_slot(slot)].is_none(), "slot {slot}: the file has a species the game has not");
            }
            continue;
        }
        let count = ee.u32_at(sp + at::SOLID_COUNT);
        let file = esb.as_ref().map(|e| e.species[file_slot(slot)].clone().expect("the file has the slot"));
        if let Some(file) = &file {
            let name = (0..32).map(|i| ee.u8_at(sp + i)).take_while(|&c| c != 0).map(char::from).collect::<String>();
            assert_eq!(file.model, name, "game slot {slot}: the species");
        }
        if let Some(file) = &file {
            assert_eq!(file.solids.len() as u32, count, "slot {slot}: the solids");
        }
        for k in 0..count {
            let r = ee.u32_at(sp + at::SOLIDS) + at::SOLID_SIZE * k;
            let f = |a: u32| F(f32::from_bits(ee.u32_at(a)));
            let solid = Solid { kind: ee.u32_at(r + 0x20), at: [f(r), f(r + 4), f(r + 8)], radius: f(r + 0x24), height: f(r + 0x28) };
            if let Some(file) = &file {
                assert_eq!(file.solids[k as usize], solid, "slot {slot}: the file's solid {k} is the game's");
            }
            assert_eq!(solid.kind, eco::CYLINDER, "slot {slot}: every solid on free09 is a cylinder");
            let mesh = ee.u32_at(ee.u32_at(sp + at::COLLIDABLES) + 4 * k);
            assert_eq!((ee.u16_at(mesh + at::MESH_VERT_COUNT), ee.u16_at(mesh + at::MESH_TRI_COUNT)), (32, 32));
            let (corners, tris) = eco::cylinder(&solid, &curve);
            let verts = ee.u32_at(mesh + at::MESH_VERTS);
            let theirs = read_tree(
                &ee,
                ee.u32_at(mesh + at::MESH_NODES),
                ee.u32_at(mesh + at::MESH_FACETS),
                32,
                (0..32).map(|i| verts + 12 * i),
            );
            // The mesh keeps the corners from before the builder's transform.
            let mut ours = eco::build(&corners, &tris);
            ours.vertices = corners.clone();
            same(&format!("slot {slot} solid {k}"), &ours, &theirs).unwrap_or_else(|e| panic!("{e}"));
            trunks += 1;
        }
    }
    println!("{trunks} trunks of free09 built alike");
    assert_eq!(trunks, 9, "free09 has nine tree species with a trunk each, and two bushes");

    // The plants the game read are the file's.
    let Some(esb) = esb else { return };
    let n = ee.u32_at(world + at::PLANT_COUNT);
    assert_eq!(n as usize, esb.plants.len(), "the plants");
    let plants = ee.u32_at(world + at::PLANTS);
    for (i, p) in esb.plants.iter().enumerate() {
        let a = plants + at::PLANT_SIZE * i as u32;
        let theirs = ([ee.u16_at(a + 8), ee.u16_at(a + 0xa), ee.u16_at(a + 0xc)], ee.u8_at(a + 0xe) as usize, ee.u8_at(a + 0x11) as i8);
        assert_eq!((p.cell, usize::from(p.species), p.width), (theirs.0, file_slot(u32::try_from(theirs.1).unwrap()), theirs.2), "plant {i}");
    }
    assert_eq!(ee.f32_at(world + at::CELL), 0.10546875);
    assert_eq!(eco::cell_size(F(6912.0)).0, 0.10546875);
}

/// free09's ecosystem with its plants swapped for one tree of `slot` standing at `(x, z)`.
#[allow(clippy::float_arithmetic)]
fn one_tree(esb: &Ecosystem, slot: u8, x: f32, z: f32) -> Ecosystem {
    let cell = eco::cell_size(F(6912.0)).0;
    let mut eco = esb.clone();
    let mut plant = *esb.plants.iter().find(|p| p.species == slot).expect("a plant of the species");
    plant.cell = [(x / cell) as u16, 0, (z / cell) as u16];
    eco.plants = vec![plant];
    eco
}

#[test]
fn a_free09_trunk_turns_a_put_back_away_within_twenty_feet() {
    let Some(esb) = free09_esb() else { return };
    let eco = one_tree(&esb, 1, 3000.0, 3000.0);
    let (trees, missed) = Trees::new(&eco, eco::cell_size(F(6912.0)), &CurveTable::build());
    assert!(missed.is_empty(), "{missed:?}");
    assert_eq!(trees.props.props.len(), 1);
    let near = Vf::new(3000.0, 0.0, 3019.0, 1.0);
    let far = Vf::new(3000.0, 0.0, 3021.0, 1.0);
    assert!(trees.crowded(near, F(30.0)), "19 feet off a trunk");
    assert!(!trees.crowded(far, F(30.0)), "21 feet off a trunk");
}

#[test]
fn riding_into_a_free09_tree_stops_the_bike_at_its_trunk() {
    use game_sim::ride::Controls;
    use game_sim::rider::{Rider, Tables};
    use game_sim::trick::Set;
    let Some(esb) = free09_esb() else { return };
    let Some((phy, wheels)) = the_125() else { return };
    // free09's TreeB (the file's slot 1), 1.5 feet round, standing 120 feet up the floor from the start.
    let tree_z = 120.0;
    let eco = one_tree(&esb, 1, 0.0, tree_z);
    let (trees, _) = Trees::new(&eco, eco::cell_size(F(6912.0)), &CurveTable::build());
    let trunk_z = trees.props.props[0].centre.z();
    let floor = || Flat { y: F::ZERO, normal: Vf::new(0.0, 1.0, 0.0, 0.0) };
    // Each tick's distance up the floor and speed along it.
    let ride = |world: &mut dyn World| {
        let tables = Tables::build();
        let mut rider = Rider::new(&phy, wheels, Vf::new(0.0, 1.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
        let gas = Controls { gas: F::ONE, ..Controls::default() };
        (0..30 * 6)
            .map(|_| {
                let f = rider.frame(&phy, &tables, &gas, F(game_sim::tick::DeltaTime::CAP_30), world);
                assert!(f.wreck.is_none(), "no wreck: the head ray never reaches a trunk the bike is stopped against");
                (rider.ride.position().z(), rider.ride.chassis.vel.z())
            })
            .collect::<Vec<(f32, f32)>>()
    };
    // Where a ride flat out first falls from over 50 ft/s to under 10.
    let stop = |ticks: &[(f32, f32)]| {
        let fast = ticks.iter().position(|t| t.1 > 50.0)?;
        ticks[fast..].iter().find(|t| t.1 < 10.0).map(|t| t.0)
    };
    let open = ride(&mut floor());
    let mut wooded = Wooded { world: floor(), trees };
    let blocked = ride(&mut wooded);
    println!("open floor: stops at {:?}, {:.1} ft by the end", stop(&open), open.last().unwrap().0);
    println!("a trunk at z {trunk_z}: stops at {:?}", stop(&blocked));
    assert_eq!(stop(&open), None, "the open floor never stops the bike");
    assert!(open.last().unwrap().0 > tree_z + 100.0, "and takes it past where the tree stands");
    let at = stop(&blocked).expect("the tree stops the bike");
    assert!(at < trunk_z - 1.5 && at > tree_z - 10.0, "at its trunk: the centre of mass {at} ft up the floor");
}

/// The disc's `race125p.phy` and the `125_faisst` wheels, as tests/modifiers.rs loads them.
fn the_125() -> Option<(game_sim::phy::Phy, (Vf, Vf))> {
    use game_sim::archive::{Psr, Res};
    use std::io::{Read, Seek, SeekFrom};
    let tables = game_sim::rider::Tables::build();
    let path = match Kit::load() {
        Ok(k) => k.ctx("iso").join("MX.PSR"),
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let Ok(mut file) = std::fs::File::open(&path) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", path.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    file.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let mut member = |name: &str| -> Option<Vec<u8>> {
        let e = psr.find(name)?;
        file.seek(SeekFrom::Start(e.at)).ok()?;
        let mut bytes = vec![0u8; e.size as usize];
        file.read_exact(&mut bytes).ok()?;
        Some(bytes)
    };
    let leaf = |res: &Res, want: &str| {
        res.entries
            .iter()
            .find(|e| {
                let n = e.name.to_ascii_lowercase();
                n == want || n.ends_with(&format!("\\{want}"))
            })
            .cloned()
    };
    let archives: Vec<String> = psr
        .entries
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.starts_with("bike\\") && n.ends_with(".res")
        })
        .collect();
    let (mut phy, mut wheels) = (None, None);
    for archive in &archives {
        let Some(bytes) = member(archive) else { continue };
        let Ok(res) = Res::parse(&bytes) else { continue };
        if phy.is_none()
            && let Some(e) = leaf(&res, "race125p.phy")
        {
            phy = res.read(&bytes, &e).ok().map(|b| game_sim::phy::Phy::read_bytes(&b, &tables.ride.curve));
        }
        if wheels.is_none() && archive.to_ascii_lowercase().contains("125_faisst") {
            let Some(e) = leaf(&res, "bike.psg") else { continue };
            let Ok(psg) = res.read(&bytes, &e) else { continue };
            let Ok(model) = game_render::psg::Model::parse(&psg) else { continue };
            let node = |name: &str| -> Option<Vf> {
                let i = model.nodes.iter().position(|n| n.name == name)?;
                let mut root = i;
                while let Some(up) = model.nodes[root].parent {
                    root = up;
                }
                Some(Vf(model.place_in(i, root)))
            };
            wheels = node("TireFront").zip(node("TireRear"));
        }
    }
    match phy.zip(wheels) {
        Some(found) => Some(found),
        None => {
            skip("no race125p.phy or 125_faisst wheels in MX.PSR");
            None
        }
    }
}

/// Solids of the other kinds (S102): a sphere (kind 2, MX vs. ATV's) is a part no search the port
/// runs takes a contact from — the plant keeps it, as `None`, and still counts as solid; a kind no
/// disc has (0, 3) is reported and kept the same way.
#[test]
fn a_sphere_is_a_part_with_no_trunk_and_other_kinds_are_reported() {
    let Some(esb) = free09_esb() else { return };
    let mut eco = one_tree(&esb, 1, 3000.0, 3000.0);
    let species = eco.species[1].as_mut().expect("TreeB");
    let trunk = species.solids[0].clone();
    species.solids = vec![Solid { kind: eco::SPHERE, ..trunk.clone() }, trunk.clone(), Solid { kind: 3, ..trunk }];
    let (trees, missed) = Trees::new(&eco, eco::cell_size(F(6912.0)), &CurveTable::build());
    assert_eq!(missed.len(), 1, "only the kind-3 solid is reported: {missed:?}");
    assert!(missed[0].contains("kind 3"), "{missed:?}");
    assert_eq!(trees.props.props.len(), 1, "only the cylinder is built");
    let g = &trees.groups()[0];
    assert_eq!(g.parts, vec![None, Some(0), None], "the parts in the species' order");
    assert!(g.solid);
    assert!(trees.crowded(Vf::new(3000.0, 0.0, 3019.0, 1.0), F(30.0)), "a plant with solids still turns a put-back away");
}

/// S101 — the per-plant bend blocks. The ecosystem has a pool of 0x50-byte blocks (a bend
/// quaternion at +0x10, an angle at +0x30, a state byte at +0x40) that a plant points at through
/// +0x13 when its +0x12 bit 1 is set, and its frame and draw passes read one; but the pool is only
/// made when the scene's `[Environment]` has `UseEcoPhysics` (the reader's default is 0), sized by
/// `EcoMaxPhysicsEco`, and nothing in the game ever writes a block but the pool's constructor or sets
/// a plant's bit 1. So the original's trees never bend, hit or not, and the port's do not either.
mod bend {
    pub const POOL: u32 = 0x80;
    pub const POOL_COUNT: u32 = 0xe8;
    pub const PHYSICS_ON: u32 = 0x1d88;
    pub const PROBE: u32 = 0x1d8c;
    pub const GATHER: u32 = 0x1d90;
    pub const FLAGS: u32 = 0x12;
    pub const BENT: u8 = 2;
    pub const BLOCK: u32 = 0x13;
    /// The four keys of the reader's ecosystem-physics part, in the order it reads them.
    pub const KEYS: [&str; 4] = ["ecosystemwindstrength", "ecosystemwindgustrate", "useecophysics", "ecomaxphysicseco"];
}

#[test]
fn no_scene_on_the_disc_turns_the_ecosystems_physics_on() {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return;
        }
    };
    let members: Vec<String> =
        disc.entries().iter().map(|e| e.name.clone()).filter(|n| n.to_ascii_lowercase().ends_with(".res")).collect();
    let mut wooded = Vec::new();
    for member in &members {
        let Ok(res) = disc.res(member) else { continue };
        for scene in res.entries().iter().filter(|e| e.name.to_ascii_lowercase().ends_with(".scn")) {
            let bytes = res.read(&scene.name).expect("the scene");
            let text = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
            let mut section = "";
            for line in text.lines().map(str::trim) {
                if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                    section = name;
                    continue;
                }
                let Some((key, value)) = line.split_once('=') else { continue };
                let key = key.trim();
                assert!(!bend::KEYS.contains(&key), "{member} {}: [{section}] {key}={value}", scene.name);
                if section == "environment" && key == "ecosystemfile" && !value.trim().is_empty() && value.trim() != "none" {
                    wooded.push(format!("{member} {}", scene.name));
                }
            }
        }
    }
    println!("{} scenes with an ecosystem, none with its physics on", wooded.len());
    assert!(wooded.len() >= 30, "the disc's free and national scenes name their ecosystems: {wooded:?}");
    assert!(wooded.iter().any(|s| s.to_ascii_lowercase().contains("free09")), "{wooded:?}");
}

#[test]
fn the_originals_ecosystems_hold_no_bend_state() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return;
        }
    };
    // free09 ridden (recording2), free06 flown (the biplane) and free06 ridden (S89's bike state).
    let states = ["recording2.p2m2_SaveState.eeMemory.bin", "biplane.p2m2_SaveState.eeMemory.bin", "s89-along.eeMemory.bin"];
    let mut read = 0;
    for name in states {
        let path = kit.ctx("states").join(name);
        if !path.exists() {
            skip(&format!("{} is not there", path.display()));
            continue;
        }
        let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
        let world = ee.u32_at(addr("world_manager").unwrap());
        assert_ne!(world, 0, "{name}: an ecosystem");
        for (what, at) in [("pool", bend::POOL), ("pool size", bend::POOL_COUNT), ("physics switch", bend::PHYSICS_ON), ("probe", bend::PROBE), ("gatherer", bend::GATHER)] {
            assert_eq!(ee.u32_at(world + at), 0, "{name}: the ecosystem's {what}");
        }
        let (plants, n) = (ee.u32_at(world + at::PLANTS), ee.u32_at(world + at::PLANT_COUNT));
        assert!(n > 700, "{name}: {n} plants");
        for i in 0..n {
            let p = plants + at::PLANT_SIZE * i;
            assert_eq!((ee.u8_at(p + bend::FLAGS) & bend::BENT, ee.u8_at(p + bend::BLOCK)), (0, 0), "{name}: plant {i}");
        }
        println!("{name}: {n} plants, no pool, none bent");
        read += 1;
    }
    if read == 0 {
        skip("no state to read");
    }
}
