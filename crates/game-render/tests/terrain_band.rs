//! MD3: the renderer's `.trn` reader takes its height band from the file — the tree's root node, the
//! `height_scale` `game_sim::track::TerrainNode` reads, times the sample spacing — and not from a fit
//! to the props. Proved on every track of the disc the reader takes (the supercross tracks): the band
//! is the sim reader's to the bit, and the scene's bales stand on it a foot or so up, which is where
//! a bale's origin is. A band read wrong would put them anywhere.

use game_render::disc::Disc;
use game_render::scene::Scene;
use game_render::{Terrain, ini};

#[test]
fn the_files_band_seats_the_bales() {
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
        let scene = Scene::parse(&ini::text(&res.read(&scn).unwrap()));
        let tagged = match scene.terrain.rsplit_once('.') {
            Some((stem, ext)) => format!("{stem}.ps2.{ext}"),
            None => continue,
        };
        let Ok(bytes) = res.read_suffix(&scene.terrain).or_else(|_| res.read_suffix(&tagged)) else { continue };
        // The free and national tracks are not laid out in patches, and this reader refuses them.
        let Ok(t) = Terrain::parse(&bytes) else { continue };
        let sim = game_sim::track::Terrain::parse(&bytes).expect("the sim reads every terrain");
        let root = sim.root().expect("a root node");
        assert_eq!(t.scale.to_bits(), (root.height_scale.0 * sim.scale.0).to_bits(), "{name}");
        assert_eq!(t.base, 0.0, "{name}");
        let (bales, above) = t.props_rest(&scene).expect("bales");
        println!("{name}: a word 1/{:.0} ft; {bales} bales {above:.2} ft up", 1.0 / t.scale);
        assert!(bales > 100 && (1.0..1.8).contains(&above), "{name}: {bales} bales {above} ft above the ground");
        read += 1;
    }
    assert!(read >= 24, "only {read} tracks read");
}
