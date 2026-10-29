//! **Machine packs** (MD37c): a machine MX Unleashed does not have, as another game's disc has it,
//! read from `mods/machines/<id>/` as a trick pack is from `mods/tricks/` (MD16). openmx-mods'
//! `tools/import-atv` writes one off the user's own MX vs. ATV Unleashed disc: a `mod.json` of kind
//! `machine` and the quad's own files at their disc paths — the builds (`.sub`), the chassis models
//! and parts, the materials and textures, the quad's and its rider's clips, the stunt table, the
//! follow cameras, the engine's `.aud` and its sounds, the particle lists and their emitters.
//!
//! This module finds the pack and reads each of those through the reader openmx already has for that
//! kind of file, and says what it could not read. Nothing rides it yet: the quad's step is MD37b and
//! picking it is MD37d, so with no pack, or a pack and no quad picked, every ride is as it was.

use crate::audio::{BikeSounds, Clip as Sound};
use crate::particles::Emt;
use crate::track_mods::Json;
use game_render::biplane::{self, Emitter};
use game_render::disc::Res;
use game_render::psg::Model;
use game_render::quad;
use game_render::rider::MotionSet;
use game_render::scene::{Preset, Vehicle};
use game_render::skin::{Clip, Skeleton};
use game_render::ini;
use game_sim::trick_pack::{self, Row};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The folder under `mods/` machine packs live in.
pub const MACHINES_DIR: &str = "machines";

/// A machine pack, its `mod.json` read and its folder opened as an archive.
pub struct Pack {
    pub id: String,
    pub name: String,
    /// The machine it is to be ridden as (`atv`).
    pub machine: String,
    pub folder: PathBuf,
    /// The pack's files, by their disc paths.
    pub files: Res,
    /// `builds`: each `.sub` with its motion set and the chassis model that set names.
    pub builds: Vec<Build>,
    /// `rider`: the rider's clip prefix (`ATV`), its riding and stunt motion sets and stunt table.
    pub rider: String,
    rider_motions: String,
    rider_stunts: String,
    table: String,
    cameras: String,
    engine: String,
    particles: String,
    /// What the importer could not bring (`todo`).
    pub todo: Vec<String>,
}

/// One build of the machine, as `mod.json` lists it.
#[derive(Clone, Debug)]
pub struct Build {
    pub sub: String,
    pub mcf: String,
    pub chassis: String,
}

/// Every machine pack in `mods/machines/`, by folder name, and a line for each folder that is a
/// machine pack and could not be read.
pub fn find(mods: &Path) -> (Vec<Pack>, Vec<String>) {
    let dir = mods.join(MACHINES_DIR);
    let mut folders: Vec<PathBuf> = match std::fs::read_dir(&dir) {
        Ok(d) => d.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.join("mod.json").is_file()).collect(),
        Err(_) => Vec::new(),
    };
    folders.sort();
    let (mut packs, mut why) = (Vec::new(), Vec::new());
    for folder in folders {
        match read(&folder) {
            Ok(Some(pack)) => packs.push(pack),
            Ok(None) => {}
            Err(e) => why.push(format!("{}: {e}", folder.display())),
        }
    }
    (packs, why)
}

/// `Ok(None)` for a mod that is not a machine pack.
pub fn read(folder: &Path) -> Result<Option<Pack>, String> {
    let text = std::fs::read_to_string(folder.join("mod.json")).map_err(|e| format!("mod.json: {e}"))?;
    let json = Json::parse(&text).map_err(|e| format!("mod.json: {e}"))?;
    if json.get("kind").and_then(Json::str) != Some("machine") {
        return Ok(None);
    }
    let s = |j: Option<&Json>| j.and_then(Json::str).unwrap_or("").to_owned();
    let id = json.get("id").and_then(Json::str).ok_or("mod.json has no id")?.to_owned();
    let mut builds = Vec::new();
    if let Some(Json::Arr(list)) = json.get("builds") {
        for b in list {
            builds.push(Build { sub: s(b.get("sub")), mcf: s(b.get("mcf")), chassis: s(b.get("chassis")) });
        }
    }
    if builds.is_empty() {
        return Err("mod.json lists no builds".into());
    }
    let rider = json.get("rider");
    let todo = match json.get("todo") {
        Some(Json::Arr(list)) => list.iter().filter_map(Json::str).map(str::to_owned).collect(),
        _ => Vec::new(),
    };
    Ok(Some(Pack {
        name: s(json.get("name")),
        machine: s(json.get("machine")),
        files: Res::folder(folder)?,
        folder: folder.to_owned(),
        builds,
        rider: rider.and_then(|r| r.get("rider")).and_then(Json::str).unwrap_or("ATV").to_owned(),
        rider_motions: s(rider.and_then(|r| r.get("motions"))),
        rider_stunts: s(rider.and_then(|r| r.get("stunts"))),
        table: s(rider.and_then(|r| r.get("table"))),
        cameras: s(json.get("cameras")),
        engine: s(json.get("sounds").and_then(|j| j.get("engine"))),
        particles: s(json.get("particles").and_then(|j| j.get("list"))),
        todo,
        id,
    }))
}

