//! The trees and bushes of a track's `.esb`, against the real disc and the original's own numbers
//! (V48).
//!
//! The species' sizes, the models' scales and the billboards' spans below were read out of the
//! original's ecosystem in save states of free09 (the first human recording's) and free06 (the
//! biplane recording's); the journal entry for V48 has how they were found.
//!
//! Needs the user's own disc, so every test skips with the reason when it is not there.

use game_render::disc::{Disc, Res};
use game_render::eco::{self, Draw, Ecosystem, Grove, Look};
use game_render::mat::v3;
use game_render::scene::Scene;
use game_render::world::{Assets, Camera};
use game_sim::track;

fn disc() -> Option<Disc> {
    match Disc::open_configured() {
        Ok(d) => Some(d),
        Err(why) => {
            println!("SKIPPED: {why}");
            None
        }
    }
}

/// A track's two archives, as the shell opens them: `name.res` and `name_2.res`.
fn archives(disc: &mut Disc, name: &str) -> Vec<Res> {
    [format!("\\{name}.res"), format!("\\{name}_2.res")]
        .iter()
        .filter_map(|want| {
            let member = disc
                .entries()
                .iter()
                .find(|e| e.name.to_ascii_lowercase().ends_with(want.as_str()))?
                .name
                .clone();
            Some(disc.res(&member).expect("the track's archive"))
        })
        .collect()
}

/// The scene's bytes, `None` for an archive with no `.scn`.
fn scn(res: &Res) -> Option<Vec<u8>> {
    let member = res.entries().iter().find(|e| e.name.to_ascii_lowercase().ends_with(".scn"))?.name.clone();
    Some(res.read(&member).expect("the scene"))
}

fn terrain_bytes(res: &Res, named: &str) -> Vec<u8> {
    res.read_suffix(named)
        .or_else(|_| res.read_suffix(&named.replace(".trn", ".ps2.trn")))
        .unwrap_or_else(|_| panic!("no {named}"))
}

/// The ground's height under `(x, z)`, from the sim's own terrain.
fn ground(terrain: &track::Terrain, x: f32, z: f32) -> Option<f32> {
    let sp = terrain.scale.0;
    let (u, v) = (x / sp, z / sp);
    let (c, r) = (u.floor(), v.floor());
    let q = terrain.cell(c as u32, r as u32, 1)?;
    let (fu, fv) = (u - c, v - r);
    let h: Vec<f32> = q.height.iter().map(|h| h.0 * sp).collect();
    let near = h[0] + (h[1] - h[0]) * fu;
    let far = h[2] + (h[3] - h[2]) * fu;
    Some(near + (far - near) * fv)
}

