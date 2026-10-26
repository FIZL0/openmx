//! V84: replaced textures, opentennis's lookup. A key-named PNG in `mods/texture-replacements/` is
//! drawn in place of the texture it names with Upscaled Textures on and not with it off, at its own
//! size with the disc texture's tiling; a texture pack beats it, stacked by settings.txt's order and
//! off list; a track mod's own `textures/upscaled/` replaces the `.pst` it carries; `--dump-textures`
//! names a texture by the same key every run; and `tools/upscale_mods.py --check` says what a mod
//! still needs, without chaiNNer.
//!
//! Every texture here is a hand-made `.pst`; no byte comes off a disc, so nothing skips.

#[path = "../src/track_mods.rs"]
#[allow(dead_code)]
mod track_mods;
#[path = "../src/textures.rs"]
#[allow(dead_code)]
mod textures;

use game_render::disc::Res;
use game_render::pst::Texture;
use game_render::world::decode_texture;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

/// The renderer's source is one per process: the tests that install one take turns.
static SOURCE: Mutex<()> = Mutex::new(());

/// A `w` x `h` 8-bit `.pst`, one mip, its pixels a checker of palette entries 0 and 1 (`a` and `b`).
fn pst(w: u32, h: u32, a: [u8; 3], b: [u8; 3]) -> Vec<u8> {
    let mut d = vec![0u8; 8 + 1024];
    // Entries 0 and 1 are where they are (the swizzle moves 8..24), at the GS's "opaque", 0x80.
    d[8..12].copy_from_slice(&[a[0], a[1], a[2], 0x80]);
    d[12..16].copy_from_slice(&[b[0], b[1], b[2], 0x80]);
    for v in [1, h, w, w * h] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    d.extend_from_slice(&[0; 4]);
    d.extend((0..w * h).map(|i| ((i % w + i / w) % 2) as u8));
    d
}

fn solid(w: usize, h: usize, c: [u8; 4]) -> Texture {
    Texture { width: w, height: h, rgba: c.repeat(w * h) }
}

/// A fresh folder for one test.
fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("v84").join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(root: &Path, rel: &str, bytes: impl AsRef<[u8]>) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn png(root: &Path, rel: &str, t: &Texture) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    textures::write_png(&path, t).unwrap();
}

fn key(file: &[u8]) -> String {
    game_render::replace::key(&game_render::world::decode_disc_texture("a.pst", file).expect("the .pst decodes"))
}

/// The disc's 4x4 checker, replaced by a 16x16 of four quadrant colours: drawn, at 16x16, with the
/// setting on, and the disc's with it off or with no mods folder. A tiled face sampling it at
/// `u = 2.375` reads what it reads at `0.375`, the replacement's own texel there: the UVs are the
/// disc's, not scaled by the replacement's size (opentennis B222).
#[test]
fn a_key_named_file_is_drawn_with_the_setting_on_and_keeps_its_tiling() {
    let _turn = SOURCE.lock().unwrap_or_else(|e| e.into_inner());
    let mods = scratch("replacements").join("mods");
    let disc = pst(4, 4, [10, 20, 30], [200, 210, 220]);
    let mut big = solid(16, 16, [0, 0, 0, 255]);
    for y in 0..16 {
        for x in 0..16 {
            let c = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [255, 255, 0, 255]][(y / 8) * 2 + x / 8];
            big.rgba[4 * (y * 16 + x)..][..4].copy_from_slice(&c);
        }
    }
    png(&mods, &format!("texture-replacements/some/folder/rock-{}.png", key(&disc)), &big);
    // A file whose name has no key, and one with a key nothing has, change nothing.
    png(&mods, "texture-replacements/rock.png", &solid(2, 2, [1, 1, 1, 255]));
    png(&mods, "texture-replacements/rock-0123456789abcdef.png", &solid(2, 2, [1, 1, 1, 255]));

    let line = textures::install(Some(&mods), true, &[], &[]);
    assert_eq!(line, "textures: 0 from 0 texture packs, 2 key-named in mods/texture-replacements, 0 mods' own upscales");
    let drawn = decode_texture("rock.pst", &disc).unwrap();
    assert_eq!((drawn.width, drawn.height), (16, 16));
    assert_eq!(drawn.rgba, big.rgba);
    for (u, v) in [(0.375, 0.125), (0.875, 0.625), (0.125, 0.875)] {
        assert_eq!(drawn.sample(u + 2.0, v - 3.0), drawn.sample(u, v), "tiled at {u}, {v}");
        assert_eq!(drawn.sample(u, v), big.sample(u, v));
    }
    // Another texture is the disc's.
    let other = pst(4, 4, [1, 2, 3], [4, 5, 6]);
    assert_eq!(decode_texture("other.pst", &other).unwrap().rgba, game_render::world::decode_disc_texture("other.pst", &other).unwrap().rgba);

    for (on, mods) in [(false, Some(mods.as_path())), (true, None)] {
        let line = textures::install(mods, on, &[], &[]);
        let drawn = decode_texture("rock.pst", &disc).unwrap();
        assert_eq!((drawn.width, drawn.height, &drawn.rgba[..3]), (4, 4, &[10, 20, 30][..]), "{line}");
        assert!(!game_render::replace::on());
    }
    assert_eq!(textures::install(Some(&mods), false, &[], &[]), "textures: the disc's (Upscaled Textures off)");
}

