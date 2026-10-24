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
use crate::mat::{M4, V3, v3};
use crate::skin::{rotation, skin};
use crate::world::Loaded;
use game_sim::motion::{Animations, Clip, ClipId, Library, Params, Played};
use game_sim::rider::Gate;
use game_sim::ride::Controls;
use game_sim::trick::{SLOTS, Stunt};
use game_sim::vehicle_anim;
use ps2_float::{F, Vf};

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

/// One of the four limbs the original aims at an anchor, as the rider's own bones name it: the joint
/// the limb swings from, the joint it bends at, and the dummy anchor at the end of it — which is also
/// the name of the chassis bone it is aimed at, because the two skeletons spell that joint the same
/// (give or take the case, which [`Skeleton::find`] ignores as the game does).
///
/// Nothing between the bend and the anchor is turned, and that is the original's choice rather than a
/// simplification: it corrects each target by the end bone's own offset to the anchor and solves the
/// limb against *that*, so a hand keeps the orientation the clip gave it and the arm is what moves to
/// put it on the grip. The same offset is already in this joint array — the anchor is a joint of it —
/// so the correction is not needed here and the hand's own rotation is left alone either way.
pub struct Limb {
    pub root: &'static str,
    pub bend: &'static str,
    pub anchor: &'static str,
}

/// The four limbs the original builds a controller for, in the order the vehicle caches the anchors
/// they are aimed at: the two feet, then the two hands.
pub const LIMBS: [Limb; 4] = [
    Limb { root: "UprLegL", bend: "LwrLegL", anchor: FOOT_ANCHORS[0] },
    Limb { root: "UprLegR", bend: "LwrLegR", anchor: FOOT_ANCHORS[1] },
    Limb { root: "UprArmL", bend: "LwrArmL", anchor: HAND_ANCHORS[0] },
    Limb { root: "UprArmR", bend: "LwrArmR", anchor: HAND_ANCHORS[1] },
];

/// Where the four anchors of [`LIMBS`] are to be put, in the bike's own space — which is the body's,
/// because a posed rider is drawn at the bike's transform and bolts nothing. `None` for an anchor the
/// chassis has not got, and that limb is then left as the clip posed it.
pub type Anchors = [Option<V3>; 4];

/// Read the four targets off a chassis: where its own grips and foot pegs are this frame.
///
/// This is what the original hands its limb controllers — the vehicle caches those four chassis nodes
/// when it loads and the rider reads their world positions every frame. `world` is the chassis's own
/// joint array ([`Skeleton::world`]); pass an empty slice for the bind pose, which is where a chassis
/// that nothing animates keeps them.
pub fn anchors_of(chassis: &Skeleton, world: &[M4]) -> Anchors {
    let mut out = [None; 4];
    for (limb, slot) in LIMBS.iter().zip(out.iter_mut()) {
        let Some(j) = chassis.find(limb.anchor) else { continue };
        *slot = Some(match world.get(j) {
            Some(m) => m.point(v3(0.0, 0.0, 0.0)),
            None => chassis.bones[j].at(),
        });
    }
    out
}

