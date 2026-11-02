//! MD58: a woken prop on a world of many tiles searches only the shapes near it. The props' collide
//! pass ([`PropSet::search_in`], from [`Traffic::frame_held`]) took the whole world's shapes
//! ([`World::wooded`]) while any prop was filed (S131), so a knocked prop's tick grew with the world;
//! it now asks [`World::wooded_near`] around each collection's bodies, as the plane's search does
//! (V96).
//!
//! free09's beach ball by the track, woken and tossed as `tests/prop_collision.rs` tosses it, over a
//! generated world of three rows of posted tiles laid under it (`tests/plane_world_cost.rs`'s rows,
//! with a post set beside the ball for its rays to meet), over the same world nine rows deep, and over
//! the three rows handing the whole world's shapes to every search (the port before this task): the
//! same ride of every prop each tick ([`ridden`]), the same questions of both worlds of tiles, and
//! the whole world's set never asked for.
//!
//! The terrains and posts are written here ([`game_sim::tiles::generate`]); the props are the disc's
//! (free09's `.toy`, models and `m.col`s): it skips without the disc.

use game_render::disc::{Disc, Res};
use game_sim::biplane::Area;
use game_sim::collide::{Bundle, Record};
use game_sim::contact_search::Heightfield;
use game_sim::eco::Trees;
use game_sim::flight_search;
use game_sim::phy::Ini;
use game_sim::prop_set::PropSet;
use game_sim::tiles::{generate, Piece, Reach, Source, Tile, Tiled};
use game_sim::track::{self, Placement, Scene, TerrainLayout};
use game_sim::traffic::Traffic;
use game_sim::world::{Props, World};
use ps2_float::{F, Vf};
use ps2_oracle::skip;

const DT: F = F(1.0 / 30.0);
/// A row's terrain: 16 x 4 root cells of 16 samples at 9 feet, 2295 x 567 feet.
const CELLS: (u32, u32) = (16, 4);
const SPACING: f32 = 9.0;
const ROW: f32 = 567.0;
/// The beach ball (`tests/prop_collision.rs`'s), where free09 stands it.
const BALL: [f32; 3] = [1761.45, 127.52, 5192.78];
/// Where the first row starts along `z`, a whole number of feet as a tile's corner must be: the ball
/// in the middle of the second row.
const Z0: f32 = 4342.0;

/// The ground's height in feet at a world point: gentle rolls a few feet under the ball.
fn relief(x: f64, z: f64) -> f64 {
    119.0 + 2.0 * (x / 97.0).sin() * (z / 61.0).cos() + 1.0 * ((x + z) / 23.0).sin()
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

/// A post at `z` feet into a row that starts at `oz`, standing on the ground, facing `look`: placed in
/// the tile's own space, which the world moves to the tile's corner.
fn posted(x: f32, z: f32, oz: f64, look: [f32; 3]) -> (track::Mesh, Placement) {
    let y = relief(f64::from(x), oz + f64::from(z)) as f32;
    (post(), Placement { file: "post.col".into(), position: [F(x), F(y), F(z)], look: look.map(F), ..Placement::default() })
}

/// Row `r`'s terrain, a post every 40 feet over it, every third turned, and in the ball's row one
/// more a few feet past the ball, across the way it is tossed.
struct Rows;

impl Source for Rows {
    fn load(&mut self, index: usize) -> Result<Piece, String> {
        let oz = f64::from(Z0 + index as f32 * ROW);
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
                let look = if k % 3 == 0 { [0.6, 0.0, 0.8] } else { [0.0, 0.0, 1.0] };
                shapes.push(posted(x, z, oz, look));
                z += 40.0;
                k += 1;
            }
            x += 40.0;
        }
        if index == 1 {
            shapes.push(posted(BALL[0] - 3.0, BALL[2] + 7.0 - (Z0 + ROW), oz, [0.0, 0.0, 1.0]));
        }
        Ok(Piece { terrain, shapes })
    }
}

