//! The track's animated toys (V43): the `.toy`'s `[AnimatedToyN]` models, each playing one clip on a
//! loop where the track puts it — windmills, a windsock, a gondola lift, the sx tracks' blimps and
//! finish gates — and, for the ones with `HasCollision`, their collision going round with them.
//!
//! The original's toy is a scene node (the animated toy class, `research/oracle.txt` V43) whose update
//! hands the frame's `dt` to its model's motion object (`motion_advance`): the clip's clock goes on,
//! every rotation channel and then every position channel is sampled at the new time and written into
//! the joint's transform node, and the node chain is composed again when something asks for a world
//! matrix (`transform_rebuild`). The load does one advance of `InitialTime` (zero or not) straight
//! after it puts the clip on. A toy with a collision hierarchy (`.col` of kind 1) has one mesh per part,
//! each bound to a bone, so the clip that turns the model turns its collision with it.
//!
//! Here, all bit for bit (`crates/game-sim/tests/toys.rs`):
//!
//! - **the clock**: [`motion::Player::advance`], the proved one;
//! - **the keys**: [`Turns`], the two sine/cosine tables the original builds at startup, and
//!   [`decode`], one rotation key as a quaternion through them;
//! - **the samplers**: [`sample_rotation`] (the hint, its two guesses, the walks, the held ends,
//!   [`math::slerp`] on the shortest path) and [`sample_position`] (the binary search, the linear
//!   blend, the extrapolation past the end);
//! - **the pose**: the joints' local matrices ([`math::quat_to_mat4`]'s rows, the translation's three
//!   lanes) composed down the chain ([`compose`]).
//!
//! The clocks and the rotation channels' hints are state ([`Toys::clocks`], [`Toys::hints`], hashed
//! and sent): a rollback puts a windmill's blades back where they were. The hint is the channel's,
//! not the toy's, so toys that share a clip share it, as in the original, and they are stepped in the
//! `.toy`'s order.
//!
//! Not here: the update's visibility gate (a toy nobody can see, without `AnimateWhenNotVisible`, does
//! not advance in the original; here every toy does), the sounds, `CameraCollision`,
//! `DisableWhenIdle` and `InitialFrame`.

#![deny(clippy::float_arithmetic)] // every sum below is the PS2's, through ps2-float

use std::sync::{Arc, OnceLock};

use ps2_float::{F, Field, Vf, libm};

use crate::air::AcosTable;
use crate::hash::{State, Walk};
use crate::math::{self, Mat4};
use crate::motion::{self, ClipId, Player};
use crate::track::Mesh;
use crate::world::{Prop, Props, World};

/// What a joint map holds for a channel the model has no joint for: the channel is skipped.
pub const SKIP: u8 = 0xff;

// --- the turn tables --------------------------------------------------------------------------------

/// The rotation keys' sine and cosine tables, built as the original's startup builds them: entry `n`
/// of the wide one is `(sinf(n·a) · s, cosf(n·a))` for an angle step `a` of π/4096 and a scale `s` of
/// 1/32767, the narrow one the same in 256ths of π and 127ths, each angle one `mul.s` of `n` as a
/// float, and the sine and cosine the C library's own ([`libm::sinf`], [`libm::cosf`]).
///
/// Half the rotation angle is the index, so the cosine is the quaternion's `w` and the scaled sine
/// what the stored axis is multiplied by.
pub struct Turns {
    pub wide: Vec<[F; 2]>,
    pub narrow: Vec<[F; 2]>,
}

impl Turns {
    fn build() -> Turns {
        let table = |n: i32, step: u32, scale: u32| -> Vec<[F; 2]> {
            (0..n)
                .map(|i| {
                    let angle = F::from_bits(step) * F(ps2_float::fpu::from_i32(i));
                    [F::from_bits(scale) * F(libm::sinf(angle.0)), F(libm::cosf(angle.0))]
                })
                .collect()
        };
        Turns { wide: table(4096, 0x3a49_0fdb, 0x3800_0100), narrow: table(256, 0x3c49_0fdb, 0x3c01_0204) }
    }

