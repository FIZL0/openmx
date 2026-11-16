//! MD162 — MX vs. ATV's quad's engine note on its own sound update. The original's quad note is the
//! bike's class (the same `vehicle.aud` lists, states and decision; nothing reads the engine's rpm),
//! wired to the quad's own words: any wheel down, the gas lever after its cut, the body's speed and
//! the clutch lever as pressed. Its decision differs from MX Unleashed's bike's in one test: on the
//! ground it revs freely with the clutch in *or* the throttle locked. Its `[Wreck]` clank is the
//! plane's emitter on the quad's words.
//!
//! The note and clank halves run the shell's sound on made-up frames; the levers half rides the
//! quad off openmx-mods' `mxvatv_atv` pack on a flat floor, and skips without it.

#[allow(unused)]
#[path = "../src/thread.rs"]
mod thread;
#[allow(dead_code)]
#[path = "../src/audio.rs"]
mod audio;
#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;

use audio::{AtvFrame, BikeSounds, Clip, Hands, List, Mixer, Music, Note, PlaneCrash, SeatFrame};
use game_sim::ride::Controls;
use game_sim::world::Flat;
use ps2_float::{F, Vf};
use std::path::Path;
use std::sync::{Arc, Mutex};

const DT: f32 = 1.0 / 30.0;

fn tone(amplitude: f32, seconds: f32) -> Arc<Clip> {
    let rate = 22050;
    let pcm = (0..(seconds * rate as f32) as usize).map(|i| ((i as f32 * 0.07).sin() * amplitude) as i16).collect();
    Arc::new(Clip { rate, channels: 1, pcm })
}

/// A quad pack's `vehicle.aud` of tones, and one `[Wreck]` clank.
fn tone_quad() -> BikeSounds {
    let one = |c: Arc<Clip>, looping: bool| List { sounds: vec![(c, looping)] };
    BikeSounds {
        idle: one(tone(600.0, 0.5), true),
        low: one(tone(9000.0, 0.4), false),
        high: one(tone(9100.0, 0.4), false),
        power: one(tone(9200.0, 0.3), false),
        sputter: one(tone(7000.0, 0.2), false),
        decel: one(tone(2500.0, 0.6), false),
        clutch: one(tone(6000.0, 0.3), false),
        panic: List { sounds: vec![(tone(6100.0, 0.5), false), (tone(6200.0, 0.3), true)] },
        wreck: audio::MachineWreck { list: one(tone(12000.0, 0.9), false), near: 30.0, far: 150.0, min_impulse: 2.0, max_impulse: 20.0 },
        low_torque_speed: 25.0 * 1.4667,
        airborne_to_decel: 0.3,
        rev_midpoint: 0.311,
        overlap: 0.008,
        pitch_variance: 0,
        doppler: 1.3,
        near: 20.0,
        far: 200.0,
    }
}

/// The shell's sound with one seat on the quad, its pack's sounds `quad`'s; the bike's are none.
fn on_the_quad(quad: &BikeSounds) -> (audio::Sound, Arc<Mutex<Mixer>>) {
    let mixer = Arc::new(Mutex::new(Mixer::new(Music::new(Vec::new(), 1))));
    let mut sound = audio::Sound::new(mixer.clone());
    sound.seats.push(audio::SeatSound::new(Arc::new(BikeSounds::default()), 1));
    sound.set_own_atv(0, Some(Arc::new(quad.clone())));
    (sound, mixer)
}

fn quad_frame(hands: Hands, crash: PlaneCrash) -> SeatFrame {
    SeatFrame { atv: Some(AtvFrame { hands, crash }), ..Default::default() }
}

/// Standing on the ground, a second at idle and then the gas: the note it goes to.
fn gas_from_idle(locked: bool) -> Vec<Note> {
    let quad = tone_quad();
    let (mut sound, _) = on_the_quad(&quad);
    let still = Hands { grounded: true, ..Default::default() };
    let mut notes = Vec::new();
    for t in 0..45 {
        let gas = if t < 30 { 0.0 } else { 1.0 };
        let hands = Hands { gas, ..still }.throttle_locked(locked);
        sound.frame(&[quad_frame(hands, PlaneCrash::default())], DT);
        notes.push(sound.seats[0].atv_engine.note());
    }
    notes
}

