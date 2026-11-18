//! MD153a: on a world that wraps (MD29, MD43) the moving list's shapes — the tiles' path cars, the
//! animated toys' parts — stand again one width over while they are by a seam, in every set of
//! shapes the searches are handed, as the tiles' still shapes do; so a bike by the near seam meets a
//! truck by the far one.
//!
//! The world is wrap.rs's two generated tiles along `x`, 1,134 feet wide and wrapped on `x` alone,
//! with one wall in the moving list, turned a little, as a car's mesh is stood at its pose.
//!
//! What is proved:
//!
//! * a wall 4 feet in from the far seam has a copy a width back, in the near set by the near seam
//!   and in the plane's set: its frame, its inverse and its sphere are the wall's stood a width over;
//! * a wall in the middle has none: its copy places are empty, no mesh and no reach;
//! * the places are always there, so a still shape keeps its number whether a copy is in use;
//! * a strike heard on a copy is told to the wall it copies;
//! * a world that does not wrap mirrors the list alone, as before;
//! * (MD167) a bike by the near seam strikes a knockable ball by the far one, the record where the
//!   bike is; on a world that does not wrap the strike is the plain one (skipped without the disc).

use game_sim::math::{self, Mat4};
use game_sim::tiles::{generate, Piece, Reach, SEAM_COPIES, Source, Tile, Tiled};
use game_sim::traffic::Moving;
use game_sim::track::{Mesh, Node, Placement, Points, TerrainLayout, Triangle};
use game_sim::world::{Prop, World};
use ps2_float::{F, Vf};

const WIDTH: f32 = 1134.0;
const SPACING: f32 = 9.0;

struct Memory(Vec<Vec<u8>>, Vec<(Mesh, Placement)>);

impl Source for Memory {
    fn load(&mut self, index: usize) -> Result<Piece, String> {
        // One still shape in the first tile, so the sets have one after the list's mirror.
        let shapes = if index == 0 { self.1.clone() } else { Vec::new() };
        Ok(Piece { terrain: self.0[index].clone(), shapes })
    }
}

/// Two tiles along `x`, flat at 40 feet, wrapped on `x` or not.
fn world(wrap: bool) -> Tiled {
    let mut tiles = Vec::new();
    let mut files = Vec::new();
    for k in 0..2u32 {
        let at = k * 567;
        files.push(generate::terrain("t", (4, 4), SPACING, |_, _| 40.0, |_, _| 0));
        tiles.push(Tile {
            name: format!("t{k}"),
            at: [F(at as f32), F::ZERO],
            layout: TerrainLayout { width: 4, breadth: 4, width_scale: F(SPACING), breadth_scale: F(SPACING), ..TerrainLayout::default() },
            spacing: F(SPACING),
            levels: 1,
            keep: [0, 0, 63, 63],
            first_owner: 0,
        });
    }
    let still = (mesh(), Placement { position: [F(300.0), F(40.0), F(300.0)], look: [F::ZERO, F::ZERO, F::ONE], up: [F::ZERO, F::ONE, F::ZERO], ..Placement::default() });
    let tiled = Tiled::new(tiles, Box::new(Memory(files, vec![still])), Reach::DESKTOP).unwrap();
    tiled.wrapped([wrap, false]).unwrap()
}

/// A wall twenty feet wide and ten high about its own origin, in one leaf (tests/ride.rs's).
fn mesh() -> Mesh {
    let v = |x: f32, y: f32| [F(x), F(y), F::ZERO];
    Mesh {
        version: 0,
        surface: None,
        transform: [[F::ONE, F::ZERO, F::ZERO], [F::ZERO, F::ONE, F::ZERO], [F::ZERO, F::ZERO, F::ONE], [F::ZERO, F::ZERO, F::ZERO]],
        vertices: Points::Wide(vec![v(-10.0, 0.0), v(10.0, 0.0), v(10.0, 10.0), v(-10.0, 10.0)]),
        normals: None,
        nodes: vec![Node { volume: F::ZERO, centre: [F::ZERO, F(5.0), F::ZERO], half_extent: [F(10.0), F(5.0), F(0.5)], children: [0x8000, 0x8001] }],
        triangles: vec![
            Triangle { distance: F::ZERO, normal: [F::ZERO; 3], vertices: [0, 2, 1] },
            Triangle { distance: F::ZERO, normal: [F::ZERO; 3], vertices: [0, 3, 2] },
        ],
    }
}

/// A car's pose at `x`, `z`, turned a little off the axes.
fn pose(x: f32, z: f32) -> Mat4 {
    math::basis(Vf::new(x, 40.0, z, 1.0), Vf::new(0.28, 0.0, 0.96, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0), true, true)
}

