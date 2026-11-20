//! P383: the Xbox disc's textures as a texture pack. `tools/xbox_textures.py pack` keeps an Xbox
//! picture with more pixels than its PS2 twin (or one named to keep), named by the PS2 texture's key,
//! in a `"kind": "textures"` pack the shell then draws; `tools/upscale_mods.py --disc --xbox` takes
//! that picture as the upscale source in the PS2 one's place. And a pack PNG keyed to free06's grass
//! (its PS2 key, off the disc) replaces it.
//!
//! The Xbox `.dxt`s here are hand-made: no Xbox byte comes off a disc. The free06 test reads the PS2
//! disc and skips without it.

// the browser's own clock, threads and files, which are std's here (P196)
#[allow(unused)]
#[path = "../src/clock.rs"]
mod clock;
#[allow(unused)]
#[path = "../src/thread.rs"]
mod thread;
#[allow(unused)]
#[path = "../src/files.rs"]
mod files;
#[path = "../src/track_mods.rs"]
#[allow(dead_code)]
mod track_mods;
#[path = "../src/textures.rs"]
#[allow(dead_code)]
mod textures;

use game_render::Disc;
use game_render::pst::Texture;
use game_render::world::{decode_disc_texture, decode_texture};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

/// The renderer's source is one per process: the tests that install one take turns.
static SOURCE: Mutex<()> = Mutex::new(());

/// A `w` x `h` 8-bit `.pst`, one mip, every pixel palette entry 0 (`c`, at the GS's opaque 0x80).
fn pst(w: u32, h: u32, c: [u8; 3]) -> Vec<u8> {
    let mut d = vec![0u8; 8 + 1024];
    d[8..12].copy_from_slice(&[c[0], c[1], c[2], 0x80]);
    for v in [1, h, w, w * h] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    d.extend_from_slice(&[0; 4]);
    d.extend(std::iter::repeat_n(0u8, (w * h) as usize));
    d
}

/// A `w` x `h` Xbox `.dxt`, 8-bit palette without alpha (format 0x37), one mip, every pixel entry 0
/// (`c`; the palette is BGRA).
fn dxt(w: u32, h: u32, c: [u8; 3]) -> Vec<u8> {
    let mut d = Vec::new();
    for v in [2u32, 0x37, 256] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    let mut pal = vec![0u8; 1024];
    pal[..4].copy_from_slice(&[c[2], c[1], c[0], 0]);
    d.extend(pal);
    for v in [1, w, h, w, h, w * h] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    d.extend(std::iter::repeat_n(0u8, (w * h) as usize));
    d
}

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("p383").join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(path: &Path, bytes: impl AsRef<[u8]>) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// `--dump-textures`'s file for a disc texture: `src/<rel>-<key>.png`; its key.
fn dumped(src: &Path, rel: &str, file: &[u8]) -> String {
    let t = decode_disc_texture("a.pst", file).unwrap();
    let k = game_render::replace::key(&t);
    let p = src.join(format!("{rel}-{k}.png"));
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    textures::write_png(&p, &t).unwrap();
    k
}

fn tool(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools").join(name)
}

fn python(args: &[&dyn AsRef<std::ffi::OsStr>]) -> (Option<i32>, String) {
    let out = Command::new("python3").arg("-B").args(args).output().expect("python3 runs");
    (out.status.code(), String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr))
}

