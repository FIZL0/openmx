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
//! * a world that does not wrap mirrors the list alone, as before.

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
