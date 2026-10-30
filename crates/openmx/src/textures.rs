//! Replaced textures (V84), as opentennis has them (its `textures.rs`, `mods_textures.rs` and
//! `texhash.rs`'s `Overrides`), on openmx's own texture decode ([`game_render::replace`]).
//!
//! Each world texture is taken from the first match:
//!
//! 0. **Texture packs**: mod folders under `mods/` whose `mod.json` says `"kind": "textures"`, with
//!    key-named files (`<anything>-<key>.png`) anywhere under their `textures/`. They stack: a pack
//!    nobody ordered is on top, then settings.txt's `texture_packs = a, b` (top first);
//!    `texture_packs_off = c` turns one off. Named by their folder under `mods/`; the mod store installs
//!    one into `mods/textures/<id>/` and its Pack Order page writes both lines (P230).
//! 1. **Key-named files** in `mods/texture-replacements/` (any subfolder, or flat), matched by the part
//!    of the name after the last `-`: your own, edited or upscaled, at any size, straight alpha.
//! 2. **A PCSX2 texture pack** (V87): hash-named PNGs (`<tex>-<clut>-<bits>.png`, PCSX2's Dump
//!    Textures names) at `mods/texture-replacements/`'s top level, as a pack made for the original MX
//!    Unleashed in PCSX2 has them, matched by [`game_render::texhash`]'s names for the `.pst` the
//!    texture was read from; their alpha is the GS's (0x80 opaque) and is doubled. The `-mipN` files
//!    beside them are not read: each texture is drawn from its largest level.
//! 3. **A mod's own upscales**: key-named files under `textures/upscaled/` in any other mod's folder
//!    (`tools/upscale_mods.py` writes them for the textures the mod carries).
//! 4. The disc's, or the mod's own.
//!
//! The key is [`game_render::replace::key`], a hash of the decoded picture; `openmx --dump-textures`
//! writes every disc texture under it to `mods/textures-src/`. The folders are walked when a ride loads
//! and a picture is decoded when the ride's loaders first ask for it, so a changed file is drawn from the
//! next ride on — or, with `OPENMX_TEXTURE_RELOAD=1` (opentennis's `HST_TEXTURE_RELOAD`, V86), walked
//! again once a second while it rides, and a changed file drawn from the next frame ([`Reload`]). The
//! Upscaled Textures setting is the switch for all of it.

use crate::track_mods::Json;
use game_render::pst::Texture;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

use crate::clock::Instant;

/// Your own key-named files, under the mods folder.
pub const REPLACEMENTS: &str = "texture-replacements";
/// Where `--dump-textures` writes the disc's, under the mods folder.
pub const SOURCES: &str = "textures-src";
/// A mod's own upscales, under its folder.
pub const UPSCALED: &str = "textures/upscaled";

/// Whether `k` is a texture key ([`game_render::replace::key`]: 16 lowercase hex digits).
fn is_key(k: &str) -> bool {
    k.len() == 16 && k.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The key of a file name `<anything>-<key>.png` (case-blind extension).
pub fn key_of(name: &str) -> Option<&str> {
    let stem = name.len().checked_sub(4).filter(|&n| name.is_char_boundary(n) && name[n..].eq_ignore_ascii_case(".png")).map(|n| &name[..n])?;
    stem.rsplit_once('-').map(|(_, k)| k).filter(|k| is_key(k))
}

/// Every key-named PNG under `d`, walked in name order; the first file of a key wins.
fn walk(d: &Path, out: &mut HashMap<String, PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(d).into_iter().flatten().flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out);
        } else if let Some(k) = p.file_name().and_then(|n| n.to_str()).and_then(key_of) {
            out.entry(k.to_string()).or_insert(p);
        }
    }
}

