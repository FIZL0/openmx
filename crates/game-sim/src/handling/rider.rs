//! **SLUS-21104's rider's "a ragdoll part is touching" flag, on RAM** (S278): the one word of the
//! rider's the bike's frame reads (F41). The bike's wreck clock ([`super::wreck::clock`]) starts its
//! put-back count only once the bike's [`super::wreck::SETTLES`] is set, and that word is the rider's
//! flag, handed into the bike's collision record every frame. Where the original keeps them:
//! research/oracle.txt `# S278`.
//!
//! The rider is MX Unleashed's ragdoll actor ([`crate::ragdoll_body::Actor`]) in MX vs. ATV's layout.
//! Its parts are the ragdoll's bone collisions (the world's type-8 constraints,
//! [`crate::ragdoll_contact`]'s), each with its four flags at `+0x14` (touched: a push-out this world
//! step), `+0x18` (ground), `+0x1c` (scene) and `+0x20` (swept), and its velocity change at `+0x60`.
//! The frame runs, in this order:
//!
//! 1. the rider's mode machine; in its wreck arm, [`touching`]: the flag is set, and its clock runs
//!    on, while any part of the rider's list was touched in the last world step, else both go to 0
//!    (outside the wreck arm the flag is left as it was);
//! 2. at the mode machine's end, every part's four flags cleared ([`clear_all`]);
//! 3. the rider's ragdoll world steps, its bone collisions setting `touched` again on every push-out;
//! 4. inside that step, [`hand_over`]: the flag into the record's `+0x1f0` (the bike's `SETTLES`), the
//!    step's impact ([`impact`]: `(0, 0, 0, 1)` plus each touched named part's velocity change, or
//!    just `(0, 0, 0, 1)` with the flag off) into the record's `+0x160`, the rider's main body's
//!    position into its `+0x200`, and the nine named parts into the record's part block.
//!
//! The mode machine itself ([`modes`], S278b) runs, every frame the rider's record says it is
//! ridden: the mode chosen ([`choose`]: 0 the respawn, 1 riding, 2 wrecked, 3 not on the bike, 4
//! riding with a hold), the change of mode acted on ([`switch`]), the hand and foot holds
//! ([`holds`]), the lean ([`lean`]), the pose follow ([`follow`]), the wreck arm, [`clear_all`], and
//! the record's words copied in ([`frame_end`]).
//!
//! The wreck arm ([`wreck_arm`], S285) runs the impact aim and its force, the holds let go, the
//! nearby gather, [`touching`], the bike's collidable released by the parts once clear of it, the
//! ground under the rider, and the three pose-target clauses ([`pose_targets`]).
//!
//! The respawn's and the follow's two virtuals ([`Call::Pose`], [`Call::Settle`]) are [`pose`] and
//! [`settle`] (S286): the ragdoll's bodies started where the skeleton's bones are, then made to be
//! there.
//!
//! What is not here: the world step (3); and the rest of the calls in [`Call`]. The `touched` words this reads
//! come from the original's ragdoll world until that is ported.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use super::ground;
use super::node::{self, Ram};
use super::wreck::turned;
use crate::track;
use core::cell::RefCell;
use ps2_float::vu::Dest;
use ps2_float::fpu::{self, Acc};
use ps2_float::{F, Vf};

/// The rider's words: its target collision record (the bike's, at bike `+0x730`), the flag and how
/// long it has been set, the part list's count and array, the impact, the main body's node holder.
pub const RECORD: u32 = 0x378;
pub const TOUCHING: u32 = 0x37c;
pub const TOUCHING_TIME: u32 = 0x380;
pub const PARTS_COUNT: u32 = 0x5b0;
pub const PARTS: u32 = 0x5b4;
pub const IMPACT: u32 = 0x670;
pub const MAIN: u32 = 0x2a4;
/// The nine named parts the impact sums, in its order (`+0x58c..+0x5a8`, then `+0x5ac`): the same
/// nine pointers the hand-over writes into the record's part block, in its own order.
pub const NAMED: [u32; 9] = [0x58c, 0x590, 0x594, 0x598, 0x59c, 0x5a0, 0x5a4, 0x5a8, 0x5ac];
pub const BLOCK_ORDER: [u32; 9] = [0x5a8, 0x5ac, 0x58c, 0x590, 0x594, 0x598, 0x59c, 0x5a0, 0x5a4];
/// A part's flags (`+0x14` touched first) and its velocity change.
pub const TOUCHED: u32 = 0x14;
pub const DV: u32 = 0x60;
/// The record's words the hand-over writes: the flag, the impact, the main body's position, and the
/// holder of its part block (`+0x14` of it, nine words).
pub const R_TOUCHING: u32 = 0x1f0;
pub const R_IMPACT: u32 = 0x160;
pub const R_MAIN: u32 = 0x200;
pub const R_BLOCK: u32 = 0x1e4;
/// The rest quadword `(0, 0, 0, 1)`: rodata.
pub const ORIGIN: u32 = 0x0039_6fc0;

/// A quadword load or store, as `lq`/`sq` and `lqc2`/`sqc2` make it: the address's low four bits
/// dropped.
fn quad(m: &mut impl Ram, at: u32) -> Vf {
    Vf::from_bits([0, 4, 8, 12].map(|k| m.word((at & !0xf) + k)))
}

fn put_quad(m: &mut impl Ram, at: u32, v: Vf) {
    for (k, w) in v.to_bits().into_iter().enumerate() {
        m.set((at & !0xf) + 4 * k as u32, w);
    }
}

/// **The wreck arm's flag** (once a frame while the mode machine is in its wreck arm, after the
/// ground ask): the list's count read once, the parts walked from the first while `touched` is 0; the
/// walk stopping short of the count (a negative count among them: the walk never starts, and 0 is not
/// the count) sets the flag and adds `dt` to its clock, else both are 0.
pub fn touching(m: &mut impl Ram, rider: u32, dt: F) {
    let n = m.word(rider + PARTS_COUNT) as i32;
    let mut i = 0;
    while i < n {
        let list = m.word(rider + PARTS);
        let part = m.word(list.wrapping_add(4 * i as u32));
        if m.word(part + TOUCHED) != 0 {
            break;
        }
        i += 1;
    }
    if i == n {
        m.set(rider + TOUCHING, 0);
        m.set(rider + TOUCHING_TIME, 0);
    } else {
        m.set(rider + TOUCHING, 1);
        let t = fpu::add(f32::from_bits(m.word(rider + TOUCHING_TIME)), dt.0);
        m.set(rider + TOUCHING_TIME, t.to_bits());
    }
}

