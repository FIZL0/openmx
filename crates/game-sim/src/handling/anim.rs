//! **SLUS-21104's rider animation controller on RAM** (F43a): the three calls the bike's vehicle
//! update ([`super::update::prepare`]) makes on the rider's controller every frame, as word-level
//! ports over the controller, its clips and its bones' scene nodes in memory, in the update's call
//! shapes (no mirror, no channel mask, no pose handed in, every channel, no clip events). Where the
//! original keeps them: research/oracle.txt `# F43a`.
//!
//! - [`stop`]: every bone's own rotation and position saved, the pose a clip then fades out of.
//! - [`play`]: one clip at a fraction of its length, faded towards the saved pose.
//! - [`blend`]: two clips at the same fraction, mixed bone by bone, then the same fade.
//! - [`rotation`] and [`position`]: one channel sampled at a time, the two channel kinds the
//!   rider's clips are made of.
//! - [`advance`] and [`start`] (F62b): the controller's own clip advance (its vt `+0x88`: the clip
//!   run on by `dt`, sampled, and crossfaded from the saved pose or the clip before it while `+0x2d`
//!   bit 1 holds) and a clip started with a crossfade of a span (`+0x68` clock, `+0x6c` end, `+0x70`
//!   start, `+0x74` span, `+0x40` the old clip); research/oracle.txt `# F62b`.
//!
//! The controller's words: its flag bytes (`+0x2c`: bit 4 a per-bone fade list in use; `+0x2d`: bit 5
//! the bones' nodes turned, bit 4 placed), `+0x3c` (cleared by a play or blend), the last clip and
//! time (`+0x44`, `+0x48`), the bone count (`+0x1f8`) and bones (`+0x1fc`, 0x40 bytes each: the node,
//! the position at `+0x10`, the rotation at `+0x20`), the saved rotations and positions (`+0x208`,
//! `+0x20c`, a quadword a bone) and the fade list (`+0x210`, a word a bone). A clip: its channel-to-
//! bone byte map (`+0`) and its data (`+4`: position channels `+0x18`, rotation channels `+0x1c`, their
//! counts `+0x24` and `+0x28`, its length `+0x44`). A channel: its name (`+0`, an index into the map),
//! on (`+4`), key count (`+8`), keys (`+0x14`) and, for a rotation, the key it last stopped at
//! (`+0x18`).
//!
//! **Words the original never wrote.** A rotation channel that is off or has no keys leaves the
//! play's (or blend's) quaternion as the previous channel left it, and a position channel that is
//! off or has one key writes only two of its three words (its `x` is the key's `z`). The first
//! channel of a call then reads whatever the stack held; the port starts those at zero.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use super::node::{self, Ram};
use super::update::Quad;
use crate::air::AcosTable;
use crate::math::{self, Mat4};
use crate::toys::{self, Turns};
use ps2_float::fpu::Acc;
use ps2_float::{F, Vf};

/// What the controller's memory needs beyond words: the saved poses' buffers, made on the first
/// [`stop`] (the original's heap, a quadword a bone).
pub trait Anim: Ram {
    fn alloc(&mut self, bytes: u32, buffer: Saved) -> u32;
}

/// The two saved-pose buffers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Saved {
    Rotations,
    Positions,
}

/// The two startup tables the keys are read through, and the word that tells a rotation channel of
/// the narrow kind (F62a): its sampler, the channel's vtable (`+0x10`) at `+0x10`. Every other
/// rotation channel is of the wide kind.
#[derive(Clone, Copy)]
pub struct Tables<'a> {
    pub turns: &'a Turns,
    pub acos: &'a AcosTable,
    pub narrow: u32,
}

const FADE_LIST_ON: u32 = 0x10;
const TURN_NODES: u32 = 0x20;
const PLACE_NODES: u32 = 0x10;
const BONE: u32 = 0x40;

fn quad(m: &mut impl Ram, at: u32) -> Quad {
    [0, 4, 8, 12].map(|k| m.word(at.wrapping_add(k)))
}

fn put_quad(m: &mut impl Ram, at: u32, q: Quad) {
    for (k, w) in q.into_iter().enumerate() {
        m.set(at.wrapping_add(4 * k as u32), w);
    }
}

/// The word at `at`'s word plus `plus`.
fn deref(m: &mut impl Ram, at: u32, plus: u32) -> u32 {
    let p = m.word(at);
    m.word(p.wrapping_add(plus))
}

/// The address `at`'s word plus `plus`.
fn offset(m: &mut impl Ram, at: u32, plus: u32) -> u32 {
    m.word(at).wrapping_add(plus)
}

/// A bone's saved rotation (`buffer` 0x208) or position (0x20c).
fn saved(m: &mut impl Ram, ctrl: u32, buffer: u32, bone: u32) -> Quad {
    let at = offset(m, ctrl + buffer, 16 * bone);
    quad(m, at)
}