/// Every folder under `mods` with a `mod.json`, as a `/`-separated name under it, in name order: the
/// mods folder's own layout (`tracks/<id>`, `tricks/<id>`) and a pack put straight in it alike. Not
/// below a mod's folder, and not into the two texture folders.
fn mod_dirs(mods: &Path) -> Vec<(String, PathBuf)> {
    fn go(d: &Path, name: &str, depth: u32, out: &mut Vec<(String, PathBuf)>) {
        if !name.is_empty() && d.join("mod.json").is_file() {
            out.push((name.to_string(), d.to_path_buf()));
            return;
        }
        if depth == 3 {
            return;
        }
        let mut subs: Vec<PathBuf> = std::fs::read_dir(d).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        subs.sort();
        for s in subs {
            let Some(n) = s.file_name().and_then(|n| n.to_str()) else { continue };
            if name.is_empty() && (n == REPLACEMENTS || n == SOURCES) {
                continue;
            }
            go(&s, &if name.is_empty() { n.to_string() } else { format!("{name}/{n}") }, depth + 1, out);
        }
    }
    let mut out = Vec::new();
    go(mods, "", 0, &mut out);
    out
}

/// A texture pack: its folder's name under `mods/`, and its textures by key.
#[derive(Debug)]
pub struct Pack {
    pub folder: String,
    pub textures: HashMap<String, PathBuf>,
}

/// Whether `dir/mod.json` says `"kind": "textures"`.
fn is_pack(dir: &Path) -> bool {
    let text = std::fs::read_to_string(dir.join("mod.json")).unwrap_or_default();
    Json::parse(&text).ok().is_some_and(|j| j.get("kind").and_then(Json::str) == Some("textures"))
}

/// The texture packs under `mods` switched on, top first: the ones `order` does not name first, in
/// their folders' order, then its named ones in its order; those in `off` left out. A pack with no
/// key-named file is one log line and left out.
pub fn packs(mods: &Path, order: &[String], off: &[String]) -> Vec<Pack> {
    packs_saying(mods, order, off, true)
}

/// [`packs`], with its log line for an empty pack or without it (a reload's walk, once a second).
fn packs_saying(mods: &Path, order: &[String], off: &[String], say: bool) -> Vec<Pack> {
    let mut all: Vec<Pack> = mod_dirs(mods)
        .into_iter()
        .filter(|(_, d)| is_pack(d))
        .filter(|(f, _)| !off.contains(f))
        .filter_map(|(folder, dir)| {
            let mut textures = HashMap::new();
            walk(&dir.join("textures"), &mut textures);
            if textures.is_empty() {
                if !say {
                    return None;
                }
                println!("textures: {folder}: no textures: a pack holds textures/**/<name>-<key>.png files (--dump-textures names)");
                return None;
            }
            Some(Pack { folder, textures })
        })
        .collect();
    all.sort_by_key(|p| order.iter().position(|n| *n == p.folder).map_or(0, |i| i + 1));
    all
}

/// P230: the texture pack in `dir`, named `folder`, read as [`packs`] reads one and checked as the mod
/// store installs one (hostile input): its `mod.json` says textures, it holds a key-named file under
/// `textures/`, and every such file is a PNG whose header reads. The pictures themselves decode when a
/// ride first asks for them, as any pack's do.
pub fn pack_at(dir: &Path, folder: &str) -> Result<Pack, String> {
    if !is_pack(dir) {
        return Err("its mod.json does not say \"kind\": \"textures\"".into());
    }
    let mut textures = HashMap::new();
    walk(&dir.join("textures"), &mut textures);
    if textures.is_empty() {
        return Err("no textures: a pack holds textures/**/<name>-<key>.png files (--dump-textures names)".into());
    }
    let mut files: Vec<&PathBuf> = textures.values().collect();
    files.sort();
    for f in files {
        let read = std::fs::File::open(f).map_err(|e| e.to_string()).and_then(|r| png::Decoder::new(std::io::BufReader::new(r)).read_info().map(|_| ()).map_err(|e| e.to_string()));
        if let Err(why) = read {
            let rel = f.strip_prefix(dir).unwrap_or(f).to_string_lossy().replace('\\', "/");
            return Err(format!("{rel} is not a PNG that reads: {why}"));
        }
    }
    Ok(Pack { folder: folder.to_string(), textures })
}

