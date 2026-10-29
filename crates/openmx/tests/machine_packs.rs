//! **MD37c:** a machine pack openmx-mods' `tools/import-atv` built off MX vs. ATV Unleashed's disc
//! loads: every build's chassis and parts, the quad's nodes, the rider's and the quad's clips, the
//! stunt table, the follow cameras, the engine's sounds and the particles, each through the reader
//! openmx already has for that file. It skips without a pack in `mods/machines/`, and says so.

#[allow(dead_code)]
#[path = "../src/machine_packs.rs"]
mod machine_packs;
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
    let (mut packs, why) = machine_packs::find(&mods());
    assert!(why.is_empty(), "a machine pack that does not read: {why:?}");
    match packs.is_empty() {
        true => {
            println!(
                "SKIPPED: no machine pack in {} (openmx-mods' tools/import-atv makes one)",
                mods().join(machine_packs::MACHINES_DIR).display()
            );
            None
        }
        false => Some(packs.remove(0)),
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
