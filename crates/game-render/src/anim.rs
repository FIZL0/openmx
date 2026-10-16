//! **The animation seam: the simulation's clock driving the disc's own clips on a real skeleton.**
//!
//! Three halves used to meet here and none of them touched. The simulation ported the *driving* —
//! which motion a state plays, how far into it the frame is, how two of them blend
//! ([`game_sim::motion`], S6 and S20) — and declared a clip as nothing but a duration, a loop flag
//! and a list of action-frame times. The renderer ported the *look* — the `.mot` file, its channels,
//! the quaternion blend, the pose on a model ([`crate::skin`], V5) — and declared a clip as a file.
//! Between them sat [`game_sim::motion::Animations`], the trait the simulation declares because it
//! may not read a file, with a stub on each side and no implementation anywhere. And the joint map
//! the original builds when it *binds* a clip to a skeleton — the byte per channel name that says
//! which joint of this model that channel drives — was in neither: the renderer could compute one
//! ([`crate::skin::Clip::joints`]) but nothing held it, and the simulation's clip table held
//! handles that pointed at nothing.
//!
//! This module is the join. [`Set`] is one animation set — one `.mcf`, one skeleton, the archives it
//! reads clips out of — and it is the port's one implementation of `Animations`: `load` reads a
//! clip off the user's disc and **binds** it, which is where the joint map is built and kept
//! ([`Bound`]); `bound` answers with the handle the simulation's [`game_sim::motion::Library`]
//! stores. [`Rig`] is that set with the simulation's driver, clip table and clock beside it, so one
//! call is one frame of animation: the selection, the clock, the sample, the pose, the bones.
//!
//! ## How a name becomes a file
//!
//! The simulation asks for a clip by name — `"<prefix>bike_<stunt name>"` for a gesture slot,
//! `bike_air_Fwd` / `_Mid` / `_Bck` for the three airborne poses. The original's loader looks in the
//! animation set first and falls back on the disc, and so does [`Set::member`]:
//!
//! 1. the set's own `[MOTION n]` whose `NAME=` is that name, which gives a `VUE=`. The written
//!    extension is the editors' name for the format and not the member's — a rider's say `.MOT` and
//!    the UI bike's say `.vue`, and both are `.mot` members — so it is replaced;
//! 2. else the name itself, as `<ContentDirectory><name>.mot`;
//! 3. else any member ending in `<name>.mot`, whatever directory it is in.
//!
//! That second rule is not a guess. A bike archive's `bike.mcf` lists six motions, all airborne, yet
//! the archive also carries `bike\anim\bike_kod.mot`, `bike_nacnac.mot`, `bike_taunt.mot` and five
//! more — the stunt clips the bike itself animates — which are in no `.mcf` at all. They are found
//! by name, and `stunts.ini`'s `UsedForBike` flag is true for exactly the stunts whose clip the
//! archive ships: the two agree slot for slot on all 24 gesture slots, which is what
//! `tests/anim.rs` asserts. `UsedForBike` is the index of this unlisted set.
//!
//! ## What a frame does
//!
//! [`Rig::frame`] runs the simulation's own order and adds the drawing to the end of it:
//!
//! ```text
//!   Driver::frame      the ramp, the restart test, the latch, the selection      -> a Blend
//!   Set::rewind        on a restart: the pose the cross-fade will fade from
//!   Player::run        the clock: the dt advance, or the phase-driven sample     -> a Played
//!   Set::apply         that Blend's clips, sampled at Player::sample_time        -> a Pose
//!   Skeleton::world    the pose down the tree                                    -> a matrix a joint
//! ```
//!
//! Everything above `Set::apply` is ported and proved against the original (S6, S20); everything
//! below it is proved to a tolerance against the original's own samplers (V5). What this module adds
//! is the wiring, and the wiring is proved only against the disc — see V7's journal entry for the
//! list of what that does not cover, in particular [`Set::apply`]'s mapping from a blend mode to a
//! sampler call, which is the renderer's reading of the original's four blend entries and not a
//! transliteration of them.

use crate::disc::Res;
use crate::ini;
use crate::mat::M4;
use crate::psg::Model;
use crate::rider::MotionSet;
use crate::skin::{Clip, Pose, SKIP, Skeleton};
use game_sim::motion::{
    Animations, Blend, ClipId, Driver, Frame, Library, Params, Played, Player, Request, Slot,
    Snapshot,
};
use game_sim::trick::Stunt;
use ps2_float::F;

