//! **Knockable props**: the scene models a track's `.toy` turns into loose rigid bodies — free09's
//! beach balls, crates, barrels and road signs, the supercross bales and banners.
//!
//! The `.toy`'s `[PhysicsCollections]` lists them by name ([`entries`]). The loader walks that list
//! once, in file order, and for each entry claims scene models by their instance name, which is the
//! model's `SLT=` as the scene spells it ([`claim`]):
//!
//! * **a plain name** (`BeachBall`) loads `<track dir><name>.pco` and gives every unclaimed model of
//!   exactly that name (case aside) a collection of its own — or, for `Type = group`, one collection
//!   for the lot;
//! * **a name with a `*`** (`sign_*`) cuts it at the star: the file is `<track dir><prefix>.pco`, and
//!   every unclaimed model whose name starts with the prefix is handed to the plain-name step under its
//!   own full name, so the four `Sign_SpeedLmt` go together, then the two `Sign_RoadEnds`, and so on.
//!   Without a `Type` such an entry is a group, not a single.
//!
//! A collection is the `.pco` ([`Pco`]): `[PhysicsParts]` gives its world's constraint passes and
//! substep rate, and each `[PhysicsPartN]` of `Type = RigidBody` becomes one body — the generic
//! physics object the wreck bike is ([`crate::wreck_bike`]), placed where the model stands, its mass
//! and damping from [`Part::read`], its inertia from the model's box through [`Part::inertia`]
//! ([`bodies`]). The `[ConstraintN]` sections are read as written ([`Constraint`]).
//!
//! A model is claimed once: by the first collection that loads a body for it. A group takes a second
//! model of its name only while it is one part and no constraints; any other model of the name is left
//! for a later entry.
//!
//! Each body also loads two collision files named off its node, the instance it stands on
//! ([`rays_file`], [`shape_file`]): `<node>m.col`, a ray probe, and `<node>f.col`, the mesh other
//! things strike. A prop that has neither does not collide.
//!
//! What is not here, and is the follow-up's: the collection world's step, the constraints' solve, the
//! enable/disable and reset lifecycle, the props against the bike and rider, and their place in the
//! rollback snapshot. Also not here: a part with `Create = TRUE`, which no retail file has; [`claim`]
//! reports it rather than guessing.
//!
//! An entry no model is named after falls back on a **part search** ([`part_models`]): each part's
//! model is found by the part's own `Name`, part 0's model's `RefName` giving the suffix the rest are
//! looked up by (nat07's `SnowmanA` is `Nat07_Sman_Head` `_1`, `Nat07_Sman_Hat_1`, …). It makes one
//! collection a time, for as long as every part is found — two snowmen of each kind on nat07.

#![deny(clippy::float_arithmetic)]

use crate::biplane::Body;
use crate::math::{Mat4, basis, mat4_to_quat};
use crate::phy::{Ini, Part, Section};
use crate::track::Placement;
use crate::wreck_bike::WreckBike;
use ps2_float::{F, Vf};

/// `[PhysicsParts] SimulationHz` when the file has none.
pub const DEFAULT_RATE: F = F(55.0);
/// `[PhysicsParts] ResetTime` when the file has none: never.
pub const NO_RESET: F = F(-1.0);

/// How a collection holds the models of its name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    /// One collection per model.
    Single,
    /// One collection for every model of the name.
    Group,
}

/// One `[CollectionN]` of the `.toy`, as written.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub name: String,
    /// The `Type` key as written, or `None` when the entry has none.
    pub kind: Option<String>,
}

impl Entry {
    /// The name up to its `*`, and whether it had one.
    pub fn prefix(&self) -> (&str, bool) {
        match self.name.find('*') {
            Some(at) => (&self.name[..at], true),
            None => (&self.name, false),
        }
    }

    /// The `Type` this entry is loaded as: the key, or `single` for a plain name and `group` for a
    /// starred one. A word that is neither loads nothing.
    pub fn kind(&self) -> Option<Kind> {
        let starred = self.prefix().1;
        match self.kind.as_deref() {
            None => Some(if starred { Kind::Group } else { Kind::Single }),
            Some(k) if k.eq_ignore_ascii_case("single") => Some(Kind::Single),
            Some(k) if k.eq_ignore_ascii_case("group") => Some(Kind::Group),
            Some(_) => None,
        }
    }

