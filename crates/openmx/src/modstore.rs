//! The mod store (P198, `plan/P198.md`), opentennis's M17 (`hst-base/src/modstore.rs`) on openmx's
//! mods: the public Hugging Face dataset [`REPO`], one folder a mod whatever its kind —
//! `mods/<id>/mod.zip` (the mod's folder as the shell reads it, zipped), `meta.json` ([`Meta`]) and
//! `preview.webp`. Browse lists `main` (merged: Verified) and every open pull request (an upload goes
//! live as its PR opens, the PR's author its author); M116: from the store's `index.json` plus the
//! PRs opened since it was built, never every PR's tree, and kept an hour ([`CACHE`]).
//!
//! Install streams `mod.zip` to a file beside the mods (B204: never whole in memory), hashing it as it
//! comes, and only a zip of the listed size and sha256 is opened. Then the *Mods are data only*
//! checks: every entry a plain path inside the mod with a data extension ([`EXTENSIONS`]), no link,
//! no duplicate, the counts and sizes measured while inflating ([`MAX_ENTRIES`], [`MAX_FILE`],
//! [`MAX_TOTAL`]), its `mod.json` the store's id and kind, and on the desktop the staged copy read by
//! the shell's own parsers for its kind (`track_mods`, `worlds`, `trick_packs`). Only then does it go
//! into `mods/<kind>/<id>/`, with `store.json` beside it ([`Stamp`]). A refused mod leaves nothing.
//!
//! Network work runs off the frame — a thread a job on the desktop, the page's event loop in the
//! browser — and comes back through [`Store::take`]. B339: installs run [`PARALLEL`] at a time.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::track_mods::{self, Json};

/// The store: one public dataset repo (the user's, 2026-10-28).
pub const REPO: &str = "FIZL0/openmx-mods";
/// Where it is.
pub const HF: &str = "https://huggingface.co";
/// The listing is kept this long (M116: Hugging Face rate-limits by address), in this file beside
/// settings.txt.
const CACHE_SECS: u64 = 3600;
pub const CACHE: &str = "mods-store.json";
/// The previews, by the zip's sha256 (B259: an update's new picture is fetched again), beside it.
pub const PREVIEWS: &str = "mods-store-previews";
/// What an installed store mod came from, in its folder.
pub const STAMP: &str = "store.json";
/// The browser's list of what it installed (its OPFS `mods/` is not read before the shell starts).
const INSTALLED: &str = "mods-store-installed.json";

/// M116: what a 429 or 5xx from the store says.
pub const BUSY: &str = "The mod store is busy, try again in a minute.";

/// B339: installs at once — fewer in the browser, which a phone runs.
pub const PARALLEL: usize = if cfg!(target_arch = "wasm32") { 2 } else { 3 };

/// The extensions a mod's files may have: the formats the shell reads out of a track, a world, a trick
/// pack or a music pack (every one in the mods openmx-mods makes), and pictures and text. Nothing a
/// system would run.
pub const EXTENSIONS: &[&str] = &[
    "aty", "atm", "aud", "blend", "bpb", "cmt", "col", "cub", "dat", "emt", "esb", "ess", "hit", "ini", "json", "mcf", "mlb", "mot", "mst", "ogg", "pb", "pco", "phy", "png", "prt", "psg", "psm", "pst", "scn",
    "set", "sub", "tdf", "toy", "trn", "txt", "vig", "vub", "vue", "vut", "wav", "webp", "wpt",
];
/// Caps on a zip's entries, one unpacked file and all of them together: measured while inflating, not
/// taken from the zip's headers. The biggest mod openmx-mods makes is a 3.4 GB world of 34,409 files.
pub const MAX_ENTRIES: usize = 65_000;
pub const MAX_FILE: u64 = 1 << 30;
pub const MAX_TOTAL: u64 = 0xf000_0000;
/// The biggest `mod.zip` (no zip64: its offsets are 32 bits).
pub const MAX_ZIP: u64 = u32::MAX as u64;
const MAX_META: u64 = 64 << 10;
const MAX_PREVIEW: u64 = 4 << 20;
const MAX_LISTING: u64 = 32 << 20;
/// A preview's widest, as the grid shows it.
pub const PREVIEW_W: u32 = 480;

/// A mod's kind: its `mod.json`'s `kind` (openmx-mods' standard §6).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Track,
    World,
    Tricks,
    Music,
    /// One this build does not know: listed on no tab, never installed.
    #[serde(other)]
    Other,
}

/// Browse's tabs, by [`Kind::tab`].
pub const TABS: [&str; 3] = ["Tracks", "Trick Packs", "Music"];

impl Kind {
    /// Its folder under `mods/`.
    pub fn folder(self) -> Option<&'static str> {
        match self {
            Kind::Track | Kind::World => Some("tracks"),
            Kind::Tricks => Some("tricks"),
            Kind::Music => Some("music"),
            Kind::Other => None,
        }
    }

    /// Its word in `mod.json`.
    pub fn word(self) -> &'static str {
        match self {
            Kind::Track => "track",
            Kind::World => "world",
            Kind::Tricks => "tricks",
            Kind::Music => "music",
            Kind::Other => "",
        }
    }

    /// Which of [`TABS`] it shows on: a world beside the tracks, as Free Ride lists it.
    pub fn tab(self) -> Option<usize> {
        match self {
            Kind::Track | Kind::World => Some(0),
            Kind::Tricks => Some(1),
            Kind::Music => Some(2),
            Kind::Other => None,
        }
    }
}

/// `mods/<id>/meta.json`: what Browse shows and what Install checks.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Meta {
    pub id: String,
    #[serde(default)]
    pub kind: Kind,
    pub name: String,
    #[serde(default)]
    pub author: String,
    /// The source game, as `mod.json`'s `source.game` says (MX Unleashed, MX vs. ATV Unleashed, …).
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub version: String,
    /// `mod.zip`'s bytes and sha256 (lowercase hex).
    pub size: u64,
    pub sha256: String,
    /// The standard's version its `mod.json` is written to.
    #[serde(default)]
    pub standard: u64,
}

/// One mod in the store.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub meta: Meta,
    /// `main` or `refs/pr/<n>`.
    pub rev: String,
    /// On `main`: its PR was merged.
    pub verified: bool,
}

impl Entry {
    /// What tells store mods apart, two authors' uploads of an id both listed (B248): `<author>/<id>`.
    pub fn key(&self) -> String {
        format!("{}/{}", self.meta.author, self.meta.id)
    }
}

/// What an installed store mod came from (`store.json` beside its `mod.json`).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Stamp {
    pub id: String,
    pub kind: Kind,
    pub sha256: String,
    pub rev: String,
    pub verified: bool,
    pub author: String,
    pub version: String,
}

/// A store entry as against what is installed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Have {
    No,
    /// This upload, as listed.
    Installed,
    /// This upload, at another zip: an update (or the store's copy of an id installed from another
    /// author's upload, which Install replaces).
    Update,
    /// A mod of this id in the folder that is not from the store, which Install never overwrites.
    Own,
}

