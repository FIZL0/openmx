//! MD34: what a plane's tick costs a world of many tiles. MD23 flew the parked biplane on world_ultra
//! and its sim took 30-300 ms a tick as it sank to the ground; V96 found the cause (the plane's two
//! broad phases built over every tile's shapes, twice a tick) and gave it MD28's near sets. This
//! holds that bound: a plane's tick asks a world of tiles only for the shapes near it, never for the
//! whole world's, so what it does a tick does not grow with the world.
//!
//! The same two flights — the plane parked on the ground with the throttle open, as MD23 started
//! it, and the plane launched slow a few feet up, sinking onto the ground — over a generated world of
//! three rows of posted tiles and over the same world with six more rows beyond: the same flight to
//! the digit (its search's bookkeeping aside, see `flown`), the same questions asked of the world
//! each tick, the same shapes handed for its searches, and none of the whole world's sets asked for.
//!
//! The terrains and posts are written here ([`game_sim::tiles::generate`]); the plane's file and ray
//! probe are the disc's (`machine.res`): it skips without them.

use game_sim::archive::{Psr, Res};
use game_sim::biplane::{Area, Pad};
use game_sim::collide::{Bundle, Record};
use game_sim::contact_search::Heightfield;
use game_sim::eco::Trees;
use game_sim::flight::Flight;
use game_sim::flight_search::{self, Rays};
use game_sim::phy::{self, Plane};
use game_sim::tiles::{generate, Piece, Reach, Source, Tile, Tiled};
use game_sim::track::{self, Placement, TerrainLayout};
use game_sim::world::{Props, World};
use ps2_float::{F, Vf};
use ps2_oracle::{Kit, skip};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

const DT: F = F(1.0 / 30.0);
/// A row's terrain: 16 x 4 root cells of 16 samples at 9 feet, 2295 x 567 feet.
const CELLS: (u32, u32) = (16, 4);
const SPACING: f32 = 9.0;
const ROW: f32 = 567.0;

/// The ground's height in feet at a world point: gentle rolls.
fn relief(x: f64, z: f64) -> f64 {
    120.0 + 6.0 * (x / 97.0).sin() * (z / 61.0).cos() + 2.5 * ((x + z) / 23.0).sin()
}

/// A wall facing −z, ten feet wide and ten high, in its own space at the origin (`tests/tiles.rs`'s).
fn post() -> track::Mesh {
    use game_sim::track::{Mesh, Node, Points, Triangle};
    let v = |x: f32, y: f32| [F(x), F(y), F::ZERO];
    Mesh {
        version: 0,
        surface: None,
        transform: [
            [F::ONE, F::ZERO, F::ZERO],
            [F::ZERO, F::ONE, F::ZERO],
            [F::ZERO, F::ZERO, F::ONE],
            [F::ZERO, F::ZERO, F::ZERO],
        ],
        vertices: Points::Wide(vec![v(-5.0, 0.0), v(5.0, 0.0), v(5.0, 10.0), v(-5.0, 10.0)]),
        normals: None,
        nodes: vec![Node {
            volume: F::ZERO,
            centre: [F::ZERO, F(5.0), F::ZERO],
            half_extent: [F(5.0), F(5.0), F(0.5)],
            children: [0x8000, 0x8001],
        }],
        triangles: vec![
            Triangle { distance: F::ZERO, normal: [F::ZERO; 3], vertices: [0, 2, 1] },
            Triangle { distance: F::ZERO, normal: [F::ZERO; 3], vertices: [0, 3, 2] },
        ],
    }
}

/// Row `r`'s terrain, and a post every 40 feet over it at the ground's height, every third turned.
struct Rows;

