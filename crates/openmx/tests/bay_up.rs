//! S205: a plane taken from its `machine.txt` bay stands as the original's placement stands it — its
//! up the ground query's normal under the bay, squared against its nose as the original's place
//! squares it (`tests/spawn_axes.rs` in game-sim proves that against the original's instructions),
//! and its spot on the ground there or, with the section's `SetGroundHeight` off, at the file's
//! height, lifted by the model's bounds either way (S164).
//!
//! Every biplane bay on the disc is flat (normal `0.99999994` up, journal
//! 2026-11-02-s205-biplane-spawn-up) and no track turns `SetGroundHeight` off for one, so these ride
//! a sloped plane of the port's and a file of this test's own; the disc gives only the plane's
//! `.phy`, and they skip without it. The disc's own bays are checked to stay flat.
//!
//! `src/vehicle.rs` is compiled in by path, the way `tests/machines.rs` takes it.

#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;

use game_render::Disc;
use game_sim::Sim;
use game_sim::archive::Res;
use game_sim::flight::{self, Flight};
use game_sim::phy;
use game_sim::track::{Scene, Terrain};
use game_sim::world::{Flat, Ground, World};
use ps2_float::{F, Vf};
use vehicle::{Lot, Machine, ground_heights};

/// The freeride tracks whose `machine.txt` the machines' archive carries.
const TRACKS: [&str; 5] = ["free06", "free07", "free09", "free10", "free11"];

/// A `machine.txt` in the disc's shape: the first plane as the disc writes its sections, the second
/// with `SetGroundHeight` off, as free06 writes it for one of its bikes.
const MACHINES: &str = "[Machines]\nNumberOfMachines = 2\n\n\
    [Biplane1]\nType = Biplane\nPosition = 100, 20, -50\nDirection = 0, 0, -1\nHasPlayer = 1\n\n\
    [Biplane2]\nType = Biplane\nPosition = 140, 37.5, -50\nDirection = 1, 0, 0\nHasPlayer = 1\nSetGroundHeight = FALSE\n";

/// The lift the bays get here: the disc's plane's is 3.6154737 ft, and any will do.
const LIFT: f32 = 3.5;

fn disc() -> Option<Disc> {
    match Disc::open_configured() {
        Ok(d) => Some(d),
        Err(why) => {
            eprintln!("skipped: {why}");
            None
        }
    }
}

/// The disc's plane, as the shell builds it.
fn the_flight(disc: &mut Disc) -> Flight {
    let machines = disc.res("machine.res").expect("the machines' archive");
    Flight::new(&phy::Plane::read_bytes(&machines.read_suffix(phy::BIPLANE_PHY).expect("biplane.phy")))
}

/// Player `player`'s plane taken from its bay in [`MACHINES`] over `world`, after its first step.
fn taken(flight: Flight, player: usize, world: &mut Flat) -> Flight {
    let mut lot = Lot::new(MACHINES.as_bytes(), 2, &mut |at| world.ground(at).0.y());
    lot.lift = Some(F(LIFT));
    let mut plane = lot.plane(player, Some(flight)).expect("the bay's plane");
    plane.step(&Sim::new(0), F(1.0 / 30.0), world);
    plane.flight.expect("the plane still has its flight")
}

fn xyz(v: Vf) -> [u32; 3] {
    [v.x().to_bits(), v.y().to_bits(), v.z().to_bits()]
}

#[test]
fn set_ground_height_is_read_per_bay() {
    assert_eq!(ground_heights(MACHINES.as_bytes(), Machine::Biplane), [true, false]);
    assert_eq!(ground_heights(MACHINES.as_bytes(), Machine::Helicopter), Vec::<bool>::new());
    let lot = Lot::new(MACHINES.as_bytes(), 3, &mut |_| 0.0);
    let heights: Vec<bool> = (0..3).map(|p| lot.plane(p, None).unwrap().ground_height).collect();
    assert_eq!(heights, [true, false, true], "a copy past the file's goes on the ground");
}

