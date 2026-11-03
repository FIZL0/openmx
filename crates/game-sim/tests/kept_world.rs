//! S236: the plane's and the ride's searches keep their world between searches rather than build it
//! afresh for each ([`flight_search::kept`]), and bring a kept one up to the props that moved since.
//! A world built afresh every search was most of a Steam Deck's 17-24 ms ticks on a map of 577 props
//! and 23 path cars.
//!
//! 1. **the same world** — over a ride of ticks in which path cars drive, one leaves the props' box,
//!    one stops and drives again, a still prop is moved and the bikes ride and leave the world, the
//!    kept world handed to every search is the fresh build's ([`flight_search::world_wooded`],
//!    [`flight_search::world_flown`]) bit for bit: its broad phase, every query field included, and its
//!    collidables; with plants and without, with bikes and without, over a field, an edgeless field and
//!    none;
//! 2. **the same searches** — the nearby list each tick's refresh gathers is a fresh world's, and the
//!    ray probe's hits through the plane's scene the same as with nothing kept;
//! 3. **no build a steady tick** — a tick whose props stand still builds no world and updates none,
//!    and one whose path cars drive updates one and builds none.
//!
//! Everything is written here: posts for props, a two-species ecosystem, a cross of rays.

use game_sim::broad_phase;
use game_sim::camera::CurveTable;
use game_sim::contact::Node as Frame;
use game_sim::contact_search::Search;
use game_sim::eco::{self, Ecosystem, Plant, Solid, Species, Trees};
use game_sim::flight_search::{self, Bike, Bikes, Rays};
use game_sim::math::{self, Mat4};
use game_sim::ray_probe;
use game_sim::track::{self, Mesh, Node, Placement, Points, Triangle};
use game_sim::traffic::Moving;
use game_sim::world::{Prop, Props};
use ps2_float::{F, Vf};

/// A wall facing −z, ten feet wide and ten high, in its own space at the origin.
fn post() -> Mesh {
    let v = |x: f32, y: f32| [F(x), F(y), F::ZERO];
    Mesh {
        version: 0,
        surface: None,
        transform: [[F::ONE, F::ZERO, F::ZERO], [F::ZERO, F::ONE, F::ZERO], [F::ZERO, F::ZERO, F::ONE], [F::ZERO, F::ZERO, F::ZERO]],
        vertices: Points::Wide(vec![v(-5.0, 0.0), v(5.0, 0.0), v(5.0, 10.0), v(-5.0, 10.0)]),
        normals: None,
        nodes: vec![Node { volume: F::ZERO, centre: [F::ZERO, F(5.0), F::ZERO], half_extent: [F(5.0), F(5.0), F(0.5)], children: [0x8000, 0x8001] }],
        triangles: vec![
            Triangle { distance: F::ZERO, normal: [F::ZERO, F::ZERO, F(-1.0)], vertices: [0, 2, 1] },
            Triangle { distance: F::ZERO, normal: [F::ZERO, F::ZERO, F(-1.0)], vertices: [0, 3, 2] },
        ],
    }
}

/// A post at `(x, z)`, facing `look`.
fn at(x: f32, z: f32, look: [f32; 2]) -> Prop {
    let p = Placement {
        position: [F(x), F::ZERO, F(z)],
        look: [F(look[0]), F::ZERO, F(look[1])],
        up: [F::ZERO, F::ONE, F::ZERO],
        ..Placement::default()
    };
    Prop::place(post(), &p)
}

/// Car `k`'s post at tick `t`: driving round a loop of its own, inside the props' box but for car 0,
/// which runs out past it from tick 12 to 15 — and stopped (still, no longer a toy) from tick 20 to 23
/// for car 1.
fn car(k: usize, t: usize) -> Prop {
    let a = (t as f32) * 0.3 + k as f32;
    let r = if k == 0 && (12..16).contains(&t) { 900.0 } else { 40.0 };
    let mut p = at(100.0 + 50.0 * k as f32 + r * a.cos(), 120.0 + r * a.sin(), [a.sin(), -a.cos()]);
    if !(k == 1 && (20..24).contains(&t)) {
        p.moving = Some(Moving { pose: p.to_world, velocity: Vf::new(1.0, 0.0, 2.0, 0.0), spin: Vf::ZERO });
    }
    p
}

