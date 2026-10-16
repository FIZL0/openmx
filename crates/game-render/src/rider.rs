//! The rider: the one skinned body on the disc, the gear it is painted with, and how it bolts to the
//! bike.
//!
//! A bike is a kit — a shared chassis with a `.sub` that bolts parts onto its nodes and repaints it
//! (see [`crate::scene::Vehicle`]). A rider is the same idea with the parts left out: one body mesh
//! shared by all 162 rider archives, a `.sub` that repaints it in that rider's own gear, and a
//! skeleton the motions move. Reading one takes three files, each naming the next:
//!
//! ```text
//!   Rider\thq1.res        the archive, named after the rider
//!     rider\anim\thq1.sub         [Rider] MCF = rider.mcf        -> the motion set
//!                                 [Base Materials] / [Material Changes]
//!     rider\anim\mrider.mcf       SLTFile = mrider.Slt           -> the geometry
//!                                 VUTfilename, RotationType, 40 x [MOTION n]
//!     rider\anim\mrider.psg       22 nodes, 30 meshes, 5 LOD groups
//! ```
//!
//! Three things that chain teaches, all read off the disc rather than out of the loader:
//!
//! * **The `m` in `mrider` is the body's sex.** The `.sub` says `MCF = rider.mcf` and its own comment
//!   says the loader puts `m` or `f` in front of it. Every rider archive ships `mrider.mcf` and none
//!   ships an `frider.mcf`, so only the male body is on this disc; the AI rider's `.sub` names
//!   `airider.mcf` outright, which the prefix would spoil, so [`Rider::load`] tries the prefixed name
//!   and falls back on the name as written. That is what makes both of an archive's two `.sub`s work.
//! * **`SLT` is the geometry.** A scene says `SLT=bale_single_02` for `bale_single_02.psg`, and an
//!   `.mcf` says `SLTFile=mrider.Slt` for the same thing: the extension in the file is the editors'
//!   name for the format, the archive member is a `.psg`.
//! * **The gear is a repaint.** The body's meshes name `THQ_Helmet_01`, `THQ_Pants_01` and four more —
//!   the `[Base Materials]` list — and a licensed rider's `[Material Changes]` turns them into
//!   `Thor_Helmet_01`, `Alpinestars_Boots_01` and so on, textures its own archive carries. Exactly the
//!   mechanism that paints the CR 250's chassis in another bike's colours, so [`Assets`] needs no new
//!   code: hand it [`Suit::materials`] before loading the body.
//!
//! [`Assets`]: crate::world::Assets
//!
//! ## The rest pose, and bolting it on
//!
//! The `.psg`'s own node tree is a bind pose: every one of the 22 nodes has an identity rotation, so
//! the skeleton is nothing but bone offsets and the body stands in a T-pose, arms straight out. The
//! vertices are in bone space (the node index is in the position's `w`, as for any `.psg`), so
//! [`crate::psg::Model::triangles`] already draws that pose correctly with no new maths. What the
//! files do *not* say is where the pose sits on the bike: the rider's root is its pelvis, at the model
//! origin, and the game's motions carry the root's position per frame — a rider is never drawn in its
//! bind pose, which is why this is the one placement here that is a choice and not a fact.
//!
//! The choice has a hook in the data. Four of the rider's nodes have no geometry and exist only to be
//! aimed at — `Dummy_hand_L`, `Dummy_hand_R`, `Dummy_foot_L`, `Dummy_foot_R` — and the bike's chassis
//! has four nodes with the same four names, at its grips and its foot pegs. Those are the pairs the
//! game's own inverse kinematics would close, so [`bolt_onto`] lines up a chosen pair: the translation
//! that puts the mean of the rider's named anchors on the mean of the chassis's. Taking the feet
//! ([`FOOT_ANCHORS`]) stands the rider on the pegs, which is where a rider stands and the only pair
//! the bind pose's straight legs suit; taking the grips would hang the body from the bars with its
//! feet through the engine. The motions replace the whole question with their own root channel.
//!
//! And they do, which is now measured rather than expected: every clip in a rider's set writes the
//! pelvis's position, and the pose that comes out stands on the bike by itself. Posed by a clip the
//! rider rides in — the turns, the three airborne poses at the midpoint the grounded idle plays
//! them at, `backflip` — the body's four dummy anchors land within a third of an inch of the
//! chassis's four, feet on the pegs and hands on the grips, against five inches out in the bind
//! pose. So [`Body`], the posed rider the shell draws, is drawn at the bike's own transform and
//! bolts nothing. [`bolt_onto`] stays for the one case it was written for: a body with no clip to
//! pose it.
//!
//! A gesture is the exception that proves it: `hold_knee_left` takes a foot four inches off its peg
//! and `wreck_sep_over_bars` throws the whole body over the bars. That is the stunt being done, and
//! it is why where the rider is cannot be a placement this module computes.

