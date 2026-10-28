//! V86: `OPENMX_TEXTURE_RELOAD=1`, opentennis's `HST_TEXTURE_RELOAD`. A key-named file in
//! `mods/texture-replacements/` rewritten while a ride runs is drawn from then on: every loaded texture
//! remembers its disc key, the software renderer's pixels are swapped in the assets and the GPU
//! renderer uploads that texture (and its levels) again. Without the variable the folders are read
//! once, when the ride loads, and a rewrite is not drawn until the next ride.
//!
//! The first test is hand-made bytes and needs no disc; the ride needs the user's disc and skips
//! without one.

// the browser's own clock, threads and files, which are std's here (P196)
#[allow(unused)]
#[path = "../src/clock.rs"]
mod clock;
#[allow(unused)]
#[path = "../src/thread.rs"]
mod thread;
#[path = "../src/track_mods.rs"]
#[allow(dead_code)]
mod track_mods;
#[path = "../src/textures.rs"]
#[allow(dead_code)]
mod textures;

use game_render::pst::Texture;
use game_render::world::Assets;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn solid(w: usize, h: usize, c: [u8; 4]) -> Texture {
    Texture { width: w, height: h, rgba: c.repeat(w * h) }
}

/// A fresh folder for one test.
fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("v86").join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// `t` written to `path` whole or not at all: beside it first, then moved over it, so a walk that
/// reads it mid-write never sees half a file.
fn put_png(path: &Path, t: &Texture) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let part = path.with_extension("part");
    textures::write_png(&part, t).unwrap();
    std::fs::rename(&part, path).unwrap();
}

/// A `w` x `h` 8-bit `.pst`, one mip, its pixels a checker of palette entries 0 and 1.
fn pst(w: u32, h: u32, a: [u8; 3], b: [u8; 3]) -> Vec<u8> {
    let mut d = vec![0u8; 8 + 1024];
    d[8..12].copy_from_slice(&[a[0], a[1], a[2], 0x80]);
    d[12..16].copy_from_slice(&[b[0], b[1], b[2], 0x80]);
    for v in [1, h, w, w * h] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    d.extend_from_slice(&[0; 4]);
    d.extend((0..w * h).map(|i| ((i % w + i / w) % 2) as u8));
    d
}