/// A picture, RGBA.
#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Listing(Result<Vec<Entry>, String>),
    /// Something to say after a listing: the store was busy, so the last one is shown.
    Note(String),
    /// An install under way ([`Entry::key`]): the share done, 0 to 1, and what to show.
    Progress(String, f32, String),
    /// An install's outcome ([`Entry::key`]): the mod's name, or why it was refused (nothing kept).
    Installed(String, Result<String, String>),
    /// A removal's ([`Entry::key`]).
    Removed(String, Result<String, String>),
    /// The preview of the zip with this sha256, or None (it has none, or it does not decode).
    Preview(String, Option<Picture>),
}

// ---- the store's replies, read as hostile input ----------------------------------------------

/// `s` as plain text: no control characters, at most `n` characters.
fn plain_text(s: &str, n: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(n).collect::<String>().trim().to_string()
}

/// `mods/<id>/meta.json`, checked: its id its folder's and a mod id's, a kind this build has a folder
/// for, a sha256, a size within the cap; its text capped.
pub fn meta_of(b: &[u8], id: &str) -> Option<Meta> {
    let mut m: Meta = serde_json::from_slice(b).ok()?;
    let hex = m.sha256.len() == 64 && m.sha256.bytes().all(|c| c.is_ascii_hexdigit());
    if m.id != id || !track_mods::valid_id(id) || m.kind == Kind::Other || !hex || m.size == 0 || m.size > MAX_ZIP || m.standard > track_mods::STANDARD {
        return None;
    }
    m.sha256.make_ascii_lowercase();
    m.name = plain_text(&m.name, 48);
    m.author = plain_text(&m.author, 40);
    m.source = plain_text(&m.source, 40);
    m.version = plain_text(&m.version, 16);
    (!m.name.is_empty()).then_some(m)
}

/// The mod ids in a tree listing of `mods/`.
pub fn ids_in(tree: &[u8]) -> Vec<String> {
    #[derive(Deserialize)]
    struct Node {
        #[serde(rename = "type")]
        kind: String,
        path: String,
    }
    let nodes: Vec<Node> = serde_json::from_slice(tree).unwrap_or_default();
    nodes.into_iter().filter(|n| n.kind == "directory").filter_map(|n| n.path.strip_prefix("mods/").map(str::to_string)).filter(|id| track_mods::valid_id(id)).collect()
}

/// A discussions page: its open PRs (number, author), how many items it had, and the reply's count.
pub fn pulls_in(page: &[u8]) -> (Vec<(u32, String)>, usize, usize) {
    let v: serde_json::Value = serde_json::from_slice(page).unwrap_or_default();
    let list = v["discussions"].as_array().cloned().unwrap_or_default();
    let open = list
        .iter()
        .filter(|d| d["isPullRequest"].as_bool() == Some(true) && d["status"] == "open")
        .filter_map(|d| Some((u32::try_from(d["num"].as_u64()?).ok()?, d["author"]["name"].as_str().unwrap_or("").to_string())))
        .collect();
    (open, list.len(), v["count"].as_u64().map_or(usize::MAX, |n| n as usize))
}

/// Every PR's head commit, from the refs listing.
pub fn heads_in(b: &[u8]) -> BTreeMap<u32, String> {
    let v: serde_json::Value = serde_json::from_slice(b).unwrap_or_default();
    let prs = v["pullRequests"].as_array().cloned().unwrap_or_default();
    prs.iter().filter_map(|r| Some((r["name"].as_str()?.parse().ok()?, r["targetCommit"].as_str()?.to_string()))).collect()
}

/// M116: `index.json` on `main`, which the store's own tooling keeps: every mod on `main` and on each
/// PR open when it was built, at the head commit it read.
#[derive(Deserialize)]
pub struct Index {
    pub version: u32,
    #[serde(default)]
    pub pulls: BTreeMap<u32, Pull>,
    #[serde(default)]
    pub entries: Vec<Entry>,
}

#[derive(Deserialize)]
pub struct Pull {
    #[serde(default)]
    pub commit: String,
}

/// One rev's part of a listing: the index's copy, or to read from the store (rev, its PR's author).
#[derive(Debug, PartialEq)]
pub enum Part {
    Have(Vec<Entry>),
    Read(String, Option<String>),
}

/// A store entry: `meta` at `rev`; a PR's mod is its author's.
fn entry(mut meta: Meta, rev: &str, author: Option<&str>) -> Entry {
    if let Some(a) = author {
        meta.author = plain_text(a, 40);
    }
    Entry { meta, verified: rev == "main", rev: rev.to_string() }
}

/// `main`, then each open PR newest first: the index's copy, re-checked, where it has one (a PR only
/// while open, as its author, and at the commit the index read); without an index every rev is read.
pub fn parts(ix: Option<Index>, open: Vec<(u32, String)>, heads: &BTreeMap<u32, String>) -> Vec<Part> {
    let Some(ix) = ix.filter(|i| i.version == 1) else {
        return std::iter::once(Part::Read("main".into(), None)).chain(open.into_iter().map(|(n, who)| Part::Read(format!("refs/pr/{n}"), Some(who)))).collect();
    };
    let at = |rev: &str, author: Option<&str>| -> Vec<Entry> {
        let again = |e: &Entry| meta_of(&serde_json::to_vec(&e.meta).unwrap_or_default(), &e.meta.id);
        ix.entries.iter().filter(|e| e.rev == rev).filter_map(again).map(|m| entry(m, rev, author)).collect()
    };
    let mut out = vec![Part::Have(at("main", None))];
    for (n, who) in open {
        let rev = format!("refs/pr/{n}");
        let same = ix.pulls.get(&n).is_some_and(|p| heads.get(&n) == Some(&p.commit));
        out.push(if same { Part::Have(at(&rev, Some(&who))) } else { Part::Read(rev, Some(who)) });
    }
    out
}

/// `main` first, then each PR newest first; one already listed (the same id, and the same author or
/// zip) is left out, so two authors' uploads of an id are both listed (B248). By name.
pub fn merge(lists: Vec<Vec<Entry>>) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for e in lists.into_iter().flatten() {
        if !out.iter().any(|o| o.meta.id == e.meta.id && (o.meta.author == e.meta.author || o.meta.sha256 == e.meta.sha256)) {
            out.push(e);
        }
    }
    out.sort_by(|a, b| a.meta.name.to_lowercase().cmp(&b.meta.name.to_lowercase()).then_with(|| a.key().cmp(&b.key())));
    out
}

#[derive(Serialize, Deserialize)]
struct Cached {
    at: u64,
    entries: Vec<Entry>,
}

fn now_secs() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    use std::time::{SystemTime, UNIX_EPOCH};
    #[cfg(target_arch = "wasm32")]
    use web_time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn hex(d: &[u8]) -> String {
    d.iter().map(|x| format!("{x:02x}")).collect()
}

/// `rev` as one URL path part.
fn rev_part(rev: &str) -> String {
    rev.replace('/', "%2F")
}

