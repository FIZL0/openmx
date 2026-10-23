//! The trees and bushes of a track's `.esb`, against the real disc and the original's own numbers
//! (V48).
//!
//! The species' sizes, the models' scales and the billboards' spans below were read out of the
//! original's ecosystem in save states of free09 (the first human recording's) and free06 (the
//! biplane recording's); the journal entry for V48 has how they were found.
//!
//! Needs the user's own disc, so every test skips with the reason when it is not there.

use game_render::disc::{Disc, Res};
use game_render::eco::{self, Draw, Ecosystem, Grove, Lod, Look, Part};
use game_render::mat::v3;
use game_render::scene::Scene;
use game_render::world::{Assets, Camera};
use game_sim::track;
use ps2_oracle::{Kit, addr};

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
    let mut grove = Grove::load(&archives, &look, &place.terrain, &terrain, &mut assets).expect("the .esb reads").expect("free09 has one");
    // The original's rule; the smooth one is V55's, tested below.
    grove.lod = Lod::Original;
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
    // Every plant in view is drawn by the original's rule; the ones behind the camera are left out.
    let mut checked = 0;
    for p in &e.plants {
        let seen = eco::look(camera.eye, p.pos, fov, look.eco_distance_3d, look.eco_distance_2d);
        let at = [p.pos.x, p.pos.y, p.pos.z];
        let of: Vec<&Draw> = drawn.iter().filter(|d| d.at.0[3][..3] == at).collect();
        let (model, board) = (&grove.models[p.species], &grove.billboards[p.species]);
        if of.is_empty() {
            continue;
        }
        checked += 1;
        match seen {
            Look::Model => assert!(of.len() == 1 && of[0].model == model),
            Look::Both(_) => assert!(of.len() == 2 && of[0].model == model && of[1].model == board),
            Look::Billboard => assert!(of.len() == 1 && of[0].model == board && of[0].alpha == 1.0),
        }
    }
    assert!(checked > 100, "{checked} plants in view");
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
        // Every terrain reads, the nationals' with their textures (S98), so every map has its size.
        let terrain = track::Terrain::parse(&terrain_file).unwrap_or_else(|why| panic!("{t}: the terrain: {why}"));
        let extent = eco::extent(&place.terrain, &terrain);
        let esb = archives.iter().find_map(|a| a.read_suffix(file).ok()).unwrap_or_else(|| panic!("{t}: no {file}"));
        let e = Ecosystem::parse(&esb, extent).unwrap_or_else(|why| panic!("{t}: {why}"));
        let models: std::collections::BTreeSet<&str> = e.species.iter().map(|s| s.model.as_str()).collect();
        println!("{t}: {file}, {} species, {} plants, {models:?}", e.species.len(), e.plants.len());
        read.push(t.clone());
    }
    println!("{} scenes with an ecosystem", read.len());
    assert!(read.iter().any(|t| t == "free09") && read.iter().any(|t| t.starts_with("nat")), "{read:?}");
}

/// A track loaded as the shells load it: the scene, its terrain and the grove over it.
struct Track {
    layout: track::TerrainLayout,
    terrain_file: Vec<u8>,
    grove: Grove,
}

fn track(disc: &mut Disc, name: &str) -> Track {
    let archives = archives(disc, name);
    let bytes = scn(&archives[0]).expect("the scene");
    let look = Scene::parse(&game_render::ini::text(&bytes));
    let place = track::Scene::parse(&bytes).expect("the sim's scene");
    let terrain_file = terrain_bytes(&archives[0], &place.terrain_file);
    let terrain = track::Terrain::parse(&terrain_file).expect("the terrain");
    let mut assets = Assets::default();
    let grove = Grove::load(&archives, &look, &place.terrain, &terrain, &mut assets).expect("the .esb").expect("an ecosystem");
    Track { layout: place.terrain, terrain_file, grove }
}

/// One plant as the original's ecosystem object holds it in RAM (0x1c bytes).
struct Held {
    pos: [u16; 3],
    slot: u8,
    fade: u8,
    size: [i8; 2],
    turn: [i16; 2],
}

