//! S247 — a driven car heard through `carengine.aud`: the keyed-loop voice's revs, volumes and pitch
//! words against the car's own revs, and the drivetrain's sound pitch fed the engine's smooth noise.
//!
//! The first half needs no disc: a hand-made profile, its keys worked by hand. The second drives each
//! of the disc's three cars on a flat floor through the shell's own [`vehicle::CarRide::step`], hands
//! its engine voice the frame the shell does, and checks every loop's voice against its revs; it skips
//! without a disc.
//!
//! `src/vehicle.rs` and `src/audio.rs` are compiled in by path, as `tests/machines.rs` and
//! `tests/audio.rs` take them.

#[allow(unused)]
#[path = "../src/thread.rs"]
mod thread;
#[allow(dead_code)]
#[path = "../src/audio.rs"]
mod audio;
#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;

use audio::{CarEngine, CarFrame, CarGas, Clip, Mixer, Music};
use game_sim::input::{EventQueue, Pad, Report};
use game_sim::world::Flat;
use ps2_float::{F, Vf};
use std::sync::{Arc, Mutex};

/// A profile as `carengine.aud` writes one: two loops, keyed in revs, volume percent and hertz off
/// the file's 32 kHz; a half-volume ramp off the gas.
const PROFILE: &str = "[CarEngine]\nNumberOfProfiles = 1\n\n\
    [toy]\nMinRPM=250\nMaxRPM=6000\nNumberOfEngineLoops=2\nOriginalSampleRatePS2=32000\n\
    AttenuationMinDistance=10\nAttenuationMaxDistance=450\nThrottleThreshold=0.9\nDopplerFactor=1.3\n\
    OffThrottleVolume=0.5\nOffThrottleVolumeRampTime=0.05\nOnThrottleVolumeRampTime=0.1\n\n\
    [toyLoop1]\nFilename=low\nBaseRPM=1000\nNumberOfKeys=3\n\
    key1RPM=0\nkey1Vol=95\nkey1Rate=-3072\nkey2RPM=1000\nkey2Vol=100\nkey2Rate=0\nkey3RPM=2000\nkey3Vol=35\nkey3Rate=5783\n\n\
    [TOYLOOP2]\nFilename=high\nBaseRPM=4000\nNumberOfKeys=2\n\
    key1RPM=1500\nkey1Vol=0\nkey1Rate=-1446\nkey2RPM=6000\nkey2Vol=100\nkey2Rate=8000\n";

fn tone() -> Arc<Clip> {
    Arc::new(Clip { rate: 32000, channels: 1, pcm: (0..3200).map(|i| ((i % 64) as i16 - 32) * 500).collect() })
}

fn toy() -> CarEngine {
    CarEngine::read(PROFILE, "toy", &mut |_| Some(tone()))
}

#[test]
fn the_profiles_keys_are_read_as_the_original_reads_them() {
    let e = toy();
    assert_eq!(e.loops.len(), 2, "the loop sections, any case");
    // (32000 + rate) × 4096 / 48000, truncated: 32000 is 2730 (2730.67), −3072 → 2468 (2468.5),
    // +5783 → 3224 (3224.1), −1446 → 2607 (2607.3), +8000 → 3413 (3413.3).
    assert_eq!(e.loops[0].keys, vec![(0, 95, 2468), (1000, 100, 2730), (2000, 35, 3224)]);
    assert_eq!(e.loops[1].keys, vec![(1500, 0, 2607), (6000, 100, 3413)]);
    assert_eq!((e.min_rpm, e.max_rpm, e.near, e.far, e.threshold, e.doppler), (250, 6000, 10.0, 450.0, 0.9, 1.3));
    assert_eq!((e.off_cut, e.on_ramp, e.off_ramp), (0.5, 0.1, 0.05));

    // A missing profile is silent; the reader's own defaults under a bare one.
    assert!(CarEngine::read(PROFILE, "biplane", &mut |_| Some(tone())).loops.is_empty());
    let bare = CarEngine::read("[bare]\n", "bare", &mut |_| Some(tone()));
    assert_eq!((bare.threshold, bare.off_cut, bare.on_ramp, bare.off_ramp, bare.doppler), (0.9, 0.0, 0.0, 0.0, 0.25));
}