/// The world with `walls` in its moving list at those `x`s, `z` 200.
fn with_walls(wrap: bool, xs: &[f32]) -> Tiled {
    let mut w = world(wrap);
    let list = World::moving(&mut w).expect("a tiled world has a moving list");
    // Each with a motion, as a car's mesh has: only a mover's struck listener listens.
    list.props.extend(xs.iter().map(|&x| {
        let mut p = Prop::on(mesh(), &pose(x, 200.0));
        p.moving = Some(Moving::of(&pose(x - 1.0, 200.0), &pose(x, 200.0), F(30.0)));
        p
    }));
    w
}

fn close(a: Vf, b: Vf) -> bool {
    a.0.iter().zip(b.0.iter()).all(|(a, b)| (a - b).abs() < 1.0e-3)
}

#[test]
fn a_wall_by_the_far_seam_stands_again_a_width_back() {
    let mut w = with_walls(true, &[WIDTH - 4.0]);
    let want = Prop::on(mesh(), &pose(-4.0, 200.0));
    let (_, near, _) = w.wooded_near(Vf::new(5.0, 40.0, 200.0, 1.0));
    let near = near.expect("a near set");
    // The wall, its three copy places, then the still shapes.
    let copy = &near.props[1];
    assert_eq!(copy.mesh.triangles.len(), 2, "the copy has the wall's mesh");
    assert_eq!(copy.radius, want.radius);
    assert!(close(copy.centre, want.centre), "centre {:?}, want {:?}", copy.centre, want.centre);
    for r in 0..4 {
        assert!(close(copy.to_world.0[r], want.to_world.0[r]), "to_world row {r}");
        assert!(close(copy.from_world.0[r], want.from_world.0[r]), "from_world row {r}: {:?} vs {:?}", copy.from_world.0[r], want.from_world.0[r]);
    }
    // A point on the copy goes back into the wall's own space where the same point a width on does.
    let p = Vf::new(-3.0, 45.0, 201.0, 1.0);
    let q = Vf::new(WIDTH - 3.0, 45.0, 201.0, 1.0);
    assert!(close(math::transform_point(&copy.from_world, p), math::transform_point(&near.props[0].from_world, q)));
    for empty in &near.props[2..1 + SEAM_COPIES] {
        assert!(empty.mesh.triangles.is_empty() && empty.radius == F::ZERO, "an unused place is empty");
    }
    // Its motion is the wall's, about the copy's place.
    let m = copy.moving.expect("the copy moves as the wall does");
    assert_eq!(m.velocity, near.props[0].moving.unwrap().velocity);
    assert!(close(m.pose.0[3], Vf::new(-4.0, 40.0, 200.0, 1.0)));
    // The plane's set has it too.
    let every = w.every_shape();
    assert!(close(every.props[1].centre, want.centre));
}

#[test]
fn a_wall_in_the_middle_has_only_empty_places_and_the_still_shapes_keep_their_numbers() {
    let mut w = with_walls(true, &[560.0]);
    let at = Vf::new(300.0, 40.0, 300.0, 1.0);
    let still = |w: &mut Tiled| {
        let (_, near, _) = w.wooded_near(at);
        let near = near.expect("a near set");
        assert!(near.props[1..=SEAM_COPIES].iter().all(|p| p.mesh.triangles.is_empty() && p.radius == F::ZERO));
        near.props.iter().skip(1 + SEAM_COPIES).position(|p| close(p.centre, Vf::new(300.0, 45.0, 300.0, 1.0))).map(|i| i + 1 + SEAM_COPIES)
    };
    let before = still(&mut w).expect("the still wall is in the near set");
    // The wall drives up to the seam: a copy comes into use, and nothing else moves in the set.
    World::moving(&mut w).unwrap().props[0].stand(&pose(WIDTH - 2.0, 200.0));
    let (_, near, _) = w.wooded_near(at);
    let near = near.unwrap();
    assert_eq!(near.props[1].mesh.triangles.len(), 2, "the copy is in use");
    assert!(close(near.props[before].centre, Vf::new(300.0, 45.0, 300.0, 1.0)), "the still shape kept its number");
}

#[test]
fn a_strike_heard_on_a_copy_is_the_walls() {
    let mut w = with_walls(true, &[WIDTH - 4.0]);
    let (_, near, _) = w.wooded_near(Vf::new(5.0, 40.0, 200.0, 1.0));
    near.unwrap().props[1].strike_heard();
    let list = World::moving(&mut w).unwrap();
    assert!(list.props[0].take_heard(), "the wall heard the strike on its copy");
}

