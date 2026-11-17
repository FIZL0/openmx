//! MD138a — the quad's own clips and the rider's stunt clips, ticked with the shell's quad
//! ([`vehicle::AtvRide::step`] and its [`game_render::quad::Driver`]): a backflip off a ramp
//! starts the rider's `ATV_rider_backflip`, and the chassis plays what the original's quad plays
//! for it. That is not `ATV_Bike_BackFlip`. MX vs. ATV's stunt table marks the backflip
//! `UsedForBike=F`, so the original binds no quad clip for it, and its animation driver plays the riding
//! pose's middle clip (`ATV_Bike_Air_Mid`) at a half (MD138a's journal).
//!
//! The ramp is a world of the test's own: free09 has no lip near the spawn that a hand-written tape
//! could be sure of. Skips without openmx-mods' `mxvatv_atv` pack in this checkout's
//! `mods/machines/`.
//!
//! `src/vehicle.rs` is compiled in by path, the way `tests/atv_modifiers.rs` takes it.

#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;

use game_render::quad::{Played, Stunt};
use game_sim::markers::Spot;
use game_sim::ride::Controls;
use game_sim::collide::{Bundle, Record};
use game_sim::world::World;
use ps2_float::{F, Vf};
use std::path::{Path, PathBuf};

const DT: f32 = 1.0 / 30.0;
const PHY: &str = "Veh/atv_Race250P.phy";

fn pack() -> Option<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/machines/mxvatv_atv");
    if !dir.join("mod.json").is_file() {
        println!("SKIPPED: no mods/machines/mxvatv_atv (openmx-mods' import-atv makes it)");
        return None;
    }
    Some(dir)
}

/// The quad's clip table as `quad_draw::stunts` lays it, read here off the pack's files: each of the
/// first 46 rows, and the quad's own clip's length for a row `UsedForBike` that the pack has a clip for.
fn table(files: &game_render::disc::Res) -> Vec<Stunt> {
    let rows = game_sim::trick_pack::table(&files.read("Rider\\Anim\\ATV_Rider_Stunts.ini").expect("the stunt table")).expect("it reads");
    rows.iter()
        .take(game_sim::quad::RIDING_POSE as usize)
        .map(|r| {
            let clip = files.read_suffix(&format!("ATV_Bike_{}.mot", r.stunt.name)).ok().and_then(|b| game_render::skin::Clip::parse(&b).ok());
            Stunt { name: r.stunt.name.clone(), duration: r.stunt.duration.0, machine: clip.filter(|_| r.stunt.for_bike).map(|c| c.duration()) }
        })
        .collect()
}

/// A run-up along z, a ramp rising `RISE` over `RUN` from z = `FOOT`, and a drop back to the floor
/// at its lip.
struct Ramp;

const FOOT: f32 = 150.0;
const RUN: f32 = 30.0;
const RISE: f32 = 16.0;

