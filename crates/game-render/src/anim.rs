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
use crate::rider::{Motion, MotionSet};
use crate::skin::{Clip, OutOf, Pose, SKIP, Skeleton};
use game_sim::motion::{
    Animations, Blend, ClipId, Cross, Driver, Frame, Library, Params, Played, Player, Request, Slot,
    Snapshot,
};
use game_sim::trick::{SLOTS, Stunt};
use game_sim::vehicle_anim;
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
    /// Which file each clip *name* means, for every `.mcf` in these archives that animates the same
    /// content directory, in the archives' own member order and first claim winning. See
    /// [`Set::claims`] for why a set's own `.mcf` is not the only one that has a say.
    pub claimed: Vec<Motion>,
    /// The model's node tree, which is what a clip's channel names are resolved against.
    pub skeleton: Skeleton,
    /// The clips bound so far, in the order they were bound. A [`ClipId`] is an index into this.
    pub clips: Vec<Bound>,
    /// The live pose: what the last [`Set::apply`] wrote, and the one buffer of joint poses in the
    /// port. The simulation reads it through [`game_sim::motion::Joints`] and never holds it.
    pub pose: Pose,
    /// The pose a cross-fade fades *from*, which [`Set::rewind`] reads out of the node tree
    /// `pose` leaves behind ([`Skeleton::nodes`]) rather than out of `pose` itself.
    pub under: Pose,
    /// Every name that was asked for and could not be bound, in the order they were asked for.
    pub missing: Vec<Missing>,
    archives: Vec<Res>,
}