impl Source for Rows {
    fn load(&mut self, index: usize) -> Result<Piece, String> {
        let oz = f64::from(index as f32 * ROW);
        let sp = f64::from(SPACING);
        let terrain = generate::terrain(&format!("row{index}"), CELLS, SPACING, |x, z| relief(f64::from(x) * sp, oz + f64::from(z) * sp), |x, z| {
            1 + ((x / 16 + z / 16) % 2) as u8
        });
        let (w, d) = ((CELLS.0 * 16 - 1) as f32 * SPACING, ROW);
        let mut shapes = Vec::new();
        let (mut x, mut k) = (7.0f32, 0u32);
        while x < w {
            let mut z = 11.0f32;
            while z < d {
                let y = relief(f64::from(x), oz + f64::from(z)) as f32;
                let look = if k % 3 == 0 { [0.6, 0.0, 0.8] } else { [0.0, 0.0, 1.0] };
                shapes.push((post(), Placement { file: "post.col".into(), position: [F(x), F(y), F(z)], look: look.map(F), ..Placement::default() }));
                z += 40.0;
                k += 1;
            }
            x += 40.0;
        }
        Ok(Piece { terrain, shapes })
    }
}

/// `rows` rows of [`Rows`] along `z`, each touching the next.
fn world(rows: usize) -> Tiled {
    let tiles = (0..rows)
        .map(|r| {
            let mut t = Tile {
                name: format!("row{r}"),
                at: [F::ZERO, F(r as f32 * ROW)],
                layout: TerrainLayout { width: CELLS.0, breadth: CELLS.1, width_scale: F(SPACING), breadth_scale: F(SPACING), ..TerrainLayout::default() },
                spacing: F(SPACING),
                levels: 1,
                keep: [0; 4],
                first_owner: 0,
            };
            t.keep = t.every_cell();
            t
        })
        .collect();
    Tiled::new(tiles, Box::new(Rows), Reach::DESKTOP).unwrap()
}

/// What a tick asked of the world: each question by name, how often, and the most shapes a near
/// set handed it.
#[derive(Clone, Debug, Default, PartialEq)]
struct Asked {
    calls: std::collections::BTreeMap<&'static str, u32>,
    widest: usize,
}

/// A world of tiles that writes down every question it is asked, and answers each as the world does.
struct Counted {
    world: Tiled,
    asked: Asked,
}

impl Counted {
    fn note(&mut self, what: &'static str) {
        *self.asked.calls.entry(what).or_default() += 1;
    }
}

impl World for Counted {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        self.note("ground");
        self.world.ground(at)
    }
    fn surface(&mut self, at: Vf) -> u8 {
        self.note("surface");
        self.world.surface(at)
    }
    fn wheel_surface(&mut self, at: Vf) -> Option<u8> {
        self.note("wheel_surface");
        self.world.wheel_surface(at)
    }
    fn solid(&mut self, bundle: &Bundle, record: &mut Record) -> bool {
        self.note("solid");
        self.world.solid(bundle, record)
    }
    fn groups(&mut self, bundle: &Bundle, record: &mut Record) -> bool {
        self.note("groups");
        self.world.groups(bundle, record)
    }
    fn crowded(&mut self, centre: Vf, radius: F) -> bool {
        self.note("crowded");
        self.world.crowded(centre, radius)
    }
    fn area(&self) -> Option<Area> {
        self.world.area()
    }
    fn wrap(&self) -> [bool; 2] {
        self.world.wrap()
    }
    fn off_world(&self, at: Vf, margin: F) -> bool {
        self.world.off_world(at, margin)
    }
    fn keeps(&mut self, at: Vf) -> bool {
        self.note("keeps");
        self.world.keeps(at)
    }
    fn heightfield(&mut self) -> Option<Box<dyn Heightfield + '_>> {
        self.note("heightfield");
        self.world.heightfield()
    }
    fn collision(&mut self) -> (Option<Box<dyn Heightfield + '_>>, Option<&Props>) {
        self.note("collision");
        self.world.collision()
    }
    fn wooded(&mut self) -> (Option<Box<dyn Heightfield + '_>>, Option<&Props>, Option<&Trees>) {
        self.note("wooded");
        self.world.wooded()
    }
    fn wooded_near(&mut self, at: Vf) -> (Option<Box<dyn Heightfield + '_>>, Option<&Props>, Option<&Trees>) {
        *self.asked.calls.entry("wooded_near").or_default() += 1;
        let (field, props, trees) = self.world.wooded_near(at);
        self.asked.widest = self.asked.widest.max(props.map_or(0, |p| p.props.len()));
        (field, props, trees)
    }
    fn near_set(&self, at: Vf) -> Option<[i32; 2]> {
        self.world.near_set(at)
    }
    fn moving(&mut self) -> Option<&mut Props> {
        self.note("moving");
        self.world.moving()
    }
}

