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
    /// The first rider's stunt table (`riders[0]`, MX vs. ATV's male motocross rider).
    rows: Vec<Row>,
    /// That rider's clip prefix (`riders[0].rider`, `MX`), which names its clips (MD21).
    rider: String,
    /// The pack's `.phy` files, by their name in lower case.
    phys: Vec<(String, PathBuf)>,
    /// How the 360 is asked for and turns on the bikes it is laid over (MD25): the easy one unless
    /// the shell says otherwise.
    pub spin_style: game_sim::air::SpinStyle,
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
    let bytes = std::fs::read(file(folder, table)).map_err(|e| format!("{table}: {e}"))?;
    let rows = trick_pack::table(&bytes).map_err(|e| format!("{table}: {e}"))?;
    let mut phys = Vec::new();
    if let Some(Json::Arr(list)) = json.get("phy") {
        for name in list.iter().filter_map(Json::str) {
            let path = file(folder, name);
            if let Some(base) = path.file_name().and_then(|n| n.to_str()) {
                phys.push((base.to_ascii_lowercase(), path));
            }
        }
    }
    Ok(Some(Pack { id, folder: folder.to_owned(), rows, rider, phys, spin_style: Default::default() }))
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
            Some(mut moves) => {
                moves.spin_style = self.spin_style;
                trick_pack::apply(bike, &ini, moves);
                format!("tricks: {}'s 360, front flip and whip from {want}", self.id)
            }
            None => format!("tricks: {want} has no [Three Sixty]; the original's tricks"),
        }
    }

    /// The pack's clips, for the rider and the bike to bind (MD21).
    pub fn clips(&self) -> Result<game_render::anim::PackClips, String> {
        game_render::anim::PackClips::open(&self.folder, &self.rider)
    }

    /// What the ride shows of the pack's tricks (MD21): which of its extras and 360s found no clip
    /// on the rider (`rider`) and the bike (`bike`) — the names [`game_render::anim::PackClips::fill`]
    /// answered, `None` for a body or chassis that was not loaded — and the front flip, which has no
    /// clip in either game and shows the backflip's.
    pub fn missing_clips(&self, stunts: &Set, rider: Option<&[String]>, bike: Option<&[String]>) -> String {
        let extras = stunts.stunts.iter().skip(game_sim::trick::SLOTS).filter(|s| !s.name.is_empty()).count();
        let side = |who: &str, missing: Option<&[String]>| match missing {
            None => format!("no {who} to bind them to"),
            Some([]) => format!("the {who} has all {} of them", extras + 2),
            Some(names) => format!("the {who} has {} of {}, none for {}", extras + 2 - names.len(), extras + 2, names.join(", ")),
        };
        format!(
            "tricks: clips from {} for {extras} extras and the two 360s: {}; {}; the front flip has none in either game and shows the backflip's",
            self.folder.display(),
            side("rider", rider),
            side("bike", bike)
        )
    }
}