fn f(m: &mut impl Ram, at: u32) -> F {
    F::from_bits(m.word(at))
}

/// A byte of a little-endian word.
fn byte(m: &mut impl Ram, at: u32) -> u32 {
    (m.word(at & !3) >> (8 * (at & 3))) & 0xff
}

fn put_byte(m: &mut impl Ram, at: u32, b: u32) {
    let (a, s) = (at & !3, 8 * (at & 3));
    let w = m.word(a);
    m.set(a, (w & !(0xff << s)) | ((b & 0xff) << s));
}

/// The node's own rotation as a quaternion (its 3×3 by largest component, [`math::mat4_to_quat`]).
fn node_rotation(m: &mut impl Ram, n: u32) -> Quad {
    let rows = [0, 1, 2, 3].map(|r| Vf::from_bits(quad(m, n + 0x30 + 0x10 * r)));
    math::mat4_to_quat(&Mat4(rows)).to_bits()
}

/// The node's own rotation set from a quaternion: the three rotation rows [`math::quat_to_mat4`]'s
/// (`w` 0), the position row left; then [`node::moved`].
pub(crate) fn turn_node(m: &mut impl Ram, n: u32, q: Quad) {
    let rows = math::quat_to_mat4(Vf::from_bits(q), Vf::ZERO).0;
    for (r, row) in rows.iter().take(3).enumerate() {
        put_quad(m, n + 0x30 + 0x10 * r as u32, row.to_bits());
    }
    node::moved(m, n);
}

/// **The controller stopped**: the saved buffers made if there are none, the fade list put out of
/// use, and every bone's own rotation ([`math::mat4_to_quat`]) and position (`xyz`, `w` 1) saved.
pub fn stop(m: &mut impl Anim, ctrl: u32) {
    for (at, buffer) in [(0x208, Saved::Rotations), (0x20c, Saved::Positions)] {
        if m.word(ctrl + at) == 0 {
            let bytes = m.word(ctrl + 0x1f8) << 4;
            let b = m.alloc(bytes, buffer);
            m.set(ctrl + at, b);
        }
    }
    let flags = byte(m, ctrl + 0x2c);
    put_byte(m, ctrl + 0x2c, flags & !FADE_LIST_ON);
    let mut i = 0;
    while (i as i32) < m.word(ctrl + 0x1f8) as i32 {
        let n = deref(m, ctrl + 0x1fc, BONE * i);
        let q = node_rotation(m, n);
        let at = m.word(ctrl + 0x208).wrapping_add(16 * i);
        put_quad(m, at, q);
        let mut p = quad(m, n + 0x60);
        p[3] = F::ONE.to_bits();
        let at = m.word(ctrl + 0x20c).wrapping_add(16 * i);
        put_quad(m, at, p);
        i += 1;
    }
}

/// One rotation key (8 bytes at `at`: the turn table's index, the axis's three halves) as a
/// quaternion ([`toys::decode`]).
fn key(m: &mut impl Ram, t: &Tables, at: u32) -> Vf {
    let lo = m.word(at);
    let hi = m.word(at.wrapping_add(4));
    let axis = [(lo >> 16) as i16, hi as i16, (hi >> 16) as i16].map(i32::from);
    toys::decode(t.turns.wide_at(lo as u16), axis)
}

/// One narrow rotation key (4 bytes at `at`: the narrow turn table's index, the axis's three signed
/// bytes) as a quaternion, the same way (F62a).
fn narrow_key(m: &mut impl Ram, t: &Tables, at: u32) -> Vf {
    let w = m.word(at);
    let axis = [(w >> 8) as i8, (w >> 16) as i8, (w >> 24) as i8].map(i32::from);
    toys::decode(t.turns.narrow[(w & 0xff) as usize], axis)
}

