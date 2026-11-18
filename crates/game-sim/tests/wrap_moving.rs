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
//! * (MD167a) a wreck bike's rays by the near seam strike a ball by the far one as they do at the
//!   ball's own place, and the strike hands its frame the ball a width back and back again; the
//!   props' step moves a wreck bike that struck across a seam to the ball and back word for word;
//!   the ragdoll's bones are offered a ball by the far seam one width over; a rolling ball wraps
//!   over a seam where a world that does not wrap puts it back (the disc ones skipped without it).
//! * (MD254) a flown plane by the near seam strikes a ball by the far one: its frame files the same
//!   mirrors on the ball's list at the ball's own place, notes itself a width over, and pushes the
//!   ball as at the ball's place; the props' step sees it there and puts it back; a whole Sim tick
//!   moves the ball as the plane at the ball's place does, also when the frame carries the plane
//!   back over the seam; a world that does not wrap leaves the ball alone (skipped without the disc
//!   or MX.PSR).

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

// MD167a: the rest of the strikers on the knockable props, and a prop that rolls over a seam.

/// A flat world with an area `width` feet across and 10,000 deep, wrapped on `x` where `wraps`.
struct Strip {
    flat: game_sim::world::Flat,
    width: F,
    wraps: bool,
}

impl World for Strip {
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
        [self.wraps, false]
    }
}

/// A frame standing at `at`, unturned.
fn standing(at: Vf) -> Mat4 {
    Mat4([Vf::new(1.0, 0.0, 0.0, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0), Vf::new(0.0, 0.0, 1.0, 0.0), at])
}

fn bits(v: Vf) -> [u32; 4] {
    v.0.map(f32::to_bits)
}

#[test]
fn a_wreck_bike_by_the_near_seam_strikes_a_ball_by_the_far_one() {
    use game_sim::prop_wreck::wreck_rays_wrapped;
    let Some((set, _)) = knock_disc() else { return };
    let flat = game_sim::world::Flat::default();
    const ME: usize = 1;
    // The wreck bike's probe ahead of a ball on +x, where its rays strike it.
    let mut found = None;
    'find: for (c, coll) in set.collections.iter().enumerate() {
        let Some(o) = coll.objects.first().filter(|o| o.collision.is_some()) else { continue };
        let ball = o.node.origin;
        for ahead in (4..32).map(|k| k as f32 * 0.25) {
            for up in [0.0, 0.5, 1.0, -0.5, 1.5, 2.0] {
                let at = Vf::new(ball.x() + ahead, ball.y() + up, ball.z(), 1.0);
                let strike = wreck_rays_wrapped(&set, &standing(at), ME, &flat);
                if strike.struck.iter().any(|s| s.0 == c) && !strike.contacts.is_empty() {
                    found = Some((c, at, strike));
                    break 'find;
                }
            }
        }
    }
    let (c, at, want) = found.expect("some spot by some ball where the wreck bike's rays strike it");
    assert!(want.across.is_empty());
    // A world whose far seam is the whole foot under the probe, the probe a width back.
    let width = at.x().floor();
    let wrapped = Strip { flat: flat.clone(), width: F(width), wraps: true };
    let near = Vf::new(at.x() - width, at.y(), at.z(), 1.0);
    assert!((0.0..1.0).contains(&near.x()));
    assert!(wreck_rays_wrapped(&set, &standing(near), ME, &flat).struck.is_empty(), "alone, the ball is a width from the bike");
    let got = wreck_rays_wrapped(&set, &standing(near), ME, &wrapped);
    assert_eq!(got.struck.iter().map(|s| (s.0, s.1, s.2.len())).collect::<Vec<_>>(), want.struck.iter().map(|s| (s.0, s.1, s.2.len())).collect::<Vec<_>>());
    assert_eq!(got.contacts.len(), want.contacts.len(), "one entry a ray, as at the ball's own place");
    for (g, w) in got.contacts.iter().zip(&want.contacts) {
        assert_eq!((g.kind, g.other, g.surface), (w.kind, w.other, w.surface));
        assert!(close(g.normal, w.normal), "normal {:?} vs {:?}", g.normal, w.normal);
        assert!(close_by(g.point, w.point, width), "point {:?} vs {:?}", g.point, w.point);
    }
    for (g, w) in got.struck.iter().zip(&want.struck).flat_map(|(g, w)| g.2.iter().zip(&w.2)) {
        assert_eq!((g.kind, g.other, g.surface), (w.kind, w.other, w.surface));
        assert!(close(g.normal, w.normal), "mirror normal {:?} vs {:?}", g.normal, w.normal);
        assert!(close(g.point, w.point), "mirror point at the ball's own place {:?} vs {:?}", g.point, w.point);
    }
    let k = got.struck.iter().position(|s| s.0 == c).unwrap();
    assert!(got.across.contains(&(k, [F(width), F::ZERO])), "struck across the seam: {:?}", got.across);
    // Put on as the wreck bike's: its frame is handed the ball a width back; written back, it is
    // where it was, word for word.
    let mut set = set;
    let before = set.collections[c].bones[0].body;
    let pushed = set.strike_as(&got, ME);
    let p = pushed.iter().find(|p| p.collection == c).expect("the ball pushed");
    assert!(close_by(p.body.position, before.position, width), "pushed by the bike: {:?} vs {:?}", p.body.position, before.position);
    assert!(set.across.iter().any(|a| (a.collection, a.object, a.player, a.by) == (c, 0, ME, [F(width), F::ZERO])), "{:?}", set.across);
    set.pushed(&pushed);
    let after = set.collections[c].bones[0].body;
    assert_eq!((bits(after.position), bits(after.last_position)), (bits(before.position), bits(before.last_position)));
    // On a world that does not wrap, the plain strike word for word, and nothing noted.
    let mut plain = set.clone();
    plain.across.clear();
    assert_eq!(wreck_rays_wrapped(&plain, &standing(at), ME, &flat), want);
    plain.strike_as(&want, ME);
    assert!(plain.across.is_empty());
}