use crate::anim::{Rig, Set};
use crate::disc::Res;
use crate::ini;
use crate::mat::{M4, v3};
use crate::skin::skin;
use crate::world::Loaded;
use game_sim::motion::{Animations, Library, Params, Played};
use game_sim::trick::{SLOTS, Stunt};
use ps2_float::F;

/// Which body a `.sub`'s `MCF = rider.mcf` means. Only [`Sex::Male`] is on this disc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sex {
    Male,
    Female,
}

impl Sex {
    /// The letter the loader puts in front of the `.mcf` the `.sub` names.
    pub fn prefix(self) -> &'static str {
        match self {
            Sex::Male => "m",
            Sex::Female => "f",
        }
    }
}

/// A parsed rider `.sub`: which motion set the rider uses, and what its gear is painted with.
#[derive(Clone, Debug, Default)]
pub struct Suit {
    /// The `.mcf` as the file writes it, before the sex prefix: `rider.mcf`.
    pub mcf: String,
    /// `Bike=RM250.sub` — a licensed rider's own bike. Commented out in all 324 `.sub`s on this disc,
    /// so it is always `None` here; kept because the comment above it says what it would mean.
    pub bike: Option<String>,
    /// The materials the body's own meshes name, in the order the file lists them.
    pub base_materials: Vec<String>,
    /// `from` -> `to`, as `[Material Changes]` lists them: the repaint that turns the shared body into
    /// this rider's gear. Hand it to [`Assets::rename_materials`] before loading the geometry.
    ///
    /// [`Assets::rename_materials`]: crate::world::Assets::rename_materials
    pub materials: Vec<(String, String)>,
}

impl Suit {
    pub fn parse(text: &str) -> Suit {
        let mut suit = Suit::default();
        for section in &ini::parse(text) {
            let lower = section.name.to_ascii_lowercase();
            if lower == "rider" || lower == "objects" {
                if let Some(mcf) = section.get("MCF") {
                    suit.mcf = mcf.to_string();
                }
                suit.bike = section.get("Bike").map(str::to_string);
            } else if lower == "base materials" {
                // Every line is `Material=<name>`, so the section is read as a list.
                for (key, value) in &section.keys {
                    if key.eq_ignore_ascii_case("material") {
                        suit.base_materials.push(value.clone());
                    }
                }
            } else if lower == "material changes" {
                // Here the key is the name a mesh asks for, so the section is pairs, not a lookup.
                for (from, to) in &section.keys {
                    suit.materials.push((from.clone(), to.clone()));
                }
            }
        }
        suit
    }

    /// The `.mcf` name to look for, and then the one to fall back on: the sex-prefixed name, and
    /// the name as the `.sub` writes it. The AI rider's `airider.mcf` needs the second.
    pub fn mcf_names(&self, sex: Sex) -> [String; 2] {
        [
            format!("{}{}", sex.prefix(), self.mcf),
            self.mcf.clone(),
        ]
    }
}

/// One entry of a motion set: the name the game looks a clip up by, and the clip it plays.
#[derive(Clone, Debug, Default)]
pub struct Motion {
    /// `NAME=turn_stop_Fwd` — the string the sim's motion driver builds when it asks for a clip.
    pub name: String,
    /// `VUE=turn_stop_Fwd.MOT` — the archive member. Several names can share one clip.
    pub clip: String,
    /// `Looping=yes`.
    pub looping: bool,
}