/// The original's plants and its feet-per-unit out of a save state's RAM image, and a word reader for
/// the rest; `None` (with the reason printed) when the image is not there.
fn held(state: &str) -> Option<(Vec<u8>, usize, Vec<Held>)> {
    let kit = Kit::load().map_err(|why| println!("SKIPPED: {why}")).ok()?;
    let path = kit.ctx("states").join(state);
    let Ok(ram) = std::fs::read(&path) else {
        println!("SKIPPED: {} is not there: `./kit state ram` of its .p2s", path.display());
        return None;
    };
    let word = |a: usize| u32::from_le_bytes(ram[a..a + 4].try_into().unwrap()) as usize;
    let eco = word(addr("world_manager").unwrap() as usize) & 0x01ff_ffff;
    let (at, count) = (word(eco + 0x68) & 0x01ff_ffff, word(eco + 0xa0));
    let plants = (0..count)
        .map(|i| {
            let p = &ram[at + i * 0x1c..at + (i + 1) * 0x1c];
            let h = |o: usize| u16::from_le_bytes([p[o], p[o + 1]]);
            Held {
                pos: [h(8), h(10), h(12)],
                slot: p[14],
                fade: p[15],
                size: [p[16] as i8, p[17] as i8],
                turn: [h(20) as i16, h(22) as i16],
            }
        })
        .collect();
    Some((ram, eco, plants))
}

/// The ground under every plant of a track, by the original's point query, and the most a plant's
/// model or billboard stands off it.
fn worst_off_the_ground(t: &Track) -> (f32, f32) {
    let terrain = track::Terrain::parse(&t.terrain_file).unwrap();
    let camera = |p: game_render::mat::V3, d: f32| {
        let eye = p.add(v3(-d, 5.0, 0.0));
        Camera { eye, at: eye.add(v3(1.0, 0.0, 0.0)), fov: 0.9, near: 0.5, far: 4000.0 }
    };
    let (mut low, mut high) = (f32::MAX, f32::MIN);
    for p in &t.grove.ecosystem.plants {
        let ground = eco::ground_under(&t.layout, &terrain, p.pos);
        // Near, the model: its matrix stands the model's origin, the foot of its trunk, on the plant's
        // spot. Far, the billboard, whose bottom edge is its origin.
        for d in [20.0, 2000.0] {
            let mut drawn: Vec<Draw> = Vec::new();
            t.grove.draws(&camera(p.pos, d), 77.0, 1.0e6, &mut drawn);
            let mine = drawn.iter().find(|w| w.at.0[3][0] == p.pos.x && w.at.0[3][2] == p.pos.z).expect("drawn");
            let base = mine.at.0[3][1] - ground;
            (low, high) = (low.min(base), high.max(base));
        }
    }
    (low, high)
}

/// free06 asks for every plant of its ecosystem to be stood on the ground, and the original does it:
/// the y word it keeps for each of the 5600 is the terrain's height under it, which the file's is
/// not (up to 54 feet off). The port stands them to the same word, every one.
#[test]
fn free06s_plants_stand_on_the_ground_as_the_originals_do() {
    let Some(mut disc) = disc() else { return };
    let t = track(&mut disc, "free06");
    let e = &t.grove.ecosystem;
    assert!(e.plants.iter().all(|p| p.snap), "every plant of free06 asks to be stood");
    let Some((ram, eco_at, held)) = held("biplane.p2m2_SaveState.eeMemory.bin") else { return };
    let terrain = track::Terrain::parse(&t.terrain_file).unwrap();
    let extent = eco::extent(&t.layout, &terrain);
    let scale = f32::from_bits(u32::from_le_bytes(ram[eco_at + 0xd4..eco_at + 0xd8].try_into().unwrap()));
    assert_eq!(scale, extent / 65536.0, "the state is on free06: its feet per unit");
    assert_eq!(held.len(), e.plants.len());
    let words = |p: &eco::Plant| [p.pos.x, p.pos.y, p.pos.z].map(|v| (v / scale) as u16);
    let wrong: Vec<usize> = (0..held.len()).filter(|&i| words(&e.plants[i]) != held[i].pos).collect();
    assert!(wrong.is_empty(), "{} of {} plants stand elsewhere, the first {:?} against {:?}", wrong.len(), held.len(),
        wrong.first().map(|&i| words(&e.plants[i])), wrong.first().map(|&i| held[i].pos));

    // And so every plant's model and billboard stands on the ground the bike rides, sunk by less than
    // one 65536th of the map.
    let (low, high) = worst_off_the_ground(&t);
    println!("free06: bases from {low} to {high} feet off the ground");
    assert!(low > -0.11 && high <= 0.0, "{low}..{high}");
}