    /// The `.pco` this entry loads: `"%s%s.pco"` of the track's directory and the name cut at its
    /// star.
    pub fn pco(&self, track_dir: &str) -> String {
        format!("{track_dir}{}.pco", self.prefix().0)
    }
}

/// The `.toy`'s `[PhysicsCollections]`, in file order. A `.toy` without the section has none.
pub fn entries(toy: &Ini) -> Vec<Entry> {
    let head = toy.section("PhysicsCollections");
    let n = head.int("NumberOfPhysicsCollections").unwrap_or(0);
    (1..=n.max(0))
        .map(|i| {
            let s = toy.section(&format!("Collection{i}"));
            Entry { name: s.text("Name").unwrap_or("").to_string(), kind: s.text("Type").map(str::to_string) }
        })
        .collect()
}

/// One `[PhysicsPartN]`.
#[derive(Clone, Debug, PartialEq)]
pub struct PartEntry {
    /// `Type`: only `RigidBody` makes a body.
    pub kind: String,
    /// `Name`: the child node of the instance the body stands on, when the instance has one.
    pub name: String,
    /// `Create`: a body placed by the part's own `Position`, `Look` and `Up` rather than its node's.
    pub create: bool,
    pub part: Part,
}

impl PartEntry {
    pub fn rigid(&self) -> bool {
        self.kind.eq_ignore_ascii_case("RigidBody")
    }
}

/// One `[ConstraintN]`, as written. The angles are degrees, as the file has them.
#[derive(Clone, Debug, PartialEq)]
pub enum Constraint {
    /// `PinToBody`: two parts pinned together.
    PinToBody {
        name1: String,
        name2: String,
        body_pin_position1: Vf,
        body_pin_position2: Vf,
        disable_part_collision: bool,
        constrain_angles: bool,
        min_angles: Vf,
        max_angles: Vf,
    },
    /// `Pin`: a part pinned to where it stands (the road signs, the banners).
    Pin { name: String, pin_position: Vf, body_pin_position: Vf, constrain_angles: bool, min_angles: Vf, max_angles: Vf },
    /// `BodyTorsionSpring`: a spring turning a part back (the banners). Its own keys are its
    /// reader's, read here as plain vectors.
    BodyTorsionSpring { name: String, spring: Vf, damping: Vf },
    /// Any other `Type`, which loads nothing.
    Other(String),
}

impl Constraint {
    pub fn read(s: Section) -> Constraint {
        let text = |k: &str| s.text(k).unwrap_or("").to_string();
        let v = |k: &str| s.vector(k, Vf::VF0);
        let kind = text("Type");
        if kind.eq_ignore_ascii_case("PinToBody") {
            Constraint::PinToBody {
                name1: text("Name1"),
                name2: text("Name2"),
                body_pin_position1: v("BodyPinPosition1"),
                body_pin_position2: v("BodyPinPosition2"),
                disable_part_collision: s.flag("DisablePartCollision", false),
                constrain_angles: s.flag("ConstrainAngles", false),
                min_angles: v("MinAngles"),
                max_angles: v("MaxAngles"),
            }
        } else if kind.eq_ignore_ascii_case("Pin") {
            Constraint::Pin {
                name: text("Name"),
                pin_position: v("PinPosition"),
                body_pin_position: v("BodyPinPosition"),
                constrain_angles: s.flag("ConstrainAngles", false),
                min_angles: v("MinAngles"),
                max_angles: v("MaxAngles"),
            }
        } else if kind.eq_ignore_ascii_case("BodyTorsionSpring") {
            Constraint::BodyTorsionSpring { name: text("Name"), spring: v("Spring"), damping: v("Damping") }
        } else {
            Constraint::Other(kind)
        }
    }
}