impl World for Ramp {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        let up = Vf::new(0.0, 1.0, 0.0, 1.0);
        let along = at.z() - FOOT;
        if (0.0..RUN).contains(&along) {
            let n = Vf::new(0.0, RUN, -RISE, 0.0);
            let l = (RUN * RUN + RISE * RISE).sqrt();
            (Vf::new(at.x(), along * RISE / RUN, at.z(), 1.0), Vf::new(n.x() / l, n.y() / l, n.z() / l, 1.0))
        } else {
            (Vf::new(at.x(), 0.0, at.z(), 1.0), up)
        }
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

#[test]
fn a_quad_backflip_plays_the_riders_backflip_and_the_originals_air_pose() {
    let Some(dir) = pack() else { return };
    let files = game_render::disc::Res::folder(&dir).expect("the pack's folder reads");
    let mut atv = vehicle::AtvRide::read("mxvatv_atv", PHY, &mut |n| files.read(&n.replace('/', "\\")).ok()).expect("the quad builds");
    atv.stunts = table(&files);
    let backflip = atv.stunts.iter().position(|s| s.name.eq_ignore_ascii_case("backflip")).expect("a backflip row") as i32;
    assert_eq!(atv.stunts[backflip as usize].machine, None, "the original binds no quad clip for the backflip");
    assert!(atv.stunts.iter().any(|s| s.machine.is_some()), "the quad has clips of its own for some stunts");
    assert!(files.read_suffix("ATV_rider_backflip.mot").or_else(|_| files.read_suffix("ATV_backflip.mot")).is_ok(), "the rider's backflip clip");

    let mut world = Ramp;
    let spot = Spot::still(Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0));
    atv.take(&spot, &mut world).expect("the floor is under it");
    let r1 = [false, false, true];
    let (mut left, mut flipped, mut on_ground_frames, mut armed) = (None, Vec::new(), 0, false);
    for t in 0..400 {
        let q = atv.quad().unwrap();
        let (z, airborne) = (q.body.position.z(), !q.q.on_ground);
        armed |= q.bike.air.flip_armed;
        let c = if z < FOOT {
            // The run-up (the quad settles on the floor from the spot first).
            Controls { gas: F::ONE, ..Controls::default() }
        } else if !airborne && left.is_none() {
            // On the ramp: R1 alone with the stick back arms the flip, then the stick forward fires
            // it, both on the lip.
            let y = if armed { 1.0 } else { -1.0 };
            Controls { gas: F::ONE, stick: (F::ZERO, F(y)), trick: r1, ..Controls::default() }
        } else {
            // In the air: R1 held keeps the flip's torque.
            left.get_or_insert(t);
            Controls { trick: r1, ..Controls::default() }
        };
        atv.step(&c, F(DT), &mut world);
        let q = atv.ridden.as_ref().unwrap();
        let d = q.clips.as_ref().expect("the quad's clips tick");
        if std::env::var_os("ATV_CLIPS_TRACE").is_some() {
            println!(
                "{t} sy {} z {:.1} y {:.1} air {} flip {}/{} vy {:.1} cz {:.1} b {} down {} stunt {} rider {:?} played {:?} rest {:?}",
                q.ride.bike.stick.y,
                q.ride.body.position.z(),
                q.ride.body.position.y(),
                !q.ride.q.on_ground,
                q.ride.bike.air.flip_armed,
                q.ride.bike.air.flip_active,
                q.ride.body.velocity.y(),
                q.ride.q.contact_vel.z(),
                q.ride.bike.buttons,
                q.ride.down(),
                d.current,
                d.rider,
                d.played,
                q.ride.rest.iter().filter(|c| matches!(c, game_sim::quad::Call::StartStunt(_) | game_sim::quad::Call::EndStunt)).collect::<Vec<_>>()
            );
        }
        if d.current == backflip {
            flipped.push((t, d.rider, d.played));
        }
        if left.is_some() && !flipped.is_empty() && q.ride.q.on_ground {
            on_ground_frames += 1;
            if on_ground_frames > 30 {
                break;
            }
        }
    }
    assert!(left.is_some(), "the quad never left the ramp");
    assert!(!flipped.is_empty(), "no backflip started");
    println!("the backflip: {} frames, from frame {}", flipped.len(), flipped[0].0);
    // The rider's clip plays from the start until the flip's own end call (the clip came round), which
    // stops it and leaves the stunt word on while the flip is active, as the original's end call does.
    let played_to = flipped.iter().position(|(_, rider, _)| rider.is_none()).unwrap_or(flipped.len());
    assert!(played_to > 0, "the rider's clip never played");
    for (t, rider, played) in &flipped {
        let want = if flipped[..played_to].iter().any(|f| f.0 == *t) { Some(backflip) } else { None };
        assert_eq!(rider.map(|r| r.0), want, "frame {t}: the rider plays its backflip clip until it ends");
        let Some(Played::One { clip, weight, .. }) = *played else { panic!("frame {t}: the chassis plays {played:?}") };
        assert_eq!((clip, weight), (game_sim::quad::POSE_CLIPS[1], 0.5), "frame {t}: the chassis plays the air pose's middle at a half");
    }
}

/// V130: the Untamed quad off the same ramp, on Untamed's step (`untamed_atv`'s own `.phy.bpb`), with
/// no trick: in the air its chassis blends the riding pose's clips (Untamed's riding pose is id 54,
/// its animate's bound; the driver keeps it as the table's row 46), as the Unleashed quad's does. The
/// stunt table is `mxvatv_atv`'s: Untamed's has the same 46 rows (MD144b). Skips without both packs.
#[test]
fn an_untamed_quad_in_the_air_blends_the_riding_pose() {
    let Some(dir) = pack() else { return };
    let untamed = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/machines/untamed_atv");
    if !untamed.join("mod.json").is_file() {
        println!("SKIPPED: no mods/machines/untamed_atv (openmx-mods' import-atv untamed-atv makes it)");
        return;
    }
    let files = game_render::disc::Res::folder(&dir).expect("the pack's folder reads");
    let ufiles = game_render::disc::Res::folder(&untamed).expect("the Untamed pack's folder reads");
    let atv = vehicle::AtvRide::read("untamed_atv", "Veh/atv_Race250P.phy.bpb", &mut |n| ufiles.read(&n.replace('/', "\\")).ok()).expect("the quad builds");
    let mut atv = atv.on_untamed_step();
    atv.stunts = table(&files);

    let mut world = Ramp;
    let spot = Spot::still(Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0));
    atv.take(&spot, &mut world).expect("the floor is under it");
    let (mut left, mut aired, mut landed) = (false, 0, 0);
    for t in 0..400 {
        atv.step(&Controls { gas: F::ONE, ..Controls::default() }, F(DT), &mut world);
        let q = atv.ridden.as_ref().unwrap();
        assert!(q.ride.bike.untamed(), "ridden on Untamed's step");
        let d = q.clips.as_ref().expect("the quad's clips tick");
        assert_eq!(d.shift, game_sim::quad::UNTAMED_STUNT_SHIFT, "frame {t}: the driver reads Untamed's ids");
        if q.ride.q.on_ground || q.ride.down() {
            if left {
                landed += 1;
                if landed > 10 {
                    break;
                }
            }
            continue;
        }
        if q.ride.body.position.z() < FOOT {
            continue;
        }
        left = true;
        aired += 1;
        assert_eq!(d.current, game_sim::quad::RIDING_POSE, "frame {t}: in the air on no stunt, the riding pose");
        let Some(Played::Two { clips, .. }) = d.played else { panic!("frame {t}: the chassis plays {:?}", d.played) };
        let p = game_sim::quad::POSE_CLIPS;
        assert!(clips == [p[1], p[0]] || clips == [p[2], p[1]], "frame {t}: the riding pose's clips, not {clips:?}");
    }
    assert!(aired > 3, "the quad was hardly in the air ({aired} frames)");
    println!("in the air {aired} frames, all on the riding pose");
}