/// P230: one texture pack as the mod store's Pack Order page lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct Listed {
    /// Its folder under `mods/`, as `texture_packs` names it.
    pub folder: String,
    /// `mod.json`'s `name`, else the folder's.
    pub name: String,
    pub textures: usize,
    pub on: bool,
}

/// P230: every texture pack under `mods` with a texture in it, top first as [`packs`] stacks them, the
/// ones `off` turns off in their places too.
pub fn listed(mods: &Path, order: &[String], off: &[String]) -> Vec<Listed> {
    packs_saying(mods, order, &[], false)
        .into_iter()
        .map(|p| {
            let text = std::fs::read_to_string(mods.join(&p.folder).join("mod.json")).unwrap_or_default();
            let name = Json::parse(&text).ok().and_then(|j| j.get("name").and_then(Json::str).map(|n| n.chars().filter(|c| !c.is_control()).take(48).collect::<String>()));
            Listed { name: name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| p.folder.clone()), on: !off.contains(&p.folder), textures: p.textures.len(), folder: p.folder }
        })
        .collect()
}

/// What a ride's lookup found: each key's file from the packs and your own files (`files`), the PCSX2
/// pack's files by name (`pcsx2_files`) and the mods' own upscales by key (`upscaled_files`), and how
/// many came from where.
#[derive(Debug, Default)]
pub struct Found {
    pub files: HashMap<String, PathBuf>,
    pub pcsx2_files: HashMap<String, PathBuf>,
    pub upscaled_files: HashMap<String, PathBuf>,
    pub packs: usize,
    pub from_packs: usize,
    pub replacements: usize,
    pub pcsx2: usize,
    pub upscaled: usize,
}

impl Found {
    /// The startup line, opentennis's.
    pub fn line(&self) -> String {
        format!(
            "textures: {} from {} texture packs, {} key-named in mods/{REPLACEMENTS}, {} PCSX2-named, {} mods' own upscales",
            self.from_packs, self.packs, self.replacements, self.pcsx2, self.upscaled
        )
    }

    /// Whether nothing was found.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty() && self.pcsx2_files.is_empty() && self.upscaled_files.is_empty()
    }

    /// The file for the texture keyed `key` whose PCSX2 names `names` gives, by the order above.
    pub fn file(&self, key: &str, names: impl FnOnce() -> Vec<String>) -> Option<(&PathBuf, bool)> {
        if let Some(f) = self.files.get(key) {
            return Some((f, false));
        }
        if !self.pcsx2_files.is_empty()
            && let Some(f) = names().iter().find_map(|n| self.pcsx2_files.get(n))
        {
            return Some((f, true));
        }
        self.upscaled_files.get(key).map(|f| (f, false))
    }

    /// The PCSX2 pack's picture for a texture of these names, its alpha doubled to straight.
    pub fn pcsx2_texture(&self, names: &[String]) -> Option<Texture> {
        let mut t = read_png(names.iter().find_map(|n| self.pcsx2_files.get(n))?)?;
        game_render::texhash::expand_alpha(&mut t.rgba);
        Some(t)
    }
}

/// The lookup over the mods folder `mods`, with settings.txt's pack order.
pub fn scan(mods: &Path, order: &[String], off: &[String]) -> Found {
    scan_saying(mods, order, off, true)
}

fn scan_saying(mods: &Path, order: &[String], off: &[String], say: bool) -> Found {
    let mut f = Found::default();
    let active = packs_saying(mods, order, off, say);
    f.packs = active.len();
    for p in active {
        for (k, file) in p.textures {
            f.files.entry(k).or_insert(file);
        }
    }
    f.from_packs = f.files.len();
    let mut own = HashMap::new();
    walk(&mods.join(REPLACEMENTS), &mut own);
    f.replacements = own.len();
    for (k, file) in own {
        f.files.entry(k).or_insert(file);
    }
    f.pcsx2_files = game_render::texhash::pack(&mods.join(REPLACEMENTS));
    f.pcsx2 = f.pcsx2_files.len();
    for (_, d) in mod_dirs(mods).into_iter().filter(|(_, d)| !is_pack(d)) {
        walk(&d.join(UPSCALED), &mut f.upscaled_files);
    }
    f.upscaled = f.upscaled_files.len();
    f
}

