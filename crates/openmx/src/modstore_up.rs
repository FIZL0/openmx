//! The mod store's upload side (P199, `plan/P198.md` § P199), opentennis's M19 (`hst-base/src/modstore.rs`)
//! on openmx's mods.
//!
//! Sign in: the player pastes a Hugging Face token with write access ([`TOKENS`] makes one); Hugging
//! Face's `whoami-v2` names its account, and the name and token are kept private: [`TOKEN`] beside
//! settings.txt, made owner-only (0600) before the token goes in, or the page's localStorage in the
//! browser (`web/index.html`, never in an Export).
//!
//! Upload my mod: the rules first (`menu::STORE_RULES`), then the mod's folder zipped ([`pack`]) and put through
//! the same checks Install puts a download through (the zip's directory, `mod.json` against its meta,
//! every entry inflated and CRC-checked, the shell's parsers for its kind), `meta.json` and
//! `preview.webp` made, and `mods/<id>/` committed as a pull request on [`REPO`] under the player's
//! account. A re-upload goes onto the player's own open PR, over the old copy ([`plan`]); a new mod
//! goes onto their newest open PR, so a player has one PR, not one per mod. Upload all: one commit a
//! mod, in turn, in the background, a status line each ([`Event::Upload`]).
//!
//! My public mods: the player's mods on their open PRs and on `main` ([`Store::mine`]); ✕ takes one
//! down ([`Store::take_down`]): a PR holding only it is closed, one holding more gets a commit deleting
//! `mods/<id>/`, and one on `main` gets a PR deleting it, for a maintainer to merge.
//!
//! The maintainer's side (`openmx --mod-store-admin`, [`admin`]): the dataset card and `index.json`
//! (M116: what Browse reads instead of every PR's tree) committed to `main`.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::{decode_preview, hex, meta_of, parts, plain, plain_text, rev_part, Cached, Entry, Event, Kind, Meta, Part, Store, ZipDir, CACHE, EXTENSIONS, MAX_ENTRIES, MAX_FILE, MAX_PREVIEW, MAX_TOTAL, MAX_ZIP, REPO, STAMP};
#[cfg(not(target_arch = "wasm32"))]
use super::{parse_check, safe, HF};
use crate::track_mods::{self, Json};

/// The kept token's file, beside settings.txt: the account's name, then the token, a line each.
pub const TOKEN: &str = "hf-token.txt";
/// The page where the player makes a token with write access, to paste.
pub const TOKENS: &str = "https://huggingface.co/settings/tokens/new?tokenType=write&description=openMX%20mods";

/// An upload's state, by the local mod's id.
#[derive(Clone, Debug, PartialEq)]
pub enum Up {
    Waiting,
    Working(String),
    Done(String),
    Skipped(String),
    Failed(String),
}

impl Up {
    /// Its status line.
    pub fn line(&self) -> &str {
        match self {
            Up::Waiting => "Waiting to upload",
            Up::Working(s) | Up::Done(s) | Up::Skipped(s) | Up::Failed(s) => s,
        }
    }

    /// Whether it is over (either way).
    pub fn over(&self) -> bool {
        !matches!(self, Up::Waiting | Up::Working(_))
    }
}

// ---- the token -----------------------------------------------------------------------------------

/// `text` as a Hugging Face token (`hf_…`), if it holds one: a pasted line may carry more.
pub fn token_in(text: &str) -> Option<String> {
    text.split(|c: char| !c.is_ascii_alphanumeric() && c != '_').find(|w| w.starts_with("hf_") && w.len() >= 30 && w.len() <= 200).map(str::to_string)
}

/// A kept token's text as (account, token).
pub fn account_in(text: &str) -> Option<(String, String)> {
    let (name, token) = text.trim().split_once('\n')?;
    let (name, token) = (plain_text(name, 40), token_in(token)?);
    (!name.is_empty()).then_some((name, token))
}

/// The signed-in account in `dir` (settings.txt's): its name and token.
pub fn account(dir: &Path) -> Option<(String, String)> {
    #[cfg(not(target_arch = "wasm32"))]
    return account_in(&std::fs::read_to_string(dir.join(TOKEN)).ok()?);
    #[cfg(target_arch = "wasm32")]
    {
        let _ = dir;
        account_in(&crate::web_host::token())
    }
}

/// Keeps `text` as the token (empty: signed out).
fn keep(dir: &Path, text: &str) -> Result<(), String> {
    #[cfg(not(target_arch = "wasm32"))]
    return write_private(&dir.join(TOKEN), text).map_err(|e| format!("can't keep the token: {e}"));
    #[cfg(target_arch = "wasm32")]
    {
        let _ = dir;
        crate::web_host::token_keep(text);
        Ok(())
    }
}

/// Writes `text` to `path`, a file only its owner can read (0600 from the moment it is made, and an
/// older file tightened before the text goes in).
#[cfg(not(target_arch = "wasm32"))]
pub fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)?;
        f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        f.write_all(text.as_bytes())
    }
    #[cfg(not(unix))]
    std::fs::File::create(path)?.write_all(text.as_bytes())
}