    /// The tables, built the first time they are asked for.
    pub fn get() -> &'static Turns {
        static TURNS: OnceLock<Turns> = OnceLock::new();
        TURNS.get_or_init(Turns::build)
    }

    /// A wide key's entry. The original indexes without a mask, and the narrow table lies straight
    /// after the wide one, so the 256 indices past the end are the narrow table's; past those it reads
    /// whatever follows, which here wraps instead (no key on the disc gets there).
    fn wide_at(&self, turn: u16) -> [F; 2] {
        let n = turn as usize;
        match self.wide.get(n) {
            Some(e) => *e,
            None => self.narrow.get(n - self.wide.len()).copied().unwrap_or(self.wide[n & 0xfff]),
        }
    }
}

fn acos_table() -> &'static AcosTable {
    static TABLE: OnceLock<AcosTable> = OnceLock::new();
    TABLE.get_or_init(AcosTable::build)
}

// --- the keys ---------------------------------------------------------------------------------------

/// One rotation key of the wide kind: a time and a quaternion in eight bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotKey {
    pub t: F,
    /// Index into the wide turn table: half the angle in 4096ths of π.
    pub turn: u16,
    /// The axis, scaled to ±32767.
    pub axis: [i16; 3],
}

/// One rotation key of the narrow kind: the same in four bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NarrowKey {
    pub t: F,
    /// Index into the narrow turn table: half the angle in 256ths of π.
    pub turn: u8,
    /// The axis, scaled to ±127.
    pub axis: [i8; 3],
}

/// A rotation channel's keys, in whichever kind its clip holds.
#[derive(Clone, Debug, PartialEq)]
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
    fn t(&self, i: usize) -> F {
        match self {
            RotKeys::Wide(k) => k[i].t,
            RotKeys::Narrow(k) => k[i].t,
        }
    }
    fn quat(&self, i: usize) -> Vf {
        let turns = Turns::get();
        match self {
            RotKeys::Wide(k) => decode(turns.wide_at(k[i].turn), k[i].axis.map(i32::from)),
            RotKeys::Narrow(k) => decode(turns.narrow[k[i].turn as usize], k[i].axis.map(i32::from)),
        }
    }
}

/// One rotation key as a quaternion: the cosine's sign folded into the sine (so `w`, the cosine's
/// magnitude, is never negative), the axis converted to floats (`vitof0`) and multiplied by that sine
/// in VU0 (`vmulx`). Both key kinds decode this way.
pub fn decode(entry: [F; 2], axis: [i32; 3]) -> Vf {
    let [sin, cos] = entry.map(F::to_bits);
    let sign = cos & 0x8000_0000;
    let s = f32::from_bits(sin ^ sign);
    let w = f32::from_bits(cos ^ sign);
    let v = Vf::itof([axis[0], axis[1], axis[2], 0], 0).mul(Vf::splat(s));
    Vf::new(v.x(), v.y(), v.z(), w)
}

/// One position key: a time and a translation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PosKey {
    pub t: F,
    pub p: [F; 3],
}

/// A joint's rotation over a clip.
#[derive(Clone, Debug, PartialEq)]
pub struct RotChannel {
    /// The joint name's index in the clip, which the model's joint map turns into a joint.
    pub name: u32,
    /// The channel's on flag: an off channel writes nothing.
    pub on: bool,
    pub keys: RotKeys,
}

/// A joint's translation over a clip.
#[derive(Clone, Debug, PartialEq)]
pub struct PosChannel {
    pub name: u32,
    pub on: bool,
    pub keys: Vec<PosKey>,
}

/// One clip: its timing and its channels.
#[derive(Clone, Debug, PartialEq)]
pub struct Motion {
    pub timing: motion::Clip,
    pub rotation: Vec<RotChannel>,
    pub position: Vec<PosChannel>,
}

// --- the samplers -----------------------------------------------------------------------------------