#[test]
fn the_props_step_moves_a_wreck_bike_that_struck_across_a_seam_and_back() {
    use game_sim::biplane::Body;
    use game_sim::contact::Collider;
    use game_sim::prop_contact::{Across, across_back, across_moved};
    let width = F(1134.0);
    let body = Body { position: Vf::new(0.3, 2.0, 40.0, 1.0), last_position: Vf::new(0.2875, 2.0, 40.0, 1.0), ..Body::default() };
    let mut bodies = vec![(Collider::default(), Body::default(), F(1.0)), (Collider::default(), body, F(1.0))];
    // Two players' vehicles before the bodies: wreck bike 1 is the step's other side 3.
    let mut vehicles = vec![game_sim::contact::Vehicle::default(); 2];
    let across = vec![
        Across { collection: 0, object: 0, player: 3, by: [width, F::ZERO], pushed: true },
        Across { collection: 0, object: 1, player: 3, by: [width, F::ZERO], pushed: true },
    ];
    let moved = across_moved(across, &mut vehicles, &mut bodies);
    assert_eq!(moved.len(), 1, "a striker is moved once");
    assert!(close_by(body.position, bodies[1].1.position, 1134.0), "{:?}", bodies[1].1.position);
    assert_eq!(bits(bodies[0].1.position), bits(Vf::ZERO), "the other wreck bike stays");
    assert!(vehicles.iter().all(|v| bits(v.node.origin) == bits(game_sim::contact::Vehicle::default().node.origin)));
    // A place the step left alone is the one it had, word for word (0.3 + 1134 rounds).
    let mut same = bodies.clone();
    across_back(moved.clone(), &mut same);
    assert_eq!((bits(same[1].1.position), bits(same[1].1.last_position)), (bits(body.position), bits(body.last_position)));
    // One the step changed is moved back its width.
    bodies[1].1.position = Vf::new(1134.5, 2.0, 40.0, 1.0);
    across_back(moved, &mut bodies);
    assert_eq!(bits(bodies[1].1.position), bits(Vf::new(0.5, 2.0, 40.0, 1.0)));
    assert_eq!(bits(bodies[1].1.last_position), bits(body.last_position));
}

#[test]
fn the_ragdoll_is_offered_a_ball_by_the_far_seam_one_width_over() {
    use game_sim::ragdoll_scene::KnockedProp;
    let Some((mut set, _)) = knock_disc() else { return };
    let (c, ball) = set
        .collections
        .iter()
        .enumerate()
        .find_map(|(c, coll)| Some((c, coll.objects.first().filter(|o| o.collision.is_some())?.node.origin)))
        .expect("a ball");
    // The ball a foot or two short of the far seam; the probe half a foot past the near one.
    let width = ball.x().floor() + 2.0;
    let wrapped = Strip { flat: game_sim::world::Flat::default(), width: F(width), wraps: true };
    let probe = Vf::new(0.5, ball.y(), ball.z(), 1.0);
    let reach = F(3.0);
    let seams = game_sim::tiles::seam_shifts(&wrapped, probe, reach);
    assert!(seams.contains(&[F(width), F::ZERO]), "{seams:?}");
    assert!(KnockedProp::near(&set, probe, reach).is_empty() && KnockedProp::across(&set, probe, reach, &[]).is_empty());
    // Asleep or awake, the copy is the ball stood a width back: what the bones meet where the ball is
    // met from a width over.
    for awake in [false, true] {
        set.collections[c].bones[0].body.enabled = awake;
        let got = KnockedProp::across(&set, probe, reach, &seams);
        let g = got.iter().find(|p| (p.pushed.collection, p.pushed.object) == (c, 0)).expect("the ball one width over");
        assert_eq!(g.place, None);
        let there = Vf::new(probe.x() + width, probe.y(), probe.z(), 1.0);
        let want = if awake {
            KnockedProp::near(&set, there, reach).into_iter().find(|p| (p.pushed.collection, p.pushed.object) == (c, 0)).expect("near at its own place")
        } else {
            let mut woken = set.clone();
            woken.collections[c].bones[0].body.enabled = true;
            KnockedProp::near(&woken, there, reach).into_iter().find(|p| (p.pushed.collection, p.pushed.object) == (c, 0)).expect("near at its own place")
        };
        assert!(close_by(g.frame.origin, want.frame.origin, width), "frame {:?} vs {:?}", g.frame.origin, want.frame.origin);
        assert_eq!(g.frame.rows.map(bits), want.frame.rows.map(bits));
        assert!(close_by(g.pushed.body.position, want.pushed.body.position, width));
        assert!((g.distance - want.distance).0.abs() < 1e-3, "distance {:?} vs {:?}", g.distance, want.distance);
    }
}