/// A parsed `.mcf`: which geometry a body is, which microprograms draw it, and its clips.
#[derive(Clone, Debug, Default)]
pub struct MotionSet {
    /// `ContentDirectory=Rider\Anim\`, the directory inside the archive the clips live in.
    pub directory: String,
    /// `SLTFile=mrider.Slt` as written; [`MotionSet::geometry`] turns it into a model name.
    pub slt: String,
    /// `VUTfilename=mrider.Vut`, the VU1 microprogram table. Not read by this crate.
    pub vut: String,
    /// `RotationType=SHORTAXIS`, how a clip's rotation channels are packed. The motions' business;
    /// every rider's `.mcf` on this disc says `SHORTAXIS` and no bike's `.mcf` says anything.
    pub rotations: Option<String>,
    /// The clips the loader has, which is the first `NumberOfMotions` of them.
    pub motions: Vec<Motion>,
    /// The `[MOTION n]` sections past `NumberOfMotions`: written, never loaded. The rider's `.mcf`
    /// has three of them — `Turn_fast_fwd`, `_mid`, `_bck`, appended under a heading of their own
    /// without the count at the top being raised — and the clips they name are not in any archive on
    /// the disc, which is what says the count and not the section list is what the loader walks.
    pub unlisted: Vec<Motion>,
}

impl MotionSet {
    pub fn parse(text: &str) -> MotionSet {
        let mut set = MotionSet::default();
        let mut stated = None;
        for section in &ini::parse(text) {
            let lower = section.name.to_ascii_lowercase();
            if lower == "general info" {
                set.directory = section.get("ContentDirectory").unwrap_or("").to_string();
                set.slt = section.get("SLTFile").unwrap_or("").to_string();
                set.vut = section.get("VUTfilename").unwrap_or("").to_string();
                set.rotations = section.get("RotationType").map(str::to_string);
                stated = section.get("NumberOfMotions").and_then(|v| v.trim().parse().ok());
            } else if lower.starts_with("motion") {
                let Some(name) = section.get("NAME") else { continue };
                set.motions.push(Motion {
                    name: name.to_string(),
                    clip: section.get("VUE").unwrap_or("").to_string(),
                    looping: section
                        .get("Looping")
                        .is_some_and(|v| v.eq_ignore_ascii_case("yes")),
                });
            }
        }
        // The count at the top wins over the sections below it: see `unlisted`.
        if let Some(n) = stated.filter(|&n: &usize| n < set.motions.len()) {
            set.unlisted = set.motions.split_off(n);
        }
        set
    }

    /// The model name [`Assets::load`] wants: `SLTFile=mrider.Slt` is the member `mrider.psg`.
    ///
    /// [`Assets::load`]: crate::world::Assets::load
    pub fn geometry(&self) -> String {
        match self.slt.rsplit_once('.') {
            Some((stem, _)) => stem.to_string(),
            None => self.slt.clone(),
        }
    }

    /// The clip a motion name plays, matched the way the game's own lookup is spelled — the names in
    /// the `.mcf` are mixed case and nothing else agrees with them.
    pub fn clip(&self, name: &str) -> Option<&str> {
        self.motions
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(name))
            .map(|m| m.clip.as_str())
    }
}

// The skeleton this module used to declare is now `skin::Skeleton`: the clip side needs the same
// tree to hang its channels on, and two of them meant a clip bound to one could not pose the other.
// It keeps the name it landed with here, so nothing that reads a rider's skeleton has to change.
pub use crate::skin::{Bone, Skeleton};

/// The rider's foot anchors, which the chassis spells `Dummy_Foot_L` / `Dummy_Foot_R` at its pegs.
pub const FOOT_ANCHORS: [&str; 2] = ["Dummy_foot_L", "Dummy_foot_R"];

/// The rider's hand anchors, which the chassis has at its grips.
pub const HAND_ANCHORS: [&str; 2] = ["Dummy_hand_L", "Dummy_hand_R"];

/// Rider space to chassis space: the translation that puts the mean of the rider's named anchors on
/// the mean of the chassis's bones of the same names.
///
/// A translation and no rotation, because this is the bind pose: every bone of it has an identity
/// rotation, so there is no orientation in the rider to line up with — see the module's note on why
/// the placement of a bind pose is a choice. Compose it with the bike's own placement the way a
/// `.sub`'s part is composed, `bolt_onto(..).mul(bike_at)`.
///
/// `None` if either skeleton is missing one of the anchors.
pub fn bolt_onto(rider: &Skeleton, chassis: &Skeleton, anchors: &[&str]) -> Option<M4> {
    let mut offset = v3(0.0, 0.0, 0.0);
    for name in anchors {
        offset = offset.add(chassis.at(name)?).sub(rider.at(name)?);
    }
    if anchors.is_empty() {
        return None;
    }
    let offset = offset.scale(1.0 / anchors.len() as f32);
    let mut m = M4::IDENTITY;
    m.0[3] = [offset.x, offset.y, offset.z, 1.0];
    Some(m)
}