/// **A rotation channel at `time`** (keys of 12 bytes: the time, then the key; of 8 for the narrow
/// kind, [`Tables::narrow`]), or nothing for an off or empty channel. Fewer than three keys: the first two slerped (two keys: by the time from the
/// word *before* the keys, the original's, over their span; one: the key with itself). Three or
/// more: from the key it stopped at last (put back to 1 when that is past three quarters and the
/// time before the first quarter, to the last when it is before the first quarter and the time past
/// three quarters), walked forward and then back to the pair around the time; the pair slerped
/// the short way ([`math::slerp`]), or the end key alone when a walk ran off the end.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn rotation(m: &mut impl Ram, t: &Tables, ch: u32, time: F) -> Option<Quad> {
    if m.word(ch + 4) == 0 {
        return None;
    }
    let n = m.word(ch + 8) as i32;
    if n <= 0 {
        return None;
    }
    let keys = m.word(ch + 0x14);
    // The narrow kind's keys are 8 bytes (F62a), the same walk over them.
    let vt = m.word(ch + 0x10);
    let narrow = m.word(vt.wrapping_add(0x10)) == t.narrow;
    let size = if narrow { 8 } else { 12 };
    let at = |i: i32| keys.wrapping_add(i.wrapping_mul(size) as u32);
    let (a, b, u, slerp) = if n < 3 {
        if n == 2 {
            let u = (time - f(m, at(-1))) / (f(m, at(1)) - f(m, at(0)));
            (at(0), at(1), u, true)
        } else {
            (at(0), at(0), F::ZERO, true)
        }
    } else {
        let last = n - 1;
        let quarter = last >> 2;
        let three = (last >> 1) + quarter;
        let mut i = m.word(ch + 0x18) as i32;
        if three < i {
            if time < f(m, at(quarter)) {
                i = 1;
            }
        } else if i < quarter && !(time <= f(m, at(three))) {
            i = last;
        }
        if i <= 0 {
            i = 1;
        }
        let mut slerp = true;
        if !(time <= f(m, at(i))) {
            loop {
                i += 1;
                if i >= n {
                    i = last;
                    slerp = false;
                    break;
                }
                if time <= f(m, at(i)) {
                    break;
                }
            }
        }
        while time < f(m, at(i - 1)) {
            i -= 1;
            if i <= 0 {
                i = 1;
                slerp = false;
                break;
            }
        }
        m.set(ch + 0x18, i as u32);
        let lo = f(m, at(i - 1));
        let u = (time - lo) / (f(m, at(i)) - lo);
        (at(i - 1), at(i), u, slerp)
    };
    let mut key = |m: &mut _, at: u32| if narrow { narrow_key(m, t, at) } else { key(m, t, at) };
    let qb = key(m, b.wrapping_add(4));
    if !slerp {
        return Some(qb.to_bits());
    }
    let qa = key(m, a.wrapping_add(4));
    Some(math::slerp(qa, qb, u, true, t.acos).to_bits())
}

/// The key `i` of a position channel's keys (16 bytes each: the time, `x`, `y`, `z`) where the time
/// falls, by the original's binary search over `n` keys: `n` at or past the last key's time, 0 before
/// the first's or with no keys, else the `i` with `t[i−1] ≤ time < t[i]`.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn bracket(m: &mut impl Ram, keys: u32, n: i32, time: F) -> i32 {
    let mut t = |i: i32| F::from_bits(m.word(keys.wrapping_add(i.wrapping_mul(16) as u32)));
    if n == 0 {
        return 0;
    }
    if !(time < t(n - 1)) {
        return n;
    }
    if time < t(0) {
        return 0;
    }
    let (mut lo, mut hi) = (0i32, n);
    loop {
        let mid = lo.wrapping_add(hi) >> 1;
        if time < t(mid) {
            if !(time < t(mid - 1)) {
                return mid;
            }
            hi = mid;
        } else {
            if time < t(mid + 1) {
                return mid + 1;
            }
            lo = mid;
        }
    }
}

/// **A position channel at `time`** into `out`. On with two keys or more: the pair around the time
/// ([`bracket`]; past the end the last two, extrapolated from the last), each lane `a + u·(b − a)`
/// through the accumulator (from `b` past the end). Off, or fewer keys: the key before the time
/// held, as the original copies it — `y` the key's, `x` the key's `z`, `z` left. `w` is 1.
pub fn position(m: &mut impl Ram, ch: u32, time: F, out: &mut Quad) {
    let n = m.word(ch + 8) as i32;
    let keys = m.word(ch + 0x14);
    let at = |i: i32| keys.wrapping_add(i.wrapping_mul(16) as u32);
    if m.word(ch + 4) != 0 && n >= 2 {
        let i = bracket(m, keys, n, time);
        let (a, b, inside) = if i == n { (at(n - 2), at(n - 1), false) } else { (at(i - 1), at(i), true) };
        let (ta, tb) = (f(m, a), f(m, b));
        let u = if inside { (time - ta) / (tb - ta) } else { (time - tb) / (tb - ta) };
        for c in 0..3u32 {
            let (pa, pb) = (f(m, a + 4 + 4 * c), f(m, b + 4 + 4 * c));
            let base = if inside { pa } else { pb };
            out[c as usize] = Acc::adda(F::ZERO, base).madd(u, pb - pa).to_bits();
        }
    } else {
        let i = bracket(m, keys, n, time);
        let k = at(i);
        out[0] = m.word(k.wrapping_sub(12));
        out[1] = m.word(k.wrapping_sub(8));
        out[0] = m.word(k.wrapping_sub(4));
    }
    out[3] = F::ONE.to_bits();
}