/// Put the rider's hands on the grips and its feet on the pegs, solved in place on a joint array from
/// [`Skeleton::world`]: the clip drawn with its limbs' ends on the bike's own anchors.
///
/// **This is a stand-in, and not the original's** (V61). The original has no limb solve on the
/// drawn skeleton at all: what V10 took for four limb controllers are the riding ragdoll's four hold
/// pins (`game_sim::ragdoll::Ragdoll::holds`), and the weight it found read off each one's bone is
/// that bone's inverse mass — the pin's own solve, ported and proved as
/// `game_sim::ragdoll::Pin::one_body`. While riding, the rider is drawn from that ragdoll
/// (`game_sim::ragdoll_riding`), whose pins pull the hand and foot bodies onto the grips and pegs.
/// The shell draws from it by default since S111; this holds the clip on whenever the ragdoll is
/// not drawn (`OPENMX_RIDING_RAGDOLL=0`, online, or before it has been placed), until V63.
///
/// **When it holds is the original's**: `strength` is [`Body::hold`], 1 while the body's mode
/// machine is riding and 0 otherwise. The original's ragdoll is not placed, so not stepped and not
/// drawn, whenever a pose off the clip table is in charge — a gesture, a no-hander, a backflip's
/// hold — or the body is at the gate; the clip is drawn exactly as it was written then, every limb
/// included. `strength` between 0 and 1 fades each target towards where the clip has its anchor.
///
/// Returns how far each anchor ended from the target it was aimed at, in the disc's unit, which is a
/// foot; `None` for a limb that was not solved — no target, a joint this skeleton has not got, a limb
/// with no length, or a `world` that is not this skeleton's.
pub fn hold_on(skel: &Skeleton, world: &mut [M4], targets: &Anchors, strength: f32) -> [Option<f32>; 4] {
    hold_limbs(skel, world, targets, [strength; 4])
}

/// [`hold_on`] with a strength of its own for each of [`LIMBS`] (V36): the feet are let go of on
/// their own, by the riding body's foot switch ([`game_sim::foot_down::Feet`]), while the hands stay
/// on the grips.
pub fn hold_limbs(skel: &Skeleton, world: &mut [M4], targets: &Anchors, strength: [f32; 4]) -> [Option<f32>; 4] {
    let mut out = [None; 4];
    if world.len() != skel.len() {
        return out;
    }
    for (i, limb) in LIMBS.iter().enumerate() {
        let (Some(target), Some(root), Some(bend), Some(anchor)) = (
            targets[i],
            skel.find(limb.root),
            skel.find(limb.bend),
            skel.find(limb.anchor),
        ) else {
            continue;
        };
        if strength[i] <= SMALL {
            out[i] = Some(world[anchor].point(v3(0.0, 0.0, 0.0)).sub(target).len());
            continue;
        }
        out[i] = aim(skel, world, [root, bend, anchor], target, strength[i]);
    }
    out
}

/// Whether a body in this mode of its machine holds on to the bike (V61): only while riding.
///
/// The original's rule, through its riding ragdoll: the actor's mode is chosen by the same chain as
/// [`game_sim::rider_anim::Body::mode`], and every mode but riding (and the wreck, which is drawn
/// from the wreck's ragdoll) leaves the ragdoll unplaced, so the clip is drawn as written. Measured
/// on recording2 (journal 2026-10-23-v61-limb-hold): the ragdoll was placed on exactly the riding
/// ticks with no pose off the clip table in charge, 1499 of 1499, of which 368 were posed.
pub fn holds_in(mode: game_sim::rider_anim::Mode) -> bool {
    mode == game_sim::rider_anim::Mode::Riding
}

