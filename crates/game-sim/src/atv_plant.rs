//! **A plant group's part stood at its plant** (F54): SLUS-21104's eco part stand, what a search over
//! the world asks of a placed plant group (`Narrow::part`, [`crate::world_search`]) for each of its
//! species' parts before it pairs it.
//!
//! The plant (a group entry in the eco's list) keeps its position as three halfwords (`+0xc`, in units
//! of the eco's `+0xc4`), a lean and a size (signed bytes `+0x14`, `+0x15`), its flags (`+0x16`) and a
//! turn (`+0x17`, an index into the eco's turns at `+0x70`, `0x50` bytes each, the quaternion at
//! `+0x10`). Its species (the eco's `+0x84` list by the plant's `+0x12`) holds the parts (`+0x19c`,
//! `+0x194` of them) and the four words the stand scales a sphere by: `+0x100`, `+0x104` (lean) and
//! `+0x10c`, `+0x114` (size).
//!
//! [`stand`]: a hidden plant (flag `0x08`) or an empty slot stands nothing. Otherwise the position is
//! the halfwords times the unit, and by the part's kind (`+0x78`):
//!
//! - **a sphere** (kind 5): its size scale (`+0x188`) `(s0 + size·(s1 - s0)/128)/s0` and its lean
//!   scale (`+0x18c`) `(l0 + lean·(l0 - l1)/128)/l0`; its frame's position (`+0x170`) the plant's; then
//!   its stand slot (vt `+0xcc`) at its own frame, which for a sphere is its refresh at that frame
//!   ([`crate::atv_sphere::refresh`]);
//! - **a mesh** (kind 0): the frame it was last stood at (`+0x1b0`) with its position the plant's
//!   (written back too), turned by the plant's turn when it has one (flag `0x04`: the quaternion as an
//!   angle and an axis, [`axis_angle`]; the angle's cosine and sine off the curve table with no
//!   correction, [`coarse`]; the frame's three rows the rotation about the axis times them, the
//!   position kept); then its stand slot at that frame, the caller's ([`Stood::Mesh`]);
//! - anything else is answered with no stand.
//!
//! Field map: research/oracle.txt, `# F54`.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::air::AcosTable;
use crate::atv_sphere::{self, Sphere};
use crate::camera::CurveTable;
use ps2_float::fpu::Acc;
use ps2_float::{F, Vf, fpu};

/// The plant's flag bits (`+0x16`): not stood at all; turned by its turn.
pub mod flags {
    pub const HIDDEN: u8 = 0x08;
    pub const TURNED: u8 = 0x04;
}

/// A plant, as the stand reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Plant {
    /// `+0xc`, `+0xe`, `+0x10`: the position, in the eco's units.
    pub position: [u16; 3],
    /// `+0x14`, `+0x15`.
    pub lean: i8,
    pub size: i8,
    /// `+0x16`.
    pub flags: u8,
    /// Its turn's quaternion (the eco's turn `+0x17`, its `+0x10`), read only when [`flags::TURNED`].
    pub turn: Vf,
}

/// The species' four scale words: lean `+0x100`, `+0x104`; size `+0x10c`, `+0x114`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Species {
    pub lean: [F; 2],
    pub size: [F; 2],
}

/// The part the plant's slot holds, by its kind (`+0x78`).
pub enum Part<'a> {
    /// Kind 5: the sphere and its two scales (`+0x188` size, `+0x18c` lean).
    Sphere { sphere: &'a mut Sphere, scales: &'a mut [F; 2] },
    /// Kind 0: the frame it was last stood at (`+0x1b0`).
    Mesh { stood: &'a mut [Vf; 4] },
    /// Any other kind: nothing is written.
    Other,
}

/// What the stand left for the caller.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stood {
    /// A hidden plant or an empty slot: no part (the stand answers 0).
    Nothing,
    /// The part, stood (a sphere wholly; any other kind nothing more).
    Done,
    /// A mesh: its stand slot (vt `+0xcc`) is to be called at this frame.
    Mesh([Vf; 4]),
}

/// The tables a turned mesh reads: the arc cosine and the curve.
pub struct Tables<'a> {
    pub acos: &'a AcosTable,
    pub curve: &'a CurveTable,
}

