//! **SLUS-21104's bike vehicle update's own callees** (F39): the slots and calls the vehicle update
//! ([`super::tail::vehicle_update`]) makes on the bike every frame, ported so that the frame's tail
//! runs in Rust down to the scene graph and the animation. Where the original keeps them and their
//! calls: research/oracle.txt `# F39`.
//!
//! - [`fade`] ([`super::tail::Out::Fade`]): with the fade clock running, the collision record's
//!   search (its "searching" word off and on around it); the clock down by `dt` unless the record's
//!   part is held; at its end the bike's kind cleared, else the alpha from the clock's start onto the
//!   rider's model and each hung part. The kind clear at its end is [`kind_clear`] (F31b5l), with
//!   the bike's show ([`show`]); the search is SLUS-21104's world probe
//!   ([`crate::atv_search::collide_world`]).
//! - [`lean`] ([`super::tail::Out::Lean`]): the stunt's clock ([`stunt_clock`]), then the next stunt:
//!   none while down, else the trick step's answer, the riding pose on the ground or none; a put-back
//!   centres the two blends.
//! - [`prepare`] ([`super::tail::Out::Prepare`]): the held states' net unpack, the fade's rate, the
//!   rider's animation (a stunt's clip, the riding pose's two-clip blend, or the half pose), the
//!   rider's nodes (the seat placed, the bars and the head turned, the aim at the look point), the
//!   hung parts set from the two bone frames, and the attachment moved.
//! - [`ride`] ([`super::tail::Out::Ride`]): the auto-gear clock: past a gear's wait at speed, the
//!   gear up (or down) by its step, clamped.
//! - [`sync`] ([`super::tail::Out::Sync`]) and [`snapshot`] ([`super::tail::Out::Snapshot`]): the
//!   records other code reads the bike's frame from — the camera's, the trail's, the replay's.
//! - [`network`] ([`super::tail::Out::Network`]): the bike's net sender's clock, its timers due this
//!   frame, and the send.
//! - [`parts`] ([`super::tail::Out::Parts`]): the collision record's and the second part's steps, then
//!   the bike's own listeners' walk.
//! - The virtuals those three call out on state 4's objects (F44): the trail's on and off
//!   ([`trail_on`], [`trail_off`]) and the scene node's walk of its children ([`shown`], [`hidden`]);
//!   the net sender's send, the bike's ([`send`]); a part's step ([`part_step`]).
//! - [`stunt_start`] (vt `+0x4b8`) and [`trick_end`] (vt `+0x4ac`), which the bike's steps, the
//!   tail and the trick step call (F31b5g): the next stunt and the clip's clocks, and the player's
//!   run told ([`Out::RunScore`] and the rest).
//!
//! The quad's twin of [`prepare`]'s animation half is [`crate::quad::animate`]; this one reads and
//! writes the bike's words as the original does, in its order.
//!
//! Every word goes through [`Updates`] as it is read or written, and every call out is an [`Out`].
//! A vector handed in by pointer is in the [`Out`] as its four words; a vector a call writes back is
//! asked through [`Updates::vector`].
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::air::normalize;
use ps2_float::{F, Vf, fpu, fpu::Acc};

/// Four raw words: a quadword as RAM holds it.
pub type Quad = [u32; 4];

/// The quadword (0, 0, 0, 1) the snapshot and the net unpack reset to, the turn axis the bars and the
/// head spin about, and the up the rider's aim keeps: rodata.
pub const ORIGIN: u32 = 0x0039_6fc0;
pub const TURN_AXIS: u32 = 0x0039_6ff0;
pub const AIM_UP: u32 = 0x0039_7000;
/// The speed at or under which the stunt clock's pose stays on (the riding pose's own).
pub const POSE_SPEED: u32 = 0x0052_5378;

/// The stunt ids: the riding pose (46); a stunt table holds 46; the replay keeps ids under 41.
pub const RIDING_POSE: u32 = 46;
pub const KEPT_STUNTS: i32 = 41;
/// The bike's words, as offsets.
pub const STATE: u32 = 0xbc4;
pub const BODY: u32 = 0xb8;
pub const RECORD: u32 = 0xcc;
pub const FADE: u32 = 0xde8;
pub const FADE_START: u32 = 0xdec;
pub const STUNT: u32 = 0x1524;
pub const NEXT_STUNT: u32 = 0x1528;
pub const NET: u32 = 0x400;

/// A call the update's callees make outside the bike's words, with the arguments that vary. Floats
/// are their bits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Out {
    /// The fade's: the collision record's search (the record, 0); the bike's kind cleared (vt
    /// `+0x39c`).
    Search(u32),
    KindClear,
    /// The ride's gear (vt `+0x358`, an answer: the gear table's index).
    Gear,
    /// The lean's: the trick step (vt `+0x4e4`, `dt`; an answer, 0 for "no trick", ported as
    /// [`trick`]), a trick's end (vt `+0x4ac`, the stunt's id: −1 from the stunt clock, a clip's own
    /// from the trick step).
    Trick,
    TrickEnd(u32),
    /// The prepare's net check (vt `+0x3e8`, an answer; [`net_check`]) and unpack (vt `+0x3f0`, `dt`;
    /// [`net_unpack`]), in the held vehicle states.
    NetCheck,
    NetUnpack,
    /// The trick step's: the remote bike's own (vt `+0x4e8`, `dt`; an answer, which the step gives
    /// back); a player's stunt mark set or cleared (the run `+0x684`, the player `+0x680`, on, the
    /// clip's id); the clip held at its end (vt `+0x4ec`, the stunt's table entry, `dt`); whether a
    /// trick is asked for (vt `+0x4b4`, an answer); a trick started (vt `+0x4b8`, its id, 0).
    TrickRemote,
    Mark { run: u32, player: u32, on: u32, id: u32 },
    TrickHold(u32),
    TrickWanted,
    TrickStart(u32),
    /// The net check's: the bike put between two network snapshots (vt `+0x3ec`, the older and the
    /// newer snapshot, `t` how far from the older; `dt` as it came).
    NetInterpolate { older: u32, newer: u32, t: u32 },
    /// The net unpack's: a node turned to the quaternion at `at` and placed at `pos` (the node, 0,
    /// `at`, `pos`); a wheel's contacts cleared; the bike's wheel contacts (`dt`); a wheel's suspension
    /// and its tire forces (`dt`); the vehicle's end of the unpack (`dt`).
    Orient { node: u32, at: u32, pos: u32 },
    WheelClear(u32),
    Contacts,
    Suspension(u32),
    TireForces(u32),
    NetPost,
    /// The rider's animation controller: stopped; one clip played; two blended.
    AnimStop(u32),
    AnimPlay { ctrl: u32, clip: u32, weight: u32, fade: u32 },
    AnimBlend { ctrl: u32, clips: [u32; 2], weight: u32, mix: u32, fade: u32 },
    /// A node's point placed in its parent's frame (asked through [`Updates::vector`]); a node's
    /// position set; a node spun about an axis in RAM by an angle; a node aimed along `dir` with the
    /// up at `up` (and 1, 1, 1).
    NodePlace { node: u32, at: Quad },
    NodeSet { node: u32, at: Quad },
    Spin { node: u32, axis: u32, angle: u32 },
    Aim { node: u32, dir: Quad, up: u32 },
    /// A node's frame (the node, 0, out) into one of the prepare's two buffers, a hung part's frame
    /// set from one; and into the bike's words at `at`.
    Pose { node: u32, buf: u32 },
    Place { part: u32, buf: u32 },
    Frame { node: u32, at: u32 },
    /// A node's position (the node, 0, out) into the bike's words at `at`.
    Position { node: u32, at: u32 },
    /// The rider's attachment moved in the spatial database.
    Moved(u32),
    /// A node's axes (the node, 0, z out, y out) into the bike's words; a point in RAM into a node's
    /// frame, and a vector rotated by one (both asked through [`Updates::vector`]).
    Axes { node: u32, z: u32, y: u32 },
    Local { node: u32, at: u32 },
    Rotate { node: u32, v: Quad },
    /// The trail's on and off (its vt `+0x18` / `+0x20`, on its vt at `+4`; the scene node's own are
    /// [`trail_on`] and [`trail_off`]), and a node's shown and hidden (vt `+0x28` / `+0x2c`: the scene
    /// node's own are [`shown`] and [`hidden`]).
    /// The kind clear's: the bike shown (vt `+0x398`, the bike, 1; ported as [`show`]); the bike's
    /// shown object (`+0xd0`) switched on (its vt at `+0x28`, slot `+0x20`, on its `+0x24`); and that
    /// object put back in the spatial database (it, the bike's `+0xb4` and `+0xc0`, 1).
    Show,
    ObjectOn(u32),
    Insert { obj: u32, space: u32, at: u32 },
    TrailOn(u32),
    TrailOff(u32),
    Shown(u32),
    Hidden(u32),
    /// The net sender's send (its vt `+0x20`, the vt at `+0x38`: the sender, its `+0x34`; `dt`; the
    /// bike's own is [`send`]), and the bike's three packets it makes (the sender's `+0x34`, `dt`):
    /// the stunt's (vt `+0x544`), the whole state (vt `+0x400`) and the change since (vt `+0x404`).
    Send { net: u32, arg: u32 },
    NetStunt(u32),
    NetFull(u32),
    NetDelta(u32),
    /// A part's step (its vt `+0xbc`, the vt at `+0xc`; `dt`; the record's and the second part's
    /// own is [`part_step`]) and the listener walk of a group
    /// ([`super::tail::walk`]; `dt`).
    Part(u32),
    Walk(u32),
    /// The packets' (F44a; [`net_stunt`], [`net_full`], [`net_delta`]): the net sender's own send
    /// (its vt `+0x28`: the sender, the sender's `+0x34`, the packet, its length, 0, 0; the bike's
    /// sender's is [`sender_send`]); the connection's queue it ends in (the connection, the two
    /// header bytes, the packet, the flags); the quantiser table's hash of a key (its `+0x10`, an
    /// answer; state 4's is [`decimal_hash`]) and its test of two keys (its `+0x14`, an answer;
    /// [`same_key`]); and the stunt's stamp: the interrupts held off, the hardware's RCNT0 count read
    /// (an answer), the interrupts on again.
    SenderSend { sender: u32, arg: u32, packet: Packet },
    NetQueue { conn: u32, head: [u8; 2], packet: Packet, flags: u32 },
    /// A full queue out the connection's socket ([`net_queue`]; the socket, [`FLUSH_BUFFER`]'s
    /// length, reliable on the second pair's): the answer negative when it failed.
    NetFlush { socket: u32, len: u32, reliable: bool },
    Hash { f: u32, key: u32 },
    SameKey { f: u32, a: u32, b: u32 },
    IrqOff,
    Counter,
    IrqOn,
    /// The player's run's calls (F31b5g; [`stunt_start`], [`trick_end`]), each with the run
    /// (`+0x684`) and the player (`+0x680`): the run opened (the run only) when the player's `+8`
    /// bit 5 is clear; a stunt scored (the id, 12, `$f12` and `$f14` the entry's two scores, `$f13` 0,
    /// `$f15` and `$f16` 1.0; the stunt's mark then [`Out::Mark`]'s, on the start's third argument); and
    /// at a trick's end, the run closed (1.0, 1), then tallied (the player's `+0xcc` as a float, 1), or one
    /// stunt ended (1.0, 1, the id, 0).
    RunOpen(u32),
    RunScore { run: u32, player: u32, id: u32, scores: [u32; 2] },
    RunClose { run: u32, player: u32 },
    RunTally { run: u32, player: u32, count: u32 },
    RunEnd { run: u32, player: u32, id: u32 },
}

