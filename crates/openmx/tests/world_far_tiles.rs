//! V92: a world's far tiles are there before they stream in, and a streamed tile blends in. With no
//! fog (Whole Map) a world's horizon was empty past the pieces in reach of the rider, and each piece
//! that streamed in appeared whole in one frame.
//!
//! Now the whole world's ground is held at its coarsest from the load (`worlds::stand_in`, one
//! heightfield over every piece, a few MB), drawn wherever a piece is not shown yet; a piece that
//! streams in comes in unseen and is shown over [`GROUND_FADE`]'s second, a sixty-fourth of an 8x8
//! screen pattern's pixels at a time, the stand-in drawing the rest; and props and plants fade out by
//! distance before the reach they come in at.
//!
//! What is proved: a headless ride on a generated nine-tile world at Whole Map, with a reach that
//! loads only the first tile, draws ground over every tile's rectangle in its first frame, where
//! without the stand-in eight of the nine are empty; and on the GPU, a piece shown over its stand-in
//! changes the picture by at most [`MOST_A_FRAME`] of its pixels a frame over its whole fade (and as
//! little fading out), ending on the piece drawn whole, where a swap changes all of them in one
//! frame. The rides skip without the disc and the GPU half without an adapter.

use game_render::gpu::{GroundChunk, GroundPaint, GroundPiece, GroundVertex, Renderer, Scene3, StandIn, Target, View};
use game_render::mat::v3;
use game_render::world::{Assets, Camera, Lighting};
use game_sim::tiles::generate;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The shell's fade, in seconds (`GROUND_FADE` in main.rs).
const GROUND_FADE: f32 = 1.0;

/// The most of a fading piece's pixels one frame at 60 a second may change: 64 steps over a second
/// are a step or two a frame, each a sixty-fourth (1.6%) of its pixels, and a little over for the
/// pattern's cut at the piece's edges.
const MOST_A_FRAME: f32 = 0.04;

const SPACING: f32 = 9.0;
/// How near a piece is loaded, and how far it is let go: from the start line only the first tile is
/// in reach.
const LOAD: f32 = 200.0;
const DROP: f32 = 300.0;

/// Rolling ground, as MD17's world has it.
fn relief(x: f64, z: f64) -> f64 {
    40.0 + 1.5 * (x / 140.0).sin() * (z / 90.0).cos()
}

/// One piece of the world: its name, where it is, its root cells, and whether it is a tile.
struct Spec {
    name: String,
    at: [u32; 2],
    cells: (u32, u32),
    tile: bool,
}

impl Spec {
    fn rect(&self) -> [f32; 4] {
        let (w, b) = ((self.cells.0 * 16) as f32 * SPACING, (self.cells.1 * 16) as f32 * SPACING);
        [self.at[0] as f32, self.at[1] as f32, self.at[0] as f32 + w, self.at[1] as f32 + b]
    }
}

/// MD17's world: nine tiles 720 feet apart, and four joins through the gaps between them.
fn specs() -> Vec<Spec> {
    let mut out = Vec::new();
    for row in 0..3 {
        for col in 0..3 {
            out.push(Spec { name: format!("t{col}{row}"), at: [720 * col, 720 * row], cells: (4, 4), tile: true });
        }
    }
    for (i, x) in [432, 1152].into_iter().enumerate() {
        out.push(Spec { name: format!("v{i}"), at: [x, 0], cells: (3, 15), tile: false });
    }
    for (i, z) in [432, 1152].into_iter().enumerate() {
        out.push(Spec { name: format!("h{i}"), at: [0, z], cells: (15, 3), tile: false });
    }
    out
}

fn terrain(s: &Spec) -> Vec<u8> {
    let (ox, oz) = (f64::from(s.at[0]), f64::from(s.at[1]));
    let sp = f64::from(SPACING);
    generate::terrain(&s.name, s.cells, SPACING, |x, z| relief(ox + f64::from(x) * sp, oz + f64::from(z) * sp), |_, _| 1)
}