/// A string as a URL's query value.
fn query(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// Report: a new Community discussion on the store, prefilled with the mod's id, author and rev.
pub fn report_url(e: &Entry) -> String {
    let title = format!("Report: {} ({})", e.meta.name, e.meta.id);
    let body = format!("Mod id: {}\nAuthor: {}\nRevision: {}\nsha256: {}\n\nWhat is wrong with it:\n", e.meta.id, e.meta.author, e.rev, e.meta.sha256);
    format!("{HF}/datasets/{REPO}/discussions/new?title={}&description={}", query(&title), query(&body))
}

// ---- mod zips: the data-only checks ------------------------------------------------------------

/// Whether `n` is a plain relative path that stays inside the mod's folder: no root, drive, `..`,
/// `.`, empty part, backslash, colon, control character or Windows device name.
pub fn plain(n: &str) -> bool {
    let device = |p: &str| {
        let s = p.split('.').next().unwrap_or("").to_ascii_uppercase();
        matches!(s.as_str(), "CON" | "PRN" | "AUX" | "NUL") || (s.len() == 4 && (s.starts_with("COM") || s.starts_with("LPT")) && s.as_bytes()[3].is_ascii_digit())
    };
    !n.is_empty()
        && n.len() <= 255
        && !n.starts_with('/')
        && !n.chars().any(|c| c == '\\' || c == ':' || c.is_control())
        && n.trim_end_matches('/').split('/').all(|p| !p.is_empty() && !p.ends_with(['.', ' ']) && !device(p))
}

fn u16_at(d: &[u8], at: usize) -> Result<usize, String> {
    d.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize).ok_or_else(|| "the zip is cut short".into())
}

fn u32_at(d: &[u8], at: usize) -> Result<u32, String> {
    d.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(|| "the zip is cut short".into())
}

fn crc32(d: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let t = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, e) in t.iter_mut().enumerate() {
            *e = (0..8).fold(i as u32, |c, _| (c >> 1) ^ (0xEDB8_8320 & (c & 1).wrapping_neg()));
        }
        t
    });
    !d.iter().fold(!0u32, |c, &b| t[((c ^ b as u32) & 0xff) as usize] ^ (c >> 8))
}

/// Reads `n` bytes of a zip from `at`.
pub type ReadAt<'a> = dyn FnMut(u64, usize) -> Result<Vec<u8>, String> + 'a;

/// One file in a zip's directory, its data not read yet.
#[derive(Clone, Debug)]
pub struct ZipFile {
    /// Its path in the mod (the zip's one top folder, if it has one, dropped).
    pub name: String,
    method: usize,
    crc: u32,
    packed: u64,
    size: u64,
    local: u64,
    whole: String,
}

/// A mod's zip's directory, every entry checked before any is read: a plain file under a [`plain`]
/// path with one of [`EXTENSIONS`], no link or special file, no duplicate (case-blind), stored or
/// deflated, not encrypted, at most [`MAX_ENTRIES`]; `mod.json` at the top or in the one folder
/// everything is in. macOS's `__MACOSX/` and `.DS_Store` are left out; anything else wrong refuses the
/// whole zip.
#[derive(Debug)]
pub struct ZipDir {
    pub len: u64,
    pub files: Vec<ZipFile>,
}

impl ZipDir {
    pub fn read(len: u64, read: &mut ReadAt) -> Result<ZipDir, String> {
        if len > MAX_ZIP {
            return Err("the zip is too big".into());
        }
        let mut at = |from: u64, n: usize| if from.checked_add(n as u64).is_some_and(|e| e <= len) { read(from, n) } else { Err("the zip is cut short".into()) };
        // the end of central directory record: the last one, within its comment's 64 KB of the end
        let from = len.saturating_sub(22 + 0xffff);
        let tail = at(from, (len - from) as usize)?;
        let end = (0..=tail.len().saturating_sub(22)).rev().find(|&i| tail[i..].starts_with(b"PK\x05\x06")).ok_or("not a zip file")?;
        let (n, size, dir_at) = (u16_at(&tail, end + 10)?, u32_at(&tail, end + 12)?, u32_at(&tail, end + 16)?);
        if u16_at(&tail, end + 4)? != 0 || u16_at(&tail, end + 6)? != 0 || u16_at(&tail, end + 8)? != n || n == 0xffff || dir_at == u32::MAX {
            return Err("split and zip64 archives aren't taken".into());
        }
        if n > MAX_ENTRIES {
            return Err(format!("more than {MAX_ENTRIES} files"));
        }
        if size as usize > n * (46 + 3 * 0xffff) {
            return Err("a damaged zip directory".into());
        }
        let dir = at(dir_at as u64, size as usize)?;
        let (mut i, mut seen, mut files) = (0, std::collections::HashSet::new(), Vec::new());
        for _ in 0..n {
            if u32_at(&dir, i)? != 0x0201_4b50 {
                return Err("a damaged zip directory".into());
            }
            let (made, flags, method) = (u16_at(&dir, i + 4)?, u16_at(&dir, i + 8)?, u16_at(&dir, i + 10)?);
            let (crc, packed, size) = (u32_at(&dir, i + 16)?, u32_at(&dir, i + 20)?, u32_at(&dir, i + 24)?);
            let (nlen, xlen, clen) = (u16_at(&dir, i + 28)?, u16_at(&dir, i + 30)?, u16_at(&dir, i + 32)?);
            let (attrs, local) = (u32_at(&dir, i + 38)?, u32_at(&dir, i + 42)? as u64);
            let raw = dir.get(i + 46..i + 46 + nlen).ok_or("the zip is cut short")?;
            i += 46 + nlen + xlen + clen;
            let name = std::str::from_utf8(raw).map_err(|_| "a file name that isn't UTF-8")?;
            if !plain(name) {
                return Err(format!("`{name}`: not a plain path inside the mod"));
            }
            // the Unix file type (made on Unix or macOS): plain files and folders only
            let kind = attrs >> 16 & 0o170000;
            if matches!(made >> 8, 3 | 19) && !matches!(kind, 0 | 0o100000 | 0o040000) {
                return Err(format!("`{name}`: a link or special file"));
            }
            if !seen.insert(name.trim_end_matches('/').to_lowercase()) {
                return Err(format!("`{name}` is in the zip twice"));
            }
            if flags & 1 != 0 {
                return Err(format!("`{name}` is encrypted"));
            }
            if name.ends_with('/') || attrs & 0x10 != 0 || kind == 0o040000 {
                if size != 0 {
                    return Err(format!("`{name}`: a folder with data"));
                }
                continue;
            }
            let file = name.rsplit('/').next().unwrap_or(name);
            if name.starts_with("__MACOSX/") || file == ".DS_Store" {
                continue;
            }
            let ext = file.rsplit_once('.').map(|e| e.1.to_ascii_lowercase()).unwrap_or_default();
            if !EXTENSIONS.contains(&ext.as_str()) {
                return Err(format!("`{name}`: a .{ext} file, and a mod is data only"));
            }
            if !matches!(method, 0 | 8) {
                return Err(format!("`{name}`: packed with a method other than stored or deflate"));
            }
            if packed as u64 + local > len {
                return Err(format!("`{name}`: damaged in the zip"));
            }
            files.push(ZipFile { name: name.to_string(), method, crc, packed: packed as u64, size: size as u64, local, whole: name.to_string() });
        }
        // `mod.json` at the top, or one folder holding everything (a zipped mod folder), dropped
        let top = if files.iter().any(|f| f.name == "mod.json") {
            String::new()
        } else {
            let top = files.first().and_then(|f| f.name.split_once('/')).map(|t| format!("{}/", t.0)).ok_or("no mod.json in the zip")?;
            if !files.iter().all(|f| f.name.starts_with(&top)) || !files.iter().any(|f| f.name == format!("{top}mod.json")) {
                return Err("no mod.json in the zip (at the top, or in the one folder everything is in)".into());
            }
            top
        };
        for f in &mut files {
            f.name = f.name[top.len()..].to_string();
        }
        if files.iter().any(|f| f.name.eq_ignore_ascii_case(STAMP)) {
            return Err(format!("`{STAMP}` is the store's own, and a mod cannot bring one"));
        }
        Ok(ZipDir { len, files })
    }

