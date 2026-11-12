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
//! The riding reset the machine calls into modes 1 and 4 ([`Call::RidingReset`]) is
//! [`riding_reset`] (S287): the holds and parts switched back for riding, the parts ignoring the bike
//! again ([`ignore`]), the joints' springs and the bodies' masses, inertias and targets put back.
//!
//! After the frame, the rider's listener step copies the wreck's two spots into the record
//! ([`wreck_spots`], F31b5i). The frame's last call, the rider's vt `+0x148` ([`after_step`]), runs
//! [`hand_over`] and puts the wreck's spot node into the record. Between them, every word the rider
//! writes into the bike's record.
//!
//! The listener step's own code is here too (F55): the rider's frame around the mode machine
//! ([`frame`]: the node's [`alpha`], two rate-limited clocks), the copy after it ([`copy_after`]) and
//! the extra step ([`extra_step`]), put together as [`listener_step`].
//!
//! The frame's world step ([`Call::WorldStep`]) is [`world_step`] (F62): the holders' drawn frames
//! kept ([`keep_drawn`]), the animation's node bits, the idle layer, the ragdoll world and the nodes
//! put where it is ([`aim_nodes`]), the respawn's still pose, the node's bounds and the controller's
//! base step ([`base_step`]).
//!
//! The idle layer ([`Call::Idle`]) is [`idle`] (F62a): the state picked ([`idle_state`]) and its clips
//! chosen and blended ([`idle_clips`], through [`anim`]).
//!
//! What is not here: the controller's own clip advance and the wreck poses' clip start (the idle
//! layer's [`Call::ClipAdvance`], [`Call::ClipStart`]), the ragdoll world itself, and the rest of the
//! calls in [`Call`].
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

/// The record's two quadwords the rider's listener step writes after its frame while the record is
/// wrecked (the bike's camera spots on the wreck): the impact aim, and the main body's node's
/// `+0x80`.
pub const R_SPOT_AIM: u32 = 0x140;
pub const R_SPOT_MAIN: u32 = 0x150;

/// **The wreck's spots into the record** (the rider's listener step, after the rider's frame): with
/// a record whose wreck kind is set, the impact aim (`+0x660`) to the record's `+0x140`, then the
/// main body's node's `+0x80` to its `+0x150`; the record pointer read again for the second. Both
/// are `lq`/`sq` moves: words as they are, the addresses' low four bits dropped.
pub fn wreck_spots(m: &mut impl Ram, rider: u32) {
    let record = m.word(rider + RECORD);
    if record == 0 || m.word(record + R_KIND) == 0 {
        return;
    }
    copy_words(m, record + R_SPOT_AIM, rider + AIM);
    let holder = m.word(rider + MAIN);
    let record = m.word(rider + RECORD);
    let main = m.word(holder + 0xa4);
    copy_words(m, record + R_SPOT_MAIN, main + 0x80);
}

/// The record's words the rider's vt `+0x148` writes: the spot node's position and its `z` and `y`
/// rows while wrecked (the bike's `+0x870..+0x8a0`), and the word it copies to the rider's `+0x4f0`.
pub const R_SPOT_AT: u32 = 0x220;
pub const R_SPOT_Z: u32 = 0x230;
pub const R_SPOT_Y: u32 = 0x240;
pub const R_KEPT: u32 = 0x128;
pub const KEPT: u32 = 0x4f0;

/// **The rider's vt `+0x148`** (its frame's last call, after the ragdoll world's step):
/// [`hand_over`]; then with a record, while its wreck kind is set the spot node's (`+0xb14`) world
/// position to the record's `+0x220` and its `z` and `y` rows to `+0x230` and `+0x240`
/// ([`node::position`], [`node::axes`]), and either way the record's `+0x128` to the rider's
/// `+0x4f0` (the record pointer read again for each).
pub fn after_step(m: &mut impl Ram, rider: u32) {
    hand_over(m, rider);
    let record = m.word(rider + RECORD);
    if record == 0 {
        return;
    }
    if m.word(record + R_KIND) != 0 {
        let node = m.word(rider + SPOT_NODE);
        node::position(m, node, record + R_SPOT_AT);
        let (node, record) = (m.word(rider + SPOT_NODE), m.word(rider + RECORD));
        node::axes(m, node, record + R_SPOT_Z, record + R_SPOT_Y);
    }
    let record = m.word(rider + RECORD);
    let w = m.word(record + R_KEPT);
    m.set(rider + KEPT, w);
}