/// `rows` rows of [`Rows`] along `z` from [`Z0`], each touching the next.
fn world(rows: usize) -> Tiled {
    let tiles = (0..rows)
        .map(|r| {
            let mut t = Tile {
                name: format!("row{r}"),
                at: [F::ZERO, F(Z0 + r as f32 * ROW)],
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
/// `whole` hands the whole world's set to a search asking for the shapes near (the port before
/// MD58), and names no near squares.
struct Counted {
    world: Tiled,
    asked: Asked,
    whole: bool,
}

impl Counted {
    fn note(&mut self, what: &'static str) {
        *self.asked.calls.entry(what).or_default() += 1;
    }
    /// The set a search at `at` is handed.
    fn set_at(&mut self, at: Vf) -> Option<&Props> {
        if self.whole { self.world.wooded().1 } else { self.world.wooded_near(at).1 }
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
        let (field, props, trees) = if self.whole { self.world.wooded() } else { self.world.wooded_near(at) };
        self.asked.widest = self.asked.widest.max(props.map_or(0, |p| p.props.len()));
        (field, props, trees)
    }
    fn near_set(&self, at: Vf) -> Option<[i32; 2]> {
        if self.whole { None } else { self.world.near_set(at) }
    }
    fn moving(&mut self) -> Option<&mut Props> {
        self.note("moving");
        self.world.moving()
    }
}

/// free09's props, as a ride loads them (`tests/prop_collision.rs`'s loader).
fn free09() -> Option<PropSet> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let name = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("Tracks\\free09.res")).map(|e| e.name.clone())?;
    let res: Res = disc.res(&name).expect("free09.res");
    let toy = Ini::parse_bytes(&res.read_suffix("free09.toy").expect("free09.toy"));
    let scene = Scene::parse(&res.read_suffix("free09.scn").expect("free09.scn")).expect("free09.scn parses");
    let (set, said) = PropSet::load(
        &toy,
        "Tracks\\Free09\\",
        &scene.models,
        &mut |p| res.read(p).ok(),
        &mut |p| res.read_suffix(p).ok(),
        Some(Area::new(F(3.0), F(3.0), F(9.0), F(9.0))),
    );
    assert!(said.is_empty(), "free09 loads whole: {said:?}");
    Some(set)
}

/// Every prop as it rides, in full (`Debug`'s every digit), but for its search's bookkeeping: the
/// shape a record's rays struck last is named by its place among the search's collidables (the set
/// it was handed, after [`flight_search::FIRST`]), another number in a near set than in the whole
/// world's, so it is named here by where it stands. Nothing empties it, so the set is the one the
/// strike was found in: the same near square's all through this ride.
/// `sets` is each collection's set this tick, `None` for one that did not search.
fn ridden(props: &PropSet, sets: &[Option<Vec<[Vf; 4]>>]) -> String {
    let mut out = String::new();
    for (c, set) in props.collections.iter().zip(sets) {
        let mut c = c.clone();
        let mut struck = Vec::new();
        for o in &mut c.objects {
            if let Some(col) = o.collision.as_mut() {
                struck.push(col.record.struck.take().map(|i| set.as_ref().expect("a set to strike in")[i - flight_search::FIRST]));
            }
        }
        out += &format!("{c:?} struck {struck:?}\n");
    }
    out
}

/// The set each collection's search is handed this tick, by its shapes' places: where its filed
/// bodies stand before the tick (one body each, on free09's woken ball).
fn sets(props: &PropSet, world: &mut Counted) -> Vec<Option<Vec<[Vf; 4]>>> {
    props
        .collections
        .iter()
        .map(|c| {
            let o = c.objects.iter().find(|o| o.filed)?;
            assert_eq!(c.objects.iter().filter(|o| o.filed).count(), 1, "one filed body in a collection");
            let at = o.node.origin;
            Some(world.set_at(at).map_or_else(Vec::new, |p| p.props.iter().map(|p| p.to_world.0).collect()))
        })
        .collect()
}

/// **A woken prop's tick asks a world of tiles only for the shapes near it** (MD58): free09's beach
/// ball woken and tossed (as S198's test tosses it) over 3 rows of posted tiles, over the same world
/// 9 rows deep, and over the 3 rows handing the whole world's shapes to every search, 600 ticks of
/// [`Traffic::frame`]: every prop the same each tick ([`ridden`]), the same questions of both worlds
/// of tiles, the whole world's set never asked for by either, and its rays meeting a post.
#[test]
fn a_woken_props_tick_asks_a_world_of_tiles_only_for_the_shapes_near_it() {
    let Some(set) = free09() else { return };
    let near = |p: &PropSet| {
        let dist = |c: &game_sim::prop_world::Collection| (0..3).map(|k| (c.bones[0].body.position[k] - BALL[k]).powi(2)).sum::<f32>();
        p.collections.iter().enumerate().min_by(|a, b| dist(a.1).total_cmp(&dist(b.1))).map(|(i, c)| (i, dist(c))).unwrap()
    };
    let (ball, d) = near(&set);
    assert!(d < 1.0, "the ball is {} from {BALL:?}", d.sqrt());
    assert!(set.collections.iter().all(|c| c.objects.iter().all(|o| !o.filed)), "every prop asleep and unfiled as loaded");
    let toss = |mut set: PropSet| {
        set.wake(ball, 0);
        let body = &mut set.collections[ball].bones[0].body;
        body.velocity = Vf::new(-4.0, 18.0, 6.0, body.velocity[3]);
        body.rates = Vf::new(1.5, -0.5, 2.0, body.rates[3]);
        let mut t = Traffic::new(Vec::new());
        t.props = set;
        t
    };
    let mut small = Counted { world: world(3), asked: Asked::default(), whole: false };
    let mut deep = Counted { world: world(9), asked: Asked::default(), whole: false };
    let mut whole = Counted { world: world(3), asked: Asked::default(), whole: true };
    let every = deep.world.every_shape().props.len();
    assert!(every > 2 * small.world.every_shape().props.len(), "the deep world has near three times the shapes");
    let (mut a, mut b, mut c) = (toss(set.clone()), toss(set.clone()), toss(set));
    let (mut touched, mut on_object, mut widest, mut searched) = (0, 0, 0, 0);
    for tick in 0..600 {
        let at = [sets(&a.props, &mut small), sets(&b.props, &mut deep), sets(&c.props, &mut whole)];
        small.asked = Asked::default();
        deep.asked = Asked::default();
        a.frame(DT, &mut small);
        b.frame(DT, &mut deep);
        c.frame(DT, &mut whole);
        let [fa, fb, fc] = [ridden(&a.props, &at[0]), ridden(&b.props, &at[1]), ridden(&c.props, &at[2])];
        assert_eq!(fa, fc, "tick {tick}: the props over the near sets parted from the props over the whole world");
        assert_eq!(fa, fb, "tick {tick}: the props over 9 rows parted from the props over 3");
        assert_eq!(small.asked, deep.asked, "tick {tick}: the questions asked of the world");
        for asked in [&small.asked, &deep.asked] {
            for name in ["collision", "wooded", "heightfield"] {
                assert!(!asked.calls.contains_key(name), "tick {tick}: the props asked for the whole world's `{name}`");
            }
        }
        let rec = &a.props.collections[ball].objects[0].collision.as_ref().unwrap().record;
        touched += usize::from(rec.on_ground);
        on_object += usize::from(rec.on_object);
        searched += usize::from(at[0][ball].is_some());
        widest = widest.max(deep.asked.widest);
    }
    let body = &a.props.collections[ball].bones[0].body;
    println!(
        "ended at {:?} enabled {}; {searched} ticks searching, {touched} on the ground, {on_object} on a post; the widest near set {widest} of {every} shapes",
        body.position, body.enabled
    );
    assert!(searched > 100 && touched > 10, "the ball must search and come down ({searched} searching, {touched} on the ground)");
    assert!(on_object > 0, "the ball's rays must meet the post beside it");
    assert!(widest > 0 && widest * 8 < every, "a near set must be a corner of the world: {widest} of {every}");
}