    /// The file `f`, inflated and checked against its CRC, at most `cap` bytes.
    pub fn file(&self, f: &ZipFile, cap: u64, read: &mut ReadAt) -> Result<Vec<u8>, String> {
        let name = &f.whole;
        let cap = cap.min(MAX_FILE);
        if f.size > cap || (f.method == 0 && f.packed > cap) {
            return Err(format!("`{}` is too big", f.name));
        }
        let start = 30 + name.len() as u64;
        if f.local + start > self.len {
            return Err(format!("`{}`: damaged in the zip", f.name));
        }
        let head = read(f.local, start as usize)?;
        if u32_at(&head, 0)? != 0x0403_4b50 || &head[30..] != name.as_bytes() {
            return Err(format!("`{}`: damaged in the zip", f.name));
        }
        let from = f.local + start + u16_at(&head, 28)? as u64;
        if from + f.packed > self.len {
            return Err(format!("`{}`: damaged in the zip", f.name));
        }
        let data = read(from, f.packed as usize)?;
        let bytes = match f.method {
            0 => data,
            // never more out than the directory says is in: a bomb stops at its own claim
            _ => miniz_oxide::inflate::decompress_to_vec_with_limit(&data, f.size as usize).map_err(|_| format!("`{}`: damaged in the zip", f.name))?,
        };
        if bytes.len() as u64 != f.size || crc32(&bytes) != f.crc {
            return Err(format!("`{}`: damaged in the zip", f.name));
        }
        Ok(bytes)
    }

    /// Every file, in turn, to `put`, the total held to [`MAX_TOTAL`] as they inflate.
    pub fn each(&self, read: &mut ReadAt, mut put: impl FnMut(&str, Vec<u8>) -> Result<(), String>) -> Result<u64, String> {
        let mut total = 0;
        for f in &self.files {
            let b = self.file(f, MAX_TOTAL - total, read)?;
            total += b.len() as u64;
            put(&f.name, b)?;
        }
        Ok(total)
    }

    /// Its `mod.json`, checked against what the store says the mod is: the standard's fields this
    /// build reads, its id and kind the store's.
    pub fn check_mod_json(&self, read: &mut ReadAt, meta: &Meta) -> Result<(), String> {
        let f = self.files.iter().find(|f| f.name == "mod.json").ok_or("no mod.json in the zip")?;
        check_mod_json(&self.file(f, MAX_META, read)?, meta)
    }
}

/// A mod's `mod.json` against its store [`Meta`].
pub fn check_mod_json(b: &[u8], meta: &Meta) -> Result<(), String> {
    let text = std::str::from_utf8(b).map_err(|_| "mod.json is not UTF-8")?;
    let json = Json::parse(text).map_err(|e| format!("mod.json: {e}"))?;
    let string = |key: &str| json.get(key).and_then(Json::str).ok_or_else(|| format!("mod.json has no {key}"));
    let standard = json.get("standard").and_then(Json::num).and_then(|n| n.parse::<u64>().ok()).ok_or("mod.json: standard is not a whole number")?;
    if standard > track_mods::STANDARD {
        return Err(format!("mod.json is standard {standard}, and this build reads {}", track_mods::STANDARD));
    }
    let (id, kind) = (string("id")?, string("kind")?);
    if id != meta.id {
        return Err(format!("mod.json's id is {id}, and the store lists it as {}", meta.id));
    }
    if kind != meta.kind.word() {
        return Err(format!("mod.json's kind is {kind}, and the store lists it as {}", meta.kind.word()));
    }
    Ok(())
}

/// The staged copy in `stage` (its `<kind folder>/<id>/`) read the way the shell reads its kind, as
/// hostile input: a track's mod.json and its one scene, a world's pieces, a trick pack's tables, a
/// music pack's listing. The browser's build has none of the shell's mods readers, and checks the zip
/// and `mod.json` alone.
#[cfg(not(target_arch = "wasm32"))]
pub fn parse_check(stage: &Path, kind: Kind, id: &str) -> Result<(), String> {
    match kind {
        Kind::Track => track_mods::find(stage, id).map(|_| ()),
        Kind::World => crate::worlds::find(stage, id).map(|_| ()),
        Kind::Tricks => crate::trick_packs::by_id(stage, id).map(|_| ()),
        Kind::Music => track_mods::music_packs(stage).iter().any(|p| p.0 == id).then_some(()).ok_or_else(|| "not a music pack the shell reads".into()),
        Kind::Other => Err("a kind this build does not know".into()),
    }
}

// ---- previews ----------------------------------------------------------------------------------

/// A `preview.webp` (or a PNG, which an upload may carry instead) decoded and made at most
/// [`PREVIEW_W`] wide; None if it does not decode or is bigger than 4096 a side.
pub fn decode_preview(b: &[u8]) -> Option<Picture> {
    #[cfg(target_arch = "wasm32")]
    let png = false;
    #[cfg(not(target_arch = "wasm32"))]
    let png = b.starts_with(b"\x89PNG");
    let (w, h, rgba) = if png {
        png_rgba(b)?
    } else {
        let mut d = image_webp::WebPDecoder::new(std::io::Cursor::new(b)).ok()?;
        let (w, h) = d.dimensions();
        if w == 0 || h == 0 || w > 4096 || h > 4096 {
            return None;
        }
        let mut buf = vec![0; d.output_buffer_size()?];
        d.read_image(&mut buf).ok()?;
        let rgba = if d.has_alpha() { buf } else { buf.chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect() };
        (w, h, rgba)
    };
    Some(shrink(Picture { width: w as usize, height: h as usize, rgba }, PREVIEW_W as usize))
}