/// Tick `t`'s props: forty posts standing on a grid (the fifth moved at tick 8 and back at tick 9),
/// then four cars.
fn props(t: usize) -> Props {
    let mut props: Vec<Prop> = (0..40)
        .map(|i| {
            let shift = if i == 5 && t == 8 { 7.0 } else { 0.0 };
            at((i % 8) as f32 * 45.0 + shift, (i / 8) as f32 * 60.0, [0.0, 1.0])
        })
        .collect();
    props.extend((0..4).map(|k| car(k, t)));
    Props { props }
}

/// Two species of one cylinder each, eighty plants across the posts' grid.
fn trees() -> Trees {
    let species = |r: f32, h: f32| Species {
        model: "trunk".into(),
        height: [F(h), F(h * 0.8), F(h * 1.2)],
        width: [F(r), F(r * 0.8), F(r * 1.5)],
        solids: vec![Solid { kind: eco::CYLINDER, at: [F::ZERO; 3], radius: F(r), height: F(h) }],
    };
    let mut slots = vec![None; 256];
    slots[3] = Some(species(1.5, 20.0));
    slots[7] = Some(species(0.8, 12.0));
    let plants = (0..80u16)
        .map(|i| Plant { species: if i % 3 == 0 { 7 } else { 3 }, snap: false, cell: [i % 10 * 400 + 37, 0, i / 10 * 600 + 91], height: (i % 7) as i8 * 9 - 30, width: (i % 5) as i8 * 20 })
        .collect();
    let eco = Ecosystem { version: 1, species: slots, plants };
    let (trees, missed) = Trees::new(&eco, eco::cell_size(F(4096.0)), &CurveTable::build());
    assert!(missed.is_empty(), "{missed:?}");
    trees
}

/// The bikes at tick `t`: two, the second out of the world on odd ticks.
fn bikes(t: usize) -> Vec<Bike> {
    (0..2)
        .map(|k| {
            let a = t as f32 * 0.2 + k as f32 * 2.0;
            let pos = Vf::new(150.0 + 30.0 * a.cos(), 2.0, 100.0 + 30.0 * a.sin(), 1.0);
            Bike { transform: math::basis(pos, Vf::new(a.sin(), 0.0, a.cos(), 0.0), Vf::new(0.0, 1.0, 0.0, 0.0), false, true), in_world: k == 0 || t % 2 == 0 }
        })
        .collect()
}

/// A cross of rays ten feet across, its tree's one node over both.
fn rays() -> Rays {
    let p = |x: f32, y: f32, z: f32| Vf::new(x, y, z, 1.0);
    Rays {
        root: 0,
        nodes: vec![track::Node { volume: F::ZERO, centre: [F::ZERO, F(4.0), F::ZERO], half_extent: [F(6.0), F(1.0), F(6.0)], children: [0x8000, 0x8001] }],
        segments: vec![
            ray_probe::Segment { from: p(-6.0, 4.0, 0.0), to: p(6.0, 4.0, 0.0) },
            ray_probe::Segment { from: p(0.0, 4.0, -6.0), to: p(0.0, 4.0, 6.0) },
        ],
    }
}

/// The plane at tick `t`: flying up through two rows of posts and the cars' first loop, turning a
/// little.
fn node(t: usize) -> Frame {
    let a = t as f32 * 0.02;
    let m: Mat4 = math::basis(Vf::new(90.0 + 0.2 * t as f32, 5.0, 50.0 + 4.0 * t as f32, 1.0), Vf::new(a.sin(), 0.0, a.cos(), 0.0), Vf::new(0.0, 1.0, 0.0, 0.0), false, true);
    Frame { rows: [m.0[0], m.0[1], m.0[2]], origin: m.0[3] }
}

