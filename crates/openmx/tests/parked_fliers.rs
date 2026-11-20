//! F80: the fliers the original parks in play, free06's helicopter and free10's biplane, settled by
//! the shell with nobody aboard ([`vehicle::parked_flier`]) against R52's RAM images
//! (`context/states/r52-free06-heli.ee.bin`, `r52-free10-biplane.ee.bin`; research/r52/index.md):
//! the spot each is stored at, and where its body has gone 300 frames in. game-sim's
//! `tests/parked_fliers.rs` proves the images' hulls are [`game_sim::ai::parked_hull`] of their
//! bodies. Skipped with a line where the disc or the images are not there.
//!
//! `src/vehicle.rs` is compiled in by path, the way `tests/machines.rs` takes it.

#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;

use game_render::Disc;
use game_sim::archive::Res;
use game_sim::camera::CurveTable;
use game_sim::eco::{Trees, Wooded};
use game_sim::track::{Scene, Terrain};
use game_sim::world::{Ground, Place, Props};
use ps2_float::Vf;
use ps2_oracle::{Ee, Kit, skip};
use vehicle::{Machine, Spawn};

/// A flier one of R52's images parks at `[Machine1]`: the image, the track's archives, the machine,
/// its machine object in RAM (`+0x50 + 0x90` its stored spot, `+0x160` its body), and how far off
/// the image's the port's stored spot and settled body may stand.
struct Flier {
    state: &'static str,
    track: &'static str,
    archives: [&'static str; 2],
    machine: Machine,
    at: u32,
    spot_within: f32,
    body_within: f32,
}

const FLIERS: [Flier; 2] = [
    // The helicopter's lift reads 5.88528 in the image, its model's box 5.885306 (flier_spot).
    Flier { state: "r52-free06-heli.ee.bin", track: "free06", archives: ["Tracks\\free06.res", "Tracks\\free06_2.res"], machine: Machine::Helicopter, at: 0x005f_f210, spot_within: 0.0001, body_within: 0.25 },
    Flier { state: "r52-free10-biplane.ee.bin", track: "free10", archives: ["Tracks\\free10.res", "Tracks\\free10_2.res"], machine: Machine::Biplane, at: 0x005d_beb0, spot_within: 0.0, body_within: 0.25 },
];

fn quad(ee: &Ee, at: u32) -> Vf {
    Vf::from_bits([0, 4, 8, 12].map(|i| ee.u32_at(at + i)))
}

fn far(a: Vf, b: Vf) -> f32 {
    [a.x() - b.x(), a.y() - b.y(), a.z() - b.z()].iter().fold(0.0f32, |m, d| m.max(d.abs()))
}

/// `then` on the track out of `names`' archives as a parked machine meets it: the terrain, the props
/// and the trees.
fn in_track<R>(disc: &mut Disc, names: &[&str], then: impl FnOnce(&mut Wooded<Place<'_>>) -> R) -> R {
    let parts: Vec<(Vec<u8>, Res)> = names.iter().filter_map(|n| disc.member(n).ok()).filter_map(|b| Res::parse(&b).ok().map(|r| (b, r))).collect();
    assert!(!parts.is_empty(), "no {}", names[0]);
    let by_end = |end: &str| {
        let want = end.to_ascii_lowercase();
        parts.iter().find_map(|(b, r)| {
            let e = r.entries.iter().find(|e| {
                let n = e.name.to_ascii_lowercase();
                n == want || n.ends_with(&format!("\\{want}"))
            })?;
            r.read(b, e).ok()
        })
    };
    let (bytes, res) = &parts[0];
    let member = |end: &str| res.read(bytes, res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(end)).expect(end)).expect(end);
    let scene = Scene::parse(&member(".scn")).expect("the scene");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let terrain_bytes = by_end(leaf.rsplit('\\').next().unwrap()).expect("the terrain");
    let terrain = Terrain::parse(&terrain_bytes).expect("the terrain parses");
    let (props, _) = Props::load(&scene, |want| by_end(want).ok_or_else(|| format!("no {want}")));
    let ground = Ground::new(&terrain, &scene.terrain);
    let (trees, _) = Trees::on_ground(&member(".esb"), &ground, &CurveTable::build()).expect("the ecosystem");
    let mut place = Wooded { world: Place { ground, props }, trees };
    then(&mut place)
}

/// **The shell's parked fliers stand where the original's do.** Each flier's stored spot
/// ([`vehicle::flier_spot`]: the ground or the file's height under `[Machine1]`, lifted by its
/// model) is the image's, and settled with nobody aboard its body stands near the image's.
#[test]
fn the_parked_fliers_settle_where_the_originals_stand() {
    let Some(kit) = Kit::load().map_err(|w| skip(&w)).ok() else { return };
    let Some(mut disc) = Disc::open_configured().map_err(|w| skip(&w)).ok() else { return };
    let machines = disc.res("machine.res").expect("machine.res");
    let tables = game_sim::ride::Tables::build();
    let mut seen = 0;
    for f in &FLIERS {
        let path = kit.ctx("states").join(f.state);
        if !path.exists() {
            skip(&format!("{} is not there: research/r52/boot.py makes it", path.display()));
            continue;
        }
        let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
        let txt = machines.read_suffix(&format!("{}\\machine.txt", f.track)).expect("machine.txt");
        let (slot, machine, spot): (usize, Machine, Spawn) = vehicle::in_play(&txt)[0];
        assert_eq!((slot, machine), (0, f.machine), "{}: [Machine1] is the flier", f.track);
        let ground_height = vehicle::in_play_ground_heights(&txt)[0];
        let stored = quad(&ee, f.at + 0x50 + 0x90);
        let b = ee.u32_at(f.at + 0x160);
        let want = quad(&ee, b + 0x60);
        let read = &mut |n: &str| machines.read_suffix(n).ok();
        in_track(&mut disc, &f.archives, |world| {
            let model = match f.machine {
                Machine::Helicopter => game_sim::kiowa::KIOWA_MODEL,
                _ => game_sim::flight::BIPLANE_MODEL,
            };
            let lift = game_sim::flight::spawn_lift(&read(model).expect("the model")).expect("its lift");
            let (at, _) = vehicle::flier_spot(world, spot.at, ground_height, lift).expect("ground under the spot");
            println!("{}: stored {:?}, port {:?}", f.track, [stored.x(), stored.y(), stored.z()], [at.x(), at.y(), at.z()]);
            assert!(far(at, stored) <= f.spot_within, "{}: stored at {at:?}, the original {stored:?}", f.track);
            let (body, hull) = vehicle::parked_flier(f.machine, spot, ground_height, read, world, &tables).expect("settled");
            let got = body.position;
            println!("{}: image body {:?}, port body {:?}", f.track, [want.x(), want.y(), want.z()], [got.x(), got.y(), got.z()]);
            assert!(far(got, want) <= f.body_within, "{}: settled at {got:?}, the original {want:?}", f.track);
            assert_eq!(hull.to_world, game_sim::ai::parked_hull(hull.mesh.clone(), &body).to_world, "{}: the hull is where the body stands", f.track);
        });
        seen += 1;
    }
    if seen == 0 {
        skip("no R52 image");
    }
}