/// A rotation channel at `time`, from the hint `cursor` (which it moves), or `None` when the channel
/// writes nothing (off, or no keys).
///
/// The original's `motion_rot_sample`: with three keys or more, the hint is thrown away by two guesses
/// (a hint past three quarters of the keys with a time before the first quarter starts again at 1; a
/// hint before the first quarter with a time after three quarters jumps to the end), then a walk
/// forward and one back find the pair. A walk that runs off either end holds that key — before the
/// first that is key *1*. One or two keys blend the first and the last; the original weights a pair
/// from a time stored in front of the array, which only one disc channel reaches (a windsock's), and
/// the first key's time is used here instead.
// The negated comparisons are the original's `c.olt.s`/`c.ole.s` branches on false.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn sample_rotation(ch: &RotChannel, time: F, cursor: &mut u32) -> Option<Vf> {
    let k = &ch.keys;
    if !ch.on || k.is_empty() {
        return None;
    }
    let n = k.len();
    if n < 3 {
        let (u, b) = if n == 2 { ((time - k.t(0)) / (k.t(1) - k.t(0)), 1) } else { (F::ZERO, 0) };
        return Some(math::slerp(k.quat(0), k.quat(b), u, true, acos_table()));
    }
    let last = n as i32 - 1;
    let quarter = last >> 2;
    let three_q = (last >> 1) + quarter;
    let mut hint = *cursor as i32;
    if three_q < hint {
        if time < k.t(quarter as usize) {
            hint = 1;
        }
    } else if hint < quarter && !(time <= k.t(three_q as usize)) {
        hint = last;
    }
    let mut i = if hint > 0 { hint } else { 1 };
    let mut blend = true;
    if !(time <= k.t(i as usize)) {
        loop {
            i += 1;
            if !(i < n as i32) {
                i = last;
                blend = false;
                break;
            }
            if time <= k.t(i as usize) {
                break;
            }
        }
    }
    while time < k.t(i as usize - 1) {
        i -= 1;
        if i <= 0 {
            i = 1;
            blend = false;
            break;
        }
    }
    *cursor = i as u32;
    let i = i as usize;
    if !blend {
        return Some(k.quat(i));
    }
    let u = (time - k.t(i - 1)) / (k.t(i) - k.t(i - 1));
    Some(math::slerp(k.quat(i - 1), k.quat(i), u, true, acos_table()))
}

/// A position channel at `time` as `(x, y, z)`, or `None` when the channel writes nothing.
///
/// The original's `motion_pos_sample` over its bracket's binary search: inside the keys, `a + u·(b − a)`
/// for the pair the time falls between; at or past the last key, `b + u·(b − a)` over the last pair
/// with `u` counted from the last key, so the last interval's slope carries on. A time before the
/// first key brackets a key in front of the array in the original; here the first pair is used. The
/// original's path for an off channel or a single key writes the key's `z` into `x`; no channel on the
/// disc takes it, and here it writes nothing.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn sample_position(ch: &PosChannel, time: F) -> Option<[F; 3]> {
    let k = &ch.keys;
    if !ch.on || k.len() < 2 {
        return None;
    }
    let n = k.len();
    let (a, b, u, inside) = if !(time < k[n - 1].t) {
        (n - 2, n - 1, (time - k[n - 1].t) / (k[n - 1].t - k[n - 2].t), false)
    } else {
        let i = if time < k[0].t { 1 } else { bracket(k, time) };
        (i - 1, i, (time - k[i - 1].t) / (k[i].t - k[i - 1].t), true)
    };
    let (pa, pb) = (k[a].p, k[b].p);
    Some(std::array::from_fn(|c| {
        let base = if inside { pa[c] } else { pb[c] };
        base + u * (pb[c] - pa[c])
    }))
}

/// The bracket's binary search, for a time inside the keys: the `i` with `t[i−1] ≤ time < t[i]`, found
/// the original's way, so that two keys on one time give the original's pair.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn bracket(k: &[PosKey], time: F) -> usize {
    let (mut lo, mut hi) = (0usize, k.len());
    loop {
        let mid = (lo + hi) >> 1;
        if time < k[mid].t {
            if mid == 0 || !(time < k[mid - 1].t) {
                return mid.max(1);
            }
            hi = mid;
        } else {
            if mid + 1 >= k.len() || time < k[mid + 1].t {
                return (mid + 1).min(k.len() - 1);
            }
            lo = mid;
        }
    }
}

// --- the pose ---------------------------------------------------------------------------------------

/// A child's world matrix from its local one and its parent's world, as `transform_rebuild` composes
/// it in VU0: each rotation row `P0·l.x + P1·l.y + P2·l.z` (`vmulax`, `vmadday`, `vmaddz`), the
/// translation row the same begun from `P3` (`vaddax` with zero).
pub fn compose(local: &Mat4, parent: &Mat4) -> Mat4 {
    let [p0, p1, p2, p3] = parent.0;
    // `vmulax` opens a rotation row with a product, not a sum onto zero (they differ for a `−0`).
    let rest = |acc: Vf, l: Vf| Vf::madd(Vf::madd(acc, p1, l.bc(Field::Y)), p2, l.bc(Field::Z));
    let rotation = |l: Vf| rest(p0.mul(l.bc(Field::X)), l);
    let [l0, l1, l2, l3] = local.0;
    let translation = rest(Vf::madd(p3.add(Vf::ZERO), p0, l3.bc(Field::X)), l3);
    Mat4([rotation(l0), rotation(l1), rotation(l2), translation])
}

