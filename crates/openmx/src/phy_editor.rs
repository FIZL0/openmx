//! **The physics editor** (P387, after MXA's Physics Studio's knob groups):
//! the bike's `.phy` edited while it is ridden, a knob at a time, and kept as a handling mod.
//!
//! A knob is a scale on a group of the file's own numbers (Engine Power is every `HP<n>` of
//! `[Engine]`, Brakes every `MaxBrakingTorque<n>` of both tires), stepped in percent. The editor
//! never touches the simulation's numbers: it writes the scaled values back into the file's text and
//! the ported loader ([`Phy::read`]) reads that, so an edited bike is one the original's reader would
//! build off the same file. All knobs at 100% is the disc's file, read as it always is.
//!
//! Saved, the edited file goes to `mods/handling/<id>/` beside a `mod.json` of kind `handling`
//! (which names the bike file and the knobs), a pack as every other mod is (P318), which the lobby
//! lists and hashes like any. An online ride takes no edits ([`held`]), but rides a saved one where
//! every seat holds it (P387a): the lobby pick carries `handling/<id>` ([`picked`]), the handshake
//! refuses a seat without the same folder, and every machine builds that seat's bike from the mod's
//! knobs over its own disc file ([`seat_phy`]).

use game_sim::camera::CurveTable;
use game_sim::phy::{Ini, Phy};
use std::path::{Path, PathBuf};
use stitch::json::Json;

/// The folder under `mods/` handling mods live in.
pub const HANDLING_DIR: &str = "handling";
/// What a saved mod's `mod.json` says its kind is.
pub const KIND: &str = "handling";

/// A knob's step, and how far it goes either way, in percent of the disc's value.
pub const STEP: u16 = 5;
pub const MIN: u16 = 10;
pub const MAX: u16 = 400;
/// A knob left as the disc has it.
pub const STOCK: u16 = 100;

/// One knob: a scale on every value of the file it names.
pub struct Knob {
    /// The heading it is under.
    pub group: &'static str,
    pub label: &'static str,
    pub about: &'static str,
    /// `(section, key, numbered)`: a numbered key is the key followed by a number (`HP1`, `HP2`…),
    /// each one of the file's.
    pub keys: &'static [(&'static str, &'static str, bool)],
}

const FRONT: &str = "Front Tire";
const REAR: &str = "Rear Tire";
const MASS: &str = "Mass Properties";
const ENGINE: &str = "Engine";