/// `machine.res` out of the disc's `MX.PSR` (`tests/flight_collision.rs`'s).
fn machines() -> Option<(Res, Vec<u8>)> {
    let kit = Kit::load().ok()?;
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/iso/MX.PSR");
    let path = if here.exists() { here } else { kit.ctx("iso").join("MX.PSR") };
    let Ok(mut f) = File::open(&path) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", path.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find("machine.res").expect("machine.res is in MX.PSR");
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    Some((Res::parse(&bytes).expect("machine.res"), bytes))
}

fn member(res: &Res, bytes: &[u8], name: &str) -> Vec<u8> {
    let e = res.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name)).unwrap_or_else(|| panic!("machine.res has {name}"));
    res.read(bytes, e).expect("its bytes")
}

/// The disc's plane with its ray probe, so it searches the world's shapes.
fn the_plane() -> Option<Flight> {
    let (res, bytes) = machines()?;
    let plane = Plane::read_bytes(&member(&res, &bytes, phy::BIPLANE_PHY));
    let rays = track::Rays::parse(&member(&res, &bytes, flight_search::BIPLANE_RAYS)).expect("biplanem.col");
    Some(Flight::new(&plane).with_rays(Rays::of(&rays)))
}

/// The two starts in the world's second row, flying east along a lane between posts: parked on the
/// ground (MD23's), and launched at 60 ft/s eight feet over it.
fn starts(plane: &Flight, tables: &game_sim::ride::Tables) -> [(&'static str, Flight); 2] {
    let (x, z) = (200.0f32, ROW + 31.0);
    let y = relief(f64::from(x), f64::from(z)) as f32;
    let east = Vf::new(1.0, 0.0, 0.0, 0.0);
    let mut parked = plane.clone();
    parked.place(Vf::new(x, y, z, 1.0), east, &tables.acos);
    parked.attitude.flown = true;
    // On a line of posts, every third turned across it.
    let z = ROW + 51.0;
    let y = relief(f64::from(x), f64::from(z)) as f32;
    let mut sinking = plane.clone();
    sinking.attitude.flown = true;
    sinking.launch(Vf::new(x, y + 8.0, z, 1.0), east, Vf::new(60.0, 0.0, 0.0, 0.0), &tables.acos);
    [("parked", parked), ("sinking", sinking)]
}

/// What a plane flies as, in full: its body, its contacts, its crash state, its attitude and its air.
/// Not its search's bookkeeping: the broad phase's cells are cut from the field's box, the whole
/// world's rectangle, so how far a gathered list is good for (`nearby.bound`) and when it is gathered
/// again differ with the world's size, as they do between two disc maps (the original's tree over
/// its terrain's box). What it is gathered from is the near set, and what it finds is the same.
fn flown(f: &Flight) -> String {
    format!("{:?}", (&f.body, &f.record.contacts, &f.crash, &f.attitude, &f.air, &f.surfaces))
}

/// **The plane's tick does not grow with the world** (MD34): 450 ticks of each start on full
/// throttle over a world of 3 rows of posted tiles and over the same world 9 rows deep (three times
/// the tiles and the shapes, all of them far): the same flight each tick ([`flown`], `Debug`'s every digit), the same
/// questions asked of the world each tick, the same widest near set — a corner of the world's shapes
/// — and the whole world's sets (`collision`, `wooded`) never asked for.
#[test]
fn a_planes_tick_asks_a_world_of_tiles_only_for_the_shapes_near_it() {
    let Some(plane) = the_plane() else { return };
    let tables = game_sim::ride::Tables::build();
    let pad = Pad { forward: F::ONE, ..Pad::default() };
    for ((name, start), (_, same)) in starts(&plane, &tables).into_iter().zip(starts(&plane, &tables)) {
        let mut small = Counted { world: world(3), asked: Asked::default() };
        let mut deep = Counted { world: world(9), asked: Asked::default() };
        let (every_small, every_deep) = (small.world.every_shape().props.len(), deep.world.every_shape().props.len());
        assert!(every_deep >= 3 * every_small, "{every_deep} shapes in the deep world, {every_small} in the small");
        let (mut a, mut b) = (start, same);
        let (mut grounded, mut touched, mut widest, mut most) = (0, 0, 0, 0);
        for tick in 0..450 {
            small.asked = Asked::default();
            deep.asked = Asked::default();
            a.frame(DT, pad, &mut small, &tables);
            b.frame(DT, pad, &mut deep, &tables);
            assert_eq!(flown(&a), flown(&b), "{name} tick {tick}: the plane over 9 rows parted from the plane over 3");
            assert_eq!(small.asked, deep.asked, "{name} tick {tick}: the questions asked of the world");
            for whole in ["collision", "wooded", "heightfield"] {
                assert!(!small.asked.calls.contains_key(whole), "{name} tick {tick}: the plane asked for the whole world's `{whole}`");
            }
            grounded += usize::from(a.attitude.grounded);
            touched += usize::from(!a.record.contacts.is_empty());
            widest = widest.max(deep.asked.widest);
            most = most.max(deep.asked.calls.values().sum::<u32>());
        }
        println!(
            "{name}: ended at {:?}, crash {}; {grounded} ticks grounded, {touched} touching; the widest near set {widest} of {every_deep} shapes; at most {most} questions a tick",
            a.body.position, a.crash.reason
        );
        assert!(grounded > 30 || touched > 30, "{name}: the plane must come down among the posts ({grounded} grounded, {touched} touching)");
        assert!(widest > 0 && widest * 8 < every_deep, "{name}: a near set must be a corner of the world: {widest} of {every_deep}");
    }
}

/// [`Counted`] as the wreck's search took a world before V99: its near set is the whole world's
/// ([`World::wooded`]), wherever it is asked.
struct Whole(Counted);

impl World for Whole {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        self.0.ground(at)
    }
    fn surface(&mut self, at: Vf) -> u8 {
        self.0.surface(at)
    }
    fn solid(&mut self, bundle: &Bundle, record: &mut Record) -> bool {
        self.0.solid(bundle, record)
    }
    fn area(&self) -> Option<Area> {
        self.0.area()
    }
    fn wrap(&self) -> [bool; 2] {
        self.0.wrap()
    }
    fn off_world(&self, at: Vf, margin: F) -> bool {
        self.0.off_world(at, margin)
    }
    fn heightfield(&mut self) -> Option<Box<dyn Heightfield + '_>> {
        self.0.heightfield()
    }
    fn collision(&mut self) -> (Option<Box<dyn Heightfield + '_>>, Option<&Props>) {
        self.0.collision()
    }
    fn wooded(&mut self) -> (Option<Box<dyn Heightfield + '_>>, Option<&Props>, Option<&Trees>) {
        self.0.wooded()
    }
    fn wooded_near(&mut self, _: Vf) -> (Option<Box<dyn Heightfield + '_>>, Option<&Props>, Option<&Trees>) {
        self.0.wooded()
    }
}