#[test]
fn a_rolling_ball_wraps_over_a_seam() {
    use game_sim::prop_world::Edge;
    let Some((set, _)) = knock_disc() else { return };
    let (c, ball) = set
        .collections
        .iter()
        .enumerate()
        .find_map(|(c, coll)| Some((c, coll.objects.first().filter(|o| o.collision.is_some() && coll.joints.is_empty())?.node.origin)))
        .expect("a ball pinned to nothing");
    // The ball a foot short of the far seam, rolled at it at 20 ft/s for a second.
    let width = ball.x().floor() + 1.0;
    let ride = |world: &mut Strip, edge: f32| {
        let mut set = set.clone();
        for coll in &mut set.collections {
            coll.edge = Some(Edge { min: [F::ZERO, F::ZERO], max: [F(edge), F(10_000.0)] });
        }
        set.collections[c].objects[0].reset_off_edge = true;
        set.wake(c, 0);
        set.collections[c].bones[0].body.velocity = Vf::new(20.0, 0.0, 0.0, 0.0);
        let place = set.collections[c].bones[0].body.position;
        for _ in 0..30 {
            let movers = set.search_in(DT, world);
            set.step_with(DT, &movers, &mut []);
        }
        (set, place)
    };
    let (wrapped, place) = ride(&mut Strip { flat: game_sim::world::Flat::default(), width: F(width), wraps: true }, width);
    let (wide, _) = ride(&mut Strip { flat: game_sim::world::Flat::default(), width: F(2.0 * width), wraps: false }, 2.0 * width);
    let (edged, _) = ride(&mut Strip { flat: game_sim::world::Flat::default(), width: F(width), wraps: false }, width);
    let (g, w) = (wrapped.collections[c].bones[0].body, wide.collections[c].bones[0].body);
    assert!(w.position.x() > width + 1.0, "the ball rolled past where the seam is: {:?}", w.position);
    // Wrapped, it is on the world, where it is on the wider one less a width, moving as it does.
    assert!((0.0..width).contains(&g.position.x()), "on the world: {:?}", g.position);
    // Within 0.02 ft: the wider world's ball, 1,700 feet out, rounds to 1.2e-4 a substep where the
    // wrapped one, back by the near seam, rounds finer.
    let roughly = |a: Vf, b: Vf| (0..3).all(|i| (a.0[i] + [width, 0.0, 0.0][i] - b.0[i]).abs() < 0.02);
    assert!(roughly(g.position, w.position), "position {:?} vs {:?}", g.position, w.position);
    assert!(roughly(g.last_position, w.last_position));
    assert!(close(g.velocity, w.velocity), "velocity {:?} vs {:?}", g.velocity, w.velocity);
    let node = wrapped.collections[c].objects[0].node.origin;
    assert!(roughly(node, wide.collections[c].objects[0].node.origin), "node {:?}", node);
    // Its edge is the world's again after the step; the put-back place is its own.
    assert_eq!(wrapped.collections[c].edge, edged.collections[c].edge);
    assert_eq!(wrapped.collections[c].objects[0].place, set.collections[c].objects[0].place);
    // A world that does not wrap puts it back over its edge, as before.
    let e = edged.collections[c].bones[0].body;
    assert_eq!(bits(e.position), bits(place), "put back over the edge: {:?}", e.position);
    assert!(!e.enabled);
}

// MD254: the flown plane's strike on the knockable props across a seam.

/// The biplane as the port builds it off `machine.res` in MX.PSR (tests/prop_plane.rs's), without its
/// hull: the props' rays on it are not this test's (MD254a's are).
fn the_plane() -> Option<game_sim::flight::Flight> {
    the_plane_and_hull().map(|(p, _)| p)
}

