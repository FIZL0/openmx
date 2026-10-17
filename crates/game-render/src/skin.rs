//! `*.mot` — a motion clip, and how one frame of it moves a model.
//!
//! A clip is a list of *channels*, one per joint it animates, and a channel is a list of keys with a
//! time in seconds each. There are two kinds: rotation channels, whose key is a packed quaternion, and
//! position channels, whose key is three plain floats. Sampling a channel finds the pair of
//! keys a time falls between and interpolates — quaternions by a spherical blend ([`slerp`]), positions
//! by a straight line — which gives a joint's local rotation and translation. Hanging those on a
//! skeleton and walking it to the root gives one matrix per joint, and a `.psg` vertex names the joint
//! that places it (`psg::Vertex::node`), so that matrix is the whole of this game's skinning: one bone
//! per vertex, no weights.
//!
//! The file, little-endian throughout, as the original's reader walks it:
//!
//! ```text
//! i32            negated version if negative (3 on this disc), else version 1 and this is the next field
//! i32            frame count — the longest channel's key count; nothing here uses it
//! version >= 3:
//!   i32          name count, then that many: i32 length (the NUL included), then the bytes
//! f32            duration in seconds
//! i32            rotation key kind: 0 eight bytes, 1 twelve
//! i32            position key kind: 0 sixteen bytes, 1 forty
//! i32            position channel count
//! i32            rotation channel count
//! then the position channels, then the rotation channels, each:
//!   i32 key count, i32 name — an index into the name table above
//!   key[count]   kind 1 rotation: f32 time, u16 turn, i16 axis[3]   (twelve bytes, [`RotKey`])
//!                kind 0 rotation: f32 time, u8  turn, i8  axis[3]   (eight bytes, [`NarrowKey`])
//!                kind 0 position: f32 time, f32 xyz[3]
//! ```
//!
//! All 17,337 `.mot` members on this disc are version 3 with position kind 0. 17,295 of them are
//! rotation kind 1 — every rider and bike clip, and most of a track's moving scenery — and 42 are
//! rotation kind 0: the trucks and scrapers that drive around a few of the free-ride tracks. Version 1
//! and 2 headers and the forty-byte position key are read by the original's own reader but appear
//! nowhere on the disc, so they are rejected here rather than guessed at.
//!
//! **The compressed quaternion.** A rotation key spends its first bytes on an angle and the rest on an
//! axis. In the twelve-byte kind, `turn` indexes a 4096-entry table of
//! `(sin(n·π/4096) / 32767, cos(n·π/4096))` and `axis` is the axis scaled to ±32767; in the eight-byte
//! kind they are a byte each, indexing a 256-entry table of `(sin(n·π/256) / 127, cos(n·π/256))` with
//! the axis scaled to ±127. Either way the quaternion is `(axis * sin, |cos|)` with the *sign* of the
//! cosine folded into the axis — so `w` is never negative, and `q` and `-q`, which are the same
//! rotation, always come out the same way round. The narrow kind is coarser in both: 256 steps of angle
//! instead of 4096, and an axis to a part in 127.
//!
//! The original reads those two tables and the slerp's arc-cosine table out of memory the game fills at
//! start-up; nothing in the image references any of them, so [`tables`] rebuilds all three from `sin`,
//! `cos` and `acos`. They agree with a save state's copies to 1.8e-7, 6e-8 and 1.1e-5 respectively —
//! see the journal entry for V5.
//!
//! The arithmetic here is ordinary `f32`, not the PS2's. A joint that lands a ULP away from the
//! console's is invisible, and this crate draws rather than simulates.

use crate::mat::{M4, V3, v3};
use crate::psg::{Model, Strip, Tri};
use game_sim::motion;
use ps2_float::F;
use std::cell::Cell;
use std::sync::OnceLock;

/// What a joint map holds where the skeleton has no joint of that name: the channel is skipped.
pub const SKIP: u8 = 0xff;

/// One rotation key of the wide kind: a time, and a quaternion packed into eight bytes.
#[derive(Clone, Copy, Debug)]
pub struct RotKey {
    pub t: f32,
    /// Index into the sine/cosine table — half the rotation angle, in 4096ths of π.
    pub turn: u16,
    /// The rotation axis, scaled to ±32767.
    pub axis: [i16; 3],
}

/// One rotation key of the narrow kind: the same quaternion in four bytes instead of eight, which is
/// what a track's trucks and scrapers are animated with.
#[derive(Clone, Copy, Debug)]
pub struct NarrowKey {
    pub t: f32,
    /// Index into the narrow sine/cosine table — half the angle, in 256ths of π.
    pub turn: u8,
    /// The rotation axis, scaled to ±127.
    pub axis: [i8; 3],
}

/// A rotation channel's keys, in whichever of the two kinds its clip holds.
///
/// The original has a channel *class* per kind, with its own sampler and its own decoder; the search
/// through the keys is the same code twice, so here it is one sampler over this.
#[derive(Clone, Debug)]
pub enum RotKeys {
    Wide(Vec<RotKey>),
    Narrow(Vec<NarrowKey>),
}

