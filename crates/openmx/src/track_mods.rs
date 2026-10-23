//! Track mods (MD2): a track loaded from a folder rather than off the disc.
//!
//! The layout is openmx-mods' standard (`standard/OPENMX-TRACK-STANDARD.md`, its T1): a `mods/`
//! folder beside `settings.txt`, and in it `tracks/<id>/`, each with a `mod.json` and the files of the
//! source's track `.res` as loose files at their member paths — the scene, the terrain it names, the
//! `.col` shapes it places, textures, sky. The folder is read through [`Res::folder`], so every loader
//! that reads a disc track's archive reads a mod's folder the same way.
//!
//! What this module does is find them and refuse the broken ones: [`scan`] lists every track mod
//! that reads (its `mod.json` has every required field right, its folder has exactly one `.scn`, the
//! scene parses, and the terrain it names is there and parses), and says in one log line why each
//! other one is left out. Nothing in here can panic on a mod's bytes.
//!
//! A mod is not the disc's track: it has no second archive, so its bike, physics and follow camera
//! are a disc freeride's ([`FREERIDE`]), and it rides as a freeride. With no `mods/` folder, or with
//! the setting off, nothing here is called, so the pick and every ride are the disc's alone.

use game_render::disc::Res;
use game_sim::track;
use std::path::{Path, PathBuf};

/// The folder beside `settings.txt` the mods are in.
pub const MODS_DIR: &str = "mods";
/// The version of the standard this loader reads; a mod.json asking for a later one is left out.
pub const STANDARD: u64 = 1;
/// The disc freeride whose second archive (the bike's animation set, its physics and the follow
/// camera) a mod rides with, since a mod has none of its own: the first freeride, the shell's
/// default track.
pub const FREERIDE: &str = "free06";

/// One track mod that reads, as the pick lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct ModTrack {
    /// The folder's name and `mod.json`'s `id`, `mxvatv_mc01`: what the pick and `--track` say.
    pub id: String,
    /// What the pick shows.
    pub name: String,
    /// The mod's folder.
    pub dir: PathBuf,
    /// The scene's file stem (`MC01`), which the shell builds the track's other names from
    /// (`<stem>_shadow_01.pst`, `<stem>.toy`) as it does from a disc track's name.
    pub stem: String,
    /// `mod.json`'s `spawn`, as the text of its numbers: where a freeride starts when it is not the
    /// scene's own `DefaultStartPosition`.
    pub spawn: Option<([String; 3], [String; 3])>,
}

impl ModTrack {
    /// The folder, as an archive every loader reads.
    pub fn open(&self) -> Result<Res, String> {
        Res::folder(&self.dir)
    }

    /// Put `mod.json`'s spawn into the scene the simulation reads, converted as the scene converts
    /// its own `DefaultStartPosition` (the scene's reader parses it, so the conversion is its).
    pub fn place_spawn(&self, scene: &mut track::Scene) -> Result<(), String> {
        let Some((at, dir)) = &self.spawn else { return Ok(()) };
        let text = format!(
            "[SceneInfo]\r\nDefaultStartPosition={}\r\nDefaultStartDirection={}\r\n[Environment]\r\nTerrainFile=-\r\n",
            at.join(","),
            dir.join(",")
        );
        let read = track::Scene::parse(text.as_bytes()).map_err(|e| format!("mod.json spawn: {e}"))?;
        scene.start = read.start;
        scene.start_direction = read.start_direction;
        Ok(())
    }
}

/// What [`scan`] found: the mods that read, in folder-name order, and a line for each one left out.
#[derive(Debug, Default)]
pub struct Scan {
    pub tracks: Vec<ModTrack>,
    pub log: Vec<String>,
}

/// Every track mod under `mods` (the `mods/` folder itself). No folder is no mods and no lines.
pub fn scan(mods: &Path) -> Scan {
    let mut out = Scan::default();
    let dir = mods.join("tracks");
    let Ok(list) = std::fs::read_dir(&dir) else { return out };
    let mut folders: Vec<PathBuf> = list.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    folders.sort();
    for folder in folders {
        let label = folder.file_name().map_or_else(|| folder.display().to_string(), |n| n.to_string_lossy().into_owned());
        match check(&folder) {
            Ok(Some(t)) => out.tracks.push(t),
            Ok(None) => {}
            Err(why) => out.log.push(format!("mods: tracks/{label} left out: {why}")),
        }
    }
    out
}

