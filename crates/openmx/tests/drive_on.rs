//! P145 — the wheels' drive-on loop: the speed rule against values worked by hand from the disc's
//! numbers, the emitter's start / move / stop as the original runs them, and the disc's own
//! `Col\template.cmt` (that part skips without the disc).

use std::sync::Arc;

// the browser's own clock, threads and files, which are std's here (P196)
#[allow(unused)]
#[path = "../src/thread.rs"]
mod thread;
#[allow(dead_code)]
#[path = "../src/audio.rs"]
mod audio;

use audio::{Clip, DriveOn, DriveOnFrame, DriveOnMaterial, DriveOnTable, Mixer, Music};

/// The disc's numbers for every material: `SampleSpeed` 50, `MinVolume` 0, `MinSpeed` 1,
/// `MaxSpeed` 99, `ResonantFreqSpeed` 50, `FreqSlope` 0.75.
fn disc_like(id: u8, near: f32, far: f32, clip: Option<Arc<Clip>>) -> DriveOnMaterial {
    DriveOnMaterial {
        id,
        clip,
        sample_speed: 50,
        min_volume: 0,
        min_speed: 1,
        max_speed: 99,
        resonant_speed: 50,
        freq_slope: 0.75,
        near,
        far,
    }
}

/// A draw that always answers the same.
struct Always(i32);

impl audio::Draw for Always {
    fn rand(&mut self) -> i32 {
        self.0
    }
}

#[test]
fn the_speed_rule_is_the_originals() {
    let m = disc_like(2, 30.0, 70.0, None);
    // 50 ft/s is 34.09 mph, cut to 34.
    assert_eq!(audio::drive_on_mph([30.0, 40.0, 0.0]), 34);
    assert_eq!(audio::drive_on_mph([0.0; 3]), 0);
    // A 22,050 Hz sample's own pitch, 4096 at 48 kHz.
    let clip = Clip { rate: 22050, channels: 1, pcm: Vec::new() };
    assert_eq!(audio::own_pitch(&clip), 1881);
    // 0.75 × 1881 / 50 = 28.215, plus a half, cut: 28 pitch units a mile an hour.
    assert_eq!(audio::drive_on_slope(&m, 1881), 28);
    // At `SampleSpeed` with a zero draw, the sound's own pitch; 10 mph over, 280 up.
    assert_eq!(audio::drive_on_pitch(&m, 1881, 28, 50, &mut Always(0)), 1881);
    assert_eq!(audio::drive_on_pitch(&m, 1881, 28, 60, &mut Always(0)), 1881 + 280);
    // The largest draw takes off just under slope + 1: 28.
    assert_eq!(audio::drive_on_pitch(&m, 1881, 28, 60, &mut Always(i32::MAX)), 1881 + 280 - 28);
    assert_eq!(audio::drive_on_pitch(&m, 1881, 28, 0, &mut Always(0)), 1881 - 1400);
    // (100 − 0) / (50 − 1) = 2 a mile an hour up to 50, then −100 / 49 = −2 down to 99.
    let levels: Vec<i32> = [0, 1, 2, 25, 50, 51, 75, 98, 99, 120].iter().map(|&s| audio::drive_on_level(&m, s)).collect();
    assert_eq!(levels, [0, 0, 2, 48, 98, 96, 48, 2, 0, 0]);
}

fn mixer() -> Mixer {
    let mut m = Mixer::new(Music::new(Vec::new(), 1));
    m.listeners = vec![audio::Listener { at: [0.0; 3], right: [1.0, 0.0, 0.0] }];
    m
}

fn table() -> DriveOnTable {
    let clip = Arc::new(Clip { rate: 22050, channels: 1, pcm: vec![0; 22050] });
    DriveOnTable {
        materials: vec![
            disc_like(2, 30.0, 70.0, Some(clip.clone())),
            // A wood fence: its distances are 0, so it is never started.
            disc_like(3, 0.0, 0.0, Some(clip)),
        ],
    }
}

fn on(material: u8, at: [f32; 3], speed: f32) -> DriveOnFrame {
    DriveOnFrame { material, at, vel: [0.0, 0.0, speed] }
}

