//! The original's fog range (V33): what the renderer of `recording2.p2m2`'s save state holds against
//! what [`Scene::fog_range`] makes of free09's scene off the disc.
//!
//! The scene says fog 10..1000; the renderer holds 374.4..1000, because the fog object starts the fog
//! `1 - Haziness` of the way to 64 feet short of its end (research/oracle.txt, `fog_apply`). The
//! renderer's pair is at `display_object`'s `+0x48`/`+0x4c`, and the view it draws through (the
//! object's first word) has the end as its far clip, at `+0x1a0`.
//!
//! Needs the user's disc (`kit.toml`) and the save state's RAM image (`./kit state ram` of
//! `recording2.p2m2_SaveState.p2s`); says SKIPPED without either.

use game_render::disc::Disc;
use game_render::scene::Scene;
use ps2_oracle::{Kit, addr, skip};

const STATE: &str = "recording2.p2m2_SaveState.eeMemory.bin";
const FOG_START: usize = 0x48;
const FOG_END: usize = 0x4c;
const VIEW_FAR: usize = 0x1a0;

fn free09_scene(disc: &mut Disc) -> Scene {
    let member = disc
        .entries()
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().ends_with("free09.res"))
        .min_by_key(|e| e.name.len())
        .map(|e| e.name.clone())
        .expect("free09's archive is on the disc");
    let res = disc.res(&member).unwrap();
    let text = res.read_suffix("free09.scn").expect("free09's scene");
    Scene::parse(&String::from_utf8_lossy(&text))
}

#[test]
fn free09s_fog_is_the_save_states() {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => return skip(&why.to_string()),
    };
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => return skip(&why),
    };
    let path = kit.ctx("states").join(STATE);
    let Ok(ram) = std::fs::read(&path) else {
        return skip(&format!("{} is not there: `./kit state ram` of its .p2s", path.display()));
    };
    let word = |a: usize| u32::from_le_bytes(ram[a..a + 4].try_into().unwrap());

    let scene = free09_scene(&mut disc);
    assert_eq!(scene.fog_color, [150, 151, 139], "the state is on free09: its fog colour");
    assert_eq!((scene.fog_near, scene.fog_far), (10.0, 1000.0));
    // 0.6 as the original reads it, the float below; the fog object in the state holds the same.
    assert_eq!((scene.fog_visibility.to_bits(), scene.fog_haziness.to_bits()), (0x3f80_0000, 0x3f19_9999));
    let (start, end) = scene.fog_range();

    let display = word(addr("display_object").unwrap() as usize) as usize & 0x01ff_ffff;
    let held = (f32::from_bits(word(display + FOG_START)), f32::from_bits(word(display + FOG_END)));
    assert_eq!((start.to_bits(), end.to_bits()), (held.0.to_bits(), held.1.to_bits()), "{start}..{end} against {held:?}");
    let view = word(display) as usize & 0x01ff_ffff;
    assert_eq!(f32::from_bits(word(view + VIEW_FAR)), end, "the view's far clip is the fog's end");
}