/// One track mod by its id, checked as [`scan`] checks it.
pub fn find(mods: &Path, id: &str) -> Result<ModTrack, String> {
    let folder = mods.join("tracks").join(id);
    if !valid_id(id) || !folder.is_dir() {
        return Err(format!("no track mod {id} in {}", mods.join("tracks").display()));
    }
    check(&folder)?.ok_or_else(|| format!("{}: not a track mod", folder.display()))
}

/// Each listed mod's source game (MD11), `(id, game)`: its `mod.json`'s `source.game`, for the
/// pick's filters. A mod whose file does not say is left out, and the pick files it by its pack.
pub fn source_games(mods: &Path, ids: &[String]) -> Vec<(String, String)> {
    ids.iter()
        .filter(|id| valid_id(id))
        .filter_map(|id| {
            let text = std::fs::read_to_string(mods.join("tracks").join(id).join("mod.json")).ok()?;
            let json = Json::parse(&text).ok()?;
            let game = json.get("source")?.get("game")?.str()?.trim();
            (!game.is_empty()).then(|| (id.clone(), game.to_string()))
        })
        .collect()
}

/// Whether `id` is spelled as a mod's id may be: `[a-z0-9_]`, 1 to 40 of them, `<prefix>_<map>`.
/// No disc track's name has the underscore, so a mod can never take one's place.
pub fn valid_id(id: &str) -> bool {
    (1..=40).contains(&id.len())
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        && id.trim_matches('_').contains('_')
}

/// `Ok(None)` for a mod that is not a track (a world or a music pack, or a kind this build does not
/// know), which the standard says to skip quietly; `Err` for a broken one.
fn check(folder: &Path) -> Result<Option<ModTrack>, String> {
    let name = folder.file_name().and_then(|n| n.to_str()).ok_or("the folder's name is not UTF-8")?;
    let path = folder.join("mod.json");
    let text = std::fs::read(&path).map_err(|e| format!("mod.json: {e}"))?;
    let text = String::from_utf8(text).map_err(|_| "mod.json is not UTF-8".to_string())?;
    let json = Json::parse(&text).map_err(|e| format!("mod.json: {e}"))?;
    let field = |key: &str| json.get(key).ok_or_else(|| format!("mod.json has no {key}"));
    let string = |key: &str| field(key)?.str().ok_or_else(|| format!("mod.json: {key} is not a string"));

    let standard = field("standard")?.num().and_then(|n| n.parse::<u64>().ok()).ok_or("mod.json: standard is not a whole number")?;
    if standard > STANDARD {
        return Err(format!("standard {standard}, and this build reads {STANDARD}"));
    }
    if string("kind")? != "track" {
        return Ok(None);
    }
    let id = string("id")?;
    if id != name {
        return Err(format!("mod.json's id {id:?} is not the folder's name"));
    }
    if !valid_id(id) {
        return Err(format!("{id:?} is not an id: [a-z0-9_], 1 to 40 of them, <prefix>_<map>"));
    }
    let title = string("name")?.trim();
    if title.is_empty() {
        return Err("mod.json's name is empty".into());
    }
    field("source")?.get("game").and_then(Json::str).ok_or("mod.json: source has no game")?;
    let version = string("version")?;
    if version.split('.').count() != 3 || !version.split('.').all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())) {
        return Err(format!("mod.json: version {version:?} is not major.minor.patch"));
    }
    string("author")?;
    string("permission")?;
    match field("todo")? {
        Json::Arr(items) if items.iter().all(|i| i.str().is_some()) => {}
        _ => return Err("mod.json: todo is not a list of strings".into()),
    }
    let spawn = match json.get("spawn") {
        None => None,
        Some(s) => {
            let three = |key: &str| -> Result<[String; 3], String> {
                match s.get(key) {
                    Some(Json::Arr(v)) if v.len() == 3 => {
                        let n: Vec<String> = v.iter().filter_map(|n| n.num().map(str::to_string)).collect();
                        n.try_into().map_err(|_| format!("mod.json: spawn {key} is not three numbers"))
                    }
                    _ => Err(format!("mod.json: spawn {key} is not three numbers")),
                }
            };
            s.get("from").and_then(Json::str).filter(|f| !f.is_empty()).ok_or("mod.json: spawn has no from")?;
            Some((three("position")?, three("direction")?))
        }
    };

    // The files: one scene, and the terrain it names.
    let res = Res::folder(folder)?;
    let scenes: Vec<&str> = res
        .entries()
        .iter()
        .map(|e| e.name.as_str())
        .filter(|n| n.to_ascii_lowercase().ends_with(".scn"))
        .collect();
    let scn = match scenes.as_slice() {
        [one] => one.to_string(),
        [] => return Err("no .scn".into()),
        more => return Err(format!("{} .scn files, and a track has one", more.len())),
    };
    let stem = scn.rsplit('\\').next().unwrap_or(&scn);
    let stem = stem[..stem.len() - 4].to_string();
    let scene_bytes = res.read(&scn)?;
    let mut scene = track::Scene::parse(&scene_bytes).map_err(|e| format!("{scn}: {e}"))?;
    let terrain = terrain_bytes(&res, &scene.terrain_file)?;
    track::Terrain::parse(&terrain).map_err(|e| format!("{}: {e}", scene.terrain_file))?;
    let t = ModTrack { id: id.to_string(), name: title.to_string(), dir: folder.to_path_buf(), stem, spawn };
    t.place_spawn(&mut scene)?;
    Ok(Some(t))
}