/// The longest packet [`Packet`] holds: the change's is at most 43 bytes, the whole state's 41.
pub const PACKET: usize = 64;

/// A net packet as the original builds it on its stack: its first `len` bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Packet {
    pub len: u32,
    pub bytes: [u8; PACKET],
}

impl Packet {
    pub const EMPTY: Packet = Packet { len: 0, bytes: [0; PACKET] };

    /// The packet's bytes.
    pub fn data(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }

    /// `n` bytes of `w`, low first, at `at`.
    fn put(&mut self, at: usize, w: u32, n: usize) {
        for k in 0..n {
            self.bytes[at + k] = (w >> (8 * k)) as u8;
        }
    }
}

/// The callees' outside world: words by address (the bike is `bike`), and their calls. A call's
/// answer is its `v0`; only [`Out::Gear`], [`Out::Trick`], [`Out::NetCheck`], [`Out::TrickRemote`]
/// and [`Out::TrickWanted`] have one.
pub trait Updates {
    fn bike(&self) -> u32;
    fn word(&mut self, at: u32) -> u32;
    fn set(&mut self, at: u32, w: u32);
    fn out(&mut self, c: Out) -> u32;
    /// The vector [`Out::NodePlace`], [`Out::Local`] or [`Out::Rotate`] writes back.
    fn vector(&mut self, c: Out) -> Quad;
    /// The engine's shared smooth noise at `t` ([`crate::noise::SmoothNoise::at_accumulated`] on its
    /// tables), which [`trick_hold`] shakes the held clip by.
    fn noise(&mut self, t: F) -> F;
}

fn w(m: &mut impl Updates, o: u32) -> u32 {
    let b = m.bike();
    m.word(b.wrapping_add(o))
}

fn put(m: &mut impl Updates, o: u32, v: u32) {
    let b = m.bike();
    m.set(b.wrapping_add(o), v);
}

fn f(m: &mut impl Updates, at: u32) -> F {
    F(f32::from_bits(m.word(at)))
}

fn fb(m: &mut impl Updates, o: u32) -> F {
    let b = m.bike();
    f(m, b.wrapping_add(o))
}

fn putf(m: &mut impl Updates, o: u32, v: F) {
    put(m, o, v.to_bits());
}

fn quad(m: &mut impl Updates, at: u32) -> Quad {
    [0, 4, 8, 12].map(|k| m.word(at.wrapping_add(k)))
}

fn set_quad(m: &mut impl Updates, at: u32, q: Quad) {
    for (k, x) in q.into_iter().enumerate() {
        m.set(at.wrapping_add(4 * k as u32), x);
    }
}

fn copy_quad(m: &mut impl Updates, to: u32, from: u32) {
    let q = quad(m, from);
    set_quad(m, to, q);
}

/// A VU op's `.xyz` result over `a`: the three lanes computed, `a`'s `w` as it was in RAM.
fn xyz(v: Vf, a: Quad) -> Quad {
    let mut q = v.to_bits();
    q[3] = a[3];
    q
}

fn vf(q: Quad) -> Vf {
    Vf::from_bits(q)
}

fn byte(m: &mut impl Updates, at: u32) -> u32 {
    m.word(at & !3) >> (8 * (at & 3)) & 0xff
}

fn set_byte(m: &mut impl Updates, at: u32, b: u32) {
    let s = 8 * (at & 3);
    let x = m.word(at & !3) & !(0xff << s) | (b & 0xff) << s;
    m.set(at & !3, x);
}

/// `x` held to 0..1 (`max.s` 0, then `min.s` 1).
fn unit(x: F) -> F {
    x.max(F::ZERO).min(F::ONE)
}

/// A stunt table's entry for a stunt id: the table's `+0x18 + 12 id` for an id under 46 (signed, so a
/// negative one too), else none.
pub fn stunt_entry(table: u32, id: u32) -> u32 {
    if (id as i32) < RIDING_POSE as i32 { table.wrapping_add(id.wrapping_mul(12)).wrapping_add(0x18) } else { 0 }
}

/// **The fade** (`dt`): see the module's list. Each step reads the record's pointer again.
pub fn fade(m: &mut impl Updates, dt: F) {
    if fb(m, FADE) <= F::ZERO {
        return;
    }
    let rec = w(m, RECORD);
    m.set(rec.wrapping_add(0xf8), 0);
    let rec = w(m, RECORD);
    m.out(Out::Search(rec));
    let rec = w(m, RECORD);
    m.set(rec.wrapping_add(0xf8), 1);
    let rec = w(m, RECORD);
    if byte(m, rec.wrapping_add(0x126)) & 0x20 == 0 {
        let left = fb(m, FADE) - dt;
        putf(m, FADE, left);
    }
    let left = fb(m, FADE);
    if left <= F::ZERO {
        m.out(Out::KindClear);
        return;
    }
    let start = fb(m, FADE_START);
    let (lo, hi) = (F(0.1), F::ONE);
    let alpha = if start == F::ZERO {
        lo
    } else {
        let t = (left - start) / (F::ZERO - start);
        Acc::adda(F::ZERO, lo).madd(t, hi - lo).max(lo.min(hi)).min(lo.max(hi))
    };
    let model = w(m, 0x1428);
    let model = m.word(model.wrapping_add(0x24));
    m.set(model.wrapping_add(0x17c), alpha.to_bits());
    let mut i = 0u32;
    while (i as i32) < w(m, 0x1470) as i32 {
        let hung = w(m, 0x1474);
        let part = m.word(hung.wrapping_add(4 * i));
        m.set(part.wrapping_add(0x17c), alpha.to_bits());
        i += 1;
    }
}

/// The pointer to the collision categories' name table: a count, then the names, 32 bytes each.
pub const CATEGORY_NAMES: u32 = 0x0038_d400;
/// A collision category the game set up as it loaded (0 for none), cleared from the parts beside
/// the named ones.
pub const LOADED_CATEGORY: u32 = 0x0054_2388;
/// The three categories by name the kind clear takes off the bike's two parts, in its order around
/// the bike's own (`+0x30c`) and [`LOADED_CATEGORY`].
pub const CLEARED_NAMES: [&[u8]; 3] = [b"DYNAMICOBJECT", b"PHYSICSBODY", b"PHYSICSOBJECT"];

/// **The kind clear** (vt `+0x39c`, the fade's end: F31b5l): nothing while the bike's state is of
/// kind 3. Else each hung part's alpha (`+0x17c`) to 1, then (the state checked again) the bike
/// shown ([`Out::Show`]), `+0xc1c` set; each of the collision record's part (`+0xcc`) and the
/// second part (`+0xd4`) set colliding (`+0xe0`) and these categories taken off its list
/// ([`uncategorise`]): DYNAMICOBJECT, the bike's own, [`LOADED_CATEGORY`]'s, PHYSICSBODY,
/// PHYSICSOBJECT (the named ones' bits from [`category`]); then `+0x9a8` set and the rider's model's
/// alpha (`+0xe4`'s `+0x17c`) to 1.
pub fn kind_clear(m: &mut impl Updates) {
    if kind_three(m) {
        return;
    }
    let mut i = 0u32;
    while (i as i32) < w(m, 0x1470) as i32 {
        let hung = w(m, 0x1474);
        let part = m.word(hung.wrapping_add(4 * i));
        m.set(part.wrapping_add(0x17c), F::ONE.to_bits());
        i += 1;
    }
    if kind_three(m) {
        return;
    }
    m.out(Out::Show);
    put(m, 0xc1c, 1);
    for at in [RECORD, 0xd4] {
        let part = w(m, at);
        if part == 0 {
            continue;
        }
        m.set(part.wrapping_add(0xe0), 1);
        let bit = category(m, CLEARED_NAMES[0]);
        let part = w(m, at);
        uncategorise(m, part, bit);
        let (part, own) = (w(m, at), w(m, 0x30c));
        uncategorise(m, part, own);
        let loaded = m.word(LOADED_CATEGORY);
        let part = w(m, at);
        uncategorise(m, part, loaded);
        for name in &CLEARED_NAMES[1..] {
            let bit = category(m, name);
            let part = w(m, at);
            uncategorise(m, part, bit);
        }
    }
    put(m, 0x9a8, 1);
    let model = w(m, 0xe4);
    m.set(model.wrapping_add(0x17c), F::ONE.to_bits());
}

/// Whether the bike's state (`+0xbc4`, if any) is of kind 3 (`+0x28`).
fn kind_three(m: &mut impl Updates) -> bool {
    let state = w(m, STATE);
    state != 0 && m.word(state.wrapping_add(0x28)) == 3
}

/// **The bike shown** (vt `+0x398` with 1, [`Out::Show`]): with `+0x44`'s low bit and a shown
/// object (`+0xd0`), the object switched on, and put back in the spatial database if it is in no
/// space (its `+0x20`). The object is read again for each step.
pub fn show(m: &mut impl Updates) {
    if w(m, 0x44) & 1 == 0 || w(m, 0xd0) == 0 {
        return;
    }
    let obj = w(m, 0xd0);
    m.out(Out::ObjectOn(obj));
    let obj = w(m, 0xd0);
    if m.word(obj.wrapping_add(0x20)) == 0 {
        let (obj, space, at) = (w(m, 0xd0), w(m, 0xb4), w(m, 0xc0));
        m.out(Out::Insert { obj, space, at });
    }
}

/// A collision category's bit by name: `1 << i` for the name's place `i` in the table
/// ([`CATEGORY_NAMES`]), the name added at the end (and the count raised) when it is not there.
pub fn category(m: &mut impl Updates, name: &[u8]) -> u32 {
    let table = m.word(CATEGORY_NAMES);
    let mut n = m.word(table);
    let mut i = 0u32;
    while (i as i32) < n as i32 {
        let at = table.wrapping_add(4 + 32 * i);
        let same = (0..=name.len()).all(|k| byte(m, at.wrapping_add(k as u32)) == u32::from(name.get(k).copied().unwrap_or(0)));
        if same {
            return 1 << (i & 31);
        }
        n = m.word(table);
        i += 1;
    }
    let at = table.wrapping_add(4 + 32 * n);
    for k in 0..=name.len() {
        set_byte(m, at.wrapping_add(k as u32), u32::from(name.get(k).copied().unwrap_or(0)));
    }
    let count = m.word(table);
    m.set(table, count.wrapping_add(1));
    1 << (n & 31)
}

/// The first entry of `part`'s category list (`+0xfc`, `+0x100` long) that is `bit`, zeroed.
pub fn uncategorise(m: &mut impl Updates, part: u32, bit: u32) {
    let list = m.word(part.wrapping_add(0xfc));
    let mut i = 0u32;
    while (i as i32) < m.word(part.wrapping_add(0x100)) as i32 {
        if m.word(list.wrapping_add(4 * i)) == bit {
            let list = m.word(part.wrapping_add(0xfc));
            m.set(list.wrapping_add(4 * i), 0);
            return;
        }
        i += 1;
    }
}

