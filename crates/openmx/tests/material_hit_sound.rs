//! F27 — the material table's collision sounds on the audio side: the disc's `Col\template.cmt` read
//! as the original's table holds it (the RAM table of `s195-knock-2881`, checked in
//! `game-sim/tests/material_hit.rs`), and a frame's hits played as overlapping one-shots. The disc's
//! half skips without the disc.

use std::sync::Arc;

#[allow(unused)]
#[path = "../src/thread.rs"]
mod thread;
#[allow(dead_code)]
#[path = "../src/audio.rs"]
mod audio;

use audio::{Clip, HitMaterial, HitTable, Mixer, Music};
use game_sim::material_hit::{self, Hit, BIKE};
use ps2_float::{F, Vf};

/// The free09 table's IDs: the ones the original keeps (its track uses), as the knock state's RAM holds them.
const IN_USE: [u8; 15] = [1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 14, 16, 19, 21, 22];

#[test]
fn the_discs_table_is_the_states() {
    let d = match game_render::disc::Disc::open_configured() {
        Ok(d) => d,
        Err(e) => {
            println!("skipped the disc's half: {e}");
            return;
        }
    };
    let mut lib = audio::Library::open(d).unwrap();
    let t = audio::Reading { lib: &mut lib }.hits("tracks\\free09.res");
    assert_eq!(t.foliage, 22);
    assert_eq!(t.rule.len(), t.materials.len());
    for id in IN_USE {
        let m = t.materials.iter().find(|m| m.id == id).unwrap_or_else(|| panic!("no material {id}"));
        assert_eq!((m.near, m.far), (30.0, 125.0), "material {id}");
        let want = if id == BIKE { (2.0, 9.0) } else { (5.0, 25.0) };
        assert_eq!((m.min_impulse, m.max_impulse), want, "material {id}");
        let c = m.clip.as_ref().unwrap_or_else(|| panic!("material {id} has no sound"));
        assert!(!c.pcm.is_empty(), "material {id}");
    }
    // The three whose names the state's table was checked by, as clips: the bike's is not the ball's.
    let clip = |id: u8| t.materials.iter().find(|m| m.id == id).and_then(|m| m.clip.clone()).unwrap();
    assert!(!Arc::ptr_eq(&clip(BIKE), &clip(21)) && clip(BIKE).pcm != clip(21).pcm);
    assert!(clip(22).pcm != clip(21).pcm);
}

fn mixer() -> Mixer {
    let mut m = Mixer::new(Music::new(Vec::new(), 1));
    m.listeners = vec![audio::Listener { at: [0.0; 3], right: [1.0, 0.0, 0.0] }];
    m
}

fn table() -> HitTable {
    let clip = |n: u16| Some(Arc::new(Clip { rate: 22050, channels: 1, pcm: vec![n as i16; 22050] }));
    let materials = vec![
        HitMaterial { id: 3, clip: clip(3), near: 30.0, far: 125.0, min_impulse: 5.0, max_impulse: 25.0 },
        HitMaterial { id: BIKE, clip: clip(16), near: 30.0, far: 125.0, min_impulse: 2.0, max_impulse: 9.0 },
    ];
    let rule = materials
        .iter()
        .map(|m| material_hit::Material { id: m.id, min_impulse: F(m.min_impulse), max_impulse: F(m.max_impulse) })
        .collect();
    HitTable { materials, foliage: 22, rule }
}

fn hit(owner: u32, owner_material: u8, struck: u32, struck_material: u8, speed: f32) -> Hit {
    Hit {
        owner: Some(owner),
        owner_material,
        struck,
        struck_material,
        struck_foliage: false,
        velocity: Vf::new(0.0, 0.0, speed, 0.0),
        at: Vf::new(owner as f32, 0.0, 0.0, 1.0),
    }
}

#[test]
fn a_frames_hits_play_over_each_other() {
    let (mut m, t) = (mixer(), table());
    // A ball (material 3 struck) twice in a frame, and a bike on a bike: three voices.
    let mut q = vec![hit(1, 0, 10, 3, 15.0), hit(2, 0, 10, 3, 30.0), hit(3, BIKE, 4, BIKE, 4.5)];
    let sounds = material_hit::play(&mut q, &t.rule, t.foliage);
    t.sound(&mut m, &sounds);
    let played: Vec<(i16, f32)> = m.clips().iter().map(|(c, g)| (c.pcm[0], *g)).collect();
    // 15 / 25 is 60; 30 is past the top, 100; 4.5 / 9 is 50 with the bike's own numbers.
    assert_eq!(played, [(3, 0.6), (3, 1.0), (16, 0.5)]);
    assert!(q.is_empty());
    // The next frame's hit does not stop the last ones.
    let mut q = vec![hit(1, 0, 10, 3, 5.0), hit(1, 0, 10, 3, 4.9)];
    t.sound(&mut m, &material_hit::play(&mut q, &t.rule, t.foliage));
    assert_eq!(m.voices(), 4);
    assert_eq!(m.clips()[3].1, 0.2);
}
