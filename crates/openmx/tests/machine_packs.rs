//! **MD37c:** a machine pack openmx-mods' `tools/import-atv` built off MX vs. ATV Unleashed's disc
//! loads: every build's chassis and parts, the quad's nodes, the rider's and the quad's clips, the
//! stunt table, the follow cameras, the engine's sounds and the particles, each through the reader
//! openmx already has for that file. It skips without a pack in `mods/machines/`, and says so.
//!
//! **MD80d:** the same for the biplane and Kiowa pack (`import-atv fliers`): each of the eight skins'
//! models, garage models, flier nodes, collision pair and textures, and each flier's `.phy`, cameras,
//! sounds and particles.

#[allow(dead_code)]
#[path = "../src/machine_packs.rs"]
mod machine_packs;
#[allow(unused)]
#[path = "../src/files.rs"]
mod files;
#[allow(dead_code)]
#[path = "../src/track_mods.rs"]
mod track_mods;
#[allow(dead_code)]
#[path = "../src/audio.rs"]
mod audio;
#[allow(unused)]
#[path = "../src/thread.rs"]
mod thread;
#[allow(dead_code)]
#[path = "../src/particles.rs"]
mod particles;

use game_render::quad;
use game_render::skin::Skeleton;
use std::path::PathBuf;

fn mods() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../mods")
}

fn pack() -> Option<machine_packs::Pack> {
    pack_of("atv")
}

/// The pack of one machine (`atv`, MD37c's; `fliers`, MD80d's), or none and a SKIPPED line.
fn pack_of(machine: &str) -> Option<machine_packs::Pack> {
    let (mut packs, why) = machine_packs::find(&mods());
    assert!(why.is_empty(), "a machine pack that does not read: {why:?}");
    match packs.iter().position(|p| p.machine == machine) {
        None => {
            println!(
                "SKIPPED: no {machine} machine pack in {} (openmx-mods' tools/import-atv makes one)",
                mods().join(machine_packs::MACHINES_DIR).display()
            );
            None
        }
        Some(i) => Some(packs.remove(i)),
    }
}

#[test]
fn an_imported_machine_pack_loads() {
    let Some(pack) = pack() else { return };
    assert_eq!(pack.machine, "atv");
    // 96 quad builds on the disc, over three chassis models.
    assert_eq!(pack.builds.len(), 96);
    let mut chassis: Vec<String> = pack.builds.iter().map(|b| b.chassis.to_ascii_lowercase()).collect();
    chassis.sort();
    chassis.dedup();
    assert_eq!(chassis, ["250r_atv_chasis", "atv_s_chasis", "atvs_r_chasis"]);
    for c in &chassis {
        let nodes = pack.quad_nodes(c).unwrap();
        assert!(nodes.missing().is_empty(), "{c} lacks {:?}", nodes.missing());
    }
    for b in &pack.builds {
        let v = pack.vehicle(b).unwrap();
        assert!(!v.parts.is_empty(), "{}: no parts", b.sub);
        for p in &v.parts {
            pack.model(&p.model).unwrap_or_else(|e| panic!("{}: {e}", b.sub));
        }
    }
}

#[test]
fn the_quads_clips_pose_the_quads_nodes() {
    let Some(pack) = pack() else { return };
    let (rider, why) = pack.rider_clips().unwrap();
    assert!(why.is_empty(), "rider clips openmx does not read: {why:?}");
    assert!(rider.len() >= 60, "{} rider clips", rider.len());
    // Each of the quad's own trick clips animates nodes the chassis has.
    let skeleton = Skeleton::of(&pack.model(&pack.builds[0].chassis).unwrap());
    let clips = pack.machine_clips().unwrap();
    assert!(clips.iter().any(|(n, _)| n == "backflip"), "no ATV_Bike_BackFlip among {:?}", clips.iter().map(|(n, _)| n).collect::<Vec<_>>());
    for (name, clip) in &clips {
        let bound = clip.names.iter().filter(|n| skeleton.find(n).is_some()).count();
        assert!(bound > 0, "ATV_Bike_{name}: none of {:?} is a chassis node", clip.names);
    }
    assert!(skeleton.find(quad::WHEELS[0]).is_some());
}

#[test]
fn the_quads_table_cameras_sounds_and_particles_read() {
    let Some(pack) = pack() else { return };
    assert_eq!(pack.stunts().unwrap().len(), 46);
    let cams = pack.cameras("4x3/1p").unwrap();
    assert_eq!(cams.len(), 8);
    // FC_ATVDefault.pb's first camera, the one GAME_REL.BIN loads (FC_ATV.ini's says 13.0 and 43).
    assert_eq!((cams[0].name.as_str(), cams[0].distance, cams[0].angle_x), ("Race Camera 1", 14.5, 42.0));
    assert_eq!(pack.cameras("16x9/1p").unwrap().len(), 8);
    let (sounds, why) = pack.engine_sounds().unwrap();
    assert!(why.is_empty(), "sounds openmx does not read: {why:?}");
    for (list, name) in [(&sounds.idle, "Idle"), (&sounds.high, "HighAccel"), (&sounds.wreck.list, "Wreck")] {
        assert!(!list.is_empty(), "[{name}] read empty");
    }
    let emitters = pack.emitters().unwrap();
    assert!(!emitters.is_empty());
    let lacking: Vec<&str> = emitters.iter().filter(|(_, e)| e.is_none()).map(|(e, _)| e.emitter.as_str()).collect();
    assert!(lacking.is_empty(), "emitters without their .emt: {lacking:?}");
}