/// One limb's solve: bend the middle joint until the anchor is as far from the root as the target is,
/// then swing the root until it points at the target.
///
/// Two turns and no iteration, which is the shape of the original's as well — its solve is a page of
/// VU0 quaternion arithmetic with no loop in it, so it is a closed form of the same two moves. The
/// hinge goes first because bending it is what sets how far the limb reaches; once the reach is right
/// the swing is a single rotation that lands the anchor on the target exactly. A target further away
/// than the limb can reach straightens it and the limb points at the target as far as it goes; one
/// nearer than the limb can fold leaves it folded.
///
/// `joints` are the limb's three in order: swing, bend, anchor. `strength` fades the target towards
/// where the anchor already is, and the answer is how far the anchor ended from the target that was
/// *asked* for — so a limb held at half weight answers about half of what it started out.
fn aim(skel: &Skeleton, world: &mut [M4], joints: [usize; 3], target: V3, strength: f32) -> Option<f32> {
    let [root, bend, anchor] = joints;
    let at = |world: &[M4], j: usize| world[j].point(v3(0.0, 0.0, 0.0));
    let (r, b, e) = (at(world, root), at(world, bend), at(world, anchor));
    // What the limb is made of: the two lengths the solve has to work with. They are the posed
    // lengths rather than the skeleton's, because the second one runs through the end bone to the
    // anchor and so depends on how the clip turned that bone.
    let (upper, lower) = (b.sub(r).len(), e.sub(b).len());
    if upper < SMALL || lower < SMALL {
        return None;
    }
    let asked = target;
    let target = e.add(target.sub(e).scale(strength));
    let span = target.sub(r).len().clamp((upper - lower).abs() + SMALL, upper + lower - SMALL);
    // The hinge, by the law of cosines: the angle between the two lengths that holds the anchor
    // `span` from the root, less the angle they are at now.
    let folded = |across: f32| {
        ((upper * upper + lower * lower - across * across) / (2.0 * upper * lower)).clamp(-1.0, 1.0).acos()
    };
    let turn = folded(span) - folded(e.sub(r).len());
    if turn.abs() > SMALL {
        // The hinge plane's own normal, and any normal at all when the limb is folded flat and has
        // none. Which way round it the joint goes is not the sign of the normal — that flips with how
        // the limb happens to be bent — so take whichever of the two turns gets the reach right.
        let normal = r.sub(b).cross(e.sub(b));
        let normal = if normal.len() > SMALL { normal.norm() } else { across(b.sub(r)) };
        let reach = |way: f32| spun(e, b, turned(normal, way * turn)).sub(r).len();
        let way = if (reach(1.0) - span).abs() <= (reach(-1.0) - span).abs() { 1.0 } else { -1.0 };
        spin(world, &under(skel, bend), b, turned(normal, way * turn));
    }
    // And the swing, which is one rotation of the whole limb about the root: the anchor where the
    // hinge left it, turned onto the line from the root to the target.
    let (from, to) = (at(world, anchor).sub(r), target.sub(r));
    if from.len() > SMALL && to.len() > SMALL {
        let (from, to) = (from.norm(), to.norm());
        let angle = from.dot(to).clamp(-1.0, 1.0).acos();
        let axis = from.cross(to);
        let axis = if axis.len() > SMALL { axis.norm() } else { across(from) };
        if angle > SMALL {
            spin(world, &under(skel, root), r, turned(axis, angle));
        }
    }
    Some(at(world, anchor).sub(asked).len())
}

/// Below this a length is noise and an angle is nothing: the solve's own guard against dividing by a
/// limb of no length or turning about an axis of none.
const SMALL: f32 = 1e-5;

/// A rotation of `angle` radians about a unit `axis`, as the quaternion [`rotation`] wants.
fn turned(axis: V3, angle: f32) -> [f32; 4] {
    let half = angle / 2.0;
    let s = half.sin();
    [axis.x * s, axis.y * s, axis.z * s, half.cos()]
}

/// One point turned about an axis through `pivot`, which is how the solve measures a turn before it
/// commits to it.
fn spun(p: V3, pivot: V3, q: [f32; 4]) -> V3 {
    rotation(q).dir(p.sub(pivot)).add(pivot)
}

/// Turn a set of joints about an axis through `pivot`, in the space their matrices are already in:
/// each gets the same rotation after its own, which is what carries a limb rigidly with the joint
/// that turned.
fn spin(world: &mut [M4], joints: &[usize], pivot: V3, q: [f32; 4]) {
    let mut m = rotation(q);
    // A turn about a point, in the row-vector form the rest of the crate uses: `p * r` and then back
    // by however far the pivot itself moved.
    let back = pivot.sub(m.dir(pivot));
    m.0[3] = [back.x, back.y, back.z, 1.0];
    for &j in joints {
        world[j] = world[j].mul(m);
    }
}

/// A joint and everything hanging off it, which is what a turn of that joint has to carry.
fn under(skel: &Skeleton, joint: usize) -> Vec<usize> {
    (0..skel.len())
        .filter(|&j| {
            let mut at = Some(j);
            // The walk is capped rather than trusted: a parent loop gives up instead of hanging, as
            // everywhere else this crate walks the tree.
            for _ in 0..=skel.len() {
                match at {
                    Some(i) if i == joint => return true,
                    Some(i) => at = skel.bones[i].parent,
                    None => return false,
                }
            }
            false
        })
        .collect()
}

