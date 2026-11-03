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
    /// Its `mod.json` name, the id when it has none.
    pub name: String,
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
    let mut folders: Vec<PathBuf> = crate::files::list(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .into_iter()
        .map(|e| e.path)
        .filter(|p| crate::files::is_file(p.join("mod.json")))
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
    let mut folders: Vec<PathBuf> = crate::files::list(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .into_iter()
        .map(|e| e.path)
        .filter(|p| crate::files::is_file(p.join("mod.json")))
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

/// Every trick pack in `mods/tricks/` (MD77), in folder-name order — the order the Ride tab lists
/// them and a stacked ride settles a stunt two packs both have — and what could not be read. A
/// folder without one is no pack; an id two folders both have is the first's.
pub fn listed(mods: &Path) -> (Vec<Pack>, Vec<String>) {
    let dir = mods.join(TRICKS_DIR);
    let Ok(entries) = crate::files::list(&dir) else { return (Vec::new(), Vec::new()) };
    let mut folders: Vec<PathBuf> = entries.into_iter().map(|e| e.path).filter(|p| crate::files::is_file(p.join("mod.json"))).collect();
    folders.sort();
    let (mut packs, mut why): (Vec<Pack>, Vec<String>) = (Vec::new(), Vec::new());
    for folder in folders {
        match read(&folder) {
            Ok(Some(pack)) if !packs.iter().any(|p| p.id == pack.id) => packs.push(pack),
            Ok(_) => {}
            Err(e) => why.push(format!("{}: {e}", folder.display())),
        }
    }
    (packs, why)
}

/// The packs of `ids` (MD77), in `ids`' order — a room's, which is the host's folder's and may not
/// be this machine's — or why one of them is not there.
pub fn on(mods: &Path, ids: &[String]) -> Result<Vec<Pack>, String> {
    let (packs, why) = listed(mods);
    if let Some(id) = ids.iter().find(|id| !packs.iter().any(|p| p.id == **id)) {
        let mut why = why;
        why.push(format!("no trick pack {id} in {}", mods.join(TRICKS_DIR).display()));
        return Err(why.join("; "));
    }
    let mut packs: Vec<Pack> = packs.into_iter().filter(|p| ids.contains(&p.id)).collect();
    packs.sort_by_key(|p| ids.iter().position(|i| *i == p.id));
    Ok(packs)
}

/// The rider's stunts with the extras of every pack in `packs` stacked (MD77,
/// [`trick_pack::stacked`]), and how many of them found no gesture.
pub fn stunts(packs: &[Pack], stunts: Set) -> (Set, usize) {
    let tables: Vec<&[Row]> = packs.iter().map(|p| p.rows.as_slice()).collect();
    let (extras, over) = trick_pack::stacked(&tables, &stunts);
    (stunts.with_extras(extras), over)
}

/// Lay the stacked packs' moves over a bike (MD77): the first of them, in order, with a file for it
/// that has a 360 lays its 360, front flip and whip; the line says which, or why none did.
pub fn lay(packs: &[Pack], bike: &mut Phy, member: &str) -> String {
    let mut lines = Vec::new();
    for pack in packs {
        let line = pack.lay(bike, member);
        if bike.tricks.is_some() {
            return line;
        }
        lines.push(line);
    }
    lines.join("; ")
}

/// What an online ride's handshake compares for a stack of packs (MD77): one pack's own
/// [`Pack::hash`], so a room of one pack is what it was before; for more, FNV-1a over each id and
/// hash in order.
pub fn hash(packs: &[Pack]) -> Result<u64, String> {
    if let [one] = packs {
        return one.hash();
    }
    let mut all = Vec::new();
    for p in packs {
        all.extend(p.id.as_bytes());
        all.push(0);
        all.extend(p.hash()?.to_le_bytes());
    }
    Ok(openmx_net::handshake::fnv(&all))
}

/// The packs a ride stacks out of `installed`'s ids (MD77), in their order: those `named`, or with
/// none named the first (MD16's choice).
pub fn chosen(installed: &[String], named: &[String]) -> Vec<String> {
    match named.is_empty() {
        true => installed.iter().take(1).cloned().collect(),
        false => installed.iter().filter(|id| named.contains(id)).cloned().collect(),
    }
}

/// A room's list of trick pack ids (MD77): `a,b`, and back.
pub fn room_ids(ids: &[String]) -> String {
    ids.join(",")
}

/// The ids a room's `tricks` names (MD77), in its order; none for the original's.
pub fn ids_of_room(word: &str) -> Vec<String> {
    word.split(',').map(str::trim).filter(|w| !w.is_empty()).map(str::to_string).collect()
}

/// The stacked packs' clips (MD21, MD77), each stunt's looked for in the packs in order.
pub fn clips(packs: &[Pack]) -> Result<Option<game_render::anim::PackClips>, String> {
    let clips = packs.iter().map(Pack::clips).collect::<Result<Vec<_>, String>>()?;
    Ok(game_render::anim::PackClips::stack(clips))
}

/// `Ok(None)` for a mod that is not a trick pack.
fn read(folder: &Path) -> Result<Option<Pack>, String> {
    let text = crate::files::read_to_string(folder.join("mod.json")).map_err(|e| format!("mod.json: {e}"))?;
    let json = Json::parse(&text).map_err(|e| format!("mod.json: {e}"))?;
    if json.get("kind").and_then(Json::str) != Some("tricks") {
        return Ok(None);
    }
    let id = json.get("id").and_then(Json::str).ok_or("mod.json has no id")?.to_owned();
    let name = json.get("name").and_then(Json::str).unwrap_or(&id).to_owned();
    let first = match json.get("riders") {
        Some(Json::Arr(riders)) => riders.first(),
        _ => None,
    };
    let table = first.and_then(|r| r.get("table")).and_then(Json::str).ok_or("mod.json has no riders[0].table")?;
    // The motocross rider's when the pack does not say: every pack import-tricks writes says `MX`.
    let rider = first.and_then(|r| r.get("rider")).and_then(Json::str).unwrap_or("MX").to_owned();
    let table = with_bpb(file(folder, table));
    let bytes = crate::files::read(&table).map_err(|e| format!("{}: {e}", table.display()))?;
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
    Ok(Some(Pack { id, name, folder: folder.to_owned(), table, rows, rider, phys }))
}

/// The table's `<name>.bpb` when the pack has one, else the name: SLUS_217.01's loader (`0x001425c0`)
/// opens `%s.bpb` first and the text only when there is none (MD69).
fn with_bpb(path: PathBuf) -> PathBuf {
    let mut bpb = path.clone().into_os_string();
    bpb.push(".bpb");
    let bpb = PathBuf::from(bpb);
    if crate::files::is_file(&bpb) { bpb } else { path }
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

    /// The names of every stunt the pack could add to `stunts` (MD77), for the Ride tab's count.
    pub fn extra_names(&self, stunts: &Set) -> Vec<String> {
        trick_pack::candidates(&self.rows, stunts).into_iter().map(|s| s.name).collect()
    }

    /// The bike read from `member` (`race125p.phy`) on MX vs. ATV's handling (MD40): the pack's own
    /// file for it, `mx_` + its name, read through [`game_sim::handling::mxvatv_phy`]. A bike the
    /// pack has no file for is `Err` with the line that says so, and rides the original's.
    ///
    /// Untamed's pack (MD81a) has the files compiled, `mx_<name>.phy.bpb`: that is taken first, as
    /// SLUS-21701's loader opens `%s.bpb` first, and read through [`game_sim::bpb::ini`].
    pub fn handling(&self, member: &str, curve: &game_sim::camera::CurveTable) -> Result<(Phy, String), String> {
        let want = trick_pack::phy_name_for(member);
        let bpb = format!("{want}.bpb");
        let found = self.phys.iter().find(|(n, _)| *n == bpb).or_else(|| self.phys.iter().find(|(n, _)| *n == want));
        let Some((name, path)) = found else {
            return Err(format!("handling: {} has no {want}, so this bike handles as the original's", self.id));
        };
        let bytes = crate::files::read(path).map_err(|e| format!("handling: {}: {e}; the original's", path.display()))?;
        let ini = game_sim::bpb::ini(&bytes).map_err(|e| format!("handling: {}: {e}; the original's", path.display()))?;
        Ok((game_sim::handling::mxvatv_phy_ini(&ini, curve), format!("handling: MX vs. ATV's, {name} from {}", self.id)))
    }

    /// Lay the pack's moves over a bike read from `member` (`race125p.phy`): the pack's own file for
    /// that bike, `mx_` + its name. A bike the pack has no file for, or whose file has no 360, keeps
    /// the original's tricks; the line says so.
    pub fn lay(&self, bike: &mut Phy, member: &str) -> String {
        let want = trick_pack::phy_name_for(member);
        let Some((_, path)) = self.phys.iter().find(|(n, _)| *n == want) else {
            return format!("tricks: {} has no {want}, so this bike keeps the original's tricks", self.id);
        };
        let ini = match crate::files::read(path) {
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
            all.extend(crate::files::read(&path).map_err(|e| format!("{}: {e}", path.display()))?);
        }
        Ok(openmx_net::handshake::fnv(&all))
    }

    /// The pack's clips, for the rider and the bike to bind (MD21).
    pub fn clips(&self) -> Result<game_render::anim::PackClips, String> {
        Ok(game_render::anim::PackClips::of(crate::files::folder(&self.folder)?, &self.rider))
    }

    /// What the ride shows of the pack's tricks (MD21): which of its extras and 360s found no clip
    /// on the rider (`rider`) and the bike (`bike`) — the names [`game_render::anim::PackClips::fill`]
    /// answered, `None` for a body or chassis that was not loaded — and the front flip, which has no
    /// clip in either game: the rider leans forward on the airborne row (P159) and the bike, which
    /// follows the body's state (MD41), stays on its own airborne row, not the backflip's clip.
    pub fn missing_clips(&self, stunts: &Set, rider: Option<&[String]>, bike: Option<&[String]>) -> String {
        missing_clips(std::slice::from_ref(self), stunts, rider, bike)
    }
}

/// [`Pack::missing_clips`] for a stack of packs (MD77), whose clips are looked for in each in turn.
pub fn missing_clips(packs: &[Pack], stunts: &Set, rider: Option<&[String]>, bike: Option<&[String]>) -> String {
    {
        let extras = stunts.stunts.iter().skip(game_sim::trick::SLOTS).filter(|s| !s.name.is_empty()).count();
        let side = |who: &str, missing: Option<&[String]>| match missing {
            None => format!("no {who} to bind them to"),
            Some([]) => format!("the {who} has all {} of them", extras + 2),
            Some(names) => format!("the {who} has {} of {}, none for {}", extras + 2 - names.len(), extras + 2, names.join(", ")),
        };
        format!(
            "tricks: clips from {} for {extras} extras and the two 360s: {}; {}; the front flip has none in either game: the rider leans forward and the bike stays on its airborne row",
            packs.iter().map(|p| p.folder.display().to_string()).collect::<Vec<_>>().join(" and "),
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

    /// MD81a: Untamed's pack (`import-tricks untamed`, installed in this checkout's `mods/tricks`) gives
    /// the MX P bikes on MX vs. ATV's handling its compiled `mx_*.phy.bpb`, decoded, with the `[Wrecks]`
    /// limits SLUS-21701 raised (front 14, back 17, side 14.5 mph, both downs 54); its A bikes keep
    /// Unleashed's. Skips without the pack.
    #[test]
    fn untamed_s_compiled_bikes_ride_with_their_wreck_limits() {
        let mods = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods");
        let Ok(pack) = by_id(&mods, "untamed_tricks") else {
            eprintln!("skipped: no untamed_tricks pack in {} (openmx-mods: import-tricks untamed)", mods.display());
            return;
        };
        let curve = game_sim::camera::CurveTable::build();
        let mph = |v: f32| (ps2_float::F(v) * game_sim::phy::MPH).0;
        for set in ["race50p", "race125p", "race250p", "race500p", "pro50p", "pro125p", "pro250p", "pro500p"] {
            let (phy, line) = pack.handling(&format!("{set}.phy"), &curve).unwrap();
            assert!(line.contains(&format!("mx_{set}.phy.bpb from untamed_tricks")), "{line}");
            assert_eq!(phy.handling, game_sim::handling::Handling::MxVsAtv);
            let a = &phy.air;
            let got = [a.front_wreck_speed_limit.0, a.back_wreck_speed_limit.0, a.side_wreck_speed_limit.0,
                       a.racing_down_wreck_speed_limit.0, a.freestyle_down_wreck_speed_limit.0];
            assert_eq!(got, [mph(14.0), mph(17.0), mph(14.5), mph(54.0), mph(54.0)], "{set}");
        }
        // The A bikes are Unleashed's files: the same limits as the Unleashed pack's, when it is there.
        if let Ok(unleashed) = by_id(&mods, "mxvatv_tricks") {
            for set in ["race125a", "pro500a"] {
                let limits = |p: &Pack| {
                    let a = p.handling(&format!("{set}.phy"), &curve).unwrap().0.air;
                    [a.front_wreck_speed_limit, a.back_wreck_speed_limit, a.side_wreck_speed_limit,
                     a.racing_down_wreck_speed_limit, a.freestyle_down_wreck_speed_limit]
                };
                assert_eq!(limits(&pack), limits(&unleashed), "{set} keeps Unleashed's limits");
            }
            let (phy, _) = unleashed.handling("race125p.phy", &curve).unwrap();
            assert_eq!(phy.air.front_wreck_speed_limit.0, mph(12.5), "Unleashed's race125p");
        }
    }
}