/// **A part's flags cleared**: touched, ground, scene and swept.
pub fn clear(m: &mut impl Ram, part: u32) {
    for k in 0..4 {
        m.set(part + TOUCHED + 4 * k, 0);
    }
}

/// **Every part of the list cleared**, the mode machine's last step: the count re-read each part.
pub fn clear_all(m: &mut impl Ram, rider: u32) {
    let mut i = 0;
    while i < m.word(rider + PARTS_COUNT) as i32 {
        let list = m.word(rider + PARTS);
        let part = m.word(list.wrapping_add(4 * i as u32));
        clear(m, part);
        i += 1;
    }
}

/// **The step's impact** into the rider's `+0x670`: `(0, 0, 0, 1)`, then each named part's velocity
/// change added in [`NAMED`]'s order (`x`, `y` and `z`; `w` kept) when that part is touched. Each sum
/// is stored and read back.
pub fn impact(m: &mut impl Ram, rider: u32) {
    let rest = quad(m, ORIGIN);
    put_quad(m, rider + IMPACT, rest);
    for o in NAMED {
        let part = m.word(rider + o);
        if m.word(part + TOUCHED) != 0 {
            let sum = quad(m, rider + IMPACT);
            let dv = quad(m, part + DV);
            put_quad(m, rider + IMPACT, sum.merge(Dest::XYZ, sum.add(dv)));
        }
    }
}

/// **The hand-over into the target record** (the rider's world step calls it through the rider's
/// virtual): the flag to the record's `+0x1f0`; the impact ([`impact`] with the flag set, else the
/// rest quadword) kept at the rider's `+0x670` and copied to the record's `+0x160`; the main body's
/// node's `+0x70` quadword to the record's `+0x200`; and, with a part block, the nine named parts'
/// pointers into it in [`BLOCK_ORDER`].
pub fn hand_over(m: &mut impl Ram, rider: u32) {
    let record = m.word(rider + RECORD);
    let flag = m.word(rider + TOUCHING);
    m.set(record + R_TOUCHING, flag);
    if m.word(rider + TOUCHING) == 0 {
        let rest = quad(m, ORIGIN);
        put_quad(m, rider + IMPACT, rest);
    } else {
        impact(m, rider);
    }
    let record = m.word(rider + RECORD);
    let v = quad(m, rider + IMPACT);
    put_quad(m, record + R_IMPACT, v);
    let record = m.word(rider + RECORD);
    let holder = m.word(rider + MAIN);
    let main = m.word(holder + 0xa4);
    let v = quad(m, main + 0x70);
    put_quad(m, record + R_MAIN, v);
    let record = m.word(rider + RECORD);
    if m.word(record + R_BLOCK) != 0 {
        for (k, o) in BLOCK_ORDER.into_iter().enumerate() {
            let p = m.word(rider + o);
            let record = m.word(rider + RECORD);
            let holder = m.word(record + R_BLOCK);
            let block = m.word(holder + 0x14);
            m.set(block + 4 * k as u32, p);
        }
    }
}

// --- S278b: the mode machine ------------------------------------------------------------------------

/// The mode machine's words. The mode this frame and the one acted on (`+0x368`, `+0x36c`): 0 the
/// respawn, 1 riding, 2 wrecked, 3 off the bike, 4 riding with a hold. The respawn's frames left
/// (`+0x8f8`); the record's wreck kind kept from the last frame (`+0x370`, 13 re-enters the wreck);
/// the rider's scene node (`+0x24`).
pub const MODE: u32 = 0x368;
pub const ACTED: u32 = 0x36c;
pub const KIND_KEPT: u32 = 0x370;
pub const RESPAWN: u32 = 0x8f8;
pub const NODE: u32 = 0x24;
/// The record's words the choice reads: ridden (`+8`, the frame runs the machine only then), the
/// respawn ask (`+0xc`), the wreck kind (`+0x138`), the hold ask (`+0x170`).
pub const R_RIDDEN: u32 = 0x8;
pub const R_RESPAWN: u32 = 0xc;
pub const R_KIND: u32 = 0x138;
pub const R_HOLD: u32 = 0x170;
/// The hold switches' four parts: the left and right holds (`+0x55c`, `+0x558`, each with its two
/// strengths at `+0xd0` and `+0x118`), and the two named parts each one stands in for (`+0x5ac`,
/// `+0x5a8`); the holds' fades (`+0x5cc`, `+0x5c8`).
pub const HOLD_L: u32 = 0x55c;
pub const HOLD_R: u32 = 0x558;
pub const FREE_L: u32 = 0x5ac;
pub const FREE_R: u32 = 0x5a8;
pub const FADE_L: u32 = 0x5cc;
pub const FADE_R: u32 = 0x5c8;
/// The wreck kind that enters the wreck again while wrecked.
pub const KIND_AGAIN: u32 = 13;

/// A call the mode machine makes outside its own words. Each is made with the rider in `a0`, and
/// `dt` in `f12` where the original hands it on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    /// `rand()`: its answer.
    Rand,
    /// The riding reset (the rider put back on the bike), with `dt`.
    RidingReset,
    /// The wreck entry, with the record's wreck kind and `dt`.
    WreckEntry(u32),
    /// The rider's vt `+0xd0` (`dt`, 0): the ragdoll posed off the scene ([`pose`]); vt `+0xcc`: its
    /// world's bodies settled ([`settle`]).
    Pose,
    Settle,
    /// A part's on/off virtual (its vt at `+0x10`, slot `+0x10`): the part, on or off. The wreck
    /// arm's aim force (`+0x5bc`) is switched off by the same call, its vt at `+8`.
    Switch { part: u32, on: u32 },
    /// The wreck arm's nearby gather: the rider's list (`+0x5b8`) refilled from the broad phase
    /// around the first part's sphere, kind 3 (the quad's [`crate::atv_world`] gather on RAM).
    Gather { list: u32, probe: u32, kind: u32 },
    /// A ragdoll body's vt `+0x68`: its `+0x150` quadword from `from` (on the rider's own bodies,
    /// [`body_target`]).
    BodyTarget { body: u32, from: u32 },
}

/// The mode machine's RAM and calls; [`Rider::call`]'s answer is `v0` (only [`Call::Rand`]'s is read).
pub trait Rider: Ram {
    fn call(&mut self, c: Call, dt: F) -> u32;
}