/// The terrain a scene names, looked up as the shell looks up a disc track's: the name as written,
/// then with the platform tag (`MC01.trn` → `MC01.ps2.trn`).
pub fn terrain_bytes(res: &Res, named: &str) -> Result<Vec<u8>, String> {
    if let Ok(bytes) = res.read_suffix(named) {
        return Ok(bytes);
    }
    let tagged = match named.rsplit_once('.') {
        Some((stem, ext)) => format!("{stem}.ps2.{ext}"),
        None => return Err(format!("no file ending in {named}")),
    };
    res.read_suffix(&tagged).map_err(|_| format!("no file ending in {named} or {tagged}"))
}

// ---- mod.json ------------------------------------------------------------------------------------

/// Just enough JSON for `mod.json`: numbers are kept as the text they were written as, so a
/// spawn's coordinates reach the scene's own float conversion untouched.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(String),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn parse(text: &str) -> Result<Json, String> {
        let mut p = Parser { s: text.as_bytes(), at: 0, depth: 0 };
        let v = p.value()?;
        p.space();
        if p.at != p.s.len() {
            return Err(format!("text after the value at byte {}", p.at));
        }
        Ok(v)
    }

    /// An object's field; the last one where a key is written twice.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(fields) => fields.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn num(&self) -> Option<&str> {
        match self {
            Json::Num(s) => Some(s),
            _ => None,
        }
    }
}

struct Parser<'a> {
    s: &'a [u8],
    at: usize,
    depth: usize,
}