/// free09 asks for none: the original keeps the file's height for every plant, so the port does too,
/// and those heights put every plant within a fraction of a foot of the ground but a few, the
/// furthest 1.65 feet into it, the same in both.
#[test]
fn free09s_plants_keep_the_files_heights_as_the_originals_do() {
    let Some(mut disc) = disc() else { return };
    let t = track(&mut disc, "free09");
    let e = &t.grove.ecosystem;
    let (low, high) = worst_off_the_ground(&t);
    println!("free09: bases from {low} to {high} feet off the ground");
    assert!(low > -1.7 && high < 1.2, "{low}..{high}");
    let terrain = track::Terrain::parse(&t.terrain_file).unwrap();
    let off: Vec<f32> = e.plants.iter().map(|p| (p.pos.y - eco::ground_under(&t.layout, &terrain, p.pos)).abs()).collect();
    let close = off.iter().filter(|o| **o < 0.5).count();
    assert!(close * 100 >= off.len() * 98, "{close} of {} within half a foot", off.len());

    let Some((ram, eco_at, held)) = held("first_recording.p2m2_SaveState.eeMemory.bin") else { return };
    let scale = f32::from_bits(u32::from_le_bytes(ram[eco_at + 0xd4..eco_at + 0xd8].try_into().unwrap()));
    assert_eq!(scale, 0.10546875, "the state is on free09");
    assert_eq!(held.len(), e.plants.len());
    for (p, h) in e.plants.iter().zip(&held) {
        assert_eq!([p.pos.x, p.pos.y, p.pos.z], h.pos.map(|v| v as f32 * scale));
    }
}

/// The original's matrix for each plant its biplane state has near enough to be a model, rebuilt from
/// that state's own numbers the way its model item builds it (the species' 1/half.x and 0.5/half.y,
/// the size bytes, the two shorts it turned to, and the position words), against the port's.
#[test]
fn near_plants_matrices_are_the_originals() {
    let Some(mut disc) = disc() else { return };
    let t = track(&mut disc, "free06");
    let Some((ram, eco_at, held)) = held("biplane.p2m2_SaveState.eeMemory.bin") else { return };
    let word = |a: usize| u32::from_le_bytes(ram[a..a + 4].try_into().unwrap()) as usize;
    let float = |a: usize| f32::from_bits(word(a) as u32);
    let scale = float(eco_at + 0xd4);
    let table = word(eco_at + 0x94) & 0x01ff_ffff;
    let e = &t.grove.ecosystem;
    let mut checked = Vec::new();
    for (i, h) in held.iter().enumerate() {
        if h.fade >= 0x80 || h.turn == [0, 0] {
            continue;
        }
        let sp = word(table + h.slot as usize * 4) & 0x01ff_ffff;
        let (big_h, min_h, big_w, max_w) = (float(sp + 0x100), float(sp + 0x104), float(sp + 0x10c), float(sp + 0x114));
        let k = float(sp + 0x150) * (big_w + h.size[1] as f32 * (max_w - big_w) / 128.0) * 3.051851e-05;
        let (c, s) = (h.turn[0] as f32 * k, h.turn[1] as f32 * k);
        let up = float(sp + 0x14c) * (big_h + h.size[0] as f32 * (big_h - min_h) / 128.0);
        let pos = h.pos.map(|v| v as f32 * scale);
        let want = [[c, 0.0, s, 0.0], [0.0, up, 0.0, 0.0], [-s, 0.0, c, 0.0], [pos[0], pos[1], pos[2], 1.0]];
        let got = t.grove.model_at(&e.plants[i], (h.turn[0] as f32 / 32767.0, h.turn[1] as f32 / 32767.0));
        for (g, w) in got.0.iter().flatten().zip(want.iter().flatten()) {
            assert!((g - w).abs() <= 1e-5 * w.abs().max(1.0), "plant {i}: {:?}, the original's {want:?}", got.0);
        }
        checked.push(i);
    }
    println!("{} near plants' matrices: {:?}", checked.len(), &checked[..checked.len().min(8)]);
    assert!(checked.len() >= 3, "{} near plants in the state", checked.len());
}