/// A `.pco`.
#[derive(Clone, Debug, PartialEq)]
pub struct Pco {
    /// `NumberOfIterations` (1) and `SimulationHz` (55): the collection world's two numbers.
    pub iterations: i32,
    pub rate: F,
    /// `[PhysicsParts] ResetTime`, -1 when absent. Each part's own `ResetTime` is in its [`Part`].
    pub reset_time: F,
    pub parts: Vec<PartEntry>,
    pub constraints: Vec<Constraint>,
}

impl Pco {
    pub fn read_bytes(bytes: &[u8]) -> Pco {
        Pco::read(&Ini::parse_bytes(bytes))
    }

    pub fn read(ini: &Ini) -> Pco {
        let head = ini.section("PhysicsParts");
        let parts = head.int("NumberOfPhysicsParts").unwrap_or(0);
        let constraints = head.int("NumberOfConstraints").unwrap_or(0);
        Pco {
            iterations: head.int("NumberOfIterations").unwrap_or(1),
            rate: head.float_or("SimulationHz", DEFAULT_RATE),
            reset_time: head.float_or("ResetTime", NO_RESET),
            parts: (1..=parts.max(0))
                .map(|i| {
                    let s = ini.section(&format!("PhysicsPart{i}"));
                    PartEntry {
                        kind: s.text("Type").unwrap_or("").to_string(),
                        name: s.text("Name").unwrap_or("").to_string(),
                        create: s.int("Create").unwrap_or(0) != 0,
                        part: Part::read(s),
                    }
                })
                .collect(),
            constraints: (1..=constraints.max(0))
                .map(|i| Constraint::read(ini.section(&format!("Constraint{i}"))))
                .collect(),
        }
    }

    /// One part and no constraints: the shape a group may take a second model into.
    pub fn one_body(&self) -> bool {
        self.parts.len() == 1 && self.constraints.is_empty()
    }
}

/// One body of a collection: the scene model it stands on and the part it is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Member {
    /// The model's index in the scene's [`crate::track::Scene::models`].
    pub model: usize,
    /// The part's index in the collection's [`Pco::parts`].
    pub part: usize,
}

/// One collection, as the loader leaves it.
#[derive(Clone, Debug, PartialEq)]
pub struct Collection {
    /// The name the collection was created under: the model's full name for a starred entry.
    pub name: String,
    /// The `.pco` path it read.
    pub pco_file: String,
    pub kind: Kind,
    pub pco: Pco,
    /// Set when the first model attached was the only part and there are no constraints: what lets
    /// a group take more.
    pub one_body: bool,
    pub members: Vec<Member>,
}

/// What [`claim`] could not load, and why. No retail track has one.
#[derive(Clone, Debug, PartialEq)]
pub enum Unported {
    /// An entry no model is named after whose `.pco` is missing or has no parts. The original's part
    /// search passes such a file without finding anything, so its loader makes empty collections of
    /// it forever; the port makes none and says so.
    EndlessSearch { entry: String },
    /// A part with `Create = TRUE`.
    Create { entry: String, part: String },
}

/// The loader's walk: every `.toy` entry in order, claiming the scene's models.
///
/// `track_dir` is the track's directory as the paths are formatted (`Tracks\Free09\`); `pco` hands
/// back a file's bytes by that path, or `None` when there is no such file — which loads nothing for
/// that entry. Returns the collections in the order they were made, which is the order the original's
/// collection list runs.
pub fn claim(
    toy: &[Entry],
    track_dir: &str,
    models: &[Placement],
    pco: &mut dyn FnMut(&str) -> Option<Vec<u8>>,
) -> (Vec<Collection>, Vec<Unported>) {
    let mut claimed = vec![false; models.len()];
    let mut out = Vec::new();
    let mut unported = Vec::new();
    for entry in toy {
        let Some(kind) = entry.kind() else { continue };
        let file = entry.pco(track_dir);
        let (prefix, starred) = entry.prefix();
        if !starred {
            create(kind, &file, &entry.name, models, &mut claimed, pco, &mut out, &mut unported);
            continue;
        }
        for i in 0..models.len() {
            if claimed[i] {
                continue;
            }
            let name = &models[i].file;
            let starts = name.len() >= prefix.len()
                && name.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes());
            if starts {
                let name = name.clone();
                create(kind, &file, &name, models, &mut claimed, pco, &mut out, &mut unported);
            }
        }
    }
    (out, unported)
}