/// A PNG preview's size and RGBA.
#[cfg(not(target_arch = "wasm32"))]
fn png_rgba(b: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut d = png::Decoder::new(std::io::Cursor::new(b));
    d.set_transformations(png::Transformations::normalize_to_color8() | png::Transformations::ALPHA);
    let mut r = d.read_info().ok()?;
    let (w, h) = (r.info().width, r.info().height);
    if w > 4096 || h > 4096 {
        return None;
    }
    let mut buf = vec![0; r.output_buffer_size()?];
    let info = r.next_frame(&mut buf).ok()?;
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..w as usize * h as usize * 4].to_vec(),
        png::ColorType::GrayscaleAlpha => buf.chunks(2).take(w as usize * h as usize).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        _ => return None,
    };
    Some((w, h, rgba))
}

#[cfg(target_arch = "wasm32")]
fn png_rgba(_: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    None
}

/// `p` box-filtered down to at most `most` wide.
fn shrink(p: Picture, most: usize) -> Picture {
    if p.width <= most {
        return p;
    }
    let (w, h) = (most, (p.height * most / p.width).max(1));
    let mut rgba = vec![0; w * h * 4];
    for y in 0..h {
        let (y0, y1) = (y * p.height / h, ((y + 1) * p.height / h).max(y * p.height / h + 1));
        for x in 0..w {
            let (x0, x1) = (x * p.width / w, ((x + 1) * p.width / w).max(x * p.width / w + 1));
            let mut sum = [0u32; 4];
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let at = 4 * (sy * p.width + sx);
                    for c in 0..4 {
                        sum[c] += p.rgba[at + c] as u32;
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            for c in 0..4 {
                rgba[4 * (y * w + x) + c] = (sum[c] / n) as u8;
            }
        }
    }
    Picture { width: w, height: h, rgba }
}

// ---- the store ---------------------------------------------------------------------------------

#[derive(Default)]
struct Shared {
    events: Mutex<Vec<Event>>,
    jobs: Mutex<Jobs>,
}

#[derive(Default)]
struct Jobs {
    running: usize,
    queue: VecDeque<Entry>,
    /// Every install queued or running, by [`Entry::key`].
    busy: Vec<String>,
    /// The browser's downloads under way: the key, the download's name, and its size.
    #[cfg(target_arch = "wasm32")]
    fetching: Vec<(String, String, u64)>,
}

/// The store, as the shell talks to it: where it is, the folder beside settings.txt its listing and
/// previews are kept in, and the `mods/` folder installs go into.
#[derive(Clone)]
pub struct Store {
    base: String,
    dir: PathBuf,
    mods: PathBuf,
    shared: Arc<Shared>,
    client: reqwest::Client,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store").field("base", &self.base).field("mods", &self.mods).finish()
    }
}

impl Store {
    /// The store at `base` ([`HF`], or a test's own server), its listing kept in `dir`, installing into
    /// `mods`.
    pub fn new(base: &str, dir: &Path, mods: &Path) -> Store {
        #[cfg(not(target_arch = "wasm32"))]
        let _ = rustls::crypto::ring::default_provider().install_default();
        #[cfg(not(target_arch = "wasm32"))]
        let client = reqwest::Client::builder().connect_timeout(std::time::Duration::from_secs(20)).read_timeout(std::time::Duration::from_secs(60)).build().expect("the HTTP client");
        #[cfg(target_arch = "wasm32")]
        let client = reqwest::Client::new();
        Store { base: base.trim_end_matches('/').to_string(), dir: dir.to_path_buf(), mods: mods.to_path_buf(), shared: Arc::default(), client }
    }

    pub fn mods(&self) -> &Path {
        &self.mods
    }

    fn send(&self, e: Event) {
        self.shared.events.lock().unwrap().push(e);
    }

    /// What finished since the last call; and the next queued installs started (B339).
    pub fn take(&self) -> Vec<Event> {
        self.pump();
        std::mem::take(&mut *self.shared.events.lock().unwrap())
    }

    /// Whether any install is queued or running.
    pub fn busy(&self) -> bool {
        !self.shared.jobs.lock().unwrap().busy.is_empty()
    }

    fn file_url(&self, rev: &str, path: &str) -> String {
        format!("{}/datasets/{REPO}/resolve/{}/{path}", self.base, rev_part(rev))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn spawn(&self, f: impl std::future::Future<Output = ()> + Send + 'static) {
        std::thread::spawn(move || match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt.block_on(f),
            Err(e) => println!("mod store: no runtime for a job: {e}"),
        });
    }

    #[cfg(target_arch = "wasm32")]
    fn spawn(&self, f: impl std::future::Future<Output = ()> + 'static) {
        wasm_bindgen_futures::spawn_local(f);
    }

    /// GET `url` of the store: its answer, unless it says it is more than `cap` bytes.
    async fn answer(&self, url: &str, cap: u64) -> Result<reqwest::Response, String> {
        let r = self.client.get(url).send().await.map_err(|e| format!("can't reach the store: {e}"))?;
        // M116: Hugging Face rate-limits by address (429), and a 5xx passes too
        if r.status().as_u16() == 429 || r.status().is_server_error() {
            return Err(BUSY.into());
        }
        if !r.status().is_success() {
            return Err(format!("the store said {}", r.status().as_u16()));
        }
        if r.content_length().is_some_and(|n| n > cap) {
            return Err("too big".into());
        }
        Ok(r)
    }

    /// GET `url` of the store, at most `cap` bytes.
    async fn get(&self, url: &str, cap: u64) -> Result<Vec<u8>, String> {
        let b = self.answer(url, cap).await?.bytes().await.map_err(|e| format!("download cut short: {e}"))?;
        if b.len() as u64 > cap {
            return Err("too big".into());
        }
        Ok(b.to_vec())
    }