/// **The ride's auto-gear clock** (`dt`): answers whether the gear moved. With auto gears on
/// (`+0x10d4`), the gear's entry (`+0xec0` its wait, `+0xec4` its step, `+0xec8` up or down): a
/// downshift entry with no wait drops at once to 0; else past 5 (down) or 10 (up) ft/s the clock
/// counts, and past the wait the gear moves by the step, held to 0..`+0x10d8`, and the clock restarts.
pub fn ride(m: &mut impl Updates, dt: F) -> u32 {
    if w(m, 0x10d4) == 0 {
        return 0;
    }
    let g = m.out(Out::Gear);
    let e = (g << 5).wrapping_add(m.bike());
    let down = m.word(e.wrapping_add(0xec8)) != 0;
    if down && f(m, e.wrapping_add(0xec0)) <= F::ZERO {
        putf(m, 0x10d0, F::ZERO);
        putf(m, 0x10cc, F::ZERO);
        return 1;
    }
    if fb(m, 0x32c) <= F(if down { 5.0 } else { 10.0 }) {
        return 0;
    }
    let clock = fb(m, 0x10d0) + dt;
    putf(m, 0x10d0, clock);
    if clock <= f(m, e.wrapping_add(0xec0)) {
        return 0;
    }
    let (step, gear, top) = (f(m, e.wrapping_add(0xec4)), fb(m, 0x10cc), fb(m, 0x10d8));
    let next = if down { gear - step } else { gear + step };
    putf(m, 0x10cc, next.max(F::ZERO).min(top));
    if down {
        putf(m, 0x10d0, F::ZERO);
    } else {
        put(m, 0x10d0, 0);
    }
    1
}

/// **The stunt clock**, first in the lean (`dt`): whether the stunt's clip has run out (`+0x14cc`);
/// with the stunt in the table, its hold (`+0x14f4`) set to 0.2 s when both loads are at or under
/// 0, and the riding pose let go (next stunt none) when it is the pose, the bike is driven and slow;
/// then on the ground with a stunt other than the pose, the hold counts down, and at its end the
/// trick ends and the next stunt is none.
pub fn stunt_clock(m: &mut impl Updates, dt: F) {
    put(m, 0x14cc, 0);
    let (table, id) = (w(m, 0x14f0), w(m, STUNT));
    let entry = stunt_entry(table, id);
    if entry != 0 {
        let anim = m.word(entry);
        if !(fb(m, 0x14d8) < f(m, anim.wrapping_add(0x40))) {
            put(m, 0x14cc, 1);
        }
        if fb(m, 0x704) <= F::ZERO && fb(m, 0x708) <= F::ZERO {
            put(m, 0x14f4, 0x3e4c_cccd);
        }
        if w(m, STUNT) == w(m, 0x152c) && w(m, 0xbbc) == 1 && fb(m, 0x32c) <= f(m, POSE_SPEED) {
            put(m, NEXT_STUNT, 0xff);
        }
    }
    if w(m, 0x324) != 0 {
        let hold = fb(m, 0x14f4);
        if !(hold <= F::ZERO) && w(m, STUNT) != w(m, 0x152c) {
            let hold = hold - dt;
            putf(m, 0x14f4, hold);
            if hold <= F::ZERO {
                m.out(Out::TrickEnd(u32::MAX));
                put(m, NEXT_STUNT, 0xff);
            }
        }
    }
}

/// **The lean** (`dt`): see the module's list.
pub fn lean(m: &mut impl Updates, dt: F) {
    stunt_clock(m, dt);
    if w(m, 0xc80) != 0 {
        put(m, NEXT_STUNT, 0xff);
    } else if m.out(Out::Trick) == 0 {
        if w(m, 0x324) == 0 {
            put(m, NEXT_STUNT, RIDING_POSE);
        } else {
            put(m, 0x13e4, 0);
            put(m, NEXT_STUNT, 0xff);
        }
    }
    if w(m, 0x31c) != 0 {
        put(m, 0x14b4, 0x3f00_0000);
        put(m, 0x14b8, 0x3f00_0000);
    }
}

/// Whether the bike's vehicle state is a held one (3 or 4).
fn held(m: &mut impl Updates) -> bool {
    let s = w(m, STATE);
    let k = m.word(s.wrapping_add(0x28));
    k == 3 || k == 4
}

/// **The prepare** (`dt`): see the module's list.
pub fn prepare(m: &mut impl Updates, dt: F) {
    // The held states' net unpack.
    let s = w(m, STATE);
    if m.word(s.wrapping_add(0x28)).wrapping_sub(3) < 2 {
        let b = m.bike();
        copy_quad(m, b.wrapping_add(0x340), ORIGIN);
        if m.out(Out::NetCheck) != 0 {
            m.out(Out::NetUnpack);
        }
    }
    // The fade's rate, and the clip stopped for a new stunt.
    let rate = unit(Acc::adda(F::ZERO, fb(m, 0x14bc)).msub(fb(m, 0x14c0), dt));
    putf(m, 0x14bc, rate);
    let (mut weight, mut mix) = (F::ZERO, F::ZERO);
    let (mut clips, mut mode) = ([0u32; 2], 0);
    let mut stopped = false;
    if w(m, 0x31c) == 0 && (w(m, NEXT_STUNT) != w(m, STUNT) || w(m, 0x1650) != 0) {
        let ctrl = w(m, 0x1428);
        m.out(Out::AnimStop(ctrl));
        stopped = true;
        putf(m, 0x14bc, F::ONE);
    }
    let next = w(m, NEXT_STUNT);
    put(m, STUNT, next);
    let id = w(m, STUNT);
    if (id as i32) < RIDING_POSE as i32 {
        let table = w(m, 0x14f0);
        let entry = stunt_entry(table, id);
        if entry != 0 {
            let anim = m.word(entry);
            if byte(m, anim.wrapping_add(0x59)) != 0 {
                let id = w(m, STUNT);
                clips[0] = w(m, id.wrapping_mul(4).wrapping_add(0x1534));
                if !stopped {
                    let clip = m.word(clips[0].wrapping_add(4));
                    weight = fb(m, 0x14d8) / f(m, clip.wrapping_add(0x44));
                }
                mode = 1;
            }
        }
    }
    if mode == 0 {
        if w(m, STUNT) == RIDING_POSE {
            if !held(m) {
                let fore_aft = F(0.5) * (F::ONE + fb(m, 0x12ec));
                putf(m, 0x14b8, fore_aft);
                let hi = fb(m, 0x14b0);
                let lo = F(fpu::neg(hi.0));
                let cross = if lo == hi {
                    F::ZERO
                } else {
                    let t = (fb(m, 0x330) - lo) / (hi - lo);
                    let (z, one) = (F::ZERO, F::ONE);
                    Acc::adda(F::ZERO, z).madd(t, one - z).max(z.min(one)).min(z.max(one))
                };
                putf(m, 0x14b4, cross);
            }
            weight = fb(m, 0x14b4);
            putf(m, 0x141c, weight);
            let fore_aft = fb(m, 0x14b8);
            if fore_aft < F(0.5) {
                mix = F(2.0) * fore_aft;
                clips = [w(m, 0x15f0), w(m, 0x15ec)];
            } else {
                let over = fore_aft - F(0.5);
                clips = [w(m, 0x15f4), w(m, 0x15f0)];
                mix = F(2.0) * over;
            }
            mode = 2;
        } else {
            if !held(m) {
                let cross = Acc::adda(F::ZERO, F(0.5)).madd(F(0.5), fb(m, 0x12e8));
                putf(m, 0x14b4, cross);
                let fore_aft = F(0.5) * (F::ONE + fb(m, 0x12ec));
                putf(m, 0x14b8, fore_aft);
            }
            clips[0] = w(m, 0x15f0);
            weight = F(0.5);
            mode = 1;
        }
    }
    let (weight, mix) = (unit(weight), unit(mix));
    let ctrl = w(m, 0x1428);
    let fade = w(m, 0x14bc);
    if mode == 1 {
        m.out(Out::AnimPlay { ctrl, clip: clips[0], weight: weight.to_bits(), fade });
    } else {
        m.out(Out::AnimBlend { ctrl, clips, weight: weight.to_bits(), mix: mix.to_bits(), fade });
    }
    // The seat: the lean's offset (+0x1800) by the suspension's travel, onto the rest spot.
    let b = m.bike();
    let susp = w(m, 0x17b0);
    let s = fb(m, 0x1810) * f(m, susp.wrapping_add(0xf0));
    let lean = quad(m, b.wrapping_add(0x1800));
    let lean = xyz(vf(lean).mul(Vf::from_bits([s.to_bits(); 4])), lean);
    let rest = quad(m, b.wrapping_add(0x17e0));
    let seat = xyz(vf(rest).add(vf(lean)), rest);
    let frame = w(m, 0x1104);
    let at = m.vector(Out::NodePlace { node: frame, at: seat });
    let node = w(m, 0x17c0);
    m.out(Out::NodeSet { node, at });
    // The bars and the head turned.
    if w(m, 0x324) != 0 {
        let susp = w(m, 0x17b0);
        let node = w(m, 0x1108);
        let angle = m.word(susp.wrapping_add(0x8ac));
        m.out(Out::Spin { node, axis: b.wrapping_add(0x1800), angle });
    }
    let susp = w(m, 0x17b0);
    let node = w(m, 0x17c4);
    let angle = m.word(susp.wrapping_add(0x1d0));
    m.out(Out::Spin { node, axis: TURN_AXIS, angle });
    // The aim at the look point (+0x17f0) from the head's spot.
    let head = w(m, 0x17b4);
    let y = f(m, head.wrapping_add(0x94)) + f(m, head.wrapping_add(0xf0));
    let from = [m.word(head.wrapping_add(0x90)), y.to_bits(), m.word(head.wrapping_add(0x98)), F::ONE.to_bits()];
    let look = quad(m, b.wrapping_add(0x17f0));
    let d = xyz(vf(look).sub(vf(from)), look);
    let dir = xyz(normalize(vf(d)), d);
    let node = w(m, 0x17bc);
    m.out(Out::Aim { node, dir, up: AIM_UP });
    let head = w(m, 0x17b4);
    let node = w(m, 0x17c8);
    let angle = m.word(head.wrapping_add(0x1d0));
    m.out(Out::Spin { node, axis: TURN_AXIS, angle });
    // The hung parts from the two bone frames.
    let node = w(m, 0x1104);
    m.out(Out::Pose { node, buf: 0 });
    let node = w(m, 0x1108);
    m.out(Out::Pose { node, buf: 1 });
    let mut i = 0u32;
    while (i as i32) < w(m, 0x1470) as i32 {
        let kinds = w(m, 0x1478);
        let kind = m.word(kinds.wrapping_add(4 * i));
        if kind == 0 || kind == 1 {
            let hung = w(m, 0x1474);
            let part = m.word(hung.wrapping_add(4 * i));
            m.out(Out::Place { part, buf: kind });
        }
        i += 1;
    }
    let a = w(m, 0x1480);
    m.out(Out::Moved(a));
}

