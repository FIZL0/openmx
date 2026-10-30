//! MD2: a track mod — a folder under `mods/tracks/<id>/` in openmx-mods' standard layout — lists in
//! the pick, loads and rides; a broken one is one log line and left out; and with no mods (no folder,
//! or the switch off) a disc track's ride is the same ride, hash for hash.
//!
//! The mod is written by the test: a flat two-level PLN v7 terrain of its own (256 samples a side,
//! 4 feet a sample), a scene naming it as `MDFLAT.trn` the way a disc scene names its `.ps2.trn`, and
//! one wall `.col` the scene places. No byte of it comes off a disc. The rides go through the shell's
//! own binary headless, as `tests/switch_tape.rs`'s do, and skip without the disc, since the bike, its
//! physics and the rider are the disc's.

#[path = "../src/track_mods.rs"]
#[allow(dead_code)]
mod track_mods;
// P70's pictures on the pick's cards.
#[path = "../src/track_previews.rs"]
#[allow(dead_code)]
mod track_previews;

use game_render::disc::Res;
use std::path::{Path, PathBuf};
use std::process::Command;

fn word(out: &mut Vec<u8>, w: u32) {
    out.extend_from_slice(&w.to_le_bytes());
}

fn single(out: &mut Vec<u8>, f: f32) {
    word(out, f.to_bits());
}

/// A terrain node: full relief, half a unit a raw height, the block and the children's table.
fn node(out: &mut Vec<u8>, block: u16, children_at: u32) {
    for f in [1.0, 0.5, 0.0, 0.0] {
        single(out, f);
    }
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&block.to_le_bytes());
    out.extend_from_slice(&[0; 4 + 64 + 4]);
    out.push(1);
    word(out, children_at);
}

/// A flat terrain at height 0, two levels as the disc's are: one root cell, one block all of its 16×16
/// leaves share, no palette, no texture pages, 4 feet a sample.
fn flat_terrain() -> Vec<u8> {
    flat_terrain_of(2)
}

/// The same flat terrain as a tree of `levels` (1 or 2). One level is MX vs. ATV's supercross and
/// freestyle maps' shape: the root's 16×16 cells are the leaves themselves, 256 samples a side under
/// a scene's `TerrainWidth=1` just as the two-level tree's one root cell is.
fn flat_terrain_of(levels: u32) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"PLN\0");
    out.push(b'7');
    out.extend_from_slice(b"ps2\0");
    let mut name = [0u8; 32];
    name[..6].copy_from_slice(b"mdflat");
    out.extend_from_slice(&name);
    word(&mut out, 0); // no palette
    single(&mut out, 4.0);
    word(&mut out, levels);
    let tree_slot = out.len();
    word(&mut out, 0);
    word(&mut out, 1); // the words in front of the blocks
    word(&mut out, 1); // blocks
    word(&mut out, 0);
    word(&mut out, 0); // no texture table
    word(&mut out, 0); // and no pages
    // The block: its base, 17 rows of 20 heights (17 used), a normal a sample, a byte a sample not
    // read, and the surfaces.
    word(&mut out, 0);
    out.extend(std::iter::repeat_n(0u8, 17 * 20 * 2));
    for _ in 0..289 {
        out.extend_from_slice(&[0, 127, 0]);
    }
    out.extend(std::iter::repeat_n(0u8, 289));
    out.extend(std::iter::repeat_n(0x81u8, 289));
    let tree_at = out.len() as u32;
    out[tree_slot..tree_slot + 4].copy_from_slice(&tree_at.to_le_bytes());
    if levels == 1 {
        // The root, whose 16×16 children are all the one leaf.
        let root_children = tree_at + 97;
        let leaf_at = root_children + 256 * 4;
        node(&mut out, 0, root_children);
        for _ in 0..256 {
            word(&mut out, leaf_at);
        }
        node(&mut out, 0, 0);
        return out;
    }
    // The root's one cell the scene's width of 1 covers, and its 16×16 leaves.
    let root_children = tree_at + 97;
    let cell_at = root_children + 256 * 4;
    let cell_children = cell_at + 97;
    let leaf_at = cell_children + 256 * 4;
    node(&mut out, 0, root_children);
    word(&mut out, cell_at);
    for _ in 1..256 {
        word(&mut out, 0);
    }
    node(&mut out, 0, cell_children);
    for _ in 0..256 {
        word(&mut out, leaf_at);
    }
    node(&mut out, 0, 0);
    out
}

/// A wall: two triangles in the x-y plane, 40 feet wide and 20 high, facing -z.
fn wall_col() -> Vec<u8> {
    let mut out = Vec::new();
    word(&mut out, 1234);
    out.extend_from_slice(b"col\0");
    word(&mut out, 2); // version
    word(&mut out, 0); // a mesh
    word(&mut out, 1); // its surface
    for v in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0]] {
        v.into_iter().for_each(|f| single(&mut out, f));
    }
    word(&mut out, 0); // the transform's fourth column
    word(&mut out, 4); // vertices
    word(&mut out, 1); // nodes
    word(&mut out, 2); // triangles
    word(&mut out, 0); // wide
    for v in [[-20.0, 0.0, 0.0], [20.0, 0.0, 0.0], [20.0, 20.0, 0.0], [-20.0, 20.0, 0.0]] {
        v.into_iter().for_each(|f| single(&mut out, f));
    }
    word(&mut out, 0); // no normals
    single(&mut out, 400.0);
    [0.0, 10.0, 0.0, 20.0, 10.0, 0.5].into_iter().for_each(|f| single(&mut out, f));
    out.extend_from_slice(&0x8000u16.to_le_bytes());
    out.extend_from_slice(&0x8001u16.to_le_bytes());
    for tri in [[0u16, 1, 2], [0, 2, 3]] {
        single(&mut out, 0.0);
        [0.0, 0.0, -1.0].into_iter().for_each(|f| single(&mut out, f));
        tri.into_iter().for_each(|i| out.extend_from_slice(&i.to_le_bytes()));
    }
    out
}