/// A model's skeleton as the toy's motion object holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct Rig {
    /// Each joint's parent joint, or `None` for a joint on the model's root (the toy's placement).
    pub parents: Vec<Option<usize>>,
    /// Each joint's local matrix as the model loads it; a channel overwrites its part every sample.
    pub rest: Vec<Mat4>,
    /// The clip's joint map: for each channel name index, the joint, or [`SKIP`].
    pub map: Vec<u8>,
}

impl Rig {
    fn joint(&self, name: u32) -> Option<usize> {
        match self.map.get(name as usize) {
            Some(&j) if j != SKIP && (j as usize) < self.rest.len() => Some(j as usize),
            _ => None,
        }
    }
}

/// One collision part, riding a joint.
#[derive(Clone, Debug)]
pub struct Part {
    /// The joint, or `None` for the toy's root node itself (the hierarchy's bone 0): the placement.
    pub joint: Option<usize>,
    pub mesh: Mesh,
}

/// A toy's constants: its clip, its skeleton, where it stands and what collides.
#[derive(Clone, Debug)]
pub struct Spec {
    /// Index into [`Toys::motions`].
    pub motion: usize,
    pub rig: Rig,
    /// The toy's matrix: its root node's, and so every joint's parent at the top.
    pub place: Mat4,
    /// `InitialTime`: the one advance the load does.
    pub start: F,
    /// A `.col` mesh (kind 0): it stands at the placement and never moves.
    pub mesh: Option<Mesh>,
    /// A `.col` hierarchy's parts (kind 1): each follows its joint.
    pub parts: Vec<Part>,
}

impl Spec {
    /// Where a collision part stands: its joint's world matrix, or the placement for the root's.
    fn at(&self, part: &Part, worlds: &[Mat4]) -> Mat4 {
        match part.joint {
            Some(j) if j < worlds.len() => worlds[j],
            _ => self.place,
        }
    }
}

/// A toy's clock: its motion object's.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Clock {
    pub time: F,
    pub prev: F,
    pub finished: bool,
}

/// One rotation channel's hint (the channel's `+0x18` cursor): which pair its last sample used.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hint(pub u32);

/// The track's animated toys: their clips and constants, their clocks, and where their collision is.
#[derive(Clone, Default)]
pub struct Toys {
    /// The clips, one per distinct clip of a model: a constant of the track.
    pub motions: Arc<Vec<Motion>>,
    /// Every toy's constants, in the `.toy`'s order.
    pub specs: Arc<Vec<Spec>>,
    /// Where each motion's rotation hints begin in [`Toys::hints`].
    pub base: Arc<Vec<usize>>,
    /// Every toy's clock.
    pub clocks: Vec<Clock>,
    /// Every motion's rotation channels' hints, one after another.
    pub hints: Vec<Hint>,
    /// Where the toys' meshes begin in the world's props ([`Toys::attach`]).
    pub first: Option<usize>,
}

impl Toys {
    /// The toys as the track's load leaves them: each clip put on at zero and advanced once by its
    /// `InitialTime`, in order.
    pub fn new(motions: Vec<Motion>, specs: Vec<Spec>) -> Toys {
        let mut base = Vec::with_capacity(motions.len());
        let mut n = 0;
        for m in &motions {
            base.push(n);
            n += m.rotation.len();
        }
        let mut toys = Toys {
            clocks: vec![Clock::default(); specs.len()],
            hints: vec![Hint(1); n],
            motions: Arc::new(motions),
            specs: Arc::new(specs),
            base: Arc::new(base),
            first: None,
        };
        for k in 0..toys.specs.len() {
            let start = toys.specs[k].start;
            toys.advance(k, start);
        }
        toys
    }

    /// Whether the track has any.
    pub fn is_empty(&self) -> bool {
        self.clocks.is_empty()
    }