/// A quadword moved as `lq`/`sq` move it: four words as they are, the addresses' low four bits
/// dropped.
fn copy_words(m: &mut impl Ram, to: u32, from: u32) {
    for k in [0, 4, 8, 12] {
        let w = m.word((from & !0xf) + k);
        m.set((to & !0xf) + k, w);
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
    /// The wreck entry's last call: the rider's `+0x8f4` object (`a0`, `a1` 0) told of the wreck,
    /// which reads the record's wreck kind itself.
    WreckCue(u32),
    /// The rider's vt `+0xd0` (`dt`, 0): the ragdoll posed off the scene ([`pose`]); vt `+0xcc`: its
    /// world's bodies settled ([`settle`]).
    Pose,
    Settle,
    /// A part's on/off virtual (its vt at `+0x10`, slot `+0x10`): the part, on or off. The wreck
    /// arm's aim force (`+0x5bc`) is switched off by the same call, its vt at `+8` ([`switch_on`]).
    Switch { part: u32, on: u32 },
    /// The wreck arm's nearby gather: the rider's list (`+0x5b8`) refilled from the broad phase
    /// around the first part's sphere, kind 3 (the quad's [`crate::atv_world`] gather on RAM).
    Gather { list: u32, probe: u32, kind: u32 },
    /// A ragdoll body's vt `+0x68`: its `+0x150` quadword from `from` (on the rider's own bodies,
    /// [`body_target`]).
    BodyTarget { body: u32, from: u32 },
    /// The riding reset's (S287) and the wreck entry's (S288): a body's vt `+0x18`, its mass (the float's bits, in `f12`)
    /// ([`body_mass`]); its vt `+0x60`, its inertia from the quadword at `from` ([`body_inertia`]); its
    /// vt `+0x20`, its `+0x30` from the quadword at `from` ([`body_at`]).
    BodyMass { body: u32, mass: u32 },
    BodyInertia { body: u32, from: u32 },
    BodyAt { body: u32, from: u32 },
    /// The heap's resize of a shape's ignore list (`list` to `bytes`, [`ignore`]): its answer the
    /// list's new address, the old entries in it.
    Grow { list: u32, bytes: u32 },
    /// The rider's frame's (F55): the rider's ragdoll world's step, with `dt` (its answer the frame's),
    /// and the rider's vt `+0x148` after it.
    WorldStep,
    AfterStep,
    /// The copy after the frame's (F55): the scene node's position (`a1` 0) into the quadword at `out`.
    NodePosition { node: u32, out: u32 },
    /// The world step's ([`world_step`], F62): the rider's vt `+0xd4`, its idle layer (with `dt`);
    /// the ragdoll world (`+0x27c`) stepped by `dt`; the respawn's vt `+0xd0` with `a1` 1, the
    /// ragdoll posed still ([`pose`]); the scene node's bounds into the spatial database.
    Idle,
    RagWorld,
    PoseStill,
    Bounds { node: u32 },
    /// The base step's ([`base_step`]): the controller's entry `index` bound (the entry's words
    /// unset); the entry list at `list` grown to `count`; a listener of the rider's own group stepped
    /// by `dt`.
    EntryBind { index: u32 },
    EntriesGrow { list: u32, count: u32 },
    Listener { listener: u32 },
    /// The idle layer's ([`idle`], F62a): `rand` seeded from the clock once (before a wreck pose is
    /// drawn by lot); the wreck pose's clip started at the rate the layer holds in `f12`; the
    /// controller's own clip advance (its vt `+0x88`, `f12` `dt`); a saved-pose buffer of `bytes` made
    /// for [`anim::stop`] (its address the answer).
    Seed,
    ClipStart { clip: u32 },
    ClipAdvance,
    SavedPose { bytes: u32, buffer: anim::Saved },
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

/// A quadword moved as `lq`/`sq` move it: the raw words (S287: through [`Vf`] a denormal would come
/// out flushed).
fn copy_quad(m: &mut impl Ram, to: u32, from: u32) {
    copy_words(m, to, from);
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

/// **A part's on/off virtual** (every hold's and named part's on state 4): its `+4` word is `on`.
pub fn part_switch(m: &mut impl Ram, part: u32, on: u32) {
    m.set(part + 4, on);
}

/// **The aim force's on/off virtual**: its `+0` word is `on`.
pub fn force_switch(m: &mut impl Ram, force: u32, on: u32) {
    m.set(force, on);
}

/// [`Call::Switch`] answered: the rider's aim force (`+0x5bc`) by [`force_switch`], any other part
/// by [`part_switch`].
pub fn switch_on(m: &mut impl Ram, rider: u32, part: u32, on: u32) {
    if part == m.word(rider + AIM_FORCE) {
        force_switch(m, part, on);
    } else {
        part_switch(m, part, on);
    }
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

// --- S287: the riding reset -------------------------------------------------------------------------

/// The riding reset's words: the ragdoll left as it is (`+0x25c`, else posed and settled first); the
/// world's iterations and rate for riding (`+0x544`, `+0x54c`, into the world's `+0` and `+4`); the
/// parts always switched on (the four holds, `+0x550..+0x55c`, and `+0x578`) and the four switched on
/// only with `+0x4f0` (`+0x57c..+0x588`); the three latches set at the end (`+0x38c`, `+0x264`
/// [`TARGETED`], `+0x254`). The pose list's entries (`+0x3b0`, `0x50` each) hand their joint (`+8`)
/// its strength (`+0x10`), its damping ratio (`+0x20`) and `+0xc`; the body list's (`+0x3a8`, `0x60`
/// each) hand their body its mass (`+0`), inertia (`+0x10`) and target (`+0x30`).
pub const LEFT_AS_IS: u32 = 0x25c;
pub const RIDE_ITERATIONS: u32 = 0x544;
pub const RIDE_RATE: u32 = 0x54c;
pub const RIDE_ON: [u32; 5] = [0x550, 0x554, 0x558, 0x55c, 0x578];
pub const RIDE_EXTRA: u32 = 0x4f0;
pub const RIDE_EXTRA_PARTS: [u32; 4] = [0x57c, 0x580, 0x584, 0x588];
/// The quadword the reset puts at every body's `+0x30`: rodata's `(1, 1, 1, 1)`, after [`ORIGIN`].
pub const ONES: u32 = 0x0039_6fd0;

/// **The rider put back on the bike** (the mode machine's [`Call::RidingReset`], into modes 1 and 4):
/// unless `+0x25c`, the ragdoll posed ([`Call::Pose`]) and settled ([`Call::Settle`]); the world's
/// iterations and rate the riding ones; the holds and `+0x578` switched on, the four extra parts on or
/// off by `+0x4f0`, the aim force off; every part of the list off, its touched flag 0, and with a
/// record its sphere ignoring the record's collidable ([`ignore`]); every joint of the pose list
/// stopped (`+0x80` 0) and given its entry's strength, damping ratio and damping (ratio × 2 ×
/// √(its second body's (`+0x7c`) inertia × strength), per axis) and `+0xd4`; every body given its mass, inertia, target and
/// `(1, 1, 1, 1)` at its `+0x30` (its vt `+0x18`, `+0x60`, `+0x68`, `+0x20`); then `+0x38c`, `+0x264` and `+0x254`
/// set. Every count is re-read after each entry.
pub fn riding_reset(m: &mut impl Rider, rider: u32, dt: F) {
    if m.word(rider + LEFT_AS_IS) == 0 {
        m.call(Call::Pose, dt);
        m.call(Call::Settle, dt);
    }
    let it = m.word(rider + RIDE_ITERATIONS);
    let world = m.word(rider + WORLD);
    m.set(world, it);
    let rate = m.word(rider + RIDE_RATE);
    let world = m.word(rider + WORLD);
    m.set(world + 4, rate);
    for o in RIDE_ON {
        switch_part(m, rider, o, 1, dt);
    }
    let on = u32::from(m.word(rider + RIDE_EXTRA) != 0);
    for o in RIDE_EXTRA_PARTS {
        switch_part(m, rider, o, on, dt);
    }
    switch_part(m, rider, AIM_FORCE, 0, dt);
    let mut i = 0u32;
    while (i as i32) < m.word(rider + PARTS_COUNT) as i32 {
        let part = part_at(m, rider, i);
        m.call(Call::Switch { part, on: 0 }, dt);
        let part = part_at(m, rider, i);
        m.set(part + TOUCHED, 0);
        let rec = m.word(rider + RECORD);
        if rec != 0 {
            let part = part_at(m, rider, i);
            let sphere = m.word(part + SPHERE);
            let c = m.word(rec + R_COLLIDABLE);
            ignore(m, sphere, c, dt);
        }
        i += 1;
    }
    let mut i = 0u32;
    while (i as i32) < m.word(rider + 0x3ac) as i32 {
        let e = m.word(rider + 0x3b0).wrapping_add(i.wrapping_mul(0x50));
        let joint = m.word(e + 8);
        m.set(joint + 0x80, 0);
        let e = m.word(rider + 0x3b0).wrapping_add(i.wrapping_mul(0x50));
        let joint = m.word(e + 8);
        let d4 = m.word(e + 0xc);
        copy_quad(m, joint + 0xe0, e + 0x10);
        copy_quad(m, joint + 0xf0, e + 0x20);
        for k in 0..3 {
            let other = m.word(joint + 0x7c);
            let ratio = fpu::mul(2.0, fw(m, joint + 0xf0 + 4 * k));
            let mk = fpu::mul(fw(m, other + 0x130 + 4 * k), fw(m, joint + 0xe0 + 4 * k));
            put_f(m, joint + 0x100 + 4 * k, fpu::mul(ratio, fpu::sqrt(mk)));
        }
        m.set(joint + 0xd4, d4);
        i += 1;
    }
    let mut i = 0u32;
    while (i as i32) < m.word(rider + BODIES) as i32 {
        let body = body_of(m, rider, i);
        let e = poses(m, rider, i);
        let mass = m.word(e);
        m.call(Call::BodyMass { body, mass }, dt);
        let (body, e) = (body_of(m, rider, i), poses(m, rider, i));
        m.call(Call::BodyInertia { body, from: e + 0x10 }, dt);
        let (body, e) = (body_of(m, rider, i), poses(m, rider, i));
        m.call(Call::BodyTarget { body, from: e + 0x30 }, dt);
        let body = body_of(m, rider, i);
        m.call(Call::BodyAt { body, from: ONES }, dt);
        i += 1;
    }
    m.set(rider + 0x38c, 1);
    m.set(rider + TARGETED, 1);
    m.set(rider + 0x254, 1);
}

/// The `i`th part of the list.
fn part_at(m: &mut impl Ram, rider: u32, i: u32) -> u32 {
    let list = m.word(rider + PARTS);
    m.word(list.wrapping_add(4 * i))
}

/// The body of the `i`th holder, and the `i`th entry of the body list.
fn body_of(m: &mut impl Ram, rider: u32, i: u32) -> u32 {
    let holder = m.word(rider + MAIN).wrapping_add(i.wrapping_mul(0xb0));
    m.word(holder + H_BODY)
}

fn poses(m: &mut impl Ram, rider: u32, i: u32) -> u32 {
    m.word(rider + POSES).wrapping_add(i.wrapping_mul(0x60))
}

/// **A collidable put on a shape's ignore list** (not 0, and not there already): the count read once;
/// the first empty entry takes it, else the list grows by one entry ([`Call::Grow`]) and it goes last.
pub fn ignore(m: &mut impl Rider, shape: u32, collidable: u32, dt: F) {
    if collidable == 0 {
        return;
    }
    let n = m.word(shape + S_IGNORE_COUNT);
    if n as i32 > 0 {
        let list = m.word(shape + S_IGNORE);
        if (0..n).any(|i| m.word(list.wrapping_add(4 * i)) == collidable) {
            return;
        }
        let list = m.word(shape + S_IGNORE);
        if let Some(i) = (0..n).find(|&i| m.word(list.wrapping_add(4 * i)) == 0) {
            m.set(list.wrapping_add(4 * i), collidable);
            return;
        }
    }
    let list = m.word(shape + S_IGNORE);
    let new = m.call(Call::Grow { list, bytes: n.wrapping_add(1).wrapping_mul(4) }, dt);
    m.set(shape + S_IGNORE, new);
    let n = m.word(shape + S_IGNORE_COUNT);
    let list = m.word(shape + S_IGNORE);
    m.set(list.wrapping_add(n.wrapping_mul(4)), collidable);
    let n = m.word(shape + S_IGNORE_COUNT);
    m.set(shape + S_IGNORE_COUNT, n.wrapping_add(1));
}

/// **A ragdoll body's vt `+0x18`: its mass** (the float's bits) at `+0x18`, and `1 / mass` at `+0x1c`.
pub fn body_mass(m: &mut impl Ram, body: u32, mass: u32) {
    m.set(body + 0x18, mass);
    let x = fw(m, body + 0x18);
    put_f(m, body + 0x1c, fpu::div(1.0, x));
}

/// **A ragdoll body's vt `+0x60`: its inertia** from the quadword at `from`: `from`'s `w` set to 1
/// first, the quadword copied to the body's `+0x130`, and each of `x`, `y`, `z`'s inverse (0 where it
/// is not above 0) to its `+0x140`.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn body_inertia(m: &mut impl Ram, body: u32, from: u32) {
    m.set(from + 0xc, 0x3f80_0000);
    copy_quad(m, body + 0x130, from);
    for k in 0..3 {
        let x = op(m, from + 4 * k);
        let inv = if x <= 0.0 { 0.0 } else { fpu::div(1.0, x) };
        put_f(m, body + 0x140 + 4 * k, inv);
    }
}

/// **A ragdoll body's vt `+0x20`**: its `+0x30` quadword the one at `from`.
pub fn body_at(m: &mut impl Ram, body: u32, from: u32) {
    copy_quad(m, body + 0x30, from);
}

// --- S288: the wreck entry -----------------------------------------------------------------------

use ps2_float::vu::Field;

/// The wreck entry's words: the ragdoll world's two settings it puts back (`+0x540`, `+0x548` into
/// the world's `+0` and `+4`); the seven parts it switches off first; the pose list's entries
/// (`+0x3b0`, `+0x3ac` of them, `0x50` bytes); the latch `+0x254`; the rider's `+0x8f4` object.
pub const WORLD_SETTINGS: [u32; 2] = [0x540, 0x548];
pub const OFF_FIRST: [u32; 7] = [0x558, 0x55c, 0x578, 0x57c, 0x580, 0x584, 0x588];
pub const CUE: u32 = 0x8f4;
/// The aim force's body (`+0x24` of the force).
const F_BODY: u32 = 0x24;
/// The record's velocities the wreck kinds hand the bodies: `+0x180` (6, 7, 13), `+0xa0` and `+0xb0`
/// (5, 12).
const R_VELOCITY: u32 = 0x180;
const R_KICK: u32 = 0xa0;
const R_KICK_SPIN: u32 = 0xb0;
/// A ragdoll body's spin rates (`+0x1d0`, beside [`B_VELOCITY`]).
const B_RATES: u32 = 0x1d0;

/// `2⁻³¹`, and rand's answer scaled by it into `[0, 1)`.
const RAND_SCALE: f32 = f32::from_bits(0x3000_0000);

fn unit(m: &mut impl Rider, dt: F) -> f32 {
    let r = m.call(Call::Rand, dt) as i32;
    fpu::mul(RAND_SCALE, fpu::from_i32(r))
}

/// A rand answer into `[-1, 1)`: `2u − 1`.
fn signed_unit(m: &mut impl Rider, dt: F) -> f32 {
    let u = unit(m, dt);
    fpu::sub(fpu::mul(2.0, u), 1.0)
}

/// A quadword's `xyz` stored from `v`, its `w` the word `w` as it is (a lane the VU kept).
fn put_xyz(m: &mut impl Ram, at: u32, v: Vf, w: u32) {
    let [x, y, z, _] = v.to_bits();
    for (k, b) in [x, y, z, w].into_iter().enumerate() {
        m.set((at & !0xf) + 4 * k as u32, b);
    }
}

/// The body of holder `i`.
fn holder_body(m: &mut impl Ram, rider: u32, i: u32) -> u32 {
    let holder = m.word(rider + MAIN);
    m.word(holder.wrapping_add(0xb0u32.wrapping_mul(i)) + H_BODY)
}

/// Every holder's body's velocity (`+0x80`) the record's quadword at `from`, the count re-read
/// after each; with `stop`, its `y` then 0.
fn bodies_from_record(m: &mut impl Ram, rider: u32, from: u32, stop: bool) {
    let mut i = 0u32;
    while (i as i32) < m.word(rider + BODIES) as i32 {
        let rec = m.word(rider + RECORD);
        let body = holder_body(m, rider, i);
        copy_words(m, body + B_VELOCITY, rec + from);
        if stop {
            let body = holder_body(m, rider, i);
            m.set(body + B_VELOCITY + 4, 0);
        }
        i += 1;
    }
}

/// Kind 5's kick: each holder's body (the count re-read after each) given the record's `+0xa0`
/// velocity and its `+0xb0` rates.
fn kick(m: &mut impl Ram, rider: u32) {
    let mut i = 0u32;
    while (i as i32) < m.word(rider + BODIES) as i32 {
        let rec = m.word(rider + RECORD);
        let body = holder_body(m, rider, i);
        copy_words(m, body + B_VELOCITY, rec + R_KICK);
        let rec = m.word(rider + RECORD);
        let body = holder_body(m, rider, i);
        copy_words(m, body + B_RATES, rec + R_KICK_SPIN);
        i += 1;
    }
}

/// The aim force pointed at holder `at`'s body (`+0xa4` the main one), and `+0x630` (the aim turned
/// by that body) set or cleared, `+0x62c` (as is) cleared.
fn aim_at(m: &mut impl Ram, rider: u32, at: u32, by_body: bool) {
    let holder = m.word(rider + MAIN);
    let force = m.word(rider + AIM_FORCE);
    let body = m.word(holder + at);
    m.set(force + F_BODY, body);
    m.set(rider + AIM_BY_BODY, u32::from(by_body));
    m.set(rider + AIM_AS_IS, 0);
}

/// **A wreck kind's random aim and clocks**, off its block of the rider's words at `b` (`0x40`
/// bytes: `+0` the aim's and let-go's base times, `+0x10` the aim, `+0x20` the times' spreads,
/// `+0x30` the aim's spread): the aim `+0x650` the block's plus its spread by three signed rands
/// (`w` the block's), its `x` flipped where `flip`; then the aim clock `+0x640` and the let-go clock
/// `+0x644` each its base plus its spread by a signed rand.
fn aim_and_clocks(m: &mut impl Rider, rider: u32, b: u32, flip: bool, dt: F) {
    let mut s = [0f32; 3];
    for (k, s) in s.iter_mut().enumerate() {
        let u = signed_unit(m, dt);
        *s = fpu::mul(fw(m, rider + b + 0x30 + 4 * k as u32), u);
    }
    let base = quad(m, rider + b + 0x10);
    let w = m.word(rider + b + 0x1c);
    put_xyz(m, rider + AIM_FROM, base.add(Vf::new(s[0], s[1], s[2], 1.0)), w);
    if flip {
        let x = fpu::mul(fw(m, rider + AIM_FROM), -1.0);
        put_f(m, rider + AIM_FROM, x);
    }
    for (k, to) in [AIM_TIME, LET_GO_TIME].into_iter().enumerate() {
        let u = signed_unit(m, dt);
        let at = rider + b + 4 * k as u32;
        let acc = fpu::adda(0.0, fw(m, at));
        let spread = fw(m, at + 0x20);
        put_f(m, rider + to, fpu::madd(acc, spread, u));
    }
}

/// A random hold, on where rand's `[0, 1)` comes under 0.66.
fn maybe_hold(m: &mut impl Rider, rider: u32, at: u32, dt: F) {
    let u = unit(m, dt);
    let on = u32::from(u < f32::from_bits(0x3f28_f5c3));
    switch_part(m, rider, at, on, dt);
}

/// Kind 13's tumble: a random spin `s` (each lane 5 × rand's `[0, 1)`), and each holder's body
/// (the count read before the first, re-read after each) given the record's velocity, its `x` and `z`
/// × 1.02 and `y` × 1.01, plus `s × d` for its offset `d` from the main body; and the spin `s` turned
/// by its orientation into its rates (`w` 1).
fn tumble(m: &mut impl Rider, rider: u32, dt: F) {
    let mut s = [0f32; 3];
    for s in &mut s {
        let u = unit(m, dt);
        *s = fpu::mul(5.0, u);
    }
    let s = Vf::new(s[0], s[1], s[2], 1.0);
    let rec = m.word(rider + RECORD);
    let v = quad(m, rec + R_VELOCITY);
    let v_w = m.word(rec + R_VELOCITY + 0xc);
    let v = Vf::new(fpu::mul(v.x(), f32::from_bits(0x3f82_8f5c)), fpu::mul(v.y(), f32::from_bits(0x3f81_47ae)), fpu::mul(v.z(), f32::from_bits(0x3f82_8f5c)), 0.0);
    let mut i = 0u32;
    while (i as i32) < m.word(rider + BODIES) as i32 {
        let holder = m.word(rider + MAIN);
        let first = m.word(holder + H_BODY);
        let body = m.word(holder.wrapping_add(0xb0u32.wrapping_mul(i)) + H_BODY);
        let d = quad(m, body + B_POSITION).sub(quad(m, first + B_POSITION));
        let c = Vf::opmsub(Vf::opmula(s, d), d, s);
        let body = holder_body(m, rider, i);
        put_xyz(m, body + B_VELOCITY, v.add(c), v_w);
        let body = holder_body(m, rider, i);
        let q = quad(m, body + B_QUAT);
        let acc = Vf::madd(Vf::opmula(s, q), s, q.bc(Field::W));
        let t = Vf::opmsub(acc, q, s);
        let qs = q.mul(s);
        let acc = Vf::opmula(t, q);
        let acc = Vf::madd(acc, q, qs.bc(Field::X));
        let acc = Vf::madd(acc, q, qs.bc(Field::Y));
        let acc = Vf::madd(acc, q, qs.bc(Field::Z));
        let acc = Vf::madd(acc, t, q.bc(Field::W));
        put_xyz(m, body + B_RATES, Vf::opmsub(acc, q, t), 0x3f80_0000);
        i += 1;
    }
}

/// **The wreck entry** ([`Call::WreckEntry`]: the mode machine going into the wreck, or the record's
/// kind turning 13 while wrecked), `kind` the record's wreck kind:
///
/// 1. the ragdoll posed and settled ([`Call::Pose`], [`Call::Settle`]), its world's two settings put
///    back, the seven parts of [`OFF_FIRST`] off and the aim force on;
/// 2. every part switched on and its sphere made to ignore the record's collidable ([`ignore`]);
/// 3. each pose-list entry's body woken (`+0x80` 1), its `+0xe0` and `+0xf0` quadwords the entry's
///    `+0x30` and `+0x40`, its three `+0x100` lanes `2·f × √(m·k)` (`f` of `+0xf0`, `k` of `+0xe0`,
///    `m` its `+0x7c` object's `+0x130..`), its `+0xd4` the entry's `+0xc`;
/// 4. each holder's body given the pose list's mass, inertia and target ([`body_mass`],
///    [`body_inertia`], [`body_target`]);
/// 5. `+0x38c` 1; the pose targets, latch `+0x254`, the bike released, high, slow and the touching
///    clock all 0;
/// 6. by the kind: 1–5, 8, 11 and 12 the holds (5: each at random), the aim force at a body, and a
///    random aim and clocks ([`aim_and_clocks`]; 5 and 12 first hand the bodies the record's kick);
///    6 and 7 the record's velocity to every body, holds and force off; 13 holds off and the
///    tumble ([`tumble`]); 15 holds and force off and every body dropped (velocity `(0, −2, 0)`,
///    `+0xa0`, `+0xa8` 0); any other nothing;
/// 7. the rider's `+0x8f4` object told ([`Call::WreckCue`]).
pub fn wreck_entry(m: &mut impl Rider, rider: u32, kind: u32, dt: F) {
    m.call(Call::Pose, dt);
    m.call(Call::Settle, dt);
    let a = m.word(rider + WORLD_SETTINGS[0]);
    let world = m.word(rider + WORLD);
    m.set(world, a);
    let b = m.word(rider + WORLD_SETTINGS[1]);
    let world = m.word(rider + WORLD);
    m.set(world + 4, b);
    for at in OFF_FIRST {
        switch_part(m, rider, at, 0, dt);
    }
    switch_part(m, rider, AIM_FORCE, 1, dt);
    let mut i = 0u32;
    while (i as i32) < m.word(rider + PARTS_COUNT) as i32 {
        let list = m.word(rider + PARTS);
        let part = m.word(list.wrapping_add(4 * i));
        m.call(Call::Switch { part, on: 1 }, dt);
        let list = m.word(rider + PARTS);
        let rec = m.word(rider + RECORD);
        let part = m.word(list.wrapping_add(4 * i));
        let sphere = m.word(part + SPHERE);
        let c = m.word(rec + R_COLLIDABLE);
        ignore(m, sphere, c, dt);
        i += 1;
    }
    let mut i = 0u32;
    while (i as i32) < m.word(rider + 0x3ac) as i32 {
        let entry = |m: &mut _, i: u32| -> u32 { Ram::word(m, rider + 0x3b0).wrapping_add(i.wrapping_mul(0x50)) };
        let e = entry(m, i);
        let body = m.word(e + 8);
        m.set(body + 0x80, 1);
        let e = entry(m, i);
        let body = m.word(e + 8);
        let d4 = m.word(e + 0xc);
        copy_words(m, body + 0xe0, e + 0x30);
        copy_words(m, body + 0xf0, e + 0x40);
        for k in 0..3 {
            let o = m.word(body + 0x7c);
            let f = fw(m, body + 0xf0 + 4 * k);
            let kk = fw(m, body + 0xe0 + 4 * k);
            let mm = fw(m, o + 0x130 + 4 * k);
            let x = fpu::mul(fpu::mul(2.0, f), fpu::sqrt(fpu::mul(mm, kk)));
            put_f(m, body + 0x100 + 4 * k, x);
        }
        m.set(body + 0xd4, d4);
        i += 1;
    }
    let mut i = 0u32;
    while (i as i32) < m.word(rider + BODIES) as i32 {
        let at = |m: &mut _| Ram::word(m, rider + POSES).wrapping_add(0x60u32.wrapping_mul(i));
        let body = holder_body(m, rider, i);
        let p = at(m);
        let mass = m.word(p + 4);
        m.call(Call::BodyMass { body, mass }, dt);
        let body = holder_body(m, rider, i);
        let from = at(m) + 0x20;
        m.call(Call::BodyInertia { body, from }, dt);
        let body = holder_body(m, rider, i);
        let from = at(m) + 0x50;
        m.call(Call::BodyTarget { body, from }, dt);
        i += 1;
    }
    m.set(rider + 0x38c, 1);
    m.set(rider + TARGETED, 0);
    m.set(rider + 0x254, 0);
    m.set(rider + RELEASED, 0);
    m.set(rider + HIGH, 0);
    m.set(rider + SLOW, 0);
    m.set(rider + TOUCHING_TIME, 0);
    let [l, r] = LET_GO;
    // Each aimed kind: its two holds, the holder its aim force follows, whether the aim turns with
    // that body, its block of words, whether the aim's x flips.
    let aimed = |l_on: u32, r_on: u32, at: u32, by_body: bool, b: u32, flip: bool| (Some((l_on, r_on)), at, by_body, b, flip);
    let plan = match kind {
        1 => Some(aimed(1, 1, H_BODY, true, 0x680, false)),
        2 => Some(aimed(0, 1, H_BODY, false, 0x6c0, false)),
        3 => Some(aimed(1, 0, H_BODY, false, 0x6c0, true)),
        4 => Some(aimed(0, 0, 3 * 0xb0 + H_BODY, false, 0x700, true)),
        5 => {
            kick(m, rider);
            Some((None, H_BODY, true, 0x740, false))
        }
        8 => Some(aimed(1, 1, H_BODY, false, 0x780, true)),
        11 => Some(aimed(0, 0, H_BODY, false, 0x7c0, true)),
        12 => {
            bodies_from_record(m, rider, R_KICK, true);
            Some(aimed(0, 0, H_BODY, false, 0x800, true))
        }
        6 | 7 => {
            bodies_from_record(m, rider, R_VELOCITY, false);
            switch_part(m, rider, l, 0, dt);
            switch_part(m, rider, r, 0, dt);
            switch_part(m, rider, AIM_FORCE, 0, dt);
            aim_at(m, rider, H_BODY, true);
            None
        }
        13 => {
            switch_part(m, rider, l, 0, dt);
            switch_part(m, rider, r, 0, dt);
            tumble(m, rider, dt);
            None
        }
        15 => {
            switch_part(m, rider, l, 0, dt);
            switch_part(m, rider, r, 0, dt);
            switch_part(m, rider, AIM_FORCE, 0, dt);
            m.set(rider + AIM_BY_BODY, 0);
            m.set(rider + AIM_AS_IS, 0);
            let mut i = 0u32;
            while (i as i32) < m.word(rider + BODIES) as i32 {
                for (o, w) in [(0x80, 0), (0x88, 0), (0x84, 0xc000_0000), (0xa0, 0), (0xa8, 0)] {
                    let body = holder_body(m, rider, i);
                    m.set(body + o, w);
                }
                i += 1;
            }
            None
        }
        _ => None,
    };
    if let Some((holds, at, by_body, b, flip)) = plan {
        match holds {
            Some((l_on, r_on)) => {
                switch_part(m, rider, l, l_on, dt);
                switch_part(m, rider, r, r_on, dt);
            }
            None => {
                maybe_hold(m, rider, l, dt);
                maybe_hold(m, rider, r, dt);
            }
        }
        aim_at(m, rider, at, by_body);
        aim_and_clocks(m, rider, b, flip, dt);
    }
    let cue = m.word(rider + CUE);
    m.call(Call::WreckCue(cue), dt);
}

// --- F55: the rider's listener step -----------------------------------------------------------------

/// The rider's frame's words: its kind (`+0x3a4`; 2 and 3 fade the node) and the kind-3 fade's
/// switch (`+0x8fc`); the two clocks (`+0x360`, `+0x364`), each run toward its target (`+0x340`,
/// `+0x344`) by at most its rate times `dt`, the rate chosen by a flag (`+0x338`/`+0x33c`: set
/// `+0x350`/`+0x354`, clear `+0x358`/`+0x35c`); the word cleared when the record is not ridden
/// (`+0x25c`).
pub const KIND: u32 = 0x3a4;
pub const KIND3_FADE: u32 = 0x8fc;
pub const CLOCKS: [(u32, u32, u32, u32, u32); 2] = [(0x360, 0x340, 0x338, 0x350, 0x358), (0x364, 0x344, 0x33c, 0x354, 0x35c)];
pub const UNRIDDEN_CLEAR: u32 = 0x25c;
/// A scene node's alpha (`+0x17c`; the setter answers the old one, which the frame drops).
pub const ALPHA: u32 = 0x17c;
/// The record's fade words: how the record asks for the alpha (`+0x278`: 1 whole, 2 half), and the
/// fade's time and its start (`+0x27c`, `+0x280`).
pub const R_ALPHA: u32 = 0x278;
pub const R_FADE: u32 = 0x27c;
pub const R_FADE_FROM: u32 = 0x280;

fn put_alpha(m: &mut impl Ram, rider: u32, a: f32) {
    let node = m.word(rider + NODE);
    put_f(m, node + ALPHA, a);
}

/// **The rider's node's alpha** (the frame's first call). Kind 2: a half. Kind 3: whole with the
/// kind-3 switch (`+0x8fc`) clear, a half with it set (the switch read twice, the second time by the
/// shared kind-3 test). Any other kind: the record's ask (`+0x278`: 1 whole, 2 a half, others
/// nothing); then, while the record's fade (`+0x27c`) is above 0, `0.1 + (fade − from) / (0 − from)
/// · 0.9` clamped to 0.1..1 (on the accumulator), or just 0.1 with `from` (`+0x280`) 0.
pub fn alpha(m: &mut impl Ram, rider: u32) {
    let kind = m.word(rider + KIND);
    if kind == 2 {
        put_alpha(m, rider, 0.5);
        return;
    }
    if kind == 3 && m.word(rider + KIND3_FADE) == 0 {
        put_alpha(m, rider, 1.0);
        return;
    }
    if kind == 3 && m.word(rider + KIND3_FADE) != 0 {
        put_alpha(m, rider, 0.5);
        return;
    }
    let rec = m.word(rider + RECORD);
    match m.word(rec + R_ALPHA) {
        2 => put_alpha(m, rider, 0.5),
        1 => put_alpha(m, rider, 1.0),
        _ => {}
    }
    let rec = m.word(rider + RECORD);
    if op(m, rec + R_FADE) <= 0.0 {
        return;
    }
    let (fade, from) = (fw(m, rec + R_FADE), fw(m, rec + R_FADE_FROM));
    let low = f32::from_bits(0x3dcc_cccd);
    let a = if op(m, rec + R_FADE_FROM) == 0.0 {
        low
    } else {
        let t = fpu::div(fpu::sub(fade, from), fpu::sub(0.0, from));
        let v = fpu::madd(fpu::adda(0.0, low), t, fpu::sub(1.0, low));
        fpu::min(fpu::max(v, fpu::min(low, 1.0)), fpu::max(low, 1.0))
    };
    put_alpha(m, rider, a);
}

/// **The rider's frame** (the listener step's first call; its answer is the step's): 1 at once
/// with no scene node or no record. Else [`alpha`]; the two clocks each moved toward their targets
/// by at most `rate · dt` (`+0x360` toward `+0x340`, `+0x364` toward `+0x344`, the rates read before
/// either moves); the mode machine ([`modes`]) with the record ridden, else `+0x25c` cleared; then
/// the ragdoll world's step ([`Call::WorldStep`], whose answer is the frame's) and the rider's vt
/// `+0x148` ([`Call::AfterStep`]).
pub fn frame(m: &mut impl Rider, rider: u32, dt: F, map: Option<&track::Terrain<'_>>) -> u32 {
    if m.word(rider + NODE) == 0 || m.word(rider + RECORD) == 0 {
        return 1;
    }
    alpha(m, rider);
    let rates = CLOCKS.map(|(_, _, flag, set, clear)| {
        let r = if m.word(rider + flag) != 0 { set } else { clear };
        fpu::mul(fw(m, rider + r), dt.0)
    });
    for ((clock, target, ..), step) in CLOCKS.into_iter().zip(rates) {
        let c = fw(m, rider + clock);
        let d = fpu::sub(fw(m, rider + target), c);
        let d = fpu::min(fpu::max(d, fpu::neg(step)), step);
        put_f(m, rider + clock, fpu::add(c, d));
    }
    let rec = m.word(rider + RECORD);
    if m.word(rec + R_RIDDEN) != 0 {
        modes(m, rider, dt, map);
    } else {
        m.set(rider + UNRIDDEN_CLEAR, 0);
    }
    let v = m.call(Call::WorldStep, dt);
    m.call(Call::AfterStep, dt);
    v
}

/// The copy's words: the rider's own record word (`+0xb0c`), taken from the record's `+0x214`, or
/// with no record from `+0xa74`; three quadwords from the record's `+0x220` (`+0x9e0..+0xa10`) when
/// `+0xa70` is 1; the scene node the positions come from without a wrecked record (`+0xb14`); the
/// mask (`+0x2f4`) each of the three copies tests against its own (`+0x9d4`, `+0x904`, `+0xa94`).
pub const OWN_WORD: u32 = 0xb0c;
pub const SPOT_NODE: u32 = 0xb14;
pub const COPY_MASK: u32 = 0x2f4;

/// **The copy after the frame** (the listener step's second call). With a record its `+0x214` to
/// the rider's `+0xb0c`; without, and with the mask's `+0x9d4` bits, `+0xa74` to `+0xb0c`, and with
/// `+0xa70` 1 the three quadwords at the record's `+0x220` to `+0x9e0` (the record pointer read
/// each time; it is null on this arm, so they come from low memory, as in the original). Then
/// twice, with the mask's `+0x904` and its `+0xa94` bits: the record's `+0x220` (with a record
/// whose wreck kind is set) or the spot node's position ([`Call::NodePosition`]) to `+0x910` and to
/// `+0xaa0`; after the second, the record's `+0xa0` to `+0xab0`. Quadwords move as `lq`/`sq`.
pub fn copy_after(m: &mut impl Rider, rider: u32) {
    let rec = m.word(rider + RECORD);
    if rec != 0 {
        let w = m.word(rec + 0x214);
        m.set(rider + OWN_WORD, w);
    } else if m.word(rider + COPY_MASK) & m.word(rider + 0x9d4) != 0 {
        let w = m.word(rider + 0xa74);
        m.set(rider + OWN_WORD, w);
        if m.word(rider + 0xa70) == 1 {
            for k in [0, 0x10, 0x20] {
                let rec = m.word(rider + RECORD);
                copy_words(m, rider + 0x9e0 + k, rec.wrapping_add(0x220 + k));
            }
        }
    }
    for (bits, to) in [(0x904, 0x910), (0xa94, 0xaa0)] {
        if m.word(rider + COPY_MASK) & m.word(rider + bits) == 0 {
            continue;
        }
        let rec = m.word(rider + RECORD);
        if rec != 0 && m.word(rec + R_KIND) != 0 {
            copy_words(m, rider + to, rec + 0x220);
        } else {
            let node = m.word(rider + SPOT_NODE);
            m.call(Call::NodePosition { node, out: rider + to }, F::ZERO);
        }
        if to == 0xaa0 {
            let rec = m.word(rider + RECORD);
            copy_words(m, rider + 0xab0, rec.wrapping_add(0xa0));
        }
    }
}

/// The extra step's words: its switch (`+0xd34`), its value and clock (`+0xd2c`, `+0xd30`) and the
/// value's ceiling (`+0xd38`); the table it reads, 32 bytes an entry at `+0xb20` indexed by the
/// record's `+0x1e8` (the time `+0`, the step `+4`, down or up `+8`); the record's speed (`+0x1a8`).
pub const EXTRA_ON: u32 = 0xd34;
pub const EXTRA_VALUE: u32 = 0xd2c;
pub const EXTRA_CLOCK: u32 = 0xd30;
pub const EXTRA_MAX: u32 = 0xd38;
pub const EXTRA_TABLE: u32 = 0xb20;

/// The extra step's table entry for the record's `+0x1e8`.
fn extra_entry(m: &mut impl Ram, rider: u32) -> u32 {
    let rec = m.word(rider + RECORD);
    rider.wrapping_add(m.word(rec + 0x1e8) << 5) + EXTRA_TABLE
}

/// **The extra step** (the listener step's last call, while `+0xd34` is set; its answer, whether
/// the value moved, is dropped). With the entry's down word set: a time of 0 or less zeroes the
/// value and the clock; else above 5 speed the clock runs on by `dt` and, once past the entry's
/// time, the value goes down by the step (clamped to 0..`+0xd38`) and the clock to 0. With it clear:
/// the same above 10 speed, the value going up, and no zero-time arm.
pub fn extra_step(m: &mut impl Ram, rider: u32, dt: F) -> u32 {
    if m.word(rider + EXTRA_ON) == 0 {
        return 0;
    }
    let e = extra_entry(m, rider);
    let rec = m.word(rider + RECORD);
    let down = m.word(e + 8) != 0;
    if down && op(m, e) <= 0.0 {
        m.set(rider + EXTRA_CLOCK, 0);
        m.set(rider + EXTRA_VALUE, 0);
        return 1;
    }
    let over = if down { 5.0 } else { 10.0 };
    if op(m, rec + 0x1a8) <= over {
        return 0;
    }
    let c = fpu::add(fw(m, rider + EXTRA_CLOCK), dt.0);
    put_f(m, rider + EXTRA_CLOCK, c);
    let e = extra_entry(m, rider);
    if op(m, rider + EXTRA_CLOCK) <= op(m, e) {
        return 0;
    }
    let (step, v) = (fw(m, e + 4), fw(m, rider + EXTRA_VALUE));
    let v = if down { fpu::sub(v, step) } else { fpu::add(v, step) };
    let v = fpu::min(fpu::max(v, 0.0), fw(m, rider + EXTRA_MAX));
    put_f(m, rider + EXTRA_VALUE, v);
    m.set(rider + EXTRA_CLOCK, 0);
    1
}

/// **The rider's listener step**: [`frame`], [`wreck_spots`], [`copy_after`], and with `+0xd34`
/// set [`extra_step`]; its answer is the frame's.
pub fn listener_step(m: &mut impl Rider, rider: u32, dt: F, map: Option<&track::Terrain<'_>>) -> u32 {
    let v = frame(m, rider, dt, map);
    wreck_spots(m, rider);
    copy_after(m, rider);
    if m.word(rider + EXTRA_ON) != 0 {
        extra_step(m, rider, dt);
    }
    v
}

// --- F62: the ragdoll world's step, with its own steps around it ------------------------------------

use super::anim;
use super::tail::{self, Listeners};
use ps2_float::vu;

/// The world step's words: the latch (`+0x254`, set by the riding reset), the respawn ask
/// (`+0x258`), the ragdoll's switch (`+0x260`; with [`LEFT_AS_IS`] set and neither of the other two,
/// the ragdoll world steps and the nodes follow it); the parts' placing ask (`+0x390`, set while
/// the nodes follow the record).
pub const LATCH: u32 = 0x254;
pub const RESPAWN_ASK: u32 = 0x258;
pub const RAG_ON: u32 = 0x260;
pub const PLACE_PARTS: u32 = 0x390;
/// The rider is an animation controller: its second flag byte (`+0x2d`), [`NODES_PLACED`] and
/// [`NODES_TURNED`] set while the animation moves the nodes; the toggle (`+0x58`); its entry list
/// (`+0x228`: the array, the room `+4`, the count `+0xc`, 16 bytes an entry).
pub const CTRL_FLAGS: u32 = 0x2d;
pub const NODES_PLACED: u32 = 0x10;
pub const NODES_TURNED: u32 = 0x20;
pub const TOGGLE: u32 = 0x58;
pub const ENTRIES: u32 = 0x228;
/// A quadword's four words as they are.
fn words(m: &mut impl Ram, at: u32) -> [u32; 4] {
    [0, 4, 8, 12].map(|k| m.word((at & !0xf) + k))
}

/// A byte stored as `sb` stores it.
fn put_byte(m: &mut impl Ram, at: u32, b: u32) {
    let (w, s) = (at & !3, 8 * (at & 3));
    let old = m.word(w);
    m.set(w, (old & !(0xff << s)) | ((b & 0xff) << s));
}

/// The holder `i` (`0xb0` bytes each from [`MAIN`]).
fn holder(m: &mut impl Ram, rider: u32, i: u32) -> u32 {
    m.word(rider + MAIN).wrapping_add(0xb0u32.wrapping_mul(i))
}

/// **The rider's world step** ([`Call::WorldStep`], F62). With the game paused (the outside world's
/// answer, `paused`) only the base step ([`base_step`]). Else, in order:
///
/// 1. the drawn frames kept ([`keep_drawn`], the rider's vt `+0xdc`);
/// 2. the animation's node bits ([`NODES_PLACED`], [`NODES_TURNED`]) cleared while the ragdoll has
///    the nodes ([`LEFT_AS_IS`] and [`RAG_ON`] set, [`LATCH`] and [`RESPAWN_ASK`] clear), else set,
///    each bit its own store;
/// 3. with the latch, every holder's node but the first put at its holder's `+0x50`
///    ([`node::set_position`], the count re-read after each);
/// 4. the idle layer ([`Call::Idle`], vt `+0xd4`);
/// 5. while the ragdoll has the nodes but for the latch: the ragdoll world ([`Call::RagWorld`]),
///    then the nodes put on it ([`aim_nodes`], vt `+0xd8`);
/// 6. with the respawn ask: the ragdoll posed still and settled ([`Call::PoseStill`],
///    [`Call::Settle`], vt `+0xd0` and `+0xcc`), and the ask cleared;
/// 7. the scene node's bounds ([`Call::Bounds`]) and the base step.
pub fn world_step(m: &mut impl Rider, rider: u32, dt: F, paused: bool) {
    if paused {
        base_step(m, rider, dt);
        return;
    }
    keep_drawn(m, rider);
    let off = m.word(rider + LEFT_AS_IS) != 0 && m.word(rider + RAG_ON) != 0 && m.word(rider + LATCH) == 0 && m.word(rider + RESPAWN_ASK) == 0;
    let set = if off { 0 } else { 0xff };
    for bit in [NODES_PLACED, NODES_TURNED] {
        let b = byte(m, rider + CTRL_FLAGS);
        put_byte(m, rider + CTRL_FLAGS, b & !bit | bit & set);
    }
    if m.word(rider + LATCH) != 0 {
        let mut i = 1u32;
        while (i as i32) < m.word(rider + BODIES) as i32 {
            let h = holder(m, rider, i);
            let (n, p) = (m.word(h + H_NODE), words(m, h + 0x50));
            node::set_position(m, n, p);
            i += 1;
        }
    }
    m.call(Call::Idle, dt);
    if m.word(rider + LEFT_AS_IS) != 0 && m.word(rider + RAG_ON) != 0 && m.word(rider + RESPAWN_ASK) == 0 {
        m.call(Call::RagWorld, dt);
        aim_nodes(m, rider);
    }
    if m.word(rider + RESPAWN_ASK) != 0 {
        m.call(Call::PoseStill, dt);
        m.call(Call::Settle, dt);
        m.set(rider + RESPAWN_ASK, 0);
    }
    let node = m.word(rider + NODE);
    m.call(Call::Bounds { node }, dt);
    base_step(m, rider, dt);
}

/// **The drawn frames kept** (the rider's vt `+0xdc`): each holder's node's world frame (its four
/// rows, `+0x70`) to the holder's drawn frame ([`H_DRAWN`]) as `lq`/`sq` move them, the count re-read
/// after each.
pub fn keep_drawn(m: &mut impl Ram, rider: u32) {
    let mut i = 0u32;
    while (i as i32) < m.word(rider + BODIES) as i32 {
        let h = holder(m, rider, i);
        let from = m.word(h + H_NODE) + 0x70;
        for r in [0, 0x10, 0x20, 0x30] {
            copy_words(m, h + H_DRAWN + r, from + r);
        }
        i += 1;
    }
}

/// A quaternion made unit as the node aims make it: `s = q·q` over the four lanes, its sum
/// `((w + x) + y) + z` through the accumulator, then `q · (1 / √sum)` (`vrsqrt`).
fn unit_quat(q: Vf) -> Vf {
    let s = q.mul(q);
    let sum = vu::madd(vu::madd(vu::add(s.w(), s.x()), 1.0, s.y()), 1.0, s.z());
    q.mul_q(vu::rsqrt(1.0, sum))
}

/// Holder `i`'s body's orientation made unit ([`unit_quat`]) and stored back; its words.
fn body_quat(m: &mut impl Ram, rider: u32, i: u32) -> (u32, [u32; 4]) {
    let b = holder_body(m, rider, i);
    let q = unit_quat(quad(m, b + B_QUAT));
    put_quad(m, b + B_QUAT, q);
    (b, words(m, b + B_QUAT))
}

/// Holder `i`'s node put on its body ([`node::orient`]): turned to the body's orientation (made unit
/// first, [`body_quat`]) at the body's position less its centre of gravity's offset turned by it
/// ([`turned`], `xyz`; the `w` the position's word).
fn node_on_body(m: &mut impl Ram, rider: u32, i: u32) {
    let (b, q) = body_quat(m, rider, i);
    let t = turned(quad(m, b + B_QUAT), quad(m, b + B_COFG));
    let at = quad(m, b + B_POSITION);
    let [x, y, z, _] = at.sub(t).to_bits();
    let pos = [x, y, z, m.word(b + B_POSITION + 12)];
    let h = holder(m, rider, i);
    let n = m.word(h + H_NODE);
    node::orient(m, n, q, pos);
}

/// **The nodes put where the ragdoll is** (the rider's vt `+0xd8`). The scene node: wrecked (mode
/// 2) with no respawn ask, turned to the first body's orientation at its position
/// ([`node::orient`], the words as they are); else at the record's `+0x1b0` ([`node::set`]) and
/// turned to its `+0x1c0` (`anim::turn_node`). Then, not wrecked: every holder's node on its body
/// ([`node_on_body`], the count re-read after each) and [`PLACE_PARTS`] 1. Wrecked: with
/// [`PLACE_PARTS`], the parts placed ([`place_parts`]) and it cleared; the first holder's node on its
/// body; every other's turned to its body's orientation (made unit) where it is
/// ([`node::orient_here`]).
pub fn aim_nodes(m: &mut impl Ram, rider: u32) {
    let rec = m.word(rider + RECORD);
    let node = m.word(rider + NODE);
    if m.word(rider + MODE) == 2 && m.word(rec + R_RESPAWN) == 0 {
        let b = holder_body(m, rider, 0);
        let (q, at) = (words(m, b + B_QUAT), words(m, b + B_POSITION));
        node::orient(m, node, q, at);
    } else {
        let at = words(m, rec + 0x1b0);
        node::set(m, node, at);
        let q = words(m, rec + 0x1c0);
        anim::turn_node(m, node, q);
    }
    if m.word(rider + MODE) != 2 {
        let mut i = 0u32;
        while (i as i32) < m.word(rider + BODIES) as i32 {
            node_on_body(m, rider, i);
            i += 1;
        }
        m.set(rider + PLACE_PARTS, 1);
        return;
    }
    if m.word(rider + PLACE_PARTS) != 0 {
        place_parts(m, rider);
        m.set(rider + PLACE_PARTS, 0);
    }
    node_on_body(m, rider, 0);
    let mut i = 1u32;
    while (i as i32) < m.word(rider + BODIES) as i32 {
        let (_, q) = body_quat(m, rider, i);
        let h = holder(m, rider, i);
        let n = m.word(h + H_NODE);
        node::orient_here(m, n, q);
        i += 1;
    }
}

/// The controller's entry `i` (16 bytes each from the list's array): the list grown to `2(i + 1)`
/// first when `i` is past its room ([`Call::EntriesGrow`]), and its count raised to `i + 1`.
fn entry(m: &mut impl Rider, rider: u32, i: u32, dt: F) -> u32 {
    let list = rider + ENTRIES;
    if (i as i32) >= m.word(list + 4) as i32 {
        m.call(Call::EntriesGrow { list, count: (i + 1) << 1 }, dt);
    }
    if (m.word(list + 0xc) as i32) < (i + 1) as i32 {
        m.set(list + 0xc, i + 1);
    }
    m.word(list).wrapping_add(i << 4)
}

/// The rider's own listener group as [`tail::walk`] reads it: each listener's step is the outside
/// world's ([`Call::Listener`]).
struct Group<'a, M>(&'a mut M);

impl<M: Rider> Listeners for Group<'_, M> {
    fn word(&mut self, at: u32) -> u32 {
        self.0.word(at)
    }
    fn step(&mut self, listener: u32, dt: F) {
        self.0.call(Call::Listener { listener }, dt);
    }
}

/// **The controller's base step**: the toggle ([`TOGGLE`]) flipped to whether it was 0; each entry
/// (the count re-read after each) whose first word is set and whose `+8` is not bound
/// ([`Call::EntryBind`]); then the walk of the rider's own listeners ([`tail::walk`]).
pub fn base_step(m: &mut impl Rider, rider: u32, dt: F) {
    let t = m.word(rider + TOGGLE);
    m.set(rider + TOGGLE, u32::from(t == 0));
    let mut i = 0u32;
    while (i as i32) < m.word(rider + ENTRIES + 0xc) as i32 {
        let e = entry(m, rider, i, dt);
        if m.word(e) != 0 {
            let e = entry(m, rider, i, dt);
            if m.word(e + 8) == 0 {
                m.call(Call::EntryBind { index: i }, dt);
            }
        }
        i += 1;
    }
    tail::walk(&mut Group(m), rider, dt);
}

// --- F62a: the idle layer ---------------------------------------------------------------------------

/// The idle layer's words. The state asked for and the one acted on (`+0x86e`, `+0x86d`, a byte
/// each; the states [`STILL`] .. [`PAUSE`] and none, 255); the record's `+0x1d8` kept after
/// (`+0x870`); the named clip set picked (`+0x874`, negative for none) and the sets' holder
/// (`+0x878`, its list at `+8`); the clips by id (`+0xb18`, a word an id).
pub const IDLE_WANT: u32 = 0x86e;
pub const IDLE_STATE: u32 = 0x86d;
pub const IDLE_KEPT: u32 = 0x870;
pub const CLIP_SET: u32 = 0x874;
pub const CLIP_SETS: u32 = 0x878;
pub const CLIPS_BY_ID: u32 = 0xb18;
/// The speeds the states read: under `+0x890` the rider may pause, and the three bands (`+0x894`,
/// `+0x898`, `+0x89c`) the still state's clips are picked by. The pause: asked (`+0x8ac`, which
/// [`choose`] reads too), its clock (`+0x8b0`) drawn between `+0x8b4` and `+0x8b8`, and its end
/// (`+0x24c`). The lap of the slow states' clip pair (`+0x8bc`).
pub const PAUSE_SPEED: u32 = 0x890;
pub const BANDS: [u32; 3] = [0x894, 0x898, 0x89c];
pub const PAUSE_ASK: u32 = 0x8ac;
pub const PAUSE_CLOCK: u32 = 0x8b0;
pub const PAUSE_RANGE: [u32; 2] = [0x8b4, 0x8b8];
pub const PAUSE_END: u32 = 0x24c;
pub const LAP: u32 = 0x8bc;
/// The two blend slots (`+0x5c0`, `+0x5c4`: each points at its value, its time constant and its last
/// rate); the fade out of the stopped pose (`+0x610`) and the rate it falls at (`+0x614`), taken from
/// one of five (`+0x618` the pause's, `+0x61c` the wreck poses', `+0x620` the riding states', `+0x624`
/// the turn's, `+0x628` the record's own).
pub const SLOTS: [u32; 2] = [0x5c0, 0x5c4];
pub const IDLE_FADE: u32 = 0x610;
pub const IDLE_RATE: u32 = 0x614;
pub const RATES: [u32; 5] = [0x618, 0x61c, 0x620, 0x624, 0x628];
/// The states: still (or none), the two slow ones, the turn, the wreck poses (`@` then `A` .. `E`,
/// drawn by lot) and the pause. State bytes under [`OFF_STATES`] name a clip by id of their own.
pub const STILL: u32 = 0x38;
pub const SLOW_STATES: [u32; 2] = [0x3b, 0x3c];
pub const TURN: u32 = 0x3d;
pub const WRECK_POSE: u32 = 0x40;
pub const PAUSE: u32 = 0x46;
pub const OFF_STATES: u32 = 46;
/// The record's words the layer reads besides the choice's: the state held (`+0x10`, the state then
/// its `+0x210`, which the layer writes back), the layer left alone (`+0x14`), the offset the node is
/// put at (`+0x100`, a pointer), the turn asked (`+0x130`), the slots' targets (`+0x1a4`, `+0x1a0`;
/// the float maximum keeps the slot), the stick held (`+0x19c`), `+0x190`, the speed (`+0x1a8`), the
/// node's place and turn (`+0x1b0`, `+0x1c0`), a rate ask (`+0x1d0`), a state asked (`+0x1d4`, a byte,
/// 255 none) and the own clip's weight (`+0x1d8`).
const R_HELD: u32 = 0x10;
const R_LEFT: u32 = 0x14;
const R_STATE: u32 = 0x210;

fn fl(m: &mut impl Ram, at: u32) -> F {
    F::from_bits(m.word(at))
}

/// The `0.03` the slow states' lap runs at, a unit of speed a second.
const LAP_RATE: u32 = 0x3cf5_c28f;

/// The anim controller's saved buffers made through [`Call::SavedPose`].
struct Saving<'a, M> {
    m: &'a mut M,
    dt: F,
}