/// Every knob, in the order the panel lists them, grouped as MXA's Physics Studio groups its own.
pub const KNOBS: [Knob; 24] = [
    Knob { group: "Tires", label: "Tire Grip", about: "How hard each tire holds the ground before it spins or slides: every surface's wheel friction.", keys: &[(FRONT, "WheelFriction", true), (REAR, "WheelFriction", true)] },
    Knob { group: "Tires", label: "Rolling Drag", about: "How much the tires drag as they roll.", keys: &[(FRONT, "WheelDrag", true), (REAR, "WheelDrag", true)] },
    Knob { group: "Tires", label: "Tire Size", about: "Both wheels' radius.", keys: &[(FRONT, "TireRadius", false), (REAR, "TireRadius", false)] },
    Knob { group: "Suspension", label: "Front Spring", about: "The fork's spring, riding and stopped.", keys: &[(FRONT, "Spring", false), (FRONT, "SpringWhenStopped", false)] },
    Knob { group: "Suspension", label: "Rear Spring", about: "The shock's spring, riding and stopped.", keys: &[(REAR, "Spring", false), (REAR, "SpringWhenStopped", false)] },
    Knob { group: "Suspension", label: "Damping", about: "How quickly both ends settle, compressing and rebounding.", keys: &[(FRONT, "BounceDampingRatio", false), (FRONT, "ReboundDampingRatio", false), (REAR, "BounceDampingRatio", false), (REAR, "ReboundDampingRatio", false)] },
    Knob { group: "Suspension", label: "Travel", about: "How far both ends compress.", keys: &[(FRONT, "StrutCompression", false), (REAR, "StrutCompression", false)] },
    Knob { group: "Crashes", label: "Crash Speed", about: "How fast a hit has to be to throw the rider off: front, back, side and coming down.", keys: &[("Wrecks", "FrontWreckSpeedLimit", false), ("Wrecks", "BackWreckSpeedLimit", false), ("Wrecks", "SideWreckSpeedLimit", false), ("Wrecks", "RacingDownWreckSpeedLimit", false), ("Wrecks", "FreestyleDownWreckSpeedLimit", false)] },
    Knob { group: "Crashes", label: "Crash Angles", about: "How far the bike may pitch and roll, on the ground and in the air, before the rider is thrown off.", keys: &[("Wrecks", "GroundPitchDownWreckAngle", false), ("Wrecks", "GroundPitchUpWreckAngle", false), ("Wrecks", "AirbornePitchDownWreckAngle", false), ("Wrecks", "AirbornePitchUpWreckAngle", false), ("Wrecks", "AirborneRollWreckAngle", false)] },
    Knob { group: "Engine", label: "Engine Power", about: "The horsepower at every point of the engine's curve.", keys: &[(ENGINE, "HP", true)] },
    Knob { group: "Engine", label: "Rev Range", about: "Every rev the engine's curve, idle and shift points name.", keys: &[(ENGINE, "RPM", true), (ENGINE, "MinRPM", false), (ENGINE, "UpShiftRPM", false), (ENGINE, "DownShiftRPM", false)] },
    Knob { group: "Engine", label: "Engine Braking", about: "How hard the engine slows the bike off the throttle.", keys: &[(ENGINE, "EngineBraking", false)] },
    Knob { group: "Engine", label: "Clutch", about: "How much the clutch grips, slipping and locked.", keys: &[(ENGINE, "ClutchStaticFriction", false), (ENGINE, "ClutchKineticFriction", false)] },
    Knob { group: "Drivetrain", label: "Final Drive", about: "The final gear ratio: higher pulls harder and tops out sooner.", keys: &[(ENGINE, "FinalGearRatio", false)] },
    Knob { group: "Drivetrain", label: "Gears", about: "Every gear's ratio.", keys: &[(ENGINE, "GearRatio", true)] },
    Knob { group: "Controls", label: "Throttle", about: "How quickly the throttle opens and how open it idles.", keys: &[("Joystick", "GasRateLimit", false), (ENGINE, "IdleThrottle", false)] },
    Knob { group: "Controls", label: "Steering Lock", about: "How far the bars turn.", keys: &[(FRONT, "MaxSteerAngle", false)] },
    Knob { group: "Controls", label: "Steering Response", about: "How quickly the bars follow the stick.", keys: &[(FRONT, "SteeringTorque", false), ("Joystick", "XAxisRateLimit", false)] },
    Knob { group: "Controls", label: "Lean Speed", about: "How quickly the bike leans into a turn.", keys: &[("Roll Control", "MaxRollRateCommand", false), ("Roll Control", "RollGain", false)] },
    Knob { group: "Body", label: "Gravity", about: "The bike's gravity, as a share of the world's.", keys: &[(MASS, "Gravity", false)] },
    Knob { group: "Body", label: "Weight", about: "The bike and rider's weight.", keys: &[(MASS, "Weight", false)] },
    Knob { group: "Body", label: "Inertia", about: "How hard the bike is to turn, pitch and roll.", keys: &[(MASS, "Inertia", false)] },
    Knob { group: "Body", label: "Air Drag", about: "How much the air holds the bike back.", keys: &[("Aerodynamics", "Drag", false)] },
    Knob { group: "Grip", label: "Traction", about: "How much drive and side force the tires take before they skid.", keys: &[(FRONT, "MaxDriveForce", true), (FRONT, "SkidDriveForce", true), (FRONT, "MaxSideForce", true), (FRONT, "SkidSideForce", true), (REAR, "MaxDriveForce", true), (REAR, "SkidDriveForce", true), (REAR, "MaxSideForce", true), (REAR, "SkidSideForce", true)] },
];

/// Brakes is its own knob after the rest, so the panel ends on it as MXA's list does.
pub const BRAKES: Knob = Knob { group: "Grip", label: "Brakes", about: "Each tire's braking torque.", keys: &[(FRONT, "MaxBrakingTorque", true), (REAR, "MaxBrakingTorque", true)] };

