//! **SLUS-21104's bike vehicle update's own callees** (F39): the slots and calls the vehicle update
//! ([`super::tail::vehicle_update`]) makes on the bike every frame, ported so that the frame's tail
//! runs in Rust down to the scene graph and the animation. Where the original keeps them and their
//! calls: research/oracle.txt `# F39`.
//!
//! - [`fade`] ([`super::tail::Out::Fade`]): with the fade clock running, the collision record's
//!   search (its "searching" word off and on around it); the clock down by `dt` unless the record's
//!   part is held; at its end the bike's kind cleared, else the alpha from the clock's start onto the
//!   rider's model and each hung part.
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
    /// The lean's: the trick step (vt `+0x4e4`, `dt`; an answer, 0 for "no trick"), the trick's end
    /// (vt `+0x4ac`, −1).
    Trick,
    TrickEnd,
    /// The prepare's net check (vt `+0x3e8`, an answer) and unpack (vt `+0x3f0`, `dt`), in the held
    /// vehicle states.
    NetCheck,
    NetUnpack,
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
    /// The trail's on and off (its vt `+0x18` / `+0x20`, on its vt at `+4`).
    TrailOn(u32),
    TrailOff(u32),
    /// The net sender's send (its vt `+0x20`, the vt at `+0x38`: the sender, its `+0x34`; `dt`).
    Send { net: u32, arg: u32 },
    /// A part's step (its vt `+0xbc`, the vt at `+0xc`; `dt`) and the listener walk of a group
    /// ([`super::tail::walk`]; `dt`).
    Part(u32),
    Walk(u32),
}

/// The callees' outside world: words by address (the bike is `bike`), and their calls. A call's
/// answer is its `v0`; only [`Out::Gear`], [`Out::Trick`] and [`Out::NetCheck`] have one.
pub trait Updates {
    fn bike(&self) -> u32;
    fn word(&mut self, at: u32) -> u32;
    fn set(&mut self, at: u32, w: u32);
    fn out(&mut self, c: Out) -> u32;
    /// The vector [`Out::NodePlace`], [`Out::Local`] or [`Out::Rotate`] writes back.
    fn vector(&mut self, c: Out) -> Quad;
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
                m.out(Out::TrickEnd);
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