impl Set {
    /// An animation set from an already-read `.mcf` and an already-read model.
    pub fn new(set: MotionSet, skeleton: Skeleton, archives: Vec<Res>) -> Set {
        let joints = skeleton.len();
        let claimed = Set::claims(&archives, &set);
        Set {
            set,
            claimed,
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

    /// Every `[MOTION n]` of every `.mcf` in these archives that names the same content directory,
    /// in the archives' own member order, with the first claim of a name kept and later ones dropped.
    ///
    /// **Why another `.mcf` gets a say in what this one's clips are.** The original has *one*
    /// animation set for the whole game, and it is keyed by clip *name*: a `.mcf` walk loads each
    /// `NAME=` from the file its own `VUE=` points at, but only if no set has claimed that name yet,
    /// and a name already in the set is handed over as it is — whatever file it came from. So the
    /// first `.mcf` loaded decides what a name means for every motion object built after it, and a
    /// later `.mcf`'s `VUE=` for that name is never read. (The other way into the set, the
    /// load-on-demand path a clip table takes for a name no `.mcf` lists, does not read `VUE=` at
    /// all: it opens `<ContentDirectory><name>.MOT`.)
    ///
    /// On this disc that is the difference between the right pose and the wrong one on three clips.
    /// Every rider archive ships two sets — `airider.mcf` for the AI bodies and `mrider.mcf` for the
    /// player's — and they disagree about exactly three of their forty motions:
    ///
    /// | name | `airider.mcf` | `mrider.mcf` |
    /// | --- | --- | --- |
    /// | `turn_stop_Fwd` | `turn_stop_Mid.MOT` | `turn_stop_Fwd.MOT` |
    /// | `turn_stop_Bck` | `turn_stop_Mid.MOT` | `turn_stop_Bck.MOT` |
    /// | `turn_still_Bck` | `turn_still_Bck.MOT` | `turn_still_Mid.MOT` (its own line is commented out) |
    ///
    /// V12's capture of the original posing the *player's* rider — with NPC riders on the track, so
    /// the AI set was built first — agrees with the `airider.mcf` column on all three and with
    /// `mrider.mcf` on none of them, which is what says the AI set's claims are the ones that
    /// survive. Taking the archive's `.mcf`s in member order reproduces that, because `airider`
    /// sorts before `mrider` in every one of the 162 rider archives. V14's journal entry has the
    /// measurements and what this does not prove.
    fn claims(archives: &[Res], set: &MotionSet) -> Vec<Motion> {
        let mut out: Vec<Motion> = Vec::new();
        let members: Vec<(usize, String)> = archives
            .iter()
            .enumerate()
            .flat_map(|(i, res)| {
                res.entries()
                    .iter()
                    .filter(|e| e.name.to_ascii_lowercase().ends_with(".mcf"))
                    .map(move |e| (i, e.name.clone()))
            })
            .collect();
        for (i, member) in members {
            let Ok(bytes) = archives[i].read(&member) else { continue };
            let other = MotionSet::parse(&ini::text(&bytes));
            // A name only means a file *under the claimant's own content directory*, and this set
            // builds its paths out of its own; a set that animates something else — a bike's `.mcf`
            // beside a rider's in the same archive list — is none of this set's business.
            if !other.directory.eq_ignore_ascii_case(&set.directory) {
                continue;
            }
            for motion in other.motions {
                if !out.iter().any(|m| m.name.eq_ignore_ascii_case(&motion.name)) {
                    out.push(motion);
                }
            }
        }
        out
    }

    /// The entry that claimed a clip name, which is what says which file it is and whether it loops:
    /// the first `.mcf` of the archives to name it, and this set's own entry when the archives have
    /// no `.mcf` at all (a set built by hand, which is every fixture in the tests).
    fn claim(&self, name: &str) -> Option<&Motion> {
        self.claimed
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(name))
            .or_else(|| self.set.motions.iter().find(|m| m.name.eq_ignore_ascii_case(name)))
    }

    /// The archive members a clip of this name could be, in the order the loader tries them. See the
    /// module's own docs for why there are three.
    pub fn member(&self, name: &str) -> Vec<String> {
        let dir = &self.set.directory;
        let mut out = Vec::new();
        if let Some(vue) = self.claim(name).map(|m| m.clip.as_str()).filter(|v| !v.is_empty()) {
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
    /// renderer's copy of it, from the one node tree and in one call so the two cannot drift.
    ///
    /// The original asks the motion object for this from two places — a restart in the frame's
    /// header, and [`Player::play`] when a transition has no clip to come out of — and both end in
    /// the same joint-by-joint copy, which is [`Snapshot::take`].
    ///
    /// **What it copies is the node tree, not the blend buffer** ([`Skeleton::nodes`]). The original
    /// reads each joint record's transform node, so a joint the playing clip has no translation
    /// channel for is snapshotted at the `.psg`'s own local translation and not at the zero the
    /// blend buffer holds for it. On this disc's rider that is 21 joints of 22.
    pub fn rewind(&mut self, snapshot: &mut Snapshot) {
        self.under = self.skeleton.nodes(&self.pose);
        snapshot.take(&self.under, None, 0);
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
    /// - [`Blend::Three`] — four clips and three weights: two pairs mixed by `mix`, the two results
    ///   crossed by `cross`, all four at the first clip's clip time, faded by `ramp`
    ///   ([`Clip::pose_four_faded`]). No arm of the rider's own selection asks for it; the rider's
    ///   *other* state machine does, from the two interpolating bands of its pose grid
    ///   ([`Set::apply_vehicle`], V11).
    ///
    /// A blend whose clips are not all bound draws nothing and leaves the pose alone, which is the
    /// same frame the simulation's clock skips.
    pub fn apply(&mut self, blend: Blend, lib: &Library, player: &Player) -> bool {
        let time = player.sample_time(blend).0;
        let clip = |set: &Set, slot: Slot| -> Option<usize> {
            lib.clip(slot).map(|id| id.0 as usize).filter(|&at| at < set.clips.len())
        };
        match blend {
            Blend::Free => self.draw_free(time, player),
            Blend::One { ramp, clip: slot, .. } => self.draw_one(time, ramp.0, clip(self, slot)),
            Blend::Two { mix, ramp, a, b, .. } => {
                self.draw_two(time, mix.0, ramp.0, clip(self, a), clip(self, b))
            }
            Blend::Three { mix, cross, ramp, a, b, c, d, .. } => {
                let four = [clip(self, a), clip(self, b), clip(self, c), clip(self, d)];
                self.draw_four(time, mix.0, cross.0, ramp.0, four)
            }
        }
    }

    /// Write [`Set::pose`] from a blend of the *other* animation machine — the sixty-state one the
    /// rider's own body actor runs ([`game_sim::vehicle_anim`], S25 and V11) — which is where the two
    /// entry points nothing else in the image calls come from.
    ///
    /// The same five samplers as [`Set::apply`], plus:
    ///
    /// - [`vehicle_anim::Blend::Four`] — the pose grid's two interpolating bands, which is
    ///   [`Blend::Three`] on the library's side and the same [`Clip::pose_four_faded`] call;
    /// - [`vehicle_anim::Blend::Pair`] — the gesture slots' arm: two clips **each at its own clip
    ///   time**, `b`'s being last frame's progress through the gesture that was running
    ///   ([`Clip::pose_pair_faded`]).
    ///
    /// It takes no library: this machine's blends carry the handles themselves, because its clip table
    /// is indexed by state and not by stunt slot.
    pub fn apply_vehicle(&mut self, blend: vehicle_anim::Blend, player: &Player) -> bool {
        let time = blend.sample_time(player).0;
        let at = |set: &Set, id: Option<ClipId>| -> Option<usize> {
            id.map(|id| id.0 as usize).filter(|&at| at < set.clips.len())
        };
        match blend {
            vehicle_anim::Blend::Free => self.draw_free(time, player),
            vehicle_anim::Blend::One { ramp, clip, .. } => {
                self.draw_one(time, ramp.0, at(self, clip))
            }
            vehicle_anim::Blend::Two { mix, ramp, a, b, .. } => {
                self.draw_two(time, mix.0, ramp.0, at(self, a), at(self, b))
            }
            vehicle_anim::Blend::Four { mix, cross, ramp, a, b, c, d, .. } => {
                let four = [at(self, a), at(self, b), at(self, c), at(self, d)];
                self.draw_four(time, mix.0, cross.0, ramp.0, four)
            }
            vehicle_anim::Blend::Pair { mix, ramp, a, b, .. } => {
                // the second clip's own clip time: its duration times its own phase, which is the one
                // number this entry point needs and the four-clip one does not
                let second = blend.pair_time(&self.timing(b)).unwrap_or_default();
                self.draw_pair(time, second.0, mix.0, ramp.0, at(self, a), at(self, b))
            }
        }
    }

    /// [`Set::apply_vehicle`] with the clip-to-clip transition the frame's advance was running
    /// ([`game_sim::motion::Cross`], V17), which is the one thing the blend itself cannot carry: the
    /// transition lives in the motion object, is started by [`Player::play`] and not by any
    /// selection, and the advance's own end-of-frame reset wipes it on the frame it ends on.
    ///
    /// It only shows in mode 0. The original's advance samples the transition *first* and the caller's
    /// blend entry runs after it, writing the same joints again — so on modes 1, 2, 4 and 5 the blend
    /// is what the frame is posed with and the transition is thrown away, and mode 0 is the arm whose
    /// dispatch makes no call at all. A `None` cross, or any mode but 0, is exactly
    /// [`Set::apply_vehicle`].
    pub fn apply_vehicle_crossed(
        &mut self,
        blend: vehicle_anim::Blend,
        player: &Player,
        cross: Option<Cross>,
    ) -> bool {
        match (blend, cross) {
            (vehicle_anim::Blend::Free, Some(cross)) => {
                self.draw_crossed(blend.sample_time(player).0, player, cross)
            }
            _ => self.apply_vehicle(blend, player),
        }
    }

    /// Mode 0: the clip already on, at the clock its own dt step left.
    fn draw_free(&mut self, time: f32, player: &Player) -> bool {
        // A clip that has run out holds the pose it ended on. The advance stops stepping a finished
        // clip and resets its clock to zero on the way out, so sampling at `sample_time` here would
        // snap the rider back to the clip's first frame — which is not what "a finished clip holds its
        // last pose" means. See the journal.
        if player.finished {
            return false;
        }
        let Some(at) = player.clip.map(|id| id.0 as usize).filter(|&at| at < self.clips.len()) else {
            return false;
        };
        let bound = &self.clips[at];
        bound.clip.pose(time, &bound.joints, &mut self.pose);
        true
    }

    /// Mode 0 while a transition is running: the clip that was just put on, at its own clock, crossed
    /// out of the clip it replaced (at the transition's clock) or out of the pose snapshot.
    ///
    /// The snapshot arm reads [`Set::under`], which is the renderer's copy of the simulation's — one
    /// [`Set::rewind`] fills both, and `Player::play` is one of its two callers for exactly this.
    /// Either arm with a clip the set does not have draws nothing, as every other blend does.
    fn draw_crossed(&mut self, time: f32, player: &Player, cross: Cross) -> bool {
        let Some(at) = player.clip.map(|id| id.0 as usize).filter(|&at| at < self.clips.len()) else {
            return false;
        };
        let out_of = match cross.from.map(|id| id.0 as usize) {
            // the old clip's channels, paired with the new one's by position
            Some(k) if k < self.clips.len() => {
                OutOf::Clip { clip: &self.clips[k].clip, at: cross.at.0 }
            }
            Some(_) => return false,
            None => OutOf::Snapshot(&self.under),
        };
        let bound = &self.clips[at];
        bound.clip.pose_crossed(time, &bound.joints, cross.weight.0, out_of, &mut self.pose);
        true
    }

    /// Mode 1: one clip, faded back towards the snapshotted pose.
    fn draw_one(&mut self, time: f32, ramp: f32, a: Option<usize>) -> bool {
        let Some(at) = a else { return false };
        let bound = &self.clips[at];
        bound.clip.pose_faded(time, &bound.joints, ramp, &self.under, &mut self.pose);
        true
    }

    /// Mode 2: two clips at the first one's clip time.
    fn draw_two(&mut self, time: f32, mix: f32, ramp: f32, a: Option<usize>, b: Option<usize>) -> bool {
        let (Some(x), Some(y)) = (a, b) else { return false };
        let (first, second) = (&self.clips[x], &self.clips[y]);
        first.clip.pose_mixed_faded(
            &second.clip,
            time,
            mix,
            &first.joints,
            ramp,
            &self.under,
            &mut self.pose,
        );
        true
    }

    /// Mode 4: four clips and three weights, all at the first one's clip time.
    fn draw_four(&mut self, time: f32, mix: f32, cross: f32, ramp: f32, of: [Option<usize>; 4]) -> bool {
        let [Some(w), Some(x), Some(y), Some(z)] = of else { return false };
        let (a, b, c, d) = (&self.clips[w], &self.clips[x], &self.clips[y], &self.clips[z]);
        a.clip.pose_four_faded(
            &b.clip,
            &c.clip,
            &d.clip,
            time,
            mix,
            cross,
            &a.joints,
            ramp,
            &self.under,
            &mut self.pose,
        );
        true
    }

    /// Mode 5: two clips, each at its own clip time.
    fn draw_pair(
        &mut self,
        time: f32,
        other_time: f32,
        mix: f32,
        ramp: f32,
        a: Option<usize>,
        b: Option<usize>,
    ) -> bool {
        let (Some(x), Some(y)) = (a, b) else { return false };
        let (first, second) = (&self.clips[x], &self.clips[y]);
        first.clip.pose_pair_faded(
            &second.clip,
            time,
            other_time,
            mix,
            &first.joints,
            &second.joints,
            ramp,
            &self.under,
            &mut self.pose,
        );
        true
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
        // The loop flag is the animation set's and not the file's, and it belongs to whoever claimed
        // the name — the original writes it into the clip when it creates it, and a later `.mcf`
        // that names the same clip never gets to change it.
        clip.timing.looping = self.claim(name).is_some_and(|m| m.looping);
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

// ---------------------------------------------------------------------------------------------------
// the sixty-state machine's own rig
// ---------------------------------------------------------------------------------------------------

/// The time constant the mix axis is eased in with, as the body's build writes it, and the phase
/// axis'.
///
/// They are not in [`vehicle_anim::Params`] because in the original they are not on the object: each
/// axis is a five-word record of its own ({value, time constant, 1.0, FLT_MAX, -FLT_MAX}) that the
/// build allocates beside it, and the step walks both. V12 read the two taus off the running game and
/// they are the same two numbers the build writes.
pub const MIX_TAU: F = F(0.125);
pub const PHASE_TAU: F = F(0.15);

/// Where both axes start: the middle of a row, which is what makes a body that is not being steered
/// sit in the middle of each row of the pose grid rather than snap in from a column.
pub const AXIS_START: F = F(0.5);

/// One animation set driven by the **sixty-state** machine ([`game_sim::vehicle_anim`]) — which is
/// the rider's body, not the bike.
///
/// This is [`Rig`]'s sibling: the same join of a set, a clip table, a clock and a driver, for the
/// other of the two animation machines the game runs. The two are not interchangeable and the port
/// had them crossed over until V13:
///
/// | | the machine | its clip table | the skeleton it poses |
/// |---|---|---|---|
/// | [`Rig`] | [`game_sim::motion`], 28 slots | `bike_<stunt>` ×24 and `bike_air_Fwd/Mid/Bck` | the bike's |
/// | [`Stance`] | [`game_sim::vehicle_anim`], 60 states | `<prefix><stunt>` ×24 and [`vehicle_anim::POSE_CLIPS`] ×36 | the rider's 22 joints |
///
/// V12's capture settles it from the running game: the object the sixty-state step runs on has 22
/// joints, reads its clips out of `Rider\Anim\`, and binds **all sixty** of its table's slots — the
/// 24 `rider_<Stunt>` gestures and the 36 bare pose names. The 12 `bike_*` clips a bike archive
/// ships are the *other* machine's, which is why V9's grounded pose grid drew nothing: every
/// `turn_*` row of the grid is a rider clip and a bike set has none of them.
///
/// Nothing here decides which state runs. The chooser is the body's own update
/// ([`vehicle_anim::Update`], V11) and the caller hands its answer to [`Stance::frame`], the same way
/// [`crate::vehicle::Chassis`] hands it the reading it makes off the ride.
pub struct Stance {
    /// The clips, the skeleton they are bound to and the live pose.
    pub set: Set,
    /// The sixty-state clip table, as [`Stance::new`] filled it.
    pub lib: vehicle_anim::Library,
    /// The machine: the latched state, the cross-fade ramp, the two eased axes.
    pub driver: vehicle_anim::Driver,
    /// The motion object's clock, which is the same clock [`Rig`] runs on — one engine underneath.
    pub player: Player,
    /// The pose a cross-fade fades from, as the simulation records it.
    pub snapshot: Snapshot,
    /// The machine's own numbers ([`crate::vehicle::params`]).
    pub params: vehicle_anim::Params,
    /// A clip has written a pose. Until one has, the live pose is the bind pose — which for a rider
    /// is a body with its arms out, and for a bike is the bike.
    pub posed: bool,
    /// The *last* frame wrote a pose: the blend it asked for found every clip it needed. A frame
    /// that comes back false is a row of the table the set does not have — which is how a set
    /// paired with the wrong machine shows up.
    pub drawn: bool,
}

impl Stance {
    /// The machine as the object's constructor builds it, for a set whose table is already filled.
    ///
    /// Three of these are the constructor's own numbers rather than a zeroed default, and each one
    /// shows up on the first frame:
    ///
    /// * the state, the latched state and the previous state all start at
    ///   [`vehicle_anim::TURN_RIDING`] — the body starts in the pose grid, so a first frame that asks
    ///   for the grid is not a restart and does not cross-fade out of the bind pose;
    /// * `extra` is "which added record the selection takes instead of the state machine" and
    ///   **negative is none**, where a zeroed driver would ask for record 0 on its first frame;
    /// * both axes start centred ([`AXIS_START`]) with the two time constants above.
    pub fn bound(set: Set, lib: vehicle_anim::Library, params: vehicle_anim::Params) -> Stance {
        Stance {
            set,
            lib,
            driver: vehicle_anim::Driver {
                want: vehicle_anim::TURN_RIDING,
                state: vehicle_anim::TURN_RIDING,
                prev: vehicle_anim::TURN_RIDING,
                extra: -1,
                mix: vehicle_anim::Lag { value: AXIS_START, tau: MIX_TAU, ratio: F::ZERO },
                phase: vehicle_anim::Lag { value: AXIS_START, tau: PHASE_TAU, ratio: F::ZERO },
                ..vehicle_anim::Driver::default()
            },
            player: Player::default(),
            snapshot: Snapshot::default(),
            params,
            posed: false,
            drawn: false,
        }
    }

    /// Read a set's clip table the way the body's loader fills it, and bind every clip it names.
    ///
    /// `prefix` is what goes in front of a gesture slot's stunt name and nothing else: the original's
    /// spawn hands the body `"rider_"`, or `"ff_rider_"` for the freestyle copy of the same archive,
    /// and the 36 pose clips are looked up bare. Three passes, in the loader's own order:
    ///
    /// 1. the set's own `[MOTION n]` list, which is where a rider's 36 pose clips and a bike's three
    ///    airborne ones come from ([`Set::load_listed`]);
    /// 2. the 24 gesture slots, loaded and bound as `<prefix><stunt name>` — every slot, with no
    ///    `UsedForBike` gate, which is the one place this loader differs from the bike's;
    /// 3. the 36 pose names, asked for bare ([`vehicle_anim::Library::load`] does 2 and 3).
    ///
    /// Then one pass this port adds for a set that names its own clips after its actor: a bike's
    /// `.mcf` spells the airborne row `NAME=bike_air_Mid`, so a pose slot still empty is asked for a
    /// second time with the prefix in front. Only the empty ones, so a set that answered to the bare
    /// name keeps what it bound. On a rider's set nothing reaches it — all 36 answer bare — and it is
    /// this port's own reading of a seam the decompilation was not re-read for; see V9's journal.
    pub fn new(
        mut set: Set,
        prefix: &str,
        stunts: &[Stunt],
        params: vehicle_anim::Params,
    ) -> Stance {
        set.load_listed();
        let mut lib = vehicle_anim::Library::load(prefix, stunts, &mut set);
        for (i, name) in vehicle_anim::POSE_CLIPS.iter().enumerate() {
            let slot = SLOTS + i;
            if lib.clip[slot].is_none() {
                let prefixed = format!("{prefix}{name}");
                set.load(&prefixed);
                lib.clip[slot] = set.bound(&prefixed);
            }
        }
        Stance::bound(set, lib, params)
    }

    /// How many of the sixty states found a clip — 60 on a rider's set, 12 on a bike's.
    pub fn bound_slots(&self) -> usize {
        self.lib.clip.iter().filter(|c| c.is_some()).count()
    }

    /// One frame of the machine: the selection, the restart's snapshot, the clock, the pose.
    ///
    /// The original's own order, which is [`vehicle_anim::Driver::frame`] and then the motion
    /// library's entry point the frame names:
    ///
    /// ```text
    ///   Driver::frame        the ramp, the restart, the latch, the two lags, the selection
    ///   Set::rewind          on a restart: the pose the cross-fade fades from
    ///   Player::play         a played-once group putting its clip on, with a cross out of the last
    ///   Blend::run           the clock: a dt advance, or a phase-driven sample
    ///   Set::apply_vehicle_crossed  that blend's clips sampled on this skeleton       -> a Pose
    /// ```
    ///
    /// `want` is the state to run and `r` the request block. The placement is left to the caller —
    /// the shell already puts the bike and the body on it where the simulation says — so the step is
    /// run as a self-placing actor and [`vehicle_anim::Frame::place`] comes back `None`.
    pub fn frame(&mut self, dt: F, want: i32, r: &vehicle_anim::Request) -> vehicle_anim::Frame {
        // The machine is `Copy`, which is what lets the clip-length closure borrow the set while the
        // step runs.
        let mut driver = self.driver;
        driver.want = want;
        let frame = {
            let set = &self.set;
            driver.frame(dt, &self.params, r, &self.lib, true, |id| set.timing(Some(id)).duration)
        };
        self.driver = driver;

        if frame.restart {
            self.set.rewind(&mut self.snapshot);
        }
        // The two played-once groups put a clip on with a cross out of whatever was playing. A state
        // of those groups with no clip of its own still reaches the engine's play in the original,
        // which stores the null handle — and a null handle in the clock is "nothing is playing".
        if let Some(play) = frame.play {
            match play.put() {
                Some(put) => {
                    let playing = self.set.timing(self.player.clip).duration;
                    if self.player.play(put, playing) {
                        self.set.rewind(&mut self.snapshot);
                    }
                }
                None => {
                    self.player.clip = None;
                    self.player.finished = false;
                    self.player.fade.on = false;
                    self.player.time = F::ZERO;
                }
            }
        }
        let clip = self.set.timing(frame.blend.clock_clip(&self.player));
        // The advance's answer is kept: a clip-to-clip transition is in it and nowhere else by the
        // time the pose is drawn ([`Set::apply_vehicle_crossed`]).
        let played = frame.blend.run(&mut self.player, dt, &clip);
        self.drawn = self.set.apply_vehicle_crossed(frame.blend, &self.player, played.fade);
        self.posed |= self.drawn;
        frame
    }

    /// Every joint's transform under the live pose — one matrix per joint, which is what places a
    /// vertex.
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