/// Whether the bone `src` (the map's) fades: the fade list out of use or naming it off, a fade above
/// zero, and a saved buffer at `saved`.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn fades(m: &mut impl Ram, ctrl: u32, src: u32, fade: F, saved: u32) -> bool {
    let list = m.word(ctrl + 0x210);
    let listed = byte(m, ctrl + 0x2c) & FADE_LIST_ON != 0 && list != 0 && m.word(list.wrapping_add(4 * src)) != 0;
    !listed && !(fade <= F::ZERO) && m.word(ctrl + saved) != 0
}

/// A bone's rotation stored, faded first (slerped the short way towards the saved one by `fade`),
/// and its node turned if the controller turns them.
fn put_rotation(m: &mut impl Ram, t: &Tables, ctrl: u32, src: u32, bone: u32, mut q: Quad, fade: F) -> Quad {
    if fades(m, ctrl, src, fade, 0x208) {
        let saved = saved(m, ctrl, 0x208, bone);
        q = math::slerp(Vf::from_bits(q), Vf::from_bits(saved), fade, true, t.acos).to_bits();
    }
    let at = m.word(ctrl + 0x1fc).wrapping_add(BONE * bone);
    put_quad(m, at + 0x20, q);
    if byte(m, ctrl + 0x2d) & TURN_NODES != 0 {
        let n = m.word(at);
        turn_node(m, n, q);
    }
    q
}

/// A bone's position (already stored) faded towards the saved one, `xyz` `p + (saved − p)·fade` on
/// VU0, and its node placed if the controller places them. The saved position's words when it fades
/// (a blend's second sample is left holding them).
fn fade_position(m: &mut impl Ram, ctrl: u32, src: u32, bone: u32, fade: F) -> Option<Quad> {
    let at = m.word(ctrl + 0x1fc).wrapping_add(BONE * bone);
    let mut held = None;
    if fades(m, ctrl, src, fade, 0x20c) {
        let words = saved(m, ctrl, 0x20c, bone);
        let p = quad(m, at + 0x10);
        let pv = Vf::from_bits(p);
        let s = Vf::from_bits([fade.to_bits(); 4]);
        let r = pv.add(Vf::from_bits(words).sub(pv).mul(s)).to_bits();
        put_quad(m, at + 0x10, [r[0], r[1], r[2], p[3]]);
        held = Some(words);
    }
    if byte(m, ctrl + 0x2d) & PLACE_NODES != 0 {
        let n = m.word(at);
        let p = quad(m, at + 0x10);
        node::set_position(m, n, p);
    }
    held
}

/// A clip's `k`th channel in its list at `list` (the data's `+0x18` or `+0x1c`), and the bone its
/// name maps to.
fn channel(m: &mut impl Ram, clip: u32, list: u32, k: u32) -> (u32, u32) {
    let data = m.word(clip + 4);
    let ch = deref(m, data + list, 4 * k);
    let name = m.word(ch);
    let map = m.word(clip);
    (ch, byte(m, map.wrapping_add(name)))
}

const NONE: u32 = 0xff;

/// The tail both calls end with: the last clip and its time.
fn finish(m: &mut impl Ram, ctrl: u32, clip: u32, time: F) {
    m.set(ctrl + 0x44, clip);
    m.set(ctrl + 0x48, time.to_bits());
}

/// **One clip played** at `weight` of its length: every rotation channel with a bone sampled
/// ([`rotation`]; an off one keeps the last quaternion), faded and stored ([`put_rotation`]), then
/// every position channel with a bone sampled into the bone ([`position`]) and faded
/// ([`fade_position`]).
pub fn play(m: &mut impl Ram, t: &Tables, ctrl: u32, clip: u32, weight: F, fade: F) {
    m.set(ctrl + 0x3c, 0);
    let data = m.word(clip + 4);
    let time = weight * f(m, data + 0x44);
    let mut q: Quad = [0; 4];
    let mut k = 0;
    while (k as i32) < deref(m, clip + 4, 0x28) as i32 {
        let (ch, bone) = channel(m, clip, 0x1c, k);
        if bone != NONE {
            if let Some(r) = rotation(m, t, ch, time) {
                q = r;
            }
            q = put_rotation(m, t, ctrl, bone, bone, q, fade);
        }
        k += 1;
    }
    let mut k = 0;
    while (k as i32) < deref(m, clip + 4, 0x24) as i32 {
        let (ch, bone) = channel(m, clip, 0x18, k);
        if bone != NONE {
            let at = m.word(ctrl + 0x1fc).wrapping_add(BONE * bone) + 0x10;
            let mut p = quad(m, at);
            position(m, ch, time, &mut p);
            put_quad(m, at, p);
            fade_position(m, ctrl, bone, bone, fade);
        }
        k += 1;
    }
    finish(m, ctrl, clip, time);
}

