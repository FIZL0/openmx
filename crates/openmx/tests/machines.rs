//! One of each machine per player (P87), on the disc's own tracks.
//!
//! The original parks two of every machine on a freeride track, one per player of its two, in its
//! `machine.res`'s `tracks\<track>\machine.txt`; the port parks as many as there are players — the
//! file's two where it says, the rest beside them — and every second copy wears the disc's alternate
//! livery. This test reads every freeride track's file and terrain off the user's disc and, for 1, 2,
//! 4 and 8 players, asserts that every machine is parked that many times, the first two exactly where
//! the file says and the rest nearby, clear of each other and standing as high over the dirt as the
//! file's own spot does; that each player takes their own plane and no two take one; and that the
//! second copy's model is the alternate livery's, with that livery's textures.
//!
//! `src/vehicle.rs` is compiled in by path, the way `tests/ride.rs` takes it.

#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;

use game_render::Disc;
use game_render::biplane::Biplane;
use game_render::world::Assets;
use game_sim::archive::Res;
use game_sim::track::{Scene, Terrain};
use game_sim::world::{Ground, World};
use ps2_float::Vf;
use vehicle::{Bay, Garage, Kind, Lot, Machine, Spawn, spots};

/// The freeride tracks whose `machine.txt` the machines' archive carries.
const TRACKS: [&str; 5] = ["free06", "free07", "free09", "free10", "free11"];

fn disc() -> Option<Disc> {
    match Disc::open_configured() {
        Ok(d) => Some(d),
        Err(why) => {
            eprintln!("skipped: {why}");
            None
        }
    }
}

/// A track's scene and terrain bytes, out of its own archive.
fn terrain(disc: &mut Disc, track: &str) -> (Vec<u8>, Scene) {
    let res_bytes = disc.member(&format!("Tracks\\{track}.res")).expect("the track's archive");
    let res = Res::parse(&res_bytes).expect("the track's archive parses");
    let scene_entry = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(".scn")).expect("a scene");
    let scene = Scene::parse(&res.read(&res_bytes, scene_entry).expect("the scene")).expect("the scene parses");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let te = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(&leaf)).expect("the terrain");
    (res.read(&res_bytes, te).expect("the terrain bytes"), scene)
}

fn apart(a: &Bay, b: &Bay) -> f32 {
    (a.spawn.at.x() - b.spawn.at.x()).hypot(a.spawn.at.z() - b.spawn.at.z())
}

#[test]
fn every_player_has_one_of_each_machine() {
    let Some(mut disc) = disc() else { return };
    let machines = disc.res("machine.res").expect("the machines' archive");
    // Every machine's four models are on the disc, in its own directory: the factory's skins.
    for m in Machine::ALL {
        for model in m.models() {
            let name = format!("{}\\anim\\{model}.psg", m.dir());
            assert!(machines.find_suffix(&name).is_some(), "machine.res has no {name}");
        }
    }
    let mut checked = 0;
    for track in TRACKS {
        let txt = machines.read_suffix(&format!("tracks\\{track}\\machine.txt")).expect("the track's machine.txt");
        let (bytes, scene) = terrain(&mut disc, track);
        let field = Terrain::parse(&bytes).expect("the track's terrain");
        let mut probe = Ground::new(&field, &scene.terrain);
        let mut ground = |at: Vf| probe.ground(Vf::new(at.x(), 1.0e4, at.z(), 1.0)).0.y();
        for m in Machine::ALL {
            let file = spots(&txt, m);
            assert_eq!(file.len(), 2, "{track} parks two {m:?} for the original's two players");
            for (i, s) in file.iter().enumerate() {
                assert_eq!(Spawn::machine(&txt, m, i), Some(*s));
            }
        }
        for players in [1, 2, 4, 8] {
            let lot = Lot::new(&txt, players, &mut ground);
            for m in Machine::ALL {
                let file = spots(&txt, m);
                let bays: Vec<Bay> = lot.of(m).copied().collect();
                assert_eq!(bays.len(), players, "{track}, {players} players: {m:?}");
                for (i, b) in bays.iter().enumerate() {
                    assert_eq!((b.copy, b.livery), (i, i % 2), "{track} {m:?} {i}");
                    if i < 2 {
                        assert!(b.original);
                        assert_eq!(b.spawn, file[i], "{track} {m:?} {i} stands on the file's spot");
                        continue;
                    }
                    assert!(!b.original);
                    let base = &file[i % 2];
                    let from = (b.spawn.at.x() - base.at.x()).hypot(b.spawn.at.z() - base.at.z());
                    assert!(from <= 4.0 * m.spacing(), "{track} {m:?} {i} is {from} ft from its spot");
                    let (clear, now) = (base.at.y() - ground(base.at), b.spawn.at.y() - ground(b.spawn.at));
                    assert!((clear - now).abs() < 1e-2, "{track} {m:?} {i} stands {now} ft up, the file's {clear}");
                    for c in &bays[..i] {
                        assert!(apart(b, c) >= m.spacing() - 0.01, "{track} {m:?} {i} is {} ft from {}", apart(b, c), c.copy);
                    }
                }
            }
            // Each player takes their own plane, and no two take one.
            let mut garages: Vec<Garage> = (0..players).map(|p| Garage::new(Kind::Biplane, lot.plane(p, None)).unwrap()).collect();
            for (p, g) in garages.iter_mut().enumerate() {
                let plane = g.plane.as_ref().unwrap();
                assert_eq!(plane.at, lot.bay(Machine::Biplane, p).unwrap().spawn.at);
                assert_eq!(plane.livery, p % 2);
                assert_eq!(g.swap(), Ok(Kind::Bike));
                assert_eq!(g.swap(), Ok(Kind::Biplane));
            }
            for (p, g) in garages.iter().enumerate() {
                for h in &garages[..p] {
                    assert_ne!(g.plane.as_ref().unwrap().at, h.plane.as_ref().unwrap().at, "{track}: two players in one plane");
                }
            }
            checked += 1;
        }
    }
    assert_eq!(checked, TRACKS.len() * 4);

    // The second copy wears the alternate livery: its own model, on its own body and wing textures.
    let archives = [machines];
    let mut assets = Assets::default();
    let first = Biplane::load(&mut assets, &archives, vehicle::livery(0)).expect("biplane");
    let second = Biplane::load(&mut assets, &archives, vehicle::livery(1)).expect("biplane2");
    let third = Biplane::load(&mut assets, &archives, vehicle::livery(2)).expect("biplane, again");
    assert_eq!((first.model.as_str(), second.model.as_str(), third.model.as_str()), ("biplane", "biplane2", "biplane"));
    let textures = |model: &str| -> Vec<String> {
        let mut t: Vec<String> = assets.models[model].model.materials.iter().map(|m| m.to_ascii_lowercase()).collect();
        t.sort();
        t
    };
    let (a, b) = (textures("biplane"), textures("biplane2"));
    assert!(a.iter().any(|t| t.contains("pitts_body01")), "biplane's materials: {a:?}");
    assert!(b.iter().any(|t| t.contains("pitts_body02")), "biplane2's materials: {b:?}");
    assert_ne!(a, b, "the two liveries are drawn alike");
}