/// The scene, CRLF INI as the disc's: the start 300 feet in, facing +z, and the wall off to the side.
fn scene() -> String {
    [
        "[SceneInfo]",
        "SceneName=MD2 Flat",
        "DefaultStartPosition=512.00,5.00,300.00",
        "DefaultStartDirection=0.00,0.00,1.00",
        "[Environment]",
        "TerrainFile=MDFLAT.trn",
        "TerrainWidth=1",
        "TerrainBreadth=1",
        "TerrainWidthScale=4.00",
        "TerrainBreadthScale=4.00",
        "TerrainBorderWidth=0.00",
        "MinDivisionSize=16.00",
        "[Fog]",
        "ColorRGB=150,151,139",
        "[CollisionObjects]",
        "NumberOfCollisionObjects=1",
        "[Object1]",
        "COL=mdwall.col",
        "Position=700.00,0.00,500.00",
        "",
    ]
    .join("\r\n")
}

fn mod_json(id: &str) -> String {
    format!(
        r#"{{
  "standard": 1,
  "kind": "track",
  "id": "{id}",
  "name": "MD2 Flat",
  "source": {{"game": "Real elevation", "data": "generated by openmx's MD2 test", "date": "2026-10-22"}},
  "version": "1.0.0",
  "author": "crates/openmx/tests/track_mods.rs",
  "permission": "public-domain",
  "todo": []
}}"#
    )
}

fn write(root: &Path, rel: &str, bytes: impl AsRef<[u8]>) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// A good track mod, `md2_flat`, in `mods/tracks/`.
fn good(tracks: &Path, id: &str) {
    good_of(tracks, id, 2);
}

/// A good track mod whose terrain is a tree of `levels`.
fn good_of(tracks: &Path, id: &str, levels: u32) {
    let dir = tracks.join(id);
    write(&dir, "mod.json", mod_json(id));
    write(&dir, "Env/MDFLAT/MDFLAT.scn", scene());
    write(&dir, "Env/MDFLAT/MDFLAT.ps2.trn", flat_terrain_of(levels));
    write(&dir, "Col/mdwall.col", wall_col());
}

/// A `mods/` folder with the good mod, a music pack, and one of each kind of broken track mod.
fn mods_folder(name: &str) -> PathBuf {
    let mods = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md2").join(name).join("mods");
    let _ = std::fs::remove_dir_all(&mods);
    let tracks = mods.join("tracks");
    good(&tracks, "md2_flat");
    // Broken, one way each.
    good(&tracks, "md2_badjson");
    write(&tracks.join("md2_badjson"), "mod.json", "{\"standard\": 1, \"kind\": \"track\",");
    good(&tracks, "md2_noscene");
    std::fs::remove_file(tracks.join("md2_noscene/Env/MDFLAT/MDFLAT.scn")).unwrap();
    good(&tracks, "md2_twoscenes");
    write(&tracks.join("md2_twoscenes"), "Env/MDFLAT/other.scn", scene());
    good(&tracks, "md2_noterrain");
    std::fs::remove_file(tracks.join("md2_noterrain/Env/MDFLAT/MDFLAT.ps2.trn")).unwrap();
    good(&tracks, "md2_badterrain");
    write(&tracks.join("md2_badterrain"), "Env/MDFLAT/MDFLAT.ps2.trn", &flat_terrain()[..300]);
    good(&tracks, "md2_wrongid");
    write(&tracks.join("md2_wrongid"), "mod.json", mod_json("md2_other"));
    good(&tracks, "md2_later");
    write(&tracks.join("md2_later"), "mod.json", mod_json("md2_later").replace("\"standard\": 1", "\"standard\": 2"));
    good(&tracks, "md2_noversion");
    write(&tracks.join("md2_noversion"), "mod.json", mod_json("md2_noversion").replace("\"1.0.0\"", "\"one\""));
    good(&tracks, "md2_badspawn");
    write(
        &tracks.join("md2_badspawn"),
        "mod.json",
        mod_json("md2_badspawn").replace("\"todo\": []", "\"todo\": [], \"spawn\": {\"position\": [1, 2], \"direction\": [0, 0, 1], \"from\": \"x\"}"),
    );
    write(&tracks, "md2_nojson/Env/MDFLAT/MDFLAT.scn", scene());
    // A music pack is skipped quietly, wherever it is put.
    write(&tracks, "md2_music/mod.json", mod_json("md2_music").replace("\"track\"", "\"music\""));
    // And a loose file in tracks/ is not a mod.
    write(&tracks, "README.txt", "not a mod");
    mods
}

/// The mods whose `mod.json` is wrong, which the scan leaves out.
const BROKEN: [&str; 6] = ["md2_badjson", "md2_badspawn", "md2_later", "md2_nojson", "md2_noversion", "md2_wrongid"];