#[test]
fn the_voice_keys_its_loops_on_the_cars_revs() {
    let e = toy();
    // The pitch over the engine's top revs, onto the profile's MaxRPM, held to MinRPM..MaxRPM.
    assert_eq!(e.revs(3000.0, 6000.0), 3000.0);
    assert_eq!(e.revs(1000.0, 4000.0), 1500.0, "a 4000-rev engine at 1000 sounds at a quarter of 6000");
    assert_eq!(e.revs(10.0, 6000.0), 250.0, "held to MinRPM");
    assert_eq!(e.revs(9000.0, 6000.0), 6000.0, "held to MaxRPM");

    let (low, high) = (&e.loops[0], &e.loops[1]);
    // On a key: the key's own.
    assert_eq!(CarEngine::key(low, 1000.0, 1.0), (100, Some(2730)));
    // Between 1000 and 2000 at 1250: t = 0.25, volume 100 + (int)(0.25 × −65) = 100 − 16 = 84, pitch
    // 2730 + (int)(0.25 × 494) = 2730 + 123 = 2853.
    assert_eq!(CarEngine::key(low, 1250.0, 1.0), (84, Some(2853)));
    // The gain scales the interpolated volume, truncated: 84 × 0.5 = 42.
    assert_eq!(CarEngine::key(low, 1250.0, 0.5), (42, Some(2853)));
    // Outside a loop's keys it is silent and its pitch is not set.
    assert_eq!(CarEngine::key(low, 2000.5, 1.0), (0, None));
    assert_eq!(CarEngine::key(high, 1499.0, 1.0), (0, None));
    // The high loop at 3750: t = 0.5, volume 0 + 50, pitch 2607 + (int)(0.5 × 806) = 3010.
    assert_eq!(CarEngine::key(high, 3750.0, 1.0), (50, Some(3010)));
}

#[test]
fn the_gas_ramps_the_volume_as_the_original_does() {
    let e = toy();
    let mut gas = CarGas::default();
    let dt = 1.0 / 30.0;
    // Off the gas from the start: no ramp started, OffThrottleVolume.
    assert_eq!(gas.frame(&e, 0.0, dt), 0.5);
    // Onto it: the on ramp (0.1 s) restarts, and the gain climbs from 1 − 0.5 back to 1.
    assert_eq!(gas.frame(&e, 0.95, dt), 0.5);
    let g = gas.frame(&e, 1.0, dt);
    assert!((g - (1.0 - 0.5 * (0.1 - dt) / 0.1)).abs() < 1e-6, "{g}");
    gas.frame(&e, 1.0, dt);
    assert_eq!(gas.frame(&e, 1.0, dt), 1.0, "the ramp run out");
    // Off again under the threshold: the off ramp (0.05 s) starts, read over the *on* ramp's 0.1.
    let g = gas.frame(&e, 0.89, dt);
    assert!((g - (0.5 + 0.5 * 0.05 / 0.1)).abs() < 1e-6, "{g}");
    assert_eq!(gas.frame(&e, 0.0, dt), 0.5 + 0.5 * ((0.05f32 - dt) / 0.1));
    assert_eq!(gas.frame(&e, 0.0, dt), 0.5);
}

/// The pitch a loop's voice plays its sample at for a pitch word: `word × 48000 / 4096` hertz over the
/// sample's own rate.
fn pitch_of_word(word: i32, clip: &Clip) -> f32 {
    word as f32 * 48000.0 / 4096.0 / clip.rate as f32
}