/// Any unit vector at right angles to `v`: the axis to turn about when the geometry has none of its
/// own, which is a limb folded flat or already pointing where it is being sent.
fn across(v: V3) -> V3 {
    let other = if v.x.abs() < 0.9 { v3(1.0, 0.0, 0.0) } else { v3(0.0, 1.0, 0.0) };
    let c = v.cross(other);
    if c.len() > SMALL { c.norm() } else { v3(0.0, 1.0, 0.0) }
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
/// of its own. The driving is the simulation's, and **it is the body's own machine**: the sixty-state
/// one ([`game_sim::rider_anim`], S27), stepped by [`game_sim::rider::Rider::body_frame`] and drawn
/// here by [`Body::ride`]. Running a driver of its own beside that would be two state machines where
/// the original has one, and the second one would be guessing at the first one's inputs.
///
/// V8 posed it from the other machine — the 28-slot [`game_sim::motion::Driver`] the rider's frame
/// runs for the crossup its landing test reads, which is the *bike's* ([`Body::pose`]) — and what
/// that drew was a body stood upright on a bike being ridden, because every row of the grid the
/// grounded rider is posed out of is a clip only the sixty-state table names. V18 is the swap back.
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
    /// The **sixty-state** machine's clip table over the same clips: the 24 gesture slots under the
    /// rider's own names and [`vehicle_anim::POSE_CLIPS`] bare, which is the table
    /// [`game_sim::rider::Rider::body_frame`] selects out of ([`Body::ride`]).
    ///
    /// Beside [`Rig::lib`] and not instead of it, because the two tables are two actors' and the set
    /// underneath is one: the clips are read and bound once, and which table names them decides
    /// which machine can play them. [`Rig::lib`]'s 28 slots are the bike's.
    pub pose_lib: vehicle_anim::Library,
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
    /// How hard the drawn clip holds on to the bike ([`hold_on`]'s `strength`): 1 while the body's
    /// machine is riding, 0 while anything else poses it ([`holds_in`]). [`Body::ride`] writes it;
    /// a body nothing rides (the biplane's pilot) holds on.
    pub hold: f32,
    /// The riding body's two foot holds (V36), which [`Body::ride`] steps: a stopped bike's rider
    /// lets both feet off the pegs, so the stopped row's clip draws them on the ground, and a slow,
    /// tipped one puts the inside foot out. [`Body::holds`] is what the draw holds each limb with.
    pub feet: game_sim::foot_down::Feet,
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
        // And the other machine's table over the same clips (V18). The 24 gesture slots are already
        // loaded, under the same names this table wants — `rider_<stunt>` is what both loaders ask
        // for, one through `stunt_name` and one through `Library::stunt_name` with `rider_` in the
        // prefix — so these two passes only bind. The 36 pose clips come off the `.mcf`'s own list
        // and are looked up bare; one the list leaves out is asked for by name rather than left
        // empty, which is `crate::anim::Stance::new`'s last pass and this disc's rider needs none of
        // it.
        let mut pose_lib = vehicle_anim::Library::default();
        for (slot, stunt) in stunts.iter().take(SLOTS).enumerate() {
            if !stunt.name.is_empty() {
                pose_lib.clip[slot] = set.bound(&stunt_name(prefix, &stunt.name));
            }
        }
        for (i, name) in vehicle_anim::POSE_CLIPS.iter().enumerate() {
            if set.bound(name).is_none() {
                set.load(name);
            }
            pose_lib.clip[SLOTS + i] = set.bound(name);
        }
        Ok(Body {
            rider,
            rig: Rig::bound(set, lib, params),
            pose_lib,
            posed: false,
            hold: 1.0,
            feet: Default::default(),
        })
    }

    /// **MD21:** a trick pack's rider clips in [`Body::pose_lib`]'s states past the sixty — its
    /// extras and the 360s — bound to this body's skeleton. Answers the names that found no clip.
    pub fn add_pack(&mut self, pack: &crate::anim::PackClips, stunts: &[Stunt]) -> Vec<String> {
        pack.fill(&mut self.rig.set, &mut self.pose_lib, stunts, crate::anim::PackClips::rider_members)
    }

    /// Pose the body from a frame of the **bike's** animation driving
    /// ([`game_sim::rider::Rider::animation`]), at the same `dt` the simulation stepped with.
    ///
    /// This is V8's wiring and it is not what poses a rider: the frame it takes is the 28-slot
    /// machine's, which drives the vehicle (see [`crate::anim::Stance`]'s table, and V13 on the two
    /// being crossed over). A body posed from it stands upright on a bike that is being ridden.
    /// [`Body::ride`] is the rider's own machine; what this one is still good for is measuring that
    /// driving against this set, which is what `tests/body.rs` does with it.
    pub fn pose(&mut self, dt: F, frame: game_sim::motion::Frame) -> Played {
        let (played, posed) = self.rig.driven(dt, frame);
        self.posed |= posed;
        played
    }

    /// **One frame of the rider on the bike** (V18): the body's own sixty-state machine chooses, and
    /// the pose it chose is written onto this skeleton.
    ///
    /// The selection and the clock are the simulation's — [`game_sim::rider::Rider::body_frame`] is
    /// the entry point, and it is documented as the one the holder of the clip table calls, which is
    /// this type. So the two halves are this: the sim's machine is run against [`Body::pose_lib`]
    /// and this set's clip lengths, and then [`Body::posed_by`] samples what it asked for.
    ///
    /// `c` is the same controls the ride's own frame was stepped with and `gate` the two inputs the
    /// body has that the rider does not hold ([`Gate`]). Answers the frame, for a caller that wants
    /// to know which state the body is in.
    pub fn ride(&mut self, dt: F, rider: &mut game_sim::rider::Rider, c: &Controls, gate: Gate)
    -> vehicle_anim::Frame {
        let frame = {
            let lib = &self.pose_lib;
            let set = &self.rig.set;
            rider.body_frame(dt, c, gate, lib, &|id| set.timing(Some(id)))
        };
        self.posed_by(frame, &rider.body.player, rider.body_clocked);
        self.hold = if holds_in(rider.body.machine.mode) { 1.0 } else { 0.0 };
        // The foot switch runs where the body actor runs it: on every tick that is neither a wreck
        // nor a pose off the clip table, off what the shell's riding request hands the ragdoll.
        let mode = rider.body.machine.mode;
        if mode != game_sim::rider_anim::Mode::Wrecked && mode != game_sim::rider_anim::Mode::Posed {
            self.feet.step(dt, &game_sim::foot_down::Input::of(&rider.ride, &rider.body.veh));
        }
        frame
    }

    /// How hard the draw holds each of [`LIMBS`] on: [`Body::hold`], and each foot at its hold's
    /// weight from [`Body::feet`] as well.
    pub fn holds(&self) -> [f32; 4] {
        let [l, r] = self.feet.strength();
        [self.hold * l, self.hold * r, self.hold, self.hold]
    }

    /// Write the pose a frame of the body's own machine asked for, from the clock the simulation
    /// already ran.
    ///
    /// The sixty-state half of [`Rig::driven`], and shorter than it by the clock: the simulation's
    /// body *is* the motion object ([`game_sim::rider_anim::Body::player`]), so the advance has
    /// happened and sampling it again here would run the clip at twice the rate. What is left is the
    /// snapshot the restart and the put owe the cross-fade, and the sample itself — the same
    /// `apply_vehicle_crossed` [`crate::anim::Stance::frame`] ends in, with the transition the step
    /// answered (V17).
    ///
    /// Answers whether the frame wrote a pose at all: a blend whose clips are not all in this set
    /// leaves the one that was already there, which before anything has posed it is the bind pose
    /// ([`Body::posed`]).
    pub fn posed_by(
        &mut self,
        frame: vehicle_anim::Frame,
        player: &game_sim::motion::Player,
        clocked: game_sim::rider_anim::Clocked,
    ) -> bool {
        if frame.restart || clocked.snapshot {
            let Body { rig, .. } = self;
            rig.set.rewind(&mut rig.snapshot);
        }
        let drawn = self.rig.set.apply_vehicle_crossed(frame.blend, player, clocked.played.fade);
        self.posed |= drawn;
        drawn
    }

    /// How long a clip of this set is, which is the one thing the body's machine asks its holder
    /// ([`game_sim::rider::Rider::body_frame`]'s `clip` argument).
    pub fn timing(&self, id: ClipId) -> Clip {
        self.rig.set.timing(Some(id))
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
    /// Expands the same level of detail `Assets` does, which is the finest, and holds nothing: for a
    /// rider the camera is going to pull away from, or one whose hands belong on the bars,
    /// [`Body::skin_into_at`].
    pub fn skin_into(&self, loaded: &mut Loaded) {
        self.skin_into_at(loaded, 0, None);
    }

    /// The same, at one level of detail and holding on to the bike.
    ///
    /// `group` is which of the model's five the camera has earned ([`Body::group_for`]); `hold` the
    /// four places the chassis wants the body's anchors ([`anchors_of`]), or `None` to draw the pose
    /// as the clips left it. Answers what [`hold_on`] does: how far each anchor ended from its
    /// target, which is the one number that says whether the rider is holding on.
    pub fn skin_into_at(
        &self,
        loaded: &mut Loaded,
        group: usize,
        hold: Option<&Anchors>,
    ) -> [Option<f32>; 4] {
        let mut world = self.rig.world();
        let held = match hold {
            Some(targets) => hold_limbs(&self.rig.set.skeleton, &mut world, targets, self.holds()),
            None => [None; 4],
        };
        loaded.tris = skin(&loaded.model, &world, group);
        held
    }

    /// The pose as the GPU draws it (P44): the node matrices [`Body::skin_into_at`] skins with, held
    /// on to the bike the same way, with nothing skinned — the vertex shader places each vertex by its
    /// node's matrix itself.
    pub fn palette(&self, hold: Option<&Anchors>) -> (Vec<M4>, [Option<f32>; 4]) {
        let mut world = self.rig.world();
        let held = match hold {
            Some(targets) => hold_limbs(&self.rig.set.skeleton, &mut world, targets, self.holds()),
            None => [None; 4],
        };
        (world, held)
    }

    /// Which of the body's levels of detail to draw, for a camera `distance` from the bike whose
    /// focal length is `focal` pixels ([`crate::world::Camera::focal`]).
    ///
    /// The body's own thresholds, through the engine's own rule — [`crate::psg::Model::group_for`],
    /// which is where the rule is written down. A rider two hundred feet away is three groups coarser
    /// than the one on your bike and costs a tenth of its triangles.
    pub fn group_for(&self, loaded: &Loaded, distance: f32, focal: f32) -> usize {
        loaded.model.group_for(distance, loaded.radius, focal)
    }
}

