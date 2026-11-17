//! P325: a pack's flier is drawn in the pack's own model beside the disc's of the same name.
//! `mods/machines/mxvatv_fliers` names its planes and helicopters (`biplane`, `kiowa`), their cones
//! and rotor parts (`propconeblur`, `bladeblur`) and their textures (`Pitts_body01`) as the disc's
//! files are named; keyed by name, whichever loaded first was drawn for both seats. Here P1's Plane
//! (1) off the pack and P2's MX Unleashed Biplane load into one set of assets, in both orders, and
//! the software path draws each: two different pictures, the same two whichever loaded first.
//!
//! It skips with no disc, and with no pack in this checkout's `mods/machines/`.

use game_render::biplane::Biplane;
use game_render::disc::Res;
use game_render::kiowa::{Kiowa, Rotor};
use game_render::mat::{M4, v3};
use game_render::raster::Frame;
use game_render::world::{Assets, Camera, Lighting, pack_key};
use game_render::Disc;
use std::path::Path;

const PACK: &str = "mxvatv_fliers";

/// The disc's machines' archive and the fliers pack, or `None` (said) without either.
fn archives() -> Option<(Res, Res)> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            eprintln!("skipped: {why}");
            return None;
        }
    };
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/machines").join(PACK);
    if !folder.join("mod.json").is_file() {
        eprintln!("SKIPPED: no mods/machines/{PACK} (openmx-mods' import-atv fliers makes it)");
        return None;
    }
    Some((disc.res("machine.res").expect("the machines' archive"), Res::folder(&folder).expect("the pack")))
}

/// The software path's picture of `parts` (names in `assets`, each at its matrix).
fn picture(assets: &Assets, parts: &[(&str, M4)]) -> Vec<u8> {
    let scene = game_render::scene::Scene::parse("[Fog]\nColorRGB=61,68,76\nMinimumFogDistance=5000.00\nMaximumFogDistance=9000.00\n");
    let light = Lighting::of(&scene);
    let camera = Camera { eye: v3(-20.0, 14.0, -34.0), at: v3(0.0, 3.0, 0.0), fov: 0.9, near: 0.5, far: 500.0 };
    let (w, h) = (320usize, 240usize);
    let mut frame = Frame::new(w, h, [0, 0, 0]);
    let vp = camera.view_projection(w, h);
    for (name, at) in parts {
        let loaded = assets.models.get(*name).unwrap_or_else(|| panic!("{name} is not loaded"));
        game_render::world::draw_model(&mut frame, loaded, assets, *at, vp, camera.eye, &light);
    }
    frame.color
}

fn differ(a: &[u8], b: &[u8]) -> usize {
    a.chunks(3).zip(b.chunks(3)).filter(|(p, q)| p.iter().zip(q.iter()).any(|(x, y)| x.abs_diff(*y) > 8)).count()
}

fn lit(a: &[u8]) -> usize {
    a.chunks(3).filter(|p| p.iter().any(|&c| c > 0)).count()
}

/// The disc's and the pack's file of one name are not the same file: else nothing here is proved.
fn files_differ(disc: &Res, pack: &Res, name: &str) {
    let read = |res: &Res| res.read(&res.find_suffix(&format!("{name}.psg")).expect(name).name).expect(name);
    assert_ne!(read(disc), read(pack), "{name}.psg is the same file on the disc and in the pack");
}

#[test]
fn plane_one_beside_mx_unleasheds_biplane_draws_two_models() {
    let Some((disc, pack)) = archives() else { return };
    files_differ(&disc, &pack, "biplane");
    let draw = |assets: &Assets, d: &Biplane| picture(assets, &d.parts(M4::IDENTITY, false));
    let mut both = Vec::new();
    for pack_first in [false, true] {
        let mut assets = Assets::default();
        let load_disc = |a: &mut Assets| Biplane::load(a, std::slice::from_ref(&disc), 0).expect("the disc's biplane");
        let load_pack = |a: &mut Assets| Biplane::load_model(a, std::slice::from_ref(&pack), &pack_key(PACK, "biplane")).expect("the pack's biplane");
        let (mxu, atv) = match pack_first {
            false => {
                let m = load_disc(&mut assets);
                (m, load_pack(&mut assets))
            }
            true => {
                let a = load_pack(&mut assets);
                (load_disc(&mut assets), a)
            }
        };
        assert_eq!(mxu.model, "biplane");
        assert_eq!(atv.model, "mxvatv_fliers:biplane");
        assert_eq!(atv.cone(false), "mxvatv_fliers:propconeblur");
        let (a, b) = (draw(&assets, &mxu), draw(&assets, &atv));
        assert!(lit(&a) > 2000 && lit(&b) > 2000, "{} and {} pixels of plane", lit(&a), lit(&b));
        let apart = differ(&a, &b);
        eprintln!("pack first {pack_first}: {apart} pixels apart, {} and {} lit", lit(&a), lit(&b));
        assert!(apart > 500, "pack first {pack_first}: the two planes draw alike ({apart} pixels apart)");
        both.push((a, b));
    }
    // The same two pictures whichever loaded first.
    assert_eq!(differ(&both[0].0, &both[1].0), 0, "the disc's plane draws differently after the pack's");
    assert_eq!(differ(&both[0].1, &both[1].1), 0, "the pack's plane draws differently after the disc's");
}

#[test]
fn the_packs_kiowa_beside_mx_unleasheds_draws_two_models() {
    let Some((disc, pack)) = archives() else { return };
    files_differ(&disc, &pack, "kiowa");
    let draw = |assets: &Assets, d: &Kiowa| {
        let pose = d.pose(assets, &Rotor::WHOLE);
        let mut parts = vec![(d.model.as_str(), M4::IDENTITY)];
        parts.extend(d.parts(M4::IDENTITY, &pose, &Rotor::WHOLE));
        picture(assets, &parts)
    };
    let mut both = Vec::new();
    for pack_first in [false, true] {
        let mut assets = Assets::default();
        let load_disc = |a: &mut Assets| Kiowa::load(a, std::slice::from_ref(&disc), 0).expect("the disc's kiowa");
        let load_pack = |a: &mut Assets| Kiowa::load_model(a, std::slice::from_ref(&pack), &pack_key(PACK, "kiowa")).expect("the pack's kiowa");
        let (mxu, atv) = match pack_first {
            false => {
                let m = load_disc(&mut assets);
                (m, load_pack(&mut assets))
            }
            true => {
                let a = load_pack(&mut assets);
                (load_disc(&mut assets), a)
            }
        };
        assert_eq!(atv.bent(), "mxvatv_fliers:bladebent");
        assert_eq!(mxu.bent(), "bladebent");
        let (a, b) = (draw(&assets, &mxu), draw(&assets, &atv));
        let apart = differ(&a, &b);
        eprintln!("pack first {pack_first}: {apart} pixels apart, {} and {} lit", lit(&a), lit(&b));
        assert!(lit(&a) > 2000 && lit(&b) > 2000, "{} and {} pixels of helicopter", lit(&a), lit(&b));
        assert!(apart > 500, "pack first {pack_first}: the two helicopters draw alike ({apart} pixels apart)");
        both.push((a, b));
    }
    assert_eq!(differ(&both[0].0, &both[1].0), 0, "the disc's kiowa draws differently after the pack's");
    assert_eq!(differ(&both[0].1, &both[1].1), 0, "the pack's kiowa draws differently after the disc's");
}
