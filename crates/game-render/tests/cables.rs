//! The wires between poles, against the real disc and against the original's own numbers (V41).
//!
//! Each placement's matrix and each model's bounds below were read out of the drawn instances in save
//! states of the original: free09's in the first human recording's state, free06's in the biplane
//! recording's. The journal entry for V41 has how they were found.
//!
//! Needs the user's own disc, so every test skips with the reason when it is not there.

use game_render::disc::{Disc, Res};
use game_render::mat::{M4, V3, v3};
use game_render::scene::{Placement, Scene};
use game_render::world::Assets;

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

fn scene(archives: &[Res], name: &str) -> Scene {
    let res = &archives[0];
    let want = format!("{name}.scn");
    let member = res
        .entries()
        .iter()
        .find(|e| e.name.to_ascii_lowercase().ends_with(&want))
        .unwrap_or_else(|| panic!("no {want}"))
        .name
        .clone();
    Scene::parse(&game_render::ini::text(&res.read(&member).expect("the scene")))
}

fn at(scene: &Scene, model: &str, pos: V3) -> Placement {
    scene
        .models
        .iter()
        .find(|p| p.model == model && p.pos.sub(pos).len() < 0.01)
        .unwrap_or_else(|| panic!("no {model} at {pos:?}"))
        .clone()
}

fn rows_match(got: &M4, want: [[f32; 4]; 4], what: &str) {
    for (r, (g, w)) in got.0.iter().zip(want.iter()).enumerate() {
        for c in 0..4 {
            assert!((g[c] - w[c]).abs() < 1e-3, "{what}: row {r} is {g:?}, the original's {w:?}");
        }
    }
}

/// The bounds of what the model draws, in its own space.
fn drawn_bounds(assets: &Assets, model: &str) -> (V3, V3) {
    let loaded = &assets.models[model];
    let mut lo = v3(f32::MAX, f32::MAX, f32::MAX);
    let mut hi = v3(f32::MIN, f32::MIN, f32::MIN);
    for t in &loaded.tris {
        for v in &t.v {
            lo = v3(lo.x.min(v.pos.x), lo.y.min(v.pos.y), lo.z.min(v.pos.z));
            hi = v3(hi.x.max(v.pos.x), hi.y.max(v.pos.y), hi.z.max(v.pos.z));
        }
    }
    (lo, hi)
}

/// The box the original kept for the instance, as centre and half size, against what the port draws.
fn bounds_match(assets: &Assets, model: &str, centre: V3, half: V3) {
    let (lo, hi) = drawn_bounds(assets, model);
    let (got_centre, got_half) = (lo.add(hi).scale(0.5), hi.sub(lo).scale(0.5));
    println!("{model}: drawn from {lo:?} to {hi:?}");
    for (g, w, axis) in [
        (got_centre.x, centre.x, "centre x"),
        (got_centre.y, centre.y, "centre y"),
        (got_centre.z, centre.z, "centre z"),
        (got_half.x, half.x, "half x"),
        (got_half.y, half.y, "half y"),
        (got_half.z, half.z, "half z"),
    ] {
        assert!((g - w).abs() < 0.02, "{model}: {axis} is {g}, the original's {w}");
    }
}