/// One rider, as the three files name each other: the archive's `.sub`, the `.mcf` it points at, and
/// the name of the geometry that `.mcf` points at.
///
/// The geometry itself is deliberately not loaded here. [`Assets`] has to be told the repaint before
/// it loads a model, and it caches by name, so the caller's order is: load this, hand
/// [`Suit::materials`] to [`Assets::rename_materials`], then [`Assets::load`] the [`Rider::geometry`]
/// name. [`Skeleton::of`] then reads the skeleton off the model [`Assets`] is holding.
///
/// [`Assets`]: crate::world::Assets
/// [`Assets::rename_materials`]: crate::world::Assets::rename_materials
/// [`Assets::load`]: crate::world::Assets::load
#[derive(Clone, Debug)]
pub struct Rider {
    /// The name the archive is called after: `thq1`, `reed1`, `ghost_ai_01`.
    pub name: String,
    pub suit: Suit,
    pub set: MotionSet,
    /// The model name to load: `mrider` for a player, `airider` for one of the pack.
    pub geometry: String,
    /// The `.mcf` member that answered, by its own file name — `mrider.mcf` for a race rider and
    /// `ff_mrider.mcf` for the freestyle copy of the same one, from the same `MCF = rider.mcf`.
    pub mcf: String,
}

impl Rider {
    /// Follow the chain in the archives given, for the rider named after one of them.
    ///
    /// `name` is the archive's own name (`thq1`), and the `.sub` inside it has the same name. An
    /// archive also holds a second `.sub` for when the rider is one of the pack (`thq1_ai`), which
    /// names the lower-detail `airider` body; ask for that name to get it.
    pub fn load(archives: &[Res], name: &str, sex: Sex) -> Result<Rider, String> {
        let member = format!("{name}.sub");
        let bytes = read(archives, &member)?;
        let suit = Suit::parse(&ini::text(&bytes));
        if suit.mcf.is_empty() {
            return Err(format!("{member}: no [Rider] MCF"));
        }
        // The sex-prefixed name first, then the name as written — both allowing a mode in front.
        let names = suit.mcf_names(sex);
        let found = pick(archives, |n| names_it(n, &names[0]))
            .or_else(|| pick(archives, |n| names_it(n, &names[1])));
        let (mcf, bytes) = found
            .ok_or_else(|| format!("{member} names {}, which no archive has", names.join(" or ")))?;
        let bytes = bytes?;
        let set = MotionSet::parse(&ini::text(&bytes));
        let geometry = set.geometry();
        if geometry.is_empty() {
            return Err(format!("{mcf}: no [General info] SLTFile"));
        }
        Ok(Rider {
            name: name.to_string(),
            suit,
            set,
            geometry,
            mcf,
        })
    }
}

/// A member of the first archive that holds a file of exactly that name, in whatever directory.
fn read(archives: &[Res], file: &str) -> Result<Vec<u8>, String> {
    pick(archives, |name| name.eq_ignore_ascii_case(file))
        .ok_or_else(|| format!("no archive holds a file called {file}"))?
        .1
}

/// Is this member's file name the one asked for, give or take a mode in front of it?
///
/// The `.mcf` is matched like this because the name in the `.sub` is a stem rather than a file name.
/// The sex goes in front of it — `MCF = rider.mcf` is `mrider.mcf` — and the game's mode can go in
/// front of that: the freestyle archives hold `ff_mrider.mcf` and `ff_airider.mcf` where a race
/// archive holds `mrider.mcf` and `airider.mcf`, from `.sub`s that are word for word the same. So
/// what may stand in front of the name asked for is a mode and its underscore, and nothing else.
///
/// That last part is the whole point of spelling the rule out rather than matching any suffix. Three
/// of this disc's own names end in each other — `rider.mcf`, `mrider.mcf`, `airider.mcf` — so a
/// plain suffix match would answer a request for the female body with the male one, and a request
/// for either with the AI's. Matching the file *name* rather than the whole path matters for the
/// same reason: `rider\anim\mrider.mcf` ends with `rider.mcf` too.
fn names_it(member: &str, asked: &str) -> bool {
    let (member, asked) = (member.to_ascii_lowercase(), asked.to_ascii_lowercase());
    match member.strip_suffix(&asked) {
        Some(mode) => mode.is_empty() || mode.ends_with('_'),
        None => false,
    }
}