fn fw(m: &mut impl Ram, at: u32) -> f32 {
    f32::from_bits(m.word(at))
}

fn put_f(m: &mut impl Ram, at: u32, x: f32) {
    m.set(at, x.to_bits());
}

fn byte(m: &mut impl Ram, at: u32) -> u32 {
    (m.word(at & !3) >> (8 * (at & 3))) & 0xff
}

fn copy_quad(m: &mut impl Ram, to: u32, from: u32) {
    let v = quad(m, from);
    put_quad(m, to, v);
}

/// The respawn count's start: 1 if the rider's `+0x3a4` is 0 or 1, else `other`.
fn respawn_frames(m: &mut impl Ram, rider: u32, other: u32) -> u32 {
    let a = m.word(rider + 0x3a4);
    if a == 0 || a == 1 { 1 } else { other }
}

/// **The mode chosen** from the record: a respawn ask makes mode 0 and starts the respawn count (1
/// or 3); else a wreck kind makes 2; else a rider whose `+0x86d` byte is under 46 is off the bike
/// (3); else a hold asked by the record's `+0x170` or the rider's `+0x8ac` makes 4, and 1 without.
pub fn choose(m: &mut impl Ram, rider: u32) {
    let rec = m.word(rider + RECORD);
    let mode = if m.word(rec + R_RESPAWN) != 0 {
        m.set(rider + MODE, 0);
        let n = respawn_frames(m, rider, 3);
        m.set(rider + RESPAWN, n);
        return;
    } else if m.word(rec + R_KIND) != 0 {
        2
    } else if byte(m, rider + 0x86d) < 46 {
        3
    } else if m.word(rec + R_HOLD) != 0 || m.word(rider + 0x8ac) != 0 {
        4
    } else {
        1
    };
    m.set(rider + MODE, mode);
}

/// **The parts placed on their bodies** (each frame the ragdoll is reposed): for every entry of the
/// rider's `+0x3b0` list (`+0x3ac` of them, 0x50 bytes each, the count re-read each entry), the
/// entry's node (`+4`) put at its body's (`+8`) `+0x30` plus the body's offset (`*(body + 0x78) +
/// 0x1a0`, `xyz`), and the offset kept at the node's `+0xb0`.
pub fn place_parts(m: &mut impl Ram, rider: u32) {
    let mut i = 0u32;
    while (i as i32) < m.word(rider + 0x3ac) as i32 {
        let e = m.word(rider + 0x3b0).wrapping_add(i.wrapping_mul(0x50));
        let body = m.word(e + 8);
        let at = quad(m, body + 0x30);
        let off_at = m.word(body + 0x78) + 0x1a0;
        let p = at.merge(Dest::XYZ, at.add(quad(m, off_at)));
        let e = m.word(rider + 0x3b0).wrapping_add(i.wrapping_mul(0x50));
        let body = m.word(e + 8);
        let off_at = m.word(body + 0x78) + 0x1a0;
        let off = quad(m, off_at);
        let e = m.word(rider + 0x3b0).wrapping_add(i.wrapping_mul(0x50));
        let n = m.word(e + 4);
        node::set_position(m, n, p.to_bits());
        let e = m.word(rider + 0x3b0).wrapping_add(i.wrapping_mul(0x50));
        let n = m.word(e + 4);
        put_quad(m, n + 0xb0, off);
        i += 1;
    }
}

/// The rider's node put on the record: at the record's `+0x1b0`, plus (unridden record, `+8` 0) its
/// `+0x100` point turned by its orientation `+0x1c0`; turned to that orientation.
fn place_on_record(m: &mut impl Ram, rider: u32) {
    let rec = m.word(rider + RECORD);
    let p = if m.word(rec + R_RIDDEN) == 0 {
        let at = m.word(rec + 0x100);
        let t = turned(quad(m, rec + 0x1c0), quad(m, at));
        let rec = m.word(rider + RECORD);
        t.merge(Dest::XYZ, t.add(quad(m, rec + 0x1b0)))
    } else {
        quad(m, rec + 0x1b0)
    };
    let n = m.word(rider + NODE);
    node::set(m, n, p.to_bits());
    let rec = m.word(rider + RECORD);
    let n = m.word(rider + NODE);
    let q = quad(m, rec + 0x1c0);
    super::anim::turn_node(m, n, q.to_bits());
}

/// The respawn's body, both times it runs (each respawn frame, and leaving the wreck): the node
/// put on the record, `+0x38c` 0, `+0x258` 1, the ragdoll posed and settled, its parts placed.
fn respawn(m: &mut impl Rider, rider: u32, dt: F) {
    place_on_record(m, rider);
    m.set(rider + 0x38c, 0);
    m.set(rider + 0x258, 1);
    m.call(Call::Pose, dt);
    m.call(Call::Settle, dt);
    place_parts(m, rider);
}

/// **The change of mode acted on**. While the respawn count runs: the respawn, the count down by
/// one, the acted mode 0. Else, by the mode (read once) against the acted one: into 4, `+0x38c` 1,
/// the parts placed, `+0x8a8` a random half (`rand × 2⁻³¹ × 0.5`), the riding reset; into 2, or a
/// wreck kind 13 while wrecked that was not 13 last frame, the wreck entry; out of 2, the wreck's
/// clocks and flags reset (`+0x63c`, `+0x638`, the touching clock 0, `+0x640` −1), the respawn and
/// its count started (1 or 2); into 3, `+0x38c` 0 and the parts placed; into 1, the riding reset.
/// Each change makes the mode the acted one.
pub fn switch(m: &mut impl Rider, rider: u32, dt: F) {
    if m.word(rider + RESPAWN) as i32 > 0 {
        respawn(m, rider, dt);
        let n = m.word(rider + RESPAWN).wrapping_sub(1);
        m.set(rider + RESPAWN, n);
        m.set(rider + ACTED, 0);
        return;
    }
    let mode = m.word(rider + MODE);
    if mode == 4 && m.word(rider + ACTED) != 4 {
        m.set(rider + 0x38c, 1);
        place_parts(m, rider);
        let r = m.call(Call::Rand, dt) as i32;
        let x = fpu::mul(f32::from_bits(0x3000_0000), fpu::from_i32(r));
        put_f(m, rider + 0x8a8, fpu::mul(0.5, x));
        m.call(Call::RidingReset, dt);
    } else if mode == 2 && (m.word(rider + ACTED) != 2 || {
        let rec = m.word(rider + RECORD);
        m.word(rec + R_KIND) == KIND_AGAIN && m.word(rider + KIND_KEPT) != KIND_AGAIN
    }) {
        let rec = m.word(rider + RECORD);
        let kind = m.word(rec + R_KIND);
        m.call(Call::WreckEntry(kind), dt);
    } else if mode != 2 && m.word(rider + ACTED) == 2 {
        m.set(rider + 0x63c, 0);
        m.set(rider + 0x638, 0);
        m.set(rider + TOUCHING_TIME, 0);
        m.set(rider + 0x640, 0xbf80_0000);
        respawn(m, rider, dt);
        let n = respawn_frames(m, rider, 2);
        m.set(rider + RESPAWN, n);
    } else if mode == 3 && m.word(rider + ACTED) != 3 {
        m.set(rider + 0x38c, 0);
        place_parts(m, rider);
    } else if mode == 1 && m.word(rider + ACTED) != 1 {
        m.call(Call::RidingReset, dt);
    } else {
        return;
    }
    let mode = m.word(rider + MODE);
    m.set(rider + ACTED, mode);
}