/// Whether either of the two words is set, as a word.
fn either(m: &mut impl Updates, a: u32, b: u32) -> u32 {
    u32::from(w(m, a) != 0 || w(m, b) != 0)
}

/// Whether the bike is in the air (`+0x324` clear), as a word.
fn airborne(m: &mut impl Updates) -> u32 {
    u32::from(w(m, 0x324) == 0)
}

/// **The sync**: the camera's record (with its mask, `+0x9c4`, in the bike's flags `+0x444`): the
/// take-off spot and heading kept on leaving the ground, airborne, speed, heading, axes, the body's
/// spot and in the rider's frame, the spin; the replay's (`+0xa94`) copies; the trail's (`+0x734`
/// too) on and off with its word, and its spot, or the wreck's flag; the body's two quadwords
/// (`+0xb54`).
pub fn sync(m: &mut impl Updates) {
    let b = m.bike();
    if w(m, 0x444) & w(m, 0x9c4) != 0 {
        if w(m, 0x324) == 0 && w(m, 0xa70) == 0 {
            let body = w(m, BODY);
            copy_quad(m, b.wrapping_add(0xa60), body.wrapping_add(0x80));
            copy_quad(m, b.wrapping_add(0xa50), b.wrapping_add(0xbd0));
        }
        let air = airborne(m);
        put(m, 0xa70, air);
        let speed = fb(m, 0x32c).max(F::ZERO);
        putf(m, 0xa74, speed);
        copy_quad(m, b.wrapping_add(0x9d0), b.wrapping_add(0xbd0));
        let node = w(m, 0xe4);
        m.out(Out::Axes { node, z: b.wrapping_add(0x9f0), y: b.wrapping_add(0xa00) });
        let body = w(m, BODY);
        copy_quad(m, b.wrapping_add(0xa10), body.wrapping_add(0x80));
        let body = w(m, BODY);
        let node = w(m, 0x1104);
        let v = m.vector(Out::Local { node, at: body.wrapping_add(0x80) });
        set_quad(m, b.wrapping_add(0xa20), v);
        copy_quad(m, b.wrapping_add(0xa40), b.wrapping_add(0x340));
        if w(m, 0x444) & w(m, 0xa94) != 0 {
            copy_quad(m, b.wrapping_add(0xad0), b.wrapping_add(0xa10));
            let moving = either(m, 0x1194, 0x11d0);
            put(m, 0xa98, moving);
            copy_quad(m, b.wrapping_add(0xae0), b.wrapping_add(0xa40));
            let x = w(m, 0x368);
            put(m, 0xb40, x);
            let x = w(m, 0xa70);
            put(m, 0xb38, x);
            let x = w(m, 0x12c8);
            put(m, 0xb3c, x);
        }
    }
    let flags = w(m, 0x444);
    if flags & w(m, 0xa94) == 0 || flags & w(m, 0x734) == 0 {
        body_quads(m);
        return;
    }
    let (on, was) = (w(m, 0xb34), w(m, 0x944));
    if on != 0 && was == 0 {
        let trail = w(m, 0x10dc);
        m.out(Out::TrailOn(trail));
    } else if on == 0 && was != 0 {
        let trail = w(m, 0x10dc);
        m.out(Out::TrailOff(trail));
    }
    let on = w(m, 0xb34);
    put(m, 0x944, on);
    if w(m, 0x868) != 0 {
        put(m, 0xb30, 1);
        body_quads(m);
        return;
    }
    put(m, 0xb30, 0);
    let (off, spot) = (quad(m, b.wrapping_add(0xda0)), quad(m, b.wrapping_add(0xb20)));
    let v = xyz(vf(off).add(vf(spot)), off);
    let camera = w(m, 0x444) & w(m, 0x9c4) != 0;
    let node = w(m, 0xe4);
    let r = m.vector(Out::Rotate { node, v });
    let at = quad(m, b.wrapping_add(0xbd0));
    let at = xyz(vf(at).add(vf(r)), at);
    set_quad(m, b.wrapping_add(0xaa0), at);
    let node = w(m, 0xe4);
    m.out(Out::Axes { node, z: b.wrapping_add(0xab0), y: b.wrapping_add(0xac0) });
    if !camera {
        let body = w(m, BODY);
        copy_quad(m, b.wrapping_add(0xad0), body.wrapping_add(0x80));
        let moving = either(m, 0x1194, 0x11d0);
        put(m, 0xa98, moving);
        copy_quad(m, b.wrapping_add(0xae0), b.wrapping_add(0x340));
        let x = w(m, 0x368);
        put(m, 0xb40, x);
        let air = airborne(m);
        put(m, 0xb38, air);
        let x = w(m, 0x12c8);
        put(m, 0xb3c, x);
    }
    body_quads(m);
}

/// The sync's last: with `+0xb54` in the flags, the body's `+0x70` and `+0x80` copied.
fn body_quads(m: &mut impl Updates) {
    let b = m.bike();
    if w(m, 0x444) & w(m, 0xb54) != 0 {
        let body = w(m, BODY);
        copy_quad(m, b.wrapping_add(0xb60), body.wrapping_add(0x70));
        let body = w(m, BODY);
        copy_quad(m, b.wrapping_add(0xb70), body.wrapping_add(0x80));
    }
}

/// **The snapshot**: with the replay's mask (`+0x734`) in the flags, the replay's record of the
/// frame — the put-back's word, the two bone frames, four node positions, the trail's two buffers,
/// the rider's spot (the body's plus the suspension's and the head's offsets, both offsets then
/// reset), the body's spot in the rider's frame, airborne, the pose, the wreck, the wreck's throw or
/// the body's spot, the blends and speeds, the heading and the body's quaternion, the stunt id (a
/// replayed one, or none), and the rest; then the run's "not riding" word.
pub fn snapshot(m: &mut impl Updates) {
    let b = m.bike();
    if w(m, 0x444) & w(m, 0x734) != 0 {
        let x = w(m, 0x31c);
        put(m, 0x73c, x);
        for (o, at, frame) in [
            (0xe4, 0x750, true),
            (0x1104, 0x790, true),
            (0x17cc, 0x810, false),
            (0x17d0, 0x820, false),
            (0x17d4, 0x7f0, false),
            (0x17d8, 0x800, false),
        ] {
            let node = w(m, o);
            let at = b.wrapping_add(at);
            m.out(if frame { Out::Frame { node, at } } else { Out::Position { node, at } });
        }
        put(m, 0x830, b.wrapping_add(0x1490));
        put(m, 0x834, b.wrapping_add(0x14a0));
        let (susp, head, body) = (w(m, 0x17b0), w(m, 0x17b4), w(m, BODY));
        let spot = quad(m, body.wrapping_add(0x80));
        let s = quad(m, susp.wrapping_add(0xc0));
        let p = xyz(vf(spot).add(vf(s)), spot);
        let h = quad(m, head.wrapping_add(0xc0));
        let p = xyz(vf(p).add(vf(h)), p);
        set_quad(m, b.wrapping_add(0x7d0), p);
        let susp = w(m, 0x17b0);
        copy_quad(m, susp.wrapping_add(0xc0), ORIGIN);
        let head = w(m, 0x17b4);
        copy_quad(m, head.wrapping_add(0xc0), ORIGIN);
        let node = w(m, 0x1104);
        let v = m.vector(Out::Local { node, at: b.wrapping_add(0x340) });
        set_quad(m, b.wrapping_add(0x7e0), v);
        let air = airborne(m);
        put(m, 0x860, air);
        let pose = u32::from(w(m, STUNT) == w(m, 0x152c));
        put(m, 0x864, pose);
        let x = w(m, 0xd08);
        put(m, 0x740, x);
        let x = w(m, 0xc80);
        put(m, 0x868, x);
        if w(m, 0x868) == 7 {
            copy_quad(m, b.wrapping_add(0x8b0), b.wrapping_add(0x1260));
        } else {
            let body = w(m, BODY);
            copy_quad(m, b.wrapping_add(0x8b0), body.wrapping_add(0x80));
        }
        for (to, from) in [(0x8d0, 0x14b4), (0x8d4, 0x14b8), (0x8d8, 0x32c), (0x8c0, 0x318), (0x8c4, 0x12cc), (0x8c8, 0x12d4), (0x8cc, 0x12c4)] {
            let x = w(m, from);
            put(m, to, x);
        }
        copy_quad(m, b.wrapping_add(0x8e0), b.wrapping_add(0xbd0));
        let body = w(m, BODY);
        copy_quad(m, b.wrapping_add(0x8f0), body.wrapping_add(0x190));
        put(m, 0x85c, 0);
        let id = w(m, NEXT_STUNT);
        let kept = (id as i32) < KEPT_STUNTS || id == w(m, 0x13b8) || id == w(m, 0x13bc) || id == w(m, 0x13c0);
        set_byte(m, b.wrapping_add(0x904), if kept { id } else { 0xff });
        for (to, from) in [(0x900, 0x1650), (0x908, 0x14e4), (0x910, 0xd0)] {
            let x = w(m, from);
            put(m, to, x);
        }
        let run = w(m, 0x684);
        let x = if run == 0 { 0 } else { m.word(run.wrapping_add(0x2c0)) };
        put(m, 0x8a4, x);
        let x = w(m, 0x16d4);
        put(m, 0x9a8, x);
        put(m, 0x914, if b == 0 { 0 } else { b.wrapping_add(0xb90) });
        let x = w(m, FADE);
        put(m, 0x9ac, x);
        let x = w(m, FADE_START);
        put(m, 0x9b0, x);
    }
    let run = w(m, 0x684);
    if run != 0 && w(m, 0x444) & w(m, 0x734) != 0 {
        let off = u32::from(m.word(run.wrapping_add(0x50)) & 1 == 0);
        put(m, 0x8a0, off);
    }
}

/// **The net sender's step** (the sender at `net`, `dt`; answers 1): its flags (`+0x14`) cleared but
/// for bits 1-7 and the top half, its clock (`+0x18`) on by `dt`; each timer (`+0x1c` of them, the
/// list at `+0x24`) due at or before the clock sets its bits and bit 0, and its due time moves on by
/// its period; with bit 0 set and bit 1 clear, the send.
pub fn network(m: &mut impl Updates, net: u32, dt: F) -> u32 {
    let flags = m.word(net.wrapping_add(0x14)) & 0xffff_00fe;
    m.set(net.wrapping_add(0x14), flags);
    let clock = f(m, net.wrapping_add(0x18)) + dt;
    m.set(net.wrapping_add(0x18), clock.to_bits());
    let mut i = 0u32;
    while (i as i32) < m.word(net.wrapping_add(0x1c)) as i32 {
        let list = m.word(net.wrapping_add(0x24));
        let e = m.word(list.wrapping_add(4 * i));
        if f(m, e.wrapping_add(8)) <= f(m, net.wrapping_add(0x18)) {
            let bits = m.word(e) | 1;
            let flags = m.word(net.wrapping_add(0x14)) | bits;
            m.set(net.wrapping_add(0x14), flags);
            let due = f(m, e.wrapping_add(8)) + f(m, e.wrapping_add(4));
            m.set(e.wrapping_add(8), due.to_bits());
        }
        i += 1;
    }
    let flags = m.word(net.wrapping_add(0x14));
    if flags & 1 != 0 && flags & 2 == 0 {
        let arg = m.word(net.wrapping_add(0x34));
        m.out(Out::Send { net, arg });
    }
    1
}