impl<M: Rider> Ram for Saving<'_, M> {
    fn word(&mut self, at: u32) -> u32 {
        self.m.word(at)
    }
    fn set(&mut self, at: u32, w: u32) {
        self.m.set(at, w)
    }
}

impl<M: Rider> anim::Anim for Saving<'_, M> {
    fn alloc(&mut self, bytes: u32, buffer: anim::Saved) -> u32 {
        self.m.call(Call::SavedPose { bytes, buffer }, self.dt)
    }
}

/// **The rider's idle layer** ([`Call::Idle`], its vt `+0xd4`, F62a): the state picked
/// ([`idle_state`], or with the record's [`R_HELD`] the record's [`R_STATE`]); the clips chosen
/// and played ([`idle_clips`]); the record's `+0x1d8` kept ([`IDLE_KEPT`]).
pub fn idle(m: &mut impl Rider, rider: u32, dt: F, t: &anim::Tables) {
    let rec = m.word(rider + RECORD);
    if m.word(rec + R_HELD) != 0 {
        let s = m.word(rec + R_STATE);
        put_byte(m, rider + IDLE_WANT, s);
    } else {
        idle_state(m, rider, dt);
        let rec = m.word(rider + RECORD);
        if !(fl(m, rec + 0x1a8) <= fl(m, rider + BANDS[0])) {
            m.set(rider + PAUSE_CLOCK, 0);
            m.set(rider + PAUSE_ASK, 0);
        }
    }
    idle_clips(m, rider, dt, t);
    let rec = m.word(rider + RECORD);
    let w = m.word(rec + 0x1d8);
    m.set(rider + IDLE_KEPT, w);
}