// ---------------------------------------------------------------------------------------------------
// the ragdoll (S53)
// ---------------------------------------------------------------------------------------------------

/// The clip the ragdoll's riding pose is read from, at its first key: the rider sat square on a bike
/// going straight on. The original reads the live pose at the moment the bones are built; the port
/// builds them once ([`Ragdolled`]).
pub const RAGDOLL_POSE: &str = "turn_riding_Mid";

/// **A body's ragdoll**, as the shell keeps it: the riding skeleton the simulation's
/// [`game_sim::ragdoll_ride::Kit`] is built from, and which skeleton node each of its parts is, for
/// drawing the body from the bones while the rider is down.
#[derive(Clone, Debug)]
pub struct Ragdolled {
    pub rig: game_sim::ragdoll_ride::Rig,
    /// One skeleton node per part, in the rider file's order.
    pub nodes: Vec<usize>,
}

impl Body {
    /// **The riding skeleton, as the ragdoll reads it.** `model` is the body's geometry (whose
    /// bounding centres are the parts' default centres of mass) and `rider_file` the rider's physics
    /// file (`mrider.phy`), whose `[PartN]` sections name a node each.
    ///
    /// `None` when a part names a node the skeleton does not have, or the pose clip is missing: a body
    /// whose bones would be drawn from the wrong place is better drawn as before.
    pub fn ragdoll_rig(&mut self, model: &crate::psg::Model, rider_file: &[u8]) -> Option<Ragdolled> {
        use game_sim::ragdoll_pose::Bone as Placed;
        use game_sim::ragdoll_ride::{Rig, RigPart};
        let set = &mut self.rig.set;
        if set.bound(RAGDOLL_POSE).is_none() {
            set.load(RAGDOLL_POSE);
        }
        let id = set.bound(RAGDOLL_POSE)?;
        let bound = set.get(id)?;
        let skeleton = &set.skeleton;
        let mut pose = crate::skin::Pose::new(skeleton.len());
        bound.clip.pose(0.0, &bound.joints, &mut pose);
        let locals = skeleton.locals(&pose);
        let world = skeleton.compose(&locals);
        let row = |r: [f32; 4], w: f32| Vf::new(r[0], r[1], r[2], w);

        let ini = game_sim::phy::Ini::parse_bytes(rider_file);
        let count = ini.section("Physics").int("NumberOfParts").unwrap_or(0).max(0) as usize;
        let mut parts = Vec::with_capacity(count);
        let mut nodes = Vec::with_capacity(count);
        for i in 1..=count {
            let s = ini.section(&format!("Part{i}"));
            let name = s.text("Name")?.trim();
            let node = skeleton.find(name)?;
            let default_cg = s.text("DefaultCG").is_none_or(|t| t.trim().starts_with(['T', 't']));
            let cofg = match model.nodes.iter().find(|n| n.name.eq_ignore_ascii_case(name)) {
                Some(n) if default_cg => Vf::new(n.center.x, n.center.y, n.center.z, 1.0),
                _ => {
                    let v = s.vector("CG", Vf::ZERO);
                    Vf::new(v.x(), v.y(), v.z(), 1.0)
                }
            };
            let m = world[node].0;
            let rows = locals[node].0.map(|r| row(r, r[3]));
            let q = game_sim::math::mat4_to_quat(&game_sim::math::Mat4(rows));
            let (x, y, z, w) = (q.x(), q.y(), q.z(), q.w());
            let n = (x * x + y * y + z * z + w * w).sqrt();
            let k = if w < 0.0 { -1.0 / n } else { 1.0 / n };
            parts.push(RigPart {
                local: row(locals[node].0[3], 1.0),
                cofg,
                bone: Placed { at: row(m[3], 1.0), forward: row(m[2], 0.0), up: row(m[1], 0.0) },
                rest: Vf::new(x * k, y * k, z * k, w * k),
            });
            nodes.push(node);
        }
        let hand = |name: &str| skeleton.find(name).map_or(Vf::ZERO, |j| row(locals[j].0[3], 1.0));
        let hands = [hand("Dummy_hand_L"), hand("Dummy_hand_R")];
        Some(Ragdolled { rig: Rig { parts, hands }, nodes })
    }