/// A clip bound to a skeleton: the file, and the map from its channel names to that skeleton's
/// joints.
///
/// The map is the thing the original builds at bind time and the port had nowhere to put. It is one
/// byte per name in the clip's own name table — the joint index, or [`SKIP`] where this skeleton has
/// no joint of that name — and every sampler goes through it, which is how one clip can drive a
/// rider and a bike that number their joints differently.
#[derive(Clone, Debug)]
pub struct Bound {
    /// The name the simulation asked for, which is the `NAME=` of the set's entry or the stem of the
    /// member when there is no entry.
    pub name: String,
    /// The archive member the clip was read from.
    pub member: String,
    /// The parsed `.mot`, carrying the simulation's timing record as its `timing` field.
    pub clip: Clip,
    /// One joint index per name in [`Clip::names`], [`SKIP`] where the skeleton has none.
    pub joints: Vec<u8>,
}

impl Bound {
    /// How many of the clip's channel names are joints of the skeleton it was bound to. A clip and a
    /// model that do not belong together show up here as a small number.
    pub fn named(&self) -> usize {
        self.joints.iter().filter(|&&j| j != SKIP).count()
    }
}

/// Why a clip the simulation asked for is not bound.
///
/// The original has one answer for all of these — a null handle, which makes the slot fall back on
/// the neutral pose — so nothing downstream behaves differently per reason. They are kept apart
/// because a port that cannot say *why* a clip is missing is a port nobody can debug.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Missing {
    /// No member of any archive answers to that name.
    NoMember(String),
    /// A member was found and is not a clip this port reads.
    Unreadable(String, String),
}

/// One animation set: a `.mcf`, a skeleton, and the clips of that set bound to it.
///
/// This is the port's implementation of [`Animations`], and the only thing in it that reads a file.
/// It owns its archives because the trait's `load` takes nothing but a name — which is the point of
/// the trait: the simulation names a clip and knows nothing about where it comes from.
pub struct Set {
    /// The `.mcf` the set was built from: its motion list, its content directory, its geometry.
    pub set: MotionSet,
    /// The model's node tree, which is what a clip's channel names are resolved against.
    pub skeleton: Skeleton,
    /// The clips bound so far, in the order they were bound. A [`ClipId`] is an index into this.
    pub clips: Vec<Bound>,
    /// The live pose: what the last [`Set::apply`] wrote, and the one buffer of joint poses in the
    /// port. The simulation reads it through [`game_sim::motion::Joints`] and never holds it.
    pub pose: Pose,
    /// The pose a cross-fade fades *from*, which [`Set::rewind`] copies out of `pose`.
    pub under: Pose,
    /// Every name that was asked for and could not be bound, in the order they were asked for.
    pub missing: Vec<Missing>,
    archives: Vec<Res>,
}

impl Set {
    /// An animation set from an already-read `.mcf` and an already-read model.
    pub fn new(set: MotionSet, skeleton: Skeleton, archives: Vec<Res>) -> Set {
        let joints = skeleton.len();
        Set {
            set,
            skeleton,
            clips: Vec::new(),
            pose: Pose::new(joints),
            under: Pose::new(joints),
            missing: Vec::new(),
            archives,
        }
    }

    /// Follow a `.mcf` to its own geometry and build the set: the motion list, the `SLTFile`'s
    /// `.psg` as the skeleton, and the archives to read clips out of.
    ///
    /// `mcf` is the member name of the `.mcf` — `bike\anim\bike.mcf`, `rider\anim\mrider.mcf`. Which
    /// one a rider uses is [`crate::rider::Rider::load`]'s question, not this module's.
    pub fn load(archives: Vec<Res>, mcf: &str) -> Result<Set, String> {
        let (_, bytes) =
            find(&archives, mcf).ok_or_else(|| format!("no archive holds {mcf}"))?;
        let bytes = bytes.map_err(|e| format!("{mcf}: {e}"))?;
        let set = MotionSet::parse(&ini::text(&bytes));
        let geometry = set.geometry();
        if geometry.is_empty() {
            return Err(format!("{mcf}: no [General info] SLTFile"));
        }
        // The `.psg` is named the way the scene names its models — a stem, no directory — so it is
        // looked for under the set's own content directory first and then anywhere in the archive.
        let psg = format!("{}{geometry}.psg", set.directory);
        let (psg, bytes) = find(&archives, &psg)
            .ok_or_else(|| format!("{mcf} names {geometry}.psg, which no archive has"))?;
        let bytes = bytes.map_err(|e| format!("{psg}: {e}"))?;
        let model = Model::parse(&bytes).map_err(|e| format!("{psg}: {e}"))?;
        Ok(Set::new(set, Skeleton::of(&model), archives))
    }