    /// One toy's clock advanced by `dt` and its channels sampled: the joints' local matrices it leaves,
    /// or `None` once a clip that does not loop has finished (nothing is sampled again).
    pub fn advance(&mut self, k: usize, dt: F) -> Option<Vec<Mat4>> {
        let spec = &self.specs[k];
        let motion = &self.motions[spec.motion];
        let c = &mut self.clocks[k];
        let mut player = Player {
            clip: Some(ClipId(spec.motion as u32)),
            time: c.time,
            prev: c.prev,
            finished: c.finished,
            ..Player::default()
        };
        if player.finished {
            return None;
        }
        let played = player.advance(dt, &motion.timing);
        // The finishing advance samples before it puts the clock back to zero.
        let time = if played.finished { player.prev + dt } else { player.time };
        (c.time, c.prev, c.finished) = (player.time, player.prev, player.finished);
        let hints = &mut self.hints[self.base[spec.motion]..];
        Some(pose(motion, &spec.rig, time, hints))
    }

    /// Every joint's world matrix from its local one, down the chain from the toy's placement.
    pub fn worlds(&self, k: usize, locals: &[Mat4]) -> Vec<Mat4> {
        let spec = &self.specs[k];
        let mut out: Vec<Mat4> = Vec::with_capacity(locals.len());
        for (j, local) in locals.iter().enumerate() {
            let parent = match spec.rig.parents[j] {
                Some(p) if p < j => out[p],
                _ => spec.place,
            };
            out.push(compose(local, &parent));
        }
        out
    }

    /// Put the toys' meshes at the end of the world's props, each where its toy stands at load.
    pub fn attach(&mut self, props: &mut Props) {
        self.first = Some(props.props.len());
        for (k, spec) in self.specs.iter().enumerate() {
            if let Some(mesh) = &spec.mesh {
                props.props.push(Prop::on(mesh.clone(), &spec.place));
            }
            if !spec.parts.is_empty() {
                let locals = pose(&self.motions[spec.motion], &spec.rig, self.clocks[k].time, &mut self.hints.clone());
                let worlds = self.worlds(k, &locals);
                for part in &spec.parts {
                    props.props.push(Prop::on(part.mesh.clone(), &spec.at(part, &worlds)));
                }
            }
        }
    }

    /// One tick of every toy, in order, and the parts that follow a joint moved with it.
    pub fn frame(&mut self, dt: F, world: &mut dyn World) {
        if self.clocks.is_empty() {
            return;
        }
        let mut props = world.moving();
        let mut at = self.first;
        for k in 0..self.specs.len() {
            let locals = self.advance(k, dt);
            let spec = Arc::clone(&self.specs);
            let spec = &spec[k];
            if spec.mesh.is_some() {
                at = at.map(|a| a + 1);
            }
            if spec.parts.is_empty() {
                continue;
            }
            if let (Some(locals), Some(props), Some(a)) = (locals, props.as_deref_mut(), at) {
                let worlds = self.worlds(k, &locals);
                for (p, part) in spec.parts.iter().enumerate() {
                    if let Some(prop) = props.props.get_mut(a + p) {
                        prop.stand(&spec.at(part, &worlds));
                    }
                }
            }
            at = at.map(|a| a + spec.parts.len());
        }
    }
}

/// The joints' local matrices at `time`: the rest pose with every rotation channel's rows and then
/// every position channel's translation written over it, in the clip's order, as `motion_advance`
/// writes them (`transform_set_rotation`'s [`math::quat_to_mat4`] rows; three lanes of the
/// translation row, its `w` left alone). `hints` begins at the motion's first rotation hint.
pub fn pose(motion: &Motion, rig: &Rig, time: F, hints: &mut [Hint]) -> Vec<Mat4> {
    let mut locals = rig.rest.clone();
    for (ch, hint) in motion.rotation.iter().zip(hints.iter_mut()) {
        let Some(j) = rig.joint(ch.name) else { continue };
        if let Some(q) = sample_rotation(ch, time, &mut hint.0) {
            let m = math::quat_to_mat4(q, Vf::ZERO);
            locals[j].0[..3].copy_from_slice(&m.0[..3]);
        }
    }
    for ch in &motion.position {
        let Some(j) = rig.joint(ch.name) else { continue };
        if let Some(p) = sample_position(ch, time) {
            let w = locals[j].0[3].w();
            locals[j].0[3] = Vf::new(p[0].0, p[1].0, p[2].0, w);
        }
    }
    locals
}

impl State for Clock {
    fn walk(&self, w: &mut Walk<'_>) {
        w.f("time", self.time);
        w.f("prev", self.prev);
        w.bool("finished", self.finished);
    }
}

impl State for Hint {
    fn walk(&self, w: &mut Walk<'_>) {
        w.u32("hint", self.0);
    }
}