/// What is on the clipboard (the token's paste: the on-screen keyboard has no `_` or capitals). The
/// desktop asks the system's own tool; the page keeps the last thing pasted into it.
pub fn clipboard() -> Result<String, String> {
    #[cfg(target_arch = "wasm32")]
    {
        let t = crate::web_host::pasted();
        if t.is_empty() {
            return Err("Paste the token into the page (Ctrl+V), then pick Paste token".into());
        }
        Ok(t)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let tools: &[&[&str]] = if cfg!(target_os = "macos") {
            &[&["pbpaste"]]
        } else if cfg!(windows) {
            &[&["powershell", "-NoProfile", "-Command", "Get-Clipboard"]]
        } else {
            &[&["wl-paste", "--no-newline"], &["xclip", "-o", "-selection", "clipboard"], &["xsel", "-ob"]]
        };
        for t in tools {
            if let Ok(out) = std::process::Command::new(t[0]).args(&t[1..]).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output() {
                if out.status.success() {
                    return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
                }
            }
        }
        Err("Can't read the clipboard (wl-paste, xclip or xsel)".into())
    }
}

// ---- the player's own mods -------------------------------------------------------------------

/// One of the player's own mods in the mods folder (not a store install: no [`STAMP`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Local {
    pub id: String,
    pub kind: Kind,
    pub name: String,
    pub version: String,
    /// `mod.json`'s `source.game`.
    pub source: String,
    pub dir: PathBuf,
}

/// A mod folder's `mod.json` read as a [`Local`], if it is one of `folder`'s kinds.
pub fn local_of(dir: &Path, folder: &str) -> Option<Local> {
    let text = crate::files::read(dir.join("mod.json")).ok()?;
    let json = Json::parse(std::str::from_utf8(&text).ok()?).ok()?;
    let s = |k: &str| json.get(k).and_then(Json::str).map(|v| plain_text(v, 48)).unwrap_or_default();
    let id = s("id");
    let kind = serde_json::from_value::<Kind>(Value::String(s("kind"))).ok().filter(|k| k.folder() == Some(folder))?;
    if !track_mods::valid_id(&id) || dir.file_name().and_then(|n| n.to_str()) != Some(id.as_str()) {
        return None;
    }
    let source = json.get("source").and_then(|j| j.get("game").or(Some(j))).and_then(Json::str).map(|v| plain_text(v, 40)).unwrap_or_default();
    let name = Some(s("name")).filter(|n| !n.is_empty()).unwrap_or_else(|| id.clone());
    Some(Local { id, kind, name, version: s("version"), source, dir: dir.to_path_buf() })
}

