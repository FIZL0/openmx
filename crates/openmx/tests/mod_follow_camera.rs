//! MD82d: the shell follows each mod machine on its own `FC_` file with SLUS-21104's follow camera.
//!
//! The shell keeps every follow camera as MX Unleashed's `Race`, built its way (the fog's ceiling, its
//! speed reference, the reverse threshold, a snap), and steps a mod machine's through
//! `chase::mod_follow`. These tests hold that path to `game_sim::camera::ModFollow` as MD82b and MD82c
//! replayed it against SLUS-21104's captures: over generated frames — on the ground, airborne fast and
//! slow, reversing, straight up — the shell's camera and a `ModFollow` built on the same presets give
//! the same view and the same camera, bit for bit, and the shell's own ceiling and speed reference
//! survive the frame. The presets are the `mxvatv_atv` pack's `Cam\FC_ATVDefault.pb`; the test skips
//! without the pack.

#[allow(dead_code)]
#[path = "../src/vehicle.rs"]
mod vehicle;
#[allow(dead_code)]
#[path = "../src/chase.rs"]
mod chase;

use std::path::PathBuf;

use game_sim::camera::{Aspect, CurveTable, Follow, ModFollow, Preset, Race, Ridden, Surface, Switches, Target, View, Viewport};
use ps2_float::{F, Vf};

use vehicle::Kind;

/// The shell's constants for its follow cameras (`main.rs`).
const REVERSE_SPEED: f32 = 5.0;
const SPEED_REFERENCE: f32 = 117.332_8;
const NEAR_CLIP: f32 = 2.4;
/// The race's ceiling on a scene fogged past 1500 ft, which the shell gives a ground machine's camera.
const FOG_CEILING: f32 = 200.0;
const DT: f32 = 0.033_333_335;

fn pack_camera() -> Option<Vec<u8>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../mods/machines/mxvatv_atv/Cam/FC_ATVDefault.pb");
    std::fs::read(root).ok()
}

/// Rolling ground, so the probe, the lift and the eye's floor all have work to do.
struct Hills;

impl Surface for Hills {
    fn on_surface(&mut self, p: Vf) -> Vf {
        let h = 6.0 * (p.x() * 0.031).sin() + 4.0 * (p.z() * 0.047).cos();
        Vf::new(p.x(), h, p.z(), p.w())
    }
}

/// A small generator, so the frames are the same every run.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f32) / ((1u64 << 24) as f32)
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next()
    }
}

/// Frame `i`'s target block: a machine driving and turning over the hills, with stretches in the
/// air, some fast, some slow, some straight up, and some backing up.
fn rider(i: usize, g: &mut Lcg, point: &mut Vf, heading: &mut f32) -> Ridden {
    *heading += g.range(-0.08, 0.08);
    let (s, c) = heading.sin_cos();
    let forward = Vf::new(s, g.range(-0.3, 0.3), c, 1.0);
    let phase = (i / 90) % 5;
    let speed = match phase {
        0 => g.range(0.0, 3.0),
        1 => g.range(15.0, 70.0),
        _ => g.range(25.0, 90.0),
    };
    let sign = if phase == 4 && i % 90 < 30 { -1.0 } else { 1.0 };
    let climb = if phase == 3 && i % 90 < 10 { speed } else { g.range(-12.0, 12.0) };
    let vel = Vf::new(s * speed * sign, climb, c * speed * sign, 1.0);
    *point = Vf::new(point.x() + vel.x() * DT, 0.0, point.z() + vel.z() * DT, 1.0);
    let ground = Hills.on_surface(*point).y();
    let airborne = phase >= 2 && i % 90 < 45;
    *point = Vf::new(point.x(), ground + 2.5 + if airborne { g.range(0.0, 20.0) } else { 0.0 }, point.z(), 1.0);
    let len = (vel.x() * vel.x() + vel.y() * vel.y() + vel.z() * vel.z()).sqrt();
    Ridden {
        target: Target { body_point: *point, forward, up: Vf::new(0.0, 1.0, 0.0, 1.0), vel, lean: F(g.range(-0.5, 0.5)) },
        airborne,
        speed: F(len),
    }
}

/// Every field of the two cameras that differs, by bits.
fn differences(a: &Follow, b: &Follow, va: &View, vb: &View) -> Vec<String> {
    let mut bad = Vec::new();
    let mut check = |what: &str, x: f32, y: f32| {
        if x.to_bits() != y.to_bits() {
            bad.push(format!("{what}: shell {x:?}, ModFollow {y:?}"));
        }
    };
    for k in 0..3 {
        check("eye", va.position[k], vb.position[k]);
        check("look-at", va.look_at[k], vb.look_at[k]);
        check("look lag", a.look_filter[k].value.0, b.look_filter[k].value.0);
        check("look tau", a.look_filter[k].tau.0, b.look_filter[k].tau.0);
        check("position lag", a.pos_filter[k].value.0, b.pos_filter[k].value.0);
        check("position tau", a.pos_filter[k].tau.0, b.pos_filter[k].tau.0);
    }
    check("fov", va.fov.0, vb.fov.0);
    check("distance", a.distance.0, b.distance.0);
    check("yaw", a.yaw.0, b.yaw.0);
    check("pitch", a.pitch.0, b.pitch.0);
    check("lift", a.ground_lift.0, b.ground_lift.0);
    if a.pitch_down_locked != b.pitch_down_locked || a.snap != b.snap {
        bad.push("lock or snap".into());
    }
    bad
}

