//! V72: every track's sun flares off the disc — how many each `.toy` lists, that each one's picture is
//! in the track's archive, and the settings the original's reader leaves it with.
//!
//! The disc is not in the repo: without it the test says so and passes.

use game_render::{Disc, ini, sun_flare};

#[test]
fn every_tracks_flares() {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            eprintln!("skipped: {why}");
            return;
        }
    };
    let none = ["free10", "nat08", "nat11", "nat19", "sx04", "sx06", "sx10", "sx15", "sx16", "sx19", "sx22", "sx24"];
    // No useToyFlareSettings, occlusionSampleSize, flareAlpha or flareScreenSize: the reader's own.
    let defaults = ["sx14", "sx21", "sx23"];
    // Every track archive with a `.toy` of its own: the 51 tracks and the freeride areas' `_ss`
    // scenes, which carry their area's flare.
    let mut members: Vec<String> = disc
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| n.starts_with("Tracks\\") && n.ends_with(".res"))
        .collect();
    members.sort();
    members.dedup();
    let mut tracks = Vec::new();
    for member in members {
        let res = disc.res(&member).unwrap();
        let stem = member["Tracks\\".len()..member.len() - ".res".len()].to_string();
        if let Ok(toy) = res.read_suffix(&format!("\\{stem}.toy")) {
            tracks.push((stem, res, toy));
        }
    }
    assert_eq!(tracks.len(), 56, "{:?}", tracks.iter().map(|t| &t.0).collect::<Vec<_>>());
    let mut drawn = 0;
    for (track, res, toy) in &tracks {
        let flares = sun_flare::flares(&ini::text(toy));
        let area = track.strip_suffix("_ss").unwrap_or(track);
        let want = if none.contains(&area) { 0 } else { 1 };
        assert_eq!(flares.len(), want, "{track}");
        for f in &flares {
            let picture = res.read_suffix(&format!("\\{}.pst", f.texture)).unwrap_or_else(|e| panic!("{track}: {e}"));
            game_render::pst::Texture::parse(&picture).unwrap();
            // The sun is up, and FlareRGBA is never the colour.
            assert!(f.light.y < 0.0, "{track}: {:?}", f.light);
            assert_eq!(f.color, [128.0; 4], "{track}");
            if defaults.contains(&track.as_str()) {
                assert_eq!((f.sample_size, f.alpha, f.screen_size), (16.0, 40.0, 350.0), "{track}");
            } else {
                assert_eq!(f.sample_size, 26.0, "{track}");
                assert!((30.0..=60.0).contains(&f.alpha) && (130.0..=250.0).contains(&f.screen_size), "{track}: {f:?}");
            }
            drawn += 1;
        }
    }
    // 39 tracks and four freeride `_ss` scenes (free10_ss has none, as free10).
    assert_eq!(drawn, 43);
}