const TICKS: usize = 30;

/// The fields: the ground's box, one with no edges, and none.
fn fields() -> [Option<([F; 2], [F; 2])>; 3] {
    [Some(([F(-50.0), F(-80.0)], [F(500.0), F(400.0)])), Some(([F(-3.0e7), F(-3.0e7)], [F(3.0e7), F(3.0e7)])), None]
}

/// **The kept world is the fresh one**: every tick, for the ride's world and the plane's, at two
/// reaches, what a search is handed is the fresh build's bit for bit — and the ride does update its
/// kept worlds, so the update is what is held up.
#[test]
fn the_kept_world_is_the_fresh_one() {
    let mesh = post();
    let woods = trees();
    let rays = rays();
    for field in fields() {
        for trees in [None, Some(&woods)] {
            for with_bikes in [false, true] {
                flight_search::forget_worlds();
                let before = flight_search::world_builds();
                for t in 0..TICKS {
                    let props = props(t);
                    let riders = bikes(t);
                    let bikes = with_bikes.then_some(Bikes { mesh: &mesh, bikes: &riders });
                    let (centre, radius) = flight_search::reach(&rays, &{
                        let n = node(t);
                        [n.rows[0], n.rows[1], n.rows[2], n.origin]
                    });
                    for toys in [false, true] {
                        for (centre, radius) in [(centre, radius), (Vf::new(200.0, 3.0, 90.0, 1.0), F(0.5))] {
                            let fresh = if toys {
                                flight_search::world_flown(&props.props, trees, bikes.as_ref(), field, centre, radius)
                            } else {
                                flight_search::world_wooded(&props.props, trees, bikes.as_ref(), field, centre, radius)
                            };
                            let kept = flight_search::kept(&props.props, trees, bikes.as_ref(), field, centre, radius, toys);
                            let what = format!("tick {t}, field {field:?}, plants {}, bikes {with_bikes}, toys {toys}", trees.is_some());
                            assert!(kept.1 == fresh.1, "{what}: the collidables");
                            assert!(kept.0 == fresh.0, "{what}: the broad phase");
                        }
                    }
                }
                let after = flight_search::world_builds();
                // The ride and the plane each build once, and again only when a car leaves the box,
                // comes back, stops or starts.
                assert!(after.1 - before.1 >= 20, "field {field:?}: only {} updates", after.1 - before.1);
            }
        }
    }
}

/// **The same searches**: each tick's refresh gathers the fresh world's nearby list, and the plane's
/// ray probe strikes the same posts and plants, at the same points, as with nothing kept.
#[test]
fn the_kept_world_searches_as_the_fresh_one() {
    let mesh = post();
    let woods = trees();
    let rays = rays();
    for field in fields() {
        for trees in [None, Some(&woods)] {
            flight_search::forget_worlds();
            let (mut kept, mut fresh) = (flight_search::search(), flight_search::search());
            let mut hits = 0;
            for t in 0..TICKS {
                let props = props(t);
                let riders = bikes(t);
                let bikes = || Some(Bikes { mesh: &mesh, bikes: &riders });
                let n = node(t);
                flight_search::refresh_flown(Some(&props), trees, bikes().as_ref(), field, &rays, &n, &mut kept);
                let (centre, radius) = flight_search::reach(&rays, &[n.rows[0], n.rows[1], n.rows[2], n.origin]);
                let (mut broad, mut objects) = flight_search::world_flown(&props.props, trees, bikes().as_ref(), field, centre, radius);
                if fresh.nearby.found.iter().any(|c| objects.len() <= c.object) {
                    fresh.built = false;
                    fresh.nearby.found.clear();
                }
                broad_phase::gather(&mut objects, &mut broad, &mut fresh, flight_search::RAY, 3);
                assert_eq!(kept, fresh, "tick {t}, field {field:?}: the nearby list");

                // Twice against the kept world, the second with the collidables the first searched in
                // handed back, then against one built afresh.
                let search = |kept: &broad_phase::Search| {
                    let mut s = kept.clone();
                    flight_search::Scene::flown(Some(&props), trees, bikes(), field, &rays, &n, &mut s).rays(true)
                };
                let (once, twice) = (search(&kept), search(&kept));
                flight_search::forget_worlds();
                let theirs = search(&kept);
                assert_eq!(once, theirs, "tick {t}, field {field:?}: the ray probe's hits");
                assert_eq!(twice, theirs, "tick {t}, field {field:?}: the ray probe's hits, searched again");
                hits += theirs.len();
            }
            assert!(hits >= 4, "field {field:?}: the plane struck {hits} times");
        }
    }
}