#[test]
fn a_driven_cars_voice_follows_its_revs() {
    let Ok(mut disc) = game_render::Disc::open_configured() else {
        println!("skipped the disc's half: no disc");
        return;
    };
    let machines = disc.res("machine.res").expect("the machines' archive");
    let mut lib = audio::Library::open(game_render::Disc::open_configured().unwrap()).unwrap();
    let engines = audio::Reading { lib: &mut lib }.cars("machine.res");
    let tables = game_sim::rider::Tables::build();
    let table = game_sim::bindings::Table::SHIPPED;
    for machine in [vehicle::Machine::Buggy, vehicle::Machine::Monster, vehicle::Machine::Baja] {
        let e = engines.iter().find(|e| e.name == machine.dir()).unwrap_or_else(|| panic!("{machine:?}'s profile"));
        assert!(e.loops.len() >= 8 && e.max_rpm > e.min_rpm, "{machine:?}: carengine.aud's [{}] read off the disc", e.name);

        let mut car = vehicle::CarRide::new(machine, 0, None, &mut |n| machines.read_suffix(n).ok()).unwrap();
        let mut floor = Flat::default();
        let up = Vf::new(0.0, 1.0, 0.0, 0.0);
        car.take(&game_sim::markers::Spot::still(Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), up), &mut floor).unwrap();

        let mixer = Arc::new(Mutex::new(Mixer::new(Music::new(Vec::new(), 1))));
        let mut sound = audio::Sound::new(mixer.clone());
        sound.cars = engines.clone();
        sound.seats.push(audio::SeatSound::new(Arc::new(audio::BikeSounds::default()), 1));

        // A pad starts disconnected, and a disconnected one reads nothing.
        let mut pad = Pad::new(0);
        pad.connected = true;
        let mut events = EventQueue::new();
        let dt = 1.0 / 30.0;
        let (mut revs, mut noisy) = (Vec::new(), 0);
        // A second at idle, then six on the gas (Cross, the shipped preset's), then two off it.
        for tick in 0..270 {
            let buttons = if (30..210).contains(&tick) { game_sim::input::Control::Cross.mask() } else { 0 };
            pad.poll(F(dt), &Report::from_buttons(buttons), &mut events);
            car.step(&pad, &table, &tables.ride, F(dt), &mut floor);
            let m = car.driven.as_ref().unwrap();
            let s = car.engine_sound().unwrap();
            assert_eq!((s.pitch, s.top, s.throttle), (m.drivetrain.pitch.0, m.drivetrain.redline.0, m.controls.throttle.0));
            let lo = m.drivetrain.min_rpm.0;
            let place = (m.drivetrain.rpm.0 - lo) / (m.drivetrain.redline.0 - lo);
            if place < 0.2 && car.noise.is_some() {
                noisy += 1;
            }

            let frame = CarFrame { profile: s.profile, pitch: s.pitch, top: s.top, throttle: s.throttle, at: s.at, vel: s.vel };
            sound.frame(&[audio::SeatFrame { car: Some(frame), ..Default::default() }], dt);
            let heard = sound.seats[0].car.revs;
            assert_eq!(heard, e.revs(s.pitch, s.top), "{machine:?} tick {tick}: the voice's revs are the car's");
            let m = mixer.lock().unwrap();
            let voices = sound.seats[0].car.voices();
            assert_eq!(voices.len(), e.loops.len(), "{machine:?}: a voice per loop");
            // Each loop's voice at its key's pitch, sounding only inside its keys.
            for (v, l) in voices.iter().zip(&e.loops) {
                let (vol, word) = CarEngine::key(l, heard, 1.0);
                let gain = m.gain_of(*v).unwrap();
                assert_eq!(gain > 0.0, vol > 0, "{machine:?} tick {tick}: {} at {heard} revs, gain {gain}", l.clip.seconds());
                if let Some(w) = word {
                    let want = pitch_of_word(w, &l.clip);
                    let got = m.pitch_of(*v).unwrap();
                    assert!((got - want).abs() < 1e-5, "{machine:?} tick {tick}: pitch {got}, want {want} at {heard} revs");
                }
            }
            revs.push(heard);
        }
        // The gas brings the revs up the profile and letting go brings them back down.
        let idle = revs[29];
        let top = revs[30..210].iter().cloned().fold(f32::MIN, f32::max);
        let end = revs[269];
        println!("{machine:?}: revs {idle:.0} idle, {top:.0} on the gas, {end:.0} after; noise sampled on {noisy} ticks");
        assert!(top > idle + 0.3 * (e.max_rpm - e.min_rpm) as f32, "{machine:?}: the gas revs the voice up ({idle} → {top})");
        assert!(end < top, "{machine:?}: off the gas the voice comes down ({top} → {end})");
        assert!(car.noise.is_some() && noisy > 0, "{machine:?}: the noise's tables were built, and sampled at the bottom of the revs");
    }
}