/// Grass is 8x8 on the PS2 and 16x16 on the Xbox: the pack holds the Xbox one under the PS2 key,
/// and the shell draws it. Rock is 8x8 on both: the PS2's stays, until `--keep rock`. Sky has no Xbox
/// twin. Upscaling the disc's textures with `--xbox` takes the Xbox grass as grass's source.
#[test]
fn the_pack_keeps_the_bigger_xbox_picture_under_the_ps2_key() {
    let _turn = SOURCE.lock().unwrap_or_else(|e| e.into_inner());
    let root = scratch("pack");
    let (src, xbox, mods) = (root.join("textures-src"), root.join("xbox"), root.join("mods"));
    let pack = mods.join("xbox-textures");
    let grass = pst(8, 8, [10, 200, 10]);
    let rock = pst(8, 8, [100, 100, 100]);
    let gk = dumped(&src, "Tracks/free06/tracks/free06/free06_grass", &grass);
    let rk = dumped(&src, "Tracks/free06/tracks/free06/rock", &rock);
    dumped(&src, "Tracks/free06/tracks/free06/sky", &pst(4, 4, [0, 0, 255]));
    // case-blind, as the Xbox disc's names may differ from the PS2's
    write(&xbox.join("Tracks/free06/Tracks/free06/free06_grass.dxt"), dxt(16, 16, [20, 220, 30]));
    write(&xbox.join("Tracks/free06/tracks/free06/rock.dxt"), dxt(8, 8, [110, 90, 80]));

    let (code, said) = python(&[&tool("xbox_textures.py"), &"pack", &src, &xbox, &pack]);
    assert_eq!(code, Some(0), "{said}");
    assert!(said.contains("2 pairs, 1 Xbox pictures kept, 1 PS2 textures with no Xbox twin"), "{said}");
    let kept = pack.join(format!("textures/Tracks/free06/tracks/free06/free06_grass-{gk}.png"));
    let t = textures::read_png(&kept).expect("the Xbox grass, under the PS2 key");
    assert_eq!((t.width, t.height, &t.rgba[..4]), (16, 16, &[20, 220, 30, 255][..]));
    assert!(!pack.join(format!("textures/Tracks/free06/tracks/free06/rock-{rk}.png")).exists());
    let survey = std::fs::read_to_string(pack.join("survey.tsv")).unwrap();
    assert!(survey.contains(&format!("Tracks/free06/tracks/free06/free06_grass\t{gk}\tPS2 8x8\tXbox 16x16\tmore pixels")), "{survey}");
    assert!(survey.contains("rock\t") && survey.contains("Xbox 8x8\tPS2"), "{survey}");
    assert!(textures::pack_at(&pack, "xbox-textures").is_ok(), "the store's checks pass");

    let line = textures::install(Some(&mods), true, &[], &[]);
    assert!(line.starts_with("textures: 1 from 1 texture packs"), "{line}");
    let drawn = decode_texture("free06_grass.pst", &grass).unwrap();
    assert_eq!((drawn.width, drawn.height, &drawn.rgba[..4]), (16, 16, &[20, 220, 30, 255][..]));
    assert_eq!(decode_texture("rock.pst", &rock).unwrap().width, 8);
    textures::install(None, false, &[], &[]);

    let (code, said) = python(&[&tool("xbox_textures.py"), &"pack", &src, &xbox, &pack, &"--keep", &"rock"]);
    assert_eq!(code, Some(0), "{said}");
    assert!(said.contains("2 Xbox pictures kept"), "{said}");
    assert!(pack.join(format!("textures/Tracks/free06/tracks/free06/rock-{rk}.png")).exists());
    // and a run without it drops the rock it kept before
    let (code, said) = python(&[&tool("xbox_textures.py"), &"pack", &src, &xbox, &pack]);
    assert_eq!(code, Some(0), "{said}");
    assert!(said.contains(&format!("dropped textures/Tracks/free06/tracks/free06/rock-{rk}.png")), "{said}");
    assert!(!pack.join(format!("textures/Tracks/free06/tracks/free06/rock-{rk}.png")).exists() && kept.exists());

    // The disc's upscale: all three to do; once sources.json names the Xbox grass's SHA-1 as grass's
    // source, grass is done and the other two still are not (the PS2 grass's would leave it stale).
    let up = mods.join("upscaled-disc");
    let check = || python(&[&tool("upscale_mods.py"), &"--disc", &up, &"--src", &src, &"--xbox", &pack, &"--check"]);
    let (code, said) = check();
    assert_eq!(code, Some(1), "{said}");
    assert!(said.contains("3 disc textures to upscale"), "{said}");
    assert!(said.contains(&format!("textures/upscaled/Tracks/free06/tracks/free06/free06_grass-{gk}.png (missing)")), "{said}");
    let rel = format!("Tracks/free06/tracks/free06/free06_grass-{gk}.png");
    let sha = |p: &Path| {
        let out = Command::new("sha1sum").arg(p).output().expect("sha1sum runs");
        String::from_utf8_lossy(&out.stdout).split_whitespace().next().unwrap().to_string()
    };
    write(&up.join("textures/upscaled").join(&rel), b"");
    textures::write_png(&up.join("textures/upscaled").join(&rel), &t).unwrap();
    write(&up.join("textures/upscaled/sources.json"), format!("{{\"{rel}\": \"{}\"}}", sha(&kept)));
    let (code, said) = check();
    assert_eq!(code, Some(1), "{said}");
    assert!(said.contains("2 disc textures to upscale") && !said.contains("free06_grass"), "{said}");
    write(&up.join("textures/upscaled/sources.json"), format!("{{\"{rel}\": \"{}\"}}", sha(&src.join(&rel))));
    let (_, said) = check();
    assert!(said.contains("3 disc textures to upscale"), "the PS2 grass is not the source with --xbox: {said}");
    assert!(!up.join("mod.json").exists(), "--check writes nothing");
}

/// The task's own check: a pack PNG named by free06_grass's PS2 key (decoded off the disc as a ride
/// decodes it) is drawn in its place, at the pack's 256x256, and the disc's 128x128 with the pack off.
#[test]
fn a_pack_png_keyed_to_free06_grass_replaces_it() {
    let _turn = SOURCE.lock().unwrap_or_else(|e| e.into_inner());
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => return eprintln!("skipped: {why}"),
    };
    let res = disc.res("Tracks\\free06.res").expect("free06's archive");
    let file = res.read_suffix("\\free06_grass.pst").expect("free06_grass.pst");
    let ps2 = decode_disc_texture("free06_grass.pst", &file).unwrap();
    assert_eq!((ps2.width, ps2.height), (128, 128), "the PS2's grass");
    let key = game_render::replace::key(&ps2);

    let mods = scratch("free06").join("mods");
    let pack = mods.join("xbox-textures");
    write(&pack.join("mod.json"), "{\"standard\": 1, \"kind\": \"textures\", \"id\": \"xbox-textures\"}");
    let big = Texture { width: 256, height: 256, rgba: [40, 160, 50, 255].repeat(256 * 256) };
    let p = pack.join(format!("textures/tracks/free06/free06_grass-{key}.png"));
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    textures::write_png(&p, &big).unwrap();

    let line = textures::install(Some(&mods), true, &[], &[]);
    assert!(line.starts_with("textures: 1 from 1 texture packs"), "{line}");
    let drawn = decode_texture("tracks\\free06\\free06_grass.pst", &file).unwrap();
    assert_eq!((drawn.width, drawn.height), (256, 256));
    assert_eq!(drawn.rgba, big.rgba);

    textures::install(Some(&mods), true, &[], &["xbox-textures".to_string()]);
    let off = decode_texture("tracks\\free06\\free06_grass.pst", &file).unwrap();
    assert_eq!((off.width, off.height, &off.rgba), (128, 128, &ps2.rgba), "the pack off: the disc's");
    textures::install(None, false, &[], &[]);
}