/// On a slope across its nose the plane's spawn takes the ground's normal for its up, squared as the
/// original squares it, and its body stands on the ground under the bay lifted by the model.
#[test]
fn a_sloped_bay_stands_the_plane_on_the_slopes_normal() {
    let Some(mut disc) = disc() else { return };
    let normal = Vf::new(0.5, 0.8660254, 0.0, 1.0);
    let mut world = Flat { y: F(12.25), normal };
    let f = taken(the_flight(&mut disc), 0, &mut world);
    let (facing, up) = flight::spawn_axes(Vf::new(0.0, 0.0, -1.0, 0.0), normal);
    assert_eq!(xyz(f.spawn.0), xyz(flight::bay_spot(Vf::new(100.0, 12.25, -50.0, 1.0), F(LIFT))));
    assert_eq!((xyz(f.spawn.1), xyz(f.spawn_up)), (xyz(facing), xyz(up)), "the spawn is not squared to the slope");
    assert!(f.spawn_up.x() > 0.49, "the wings are level on a slope: {:?}", f.spawn_up);

    // The same bay on level ground is the level spawn the port had before.
    let mut level = Flat { y: F(12.25), normal: Vf::new(0.0, 1.0, 0.0, 1.0) };
    let f = taken(the_flight(&mut disc), 0, &mut level);
    assert_eq!((f.spawn_up.x(), f.spawn_up.y(), f.spawn_up.z()), (0.0, 1.0, 0.0));
}

/// With `SetGroundHeight` off the bay keeps the file's height, still lifted, and still takes the
/// ground's normal.
#[test]
fn a_bay_without_set_ground_height_keeps_the_files_height() {
    let Some(mut disc) = disc() else { return };
    let normal = Vf::new(0.0, 0.8660254, -0.5, 1.0);
    let mut world = Flat { y: F(12.25), normal };
    let f = taken(the_flight(&mut disc), 1, &mut world);
    assert_eq!(xyz(f.spawn.0), xyz(flight::bay_spot(Vf::new(140.0, 37.5, -50.0, 1.0), F(LIFT))));
    let (_, up) = flight::spawn_axes(Vf::new(1.0, 0.0, 0.0, 0.0), normal);
    assert_eq!(xyz(f.spawn_up), xyz(up));
}

/// The disc's biplane bays are all on flat ground, and none turns `SetGroundHeight` off: on the
/// disc the normal changes nothing but the spawn's last bits.
#[test]
fn the_discs_biplane_bays_are_flat() {
    let Some(mut disc) = disc() else { return };
    let machines = disc.res("machine.res").expect("the machines' archive");
    for track in TRACKS {
        let txt = machines.read_suffix(&format!("tracks\\{track}\\machine.txt")).expect("the track's machine.txt");
        assert_eq!(ground_heights(&txt, Machine::Biplane), [true, true], "{track}");
        let res_bytes = disc.member(&format!("Tracks\\{track}.res")).expect("the track's archive");
        let res = Res::parse(&res_bytes).expect("the track's archive parses");
        let entry = |end: &str| res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(end)).expect(end);
        let scene_bytes = res.read(&res_bytes, entry(".scn")).expect("the scene");
        let scene = Scene::parse(&scene_bytes).expect("the scene parses");
        let terrain_bytes = res.read(&res_bytes, entry(&scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn"))).expect("the terrain");
        let field = Terrain::parse(&terrain_bytes).expect("the terrain parses");
        let mut ground = Ground::new(&field, &scene.terrain);
        for spot in vehicle::spots(&txt, Machine::Biplane) {
            let (_, n) = ground.ground(Vf::new(spot.at.x(), spot.at.y() + 1.0e4, spot.at.z(), 1.0));
            assert_eq!((n.x(), n.y(), n.z()), (0.0, 0.99999994, 0.0), "{track}: a bay at {:?} is on a slope", spot.at);
        }
    }
}