/// The fliers' eight skins, as `vehicle::Machine::mxvatv_skins` names them: two per model, model one
/// being MX Unleashed's machine.
const FLIER_SKINS: [(&str, &str, u32, &str); 8] = [
    ("biplane", "biplane", 1, "Biplane"),
    ("biplane", "biplane2", 1, "Biplane2"),
    ("biplane", "biplane3", 2, "Biplane3"),
    ("biplane", "biplane4", 2, "Biplane4"),
    ("kiowa", "kiowa", 1, "Kiowa"),
    ("kiowa", "kiowa2", 1, "Kiowa2"),
    ("kiowa", "kiowa3", 2, "Kiowa3"),
    ("kiowa", "kiowa4", 2, "Kiowa4"),
];

#[test]
fn the_fliers_pack_loads_every_skin() {
    let Some(pack) = pack_of("fliers") else { return };
    assert!(pack.builds.is_empty());
    let got: Vec<(&str, &str, u32)> = pack.skins.iter().map(|k| (k.machine.as_str(), k.skin.as_str(), k.model)).collect();
    let want: Vec<(&str, &str, u32)> = FLIER_SKINS.iter().map(|&(m, k, n, _)| (m, k, n)).collect();
    assert_eq!(got, want);
    for (k, &(_, _, _, col)) in pack.skins.iter().zip(&FLIER_SKINS) {
        assert_eq!((k.hull.as_str(), k.rays.as_str()), (format!("Veh/Col/{col}F.col").as_str(), format!("Veh/Col/{col}M.col").as_str()));
        let (model, ui) = pack.skin_models(k).unwrap();
        assert!(!model.nodes.is_empty() && !ui.nodes.is_empty(), "{}: an empty model", k.skin);
        // Every node openmx turns or seats the pilot on; kiowa3/4 spell three seat dummies `…01`.
        for (w, spelt) in pack.skin_nodes(k).unwrap() {
            let spelt = spelt.unwrap_or_else(|| panic!("{} lacks {w}", k.skin));
            let odd = k.skin == "kiowa3" || k.skin == "kiowa4";
            assert!(spelt == w || (odd && spelt == format!("{w}01")), "{}: {w} spelt {spelt}", k.skin);
        }
        let (hull, rays) = pack.skin_collision(k).unwrap();
        assert!(!hull.triangles.is_empty() && !rays.segments.is_empty(), "{}: an empty collision pair", k.skin);
        let (textures, why) = pack.skin_textures(k).unwrap();
        assert!(why.is_empty(), "{}: {why:?}", k.skin);
        assert!(textures >= 2, "{}: {textures} textures", k.skin);
    }
}

#[test]
fn the_fliers_phy_cameras_sounds_and_particles_read() {
    let Some(pack) = pack_of("fliers") else { return };
    let names: Vec<&str> = pack.fliers.iter().map(|f| f.machine.as_str()).collect();
    assert_eq!(names, ["biplane", "kiowa"]);
    for f in &pack.fliers {
        // Both models' keys, the same keys for each.
        let (_, [one, two]) = pack.flier_phy(f).unwrap();
        assert!(one > 0 && one == two, "{}: {one} ModelOne keys, {two} ModelTwo", f.phy);
        let (follow, replay) = pack.flier_cameras(f, "4x3/1p").unwrap();
        assert!(!follow.is_empty() && !replay.is_empty(), "{}: no cameras", f.machine);
        let (sounds, why) = pack.flier_sounds(f).unwrap();
        assert!(why.is_empty(), "{}: sounds openmx does not read: {why:?}", f.machine);
        assert!(!sounds.wreck.list.is_empty(), "[{}Wreck] read empty", f.sounds);
        match f.machine.as_str() {
            // carengine.aud's [biplane]: seven keyed loops; machine.aud's one touchdown.
            "biplane" => {
                assert_eq!(sounds.loops.loops.len(), 7);
                assert!(!sounds.touchdown.list.is_empty());
            }
            _ => assert!(sounds.rotor.clip.is_some(), "otherengine.aud's [Helicopter] EngineLoop"),
        }
        let emitters = pack.flier_emitters(f).unwrap();
        assert!(!emitters.is_empty());
        let lacking: Vec<&str> = emitters.iter().filter(|(_, e)| e.is_none()).map(|(e, _)| e.emitter.as_str()).collect();
        assert!(lacking.is_empty(), "{}: emitters without their .emt: {lacking:?}", f.particles);
    }
}