#[test]
fn free09s_ecosystem_places_the_originals_trees_and_bushes() {
    let Some(mut disc) = disc() else { return };
    let archives = archives(&mut disc, "free09");
    let bytes = scn(&archives[0]).expect("free09's scene");
    let mut look = Scene::parse(&game_render::ini::text(&bytes));
    assert_eq!(look.ecosystem_file.as_deref(), Some("free09.esb"));
    assert_eq!((look.eco_distance_2d, look.eco_distance_3d, look.shrubbery_limit), (350.0, 300.0, 400.0));

    // The map's larger side, which the positions are 65536ths of: 6912 feet, the original's
    // 0.10546875 a unit times 65536.
    let place = track::Scene::parse(&bytes).expect("the sim's scene");
    let terrain_file = terrain_bytes(&archives[0], &place.terrain_file);
    let terrain = track::Terrain::parse(&terrain_file).expect("the terrain");
    let extent = eco::extent(&place.terrain, &terrain);
    assert_eq!(extent, 6912.0);

    let mut assets = Assets::default();
    let grove = Grove::load(&archives, &look, extent, &mut assets).expect("the .esb reads").expect("free09 has one");
    let e = &grove.ecosystem;
    assert_eq!((e.version, e.plants.len()), (2, 726));
    assert_eq!(e.probability.as_deref(), Some("Free09_TreeProbability.tga"));
    let names: Vec<&str> = e.species.iter().map(|s| s.model.as_str()).collect();
    assert_eq!(
        names,
        [
            "SpringTime2_TreeB", "SpringTime2_BushA", "SpringTime2_TreeA", "SpringTime2_TreeF", "SpringTime2_BushB",
            "SpringTime2_TreeC", "SpringTime2_TreeD", "SpringTime2_TreeE", "SpringTime2_TreeG", "SpringTime2_TreeH",
            "SpringTime2_TreeI",
        ]
    );
    let mut each = vec![0; e.species.len()];
    for p in &e.plants {
        each[p.species] += 1;
    }
    assert_eq!(each, [87, 63, 23, 79, 33, 76, 79, 94, 25, 76, 91]);
    assert!(e.plants.iter().all(|p| !p.snap), "free09 stands none of its plants by the loader");

    // Every plant stands on the ground the sim rides, where the file put it.
    let mut off: Vec<f32> = e.plants.iter().map(|p| p.pos.y - ground(&terrain, p.pos.x, p.pos.z).expect("on the map")).collect();
    off.sort_by(f32::total_cmp);
    let median = off[off.len() / 2];
    println!("plants above the ground: least {}, median {median}, most {}", off[0], off[off.len() - 1]);
    assert!(median.abs() < 0.1 && off[0] > -2.0 && off[off.len() - 1] < 2.0);

    // The first plant of the file, a TreeC with its size bytes -79 and -61, and the original's
    // reading of them.
    let first = e.plants[0];
    assert_eq!(first.species, 5);
    assert_eq!(first.pos, v3(32547.0 * 0.10546875, 1820.0 * 0.10546875, 35505.0 * 0.10546875));
    assert_eq!((first.height, first.width), (65.0 - 79.0 * 25.0 / 128.0, 20.0 - 61.0 * 10.0 / 128.0));

    // TreeB as the original holds it: the model's scales one over its bounds, and the billboard two
    // widths across, split at the trunk.
    let tree_b = &e.species[0];
    let node = &assets.models["springtime2_treeb"].model.nodes[0];
    assert!((0.5 / node.half_extents.y - 0.0157).abs() < 1e-4 && (1.0 / node.half_extents.x - 0.04106).abs() < 1e-4);
    let (left, right) = tree_b.billboard_span();
    assert!((left - 19.77).abs() < 0.01 && (right - 20.23).abs() < 0.01, "{left} {right}");
    assert_eq!(tree_b.height[0], 50.0);

    // Every species' model and billboard is on the disc, and the billboard is painted from the atlas.
    for (model, board) in grove.models.iter().zip(&grove.billboards) {
        assert!(!assets.models[model].tris.is_empty(), "{model}");
        let b = &assets.models[board];
        assert_eq!(b.tris.len(), 2);
        assert!(b.textures[0].is_some(), "{board} has no texture");
        assert!(b.surfaces[0].blend && b.surfaces[0].cutoff == 100, "{board}: {:?}", b.surfaces[0]);
    }

    // From beside the first plant, looking along (3, 0, 4): it is its model, turned to the camera's
    // heading and scaled to its size; everything past `EcoDistance2D` is a picture only.
    let eye = first.pos.add(v3(-12.0, 6.0, -16.0));
    let camera = Camera { eye, at: eye.add(v3(3.0, 0.0, 4.0)), fov: 0.9, near: 0.5, far: 4000.0 };
    let fov = eco::horizontal_fov(&camera, 4.0 / 3.0);
    let mut drawn: Vec<Draw> = Vec::new();
    grove.draws(&camera, fov, 1.0e6, &mut drawn);
    let mine = drawn.iter().find(|d| d.at.0[3][..3] == [first.pos.x, first.pos.y, first.pos.z]).expect("the first plant");
    assert_eq!(mine.model, "springtime2_treec");
    let half = assets.models["springtime2_treec"].model.nodes[0].half_extents;
    let (sw, sh) = (first.width / half.x, first.height * 0.5 / half.y);
    // The original's two shorts for that heading: its z and minus its x, cos 0.8 and sin -0.6.
    let (c, sn) = (0.8, -0.6);
    let want = [[c * sw, 0.0, sn * sw, 0.0], [0.0, sh, 0.0, 0.0], [-sn * sw, 0.0, c * sw, 0.0]];
    for (got, want) in mine.at.0.iter().zip(want) {
        assert!(got.iter().zip(want).all(|(g, w)| (g - w).abs() < 1e-4), "{:?}, the original's {want:?}", mine.at.0);
    }
    for p in &e.plants {
        let seen = eco::look(camera.eye, p.pos, fov, look.eco_distance_3d, look.eco_distance_2d);
        let at = [p.pos.x, p.pos.y, p.pos.z];
        let of: Vec<&Draw> = drawn.iter().filter(|d| d.at.0[3][..3] == at).collect();
        let (model, board) = (&grove.models[p.species], &grove.billboards[p.species]);
        match seen {
            Look::Model => assert!(of.len() == 1 && of[0].model == model),
            Look::Both(_) => assert!(of.len() == 2 && of[0].model == model && of[1].model == board),
            Look::Billboard => assert!(of.len() == 1 && of[0].model == board && of[0].alpha == 1.0),
        }
    }
    let far = e.plants.iter().filter(|p| eco::look(camera.eye, p.pos, fov, 300.0, 350.0) == Look::Billboard).count();
    assert!(far > 600, "{far} of 726 past the distance");

    // A model turned once keeps its turn while it stays near, as the original's two shorts do.
    let turned = Camera { at: eye.add(v3(0.0, 0.0, 1.0)), ..camera };
    let mut again = Vec::new();
    grove.draws(&turned, fov, 1.0e6, &mut again);
    let still = again.iter().find(|d| d.at.0[3][..3] == [first.pos.x, first.pos.y, first.pos.z]).unwrap();
    assert_eq!(still.at, mine.at);

    // And the scene carries it to both renderers.
    look.ecosystem = Some(grove);
    assert!(look.ecosystem.is_some());
}