    /// The archive members a clip of this name could be, in the order the loader tries them. See the
    /// module's own docs for why there are three.
    pub fn member(&self, name: &str) -> Vec<String> {
        let dir = &self.set.directory;
        let mut out = Vec::new();
        if let Some(vue) = self.set.clip(name) {
            let stem = vue.rsplit_once('.').map_or(vue, |(stem, _)| stem);
            out.push(format!("{dir}{stem}.mot"));
        }
        let named = format!("{dir}{name}.mot");
        if !out.contains(&named) {
            out.push(named);
        }
        out
    }

    /// The handle of a clip already bound, by the name it was bound under.
    pub fn id(&self, name: &str) -> Option<ClipId> {
        let at = self.clips.iter().position(|c| c.name.eq_ignore_ascii_case(name))?;
        Some(ClipId(at as u32))
    }

    /// A bound clip by its handle.
    pub fn get(&self, id: ClipId) -> Option<&Bound> {
        self.clips.get(id.0 as usize)
    }

    /// The timing record behind a handle, which is what the simulation's clock runs on. A handle
    /// that names nothing answers with a clip of no length, which is what the clock does with a
    /// missing clip anyway.
    pub fn timing(&self, id: Option<ClipId>) -> game_sim::motion::Clip {
        id.and_then(|id| self.get(id)).map_or_else(Default::default, |b| b.clip.timing.clone())
    }

    /// Take the snapshot a restarted motion fades from: the simulation's record of the pose, and the
    /// renderer's copy of it, from the one live pose and in one call so the two cannot drift.
    ///
    /// The original asks the motion object for this from two places — a restart in the frame's
    /// header, and [`Player::play`] when a transition has no clip to come out of — and both end in
    /// the same joint-by-joint copy, which is [`Snapshot::take`].
    pub fn rewind(&mut self, snapshot: &mut Snapshot) {
        snapshot.take(&self.pose, None, 0);
        self.under.rot.clone_from(&self.pose.rot);
        self.under.pos.clone_from(&self.pose.pos);
        self.under.turned.clear();
        self.under.turned.resize(self.pose.rot.len(), true);
        self.under.moved.clear();
        self.under.moved.resize(self.pose.pos.len(), true);
    }

    /// Load and bind every motion the `.mcf` itself lists, in its own order.
    ///
    /// This is the set loader's half of the job and it comes first: a bike's `.mcf` lists the three
    /// airborne poses and nothing else, and [`Library::load`] only ever *asks* for those three — it
    /// loads the gesture slots. So a clip table filled without this has three empty airborne slots
    /// and an animation that can only ever show a stunt.
    ///
    /// Only the first `NumberOfMotions` are loaded, which is [`MotionSet::motions`]: the sections
    /// past the count name clips no archive holds.
    pub fn load_listed(&mut self) {
        let names: Vec<String> = self.set.motions.iter().map(|m| m.name.clone()).collect();
        for name in names {
            self.load(&name);
        }
    }