/// free09's telephone wire: one 200 ft span from crossarm to crossarm, sagging 8.1 ft at the middle,
/// a three-sided tube 0.26 ft across, placed with the scene's look and up so a span that climbs a hill
/// climbs with it. The matrix the port places it with is the one the original drew it with, and
/// nothing else: the file's root turns the sag sideways, and the original does not draw that turn.
#[test]
fn free09s_telephone_wires_are_drawn_as_the_original_draws_them() {
    let Some(mut disc) = disc() else { return };
    let archives = archives(&mut disc, "free09");
    let scene = scene(&archives, "free09");
    let mut assets = Assets::default();
    assert!(assets.load_placed(&archives, "Free09_PowerLine"), "{:?}", assets.missing());

    // A level span, and one that runs 68.6 ft down a slope off the end of a line.
    let level = at(&scene, "Free09_PowerLine", v3(3119.264_9, 201.615_94, 2325.564_2));
    rows_match(
        &level.transform(),
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, -1.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [3119.265, 201.616, 2325.564, 1.0],
        ],
        "the level span",
    );
    let sloped = at(&scene, "Free09_PowerLine", v3(4519.141_6, 201.615_94, 2325.564_2));
    rows_match(
        &sloped.transform(),
        [
            [0.939_271, -0.343_176, 0.0, 0.0],
            [0.0, 0.0, -1.0, 0.0],
            [0.343_176, 0.939_271, 0.0, 0.0],
            [4519.142, 201.616, 2325.564, 1.0],
        ],
        "the sloped span",
    );

    // The instance's own box in the original: the span, the sag and the thickness, all from the file.
    bounds_match(&assets, "Free09_PowerLine", v3(100.0, 0.0, -3.992), v3(100.009, 0.130, 4.141));

    // Drawn, the level span leaves its pole at the crossarm and reaches the next pole 200 ft on, at
    // the same height, and hangs lowest halfway.
    let (lo, hi) = drawn_bounds(&assets, "Free09_PowerLine");
    let m = level.transform();
    let start = m.point(v3(lo.x, 0.0, 0.0));
    let end = m.point(v3(hi.x, 0.0, 0.0));
    assert!(start.sub(v3(3119.265, 201.616, 2325.564)).len() < 0.05, "starts at {start:?}");
    assert!(end.sub(v3(3319.247, 201.616, 2325.564)).len() < 0.05, "ends at {end:?}");
    let bottom = m.point(v3(100.0, 0.0, lo.z));
    assert!((bottom.y - (201.616 - 8.13)).abs() < 0.05, "hangs to {bottom:?}");
    let poles: Vec<V3> = scene
        .models
        .iter()
        .filter(|p| p.model == "Free09_TelephonePole")
        .map(|p| p.pos)
        .collect();
    let next = poles
        .iter()
        .map(|&p| v3(p.x - end.x, 0.0, p.z - end.z).len())
        .fold(f32::MAX, f32::min);
    assert!(next < 15.0, "the span ends {next} ft from the nearest pole");
}

/// free06's high-tension line: eight models strung between its pylons, named `Free06_Power Line_A`
/// and so on in the scene while the files are `free06_powerline_a.psg`. The original's text reader
/// drops the space, so it draws all 18; the port asked the disc for a name with a space in it, found
/// nothing, and drew the pylons bare.
#[test]
fn free06s_high_tension_wires_are_strung_where_the_originals_are() {
    let Some(mut disc) = disc() else { return };
    let archives = archives(&mut disc, "free06");
    let scene = scene(&archives, "free06");
    let mut assets = Assets::default();
    let lines: Vec<&Placement> =
        scene.models.iter().filter(|p| p.model.starts_with("Free06_PowerLine_")).collect();
    assert_eq!(lines.len(), 18, "the scene's high-tension spans");
    for p in &lines {
        assert!(assets.load_placed(&archives, &p.model), "{} does not load: {:?}", p.model, assets.missing());
    }

    let first = at(&scene, "Free06_PowerLine_A", v3(4235.519, 260.543_88, 4304.064_5));
    rows_match(
        &first.transform(),
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [4235.519, 260.544, 4304.064, 1.0],
        ],
        "the first span",
    );
    bounds_match(&assets, "Free06_PowerLine_A", v3(6.852, 3.352, -14.884), v3(172.997, 10.131, 100.064));

    // The wires hang from the pylons' arms. The pylon's file carries a 60° turn in its root that the
    // original does not draw; with it, the arms stood a median 10 ft off the wires.
    assert!(assets.load_placed(&archives, "MM_HTTower"), "{:?}", assets.missing());
    let towers: Vec<&Placement> = scene.models.iter().filter(|p| p.model == "MM_HTTower").collect();
    assert_eq!(towers.len(), 7, "the scene's pylons");
    let arms: Vec<V3> = towers
        .iter()
        .flat_map(|p| {
            let m = p.transform();
            assets.models["MM_HTTower"].tris.iter().flat_map(move |t| t.v.iter().map(move |v| m.point(v.pos)))
        })
        .collect();
    let mut off: Vec<f32> = lines
        .iter()
        .flat_map(|p| {
            let m = p.transform();
            assets.models[&p.model].tris.iter().flat_map(move |t| t.v.iter().map(move |v| m.point(v.pos)))
        })
        .filter(|w| towers.iter().any(|t| v3(t.pos.x - w.x, 0.0, t.pos.z - w.z).len() < 40.0))
        .map(|w| arms.iter().map(|a| a.sub(w).len()).fold(f32::MAX, f32::min))
        .collect();
    off.sort_by(f32::total_cmp);
    assert!(off.len() > 100, "only {} wire points by a pylon", off.len());
    let median = off[off.len() / 2];
    println!("{} wire points by a pylon, a median {median} ft from it", off.len());
    assert!(median < 1.0, "the wires pass a median {median} ft from the pylons' arms");
}