/// The `y` of the orientation's up axis (the quaternion's axes, the second row of the second out): `1 − (z·2z +
/// x·2x)`, the sum on the accumulator from zero.
fn up_y(m: &mut impl Ram, q: u32) -> f32 {
    let (x, z) = (fw(m, q), fw(m, q + 8));
    let xx = fpu::mul(x, fpu::mul(x, 2.0));
    let zz2 = fpu::mul(z, 2.0);
    fpu::sub(1.0, fpu::madd(fpu::adda(0.0, xx), z, zz2))
}

fn switch_part(m: &mut impl Rider, rider: u32, at: u32, on: u32, dt: F) {
    let part = m.word(rider + at);
    m.call(Call::Switch { part, on }, dt);
}

/// A hold full on: its strengths 1.
fn hold_full(m: &mut impl Ram, rider: u32, at: u32) {
    let h = m.word(rider + at);
    m.set(h + 0xd0, 0x3f80_0000);
    m.set(h + 0x118, 0x3f80_0000);
}

/// One hand reaching for its hold: on (strength 1) and its free part off when the record's `+0x12c`
/// is set, else the hold off, the free part on and its fade 2 s.
fn reach(m: &mut impl Rider, rider: u32, (hold, free, fade): (u32, u32, u32), dt: F) {
    let rec = m.word(rider + RECORD);
    if m.word(rec + 0x12c) != 0 {
        switch_part(m, rider, hold, 1, dt);
        hold_full(m, rider, hold);
        switch_part(m, rider, free, 0, dt);
        m.set(rider + fade, 0);
    } else {
        switch_part(m, rider, hold, 0, dt);
        switch_part(m, rider, free, 1, dt);
        m.set(rider + fade, 0x4000_0000);
    }
}

/// A hold fading in: its fade run down by `dt` while above 0, and both strengths `1 − fade / 2`
/// clamped to 0..1 (a remap from 0..2 to 1..0, on the accumulator).
fn fade_in(m: &mut impl Ram, rider: u32, hold: u32, fade: u32, dt: F) {
    let f = fw(m, rider + fade);
    if f <= 0.0 {
        return;
    }
    let f = fpu::sub(f, dt.0);
    put_f(m, rider + fade, f);
    let t = fpu::div(fpu::sub(f, 0.0), fpu::sub(2.0, 0.0));
    let v = fpu::madd(fpu::adda(0.0, 1.0), t, fpu::sub(0.0, 1.0));
    let v = fpu::min(fpu::max(v, fpu::min(1.0, 0.0)), fpu::max(1.0, 0.0));
    let h = m.word(rider + hold);
    put_f(m, h + 0xd0, v);
    put_f(m, h + 0x118, v);
}

/// **The hand and foot holds**, on a rider on the bike (`+0x874` −1, the mode neither 0 nor 2, the
/// `+0x86d` byte 46 or more). Unless the record is in the air (`+0x130`) or level (`+0x19c` 0) with
/// `+0x1a8` not below 0 and either `|+0xb4|` at 1.5 or more or `+0x1a8` at the rider's `+0x894` or
/// more: both holds on and their free parts off with the record's `+0x12c`, else the other way with
/// both fades ½. Otherwise, a rider of kind `0x38` (`+0x86e`) under its `+0x8a0` with the up axis'
/// `y` under 0.9 reaches with the left hand past `+0x1a0` 0.7, or the right one under 0.3, the other
/// hold full on; and in every other case both holds on, free parts off, the fades run in.
pub fn holds(m: &mut impl Rider, rider: u32, dt: F) {
    let mode = m.word(rider + MODE);
    if m.word(rider + 0x874) as i32 != -1 || mode == 2 || mode == 0 || byte(m, rider + 0x86d) < 46 {
        return;
    }
    let rec = m.word(rider + RECORD);
    let up = up_y(m, rec + 0x1c0);
    let rec = m.word(rider + RECORD);
    let away = m.word(rec + 0x130) != 0 || {
        let a8 = fw(m, rec + 0x1a8);
        fw(m, rec + 0x19c) == 0.0
            && !(a8 < 0.0)
            && (!(fpu::abs(fw(m, rec + 0xb4)) < 1.5) || !(a8 < fw(m, rider + 0x894)))
    };
    let (l, r) = ((HOLD_L, FREE_L, FADE_L), (HOLD_R, FREE_R, FADE_R));
    if !away {
        let on = m.word(rec + 0x12c) != 0;
        let (a, b) = if on { (1, 0) } else { (0, 1) };
        switch_part(m, rider, HOLD_L, a, dt);
        switch_part(m, rider, FREE_L, b, dt);
        switch_part(m, rider, HOLD_R, a, dt);
        switch_part(m, rider, FREE_R, b, dt);
        let fade = if on { 0 } else { 0x3f00_0000 };
        m.set(rider + FADE_L, fade);
        m.set(rider + FADE_R, fade);
        return;
    }
    let low = byte(m, rider + 0x86e) == 0x38 && fw(m, rec + 0x1a8) < fw(m, rider + 0x8a0);
    let lean = fw(m, rec + 0x1a0);
    let (reach_with, other) = if low && !(lean <= 0.7) && up < 0.9 {
        (l, r)
    } else if low && lean < 0.3 && up < 0.9 {
        (r, l)
    } else {
        switch_part(m, rider, HOLD_L, 1, dt);
        switch_part(m, rider, FREE_L, 0, dt);
        switch_part(m, rider, HOLD_R, 1, dt);
        switch_part(m, rider, FREE_R, 0, dt);
        fade_in(m, rider, HOLD_R, FADE_R, dt);
        fade_in(m, rider, HOLD_L, FADE_L, dt);
        return;
    };
    reach(m, rider, reach_with, dt);
    switch_part(m, rider, other.0, 1, dt);
    hold_full(m, rider, other.0);
    switch_part(m, rider, other.1, 0, dt);
}

