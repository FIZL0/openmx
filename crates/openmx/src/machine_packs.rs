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

use crate::audio::{BikeSounds, Clip as Sound, HeliEngine, LoopEngine, MachineTouchdown, MachineWreck};
use crate::particles::Emt;
use crate::track_mods::Json;
use game_render::biplane::{self, Emitter};
use game_render::kiowa;
use game_render::Texture;
use game_render::scene::Material;
use game_render::world::texture_files;
use game_sim::phy::Ini as PhyIni;
use game_sim::track::{Mesh, Rays};
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
    /// `skins` (MD80d): a flier pack's skins, each with its models and its collision pair. Empty
    /// for a pack of builds.
    pub skins: Vec<Skin>,
    /// `fliers`: each flier's own files.
    pub fliers: Vec<Flier>,
    helicopter_engine: String,
    machine_sounds: String,
}

/// One skin of a flier, as `mod.json`'s `skins` lists it (openmx-mods' `import-atv fliers`).
#[derive(Clone, Debug)]
pub struct Skin {
    /// `biplane` or `kiowa`.
    pub machine: String,
    /// The skin's model name: `biplane`, `biplane2`, … `kiowa4`.
    pub skin: String,
    /// 1 or 2: which of the `.phy`'s `ModelOne`/`ModelTwo` keys it flies on.
    pub model: u32,
    pub psg: String,
    /// The garage's model of it, `<skin>_ui.psg`.
    pub ui: String,
    /// `<stem>F.col`, the hull the world meets, and `<stem>M.col`, the ray probe.
    pub hull: String,
    pub rays: String,
    /// The garage set its `_ui` model and textures came out of (`UI/Res/Veh_BiPlane1.res`): where
    /// they came from, not a file in the pack.
    pub garage: String,
}

/// One flier's files, `mod.json`'s `fliers.<machine>`.
#[derive(Clone, Debug)]
pub struct Flier {
    pub machine: String,
    pub phy: String,
    /// The follow cameras (`FC_*Default.pb`) and the replay's (`RC_*Default.pb`).
    pub cameras: String,
    pub replay_cameras: String,
    pub particles: String,
    /// The prefix of its sections in the `.aud` tables: `biplane`, `helicopter`.
    pub sounds: String,
}