/// The first archive holding a member whose file name the test accepts, shortest name first, as its
/// name and its bytes.
fn pick(
    archives: &[Res],
    accept: impl Fn(&str) -> bool,
) -> Option<(String, Result<Vec<u8>, String>)> {
    for archive in archives {
        let found = archive
            .entries()
            .iter()
            .filter(|e| accept(basename(&e.name)))
            .min_by_key(|e| e.name.len())
            .map(|e| e.name.clone());
        if let Some(name) = found {
            let bytes = archive.read(&name);
            return Some((basename(&name).to_string(), bytes));
        }
    }
    None
}

/// The last `\`-separated part of an archive member's name.
fn basename(name: &str) -> &str {
    name.rsplit(['\\', '/']).next().unwrap_or(name)
}

// ---------------------------------------------------------------------------------------------------
// the body on the bike, posed
// ---------------------------------------------------------------------------------------------------

/// The name a rider's clip for a gesture slot is asked for by: the mode's prefix, the word `rider`,
/// and the stunt's `Name` from `stunts.ini`.
///
/// [`game_sim::motion::Library::stunt_name`] builds the same name with `bike` in the middle, because
/// the loader it was read off is the *vehicle's*: a stunt animates the bike and the body on it with
/// two clips of the same name but for that one word. Both are in the archives side by side —
/// `bike\anim\bike_nacnac.mot` in `Bike\250sx.res`, `rider\anim\rider_nacnac.mot` in `Rider\thq1.res`
/// — and a rider archive's `motions.ini` lists its own under `[Stunts]` and `[ff_Stunts]`, which is
/// where the mode prefix comes in: `ff_rider_nacnac.mot` for the freestyle minibikes.
///
/// The 31-byte truncation is the original's, kept here for the same reason it is kept there: the
/// name is formatted into a 32-byte buffer. No name on this disc is long enough to reach it.
pub fn stunt_name(prefix: &str, stunt: &str) -> String {
    let whole = format!("{prefix}rider_{stunt}");
    String::from_utf8_lossy(&whole.as_bytes()[..whole.len().min(31)]).into_owned()
}

/// The three airborne poses, as a rider's own `.mcf` names them: forward, middle and back.
///
/// A bike's `.mcf` spells the same three `bike_air_Fwd`, `_Mid` and `_Bck`, which is what
/// [`game_sim::motion::Library::load`] asks for; a rider's `.mcf` lists them without the actor's
/// word, and no rider archive holds a `rider_air_*.mot`. So this is the one place where the rider's
/// names are not the bike's with one word changed, and it is read off the disc rather than derived.
pub const AIR_POSES: [&str; 3] = ["air_Fwd", "air_Mid", "air_Bck"];

/// A rider ready to be drawn: the three files, the clips bound to the body's own skeleton, and the
/// pose the last frame left on it.
///
/// This is [`crate::anim::Rig`] with a rider's clip table instead of a bike's, and without a driver
/// of its own. The driving is the simulation's: [`game_sim::rider::Rider`] runs one
/// [`game_sim::motion::Driver`] per frame for the crossup its landing test reads, and what that
/// driver answers — which clip, at what phase, faded by how much — is what poses the body here
/// ([`Body::pose`]). Running a second driver beside it would be two state machines where the
/// original has one, and the second one would be guessing at the first one's inputs.
///
/// ## Where the body sits
///
/// Nowhere, by this type: the clips put it there. Every one of them carries a position channel on
/// the pelvis, and the place it moves the pelvis to is on the bike — posed by a clip the rider
/// rides in, the body's four dummy anchors land within a third of an inch of the chassis's four,
/// feet on the pegs and hands on the grips. So a posed rider is drawn with the bike's own
/// transform and nothing else, which is the answer to the question [`bolt_onto`] was written for:
/// the bind pose needs a placement because the game never draws one, and a posed body does not.
pub struct Body {
    /// The chain: the `.sub`'s gear, the `.mcf` it names, and the name of the geometry to load.
    pub rider: Rider,
    /// The clips, the clip table, the clock and the live pose.
    pub rig: Rig,
    /// Whether any frame has written a pose yet. Until one has, the live pose is the bind pose the
    /// body was modelled in, and a shell that drew it then would draw a rider with its arms out.
    /// A frame writes nothing when the blend it carries has no clip behind it — the
    /// [`game_sim::motion::Blend::Free`] a rig starts in, a gesture slot the set could not bind —
    /// so this stays false until the driving asks for something the set has.
    ///
    /// The first frame of a ride is *not* one of those: it asks for the grounded idle and gets it,
    /// but a restart puts the cross-fade ramp at 1, so what it draws is all of the pose the rider
    /// was in, which is the bind pose. That fade is the driving's and is left alone; see
    /// `tests/body.rs`.
    pub posed: bool,
}