/// [`the_plane`] and its hull (`biplanef.col`).
fn the_plane_and_hull() -> Option<(game_sim::flight::Flight, Mesh)> {
    use game_sim::archive::{Psr, Res};
    use std::io::{Read, Seek, SeekFrom};
    let kit = ps2_oracle::Kit::load().map_err(|why| ps2_oracle::skip(&why)).ok()?;
    let here = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/iso/MX.PSR");
    let path = if here.exists() { here } else { kit.ctx("iso").join("MX.PSR") };
    let Ok(mut f) = std::fs::File::open(&path) else {
        ps2_oracle::skip(&format!("no {} — `./kit extract MX.PSR` writes it", path.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find("machine.res").expect("machine.res is in MX.PSR");
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    let res = Res::parse(&bytes).expect("machine.res");
    let member = |name: &str| {
        let e = res.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name)).unwrap_or_else(|| panic!("machine.res has {name}"));
        res.read(&bytes, e).expect("its bytes")
    };
    let plane = game_sim::phy::Plane::read_bytes(&member(game_sim::phy::BIPLANE_PHY));
    let rays = game_sim::track::Rays::parse(&member(game_sim::flight_search::BIPLANE_RAYS)).expect("biplanem.col");
    let hull = Mesh::parse(&member(game_sim::flight::HULL)).expect("biplanef.col");
    Some((game_sim::flight::Flight::new(&plane).with_rays(game_sim::flight_search::Rays::of(&rays)), hull))
}

/// `plane` with its body (and where it was a substep ago) at `at`, rolling at 10 ft/s along -x.
fn flying_at(plane: &game_sim::flight::Flight, at: Vf) -> game_sim::flight::Flight {
    let mut p = plane.clone();
    let v = Vf::new(-10.0, 0.0, 0.0, 0.0);
    p.body.position = at;
    p.body.last_position = Vf::new(at.x() + 10.0 * DT.0, at.y(), at.z(), 1.0);
    p.body.velocity = v;
    p
}

#[test]
fn a_plane_by_the_near_seam_strikes_a_ball_by_the_far_one() {
    let Some((set, _)) = knock_disc() else { return };
    let Some(plane) = the_plane() else { return };
    // The plane ahead of a ball on +x, where its rays strike it.
    let mut found = None;
    let mut roomy = Strip { flat: game_sim::world::Flat::default(), width: F(40_000.0), wraps: false };
    'find: for (c, coll) in set.collections.iter().enumerate() {
        let Some(o) = coll.objects.first().filter(|o| o.collision.is_some()) else { continue };
        let ball = o.node.origin;
        for ahead in (4..80).map(|k| k as f32 * 0.25) {
            for up in [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, -0.5, -1.0] {
                let at = Vf::new(ball.x() + ahead, ball.y() + up, ball.z(), 1.0);
                let mut p = flying_at(&plane, at);
                if p.search_props(DT, &mut roomy, &set).iter().any(|s| s.0 == c && !s.2.is_empty()) {
                    found = Some((c, at));
                    break 'find;
                }
            }
        }
    }
    let (c, at) = found.expect("some spot by some ball where the plane's rays strike it");
    // A world whose far seam is the whole foot under the plane, the plane a width back.
    let width = at.x().floor();
    let near = Vf::new(at.x() - width, at.y(), at.z(), 1.0);
    assert!((0.0..1.0).contains(&near.x()));
    let mut wrapped = Strip { flat: game_sim::world::Flat::default(), width: F(width), wraps: true };
    let mut plain = Strip { flat: game_sim::world::Flat::default(), width: F(width), wraps: false };
    assert!(flying_at(&plane, near).search_props(DT, &mut plain, &set).iter().all(|s| s.2.is_empty()), "alone, the ball is a width from the plane");

    // One frame among the props, as a lone plane's (the shell's split-screen seat's): the ball's list
    // takes the same mirrors at its own place, and the plane is noted a width over.
    const ME: usize = 0;
    let tables = game_sim::ride::Tables::build();
    let frame = |at: Vf, world: &mut Strip| {
        let mut set = set.clone();
        let mut p = flying_at(&plane, at);
        p.frame_props(DT, Default::default(), world, &tables, &[], None, &mut set, ME);
        (set, p)
    };
    let (want, wp) = frame(at, &mut roomy);
    let (got, gp) = frame(near, &mut wrapped);
    let (wl, gl) = (&want.collections[c].objects[0].collision.as_ref().unwrap().record.contacts, &got.collections[c].objects[0].collision.as_ref().unwrap().record.contacts);
    assert!(!wl.is_empty() && wl.len() == gl.len(), "the ball's list: {} vs {}", gl.len(), wl.len());
    for (g, w) in gl.iter().zip(wl) {
        assert_eq!((g.kind, g.other, g.surface), (w.kind, w.other, w.surface));
        assert!(close(g.normal, w.normal), "mirror normal {:?} vs {:?}", g.normal, w.normal);
        assert!(close(g.point, w.point), "mirror point at the ball's own place {:?} vs {:?}", g.point, w.point);
    }
    assert!(got.collections[c].bones[0].body.enabled, "the strike woke the ball");
    assert!(got.across.iter().any(|a| (a.collection, a.object, a.player, a.by) == (c, 0, ME, [F(width), F::ZERO])), "{:?}", got.across);
    assert!(want.across.is_empty());
    // The plane's frame pushed the ball a width back, by the plane, and wrote it back at its place.
    let (g, w) = (got.collections[c].bones[0].body, want.collections[c].bones[0].body);
    assert!(close(g.position, w.position), "ball {:?} vs {:?}", g.position, w.position);
    assert!(close(g.velocity, w.velocity), "ball velocity {:?} vs {:?}", g.velocity, w.velocity);
    assert!(close_by(gp.body.position, wp.body.position, width), "plane {:?} vs {:?}", gp.body.position, wp.body.position);
    assert!(close(gp.body.velocity, wp.body.velocity), "plane velocity {:?} vs {:?}", gp.body.velocity, wp.body.velocity);
    assert!(gp.record.contacts.iter().any(|e| e.other.is_some()), "the plane's entries name the ball");

    // The props' step sees the plane a width over, by the ball, and puts it back.
    use game_sim::prop_set::PropSet;
    let movers = game_sim::prop_set::PropMovers::default();
    let step = |mut set: PropSet, mut p: game_sim::flight::Flight| {
        let before = p.body;
        set.step_flown(DT, &movers, &mut p.collider, &mut p.body, p.mass);
        (set, p, before)
    };
    let (want, wp, _) = step(want, wp);
    let (got, gp, gbefore) = step(got, gp);
    assert!(got.across.is_empty(), "the notes are used up");
    let (g, w) = (got.collections[c].bones[0].body, want.collections[c].bones[0].body);
    assert!(close(g.position, w.position), "ball {:?} vs {:?}", g.position, w.position);
    assert!(close(g.velocity, w.velocity), "ball velocity {:?} vs {:?}", g.velocity, w.velocity);
    assert!(close(g.rates, w.rates), "ball rates {:?} vs {:?}", g.rates, w.rates);
    assert!(close(gp.body.velocity, wp.body.velocity), "plane velocity {:?} vs {:?}", gp.body.velocity, wp.body.velocity);
    if bits(wp.body.position) == bits(flying_at(&plane, at).body.position) {
        assert_eq!(bits(gp.body.position), bits(gbefore.position), "a place the step left alone is back word for word");
    }

    // A whole tick of a Sim flying the plane (the late-search path, B34): the ball goes where it goes
    // with the plane at its own place.
    let tick = |at: Vf, world: &mut Strip| {
        let mut sim = game_sim::Sim::new(0x5254);
        sim.traffic.props = set.clone();
        sim.plane = Some(flying_at(&plane, at));
        sim.step(DT, &Default::default(), world);
        sim
    };
    let want = tick(at, &mut roomy);
    let got = tick(near, &mut wrapped);
    let (g, w) = (got.traffic.props.collections[c].bones[0].body, want.traffic.props.collections[c].bones[0].body);
    assert!(w.velocity != Vf::ZERO || w.rates != Vf::ZERO, "the tick moved the ball: {w:?}");
    assert!(close(g.position, w.position), "ball {:?} vs {:?}", g.position, w.position);
    assert!(close(g.velocity, w.velocity), "ball velocity {:?} vs {:?}", g.velocity, w.velocity);
    let (gp, wp) = (&got.plane.as_ref().unwrap().body, &want.plane.as_ref().unwrap().body);
    assert!(close(gp.velocity, wp.velocity), "plane velocity {:?} vs {:?}", gp.velocity, wp.velocity);
    assert!(got.traffic.props.across.is_empty());
    // A plane its frame carries back over the near seam, onto the ball's side: the props' step sees
    // it where it now is, not a width further on.
    let over = at.x() - 0.1;
    let mut crossing = Strip { flat: game_sim::world::Flat::default(), width: F(over), wraps: true };
    let got = tick(Vf::new(0.1, at.y(), at.z(), 1.0), &mut crossing);
    let gp = &got.plane.as_ref().unwrap().body;
    assert!(gp.position.x() > over - 1.0, "the plane went over the seam: {:?}", gp.position);
    let (g, w) = (got.traffic.props.collections[c].bones[0].body, want.traffic.props.collections[c].bones[0].body);
    assert!(close(g.position, w.position), "ball {:?} vs {:?}", g.position, w.position);
    assert!(close(g.velocity, w.velocity), "ball velocity {:?} vs {:?}", g.velocity, w.velocity);
    assert!(close(gp.velocity, wp.velocity), "plane velocity {:?} vs {:?}", gp.velocity, wp.velocity);
    // On a world that does not wrap the plane by the near seam flies through: the ball sleeps on.
    let alone = tick(near, &mut plain);
    assert!(!alone.traffic.props.collections[c].bones[0].body.enabled || alone.traffic.props.collections[c].bones[0].body.velocity == set.collections[c].bones[0].body.velocity);
}