/// **The lean**: the rider's `+0x600` (`xyz`) scaled into `+0x5f0` by the record's `+0x1a8` over the
/// rider's `+0x894` taken from 0..1 to 2..1 and clamped (2 with a `+0x894` of 0); and, out of a
/// wreck, the main body's node orientation (`+0x190`) the record's orientation times the reference
/// pose's (`+0x1fc`'s entry of the holder's `+0x40`, at `+0x20`).
pub fn lean(m: &mut impl Ram, rider: u32) {
    let rec = m.word(rider + RECORD);
    let over = fw(m, rider + 0x894);
    let s = if over == 0.0 {
        2.0
    } else {
        let t = fpu::div(fpu::sub(fw(m, rec + 0x1a8), 0.0), fpu::sub(over, 0.0));
        let v = fpu::madd(fpu::adda(0.0, 2.0), t, fpu::sub(1.0, 2.0));
        fpu::min(fpu::max(v, fpu::min(2.0, 1.0)), fpu::max(2.0, 1.0))
    };
    let v = quad(m, rider + 0x600);
    put_quad(m, rider + 0x5f0, v.merge(Dest::XYZ, v.mul(Vf::splat(s))));
    if m.word(rider + MODE) == 2 {
        return;
    }
    let holder = m.word(rider + MAIN);
    let rec = m.word(rider + RECORD);
    let table = m.word(rider + 0x1fc);
    let k = m.word(holder + 0x40);
    let node = m.word(holder + 0xa4);
    let a = quad(m, rec + 0x1c0);
    let b = quad(m, table.wrapping_add(k << 6).wrapping_add(0x20));
    put_quad(m, node.wrapping_add(0x190), crate::chassis::quat_mul(a, b));
}

/// **The pose follow**: `+0x258` the record's respawn ask if it was 0; `+0x25c` the `+0x38c`. With
/// the rider's `+0x30c`'s `+0x28` 2: a follow asked out of a wreck is dropped without `+0x384`, and
/// on `+0x384`'s rising edge (`+0x388` its last) the ragdoll is reposed (parts placed, posed,
/// settled); `+0x388` then follows `+0x384`.
pub fn follow(m: &mut impl Rider, rider: u32, dt: F) {
    if m.word(rider + 0x258) == 0 {
        let rec = m.word(rider + RECORD);
        let v = m.word(rec + R_RESPAWN);
        m.set(rider + 0x258, v);
    }
    let v = m.word(rider + 0x38c);
    m.set(rider + 0x25c, v);
    let s = m.word(rider + 0x30c);
    if m.word(s + 0x28) != 2 {
        return;
    }
    if m.word(rider + 0x25c) != 0 && m.word(rider + MODE) != 2 {
        if m.word(rider + 0x384) == 0 {
            m.set(rider + 0x25c, 0);
        } else if m.word(rider + 0x388) == 0 {
            place_parts(m, rider);
            m.call(Call::Pose, dt);
            m.call(Call::Settle, dt);
        }
    }
    let v = m.word(rider + 0x384);
    m.set(rider + 0x388, v);
}

/// **The record's words copied in**, the machine's last step: `+0x420` the record's `+0xa0` × 0.8
/// (`xyz`); `+0x3d0`, `+0x3c0` its `+0xd0`, `+0xc0`; `+0x3f0`, `+0x3e0` its `+0xf0`, `+0xe0` less the
/// rider's `+0x410`, `+0x400` turned by its orientation (`xyz`); each of its five point pairs
/// (`+0x100..+0x124`) with both pointers set into `+0x430..+0x4cc`; its 4×4 at `+0x60` into
/// `+0x500`; its wreck kind kept at `+0x370`.
pub fn frame_end(m: &mut impl Ram, rider: u32) {
    let rec = m.word(rider + RECORD);
    let v = quad(m, rec + 0xa0);
    put_quad(m, rider + 0x420, v.merge(Dest::XYZ, v.mul(Vf::splat(f32::from_bits(0x3f4c_cccd)))));
    for (to, from) in [(0x3d0, 0xd0), (0x3c0, 0xc0)] {
        let rec = m.word(rider + RECORD);
        copy_quad(m, rider + to, rec + from);
    }
    for (to, from, by) in [(0x3f0, 0xf0, 0x410), (0x3e0, 0xe0, 0x400)] {
        let rec = m.word(rider + RECORD);
        let t = turned(quad(m, rec + 0x1c0), quad(m, rider + by));
        let rec = m.word(rider + RECORD);
        let p = quad(m, rec + from);
        put_quad(m, rider + to, p.merge(Dest::XYZ, p.sub(t)));
    }
    for k in 0..5 {
        let rec = m.word(rider + RECORD);
        let a = rec + 0x100 + 8 * k;
        if m.word(a) != 0 && m.word(a + 4) != 0 {
            let p = m.word(a);
            copy_quad(m, rider + 0x430 + 0x20 * k, p);
            let rec = m.word(rider + RECORD);
            let p = m.word(rec + 0x104 + 8 * k);
            copy_quad(m, rider + 0x440 + 0x20 * k, p);
        }
    }
    let rec = m.word(rider + RECORD);
    let rows = [0, 1, 2, 3].map(|r| quad(m, rec + 0x60 + 0x10 * r));
    for (r, v) in rows.into_iter().enumerate() {
        put_quad(m, rider + 0x500 + 0x10 * r as u32, v);
    }
    let rec = m.word(rider + RECORD);
    let k = m.word(rec + R_KIND);
    m.set(rider + KIND_KEPT, k);
}

// --- S285: the wreck arm ----------------------------------------------------------------------------