/// nat19's ground reads, textures and all, and its plants stand on it (S98). The nationals carry their
/// ground's pictures in the terrain file, between the blocks and the tree: 32 of them on nat19, each one
/// level of 256×256 bytes, and the table in front of the blocks says where each starts.
#[test]
fn nat19s_terrain_reads_and_its_plants_stand_on_it() {
    let Some(mut disc) = disc() else { return };
    let archives = archives(&mut disc, "nat19");
    let bytes = scn(&archives[0]).expect("nat19's scene");
    let place = track::Scene::parse(&bytes).expect("the sim's scene");
    let terrain_file = terrain_bytes(&archives[0], &place.terrain_file);
    let terrain = track::Terrain::parse(&terrain_file).expect("nat19's terrain");
    assert_eq!((terrain.blocks, terrain.levels, terrain.scale.0), (538, 2, 4.5));
    assert_eq!(terrain.textures.len(), 32);
    for tex in &terrain.textures {
        assert_eq!((tex.format, tex.levels.len()), (39, 1));
        let level = tex.levels[0];
        assert_eq!((level.width, level.height, level.pixels.len()), (256, 256, 65536));
    }
    // Five root cells each way, 16² samples a root, 4.5 feet a sample.
    let extent = eco::extent(&place.terrain, &terrain);
    assert_eq!(extent, 5760.0);

    let t = track(&mut disc, "nat19");
    let e = &t.grove.ecosystem;
    let snapped = e.plants.iter().filter(|p| p.snap).count();
    let mut off: Vec<f32> = e.plants.iter().map(|p| p.pos.y - eco::ground_under(&t.layout, &terrain, p.pos)).collect();
    off.sort_by(f32::total_cmp);
    let median = off[off.len() / 2];
    println!("nat19: {} plants, {snapped} stood; above the ground least {}, median {median}, most {}", off.len(), off[0], off[off.len() - 1]);
    assert_eq!((e.plants.len(), snapped), (12785, 12785), "nat19 asks for every plant to be stood");
    assert!(off[0] > -0.09 && off[off.len() - 1] <= 0.0, "{}..{}", off[0], off[off.len() - 1]);

    // The map's size is the one the positions are 65536ths of: the plants spread over all of it and
    // none past its edge, which a wrong extent would not do.
    let (most_x, most_z) = e.plants.iter().fold((0f32, 0f32), |(x, z), p| (x.max(p.pos.x), z.max(p.pos.z)));
    println!("nat19: plants out to x {most_x}, z {most_z} of {extent}");
    assert!(e.plants.iter().all(|p| p.pos.x >= 0.0 && p.pos.z >= 0.0 && p.pos.x < extent && p.pos.z < extent));
    assert!(most_x > 0.9 * extent && most_z > 0.9 * extent, "{most_x} {most_z}");

    // And every plant's model and billboard stands on the ground the bike would ride, sunk by less
    // than one 65536th of the map, as free06's do.
    let (low, high) = worst_off_the_ground(&t);
    println!("nat19: bases from {low} to {high} feet off the ground");
    assert!(low > -0.09 && high <= 0.0, "{low}..{high}");
}

// V55: the change from model to picture, smoothed. Presentation only: none of it reaches the sim.

/// free09's scene with its fog pushed out of the way (so neither renderer's fog or reach is in the
/// picture), its props gone, and a grove of the one plant `which` of free09's ecosystem; with its
/// archives and assets.
fn lone_plant(disc: &mut Disc, which: usize, lod: Lod) -> (Scene, Assets, eco::Plant) {
    let archives = archives(disc, "free09");
    let bytes = scn(&archives[0]).expect("free09's scene");
    let mut look = Scene::parse(&game_render::ini::text(&bytes));
    let place = track::Scene::parse(&bytes).expect("the sim's scene");
    let terrain_file = terrain_bytes(&archives[0], &place.terrain_file);
    let terrain = track::Terrain::parse(&terrain_file).expect("the terrain");
    let mut assets = Assets::default();
    let whole = Grove::load(&archives, &look, &place.terrain, &terrain, &mut assets).expect("the .esb").expect("free09 has one");
    let mut e = whole.ecosystem.clone();
    let plant = e.plants[which];
    e.plants = vec![plant];
    (look.fog_near, look.fog_far, look.fog_visibility, look.fog_haziness) = (20000.0, 20000.0, 1.0, 0.0);
    look.models.clear();
    let mut grove = Grove::new(e, whole.near, whole.far, &archives, &mut assets);
    grove.match_billboards(Some(&game_render::world::Lighting::of(&look)), &archives, &mut assets);
    grove.lod = lod;
    look.ecosystem = Some(grove);
    (look, assets, plant)
}