/// The plain-name step: collections of `name` out of `file`, as `kind` says.
#[allow(clippy::too_many_arguments)]
fn create(
    kind: Kind,
    file: &str,
    name: &str,
    models: &[Placement],
    claimed: &mut [bool],
    pco: &mut dyn FnMut(&str) -> Option<Vec<u8>>,
    out: &mut Vec<Collection>,
    unported: &mut Vec<Unported>,
) {
    let fresh = || Collection {
        name: name.to_string(),
        pco_file: file.to_string(),
        kind,
        pco: Pco { iterations: 1, rate: DEFAULT_RATE, reset_time: NO_RESET, parts: Vec::new(), constraints: Vec::new() },
        one_body: false,
        members: Vec::new(),
    };
    let named = |i: usize| models[i].file.eq_ignore_ascii_case(name);
    match kind {
        Kind::Single => {
            let mut any = false;
            for i in 0..models.len() {
                if claimed[i] || !named(i) {
                    continue;
                }
                any = true;
                let mut c = fresh();
                if attach(&mut c, i, models, pco, unported) {
                    claimed[i] = true;
                    out.push(c);
                }
            }
            // With no model of the name, the original goes looking for each part's model by the
            // part's own name: a collection a time, for as long as the search finds every part. (A
            // starred entry never gets here: it names a model it has just seen.)
            if !any {
                loop {
                    let Some(file_parts) = pco(file).map(|b| Pco::read_bytes(&b)).filter(|f| !f.parts.is_empty())
                    else {
                        unported.push(Unported::EndlessSearch { entry: name.to_string() });
                        break;
                    };
                    let mut look = claimed.to_vec();
                    if part_models(&file_parts, models, &mut look, false).iter().any(Option::is_none) {
                        break;
                    }
                    let mut c = fresh();
                    attach_parts(&mut c, models, claimed, pco, unported);
                    out.push(c);
                }
            }
        }
        Kind::Group => {
            let mut c = fresh();
            let mut any = false;
            for i in 0..models.len() {
                if claimed[i] || !named(i) {
                    continue;
                }
                if attach(&mut c, i, models, pco, unported) {
                    claimed[i] = true;
                    any = true;
                }
            }
            if any {
                out.push(c);
            }
        }
    }
}

/// One model into a collection: the `.pco` read again, and a body per rigid part. True when any part
/// made a body, which is what claims the model.
fn attach(
    c: &mut Collection,
    model: usize,
    models: &[Placement],
    pco: &mut dyn FnMut(&str) -> Option<Vec<u8>>,
    unported: &mut Vec<Unported>,
) -> bool {
    if !c.members.is_empty() && !(c.one_body && models[model].file.eq_ignore_ascii_case(&c.name)) {
        return false;
    }
    let Some(bytes) = pco(&c.pco_file) else { return false };
    let file = Pco::read_bytes(&bytes);
    if c.members.is_empty() && file.one_body() {
        c.one_body = true;
    }
    let mut any = false;
    for (p, entry) in file.parts.iter().enumerate() {
        if !entry.rigid() {
            continue;
        }
        if entry.create {
            unported.push(Unported::Create { entry: c.name.clone(), part: entry.name.clone() });
            continue;
        }
        c.members.push(Member { model, part: p });
        any = true;
    }
    c.pco = file;
    any
}

