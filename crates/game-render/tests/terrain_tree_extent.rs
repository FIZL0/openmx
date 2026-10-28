//! MD33: every disc map's terrain tree is exactly as big as its scene says, `TerrainWidth` ×
//! `TerrainBreadth` 256-sample units, so the cap [`game_sim::world::Ground::new`] puts on a field at its
//! tree's samples changes no disc map's ground or edge. Skips without the disc.

use game_render::disc::Disc;

#[test]
fn every_disc_maps_tree_is_its_scenes_size() {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => return println!("SKIPPED: {why}"),
    };
    let names: Vec<String> = disc
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| n.to_ascii_lowercase().starts_with("tracks\\") && n.to_ascii_lowercase().ends_with(".res"))
        .collect();
    let mut read = 0;
    for name in names {
        let Ok(res) = disc.res(&name) else { continue };
        let Some(scn) = res.entries().iter().find(|e| e.name.to_ascii_lowercase().ends_with(".scn")).map(|e| e.name.clone()) else {
            continue;
        };
        let scene = game_sim::track::Scene::parse(&res.read(&scn).unwrap()).expect("the scene reads");
        let tagged = match scene.terrain_file.rsplit_once('.') {
            Some((stem, ext)) => format!("{stem}.ps2.{ext}"),
            None => continue,
        };
        let Ok(bytes) = res.read_suffix(&scene.terrain_file).or_else(|_| res.read_suffix(&tagged)) else { continue };
        let terrain = game_sim::track::Terrain::parse(&bytes).expect("the sim reads every terrain");
        let per = game_sim::world::SAMPLES_PER_WIDTH;
        let want = [scene.terrain.width.max(1) * per, scene.terrain.breadth.max(1) * per];
        assert_eq!(terrain.samples(), Some(want), "{name}: levels {}", terrain.levels);
        read += 1;
    }
    println!("{read} maps");
    assert!(read >= 20, "only {read} maps read");
}