    /// The mod ids under `mods/` at `rev` (none when it has no `mods/`).
    async fn ids(&self, rev: &str) -> Result<Vec<String>, String> {
        match self.get(&format!("{}/api/datasets/{REPO}/tree/{}/mods", self.base, rev_part(rev)), 4 << 20).await {
            Ok(b) => Ok(ids_in(&b)),
            Err(e) if e.starts_with("the store said 404") => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    /// Open pull requests, newest first: pages of 50 only until the reply's count is in.
    async fn pulls(&self) -> Result<Vec<(u32, String)>, String> {
        let (mut out, mut seen) = (Vec::new(), 0);
        for p in 0..20 {
            let url = format!("{}/api/datasets/{REPO}/discussions?type=pull_request&status=open&p={p}", self.base);
            let (page, n, count) = pulls_in(&self.get(&url, 4 << 20).await?);
            out.extend(page);
            seen += n;
            if n == 0 || seen >= count {
                break;
            }
        }
        out.sort_by_key(|p| std::cmp::Reverse(p.0));
        Ok(out)
    }

    async fn heads(&self) -> Result<BTreeMap<u32, String>, String> {
        Ok(heads_in(&self.get(&format!("{}/api/datasets/{REPO}/refs?include_prs=1", self.base), 4 << 20).await?))
    }

    async fn index(&self) -> Result<Option<Index>, String> {
        match self.get(&self.file_url("main", "index.json"), MAX_LISTING).await {
            Ok(b) => Ok(serde_json::from_slice::<Index>(&b).ok()),
            Err(e) if e == BUSY => Err(e),
            Err(_) => Ok(None),
        }
    }

    async fn get_meta(&self, rev: &str, id: &str) -> Result<Option<Meta>, String> {
        match self.get(&self.file_url(rev, &format!("mods/{id}/meta.json")), MAX_META).await {
            Ok(b) => Ok(meta_of(&b, id)),
            Err(e) if e == BUSY => Err(e),
            Err(_) => Ok(None),
        }
    }

    async fn entries(&self, rev: String, author: Option<String>) -> Result<Vec<Entry>, String> {
        let ids = self.ids(&rev).await?;
        let metas = in_turn(ids.iter().map(|id| self.get_meta(&rev, id)).collect()).await;
        let metas = metas.into_iter().collect::<Result<Vec<_>, _>>()?;
        Ok(metas.into_iter().flatten().map(|m| entry(m, &rev, author.as_deref())).collect())
    }

    /// The store's mods, unmerged: `main`'s, then each open PR's newest first. M116: the index, the
    /// open PRs and their heads (three requests), and the trees of only the PRs the index has not read.
    async fn lists(&self) -> Result<Vec<Vec<Entry>>, String> {
        let (ix, open, heads) = (self.index().await?, self.pulls().await?, self.heads().await?);
        let mut out = Vec::new();
        for p in parts(ix, open, &heads) {
            out.push(match p {
                Part::Have(l) => l,
                Part::Read(rev, who) => self.entries(rev, who).await?,
            });
        }
        Ok(out)
    }

    fn cached(&self) -> Option<Cached> {
        crate::files::read(self.dir.join(CACHE)).ok().and_then(|b| serde_json::from_slice(&b).ok())
    }

    /// Lists the store ([`Event::Listing`]): the kept listing while it is under an hour old and not
    /// `fresh`. M116: a store that cannot be read leaves the last listing up, with a note saying why.
    pub fn list(&self, fresh: bool) {
        let cached = self.cached();
        if !fresh && let Some(c) = cached.as_ref().filter(|c| now_secs().saturating_sub(c.at) < CACHE_SECS) {
            return self.send(Event::Listing(Ok(c.entries.clone())));
        }
        let s = self.clone();
        self.spawn(async move {
            match s.lists().await {
                Ok(l) => {
                    let all = merge(l);
                    let _ = crate::files::write(s.dir.join(CACHE), serde_json::to_vec(&Cached { at: now_secs(), entries: all.clone() }).unwrap_or_default());
                    s.send(Event::Listing(Ok(all)));
                }
                Err(e) => match cached.filter(|c| !c.entries.is_empty()) {
                    Some(c) => {
                        s.send(Event::Listing(Ok(c.entries)));
                        s.send(Event::Note(if e == BUSY { e } else { format!("Showing the last listing: {e}") }));
                    }
                    None => s.send(Event::Listing(Err(if e == BUSY { e } else { format!("Can't read the mod store: {e}") }))),
                },
            }
        });
    }

    /// The listing goes stale: the next Browse reads the store again.
    fn stale(&self) {
        if let Some(mut c) = self.cached() {
            c.at = 0;
            let _ = crate::files::write(self.dir.join(CACHE), serde_json::to_vec(&c).unwrap_or_default());
        }
    }

    /// Fetches `e`'s preview ([`Event::Preview`] by its zip's sha256): the kept one first.
    pub fn preview(&self, e: &Entry) {
        let sha = e.meta.sha256.clone();
        let kept = self.dir.join(PREVIEWS).join(format!("{sha}.webp"));
        let url = self.file_url(&e.rev, &format!("mods/{}/preview.webp", e.meta.id));
        let s = self.clone();
        self.spawn(async move {
            let b = match crate::files::read(&kept) {
                Ok(b) => b,
                Err(_) => match s.get(&url, MAX_PREVIEW).await {
                    Ok(b) => {
                        let _ = crate::files::create_dir_all(s.dir.join(PREVIEWS)).and_then(|_| crate::files::write(&kept, &b));
                        b
                    }
                    // none in the store: kept as none, till the zip changes
                    Err(e) if e.starts_with("the store said 404") => {
                        let _ = crate::files::create_dir_all(s.dir.join(PREVIEWS)).and_then(|_| crate::files::write(&kept, b""));
                        Vec::new()
                    }
                    Err(_) => Vec::new(),
                },
            };
            s.send(Event::Preview(sha, decode_preview(&b)));
        });
    }

    // ---- what is installed -------------------------------------------------------------------

    /// Every installed store mod's stamp.
    pub fn stamps(&self) -> Vec<Stamp> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut out = Vec::new();
            for folder in ["tracks", "tricks", "music"] {
                let Ok(list) = std::fs::read_dir(self.mods.join(folder)) else { continue };
                for e in list.flatten() {
                    if let Some(s) = std::fs::read(e.path().join(STAMP)).ok().and_then(|b| serde_json::from_slice::<Stamp>(&b).ok()) {
                        if e.file_name().to_str() == Some(s.id.as_str()) && s.kind.folder() == Some(folder) {
                            out.push(s);
                        }
                    }
                }
            }
            out.sort_by(|a, b| a.id.cmp(&b.id));
            out
        }
        #[cfg(target_arch = "wasm32")]
        self.web_installed()
    }

    /// Whether `mods/<folder>/<id>` is there at all.
    fn present(&self, kind: Kind, id: &str) -> bool {
        let Some(folder) = kind.folder() else { return false };
        #[cfg(not(target_arch = "wasm32"))]
        return self.mods.join(folder).join(id).exists();
        #[cfg(target_arch = "wasm32")]
        {
            let _ = folder;
            self.web_installed().iter().any(|s| s.id == id && s.kind.folder() == kind.folder())
        }
    }

    /// `e` against what is installed: B248, by its id and author, and its zip for an update.
    pub fn have(e: &Entry, stamps: &[Stamp], own: &[String]) -> Have {
        match stamps.iter().find(|s| s.id == e.meta.id && s.kind.folder() == e.meta.kind.folder()) {
            Some(s) if s.sha256 == e.meta.sha256 => Have::Installed,
            Some(_) => Have::Update,
            None if own.iter().any(|o| *o == e.meta.id) => Have::Own,
            None => Have::No,
        }
    }