/// A flier's sounds, each through the reader openmx plays MX Unleashed's with (V114, P146, P164).
pub struct FlierSounds {
    /// The plane's keyed loops, `carengine.aud`'s `[<prefix>]` (empty for the helicopter).
    pub loops: LoopEngine,
    /// The helicopter's one loop, `otherengine.aud`'s `[Helicopter]` (none for the plane).
    pub rotor: HeliEngine,
    /// `machine.aud`'s `[<prefix>Wreck]`.
    pub wreck: MachineWreck,
    /// `machine.aud`'s `[<prefix>TireTouchdown]` (the helicopter has none).
    pub touchdown: MachineTouchdown,
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
    let mut skins = Vec::new();
    if let Some(Json::Arr(list)) = json.get("skins") {
        for k in list {
            let model = match k.get("model") {
                Some(Json::Num(n)) => n.parse().unwrap_or(1),
                _ => 1,
            };
            skins.push(Skin {
                machine: s(k.get("machine")),
                skin: s(k.get("skin")),
                model,
                psg: s(k.get("psg")),
                ui: s(k.get("ui")),
                hull: s(k.get("hull")),
                rays: s(k.get("rays")),
                garage: s(k.get("garage")),
            });
        }
    }
    let mut fliers = Vec::new();
    if let Some(Json::Obj(list)) = json.get("fliers") {
        for (machine, f) in list {
            fliers.push(Flier {
                machine: machine.clone(),
                phy: s(f.get("phy")),
                cameras: s(f.get("cameras")),
                replay_cameras: s(f.get("replay_cameras")),
                particles: s(f.get("particles")),
                sounds: s(f.get("sounds")),
            });
        }
    }
    if builds.is_empty() && skins.is_empty() {
        return Err("mod.json lists neither builds nor skins".into());
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
        helicopter_engine: s(json.get("sounds").and_then(|j| j.get("helicopter"))),
        machine_sounds: s(json.get("sounds").and_then(|j| j.get("machine"))),
        todo,
        skins,
        fliers,
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

    /// A flier skin's model and its garage `_ui` model.
    pub fn skin_models(&self, skin: &Skin) -> Result<(Model, Model), String> {
        let read = |name: &str| self.files.read(&member(name)).and_then(|b| Model::parse(&b).map_err(|e| format!("{name}: {e}")));
        Ok((read(&skin.psg)?, read(&skin.ui)?))
    }

    /// The nodes openmx moves or seats the pilot on in a flier (`biplane::PROP_NODE`, the Kiowa's
    /// rotor nodes, and the four seat dummies), each with the name the skin's model spells it with:
    /// `kiowa3`/`kiowa4` spell three of the seat's with a `01` on the end (`Dummy_Foot_R_pos01`).
    pub fn skin_nodes(&self, skin: &Skin) -> Result<Vec<(&'static str, Option<String>)>, String> {
        let (model, _) = self.skin_models(skin)?;
        let mut wanted = match skin.machine.as_str() {
            "kiowa" => {
                let mut v = vec![kiowa::SPINDLE, kiowa::TAIL];
                v.extend(kiowa::BLADES);
                v.extend(kiowa::SEAT);
                v
            }
            _ => {
                let mut v = vec![biplane::PROP_NODE];
                v.extend(biplane::SEAT);
                v
            }
        };
        wanted.dedup();
        let spelt = |name: &str| model.nodes.iter().find(|n| n.name.eq_ignore_ascii_case(name)).map(|n| n.name.clone());
        Ok(wanted.into_iter().map(|w| (w, spelt(w).or_else(|| spelt(&format!("{w}01"))))).collect())
    }

    /// A flier skin's collision pair: the hull the world meets and the ray probe, through the readers
    /// the biplane's and the Kiowa's own are read with.
    pub fn skin_collision(&self, skin: &Skin) -> Result<(Mesh, Rays), String> {
        let hull = self.files.read(&member(&skin.hull)).and_then(|b| Mesh::parse(&b)).map_err(|e| format!("{}: {e}", skin.hull))?;
        let rays = self.files.read(&member(&skin.rays)).and_then(|b| Rays::parse(&b)).map_err(|e| format!("{}: {e}", skin.rays))?;
        Ok((hull, rays))
    }

    /// Every material a skin's two models name, and each one's own texture, read: how many textures
    /// read, and a line for each material or texture the pack lacks or openmx refuses.
    pub fn skin_textures(&self, skin: &Skin) -> Result<(usize, Vec<String>), String> {
        let (model, ui) = self.skin_models(skin)?;
        let mut names: Vec<String> = model.materials.iter().chain(&ui.materials).map(|m| m.to_ascii_lowercase()).collect();
        names.sort();
        names.dedup();
        let (mut read, mut why) = (0, Vec::new());
        for m in &names {
            let text = match self.files.read_suffix(&format!("\\{m}.psm")) {
                Ok(b) => ini::text(&b),
                Err(e) => {
                    why.push(format!("{m}.psm: {e}"));
                    continue;
                }
            };
            let files = texture_files(m, &Material::parse(&text));
            if files.is_empty() {
                continue;
            }
            match files.iter().find_map(|f| self.files.read_suffix(&format!("\\{f}")).ok().map(|b| (f, b))) {
                Some((f, b)) => match Texture::parse(&b) {
                    Ok(_) => read += 1,
                    Err(e) => why.push(format!("{m}.psm: {f}: {e}")),
                },
                None => why.push(format!("{m}.psm: none of {}", files.join(", "))),
            }
        }
        Ok((read, why))
    }

    /// A flier's `.phy`, and how many keys it gives each of its two models (`ModelOne<Key>`,
    /// `ModelTwo<Key>`): skins 1 and 2 fly on the first, 3 and 4 on the second.
    pub fn flier_phy(&self, flier: &Flier) -> Result<(PhyIni, [usize; 2]), String> {
        let ini = PhyIni::parse_bytes(&self.files.read(&member(&flier.phy))?);
        let mut keys = [0, 0];
        for (_, k, _) in ini.entries() {
            if k.starts_with("ModelOne") {
                keys[0] += 1;
            } else if k.starts_with("ModelTwo") {
                keys[1] += 1;
            }
        }
        Ok((ini, keys))
    }

    /// A flier's follow and replay cameras of one screen layout.
    pub fn flier_cameras(&self, flier: &Flier, view: &str) -> Result<(Vec<Preset>, Vec<Preset>), String> {
        Ok((Preset::parse_follow(&self.text(&flier.cameras)?, view), Preset::parse_follow(&self.text(&flier.replay_cameras)?, view)))
    }

    /// A flier's sounds, with each `.vag` read from the pack's `Aud\` as [`Pack::engine_sounds`] does.
    pub fn flier_sounds(&self, flier: &Flier) -> Result<(FlierSounds, Vec<String>), String> {
        let engine = self.text(&self.engine)?;
        let helicopter = self.text(&self.helicopter_engine)?;
        let machine = self.text(&self.machine_sounds)?;
        let mut loaded: HashMap<String, Option<Arc<Sound>>> = HashMap::new();
        let mut why = Vec::new();
        let mut lib = |name: &str| -> Option<Arc<Sound>> {
            let mut path = format!("Aud\\{}", name.trim().trim_matches('"'));
            if !path.rsplit('\\').next().unwrap_or("").contains('.') {
                path = [".VAG", ".vig"].iter().map(|x| format!("{path}{x}")).find(|p| self.files.find_suffix(p).is_some()).unwrap_or(path);
            }
            loaded
                .entry(path.to_ascii_lowercase())
                .or_insert_with(|| match self.files.read_suffix(&path).and_then(|b| Sound::read(&path, &b)) {
                    Ok(c) => Some(Arc::new(c)),
                    Err(e) => {
                        why.push(format!("{path}: {e}"));
                        None
                    }
                })
                .clone()
        };
        let (loops, rotor) = match flier.machine.as_str() {
            "kiowa" => (LoopEngine::default(), HeliEngine::read(&helicopter, &mut lib)),
            _ => (LoopEngine::read(&engine, &flier.sounds, &mut lib), HeliEngine::default()),
        };
        let wreck = MachineWreck::read(&machine, &flier.sounds, &mut lib);
        let touchdown = MachineTouchdown::read(&machine, &flier.sounds, &mut lib);
        Ok((FlierSounds { loops, rotor, wreck, touchdown }, why))
    }

    /// A flier's particle list's emitters, each with its `.emt` read (`None` for one the pack lacks).
    pub fn flier_emitters(&self, flier: &Flier) -> Result<Vec<(Emitter, Option<Emt>)>, String> {
        let list = biplane::emitters(&self.text(&flier.particles)?);
        Ok(list
            .into_iter()
            .map(|e| {
                let emt = self.files.read_suffix(&e.emitter).ok().map(|b| Emt::parse(&ini::text(&b)));
                (e, emt)
            })
            .collect())
    }

    /// The flier half of [`Pack::report`]: a line a skin and a few a flier.
    fn flier_report(&self, lines: &mut Vec<String>) {
        for k in &self.skins {
            let nodes = match self.skin_nodes(k) {
                Ok(n) => {
                    let lacking: Vec<&str> = n.iter().filter(|(_, s)| s.is_none()).map(|(w, _)| *w).collect();
                    let other: Vec<String> = n.iter().filter_map(|(w, s)| s.as_ref().filter(|s| *s != w).map(|s| format!("{w} as {s}"))).collect();
                    match (lacking.is_empty(), other.is_empty()) {
                        (true, true) => "every node".to_owned(),
                        (true, false) => format!("every node, {}", other.join(", ")),
                        _ => format!("lacks {}", lacking.join(", ")),
                    }
                }
                Err(e) => e,
            };
            let col = match self.skin_collision(k) {
                Ok((hull, rays)) => format!("{} hull triangles, {} rays", hull.triangles.len(), rays.segments.len()),
                Err(e) => e,
            };
            let tex = match self.skin_textures(k) {
                Ok((n, why)) => {
                    let mut t = format!("{n} textures");
                    if !why.is_empty() {
                        t += &format!(", {} not: {}", why.len(), why.join("; "));
                    }
                    t
                }
                Err(e) => e,
            };
            lines.push(format!("  {} (model {}, {}, garage {}): {nodes}; {col}; {tex}", k.skin, k.model, k.machine, k.garage));
        }
        for f in &self.fliers {
            match self.flier_phy(f) {
                Ok((_, [one, two])) => lines.push(format!("  {} {}: {one} ModelOne keys, {two} ModelTwo", f.machine, f.phy)),
                Err(e) => lines.push(format!("  {} phy: {e}", f.machine)),
            }
            match self.flier_cameras(f, "4x3/1p") {
                Ok((c, r)) => lines.push(format!("  {} cameras: {} follow, {} replay (4x3/1p)", f.machine, c.len(), r.len())),
                Err(e) => lines.push(format!("  {} cameras: {e}", f.machine)),
            }
            match self.flier_sounds(f) {
                Ok((s, why)) => {
                    lines.push(format!(
                        "  {} sounds: {} loops, rotor {}, {} wreck, {} touchdown, {} not read",
                        f.machine,
                        s.loops.loops.len(),
                        if s.rotor.clip.is_some() { "read" } else { "none" },
                        s.wreck.list.sounds.len(),
                        s.touchdown.list.sounds.len(),
                        why.len()
                    ));
                    lines.extend(why.iter().map(|w| format!("    {w}")));
                }
                Err(e) => lines.push(format!("  {} sounds: {e}", f.machine)),
            }
            match self.flier_emitters(f) {
                Ok(list) => {
                    let lacking: Vec<&str> = list.iter().filter(|(_, emt)| emt.is_none()).map(|(e, _)| e.emitter.as_str()).collect();
                    lines.push(format!("  {} particles: {} emitters, {} without their .emt {}", f.machine, list.len(), lacking.len(), lacking.join(", ")));
                }
                Err(e) => lines.push(format!("  {} particles: {e}", f.machine)),
            }
        }
        lines.extend(self.todo.iter().map(|t| format!("  todo: {t}")));
        lines.push("  flown: not yet (the fliers' MX vs. ATV steps are MD80b/MD80c, their pick MD80e)".into());
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
        if self.builds.is_empty() {
            lines[0] += &format!(", {} skins", self.skins.len());
            self.flier_report(&mut lines);
            return lines;
        }
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