#[test]
fn every_scene_that_names_an_ecosystem_reads_it_to_the_last_byte() {
    let Some(mut disc) = disc() else { return };
    let tracks: Vec<String> = disc
        .entries()
        .iter()
        .filter_map(|e| {
            let n = e.name.to_ascii_lowercase();
            let stem = n.rsplit('\\').next()?.strip_suffix(".res")?.to_string();
            (!stem.ends_with("_2")).then_some(stem)
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut read = Vec::new();
    for t in &tracks {
        // Not every archive on the disc is a track's, nor opens as one.
        let member = disc.entries().iter().find(|e| e.name.to_ascii_lowercase().ends_with(&format!("\\{t}.res"))).map(|e| e.name.clone());
        let Some(Ok(_)) = member.map(|m| disc.res(&m)) else { continue };
        let archives = archives(&mut disc, t);
        let Some(bytes) = archives.first().and_then(scn) else { continue };
        let look = Scene::parse(&game_render::ini::text(&bytes));
        let Some(file) = look.ecosystem_file.as_deref() else { continue };
        let Ok(place) = track::Scene::parse(&bytes) else { continue };
        let terrain_file = terrain_bytes(&archives[0], &place.terrain_file);
        // The nationals' ground does not read in the sim yet, so neither does the map's size: their
        // files are read through for their layout, the plants' places in units of the map.
        let extent = match track::Terrain::parse(&terrain_file) {
            Ok(terrain) => Some(eco::extent(&place.terrain, &terrain)),
            Err(why) => {
                println!("{t}: the terrain does not read ({why}), so the plants are not placed");
                None
            }
        };
        let esb = archives.iter().find_map(|a| a.read_suffix(file).ok()).unwrap_or_else(|| panic!("{t}: no {file}"));
        let e = Ecosystem::parse(&esb, extent.unwrap_or(65536.0)).unwrap_or_else(|why| panic!("{t}: {why}"));
        let models: std::collections::BTreeSet<&str> = e.species.iter().map(|s| s.model.as_str()).collect();
        println!("{t}: {file}, {} species, {} plants, {models:?}", e.species.len(), e.plants.len());
        read.push(t.clone());
    }
    println!("{} scenes with an ecosystem", read.len());
    assert!(read.iter().any(|t| t == "free09") && read.iter().any(|t| t.starts_with("nat")), "{read:?}");
}
