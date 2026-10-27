//! **Trick packs** (MD16): MX vs. ATV's tricks, read from `mods/tricks/<id>/` as a track mod is from
//! `mods/tricks/`'s sibling `tracks/` (MD2). openmx-mods' `tools/import-tricks` writes one off the
//! user's own MX vs. ATV disc: a `mod.json` of kind `tricks`, the riders' stunt tables and the bikes'
//! `.phy` files. What the simulation does with them is `game_sim::trick_pack`'s; this module only
//! finds the pack, reads its files and says what it could not use.

use crate::track_mods::Json;
use game_sim::phy::{Ini, Phy};
use game_sim::trick::Set;
use game_sim::trick_pack::{self, Row};
use std::path::{Path, PathBuf};

/// The folder under `mods/` trick packs live in.
pub const TRICKS_DIR: &str = "tricks";

/// A trick pack, read.
pub struct Pack {
    pub id: String,
    folder: PathBuf,
    /// Its stunt table's file (`riders[0].table`).
    table: PathBuf,
    /// The first rider's stunt table (`riders[0]`, MX vs. ATV's male motocross rider).
    rows: Vec<Row>,
    /// That rider's clip prefix (`riders[0].rider`, `MX`), which names its clips (MD21).
    rider: String,
    /// The pack's `.phy` files, by their name in lower case.
    phys: Vec<(String, PathBuf)>,
}

/// The first trick pack in `mods/tricks/`, by folder name, or why there is none.
pub fn find(mods: &Path) -> Result<Pack, String> {
    let dir = mods.join(TRICKS_DIR);
    let mut folders: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("mod.json").is_file())
        .collect();
    folders.sort();
    let mut why = Vec::new();
    for folder in folders {
        match read(&folder) {
            Ok(Some(pack)) => return Ok(pack),
            Ok(None) => {}
            Err(e) => why.push(format!("{}: {e}", folder.display())),
        }
    }
    why.push(format!("no trick pack in {}", dir.display()));
    Err(why.join("; "))
}

/// The trick pack in `mods/tricks/` whose `mod.json` says `id` (MD19: the one an online room rides),
/// or why there is none.
pub fn by_id(mods: &Path, id: &str) -> Result<Pack, String> {
    let dir = mods.join(TRICKS_DIR);
    let mut folders: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("mod.json").is_file())
        .collect();
    folders.sort();
    let mut why = Vec::new();
    for folder in folders {
        match read(&folder) {
            Ok(Some(pack)) if pack.id == id => return Ok(pack),
            Ok(_) => {}
            Err(e) => why.push(format!("{}: {e}", folder.display())),
        }
    }
    why.push(format!("no trick pack {id} in {}", dir.display()));
    Err(why.join("; "))
}

/// `Ok(None)` for a mod that is not a trick pack.
fn read(folder: &Path) -> Result<Option<Pack>, String> {
    let text = std::fs::read_to_string(folder.join("mod.json")).map_err(|e| format!("mod.json: {e}"))?;
    let json = Json::parse(&text).map_err(|e| format!("mod.json: {e}"))?;
    if json.get("kind").and_then(Json::str) != Some("tricks") {
        return Ok(None);
    }
    let id = json.get("id").and_then(Json::str).ok_or("mod.json has no id")?.to_owned();
    let first = match json.get("riders") {
        Some(Json::Arr(riders)) => riders.first(),
        _ => None,
    };
    let table = first.and_then(|r| r.get("table")).and_then(Json::str).ok_or("mod.json has no riders[0].table")?;
    // The motocross rider's when the pack does not say: every pack import-tricks writes says `MX`.
    let rider = first.and_then(|r| r.get("rider")).and_then(Json::str).unwrap_or("MX").to_owned();
    let table = file(folder, table);
    let bytes = std::fs::read(&table).map_err(|e| format!("{}: {e}", table.display()))?;
    let rows = trick_pack::table(&bytes).map_err(|e| format!("{}: {e}", table.display()))?;
    let mut phys = Vec::new();
    if let Some(Json::Arr(list)) = json.get("phy") {
        for name in list.iter().filter_map(Json::str) {
            let path = file(folder, name);
            if let Some(base) = path.file_name().and_then(|n| n.to_str()) {
                phys.push((base.to_ascii_lowercase(), path));
            }
        }
    }
    Ok(Some(Pack { id, folder: folder.to_owned(), table, rows, rider, phys }))
}

/// A pack's file by its `/`-separated name, kept inside the pack's folder.
fn file(folder: &Path, name: &str) -> PathBuf {
    name.split(['/', '\\']).filter(|p| !p.is_empty() && *p != "..").fold(folder.to_owned(), |p, s| p.join(s))
}

impl Pack {
    /// The rider's stunts with the pack's extras added (Cross's and Square's).
    pub fn stunts(&self, stunts: Set) -> Set {
        let extras = trick_pack::extras(&self.rows, &stunts);
        stunts.with_extras(extras)
    }