/// The camera `d` feet from a plant along a fixed line, six feet over its foot, looking at its middle.
fn riding_at(plant: &eco::Plant, d: f32) -> Camera {
    let along = v3(0.6, 0.0, 0.8);
    let eye = plant.pos.add(v3(-along.x * d, 6.0, -along.z * d));
    let at = plant.pos.add(v3(0.0, plant.height / 2.0, 0.0));
    Camera { eye, at, fov: 0.9, near: 0.5, far: 20000.0 }
}

/// The ride: 2 feet a frame (82 mph at 60 a second) from 520 feet to 180, through every band.
const RIDE: std::ops::Range<usize> = 0..171;
fn ride_distance(frame: usize) -> f32 {
    520.0 - 2.0 * frame as f32
}

/// Each frame's change of a ride's coverage (pixels drawn) from the frame before, once the change in
/// size with distance is taken out (coverage times distance squared), as a share.
fn steps(coverage: &[usize]) -> Vec<f32> {
    let size = |f: usize| coverage[f] as f32 * ride_distance(f).powi(2);
    (0..coverage.len()).map(|f| if f == 0 { 0.0 } else { (size(f) - size(f - 1)).abs() / size(f).max(size(f - 1)).max(1.0) }).collect()
}

/// How much more than the model alone or the picture alone does on a frame one frame of the ride may
/// change a plant's drawn size by. Either alone has jumps of its own a handover cannot help — a
/// small bush's picture loses an eighth of its pixels when its texture's level changes — so a frame is
/// held to them, not to nothing.
const MOST_A_FRAME: f32 = 0.06;

/// The ways a plant is drawn on a ride: the smooth rule, the original's, its model alone and its
/// picture alone (the smooth rule with both distances out of reach, or at nothing).
const RIDES: [(&str, Lod, Option<f32>); 4] =
    [("smooth", Lod::Smooth, None), ("original", Lod::Original, None), ("model", Lod::Smooth, Some(1.0e5)), ("picture", Lod::Smooth, Some(0.0))];

/// Of a ride's four coverages (in [`RIDES`]' order), the worst amount by which a frame of a rule
/// changes more than the model and the picture alone do on that frame, for the smooth rule and the
/// original's, and where; over the bands, 250 to 400 feet off (both rules' at the camera's 66 degrees
/// across).
fn worst_over_alone(coverage: &[Vec<usize>]) -> [(f32, f32); 2] {
    let s: Vec<Vec<f32>> = coverage.iter().map(|c| steps(c)).collect();
    std::array::from_fn(|rule| {
        let mut worst = (0.0f32, 0.0f32);
        for f in RIDE.filter(|&f| (250.0..=400.0).contains(&ride_distance(f))) {
            let over = s[rule][f] - s[2][f].max(s[3][f]);
            if over > worst.0 {
                worst = (over, ride_distance(f));
            }
        }
        worst
    })
}

/// The two plants the rides are of: free09's first, a TreeC, and its first BushA.
fn ridden(disc: &mut Disc) -> Vec<usize> {
    let archives = archives(disc, "free09");
    let look = Scene::parse(&game_render::ini::text(&scn(&archives[0]).unwrap()));
    let e = Ecosystem::parse(&archives.iter().find_map(|a| a.read_suffix(look.ecosystem_file.as_deref().unwrap()).ok()).unwrap(), 6912.0).unwrap();
    vec![0, e.plants.iter().position(|p| p.species == 1).unwrap()]
}

/// A plant alone for one of [`RIDES`].
fn ride_plant(disc: &mut Disc, which: usize, (_, lod, both): (&str, Lod, Option<f32>)) -> (Scene, Assets, eco::Plant) {
    let (mut scene, assets, plant) = lone_plant(disc, which, lod);
    if let Some(d) = both {
        let grove = scene.ecosystem.as_mut().unwrap();
        (grove.near, grove.far) = (d, d);
    }
    (scene, assets, plant)
}