/// **The riding bike's refresh in the kept world** (S254): each tick's [`flight_search::refresh_ridden`]
/// gathers the nearby list a fresh plane's world gives with the ray probe's category list the ignore
/// list, a ghost's ([`flight_search::GHOST`]) every third tick; and the ignore list does not stay on
/// the collidables handed back, so the plane's gather and scene after it in the same kept world find
/// what they find in a world built afresh (on a thread of its own, which keeps none).
#[test]
fn the_ridden_refresh_gathers_as_the_fresh_one() {
    use game_sim::world_search::Categories;
    let woods = trees();
    let rays = rays();
    for field in fields() {
        for trees in [None, Some(&woods)] {
            flight_search::forget_worlds();
            let (mut kept, mut fresh) = (flight_search::search(), flight_search::search());
            let (mut ghosted, mut solid) = (0, 0);
            for t in 0..TICKS {
                let props = props(t);
                let n = node(t);
                let ignore: &[u32] = if t % 3 == 2 { &[flight_search::GHOST] } else { &[] };
                flight_search::refresh_ridden(Some(&props), trees, field, &rays, &n, &mut kept, ignore);
                let (centre, radius) = flight_search::reach(&rays, &[n.rows[0], n.rows[1], n.rows[2], n.origin]);
                let (mut broad, mut objects) = flight_search::world_flown(&props.props, trees, None, field, centre, radius);
                objects[flight_search::RAY].categories = Categories { mode: 1, list: ignore.to_vec() };
                if fresh.nearby.found.iter().any(|c| objects.len() <= c.object) {
                    fresh.built = false;
                    fresh.nearby.found.clear();
                }
                broad_phase::gather(&mut objects, &mut broad, &mut fresh, flight_search::RAY, 3);
                assert_eq!(kept, fresh, "tick {t}, field {field:?}, ignore {ignore:x?}: the nearby list");
                if ignore.is_empty() { solid += kept.nearby.found.len() } else { ghosted += kept.nearby.found.len() }

                let search = |kept: &broad_phase::Search| {
                    let mut s = kept.clone();
                    flight_search::Scene::flown(Some(&props), trees, None, field, &rays, &n, &mut s).rays(true)
                };
                let gathered = || {
                    let mut s = flight_search::search();
                    flight_search::refresh_flown(Some(&props), trees, None, field, &rays, &n, &mut s);
                    s
                };
                // A new thread keeps no world: its search builds one afresh.
                let theirs = std::thread::scope(|s| s.spawn(|| (gathered(), search(&kept))).join().unwrap());
                assert_eq!(gathered(), theirs.0, "tick {t}, field {field:?}: the plane's gather after the ridden refresh");
                assert_eq!(search(&kept), theirs.1, "tick {t}, field {field:?}: the plane's hits after the ridden refresh");
            }
            // Twenty solid ticks and ten ghosted: the ghost's list leaves the cars out.
            assert!(ghosted > 0 && 4 * ghosted < solid, "field {field:?}: {solid} gathered solid, {ghosted} ghosted");
        }
    }
}