/// Every knob with Brakes last.
pub fn knobs() -> impl Iterator<Item = &'static Knob> {
    KNOBS.iter().chain(std::iter::once(&BRAKES))
}

/// How many knobs there are.
pub const COUNT: usize = KNOBS.len() + 1;

/// The knob at `i` of [`knobs`].
pub fn knob(i: usize) -> &'static Knob {
    KNOBS.get(i).unwrap_or(&BRAKES)
}

impl Knob {
    /// Whether the pair `section`/`key` is one of the knob's.
    pub fn takes(&self, section: &str, key: &str) -> bool {
        self.keys.iter().any(|(s, k, numbered)| {
            s.eq_ignore_ascii_case(section)
                && match numbered {
                    false => k.eq_ignore_ascii_case(key),
                    true => key.len() > k.len() && key.is_char_boundary(k.len()) && key[..k.len()].eq_ignore_ascii_case(k) && key[k.len()..].bytes().all(|b| b.is_ascii_digit()),
                }
        })
    }
}

/// Every knob's scale in percent, in [`knobs`]' order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edits(pub [u16; COUNT]);

impl Default for Edits {
    fn default() -> Edits {
        Edits([STOCK; COUNT])
    }
}

impl Edits {
    /// Whether every knob is as the disc has it.
    pub fn is_stock(&self) -> bool {
        self.0.iter().all(|&p| p == STOCK)
    }

    /// Knob `i` a step up (`up`) or down, held between [`MIN`] and [`MAX`].
    pub fn step(&mut self, i: usize, up: bool) {
        if let Some(p) = self.0.get_mut(i) {
            *p = if up { (*p + STEP).min(MAX) } else { p.saturating_sub(STEP).max(MIN) };
        }
    }

    /// How many knobs are off the disc's.
    pub fn changed(&self) -> usize {
        self.0.iter().filter(|&&p| p != STOCK).count()
    }
}

/// A value's text with every number in it scaled by `scale`: each comma-separated part's leading
/// number, the rest of the part kept (`0.0, 1.6, 0.0` is a vector, scaled a part at a time).
pub fn scaled(value: &str, scale: f64) -> String {
    value.split(',').map(|part| scaled_part(part, scale)).collect::<Vec<_>>().join(",")
}

fn scaled_part(part: &str, scale: f64) -> String {
    let b = part.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let digits = i;
    while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
        i += 1;
    }
    if i < b.len() && b[i] | 32 == b'e' {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        let from = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > from {
            i = j;
        }
    }
    let Ok(v) = part[..i].parse::<f64>() else { return part.to_string() };
    if i == digits {
        return part.to_string();
    }
    // six significant figures: the file's own numbers have fewer, and the loader reads a decimal
    format!("{}{}", trimmed(v * scale), &part[i..])
}

/// `v` to six significant figures, with no exponent and no trailing zeros.
fn trimmed(v: f64) -> String {
    if v == 0.0 || !v.is_finite() {
        return "0".into();
    }
    let places = (5 - v.abs().log10().floor() as i32).clamp(0, 15) as usize;
    let s = format!("{v:.places$}");
    match s.contains('.') {
        true => s.trim_end_matches('0').trim_end_matches('.').to_string(),
        false => s,
    }
}

/// A bike's `.phy` being edited: the disc's file and the knobs over it.
#[derive(Clone, Debug)]
pub struct Editor {
    /// The bike file's name as its archive has it (`Bike\Race125P.phy`).
    pub member: String,
    base: Ini,
    pub edits: Edits,
}

impl Editor {
    /// The file `member`'s `bytes` with every knob at the disc's.
    pub fn new(member: &str, bytes: &[u8]) -> Editor {
        Editor { member: member.to_string(), base: Ini::parse_bytes(bytes), edits: Edits::default() }
    }