/// **The stand**: `plant`'s part (`part`, `None` for an empty slot) of `species` stood at the plant,
/// `unit` the eco's `+0xc4`. A turned mesh reads `tables`.
pub fn stand(plant: &Plant, species: &Species, unit: F, part: Option<Part>, tables: Option<&Tables>) -> Stood {
    if plant.flags & flags::HIDDEN != 0 {
        return Stood::Nothing;
    }
    let Some(part) = part else { return Stood::Nothing };
    let position = plant.position.map(|h| F(fpu::from_i32(i32::from(h))) * unit);
    match part {
        Part::Sphere { sphere, scales } => {
            let [s0, s1] = species.size;
            let [l0, l1] = species.lean;
            let size = (s0 + (F(fpu::from_i32(i32::from(plant.size))) * (s1 - s0)) / F(128.0)) / s0;
            let lean = (l0 + (F(fpu::from_i32(i32::from(plant.lean))) * (l0 - l1)) / F(128.0)) / l0;
            *scales = [size, lean];
            for (k, p) in position.iter().enumerate() {
                sphere.frame[3].0[k] = p.0;
            }
            let frame = sphere.frame;
            atv_sphere::refresh(sphere, Some(frame));
            Stood::Done
        }
        Part::Mesh { stood } => {
            for (k, p) in position.iter().enumerate() {
                stood[3].0[k] = p.0;
            }
            let mut frame = *stood;
            if plant.flags & flags::TURNED != 0 {
                let t = tables.expect("a turned mesh reads the tables");
                let (angle, axis) = axis_angle(plant.turn, t);
                let (cos, sin) = coarse(angle, t.curve);
                let r = crate::handling::node::turn([F(axis.x()), F(axis.y()), F(axis.z())], sin, cos);
                let m = crate::atv_moto::frame_mul(&r, &frame);
                frame = [m[0], m[1], m[2], frame[3]];
            }
            Stood::Mesh(frame)
        }
        Part::Other => Stood::Done,
    }
}

const PI: F = F(core::f32::consts::PI);
const TWO_PI: F = F(core::f32::consts::TAU);
const ONE_OVER_TWO_PI: F = F(0.15915494);
/// The curve table's `1/step` (the word the lookups read).
const CURVE_INV_STEP: F = F(954.77045);
/// No turn's axis.
const NO_AXIS: Vf = Vf([1.0, 0.0, 0.0, 1.0]);

/// One wrap into `-pi..=pi`: the whole turns off (`adda` of 0, `msub`), then one correction either way
/// (the first compare on false, so a value that does not compare comes down a turn).
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn wrap(a: F) -> F {
    let turns = F(fpu::from_i32((ONE_OVER_TWO_PI * a).to_i32()));
    let w = Acc::adda(F::ZERO, a).msub(TWO_PI, turns);
    if !(w <= PI) {
        w - TWO_PI
    } else if w < -PI {
        TWO_PI + w
    } else {
        w
    }
}

/// The curve table's entry below `angle` (wrapped twice), with no correction: `(cos, sin)`.
pub fn coarse(angle: F, curve: &CurveTable) -> (F, F) {
    let i = ((PI + wrap(wrap(angle))) * CURVE_INV_STEP).to_i32();
    let [cos, sin] = curve.entries()[i as usize];
    (F(cos), F(sin))
}

/// `w` held in `-1..=1`, the high test first and on false (an unordered `w` is one).
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn clamp_unit(w: F) -> F {
    if !(w <= F::ONE) {
        F::ONE
    } else if w < F(-1.0) {
        F(-1.0)
    } else {
        w
    }
}

/// **A quaternion as an angle and an axis**: the half angle the arc cosine of `w` (clamped twice), no
/// turn at all (a half angle equal to zero) the half angle as it is and the `x` axis; else the angle
/// twice the half and the axis the vector half over the curve's sine of the half (the entry below,
/// [`coarse`]), `w` one.
pub fn axis_angle(q: Vf, t: &Tables) -> (F, Vf) {
    let half = t.acos.at(clamp_unit(clamp_unit(F(q.w()))));
    if half == F::ZERO {
        return (half, NO_AXIS);
    }
    let inv = F::ONE / coarse(half, t.curve).1;
    (half * F(2.0), Vf([(F(q.x()) * inv).0, (F(q.y()) * inv).0, (F(q.z()) * inv).0, 1.0]))
}