/// **Two clips blended** at `weight` of the first's length, `mix` of the way from the second to the
/// first: channel `k` of each sampled, the rotations slerped the short way from the second's
/// ([`math::slerp`]) and the positions `b − (b − a)·mix` (`xyz` on VU0, `w` the second's), into the
/// bone the first clip's channel maps to; then each faded and stored as [`play`] does. Channels are
/// paired by their place in the lists and counted by the first clip's.
pub fn blend(m: &mut impl Ram, t: &Tables, ctrl: u32, clips: [u32; 2], weight: F, mix: F, fade: F) {
    let [a, b] = clips;
    m.set(ctrl + 0x3c, 0);
    let data = m.word(a + 4);
    let time = f(m, data + 0x44) * weight;
    let (mut qa, mut qb): (Quad, Quad) = ([0; 4], [0; 4]);
    let mut k = 0;
    while (k as i32) < deref(m, a + 4, 0x28) as i32 {
        let (cha, bone) = channel(m, a, 0x1c, k);
        if let Some(r) = rotation(m, t, cha, time) {
            qa = r;
        }
        let (chb, _) = channel(m, b, 0x1c, k);
        if let Some(r) = rotation(m, t, chb, time) {
            qb = r;
        }
        qb = math::slerp(Vf::from_bits(qb), Vf::from_bits(qa), mix, true, t.acos).to_bits();
        if bone != NONE {
            // The fade's slerp loads the saved rotation into the first clip's quaternion.
            if fades(m, ctrl, bone, fade, 0x208) {
                qa = saved(m, ctrl, 0x208, bone);
            }
            qb = put_rotation(m, t, ctrl, bone, bone, qb, fade);
        }
        k += 1;
    }
    let mut pb: Quad = [0; 4];
    let mut k = 0;
    while (k as i32) < deref(m, a + 4, 0x24) as i32 {
        let (chb, _) = channel(m, b, 0x18, k);
        position(m, chb, time, &mut pb);
        let (cha, bone) = channel(m, a, 0x18, k);
        let at = m.word(ctrl + 0x1fc).wrapping_add(BONE * bone) + 0x10;
        let mut pa = quad(m, at);
        position(m, cha, time, &mut pa);
        let (va, vb) = (Vf::from_bits(pa), Vf::from_bits(pb));
        let s = Vf::from_bits([mix.to_bits(); 4]);
        let r = vb.sub(vb.sub(va).mul(s)).to_bits();
        put_quad(m, at, [r[0], r[1], r[2], pb[3]]);
        if bone != NONE
            && let Some(saved) = fade_position(m, ctrl, bone, bone, fade)
        {
            pb = saved;
        }
        k += 1;
    }
    finish(m, ctrl, a, time);
}

/// `b − (b − a)·mix` on VU0 (`xyz`), `w` the word `b` holds: the two-way mix [`blend4`] makes of
/// each pair.
fn mix_positions(a: Quad, b: Quad, mix: F) -> Quad {
    let (va, vb) = (Vf::from_bits(a), Vf::from_bits(b));
    let s = Vf::from_bits([mix.to_bits(); 4]);
    let r = vb.sub(vb.sub(va).mul(s)).to_bits();
    [r[0], r[1], r[2], b[3]]
}

/// **Four clips blended** (F62a: the rider's idle layer's) at `weight` of the first's length: channel
/// `k` of each sampled, the first two slerped the short way by `mix` (from the second to the first,
/// as [`blend`] does), the last two the same, then those two results by `mix2` (from the second pair's
/// to the first's); into the bone the first clip's channel maps to, faded and stored as [`play`] does.
/// The positions likewise, each mix `b − (b − a)·s` with the `w` of `b`; the result stored whole into
/// the bone before the bone is checked, then faded. The first and third clips share one sample buffer,
/// the second and fourth another (so an off channel keeps the other clip's quaternion, a held position
/// the other's `z`), and a fade leaves the saved rotation in the first buffer and the saved position
/// in the second. Channels are paired by their place in the lists and counted by the first clip's.
pub fn blend4(m: &mut impl Ram, t: &Tables, ctrl: u32, clips: [u32; 4], weight: F, mix: F, mix2: F, fade: F) {
    let [a, b, c, d] = clips;
    m.set(ctrl + 0x3c, 0);
    let data = m.word(a + 4);
    let time = f(m, data + 0x44) * weight;
    let slerp = |from: Quad, to: Quad, u: F| math::slerp(Vf::from_bits(from), Vf::from_bits(to), u, true, t.acos).to_bits();
    let (mut first, mut second): (Quad, Quad) = ([0; 4], [0; 4]);
    let mut k = 0;
    while (k as i32) < deref(m, a + 4, 0x28) as i32 {
        let pair = |m: &mut _, x: u32, y: u32, first: &mut Quad, second: &mut Quad| {
            let (ch, _) = channel(m, x, 0x1c, k);
            if let Some(r) = rotation(m, t, ch, time) {
                *first = r;
            }
            let (ch, _) = channel(m, y, 0x1c, k);
            if let Some(r) = rotation(m, t, ch, time) {
                *second = r;
            }
            slerp(*second, *first, mix)
        };
        let ab = pair(m, a, b, &mut first, &mut second);
        let cd = pair(m, c, d, &mut first, &mut second);
        let q = slerp(cd, ab, mix2);
        let (_, bone) = channel(m, a, 0x1c, k);
        if bone != NONE {
            if fades(m, ctrl, bone, fade, 0x208) {
                first = saved(m, ctrl, 0x208, bone);
            }
            put_rotation(m, t, ctrl, bone, bone, q, fade);
        }
        k += 1;
    }
    let (mut first, mut second): (Quad, Quad) = ([0; 4], [0; 4]);
    let mut k = 0;
    while (k as i32) < deref(m, a + 4, 0x24) as i32 {
        let pair = |m: &mut _, x: u32, y: u32, first: &mut Quad, second: &mut Quad| {
            let (ch, _) = channel(m, x, 0x18, k);
            position(m, ch, time, first);
            let (ch, _) = channel(m, y, 0x18, k);
            position(m, ch, time, second);
            mix_positions(*first, *second, mix)
        };
        let ab = pair(m, a, b, &mut first, &mut second);
        let cd = pair(m, c, d, &mut first, &mut second);
        let p = mix_positions(ab, cd, mix2);
        let (_, bone) = channel(m, a, 0x18, k);
        let at = m.word(ctrl + 0x1fc).wrapping_add(BONE * bone) + 0x10;
        put_quad(m, at, p);
        if bone != NONE
            && let Some(saved) = fade_position(m, ctrl, bone, bone, fade)
        {
            second = saved;
        }
        k += 1;
    }
    finish(m, ctrl, a, time);
}