/// The mods whose `mod.json` is right and whose files are not (MD18): the scan reads `mod.json` only
/// and lists them, and the ride says what is wrong — [`track_mods::find`] for the folder's listing,
/// the ride's own load for the scene and the terrain.
const BROKEN_FILES: [&str; 5] = ["md2_badterrain", "md2_noscene", "md2_noterrain", "md2_twoscenes", "md2_flat_case"];

#[test]
fn a_mod_folder_lists_and_the_broken_ones_are_one_line_each() {
    let mods = mods_folder("scan");
    // Two files that differ only in case break a mod, where the file system can hold both.
    good(&mods.join("tracks"), "md2_flat_case");
    write(&mods.join("tracks/md2_flat_case"), "col/MDWALL.col", wall_col());
    let case_sensitive = std::fs::read_dir(mods.join("tracks/md2_flat_case")).unwrap().count() == 4;

    let scan = track_mods::scan(&mods);
    let ids: Vec<&str> = scan.tracks.iter().map(|t| t.id.as_str()).collect();
    let mut listed = vec!["md2_flat"];
    listed.extend(BROKEN_FILES);
    listed.sort();
    assert_eq!(ids, listed, "{:#?}", scan.log);
    let t = scan.tracks.iter().find(|t| t.id == "md2_flat").unwrap();
    assert_eq!(t.name, "MD2 Flat");
    for id in BROKEN {
        let lines: Vec<&String> = scan.log.iter().filter(|l| l.starts_with(&format!("mods: tracks/{id} left out: "))).collect();
        assert_eq!(lines.len(), 1, "{id}: {:#?}", scan.log);
    }
    assert_eq!(scan.log.len(), BROKEN.len(), "{:#?}", scan.log);
    assert!(!scan.log.iter().any(|l| l.contains("md2_music") || l.contains("README")), "{:#?}", scan.log);
    for line in &scan.log {
        println!("{line}");
    }

    // No folder: nothing, and nothing said.
    let none = track_mods::scan(&mods.join("nowhere"));
    assert!(none.tracks.is_empty() && none.log.is_empty());
    // By id, as a ride finds it; a broken one says why, an unknown one says it is not there.
    let found = track_mods::find(&mods, "md2_flat").unwrap();
    assert_eq!((&found.id, &found.name, &found.dir, found.stem.as_str()), (&t.id, &t.name, &t.dir, "MDFLAT"));
    assert!(track_mods::find(&mods, "md2_noscene").unwrap_err().contains("no .scn"));
    assert!(track_mods::find(&mods, "md2_twoscenes").unwrap_err().contains("2 .scn"));
    assert_eq!(track_mods::find(&mods, "md2_flat_case").is_err(), case_sensitive);
    // The scene and terrain are the ride's to read.
    assert!(track_mods::find(&mods, "md2_noterrain").is_ok());
    assert!(track_mods::find(&mods, "md2_badterrain").is_ok());
    for id in BROKEN {
        assert!(track_mods::find(&mods, id).is_err(), "{id}");
    }
    assert!(track_mods::find(&mods, "md2_absent").is_err());
    assert!(track_mods::find(&mods, "../md2_flat").is_err());
}

/// The folder reads as an archive does: members by name or by the end of their path, in any case
/// and with either slash, and the scene, terrain and collision readers take what it hands out.
#[test]
fn a_mod_folder_reads_as_an_archive() {
    let mods = mods_folder("res");
    let res = Res::folder(&mods.join("tracks/md2_flat")).expect("the folder opens");
    let names: Vec<&str> = res.entries().iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["Col\\mdwall.col", "Env\\MDFLAT\\MDFLAT.ps2.trn", "Env\\MDFLAT\\MDFLAT.scn", "mod.json"]);
    assert_eq!(res.read("env/mdflat/mdflat.scn").unwrap(), scene().as_bytes());
    assert_eq!(res.read_suffix("MDWALL.COL").unwrap(), wall_col());
    assert!(res.read("Env\\MDFLAT\\absent.scn").is_err());

    let scene = game_sim::track::Scene::parse(&res.read_suffix(".scn").unwrap()).unwrap();
    assert_eq!(scene.collision.len(), 1);
    let terrain = track_mods::terrain_bytes(&res, &scene.terrain_file).unwrap();
    let terrain = game_sim::track::Terrain::parse(&terrain).expect("the generated terrain parses");
    assert_eq!((terrain.levels, terrain.blocks), (2, 1));
    for (sx, sz) in [(0, 0), (128, 77), (254, 254)] {
        let cell = terrain.cell(sx, sz, 1).expect("a cell");
        assert!(cell.height.iter().all(|h| h.0 == 0.0), "{sx},{sz}");
    }
    let col = res.read_suffix(&scene.collision[0].file).unwrap();
    game_sim::track::Mesh::parse(&col).expect("the wall parses");

    // The same files handed over in memory, as the browser's storage would.
    let files = res.entries().iter().map(|e| (e.path(), res.read(&e.name).unwrap())).collect();
    let mem = Res::files(files).unwrap();
    assert_eq!(mem.read_suffix("mdflat.scn").unwrap(), scene_text_bytes());
    assert!(Res::files(vec![("a/B.col".into(), vec![1]), ("A/b.col".into(), vec![2])]).is_err());
}

fn scene_text_bytes() -> Vec<u8> {
    scene().into_bytes()
}

// ---- riding it, through the shell -----------------------------------------------------------------