fn scene(s: &Spec) -> String {
    [
        "[SceneInfo]",
        &format!("SceneName=V92 {}", s.name),
        "DefaultStartPosition=200.00,40.00,200.00",
        "DefaultStartDirection=1.00,0.00,0.00",
        "[Environment]",
        &format!("TerrainFile={}.trn", s.name.to_uppercase()),
        &format!("TerrainWidth={}", s.cells.0),
        &format!("TerrainBreadth={}", s.cells.1),
        &format!("TerrainWidthScale={SPACING:.2}"),
        &format!("TerrainBreadthScale={SPACING:.2}"),
        "TerrainBorderWidth=0.00",
        "MinDivisionSize=96.00",
        "[Fog]",
        "ColorRGB=150,151,139",
        "",
    ]
    .join("\r\n")
}

fn write(root: &Path, rel: &str, bytes: impl AsRef<[u8]>) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// A `mods/` folder holding the world, `v92_world`.
fn mods_folder() -> PathBuf {
    let mods = Path::new(env!("CARGO_TARGET_TMPDIR")).join("v92").join("mods");
    let _ = std::fs::remove_dir_all(&mods);
    let world = mods.join("tracks").join("v92_world");
    let (mut tiles, mut joins) = (Vec::new(), Vec::new());
    for s in &specs() {
        if s.tile {
            let dir = world.join("tiles").join(&s.name);
            let stem = s.name.to_uppercase();
            write(
                &dir,
                "mod.json",
                format!(
                    r#"{{"standard": 1, "kind": "track", "id": "v92_{0}", "name": "V92 {0}", "source": {{"game": "Real elevation"}}, "version": "1.0.0", "author": "crates/openmx/tests/world_far_tiles.rs", "permission": "public-domain", "todo": []}}"#,
                    s.name
                ),
            );
            write(&dir, &format!("Env/{stem}/{stem}.scn"), scene(s));
            write(&dir, &format!("Env/{stem}/{stem}.ps2.trn"), terrain(s));
            tiles.push(format!(r#"{{"tile": "{}", "at": [{}.0, {}.0], "keep": [0, 0, 63, 63]}}"#, s.name, s.at[0], s.at[1]));
        } else {
            write(&world, &format!("joins/{0}/{0}.ps2.trn", s.name), terrain(s));
            joins.push(format!(r#"{{"join": "{}", "at": [{}.0, {}.0]}}"#, s.name, s.at[0], s.at[1]));
        }
    }
    write(
        &world,
        "mod.json",
        format!(
            r#"{{
  "standard": 1, "kind": "world", "id": "v92_world", "name": "V92 nine tiles",
  "source": {{"game": "Combined", "games": ["Real elevation"]}},
  "version": "1.0.0", "author": "crates/openmx/tests/world_far_tiles.rs", "permission": "public-domain",
  "tiles": [{}],
  "joins": [{}],
  "drop": [], "sky": "t00", "spawn": {{"tile": "t00"}}, "todo": []
}}"#,
            tiles.join(", "),
            joins.join(", ")
        ),
    );
    mods
}

/// The overhead shot: straight down (but for the degree a pose must keep) over the world's middle,
/// high enough to see all of it with the clear colour round it.
const EYE: [f32; 3] = [1080.0, 2000.0, 1080.0];
const FOV: f32 = 90.0;
const SHOT: (usize, usize) = (320, 320);

/// The shell's first frame of a ride at Whole Map, shot from [`EYE`]: what it said, and the
/// picture's RGB; `None` without the disc or a GPU.
fn first_frame(mods: &Path, name: &str, args: &[&str]) -> Option<(String, Vec<u8>)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let png = Path::new(env!("CARGO_TARGET_TMPDIR")).join("v92").join(format!("{name}.png"));
    let _ = std::fs::remove_file(&png);
    let pose = format!("{},{},{},0,-89,{FOV}", EYE[0], EYE[1], EYE[2]);
    let size = format!("{}x{}", SHOT.0, SHOT.1);
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--track", "v92_world", "--mods", mods.to_str().unwrap()])
        .args(["--size", &size, "--overlay", "none", "--graphics", "full", "--world-reach", &format!("{LOAD},{DROP}")])
        .args(["--frames", "1", "--shot-pose", &pose, "--shot", png.to_str().unwrap()])
        .args(args)
        .output()
        .expect("the shell runs");
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "the shell failed:\n{said}");
    if !said.lines().any(|l| l == "renderer: gpu") {
        eprintln!("skipped: no GPU, so no streamed ground");
        return None;
    }
    let mut d = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&png).unwrap()));
    d.set_transformations(png::Transformations::normalize_to_color8());
    let mut r = d.read_info().unwrap();
    let mut buf = vec![0; r.output_buffer_size().unwrap()];
    let info = r.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    assert_eq!((info.width as usize, info.height as usize), SHOT);
    let rgb = match info.color_type {
        png::ColorType::Rgb => buf,
        png::ColorType::Rgba => buf.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect(),
        c => panic!("a shot of {c:?}"),
    };
    Some((said, rgb))
}