/// Set the renderer's replacements for the next ride: `on` with a mods folder, the lookup over it;
/// otherwise (or with nothing found) none, and every texture is the disc's. The line says which.
pub fn install(mods: Option<&Path>, on: bool, order: &[String], off: &[String]) -> String {
    install_live(mods, on, order, off, false).0
}

/// Whether `OPENMX_TEXTURE_RELOAD` asks for [`Reload`]: set, and not empty or `0`.
pub fn reload_asked() -> bool {
    std::env::var_os("OPENMX_TEXTURE_RELOAD").is_some_and(|v| !v.is_empty() && v != "0")
}

/// [`install`], and with `live` (and the setting on) a [`Reload`] that keeps the ride's textures up
/// to date with the folders. A live source is set even over empty folders, so every texture the ride
/// decodes remembers its key, and a file put in later still finds it.
pub fn install_live(mods: Option<&Path>, on: bool, order: &[String], off: &[String], live: bool) -> (String, Option<Reload>) {
    let Some(mods) = mods.filter(|_| on) else {
        game_render::replace::set(None);
        return ("textures: the disc's (Upscaled Textures off)".into(), None);
    };
    let found = scan(mods, order, off);
    let line = found.line();
    if found.is_empty() && !live {
        game_render::replace::set(None);
        return (line, None);
    }
    let seen = if live { stamp(&found) } else { HashMap::new() };
    let files = Arc::new(RwLock::new(found));
    let shared = files.clone();
    game_render::replace::set(Some(Arc::new(move |key: &str, names: &dyn Fn() -> Vec<String>| {
        let (p, pcsx2) = {
            let found = shared.read().unwrap_or_else(|e| e.into_inner());
            let (p, pcsx2) = found.file(key, names)?;
            (p.clone(), pcsx2)
        };
        let mut t = read_png(&p);
        if pcsx2 && let Some(t) = &mut t {
            game_render::texhash::expand_alpha(&mut t.rgba);
        }
        if t.is_none() {
            println!("textures: {}: not a PNG this reads; the disc's is drawn", p.display());
        }
        t
    })));
    let reload = live.then(|| Reload {
        mods: mods.to_path_buf(),
        order: order.to_vec(),
        off: off.to_vec(),
        seen,
        files,
        walk: None,
        next: Instant::now() + RELOAD_EVERY,
    });
    (line, reload)
}

/// How often [`Reload`] walks the folders again.
pub const RELOAD_EVERY: Duration = Duration::from_secs(1);

/// What a file was when it was last looked at: its path, its time and its length.
type Stamp = (PathBuf, Option<SystemTime>, u64);

/// Each key's winning file in `found` (the packs' and your own, then the mods' upscales), stamped.
fn stamp(found: &Found) -> HashMap<String, Stamp> {
    let upscaled = found.upscaled_files.iter().filter(|(k, _)| !found.files.contains_key(*k));
    found
        .files
        .iter()
        .chain(upscaled)
        .map(|(k, p)| {
            let meta = std::fs::metadata(p).ok();
            (k.clone(), (p.clone(), meta.as_ref().and_then(|m| m.modified().ok()), meta.map_or(0, |m| m.len())))
        })
        .collect()
}

/// `OPENMX_TEXTURE_RELOAD=1` (V86, opentennis B83's `reload`): the lookup walked again once a second
/// on a thread of its own, and each key whose winning file is new or changed read again, for the ride
/// to put in its textures' place ([`game_render::world::Assets::replace_keyed`], then the GPU
/// renderer's `reload_textures`). Off by default: a walk a second is for texture authors.
pub struct Reload {
    mods: PathBuf,
    order: Vec<String>,
    off: Vec<String>,
    /// The lookup as the installed source answers it, kept up to date.
    files: Arc<RwLock<Found>>,
    seen: HashMap<String, Stamp>,
    walk: Option<crate::thread::JoinHandle<Found>>,
    next: Instant,
}