#[test]
fn a_locked_throttle_revs_the_quads_note() {
    let free = gas_from_idle(false);
    let locked = gas_from_idle(true);
    assert_eq!(free[29], Note::Idle);
    assert_eq!(locked[29], Note::Idle);
    // Unlocked, the gas on the ground at a standstill pulls: the low acceleration.
    assert_eq!(free[30], Note::LowAccel, "unlocked: {free:?}");
    // Locked (a start gate), the same gas revs the engine freely, as the clutch held in does.
    assert_eq!(locked[30], Note::Rev, "locked: {locked:?}");
    assert!(locked[30..].iter().all(|n| matches!(n, Note::Rev | Note::Panic)), "locked: {locked:?}");
    // The lock is the clutch held in to the note, and nothing else of the hands.
    let h = Hands { grounded: true, gas: 0.5, clutch: 0.0, speed: 3.0, at: [1.0, 2.0, 3.0], vel: [0.0, 0.0, 3.0] };
    let l = h.throttle_locked(true);
    assert_eq!((l.grounded, l.gas, l.clutch, l.speed, l.at, l.vel), (h.grounded, h.gas, 1.0, h.speed, h.at, h.vel));
    let u = h.throttle_locked(false);
    assert_eq!((u.gas, u.clutch), (h.gas, h.clutch));
}

#[test]
fn the_quad_clanks_on_its_wreck() {
    let quad = tone_quad();
    let clank = &quad.wreck.list.sounds[0].0;
    let clanks = |crash: PlaneCrash| {
        let (mut sound, mixer) = on_the_quad(&quad);
        sound.frame(&[quad_frame(Hands { grounded: true, ..Default::default() }, crash)], DT);
        let m = mixer.lock().unwrap();
        m.clips().iter().filter(|(c, _)| Arc::ptr_eq(c, clank)).map(|&(_, g)| g).collect::<Vec<f32>>()
    };
    let hit = PlaneCrash { wrecked: true, touching: true, impulse: 10.0 };
    // Down, touching and hard enough: the pack's clank, at the impulse's place between the ranges.
    assert_eq!(clanks(hit), [0.5]);
    // Not down, not touching, or under MinImpulse: none.
    assert!(clanks(PlaneCrash { wrecked: false, ..hit }).is_empty());
    assert!(clanks(PlaneCrash { touching: false, ..hit }).is_empty());
    assert!(clanks(PlaneCrash { impulse: 1.0, ..hit }).is_empty());
}

/// The quad off the pack's Race 250 P handling file, or `None` without the pack.
fn quad() -> Option<vehicle::AtvRide> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/machines/mxvatv_atv");
    if !dir.join("mod.json").is_file() {
        println!("SKIPPED: no mods/machines/mxvatv_atv (openmx-mods' import-atv makes it)");
        return None;
    }
    let files = game_render::disc::Res::folder(&dir).expect("the pack's folder reads");
    let phy = "Veh/atv_Race250P.phy";
    Some(vehicle::AtvRide::read("mxvatv_atv", phy, &mut |n| files.read(&n.replace('/', "\\")).ok()).expect("the quad builds"))
}

#[test]
fn the_quads_note_follows_its_levers() {
    let Some(mut atv) = quad() else { return };
    let mut floor = Flat::default();
    let spot = game_sim::markers::Spot::still(Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0));
    atv.take(&spot, &mut floor).expect("the floor is under it");
    let mut lever_ahead = false;
    for t in 0..90 {
        let c = Controls { gas: F::ONE, ..Controls::default() };
        atv.step(&c, F(DT), &mut floor);
        let r = atv.quad().unwrap();
        let f = AtvFrame::of(r);
        let h = f.hands;
        assert_eq!(h.gas, r.q.gas.0, "tick {t}: the gas lever after its cut");
        assert_eq!(h.clutch, r.bike.clutch.0, "tick {t}: the clutch lever as pressed");
        assert_eq!((h.grounded, h.speed), (r.q.on_ground, r.q.speed_len.0), "tick {t}");
        assert_eq!(h.at, [r.body.position.x(), r.body.position.y(), r.body.position.z()]);
        assert!(!f.crash.wrecked, "tick {t}: it never wrecks on the floor");
        // The lever leads the rate-limited throttle MX Unleashed's bike's note reads.
        lever_ahead |= h.gas == 1.0 && r.q.gas_limited.0 < 1.0;
        // The throttle lock reaches the note as the clutch in.
        let mut locked = r.clone();
        locked.bike.held.lock.throttle = true;
        assert_eq!(AtvFrame::of(&locked).hands.clutch, 1.0, "tick {t}");
    }
    assert!(lever_ahead, "the hands read the lever, not the rate-limited throttle");
    let r = atv.quad().unwrap();
    assert!(r.q.speed_len.0 > 10.0, "it pulled away: {} ft/s", r.q.speed_len.0);
}