/// The part search's attach: a collection with no model to stand on finds each part's model by
/// [`part_models`], claiming them as it goes, and makes a body of every rigid part it found. A
/// collection made this way is kept whatever it found; it is never a one-body collection.
fn attach_parts(
    c: &mut Collection,
    models: &[Placement],
    claimed: &mut [bool],
    pco: &mut dyn FnMut(&str) -> Option<Vec<u8>>,
    unported: &mut Vec<Unported>,
) {
    let Some(bytes) = pco(&c.pco_file) else { return };
    let file = Pco::read_bytes(&bytes);
    let found = part_models(&file, models, claimed, true);
    for (p, (entry, model)) in file.parts.iter().zip(found).enumerate() {
        let Some(model) = model else { continue };
        if !entry.rigid() {
            continue;
        }
        if entry.create {
            unported.push(Unported::Create { entry: c.name.clone(), part: entry.name.clone() });
            continue;
        }
        c.members.push(Member { model, part: p });
    }
    c.pco = file;
}

/// The model each part of `file` stands on when its collection has none, as the part search finds
/// them, in part order (`None` where none is found):
///
/// * until a suffix is known, a part's model is the first unclaimed model whose name is the part's
///   `Name` (case aside);
/// * part 0's model, found so, gives the suffix: its `RefName` past the part's name, when the
///   `RefName` starts with it (case counts here) — `Nat07_Sman_Head_1` gives `_1`;
/// * with a suffix, a part's model is the first model whose `RefName` is the part's name and the
///   suffix (case aside), claimed or not.
///
/// `mark` claims each model as it is found, which is the attach; the search that decides whether
/// to attach at all marks nothing.
pub fn part_models(file: &Pco, models: &[Placement], claimed: &mut [bool], mark: bool) -> Vec<Option<usize>> {
    let mut suffix: Option<String> = None;
    let mut out = Vec::with_capacity(file.parts.len());
    for (p, part) in file.parts.iter().enumerate() {
        let found = match &suffix {
            Some(tail) => {
                let full = format!("{}{tail}", part.name);
                models.iter().position(|m| m.ref_name.as_deref().is_some_and(|r| r.eq_ignore_ascii_case(&full)))
            }
            None => {
                let found = (0..models.len()).find(|&i| !claimed[i] && models[i].file.eq_ignore_ascii_case(&part.name));
                if p == 0
                    && let Some(i) = found
                    && let Some(r) = models[i].ref_name.as_deref()
                    && let Some(tail) = r.strip_prefix(part.name.as_str())
                {
                    suffix = Some(tail.to_string());
                }
                found
            }
        };
        if mark && let Some(i) = found {
            claimed[i] = true;
        }
        out.push(found);
    }
    out
}

/// The ray probe's file of a body standing on node `node` (`"%sm.col"`, after any extension the
/// name has is cut off).
pub fn rays_file(node: &str) -> String {
    format!("{}m.col", stem(node))
}

/// The collidable mesh's file of a body standing on node `node` (`"%sf.col"`).
pub fn shape_file(node: &str) -> String {
    format!("{}f.col", stem(node))
}

/// The name up to its last `.`, or all of it when it has none.
fn stem(name: &str) -> &str {
    name.rfind('.').map_or(name, |at| &name[..at])
}

/// One prop's body, made.
#[derive(Clone, Debug, PartialEq)]
pub struct Prop {
    /// Which collection, and which of its members.
    pub collection: usize,
    pub member: usize,
    /// The body, at rest where its model stands.
    pub body: Body,
    /// The part's gravity force (`GravityScale` times the weight, down).
    pub gravity: Vf,
}

/// The origin a direction-only basis is built round.
const ORIGIN: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);

/// The frame the scene stands a model in: its look and up vectors through the engine's look-at with
/// the up kept and the look made square to it, which is how the scene's model reader sets the node.
/// The vectors' `w` is 1, the reader's default for a three-number line.
pub fn frame(at: &Placement) -> Mat4 {
    let p = |v: [F; 3], w: f32| Vf::new(v[0].0, v[1].0, v[2].0, w);
    basis(p(at.position, 1.0), p(at.look, 1.0), p(at.up, 1.0), true, false)
}

/// Where a body stands on its model, and which way it turns: the model's position (`w` 1), and
/// [`turn`] of its node's frame.
pub fn pose(at: &Placement) -> (Vf, Vf) {
    let m = frame(at);
    (m.0[3], turn(&m))
}