/// **The state picked** into [`IDLE_WANT`]: wrecked, [`WRECK_POSE`] until the wreck is [`SLOW`],
/// then once a pose by lot (`A` plus `5u`, [`Call::Seed`] first); else the record's state asked; the
/// turn asked, [`TURN`]; the stick held, a slow state by `+0x1a0` under a half; else, with the speed
/// under [`PAUSE_SPEED`], both slot targets a half and `+0x190` zero, the pause's clock — drawn by lot
/// between its range when run out or with the record's hold, else run down by `dt`, its end starting
/// the pause ([`PAUSE`]) — and then [`STILL`] unless a pause is asked (which [`PAUSE_END`] ends).
pub fn idle_state(m: &mut impl Rider, rider: u32, dt: F) {
    let rec = m.word(rider + RECORD);
    let want = rider + IDLE_WANT;
    if m.word(rec + R_KIND) != 0 {
        if m.word(rider + SLOW) == 0 {
            put_byte(m, want, WRECK_POSE);
        } else if byte(m, want) < WRECK_POSE + 1 {
            m.call(Call::Seed, dt);
            let u = unit(m, dt);
            let k = fpu::to_i32(fpu::mul(5.0, u));
            put_byte(m, want, (k + 65) as u32);
        }
        return;
    }
    let asked = byte(m, rec + 0x1d4);
    if asked != 0xff {
        put_byte(m, want, asked);
        return;
    }
    if m.word(rec + 0x130) != 0 {
        put_byte(m, want, TURN);
        return;
    }
    if fl(m, rec + 0x19c) != F::ZERO {
        let s = if fl(m, rec + 0x1a0) < F(0.5) { SLOW_STATES[0] } else { SLOW_STATES[1] };
        put_byte(m, want, s);
        return;
    }
    let half = F(0.5);
    if fl(m, rec + 0x1a8).abs() < fl(m, rider + PAUSE_SPEED) && fl(m, rec + 0x1a0) == half && fl(m, rec + 0x1a4) == half && fl(m, rec + 0x190) == F::ZERO {
        let clock = fl(m, rider + PAUSE_CLOCK);
        if clock <= F::ZERO || m.word(rec + R_HOLD) != 0 {
            let u = F(unit(m, dt));
            let (lo, hi) = (fl(m, rider + PAUSE_RANGE[0]), fl(m, rider + PAUSE_RANGE[1]));
            let x = (u - F::ZERO) / (F::ONE - F::ZERO);
            let c = Acc::adda(F::ZERO, lo).madd(x, hi - lo).max(lo.min(hi)).min(lo.max(hi));
            m.set(rider + PAUSE_CLOCK, c.to_bits());
        } else {
            let c = clock - dt;
            m.set(rider + PAUSE_CLOCK, c.to_bits());
            if c < F::ZERO {
                put_byte(m, want, PAUSE);
                m.set(rider + PAUSE_ASK, 1);
                m.set(rider + PAUSE_END, 0);
            }
        }
    }
    if m.word(rider + PAUSE_ASK) == 0 {
        put_byte(m, want, STILL);
    } else if m.word(rider + PAUSE_END) != 0 {
        m.set(rider + PAUSE_ASK, 0);
    }
}