/// Two texture packs and a key-named file all name the same texture: a pack is drawn over the file;
/// of the packs, one settings.txt does not order is on top (folder order among those), then the
/// order's; one in the off list is not read at all; and with both off the file is drawn.
#[test]
fn a_texture_pack_beats_the_key_named_files_in_settings_order() {
    let _turn = SOURCE.lock().unwrap_or_else(|e| e.into_inner());
    let mods = scratch("packs").join("mods");
    let disc = pst(8, 8, [10, 20, 30], [40, 50, 60]);
    let k = key(&disc);
    let pack = |folder: &str, shade: u8| {
        write(&mods, &format!("{folder}/mod.json"), "{\"standard\": 1, \"kind\": \"textures\", \"id\": \"x\"}");
        png(&mods, &format!("{folder}/textures/a/rock-{k}.png"), &solid(4, 4, [shade, 0, 0, 255]));
    };
    pack("hd_a", 1);
    pack("packs/hd_b", 2);
    png(&mods, &format!("texture-replacements/rock-{k}.png"), &solid(4, 4, [3, 0, 0, 255]));
    // A pack with no key-named file is left out.
    write(&mods, "hd_empty/mod.json", "{\"kind\": \"textures\"}");
    write(&mods, "hd_empty/textures/readme.txt", "nothing");

    let names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let drawn = |order: &[&str], off: &[&str]| {
        let line = textures::install(Some(&mods), true, &names(order), &names(off));
        (decode_texture("rock.pst", &disc).unwrap().rgba[0], line)
    };
    let (shade, line) = drawn(&[], &[]);
    assert_eq!(shade, 1);
    assert_eq!(line, "textures: 1 from 2 texture packs, 1 key-named in mods/texture-replacements, 0 mods' own upscales");
    assert_eq!(drawn(&["packs/hd_b", "hd_a"], &[]).0, 2);
    assert_eq!(drawn(&["hd_a", "packs/hd_b"], &[]).0, 1);
    assert_eq!(drawn(&["hd_a"], &[]).0, 2, "an unordered pack is on top");
    assert_eq!(drawn(&["packs/hd_b", "hd_a"], &["packs/hd_b"]).0, 1);
    let (shade, line) = drawn(&[], &["hd_a", "packs/hd_b"]);
    assert_eq!(shade, 3);
    assert!(line.starts_with("textures: 0 from 0 texture packs, 1 key-named"), "{line}");
    let names: Vec<String> = textures::packs(&mods, &[], &[]).into_iter().map(|p| p.folder).collect();
    assert_eq!(names, ["hd_a", "packs/hd_b"]);
    textures::install(None, true, &[], &[]);
}

