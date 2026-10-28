//! V87: PCSX2's texture-replacement names, proved against a PCSX2 dump of this game. PCSX2 2.9 with
//! Tools > Texture Replacement > Dump Textures and Dump Mipmaps on (`DumpReplaceableTextures` and
//! `DumpReplaceableMipmaps` in the ini) loaded `context/v77/free06-bike.p2s`, rode, and paused, and
//! wrote its hash-named PNGs; they are copied to `context/v87/free06-dump/`. Every `.pst` and `.atm`
//! on the disc is named by [`game_render::texhash`], and:
//!
//! - every dumped name whose palette is a disc texture's is one of the disc's names: 8-bit, 4-bit,
//!   mipped, and a far model's smaller level drawn as the base;
//! - its picture is the disc texture's level it starts at, pixel for pixel, the alpha the GS's (0x80
//!   opaque);
//! - and the shell's lookup ([`textures::scan`]) finds the file for the disc texture, alpha doubled.
//!
//! What it leaves out, with no disc palette: the rider's number plates, which the game paints at run
//! time, the loading screen (`ui\*.lsi`, not a `.pst`) and two 1024x1024 32-bit render targets.
//! Skips, saying so, without the disc or the dump.

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

use game_render::disc::Disc;
use game_render::texhash::{self, Pst};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// The dump folder, in this worktree's context.
fn dump() -> Option<PathBuf> {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../context/v87/free06-dump");
    d.is_dir().then_some(d)
}

/// What a disc texture is under one of its PCSX2 names: its member, the file's bytes and the level
/// the name starts at.
struct Named {
    member: String,
    file: Vec<u8>,
    level: usize,
}

/// Every disc `.pst`/`.atm` by each of its PCSX2 names.
fn disc_names(disc: &mut Disc) -> HashMap<String, Named> {
    let mut out = HashMap::new();
    let archives: Vec<String> = disc.entries().iter().map(|e| e.name.clone()).filter(|e| e.to_ascii_lowercase().ends_with(".res")).collect();
    for archive in archives {
        let Ok(res) = disc.res(&archive) else { continue };
        for e in res.entries() {
            let n = e.name.to_ascii_lowercase();
            if !(n.ends_with(".pst") || n.ends_with(".atm")) {
                continue;
            }
            let Ok(file) = res.read(&e.name) else { continue };
            let Some(p) = Pst::parse(&file) else { continue };
            let g = p.gs();
            for a in 0..p.levels.len() {
                for b in a..p.levels.len() {
                    let Some(name) = g.pcsx2_name(a, b, p.width, texhash::TEXA) else { continue };
                    out.entry(name).or_insert_with(|| Named { member: format!("{archive}\\{}", e.name), file: file.clone(), level: a });
                }
            }
        }
    }
    out
}

/// Level `k` of a `.pst` as RGBA, the palette's alpha as the GS has it.
fn level_rgba(p: &Pst, k: usize) -> Vec<u8> {
    let l = &p.levels[k];
    let n = ((p.width >> k).max(1) * (p.height >> k).max(1)) as usize;
    (0..n).flat_map(|i| p.clut[if p.psm == 0x14 { (l[i / 2] >> (4 * (i & 1))) & 15 } else { l[i] } as usize]).collect()
}

/// A PNG's RGBA, as it is in the file.
fn png_rgba(file: &Path) -> Vec<u8> {
    let mut d = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(file).unwrap()));
    d.set_transformations(png::Transformations::normalize_to_color8());
    let mut r = d.read_info().unwrap();
    let mut buf = vec![0; r.output_buffer_size().unwrap()];
    let info = r.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    assert_eq!(info.color_type, png::ColorType::Rgba, "{}", file.display());
    buf
}

#[test]
fn a_pcsx2_dump_of_free06_is_named_as_the_disc_textures_are() {
    let Some(dir) = dump() else {
        eprintln!("skipped: no context/v87/free06-dump (V87's journal says how to make it)");
        return;
    };
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipped: no disc: {e}");
            return;
        }
    };
    let names = disc_names(&mut disc);
    let cluts: HashSet<&str> = names.keys().filter_map(|n| n.split('-').nth(1)).collect();
    let pack = texhash::pack(&dir);
    let (mut found, mut four_bit, mut mipped, mut smaller) = (0, 0, 0, 0);
    let mut wrong = Vec::new();
    for (stem, file) in &pack {
        let Some(n) = names.get(stem) else {
            // Not a disc texture: say so only if its palette is one, which would make the hash wrong.
            if stem.split('-').count() == 3 && cluts.contains(stem.split('-').nth(1).unwrap()) {
                wrong.push(stem.clone());
            }
            continue;
        };
        let p = Pst::parse(&n.file).unwrap();
        assert_eq!(png_rgba(file), level_rgba(&p, n.level), "{stem}: the picture is not {}'s level {}", n.member, n.level);
        found += 1;
        four_bit += (p.psm == 0x14) as usize;
        mipped += dir.join(format!("{stem}-mip1.png")).is_file() as usize;
        smaller += (n.level > 0) as usize;
    }
    assert!(wrong.is_empty(), "dumped with a disc palette but not named as any disc texture: {wrong:?}");
    // 94 of the dump's 108: 4-bit, 8-bit, mipped, and far models' smaller levels drawn as their base.
    assert!(found >= 94 && four_bit >= 10 && mipped >= 12 && smaller >= 30, "{found} found, {four_bit} 4-bit, {mipped} mipped, {smaller} from a smaller level");

    // The shell finds the base-level file for the disc texture, in the GS's alpha doubled.
    let (stem, n) = pack.keys().filter_map(|s| names.get(s).filter(|n| n.level == 0).map(|n| (s, n))).min_by_key(|(s, _)| s.to_string()).unwrap();
    let mods = Path::new(env!("CARGO_TARGET_TMPDIR")).join("v87").join("mods");
    let _ = std::fs::remove_dir_all(&mods);
    std::fs::create_dir_all(mods.join(textures::REPLACEMENTS)).unwrap();
    std::fs::copy(&pack[stem], mods.join(textures::REPLACEMENTS).join(format!("{stem}.png"))).unwrap();
    let found = textures::scan(&mods, &[], &[]);
    assert_eq!(found.pcsx2, 1);
    let t = found.pcsx2_texture(&texhash::pcsx2_names(&n.file)).expect("the pack's file for the disc texture");
    let p = Pst::parse(&n.file).unwrap();
    let mut want = level_rgba(&p, 0);
    texhash::expand_alpha(&mut want);
    assert_eq!((t.width, t.height, t.rgba), (p.width as usize, p.height as usize, want), "{}", n.member);
}