#[test]
fn the_loop_starts_moves_and_stops_as_the_emitter_does() {
    let (mut m, t) = (mixer(), table());
    let mut d = DriveOn::new(1);

    // The ground (material 0) and a still bike play nothing.
    d.tick(&mut m, &t, &on(0, [5.0, 0.0, 0.0], 40.0));
    d.tick(&mut m, &t, &on(2, [5.0, 0.0, 0.0], 0.0));
    assert_eq!((d.voice(), d.material()), (None, 0));

    // Onto the wood: a loop, at the level of 40 ft/s (27 mph: 52), pitched off its own.
    d.tick(&mut m, &t, &on(2, [5.0, 0.0, 0.0], 40.0));
    let id = d.voice().expect("the loop started");
    assert!(m.playing(id));
    assert_eq!(m.gain_of(id), Some(0.52));
    // Moving on the same material keeps the one voice.
    d.tick(&mut m, &t, &on(2, [6.0, 0.0, 0.0], 60.0));
    assert_eq!(d.voice(), Some(id));
    assert!(m.playing(id));

    // Off the plank: stopped.
    d.tick(&mut m, &t, &on(0, [6.0, 0.0, 0.0], 60.0));
    assert!(!m.playing(id));
    assert_eq!((d.voice(), d.material()), (None, 0));

    // Too far to hear: not started, but the material is taken, and the loop stays off on it even
    // when the listener comes near — until the wheel leaves the prop.
    d.tick(&mut m, &t, &on(2, [100.0, 0.0, 0.0], 40.0));
    assert_eq!((d.voice(), d.material()), (None, 2));
    d.tick(&mut m, &t, &on(2, [5.0, 0.0, 0.0], 40.0));
    assert_eq!(d.voice(), None);
    d.tick(&mut m, &t, &on(0, [5.0, 0.0, 0.0], 40.0));
    d.tick(&mut m, &t, &on(2, [5.0, 0.0, 0.0], 40.0));
    let id = d.voice().expect("started again");

    // Rolled out of earshot on the same material: stopped, and not started again on coming back.
    d.tick(&mut m, &t, &on(2, [100.0, 0.0, 0.0], 40.0));
    assert!(!m.playing(id));
    d.tick(&mut m, &t, &on(2, [5.0, 0.0, 0.0], 40.0));
    assert!(!m.playing(id));
    assert_eq!(d.material(), 2);

    // A fence is never heard; and the fence, taken, holds a wheel that rolls straight on to the wood
    // silent (the original's emitter updates the material it has while that one's loop is quiet).
    d.tick(&mut m, &t, &on(0, [5.0, 0.0, 0.0], 40.0));
    d.tick(&mut m, &t, &on(3, [0.0; 3], 40.0));
    assert_eq!((d.voice(), d.material()), (None, 3));
    d.tick(&mut m, &t, &on(2, [5.0, 0.0, 0.0], 40.0));
    assert_eq!((d.voice(), d.material()), (None, 3));

    // A material the table lacks is silent.
    d.tick(&mut m, &t, &on(0, [5.0, 0.0, 0.0], 40.0));
    d.tick(&mut m, &t, &on(7, [5.0, 0.0, 0.0], 40.0));
    assert_eq!((d.voice(), d.material()), (None, 0));
}

/// Every track's `Col\template.cmt` is the same file: 22 materials, every one on
/// `Impact\drv_drt1.vag`, six of them heard within 70 ft.
#[test]
fn the_discs_table_is_free09s() {
    let d = match game_render::disc::Disc::open_configured() {
        Ok(d) => d,
        Err(e) => {
            println!("skipped the disc's half: {e}");
            return;
        }
    };
    let mut lib = audio::Library::open(d).unwrap();
    let t = audio::Reading { lib: &mut lib }.drive_on("tracks\\free09.res");
    let ids: Vec<u8> = t.materials.iter().map(|m| m.id).collect();
    assert_eq!(ids, (1..=22).collect::<Vec<u8>>());
    let heard: Vec<u8> = t.materials.iter().filter(|m| m.far > 0.0).map(|m| m.id).collect();
    assert_eq!(heard, [1, 2, 4, 5, 9, 22]);
    for m in &t.materials {
        let c = m.clip.as_ref().unwrap_or_else(|| panic!("material {} has no sound", m.id));
        assert_eq!((c.rate, c.channels), (22050, 1), "material {}", m.id);
        assert_eq!(
            (m.sample_speed, m.min_volume, m.min_speed, m.max_speed, m.resonant_speed, m.freq_slope),
            (50, 0, 1, 99, 50, 0.75),
            "material {}",
            m.id
        );
        if m.far > 0.0 {
            assert_eq!((m.near, m.far), (30.0, 70.0), "material {}", m.id);
        }
    }
}
