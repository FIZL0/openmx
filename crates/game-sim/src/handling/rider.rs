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
//! What is not here: the world step (3) and the rest of the mode machine. The `touched` words this
//! reads come from the original's ragdoll world until that is ported.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use super::node::Ram;
use ps2_float::vu::Dest;
use ps2_float::{F, Vf, fpu};

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
