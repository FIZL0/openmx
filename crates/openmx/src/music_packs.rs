//! **Music packs** (MD7): other games' songs from `mods/music/<id>/`, played beside the disc's.
//! openmx-mods' `tools/import-music` writes one off the user's own disc by its standard's §4: a
//! `mod.json` of kind `music` whose `songs` list is the playlist (`file`, `title`, `artist`, and the
//! game, the song's own or the pack's `source.game`), and each song's `.vig` as the disc has it, which
//! `audio::Adpcm::vig` plays as it plays the disc's. A song's id is `mod:<pack id>/<file stem>`.
//!
//! Not in `audio.rs`: openmx-mods compiles that file into its own tools by `#[path]`, with no JSON
//! reader, so the part that reads `mod.json` lives here.

use crate::audio::{Adpcm, MOD, MusicSource, Stream, Track};
use crate::track_mods::{Json, valid_id};
use std::path::{Path, PathBuf};

/// The folder under `mods/` music packs live in.
pub const MUSIC_DIR: &str = "music";

/// The highest `standard` this reader knows (openmx-mods' standard §1).
const STANDARD: f64 = 1.0;

/// One song of a pack: as the lists show it, its game, and its stream's file.
struct Song {
    track: Track,
    game: String,
    file: PathBuf,
}

/// **Every music pack's songs**, packs in id order, each pack's in its playlist's order.
pub struct PackMusic {
    songs: Vec<Song>,
}

impl PackMusic {
    /// The packs under `mods` (none for none), and one line for each pack or song left out.
    pub fn read(mods: Option<&Path>) -> (PackMusic, Vec<String>) {
        let (mut songs, mut log) = (Vec::new(), Vec::new());
        let dir = mods.map(|m| m.join(MUSIC_DIR));
        let mut folders: Vec<PathBuf> = dir
            .and_then(|d| crate::files::list(d).ok())
            .into_iter()
            .flat_map(|r| r.into_iter().map(|e| e.path))
            .filter(|p| crate::files::is_file(p.join("mod.json")))
            .collect();
        folders.sort();
        for folder in folders {
            let label = folder.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if let Err(why) = read_pack(&folder, &mut songs, &mut log) {
                log.push(format!("music: music/{label} left out: {why}"));
            }
        }
        (PackMusic { songs }, log)
    }

    /// How many songs the packs have.
    pub fn len(&self) -> usize {
        self.songs.len()
    }
}

/// One pack's songs onto `songs`; `Ok` too for a mod of another kind, which adds none.
fn read_pack(folder: &Path, songs: &mut Vec<Song>, log: &mut Vec<String>) -> Result<(), String> {
    let text = crate::files::read_to_string(folder.join("mod.json")).map_err(|e| format!("mod.json: {e}"))?;
    let json = Json::parse(&text).map_err(|e| format!("mod.json: {e}"))?;
    if json.get("kind").and_then(Json::str) != Some("music") {
        return Ok(());
    }
    if json.get("standard").and_then(Json::f64).is_some_and(|n| n > STANDARD) {
        return Err("a newer standard than this build reads".into());
    }
    let id = json.get("id").and_then(Json::str).ok_or("mod.json has no id")?;
    let folder_name = folder.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    if id != folder_name || !valid_id(id) {
        return Err(format!("its id {id:?} is not its folder's name"));
    }
    // The game its songs are from, or the pack's own name where its source does not say (P317: what
    // Songs From lists it as).
    let source_game = json.get("source").and_then(|s| s.get("game")).and_then(Json::str).map(str::trim).filter(|g| !g.is_empty());
    let pack_game = source_game.or_else(|| json.get("name").and_then(Json::str).map(str::trim)).unwrap_or("").to_string();
    let Some(Json::Arr(list)) = json.get("songs") else { return Err("mod.json has no songs".into()) };
    for (n, song) in list.iter().enumerate() {
        let string = |key: &str| song.get(key).and_then(Json::str).map(|s| s.trim().to_string());
        let Some(file) = string("file").filter(|f| !f.is_empty()) else {
            log.push(format!("music: music/{id} song {}: no file", n + 1));
            continue;
        };
        // A path in the pack: no way out of its folder.
        let parts: Vec<&str> = file.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
        if parts.iter().any(|p| *p == ".." || p.contains(':')) || file.starts_with('/') {
            log.push(format!("music: music/{id} song {}: {file} is not a path in the pack", n + 1));
            continue;
        }
        let path = parts.iter().fold(folder.to_path_buf(), |p, part| p.join(part));
        if !crate::files::is_file(&path) {
            log.push(format!("music: music/{id} song {}: no {file}", n + 1));
            continue;
        }
        let stem = Path::new(parts.last().unwrap_or(&"")).file_stem().map(|s| s.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        let track_id = format!("{MOD}{id}/{stem}");
        if songs.iter().any(|s| s.track.id == track_id) {
            log.push(format!("music: music/{id} song {}: {file} is listed twice", n + 1));
            continue;
        }
        let title = string("title").filter(|t| !t.is_empty()).unwrap_or_else(|| stem.clone());
        let game = string("game").filter(|g| !g.is_empty()).unwrap_or_else(|| pack_game.clone());
        songs.push(Song { track: Track { id: track_id, title, artist: string("artist").unwrap_or_default() }, game, file: path });
    }
    Ok(())
}

impl MusicSource for PackMusic {
    fn name(&self) -> &str {
        MOD.trim_end_matches(':')
    }

    fn tracks(&self) -> Vec<Track> {
        self.songs.iter().map(|s| s.track.clone()).collect()
    }

    fn open(&self, id: &str) -> Result<Box<dyn Stream>, String> {
        let song = self.songs.iter().find(|s| s.track.id == id).ok_or_else(|| format!("no song {id}"))?;
        let bytes = crate::files::read(&song.file).map_err(|e| format!("{}: {e}", song.file.display()))?;
        Ok(Box::new(Adpcm::vig(bytes)))
    }

    fn game(&self, id: &str) -> Option<String> {
        self.songs.iter().find(|s| s.track.id == id).map(|s| s.game.clone()).filter(|g| !g.is_empty())
    }
}
