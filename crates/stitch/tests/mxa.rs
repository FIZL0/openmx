//! openmx MD259: MXA Track Studio's `.mxatrack` read and written as a track mod. No real track
//! exists on this machine, so each test makes its package from the format (`stitch::mxa::package`).

use game_sim::track::Terrain;
use std::path::{Path, PathBuf};
use stitch::json::Json;
use stitch::jsonw::{J, o};
use stitch::mxa::{self, Track};

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("stitch-mxa").join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A track as Track Studio's defaults have it (720 × 520 feet), `n` × `rows` samples of a hill and a
/// dip below 0, its start on the left facing +x.
fn track(n: u32, rows: u32, width: f32, depth: f32) -> Track {
    let heights = (0..n * rows)
        .map(|i| {
            let (x, z) = ((i % n) as f32 / (n - 1) as f32 * width - width / 2.0, (i / n) as f32 / (rows - 1) as f32 * depth - depth / 2.0);
            12.0 * (-(x * x + z * z) / 9000.0).exp() - 3.0 + 0.01 * x
        })
        .collect();
    Track {
        id: "0f3a9c2e5b7d4e18a6c1f20b9d8e7a65".into(),
        title: "Back Forty".into(),
        author: "a rider".into(),
        environment: "Outdoor".into(),
        venue: "SX01".into(),
        ground_texture: "media/texture/dirt_01.tga".into(),
        description: "made for MD259's test".into(),
        location: "".into(),
        width,
        depth,
        resolution: n,
        rows,
        surface: 3,
        heights,
        surfaces: None,
        spawn: [-240.0, 2.0, -155.0],
        direction: [1.0, 0.0, 0.0],
    }
}

#[test]
fn a_package_reads_back_as_it_was_made() {
    let mut t = track(129, 97, 512.0, 384.0);
    let more = r#""route":[{"x":0.0,"z":0.0,"elevation":0.0,"width":30.0,"bank":0.0,"tension":0.5,"speedMph":55.0}],"props":[],"venueOffset":{"x":0.0,"y":0.0,"z":0.0}"#;
    assert_eq!(mxa::open(&mxa::package(&t, more)).unwrap(), t);
    t.surfaces = Some((0..129 * 97).map(|i| (i % 16) as u8).collect());
    assert_eq!(mxa::open(&mxa::package(&t, "")).unwrap(), t);
}

