//! The start gate, lifted. A gate is the original's object at `gate`; its fields are read where the
//! original reads them.
//!
//! - `+0x30` flags: 1 closed, 4 either way through counts, 8 only the left half counts, 0x10 the
//!   gate is a plane with no width
//! - `+0x40` centre, `+0x50` normal (the way through), `+0x60` across (the width's direction)
//! - `+0x70` width, `+0x74` height, `+0x7c` reach squared (0 or less: any distance)
//!
//! The race manager's tick (`race.rs`) asks it, every tick before the drop, whether a rider's last
//! step went through; the drop clears flag 1.
#![deny(clippy::float_arithmetic)]

use crate::Mem;
use ps2_float::{Acc, Dest, F, Vf};

/// What [`crossed`] answers.
pub const NOT: u32 = 0;
pub const THROUGH: u32 = 1;
pub const BESIDE: u32 = 2;
pub const OPEN: u32 = 4;

/// `vsub.xyz`: the difference in x, y and z, the first operand's w kept.
fn sub_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.sub(b))
}

/// The `mula.s` / `madda.s` / `madd.s` dot of two quadwords' x, y and z, in that order.
fn dot3(a: Vf, b: Vf) -> F {
    Acc::mula(a.x(), b.x()).madda(a.y(), b.y()).madd(a.z(), b.z())
}

/// Whether the step from the quadword at `from` to the one at `to` went through the gate at `gate`:
/// [`THROUGH`], [`BESIDE`] (through its plane but outside it, or the wrong way where that counts),
/// [`NOT`], or [`OPEN`] when the gate is not closed. A null `from` or `to` is [`NOT`].
pub fn crossed(m: &Mem, gate: u32, from: u32, to: u32) -> u32 {
    if from == 0 || to == 0 {
        return NOT;
    }
    if m.u32(gate + 0x30) & 1 == 0 {
        return OPEN;
    }
    let zero = F(0.0);
    // Out of reach of the gate's centre, when it has a reach.
    let off = sub_xyz(m.vf(gate + 0x40), m.vf(from));
    let off_sq = dot3(off, off);
    let reach = m.f(gate + 0x7c);
    if !(reach <= zero) && !(off_sq <= reach) {
        return NOT;
    }
    let step = sub_xyz(m.vf(from), m.vf(to));
    let len = dot3(step, step).sqrt();
    if len <= zero {
        return NOT;
    }
    // Where along the step the gate's plane falls, 0 at `to` and 1 at `from`.
    let back = sub_xyz(m.vf(gate + 0x40), m.vf(to));
    let along = dot3(back, m.vf(gate + 0x50)) / len;
    if along < zero {
        return NOT;
    }
    let one = F(1.0);
    if !(along <= one) {
        return NOT;
    }
    let flags = m.u32(gate + 0x30);
    if flags & 0x10 != 0 {
        return THROUGH;
    }
    let width = m.f(gate + 0x70);
    if width <= zero {
        return NOT;
    }
    // The point on the plane, measured from the centre.
    let scaled = step.merge(Dest::XYZ, step.mul(Vf::splat(along.0)));
    let to_v = m.vf(to);
    let on = to_v.merge(Dest::XYZ, to_v.add(scaled));
    let rel = sub_xyz(on, m.vf(gate + 0x40));
    let side = F(2.0) * dot3(rel, m.vf(gate + 0x60)) / width;
    if flags & 8 != 0 && side < zero {
        return BESIDE;
    }
    if !(side <= one) {
        return BESIDE;
    }
    if side < F(-1.0) {
        return BESIDE;
    }
    if !(F(rel.y()) - m.f(gate + 0x64) * side <= m.f(gate + 0x74)) {
        return BESIDE;
    }
    if flags & 4 != 0 {
        return THROUGH;
    }
    if dot3(step, m.vf(gate + 0x50)) <= zero { NOT } else { THROUGH }
}