    /// Write [`Set::pose`] from the blend the selection asked for, at the clip time the clock
    /// reached.
    ///
    /// Which sampler each blend mode means:
    ///
    /// - [`Blend::Free`] — the clip playing by itself, sampled at its own clock, no cross-fade. The
    ///   original's mode 0 makes no call into the blend entries at all; this is the advance's own
    ///   pose. A clip that has finished is not resampled, so it holds.
    /// - [`Blend::One`] — one clip at `phase` of its length, faded back towards [`Set::under`] by
    ///   `ramp` ([`Clip::pose_faded`]).
    /// - [`Blend::Two`] — two clips at the *first* one's clip time, mixed by `mix`, and the result
    ///   faded by `ramp` ([`Clip::pose_mixed_faded`]).
    /// - [`Blend::Three`] — four clips and three weights. Not drawn: no arm of the rider's own
    ///   selection asks for it (S6, S20), the nesting of two mixes into a third is not ported, and
    ///   inventing it would be arithmetic nobody has checked. The pose is left as it was.
    ///
    /// A blend whose clips are not all bound draws nothing and leaves the pose alone, which is the
    /// same frame the simulation's clock skips.
    pub fn apply(&mut self, blend: Blend, lib: &Library, player: &Player) -> bool {
        let time = player.sample_time(blend).0;
        let clip = |set: &Set, slot: Slot| -> Option<usize> {
            lib.clip(slot).map(|id| id.0 as usize).filter(|&at| at < set.clips.len())
        };
        match blend {
            Blend::Free => {
                // A clip that has run out holds the pose it ended on. The advance stops stepping a
                // finished clip and resets its clock to zero on the way out, so sampling at
                // `sample_time` here would snap the rider back to the clip's first frame — which is
                // not what "a finished clip holds its last pose" means. See the journal.
                if player.finished {
                    return false;
                }
                let Some(at) = player.clip.map(|id| id.0 as usize).filter(|&at| at < self.clips.len())
                else {
                    return false;
                };
                let bound = &self.clips[at];
                bound.clip.pose(time, &bound.joints, &mut self.pose);
                true
            }
            Blend::One { ramp, clip: slot, .. } => {
                let Some(at) = clip(self, slot) else { return false };
                let bound = &self.clips[at];
                bound.clip.pose_faded(time, &bound.joints, ramp.0, &self.under, &mut self.pose);
                true
            }
            Blend::Two { mix, ramp, a, b, .. } => {
                let (Some(x), Some(y)) = (clip(self, a), clip(self, b)) else { return false };
                let (first, second) = (&self.clips[x], &self.clips[y]);
                first.clip.pose_mixed_faded(
                    &second.clip,
                    time,
                    mix.0,
                    &first.joints,
                    ramp.0,
                    &self.under,
                    &mut self.pose,
                );
                true
            }
            Blend::Three { .. } => false,
        }
    }

    /// Every joint's transform under the live pose — one matrix per joint, which is what places a
    /// vertex ([`crate::skin::skin`]).
    pub fn world(&self) -> Vec<M4> {
        self.skeleton.world(&self.pose)
    }
}

/// The loader the simulation's clip table is filled through.
///
/// `load` is where a clip stops being a file and becomes a clip *of this skeleton*: it is read,
/// parsed, given the set's own loop flag, and bound — and binding is building the joint map. `bound`
/// is the lookup afterwards, and it answers `None` for both of the original's ways of having no
/// clip, because the original stores the same null for both.
impl Animations for Set {
    fn load(&mut self, name: &str) {
        if self.id(name).is_some() {
            return;
        }
        let found = self
            .member(name)
            .into_iter()
            .find_map(|want| exact(&self.archives, &want))
            .or_else(|| suffix(&self.archives, &format!("{name}.mot")));
        let Some((member_name, read)) = found else {
            self.missing.push(Missing::NoMember(name.to_string()));
            return;
        };
        let clip = read.and_then(|b| Clip::parse(&b));
        let mut clip = match clip {
            Ok(clip) => clip,
            Err(why) => {
                self.missing.push(Missing::Unreadable(member_name, why));
                return;
            }
        };
        // The loop flag is the animation set's and not the file's: a `.mot` does not carry one.
        clip.timing.looping = self.set.motions.iter().any(|m| m.name.eq_ignore_ascii_case(name) && m.looping);
        let joints = clip.joints(&self.skeleton);
        self.clips.push(Bound { name: name.to_string(), member: member_name, clip, joints });
    }

    fn bound(&self, name: &str) -> Option<ClipId> {
        self.id(name)
    }
}

/// A rider's or a vehicle's whole animation: the disc's clips, the simulation's driving, and the
/// skeleton they land on, in one value.
///
/// The three pieces were ported separately and this is where they meet. Nothing here decides
/// anything: [`Driver`] decides which motion plays, [`Player`] decides where in it the frame is, and
/// [`Set`] decides which bytes and which bones. What [`Rig::frame`] adds is the order, which is the
/// original's.
pub struct Rig {
    /// The animation set, its clips and the live pose.
    pub set: Set,
    /// The clip table the selection names its clips through, as the loader filled it.
    pub lib: Library,
    /// The animation driving: the latched action id, the cross-fade ramp, the shown crossup.
    pub driver: Driver,
    /// The motion object's clock.
    pub player: Player,
    /// The pose a cross-fade fades from, as the simulation records it.
    pub snapshot: Snapshot,
    /// The rider's own two numbers the driving reads.
    pub params: Params,
}