// MD254a: planes flown together strike the props across a seam; the props' rays meet a hull a width
// over.

/// A spot by some ball (its collection) where `plane`'s rays strike it, `ahead` of it along `dir`
/// (x, z) and a little up or down, on a world that does not wrap; none past `beyond` along x.
fn ball_spot(set: &game_sim::prop_set::PropSet, plane: &game_sim::flight::Flight, dir: [f32; 2], only: Option<usize>, beyond: f32) -> Option<(usize, Vf)> {
    let mut roomy = Strip { flat: game_sim::world::Flat::default(), width: F(40_000.0), wraps: false };
    for (c, coll) in set.collections.iter().enumerate().filter(|(c, _)| only.is_none_or(|o| o == *c)) {
        let Some(o) = coll.objects.first().filter(|o| o.collision.is_some()) else { continue };
        let ball = o.node.origin;
        for ahead in (4..80).map(|k| k as f32 * 0.25) {
            for up in [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, -0.5, -1.0] {
                let at = Vf::new(ball.x() + dir[0] * ahead, ball.y() + up, ball.z() + dir[1] * ahead, 1.0);
                if at.x() >= beyond {
                    continue;
                }
                let mut p = flying_at(plane, at);
                if p.search_props(DT, &mut roomy, set).iter().any(|s| s.0 == c && !s.2.is_empty()) {
                    return Some((c, at));
                }
            }
        }
    }
    None
}