/// The wreck arm's words. The impact aim (`+0x660`: where the aim force, `+0x5bc`, pulls) and its
/// clock (`+0x640`), taken from `+0x650` as it is (`+0x62c`), turned by the main body's orientation
/// (`+0x630`) or by the record's; the holds' let-go clock (`+0x644`) and the two holds it lets go
/// (`+0x550`, `+0x554`); the nearby list (`+0x5b8`); the bike's collidable released (`+0x634`, the
/// record's `+0x1dc`); the rider's terrain (`+0x26c`); the clauses' two latches (`+0x63c` high,
/// `+0x638` slow), the height (`+0x850`), the settle time (`+0x854`) and the speed (`+0x858`) they
/// test; the pose-target point (`+0x840`); the fade (`+0x5d8`, from `+0x5d4`).
pub const AIM: u32 = 0x660;
pub const AIM_FROM: u32 = 0x650;
pub const AIM_TIME: u32 = 0x640;
pub const AIM_AS_IS: u32 = 0x62c;
pub const AIM_BY_BODY: u32 = 0x630;
pub const AIM_FORCE: u32 = 0x5bc;
pub const LET_GO_TIME: u32 = 0x644;
pub const LET_GO: [u32; 2] = [0x550, 0x554];
pub const NEARBY: u32 = 0x5b8;
pub const RELEASED: u32 = 0x634;
pub const R_COLLIDABLE: u32 = 0x1dc;
pub const TERRAIN: u32 = 0x26c;
pub const HIGH: u32 = 0x63c;
pub const SLOW: u32 = 0x638;
pub const HIGH_AT: u32 = 0x850;
pub const SLOW_AFTER: u32 = 0x854;
pub const SLOW_UNDER: u32 = 0x858;
pub const TARGET: u32 = 0x840;
pub const FADE: u32 = 0x5d8;
pub const FADE_FROM: u32 = 0x5d4;
/// The pose targets' words: the latch (`+0x264`), the body count (`+0x290`), the pose list (`+0x3a8`,
/// `0x60` an entry: the wreck's target at `+0x40`, the standing one at `+0x50`); the body holders
/// (`+0x2a4`, `0xb0` apart, each body at `+0xa4`).
pub const TARGETED: u32 = 0x264;
pub const BODIES: u32 = 0x290;
pub const POSES: u32 = 0x3a8;
/// A body's position, velocity and orientation; a part's sphere; a shape's ignore list and count.
const B_POSITION: u32 = 0x70;
const B_VELOCITY: u32 = 0x80;
const B_QUAT: u32 = 0x190;
const SPHERE: u32 = 0x88;
const S_IGNORE: u32 = 0xe4;
const S_IGNORE_COUNT: u32 = 0xe8;

/// A float word as the FPU's compares read it (exponent 255 the saturated maximum, denormals 0).
fn op(m: &mut impl Ram, at: u32) -> f32 {
    fpu::operand(m.word(at))
}

/// The rider's main body: the first holder's.
fn main_body(m: &mut impl Ram, rider: u32) -> u32 {
    let holder = m.word(rider + MAIN);
    m.word(holder + 0xa4)
}

/// `|v.xyz|` on the FPU: `mula`, `madda`, `madd`, `sqrt.s`.
fn length(v: Vf) -> f32 {
    fpu::sqrt(Acc::mula(F(v.x()), F(v.x())).madda(F(v.y()), F(v.y())).madd(F(v.z()), F(v.z())).0)
}

/// **A collidable taken off a shape's ignore list**: the first entry that is it, of the count read
/// once, set to 0.
pub fn release(m: &mut impl Ram, shape: u32, collidable: u32) {
    let n = m.word(shape + S_IGNORE_COUNT) as i32;
    let list = m.word(shape + S_IGNORE);
    for i in 0..n.max(0) as u32 {
        if m.word(list + 4 * i) == collidable {
            m.set(list + 4 * i, 0);
            return;
        }
    }
}

/// **The rider's bodies' vt `+0x68`** (every one of state 4's): the body's `+0x150` quadword the one
/// at `from`.
pub fn body_target(m: &mut impl Ram, body: u32, from: u32) {
    copy_quad(m, body + 0x150, from);
}

/// **The pose targets** set (`on`) or cleared: on, the latch set, the fade started (`+0x5d8` the
/// `+0x5d4`), and each body's target the pose list's wreck one, or its standing one when the record's
/// wreck kind is 13; off, the latch cleared and each body's target the standing one. The count is
/// re-read after each body.
pub fn pose_targets(m: &mut impl Rider, rider: u32, on: bool, dt: F) {
    let at = if on {
        m.set(rider + TARGETED, 1);
        let f = m.word(rider + FADE_FROM);
        m.set(rider + FADE, f);
        let rec = m.word(rider + RECORD);
        if m.word(rec + R_KIND) == KIND_AGAIN { 0x50 } else { 0x40 }
    } else {
        m.set(rider + TARGETED, 0);
        0x50
    };
    let mut i = 0;
    while i < m.word(rider + BODIES) as i32 {
        let holder = m.word(rider + MAIN);
        let poses = m.word(rider + POSES);
        let body = m.word(holder.wrapping_add(0xb0 * i as u32) + 0xa4);
        let from = poses.wrapping_add(0x60 * i as u32) + at;
        m.call(Call::BodyTarget { body, from }, dt);
        i += 1;
    }
}

/// The impact aim: while its clock runs, `+0x660` is `+0x650` as it is, turned by the main body's
/// orientation, or turned by the record's, and the clock runs down by `dt`; once it has run out, an
/// aim force still on is pointed at `(0, 0, 0, 1)` and switched off.
fn aim(m: &mut impl Rider, rider: u32, dt: F) {
    if op(m, rider + AIM_TIME) <= 0.0 {
        let force = m.word(rider + AIM_FORCE);
        if m.word(force) != 0 {
            copy_quad(m, rider + AIM, ORIGIN);
            let force = m.word(rider + AIM_FORCE);
            m.call(Call::Switch { part: force, on: 0 }, dt);
        }
        return;
    }
    if m.word(rider + AIM_AS_IS) != 0 {
        copy_quad(m, rider + AIM, rider + AIM_FROM);
    } else {
        let q = if m.word(rider + AIM_BY_BODY) != 0 {
            let body = main_body(m, rider);
            quad(m, body + B_QUAT)
        } else {
            let rec = m.word(rider + RECORD);
            quad(m, rec + 0x1c0)
        };
        let v = quad(m, rider + AIM_FROM);
        put_quad(m, rider + AIM, turned(q, v));
    }
    let t = fpu::sub(fw(m, rider + AIM_TIME), dt.0);
    put_f(m, rider + AIM_TIME, t);
}