    /// Lay the pack's moves over a bike read from `member` (`race125p.phy`): the pack's own file for
    /// that bike, `mx_` + its name. A bike the pack has no file for, or whose file has no 360, keeps
    /// the original's tricks; the line says so.
    pub fn lay(&self, bike: &mut Phy, member: &str) -> String {
        let want = trick_pack::phy_name_for(member);
        let Some((_, path)) = self.phys.iter().find(|(n, _)| *n == want) else {
            return format!("tricks: {} has no {want}, so this bike keeps the original's tricks", self.id);
        };
        let ini = match std::fs::read(path) {
            Ok(bytes) => Ini::parse_bytes(&bytes),
            Err(e) => return format!("tricks: {}: {e}; the original's tricks", path.display()),
        };
        match trick_pack::moves(&ini, &self.rows) {
            Some(moves) => {
                trick_pack::apply(bike, &ini, moves);
                format!("tricks: {}'s 360, front flip and whip from {want}", self.id)
            }
            None => format!("tricks: {want} has no [Three Sixty]; the original's tricks"),
        }
    }

    /// What an online ride's handshake compares (MD19): 64-bit FNV-1a over every file of the pack the
    /// simulation reads — its `mod.json`, its stunt table and its `.phy` files, each after its name —
    /// so two machines with the same id but another file are told apart. The clips are drawn, not
    /// simulated, and are not in it.
    pub fn hash(&self) -> Result<u64, String> {
        let mut all = Vec::new();
        let mut files = vec![("mod.json".to_string(), self.folder.join("mod.json")), ("table".to_string(), self.table.clone())];
        files.extend(self.phys.iter().cloned());
        for (name, path) in files {
            all.extend(name.as_bytes());
            all.push(0);
            all.extend(std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?);
        }
        Ok(openmx_net::handshake::fnv(&all))
    }

    /// The pack's clips, for the rider and the bike to bind (MD21).
    pub fn clips(&self) -> Result<game_render::anim::PackClips, String> {
        game_render::anim::PackClips::open(&self.folder, &self.rider)
    }

    /// What the ride shows of the pack's tricks (MD21): which of its extras and 360s found no clip
    /// on the rider (`rider`) and the bike (`bike`) — the names [`game_render::anim::PackClips::fill`]
    /// answered, `None` for a body or chassis that was not loaded — and the front flip, which has no
    /// clip in either game: the rider leans forward on the airborne row (P159) and the bike, which
    /// follows the body's state (MD41), stays on its own airborne row, not the backflip's clip.
    pub fn missing_clips(&self, stunts: &Set, rider: Option<&[String]>, bike: Option<&[String]>) -> String {
        let extras = stunts.stunts.iter().skip(game_sim::trick::SLOTS).filter(|s| !s.name.is_empty()).count();
        let side = |who: &str, missing: Option<&[String]>| match missing {
            None => format!("no {who} to bind them to"),
            Some([]) => format!("the {who} has all {} of them", extras + 2),
            Some(names) => format!("the {who} has {} of {}, none for {}", extras + 2 - names.len(), extras + 2, names.join(", ")),
        };
        format!(
            "tricks: clips from {} for {extras} extras and the two 360s: {}; {}; the front flip has none in either game: the rider leans forward and the bike stays on its airborne row",
            self.folder.display(),
            side("rider", rider),
            side("bike", bike)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pack of `id` in a fresh `mods/` in the temp dir, its table `table`.
    fn pack(name: &str, id: &str, table: &str) -> PathBuf {
        let mods = std::env::temp_dir().join(format!("openmx-trick-packs-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&mods);
        let folder = mods.join(TRICKS_DIR).join(id);
        std::fs::create_dir_all(folder.join("Bike")).unwrap();
        let json = format!(r#"{{"kind": "tricks", "id": "{id}", "riders": [{{"rider": "MX", "table": "stunts.ini"}}], "phy": ["Bike/mx_race125p.phy"]}}"#);
        std::fs::write(folder.join("mod.json"), json).unwrap();
        std::fs::write(folder.join("stunts.ini"), table).unwrap();
        std::fs::write(folder.join("Bike/mx_race125p.phy"), "[Three Sixty]\n").unwrap();
        mods
    }

    #[test]
    fn a_pack_is_found_by_its_id_and_a_changed_file_hashes_otherwise() {
        let a = pack("a", "mxvatv", "[Stunt01]\nName=threesixtyL\n");
        let b = pack("b", "mxvatv", "[Stunt01]\nName=threesixtyL\n");
        let same = by_id(&a, "mxvatv").unwrap().hash().unwrap();
        assert_eq!(by_id(&b, "mxvatv").unwrap().hash().unwrap(), same, "the same files on another machine");
        assert!(by_id(&a, "untamed").err().unwrap_or_default().contains("no trick pack untamed"));
        std::fs::write(b.join(TRICKS_DIR).join("mxvatv").join("stunts.ini"), "[Stunt01]\nName=threesixtyR\n").unwrap();
        assert_ne!(by_id(&b, "mxvatv").unwrap().hash().unwrap(), same, "a changed table");
        let c = pack("c", "mxvatv", "[Stunt01]\nName=threesixtyL\n");
        std::fs::write(c.join(TRICKS_DIR).join("mxvatv").join("Bike/mx_race125p.phy"), "[Three Sixty]\nx=1\n").unwrap();
        assert_ne!(by_id(&c, "mxvatv").unwrap().hash().unwrap(), same, "a changed .phy");
        for m in [a, b, c] {
            let _ = std::fs::remove_dir_all(m);
        }
    }
}