/// Judge the rides of one plant: the smooth rule changes no frame by more than [`MOST_A_FRAME`] over
/// what the model or the picture alone does, and the original's does.
fn judge(which: usize, coverage: &[Vec<usize>]) {
    for (c, (name, ..)) in coverage.iter().zip(RIDES) {
        println!("plant {which}, {name}: pixels {c:?}");
        assert!(c.iter().all(|&c| c > 50), "plant {which}, {name}: the plant is in the picture all the ride");
    }
    let [smooth, original] = worst_over_alone(coverage);
    println!(
        "plant {which}: worst frame over the model or picture alone, smooth {:.1}% at {} ft, original {:.1}% at {} ft",
        smooth.0 * 100.0,
        smooth.1,
        original.0 * 100.0,
        original.1
    );
    assert!(smooth.0 <= MOST_A_FRAME, "plant {which}: smooth jumps {:.1}% in a frame at {} ft", smooth.0 * 100.0, smooth.1);
    assert!(original.0 > MOST_A_FRAME, "plant {which}: the original's rule ({:.1}%) does not jump", original.0 * 100.0);
}

/// Riding toward a plant, the software renderer's picture of it changes size smoothly through the
/// model-to-picture band under the smooth rule, where the original's rule jumps.
#[test]
fn riding_toward_a_plant_the_software_picture_changes_smoothly() {
    let Some(mut disc) = disc() else { return };
    let (w, h) = (960, 720);
    for which in ridden(&mut disc) {
        let coverage: Vec<Vec<usize>> = RIDES
            .map(|ride| {
                let (scene, assets, plant) = ride_plant(&mut disc, which, ride);
                RIDE.map(|f| {
                    let mut frame = game_render::Frame::new(w, h, [0, 0, 0]);
                    game_render::draw_scene(&mut frame, &scene, &assets, &[], &riding_at(&plant, ride_distance(f)));
                    frame.depth.iter().filter(|&&z| z > 0.0).count()
                })
                .collect()
            })
            .into();
        judge(which, &coverage);
    }
}

/// The same rides on the GPU: what the plant covers, against a frame without it.
#[test]
fn riding_toward_a_plant_the_gpu_picture_changes_smoothly() {
    use game_render::gpu::{Renderer, Scene3, Target, View};
    let Some(mut disc) = disc() else { return };
    let mut gpu = match Renderer::headless() {
        Ok(r) => r,
        Err(why) => {
            println!("SKIPPED: {why}");
            return;
        }
    };
    let (w, h) = (960u32, 720u32);
    let texture = gpu.offscreen(w, h);
    let target_view = texture.create_view(&Default::default());
    gpu.set_graphics(game_render::world::Graphics::Full);
    gpu.set_sky(None);
    for which in ridden(&mut disc) {
        let coverage: Vec<Vec<usize>> = RIDES
            .map(|ride| {
                let (mut scene, assets, plant) = ride_plant(&mut disc, which, ride);
                let light = game_render::world::Lighting::of(&scene);
                let mut frame = |gpu: &mut Renderer, camera: &Camera| {
                    let views = [View { rect: [0, 0, w, h], camera, actors: &[] }];
                    let target = Target { view: &target_view, size: (w, h), frame: [0, 0, w, h], scale: 1 };
                    gpu.render(&target, &assets, &Scene3 { views: &views, poses: &[], overlay: &[], sprites: &[], light: &light });
                    gpu.read(&texture)
                };
                let grove = scene.ecosystem.take();
                gpu.set_scene(&scene);
                let empty = frame(&mut gpu, &riding_at(&plant, 300.0));
                scene.ecosystem = grove;
                gpu.set_scene(&scene);
                RIDE.map(|f| {
                    let rgb = frame(&mut gpu, &riding_at(&plant, ride_distance(f)));
                    rgb.chunks_exact(3)
                        .zip(empty.chunks_exact(3))
                        .filter(|(a, b)| a.iter().zip(*b).map(|(&a, &b)| (a as i32 - b as i32).abs()).sum::<i32>() > 6)
                        .count()
                })
                .collect()
            })
            .into();
        judge(which, &coverage);
    }
}