/// Ride the shell headless with these arguments, or `None` with no disc to ride.
fn shell(args: &[&str]) -> Option<(bool, String)> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx")).current_dir(&root).arg("--headless").args(args).output().expect("the shell runs");
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert_ne!(out.status.code(), Some(101), "the shell panicked:\n{said}");
    Some((out.status.success(), said))
}

fn line<'a>(said: &'a str, prefix: &str) -> &'a str {
    said.lines().find(|l| l.starts_with(prefix)).unwrap_or_else(|| panic!("no `{prefix}` line:\n{said}"))
}

/// Full throttle for `frames` ticks at the engine's capped `dt`, in a file of the test's own `name`:
/// the tests run side by side, and one rewriting a shared tape truncated it under another's shell,
/// which rode an empty tape and never left the start (K17, at load ~50).
fn throttle_tape(name: &str, frames: usize) -> PathBuf {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape throttle\n");
    let report = format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17));
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md2").join(format!("throttle-{name}.tape"));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn a_track_mod_loads_and_rides_and_a_broken_one_does_not_crash() {
    let mods = mods_folder("ride");
    let tape = throttle_tape("ride", 150);
    let (mods_arg, tape) = (mods.to_str().unwrap(), tape.to_str().unwrap());
    let Some((ok, said)) = shell(&["--track", "md2_flat", "--mods", mods_arg, "--play", tape, "--frames", "600", "--steady-clock"]) else { return };
    assert!(ok, "the mod did not ride:\n{said}");
    println!("{said}");
    assert!(line(&said, "mods: md2_flat (MD2 Flat) from ").ends_with("md2_flat"));
    assert!(line(&said, "MD2 Flat: 0 props, 1 collision shapes").contains("ground 1020 x 1020 feet in 256 x 256 samples"), "\n{said}");
    assert_eq!(line(&said, "solid: "), "solid: 1 of 1 collision meshes");
    assert!(line(&said, "start: ").starts_with("start: 512, "), "\n{said}");
    // Five seconds on the throttle up +z from z = 300 goes somewhere, and stays on the ground.
    let end: Vec<f32> = line(&said, "end: ")[5..].split(", ").map(|v| v.trim().parse().unwrap()).collect();
    assert!(end[2] > 400.0 && end[1].abs() < 20.0 && (end[0] - 512.0).abs() < 100.0, "{end:?}\n{said}");
    // And the same tape on the mod is the same ride.
    let (_, again) = shell(&["--track", "md2_flat", "--mods", mods_arg, "--play", tape, "--frames", "600", "--steady-clock"]).unwrap();
    assert_eq!(line(&said, "ride hash"), line(&again, "ride hash"));

    // A broken mod asked for by name is an error, not a crash; so is a mod with the mods off.
    let (ok, said) = shell(&["--track", "md2_noterrain", "--mods", mods_arg, "--frames", "2"]).unwrap();
    assert!(!ok && said.contains("MDFLAT"), "\n{said}");
    let (ok, said) = shell(&["--track", "md2_flat", "--mods", "off", "--frames", "2"]).unwrap();
    assert!(!ok && said.contains("md2_flat"), "\n{said}");
}

/// MD18: the menu's list reads each `mod.json` and nothing else, so a folder of mods whose terrains
/// are cut down to their headers lists every one of them, and says nothing; a ride on one says why
/// it cannot, naming the terrain.
#[test]
fn mods_with_terrains_cut_to_their_headers_all_list_and_a_ride_says_why_not() {
    let mods = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md18").join("cut").join("mods");
    let _ = std::fs::remove_dir_all(&mods);
    let tracks = mods.join("tracks");
    let ids = ["md18_a", "md18_b", "md18_c"];
    for id in ids {
        good(&tracks, id);
        let trn = tracks.join(id).join("Env/MDFLAT/MDFLAT.ps2.trn");
        let bytes = std::fs::read(&trn).unwrap();
        std::fs::write(&trn, &bytes[..game_sim::track::TerrainHead::MOST.min(bytes.len())]).unwrap();
        assert!(game_sim::track::TerrainHead::parse(&std::fs::read(&trn).unwrap()).is_ok());
    }
    let scan = track_mods::scan(&mods);
    let listed: Vec<&str> = scan.tracks.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(listed, ids, "{:#?}", scan.log);
    assert!(scan.log.is_empty(), "{:#?}", scan.log);
    assert_eq!(track_mods::find(&mods, "md18_b").map(|t| t.stem), Ok("MDFLAT".to_string()));

    let Some((ok, said)) = shell(&["--track", "md18_b", "--mods", mods.to_str().unwrap(), "--frames", "2"]) else { return };
    assert!(!ok, "a terrain of its header alone rode:\n{said}");
    assert!(said.contains("MDFLAT.trn: ") && said.contains("overrun the file"), "the ride did not say the terrain is why:\n{said}");
}

/// With no mods, a disc track's ride is the ride it was: the same tape on free06 hashes the same with
/// a mods folder full of mods, with the mods off, and with no folder at all.
#[test]
fn a_disc_ride_is_the_same_with_or_without_mods() {
    let mods = mods_folder("disc");
    let tape = throttle_tape("second", 150);
    let (mods_arg, tape) = (mods.to_str().unwrap(), tape.to_str().unwrap());
    let empty = mods.parent().unwrap().join("empty");
    let ride = |m: &str| shell(&["--track", "free06", "--mods", m, "--play", tape, "--frames", "600", "--steady-clock"]);
    let Some((ok, with)) = ride(mods_arg) else { return };
    assert!(ok, "\n{with}");
    assert!(!with.contains("mods: "), "a disc ride read the mods:\n{with}");
    let (_, off) = ride("off").unwrap();
    let (_, none) = ride(empty.to_str().unwrap()).unwrap();
    assert_eq!(line(&with, "ride hash"), line(&off, "ride hash"));
    assert_eq!(line(&with, "ride hash"), line(&none, "ride hash"));
    println!("{}", line(&with, "ride hash"));
}