    /// The ids in the mods folder that are not the store's (no [`STAMP`]), which Install never
    /// overwrites.
    pub fn own_ids(&self) -> Vec<String> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut out = Vec::new();
            for folder in ["tracks", "tricks", "music"] {
                let Ok(list) = std::fs::read_dir(self.mods.join(folder)) else { continue };
                for e in list.flatten() {
                    if !e.path().join(STAMP).exists() && let Ok(id) = e.file_name().into_string() {
                        out.push(id);
                    }
                }
            }
            out
        }
        #[cfg(target_arch = "wasm32")]
        Vec::new()
    }

    // ---- install, remove -------------------------------------------------------------------------

    /// Queues `e`'s install ([`Event::Progress`], then [`Event::Installed`]); one already queued or
    /// running is left to finish.
    pub fn install(&self, e: &Entry) {
        let mut j = self.shared.jobs.lock().unwrap();
        if j.busy.contains(&e.key()) {
            return;
        }
        j.busy.push(e.key());
        j.queue.push_back(e.clone());
        drop(j);
        self.send(Event::Progress(e.key(), 0.0, format!("Waiting to download {}", e.meta.name)));
        self.pump();
    }

    /// Starts queued installs while fewer than [`PARALLEL`] run.
    fn pump(&self) {
        loop {
            let mut j = self.shared.jobs.lock().unwrap();
            #[cfg(target_arch = "wasm32")]
            for (key, name, size) in &j.fetching {
                let got = crate::web_host::store_got(name);
                self.shared.events.lock().unwrap().push(Event::Progress(key.clone(), (got as f32 / (*size).max(1) as f32).min(1.0) * 0.9, progress_line(got, *size)));
            }
            if j.running >= PARALLEL {
                return;
            }
            let Some(e) = j.queue.pop_front() else { return };
            j.running += 1;
            drop(j);
            let s = self.clone();
            self.spawn(async move {
                let got = s.install_now(&e).await;
                if got.is_ok() {
                    s.stale();
                }
                let mut j = s.shared.jobs.lock().unwrap();
                j.running -= 1;
                j.busy.retain(|k| *k != e.key());
                drop(j);
                s.send(Event::Installed(e.key(), got));
            });
        }
    }

    /// `e` downloaded, checked and installed: its name, or why not (and nothing kept).
    async fn install_now(&self, e: &Entry) -> Result<String, String> {
        let folder = e.meta.kind.folder().ok_or("a kind this build has no folder for")?;
        let id = &e.meta.id;
        if !track_mods::valid_id(id) {
            return Err(format!("`{id}` is not a mod id"));
        }
        if self.present(e.meta.kind, id) && !self.stamps().iter().any(|s| s.id == *id && s.kind.folder() == Some(folder)) {
            return Err(format!("{folder}/{id} is in your mods folder already, and not from the store: Install leaves it alone"));
        }
        let got = self.download(e).await?;
        self.send(Event::Progress(e.key(), 0.92, format!("Checking {}", e.meta.name)));
        let stamp = Stamp { id: id.clone(), kind: e.meta.kind, sha256: e.meta.sha256.clone(), rev: e.rev.clone(), verified: e.verified, author: e.meta.author.clone(), version: e.meta.version.clone() };
        self.unpack(&got, e, folder, &stamp).await?;
        Ok(e.meta.name.clone())
    }

    /// Takes the installed store mod `id` of `kind` out of the mods folder ([`Event::Removed`]).
    /// Only a store mod (it has its [`STAMP`]): the player's own folders are theirs to remove.
    pub fn remove(&self, key: &str, kind: Kind, id: &str) {
        let (key, id) = (key.to_string(), id.to_string());
        let Some(folder) = kind.folder() else { return };
        if !track_mods::valid_id(&id) || !self.stamps().iter().any(|s| s.id == id && s.kind.folder() == Some(folder)) {
            return self.send(Event::Removed(key, Err(format!("{folder}/{id} is not a store mod"))));
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let got = std::fs::remove_dir_all(self.mods.join(folder).join(&id)).map(|_| id.clone()).map_err(|e| format!("can't remove {folder}/{id}: {e}"));
            self.send(Event::Removed(key, got));
        }
        #[cfg(target_arch = "wasm32")]
        {
            let s = self.clone();
            self.spawn(async move {
                let got = crate::web_host::store_drop(&format!("mods/{folder}/{id}")).await.map(|_| id.clone());
                if got.is_ok() {
                    s.web_record(None, &id, folder);
                }
                s.send(Event::Removed(key, got));
            });
        }
    }
}

/// "12 / 340 MB".
fn progress_line(got: u64, size: u64) -> String {
    format!("Downloading {} / {} MB", got >> 20, (size >> 20).max(1))
}

/// Every future in `all`, one at a time: a crawl is only for a PR the index has not read, a few
/// metas each.
async fn in_turn<T, F: std::future::Future<Output = T>>(all: Vec<F>) -> Vec<T> {
    let mut out = Vec::with_capacity(all.len());
    for f in all {
        out.push(f.await);
    }
    out
}

// ---- the desktop's download and unpack -----------------------------------------------------------

/// A download's zip until it is installed: `mods/.download-<author>-<id>.zip` (one each, as several
/// download at once), removed when dropped.
#[cfg(not(target_arch = "wasm32"))]
struct Download(PathBuf);

#[cfg(not(target_arch = "wasm32"))]
impl Drop for Download {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// A file name part made of `s`: letters, digits, `_` and `-` only.
fn safe(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).take(40).collect()
}

#[cfg(not(target_arch = "wasm32"))]
impl Store {
    /// `e`'s `mod.zip` streamed into its [`Download`] (B204), hashed as it comes: only one of the
    /// listed size and sha256 is kept. A redirect to the store's CDN is followed with the request's
    /// own headers, and no Range is asked (B242's phones dropped one on the redirect).
    async fn download(&self, e: &Entry) -> Result<Download, String> {
        use std::io::Write;
        std::fs::create_dir_all(&self.mods).map_err(|err| format!("can't make {}: {err}", self.mods.display()))?;
        let at = Download(self.mods.join(format!(".download-{}-{}.zip", safe(&e.meta.author), e.meta.id)));
        let mut file = std::io::BufWriter::new(std::fs::File::create(&at.0).map_err(|err| format!("can't write {}: {err}", at.0.display()))?);
        let cap = e.meta.size;
        let mut r = self.answer(&self.file_url(&e.rev, &format!("mods/{}/mod.zip", e.meta.id)), cap).await?;
        let (mut got, mut hash) = (0u64, Sha256::new());
        while let Some(c) = r.chunk().await.map_err(|err| format!("download cut short: {err}"))? {
            let mb = got >> 20;
            got += c.len() as u64;
            if got > cap {
                return Err(format!("the download is more than the {cap} bytes the store says"));
            }
            hash.update(&c);
            file.write_all(&c).map_err(|err| format!("can't write {}: {err}", at.0.display()))?;
            if got >> 20 != mb {
                self.send(Event::Progress(e.key(), got as f32 / cap as f32 * 0.9, progress_line(got, cap)));
            }
        }
        file.flush().map_err(|err| format!("can't write {}: {err}", at.0.display()))?;
        if got != cap {
            return Err(format!("the download is {got} bytes, the store says {cap}"));
        }
        if hex(&hash.finalize()) != e.meta.sha256 {
            return Err("the download doesn't match its sha256 (damaged or changed)".into());
        }
        Ok(at)
    }