impl Body {
    /// Read a rider's three files and bind every clip its gesture slots and airborne poses name.
    ///
    /// `archives` are searched in order and must include the rider's own; `name` is the archive's
    /// own name (`thq1`), `prefix` the mode's (empty, or `ff_` for the freestyle minibikes) and
    /// `stunts` the `stunts.ini` table, of which only the first 24 records are gesture slots.
    ///
    /// The caller still loads the geometry itself, through [`crate::world::Assets`] and after
    /// handing it [`Suit::materials`]: the body is one mesh shared by all 162 riders and the gear is
    /// a repaint of it.
    pub fn load(
        archives: Vec<Res>,
        name: &str,
        sex: Sex,
        prefix: &str,
        stunts: &[Stunt],
        params: Params,
    ) -> Result<Body, String> {
        let rider = Rider::load(&archives, name, sex)?;
        let mut set = Set::load(archives, &rider.mcf)?;
        // The set's own motions first — the airborne poses, the turns and the idles are all in the
        // `.mcf` — and then the gesture slots, which are in no `.mcf` and are found by name.
        set.load_listed();
        for stunt in stunts.iter().take(SLOTS).filter(|s| !s.name.is_empty()) {
            // Every slot, and not just the ones `UsedForBike` marks: that flag says the *bike* has
            // no clip for this stunt, which is what makes the stunt rider-only. The rider has one
            // either way.
            set.load(&stunt_name(prefix, &stunt.name));
        }
        let mut lib = Library::default();
        for (slot, stunt) in stunts.iter().take(SLOTS).enumerate() {
            lib.stunt[slot] = set.bound(&stunt_name(prefix, &stunt.name));
        }
        lib.air_fwd = set.bound(AIR_POSES[0]);
        lib.air_mid = set.bound(AIR_POSES[1]);
        lib.air_bck = set.bound(AIR_POSES[2]);
        Ok(Body { rider, rig: Rig::bound(set, lib, params), posed: false })
    }

    /// Pose the body from a frame of the simulation's own animation driving
    /// ([`game_sim::rider::Rider::animation`]), at the same `dt` the simulation stepped with.
    pub fn pose(&mut self, dt: F, frame: game_sim::motion::Frame) -> Played {
        let (played, posed) = self.rig.driven(dt, frame);
        self.posed |= posed;
        played
    }

    /// The model name to load and to draw this body as: `mrider` for a player, `airider` for one of
    /// the pack.
    pub fn geometry(&self) -> &str {
        &self.rider.geometry
    }