/// **The parts** (`dt`): the collision record's (`+0xcc`) and the second part's (`+0xd4`) steps, each
/// when there is one, then the walk of the bike's own listeners (`+0xc`).
pub fn parts(m: &mut impl Updates) {
    for o in [RECORD, 0xd4] {
        let p = w(m, o);
        if p != 0 {
            m.out(Out::Part(p));
        }
    }
    let b = m.bike();
    m.out(Out::Walk(b.wrapping_add(0xc)));
}

/// **A scene node's on** (the trail's vt `+0x18`, F44): its two hidden bits (`+0x21` bits 0 and 1)
/// cleared, one store each, then its own shown (vt `+0x28`).
pub fn trail_on(m: &mut impl Updates, node: u32) {
    let at = node.wrapping_add(0x21);
    for bit in [1, 2] {
        let b = byte(m, at) & !bit;
        set_byte(m, at, b);
    }
    m.out(Out::Shown(node));
}

/// **A scene node's off** (vt `+0x20`): the two bits set, then its own hidden (vt `+0x2c`).
pub fn trail_off(m: &mut impl Updates, node: u32) {
    let at = node.wrapping_add(0x21);
    for bit in [1, 2] {
        let b = byte(m, at) | bit;
        set_byte(m, at, b);
    }
    m.out(Out::Hidden(node));
}

/// **A scene node's shown** (vt `+0x28`): each child (the first at `+0x10`, the next at its `+0xc`)
/// not kept apart (`+0x21` bit 3 clear) shown in turn.
pub fn shown(m: &mut impl Updates, node: u32) {
    children(m, node, Out::Shown);
}

/// **A scene node's hidden** (vt `+0x2c`): the same walk, each child hidden.
pub fn hidden(m: &mut impl Updates, node: u32) {
    children(m, node, Out::Hidden);
}

fn children(m: &mut impl Updates, node: u32, each: fn(u32) -> Out) {
    let mut c = m.word(node.wrapping_add(0x10));
    while c != 0 {
        if byte(m, c.wrapping_add(0x21)) & 8 == 0 {
            m.out(each(c));
        }
        c = m.word(c.wrapping_add(0xc));
    }
}

/// **The bike's send** (the net sender's vt `+0x20`, a thunk to the bike; `arg` the sender's `+0x34`;
/// answers 1): with the sender on (`+0x414` bit 8), the stunt's packet on the beats 3, 6 and 9 of the
/// ten (`+0xcfc`); then, the sender read again, the whole state on beat 0, the change on 1-9, and the
/// beat on by one, mod 10.
pub fn send(m: &mut impl Updates, arg: u32) -> u32 {
    if w(m, 0x414) & 0x100 != 0 && matches!(w(m, 0xcfc), 3 | 6 | 9) {
        m.out(Out::NetStunt(arg));
    }
    if w(m, 0x414) & 0x100 != 0 {
        match w(m, 0xcfc) {
            0 => {
                m.out(Out::NetFull(arg));
            }
            1..=9 => {
                m.out(Out::NetDelta(arg));
            }
            _ => {}
        }
        let beat = (w(m, 0xcfc).wrapping_add(1) as i32).wrapping_rem(10);
        put(m, 0xcfc, beat as u32);
    }
    1
}

/// **A part's step** (vt `+0xbc`, `dt`; the collision record's and the second part's alike): its
/// `+0x126` bit 4 cleared, then the walk of its listeners (`+0x24`).
pub fn part_step(m: &mut impl Updates, part: u32) {
    let at = part.wrapping_add(0x126);
    let b = byte(m, at) & !0x10;
    set_byte(m, at, b);
    m.out(Out::Walk(part.wrapping_add(0x24)));
}

/// The word the stunt packet's stamp takes the timer's whole counts from, and the character table
/// the quantiser table's hash lowers a key's characters by (bit 0: a capital), indexed by the
/// character.
pub const TIMER_COUNTS: u32 = 0x003f_ce18;
pub const CTYPE: u32 = 0x003a_e419;
/// The bike's net words: the quantiser table and the record of the words the packets send.
pub const QUANTISERS: u32 = 0xbc8;
pub const NET_FIELDS: u32 = 0xbcc;

/// An operand off RAM, as the FPU reads it.
fn fo(m: &mut impl Updates, at: u32) -> F {
    F::from_bits(m.word(at))
}

/// `n` bytes from `src` into the packet at `at` (the original's copy, a byte at a time under 32).
fn copy_in(m: &mut impl Updates, p: &mut Packet, at: usize, src: u32, n: usize) {
    for k in 0..n {
        p.bytes[at + k] = byte(m, src.wrapping_add(k as u32)) as u8;
    }
}

/// An unsigned int as the compiler turns one into a float (the halved value converted and doubled).
fn float_of(v: u32) -> F {
    F(fpu::from_u32(v))
}

/// **The stunt's packet** (the bike's vt `+0x544`; `arg` the sender's `+0x34`), 16 bytes: a flag
/// byte (2, and 4 while `+0x14e8` is over 0), the stamp, then three bytes and two words through the
/// net record's (`+0xbcc`) pointers `+0x1c`, `+0x20`, `+0x18`, `+0x10`, `+0x14`. The stamp is the
/// timer's whole counts (read with the interrupts held off, with RCNT0's count) in hundreds plus the
/// count scaled by 256000 / 147456000, as an unsigned int. Handed to the sender's send.
pub fn net_stunt(m: &mut impl Updates, arg: u32) {
    let rec = w(m, NET_FIELDS);
    let mut p = Packet { len: 16, ..Packet::EMPTY };
    for (at, o, n) in [(5, 0x1c, 1), (6, 0x20, 1), (7, 0x18, 1), (8, 0x10, 4), (12, 0x14, 4)] {
        let src = m.word(rec.wrapping_add(o));
        copy_in(m, &mut p, at, src, n);
    }
    let b = m.bike();
    let up = !(fo(m, b.wrapping_add(0x14e8)) <= F::ZERO);
    p.bytes[0] = 2 | u8::from(up) << 2;
    m.out(Out::IrqOff);
    let counts = m.word(TIMER_COUNTS);
    let count = m.out(Out::Counter);
    m.out(Out::IrqOn);
    let t = F(256000.0) * float_of(count) / F::from_bits(0x4d0c_a000) + float_of(counts.wrapping_mul(100));
    let top = F::from_bits(0x4f00_0000);
    let stamp = if top <= t { (t - top).to_i32() as u32 | 0x8000_0000 } else { t.to_i32() as u32 };
    p.put(1, stamp, 4);
    m.out(Out::SenderSend { sender: b.wrapping_add(NET), arg, packet: p });
}

/// The three vectors the whole state and the change send, in their order: the net record's first
/// (`+0`), third (`+8`) and second (`+4`) pointers, three words each.
const NET_VECTORS: [u32; 3] = [0, 8, 4];

/// The net record's second vector's `w` under 0 (its sign, as the two packets test it).
fn turned(m: &mut impl Updates) -> bool {
    let rec = w(m, NET_FIELDS);
    let v = m.word(rec.wrapping_add(4));
    fo(m, v.wrapping_add(0xc)) < F::ZERO
}

/// The network clock's time, the word both packets stamp.
fn net_time(m: &mut impl Updates) -> u32 {
    let clock = m.word(NET_CLOCK);
    m.word(clock.wrapping_add(0x4cc))
}

/// **The whole state's packet** (the bike's vt `+0x400`, beat 0), 41 bytes: a flag byte (4 when the
/// second vector's `w` is under 0), the network clock's time, the nine words as they are; then each
/// word kept as its quantiser's last ([`keep`]). Handed to the sender's send.
pub fn net_full(m: &mut impl Updates, arg: u32) {
    let mut p = Packet { len: 41, ..Packet::EMPTY };
    for (i, o) in NET_VECTORS.into_iter().enumerate() {
        for k in 0..3 {
            let rec = w(m, NET_FIELDS);
            let v = m.word(rec.wrapping_add(o));
            copy_in(m, &mut p, 5 + 12 * i + 4 * k, v.wrapping_add(4 * k as u32), 4);
        }
    }
    if turned(m) {
        p.bytes[0] = 4;
    }
    let time = net_time(m);
    p.put(1, time, 4);
    for o in NET_VECTORS {
        for k in 0..3 {
            let rec = w(m, NET_FIELDS);
            let key = m.word(rec.wrapping_add(o)).wrapping_add(4 * k);
            let x = m.word(key);
            let table = w(m, QUANTISERS);
            keep(m, x, table, key);
        }
    }
    let b = m.bike();
    m.out(Out::SenderSend { sender: b.wrapping_add(NET), arg, packet: p });
}

/// **The change's packet** (the bike's vt `+0x404`, beats 1-9): each of the nine words quantised
/// against its last ([`quantise`], at most 4 bytes each, packed from byte 7), with a 3-byte mask
/// first: 1, each word's byte count less one in two bits from bit 2, bit 20 when the second
/// vector's `w` is under 0, bit 22 while `+0x318` is over 0.1; then the network clock's time.
/// Handed to the sender's send.
pub fn net_delta(m: &mut impl Updates, arg: u32) {
    let mut p = Packet::EMPTY;
    let (mut mask, mut at) = (1u32, 7usize);
    let mut shift = 2;
    for o in NET_VECTORS {
        for k in 0..3 {
            let rec = w(m, NET_FIELDS);
            let key = m.word(rec.wrapping_add(o)).wrapping_add(4 * k);
            let table = w(m, QUANTISERS);
            let x = m.word(key);
            let mut out = [0u8; 4];
            let n = quantise(m, x, table, key, &mut out, 4);
            p.bytes[at..at + n as usize].copy_from_slice(&out[..n as usize]);
            at += n as usize;
            mask |= n.wrapping_sub(1) << shift;
            shift += 2;
        }
    }
    if turned(m) {
        mask |= 0x10_0000;
    }
    let b = m.bike();
    let fast = !(fo(m, b.wrapping_add(0x318)) <= F::from_bits(0x3dcc_cccd));
    mask |= u32::from(fast) << 22;
    p.put(0, mask, 3);
    let time = net_time(m);
    p.put(3, time, 4);
    p.len = at as u32;
    m.out(Out::SenderSend { sender: b.wrapping_add(NET), arg, packet: p });
}