/// MD12: a one-level terrain under `TerrainWidth=1` (MX vs. ATV's supercross and freestyle maps) is
/// 256 samples a side, as the standard has it, not `16^levels` = 16: the ground the ride stands on
/// is as big as the two-level one's, and the rider starting 300 feet in is on the map and rides.
#[test]
fn a_one_level_terrain_is_256_samples_a_side() {
    let one = flat_terrain_of(1);
    let terrain = game_sim::track::Terrain::parse(&one).expect("the one-level terrain parses");
    assert_eq!((terrain.levels, terrain.blocks), (1, 1));
    for (sx, sz) in [(0, 0), (128, 77), (254, 254)] {
        let cell = terrain.cell(sx, sz, 1).expect("a cell");
        assert!(cell.height.iter().all(|h| h.0 == 0.0), "{sx},{sz}");
    }
    // Past the root's sixteen cells there is no tree.
    assert!(terrain.cell(256, 0, 1).is_none());
    let scene = game_sim::track::Scene::parse(scene().as_bytes()).unwrap();
    let two = flat_terrain();
    let two = game_sim::track::Terrain::parse(&two).unwrap();
    for t in [&terrain, &two] {
        let ground = game_sim::world::Ground::new(t, &scene.terrain);
        assert_eq!((ground.samples_x, ground.samples_z), (256, 256), "{} levels", t.levels);
        // The spawn, 512 feet across and 300 in, stands on the ground.
        let hit = ground.at(ps2_float::F(512.0), ps2_float::F(300.0)).expect("ground under the start");
        assert_eq!(hit.y.0, 0.0);
        assert!(ground.at(ps2_float::F(1019.0), ps2_float::F(1019.0)).is_some());
    }

    // And the shell rides it as it rides the two-level one.
    let mods = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md12").join("mods");
    let _ = std::fs::remove_dir_all(&mods);
    good_of(&mods.join("tracks"), "md12_onelevel", 1);
    let tape = throttle_tape("onelevel", 150);
    let (mods_arg, tape) = (mods.to_str().unwrap(), tape.to_str().unwrap());
    let Some((ok, said)) = shell(&["--track", "md12_onelevel", "--mods", mods_arg, "--play", tape, "--frames", "600", "--steady-clock"]) else { return };
    assert!(ok, "the one-level mod did not ride:\n{said}");
    println!("{said}");
    assert!(line(&said, "MD2 Flat: 0 props, 1 collision shapes").contains("ground 1020 x 1020 feet in 256 x 256 samples"), "\n{said}");
    assert!(line(&said, "start: ").starts_with("start: 512, "), "\n{said}");
    let end: Vec<f32> = line(&said, "end: ")[5..].split(", ").map(|v| v.trim().parse().unwrap()).collect();
    assert!(end[2] > 400.0 && end[1].abs() < 20.0 && (end[0] - 512.0).abs() < 100.0, "{end:?}\n{said}");
}

/// P70: a custom track's card picture. A mod's own `previews/track.png` is its picture; one without
/// gets a still of its map, which the shell's own binary takes headless — from over the scene's
/// start since P166 — and keeps beside `tracks/` for the next time, taken again once the mod
/// changes; a picture that does not read is one line and no picture.
#[test]
fn a_custom_track_shows_its_own_picture_or_a_still_from_its_spawn() {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return;
    }
    let mods = Path::new(env!("CARGO_TARGET_TMPDIR")).join("p70").join("mods");
    let _ = std::fs::remove_dir_all(&mods);
    let tracks = mods.join("tracks");
    for id in ["p70_own", "p70_spawn", "p70_badpng"] {
        good(&tracks, id);
    }
    let own: Vec<u8> = (0..64 * 36 * 3).map(|i| (i * 13 % 256) as u8).collect();
    write(&tracks.join("p70_own"), track_previews::OWN, game_render::png::encode_rgb(64, 36, &own));
    write(&tracks.join("p70_badpng"), track_previews::OWN, b"\x89PNG\r\n\x1a\n not really");
    let ids: Vec<String> = ["p70_badpng", "p70_own", "p70_spawn"].map(String::from).to_vec();
    let mut log = Vec::new();
    let (have, take) = track_previews::gather(&mods, &ids, &mut log);
    assert_eq!(have.len(), 1);
    assert_eq!(have[0].0, "p70_own");
    assert_eq!((have[0].1.width, have[0].1.height), (64, 36));
    assert!(have[0].1.rgba.chunks_exact(4).zip(own.chunks_exact(3)).all(|(a, b)| a[..3] == *b && a[3] == 255));
    assert_eq!(take, ["p70_spawn"], "only the one with no picture of its own is taken");
    assert_eq!(log.len(), 1, "{log:?}");
    assert!(log[0].starts_with("mods: tracks/p70_badpng's picture: "), "{log:?}");

    // The shell takes it.
    let hear = track_previews::still_job(env!("CARGO_BIN_EXE_openmx").into(), mods.clone(), take);
    let (id, got) = hear.recv_timeout(std::time::Duration::from_secs(120)).expect("the still job answers");
    assert_eq!(id, "p70_spawn");
    let still = got.expect("a still from the spawn");
    assert_eq!((still.width, still.height), track_previews::STILL_SIZE);
    // A picture of the track, not a blank: the sky above and the ground below differ.
    let row = |y: usize| still.rgba[y * still.width * 4..(y + 1) * still.width * 4].to_vec();
    assert_ne!(row(10), row(still.height - 10));
    assert!(track_previews::still(&mods, "p70_spawn").is_file());
    assert!(!mods.join("previews").join("p70_spawn.part.png").exists());
    // The scan still lists exactly the three tracks: the stills' folder is not a mod.
    let listed: Vec<String> = track_mods::scan(&mods).tracks.into_iter().map(|t| t.id).collect();
    assert_eq!(listed, ["p70_badpng", "p70_own", "p70_spawn"]);
    // Next time it is read, not taken.
    let (have, take) = track_previews::gather(&mods, &ids, &mut Vec::new());
    assert!(take.is_empty());
    assert_eq!(have.iter().find(|(i, _)| i == "p70_spawn").map(|(_, t)| t.rgba == still.rgba), Some(true));
    // Until the mod changes.
    std::thread::sleep(std::time::Duration::from_millis(20));
    write(&tracks.join("p70_spawn"), "mod.json", mod_json("p70_spawn"));
    let (_, take) = track_previews::gather(&mods, &ids, &mut Vec::new());
    assert_eq!(take, ["p70_spawn"]);
    // And a mod whose ride does not start is no still, not a crash.
    std::fs::remove_file(tracks.join("p70_spawn/Env/MDFLAT/MDFLAT.ps2.trn")).unwrap();
    let hear = track_previews::still_job(env!("CARGO_BIN_EXE_openmx").into(), mods.clone(), take);
    let (_, got) = hear.recv_timeout(std::time::Duration::from_secs(120)).expect("the still job answers");
    assert!(got.is_err());
}