/// Two plants the same distance from a rider riding at them change on different frames; and of all
/// free09's plants set at one distance, no one frame of the ride changes more than a tenth of them
/// (they spread over 32 feet), where the original's rule changes them all at once.
#[test]
fn plants_at_one_distance_do_not_change_on_one_frame() {
    let Some(mut disc) = disc() else { return };
    let (mut scene, mut assets, plant) = lone_plant(&mut disc, 0, Lod::Smooth);
    // Two of the first plant, either side of the ride's line and the same distance off it.
    let mut grove = scene.ecosystem.take().unwrap();
    let mut e = grove.ecosystem.clone();
    let side = v3(0.8, 0.0, -0.6).scale(8.0);
    e.plants = vec![eco::Plant { pos: plant.pos.add(side), ..plant }, eco::Plant { pos: plant.pos.sub(side), ..plant }];
    let archives = archives(&mut disc, "free09");
    grove = Grove::new(e, grove.near, grove.far, &archives, &mut assets);
    let fov = eco::horizontal_fov(&riding_at(&plant, 0.0), 4.0 / 3.0);
    // The frame each first shows less than half its model.
    let mut changed = [None; 2];
    for f in RIDE.rev() {
        let camera = riding_at(&plant, ride_distance(f));
        let mut drawn = Vec::new();
        grove.draws(&camera, fov, f32::INFINITY, &mut drawn);
        for (k, p) in grove.ecosystem.plants.iter().enumerate() {
            let model = drawn.iter().find(|d| d.part == Part::Model && d.at.0[3][..3] == [p.pos.x, p.pos.y, p.pos.z]);
            if model.is_none_or(|d| d.cover < 0.5) && changed[k].is_none() {
                changed[k] = Some(f);
            }
        }
    }
    let [Some(a), Some(b)] = changed else { panic!("both change in the ride: {changed:?}") };
    println!("the two change at frames {a} and {b}, {} and {} feet off", ride_distance(a), ride_distance(b));
    assert!(a.abs_diff(b) >= 3, "frames {a} and {b}");

    // Every plant of free09 at one distance, closing 2 feet a frame: the frames each changes on.
    let n = 726;
    let mut on = vec![0usize; RIDE.len()];
    for i in 0..n {
        let f = RIDE.clone().find(|&f| eco::smooth_share(ride_distance(f), grove.near, grove.far, eco::jitter(i)) < 0.5);
        on[f.expect("changes in the ride")] += 1;
    }
    let most = *on.iter().max().unwrap();
    let frames = on.iter().filter(|&&c| c > 0).count();
    println!("{n} plants change over {frames} frames, at most {most} on one");
    assert!(most * 10 <= n && frames >= 12, "{most} of {n} on one frame, over {frames} frames");
}

/// The eco's draw time on free09's densest view (most plants within 600 feet ahead) is no worse under
/// the smooth rule than under the original's, on the software renderer: the grove's draws and the
/// scene they are drawn into, the best of interleaved runs.
#[test]
fn the_densest_view_draws_no_slower() {
    let Some(mut disc) = disc() else { return };
    let archives = archives(&mut disc, "free09");
    let bytes = scn(&archives[0]).expect("free09's scene");
    let mut look = Scene::parse(&game_render::ini::text(&bytes));
    let place = track::Scene::parse(&bytes).expect("the sim's scene");
    let terrain_file = terrain_bytes(&archives[0], &place.terrain_file);
    let terrain = track::Terrain::parse(&terrain_file).expect("the terrain");
    let mut assets = Assets::default();
    look.ecosystem = Grove::load(&archives, &look, &place.terrain, &terrain, &mut assets).unwrap();
    look.models.clear();
    // The view the shell's `--shot-pose 2750,160,3800,0,-3,85` looks from: north, 3 degrees down.
    let eye = v3(2750.0, 160.0, 3800.0);
    let pitch = (-3.0f32).to_radians();
    let camera = Camera { eye, at: eye.add(v3(0.0, pitch.sin(), pitch.cos())), fov: 85f32.to_radians(), near: 0.5, far: 20000.0 };
    let (w, h) = (640, 448);
    let mut best = [f64::MAX; 2];
    let mut plants = [0; 2];
    for round in 0..12 {
        for (k, lod) in [Lod::Original, Lod::Smooth].into_iter().enumerate() {
            look.ecosystem.as_mut().unwrap().lod = lod;
            let mut frame = game_render::Frame::new(w, h, [0, 0, 0]);
            let start = std::time::Instant::now();
            let tris = game_render::draw_scene(&mut frame, &look, &assets, &[], &camera);
            let ms = start.elapsed().as_secs_f64() * 1000.0;
            if round > 1 {
                best[k] = best[k].min(ms);
            }
            plants[k] = tris;
        }
    }
    println!("free09's densest view: the original's rule {:.2} ms ({} triangles), the smooth one {:.2} ms ({})", best[0], plants[0], best[1], plants[1]);
    assert!(plants[1] > 1000, "the view has its plants: {} triangles", plants[1]);
    assert!(best[1] <= best[0] * 1.10 + 0.2, "smooth {:.2} ms against the original's {:.2}", best[1], best[0]);
}