#[test]
fn a_world_that_does_not_wrap_mirrors_the_list_alone() {
    let mut w = with_walls(false, &[WIDTH - 4.0]);
    let (_, near, _) = w.wooded_near(Vf::new(1100.0, 40.0, 200.0, 1.0));
    let near = near.unwrap();
    assert_eq!(near.props[0].mesh.triangles.len(), 2);
    assert!(near.props.iter().skip(1).all(|p| !close(p.centre, Prop::on(mesh(), &pose(-4.0, 200.0)).centre)), "no copy");
}

// MD167: the knockable props (S197) by a seam. The bike's body rays strike a prop by the far seam
// where the prop stands again one width over, by the bike, as the moving list's shapes do.

/// A flat world wrapped on `x` alone, `width` feet across.
struct Seam {
    flat: game_sim::world::Flat,
    width: F,
}

impl World for Seam {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        self.flat.ground(at)
    }

    fn solid(&mut self, bundle: &game_sim::collide::Bundle, record: &mut game_sim::collide::Record) -> bool {
        self.flat.solid(bundle, record)
    }

    fn area(&self) -> Option<game_sim::biplane::Area> {
        Some(game_sim::biplane::Area { width: self.width, depth: F(10_000.0) })
    }

    fn wrap(&self) -> [bool; 2] {
        [true, false]
    }
}

const DT: F = F(game_sim::tick::DeltaTime::CAP_30);

/// free09's props as the ride loads them, and the bike's collision files, off the disc
/// (tests/prop_strike_paths.rs).
fn knock_disc() -> Option<(game_sim::prop_set::PropSet, std::sync::Arc<game_sim::bike_col::BikeCol>)> {
    use game_sim::bike_col::{self, BikeCol};
    let mut disc = game_render::disc::Disc::open_configured().map_err(|why| ps2_oracle::skip(&why)).ok()?;
    let member = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("Tracks\\free09.res")).expect("free09.res").name.clone();
    let res = disc.res(&member).expect("free09's archive");
    let toy = game_sim::phy::Ini::parse_bytes(&res.read_suffix("free09.toy").expect("free09.toy"));
    let scene = game_sim::track::Scene::parse(&res.read_suffix("free09.scn").expect("free09.scn")).expect("free09.scn parses");
    let (set, said) = game_sim::prop_set::PropSet::load(
        &toy,
        "Tracks\\Free09\\",
        &scene.models,
        &mut |p| res.read(p).ok(),
        &mut |p| res.read_suffix(p).ok(),
        Some(game_sim::biplane::Area::new(F(3.0), F(3.0), F(9.0), F(9.0))),
    );
    assert!(said.is_empty(), "free09 loads whole: {said:?}");
    let split = disc.res("Tracks\\free09_ss_2.res").expect("the split track's archive");
    let read = |name: String| split.read_suffix(&name).unwrap_or_else(|e| panic!("{e}"));
    let col = BikeCol::load(&read(bike_col::rays_file(bike_col::MODEL)), &read(bike_col::shape_file(bike_col::MODEL)))
        .expect("the bike's collision files");
    Some((set, std::sync::Arc::new(col)))
}

fn close_by(a: Vf, b: Vf, by: f32) -> bool {
    close(Vf::new(a.x() + by, a.y(), a.z(), a.w()), b)
}