/// The holds let go: while the clock runs it runs down by `dt`; once it has run out, both holds are
/// switched off if either is still on.
fn let_go(m: &mut impl Rider, rider: u32, dt: F) {
    let t = op(m, rider + LET_GO_TIME);
    if !(t <= 0.0) {
        put_f(m, rider + LET_GO_TIME, fpu::sub(t, dt.0));
        return;
    }
    let a = m.word(rider + LET_GO[0]);
    let on = m.word(a + 4) != 0 || {
        let b = m.word(rider + LET_GO[1]);
        m.word(b + 4) != 0
    };
    if on {
        m.call(Call::Switch { part: a, on: 0 }, dt);
        let b = m.word(rider + LET_GO[1]);
        m.call(Call::Switch { part: b, on: 0 }, dt);
    }
}

/// The bike let go of: once, when the main body is further from the record's collidable's centre
/// (`+0x50`) than its radius (`+0x70`) plus 1, every part's sphere takes it off its ignore list.
fn release_bike(m: &mut impl Ram, rider: u32) {
    let rec = m.word(rider + RECORD);
    if m.word(rider + RELEASED) != 0 || m.word(rec + R_COLLIDABLE) == 0 {
        return;
    }
    let c = m.word(rec + R_COLLIDABLE);
    let body = main_body(m, rider);
    let p = quad(m, body + B_POSITION);
    let d = p.merge(Dest::XYZ, p.sub(quad(m, c + 0x50)));
    let dist = length(d);
    let rec = m.word(rider + RECORD);
    let c = m.word(rec + R_COLLIDABLE);
    let reach = fpu::add(1.0, fw(m, c + 0x70));
    if dist <= reach {
        return;
    }
    m.set(rider + RELEASED, 1);
    let mut i = 0;
    while i < m.word(rider + PARTS_COUNT) as i32 {
        let list = m.word(rider + PARTS);
        let rec = m.word(rider + RECORD);
        let part = m.word(list.wrapping_add(4 * i as u32));
        let sphere = m.word(part + SPHERE);
        let c = m.word(rec + R_COLLIDABLE);
        release(m, sphere, c);
        i += 1;
    }
}

/// The ground's height under the main body: the rider's terrain's query on a copy of its position
/// (no normal, no surface); the position's own `y` where the terrain has no map.
fn ground_y(m: &mut impl Ram, rider: u32, map: Option<&track::Terrain<'_>>) -> f32 {
    let body = main_body(m, rider);
    let mut p = quad(m, body + B_POSITION);
    let t = m.word(rider + TERRAIN);
    let map = if m.word(t + 0x94) != 0 { map } else { None };
    let terrain = {
        let r = RefCell::new(&mut *m);
        ground::read_terrain(&|a: u32| r.borrow_mut().word(a), t, map)
    };
    crate::atv_ground::query(&terrain, &mut p, None, None);
    p.y()
}

/// **The wreck arm** (mode 2's frame before the clear loop): the impact aim ([`aim`]), the holds
/// let go ([`let_go`]), the nearby gather ([`Call::Gather`]), [`touching`], the bike let go of
/// ([`release_bike`]), the ground under the main body, then the pose-target clauses on the flag's
/// clock `t`:
///
/// - `t` 0, both clocks run out, not yet high, and the main body more than `+0x850` over the
///   ground: the targets set ([`pose_targets`]), high on, slow off;
/// - else, `t` above 0 and high: the targets cleared, high off;
/// - else, `t` past `+0x854` or the main body slower than `+0x858`, and not yet slow: slow on, the
///   targets set, and the main body's target `+0x840`.
///
/// Then the fade (`+0x5d8`) runs down by `dt` while above 0.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn wreck_arm(m: &mut impl Rider, rider: u32, dt: F, map: Option<&track::Terrain<'_>>) {
    aim(m, rider, dt);
    let_go(m, rider, dt);
    let list = m.word(rider + PARTS);
    let nearby = m.word(rider + NEARBY);
    let first = m.word(list);
    let probe = m.word(first + SPHERE);
    m.call(Call::Gather { list: nearby, probe, kind: 3 }, dt);
    touching(m, rider, dt);
    release_bike(m, rider);
    let y = ground_y(m, rider, map);
    clauses(m, rider, y, dt);
    let f = op(m, rider + FADE);
    if !(f <= 0.0) {
        put_f(m, rider + FADE, fpu::sub(f, dt.0));
    }
}

#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
fn clauses(m: &mut impl Rider, rider: u32, y: f32, dt: F) {
    let t = op(m, rider + TOUCHING_TIME);
    if t == 0.0 && op(m, rider + LET_GO_TIME) <= 0.0 && op(m, rider + AIM_TIME) <= 0.0 && m.word(rider + HIGH) == 0 {
        let body = main_body(m, rider);
        let h = fpu::sub(op(m, body + B_POSITION + 4), y);
        if !(h <= op(m, rider + HIGH_AT)) {
            pose_targets(m, rider, true, dt);
            m.set(rider + HIGH, 1);
            m.set(rider + SLOW, 0);
            return;
        }
    }
    if !(t <= 0.0) && m.word(rider + HIGH) != 0 {
        pose_targets(m, rider, false, dt);
        m.set(rider + HIGH, 0);
        return;
    }
    let slow = !(t <= op(m, rider + SLOW_AFTER)) || {
        let body = main_body(m, rider);
        let v = quad(m, body + B_VELOCITY);
        length(v) < op(m, rider + SLOW_UNDER)
    };
    if slow && m.word(rider + SLOW) == 0 {
        m.set(rider + SLOW, 1);
        pose_targets(m, rider, true, dt);
        let body = main_body(m, rider);
        m.call(Call::BodyTarget { body, from: rider + TARGET }, dt);
    }
}

/// **The mode machine** (the rider's frame runs it while its record is ridden): [`choose`],
/// [`switch`], [`holds`], [`lean`], [`follow`], in mode 2 the wreck arm ([`wreck_arm`]),
/// [`clear_all`], [`frame_end`]. `map` is the rider's terrain's cell reader, the map's tree.
pub fn modes(m: &mut impl Rider, rider: u32, dt: F, map: Option<&track::Terrain<'_>>) {
    choose(m, rider);
    switch(m, rider, dt);
    holds(m, rider, dt);
    lean(m, rider);
    follow(m, rider, dt);
    if m.word(rider + MODE) == 2 {
        wreck_arm(m, rider, dt, map);
    }
    clear_all(m, rider);
    frame_end(m, rider);
}