/// The reload's bookkeeping, without a disc or a ride: a material's texture loaded under a live source
/// keeps its key; a key-named file put in after the ride loaded, rewritten, or rewritten with nothing
/// new, is taken (or not) by the next walk, and [`Assets::replace_keyed`] puts it in every place that
/// key was loaded. A file that is not a PNG is left for the next walk.
#[test]
fn a_rewritten_file_reaches_every_texture_of_its_key() {
    let mods = scratch("unit").join("mods");
    std::fs::create_dir_all(mods.join(textures::REPLACEMENTS)).unwrap();
    let disc = pst(4, 4, [10, 20, 30], [200, 210, 220]);
    let key = game_render::replace::key(&game_render::world::decode_disc_texture("rock.pst", &disc).unwrap());
    let other = pst(4, 4, [1, 2, 3], [4, 5, 6]);

    // Not live: no source over empty folders, so nothing keeps a key.
    let (_, none) = textures::install_live(Some(&mods), true, &[], &[], false);
    assert!(none.is_none() && !game_render::replace::on());
    // Live, with the setting off: the disc's, and nothing to watch.
    let (line, none) = textures::install_live(Some(&mods), false, &[], &[], true);
    assert!(none.is_none(), "{line}");

    let (_, reload) = textures::install_live(Some(&mods), true, &[], &[], true);
    let mut reload = reload.expect("live with the setting on");
    assert!(game_render::replace::on(), "a live source is set over empty folders");
    let read = |f: &str| match f {
        "rock.psm" | "rock2.psm" => Some(b"[ShaderPass_1]\nTextureMap_1 = rock\n".to_vec()),
        "pebble.psm" => Some(b"[ShaderPass_1]\nTextureMap_1 = pebble\n".to_vec()),
        "rock.pst" => Some(disc.clone()),
        "pebble.pst" => Some(other.clone()),
        _ => None,
    };
    let mut assets = Assets::default();
    let (rock, _) = assets.resolve(&read, "rock");
    let (pebble, _) = assets.resolve(&read, "pebble");
    let (rock, pebble) = (rock.unwrap(), pebble.unwrap());
    assert_eq!(assets.key_of(rock), Some(key.as_str()));
    assert!(assets.key_of(pebble).is_some_and(|k| k != key));
    assert_eq!(assets.textures[rock].width, 4, "nothing replaces it yet");
    assert!(reload.now().is_empty(), "nothing changed");

    // Put in after the ride loaded.
    let file = mods.join(textures::REPLACEMENTS).join(format!("rock-{key}.png"));
    put_png(&file, &solid(8, 8, [255, 0, 255, 255]));
    let changed = reload.now();
    assert_eq!(changed.iter().map(|(k, t)| (k.as_str(), t.width)).collect::<Vec<_>>(), [(key.as_str(), 8)]);
    assert_eq!(assets.replace_keyed(&changed[0].0, &changed[0].1), [rock]);
    assert_eq!((assets.textures[rock].width, &assets.textures[rock].rgba[..4]), (8, &[255, 0, 255, 255][..]));
    assert_eq!(assets.textures[pebble].width, 4, "another key is untouched");
    assert!(reload.now().is_empty(), "taken once");

    // Rewritten: a new picture, at another size.
    put_png(&file, &solid(16, 16, [0, 255, 0, 255]));
    let changed = reload.now();
    assert_eq!(changed.len(), 1);
    assert_eq!(assets.replace_keyed(&key, &changed[0].1), [rock]);
    assert_eq!((assets.textures[rock].width, &assets.textures[rock].rgba[..4]), (16, &[0, 255, 0, 255][..]));
    // A texture decoded from now on is the new one too: the source reads the walk's lookup.
    let fresh = game_render::world::decode_texture("rock.pst", &disc).unwrap();
    assert_eq!((fresh.width, &fresh.rgba[..4]), (16, &[0, 255, 0, 255][..]));

    // Half a file is not taken, and is taken once it is whole.
    std::fs::write(&file, b"\x89PNG half").unwrap();
    assert!(reload.now().is_empty());
    put_png(&file, &solid(2, 2, [9, 9, 9, 255]));
    assert_eq!(reload.now().len(), 1);
    // A broken picture changes nothing.
    assert!(assets.replace_keyed(&key, &Texture { width: 2, height: 2, rgba: vec![0; 3] }).is_empty());
    game_render::replace::set(None);
}

/// A PNG the shell wrote, back to its RGB pixels: its rows are filter 0 in zlib stored blocks.
fn read_shot(path: &Path) -> (usize, usize, Vec<u8>) {
    let png = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let be = |at: usize| u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
    let (mut at, mut size, mut idat) = (8, (0, 0), Vec::new());
    while at + 8 <= png.len() {
        let (len, tag) = (be(at), &png[at + 4..at + 8]);
        let data = &png[at + 8..at + 8 + len];
        match tag {
            b"IHDR" => size = (be(at + 8), be(at + 12)),
            b"IDAT" => idat.extend_from_slice(data),
            _ => {}
        }
        at += 12 + len;
    }
    let mut raw = Vec::new();
    let mut i = 2;
    loop {
        let last = idat[i] & 1 != 0;
        let n = u16::from_le_bytes([idat[i + 1], idat[i + 2]]) as usize;
        raw.extend_from_slice(&idat[i + 5..i + 5 + n]);
        i += 5 + n;
        if last {
            break;
        }
    }
    let (w, h) = size;
    let rgb = (0..h).flat_map(|y| raw[y * (1 + 3 * w) + 1..(y + 1) * (1 + 3 * w)].to_vec()).collect();
    (w, h, rgb)
}

/// The keys of every texture the bikes' and riders' archives carry, as the disc has them.
fn bike_and_rider_keys(disc: &mut game_render::disc::Disc) -> Vec<String> {
    let archives: Vec<String> = disc
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.ends_with(".res") && (n.starts_with("bike\\") || n.starts_with("rider\\"))
        })
        .collect();
    let mut keys = Vec::new();
    for a in archives {
        let Ok(res) = disc.res(&a) else { continue };
        for e in res.entries() {
            let n = e.name.to_ascii_lowercase();
            if !(n.ends_with(".pst") || n.ends_with(".atm")) {
                continue;
            }
            let Some(t) = res.read(&e.name).ok().and_then(|b| game_render::world::decode_disc_texture(&n, &b)) else { continue };
            keys.push(game_render::replace::key(&t));
        }
    }
    keys.sort();
    keys.dedup();
    keys
}