impl Reload {
    /// Start a walk if one is due, and take a finished one's changes: each changed key and its new
    /// picture. A file that does not decode (written as it was read) is tried again on the next walk.
    /// A file taken away leaves its texture as it was until the next ride (the disc's pixels are not
    /// kept), as opentennis's does.
    pub fn poll(&mut self) -> Vec<(String, Texture)> {
        if self.walk.as_ref().is_some_and(|w| w.is_finished()) {
            let now = self.walk.take().map(|w| w.join().unwrap_or_default()).unwrap_or_default();
            return self.take(now);
        }
        if self.walk.is_none() && Instant::now() >= self.next {
            self.next = Instant::now() + RELOAD_EVERY;
            let (mods, order, off) = (self.mods.clone(), self.order.clone(), self.off.clone());
            self.walk = Some(crate::thread::spawn(move || scan_saying(&mods, &order, &off, false)));
        }
        Vec::new()
    }

    /// Walk now and wait for it: [`Reload::poll`]'s changes, at once (tests, and a caller that knows a
    /// file changed).
    pub fn now(&mut self) -> Vec<(String, Texture)> {
        if let Some(w) = self.walk.take() {
            let _ = w.join();
        }
        self.next = Instant::now() + RELOAD_EVERY;
        let now = scan_saying(&self.mods, &self.order, &self.off, false);
        self.take(now)
    }

    fn take(&mut self, found: Found) -> Vec<(String, Texture)> {
        let now = stamp(&found);
        let mut out = Vec::new();
        let mut keys: Vec<&String> = now.keys().filter(|k| self.seen.get(*k) != now.get(*k)).collect();
        keys.sort();
        for k in keys {
            let p = &now[k].0;
            let Some(t) = read_png(p).filter(game_render::replace::whole) else { continue };
            println!("textures: reloaded {}", p.display());
            self.seen.insert(k.clone(), now[k].clone());
            out.push((k.clone(), t));
        }
        self.seen.retain(|k, _| now.contains_key(k));
        *self.files.write().unwrap_or_else(|e| e.into_inner()) = found;
        out
    }
}

/// A PNG as straight RGBA8; `None` for one that does not decode (or is indexed, as opentennis's).
pub fn read_png(p: &Path) -> Option<Texture> {
    decode_png(std::io::BufReader::new(std::fs::File::open(p).ok()?))
}

/// [`read_png`] of a reader.
pub fn decode_png(r: impl std::io::BufRead + std::io::Seek) -> Option<Texture> {
    let mut d = png::Decoder::new(r);
    d.set_transformations(png::Transformations::normalize_to_color8());
    let mut r = d.read_info().ok()?;
    let mut buf = vec![0; r.output_buffer_size()?];
    let info = r.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf.chunks(2).flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&c| [c, c, c, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    Some(Texture { width: info.width as usize, height: info.height as usize, rgba })
}

/// `t` as an RGBA PNG at `p`.
pub fn write_png(p: &Path, t: &Texture) -> Result<(), String> {
    let file = std::fs::File::create(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), t.width as u32, t.height as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().and_then(|mut w| w.write_image_data(&t.rgba)).map_err(|e| format!("{}: {e}", p.display()))
}

/// The textures in a set of archive members, `(member, read)`: each `.pst` and each `.atm`'s first
/// frame, decoded as the disc has it.
fn decoded<'a>(names: impl IntoIterator<Item = &'a str>, read: impl Fn(&str) -> Option<Vec<u8>>) -> Vec<(String, Texture)> {
    names
        .into_iter()
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.ends_with(".pst") || n.ends_with(".atm")
        })
        .filter_map(|n| Some((n.to_string(), game_render::world::decode_disc_texture(n, &read(n)?)?)))
        .collect()
}

/// A member's name as a path under the dump: its `\`-separated folders kept, `.pst`/`.atm` dropped, and
/// nothing that could climb out.
fn member_path(member: &str) -> PathBuf {
    let stem = member.rsplit_once('.').map_or(member, |(s, _)| s);
    stem.split(['\\', '/']).filter(|p| !p.is_empty() && *p != "." && *p != "..").collect()
}