/// The share of each tile's rectangle, its middle eight tenths, that is not the clear colour: the
/// shot looks down with `+x` to the right and `+z` up the picture.
fn ground_in_tiles(rgb: &[u8]) -> Vec<(String, f32)> {
    let (w, h) = (SHOT.0 as f32, SHOT.1 as f32);
    let feet_a_pixel = (EYE[1] - 40.0) * (FOV.to_radians() / 2.0).tan() / (w / 2.0);
    let clear = &rgb[..3];
    let mut out = Vec::new();
    for s in specs().iter().filter(|s| s.tile) {
        let [x0, z0, x1, z1] = s.rect();
        let (mx, mz) = ((x1 - x0) * 0.1, (z1 - z0) * 0.1);
        let px = |x: f32| (w / 2.0 + (x - EYE[0]) / feet_a_pixel) as usize;
        let py = |z: f32| (h / 2.0 - (z - EYE[2]) / feet_a_pixel) as usize;
        let (mut ground, mut all) = (0, 0);
        for y in py(z1 - mz)..py(z0 + mz) {
            for x in px(x0 + mx)..px(x1 - mx) {
                let p = &rgb[(y * SHOT.0 + x) * 3..][..3];
                all += 1;
                ground += usize::from(p.iter().zip(clear).any(|(a, b)| a.abs_diff(*b) > 6));
            }
        }
        out.push((s.name.clone(), ground as f32 / all.max(1) as f32));
    }
    out
}

#[test]
fn a_worlds_every_tile_has_ground_from_the_first_frame() {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return;
    }
    let mods = mods_folder();
    let Some((said, on)) = first_frame(&mods, "stand_in_on", &[]) else { return };
    let line = said.lines().find(|l| l.starts_with("world stand-in:")).unwrap_or_else(|| panic!("no stand-in line:\n{said}"));
    assert!(!line.contains("none"), "{line}");
    let built = said.lines().find(|l| l.contains("pieces built before the ride")).expect("the ground's line");
    assert!(built.starts_with("world ground: 1 of 13 pieces"), "the reach loads only the first tile: {built}");
    let Some((_, off)) = first_frame(&mods, "stand_in_off", &["--world-stand-in", "off"]) else { return };
    let (with, without) = (ground_in_tiles(&on), ground_in_tiles(&off));
    eprintln!("ground in each tile, with the stand-in and without: {with:?} {without:?}");
    for (name, share) in &with {
        assert!(*share > 0.99, "tile {name} is {:.1}% ground in the first frame with the stand-in", share * 100.0);
    }
    // Without it only the tile in reach has any: the test sees what the stand-in adds.
    let bare = without.iter().filter(|(_, s)| *s < 0.05).count();
    assert_eq!(bare, 8, "without the stand-in eight tiles are bare: {without:?}");
}

/// A flat square chunk of 64 cells `step` feet, its corner at `at`, every corner's normal `normal`;
/// the cells past `rows` along missing.
fn flat(at: [f32; 2], step: f32, rows: u32, normal: [f32; 3]) -> GroundChunk {
    let cells = 64;
    let vertices = (0..=cells)
        .flat_map(|j| (0..=cells).map(move |i| (i, j)))
        .map(|(i, j)| GroundVertex { pos: [at[0] + i as f32 * step, 40.0, at[1] + j.min(rows) as f32 * step], normal })
        .collect();
    let missing = if rows < cells { (0..cells * cells).map(|n| n / cells >= rows).collect() } else { Vec::new() };
    let side = cells as f32 * step;
    GroundChunk { vertices, cells, at: (0, 0), missing, centre: v3(at[0] + side / 2.0, 40.0, at[1] + side / 2.0), radius: side }
}