    /// The file with the knobs on it.
    pub fn ini(&self) -> Ini {
        if self.edits.is_stock() {
            return self.base.clone();
        }
        self.base.with_values(|section, key, value| {
            let (i, _) = knobs().enumerate().find(|(_, k)| k.takes(section, key))?;
            let p = self.edits.0[i];
            (p != STOCK).then(|| scaled(value, f64::from(p) / f64::from(STOCK)))
        })
    }

    /// The bike the edited file builds, through the ported loader.
    pub fn phy(&self, curve: &CurveTable) -> Phy {
        Phy::read(&self.ini(), curve)
    }

    /// Whether this bike's file has any of knob `i`'s values; one with none changes nothing.
    pub fn has(&self, i: usize) -> bool {
        let k = knob(i);
        self.base.entries().any(|(s, key, _)| k.takes(s, key))
    }

    /// Knob `i`'s first value as the edited file has it (`26 hp` is the panel's `26`), for the panel.
    pub fn value(&self, i: usize) -> Option<String> {
        let k = knob(i);
        let ini = self.ini();
        ini.entries().find(|(s, key, _)| k.takes(s, key)).map(|(_, _, v)| v.to_string())
    }

    /// The bike file's own name (`race125p.phy`), what the saved mod keeps it as.
    pub fn file_name(&self) -> String {
        file_leaf(&self.member)
    }

    /// Keep the edited file as the handling mod `name` in `mods/handling/` (P387): a folder of its
    /// id with the `.phy` and a `mod.json` naming it and the knobs. A mod of that id already there
    /// is written over. The folder, or why it could not be.
    pub fn save(&self, mods: &Path, name: &str) -> Result<PathBuf, String> {
        let id = mod_id(name);
        let folder = mods.join(HANDLING_DIR).join(&id);
        crate::files::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        let file = self.file_name();
        let path = folder.join(&file);
        crate::files::write(&path, self.ini().text()).map_err(|e| format!("{}: {e}", path.display()))?;
        let knobs: Vec<String> = knobs().zip(self.edits.0).filter(|(_, p)| *p != STOCK).map(|(k, p)| format!("{}: {p}", quote(k.label))).collect();
        let json = format!(
            "{{\n  \"standard\": 1,\n  \"kind\": {},\n  \"id\": {},\n  \"name\": {},\n  \"version\": \"1.0\",\n  \"author\": \"openmx physics editor\",\n  \"bike\": {},\n  \"knobs\": {{{}}}\n}}\n",
            quote(KIND),
            quote(&id),
            quote(name),
            quote(&file),
            knobs.join(", ")
        );
        let manifest = folder.join("mod.json");
        crate::files::write(&manifest, json).map_err(|e| format!("{}: {e}", manifest.display()))?;
        Ok(folder)
    }

    /// The knobs a saved mod set, over this editor's file (its `mod.json`'s `knobs`).
    pub fn take(&mut self, saved: &Saved) {
        self.edits = saved.edits.clone();
    }
}

/// A saved handling mod, read back.
#[derive(Clone, Debug)]
pub struct Saved {
    pub id: String,
    pub name: String,
    /// The bike file it edits (`race125p.phy`).
    pub bike: String,
    pub edits: Edits,
}

/// The handling mod in `folder`, or why it is not one.
pub fn load(folder: &Path) -> Result<Saved, String> {
    let manifest = folder.join("mod.json");
    let text = crate::files::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let json = Json::parse(&text).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let said = |key: &str| json.get(key).and_then(Json::str).map(str::to_string);
    if said("kind").as_deref() != Some(KIND) {
        return Err(format!("{}: not a handling mod", manifest.display()));
    }
    let bike = said("bike").filter(|b| !b.is_empty() && !b.contains(['/', '\\']) && b != "..").ok_or_else(|| format!("{}: no bike", manifest.display()))?;
    let mut edits = Edits::default();
    if let Some(Json::Obj(knobs)) = json.get("knobs") {
        for (label, v) in knobs {
            let Some(i) = self::knobs().position(|k| k.label == label) else { continue };
            if let Some(p) = v.f64() {
                edits.0[i] = (p.round() as i64).clamp(i64::from(MIN), i64::from(MAX)) as u16;
            }
        }
    }
    let leaf = folder.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let id = said("id").unwrap_or(leaf);
    Ok(Saved { name: said("name").unwrap_or_else(|| id.clone()), id, bike, edits })
}