#[test]
fn a_bike_by_the_near_seam_strikes_a_ball_by_the_far_one() {
    let Some((set, col)) = knock_disc() else { return };
    let mut sim = game_sim::Sim::new(0x5167);
    sim.traffic.props = set;
    sim.rider.sense.wrecks_enabled = false;
    sim.rider.carries_bike_col(col);
    // The bike ahead of a ball on +x, facing back at it, where its rays strike it.
    let facing = Vf::new(-1.0, 0.0, 0.0, 0.0);
    let mut found = None;
    'find: for (c, coll) in sim.traffic.props.collections.iter().enumerate() {
        let Some(o) = coll.objects.first().filter(|o| o.collision.is_some()) else { continue };
        let ball = o.node.origin;
        for ahead in (8..24).map(|k| k as f32 * 0.25) {
            for up in [0.0, 0.5, 1.0, -0.5, 1.5] {
                let at = Vf::new(ball.x() + ahead, ball.y() + up, ball.z(), 1.0);
                let mut probe = sim.clone();
                probe.rider.place(at, facing);
                let strike = probe.prop_strike(0, DT);
                if strike.struck.iter().any(|s| s.0 == c) && !strike.contacts.is_empty() {
                    found = Some((c, at, strike));
                    break 'find;
                }
            }
        }
    }
    let (c, at, want) = found.expect("some spot by some ball where the bike's rays strike it");
    // A world whose far seam is the whole foot under the bike: the ball 2 to 6 feet short of it, the
    // bike a width back, within a foot past the near seam.
    let width = at.x().floor();
    let wrapped = Seam { flat: game_sim::world::Flat::default(), width: F(width) };
    let near = Vf::new(at.x() - width, at.y(), at.z(), 1.0);
    assert!((0.0..1.0).contains(&near.x()));
    let mut probe = sim.clone();
    probe.rider.place(near, facing);
    assert!(probe.prop_strike(0, DT).struck.is_empty(), "alone, the ball is a width from the bike");
    let got = probe.prop_strike_in(0, DT, &wrapped);
    // The same ball struck, by the same rays, the record where the bike is.
    assert_eq!(got.struck.iter().map(|s| (s.0, s.1, s.2.len())).collect::<Vec<_>>(), want.struck.iter().map(|s| (s.0, s.1, s.2.len())).collect::<Vec<_>>());
    assert!(got.struck.iter().any(|s| s.0 == c));
    assert_eq!(got.contacts.len(), want.contacts.len(), "one entry a ray, as at the ball's own place");
    // The bike's entries where the bike is; the mirrors on the ball's list at the ball's own place.
    for (g, w) in got.contacts.iter().zip(&want.contacts) {
        assert_eq!((g.kind, g.other, g.surface), (w.kind, w.other, w.surface));
        assert!(close(g.normal, w.normal), "normal {:?} vs {:?}", g.normal, w.normal);
        assert!(close_by(g.point, w.point, width), "point {:?} vs {:?}", g.point, w.point);
        assert!(close(g.from, w.from), "from {:?} vs {:?}", g.from, w.from);
    }
    for (g, w) in got.struck.iter().zip(&want.struck).flat_map(|(g, w)| g.2.iter().zip(&w.2)) {
        assert_eq!((g.kind, g.other, g.surface), (w.kind, w.other, w.surface));
        assert!(close(g.normal, w.normal), "mirror normal {:?} vs {:?}", g.normal, w.normal);
        assert!(close(g.point, w.point), "mirror point {:?} vs {:?}", g.point, w.point);
    }
    let k = got.struck.iter().position(|s| s.0 == c).unwrap();
    assert!(got.across.contains(&(k, [F(width), F::ZERO])), "struck across the seam: {:?}", got.across);
    assert!(want.across.is_empty());
    // The bike's frame pushes the ball a width back, by the bike; written back, it is where it was.
    let mut set = probe.traffic.props.clone();
    let before = set.collections[c].bones[0].body;
    let pushed = set.strike(&got);
    let p = pushed.iter().find(|p| p.collection == c).expect("the ball pushed");
    assert!(close_by(p.body.position, before.position, width), "pushed by the bike: {:?} vs {:?}", p.body.position, before.position);
    assert_eq!(set.across.len(), got.across.len());
    set.pushed(&pushed);
    let after = set.collections[c].bones[0].body;
    assert_eq!((after.position.0.map(f32::to_bits), after.last_position.0.map(f32::to_bits)), (before.position.0.map(f32::to_bits), before.last_position.0.map(f32::to_bits)), "back at its place, word for word");
    // The props' step sees the striker by the ball and uses the list up.
    probe.traffic.props = set;
    let movers = game_sim::prop_set::PropMovers::default();
    probe.props_step(DT, &movers);
    assert!(probe.traffic.props.across.is_empty());
    // And it moves the ball as the same strike at the ball's own place does.
    let mut there = sim.clone();
    there.rider.place(at, facing);
    let pushed = there.traffic.props.strike(&want);
    there.traffic.props.pushed(&pushed);
    there.props_step(DT, &movers);
    let (g, w) = (probe.traffic.props.collections[c].bones[0].body, there.traffic.props.collections[c].bones[0].body);
    assert!(close(g.position, w.position), "position {:?} vs {:?}", g.position, w.position);
    assert!(close(g.velocity, w.velocity), "velocity {:?} vs {:?}", g.velocity, w.velocity);
    assert!(close(g.rates, w.rates), "rates {:?} vs {:?}", g.rates, w.rates);
    assert!(w.velocity != Vf::ZERO || w.rates != Vf::ZERO, "the step moved the ball: {:?}", w);
    // And on a world that does not wrap, the strike is the plain one, word for word.
    let mut there = sim.clone();
    there.rider.place(at, facing);
    assert_eq!(there.prop_strike_in(0, DT, &game_sim::world::Flat::default()), want);
}