/// The player's own mods in `mods`, by name: every folder of a kind the store takes that is not a
/// store install. The browser has none (its mods folder holds only the store's).
pub fn locals(mods: &Path) -> Vec<Local> {
    let mut out: Vec<Local> = Vec::new();
    #[cfg(not(target_arch = "wasm32"))]
    for folder in ["tracks", "tricks", "music"] {
        let Ok(list) = std::fs::read_dir(mods.join(folder)) else { continue };
        for e in list.flatten() {
            if e.path().is_dir() && !e.path().join(STAMP).exists() && let Some(l) = local_of(&e.path(), folder) {
                out.push(l);
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = mods;
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.id.cmp(&b.id)));
    out
}

// ---- packing: a mod folder as the store's mod.zip ------------------------------------------------

/// Every file under `dir`, as (its path in the mod, where it is), sorted; checked as Install checks a
/// zip's entries: plain paths, data extensions, no links, within the caps.
#[cfg(not(target_arch = "wasm32"))]
pub fn files_of(dir: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    fn walk(at: &Path, rel: &str, out: &mut Vec<(String, PathBuf)>, total: &mut u64) -> Result<(), String> {
        let mut list: Vec<_> = std::fs::read_dir(at).map_err(|e| format!("can't read {}: {e}", at.display()))?.flatten().collect();
        list.sort_by_key(|e| e.file_name());
        for e in list {
            let name = e.file_name().into_string().map_err(|_| format!("a file name in {} that isn't UTF-8", at.display()))?;
            let path = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            let meta = std::fs::symlink_metadata(e.path()).map_err(|err| format!("can't read {path}: {err}"))?;
            if name == ".DS_Store" || (rel.is_empty() && name == STAMP) {
                continue;
            }
            if !plain(&path) {
                return Err(format!("`{path}`: not a plain file name"));
            }
            if meta.file_type().is_symlink() {
                return Err(format!("`{path}`: a link, and a mod is plain files"));
            }
            if meta.is_dir() {
                walk(&e.path(), &path, out, total)?;
                continue;
            }
            if !meta.is_file() {
                return Err(format!("`{path}`: not a plain file"));
            }
            let ext = name.rsplit_once('.').map(|x| x.1.to_ascii_lowercase()).unwrap_or_default();
            if !EXTENSIONS.contains(&ext.as_str()) {
                return Err(format!("`{path}`: a .{ext} file, and a mod is data only"));
            }
            if meta.len() > MAX_FILE {
                return Err(format!("`{path}` is more than {} MB", MAX_FILE >> 20));
            }
            *total += meta.len();
            if *total > MAX_TOTAL {
                return Err(format!("more than {} MB of files", MAX_TOTAL >> 20));
            }
            out.push((path, e.path()));
            if out.len() > MAX_ENTRIES {
                return Err(format!("more than {MAX_ENTRIES} files"));
            }
        }
        Ok(())
    }
    let (mut out, mut total) = (Vec::new(), 0);
    walk(dir, "", &mut out, &mut total)?;
    if !out.iter().any(|f| f.0 == "mod.json") {
        return Err("no mod.json".into());
    }
    Ok(out)
}

/// Writes `files` as a zip to `to` (deflated, but for pictures and sound already packed), streaming:
/// one file in memory at a time. Its size and sha256.
#[cfg(not(target_arch = "wasm32"))]
pub fn pack(files: &[(String, PathBuf)], to: &Path) -> Result<(u64, String), String> {
    use std::io::Write;
    let mut out = std::io::BufWriter::new(std::fs::File::create(to).map_err(|e| format!("can't write {}: {e}", to.display()))?);
    let (mut at, mut hash, mut dir) = (0u64, Sha256::new(), Vec::new());
    let mut put = |b: &[u8], at: &mut u64| -> Result<(), String> {
        *at += b.len() as u64;
        if *at > MAX_ZIP {
            return Err(format!("the zip is more than {} MB", MAX_ZIP >> 20));
        }
        hash.update(b);
        out.write_all(b).map_err(|e| format!("can't write the zip: {e}"))
    };
    let le16 = |n: usize| (n as u16).to_le_bytes();
    let le32 = |n: u64| (n as u32).to_le_bytes();
    for (name, path) in files {
        let data = std::fs::read(path).map_err(|e| format!("can't read {name}: {e}"))?;
        let crc = super::crc32(&data);
        let packed_already = matches!(name.rsplit_once('.').map(|x| x.1.to_ascii_lowercase()).as_deref(), Some("ogg" | "png" | "webp"));
        let deflated = (!packed_already).then(|| miniz_oxide::deflate::compress_to_vec(&data, 6)).filter(|d| d.len() < data.len());
        let (method, body) = match &deflated {
            Some(d) => (8, d.as_slice()),
            None => (0, data.as_slice()),
        };
        let local = at;
        // a local header: version 2.0, UTF-8 names, 1980-01-01
        let head = [&0x0403_4b50u32.to_le_bytes()[..], &le16(20), &le16(0x0800), &le16(method), &le16(0), &le16(0x21), &crc.to_le_bytes(), &le32(body.len() as u64), &le32(data.len() as u64), &le16(name.len()), &le16(0), name.as_bytes()].concat();
        put(&head, &mut at)?;
        put(body, &mut at)?;
        // its directory entry: made on Unix, a plain file 0644
        dir.extend([
            &0x0201_4b50u32.to_le_bytes()[..],
            &le16(3 << 8 | 20),
            &le16(20),
            &le16(0x0800),
            &le16(method),
            &le16(0),
            &le16(0x21),
            &crc.to_le_bytes(),
            &le32(body.len() as u64),
            &le32(data.len() as u64),
            &le16(name.len()),
            &le16(0),
            &le16(0),
            &le16(0),
            &le16(0),
            &(0o100644u32 << 16).to_le_bytes(),
            &le32(local),
            name.as_bytes(),
        ]
        .concat());
    }
    let dir_at = at;
    put(&dir, &mut at)?;
    let n = files.len();
    let end = [&0x0605_4b50u32.to_le_bytes()[..], &le16(0), &le16(0), &le16(n), &le16(n), &le32(dir.len() as u64), &le32(dir_at), &le16(0)].concat();
    put(&end, &mut at)?;
    drop(put);
    out.flush().map_err(|e| format!("can't write the zip: {e}"))?;
    Ok((at, hex(&hash.finalize())))
}

/// An upload's file: bytes, or a file on disk too big to hold (its size and sha256).
#[derive(Clone, Debug)]
pub enum Blob {
    Bytes(Vec<u8>),
    File(PathBuf, u64, String),
}

impl Blob {
    fn len(&self) -> u64 {
        match self {
            Blob::Bytes(b) => b.len() as u64,
            Blob::File(_, n, _) => *n,
        }
    }

    fn sha256(&self) -> String {
        match self {
            Blob::Bytes(b) => hex(&Sha256::digest(b)),
            Blob::File(_, _, sha) => sha.clone(),
        }
    }

    /// `n` bytes from `at`.
    fn read(&self, at: u64, n: usize) -> Result<Vec<u8>, String> {
        match self {
            Blob::Bytes(b) => Ok(b[(at as usize).min(b.len())..(at as usize + n).min(b.len())].to_vec()),
            #[cfg(not(target_arch = "wasm32"))]
            Blob::File(path, size, _) => {
                use std::io::{Read, Seek, SeekFrom};
                let n = n.min(size.saturating_sub(at) as usize);
                let mut f = std::fs::File::open(path).map_err(|e| format!("can't read the zip: {e}"))?;
                let mut b = vec![0; n];
                f.seek(SeekFrom::Start(at)).and_then(|_| f.read_exact(&mut b)).map_err(|e| format!("can't read the zip: {e}"))?;
                Ok(b)
            }
            #[cfg(target_arch = "wasm32")]
            Blob::File(..) => Err("no files to read in the browser".into()),
        }
    }

    fn all(&self) -> Result<Vec<u8>, String> {
        self.read(0, self.len() as usize)
    }
}

/// A mod packed and checked for the store: its `mod.zip` (removed when this goes), `meta.json`'s meta
/// and its `preview.webp`.
pub struct Packed {
    pub meta: Meta,
    pub zip: Blob,
    pub preview: Option<Vec<u8>>,
}

impl Drop for Packed {
    fn drop(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Blob::File(path, ..) = &self.zip {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// `l` zipped into `mods/.upload-<id>.zip` and put through Install's checks, as `author`'s: the zip's
/// directory, its `mod.json` against the meta, every entry inflated against its CRC, and the shell's
/// parsers for its kind (on the folder it was zipped from: the same bytes, the CRCs say).
#[cfg(not(target_arch = "wasm32"))]
pub fn package(mods: &Path, l: &Local, author: &str) -> Result<Packed, String> {
    use std::io::{Read, Seek, SeekFrom};
    let files = files_of(&l.dir)?;
    let to = mods.join(format!(".upload-{}.zip", safe(&l.id)));
    let got = pack(&files, &to);
    let (size, sha256) = got.inspect_err(|_| {
        let _ = std::fs::remove_file(&to);
    })?;
    let meta = Meta { id: l.id.clone(), kind: l.kind, name: l.name.clone(), author: plain_text(author, 40), source: l.source.clone(), version: l.version.clone(), size, sha256, standard: track_mods::STANDARD };
    // the guard first: a refused zip goes too
    let mut p = Packed { meta, zip: Blob::File(to.clone(), size, String::new()), preview: None };
    let mut f = std::fs::File::open(&to).map_err(|e| format!("can't read the zip: {e}"))?;
    let mut read = |at: u64, n: usize| -> Result<Vec<u8>, String> {
        let mut b = vec![0; n];
        f.seek(SeekFrom::Start(at)).and_then(|_| f.read_exact(&mut b)).map_err(|_| "the zip is cut short".to_string())?;
        Ok(b)
    };
    let dir = ZipDir::read(size, &mut read)?;
    // the store's own meta check, as Browse reads it back
    let meta = meta_of(&serde_json::to_vec(&p.meta).unwrap_or_default(), &l.id).ok_or("its meta.json would not read back (name, id or size)")?;
    dir.check_mod_json(&mut read, &meta)?;
    dir.each(&mut read, |_, _| Ok(()))?;
    parse_check(mods, l.kind, &l.id)?;
    p.zip = Blob::File(to, size, meta.sha256.clone());
    p.meta = meta;
    p.preview = preview_of(mods, l);
    Ok(p)
}

/// The browser has no mods of the player's own to upload.
#[cfg(target_arch = "wasm32")]
pub fn package(_: &Path, _: &Local, _: &str) -> Result<Packed, String> {
    Err("uploads run in the desktop game".into())
}

/// A track's or world's picture as `preview.webp`: its own `previews/track.png`, else the shell's
/// still of it, made at most [`super::PREVIEW_W`] wide; None for anything else, or past the cap.
#[cfg(not(target_arch = "wasm32"))]
fn preview_of(mods: &Path, l: &Local) -> Option<Vec<u8>> {
    if !matches!(l.kind, Kind::Track | Kind::World) {
        return None;
    }
    let png = std::fs::read(crate::track_previews::own(mods, &l.id)).or_else(|_| std::fs::read(crate::track_previews::still(mods, &l.id))).ok()?;
    webp_of(&png)
}

/// A picture (PNG or WebP) as a lossless WebP at most [`super::PREVIEW_W`] wide, within the cap.
pub fn webp_of(b: &[u8]) -> Option<Vec<u8>> {
    let p = decode_preview(b)?;
    let mut out = Vec::new();
    image_webp::WebPEncoder::new(&mut out).encode(&p.rgba, p.width as u32, p.height as u32, image_webp::ColorType::Rgba8).ok()?;
    (out.len() as u64 <= MAX_PREVIEW).then_some(out)
}

// ---- planning a re-upload ------------------------------------------------------------------------

/// `refs/pr/<n>`'s `n`.
pub fn pr_of(rev: &str) -> Option<u32> {
    rev.strip_prefix("refs/pr/")?.parse().ok()
}

/// What uploading a mod does, given the uploader's own copy of its id on the store.
#[derive(Debug, PartialEq)]
pub enum Plan {
    /// Never uploaded, or merged: onto their newest open PR, or a new one.
    New,
    /// Over their open PR's copy.
    Onto(u32),
    Skip(String),
}

/// A version's numbers, "1.10" past "1.9"; none is 1.
fn version(s: &str) -> Vec<u64> {
    let mut n: Vec<u64> = s.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).map(|p| p.parse().unwrap_or(u64::MAX)).collect();
    if n.is_empty() {
        n.push(1);
    }
    while n.len() > 1 && n.last() == Some(&0) {
        n.pop();
    }
    n
}

/// The same zip is skipped, and so is an older version than the store's; else over the open PR's copy,
/// or a new upload.
pub fn plan(old: Option<&Entry>, new: &Meta) -> Plan {
    match old {
        None => Plan::New,
        Some(o) if o.meta.sha256 == new.sha256 => Plan::Skip("this copy is on the store already".into()),
        Some(o) if version(&o.meta.version) > version(&new.version) => Plan::Skip(format!("the store has a newer version ({})", o.meta.version)),
        Some(o) => pr_of(&o.rev).map_or(Plan::New, Plan::Onto),
    }
}

/// The uploader's newest open PR, where their new mods go.
pub fn newest_pr(own: &[Entry]) -> Option<u32> {
    own.iter().filter_map(|e| pr_of(&e.rev)).max()
}

/// Where a commit goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum To {
    /// A new pull request off `main`.
    NewPr,
    Pr(u32),
    /// `main` itself (the maintainer's card and index).
    Main,
}

/// The PR number in a commit's answer (`pullRequestUrl`, `…/discussions/<n>`).
pub fn pr_in(answer: &Value) -> Option<u32> {
    answer["pullRequestUrl"].as_str()?.trim_end_matches('/').rsplit('/').next()?.parse().ok()
}

/// Standard base64, for the commit API's inline files.
pub fn base64(b: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            out.push(if i <= c.len() { A[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

/// A commit's message, as the account's own.
fn says(e: &Value) -> String {
    e["error"].as_str().map(str::to_string).unwrap_or_default()
}

// ---- talking to Hugging Face -----------------------------------------------------------------

impl Store {
    /// The signed-in account's name.
    pub fn signed_in(&self) -> Option<String> {
        account(&self.dir).map(|a| a.0)
    }

    /// Checks `token` (pasted: an `hf_…` word in it) with Hugging Face and keeps it ([`Event::Signed`]).
    /// A read-only token is refused: it cannot open a pull request's commit.
    pub fn sign_in(&self, pasted: &str) {
        let Some(token) = token_in(pasted) else {
            return self.send(Event::Signed(Err("No Hugging Face token (hf_…) on the clipboard: copy one, then Paste token".into())));
        };
        let s = self.clone();
        self.spawn(async move {
            let got = async {
                let r = s.call(s.client.get(format!("{}/api/whoami-v2", s.base)).bearer_auth(&token)).await.map_err(|e| format!("Hugging Face didn't take that token ({e})"))?;
                let name = plain_text(r["name"].as_str().ok_or("Hugging Face sent no account name")?, 40);
                if name.is_empty() {
                    return Err("Hugging Face sent no account name".to_string());
                }
                if r["auth"]["accessToken"]["role"] == "read" {
                    return Err("That token can only read: make one with write access".into());
                }
                keep(&s.dir, &format!("{name}\n{token}\n"))?;
                Ok(name)
            };
            s.send(Event::Signed(got.await));
        });
    }

    /// Forgets the token.
    pub fn sign_out(&self) {
        let _ = keep(&self.dir, "");
    }

    /// The player's own mods, for Upload ([`locals`]).
    pub fn locals(&self) -> Vec<Local> {
        locals(&self.mods)
    }

    /// A request's JSON answer, or what went wrong (the status and what the Hub said).
    async fn call(&self, r: reqwest::RequestBuilder) -> Result<Value, String> {
        let r = r.send().await.map_err(|e| format!("can't reach Hugging Face: {e}"))?;
        let status = r.status();
        let body = r.bytes().await.map_err(|e| e.to_string())?;
        if status.as_u16() == 429 {
            return Err(super::BUSY.into());
        }
        if !status.is_success() {
            let said = serde_json::from_slice::<Value>(&body).map(|v| says(&v)).unwrap_or_default();
            let said = if said.is_empty() { String::from_utf8_lossy(&body[..body.len().min(200)]).into_owned() } else { said };
            return Err(format!("{}: {}", status.as_u16(), plain_text(&said, 200)));
        }
        Ok(serde_json::from_slice(&body).unwrap_or_default())
    }

    /// The Hub's commit API (`huggingface_hub`'s `create_commit`): asks which files go through LFS,
    /// uploads those, then commits all of them and the deletion of the folders `gone`, to `to`. Its
    /// answer (`pullRequestUrl` for a new PR).
    pub async fn commit(&self, token: &str, summary: &str, files: &[(String, Blob)], gone: &[String], to: To, tell: &(dyn Fn(String) + Sync)) -> Result<Value, String> {
        let api = format!("{}/api/datasets/{REPO}", self.base);
        let rev = match to {
            To::NewPr => "main?create_pr=1".to_string(),
            To::Pr(n) => rev_part(&format!("refs/pr/{n}")),
            To::Main => "main".to_string(),
        };
        let mut lfs = Vec::new();
        if !files.is_empty() {
            let mut ask = Vec::new();
            for (path, b) in files {
                ask.push(json!({ "path": path, "size": b.len(), "sample": base64(&b.read(0, 512)?) }));
            }
            let modes = self.call(self.client.post(format!("{api}/preupload/{rev}")).bearer_auth(token).header("Content-Type", "application/json").body(json!({ "files": ask }).to_string())).await?;
            lfs = modes["files"].as_array().into_iter().flatten().filter(|f| f["uploadMode"] == "lfs").filter_map(|f| f["path"].as_str().map(str::to_string)).collect();
        }
        let mut lines = vec![json!({ "key": "header", "value": { "summary": summary, "description": "Uploaded from openMX." } })];
        for (path, b) in files {
            lines.push(if lfs.contains(path) {
                let oid = b.sha256();
                self.lfs_put(token, &oid, b, path, tell).await?;
                json!({ "key": "lfsFile", "value": { "path": path, "algo": "sha256", "oid": oid, "size": b.len() } })
            } else {
                json!({ "key": "file", "value": { "path": path, "content": base64(&b.all()?), "encoding": "base64" } })
            });
        }
        lines.extend(gone.iter().map(|path| json!({ "key": "deletedFolder", "value": { "path": path } })));
        let body: String = lines.iter().map(|l| l.to_string() + "\n").collect();
        self.call(self.client.post(format!("{api}/commit/{rev}")).bearer_auth(token).header("Content-Type", "application/x-ndjson").body(body)).await
    }

    /// One file into the store's LFS (the git-lfs batch API: one PUT, or parts read from the file a part
    /// at a time), then verified.
    async fn lfs_put(&self, token: &str, oid: &str, b: &Blob, path: &str, tell: &(dyn Fn(String) + Sync)) -> Result<(), String> {
        let lfs = "application/vnd.git-lfs+json";
        // no `ref`: it makes the Hub check write access to that branch, and a PR needs only read
        let ask = json!({ "operation": "upload", "transfers": ["basic", "multipart"], "hash_algo": "sha256", "objects": [{ "oid": oid, "size": b.len() }] });
        let r = self.call(self.client.post(format!("{}/datasets/{REPO}.git/info/lfs/objects/batch", self.base)).bearer_auth(token).header("Accept", lfs).header("Content-Type", lfs).body(ask.to_string())).await?;
        let o = &r["objects"][0];
        if let Some(e) = o["error"]["message"].as_str() {
            return Err(format!("LFS: {}", plain_text(e, 200)));
        }
        // no upload action: the store has these bytes already
        let Some(up) = o["actions"].get("upload") else { return Ok(()) };
        let href = up["href"].as_str().ok_or("LFS: no upload URL")?;
        let header = up["header"].as_object().cloned().unwrap_or_default();
        let file = path.rsplit('/').next().unwrap_or(path);
        let mb = b.len().div_ceil(1 << 20);
        match header.get("chunk_size").and_then(|v| v.as_str().and_then(|s| s.parse::<u64>().ok()).or(v.as_u64())) {
            Some(size) if size > 0 => {
                let mut parts: Vec<(u32, String)> = header.iter().filter_map(|(k, v)| Some((k.parse().ok()?, v.as_str()?.to_string()))).collect();
                parts.sort();
                let mut done = Vec::new();
                for (i, (n, url)) in parts.iter().enumerate() {
                    tell(format!("Uploading {file}: {} / {mb} MB", (i as u64 * size) >> 20));
                    let chunk = b.read(i as u64 * size, size as usize)?;
                    let r = self.client.put(url).body(chunk).send().await.map_err(|e| format!("upload cut short: {e}"))?;
                    let r = r.error_for_status().map_err(|e| format!("upload refused: {e}"))?;
                    let etag = r.headers().get("etag").and_then(|v| v.to_str().ok()).ok_or("upload: no ETag back")?.to_string();
                    done.push(json!({ "partNumber": n, "etag": etag }));
                }
                self.call(self.client.post(href).header("Accept", lfs).header("Content-Type", lfs).body(json!({ "oid": oid, "parts": done }).to_string())).await?;
            }
            _ => {
                tell(format!("Uploading {file} ({mb} MB)"));
                let mut put = self.client.put(href).body(b.all()?);
                for (k, v) in &header {
                    if let Some(v) = v.as_str() {
                        put = put.header(k.as_str(), v);
                    }
                }
                put.send().await.map_err(|e| format!("upload cut short: {e}"))?.error_for_status().map_err(|e| format!("upload refused: {e}"))?;
            }
        }
        if let Some(v) = o["actions"].get("verify") {
            let href = v["href"].as_str().ok_or("LFS: no verify URL")?;
            self.call(self.client.post(href).bearer_auth(token).header("Accept", lfs).header("Content-Type", lfs).body(json!({ "oid": oid, "size": b.len() }).to_string())).await?;
        }
        Ok(())
    }

    /// `name`'s mods on the store: those on their open PRs (a PR's entries are its author's), newest PR
    /// first, then those on `main` that name them. Never the kept listing.
    async fn uploads_of(&self, name: &str) -> Result<Vec<Entry>, String> {
        let mut l = self.lists().await?;
        let main = if l.is_empty() { Vec::new() } else { l.remove(0) };
        Ok(l.into_iter().flatten().chain(main).filter(|e| e.meta.author.eq_ignore_ascii_case(name)).collect())
    }

    /// My public mods ([`Event::Mine`]).
    pub fn mine(&self) {
        let Some((name, _)) = account(&self.dir) else {
            return self.send(Event::Mine(Err("Sign in to see your mods on the store".into())));
        };
        let s = self.clone();
        self.spawn(async move { s.send(Event::Mine(s.uploads_of(&name).await)) });
    }

    /// The kept listing goes stale, `gone` (a key and rev taken down) out of it at once.
    fn stale_without(&self, gone: Option<(&str, &str)>) {
        let Some(mut c) = crate::files::read(self.dir.join(CACHE)).ok().and_then(|b| serde_json::from_slice::<Cached>(&b).ok()) else { return };
        c.at = 0;
        if let Some((key, rev)) = gone {
            c.entries.retain(|e| !(e.key() == key && e.rev == rev));
        }
        let _ = crate::files::write(self.dir.join(CACHE), serde_json::to_vec(&c).unwrap_or_default());
    }

    /// Takes the player's own `e` off the store ([`Event::TakenDown`]): an open PR holding only it is
    /// closed, one holding more of `mine` gets a commit deleting `mods/<id>/`, and one on `main` gets a
    /// PR deleting it, gone when a maintainer merges that.
    pub fn take_down(&self, e: &Entry, mine: &[Entry]) {
        let Some((name, token)) = account(&self.dir) else {
            return self.send(Event::TakenDown(e.key(), Err("Sign in to take a mod down".into())));
        };
        if !e.meta.author.eq_ignore_ascii_case(&name) {
            return self.send(Event::TakenDown(e.key(), Err(format!("{} is {}'s, not yours", e.meta.name, e.meta.author))));
        }
        let shared = mine.iter().any(|o| o.rev == e.rev && o.meta.id != e.meta.id);
        let (e, s) = (e.clone(), self.clone());
        self.spawn(async move {
            let (id, title) = (e.meta.id.clone(), e.meta.name.clone());
            let got = async {
                let none = |_: String| {};
                let said = match pr_of(&e.rev) {
                    Some(n) if shared => {
                        s.commit(&token, &format!("Remove mod {title} ({id})"), &[], &[format!("mods/{id}")], To::Pr(n), &none).await?;
                        format!("{title} is off the store: taken out of your pull request #{n}.")
                    }
                    Some(n) => {
                        let body = json!({ "status": "closed", "comment": format!("Taken down by its uploader from openMX ({id}).") });
                        s.call(s.client.post(format!("{}/api/datasets/{REPO}/discussions/{n}/status", s.base)).bearer_auth(&token).header("Content-Type", "application/json").body(body.to_string())).await?;
                        format!("{title} is off the store: its pull request #{n} is closed.")
                    }
                    None => {
                        let r = s.commit(&token, &format!("Remove mod {title} ({id})"), &[], &[format!("mods/{id}")], To::NewPr, &none).await?;
                        format!("Asked to take {title} down: it goes when a maintainer merges pull request #{}.", pr_in(&r).map_or("?".into(), |n| n.to_string()))
                    }
                };
                s.stale_without(Some((&e.key(), &e.rev)));
                Ok(said)
            };
            let got = got.await;
            s.send(Event::TakenDown(e.key(), got));
        });
    }

    /// Uploads `mods` in the background, one commit a mod in turn ([`Event::Upload`] for each: Waiting,
    /// Working, then Done, Skipped or Failed). Each is packed and checked first ([`package`]); its
    /// re-upload goes over the copy on the player's open PR, and a new one onto their newest open PR (a
    /// new PR when they have none).
    pub fn upload(&self, mods: Vec<Local>) {
        let Some((name, token)) = account(&self.dir) else {
            for l in mods {
                self.send(Event::Upload(l.id, Up::Failed("Sign in first".into())));
            }
            return;
        };
        for l in &mods {
            self.send(Event::Upload(l.id.clone(), Up::Waiting));
        }
        let s = self.clone();
        self.spawn(async move {
            let mut own: Option<Vec<Entry>> = None;
            for l in mods {
                let id = l.id.clone();
                let up = match s.upload_one(&l, &name, &token, &mut own).await {
                    Ok(up) => up,
                    Err(e) => Up::Failed(format!("{} not uploaded: {e}", l.name)),
                };
                // the next Browse reads the store again, this one included
                if matches!(up, Up::Done(_)) {
                    s.stale_without(None);
                }
                s.send(Event::Upload(id, up));
            }
        });
    }

    /// `l` packed, planned and committed as `name`'s; `own` is their mods on the store, read once a
    /// batch and kept up with its commits.
    async fn upload_one(&self, l: &Local, name: &str, token: &str, own: &mut Option<Vec<Entry>>) -> Result<Up, String> {
        let tell = |line: String| self.send(Event::Upload(l.id.clone(), Up::Working(line)));
        tell(format!("Checking {}", l.name));
        let p = package(&self.mods, l, name)?;
        if own.is_none() {
            tell("Reading your mods on the store".into());
            *own = Some(self.uploads_of(name).await?);
        }
        let mine = own.as_mut().expect("read above");
        let old = mine.iter().find(|e| e.meta.id == l.id);
        let (to, verb) = match plan(old, &p.meta) {
            Plan::Skip(why) => return Ok(Up::Skipped(format!("{} not uploaded: {why}", l.name))),
            Plan::Onto(n) => (To::Pr(n), "Update"),
            Plan::New => (newest_pr(mine).map_or(To::NewPr, To::Pr), if old.is_some() { "Update" } else { "Add" }),
        };
        let dir = format!("mods/{}", l.id);
        let mut files = vec![(format!("{dir}/mod.zip"), p.zip.clone()), (format!("{dir}/meta.json"), Blob::Bytes(serde_json::to_vec_pretty(&p.meta).unwrap_or_default()))];
        files.extend(p.preview.clone().map(|w| (format!("{dir}/preview.webp"), Blob::Bytes(w))));
        let r = self.commit(token, &format!("{verb} mod {} ({})", l.name, l.id), &files, &[], to, &tell).await?;
        let n = match to {
            To::Pr(n) => n,
            _ => pr_in(&r).ok_or("Hugging Face sent no pull request back")?,
        };
        let rev = format!("refs/pr/{n}");
        mine.retain(|e| !(e.meta.id == l.id && e.rev == rev));
        mine.insert(0, Entry { meta: p.meta.clone(), rev, verified: false });
        let mb = p.meta.size.div_ceil(1 << 20);
        // Browse keeps its listing an hour: other devices see it once they read the store again
        Ok(Up::Done(match (to, verb) {
            (To::NewPr, _) => format!("{} is up ({mb} MB) as your pull request #{n}: others see it after Start on Browse", l.name),
            (_, "Update") => format!("{} updated ({mb} MB) on your pull request #{n}: others see it after Start on Browse", l.name),
            _ => format!("{} is up ({mb} MB) on your pull request #{n}: others see it after Start on Browse", l.name),
        }))
    }

    /// The store's index (M116, [`super::Index`]) as it is now: every rev read, none from the old index.
    pub async fn index_now(&self) -> Result<Value, String> {
        let (open, heads) = (self.pulls().await?, self.heads().await?);
        let mut lists = Vec::new();
        for p in parts(None, open.clone(), &heads) {
            if let Part::Read(rev, who) = p {
                lists.push(self.entries(rev, who).await?);
            }
        }
        let pulls: serde_json::Map<String, Value> = open.iter().filter_map(|(n, _)| Some((n.to_string(), json!({ "commit": heads.get(n)? })))).collect();
        let entries: Vec<Entry> = lists.into_iter().flatten().collect();
        Ok(json!({ "version": 1, "pulls": pulls, "entries": entries }))
    }
}

// ---- the maintainer's side: openmx --mod-store-admin ---------------------------------------------

/// `openmx --mod-store-admin card <README.md> | index | upload <mods dir> [id…]`, signed in with the
/// token file `--token <file>` (default `~/.config/openmx/hf-token.txt`, `name` then `token`): the
/// dataset card or a fresh `index.json` committed to `main`, or mods uploaded as the account's.
#[cfg(not(target_arch = "wasm32"))]
pub fn admin(args: &[String]) -> Result<(), String> {
    let mut args = args.to_vec();
    let mut token_file = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default().join(".config/openmx").join(TOKEN);
    let mut base = HF.to_string();
    while let Some(at) = args.iter().position(|a| a == "--token" || a == "--store") {
        let v = args.get(at + 1).cloned().ok_or(format!("{} needs a value", args[at]))?;
        if args[at] == "--token" {
            token_file = v.into();
        } else {
            base = v;
        }
        args.drain(at..at + 2);
    }
    let usage = "usage: openmx --mod-store-admin [--token FILE] [--store URL] card README.md | index | upload MODS [ID...]";
    let text = std::fs::read_to_string(&token_file).map_err(|e| format!("{}: {e}", token_file.display()))?;
    let (name, token) = account_in(&text).ok_or(format!("{}: not a name line and an hf_ token", token_file.display()))?;
    let scratch = std::env::temp_dir().join(format!("openmx-store-admin-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string())?;
    let say = |line: String| println!("{line}");
    let got = match args.first().map(String::as_str) {
        Some("card") => {
            let path = args.get(1).ok_or(usage)?;
            let card = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
            let s = Store::new(&base, &scratch, &scratch);
            rt.block_on(s.commit(&token, "The store's dataset card: what it is, its layout, its rules", &[("README.md".into(), Blob::Bytes(card))], &[], To::Main, &say)).map(|r| println!("committed: {}", r["commitUrl"].as_str().unwrap_or("")))
        }
        Some("index") => {
            let s = Store::new(&base, &scratch, &scratch);
            rt.block_on(async {
                let ix = s.index_now().await?;
                let n = ix["entries"].as_array().map_or(0, Vec::len);
                let r = s.commit(&token, &format!("index.json: {n} mods"), &[("index.json".into(), Blob::Bytes(serde_json::to_vec(&ix).unwrap_or_default()))], &[], To::Main, &say).await?;
                println!("index.json: {n} mods, committed: {}", r["commitUrl"].as_str().unwrap_or(""));
                Ok(())
            })
        }
        Some("upload") => {
            let mods = PathBuf::from(args.get(1).ok_or(usage)?);
            let ids = &args[2..];
            let s = Store::new(&base, &scratch, &mods);
            let all: Vec<Local> = locals(&mods).into_iter().filter(|l| ids.is_empty() || ids.contains(&l.id)).collect();
            if all.is_empty() {
                return Err(format!("no mods of yours in {}", mods.display()));
            }
            rt.block_on(async {
                let mut own = None;
                let mut failed = 0;
                for l in &all {
                    let up = s.upload_one(l, &name, &token, &mut own).await.unwrap_or_else(|e| Up::Failed(format!("{} not uploaded: {e}", l.name)));
                    failed += matches!(up, Up::Failed(_)) as usize;
                    println!("{}: {}", l.id, up.line());
                }
                if failed > 0 { Err(format!("{failed} of {} failed", all.len())) } else { Ok(()) }
            })
        }
        _ => Err(usage.to_string()),
    };
    let _ = std::fs::remove_dir_all(&scratch);
    got
}

#[cfg(test)]
#[path = "modstore_up_tests.rs"]
mod tests;