    /// The checked zip at `zip` into `mods/<folder>/<id>/`: unpacked into `mods/.store-<id>/` first,
    /// read there by the shell's parsers ([`parse_check`]), then swapped in for the old copy. Whatever
    /// goes wrong, the stage goes and the old copy stays.
    async fn unpack(&self, zip: &Download, e: &Entry, folder: &str, stamp: &Stamp) -> Result<(), String> {
        use std::io::{Read, Seek, SeekFrom};
        let id = &e.meta.id;
        let stage = self.mods.join(format!(".store-{id}"));
        let _ = std::fs::remove_dir_all(&stage);
        let got = (|| {
            let mut f = std::fs::File::open(&zip.0).map_err(|err| format!("can't read the download: {err}"))?;
            let len = f.metadata().map_err(|err| err.to_string())?.len();
            let mut read = |at: u64, n: usize| -> Result<Vec<u8>, String> {
                let mut b = vec![0; n];
                f.seek(SeekFrom::Start(at)).and_then(|_| f.read_exact(&mut b)).map_err(|_| "the zip is cut short".to_string())?;
                Ok(b)
            };
            let dir = ZipDir::read(len, &mut read)?;
            dir.check_mod_json(&mut read, &e.meta)?;
            let inside = stage.join(folder).join(id);
            dir.each(&mut read, |name, b| {
                let to = inside.join(name);
                to.parent().map(std::fs::create_dir_all).transpose().and_then(|_| std::fs::write(&to, b)).map_err(|err| format!("can't write {name}: {err}"))
            })?;
            parse_check(&stage, e.meta.kind, id)?;
            std::fs::write(inside.join(STAMP), serde_json::to_vec_pretty(stamp).unwrap_or_default()).map_err(|err| format!("can't write {STAMP}: {err}"))?;
            let target = self.mods.join(folder).join(id);
            std::fs::create_dir_all(self.mods.join(folder)).map_err(|err| err.to_string())?;
            // the old copy aside until the new one is in, so a failed swap puts it back
            let old = self.mods.join(format!(".store-old-{id}"));
            let _ = std::fs::remove_dir_all(&old);
            let had = target.exists();
            if had {
                std::fs::rename(&target, &old).map_err(|err| format!("can't move the old {folder}/{id} aside: {err}"))?;
            }
            if let Err(err) = std::fs::rename(&inside, &target) {
                if had {
                    let _ = std::fs::rename(&old, &target);
                }
                return Err(format!("can't put {folder}/{id} in place: {err}"));
            }
            let _ = std::fs::remove_dir_all(&old);
            Ok(())
        })();
        let _ = std::fs::remove_dir_all(&stage);
        got
    }
}

// ---- the browser's download and unpack -----------------------------------------------------------

/// A download in the page's OPFS (`web/worker.js`): fetched there by the worker through a sync
/// access handle (B204: streamed, never whole in memory; B358: no `createWritable`), removed when
/// dropped.
#[cfg(target_arch = "wasm32")]
struct Download(String, u64);

#[cfg(target_arch = "wasm32")]
impl Drop for Download {
    fn drop(&mut self) {
        crate::web_host::store_close(&self.0);
    }
}

#[cfg(target_arch = "wasm32")]
impl Store {
    fn web_installed(&self) -> Vec<Stamp> {
        crate::files::read(self.dir.join(INSTALLED)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    /// The browser's list of installed mods with `stamp` in for `id` (None: out).
    fn web_record(&self, stamp: Option<&Stamp>, id: &str, folder: &str) {
        let mut all = self.web_installed();
        all.retain(|s| !(s.id == id && s.kind.folder() == Some(folder)));
        all.extend(stamp.cloned());
        let _ = crate::files::write(self.dir.join(INSTALLED), serde_json::to_vec(&all).unwrap_or_default());
    }

    async fn download(&self, e: &Entry) -> Result<Download, String> {
        let name = format!("download-{}-{}.zip", safe(&e.meta.author), e.meta.id);
        let url = self.file_url(&e.rev, &format!("mods/{}/mod.zip", e.meta.id));
        self.shared.jobs.lock().unwrap().fetching.push((e.key(), name.clone(), e.meta.size));
        // the guard first, so a fetch cut short is closed and removed too
        let mut d = Download(name.clone(), 0);
        let got = crate::web_host::store_fetch(&url, &name, e.meta.size).await;
        self.shared.jobs.lock().unwrap().fetching.retain(|f| f.1 != name);
        d.1 = got?;
        if d.1 != e.meta.size {
            return Err(format!("the download is {} bytes, the store says {}", d.1, e.meta.size));
        }
        let (mut hash, mut at) = (Sha256::new(), 0u64);
        while at < d.1 {
            let n = (d.1 - at).min(4 << 20) as usize;
            hash.update(crate::web_host::store_read(&d.0, at, n)?);
            at += n as u64;
        }
        if hex(&hash.finalize()) != e.meta.sha256 {
            return Err("the download doesn't match its sha256 (damaged or changed)".into());
        }
        Ok(d)
    }

    /// The checked zip into OPFS `mods/<folder>/<id>/`, a file at a time; a failure takes the folder
    /// out again. An update's old copy goes first: OPFS cannot rename a folder into place.
    async fn unpack(&self, zip: &Download, e: &Entry, folder: &str, stamp: &Stamp) -> Result<(), String> {
        let id = &e.meta.id;
        let mut read = |at: u64, n: usize| crate::web_host::store_read(&zip.0, at, n);
        let dir = ZipDir::read(zip.1, &mut read)?;
        dir.check_mod_json(&mut read, &e.meta)?;
        let target = format!("mods/{folder}/{id}");
        let _ = crate::web_host::store_drop(&target).await;
        self.web_record(None, id, folder);
        let got = async {
            let mut total = 0;
            for f in &dir.files {
                let b = dir.file(f, MAX_TOTAL - total, &mut read)?;
                total += b.len() as u64;
                crate::web_host::store_put(&format!("{target}/{}", f.name), &b).await?;
            }
            crate::web_host::store_put(&format!("{target}/{STAMP}"), &serde_json::to_vec_pretty(stamp).unwrap_or_default()).await
        }
        .await;
        match got {
            Ok(()) => {
                self.web_record(Some(stamp), id, folder);
                Ok(())
            }
            Err(err) => {
                let _ = crate::web_host::store_drop(&target).await;
                Err(err)
            }
        }
    }
}

/// Opens `url` in the system's browser (Report), or the page's new tab.
pub fn open(url: &str) -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        crate::web_host::open_url(url);
        Ok(())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let opener = if cfg!(target_os = "macos") { "open" } else if cfg!(windows) { "explorer" } else { "xdg-open" };
        std::process::Command::new(opener).arg(url).spawn().map(|_| ()).map_err(|e| format!("{opener}: {e}"))
    }
}

#[cfg(test)]
#[path = "modstore_tests.rs"]
mod tests;