    /// **The skeleton as this body last posed it, as the ragdoll reads it** (S65): each part's node
    /// in the bike's frame and its rest, worked out as [`Body::ragdoll_rig`] works out the riding
    /// pose's, from the pose [`Body::ride`] last wrote. The simulation's wreck is posed from it
    /// ([`game_sim::rider::Rider::skeleton`]).
    pub fn ragdoll_posed(&self, doll: &Ragdolled) -> game_sim::ragdoll_ride::Posed {
        use game_sim::ragdoll_pose::Bone as Placed;
        let skeleton = &self.rig.set.skeleton;
        let locals = skeleton.locals(&self.rig.set.pose);
        let world = skeleton.compose(&locals);
        let row = |r: [f32; 4], w: f32| Vf::new(r[0], r[1], r[2], w);
        let mut posed = game_sim::ragdoll_ride::Posed::default();
        for &node in &doll.nodes {
            let (Some(m), Some(local)) = (world.get(node), locals.get(node)) else { break };
            let rows = local.0.map(|r| row(r, r[3]));
            let q = game_sim::math::mat4_to_quat(&game_sim::math::Mat4(rows));
            let (x, y, z, w) = (q.x(), q.y(), q.z(), q.w());
            let n = (x * x + y * y + z * z + w * w).sqrt();
            let k = if w < 0.0 { -1.0 / n } else { 1.0 / n };
            posed.bones.push(Placed { at: row(m.0[3], 1.0), forward: row(m.0[2], 0.0), up: row(m.0[1], 0.0) });
            posed.rests.push(Vf::new(x * k, y * k, z * k, w * k));
        }
        posed
    }

