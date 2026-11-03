//! **A collidable brought up to date** (MD75c2): SLUS-21104's `atv_part_sync`, what the quad's collision
//! sync ([`crate::atv_moto::collision_sync`]) runs for the riding probe, the frame mesh and the active
//! probe before the nearby list is gathered round the last.
//!
//! The sync calls the part's vt `+0xc8`, which for both kinds the quad holds is the same two steps:
//! the frame of the part's scene node in the world (`atv_node_frame`: the node brought up to date, its
//! world frame copied; [`crate::atv_moto::Scene::update`]), then the part's vt `+0xcc`, which stands it
//! at that frame ([`stand`]). Then `atv_part_box_move`: if the part has an entry in the world's spatial
//! database (`+0x20`), its vt `+0xc4` gives its box in the world ([`part_box`]), and the entry (`+0x18`)
//! is moved to it with the part's reach (`+0x70`), as `atv_spatial_move` ([`crate::atv_spatial`]) does.
//!
//! The two kinds:
//!
//! - **A ray probe** (vtable `0x003b6c20`: the riding, active, trigger and wreck rays). Its box in its
//!   own frame is its tree's root box (`+0x17c`: centre `+0xc`, half extent `+0x18`, `xyz` only), or
//!   with no tree, half the sum and half the difference of its segments' bound (`+0x180`: low, high).
//!   The stand keeps the frame (`+0x130`) as it is.
//! - **A mesh** (vtable `0x003b6d00`: the frame mesh). Its box is its mesh's (`+0x244`: centre `+0xc`,
//!   half extent `+0x18`). It keeps the frame it was stood at (`+0x1b0`) and the one before (`+0x1f0`),
//!   and its own frame (`+0x130`) is its frame on the node (`+0x170`) times the node's.
//!
//! The stand writes the box's centre through the node's frame into `+0x50` (`xyz`; its `w` is the
//! box's centre's, which for a tree is what `+0x50` held; a mesh's centre does not go through its own
//! frame there, though its box does) and the half diagonal into `+0x70` (an FPU chain).
//! The box hands the centre and the half extent with `w` 1. Field map: research/oracle.txt, `# MD75c2`.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use ps2_float::vu::{Dest, Field};
use ps2_float::{F, Vf, fpu};

/// A collidable's kind, with the words its stand and its box read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// A ray probe: its tree's root box (centre, half extent; `None` for no tree), and its segments'
    /// bound (low, high), read only with no tree.
    Rays { tree: Option<(Vf, Vf)>, bound: (Vf, Vf) },
    /// A mesh: its frame on the node (`+0x170`), its mesh's box (centre, half extent), and the frames it
    /// was last stood at (`+0x1b0`) and the time before (`+0x1f0`).
    Mesh { local: [Vf; 4], centre: Vf, half: Vf, stood: [Vf; 4], before: [Vf; 4] },
}

impl Default for Kind {
    fn default() -> Kind {
        Kind::Rays { tree: None, bound: (Vf::ZERO, Vf::ZERO) }
    }
}

/// One of the quad's collidables as the sync sees it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Collidable {
    pub kind: Kind,
    /// `+0x130`: its frame in the world.
    pub frame: [Vf; 4],
    /// `+0x50`, `+0x70`: its centre in the world and its reach, which the gather reads.
    pub centre: Vf,
    pub reach: F,
    /// `+0x20`: whether it has an entry in the world's spatial database.
    pub spatial: bool,
}

/// Where each [`crate::quad::Part`] is kept in [`crate::atv_moto::Collidables::states`].
pub fn slot(p: crate::quad::Part) -> usize {
    use crate::quad::Part;
    match p {
        Part::Riding => 0,
        Part::Frame => 1,
        Part::Sweep => 2,
        Part::Trigger => 3,
        Part::Wreck => 4,
    }
}

/// `x` and `y` with `w` 1, as the box makes its two vectors on the stack.
fn w_one(v: Vf) -> Vf {
    v.merge(Dest::W, Vf::splat(1.0))
}