/// A `/`-separated `mod.json` name as the archive's `\` one.
fn member(name: &str) -> String {
    name.replace('/', "\\")
}

impl Pack {
    fn text(&self, name: &str) -> Result<String, String> {
        Ok(ini::text(&self.files.read(&member(name))?))
    }

    /// A model by its name (`ATVS_R_Chasis`, `yfz450_plastic`): `<name>.psg` anywhere in the pack.
    pub fn model(&self, name: &str) -> Result<Model, String> {
        let bytes = self.files.read_suffix(&format!("{name}.psg"))?;
        Model::parse(&bytes).map_err(|e| format!("{name}.psg: {e}"))
    }

    /// One build's `.sub`, read.
    pub fn vehicle(&self, build: &Build) -> Result<Vehicle, String> {
        Ok(Vehicle::parse(&self.text(&build.sub)?))
    }

    /// The quad's moving nodes in a chassis model's skeleton.
    pub fn quad_nodes(&self, chassis: &str) -> Result<quad::Nodes, String> {
        Ok(quad::Nodes::of(&Skeleton::of(&self.model(chassis)?)))
    }

    /// Every clip a motion set names, read: `(name, clip)` for those openmx reads, and a line for
    /// each it could not.
    fn set_clips(&self, mcf: &str) -> Result<(Vec<(String, Clip)>, Vec<String>), String> {
        let set = MotionSet::parse(&self.text(mcf)?);
        let (mut clips, mut why) = (Vec::new(), Vec::new());
        for m in &set.motions {
            match self.files.read_suffix(&m.clip).and_then(|b| Clip::parse(&b)) {
                Ok(c) => clips.push((m.name.clone(), c)),
                Err(e) => why.push(format!("{mcf}: {} ({}): {e}", m.name, m.clip)),
            }
        }
        Ok((clips, why))
    }

    /// The rider's riding and stunt clips, out of the two motion sets `rider` names.
    pub fn rider_clips(&self) -> Result<(Vec<(String, Clip)>, Vec<String>), String> {
        let (mut clips, mut why) = self.set_clips(&self.rider_motions)?;
        let (more, more_why) = self.set_clips(&self.rider_stunts)?;
        clips.extend(more);
        why.extend(more_why);
        Ok((clips, why))
    }

    /// The quad's own trick clips (`Veh\Anim\ATV_Bike_<name>.mot`, MD21's naming), by `<name>`.
    pub fn machine_clips(&self) -> Result<Vec<(String, Clip)>, String> {
        let prefix = format!("veh\\anim\\{}_bike_", self.rider.to_ascii_lowercase());
        let mut out = Vec::new();
        for e in self.files.entries() {
            let lower = e.name.to_ascii_lowercase();
            if let Some(name) = lower.strip_prefix(&prefix).and_then(|n| n.strip_suffix(".mot")) {
                let clip = Clip::parse(&self.files.read(&e.name)?).map_err(|err| format!("{}: {err}", e.name))?;
                out.push((name.to_owned(), clip));
            }
        }
        Ok(out)
    }

    /// The rider's stunt table, read as a trick pack's is.
    pub fn stunts(&self) -> Result<Vec<Row>, String> {
        trick_pack::table(&self.files.read(&member(&self.table))?).map_err(|e| format!("{}: {e}", self.table))
    }

    /// The follow cameras of one screen layout (`4x3/1p`, `16x9/1p`, `4x3/2pv`, …).
    pub fn cameras(&self, view: &str) -> Result<Vec<Preset>, String> {
        Ok(Preset::parse_follow(&self.text(&self.cameras)?, view))
    }

    /// The engine's sounds: the `.aud`'s lists with each `.vag` it names read from the pack's `Aud\`.
    /// A sound the pack lacks or openmx cannot decode is left out of its list, and named in the lines.
    pub fn engine_sounds(&self) -> Result<(BikeSounds, Vec<String>), String> {
        let text = self.text(&self.engine)?;
        let mut loaded: HashMap<String, Option<Arc<Sound>>> = HashMap::new();
        let mut why = Vec::new();
        let mut lib = |name: &str| -> Option<Arc<Sound>> {
            // A name with no extension (`[Wreck]`'s `Vehicle\WRECK01`) is that name's `.VAG`.
            let mut path = format!("Aud\\{}", name.trim().trim_matches('"'));
            if !path.rsplit('\\').next().unwrap_or("").contains('.') {
                path = [".VAG", ".vig"].iter().map(|x| format!("{path}{x}")).find(|p| self.files.find_suffix(p).is_some()).unwrap_or(path);
            }
            loaded
                .entry(path.to_ascii_lowercase())
                .or_insert_with(|| match self.files.read(&path).and_then(|b| Sound::read(&path, &b)) {
                    Ok(c) => Some(Arc::new(c)),
                    Err(e) => {
                        why.push(format!("{path}: {e}"));
                        None
                    }
                })
                .clone()
        };
        let sounds = BikeSounds::read(&text, &mut lib);
        Ok((sounds, why))
    }