    /// Write the body into `loaded` **from the ragdoll's bones**, as it is drawn while the rider is
    /// down: each part's node at its bone ([`game_sim::ragdoll_ride::Down::nodes`], in the world),
    /// every other node at its riding-pose place under its parent. The triangles are left in the
    /// space of `at`, the transform the body is then drawn with.
    pub fn skin_ragdoll_into(
        &self,
        loaded: &mut Loaded,
        group: usize,
        doll: &Ragdolled,
        bones: &[game_sim::math::Mat4],
        at: M4,
    ) {
        loaded.tris = skin(&loaded.model, &self.ragdoll_palette(doll, bones, at), group);
    }

    /// The ragdoll's pose as the GPU draws it (P44): the node matrices [`Body::skin_ragdoll_into`]
    /// skins with, in the space of `at`.
    pub fn ragdoll_palette(&self, doll: &Ragdolled, bones: &[game_sim::math::Mat4], at: M4) -> Vec<M4> {
        let skeleton = &self.rig.set.skeleton;
        let locals = skeleton.locals(&self.rig.set.pose);
        let back = rigid_inverse(at);
        let mut world: Vec<Option<M4>> = vec![None; skeleton.len()];
        for (&node, m) in doll.nodes.iter().zip(bones) {
            world[node] = Some(M4(m.0.map(|r| [r.x(), r.y(), r.z(), r.w()])).mul(back));
        }
        let fallback = self.rig.world();
        for j in 0..skeleton.len() {
            placed_under(j, skeleton, &locals, &fallback, &mut world, 0);
        }
        world.into_iter().map(|m| m.unwrap_or(M4::IDENTITY)).collect()
    }
}

/// A node not driven by a bone, under its parent; a root keeps the riding pose's place.
fn placed_under(
    j: usize,
    skeleton: &crate::skin::Skeleton,
    locals: &[M4],
    fallback: &[M4],
    world: &mut [Option<M4>],
    depth: usize,
) -> M4 {
    if let Some(m) = world[j] {
        return m;
    }
    let m = match skeleton.bones[j].parent {
        Some(p) if p != j && p < world.len() && depth < world.len() => {
            locals[j].mul(placed_under(p, skeleton, locals, fallback, world, depth + 1))
        }
        _ => fallback.get(j).copied().unwrap_or(M4::IDENTITY),
    };
    world[j] = Some(m);
    m
}

/// The inverse of a rotation and a translation.
fn rigid_inverse(m: M4) -> M4 {
    let r = m.0;
    let t = [r[3][0], r[3][1], r[3][2]];
    let mut out = [[0.0f32; 4]; 4];
    for i in 0..3 {
        for k in 0..3 {
            out[i][k] = r[k][i];
        }
    }
    for k in 0..3 {
        out[3][k] = -(t[0] * out[0][k] + t[1] * out[1][k] + t[2] * out[2][k]);
    }
    out[3][3] = 1.0;
    M4(out)
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