/// **The step's noise is fed** (S247): the same car driven twice on the same pad, once through the
/// shell's step as it stands and once with noise tables whose every gradient is 0 — what the step was
/// handed before, a noise of 0 — rides the same to the bit, the pitch being the sound's alone; and the
/// two pitches part, only ever with the revs low in their range (the noise's clause is the pitch's
/// bottom fifth, which is the revs' bottom ninth).
#[test]
fn the_cars_sound_pitch_samples_the_smooth_noise() {
    let Ok(mut disc) = game_render::Disc::open_configured() else {
        println!("skipped: no disc");
        return;
    };
    let machines = disc.res("machine.res").expect("the machines' archive");
    let tables = game_sim::rider::Tables::build();
    let table = game_sim::bindings::Table::SHIPPED;
    for machine in [vehicle::Machine::Buggy, vehicle::Machine::Monster, vehicle::Machine::Baja] {
        let mut cars = [(); 2].map(|_| vehicle::CarRide::new(machine, 0, None, &mut |n| machines.read_suffix(n).ok()).unwrap());
        cars[1].noise = Some(game_sim::noise::SmoothNoise { perm: std::array::from_fn(|i| i as u8), g1: [F::ZERO; game_sim::noise::KNOTS] });
        let mut floor = Flat::default();
        let spot = game_sim::markers::Spot::still(Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0));
        for c in &mut cars {
            c.take(&spot, &mut floor).unwrap();
        }
        let mut pad = Pad::new(0);
        pad.connected = true;
        let mut events = EventQueue::new();
        let (mut low, mut parted) = (0, 0);
        for tick in 0..240 {
            let buttons = if (30..150).contains(&tick) { game_sim::input::Control::Cross.mask() } else { 0 };
            pad.poll(F(1.0 / 30.0), &Report::from_buttons(buttons), &mut events);
            for c in &mut cars {
                c.step(&pad, &table, &tables.ride, F(1.0 / 30.0), &mut floor);
            }
            let [a, b] = [0, 1].map(|i| cars[i].driven.as_ref().unwrap());
            assert_eq!(a.body.position.to_bits(), b.body.position.to_bits(), "{machine:?} tick {tick}: the noise moves nothing but the sound");
            assert_eq!(a.drivetrain.rpm.0.to_bits(), b.drivetrain.rpm.0.to_bits(), "{machine:?} tick {tick}");
            low += usize::from(a.drivetrain.rpm.0 < a.drivetrain.min_rpm.0 + 0.2 * (a.drivetrain.redline.0 - a.drivetrain.min_rpm.0));
            let apart = a.drivetrain.pitch.0 != b.drivetrain.pitch.0;
            let ninth = a.drivetrain.min_rpm.0 + (a.drivetrain.redline.0 - a.drivetrain.min_rpm.0) / 9.0 + 1.0;
            assert!(!apart || a.drivetrain.rpm.0 < ninth, "{machine:?} tick {tick}: the pitches parted at {} revs", a.drivetrain.rpm.0);
            parted += usize::from(apart);
        }
        println!("{machine:?}: {low} ticks low in the revs, the pitches parted on {parted}");
        assert!(cars[0].noise.as_ref().is_some_and(|n| n.g1.iter().any(|g| *g != F::ZERO)), "{machine:?}: the tables were built off the generator");
        assert!(low > 0 && parted > 0, "{machine:?}: the noise reached the pitch ({parted} ticks parted, {low} low)");
    }
}