/// The box in the part's own frame: a tree's root box, else half the sum and half the difference of
/// the bound (`vadd`/`vsub` `xyz`, then `vmulx` by 0.5; the `w`s the high corner's).
fn own_box(k: &Kind) -> (Vf, Vf) {
    match *k {
        Kind::Rays { tree: Some(b), .. } => b,
        Kind::Rays { tree: None, bound: (lo, hi) } => {
            let half = |v: Vf| v.merge(Dest::XYZ, v.mul(Vf::splat(0.5)));
            (half(hi.merge(Dest::XYZ, hi.add(lo))), half(hi.merge(Dest::XYZ, hi.sub(lo))))
        }
        Kind::Mesh { centre, half, .. } => (centre, half),
    }
}

/// `c` through `m` (`vaddax` of 0 on the position row, then `vmaddax`, `vmadday`, `vmaddz`; `xyz`, the
/// `w` `c`'s).
fn through(m: &[Vf; 4], c: Vf) -> Vf {
    let acc = Vf::madd(Vf::madd(m[3].add(Vf::ZERO), m[0], c.bc(Field::X)), m[1], c.bc(Field::Y));
    c.merge(Dest::XYZ, Vf::madd(acc, m[2], c.bc(Field::Z)))
}

/// **vt `+0xcc`**: the part stood at `frame`, the frame of its node in the world. A mesh keeps the
/// frame and the one before, and takes its own frame on the node into the world (every lane); a ray
/// probe's frame is `frame`. Then the box's centre (`xyz` into `+0x50`; a bound's all four lanes) and
/// the half diagonal of the box (`mula`, `madda`, `madd`, `sqrt.s`), and the centre through `frame` —
/// the node's, not the mesh's own (the mesh's stand reads `a1` again).
pub fn stand(c: &mut Collidable, frame: &[Vf; 4]) {
    let (centre, half) = own_box(&c.kind);
    match &mut c.kind {
        Kind::Mesh { local, stood, before, .. } => {
            *before = *stood;
            *stood = *frame;
            c.frame = crate::atv_moto::frame_mul(local, frame);
            c.centre = c.centre.merge(Dest::XYZ, centre);
        }
        Kind::Rays { tree, .. } => {
            c.frame = *frame;
            c.centre = if tree.is_some() { c.centre.merge(Dest::XYZ, centre) } else { centre };
        }
    }
    let [x, y, z] = [half.x(), half.y(), half.z()];
    c.reach = F(fpu::sqrt(fpu::madd(fpu::madda(fpu::mula(x, x), y, y), z, z)));
    c.centre = through(frame, c.centre);
}

/// **vt `+0xc4`**: the part's box in the world — the centre through its frame, the half extent
/// `(|y row·e.y| + |x row·e.x|) + |z row·e.z|·1` (`vabs`, `vadda`, `vmaddw`), both with `w` 1.
pub fn part_box(c: &Collidable) -> (Vf, Vf) {
    let (centre, half) = own_box(&c.kind);
    let (m, e) = (&c.frame, w_one(half));
    let (hx, hy, hz) = (m[0].mul(e.bc(Field::X)).abs(), m[1].mul(e.bc(Field::Y)).abs(), m[2].mul(e.bc(Field::Z)).abs());
    (through(m, w_one(centre)), e.merge(Dest::XYZ, Vf::madd(hy.add(hx), hz, Vf::splat(1.0))))
}

/// **`atv_part_sync`** after the node's frame: the part stood at `frame` ([`stand`]), then, if it has a
/// database entry, what `atv_part_box_move` moves it to — the box ([`part_box`]) and the reach.
pub fn part_sync(c: &mut Collidable, frame: &[Vf; 4]) -> Option<(Vf, Vf, F)> {
    stand(c, frame);
    c.spatial.then(|| {
        let (centre, half) = part_box(c);
        (centre, half, c.reach)
    })
}