/// Two planes flown together (split screen's, [`game_sim::flight::frames_together_props`]): one by
/// the near seam strikes a ball by the far one, the other at the ball's own side; the ball's list,
/// its body, the planes and the props' step after go as with the first plane at the ball's own place.
/// With the hulls on, so the balls' rays on them run too, shared as the fleet's one list has them.
#[test]
fn planes_flown_together_strike_a_ball_by_the_far_seam() {
    use game_sim::contact::{Other, Side, Weighed};
    use game_sim::flight::{self, Fly};
    let Some((set, _)) = knock_disc() else { return };
    let Some((plane, hull)) = the_plane_and_hull() else { return };
    let plane = plane.with_hull(hull);
    let (c, at) = ball_spot(&set, &plane, [1.0, 0.0], None, f32::MAX).expect("a spot on +x where the plane's rays strike a ball");
    let width = at.x().floor();
    let near = Vf::new(at.x() - width, at.y(), at.z(), 1.0);
    // The other plane on the ball's other side, inside the wrapped world.
    let other = [[-1.0, 0.0], [0.0, 1.0], [0.0, -1.0]]
        .into_iter()
        .find_map(|dir| ball_spot(&set, &plane, dir, Some(c), width - 1.0))
        .map(|(_, at)| at)
        .expect("a second spot by the ball");
    let roomy = || Strip { flat: game_sim::world::Flat::default(), width: F(40_000.0), wraps: false };
    let wrapped = || Strip { flat: game_sim::world::Flat::default(), width: F(width), wraps: true };
    let tables = game_sim::ride::Tables::build();
    const SEATS: [usize; 2] = [0, 1];
    let movers = game_sim::prop_set::PropMovers::default();
    // One frame together, then the props' step with both planes as its strikers, by seat.
    let fly = |places: [Vf; 2], mut world: Strip| {
        let mut set = set.clone();
        let mut planes = places.map(|at| flying_at(&plane, at));
        let [a, b] = &mut planes;
        let mut fleet = [Fly { flight: a, pad: Default::default(), met: None, bikes: &[] }, Fly { flight: b, pad: Default::default(), met: None, bikes: &[] }];
        let _ = flight::frames_together_props(DT, &mut fleet, &mut world, &tables, Some((&mut set, &SEATS)));
        let framed = (set.clone(), planes.clone());
        let [a, b] = &mut planes;
        let (moved, rest) = game_sim::prop_contact::across_flown(std::mem::take(&mut set.across), &mut [Some(&mut a.body), Some(&mut b.body)]);
        assert!(rest.is_empty());
        let (mut wa, mut wb) = (Weighed { body: &mut a.body, mass: a.mass }, Weighed { body: &mut b.body, mass: b.mass });
        let mut strikers = [Other { collider: &mut a.collider, side: Side::Body(&mut wa) }, Other { collider: &mut b.collider, side: Side::Body(&mut wb) }];
        set.step_with(DT, &movers, &mut strikers);
        drop(strikers);
        game_sim::prop_contact::across_flown_back(moved, &mut [Some(&mut a.body), Some(&mut b.body)]);
        (framed, (set, planes))
    };
    let ball = |set: &game_sim::prop_set::PropSet| set.collections[c].bones[0].body;
    let list = |set: &game_sim::prop_set::PropSet| set.collections[c].objects[0].collision.as_ref().unwrap().record.contacts.clone();
    for (order, by) in [([0, 1], 0usize), ([1, 0], 1)] {
        let pick = |a: Vf, b: Vf| if order[0] == 0 { [a, b] } else { [b, a] };
        let ((wset, wplanes), (wstep, wflown)) = fly(pick(at, other), roomy());
        let ((gset, gplanes), (gstep, gflown)) = fly(pick(near, other), wrapped());
        let (wl, gl) = (list(&wset), list(&gset));
        // The later plane's mirrors go at the head of the ball's list, which may keep none of the
        // earlier's; both planes' lists name it.
        assert!(!wl.is_empty());
        for k in 0..2 {
            assert!(wplanes[k].record.contacts.iter().any(|e| e.kind == 0 && e.other.is_some()), "plane {k} struck the ball");
            assert!(gplanes[k].record.contacts.iter().any(|e| e.kind == 0 && e.other.is_some()), "plane {k} struck the ball, wrapped");
        }
        assert_eq!(gl.len(), wl.len(), "the ball's list\ngot {gl:?}\nwant {wl:?}");
        for (g, w) in gl.iter().zip(&wl) {
            assert_eq!((g.kind, g.other, g.surface), (w.kind, w.other, w.surface));
            assert!(close(g.normal, w.normal), "normal {:?} vs {:?}", g.normal, w.normal);
            assert!(close(g.point, w.point), "point at the ball's own place {:?} vs {:?}", g.point, w.point);
        }
        assert!(gset.across.iter().any(|a| (a.collection, a.object, a.player, a.by) == (c, 0, SEATS[by], [F(width), F::ZERO])), "{:?}", gset.across);
        assert!(gset.across.iter().all(|a| a.player == SEATS[by]), "the other plane struck it at its place: {:?}", gset.across);
        assert!(wset.across.is_empty());
        let (g, w) = (ball(&gset), ball(&wset));
        assert!(g.enabled);
        assert!(close(g.position, w.position), "ball {:?} vs {:?}", g.position, w.position);
        assert!(close(g.velocity, w.velocity), "ball velocity {:?} vs {:?}", g.velocity, w.velocity);
        assert!(close(g.rates, w.rates), "ball rates {:?} vs {:?}", g.rates, w.rates);
        for k in 0..2 {
            let shift = if k == by { width } else { 0.0 };
            assert!(close_by(gplanes[k].body.position, wplanes[k].body.position, shift), "plane {k} {:?} vs {:?}", gplanes[k].body.position, wplanes[k].body.position);
            assert!(close(gplanes[k].body.velocity, wplanes[k].body.velocity), "plane {k} velocity {:?} vs {:?}", gplanes[k].body.velocity, wplanes[k].body.velocity);
            assert!(close(gflown[k].body.velocity, wflown[k].body.velocity), "plane {k} after the props' step {:?} vs {:?}", gflown[k].body.velocity, wflown[k].body.velocity);
            assert!(close_by(gflown[k].body.position, wflown[k].body.position, shift), "the props' step puts plane {k} back {:?} vs {:?}", gflown[k].body.position, wflown[k].body.position);
        }
        let (g, w) = (ball(&gstep), ball(&wstep));
        assert!(close(g.position, w.position), "ball after the step {:?} vs {:?}", g.position, w.position);
        assert!(close(g.velocity, w.velocity), "ball velocity after the step {:?} vs {:?}", g.velocity, w.velocity);
        assert!(close(g.rates, w.rates), "ball rates after the step {:?} vs {:?}", g.rates, w.rates);
    }
    // The plane at x 0.1 flying -x on a world 0.1 ft narrower: its frame carries it back over the
    // near seam, onto the ball's side, and the props' step sees it where it now is.
    let over = at.x() - 0.1;
    let ((wset, wplanes), (wstep, wflown)) = fly([at, other], roomy());
    let ((gset, gplanes), (gstep, gflown)) = fly([Vf::new(0.1, at.y(), at.z(), 1.0), other], Strip { flat: game_sim::world::Flat::default(), width: F(over), wraps: true });
    assert!(gplanes[0].body.position.x() > over - 1.0, "the plane went over the seam: {:?}", gplanes[0].body.position);
    assert!(gset.across.iter().all(|a| a.player != SEATS[0]), "no note left for it: {:?}", gset.across);
    assert!(gset.collections[c].bones[0].body.enabled);
    for (g, w) in [(ball(&gset), ball(&wset)), (ball(&gstep), ball(&wstep))] {
        assert!(close(g.position, w.position), "ball {:?} vs {:?}", g.position, w.position);
        assert!(close(g.velocity, w.velocity), "ball velocity {:?} vs {:?}", g.velocity, w.velocity);
    }
    for k in 0..2 {
        assert!(close(gplanes[k].body.velocity, wplanes[k].body.velocity), "plane {k} velocity {:?} vs {:?}", gplanes[k].body.velocity, wplanes[k].body.velocity);
        assert!(close(gflown[k].body.velocity, wflown[k].body.velocity), "plane {k} after the step {:?} vs {:?}", gflown[k].body.velocity, wflown[k].body.velocity);
    }
    // On a world that does not wrap the plane by the near seam strikes nothing: only the other's
    // mirrors are on the ball's list.
    let ((plain, _), _) = fly([near, other], Strip { flat: game_sim::world::Flat::default(), width: F(width), wraps: false });
    assert!(list(&plain).iter().all(|e| e.other != Some(SEATS[0])), "{:?}", list(&plain));
}