/// Write `textures` to `out/<folder>/<member>-<key>.png`; how many were written.
fn write_all(out: &Path, folder: &Path, textures: Vec<(String, Texture)>) -> Result<usize, String> {
    let mut n = 0;
    for (member, t) in textures {
        if t.width == 0 || t.height == 0 || t.rgba.len() != t.width * t.height * 4 {
            continue;
        }
        let rel = member_path(&member);
        let name = format!("{}-{}.png", rel.file_name().and_then(|n| n.to_str()).unwrap_or("texture"), game_render::replace::key(&t));
        let p = out.join(folder).join(rel.parent().unwrap_or(Path::new(""))).join(name);
        std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| format!("{}: {e}", p.display()))?;
        write_png(&p, &t)?;
        n += 1;
    }
    Ok(n)
}

/// `--dump-textures --from DIR`: every `.pst`/`.atm` a mod's folder carries, read as the shell reads
/// the folder (each file a member, [`game_render::disc::Res::folder`]), to `out` as
/// `<path in the mod>-<key>.png`, and its `.trn`s' ground pages as the disc's are: `upscale_mods.py`'s
/// sources. Not its `textures/` (its own upscales).
/// How many were written.
pub fn dump_folder(dir: &Path, out: &Path) -> Result<usize, String> {
    let res = game_render::disc::Res::folder(dir)?;
    let names: Vec<String> = res.entries().iter().map(|e| e.name.clone()).filter(|n| !n.to_ascii_lowercase().starts_with("textures\\")).collect();
    let n = write_all(out, Path::new(""), decoded(names.iter().map(String::as_str), |m| res.read(m).ok()))?;
    Ok(n + write_pages(out, Path::new(""), &names, |m| res.read(m).ok())?)
}

/// `--dump-textures`: every texture in the disc's `.res` archives, at its native size, to
/// `out/<archive>/<member>-<key>.png`, and the ground pages of each `.trn` in them to
/// `out/<archive>/<terrain>/page_NN-<key>.png`. How many were written.
pub fn dump_disc(disc: &mut game_render::Disc, out: &Path) -> Result<usize, String> {
    let mut n = 0;
    let archives: Vec<String> = disc.entries().iter().map(|e| e.name.clone()).filter(|e| e.to_ascii_lowercase().ends_with(".res")).collect();
    for archive in archives {
        let Ok(res) = disc.res(&archive) else { continue };
        let folder = member_path(&archive);
        let names: Vec<String> = res.entries().iter().map(|e| e.name.clone()).collect();
        n += write_all(out, &folder, decoded(names.iter().map(String::as_str), |m| res.read(m).ok()))?;
        n += write_pages(out, &folder, &names, |m| res.read(m).ok())?;
    }
    Ok(n)
}

/// The ground pages of each `.trn` in `names` to `out/<folder>/<terrain>/page_NN-<key>.png`; how many.
fn write_pages(out: &Path, folder: &Path, names: &[String], read: impl Fn(&str) -> Option<Vec<u8>>) -> Result<usize, String> {
    let mut n = 0;
    for trn in names.iter().filter(|m| m.to_ascii_lowercase().ends_with(".trn")) {
        let Some(bytes) = read(trn) else { continue };
        let Ok(terrain) = game_sim::track::Terrain::parse(&bytes) else { continue };
        // The pages through the terrain's palette; the leaves' layout does not change a page.
        let Some(pages) = pages_as_the_disc_has_them(&terrain) else { continue };
        let base = member_path(trn);
        let named = pages.into_iter().enumerate().map(|(i, t)| (format!("{}\\page_{i:02}.pst", base.to_string_lossy()), t)).collect();
        n += write_all(out, folder, named)?;
    }
    Ok(n)
}

/// A terrain's pages as [`game_render::terrain::Pages::of`] decodes them: the dump installs no
/// replacements, so these are the disc's.
fn pages_as_the_disc_has_them(terrain: &game_sim::track::Terrain) -> Option<Vec<Texture>> {
    game_render::terrain::Pages::of(terrain, 1.0, (0, 0)).map(|p| p.pages)
}