/// **No build a steady tick**: a tick of the ride's searches — the wheels' and the sweep's against the
/// ride's world, the plane's two against its own — builds no world once the first tick has; with the
/// cars standing it updates none, and with them driving one a tick for each world.
#[test]
fn a_steady_tick_builds_no_world() {
    let mesh = post();
    let woods = trees();
    let rays = rays();
    let field = fields()[0];
    flight_search::forget_worlds();
    let tick = |t: usize, props: &Props| {
        let riders = bikes(t);
        let bikes = || Some(Bikes { mesh: &mesh, bikes: &riders });
        let n = node(t);
        let mut s = flight_search::search();
        for _ in 0..4 {
            flight_search::Scene::wooded(Some(props), Some(&woods), bikes(), field, &rays, &n, &mut s).rays(true);
        }
        flight_search::refresh_flown(Some(props), Some(&woods), bikes().as_ref(), field, &rays, &n, &mut s);
        flight_search::Scene::flown(Some(props), Some(&woods), bikes(), field, &rays, &n, &mut s).rays(true);
    };
    let still = props(2);
    tick(0, &still);
    let start = flight_search::world_builds();
    for t in 1..10 {
        tick(t, &still);
    }
    assert_eq!(flight_search::world_builds(), start, "standing cars: no build and no update");
    for t in 1..10 {
        let before = flight_search::world_builds();
        tick(t, &props(t));
        let (built, updated) = flight_search::world_builds();
        assert_eq!(built, before.0, "tick {t}: a world built");
        assert_eq!(updated - before.1, 2, "tick {t}: the ride's world and the plane's brought up to the cars");
    }
}

/// Tick `t`'s props with car 2 hidden for its own search, as [`game_sim::traffic::hide`] leaves it:
/// no triangles, no reach, and not moving.
fn props_hiding_car_2(t: usize) -> Props {
    let mut props = props(t);
    let car = &mut props.props[42];
    car.mesh.nodes.clear();
    car.mesh.triangles.clear();
    car.radius = F::ZERO;
    car.moving = None;
    props
}

/// **A car searching with itself hidden builds no world** (N23): a path car with physics on searches
/// the world with its own mesh out of it, its moving word cleared, and the bike's searches see it
/// moving, every tick. Each of the two kept worlds is brought up to its own search's cars rather than
/// one built afresh at each change, and both are the fresh build's.
#[test]
fn a_hidden_car_and_its_unhidden_world_are_both_kept() {
    let field = fields()[0];
    let (centre, radius) = (Vf::new(200.0, 3.0, 90.0, 1.0), F(0.5));
    flight_search::forget_worlds();
    // The kept search's builds and updates, the fresh build's own build left out of the count.
    let mut kept_builds = (0, 0);
    let mut search = |props: &Props, what: &str| {
        let before = flight_search::world_builds();
        let kept = flight_search::kept(&props.props, None, None, field, centre, radius, true);
        let after = flight_search::world_builds();
        (kept_builds.0, kept_builds.1) = (kept_builds.0 + after.0 - before.0, kept_builds.1 + after.1 - before.1);
        let fresh = flight_search::world_flown(&props.props, None, None, field, centre, radius);
        assert!(kept.1 == fresh.1, "{what}: the collidables");
        assert!(kept.0 == fresh.0, "{what}: the broad phase");
    };
    search(&props_hiding_car_2(0), "tick 0, hidden");
    search(&props(0), "tick 0, seen");
    for t in 1..10 {
        search(&props_hiding_car_2(t), &format!("tick {t}, hidden"));
        search(&props(t), &format!("tick {t}, seen"));
    }
    assert_eq!(kept_builds, (2, 18), "one world built for each of the two searches, then each brought up to its cars");
}