/// P166: the still of a mod map is of the map. A still from the P70 days (`<id>.png`, newer than
/// its `mod.json`) is of another framing and is taken again, under the new version's name, and the
/// old one goes. The shell's `--overview` looks from [`track_previews::overview`]'s framing of the
/// map's spawn and ground, and draws no bike and no rider: from one pose, the picture over the
/// spawn with `--overview` is the picture of the same ground with the bike started far behind the
/// camera, pixel for pixel, and the picture without `--overview` is not.
#[test]
fn a_mod_maps_still_is_of_the_map_with_no_rider() {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return;
    }
    let mods = Path::new(env!("CARGO_TARGET_TMPDIR")).join("p166").join("mods");
    let _ = std::fs::remove_dir_all(&mods);
    let tracks = mods.join("tracks");
    good(&tracks, "p166_flat");
    std::thread::sleep(std::time::Duration::from_millis(20));
    let old = track_previews::still_of(&mods, "p166_flat", 1);
    write(&mods, "previews/p166_flat.png", game_render::png::encode_rgb(16, 9, &[7; 16 * 9 * 3]));
    assert_eq!(old, mods.join("previews/p166_flat.png"));
    assert_ne!(track_previews::still(&mods, "p166_flat"), old, "the new framing's still has a name of its own");
    let ids = vec!["p166_flat".to_string()];
    let (have, take) = track_previews::gather(&mods, &ids, &mut Vec::new());
    assert!(have.is_empty() && take == ids, "an old framing's still is taken again, fresh as it is");
    let hear = track_previews::still_job(env!("CARGO_BIN_EXE_openmx").into(), mods.clone(), take);
    let (_, got) = hear.recv_timeout(std::time::Duration::from_secs(120)).expect("the still job answers");
    let still = got.expect("a still over the map");
    assert_eq!((still.width, still.height), track_previews::STILL_SIZE);
    assert!(track_previews::still(&mods, "p166_flat").is_file());
    assert!(!old.exists(), "the old framing's still is gone");
    let (have, take) = track_previews::gather(&mods, &ids, &mut Vec::new());
    assert!(take.is_empty() && have.len() == 1);

    // Its camera: the framing of the scene's start (512, 5, 300 facing +z) over the flat ground at
    // 0, 1020 feet a side.
    let mods_arg = mods.to_str().unwrap();
    let size = format!("{}x{}", track_previews::STILL_SIZE.0, track_previews::STILL_SIZE.1);
    let shot = |name: &str, more: &[&str]| -> (String, game_render::pst::Texture) {
        let out = mods.join(format!("{name}.png"));
        let mut args = vec!["--track", "p166_flat", "--mods", mods_arg, "--frames", "31", "--shot-frame", "30", "--size", &size];
        args.extend(["--overlay", "none", "--markers", "off", "--minimap", "off", "--no-sound"]);
        args.extend(more);
        args.extend(["--shot", out.to_str().unwrap()]);
        let (ok, said) = shell(&args).expect("the disc is there");
        assert!(ok, "{name}:
{said}");
        (said, track_previews::load(&out).unwrap_or_else(|e| panic!("{name}: {e}")))
    };
    let (said, _) = shot("overview", &["--overview"]);
    let printed: Vec<f32> = line(&said, "overview: ")[10..].split(", ").map(|v| v.trim().parse().unwrap()).collect();
    let want = track_previews::overview([512.0, 5.0, 300.0], [0.0, 1.0], (1020.0, 1020.0), |_, _| Some(0.0));
    let want = [want.eye[0], want.eye[1], want.eye[2], want.yaw, want.pitch, want.fov];
    assert!(printed.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-3), "{printed:?} is not {want:?}");
    // Over the start, toward the middle of the map: up, behind it, looking on along +z and down.
    assert!(want[1] > 50.0 && want[2] < 300.0 && want[3].abs() < 1.0 && want[4] < -10.0, "{want:?}");

    // No rider: a pose 30 feet behind the start, looking at it.
    let pose = ["--shot-pose", "512,12,270,0,-10,60"];
    let (_, bare) = shot("pose-overview", &[&pose[..], &["--overview"]].concat());
    let (_, ridden) = shot("pose-ridden", &pose);
    let (_, away) = shot("pose-away", &[&pose[..], &["--trace-at", "100,100,180"]].concat());
    let differs = |a: &game_render::pst::Texture, b: &game_render::pst::Texture| a.rgba.iter().zip(&b.rgba).filter(|(x, y)| x != y).count();
    assert!(differs(&bare, &ridden) > 200, "the bike at the start is not in the ridden picture");
    assert_eq!(differs(&bare, &away), 0, "--overview drew something at the start");
}