/// **The balls' rays meet a hull by the far seam** ([`game_sim::prop_wreck::struck_on_in`]): a filed
/// ball whose rays strike the plane's hull at its own place strikes it the same with the hull a
/// width back, by the near seam: its entries at its own place, the mirrors at the hull's, the body
/// noted with the shift. On a world that does not wrap, nothing. Then a lone plane's frame among
/// the props with its hull ([`game_sim::flight::Flight::frame_props`]) goes as at the ball's place.
#[test]
fn the_balls_rays_meet_a_hull_by_the_far_seam() {
    use game_sim::flight;
    use game_sim::prop_wreck::struck_on_in;
    let Some((mut set, _)) = knock_disc() else { return };
    let Some((plane, hull)) = the_plane_and_hull() else { return };
    // Every ball filed, as a strike leaves one.
    for c in 0..set.collections.len() {
        if set.collections[c].objects.first().is_some_and(|o| o.collision.is_some()) {
            set.struck(c, 0);
        }
    }
    let mut roomy = Strip { flat: game_sim::world::Flat::default(), width: F(40_000.0), wraps: false };
    let node = |at: Vf| {
        let n = game_sim::wreck_bike::node_of(&flying_at(&plane, at).body);
        [n.rows[0], n.rows[1], n.rows[2], n.origin]
    };
    let strike = |set: &game_sim::prop_set::PropSet, at: Vf, world: &Strip| {
        let mut set = set.clone();
        let out = struck_on_in(&mut set, &hull, node(at), flight::COLLIDABLE, flight::COLLIDES_WITH, 0, world);
        (set, out)
    };
    let mut found = None;
    'find: for (c, coll) in set.collections.iter().enumerate() {
        let Some(o) = coll.objects.first().filter(|o| o.collision.is_some()) else { continue };
        let ball = o.node.origin;
        for ahead in (0..60).map(|k| k as f32 * 0.25) {
            for up in [0.0, 0.5, 1.0, 1.5, 2.0, -0.5, -1.0, -2.0] {
                let at = Vf::new(ball.x() + ahead, ball.y() + up, ball.z(), 1.0);
                let (_, (on, _)) = strike(&set, at, &roomy);
                if on.struck.iter().any(|p| p.collection == c) {
                    found = Some((c, at));
                    break 'find;
                }
            }
        }
    }
    let (c, at) = found.expect("some spot by some ball whose rays strike the hull there");
    let width = at.x().floor();
    let near = Vf::new(at.x() - width, at.y(), at.z(), 1.0);
    let wrapped = Strip { flat: game_sim::world::Flat::default(), width: F(width), wraps: true };
    let plain = Strip { flat: game_sim::world::Flat::default(), width: F(width), wraps: false };
    let (wset, (want, wacross)) = strike(&set, at, &roomy);
    let (gset, (got, gacross)) = strike(&set, near, &wrapped);
    assert!(wacross.is_empty());
    let i = want.struck.iter().position(|p| p.collection == c).unwrap();
    let j = got.struck.iter().position(|p| p.collection == c).expect("the ball struck the hull a width over");
    assert!(gacross.contains(&(j, [F(width), F::ZERO])), "{gacross:?}");
    let (wl, gl) = (&wset.collections[c].objects[0].collision.as_ref().unwrap().record.contacts, &gset.collections[c].objects[0].collision.as_ref().unwrap().record.contacts);
    assert_eq!(gl.len(), wl.len(), "the ball's entries\ngot {gl:?}\nwant {wl:?}");
    for (g, w) in gl.iter().zip(wl) {
        assert_eq!((g.kind, g.other, g.surface), (w.kind, w.other, w.surface));
        assert!(close(g.normal, w.normal), "normal {:?} vs {:?}", g.normal, w.normal);
        assert!(close(g.point, w.point), "entry at the ball's own place {:?} vs {:?}", g.point, w.point);
    }
    let (wm, gm): (Vec<_>, Vec<_>) = (want.mirrors.iter().filter(|e| e.other == Some(i)).collect(), got.mirrors.iter().filter(|e| e.other == Some(j)).collect());
    assert!(!wm.is_empty() && wm.len() == gm.len(), "the hull's mirrors: {} vs {}", gm.len(), wm.len());
    for (g, w) in gm.iter().zip(&wm) {
        assert!(close(g.normal, w.normal), "mirror normal {:?} vs {:?}", g.normal, w.normal);
        assert!(close_by(g.point, w.point, width), "mirror at the hull's place {:?} vs {:?}", g.point, w.point);
    }
    assert!(bits(got.struck[j].body.position) == bits(want.struck[i].body.position), "the body as it stands");
    let (_, (none, _)) = strike(&set, near, &plain);
    assert!(none.struck.iter().all(|p| p.collection != c), "a world that does not wrap");
    // A ball the hull's own place meets is met there alone, word for word as struck_on.
    let mut own = set.clone();
    let alone = game_sim::prop_wreck::struck_on(&mut own, &hull, node(at), flight::COLLIDABLE, flight::COLLIDES_WITH, 0);
    let (_, (wide, wide_across)) = strike(&set, at, &Strip { flat: game_sim::world::Flat::default(), width: F(40_000.0), wraps: true });
    assert_eq!(alone, wide);
    assert!(wide_across.is_empty());

    // The lone plane's frame with its hull, by the near seam and at the ball's own place.
    let plane = plane.with_hull(hull.clone());
    let tables = game_sim::ride::Tables::build();
    let frame = |at: Vf, world: &mut Strip| {
        let mut set = set.clone();
        let mut p = flying_at(&plane, at);
        p.frame_props(DT, Default::default(), world, &tables, &[], None, &mut set, 0);
        (set, p)
    };
    let (want, wp) = frame(at, &mut roomy);
    let (got, gp) = frame(near, &mut Strip { flat: game_sim::world::Flat::default(), width: F(width), wraps: true });
    let (wl, gl) = (&want.collections[c].objects[0].collision.as_ref().unwrap().record.contacts, &got.collections[c].objects[0].collision.as_ref().unwrap().record.contacts);
    assert_eq!(gl.len(), wl.len(), "the ball's list\ngot {gl:?}\nwant {wl:?}");
    for (g, w) in gl.iter().zip(wl) {
        assert_eq!((g.kind, g.other, g.surface), (w.kind, w.other, w.surface));
        assert!(close(g.point, w.point), "point {:?} vs {:?}", g.point, w.point);
    }
    assert!(got.across.iter().any(|a| (a.collection, a.object, a.by) == (c, 0, [F(width), F::ZERO])), "{:?}", got.across);
    let (g, w) = (got.collections[c].bones[0].body, want.collections[c].bones[0].body);
    assert!(close(g.position, w.position), "ball {:?} vs {:?}", g.position, w.position);
    assert!(close(g.velocity, w.velocity), "ball velocity {:?} vs {:?}", g.velocity, w.velocity);
    assert_eq!(gp.record.contacts.len(), wp.record.contacts.len(), "the plane's list");
    assert!(close(gp.body.velocity, wp.body.velocity), "plane velocity {:?} vs {:?}", gp.body.velocity, wp.body.velocity);
}