impl Parser<'_> {
    fn space(&mut self) {
        while self.at < self.s.len() && matches!(self.s[self.at], b' ' | b'\t' | b'\r' | b'\n') {
            self.at += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.at).copied()
    }

    fn eat(&mut self, b: u8) -> Result<(), String> {
        self.space();
        if self.peek() == Some(b) {
            self.at += 1;
            Ok(())
        } else {
            Err(format!("expected {:?} at byte {}", b as char, self.at))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.space();
        self.depth += 1;
        if self.depth > 64 {
            return Err("nested too deep".into());
        }
        let v = match self.peek().ok_or("the text ends where a value should be")? {
            b'{' => {
                self.at += 1;
                let mut fields = Vec::new();
                self.space();
                if self.peek() == Some(b'}') {
                    self.at += 1;
                } else {
                    loop {
                        self.space();
                        let key = self.string()?;
                        self.eat(b':')?;
                        fields.push((key, self.value()?));
                        self.space();
                        match self.peek() {
                            Some(b',') => self.at += 1,
                            Some(b'}') => {
                                self.at += 1;
                                break;
                            }
                            _ => return Err(format!("expected , or }} at byte {}", self.at)),
                        }
                    }
                }
                Json::Obj(fields)
            }
            b'[' => {
                self.at += 1;
                let mut items = Vec::new();
                self.space();
                if self.peek() == Some(b']') {
                    self.at += 1;
                } else {
                    loop {
                        items.push(self.value()?);
                        self.space();
                        match self.peek() {
                            Some(b',') => self.at += 1,
                            Some(b']') => {
                                self.at += 1;
                                break;
                            }
                            _ => return Err(format!("expected , or ] at byte {}", self.at)),
                        }
                    }
                }
                Json::Arr(items)
            }
            b'"' => Json::Str(self.string()?),
            b't' => self.word("true", Json::Bool(true))?,
            b'f' => self.word("false", Json::Bool(false))?,
            b'n' => self.word("null", Json::Null)?,
            b'-' | b'0'..=b'9' => {
                let from = self.at;
                while self.at < self.s.len() && matches!(self.s[self.at], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                    self.at += 1;
                }
                let text = std::str::from_utf8(&self.s[from..self.at]).map_err(|e| e.to_string())?;
                text.parse::<f64>().map_err(|_| format!("{text:?} is not a number"))?;
                Json::Num(text.to_string())
            }
            b => return Err(format!("unexpected {:?} at byte {}", b as char, self.at)),
        };
        self.depth -= 1;
        Ok(v)
    }

    fn word(&mut self, w: &str, v: Json) -> Result<Json, String> {
        if self.s[self.at..].starts_with(w.as_bytes()) {
            self.at += w.len();
            Ok(v)
        } else {
            Err(format!("unexpected text at byte {}", self.at))
        }
    }

    fn string(&mut self) -> Result<String, String> {
        if self.peek() != Some(b'"') {
            return Err(format!("expected a string at byte {}", self.at));
        }
        self.at += 1;
        let mut out = Vec::new();
        loop {
            let b = self.peek().ok_or("a string runs to the end of the text")?;
            self.at += 1;
            match b {
                b'"' => break,
                b'\\' => {
                    let e = self.peek().ok_or("a string runs to the end of the text")?;
                    self.at += 1;
                    let c = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let hi = self.hex4()?;
                            let code = if (0xd800..0xdc00).contains(&hi) && self.s[self.at..].starts_with(b"\\u") {
                                self.at += 2;
                                let lo = self.hex4()?;
                                0x10000 + ((hi - 0xd800) << 10) + (lo.wrapping_sub(0xdc00) & 0x3ff)
                            } else {
                                hi
                            };
                            char::from_u32(code).unwrap_or('\u{fffd}')
                        }
                        _ => return Err(format!("a bad escape at byte {}", self.at)),
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                }
                b if b < 0x20 => return Err(format!("a control character in a string at byte {}", self.at)),
                b => out.push(b),
            }
        }
        String::from_utf8(out).map_err(|_| "a string is not UTF-8".to_string())
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self.s.get(self.at..self.at + 4).ok_or("a \\u escape runs to the end of the text")?;
        let text = std::str::from_utf8(digits).map_err(|e| e.to_string())?;
        let v = u32::from_str_radix(text, 16).map_err(|_| format!("a bad \\u escape at byte {}", self.at))?;
        self.at += 4;
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_json_reads_and_keeps_numbers_as_written() {
        let j = Json::parse(r#"{"a": [1, -2.50, 3e2], "b": "x\\yé", "c": {"d": null, "e": true}}"#).unwrap();
        assert_eq!(j.get("a"), Some(&Json::Arr(vec![Json::Num("1".into()), Json::Num("-2.50".into()), Json::Num("3e2".into())])));
        assert_eq!(j.get("b").and_then(Json::str), Some("x\\yé"));
        assert_eq!(j.get("c").and_then(|c| c.get("e")), Some(&Json::Bool(true)));
        for bad in ["", "{", "{\"a\" 1}", "[1,]", "\"open", "{} x", "[[[[", "01x", "nul"] {
            assert!(Json::parse(bad).is_err(), "{bad:?}");
        }
        assert!(Json::parse(&"[".repeat(1000)).is_err());
    }

    #[test]
    fn ids_are_lowercase_with_a_prefix() {
        for good in ["mxvatv_mc01", "elev_n33w115", "a_b"] {
            assert!(valid_id(good), "{good}");
        }
        for bad in ["free06", "", "_x", "x_", "MXVATV_MC01", "a-b_c", &"a_".repeat(21)] {
            assert!(!valid_id(bad), "{bad}");
        }
    }
}