/// The clip of id `id` (the rider's vt `+0x174`): the word `id` of [`CLIPS_BY_ID`]'s list.
pub fn clip_by_id(m: &mut impl Ram, rider: u32, id: u32) -> u32 {
    let list = m.word(rider + CLIPS_BY_ID);
    m.word(list.wrapping_add(id << 2))
}

/// The named clip set `i`: the word `i` of the list at the sets' holder's `+8`.
pub fn clip_set(m: &mut impl Ram, holder: u32, i: u32) -> u32 {
    let list = m.word(holder + 8);
    m.word(list.wrapping_add(i << 2))
}

/// A slot run towards `target`: its rate `min(dt, τ)/τ` kept, its value `v + rate·(target − v)`
/// through the accumulator.
fn run_slot(m: &mut impl Ram, rider: u32, slot: u32, target: u32, dt: F) -> F {
    let s = m.word(rider + slot);
    let tau = fl(m, s + 4);
    let rec = m.word(rider + RECORD);
    let to = fl(m, rec + target);
    let r = dt.min(tau) / tau;
    m.set(s + 8, r.to_bits());
    let v = fl(m, s);
    let v = Acc::adda(F::ZERO, v).madd(r, to - v);
    m.set(s, v.to_bits());
    v
}

/// Where `x` falls between `lo` and `hi`, from 1 down to 0 (1 when they are the same).
fn band(x: F, lo: F, hi: F) -> F {
    if lo == hi {
        return F::ONE;
    }
    let u = (x - lo) / (hi - lo);
    Acc::adda(F::ZERO, F::ONE).madd(u, F::ZERO - F::ONE).max(F::ONE.min(F::ZERO)).min(F::ONE.max(F::ZERO))
}