/// A tape of `gas` frames on the throttle, then — with `up` — a Left press onto the spawn (P135) and
/// `up` frames with the d-pad's Up held and nothing else, then a second at rest: P132's ride away
/// and back to the start. Up is byte 2's bit 4, active low, with its pressure in byte 10; Left bit 7
/// with byte 9.
fn spawn_tape(name: &str, gas: usize, up: usize) -> PathBuf {
    let onto = if up > 0 { 2 } else { 0 };
    let mut text = format!("# game-sim pad tape: {} frames\nformat 1\ntape {name}\n", gas + onto + up + 60);
    let report = |b2: u8, b3: u8, up: u8, cross: u8| format!("0079{b2:02x}{b3:02x}7f7f7f7f0000{up:02x}000000{cross:02x}{}", "00".repeat(17));
    let (throttle, home, idle) = (report(0xff, 0xbf, 0, 0xff), report(0xef, 0xff, 0xff, 0), report(0xff, 0xff, 0, 0));
    let left = format!("00797fff7f7f7f7f00ff0000000000{}", "00".repeat(17));
    let spawn = [left, idle.clone()];
    let onto = spawn.iter().take(onto);
    for report in std::iter::repeat_n(&throttle, gas).chain(onto).chain(std::iter::repeat_n(&home, up)).chain(std::iter::repeat_n(&idle, 60)) {
        text += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md2").join(format!("{name}.tape"));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    path
}

/// The numbers off a `start: x, y, z facing ...` or `end: x, y, z` line.
fn spot(line: &str) -> Vec<f32> {
    let at = line.split_once(": ").unwrap().1.split(" facing").next().unwrap();
    at.split(", ").map(|v| v.trim().parse().unwrap()).collect()
}

/// P132: a mod map's spawn is its start too. Five seconds on the throttle carry the bike off up the
/// map; Left onto the spawn (P135) and Up held then stand it back on the mod's start, and it stays.
#[test]
fn a_mod_maps_spawn_is_its_start() {
    let mods = mods_folder("spawn");
    let tape = spawn_tape("spawn", 150, 60);
    let (mods_arg, tape) = (mods.to_str().unwrap(), tape.to_str().unwrap());
    let Some((ok, said)) = shell(&["--track", "md2_flat", "--mods", mods_arg, "--play", tape, "--frames", "600", "--steady-clock"]) else { return };
    assert!(ok, "the mod did not ride:\n{said}");
    let (start, end) = (spot(line(&said, "start: ")), spot(line(&said, "end: ")));
    // Across the ground: the start line gives the placement's height, and the bike settles onto the
    // ground under it as it does at the start of the ride.
    let off = ((start[0] - end[0]).powi(2) + (start[2] - end[2]).powi(2)).sqrt();
    assert!(end[1] <= start[1] + 1.0, "the bike is above its start: {end:?}");
    println!("md2_flat: start {start:?}, back at {end:?}, {off:.1} ft off");
    assert!(off < 3.0, "Up on the spawn did not go back to the mod's start:\n{said}");
    // And without the Up the same throttle is far off up the map.
    let gone = spawn_tape("spawn-gone", 150, 0);
    let (_, said) = shell(&["--track", "md2_flat", "--mods", mods_arg, "--play", gone.to_str().unwrap(), "--frames", "600", "--steady-clock"]).unwrap();
    assert!(spot(line(&said, "end: "))[2] - start[2] > 100.0, "\n{said}");
}

/// P113: a mod map has no `machine.txt`, and the biplane is there all the same. `--vehicle biplane`
/// starts a four-player split on it with every seat in its plane, and a tape's switch takes the
/// plane where the rider is and gives the bike back (tests/machines_everywhere.rs has the pause's
/// switch for every seat, and the spots).
#[test]
fn a_mod_map_has_the_biplane_with_no_machine_txt() {
    let mods = mods_folder("p113");
    let mods_arg = mods.to_str().unwrap();
    let Some((ok, said)) = shell(&["--track", "md2_flat", "--mods", mods_arg, "--vehicle", "biplane", "--players", "4", "--seconds", "2"]) else {
        return;
    };
    assert!(ok, "the plane did not start on the mod map:\n{said}");
    assert!(said.contains("biplane: no parked copy here"), "\n{said}");
    assert_eq!(line(&said, "vehicle: "), "vehicle: biplane");
    assert!(line(&said, "plane hash").ends_with("(biplane)"), "\n{said}");
    // The flat's ground is at 0: the plane started on it, on its wheels, not under it or over it.
    let under: f32 = line(&said, "the dirt under the biplane is at ")[33..].trim_end_matches(" feet").parse().unwrap();
    assert!(under.abs() < 1.0, "\n{said}");

    let frames = 240;
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape throttle and two switches\n");
    let report = format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17));
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    text += "switch 60 biplane\nswitch 180 bike\n";
    let tape = mods.parent().unwrap().join("switching.tape");
    std::fs::write(&tape, text).unwrap();
    let (ok, said) = shell(&["--track", "md2_flat", "--mods", mods_arg, "--play", tape.to_str().unwrap(), "--frames", "270", "--steady-clock"]).unwrap();
    assert!(ok, "\n{said}");
    let switched: Vec<&str> = said.lines().filter(|l| l.starts_with("vehicle: ")).collect();
    assert_eq!(switched, ["vehicle: bike", "vehicle: biplane", "vehicle: bike"], "\n{said}");
}