    /// Write this frame's pose into a loaded model, as the triangles the scene draw reads.
    ///
    /// A skinned model has no triangles of its own — the ones [`crate::world::Assets`] expanded at
    /// load time are the bind pose, which is the one pose the game never shows — so the frame's are
    /// written over them and the body is then drawn like any other model, at the bike's transform.
    /// They stay in the model's own space, because that is what a joint matrix leaves them in.
    ///
    /// Expands the same level of detail `Assets` does, which is the finest.
    pub fn skin_into(&self, loaded: &mut Loaded) {
        loaded.tris = skin(&loaded.model, &self.rig.world(), 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mat::V3;

    /// `thq1.sub`, the stock gear every rider archive is cut from, comments and all.
    const SUB: &str = "[Rider]\n; if male, MCF filename will automatically begin with 'm', else 'f'\n\
         ; e.g. mrider.mcf or frider.mcf\nMCF = rider.mcf\n\n\
         ; Licensed riders use specific bikes\n; Comment this line if not a licensed rider\n\
         ;Bike=RM250.sub\n\n[Base Materials]\n\
         ; Base Materials in the slt (this list shouldn't ever change):\n\
         Material=THQ_Helmet_01\nMaterial=THQ_JerseySleeves_01\nMaterial=THQ_Goggles_01\n\n\
         [Material Changes]\n; (comment if the same! (i.e. no changes))\n\n\
         THQ_Helmet_01 = Thor_Helmet_01\n;THQ_JerseySleeves_01 = THQ_JerseySleeves_01\n";

    const MCF: &str = "[General info]\nNumberOfMotions = 3\n\nContentDirectory=Rider\\Anim\\\n\n\
         SLTFile=mrider.Slt\nVUTfilename=mrider.Vut\n\nRotationType=SHORTAXIS\n\n\
         ;====== Turn (Still and Riding) and Air ==========\n\n\
         [MOTION 1]\nNAME=turn_stop_Fwd\nVUE=turn_stop_Fwd.MOT\nLooping=no\n\n\
         [MOTION 2]\nNAME=turn_still_Bck\nVUE=turn_still_Mid.MOT\nLooping=no\n\n\
         [MOTION 3]\nNAME=idle\nVUE=get_ready_idle_01.MOT\nLooping=yes\n\n\
         :=====high speed turn anims============\n\n\
         [MOTION 4]\nNAME=Turn_fast_fwd\nVUE=Turn_fast_fwd.MOT\nLooping=no\n";

    #[test]
    fn a_suit_names_its_motion_set_and_its_repaint() {
        let s = Suit::parse(SUB);
        assert_eq!(s.mcf, "rider.mcf");
        // The one line that would name a licensed rider's bike is commented out on this disc.
        assert_eq!(s.bike, None);
        assert_eq!(s.base_materials.len(), 3);
        assert_eq!(s.base_materials[0], "THQ_Helmet_01");
        // A commented-out change is not a change, which is how "no changes" is spelled.
        assert_eq!(
            s.materials,
            vec![("THQ_Helmet_01".to_string(), "Thor_Helmet_01".to_string())]
        );
        // The body is chosen by prefixing the name the file gives, with that name as the fallback.
        assert_eq!(s.mcf_names(Sex::Male), ["mrider.mcf", "rider.mcf"]);
        assert_eq!(s.mcf_names(Sex::Female)[0], "frider.mcf");
    }

    #[test]
    fn an_ai_suit_keeps_the_name_it_was_given() {
        let s = Suit::parse("[Rider]\nMCF = airider.mcf\n");
        // The prefix would make `mairider.mcf`, which no archive has; the fallback is the point.
        assert_eq!(s.mcf_names(Sex::Male), ["mairider.mcf", "airider.mcf"]);
    }

    #[test]
    fn a_motion_set_names_the_geometry_and_its_clips() {
        let set = MotionSet::parse(MCF);
        assert_eq!(set.slt, "mrider.Slt");
        assert_eq!(set.geometry(), "mrider", "an .Slt is a .psg member");
        assert_eq!(set.vut, "mrider.Vut");
        assert_eq!(set.rotations.as_deref(), Some("SHORTAXIS"));
        assert_eq!(set.directory, "Rider\\Anim\\");
        assert_eq!(set.motions.len(), 3, "NumberOfMotions, not the sections there are");
        assert_eq!(set.unlisted.len(), 1, "a clip the archive does not hold");
        assert_eq!(set.unlisted[0].name, "Turn_fast_fwd");
        assert_eq!(set.clip("Turn_fast_fwd"), None, "the loader never sees it");
        // Two names can share one clip, and the lookup is by name, not by file.
        assert_eq!(set.clip("turn_still_Bck"), Some("turn_still_Mid.MOT"));
        assert_eq!(set.clip("TURN_STOP_FWD"), Some("turn_stop_Fwd.MOT"));
        assert_eq!(set.clip("nothing_like_it"), None);
        assert!(set.motions[2].looping, "Looping=yes");
        assert!(!set.motions[0].looping);
    }

    /// A skeleton with the shape the two real ones have where it matters: a root, a chain, and a
    /// dummy at the end of it with no geometry.
    fn chain(names: &[(&str, Option<usize>, V3)]) -> Skeleton {
        let mut bones: Vec<Bone> = Vec::new();
        for (name, parent, offset) in names {
            let mut local = M4::IDENTITY;
            local.0[3] = [offset.x, offset.y, offset.z, 1.0];
            let rest = match parent {
                Some(p) => local.mul(bones[*p].rest),
                None => local,
            };
            bones.push(Bone {
                name: name.to_string(),
                parent: *parent,
                local,
                rest,
            });
        }
        Skeleton { bones }
    }

    #[test]
    fn a_skeletons_bind_pose_composes_the_chain() {
        let s = chain(&[
            ("Pelvis", None, v3(0.0, 0.0, 0.0)),
            ("UprLegL", Some(0), v3(-0.3, -0.1, 0.0)),
            ("FootL", Some(1), v3(0.0, -2.7, 0.0)),
        ]);
        assert_eq!(s.find("footl"), Some(2), "names match case-insensitively");
        assert_eq!(s.at("FootL"), Some(v3(-0.3, -2.8, 0.0)));
        assert_eq!(s.at("Elbow"), None);
    }

    #[test]
    fn the_bolt_stands_the_rider_on_the_pegs() {
        // The rider: pelvis at the origin, foot anchors three feet below it, hands out to the sides.
        let rider = chain(&[
            ("Pelvis", None, v3(0.0, 0.0, 0.0)),
            ("Dummy_foot_L", Some(0), v3(-0.4, -3.2, 0.0)),
            ("Dummy_foot_R", Some(0), v3(0.4, -3.2, 0.0)),
            ("Dummy_hand_L", Some(0), v3(-2.7, 1.4, 0.0)),
            ("Dummy_hand_R", Some(0), v3(2.7, 1.4, 0.0)),
        ]);
        // The chassis spells the same four names with capitals, and puts the pegs where they are on
        // a bike: a foot and a half up, a little behind the middle.
        let chassis = chain(&[
            ("Bike", None, v3(0.0, 0.0, 0.0)),
            ("Dummy_Foot_L", Some(0), v3(-0.8, 1.7, -0.85)),
            ("Dummy_Foot_R", Some(0), v3(0.8, 1.7, -0.85)),
            ("Dummy_Hand_L", Some(0), v3(-1.1, 4.4, 0.55)),
            ("Dummy_Hand_R", Some(0), v3(1.1, 4.4, 0.55)),
        ]);
        assert_eq!(
            rider.shared_with(&chassis).len(),
            4,
            "the four anchors are the whole of what the two skeletons share"
        );
        let feet = bolt_onto(&rider, &chassis, &FOOT_ANCHORS).expect("the anchors are there");
        // The rider's feet land on the pegs. The *mean* of the pair, not each foot: a translation
        // cannot also pull the bind pose's stance out to the width of the pegs, and the real pair is
        // half as wide as the real pegs, so each foot lands inside its own.
        let moved = |name: &str| feet.point(rider.at(name).unwrap());
        let mid = moved("Dummy_foot_L").add(moved("Dummy_foot_R")).scale(0.5);
        let pegs = chassis
            .at("Dummy_Foot_L")
            .unwrap()
            .add(chassis.at("Dummy_Foot_R").unwrap())
            .scale(0.5);
        assert!(mid.sub(pegs).len() < 1e-6, "{mid:?} is not {pegs:?}");
        assert_eq!(moved("Dummy_foot_L").y, 1.7, "and at the height of the pegs");
        // The body stands above the bike rather than inside it.
        assert!(moved("Pelvis").y > 4.5, "{:?}", moved("Pelvis"));
        // The grips are the other pair, and they put the body where the bars are instead.
        let hands = bolt_onto(&rider, &chassis, &HAND_ANCHORS).expect("the anchors are there");
        assert_eq!(hands.point(rider.at("Dummy_hand_L").unwrap()).y, 4.4);
        // ...which is why the feet are the pair to use: the grips drop the feet through the engine.
        assert!(hands.point(rider.at("Dummy_foot_L").unwrap()).y < 0.0);
        // A name neither skeleton has is a failure, not a silent identity.
        assert!(bolt_onto(&rider, &chassis, &["Dummy_elbow_L"]).is_none());
        assert!(bolt_onto(&rider, &chassis, &[]).is_none());
    }
}