    /// The particle list's emitters, each with its `.emt` read (`None` for one the pack lacks).
    pub fn emitters(&self) -> Result<Vec<(Emitter, Option<Emt>)>, String> {
        let list = biplane::emitters(&self.text(&self.particles)?);
        Ok(list
            .into_iter()
            .map(|e| {
                let emt = self.files.read_suffix(&e.emitter).ok().map(|b| Emt::parse(&ini::text(&b)));
                (e, emt)
            })
            .collect())
    }

    /// Everything above read once: what each reader made of the pack, a line a part, and every file it
    /// could not use.
    pub fn report(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "{} ({}): machine {}, in {}, {} files, {} builds",
            self.id,
            self.name,
            self.machine,
            self.folder.display(),
            self.files.entries().len(),
            self.builds.len()
        )];
        let mut chassis: Vec<(&str, &str)> = self.builds.iter().map(|b| (b.chassis.as_str(), b.mcf.as_str())).collect();
        chassis.sort_by_key(|(c, _)| c.to_ascii_lowercase());
        chassis.dedup_by_key(|(c, _)| c.to_ascii_lowercase());
        for (c, mcf) in chassis {
            match self.quad_nodes(c) {
                Ok(n) if n.missing().is_empty() => lines.push(format!("  {c} ({mcf}'s): every quad node")),
                Ok(n) => lines.push(format!("  {c} ({mcf}'s): lacks {}", n.missing().join(", "))),
                Err(e) => lines.push(format!("  {c} ({mcf}'s): {e}")),
            }
        }
        let (mut parts, mut unread) = (0, Vec::new());
        for b in &self.builds {
            match self.vehicle(b) {
                Ok(v) => {
                    for p in &v.parts {
                        match self.model(&p.model) {
                            Ok(_) => parts += 1,
                            Err(e) => unread.push(format!("{}: {e}", b.sub)),
                        }
                    }
                }
                Err(e) => unread.push(e),
            }
        }
        lines.push(format!("  builds: {parts} parts read, {} not", unread.len()));
        lines.extend(unread.iter().map(|w| format!("    {w}")));
        match self.rider_clips() {
            Ok((clips, why)) => {
                lines.push(format!("  rider {}: {} clips read, {} not", self.rider, clips.len(), why.len()));
                lines.extend(why.iter().map(|w| format!("    {w}")));
            }
            Err(e) => lines.push(format!("  rider: {e}")),
        }
        match self.machine_clips() {
            Ok(clips) => lines.push(format!("  machine clips: {}", clips.len())),
            Err(e) => lines.push(format!("  machine clips: {e}")),
        }
        match self.stunts() {
            Ok(rows) => lines.push(format!("  stunts: {} rows", rows.len())),
            Err(e) => lines.push(format!("  stunts: {e}")),
        }
        match self.cameras("4x3/1p") {
            Ok(c) => lines.push(format!("  cameras: {} (4x3/1p)", c.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", "))),
            Err(e) => lines.push(format!("  cameras: {e}")),
        }
        match self.engine_sounds() {
            Ok((s, why)) => {
                let n = [&s.idle, &s.low, &s.high, &s.power, &s.sputter, &s.decel, &s.clutch, &s.panic, &s.wreck.list].iter().map(|l| l.sounds.len()).sum::<usize>();
                lines.push(format!("  engine: {n} sounds, {} not read", why.len()));
                lines.extend(why.iter().map(|w| format!("    {w}")));
            }
            Err(e) => lines.push(format!("  engine: {e}")),
        }
        match self.emitters() {
            Ok(list) => {
                let lacking: Vec<&str> = list.iter().filter(|(_, emt)| emt.is_none()).map(|(e, _)| e.emitter.as_str()).collect();
                lines.push(format!("  particles: {} emitters, {} without their .emt {}", list.len(), lacking.len(), lacking.join(", ")));
            }
            Err(e) => lines.push(format!("  particles: {e}")),
        }
        lines.extend(self.todo.iter().map(|t| format!("  todo: {t}")));
        lines.push("  ridden: not yet (the quad's step is MD37b, its pick MD37d)".into());
        lines
    }
}