/// P122: the Mods page's music packs are `music/<id>/mod.json` of kind `music`, by id, with how many
/// songs each lists; a track in music/, a wrong id, a broken file and a nameless pack are not one.
#[test]
fn music_packs_list_from_the_music_folder() {
    let mods = Path::new(env!("CARGO_TARGET_TMPDIR")).join("p122").join("mods");
    let _ = std::fs::remove_dir_all(&mods);
    let music = mods.join("music");
    let pack = |id: &str, songs: &str| mod_json(id).replace("\"track\"", "\"music\"").replace("\"todo\": []", &format!("\"todo\": [], \"songs\": [{songs}]"));
    write(&music, "untamed_music/mod.json", &pack("untamed_music", r#"{"file": "a.vig"}, {"file": "b.vig"}"#));
    write(&music, "mxvatv_music/mod.json", &pack("mxvatv_music", r#"{"file": "a.vig"}"#));
    write(&music, "md2_track/mod.json", &mod_json("md2_track"));
    write(&music, "md2_wrongid/mod.json", &pack("md2_other", ""));
    write(&music, "md2_broken/mod.json", "{\"kind\": \"music\",");
    write(&music, "md2_noname/mod.json", &pack("md2_noname", "").replace("\"MD2 Flat\"", "\" \""));
    write(&music, "md2_nosongs/mod.json", &mod_json("md2_nosongs").replace("\"track\"", "\"music\""));
    let got = track_mods::music_packs(&mods);
    let want: Vec<(String, String, usize)> = [("md2_nosongs", 0), ("mxvatv_music", 1), ("untamed_music", 2)]
        .into_iter()
        .map(|(id, n)| (id.to_string(), "MD2 Flat".to_string(), n))
        .collect();
    assert_eq!(got, want);
    // A music pack is not a track, and no music folder is no packs.
    assert!(track_mods::scan(&mods).tracks.is_empty());
    assert!(track_mods::music_packs(&mods.join("nothing")).is_empty());
}

/// P242: a mod whose source game calls it a stadium (its `.toy`'s `[InStadium] InStadium=TRUE`, as MX
/// vs. ATV's FMX arenas and supercross stadiums all write it) has no map edge: ridden at full throttle
/// off its floor's edge, no shootoff warning starts and nobody is thrown. The same mod without the key
/// warns and throws, as free06 does.
#[test]
fn a_stadium_mod_has_no_shootoff_and_a_free_ride_mod_keeps_it() {
    let mods = Path::new(env!("CARGO_TARGET_TMPDIR")).join("p242").join("mods");
    let _ = std::fs::remove_dir_all(&mods);
    let tracks = mods.join("tracks");
    good(&tracks, "p242_open");
    good(&tracks, "p242_stadium");
    write(&tracks.join("p242_stadium"), "Env/MDFLAT/MDFLAT.toy", "[Crowd]\r\nCrowdPresent=TRUE\r\n\r\n[InStadium]\r\nInStadium=TRUE\r\n");
    // 300 feet in from the west edge, facing it: the 150-foot margin is reached in about two seconds.
    let ride = |id: &str| {
        shell(&["--track", id, "--mods", mods.to_str().unwrap(), "--trace", "200", "--trace-gas", "--trace-feel", "--trace-at", "300,512,270"])
    };
    let Some((ok, open)) = ride("p242_open") else { return };
    assert!(ok, "\n{open}");
    assert!(!open.contains("stadium: "), "\n{open}");
    assert!(open.contains("shootoff warning: ") && open.contains("thrown: tick "), "the open mod kept no edge:\n{open}");
    let (ok, stadium) = ride("p242_stadium").unwrap();
    assert!(ok, "\n{stadium}");
    assert_eq!(line(&stadium, "stadium: "), "stadium: p242_stadium has no map edge");
    assert!(!stadium.contains("shootoff warning: ") && !stadium.contains("thrown: "), "the stadium has an edge:\n{stadium}");
    // And it did ride past where the open one threw.
    let x = |l: &str| l.split_whitespace().nth(5).and_then(|v| v.parse::<f32>().ok());
    let west = stadium.lines().filter_map(x).fold(f32::MAX, f32::min);
    assert!(west < 150.0, "the stadium ride never reached the margin (west-most x {west}):\n{stadium}");
}