/// A frame of the ground from above and to the side of the piece, RGB.
fn frame(gpu: &mut Renderer) -> Vec<u8> {
    let scene = game_render::scene::Scene::parse("[Fog]\nColorRGB=61,68,76\nMinimumFogDistance=5000.00\nMaximumFogDistance=9000.00\n");
    let light = Lighting::of(&scene);
    gpu.set_scene(&scene);
    gpu.set_sky(None);
    let camera = Camera { eye: v3(576.0, 700.0, -500.0), at: v3(576.0, 40.0, 288.0), fov: 1.0, near: 0.5, far: 4000.0 };
    let (w, h) = (320u32, 180u32);
    let texture = gpu.offscreen(w, h);
    let view = texture.create_view(&Default::default());
    let views = [View { rect: [0, 0, w, h], camera: &camera, actors: &[] }];
    gpu.render(&Target { view: &view, size: (w, h), frame: [0, 0, w, h], scale: 1 }, &Assets::default(), &Scene3 { views: &views, poses: &[], overlay: &[], sprites: &[], light: &light });
    gpu.read(&texture)
}

/// How many pixels two frames differ at by more than rounding.
fn differ(a: &[u8], b: &[u8]) -> usize {
    a.chunks_exact(3).zip(b.chunks_exact(3)).filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 2)).count()
}

/// A world of two pieces side by side, 576 feet square each, with a stand-in over both: piece 1 is
/// lit one way and the stand-in another, so where each is drawn shows.
fn two_pieces(gpu: &mut Renderer) -> GroundPiece {
    let side = 576.0;
    gpu.set_ground_streamed(64, GroundPaint::Bare, [150.0, 118.0, 86.0], 4.0 * side).expect("a streamed ground");
    let step = 2.0 * side / 64.0;
    let up = [0.0, 1.0, 0.0];
    let s = StandIn {
        piece: GroundPiece::of(vec![flat([0.0, 0.0], step, 32, up)]),
        step,
        origin: [0.0, 0.0],
        cells: [64, 32],
        rects: vec![(1, [0.0, 0.0, side, side]), (2, [side, 0.0, 2.0 * side, side])],
    };
    gpu.set_ground_stand_in(s).expect("the stand-in");
    let tilted = [0.6, 0.8, 0.0];
    GroundPiece::of(vec![flat([0.0, 0.0], SPACING, 64, tilted)])
}

#[test]
fn a_piece_fades_in_over_its_stand_in_a_little_a_frame() {
    let mut gpu = match Renderer::headless() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let piece = two_pieces(&mut gpu);
    let stand_in = frame(&mut gpu);
    // It comes in unseen: the picture is the stand-in's until it is shown.
    gpu.add_ground_piece_of(1, piece).expect("the piece");
    let before = frame(&mut gpu);
    assert_eq!(differ(&stand_in, &before), 0, "a piece added is not drawn before it is shown");
    gpu.show_ground_piece(1, GROUND_FADE);
    let (mut last, mut worst, mut frames, mut changed) = (before.clone(), 0, 0, 0);
    let mut shots = Vec::new();
    for _ in 0..(GROUND_FADE * 60.0) as usize + 4 {
        gpu.advance_ground_fades(1.0 / 60.0);
        let now = frame(&mut gpu);
        let d = differ(&last, &now);
        worst = worst.max(d);
        changed += usize::from(d > 0);
        frames += 1;
        shots.push(d);
        last = now;
    }
    // Drawn whole at the end: a second piece shown at once draws the same.
    let piece_pixels = differ(&stand_in, &last);
    let whole = {
        let mut other = Renderer::headless().expect("a second renderer");
        let piece = two_pieces(&mut other);
        other.add_ground_piece_of(1, piece).expect("the piece");
        other.show_ground_piece(1, 0.0);
        frame(&mut other)
    };
    assert_eq!(differ(&whole, &last), 0, "the fade ends on the piece drawn whole");
    assert_eq!(gpu.ground_fades().0, 0, "nothing is fading after it");
    eprintln!("{piece_pixels} of the piece's pixels; per frame {shots:?}");
    assert!(piece_pixels > 5000, "the piece and its stand-in look alike on only {piece_pixels} pixels");
    let share = worst as f32 / piece_pixels as f32;
    assert!(share <= MOST_A_FRAME, "a frame changed {worst} pixels, {:.1}% of the piece's {piece_pixels}", share * 100.0);
    assert!(changed * 2 >= frames, "the fade changed the picture in only {changed} of {frames} frames");
    // And out again as gently, to the stand-in alone.
    gpu.fade_out_ground_piece(1, GROUND_FADE);
    let mut worst_out = 0;
    for _ in 0..(GROUND_FADE * 60.0) as usize + 4 {
        gpu.advance_ground_fades(1.0 / 60.0);
        let now = frame(&mut gpu);
        worst_out = worst_out.max(differ(&last, &now));
        last = now;
    }
    assert_eq!(differ(&stand_in, &last), 0, "faded out, the stand-in is all there is");
    assert!(worst_out as f32 / piece_pixels as f32 <= MOST_A_FRAME, "fading out, a frame changed {worst_out} pixels");
    assert_eq!(gpu.ground_chunks().0, 1, "the piece is let go once faded out, the stand-in's chunk kept");
}

