//! V115: a world's disc maps draw their roads and ruts. The roads, ruts and runways a freeride scene
//! lays into its ground's paint (V67, `ground_mesh.rs`) were drawn on the GPU only for a track ridden
//! alone, so in a world every disc map but for what its 128-px baked pages held (MD35's modulate, the
//! roads at four and a half feet a pixel) was a smeared wash where alone it has its road and ruts.
//!
//! Now every disc map's meshes, moved by the map's offset, are one set drawn round each eye as alone,
//! laid over the map's splat per pixel, and the baked pages give back only the light.
//!
//! What is proved: free09 inside world_mxu_free at its road spot looks like free09 alone, the ground
//! as close as free06's (the world's home map, which looked the same both ways before) — compared on
//! the middle-distance ground beside the rider at one light level, since the world lights free09 by
//! another sun (a fifth darker, and the bike's shadow falls elsewhere) — and the log names the maps whose roads are drawn. Ignored: it needs the user's
//! mods (`V115_MODS`, or `~/repos/openmx/mods`), the disc, a GPU (DISPLAY) and a release build.

use std::path::{Path, PathBuf};
use std::process::Command;

const SIZE: (usize, usize) = (960, 540);

/// A headless shot of `track` from `--trace-at`, and what the shell said.
fn shot(mods: &Path, track: &str, at: &str, out: &Path) -> String {
    let said = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .args(["--headless", "--mods", mods.to_str().unwrap(), "--track", track, "--size", &format!("{}x{}", SIZE.0, SIZE.1)])
        .args(["--overlay", "none", "--trace-at", at, "--frames", "30", "--shot", out.to_str().unwrap()])
        .output()
        .expect("the shell runs");
    let text = format!("{}{}", String::from_utf8_lossy(&said.stdout), String::from_utf8_lossy(&said.stderr));
    assert!(said.status.success(), "{track}: the shell failed:\n{text}");
    text
}

fn rgb(path: &Path) -> Vec<u8> {
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    let step = info.color_type.samples();
    assert_eq!((info.width as usize, info.height as usize), SIZE);
    buf[..info.buffer_size()].chunks(step).flat_map(|p| [p[0], p[1], p[2]]).collect()
}

/// The RMSE (0..1) of two shots over the middle distance beside the rider — rows from just under the
/// horizon to above where the bike's shadow falls, the left and right thirds — with `b`'s light
/// level: each channel of `a` scaled by the ratio of the two shots' means there. The world lights a
/// map by its own sun, not the map's (a fifth darker on free09), and that is not the paint.
fn ground_rmse(a: &[u8], b: &[u8]) -> f64 {
    let (w, h) = SIZE;
    let px: Vec<usize> = (h * 3 / 10..h * 55 / 100).flat_map(|y| (0..w / 3).chain(w * 2 / 3..w).map(move |x| y * w + x)).collect();
    let mean = |s: &[u8], c: usize| px.iter().map(|&p| f64::from(s[p * 3 + c])).sum::<f64>() / px.len() as f64;
    let gain: Vec<f64> = (0..3).map(|c| mean(b, c) / mean(a, c).max(1.0)).collect();
    let sum: f64 = px
        .iter()
        .flat_map(|&p| (0..3).map(move |c| (p, c)))
        .map(|(p, c)| ((f64::from(a[p * 3 + c]) * gain[c] - f64::from(b[p * 3 + c])) / 255.0).powi(2))
        .sum();
    (sum / (px.len() * 3) as f64).sqrt()
}

#[test]
#[ignore = "needs the user's mods (V115_MODS or ~/repos/openmx/mods) with world_mxu_free, the disc, a GPU (DISPLAY); release build"]
fn free09_in_a_world_has_its_roads_and_ruts() {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return;
    }
    let mods = std::env::var_os("V115_MODS").map(PathBuf::from).unwrap_or_else(|| Path::new(&std::env::var("HOME").unwrap()).join("repos/openmx/mods"));
    if !mods.join("tracks/world_mxu_free/mod.json").exists() {
        eprintln!("skipped: no world_mxu_free in {}", mods.display());
        return;
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("v115");
    std::fs::create_dir_all(&dir).unwrap();
    // free09 sits at 10944,5472 in the world; free06 at the origin.
    let pairs = [("free09", "1872,4500,180", "12816,9972,180"), ("free06", "2500,2500,90", "2500,2500,90")];
    let mut rmse = Vec::new();
    for (track, alone_at, world_at) in pairs {
        let (a, w) = (dir.join(format!("{track}_alone.png")), dir.join(format!("{track}_world.png")));
        shot(&mods, track, alone_at, &a);
        let said = shot(&mods, "world_mxu_free", world_at, &w);
        if !said.lines().any(|l| l == "renderer: gpu") {
            eprintln!("skipped: no GPU");
            return;
        }
        let roads = said.lines().find(|l| l.starts_with("world paint: roads and ruts of")).unwrap_or_else(|| panic!("no roads line:\n{said}"));
        assert!(roads.contains(&format!("mxu_{track}")) && !roads.contains("left out"), "{roads}");
        rmse.push(ground_rmse(&rgb(&a), &rgb(&w)));
    }
    eprintln!("ground RMSE alone vs world: free09 {:.4}, free06 {:.4}", rmse[0], rmse[1]);
    // Before V115: free09 0.0305 (the wash), free06 0.0236; after, 0.0198 and 0.0234.
    assert!(rmse[0] <= rmse[1] * 1.1, "free09's ground in the world is {:.4} from alone, free06's {:.4}", rmse[0], rmse[1]);
}