/// How many of a shot's pixels are magenta and how many green: the bike and rider, painted by the
/// replacements, lit and fogged but still far from anything free06 has.
fn painted(rgb: &[u8]) -> (usize, usize) {
    let px = rgb.chunks(3).map(|p| (p[0] as i32, p[1] as i32, p[2] as i32));
    let magenta = px.clone().filter(|&(r, g, b)| r > 60 && b > 60 && r > 3 * g && b > 3 * g).count();
    let green = px.filter(|&(r, g, b)| g > 60 && g > 3 * r && g > 3 * b).count();
    (magenta, green)
}

/// The shell rides free06 headless with every bike and rider texture replaced by magenta; once it is
/// drawing, the files are rewritten green. With `OPENMX_TEXTURE_RELOAD=1` the shot five seconds later
/// is green where the bike and rider are, with no magenta left; without it, the same shot is still
/// magenta, with no more green than the ride has without either. What the shell said, and the counts.
fn ride(mods: &Path, shot: &Path, keys: &[String], live: bool, renderer: &str) -> (String, (usize, usize)) {
    let dir = mods.join(textures::REPLACEMENTS);
    let _ = std::fs::remove_dir_all(&dir);
    let tiny = |c: [u8; 4]| solid(4, 4, c);
    for k in keys {
        put_png(&dir.join(format!("t-{k}.png")), &tiny([255, 0, 255, 255]));
    }
    let _ = std::fs::remove_file(shot);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_openmx"));
    cmd.current_dir(&root)
        .args(["--headless", "--track", "free06", "--size", "320x224", "--overlay", "none", "--controls", "original"])
        .args(["--renderer", renderer, "--textures", "upscaled", "--mods"])
        .arg(mods)
        .args(["--frames", "300", "--shot-frame", "299", "--shot"])
        .arg(shot)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    match live {
        true => cmd.env("OPENMX_TEXTURE_RELOAD", "1"),
        false => cmd.env_remove("OPENMX_TEXTURE_RELOAD"),
    };
    let mut child = cmd.spawn().expect("the shell runs");
    let mut said = String::new();
    let mut rewritten = false;
    for line in BufReader::new(child.stdout.take().unwrap()).lines() {
        let line = line.unwrap();
        said += &line;
        said.push('\n');
        // Drawing: the ride's textures are loaded and uploaded, from the magenta files.
        if !rewritten && line.starts_with("headless, drawing") {
            for k in keys {
                put_png(&dir.join(format!("t-{k}.png")), &tiny([0, 255, 0, 255]));
            }
            rewritten = true;
        }
    }
    let status = child.wait().unwrap();
    assert!(status.success() && rewritten, "the shell failed:\n{said}");
    let (w, h, rgb) = read_shot(shot);
    assert_eq!((w, h), (320, 224));
    (said, painted(&rgb))
}

#[test]
fn a_file_rewritten_mid_ride_is_drawn_with_the_variable_set() {
    let mut disc = match game_render::disc::Disc::open_configured() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let keys = bike_and_rider_keys(&mut disc);
    assert!(keys.len() > 300, "the bikes' and riders' textures: {}", keys.len());
    let base = scratch("ride");
    let mods = base.join("mods");

    for renderer in ["gpu", "software"] {
        let (said, (magenta, green)) = ride(&mods, &base.join(format!("still-{renderer}.png")), &keys, false, renderer);
        println!("{renderer}, without the variable: {magenta} magenta, {green} green");
        assert!(!said.contains("textures: reloaded"), "{said}");
        assert!(magenta > 300, "the bike and rider are drawn magenta from the files read at load: {magenta}");

        let (said, (live_magenta, live_green)) = ride(&mods, &base.join(format!("live-{renderer}.png")), &keys, true, renderer);
        println!("{renderer}, with it: {live_magenta} magenta, {live_green} green");
        assert!(said.contains("textures: OPENMX_TEXTURE_RELOAD"), "{said}");
        assert!(said.contains("textures: reloaded"), "{said}");
        assert!(live_magenta * 50 < magenta, "the magenta is gone: {live_magenta} of {magenta}");
        assert!(live_green > green + magenta / 2, "drawn green instead: {live_green}, {green} without");
    }
}
