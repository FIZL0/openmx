//! V113: the Kiowa off the user's disc, as the shell draws it — every skin's model with its textures,
//! the rotor's nodes found, the four blade parts loaded beside it and posed onto those nodes, and the
//! helicopter's camera presets in a freeride track's second archive. Skips without a disc.

use game_render::Disc;
use game_render::kiowa::{self, Kiowa, Rotor};
use game_render::mat::{M4, v3};
use game_render::world::Assets;
use game_sim::camera::Preset;

fn disc() -> Option<Disc> {
    match Disc::open_configured() {
        Ok(d) => Some(d),
        Err(why) => {
            eprintln!("skipped: {why}");
            None
        }
    }
}

#[test]
fn every_skin_loads_with_its_rotors() {
    let Some(mut disc) = disc() else { return };
    let archives = [disc.res(kiowa::ARCHIVE).expect("the machines' archive")];
    let mut assets = Assets::default();
    let skins: Vec<Kiowa> = (0..kiowa::MODELS.len()).map(|s| Kiowa::load(&mut assets, &archives, s).expect("a skin")).collect();
    for (k, name) in skins.iter().zip(kiowa::MODELS) {
        assert_eq!(k.model, name);
        assert!(k.spindle.is_some() && k.tail.is_some() && k.blades.iter().all(Option::is_some), "{name}: {k:?}");
        let loaded = &assets.models[name];
        assert!(!loaded.tris.is_empty(), "{name} has no triangles");
        assert!(loaded.textures.iter().all(Option::is_some), "{name}: a material without its texture: {:?}", loaded.model.materials);
    }
    for part in [kiowa::BLADE_BLUR, kiowa::BLADE_BENT, kiowa::TAIL_BLUR, kiowa::TAIL_BENT] {
        let loaded = assets.models.get(part).unwrap_or_else(|| panic!("{part} is not loaded"));
        assert!(!loaded.tris.is_empty() && loaded.textures.iter().all(Option::is_some), "{part}");
    }
    // The blurs are see-through and leave depth alone.
    for blur in [kiowa::BLADE_BLUR, kiowa::TAIL_BLUR] {
        let s = &assets.models[blur].surfaces;
        assert!(s.iter().any(|s| s.blend) && s.iter().filter(|s| s.blend).all(|s| !s.depth_write), "{blur}: {s:?}");
    }
    // The second skin is the other livery.
    let body = |name: &str| assets.models[name].model.materials.iter().any(|m| m.to_ascii_lowercase().contains("kiowa_body02"));
    assert!(!body("kiowa") && body("kiowa2"), "kiowa2 is not the second livery");

    // The blades hang round the mast over the cabin and the tail rotor far back; a quarter turn of
    // the rotor moves the blades round the mast and leaves the tail rotor's hub where it was.
    let k = &skins[0];
    let rest = k.pose(&assets, &Rotor::WHOLE);
    let parts = k.parts(M4::IDENTITY, &rest, &Rotor::WHOLE);
    assert_eq!(parts.iter().filter(|(n, _)| *n == kiowa::BLADE_BLUR).count(), 4);
    assert_eq!(parts.iter().filter(|(n, _)| *n == kiowa::TAIL_BLUR).count(), 1);
    let at = |m: &M4| m.point(v3(0.0, 0.0, 0.0));
    let mast = at(&rest[k.spindle.unwrap()]);
    let tail = at(&rest[k.tail.unwrap()]);
    println!("kiowa: spindle at {mast:?}, tail rotor at {tail:?}");
    assert!(mast.y > 5.0 && tail.z < -20.0, "spindle {mast:?}, tail {tail:?}");
    let turned = k.pose(&assets, &Rotor { angle: std::f32::consts::FRAC_PI_2, ..Rotor::WHOLE });
    for b in k.blades.iter().flatten() {
        let (a, c) = (at(&rest[*b]), at(&turned[*b]));
        let r = |p: game_render::mat::V3| (p.x - mast.x).hypot(p.z - mast.z);
        assert!((r(a) - r(c)).abs() < 1e-2 && (a.y - c.y).abs() < 1e-2, "blade {b} left the rotor's circle: {a:?} -> {c:?}");
    }
    assert!(at(&turned[k.tail.unwrap()]).sub(tail).len() < 1e-3);
}

#[test]
fn the_helicopter_cameras_are_in_the_freeride_archives() {
    let Some(mut disc) = disc() else { return };
    let res = disc.res("Tracks\\free09_2.res").expect("free09's second archive");
    let bytes = res.read_suffix(kiowa::PRESETS).expect("helocpresets.ini");
    let presets = Preset::read_bytes(&bytes, "");
    let names: Vec<&str> = presets.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Aero Cam 1", "Aero Cam 2", "Slot Cam", "Lookback"]);
}