/// The two ids of a pair by the slot: the pair from `id` down under a half, from `id + 1` down at or
/// past it (and the slot's half then taken off); the mix twice the slot.
fn by_half(m: &mut impl Ram, rider: u32, ids: &[u32], s: F) -> ([u32; 4], F) {
    let up = !(s < F(0.5));
    let mut clips = [0; 4];
    for (k, &id) in ids.iter().enumerate() {
        clips[k] = clip_by_id(m, rider, if up { id + 1 } else { id });
    }
    let mix = if up { F(2.0) * (s - F(0.5)) } else { F(2.0) * s };
    (clips, mix)
}

/// **The clips chosen and played** (the idle layer's 3 KB callee). Nothing with the record's
/// [`R_LEFT`]. Else:
///
/// 1. unless the ragdoll has the nodes, the scene node put at the record's `+0x1b0` (plus its
///    `+0x100` offset turned by its `+0x1c0`, [`turned`], when not ridden) and turned to `+0x1c0`;
/// 2. a new state (or the record's `+0x1d0`): the rate picked from [`RATES`] (the lap restarted for a
///    slow state), the controller stopped ([`anim::stop`]) and the fade put at 1;
/// 3. the state acted on, and into the record's [`R_STATE`]; the fade run down by the rate; the
///    record's respawn ask zeroes both and puts the slots at their targets; the two slots run
///    ([`run_slot`]);
/// 4. the clips: a named set ([`CLIP_SET`]) by its words; else a state of its own clip by id, at the
///    record's `+0x1d8`; the slow states' pair at the lap (run on by `0.03·speed·dt`, wrapped by
///    `fmodf`); the turn's pair; the still state's by speed band (a pair, or two pairs mixed by
///    [`band`]); a wreck pose's clip started on its change ([`Call::ClipStart`]);
/// 5. the weights clamped to `0..1` (the fade stored so), then four clips [`anim::blend4`], two
///    [`anim::blend`], one [`anim::play`], none the controller's own advance ([`Call::ClipAdvance`]).
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn idle_clips(m: &mut impl Rider, rider: u32, dt: F, t: &anim::Tables) {
    let rec = m.word(rider + RECORD);
    if m.word(rec + R_LEFT) != 0 {
        return;
    }
    if m.word(rider + LEFT_AS_IS) == 0 || m.word(rider + RAG_ON) == 0 {
        let at = if m.word(rec + R_RIDDEN) == 0 {
            let q = words(m, rec + 0x1c0);
            let off_at = m.word(rec + 0x100);
            let off = words(m, off_at);
            let v = turned(Vf::from_bits(q), Vf::from_bits(off));
            let rec = m.word(rider + RECORD);
            let s = v.add(Vf::from_bits(words(m, rec + 0x1b0))).to_bits();
            [s[0], s[1], s[2], off[3]]
        } else {
            let rec = m.word(rider + RECORD);
            words(m, rec + 0x1b0)
        };
        let node = m.word(rider + NODE);
        node::set(m, node, at);
        let (node, rec) = (m.word(rider + NODE), m.word(rider + RECORD));
        let q = words(m, rec + 0x1c0);
        anim::turn_node(m, node, q);
    }
    let want = byte(m, rider + IDLE_WANT);
    let rec = m.word(rider + RECORD);
    let mut changed = false;
    if want != byte(m, rider + IDLE_STATE) || m.word(rec + 0x1d0) != 0 {
        changed = true;
        let rec = m.word(rider + RECORD);
        let rate = if byte(m, rec + 0x1d4) == 0xff && m.word(rec + 0x1d0) == 0 {
            match want {
                0x3b | 0x3c => {
                    let r = m.word(rider + RATES[2]);
                    m.set(rider + IDLE_RATE, r);
                    m.set(rider + LAP, 0);
                    None
                }
                TURN => Some(RATES[3]),
                0x40..=0x45 => Some(RATES[1]),
                PAUSE => Some(RATES[0]),
                _ => Some(RATES[2]),
            }
        } else {
            Some(RATES[4])
        };
        if let Some(at) = rate {
            let r = m.word(rider + at);
            m.set(rider + IDLE_RATE, r);
        }
    }
    if changed {
        anim::stop(&mut Saving { m: &mut *m, dt }, rider);
        m.set(rider + IDLE_FADE, F::ONE.to_bits());
    }
    let s = byte(m, rider + IDLE_WANT);
    put_byte(m, rider + IDLE_STATE, s);
    let s = byte(m, rider + IDLE_WANT);
    let rec = m.word(rider + RECORD);
    m.set(rec + R_STATE, s);
    let fade = Acc::adda(F::ZERO, fl(m, rider + IDLE_FADE)).msub(fl(m, rider + IDLE_RATE), dt);
    m.set(rider + IDLE_FADE, fade.to_bits());
    let rec = m.word(rider + RECORD);
    if m.word(rec + R_RESPAWN) != 0 {
        m.set(rider + IDLE_RATE, 0);
        m.set(rider + IDLE_FADE, 0);
        for (slot, target) in [(SLOTS[0], 0x1a4), (SLOTS[1], 0x1a0)] {
            let rec = m.word(rider + RECORD);
            let v = fl(m, rec + target);
            if v != F::MAX {
                let s = m.word(rider + slot);
                m.set(s, v.to_bits());
            }
        }
    }
    let s0 = run_slot(m, rider, SLOTS[0], 0x1a4, dt);
    let s1 = run_slot(m, rider, SLOTS[1], 0x1a0, dt);
    let (mut weight, mut mix, mut mix2) = (F::ZERO, F::ZERO, F::ZERO);
    let (mut n, mut clips) = (0, [0u32; 4]);
    let set = m.word(rider + CLIP_SET);
    if (set as i32) >= 0 {
        let rec = m.word(rider + RECORD);
        let held = fpu::to_i32(fl(m, rec + 0x19c).0);
        let e = clip_set(m, rider + CLIP_SETS, set);
        let (own, a, b) = (m.word(e + 0xc), m.word(e + 4), m.word(e + 8));
        (n, clips, weight, mix) = if own != 0 && held != 0 {
            (1, [own, 0, 0, 0], F::ONE, mix)
        } else if a != 0 && b != 0 {
            (2, [a, b, 0, 0], s1, s0)
        } else if a != 0 {
            (1, [a, 0, 0, 0], s1, mix)
        } else if b != 0 {
            (1, [b, 0, 0, 0], s0, mix)
        } else {
            (0, clips, weight, mix)
        };
    } else {
        let state = byte(m, rider + IDLE_STATE);
        let own = if state < OFF_STATES { clip_by_id(m, rider, state) } else { 0 };
        if own != 0 {
            let rec = m.word(rider + RECORD);
            (n, clips[0], weight) = (1, own, fl(m, rec + 0x1d8));
        } else {
            match state {
                0x3b | 0x3c => {
                    let rec = m.word(rider + RECORD);
                    let speed = fl(m, rec + 0x1a8);
                    if !(speed <= F::ZERO) {
                        let step = F::from_bits(LAP_RATE) * speed;
                        let lap = Acc::adda(F::ZERO, fl(m, rider + LAP)).madd(dt, step);
                        m.set(rider + LAP, lap.to_bits());
                        let w = ps2_float::libm::fmodf(lap.0, 1.0);
                        m.set(rider + LAP, w.to_bits());
                    }
                    weight = fl(m, rider + LAP);
                    clips[0] = clip_by_id(m, rider, 0x3c);
                    clips[1] = clip_by_id(m, rider, 0x3b);
                    (mix, n) = (s1, 2);
                }
                TURN => {
                    weight = s1;
                    (clips, mix) = by_half(m, rider, &[0x3e, 0x3d], s0);
                    n = 2;
                }
                0xff | STILL => {
                    let rec = m.word(rider + RECORD);
                    let speed = fl(m, rec + 0x1a8);
                    let [b0, b1, b2] = BANDS.map(|at| F::from_bits(m.word(rider + at)));
                    weight = s1;
                    if speed < b0 {
                        (clips, mix) = by_half(m, rider, &[0x33, 0x32, 0x36, 0x35], s0);
                        let (rec, hi) = (m.word(rider + RECORD), fl(m, rider + BANDS[0]));
                        mix2 = band(fl(m, rec + 0x1a8), F::ZERO, hi);
                        n = 4;
                    } else if speed <= b1 {
                        (clips, mix) = by_half(m, rider, &[0x36, 0x35], s0);
                        n = 2;
                    } else if speed <= b2 {
                        (clips, mix) = by_half(m, rider, &[0x36, 0x35, 0x39, 0x38], s0);
                        let (rec, lo, hi) = (m.word(rider + RECORD), fl(m, rider + BANDS[1]), fl(m, rider + BANDS[2]));
                        mix2 = band(fl(m, rec + 0x1a8), lo, hi);
                        n = 4;
                    } else {
                        (clips, mix) = by_half(m, rider, &[0x39, 0x38], s0);
                        n = 2;
                    }
                }
                0x40..=0x46 => {
                    if changed {
                        let clip = clip_by_id(m, rider, state);
                        m.call(Call::ClipStart { clip }, dt);
                    }
                }
                _ => {}
            }
        }
    }
    let unit = |x: F| x.max(F::ZERO).min(F::ONE);
    let (weight, mix, mix2) = (unit(weight), unit(mix), unit(mix2));
    let fade = unit(fl(m, rider + IDLE_FADE));
    m.set(rider + IDLE_FADE, fade.to_bits());
    match n {
        4 => anim::blend4(m, t, rider, clips, weight, mix, mix2, fade),
        2 => anim::blend(m, t, rider, [clips[0], clips[1]], weight, mix, fade),
        1 => anim::play(m, t, rider, clips[0], weight, fade),
        _ => {
            m.call(Call::ClipAdvance, dt);
        }
    }
}
