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
//! - **The original's ride** into one (S100): two PCSX2 captures of the 125 flat out at a free09
//!   trunk — where its rays find it, on which facet, and the wreck — beside the port's same ride.
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
    let Some(col) = the_body_rays() else { return };
    let col = Some(col);
    // Each tick's distance up the floor, speed along it, and the wreck it enters.
    let ride = |world: &mut dyn World| {
        let tables = Tables::build();
        let mut rider = Rider::new(&phy, wheels, Vf::new(0.0, 1.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
        let gas = Controls { gas: F::ONE, ..Controls::default() };
        if let Some(col) = &col {
            rider.carries_bike_col(col.clone());
        }
        (0..30 * 6)
            .map(|_| {
                let f = rider.frame(&phy, &tables, &gas, F(game_sim::tick::DeltaTime::CAP_30), world);
                (rider.ride.position().z(), rider.ride.chassis.vel.z(), f.wreck)
            })
            .collect::<Vec<(f32, f32, Option<game_sim::trick::Wreck>)>>()
    };
    // Where a ride flat out (over 50 ft/s) first loses more than 15 ft/s in a tick: the original's
    // 125 goes 58 -> 35 on the trunk (S100), and a wreck freezes the chassis there.
    let stop = |ticks: &[(f32, f32, Option<game_sim::trick::Wreck>)]| {
        ticks.windows(2).find(|w| w[0].1 > 50.0 && w[0].1 - w[1].1 > 15.0).map(|w| w[1].0)
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
    // Over the bars, on the tick it stops, as the original does (S100, S126).
    let hit = blocked.iter().position(|t| t.0 == at).unwrap();
    let wreck = blocked.iter().position(|t| t.2.is_some());
    println!("a trunk at z {trunk_z}: wreck {:?} on tick {wreck:?}, the stop on tick {hit}", wreck.map(|w| blocked[w].2));
    assert_eq!(wreck, Some(hit), "the trunk wrecks the rider on the tick it stops the bike");
    assert_eq!(blocked[hit].2, Some(game_sim::trick::Wreck(1)), "over the bars");
    assert!(open.iter().all(|t| t.2.is_none()), "the open floor wrecks nobody");
}

/// S100's captures: recording2's 125 moved 60 ft short of free09's plant 347 (a TreeB at
/// 4968.84, 180.35, 3087.89) on the flat by it, pointed at its trunk, and ridden on full gas — once as
/// the state is, and once with the original's wrecks switched off (`wrecks_enabled` cleared in the
/// state) to see the contact and what the bike does after it
/// (`research/journal/2026-10-26-s100-eco-tree-ride/`).
const TREE_RIDE: &str = "s100/tree.csv";
const TREE_RIDE_NO_WRECKS: &str = "s100/nowreck.csv";
const TREE_PLANT: usize = 347;

/// A capture's rows, by column name.
struct Rows {
    names: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Rows {
    fn read(path: &std::path::Path) -> Option<Rows> {
        let text = std::fs::read_to_string(path).ok()?;
        let mut lines = text.lines();
        let names = lines.next()?.split(',').map(str::to_owned).collect();
        let rows = lines.map(|l| l.split(',').map(str::to_owned).collect()).collect();
        Some(Rows { names, rows })
    }

    fn f(&self, row: usize, name: &str) -> f32 {
        let i = self.names.iter().position(|n| n == name).unwrap_or_else(|| panic!("no column {name}"));
        self.rows[row][i].parse().unwrap_or_else(|_| panic!("row {row} {name}: {:?}", self.rows[row][i]))
    }

    fn v(&self, row: usize, name: &str) -> [f32; 3] {
        [self.f(row, &format!("{name}.x")), self.f(row, &format!("{name}.y")), self.f(row, &format!("{name}.z"))]
    }
}

/// **The original against a free09 trunk, and the port beside it** (S100).
///
/// What the original does, from the two captures:
/// - its front body rays find the trunk on the tick the bike reaches it, at 58 ft/s with the centre of
///   mass 3.1 ft short of the axis: two scenery contacts on the facet facing the bike, the point on
///   the facet's plane of the trunk this port builds for the plant;
/// - that one tick takes the bike from 58 to 35 ft/s, and the impact load it leaves (g_peak
///   (-9.2, 14.2, -28.9)) is past the 125's front limit: wreck 1, over the bars, on that tick;
/// - with wrecks off the bike rears up against the trunk, comes round, and under the same gas rides
///   away the way it came.
///
/// The port (S126), its 125 carrying its body rays, from the capture's first place and from ten ticks
/// out at the original's speed there: the rays find the trunk, their contacts' resolve leaves a
/// fore-aft load past the front limit, and the bike wrecks over the bars on that tick. Its run-up is
/// quicker than the original's (P154), so the tick it reaches the trunk on, and so the depth of the
/// rays in it and the load, are its own; `a_free09_trunk_wrecks_as_the_original_does` in
/// tests/replay.rs rides the same hit from the original's state.
#[test]
#[allow(clippy::float_arithmetic)]
fn a_free09_tree_against_the_original() {
    use game_sim::ride::Controls;
    use game_sim::rider::{Rider, Tables};
    use game_sim::trick::Set;
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => return skip(&why),
    };
    let read = |name: &str| {
        let path = kit.ctx("capture").join(name);
        let rows = Rows::read(&path);
        if rows.is_none() {
            skip(&format!("no {} — S100's capture/capture.sh records it", path.display()));
        }
        rows
    };
    let (Some(wrecks), Some(free)) = (read(TREE_RIDE), read(TREE_RIDE_NO_WRECKS)) else { return };
    let Some(esb) = free09_esb() else { return };
    let Some((phy, wheels)) = the_125() else { return };
    let plant = esb.plants[TREE_PLANT];
    let cell = eco::cell_size(F(6912.0)).0;
    let tree = [f32::from(plant.cell[0]) * cell, f32::from(plant.cell[1]) * cell, f32::from(plant.cell[2]) * cell];
    let off = |p: [f32; 3]| ((p[0] - tree[0]).powi(2) + (p[2] - tree[2]).powi(2)).sqrt();

    // The original. Both runs are one ride until the trunk: the same state, the same tape.
    let first = free.f(0, "frame_counter");
    let tick = |r: usize| (free.f(r, "frame_counter") - first) as usize;
    let touch = (0..free.rows.len()).find(|&r| free.f(r, "col_count") > 0.0).expect("the original's rays find the trunk");
    let before = touch - 1;
    let (speed, short) = (free.f(before, "ch_vel.z"), off(free.v(before, "ch_cofg")));
    println!(
        "original: the rays find the trunk on tick {} at {speed:.2} ft/s, the centre of mass {short:.2} ft off \
         its axis; {} contacts; after the tick {:?}",
        tick(touch),
        free.f(touch, "col_count"),
        free.v(touch, "ch_vel"),
    );
    assert!(speed > 55.0 && (2.5..4.0).contains(&short), "flat out into the trunk, nose first");
    // The contacts are the trunk's: scenery, found by the bike's own rays, on the facet facing the
    // bike, at a point on that facet's plane.
    let solid = esb.species[usize::from(plant.species)].as_ref().expect("the plant's species").solids[0];
    let (corners, tris) = eco::cylinder(&solid, &CurveTable::build());
    let corner = |i: i32| {
        let c = corners[i as usize];
        [c.x() + tree[0], c.y() + tree[1], c.z() + tree[2]]
    };
    for c in 0..free.f(touch, "col_count") as usize {
        let n = free.v(touch, &format!("c{c}_normal"));
        let p = free.v(touch, &format!("c{c}_point"));
        assert_eq!((free.f(touch, &format!("c{c}_kind")), free.f(touch, &format!("c{c}_other"))), (0.0, 0.0), "own, scenery");
        let facing = tris
            .iter()
            .map(|t| {
                let (a, b, d) = (corner(t[0]), corner(t[1]), corner(t[2]));
                let (u, w) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [d[0] - a[0], d[1] - a[1], d[2] - a[2]]);
                let x = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
                let l = (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt();
                let x = [x[0] / l, x[1] / l, x[2] / l];
                // Outwards, whichever way the winding has it.
                let s = if x[0] * (a[0] - tree[0]) + x[2] * (a[2] - tree[2]) < 0.0 { -1.0 } else { 1.0 };
                ([s * x[0], s * x[1], s * x[2]], a)
            })
            .map(|(x, a)| (x[0] * n[0] + x[1] * n[1] + x[2] * n[2], x, a))
            .fold((f32::MIN, [0.0; 3], [0.0; 3]), |best, t| if t.0 > best.0 { t } else { best });
        let (dot, x, a) = facing;
        let plane = (p[0] - a[0]) * x[0] + (p[1] - a[1]) * x[1] + (p[2] - a[2]) * x[2];
        println!("  contact {c}: normal {n:?}, the port's facet {x:?} (cos {dot:.6}), the point {plane:+.4} ft off its plane");
        assert!(dot > 0.99999, "contact {c}: the original's normal is a facet of the port's trunk");
        assert!(plane.abs() < 0.01, "contact {c}: and its point is on that facet");
        assert!(n[2] < -0.9, "contact {c}: the facet facing the bike");
    }
    // With wrecks on, that tick is the wreck's: over the bars, on the impact load.
    let hit = (0..wrecks.rows.len()).find(|&r| wrecks.f(r, "wreck_reason") != 0.0).expect("the original wrecks");
    let g = wrecks.v(hit, "g_peak");
    println!("original: wreck {} at frame {}, g_peak {g:?}", wrecks.f(hit, "wreck_reason"), wrecks.f(hit, "frame_counter"));
    assert_eq!(wrecks.f(hit, "wreck_reason"), 1.0, "wreck 1, over the bars");
    assert!((wrecks.f(hit, "frame_counter") - free.f(touch, "frame_counter")).abs() <= 1.0, "on the tick the trunk is found");
    assert!(g[2] < -25.0, "on the fore-aft impact load");
    // And with them off the bike comes round and rides away the way it came, gas still held.
    let last = free.rows.len() - 1;
    let fwd = [free.f(last, "bike_xform.m20"), free.f(last, "bike_xform.m22")];
    println!("original, wrecks off: {:.1} ft off the trunk at the end, going {:?}, facing {fwd:?}", off(free.v(last, "ch_cofg")), free.v(last, "ch_vel"));
    assert!(free.f(last, "ch_vel.z") < -40.0 && fwd[1] < -0.8, "back the way it came");

    // The port: the same bike from the capture's first place and heading, on a flat floor at the
    // plant's foot, with that one plant standing, carrying its body rays.
    let mut eco = esb.clone();
    eco.plants = vec![plant];
    assert!(Trees::new(&eco, eco::cell_size(F(6912.0)), &CurveTable::build()).1.is_empty(), "every solid built");
    let Some(col) = the_body_rays() else { return };
    let tables = Tables::build();
    let dt = F(game_sim::tick::DeltaTime::CAP_30);
    // One port ride from capture row `from`, at the original's speed there when `moving`, for `ticks`.
    let port = |from: usize, moving: bool, ticks: usize| {
        let floor = Flat { y: F(tree[1]), normal: Vf::new(0.0, 1.0, 0.0, 0.0) };
        let mut world = Wooded { world: floor, trees: Trees::new(&eco, eco::cell_size(F(6912.0)), &CurveTable::build()).0 };
        let (fx, fz) = (free.f(from, "bike_xform.m20"), free.f(from, "bike_xform.m22"));
        let n = (fx * fx + fz * fz).sqrt();
        let start = free.v(from, "ch_cofg");
        let mut rider =
            Rider::new(&phy, wheels, Vf::new(start[0], tree[1] + 1.0, start[2], 1.0), Vf::new(fx / n, 0.0, fz / n, 0.0), Set::default());
        rider.carries_bike_col(col.clone());
        if moving {
            let v = free.v(from, "ch_vel");
            rider.ride.chassis.vel = Vf::new(v[0], 0.0, v[2], 0.0);
            let speed = (v[0] * v[0] + v[2] * v[2]).sqrt();
            for w in [&mut rider.ride.front, &mut rider.ride.rear] {
                w.mv.spin = F(speed * w.mv.inv_radius.0);
            }
        }
        let gas = Controls { gas: F::ONE, ..Controls::default() };
        let idle = Controls::default();
        (0..ticks)
            .map(|t| {
                // What the body rays find from where the last tick left the bike, as the frame's own
                // search will.
                let found = game_sim::rider_contact::scenery(&col, &rider.ride.transform(), &mut world, dt)
                    .map_or(0, |(c, _)| c.iter().filter(|c| c.other.is_none()).count());
                let f = rider.frame(&phy, &tables, if t == 0 && !moving { &idle } else { &gas }, dt, &mut world);
                let c = rider.ride.chassis;
                Tick { at: [c.cofg_world.x(), c.cofg_world.y(), c.cofg_world.z()], vel: c.vel.z(), found, wreck: f.wreck, g: rider.ride.mv.g_peak }
            })
            .collect::<Vec<Tick>>()
    };
    let report = |what: &str, ours: &[Tick]| {
        let hit = ours.iter().position(|t| t.found > 0).unwrap_or_else(|| panic!("{what}: the port's body rays find the trunk"));
        let before = if hit == 0 { None } else { Some(&ours[hit - 1]) };
        let wreck = ours.iter().position(|t| t.wreck.is_some());
        println!(
            "port {what}: the rays find the trunk on tick {hit} ({} contacts) from {:?} ft/s, {:?} ft off the axis; \
             after it {:.2} ft/s, g_peak {:?}; wreck {:?} on tick {wreck:?}",
            ours[hit].found,
            before.map(|b| b.vel),
            before.map(|b| off(b.at)),
            ours[hit].vel,
            ours[hit].g,
            wreck.map(|w| ours[w].wreck),
        );
        (hit, before.map_or(0.0, |b| b.vel), wreck)
    };
    let rest = port(0, false, free.rows.len());
    for t in [10, 20, 30, 40, 50] {
        println!("  run-up tick {t}: original {:.2} ft/s, port {:.2}", free.f((0..free.rows.len()).find(|&r| tick(r) >= t).unwrap(), "ch_vel.z"), rest[t].vel);
    }
    // The port's launch is quicker than the original's (P154), so it also rides in from ten ticks out
    // at the original's own place and speed there.
    let at_speed = port(before - 10, true, 40);
    let mut checks = Vec::new();
    for (what, ours) in [("from rest", &rest), ("at the original's speed 10 ticks out", &at_speed)] {
        let (hit, speed_in, wreck) = report(what, ours);
        checks.push((what, hit, speed_in, wreck, ours[hit].g, ours[hit].wreck));
    }
    for (what, hit, speed_in, wreck, g, kind) in checks {
        assert!(speed_in > 50.0, "{what}: flat out into the trunk");
        assert_eq!(wreck, Some(hit), "{what}: the port wrecks on the tick its rays find the trunk, as the original does");
        assert_eq!(kind, Some(game_sim::trick::Wreck(1)), "{what}: wreck 1, over the bars");
        assert!(g.z() < -phy.air.front_wreck_speed_limit.0, "{what}: on the fore-aft impact load, past the 125's front limit");
    }
}

/// One tick of the port's ride into the trunk.
struct Tick {
    at: [f32; 3],
    vel: f32,
    /// Scenery contacts the body rays found from where the last tick left the bike.
    found: usize,
    wreck: Option<game_sim::trick::Wreck>,
    g: Vf,
}

/// The bike's body rays and frame (S86): `Bike\Col\bikem.col` and `bikef.col`, out of the split
/// track's archive as tests/roof_landing.rs reads them.
fn the_body_rays() -> Option<std::sync::Arc<game_sim::bike_col::BikeCol>> {
    use game_sim::bike_col::{self, BikeCol};
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let res = disc.res("Tracks\\free09_ss_2.res").expect("the split track's archive");
    let read = |name: String| res.read_suffix(&name).unwrap_or_else(|e| panic!("{e}"));
    let col = BikeCol::load(&read(bike_col::rays_file(bike_col::MODEL)), &read(bike_col::shape_file(bike_col::MODEL)))
        .expect("the bike's collision files");
    Some(std::sync::Arc::new(col))
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