impl Rig {
    /// Build the clip table for a set: [`Library::load`] over the gesture slots, through this
    /// module's loader.
    ///
    /// `prefix` is the rider's own — empty for a race rider, `ff_` for the freestyle copy, which is
    /// what puts `ff_bike_nacnac` in front of `bike_nacnac`. `stunts` is `stunts.ini`'s table; only
    /// its first 24 records are gesture slots and only those are asked for.
    pub fn new(mut set: Set, prefix: &str, stunts: &[Stunt], params: Params) -> Rig {
        set.load_listed();
        let lib = Library::load(prefix, stunts, &mut set);
        Rig::bound(set, lib, params)
    }

    /// A rig whose clip table was filled by somebody else.
    ///
    /// [`Library::load`] names every clip after the *bike* — `<prefix>bike_<stunt name>` — because
    /// that is the actor the simulation's loader was read off. A rider's own clips are named after
    /// the rider ([`crate::rider::Body`]), so its table is built with the same two passes and
    /// different names, and handed over here.
    pub fn bound(set: Set, lib: Library, params: Params) -> Rig {
        Rig {
            set,
            lib,
            driver: Driver::default(),
            player: Player::default(),
            snapshot: Snapshot::default(),
            params,
        }
    }

    /// One frame: the selection, the clock, the sample, the pose.
    ///
    /// `lean` is the rider's three numbers the selection reads (`game_sim::motion::Pose`), and the
    /// caller is expected to have run `trick::Tricks::update` with the same [`Request`] first — the
    /// original computes the restart test once, at the top, and both halves read it.
    pub fn frame(&mut self, dt: F, r: Request, lean: game_sim::motion::Pose) -> (Frame, Played) {
        let frame = self.driver.frame(dt, self.params, r, lean, self.lib.present());
        (frame, self.driven(dt, frame).0)
    }

    /// The rest of a frame, for a rig whose driving was done elsewhere: the rewind, the clock, the
    /// sample, the pose.
    ///
    /// The simulation's own rider runs a [`Driver`] for the crossup its landing test reads and
    /// keeps what it answered ([`game_sim::rider::Rider::animation`]). A rider drawn from that
    /// frame has had its selection made already, so this is the half that is left — and it is the
    /// same half [`Rig::frame`] runs, because it is the same code.
    ///
    /// Answers what the clock did, and whether the frame wrote a pose at all — a blend with no
    /// clip behind it leaves the one that was already there, which on the first frame of a run is
    /// the bind pose.
    pub fn driven(&mut self, dt: F, frame: Frame) -> (Played, bool) {
        if frame.restart {
            self.set.rewind(&mut self.snapshot);
        }
        let clip = self.set.timing(self.player.clock_clip(frame.blend, &self.lib));
        let played = self.player.run(dt, frame.blend, &self.lib, &clip);
        let posed = self.set.apply(frame.blend, &self.lib, &self.player);
        (played, posed)
    }

    /// Every joint's transform under the frame's pose.
    pub fn world(&self) -> Vec<M4> {
        self.set.world()
    }
}

/// The member of these archives that answers to a name, with the bytes behind it: the first archive
/// that has one spelled exactly that way, and failing that the first that has one *ending* that way.
///
/// The suffix pass is the third of the loader's three rules (see the module docs) and is also how a
/// `.mcf`'s `SLTFile=bike.Slt` finds its model: the names inside these files are written without a
/// directory as often as with one, and in whichever case and slash style the editor was set to.
fn find(archives: &[Res], name: &str) -> Option<(String, Result<Vec<u8>, String>)> {
    exact(archives, name).or_else(|| suffix(archives, name))
}

/// The first archive with a member spelled exactly that way, under the archive's own spelling of it
/// — the names inside these files agree with the directory on neither case nor slash.
fn exact(archives: &[Res], name: &str) -> Option<(String, Result<Vec<u8>, String>)> {
    for res in archives {
        if let Some(e) = res.entries().iter().find(|e| same(&e.name, name)) {
            let member = e.name.clone();
            return Some((member.clone(), res.read(&member)));
        }
    }
    None
}

/// The first archive with a member whose name *ends* that way, in whatever directory.
fn suffix(archives: &[Res], name: &str) -> Option<(String, Result<Vec<u8>, String>)> {
    for res in archives {
        if let Some(e) = res.find_suffix(name) {
            let member = e.name.clone();
            return Some((member.clone(), res.read(&member)));
        }
    }
    None
}

/// Names on this disc are spelled with either slash and in any case.
fn same(a: &str, b: &str) -> bool {
    a.replace('\\', "/").eq_ignore_ascii_case(&b.replace('\\', "/"))
}