// F62b: the controller's own clip (`+0x3c`), its time (`+0x64`) and its crossfade: the clock
// (`+0x68`), its end (`+0x6c`), its start (`+0x70`), its span (`+0x74`), the clip faded out of
// (`+0x40`, none for the saved pose), the fading bit (`+0x2d` bit 1) and the ended word (`+0x24c`).

const FADING: u32 = 0x02;
/// `+0x2c` bit 7: a list of per-bone words (`+0x214` long) the crossfades fill; not ported.
const BONE_LIST_ON: u32 = 0x80;

/// **The controller's clip set** (its vt `+0x90`): not ended, the clip its own, not fading, at
/// time 0.
pub fn set_clip(m: &mut impl Ram, ctrl: u32, clip: u32) {
    m.set(ctrl + 0x24c, 0);
    m.set(ctrl + 0x3c, clip);
    let b = byte(m, ctrl + 0x2d);
    put_byte(m, ctrl + 0x2d, b & !FADING);
    m.set(ctrl + 0x64, 0);
}

/// **A clip started with a crossfade of `span`** (the rider's wreck poses' shape: the new clip
/// from its start). A span of 0 only sets the clip ([`set_clip`]). Otherwise the crossfade's clock
/// and start are put at the old clip's time and its end `span` past it, the old clip kept to fade
/// out of; with no old clip, or a crossfade already running, the controller is stopped
/// ([`stop`]) and the fade is out of the saved pose instead. Then the clip set, and fading.
pub fn start(m: &mut impl Anim, ctrl: u32, clip: u32, span: F) {
    if span == F::ZERO {
        set_clip(m, ctrl, clip);
        return;
    }
    let now = m.word(ctrl + 0x64);
    m.set(ctrl + 0x70, now);
    m.set(ctrl + 0x68, now);
    m.set(ctrl + 0x6c, (F::from_bits(now) + span).to_bits());
    m.set(ctrl + 0x74, span.to_bits());
    let old = m.word(ctrl + 0x3c);
    m.set(ctrl + 0x40, old);
    if old == 0 || byte(m, ctrl + 0x2d) & FADING != 0 {
        m.set(ctrl + 0x40, 0);
        stop(m, ctrl);
    }
    set_clip(m, ctrl, clip);
    let b = byte(m, ctrl + 0x2d);
    put_byte(m, ctrl + 0x2d, (b & !FADING) | FADING);
}

/// A quadword as VU0 holds it: the words as they are (a slerp that cannot turn stores its first
/// quaternion back word for word, a denormal lane too).
fn raw(q: Quad) -> Vf {
    Vf(q.map(f32::from_bits))
}

/// A bone's rotation stored as the advance and its crossfades store it: the node turned if the
/// controller turns them, else into the bone (never both, unlike [`put_rotation`]).
fn store_rotation(m: &mut impl Ram, ctrl: u32, bone: u32, q: Quad) {
    let at = m.word(ctrl + 0x1fc).wrapping_add(BONE * bone);
    if byte(m, ctrl + 0x2d) & TURN_NODES != 0 {
        let n = m.word(at);
        turn_node(m, n, q);
    } else {
        put_quad(m, at + 0x20, q);
    }
}

