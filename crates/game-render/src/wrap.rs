//! A world that wraps round (MD29), as it is drawn (MD43): the world again one width off on each
//! wrapped side, so a rider near a seam sees the far side's ground across it and not the sky.
//!
//! A copy is the same world moved by whole widths. Nothing is uploaded twice: a view draws a copy
//! by drawing the world again from its camera moved back by the copy's offset, which puts every
//! point where the moved world's would be ([`Wrap::moved`]). The world spans `0..side` on each
//! wrapped axis, as the sim's `Tiled` does.

use crate::mat::{V3, v3};
use crate::world::Camera;

/// The most copies a view reaches out to on one side of a wrapped axis, however far its fog.
pub const MOST_LAPS: i32 = 4;

/// Which of a world's axes wrap, and its sides: `[x, z]`, in feet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wrap {
    pub on: [bool; 2],
    pub side: [f32; 2],
}

impl Wrap {
    /// The wrap of a world with sides `side`, `None` when neither axis wraps.
    pub fn of(on: [bool; 2], side: [f32; 2]) -> Option<Wrap> {
        (on != [false, false] && side.iter().all(|s| *s > 0.0)).then_some(Wrap { on, side })
    }

    /// The offsets, in whole widths `[x, z]`, of the copies a view at `eye` reaches with `reach`
    /// (the fog's, [`crate::world::Fog::reach`]): each copy whose rectangle comes within `reach`
    /// of the eye across the ground, out to as many widths as the reach spans, and one width each
    /// side for an endless one (Whole Map: the copies' ends and no further). Never `[0, 0]`, the
    /// world itself.
    pub fn copies(&self, eye: V3, reach: f32) -> Vec<[i32; 2]> {
        let laps = |a: usize| -> i32 {
            match (self.on[a], reach.is_finite()) {
                (false, _) => 0,
                (true, false) => 1,
                (true, true) => ((reach / self.side[a]).ceil() as i32).clamp(1, MOST_LAPS),
            }
        };
        // How far the eye is from a copy's span along one axis; 0 inside it, and on an axis that
        // does not wrap, where the copy is the world's own span.
        let gap = |a: usize, k: i32, e: f32| -> f32 {
            if !self.on[a] {
                return 0.0;
            }
            let (lo, hi) = (k as f32 * self.side[a], (k + 1) as f32 * self.side[a]);
            (lo - e).max(e - hi).max(0.0)
        };
        let (nx, nz) = (laps(0), laps(1));
        let mut out = Vec::new();
        for kz in -nz..=nz {
            for kx in -nx..=nx {
                if [kx, kz] == [0, 0] {
                    continue;
                }
                if gap(0, kx, eye.x).hypot(gap(1, kz, eye.z)) <= reach {
                    out.push([kx, kz]);
                }
            }
        }
        out
    }

    /// The copy `k`'s offset in feet.
    pub fn offset(&self, k: [i32; 2]) -> V3 {
        v3(k[0] as f32 * self.side[0], 0.0, k[1] as f32 * self.side[1])
    }

    /// `camera` moved back by the copy `k`'s offset: drawing the world from it draws the copy from
    /// `camera`.
    pub fn moved(&self, camera: &Camera, k: [i32; 2]) -> Camera {
        let by = self.offset(k);
        Camera { eye: camera.eye.sub(by), at: camera.at.sub(by), ..*camera }
    }

    /// `at` taken to its copy nearest `from` on each wrapped axis, as the sim's `nearest_copy` takes a
    /// rival: moved a width while it is more than half a width away.
    pub fn nearest(&self, from: V3, at: V3) -> V3 {
        let near = |a: usize, f: f32, x: f32| -> f32 {
            if !self.on[a] {
                return x;
            }
            let s = self.side[a];
            x - ((x - f) / s).round() * s
        };
        v3(near(0, from.x, at.x), at.y, near(1, from.z, at.z))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_view_by_a_seam_reaches_the_copy_across_it_only() {
        let w = Wrap::of([true, false], [1000.0, 800.0]).expect("wraps");
        // 100 ft from the east seam with 300 ft of fog: the copy east, not the one west.
        assert_eq!(w.copies(v3(900.0, 0.0, 400.0), 300.0), vec![[1, 0]]);
        // In the middle, neither.
        assert!(w.copies(v3(500.0, 0.0, 400.0), 300.0).is_empty());
        // A fog longer than the world reaches two widths out on each side, and no further.
        assert_eq!(w.copies(v3(500.0, 0.0, 400.0), 1600.0), vec![[-2, 0], [-1, 0], [1, 0], [2, 0]]);
        // The whole map: one copy each side.
        assert_eq!(w.copies(v3(500.0, 0.0, 400.0), f32::INFINITY), vec![[-1, 0], [1, 0]]);
        // Both axes: the corner across both seams too.
        let both = Wrap::of([true, true], [1000.0, 1000.0]).expect("wraps");
        assert_eq!(both.copies(v3(950.0, 0.0, 950.0), 100.0), vec![[1, 0], [0, 1], [1, 1]]);
        assert!(Wrap::of([false, false], [1000.0, 1000.0]).is_none());
    }

    #[test]
    fn a_moved_camera_sees_the_copy_where_the_camera_sees_the_moved_world() {
        let w = Wrap::of([true, true], [1000.0, 600.0]).expect("wraps");
        let camera = Camera { eye: v3(990.0, 20.0, 10.0), at: v3(1010.0, 10.0, 5.0), fov: 1.0, near: 1.0, far: 5000.0 };
        let p = v3(15.0, 3.0, 590.0);
        for k in [[1, 0], [0, -1], [1, -1]] {
            let moved = w.moved(&camera, k);
            let a = moved.view_projection(320, 240).clip(p);
            let b = camera.view_projection(320, 240).clip(p.add(w.offset(k)));
            for i in 0..4 {
                assert!((a[i] - b[i]).abs() <= 1e-3 * b[i].abs().max(1.0), "{k:?}: {a:?} against {b:?}");
            }
        }
        assert_eq!(w.nearest(v3(990.0, 0.0, 10.0), v3(15.0, 7.0, 590.0)), v3(1015.0, 7.0, -10.0));
        assert_eq!(w.nearest(v3(500.0, 0.0, 300.0), v3(15.0, 7.0, 290.0)), v3(15.0, 7.0, 290.0));
    }
}