/// **The quantiser table's lookup**: the key's hash (the table's `+0x10`), its bucket (the hash's
/// magnitude rem the bucket count `+4`, of the buckets at `+0`), then down the bucket's chain (next
/// `+0xc`) for an entry with the same hash (`+4`) whose key (`+8`) the table's test (`+0x14`) finds
/// the same; answers that entry's index (`+0`).
pub fn lookup(m: &mut impl Updates, table: u32, key: u32) -> Option<u32> {
    let f = m.word(table.wrapping_add(0x10));
    let h = m.out(Out::Hash { f, key });
    let n = m.word(table.wrapping_add(4)) as i32;
    let a = (h as i32).wrapping_abs();
    let bucket = if n == 0 { a } else { a.wrapping_rem(n) };
    let buckets = m.word(table);
    let mut e = m.word(buckets.wrapping_add((bucket as u32).wrapping_shl(2)));
    while e != 0 {
        if m.word(e.wrapping_add(4)) == h {
            let a = m.word(e.wrapping_add(8));
            let f = m.word(table.wrapping_add(0x14));
            if m.out(Out::SameKey { f, a, b: key }) != 0 {
                return Some(m.word(e));
            }
        }
        e = m.word(e.wrapping_add(0xc));
    }
    None
}

/// A key's quantiser: the table's entries (`+0x30`, 0x18 bytes each) at the lookup's index.
fn quantiser(m: &mut impl Updates, table: u32, key: u32) -> Option<u32> {
    let i = lookup(m, table, key)?;
    Some(m.word(table.wrapping_add(0x30)).wrapping_add(i.wrapping_mul(0x18)))
}

/// **A word kept as its quantiser's last** (the whole state's): `x` (its bits) into the entry's
/// `+8` and `+4`. Answers 1, or 0 for a key the table lacks.
pub fn keep(m: &mut impl Updates, x: u32, table: u32, key: u32) -> u32 {
    let Some(e) = quantiser(m, table, key) else { return 0 };
    m.set(e.wrapping_add(8), x);
    m.set(e.wrapping_add(4), x);
    1
}

/// **A word quantised against its last** (the change's; `x` its bits, `max` the room in `out`): a
/// move from the entry's last (`+4`) under its dead band (`+0x14`) is one zero byte. Else the last
/// goes to `+8`, the move times the scale (`+0xc`) rounded half away from zero is the step count,
/// sent in the fewest of 1-4 bytes that hold it (none if `max` is less), and the new last is `x`
/// plus the steps times the step (`+0x10`), less the move. Answers the bytes written, 0 for a key the
/// table lacks.
pub fn quantise(m: &mut impl Updates, x: u32, table: u32, key: u32, out: &mut [u8], max: u32) -> u32 {
    let Some(e) = quantiser(m, table, key) else { return 0 };
    let last = m.word(e.wrapping_add(4));
    let v = F::from_bits(x);
    let d = v - F::from_bits(last);
    if d.abs() < fo(m, e.wrapping_add(0x14)) {
        out[0] = 0;
        return 1;
    }
    m.set(e.wrapping_add(8), last);
    m.set(e.wrapping_add(4), x);
    let scale = fo(m, e.wrapping_add(0xc));
    let sign = F::from(if d < F::ZERO { -1 } else { 1 });
    let q = Acc::mula(F(0.5), sign).madd(d, scale).to_i32();
    let step = fo(m, e.wrapping_add(0x10));
    let to = Acc::adda(F::ZERO, v).madd(F::from(q), step) - d;
    m.set(e.wrapping_add(4), to.to_bits());
    let n = match q {
        ..-0x80_0000 | 0x80_0000.. => 4,
        ..-0x8000 | 0x8000.. => 3,
        ..-0x80 | 0x80.. => 2,
        _ => 1,
    };
    if (max as i32) < n {
        return 0;
    }
    for (k, b) in out.iter_mut().take(n as usize).enumerate() {
        *b = (q >> (8 * k)) as u8;
    }
    n as u32
}

/// **State 4's quantiser hash** (the table's `+0x10`): the key printed in decimal (as a signed
/// int), and over its characters, each lowered by [`CTYPE`], `h = h << 1 ^ c`.
pub fn decimal_hash(m: &mut impl Updates, key: u32) -> u32 {
    let mut digits = [0u8; 11];
    let mut n = 0;
    let mut v = (key as i32).unsigned_abs();
    loop {
        digits[n] = b'0' + (v % 10) as u8;
        n += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    if (key as i32) < 0 {
        digits[n] = b'-';
        n += 1;
    }
    let mut h = 0u32;
    for &c in digits[..n].iter().rev() {
        let c = c as i8 as i32 as u32;
        let lower = if byte(m, CTYPE.wrapping_add(c)) & 1 != 0 { c.wrapping_add(0x20) } else { c };
        h = h << 1 ^ lower;
    }
    h
}

/// **State 4's quantiser key test** (the table's `+0x14`): the two keys equal.
pub fn same_key(a: u32, b: u32) -> u32 {
    u32::from(a == b)
}

/// **The net sender's own send** (its vt `+0x28`; `a4`, `a5` the caller's last two, 0 and 0 from
/// the bike's packets): the header (the sender's `+0` low byte, the length's) and the packet into
/// the connection's queue (`arg`), with the flags `a5`, or the sender's `+0x10` when `a4` is 0.
pub fn sender_send(m: &mut impl Updates, sender: u32, arg: u32, packet: Packet, a4: u32, a5: u32) {
    let id = m.word(sender) as u8;
    let flags = if a4 == 0 { m.word(sender.wrapping_add(0x10)) } else { a5 };
    m.out(Out::NetQueue { conn: arg, head: [id, packet.len as u8], packet, flags });
}

/// The connection's five message queues (each four words: the count byte's address, the start,
/// the end, the room), its socket (`< 0` none) and its connected word.
pub const CONN_SOCKET: u32 = 0x28;
pub const CONN_UP: u32 = 0x2c;
pub const CONN_QUEUES: [u32; 5] = [0xdc, 0xec, 0xfc, 0x10c, 0x11c];
/// The buffer a full queue is copied to on its way out the socket.
pub const FLUSH_BUFFER: u32 = 0x0044_5cc0;

/// **The connection's queue** (F31b5k; the end of [`sender_send`]): a message (the two header bytes,
/// then the packet) into one of the connection's queues by its flags. Bit 1: the second pair
/// (`+0xfc`, then `+0x10c`); else bit 4: `+0x11c`, only when connected, and always taken; else the
/// first pair (`+0xdc`, then `+0xec`). When a pair's first queue is full and the connection is up
/// (its `+0x2c` set, its socket not negative), the queue goes out the socket ([`Out::NetFlush`],
/// through [`FLUSH_BUFFER`]; bit 1's the reliable send) and is emptied when that does not fail;
/// either way the message then tries the pair's second queue. 1 when a queue took it.
pub fn net_queue(m: &mut impl Updates, conn: u32, head: [u8; 2], packet: &Packet, flags: u32) -> u32 {
    let pair = if flags & 2 != 0 {
        [0xfc, 0x10c]
    } else if flags & 0x10 != 0 {
        if conn_up(m, conn) {
            queue_push(m, conn.wrapping_add(0x11c), head, packet);
        }
        return 1;
    } else {
        [0xdc, 0xec]
    };
    let q = conn.wrapping_add(pair[0]);
    if queue_push(m, q, head, packet) {
        return 1;
    }
    if conn_up(m, conn) {
        let (base, start) = (m.word(q), m.word(q.wrapping_add(4)));
        let len = m.word(q.wrapping_add(8)).wrapping_sub(start);
        for k in 0..len {
            let b = byte(m, start.wrapping_add(k));
            set_byte(m, FLUSH_BUFFER.wrapping_add(k), b);
        }
        let socket = m.word(conn.wrapping_add(CONN_SOCKET));
        if (m.out(Out::NetFlush { socket, len, reliable: flags & 2 != 0 }) as i32) >= 0 {
            set_byte(m, base, 0);
            let start = m.word(q.wrapping_add(4));
            m.set(q.wrapping_add(8), start.wrapping_add(1));
        }
    }
    u32::from(queue_push(m, conn.wrapping_add(pair[1]), head, packet))
}

/// The connection up: its `+0x2c` set and its socket not negative.
fn conn_up(m: &mut impl Updates, conn: u32) -> bool {
    m.word(conn.wrapping_add(CONN_UP)) != 0 && (m.word(conn.wrapping_add(CONN_SOCKET)) as i32) >= 0
}

/// **One queue's push**: the count byte at `q+0`, its messages (two header bytes, the second the
/// data's length, then the data) from `start + 1` to `end`, `room` bytes. A message that does not
/// fit drops messages from the front: up to and including the first whose own size plus the room
/// free (that room alone, not what the drops before it freed) covers the new one; if none within the
/// count does, the queue is left as it is and the push fails (false). Dropping every counted message
/// empties the queue (the count byte cleared, `end = start + 1`). An empty queue still looks at one
/// message past its end.
fn queue_push(m: &mut impl Updates, q: u32, head: [u8; 2], packet: &Packet) -> bool {
    let need = u32::from(head[1]) + 2;
    let base = m.word(q);
    let mut count = byte(m, base);
    let (start, end, room) = (m.word(q.wrapping_add(4)), m.word(q.wrapping_add(8)), m.word(q.wrapping_add(12)));
    let used = end.wrapping_sub(start);
    if room < need.wrapping_add(used) {
        let free = room.wrapping_sub(used);
        let first = start.wrapping_add(1);
        let (mut p, mut n) = (first, 0u32);
        let mut got;
        loop {
            let l = byte(m, p.wrapping_add(1));
            n += 1;
            got = l.wrapping_add(2).wrapping_add(free);
            p = p.wrapping_add(l).wrapping_add(2);
            if (n as i32) >= count as i32 || got >= need {
                break;
            }
        }
        if got < need {
            return false;
        }
        if (n as i32) < count as i32 {
            // A forward copy down, as the original's memcpy does it.
            let len = end.wrapping_sub(p);
            for k in 0..len {
                let b = byte(m, p.wrapping_add(k));
                set_byte(m, first.wrapping_add(k), b);
            }
            m.set(q.wrapping_add(8), first.wrapping_add(len));
            count = count.wrapping_sub(n) & 0xff;
        } else {
            m.set(q.wrapping_add(8), first);
            count = 0;
        }
        set_byte(m, base, count);
    }
    let at = m.word(q.wrapping_add(8));
    set_byte(m, at, u32::from(head[0]));
    set_byte(m, at.wrapping_add(1), u32::from(head[1]));
    let at = at.wrapping_add(2);
    m.set(q.wrapping_add(8), at);
    for k in 0..u32::from(head[1]) {
        set_byte(m, at.wrapping_add(k), u32::from(packet.bytes[k as usize]));
    }
    m.set(q.wrapping_add(8), at.wrapping_add(u32::from(head[1])));
    set_byte(m, base, count.wrapping_add(1));
    true
}

/// The trick step's slack past a clip's length, a tuning word (F42).
pub const TRICK_SLACK: u32 = 0x0052_5370;
/// The network clock's pointer (its `+0x4cc` the time the net check lags) and the lag's own word.
pub const NET_CLOCK: u32 = 0x0038_f768;
pub const NET_LAG: u32 = 0x0052_5258;

/// **The gear** (vt `+0x358`): the rear wheel's (`+0x17b4`) gear byte (`+0x110`), the index into
/// the gear table the ride's auto gears read.
pub fn gear(m: &mut impl Updates) -> u32 {
    let rear = w(m, 0x17b4);
    byte(m, rear.wrapping_add(0x110))
}

/// A stunt table's first stunt of a kind: the index of the first of its 46 entries whose kind byte
/// (`+0x20`, from the table, 12 apart) is `kind`, or none.
pub fn stunt_of_kind(m: &mut impl Updates, table: u32, kind: u32) -> Option<u32> {
    (0..RIDING_POSE).find(|&i| byte(m, table.wrapping_add(12 * i).wrapping_add(0x20)) == kind & 0xff)
}

/// **The trick step** (vt `+0x4e4`, `dt`), what the lean asks for its next stunt: answers whether a
/// trick runs (1) or not (0).
///
/// Its "held" word (`+0x1788`) cleared; a remote bike (`+0x48c`) answers as its own step
/// ([`Out::TrickRemote`]). Else, with the next stunt (`+0x1528`) in the table: its kind is its
/// entry's (`+8`, low four bits), or 4 when it is one of the five kept stunts (`+0x13b8..+0x13c8`) and
/// the bike is driven (`+0x1194` or `+0x11d0`). A clip of the asked kind (`+0x14ec`, low four bits)
/// with a length (the clip's `+0x44`) and its time (`+0x14d8`) from the length or 0.3 s, which is
/// less, to the length plus the slack: the player's mark set, and if the time is still at or past
/// the length, the clip is held at its end, the held word set, and a trick runs. Otherwise the time
/// runs on by `dt`, the clip's fraction (`+0x14e4`, over the clip's `+0x48`, at most 1) is set and a
/// trick runs; but for a kept stunt, a clip under its `+0x48` clears the mark and one past it ends
/// the trick and runs none. Then, with no stunt or its clip run out (`+0x44`), a trick asked for
/// (vt `+0x4b4`) of the asked kind starts (its clock `+0x14d4` reset), and a trick runs.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn trick(m: &mut impl Updates, dt: F) -> u32 {
    put(m, 0x1788, 0);
    if w(m, 0x48c) != 0 {
        return m.out(Out::TrickRemote);
    }
    let (table, id) = (w(m, 0x14f0), w(m, NEXT_STUNT));
    let entry = stunt_entry(table, id);
    let mut runs = 0;
    if entry != 0 {
        let id = w(m, NEXT_STUNT);
        let kept = [0x13b8, 0x13bc, 0x13c0, 0x13c4, 0x13c8].into_iter().any(|o| id == w(m, o));
        let special = kept && (w(m, 0x1194) != 0 || w(m, 0x11d0) != 0);
        let kind = if special { 4 } else { byte(m, entry.wrapping_add(8)) & 0xf };
        let anim = m.word(entry);
        let len = f(m, anim.wrapping_add(0x44));
        let early = len.min(F(0.3));
        let fits = kind == byte(m, m.bike().wrapping_add(0x14ec)) & 0xf && !(len <= F::ZERO) && {
            let time = fb(m, 0x14d8);
            !(time < early) && time <= len + f(m, TRICK_SLACK)
        };
        let held = fits && {
            let (run, player) = (w(m, 0x684), w(m, 0x680));
            let id = byte(m, anim.wrapping_add(0x58));
            m.out(Out::Mark { run, player, on: 1, id });
            let anim = m.word(entry);
            !(fb(m, 0x14d8) < f(m, anim.wrapping_add(0x44)))
        };
        if held {
            m.out(Out::TrickHold(entry));
            put(m, 0x1788, 1);
            return 1;
        }
        let time = fb(m, 0x14d8) + dt;
        putf(m, 0x14d8, time);
        let anim = m.word(entry);
        let frac = (fb(m, 0x14d8) / f(m, anim.wrapping_add(0x48))).min(F::ONE);
        runs = 1;
        putf(m, 0x14e4, frac);
        if !special {
            let anim = m.word(entry);
            if fb(m, 0x14d8) < f(m, anim.wrapping_add(0x48)) {
                let (run, player) = (w(m, 0x684), w(m, 0x680));
                let id = byte(m, anim.wrapping_add(0x58));
                m.out(Out::Mark { run, player, on: 0, id });
            } else {
                let id = byte(m, anim.wrapping_add(0x58));
                m.out(Out::TrickEnd(id));
                runs = 0;
            }
        }
    }
    if entry != 0 {
        let anim = m.word(entry);
        if fb(m, 0x14d8) < f(m, anim.wrapping_add(0x44)) {
            return runs;
        }
    }
    if m.out(Out::TrickWanted) == 0 {
        return runs;
    }
    let table = w(m, 0x14f0);
    let kind = byte(m, m.bike().wrapping_add(0x14ec));
    let Some(id) = stunt_of_kind(m, table, kind) else { return runs };
    m.out(Out::TrickStart(id));
    put(m, 0x14d4, 0);
    1
}