#[test]
fn only_mod_machines_on_their_own_file_follow_on_slus_21104s_camera() {
    for up in [Kind::Atv, Kind::MxvatvCar, Kind::Biplane, Kind::Kiowa] {
        assert!(chase::mod_follows(up, true), "{up:?} on its own file");
        assert!(!chase::mod_follows(up, false), "{up:?} on the bike's or the disc's presets");
    }
    for up in [Kind::Bike, Kind::Buggy, Kind::Monster, Kind::Baja] {
        assert!(!chase::mod_follows(up, true), "MX Unleashed's {up:?}");
    }
}

#[test]
fn each_mxvatv_car_reads_its_kinds_file() {
    use game_sim::car::MxvatvMachine as M;
    let files: Vec<&str> = M::ALL.iter().map(|&m| chase::mod_presets(chase::mxvatv_car_machine(m))).collect();
    assert_eq!(
        files,
        ["Cam\\FC_BuggyDefault.pb", "Cam\\FC_MonsterDefault.pb", "Cam\\FC_BajaDefault.pb", "Cam\\FC_GolfCartDefault.pb", "Cam\\FC_SandrailDefault.pb"]
    );
}

#[test]
fn the_shells_mod_camera_steps_as_modfollow_bit_for_bit() {
    let Some(bytes) = pack_camera() else {
        eprintln!("SKIPPED: no mods/machines/mxvatv_atv/Cam/FC_ATVDefault.pb (openmx-mods' import-atv makes it)");
        return;
    };
    let presets: Vec<Preset> = chase::flier_presets(&bytes, Aspect::Standard, Viewport::Full);
    assert!(!presets.is_empty(), "FC_ATVDefault.pb holds 4x3/1p presets");
    let curve = CurveTable::build();
    let mut kinds = 0;
    for preset in 0..presets.len() {
        // The shell's camera, built as main.rs builds a car's.
        let mut shell = Race::new(presets.clone(), F(REVERSE_SPEED));
        shell.cam.ceiling = F(FOG_CEILING);
        shell.cam.speed_reference = F(SPEED_REFERENCE);
        shell.cam.set_preset(preset);
        shell.cam.snap = true;
        let mut plain = Race { cam: shell.cam.clone() };
        let mut want = ModFollow::new(presets.clone());
        want.race.cam.set_preset(preset);
        want.race.cam.snap = true;
        let mut g = Lcg(0x5eed + preset as u64);
        let (mut point, mut heading) = (Vf::new(0.0, 0.0, 0.0, 1.0), 0.0);
        let mut apart = 0;
        for i in 0..2000 {
            let r = rider(i, &mut g, &mut point, &mut heading);
            // Behind the machine (yaw a half turn, every preset) the rear fades are all at their ends
            // and the two games' cameras step alike (MD82c); the right stick (P114) swings the yaw
            // round to a radian off straight ahead, where the fades bite.
            if i == 1000 {
                let swing = F(shell.cam.preset().angle_y.0 - 1.0);
                (shell.cam.user_yaw, want.race.cam.user_yaw) = (swing, swing);
            }
            let switches = Switches { no_look_ahead: false, short_viewport: i % 400 >= 350 };
            let got = chase::mod_follow(&mut shell, F(DT), &r, F(NEAR_CLIP), &curve, switches, &mut Hills);
            let wanted = want.update(F(DT), &r, F(NEAR_CLIP), &curve, switches, &mut Hills);
            let bad = differences(&shell.cam, &want.race.cam, &got, &wanted);
            assert!(bad.is_empty(), "preset {preset} frame {i}:\n  {}", bad.join("\n  "));
            assert_eq!(shell.cam.ceiling.0.to_bits(), FOG_CEILING.to_bits(), "the shell's ceiling is put back");
            assert_eq!(shell.cam.speed_reference.0.to_bits(), SPEED_REFERENCE.to_bits());
            let mx = plain.update(F(DT), &r, F(NEAR_CLIP), &curve, switches, &mut Hills);
            apart += (mx.position != got.position) as usize;
            plain = Race { cam: shell.cam.clone() };
        }
        // MX Unleashed's step on the same camera goes elsewhere, or the test proves nothing — except
        // on a look-back preset, where both games drop the look-ahead and this file's has no lags, so
        // none of the fades has anything to scale.
        assert!(presets[preset].is_look_back_cam || apart > 50, "preset {preset}: MX Unleashed's race camera differed from SLUS-21104's on only {apart} frames");
        kinds += 1;
    }
    eprintln!("{kinds} presets of FC_ATVDefault.pb, 2000 frames each: the shell's mod camera is ModFollow's to the bit");
}