/// What MXA refuses, refused: a damaged collision, another manifest, a grid that is not the
/// document's, a document out of range, not a zip.
#[test]
fn a_broken_package_is_refused_with_why() {
    let t = track(65, 65, 256.0, 256.0);
    let good = mxa::package(&t, "");
    let refused = |b: &[u8], why: &str| {
        let e = mxa::open(b).unwrap_err();
        assert!(e.contains(why), "{e:?} does not say {why:?}");
    };
    // A byte of the collision's heights changed: the stored entry's CRC, then (CRC fixed) the hash.
    let at = good.windows(4).position(|w| w == b"MSC1").unwrap() + 80;
    let mut b = good.clone();
    b[at] ^= 1;
    refused(&b, "collision.msc: damaged in the zip");
    refused(b"not a zip", "not a zip file");
    let mut cut = good.clone();
    cut.truncate(good.len() - 1);
    refused(&cut, "not a zip file");
    let mut bad = t.clone();
    bad.environment = "Moon".into();
    refused(&mxa::package(&bad, ""), "environment");
    let mut bad = t.clone();
    bad.width = 16.0;
    refused(&mxa::package(&bad, ""), "32 to 32768");
    let mut bad = t.clone();
    bad.id = " ".into();
    refused(&mxa::package(&bad, ""), "identity");
    // A field written again after the package's own wins (as JsonUtility's last write would), so
    // the document can say other than the collision: another grid, another size, another surface.
    refused(&mxa::package(&t, r#""resolution":81"#), "its grid is not the track's");
    refused(&mxa::package(&t, r#""surface":4"#), "its grid is not the track's");
    refused(&mxa::package(&t, r#""width":300.0"#), "its bounds are not the track's");
    refused(&mxa::package(&t, r#""resolution":64"#), "65 to 8193 a side");
    refused(&mxa::package(&t, r#""schemaVersion":9"#), "project version 9");
    refused(&mxa::package(&t, r#""surface":16"#), "riding surface 16");
    // Fewer heights than the grid has.
    let mut short = t.clone();
    short.heights.truncate(1);
    refused(&mxa::package(&short, ""), "cut short");
}

/// Written out, the folder is a track mod whose terrain has the package's ground: every sample's
/// height within the terrain's height quantum, lifted by the deepest dip, the corner at 0; the scene
/// starts where the package does and faces its way.
#[test]
fn a_package_becomes_a_track_mod_with_its_ground() {
    let root = scratch("ground");
    let t = track(257, 257, 720.0, 720.0);
    let stamp = o(vec![("file", J::S("back.mxatrack".into()))]);
    let dir = mxa::write_mod(&mxa::open(&mxa::package(&t, "")).unwrap(), &root, stamp).unwrap();
    assert_eq!(dir, root.join("mxa_0f3a9c2e5b7d4e18a6c1f20b9d8e7a65"));
    let meta = Json::parse(&std::fs::read_to_string(dir.join("mod.json")).unwrap()).unwrap();
    assert_eq!(meta.get("id").and_then(Json::str), Some("mxa_0f3a9c2e5b7d4e18a6c1f20b9d8e7a65"));
    assert_eq!(meta.get("name").and_then(Json::str), Some("Back Forty"));
    assert_eq!(meta.get("kind").and_then(Json::str), Some("track"));
    assert_eq!(meta.get("mxatrack").and_then(|m| m.get("file")).and_then(Json::str), Some("back.mxatrack"));

    let (scene, raw) = stitch::openmx::track(&dir).unwrap();
    let view = trnwrite::openmx_view(&raw).unwrap().into_owned();
    let ter = Terrain::parse(&view).unwrap();
    let lift = -t.heights.iter().copied().fold(f32::INFINITY, f32::min);
    assert!(lift > 0.0);
    let scale = ter.scale.to_f32();
    assert_eq!(scale, 720.0 / 256.0);
    let top = t.heights.iter().fold(0f32, |a, h| a.max(h + lift));
    let quantum = top / 65535.0;
    let mut worst = 0f32;
    for z in 0..256 {
        for x in 0..256 {
            let q = ter.cell(x, z, 1).unwrap();
            let got = q.height[0].to_f32() * scale;
            let want = t.heights[(z * 257 + x) as usize] + lift;
            worst = worst.max((got - want).abs());
            assert_eq!(q.surface[0] & 0x7f, 3);
        }
    }
    assert!(worst <= quantum, "worst {worst} ft against a quantum of {quantum}");
    let f = |v: game_sim::track::V3| v.map(|c| c.to_f32());
    let start = f(scene.start);
    assert!((start[0] - 120.0).abs() < 0.01 && (start[2] - 205.0).abs() < 0.01, "{start:?}");
    assert!((start[1] - (2.0 + lift)).abs() < 0.01, "{start:?}");
    assert_eq!(f(scene.start_direction), [1.0, 0.0, 0.0]);

    // Written again, the folder is replaced, not added to.
    mxa::write_mod(&t, &root, J::S("again".into())).unwrap();
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
}

/// A grid whose cells are not square, or finer than the terrain can hold, is sampled again on
/// square cells; between samples as MXA's own height query does.
#[test]
fn an_uneven_grid_is_sampled_again_on_square_cells() {
    let t = track(257, 129, 720.0, 520.0);
    let laid = mxa::lay(&t);
    assert_eq!(laid.spacing, 720.0 / 256.0);
    assert_eq!((laid.width, laid.depth), (257, (520.0f32 / laid.spacing) as u32 + 1));
    // A row of the source that falls on the new grid keeps its heights; between rows, the blend.
    let row = |j: u32| &t.heights[(j * 257) as usize..(j * 257 + 257) as usize];
    for (i, h) in row(0).iter().enumerate() {
        assert_eq!(laid.feet[i], h + laid.lift);
    }
    let z = laid.spacing / (520.0 / 128.0);
    let (a, b) = (row(0)[5], row(1)[5]);
    assert!((laid.feet[257 + 5] - (a + (b - a) * z + laid.lift)).abs() < 1e-3);
    // Finer than the terrain holds: 8193 samples over 32768 feet go down to 4097.
    let fine = Track { heights: vec![1.0; 8193 * 65], ..track(65, 65, 32768.0, 32768.0) };
    let fine = Track { resolution: 8193, ..fine };
    let laid = mxa::lay(&fine);
    assert_eq!((laid.width, laid.spacing), (4097, 8.0));
    let root = scratch("uneven");
    mxa::write_mod(&t, &root, J::S("x".into())).unwrap();
}