/// The disc's wreck bike: its physics part out of free09's second track archive
/// (`tests/wreck_bike.rs`'s).
fn the_wreck_part() -> Option<phy::Part> {
    let kit = Kit::load().ok()?;
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/iso/MX.PSR");
    let path = if here.exists() { here } else { kit.ctx("iso").join("MX.PSR") };
    let Ok(mut f) = File::open(&path) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", path.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find("Tracks\\free09_ss_2.res").expect("free09_ss_2.res is in MX.PSR");
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    let res = Res::parse(&bytes).expect("free09_ss_2.res");
    Some(phy::Part::read_bytes(&member(&res, &bytes, "bike\\wreckbike.phy"), "PhysicsPart1"))
}

/// What a wreck is, in full (`Debug`'s every digit), but for its search's bookkeeping: the shape the
/// sweep back struck last is named by its place in the set the search was handed, which is another
/// number in a near set than in the whole world's, so it is named here by where it stands.
fn wrecked(crash: &game_sim::ride::Crash, set: Option<&Props>) -> String {
    let mut c = crash.clone();
    let struck = c.bike.record.struck.take().map(|i| set.expect("a set to strike in").props[i].to_world);
    format!("{c:?} struck {struck:?}")
}

/// **A wreck's frame asks a world of tiles only for the shapes near it** (V99): the bike wrecked at
/// 40 ft/s three feet over the ground, spinning, into a post of the world's second row, stepped 150
/// frames over a world of 3 rows of posted tiles, over the same world 9 rows deep, and over the
/// 3 rows handing the whole world's shapes to every search (the port before this task): the same
/// wreck each frame ([`wrecked`]), the same questions of
/// both worlds, and the whole world's set never asked for.
#[test]
fn a_wrecks_frame_asks_a_world_of_tiles_only_for_the_shapes_near_it() {
    let Some(part) = the_wreck_part() else { return };
    assert!(part.collision_enabled, "the disc's wreck bike searches the world");
    let wrecks = phy::Wrecks::default();
    let (x, z) = (207.0f32, ROW + 41.0);
    let y = relief(f64::from(x), f64::from(z)) as f32 + 3.0;
    let mut ride = game_sim::ride::Ride::default();
    ride.chassis.quat = Vf::new(0.0, 0.0, 0.0, 1.0);
    ride.origin = Vf::new(x, y, z, 1.0);
    ride.chassis.vel = Vf::new(0.0, 0.0, 40.0, 0.0);
    ride.chassis.ang_vel = Vf::new(0.0, 1.5, 0.0, 0.0);
    let begin = || game_sim::ride::Crash::begin(&ride, game_sim::trick::Wreck(5), &part, game_sim::ride::WRECK_INERTIA, &wrecks, None);
    let mut small = Counted { world: world(3), asked: Asked::default() };
    let mut deep = Counted { world: world(9), asked: Asked::default() };
    let mut whole = Whole(Counted { world: world(3), asked: Asked::default() });
    let every = deep.world.every_shape().props.len();
    let (mut a, mut b, mut c) = (begin(), begin(), begin());
    let (mut touched, mut widest) = (0, 0);
    for frame in 0..150 {
        small.asked = Asked::default();
        deep.asked = Asked::default();
        let at = [a.bike.origin(), b.bike.origin(), c.bike.origin()];
        a.step(DT, &part, &wrecks, &mut small);
        b.step(DT, &part, &wrecks, &mut deep);
        c.step(DT, &part, &wrecks, &mut whole);
        let fa = wrecked(&a, small.world.wooded_near(at[0]).1);
        let fb = wrecked(&b, deep.world.wooded_near(at[1]).1);
        let fc = wrecked(&c, whole.0.world.wooded().1);
        assert_eq!(fa, fc, "frame {frame}: the wreck over the near set parted from the wreck over the whole world");
        assert_eq!(fa, fb, "frame {frame}: the wreck over 9 rows parted from the wreck over 3");
        assert_eq!(small.asked, deep.asked, "frame {frame}: the questions asked of the world");
        for asked in [&small.asked, &deep.asked] {
            for whole in ["collision", "wooded", "heightfield"] {
                assert!(!asked.calls.contains_key(whole), "frame {frame}: the wreck asked for the whole world's `{whole}`");
            }
        }
        touched += usize::from(!a.bike.record.contacts.is_empty());
        widest = widest.max(deep.asked.widest);
    }
    println!(
        "ended at {:?} resting {}; {touched} frames touching; the widest near set {widest} of {every} shapes",
        a.bike.body.position, a.resting
    );
    assert!(touched > 10, "the wreck must come down among the posts ({touched} frames touching)");
    assert!(widest > 0 && widest * 8 < every, "a near set must be a corner of the world: {widest} of {every}");
}