/// What a swap would do, for the journal: the piece shown at once changes all its pixels in one
/// frame, which the fade spreads over a second.
#[test]
fn a_piece_shown_at_once_changes_every_pixel_in_one_frame() {
    let mut gpu = match Renderer::headless() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let piece = two_pieces(&mut gpu);
    let stand_in = frame(&mut gpu);
    gpu.add_ground_piece_of(1, piece).expect("the piece");
    gpu.show_ground_piece(1, 0.0);
    let whole = frame(&mut gpu);
    let d = differ(&stand_in, &whole) as f32;
    assert!(d > 5000.0 && d * MOST_A_FRAME > 200.0, "a swap changes {d} pixels in one frame");
}

/// Full throttle and nothing else, `frames` long: Cross down, at full pressure.
fn throttle_tape(frames: usize) -> String {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape full throttle\n");
    let report = format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17));
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    text
}

/// The user's world_ultra ridden 30 s on the gas from its start, headless at 1920x1080 with
/// `--timings` as MD28 measured it: no frame's streaming (the `stream` stage, the tiles' and the
/// ground's and the dressing's) takes a 60 Hz frame, and it never waits for the piece underfoot
/// (V94). Ignored: it needs the user's mods (`V92_MODS`,
/// or `~/repos/openmx/mods`) and a quiet machine, since a frame's stages are wall-clock time.
#[test]
#[ignore]
fn world_ultras_gas_ride_streams_inside_a_frame() {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return;
    }
    let mods = std::env::var_os("V92_MODS").map(PathBuf::from).unwrap_or_else(|| Path::new(&std::env::var("HOME").unwrap()).join("repos/openmx/mods"));
    if !mods.join("tracks/world_ultra/mod.json").exists() {
        eprintln!("skipped: no world_ultra in {}", mods.display());
        return;
    }
    let tape = Path::new(env!("CARGO_TARGET_TMPDIR")).join("v92").join("gas.tape");
    write(tape.parent().unwrap(), "gas.tape", throttle_tape(900));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--size", "1920x1080", "--mods", mods.to_str().unwrap(), "--track", "world_ultra"])
        .args(["--timings", "--play", tape.to_str().unwrap(), "--frames", "900"])
        .output()
        .expect("the shell runs");
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "the shell failed:\n{said}");
    let stream = |l: &str| l.split("stream ").nth(1).and_then(|r| r.split(' ').next()).and_then(|v| v.parse::<f32>().ok());
    let slow: Vec<&str> = said.lines().filter(|l| l.contains("slow frame") && stream(l).is_some_and(|s| s > 16.7)).collect();
    eprintln!("{}", said.lines().filter(|l| l.starts_with("session") || l.starts_with("world stand-in")).collect::<Vec<_>>().join("\n"));
    assert!(slow.is_empty(), "{} frames streamed for more than 16.7 ms:\n{}", slow.len(), slow.join("\n"));
    // V94: the piece the rider rides onto was built before it got there, the one ahead first. A
    // wait is the stream holding the frame for the piece underfoot.
    let built = said.lines().filter(|l| l.starts_with("world ground:") && l.contains(" built, ")).count();
    let waited: Vec<&str> = said.lines().filter(|l| l.starts_with("world ground: waited")).collect();
    eprintln!("{built} pieces built, {} waits", waited.len());
    assert!(waited.is_empty(), "the ride waited for the piece underfoot:\n{}", waited.join("\n"));
}