/// A bone's position (`at`, already sampled) moved `1 − w` of the way to `from` (`xyz` on VU0,
/// `w` kept), and its node placed if the controller places them.
fn cross_position(m: &mut impl Ram, ctrl: u32, at: u32, from: Quad, w: F) {
    let p = quad(m, at);
    let pv = raw(p);
    let s = raw([(F::ONE - w).to_bits(); 4]);
    let r = pv.add(raw(from).sub(pv).mul(s)).to_bits();
    put_quad(m, at, [r[0], r[1], r[2], p[3]]);
    if byte(m, ctrl + 0x2d) & PLACE_NODES != 0 {
        let n = m.word(at - 0x10);
        node::set_position(m, n, [r[0], r[1], r[2], p[3]]);
    }
}

/// The clip's rotation and position counts.
fn counts(m: &mut impl Ram, clip: u32) -> (i32, i32) {
    let data = m.word(clip + 4);
    (m.word(data + 0x28) as i32, m.word(data + 0x24) as i32)
}

/// The controller's own clip's counts.
fn own(m: &mut impl Ram, ctrl: u32) -> (i32, i32) {
    let clip = m.word(ctrl + 0x3c);
    counts(m, clip)
}

/// **The clip played at its time**: every rotation channel with a bone sampled ([`rotation`]; an
/// off one keeps the last quaternion) and stored ([`store_rotation`]), every position channel with
/// a bone sampled into the bone ([`position`]) and its node placed if the controller places them.
fn sample(m: &mut impl Ram, t: &Tables, ctrl: u32) {
    let clip = m.word(ctrl + 0x3c);
    let time = f(m, ctrl + 0x64);
    let (rotations, positions) = counts(m, clip);
    let mut q: Quad = [0; 4];
    for k in 0..rotations.max(0) as u32 {
        let (ch, bone) = channel(m, clip, 0x1c, k);
        if bone != NONE {
            if let Some(r) = rotation(m, t, ch, time) {
                q = r;
            }
            store_rotation(m, ctrl, bone, q);
        }
    }
    for k in 0..positions.max(0) as u32 {
        let (ch, bone) = channel(m, clip, 0x18, k);
        if bone != NONE {
            let at = m.word(ctrl + 0x1fc).wrapping_add(BONE * bone) + 0x10;
            let mut p = quad(m, at);
            position(m, ch, time, &mut p);
            put_quad(m, at, p);
            if byte(m, ctrl + 0x2d) & PLACE_NODES != 0 {
                let n = m.word(at - 0x10);
                node::set_position(m, n, p);
            }
        }
    }
}

/// **The crossfade out of the saved pose** at `w` (0 the saved pose, 1 the clip): each rotation
/// slerped the short way from the saved one to the clip's, each position [`cross_position`]ed from
/// the saved one. The quaternion buffer keeps the last slerp for an off channel.
fn fade_from_saved(m: &mut impl Ram, t: &Tables, ctrl: u32, w: F) {
    let time = f(m, ctrl + 0x64);
    let mut q: Quad = [0; 4];
    let mut k = 0;
    while (k as i32) < own(m, ctrl).0 {
        let clip = m.word(ctrl + 0x3c);
        let (ch, bone) = channel(m, clip, 0x1c, k);
        if bone != NONE {
            if let Some(r) = rotation(m, t, ch, time) {
                q = r;
            }
            let s = saved(m, ctrl, 0x208, bone);
            q = math::slerp(raw(s), raw(q), w, true, t.acos).to_bits();
            store_rotation(m, ctrl, bone, q);
        }
        k += 1;
    }
    let mut k = 0;
    while (k as i32) < own(m, ctrl).1 {
        let clip = m.word(ctrl + 0x3c);
        let (ch, bone) = channel(m, clip, 0x18, k);
        if bone != NONE {
            let from = saved(m, ctrl, 0x20c, bone);
            let at = m.word(ctrl + 0x1fc).wrapping_add(BONE * bone) + 0x10;
            let mut p = quad(m, at);
            position(m, ch, time, &mut p);
            put_quad(m, at, p);
            cross_position(m, ctrl, at, from, w);
        }
        k += 1;
    }
}