/// Every handling mod in `mods/handling/` for the bike file `bike` (`race125p.phy`), by folder name.
pub fn listed(mods: &Path, bike: &str) -> Vec<Saved> {
    let dir = mods.join(HANDLING_DIR);
    let mut folders: Vec<PathBuf> = crate::files::list(&dir).unwrap_or_default().into_iter().filter(|e| e.dir).map(|e| e.path).collect();
    folders.sort();
    folders.iter().filter_map(|f| load(f).ok()).filter(|s| s.bike.eq_ignore_ascii_case(bike)).collect()
}

/// A mod's id off its name: lower case letters, digits and `_`, after `phy_`.
pub fn mod_id(name: &str) -> String {
    let mut id: String = name.trim().chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
    id.truncate(32);
    let id = id.trim_matches('_');
    format!("phy_{}", if id.is_empty() { "custom" } else { id })
}

/// Why the editor can't turn a ride's bike, or none (P387). An online ride is refused an edited bike:
/// every machine rides the files the room's handshake held each seat to, and an edit is in none of
/// them. A tape rides the file it was written on. And `stock`, the bike as the ride built it before
/// its modifiers, has to be the disc's `bytes` read as they are: MX vs. ATV's handling or a trick
/// pack laid over it is no file the knobs edit.
pub fn held(online: bool, tape: bool, stock: &Phy, bytes: &[u8], curve: &CurveTable) -> Option<&'static str> {
    if online {
        return Some("Online rides keep the bike's own file: an edited bike can't ride online.");
    }
    if tape {
        return Some("A tape rides the bike's own file.");
    }
    (format!("{stock:?}") != format!("{:?}", Phy::read_bytes(bytes, curve))).then_some("Only on the original's handling, with no trick pack laid over the bike.")
}

/// A bike file's own name off its member (`Bike\Race125P.phy` is `race125p.phy`), as a saved mod
/// names the file it edits.
pub fn file_leaf(member: &str) -> String {
    member.rsplit(['\\', '/']).next().unwrap_or(member).to_ascii_lowercase()
}

/// The handling mod a lobby pick's packs (`machine/mxvatv_atv,handling/phy_loose`) ride (P387a):
/// the id under the first `handling/` key, none without one.
pub fn picked(packs: &str) -> Option<&str> {
    packs.split(',').map(str::trim).find_map(|k| k.strip_prefix(KIND)?.strip_prefix('/')).filter(|id| !id.is_empty())
}

/// The saved mod in `mods` for the bike file `bike` whose knobs are `edits` (P387a): the one a
/// player whose editor is on them rides into a lobby. None where the knobs are the disc's, or no
/// saved mod has them: an edited bike nobody else can hold rides the disc's file online.
pub fn matching(mods: &Path, bike: &str, edits: &Edits) -> Option<Saved> {
    if edits.is_stock() {
        return None;
    }
    listed(mods, bike).into_iter().find(|s| s.edits == *edits)
}

/// A lobby seat's bike on the handling mod its pick's `packs` carry (P387a): the mod's knobs over
/// this machine's own disc file `member`'s `bytes`, which the handshake held every seat to as it
/// held them to the mod's folder, so every machine builds the same bike. `None` without a mod in
/// the pick; an error where `mods` lacks it or it edits another bike's file.
pub fn seat_phy(mods: Option<&Path>, packs: &str, member: &str, bytes: &[u8], curve: &CurveTable) -> Result<Option<Phy>, String> {
    let Some(id) = picked(packs) else { return Ok(None) };
    let mods = mods.ok_or_else(|| format!("a rider is on the handling mod {id}, and the mods folder is off"))?;
    let saved = load(&mods.join(HANDLING_DIR).join(id))?;
    let mut editor = Editor::new(member, bytes);
    if !saved.bike.eq_ignore_ascii_case(&editor.file_name()) {
        return Err(format!("the handling mod {id} edits {}, not {}", saved.bike, editor.file_name()));
    }
    editor.take(&saved);
    Ok(Some(editor.phy(curve)))
}

/// `s` as a JSON string.
fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