/// **The remote bike's stunt clocks** (vt `+0x4e8`, `dt`), what [`trick`] answers for a bike the
/// network drives (F45). With the next stunt (`+0x1528`) in the table: a held clip (the step's
/// `+0x1788`) is held at its end ([`Out::TrickHold`]) and a trick runs; else the stunt clock
/// (`+0x14d8`) runs on by `dt`, the clip's fraction (`+0x14e4`, over its `+0x48`, at most 1) is set,
/// and a trick runs while the fraction is under 1. Otherwise — no stunt, or its clip run through —
/// the three clocks (`+0x14d8`, `+0x14e8`, `+0x14dc`) go to 0 and no trick runs.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn stunt_clocks(m: &mut impl Updates, dt: F) -> u32 {
    let (table, id) = (w(m, 0x14f0), w(m, NEXT_STUNT));
    let entry = stunt_entry(table, id);
    if entry != 0 {
        if w(m, 0x1788) != 0 {
            m.out(Out::TrickHold(entry));
            return 1;
        }
        let time = fb(m, 0x14d8) + dt;
        putf(m, 0x14d8, time);
        let anim = m.word(entry);
        let frac = (fb(m, 0x14d8) / f(m, anim.wrapping_add(0x48))).min(F::ONE);
        putf(m, 0x14e4, frac);
        if frac < F::ONE {
            return 1;
        }
    }
    for o in [0x14d8, 0x14e8, 0x14dc] {
        put(m, o, 0);
    }
    0
}

/// **Whether a trick is asked for** (vt `+0x4b4`), what [`trick`] asks before it starts one (F45):
/// 1 with the bike off the ground (`+0x324` clear), its `+0xc8` object's `+0x34` clear, the start
/// clock (`+0x14d4`) not zero as a word, both halves of the asked kind (`+0x14ec`, its low and high
/// four bits) set, and the bike not wrecking (`+0x13e4`); else 0.
pub fn trick_wanted(m: &mut impl Updates) -> u32 {
    if w(m, 0x324) != 0 {
        return 0;
    }
    let o = w(m, 0xc8);
    if m.word(o.wrapping_add(0x34)) != 0 || w(m, 0x14d4) == 0 {
        return 0;
    }
    let kind = byte(m, m.bike().wrapping_add(0x14ec));
    if kind & 0xf == 0 || kind & 0xf0 == 0 || w(m, 0x13e4) != 0 {
        return 0;
    }
    1
}

/// The trick hold's shake at its widest: 0.07 of the clip's run.
pub const HOLD_SHAKE: u32 = 0x3d8f_5c29;

/// **The clip held at its end** (vt `+0x4ec`, the stunt's table entry, `dt`), what the trick step
/// does with a clip of the asked kind held past its length ([`Out::TrickHold`]).
///
/// The hold's own clock (`+0x14dc`) ramps the shake from 0 to [`HOLD_SHAKE`] over its first half
/// second, clamped; the shake is that times the smooth noise at twice the noise clock (`+0x14e0`).
/// The noise clock, the hold clock and a third clock (`+0x14e8`) run on by `dt`, and the clip's
/// fraction (`+0x14e4`, which the wreck test reads) is the stunt clock (`+0x14d8`) plus the shake,
/// over the clip's run (`+0x48`). The original's ramp is a lerp from 0 over 0 to 0.5 s (the
/// accumulator's `adda`/`madd`), its clamp `max` then `min` by the ends' `min` and `max`.
pub fn trick_hold(m: &mut impl Updates, entry: u32, dt: F) {
    let (lo, hi) = (F::ZERO, F(f32::from_bits(HOLD_SHAKE)));
    let at = (fb(m, 0x14dc) - lo) / (F(0.5) - lo);
    let ramp = Acc::adda(F::ZERO, lo).madd(at, hi - lo);
    let ramp = ramp.max(lo.min(hi)).min(lo.max(hi));
    let t = F(2.0) * fb(m, 0x14e0);
    let shake = ramp * m.noise(t);
    for o in [0x14e0, 0x14dc, 0x14e8] {
        let c = fb(m, o) + dt;
        putf(m, o, c);
    }
    let anim = m.word(entry);
    let frac = (fb(m, 0x14d8) + shake) / f(m, anim.wrapping_add(0x48));
    putf(m, 0x14e4, frac);
}

/// **The net check** (vt `+0x3e8`), the remote bike's snapshot interpolation; answers 1. With five
/// snapshots come (`+0xd04`, unsigned), it wants the network clock's time less the bike's lag
/// (`+0xd10`) less the lag word, and walks the snapshot slots (`+0xcd4`, `+0xcdc`, … 8 apart, newest
/// first) while that is before the older one's time, at most four steps — the second slot read
/// twice, as the original does — then hands the pair and `t` = `(want − older) / (newer − older)`,
/// clamped to −0.9..1.9, to [`Out::NetInterpolate`]. The quad's twin is
/// [`crate::quad::base_animate`]'s.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn net_check(m: &mut impl Updates) -> u32 {
    if w(m, 0xd04) < 5 {
        return 1;
    }
    let clock = m.word(NET_CLOCK);
    let want = f(m, clock.wrapping_add(0x4cc)) - fb(m, 0xd10) - f(m, NET_LAG);
    let (mut newer, mut older) = (w(m, 0xcd4), w(m, 0xcdc));
    let mut p = m.bike();
    for _ in 0..4 {
        if !(want < f(m, older)) {
            break;
        }
        newer = older;
        older = m.word(p.wrapping_add(0xcdc));
        p = p.wrapping_add(8);
    }
    let o = f(m, older);
    let t = (want - o) / (f(m, newer) - o);
    let t = t.max(F(f32::from_bits(0xbf66_6666))).min(F(f32::from_bits(0x3ff3_3333)));
    m.out(Out::NetInterpolate { older, newer, t: t.to_bits() });
    1
}

/// A quadword copied a word at a time, each read then written (`lwc1`/`swc1`).
fn copy_words(m: &mut impl Updates, to: u32, from: u32) {
    for k in [0, 4, 8, 12] {
        let x = m.word(from.wrapping_add(k));
        m.set(to.wrapping_add(k), x);
    }
}