/// **The crossfade out of the old clip** (`+0x40`) at `w`: channel `k` of the new clip at its time
/// and of the old at the crossfade's clock (the new clip's own channel `k` where the old has fewer),
/// the rotations slerped the short way from the old to the new, the positions [`cross_position`]ed
/// from the old. Each clip keeps its own sample buffer, so an off channel holds what the last one
/// left (the new clip's the last slerp).
fn fade_from_clip(m: &mut impl Ram, t: &Tables, ctrl: u32, w: F) {
    let time = f(m, ctrl + 0x64);
    let (mut new, mut old): (Quad, Quad) = ([0; 4], [0; 4]);
    let mut k = 0;
    while (k as i32) < own(m, ctrl).0 {
        let clip = m.word(ctrl + 0x3c);
        let (ch, bone) = channel(m, clip, 0x1c, k);
        if bone != NONE {
            if let Some(r) = rotation(m, t, ch, time) {
                new = r;
            }
            let was = m.word(ctrl + 0x40);
            let och = if (k as i32) < counts(m, was).0 { channel(m, was, 0x1c, k).0 } else { channel(m, clip, 0x1c, k).0 };
            let clock = f(m, ctrl + 0x68);
            if let Some(r) = rotation(m, t, och, clock) {
                old = r;
            }
            new = math::slerp(raw(old), raw(new), w, true, t.acos).to_bits();
            store_rotation(m, ctrl, bone, new);
        }
        k += 1;
    }
    let mut from: Quad = [0; 4];
    let mut k = 0;
    while (k as i32) < own(m, ctrl).1 {
        let clip = m.word(ctrl + 0x3c);
        let (ch, bone) = channel(m, clip, 0x18, k);
        if bone != NONE {
            let was = m.word(ctrl + 0x40);
            let och = if (k as i32) < counts(m, was).1 { channel(m, was, 0x18, k).0 } else { ch };
            let clock = f(m, ctrl + 0x68);
            position(m, och, clock, &mut from);
            let at = m.word(ctrl + 0x1fc).wrapping_add(BONE * bone) + 0x10;
            let mut p = quad(m, at);
            position(m, ch, time, &mut p);
            put_quad(m, at, p);
            cross_position(m, ctrl, at, from, w);
        }
        k += 1;
    }
}

/// **The controller's clip advanced by `dt`** (its vt `+0x88`, in the rider's idle layer's shape:
/// no mirror, no channel mask, every channel, no clip events). Nothing when ended or with no clip.
/// The last clip and time kept (`+0x44`, `+0x48`), the time moved on; past the clip's length a
/// looping clip (its data's `+0x30` bit 0) goes round by the length, any other ends (after this
/// call: ended, time and clock 0). Fading, the clock moves on too and the crossfade's weight is
/// `(clock − start − dt) / span`, 0 below 0, 1 from 1, `(3 − 2u)·u²` between, through the
/// accumulator; the fade is out of the old clip ([`fade_from_clip`]) or out of the saved pose
/// ([`fade_from_saved`]). Not fading, the clip is sampled ([`sample`]). Once the clock reaches the
/// crossfade's end, fading stops and the clock is 0.
///
/// The crossfades' per-bone list (`+0x2c` bit 7, the rider's controller never has it) is not
/// ported.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn advance(m: &mut impl Ram, t: &Tables, ctrl: u32, dt: F) {
    if m.word(ctrl + 0x24c) != 0 {
        return;
    }
    let clip = m.word(ctrl + 0x3c);
    if clip == 0 {
        return;
    }
    m.set(ctrl + 0x44, clip);
    let was = m.word(ctrl + 0x64);
    m.set(ctrl + 0x48, was);
    m.set(ctrl + 0x64, (F::from_bits(was) + dt).to_bits());
    let data = m.word(clip + 4);
    let mut ended = false;
    if f(m, data + 0x44) < f(m, ctrl + 0x64) {
        if byte(m, data + 0x30) & 1 == 0 {
            ended = true;
        } else {
            let time = f(m, ctrl + 0x64) - f(m, data + 0x44);
            m.set(ctrl + 0x64, time.to_bits());
        }
    }
    if byte(m, ctrl + 0x2d) & FADING != 0 {
        let clock = f(m, ctrl + 0x68) + dt;
        m.set(ctrl + 0x68, clock.to_bits());
        let u = ((clock - f(m, ctrl + 0x70)) - dt) / f(m, ctrl + 0x74);
        let w = if u < F::ZERO {
            F::ZERO
        } else if !(u < F::ONE) {
            F::ONE
        } else {
            let v = (u - F::ZERO) / F::ONE;
            Acc::adda(F::ZERO, F(3.0)).msub(F(2.0), v) * (v * v)
        };
        if m.word(ctrl + 0x40) == 0 {
            fade_from_saved(m, t, ctrl, w);
        } else {
            fade_from_clip(m, t, ctrl, w);
        }
        debug_assert!(byte(m, ctrl + 0x2c) & BONE_LIST_ON == 0, "the crossfades' per-bone list is not ported");
    } else {
        sample(m, t, ctrl);
    }
    if !(f(m, ctrl + 0x68) < f(m, ctrl + 0x6c)) {
        let b = byte(m, ctrl + 0x2d);
        put_byte(m, ctrl + 0x2d, b & !FADING);
        m.set(ctrl + 0x68, 0);
    }
    if ended {
        m.set(ctrl + 0x24c, 1);
        m.set(ctrl + 0x64, 0);
        m.set(ctrl + 0x68, 0);
    }
}
