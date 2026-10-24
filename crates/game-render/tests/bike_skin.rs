//! The bike's skin against the real disc (V37): every mesh of every part of every bike on the disc
//! is drawn with the texture its own build paints it with, on the chassis its own set names.
//!
//! The 125sx — the shell's first bike — repaints its plastics `125 SX_PlasticsAll`, with a space the
//! original's reader drops. Read with the space, the name was no material on the disc and the
//! plastics part (the seat, the side panels, the rear fender) and the engine's and front fender's
//! plastic were drawn white, which is how the seat went missing against the white fender. And the
//! five fast50s are built on `bike50.psg`, not `bike.psg`, which the renderer had loaded for every
//! bike: a fast50 was its parts with no frame, forks or wheels under them.
//!
//! Needs the user's own disc, so it skips with the reason when it is not there.

use game_render::disc::{Disc, Res};
use game_render::ini;
use game_render::scene::{Material, Vehicle, slt_name};
use game_render::vehicle::chassis_model;
use game_render::world::{Assets, decode_texture, texture_files};
use std::collections::HashMap;

/// Every bike archive on the disc, as `(its name, the member)`.
fn bikes(disc: &Disc) -> Vec<(String, String)> {
    disc.entries()
        .iter()
        .filter_map(|e| {
            let lower = e.name.to_ascii_lowercase();
            let stem = lower.strip_prefix("bike\\")?.strip_suffix(".res")?;
            Some((stem.to_string(), e.name.clone()))
        })
        .collect()
}

/// The texture a material slot ought to draw with, worked out without [`Assets`]: the build's
/// rename if the archive has it (or the name itself), that material's `.psm`, and the first of its
/// files the archive has.
fn expected(res: &Res, renames: &HashMap<String, String>, material: &str) -> Option<(String, game_render::pst::Texture)> {
    // A repaint the archive has no material for keeps the mesh's own: 500_03's pipe.
    let (name, psm) = renames
        .get(&material.to_ascii_lowercase())
        .and_then(|to| Some((to.clone(), res.read_suffix(&format!("{to}.psm")).ok()?)))
        .or_else(|| Some((material.to_string(), res.read_suffix(&format!("{material}.psm")).ok()?)))?;
    let m = Material::parse(&ini::text(&psm));
    texture_files(&name, &m)
        .into_iter()
        .find_map(|file| decode_texture(&file, &res.read_suffix(&file).ok()?).map(|t| (file, t)))
}

#[test]
fn every_mesh_part_of_every_bike_is_drawn_with_its_own_texture() {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            println!("SKIPPED: {why}");
            return;
        }
    };
    let mut checked = 0;
    let mut meshes = 0;
    for (bike, member) in bikes(&disc) {
        let res = disc.res(&member).expect("a bike archive opens");
        let vehicle = Vehicle::parse(&ini::text(&res.read_suffix(&format!("{bike}.sub")).expect("a .sub")));
        let archives = [res];
        let chassis = chassis_model(&archives, &vehicle);
        // The shell's own order: the build's repaint, then the chassis, then the parts.
        let mut assets = Assets::default();
        assets.rename_materials(&vehicle.materials);
        assert!(assets.load_as(&archives, &chassis, "bike"), "{bike}: no {chassis}.psg to draw");
        for part in &vehicle.parts {
            assert!(assets.load(&archives, &part.model), "{bike}: no {}.psg", part.model);
        }
        let renames: HashMap<String, String> =
            vehicle.materials.iter().map(|(from, to)| (from.to_ascii_lowercase(), to.clone())).collect();
        let models = std::iter::once("bike".to_string()).chain(vehicle.parts.iter().map(|p| p.model.clone()));
        for name in models {
            let loaded = &assets.models[&name];
            for mesh in &loaded.model.meshes {
                let material = &loaded.model.materials[mesh.material];
                let Some(i) = loaded.textures[mesh.material] else {
                    panic!("{bike}: {name}'s {material} (lod {}) is drawn untextured", mesh.group);
                };
                let (file, want) = expected(&archives[0], &renames, material)
                    .unwrap_or_else(|| panic!("{bike}: {name}'s {material} has no texture of its own"));
                let got = &assets.textures[i];
                assert!(
                    (got.width, got.height) == (want.width, want.height) && got.rgba == want.rgba,
                    "{bike}: {name}'s {material} is drawn with another texture than {file}"
                );
                meshes += 1;
            }
        }
        checked += 1;
    }
    // Every bike archive on the disc, so a skip cannot pass for a check.
    assert_eq!(checked, 54, "bikes checked");
    assert!(meshes > 54 * 30, "{meshes} meshes");
}

/// The default bike's plastics are its own orange atlas on the chassis and on every part, and the
/// fast50s are drawn on their own chassis.
#[test]
fn the_125sx_plastics_and_the_fast50_chassis() {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            println!("SKIPPED: {why}");
            return;
        }
    };
    let res = disc.res("Bike\\125sx.res").expect("the 125sx archive");
    let vehicle = Vehicle::parse(&ini::text(&res.read_suffix("125sx.sub").unwrap()));
    let to = vehicle.materials.iter().find(|(from, _)| from == "250SX_PlasticsAll").map(|(_, to)| to.as_str());
    assert_eq!(to, Some("125SX_PlasticsAll"), "the space in `125 SX_PlasticsAll` is dropped");
    assert_eq!(slt_name("125 SX_PlasticsAll"), "125SX_PlasticsAll");
    let archives = [res];
    assert_eq!(chassis_model(&archives, &vehicle), "bike");
    let mut assets = Assets::default();
    assets.rename_materials(&vehicle.materials);
    for name in std::iter::once("bike").chain(vehicle.parts.iter().map(|p| p.model.as_str())) {
        assets.load(&archives, name);
    }
    let plastics = |model: &str, material: &str| {
        let loaded = &assets.models[model];
        let slot = loaded.model.materials.iter().position(|m| m == material).unwrap();
        loaded.textures[slot]
    };
    let atlas = plastics("bike", "cr-250_plasticsall");
    assert!(atlas.is_some());
    for part in ["250SX_Plastics", "250SX_Engine", "250SX_Front_Fender"] {
        assert_eq!(plastics(part, "250sx_plasticsall"), atlas, "{part}");
    }

    let res = disc.res("Bike\\fast50_01.res").expect("a fast50 archive");
    let vehicle = Vehicle::parse(&ini::text(&res.read_suffix("fast50_01.sub").unwrap()));
    assert_eq!(chassis_model(&[res], &vehicle), "bike50");
}