/// A wheel's spin by its rate: `+0x1d4` = `forward` by the wheel's `+0x838`, `+0x1d0` on by it over
/// `dt` and wrapped to −π..π. Each step reads the wheel's pointer (`at`) again.
fn spin_wheel(m: &mut impl Updates, at: u32, forward: F, dt: F) {
    let wheel = w(m, at);
    let rate = forward * f(m, wheel.wrapping_add(0x838));
    m.set(wheel.wrapping_add(0x1d4), rate.to_bits());
    let wheel = w(m, at);
    let a = Acc::adda(F::ZERO, f(m, wheel.wrapping_add(0x1d0))).madd(f(m, wheel.wrapping_add(0x1d4)), dt);
    m.set(wheel.wrapping_add(0x1d0), a.to_bits());
    let wheel = w(m, at);
    let a = crate::quad::wrap_angle(f(m, wheel.wrapping_add(0x1d0)));
    m.set(wheel.wrapping_add(0x1d0), a.to_bits());
}

/// Whether the wheel at the bike's `o` is down (its `+0xdc`).
fn down(m: &mut impl Updates, o: u32) -> bool {
    let wheel = w(m, o);
    m.word(wheel.wrapping_add(0xdc)) != 0
}

/// **The net unpack** (vt `+0x3f0`, `dt`): the remote bike put where its network snapshot (`+0xbcc`:
/// position, quaternion, velocity) says. The body takes the velocity and the quaternion, the bike's
/// kept position (`+0xc40`) the position; the body's centre (`+0x70`) the position plus its offset
/// (`+0x1a0`) turned into the world, the position (`+0xbd0`) the kept one, the spin (`+0x1d0`) none,
/// the speed (`+0x32c`) the velocity's length; the node turned and placed. The wheels spin by the
/// velocity turned into the body (its `z`); between the two, the front's steer (`+0x8ac`, and the
/// bike's `+0x12c8`) is filtered (`+0xd1c`: [0] the steer, [1] its time, [2] this frame's fraction)
/// towards 0.08 of the body's `+0xb0` in the node's frame, held to the front's `+0x74..+0x70`. Then
/// the wheels run against the ground: both cleared, the contacts, both suspensions, both tire forces;
/// the air words (`+0xde4` the last `+0xde0`, `+0x324` either wheel down, `+0x1270` both, `+0xde0`
/// not down) and the vehicle's end of the unpack. The quad's twin is [`crate::quad::net_unpack`].
pub fn net_unpack(m: &mut impl Updates, dt: F) {
    let b = m.bike();
    let (snap, body) = (w(m, 0xbcc), w(m, BODY));
    let vel = m.word(snap.wrapping_add(8));
    copy_quad(m, body.wrapping_add(0x80), vel);
    let (snap, body) = (w(m, 0xbcc), w(m, BODY));
    let q = m.word(snap.wrapping_add(4));
    copy_words(m, body.wrapping_add(0x190), q);
    let snap = w(m, 0xbcc);
    let pos = m.word(snap);
    copy_quad(m, b.wrapping_add(0xc40), pos);
    let body = w(m, BODY);
    let (q, off) = (quad(m, body.wrapping_add(0x190)), quad(m, body.wrapping_add(0x1a0)));
    let turned = crate::quad::Body { orientation: vf(q), ..Default::default() }.to_world(vf(off));
    let turned = xyz(turned, off);
    let snap = w(m, 0xbcc);
    let pos = m.word(snap);
    let pos = quad(m, pos);
    let centre = xyz(vf(pos).add(vf(turned)), pos);
    let body = w(m, BODY);
    set_quad(m, body.wrapping_add(0x70), centre);
    copy_quad(m, b.wrapping_add(0xbd0), b.wrapping_add(0xc40));
    let body = w(m, BODY);
    copy_quad(m, body.wrapping_add(0x1d0), ORIGIN);
    let body = w(m, BODY);
    let [x, y, z] = [0x80, 0x84, 0x88].map(|o| f(m, body.wrapping_add(o)));
    let speed = Acc::mula(x, x).madda(y, y).madd(z, z).sqrt();
    putf(m, 0x32c, speed);
    let (node, body) = (w(m, 0xe4), w(m, BODY));
    m.out(Out::Orient { node, at: body.wrapping_add(0x190), pos: b.wrapping_add(0xbd0) });
    let body = w(m, BODY);
    let (q, v) = (quad(m, body.wrapping_add(0x190)), quad(m, body.wrapping_add(0x80)));
    let forward = F(crate::quad::Body { orientation: vf(q), ..Default::default() }.to_local(vf(v)).z());
    spin_wheel(m, 0x17b0, forward, dt);
    let (node, body) = (w(m, 0xe4), w(m, BODY));
    let local = m.vector(Out::Local { node, at: body.wrapping_add(0xb0) });
    let front = w(m, 0x17b0);
    let target = (F(0.08) * F(f32::from_bits(local[0]))).max(f(m, front.wrapping_add(0x74))).min(f(m, front.wrapping_add(0x70)));
    let s = w(m, 0xd1c);
    let time = f(m, s.wrapping_add(4));
    let frac = dt.min(time) / time;
    m.set(s.wrapping_add(8), frac.to_bits());
    let steer = f(m, s);
    let steer = Acc::adda(F::ZERO, steer).madd(frac, target - steer);
    m.set(s, steer.to_bits());
    let front = w(m, 0x17b0);
    m.set(front.wrapping_add(0x8ac), steer.to_bits());
    let front = w(m, 0x17b0);
    let x = m.word(front.wrapping_add(0x8ac));
    put(m, 0x12c8, x);
    spin_wheel(m, 0x17b4, forward, dt);
    for o in [0x17b0, 0x17b4] {
        let wheel = w(m, o);
        m.out(Out::WheelClear(wheel));
    }
    m.out(Out::Contacts);
    for o in [0x17b0, 0x17b4] {
        let wheel = w(m, o);
        m.out(Out::Suspension(wheel));
    }
    for o in [0x17b0, 0x17b4] {
        let wheel = w(m, o);
        m.out(Out::TireForces(wheel));
    }
    let x = w(m, 0xde0);
    put(m, 0xde4, x);
    let any = down(m, 0x17b0) || down(m, 0x17b4);
    put(m, 0x324, u32::from(any));
    let all = down(m, 0x17b0) && down(m, 0x17b4);
    put(m, 0x1270, u32::from(all));
    let up = u32::from(w(m, 0x324) == 0);
    put(m, 0xde0, up);
    m.out(Out::NetPost);
}

/// The bike's five ids of the stunts a start keeps running (`+0x13b8`..`+0x13c8`); a start of the
/// last two leaves a queued next stunt of its own.
const KEPT: [u32; 5] = [0x13b8, 0x13bc, 0x13c0, 0x13c4, 0x13c8];
/// The least a stunt's score is handed on as.
const LEAST_SCORE: F = F(10.0);

/// **The stunt start** (vt `+0x4b8`: the stunt `id`, `arg` handed on to the run; F31b5g). A stunt
/// starts when the bike is off the ground (`+0x324`) and not held (`+0xc80`), or when it is one of
/// the [`KEPT`] five. The next stunt ([`NEXT_STUNT`]) becomes `id` unless `id` is one of the five
/// and either the next is under 41 (signed) or `id` is one of the last two. The clip's clocks
/// (`+0x14d8`, `+0x14e8`, `+0x14dc`) go to 0 and `+0x1650` to 1. With a run and a player: a stunt
/// other than the five first ends each of the player's running stunts (its list at `+0xc8`, next at
/// `+0x24`, while a stunt's `+0` has bit 4 and its id `+4` is none of the five, vt `+0x4ac`); then
/// the run's calls ([`Out::RunOpen`], [`Out::RunScore`] off the stunt table's entry `+0x14f0`, its
/// two scores times the entry's scale, at least [`LEAST_SCORE`]; [`Out::Mark`], on `arg`). Answers 1.
pub fn stunt_start(m: &mut impl Updates, id: u32, arg: u32) -> u32 {
    let late = w(m, 0x13c4) == id || w(m, 0x13c8) == id;
    let kept = w(m, 0x13b8) == id || w(m, 0x13bc) == id || w(m, 0x13c0) == id || late;
    if (w(m, 0x324) != 0 || w(m, 0xc80) != 0) && !kept {
        return 1;
    }
    if !kept || (w(m, NEXT_STUNT) as i32 >= KEPT_STUNTS && !late) {
        put(m, NEXT_STUNT, id);
    }
    put(m, 0x14d8, 0);
    put(m, 0x14e8, 0);
    put(m, 0x14dc, 0);
    put(m, 0x1650, 1);
    if w(m, 0x684) == 0 {
        return 1;
    }
    let player = w(m, 0x680);
    if player == 0 {
        return 1;
    }
    if !kept {
        let mut s = m.word(player.wrapping_add(0xc8));
        while s != 0 && m.word(s) & 0x10 != 0 {
            let running = m.word(s.wrapping_add(4));
            if KEPT.iter().any(|&o| w(m, o) == running) {
                break;
            }
            m.out(Out::TrickEnd(running));
            s = m.word(s.wrapping_add(0x24));
        }
    }
    let player = w(m, 0x680);
    if m.word(player.wrapping_add(8)) & 0x20 == 0 {
        let run = w(m, 0x684);
        m.out(Out::RunOpen(run));
    }
    let table = w(m, 0x14f0);
    let e = stunt_entry(table, id);
    let entry = m.word(e);
    let scale = f(m, e.wrapping_add(4));
    let a = (f(m, entry.wrapping_add(0x50)) * scale).max(LEAST_SCORE);
    let b = (f(m, entry.wrapping_add(0x54)) * scale).max(LEAST_SCORE);
    let (run, player) = (w(m, 0x684), w(m, 0x680));
    m.out(Out::RunScore { run, player, id, scores: [a.to_bits(), b.to_bits()] });
    let (run, player) = (w(m, 0x684), w(m, 0x680));
    m.out(Out::Mark { run, player, on: arg, id });
    1
}

/// **The trick end** (vt `+0x4ac`: the stunt `id`, −1 for every one; F31b5g): with a run and a
/// player whose `+8` bit 5 is set, the run closed and tallied ([`Out::RunClose`], [`Out::RunTally`])
/// for −1, else the one stunt ended ([`Out::RunEnd`]). Writes no word of the bike. Answers 1.
pub fn trick_end(m: &mut impl Updates, id: u32) -> u32 {
    let run = w(m, 0x684);
    if run == 0 {
        return 1;
    }
    let player = w(m, 0x680);
    if player == 0 || m.word(player.wrapping_add(8)) & 0x20 == 0 {
        return 1;
    }
    if id != u32::MAX {
        m.out(Out::RunEnd { run, player, id });
        return 1;
    }
    m.out(Out::RunClose { run, player });
    let (player, run) = (w(m, 0x680), w(m, 0x684));
    let count = F(fpu::from_i32(m.word(player.wrapping_add(0xcc)) as i32));
    m.out(Out::RunTally { run, player, count: count.to_bits() });
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stunt_entry_is_twelve_bytes_past_the_header() {
        assert_eq!(stunt_entry(0x1000, 0), 0x1018);
        assert_eq!(stunt_entry(0x1000, 45), 0x1000 + 45 * 12 + 0x18);
        assert_eq!(stunt_entry(0x1000, 46), 0);
        assert_eq!(stunt_entry(0x1000, 0xff), 0);
        assert_eq!(stunt_entry(0x1000, u32::MAX), 0x1000 - 12 + 0x18);
    }
}