impl RotKeys {
    pub fn len(&self) -> usize {
        match self {
            RotKeys::Wide(k) => k.len(),
            RotKeys::Narrow(k) => k.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// One key's time, in seconds.
    pub fn time(&self, i: usize) -> f32 {
        match self {
            RotKeys::Wide(k) => k[i].t,
            RotKeys::Narrow(k) => k[i].t,
        }
    }
    /// One key as a quaternion.
    pub fn quat(&self, i: usize) -> [f32; 4] {
        match self {
            RotKeys::Wide(k) => decode(&k[i]),
            RotKeys::Narrow(k) => decode_narrow(&k[i]),
        }
    }
}

impl From<Vec<RotKey>> for RotKeys {
    fn from(k: Vec<RotKey>) -> RotKeys {
        RotKeys::Wide(k)
    }
}

impl From<Vec<NarrowKey>> for RotKeys {
    fn from(k: Vec<NarrowKey>) -> RotKeys {
        RotKeys::Narrow(k)
    }
}

/// One position key: a time and a translation.
#[derive(Clone, Copy, Debug)]
pub struct PosKey {
    pub t: f32,
    pub p: [f32; 3],
}

/// A joint's rotation over the clip.
#[derive(Clone, Debug)]
pub struct RotChannel {
    /// Index into [`Clip::names`].
    pub name: i32,
    pub keys: RotKeys,
    /// The last pair [`RotChannel::sample`] used. The original keeps it in the channel too, so clips
    /// shared between two riders share the hint; it only ever costs a few extra comparisons.
    cursor: Cell<usize>,
}

/// A joint's translation over the clip.
#[derive(Clone, Debug)]
pub struct PosChannel {
    /// Index into [`Clip::names`].
    pub name: i32,
    pub keys: Vec<PosKey>,
}

/// A parsed `.mot`.
#[derive(Clone, Debug)]
pub struct Clip {
    pub version: i32,
    /// The longest channel's key count, as the file states it.
    pub frames: i32,
    /// The joint names the channels index.
    pub names: Vec<String>,
    /// The clip's timing, which is the simulation's own record of a clip and not a copy of it: its
    /// length, whether it loops, and the times inside it that fire an action frame. [`Clip::parse`]
    /// fills the length from the file; the other two are the animation set's, so they are filled
    /// when the clip is bound ([`crate::anim::Set`]) and left at their defaults until then.
    pub timing: motion::Clip,
    pub position: Vec<PosChannel>,
    pub rotation: Vec<RotChannel>,
}

impl Clip {
    /// Parse a whole `.mot`.
    pub fn parse(d: &[u8]) -> Result<Clip, String> {
        let mut p = 0usize;
        let mut version = 1;
        let mut frames = i32_at(d, &mut p)?;
        if frames < 0 {
            version = -frames;
            frames = i32_at(d, &mut p)?;
        }
        let mut names = Vec::new();
        if version >= 3 {
            let n = i32_at(d, &mut p)?;
            if !(0..=4096).contains(&n) {
                return Err(format!("not a .mot: {n} names"));
            }
            for _ in 0..n {
                let len = i32_at(d, &mut p)?;
                if len < 1 || p + len as usize > d.len() {
                    return Err(format!("not a .mot: a name of {len} bytes at {p}"));
                }
                // the length counts the NUL the name ends with
                let bytes = &d[p..p + len as usize - 1];
                names.push(String::from_utf8_lossy(bytes).into_owned());
                p += len as usize;
            }
        }
        let duration = f32_at(d, &mut p)?;
        let rot_kind = i32_at(d, &mut p)?;
        let pos_kind = i32_at(d, &mut p)?;
        let n_pos = i32_at(d, &mut p)?;
        let n_rot = i32_at(d, &mut p)?;
        if version < 3 {
            return Err(format!("a version {version} .mot: no clip on this disc is one"));
        }
        if !(0..=1).contains(&rot_kind) || pos_kind != 0 {
            return Err(format!(
                "a .mot with rotation kind {rot_kind} and position kind {pos_kind}: every clip on this \
                 disc is 0 or 1, and 0"
            ));
        }
        let mut position = Vec::new();
        for _ in 0..n_pos.max(0) {
            let count = i32_at(d, &mut p)?;
            let name = i32_at(d, &mut p)?;
            let mut keys = Vec::new();
            for _ in 0..count.max(0) {
                let t = f32_at(d, &mut p)?;
                let p3 = [f32_at(d, &mut p)?, f32_at(d, &mut p)?, f32_at(d, &mut p)?];
                keys.push(PosKey { t, p: p3 });
            }
            position.push(PosChannel { name, keys });
        }
        let mut rotation = Vec::new();
        for _ in 0..n_rot.max(0) {
            let count = i32_at(d, &mut p)?;
            let name = i32_at(d, &mut p)?;
            let keys: RotKeys = if rot_kind == 1 {
                let mut keys = Vec::new();
                for _ in 0..count.max(0) {
                    let t = f32_at(d, &mut p)?;
                    let turn = u16_at(d, &mut p)?;
                    let axis = [i16_at(d, &mut p)?, i16_at(d, &mut p)?, i16_at(d, &mut p)?];
                    keys.push(RotKey { t, turn, axis });
                }
                keys.into()
            } else {
                let mut keys = Vec::new();
                for _ in 0..count.max(0) {
                    let t = f32_at(d, &mut p)?;
                    let turn = u8_at(d, &mut p)?;
                    let axis = [i8_at(d, &mut p)?, i8_at(d, &mut p)?, i8_at(d, &mut p)?];
                    keys.push(NarrowKey { t, turn, axis });
                }
                keys.into()
            };
            rotation.push(RotChannel::new(name, keys));
        }
        if p != d.len() {
            return Err(format!("not a .mot: its channels end at {p}, not at {}", d.len()));
        }
        let timing = motion::Clip::new(F(duration), false);
        Ok(Clip { version, frames, names, timing, position, rotation })
    }

    /// The clip's length in seconds, which is the one field of the timing record that comes out of
    /// the file rather than out of the animation set.
    pub fn duration(&self) -> f32 {
        self.timing.duration.0
    }

    /// This clip's name table resolved against a skeleton: one joint index per name, [`SKIP`] where
    /// the skeleton has no joint by that name.
    ///
    /// The original builds the same byte per name when it loads a clip for a model, and every sampler
    /// goes through it, which is how one clip drives a rider and a bike that do not share a joint
    /// numbering.
    pub fn joints(&self, skel: &Skeleton) -> Vec<u8> {
        self.names
            .iter()
            .map(|n| match skel.find(n) {
                Some(j) if j < SKIP as usize => j as u8,
                _ => SKIP,
            })
            .collect()
    }

    /// Sample every channel at `time` seconds and write the joints it names.
    pub fn pose(&self, time: f32, map: &[u8], out: &mut Pose) {
        self.lay(time, Mixed::Alone, map, None, out);
    }

    /// Sample at `time`, then fade each joint `fade` of the way back to `under`: the cross-fade a
    /// restarted motion runs against the pose it was in when it restarted. `fade` is 1 at the restart
    /// and falls to 0, so 0 is all of this clip and 1 is all of `under`.
    pub fn pose_faded(&self, time: f32, map: &[u8], fade: f32, under: &Pose, out: &mut Pose) {
        self.lay(time, Mixed::Alone, map, Some((fade, under)), out);
    }

    /// Both clips sampled at the same second and mixed: `mix` 0 is all of `other`, 1 is all of this
    /// one. The original drives both from a phase of *this* clip's duration, however long `other` is.
    ///
    /// The two clips must have the same channels in the same order — the original pairs them by
    /// position in the list, not by name — which is how the airborne idle blends two leans.
    pub fn pose_mixed(&self, other: &Clip, time: f32, mix: f32, map: &[u8], out: &mut Pose) {
        self.lay(time, Mixed::With { other, mix }, map, None, out);
    }

    /// Both at once: two clips mixed, and the result faded back towards the pose under it. This is
    /// the shape the airborne idle actually asks for — [`motion::Blend::Two`] carries a `mix` and a
    /// `ramp` together, and the original's two-clip entry takes both in one pass over the channels.
    pub fn pose_mixed_faded(
        &self,
        other: &Clip,
        time: f32,
        mix: f32,
        map: &[u8],
        fade: f32,
        under: &Pose,
        out: &mut Pose,
    ) {
        self.lay(time, Mixed::With { other, mix }, map, Some((fade, under)), out);
    }

    /// Four clips and three weights, faded back towards the pose under it: the original's fourth blend
    /// entry ([`motion::Blend::Three`]), whose only callers in the whole image are the two
    /// interpolating bands of the rider's pose grid (`game_sim::vehicle_anim`).
    ///
    /// All four clips are sampled at the same second — `time`, which is *this* clip's duration times
    /// the phase, however long the other three are — and paired by position in the channel list, as
    /// the two-clip entry pairs its two. `mix` is used inside both pairs: 1 is this clip and `c`, 0 is
    /// `b` and `d`. `cross` 1 is this clip's pair and 0 is `c`'s.
    ///
    /// Every channel writes the joint *this* clip's map names, including the three clips mixed in.
    #[allow(clippy::too_many_arguments)]
    pub fn pose_four_faded(
        &self,
        b: &Clip,
        c: &Clip,
        d: &Clip,
        time: f32,
        mix: f32,
        cross: f32,
        map: &[u8],
        fade: f32,
        under: &Pose,
        out: &mut Pose,
    ) {
        self.lay(time, Mixed::Four { b, c, d, mix, cross }, map, Some((fade, under)), out);
    }

    /// Two clips, **each at its own second**, faded back towards the pose under it: the original's
    /// sixth blend entry (`game_sim::vehicle_anim::Blend::Pair`), which the gesture slots' arm of the
    /// rider's own state machine is the only caller of.
    ///
    /// `mix` 0 is all of `other` and 1 is all of this clip, as in [`Clip::pose_mixed`] — the whole
    /// difference is `other_time`, which lets a gesture cross out of the gesture that was running at
    /// whatever point *that* clip had reached rather than at a second of this clip's length.
    ///
    /// The one asymmetry: the rotation channels write the joint **`other`'s** map names and the
    /// position channels the joint this clip's map names. That is the original's, not a port decision,
    /// and it only shows on two clips whose channel lists disagree.
    #[allow(clippy::too_many_arguments)]
    pub fn pose_pair_faded(
        &self,
        other: &Clip,
        time: f32,
        other_time: f32,
        mix: f32,
        map: &[u8],
        other_map: &[u8],
        fade: f32,
        under: &Pose,
        out: &mut Pose,
    ) {
        let mixed = Mixed::Pair { other, time: other_time, map: other_map, mix };
        self.lay(time, mixed, map, Some((fade, under)), out);
    }

    /// The clip-to-clip transition, which is the original's *other* pair of blend entries: this clip
    /// — the one that was just put on — weighted `weight`, over what the transition is coming out of
    /// ([`OutOf`]). `weight` is 0 at the start of the transition and 1 at its end, the opposite way
    /// round from the cross-fade ramp of the six entries above.
    ///
    /// It is not [`Clip::lay`] with a mirrored ramp, for two reasons. The rotation is
    /// `slerp(weight, out_of, this)` rather than `slerp(1 - weight, this, out_of)`, and the port's
    /// shortest-path slerp negates its *second* argument, so the two are not the same quaternion.
    /// And the clip arm pairs the two clips' channels **by position**, like the mixing entries, while
    /// the joint it writes always comes from this clip's map — so a transition out of a clip with a
    /// different channel list is the original's own mismatch, not something to resolve by name.
    ///
    /// Every channel of this clip that the map names a joint for is written, and nothing else is
    /// touched: a joint the new clip does not animate holds what it had, transition or no transition.
    pub fn pose_crossed(&self, time: f32, map: &[u8], weight: f32, out_of: OutOf, out: &mut Pose) {
        for (i, c) in self.rotation.iter().enumerate() {
            let Some(j) = joint(map, c.name, out.rot.len()) else { continue };
            let Some(q) = c.sample(time) else { continue };
            let was = match out_of {
                OutOf::Clip { clip, at } => match clip.rotation.get(i).and_then(|o| o.sample(at)) {
                    Some(was) => was,
                    None => continue,
                },
                OutOf::Snapshot(under) => match under.rot.get(j) {
                    Some(&was) => was,
                    None => continue,
                },
            };
            out.rot[j] = slerp(weight, was, q, true);
            out.turned[j] = true;
        }
        for (i, c) in self.position.iter().enumerate() {
            let Some(j) = joint(map, c.name, out.pos.len()) else { continue };
            let Some(p) = c.sample(time) else { continue };
            let was = match out_of {
                OutOf::Clip { clip, at } => match clip.position.get(i).and_then(|o| o.sample(at)) {
                    Some(was) => was,
                    None => continue,
                },
                OutOf::Snapshot(under) => match under.pos.get(j) {
                    Some(&was) => was,
                    None => continue,
                },
            };
            // the original's grouping: this clip's sample, plus what is left of the difference
            out.pos[j] = add4(p, scale4(sub4(was, p), 1.0 - weight));
            out.moved[j] = true;
        }
    }

    /// The body of all six entries: the channel walk every one of the original's blend entry points
    /// shares, with [`Mixed`] standing for what each of them adds to it.
    fn lay(&self, time: f32, mixed: Mixed, map: &[u8], faded: Option<(f32, &Pose)>, out: &mut Pose) {
        for (i, c) in self.rotation.iter().enumerate() {
            // Which map names the joint this channel writes. Every entry but the pair takes it from
            // this clip's own; the pair takes the *second* clip's map and the second clip's channel.
            let (named, name) = match mixed {
                Mixed::Pair { other, map, .. } => match other.rotation.get(i) {
                    Some(o) => (map, o.name),
                    None => continue,
                },
                _ => (map, c.name),
            };
            let Some(j) = joint(named, name, out.rot.len()) else { continue };
            let Some(mut q) = c.sample(time) else { continue };
            match mixed {
                Mixed::Alone => {}
                Mixed::With { other, mix } => {
                    let Some(o) = other.rotation.get(i).and_then(|c| c.sample(time)) else {
                        continue;
                    };
                    q = slerp(mix, o, q, true);
                }
                Mixed::Pair { other, time, mix, .. } => {
                    let Some(o) = other.rotation.get(i).and_then(|c| c.sample(time)) else {
                        continue;
                    };
                    q = slerp(mix, o, q, true);
                }
                Mixed::Four { b, c, d, mix, cross } => {
                    let three = (b.rotation.get(i), c.rotation.get(i), d.rotation.get(i));
                    let (Some(b), Some(c), Some(d)) = three else { continue };
                    let (Some(b), Some(c), Some(d)) =
                        (b.sample(time), c.sample(time), d.sample(time))
                    else {
                        continue;
                    };
                    // the pairs first, by the one mix, then the two results by the cross
                    q = slerp(cross, slerp(mix, d, c, true), slerp(mix, b, q, true), true);
                }
            }
            if let Some((fade, under)) = faded {
                if let (true, Some(&was)) = (fade > 0.0, under.rot.get(j)) {
                    q = slerp(fade, q, was, true);
                }
            }
            out.rot[j] = q;
            out.turned[j] = true;
        }
        for (i, c) in self.position.iter().enumerate() {
            // The position channels take the joint from this clip's own map in every entry, the pair
            // included — which is what makes the pair's rotation loop an asymmetry and not a reading.
            let Some(j) = joint(map, c.name, out.pos.len()) else { continue };
            let Some(mut p) = c.sample(time) else { continue };
            match mixed {
                Mixed::Alone => {}
                Mixed::With { other, mix } => {
                    let Some(o) = other.position.get(i).and_then(|c| c.sample(time)) else {
                        continue;
                    };
                    // the original's grouping: the second clip's sample, less the mix of the difference
                    p = sub4(o, scale4(sub4(o, p), mix));
                }
                Mixed::Pair { other, time, mix, .. } => {
                    let Some(o) = other.position.get(i).and_then(|c| c.sample(time)) else {
                        continue;
                    };
                    p = sub4(o, scale4(sub4(o, p), mix));
                }
                Mixed::Four { b, c, d, mix, cross } => {
                    let three = (b.position.get(i), c.position.get(i), d.position.get(i));
                    let (Some(b), Some(c), Some(d)) = three else { continue };
                    let (Some(b), Some(c), Some(d)) =
                        (b.sample(time), c.sample(time), d.sample(time))
                    else {
                        continue;
                    };
                    let near = sub4(b, scale4(sub4(b, p), mix));
                    let far = sub4(d, scale4(sub4(d, c), mix));
                    p = sub4(far, scale4(sub4(far, near), cross));
                }
            }
            if let Some((fade, under)) = faded {
                if let (true, Some(&was)) = (fade > 0.0, under.pos.get(j)) {
                    p = add4(p, scale4(sub4(was, p), fade));
                }
            }
            out.pos[j] = p;
            out.moved[j] = true;
        }
    }
}

/// What a clip-to-clip transition is coming *out* of ([`Clip::pose_crossed`]), which is the one thing
/// the original's two transition entries disagree about.
///
/// Which of the two it is the simulation decides, not the renderer: a transition started on top of
/// nothing — or on top of another transition, whose clip is itself half-way into a blend — has no
/// clip to come out of and fades out of the pose snapshot instead
/// (`game_sim::motion::Cross::from`).
#[derive(Clone, Copy)]
pub enum OutOf<'a> {
    /// The clip that was playing, at the transition's own clock — which starts at the clip time that
    /// clip had reached, so it keeps playing through the cross.
    Clip { clip: &'a Clip, at: f32 },
    /// The pose the rider was in when the transition started, joint by joint ([`Pose`]).
    Snapshot(&'a Pose),
}

/// What a blend entry point adds to [`Clip::lay`]'s channel walk: nothing, one more clip, three more,
/// or one more at its own clip time.
///
/// The original has six entries and they are one loop with four shapes of body. This is that
/// difference, and nothing else: the channel pairing, the joint maps, the cross-fade against the
/// snapshotted pose and the writes are the same in all of them.
#[derive(Clone, Copy)]
enum Mixed<'a> {
    /// The advance (mode 0) and the one-clip entry (mode 1): this clip alone.
    Alone,
    /// The two-clip entry (mode 2): `other` at the same second, `mix` 0 all of it and 1 all of this
    /// clip.
    With { other: &'a Clip, mix: f32 },
    /// The four-clip entry (mode 4): this clip and `b` mixed, `c` and `d` mixed, the two results
    /// crossed. All four at the one second.
    Four { b: &'a Clip, c: &'a Clip, d: &'a Clip, mix: f32, cross: f32 },
    /// The pair entry (mode 5): `other` at *its* own second `time`, and — for the rotation channels
    /// only — its own joint map.
    Pair { other: &'a Clip, time: f32, map: &'a [u8], mix: f32 },
}

/// A channel's joint, or `None` when the clip's map says to skip it.
fn joint(map: &[u8], name: i32, joints: usize) -> Option<usize> {
    let j = *map.get(usize::try_from(name).ok()?)?;
    (j != SKIP && (j as usize) < joints).then_some(j as usize)
}

impl RotChannel {
    /// A channel of these keys, the way the clip reader builds one: the pair hint starts at 1.
    pub fn new(name: i32, keys: impl Into<RotKeys>) -> RotChannel {
        RotChannel { name, keys: keys.into(), cursor: Cell::new(1) }
    }

    /// The pair hint, which a proof against the original has to be able to see.
    pub fn cursor(&self) -> usize {
        self.cursor.get()
    }

    /// This channel's rotation at `time` seconds, or `None` when it has no keys — which is what the
    /// original leaves the joint alone for.
    ///
    /// Outside the clip the nearest key is held, with one quirk kept: *before* the first key the
    /// original holds the **second** key, not the first, because the index its search falls back on is
    /// 1 rather than 0. After the last key it holds the last.
    pub fn sample(&self, time: f32) -> Option<[f32; 4]> {
        let k = &self.keys;
        let last = k.len().checked_sub(1)?;
        if k.len() < 3 {
            // a single key holds still; a pair is the pair. The original divides by this same
            // interval but subtracts the time of a key *before* the array, which no clip on this disc
            // reaches: every channel has at least six keys.
            let u = if last == 1 { (time - k.time(0)) / (k.time(1) - k.time(0)) } else { 0.0 };
            return Some(slerp(u, k.quat(0), k.quat(last), true));
        }
        // the hint, and the two guesses that throw it away: a hint past three quarters of the clip
        // with a time before the first quarter starts again, and the other way about jumps to the end
        let quarter = last >> 2;
        let three_q = (last >> 1) + quarter;
        let mut i = self.cursor.get();
        if three_q < i && time < k.time(quarter) {
            i = 1;
        } else if i < quarter && k.time(three_q) < time {
            i = last;
        }
        let mut i = i.clamp(1, last);
        let mut held = false;
        if k.time(i) < time {
            loop {
                i += 1;
                if i > last {
                    i = last;
                    held = true;
                    break;
                }
                if k.time(i) >= time {
                    break;
                }
            }
        }
        loop {
            if k.time(i - 1) <= time {
                break;
            }
            i -= 1;
            if i == 0 {
                i = 1;
                held = true;
                break;
            }
        }
        self.cursor.set(i);
        if held {
            return Some(k.quat(i));
        }
        let u = (time - k.time(i - 1)) / (k.time(i) - k.time(i - 1));
        Some(slerp(u, k.quat(i - 1), k.quat(i), true))
    }
}

impl PosChannel {
    /// A channel of these keys.
    pub fn new(name: i32, keys: Vec<PosKey>) -> PosChannel {
        PosChannel { name, keys }
    }

    /// This channel's translation at `time` seconds as `(x, y, z, 1)`, or `None` when it has no keys.
    ///
    /// Past the last key the original does not hold it: it carries the last interval's slope on, and
    /// so does this. Before the first key it reads a key before the array, which no clip on this disc
    /// reaches (every channel has at least six keys); here the first key is held instead.
    pub fn sample(&self, time: f32) -> Option<[f32; 4]> {
        let k = &self.keys;
        let last = k.len().checked_sub(1)?;
        if last == 0 {
            let p = k[0].p;
            return Some([p[0], p[1], p[2], 1.0]);
        }
        let (a, b, u, inside) = self.bracket(time);
        let (a, b) = (&k[a].p, &k[b].p);
        let base = if inside { a } else { b };
        Some([
            base[0] + u * (b[0] - a[0]),
            base[1] + u * (b[1] - a[1]),
            base[2] + u * (b[2] - a[2]),
            1.0,
        ])
    }

    /// The pair of keys `time` falls between, how far between them it is, and whether it was inside
    /// the clip at all. Past the end the pair is the last two and `u` runs on above 1.
    fn bracket(&self, time: f32) -> (usize, usize, f32, bool) {
        let k = &self.keys;
        let last = k.len() - 1;
        if time >= k[last].t {
            let u = (time - k[last].t) / (k[last].t - k[last - 1].t);
            return (last - 1, last, u, false);
        }
        if time < k[0].t {
            return (0, 0, 0.0, true);
        }
        let (mut lo, mut hi) = (0usize, k.len());
        let mut i = 0;
        // the original's binary search, which halves until the time sits between a key and the next
        'outer: for _ in 0..2 * k.len() + 8 {
            for _ in 0..2 * k.len() + 8 {
                i = (lo + hi) >> 1;
                if time < k[i].t {
                    break;
                }
                lo = i;
                if i + 1 > last || time < k[i + 1].t {
                    i += 1;
                    break 'outer;
                }
            }
            hi = i;
            if i == 0 || k[i - 1].t <= time {
                break;
            }
        }
        let i = i.clamp(1, last);
        (i - 1, i, (time - k[i - 1].t) / (k[i].t - k[i - 1].t), true)
    }
}

/// One rotation key as a quaternion `(x, y, z, w)`.
///
/// `w` is never negative: the sign of the stored cosine is folded into the axis, which negates the
/// quaternion when the cosine is — and a quaternion and its negative are the same rotation.
pub fn decode(key: &RotKey) -> [f32; 4] {
    let [sin, cos] = tables().turn[key.turn as usize & 0xfff];
    let s = f32::from_bits(sin.to_bits() ^ (cos.to_bits() & 0x8000_0000));
    [
        key.axis[0] as f32 * s,
        key.axis[1] as f32 * s,
        key.axis[2] as f32 * s,
        cos.abs(),
    ]
}

/// One narrow rotation key as a quaternion `(x, y, z, w)`: the same decode an octave coarser, through
/// the 256-entry table and an axis of bytes.
pub fn decode_narrow(key: &NarrowKey) -> [f32; 4] {
    let [sin, cos] = tables().narrow[key.turn as usize];
    let s = f32::from_bits(sin.to_bits() ^ (cos.to_bits() & 0x8000_0000));
    [
        key.axis[0] as f32 * s,
        key.axis[1] as f32 * s,
        key.axis[2] as f32 * s,
        cos.abs(),
    ]
}

/// The spherical blend between two quaternions: `t` 0 is all `from`, 1 is all `to`.
///
/// `shortest` is the original's fourth argument, and every caller in the game passes it set: with it,
/// a pair more than a quarter turn apart is blended the short way round by negating one of them.
///
/// Two things are the original's rather than the textbook's. The angle comes from a 30001-entry
/// arc-cosine table read with a linear blend between neighbours, and the three sines from a four-term
/// Taylor series; and when the two quaternions are the same to the last bit — a dot product of exactly
/// 1 or -1 — it returns `from` untouched rather than dividing by a sine of zero.
pub fn slerp(t: f32, from: [f32; 4], to: [f32; 4], shortest: bool) -> [f32; 4] {
    let tb = tables();
    let dot = ((from[0] * to[0] + from[1] * to[1]) + from[2] * to[2]) + from[3] * to[3];
    if dot.abs() >= 1.0 {
        return from;
    }
    let flip = if shortest && dot < 0.0 { -1.0 } else { 1.0 };
    let x = ARC_SCALE + dot.abs() * ARC_SCALE;
    let i = x as i32;
    let frac = x - i as f32;
    let (lo, hi) = (tb.arc[i as usize], tb.arc[i as usize + 1]);
    let theta = (lo - frac * lo) + frac * hi;
    // the original subtracts rather than taking one less the weight, which rounds differently
    let (a, b) = (sine(theta - t * theta), flip * sine(t * theta));
    let q = 1.0 / sine(theta);
    [
        (from[0] * a + to[0] * b) * q,
        (from[1] * a + to[1] * b) * q,
        (from[2] * a + to[2] * b) * q,
        (from[3] * a + to[3] * b) * q,
    ]
}

/// The four-term Taylor series the slerp's three sines go through, in its own grouping.
fn sine(x: f32) -> f32 {
    let c = SIN_POLY;
    let x2 = x * x;
    x * (c[0] + x2 * (c[1] + x2 * (c[2] + x2 * c[3])))
}

/// A quaternion as a rotation matrix, in the row-vector form the rest of this crate uses: a point is
/// `p * m`, and the last row is the translation.
pub fn rotation(q: [f32; 4]) -> M4 {
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    let (x2, y2, z2, w2) = (x + x, y + y, z + z, w + w);
    let (wx, wy, wz, ww) = (x * w2, y * w2, z * w2, w * w2 - 1.0);
    M4([
        [x * x2 + ww, x * y2 + wz, x * z2 - wy, 0.0],
        [x * y2 - wz, y * y2 + ww, y * z2 + wx, 0.0],
        [x * z2 + wy, y * z2 - wx, z * z2 + ww, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ])
}

/// What a clip writes: one local rotation and translation per joint, and which of them it wrote.
///
/// A clip animates the joints it has channels for and leaves the rest where the skeleton's rest pose
/// put them, so [`Skeleton::world`] needs to know which is which.
///
/// **This is the pose of joints, and there is one of it.** The simulation keeps a record of the same
/// two buffers ([`motion::Snapshot`] — the pose a cross-fade fades from, whose field order and
/// allocation rules are proved against the original) but it never holds the live one: it copies out
/// of this through [`motion::Joints`], which this type implements, and the copy comes back the other
/// way through [`Pose::of_snapshot`]. What the simulation calls `motion::Pose` is a different thing
/// again — the rider's lean, not a pose of joints.
#[derive(Clone, Debug, Default)]
pub struct Pose {
    pub rot: Vec<[f32; 4]>,
    pub pos: Vec<[f32; 4]>,
    pub turned: Vec<bool>,
    pub moved: Vec<bool>,
}

impl Pose {
    /// A pose of `joints` joints with nothing written yet.
    pub fn new(joints: usize) -> Pose {
        Pose {
            rot: vec![[0.0, 0.0, 0.0, 1.0]; joints],
            pos: vec![[0.0, 0.0, 0.0, 1.0]; joints],
            turned: vec![false; joints],
            moved: vec![false; joints],
        }
    }

    /// Forget which joints were written, keeping what was in them: what the original's blend buffer
    /// is for, since the cross-fade reads the pose the last frame left.
    pub fn hold(&mut self) {
        self.turned.fill(false);
        self.moved.fill(false);
    }

    /// The simulation's snapshot of a pose, as a pose again: the other end of [`motion::Joints`].
    ///
    /// Every joint counts as written, because every joint of a snapshot *was* read — the original
    /// copies the whole skeleton, not the channels a clip happens to have.
    pub fn of_snapshot(snapshot: &motion::Snapshot) -> Pose {
        let n = snapshot.rotation.len().min(snapshot.translation.len());
        Pose {
            rot: snapshot.rotation[..n].iter().map(|q| q.map(|v| v.0)).collect(),
            pos: snapshot.translation[..n].iter().map(|p| p.map(|v| v.0)).collect(),
            turned: vec![true; n],
            moved: vec![true; n],
        }
    }
}

/// The simulation reads the live joint pose through this and nothing else — which is what makes the
/// renderer's [`Pose`] the one buffer of joint poses in the port rather than one of two.
///
/// The original's two accessors pull the rotation out of the joint's local matrix as a quaternion and
/// take its translation as a point with 1 in the fourth lane. Here the pose already holds both in
/// that shape, so the reads are plain; a joint the clip did not write answers with the identity
/// rotation and the origin rather than with the bone's own rest transform, which is a divergence
/// recorded in V7's journal.
impl motion::Joints for Pose {
    fn count(&self) -> usize {
        self.rot.len()
    }

    fn rotation(&self, joint: usize) -> [F; 4] {
        self.rot.get(joint).copied().unwrap_or([0.0, 0.0, 0.0, 1.0]).map(F)
    }

    fn translation(&self, joint: usize) -> [F; 4] {
        self.pos.get(joint).copied().unwrap_or([0.0, 0.0, 0.0, 1.0]).map(F)
    }
}

/// One bone, as the `.psg`'s node tree gives it.
///
/// A bone and a joint are the same thing here: a `.psg` node. The two names are the two halves this
/// module joins — the model side calls them bones and hangs geometry off them, the clip side calls
/// them joints and names them in its channels.
#[derive(Clone, Debug)]
pub struct Bone {
    pub name: String,
    pub parent: Option<usize>,
    /// Relative to the parent bone, and what a clip's channel replaces.
    pub local: M4,
    /// The bind pose: the bone's place in model space, parents composed in. Kept rather than
    /// recomputed because the bolting-on in [`crate::rider`] is a question about two bind poses and
    /// nothing else; [`Skeleton::rebind`] puts it back in step after a hand edit of `local`.
    pub rest: M4,
}

impl Bone {
    /// Where the bone sits in the bind pose.
    pub fn at(&self) -> V3 {
        self.rest.point(v3(0.0, 0.0, 0.0))
    }
}

/// A model's node tree read as a skeleton: the bones in file order, each with its parent, its own
/// transform and its bind pose.
///
/// **One skeleton, not two.** The drawing side used to declare this twice — once for bolting a rider
/// onto a bike by its bind pose, once for hanging a clip's channels on a model — and a clip bound to
/// one of them could not be posed on the other. They are the same tree: a `.psg`'s nodes *are* the
/// joints, a vertex names one of them (`psg::Vertex::node`), and a clip's channel names are those
/// node names.
#[derive(Clone, Debug, Default)]
pub struct Skeleton {
    pub bones: Vec<Bone>,
}

impl Skeleton {
    /// A `.psg`'s own node tree, with each bone's bind pose composed down the chain.
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

    /// How many joints the skeleton has, which is how long a [`Pose`] of it is.
    pub fn len(&self) -> usize {
        self.bones.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bones.is_empty()
    }

    /// A bone by name, case-insensitively, which is how the game's own name matching works: the
    /// rider spells its anchors `Dummy_foot_L` and the chassis spells the same joint `Dummy_Foot_L`,
    /// and a clip's channel names agree with neither in case.
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

    /// Recompute every bone's bind pose from its own transform and its parents. Only needed after
    /// something has written `local` by hand — [`Skeleton::of`] leaves the two in step.
    pub fn rebind(&mut self) {
        let local: Vec<M4> = self.bones.iter().map(|b| b.local).collect();
        let parents: Vec<Option<usize>> = self.bones.iter().map(|b| b.parent).collect();
        let mut out = vec![None; local.len()];
        for j in 0..local.len() {
            resolve(j, &local, &parents, &mut out);
        }
        for (b, m) in self.bones.iter_mut().zip(out) {
            b.rest = m.unwrap_or(M4::IDENTITY);
        }
    }

    /// Every joint's local transform under a pose: the clip's rotation and translation where it wrote
    /// them, the bone's own where it did not.
    pub fn locals(&self, pose: &Pose) -> Vec<M4> {
        self.bones
            .iter()
            .enumerate()
            .map(|(j, bone)| {
                let mut m = bone.local;
                if pose.turned.get(j) == Some(&true) {
                    let r = rotation(pose.rot[j]);
                    m.0[0][..3].copy_from_slice(&r.0[0][..3]);
                    m.0[1][..3].copy_from_slice(&r.0[1][..3]);
                    m.0[2][..3].copy_from_slice(&r.0[2][..3]);
                }
                if pose.moved.get(j) == Some(&true) {
                    m.0[3][..3].copy_from_slice(&pose.pos[j][..3]);
                }
                m
            })
            .collect()
    }

    /// Every joint's transform all the way to the root — one matrix per joint, which is what places a
    /// vertex. Parents are composed before children, so a joint whose parent comes after it in the
    /// list still lands right.
    pub fn world(&self, pose: &Pose) -> Vec<M4> {
        self.compose(&self.locals(pose))
    }

    /// The same composition down the tree, from local transforms the caller built itself.
    ///
    /// [`Skeleton::world`] is this over [`Skeleton::locals`]. It is separate because a pose is not the
    /// only thing that writes a local transform: the vehicle's suspension, steering and wheel nodes
    /// are driven from the simulation's own numbers *after* the clip has been sampled
    /// ([`crate::vehicle`]), which means overwriting four of the locals and then composing. A list
    /// shorter than the skeleton is padded with the bones' own transforms.
    pub fn compose(&self, local: &[M4]) -> Vec<M4> {
        let local: Vec<M4> = (0..self.bones.len())
            .map(|j| local.get(j).copied().unwrap_or(self.bones[j].local))
            .collect();
        let parents: Vec<Option<usize>> = self.bones.iter().map(|b| b.parent).collect();
        let mut out = vec![None; local.len()];
        for j in 0..local.len() {
            resolve(j, &local, &parents, &mut out);
        }
        out.into_iter().map(|m| m.unwrap_or(M4::IDENTITY)).collect()
    }
}

/// One joint's chain, memoised, with a parent loop giving up rather than hanging.
fn resolve(j: usize, local: &[M4], parents: &[Option<usize>], out: &mut Vec<Option<M4>>) -> M4 {
    if let Some(m) = out[j] {
        return m;
    }
    out[j] = Some(local[j]); // breaks a cycle: a joint under itself gets its local transform
    let m = match parents.get(j).copied().flatten() {
        Some(p) if p != j && p < local.len() => local[j].mul(resolve(p, local, parents, out)),
        _ => local[j],
    };
    out[j] = Some(m);
    m
}

/// Every triangle of one LOD group, each vertex placed by the matrix of the joint it names.
///
/// The same expansion [`Model::triangles`] does, with animated matrices instead of the file's own: a
/// `.psg` vertex carries one joint and no weights, so the joint's matrix is all the skinning there is.
pub fn skin(model: &Model, world: &[M4], group: usize) -> Vec<Tri> {
    let mut out = Vec::new();
    for mesh in model.meshes.iter().filter(|m| m.group == group) {
        for strip in &mesh.strips {
            place(strip, mesh.material, world, &mut out);
        }
    }
    out
}

fn place(strip: &Strip, material: usize, world: &[M4], out: &mut Vec<Tri>) {
    for i in 2..strip.verts.len() {
        if strip.verts[i].adc {
            continue;
        }
        let mut t = Tri { v: [strip.verts[i - 2], strip.verts[i - 1], strip.verts[i]], material };
        for v in t.v.iter_mut() {
            if let Some(m) = world.get(v.node) {
                v.pos = m.point(v.pos);
                v.normal = m.dir(v.normal);
            }
        }
        out.push(t);
    }
}

/// `(x, y, z, 1)` as a point.
pub fn point(p: [f32; 4]) -> V3 {
    v3(p[0], p[1], p[2])
}

fn add4(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}

fn sub4(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
}

fn scale4(a: [f32; 4], k: f32) -> [f32; 4] {
    [a[0] * k, a[1] * k, a[2] * k, a[3] * k]
}

/// The sine series' four coefficients, by the bits the image holds: `1, -1/6, 1/120, -1/5040`.
const SIN_POLY: [f32; 4] = [
    f32::from_bits(0x3f80_0000),
    f32::from_bits(0xbe2a_aaab),
    f32::from_bits(0x3c08_8889),
    f32::from_bits(0xb950_0d01),
];

/// What a dot product is multiplied by to index the arc-cosine table.
const ARC_SCALE: f32 = 14999.5;

/// How many entries that table has, two of them past the end of the dot products that reach it.
const ARC_LEN: usize = 30002;

/// The two tables the original's quaternion code reads out of memory the game fills at start-up.
pub struct Tables {
    /// Per turn index: `sin(n·π/4096) / 32767`, and `cos(n·π/4096)` — whose sign matters.
    pub turn: Vec<[f32; 2]>,
    /// The same for the narrow key, 256 entries of `sin(n·π/256) / 127` and `cos(n·π/256)`. It sits
    /// immediately after the wide one in the game's memory, which is how it was found.
    pub narrow: Vec<[f32; 2]>,
    /// `acos(i / 14999.5 - 1)`, which is to say the arc cosine over -1..1 in 30000 steps.
    pub arc: Vec<f32>,
}

/// The tables, built once.
///
/// Rebuilt from `sin`, `cos` and `acos` rather than read out of the game: the builder is not in the
/// image's call graph, and a save state's copies agree with these to 1.8e-7 and 1.1e-5. The last two
/// arc entries are the game's own 2^-11 rather than an arc cosine of anything — the slerp reads
/// index 29999 for any dot product above 0.99997, so the odd value is part of the look.
pub fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        // One builder for both: `steps` turn indices over half a circle, the sine scaled by the axis's
        // own full scale so that an axis at full scale comes out a unit quaternion.
        let table = |steps: usize, scale: f64| -> Vec<[f32; 2]> {
            (0..steps)
                .map(|n| {
                    let a = n as f64 * std::f64::consts::PI / steps as f64;
                    let half = steps / 2;
                    let cos = (a.cos().abs() as f32).to_bits() | if n < half { 0 } else { 0x8000_0000 };
                    [(a.sin() / scale) as f32, f32::from_bits(cos)]
                })
                .collect()
        };
        let turn = table(4096, 32767.0);
        let narrow = table(256, 127.0);
        let mut arc: Vec<f32> = (0..ARC_LEN)
            .map(|i| {
                let x = (i as f64 / ARC_SCALE as f64 - 1.0).clamp(-1.0, 1.0);
                x.acos() as f32
            })
            .collect();
        arc[ARC_LEN - 3] = 0.000_488_281_25;
        arc[ARC_LEN - 2] = 0.000_488_281_25;
        arc[ARC_LEN - 1] = 0.0;
        Tables { turn, narrow, arc }
    })
}

fn i32_at(d: &[u8], p: &mut usize) -> Result<i32, String> {
    Ok(u32_at(d, p)? as i32)
}

fn f32_at(d: &[u8], p: &mut usize) -> Result<f32, String> {
    Ok(f32::from_bits(u32_at(d, p)?))
}

fn u32_at(d: &[u8], p: &mut usize) -> Result<u32, String> {
    let b = d.get(*p..*p + 4).ok_or_else(|| short(*p, d.len()))?;
    *p += 4;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn u8_at(d: &[u8], p: &mut usize) -> Result<u8, String> {
    let b = *d.get(*p).ok_or_else(|| short(*p, d.len()))?;
    *p += 1;
    Ok(b)
}

fn i8_at(d: &[u8], p: &mut usize) -> Result<i8, String> {
    Ok(u8_at(d, p)? as i8)
}

fn u16_at(d: &[u8], p: &mut usize) -> Result<u16, String> {
    let b = d.get(*p..*p + 2).ok_or_else(|| short(*p, d.len()))?;
    *p += 2;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}

fn i16_at(d: &[u8], p: &mut usize) -> Result<i16, String> {
    Ok(u16_at(d, p)? as i16)
}

fn short(at: usize, len: usize) -> String {
    format!("not a .mot: it ends at {len}, inside a field at {at}")
}