/// A track mod carrying its own `.pst`: `--dump-textures --from` writes it, under the same name on two
/// runs; an upscale put under that name in the mod's `textures/upscaled/` is what the track draws
/// (read from the mod's folder as a ride reads it), and the dump leaves the upscales out.
#[test]
fn a_track_mods_own_upscale_replaces_its_pst_and_the_dump_keys_it_the_same_twice() {
    let root = scratch("track");
    let mods = root.join("mods");
    let track = mods.join("tracks/v84_mod");
    write(&track, "mod.json", "{\"standard\": 1, \"kind\": \"track\", \"id\": \"v84_mod\"}");
    let file = pst(8, 4, [90, 91, 92], [93, 94, 95]);
    write(&track, "Tex/Gate.pst", &file);
    write(&track, "Tex/Flag.atm", [&1u32.to_le_bytes()[..], &1u32.to_le_bytes(), &0u32.to_le_bytes(), &pst(4, 4, [1, 1, 1], [2, 2, 2])].concat());

    let dump = |out: &Path| -> Vec<String> {
        let said = Command::new(env!("CARGO_BIN_EXE_openmx")).arg("--dump-textures").arg(out).arg("--from").arg(&track).output().expect("the shell runs");
        assert!(said.status.success(), "{}{}", String::from_utf8_lossy(&said.stdout), String::from_utf8_lossy(&said.stderr));
        let mut names: Vec<String> = walk(out).into_iter().map(|p| p.strip_prefix(out).unwrap().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    };
    let first = dump(&root.join("dump1"));
    assert_eq!(first, [format!("Tex/Flag-{}.png", key(&pst(4, 4, [1, 1, 1], [2, 2, 2]))), format!("Tex/Gate-{}.png", key(&file))]);
    let source = textures::read_png(&root.join("dump1").join(&first[1])).unwrap();
    assert_eq!(source.rgba, game_render::world::decode_disc_texture("Gate.pst", &file).unwrap().rgba, "the dump is the texture as decoded");

    let up = solid(32, 16, [7, 8, 9, 255]);
    png(&track, &format!("textures/upscaled/{}", first[1]), &up);
    assert_eq!(dump(&root.join("dump2")), first, "the same names again, the upscale not among them");

    let _turn = SOURCE.lock().unwrap_or_else(|e| e.into_inner());
    let line = textures::install(Some(&mods), true, &[], &[]);
    assert!(line.ends_with(", 1 mods' own upscales"), "{line}");
    let res = Res::folder(&track).unwrap();
    let drawn = decode_texture("Tex\\Gate.pst", &res.read("Tex\\Gate.pst").unwrap()).unwrap();
    assert_eq!((drawn.width, drawn.height, &drawn.rgba[..4]), (32, 16, &[7, 8, 9, 255][..]));
    textures::install(Some(&mods), false, &[], &[]);
    assert_eq!(decode_texture("Tex\\Gate.pst", &file).unwrap().width, 8);
}

fn walk(d: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(d).unwrap().flatten() {
        if e.path().is_dir() {
            out.extend(walk(&e.path()));
        } else {
            out.push(e.path());
        }
    }
    out
}

/// `tools/upscale_mods.py --check` over a mod with two textures lists both and exits 1; with stand-in
/// PNGs and their `sources.json` in place (what a run would leave, no chaiNNer) it lists nothing and
/// exits 0; one made from another picture is stale; `--prune` deletes the upscales nothing names. A mod with
/// `"upscale": false`, and a texture pack, are never listed.
#[test]
fn upscale_mods_check_lists_what_a_mod_still_needs() {
    let root = scratch("check").join("mods");
    let track = root.join("tracks/v84_check");
    write(&track, "mod.json", "{\"standard\": 1, \"kind\": \"track\", \"id\": \"v84_check\"}");
    let gate = pst(8, 4, [90, 91, 92], [93, 94, 95]);
    write(&track, "Tex/Gate.pst", &gate);
    write(&track, "Tex/sub/Pole.pst", pst(4, 8, [1, 2, 3], [4, 5, 6]));
    write(&root, "tracks/v84_skip/mod.json", "{\"kind\": \"track\", \"upscale\": false}");
    write(&root, "tracks/v84_skip/Tex/Gate.pst", &gate);
    write(&root, "hd/mod.json", "{\"kind\": \"textures\"}");
    write(&root, "hd/Gate.pst", &gate);

    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/upscale_mods.py");
    let run = |args: &[&str]| {
        let out = Command::new("python3")
            .arg("-B")
            .arg(&script)
            .args(args)
            .arg(&root)
            .arg("--openmx")
            .arg(env!("CARGO_BIN_EXE_openmx"))
            .output()
            .expect("python3 runs");
        (out.status.code(), String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr))
    };
    let (code, said) = run(&["--check"]);
    assert_eq!(code, Some(1), "{said}");
    let gate_name = format!("Tex/Gate-{}.png", key(&gate));
    assert!(said.contains(&format!("textures/upscaled/{gate_name} (missing)")), "{said}");
    assert!(said.contains("2 textures in 1 mods to upscale"), "{said}");
    assert!(!said.contains("v84_skip") && !said.contains("hd:"), "{said}");

    // What a run leaves: the upscales, and sources.json with each one's source's SHA-1.
    let dumped = scratch("check-dump");
    let ok = Command::new(env!("CARGO_BIN_EXE_openmx")).arg("--dump-textures").arg(&dumped).arg("--from").arg(&track).status().unwrap();
    assert!(ok.success());
    let mut sources = Vec::new();
    for f in walk(&dumped) {
        let rel = f.strip_prefix(&dumped).unwrap().to_string_lossy().replace('\\', "/");
        let t = textures::read_png(&f).unwrap();
        png(&track, &format!("textures/upscaled/{rel}"), &Texture { width: t.width * 4, height: t.height * 4, rgba: game_render::replace::resample(&t, t.width * 4, t.height * 4).rgba });
        sources.push(format!("\"{rel}\": \"{}\"", sha1(&f)));
    }
    write(&track, "textures/upscaled/sources.json", format!("{{{}}}", sources.join(", ")));
    let (code, said) = run(&["--check"]);
    assert_eq!(code, Some(0), "{said}");
    assert!(said.contains("0 textures in 0 mods to upscale"), "{said}");

    // An upscale made from another picture than its source (sources.json's SHA-1) is stale.
    let json = std::fs::read_to_string(track.join("textures/upscaled/sources.json")).unwrap();
    let gate_sha = json.split(&format!("\"{gate_name}\": \"")).nth(1).unwrap()[..40].to_string();
    write(&track, "textures/upscaled/sources.json", json.replace(&gate_sha, &"0".repeat(40)));
    let (code, said) = run(&["--check"]);
    assert_eq!(code, Some(1), "{said}");
    assert!(said.contains(&format!("  textures/upscaled/{gate_name}\n")), "listed, and not as missing:\n{said}");
    write(&track, "textures/upscaled/sources.json", json);

    // A changed texture is a new key, missing, and its old upscale is unused; so is an upscale of
    // nothing. --prune deletes both.
    png(&track, "textures/upscaled/Tex/Old-0123456789abcdef.png", &solid(4, 4, [0, 0, 0, 255]));
    write(&track, "Tex/Gate.pst", pst(8, 4, [90, 91, 92], [0, 0, 0]));
    let (code, said) = run(&["--check"]);
    assert_eq!(code, Some(1), "{said}");
    assert!(said.contains("1 textures in 1 mods to upscale"), "{said}");
    assert!(said.contains("2 unused upscales in 1 mods (--prune deletes them): v84_check 2"), "{said}");
    let (code, said) = run(&["--prune"]);
    assert_eq!(code, Some(0), "{said}");
    assert!(said.contains("2 unused upscales deleted in 1 mods"), "{said}");
    assert!(!track.join(format!("textures/upscaled/{gate_name}")).exists());
    assert!(!std::fs::read_to_string(track.join("textures/upscaled/sources.json")).unwrap().contains(&gate_name));
    assert!(!track.join("textures/upscaled/Tex/Old-0123456789abcdef.png").exists());
}

/// SHA-1, as the script's `hashlib.sha1`, so the test can write the `sources.json` a run writes.
fn sha1(path: &Path) -> String {
    let out = Command::new("sha1sum").arg(path).output().expect("sha1sum runs");
    String::from_utf8_lossy(&out.stdout).split_whitespace().next().unwrap().to_string()
}