/// The quaternion a physics object starts at on its node: the node's world look and up rows, `w`
/// 0, through the original's axes-to-quaternion (a direction-only look-at, the look kept).
pub fn turn(node: &Mat4) -> Vf {
    let axis = |v: Vf| Vf::new(v.x(), v.y(), v.z(), 0.0);
    mat4_to_quat(&basis(ORIGIN, axis(node.0[2]), axis(node.0[1]), false, true))
}

/// A scene node's box, as the node keeps it: its centre and half its size, both in the node's
/// own frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeBox {
    pub centre: Vf,
    pub half_extent: Vf,
}

/// The `.psg` file of the model a scene line names (`SLT=`), as the track's archive keeps it.
pub fn model_file(model: &str) -> String {
    format!("{model}.psg")
}

impl NodeBox {
    /// The box a body's node gets from its model, the `.psg`'s bytes: the node reader reads each
    /// node's 224-byte record (a 128-byte name, the 64-byte local matrix, then the centre and the
    /// half extent as three floats each, a radius and the parent's index), and the model's first
    /// node is the scene node itself — it copies the two vectors in and marks the node as having a
    /// box, every time. Only three lanes are read; the `w` lanes stay as the node was made, 1 in both.
    ///
    /// A model of more than one node would have its children's boxes merged into the span by
    /// [`box_size`]'s query, through their world matrices; no knockable prop's model on the disc has
    /// a second node, so that merge is not ported and such a model is an error.
    pub fn from_model(psg: &[u8]) -> Result<NodeBox, String> {
        const NODES: usize = 12;
        const RECORD: usize = 224;
        const CENTRE: usize = 192;
        const HALF: usize = 204;
        let word = |at: usize| -> Result<u32, String> {
            psg.get(at..at + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .ok_or_else(|| format!(".psg ends at {} before byte {at}", psg.len()))
        };
        if psg.get(..4) != Some(b"psg\0".as_slice()) {
            return Err("not a .psg: no psg\\0 at the front".into());
        }
        let count = word(8)?;
        if count != 1 {
            return Err(format!("a model of {count} nodes: the children's boxes are not merged"));
        }
        let three = |at: usize, w: f32| -> Result<Vf, String> {
            Ok(Vf::from_bits([word(at)?, word(at + 4)?, word(at + 8)?, w.to_bits()]))
        };
        Ok(NodeBox { centre: three(NODES + CENTRE, 1.0)?, half_extent: three(NODES + HALF, 1.0)? })
    }
}

/// The box the inertia formula takes: the node's bounds query, which spans the box as its corners
/// `centre ∓ half` and answers half their difference, doubled by the loader. A box whose centre is
/// off zero comes back a rounding away from twice its half extent — which is the egg's inertia. A
/// node with no box of its own spans nothing (`None`). Children's boxes would widen the span; no
/// prop node in free09 has a child.
pub fn box_size(node: Option<NodeBox>) -> Vf {
    let (lo, hi) = match node {
        Some(b) => (b.centre.sub(b.half_extent), b.centre.add(b.half_extent)),
        None => (Vf::VF0, Vf::VF0),
    };
    hi.sub(lo).mul(Vf::splat(0.5)).mul(Vf::splat(2.0))
}

/// Every collection's bodies, in collection order and member order within one.
///
/// `node_box` is a member's model node's box ([`NodeBox`], or `None` for a node with none): what
/// [`NodeBox::from_model`] reads off the model's `.psg`, which the caller fetches from the disc.
pub fn bodies(
    collections: &[Collection],
    models: &[Placement],
    node_box: &mut dyn FnMut(Member) -> Option<NodeBox>,
) -> Vec<Prop> {
    let mut out = Vec::new();
    for (c, coll) in collections.iter().enumerate() {
        for (m, &member) in coll.members.iter().enumerate() {
            let part = &coll.pco.parts[member.part].part;
            let size = box_size(node_box(member));
            let (position, quat) = pose(&models[member.model]);
            let made = WreckBike::new(part, part.inertia(size), position, quat);
            out.push(Prop { collection: c, member: m, body: made.body, gravity: made.gravity });
        }
    }
    out
}
