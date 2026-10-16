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

use crate::disc::Res;
use crate::ini;
use crate::mat::{M4, V3, v3};
use crate::psg::Model;

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

/// One bone, as the `.psg`'s node tree gives it.
#[derive(Clone, Debug)]
pub struct Bone {
    pub name: String,
    pub parent: Option<usize>,
    /// Relative to the parent bone.
    pub local: M4,
    /// The bind pose: the bone's place in model space, parents composed in.
    pub rest: M4,
}

impl Bone {
    /// Where the bone sits in the bind pose.
    pub fn at(&self) -> V3 {
        self.rest.point(v3(0.0, 0.0, 0.0))
    }
}

/// A model's node tree read as a skeleton: the bones in file order, each with its bind pose.
///
/// Both the rider and the chassis are read this way, because bolting one to the other is a question
/// about two bind poses and nothing else.
#[derive(Clone, Debug, Default)]
pub struct Skeleton {
    pub bones: Vec<Bone>,
}

impl Skeleton {
    pub fn of(model: &Model) -> Skeleton {
        Skeleton {
            bones: model
                .nodes
                .iter()
                .enumerate()
                .map(|(i, n)| Bone {
                    name: n.name.clone(),
                    parent: n.parent,
                    local: n.local,
                    rest: model.world(i),
                })
                .collect(),
        }
    }

    /// A bone by name, case-insensitively: the rider spells its anchors `Dummy_foot_L` and the
    /// chassis spells the same joint `Dummy_Foot_L`.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|b| b.name.eq_ignore_ascii_case(name))
    }

    /// Where a named bone sits in the bind pose.
    pub fn at(&self, name: &str) -> Option<V3> {
        Some(self.bones[self.find(name)?].at())
    }

    /// The bone names the two skeletons have in common — the anchors one is meant to be hung off the
    /// other by. Four of them for a rider and a chassis: two grips and two foot pegs.
    pub fn shared_with(&self, other: &Skeleton) -> Vec<String> {
        self.bones
            .iter()
            .filter(|b| other.find(&b.name).is_some())
            .map(|b| b.name.clone())
            .collect()
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

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