/// A prop the exporter left turned in its file is drawn by the scene's placement alone: free10's
/// telephone wires each carry their own placement's turn in their root, and drawn with both they
/// swung off the poles they span.
#[test]
fn a_props_own_root_turn_is_not_drawn_on_top_of_its_placement() {
    let Some(mut disc) = disc() else { return };
    let archives = archives(&mut disc, "free10");
    let scene = scene(&archives, "free10");
    let mut assets = Assets::default();
    let wire = at_first(&scene, "Wire03");
    assert!(assets.load_placed(&archives, "Wire03"), "{:?}", assets.missing());
    let root = assets.models["Wire03"].model.nodes[0].local;
    rows_match(&root, M4::IDENTITY.0, "Wire03's root once placed");
    // The file's own root is the placement's turn, which is how the exporter came to leave it there.
    let mut raw = Assets::default();
    assert!(raw.load(&archives, "Wire03"));
    let file_root = raw.models["Wire03"].model.nodes[0].local;
    let mut turn = wire.transform();
    turn.0[3] = [0.0, 0.0, 0.0, 1.0];
    rows_match(&file_root, turn.0, "Wire03's root in its file");
}

fn at_first(scene: &Scene, model: &str) -> Placement {
    scene.models.iter().find(|p| p.model == model).unwrap_or_else(|| panic!("no {model}")).clone()
}

/// No placed prop is left undrawn for want of its file, on any track the port loads: every model a
/// scene names, read the way the original reads the name, is on the disc in the track's archives.
#[test]
fn every_prop_a_scene_places_is_on_the_disc() {
    let Some(mut disc) = disc() else { return };
    let tracks: Vec<String> = disc
        .entries()
        .iter()
        .filter_map(|e| {
            let n = e.name.to_ascii_lowercase();
            let stem = n.rsplit('\\').next()?.strip_suffix(".res")?.to_string();
            let track = stem.starts_with("free") || stem.starts_with("nat") || stem.starts_with("sx");
            (track && !stem.ends_with("_2")).then_some(stem)
        })
        .collect();
    assert!(tracks.len() >= 50, "only {} tracks found: {tracks:?}", tracks.len());
    let mut missing = Vec::new();
    for track in &tracks {
        let archives = archives(&mut disc, track);
        let scene = scene(&archives, track);
        let mut names: Vec<&str> = scene.models.iter().map(|p| p.model.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        for name in names {
            let file = format!("{name}.psg");
            if !archives.iter().any(|a| a.read_suffix(&file).is_ok()) {
                missing.push(format!("{track}: {file}"));
            }
        }
    }
    println!("{} tracks, missing {missing:?}", tracks.len());
    assert!(missing.is_empty(), "props with no file: {missing:?}");
}