// --- S286: the ragdoll posed and settled ------------------------------------------------------------

use super::wreck::{self as bike_wreck, Wrecks};
use crate::air::AcosTable;
use crate::camera::CurveTable;
use crate::math::Mat4;
use crate::ragdoll_pose::{self, Bone, Start};

/// The rider's ragdoll world (`+0x27c`): its body count (`+8`) and body array (`+0x10`).
pub const WORLD: u32 = 0x27c;
pub const W_BODIES_COUNT: u32 = 0x8;
pub const W_BODIES: u32 = 0x10;
/// A body holder's (`+0x2a4`, `0xb0` apart) drawn frame (`+0x60`, four rows: last frame's going in),
/// its scene node (`+0xa0`) and its body (`+0xa4`).
pub const H_DRAWN: u32 = 0x60;
pub const H_NODE: u32 = 0xa0;
pub const H_BODY: u32 = 0xa4;
/// A ragdoll body's start, as its three setters write it: the position (`+0x40`), the velocity
/// (`+0x50`), the orientation (`+0x180`, word by word) and its rates (`+0x160`); and its centre of
/// gravity's offset (`+0x1a0`).
const B_START_POSITION: u32 = 0x40;
const B_START_VELOCITY: u32 = 0x50;
const B_START_QUAT: u32 = 0x180;
const B_START_RATES: u32 = 0x160;
const B_COFG: u32 = 0x1a0;
/// A scene node's world frame (F43).
const N_WORLD: u32 = 0x70;

/// **The rider's vt `+0xd0`: its ragdoll posed off the scene** ([`Call::Pose`]). For each body holder
/// (the count, `+0x290`, re-read after each): the holder's node's world position and its `z` and `y`
/// rows ([`node::position`], [`node::axes`]); from them, the body's start ([`ragdoll_pose::pose`],
/// MX Unleashed's ragdoll pose: the axes' quaternion on the `w >= 0` side, the position plus the
/// centre of gravity's offset turned by it, and unless `still` or `dt <= 0` the velocity and rates
/// from the holder's drawn frame over `1 / dt`) put through the body's three setters, and the drawn
/// frame rebuilt at that position. The mode machine calls it with `still` 0.
pub fn pose(m: &mut impl Ram, rider: u32, dt: F, still: bool, acos: &AcosTable, curve: &CurveTable) {
    // `c.ole.s dt, 0` taken is the still branch, so a frame time that does not compare divides.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    let inv_dt = if dt <= 0.0 || still { None } else { Some(F(fpu::div(1.0, dt.0))) };
    let mut i = 0u32;
    while (i as i32) < m.word(rider + BODIES) as i32 {
        let holder = m.word(rider + MAIN).wrapping_add(i.wrapping_mul(0xb0));
        let body = m.word(holder + H_BODY);
        let n = m.word(holder + H_NODE);
        let at = node_row(m, n, 3, F::ONE);
        let holder = m.word(rider + MAIN).wrapping_add(i.wrapping_mul(0xb0));
        let n = m.word(holder + H_NODE);
        let z = node_row(m, n, 2, F::ZERO);
        let y = node_row(m, n, 1, F::ZERO);
        let bone = Bone { at: Vf::from_bits(at), forward: Vf::from_bits(z), up: Vf::from_bits(y) };
        let holder = m.word(rider + MAIN).wrapping_add(i.wrapping_mul(0xb0));
        let mut drawn = Mat4([0, 1, 2, 3].map(|r| quad(m, holder + H_DRAWN + 0x10 * r)));
        let cofg = quad(m, body + B_COFG);
        let mut start = Start::default();
        ragdoll_pose::pose(&bone, cofg, &mut drawn, &mut start, inv_dt, acos, curve);
        put_quad(m, body + B_START_POSITION, start.position);
        put_quad(m, body + B_START_VELOCITY, start.velocity);
        for (k, w) in start.quat.to_bits().into_iter().enumerate() {
            m.set(body + B_START_QUAT + 4 * k as u32, w);
        }
        put_quad(m, body + B_START_RATES, start.rates);
        let holder = m.word(rider + MAIN).wrapping_add(i.wrapping_mul(0xb0));
        for (r, row) in drawn.0.into_iter().enumerate() {
            put_quad(m, holder + H_DRAWN + 0x10 * r as u32, row);
        }
        i += 1;
    }
}

/// A row of the node's world frame (brought up to date first) as [`node::position`] (row 3, `w` 1) and
/// [`node::axes`] (rows 2 and 1, `w` 0) hand it out: its `xyz` words as they are, and `w`.
fn node_row(m: &mut impl Ram, n: u32, row: u32, w: F) -> [u32; 4] {
    node::update(m, n);
    let at = n + N_WORLD + 0x10 * row;
    [m.word(at), m.word(at + 4), m.word(at + 8), w.to_bits()]
}

/// [`bike_wreck::body_commit`] on plain RAM: it reads and writes words and calls nothing.
struct Bodies<'a, M>(&'a mut M);

impl<M: Ram> Wrecks for Bodies<'_, M> {
    fn bike(&self) -> u32 {
        0
    }
    fn word(&mut self, at: u32) -> u32 {
        self.0.word(at)
    }
    fn set(&mut self, at: u32, w: u32) {
        self.0.set(at, w)
    }
    fn out(&mut self, c: bike_wreck::Out) -> u32 {
        unreachable!("a body's commit makes no call ({c:?})")
    }
    fn normal(&mut self, _: u32, _: u32) -> Vf {
        unreachable!("a body's commit asks for no ground")
    }
}

/// **The rider's vt `+0xcc`: its ragdoll world's bodies settled at their starts** ([`Call::Settle`],
/// the world's own settle): for each body of the world's list (the count re-read after each), its
/// vt `+0x3c`, which on the ragdoll's bodies is [`bike_wreck::body_commit`]: the staged position,
/// velocity, orientation and rates made the body's own, every accumulator emptied.
pub fn settle(m: &mut impl Ram, rider: u32) {
    let world = m.word(rider + WORLD);
    let mut i = 0u32;
    while (i as i32) < m.word(world + W_BODIES_COUNT) as i32 {
        let list = m.word(world + W_BODIES);
        let body = m.word(list.wrapping_add(4 * i));
        bike_wreck::body_commit(&mut Bodies(m), body);
        i += 1;
    }
}
