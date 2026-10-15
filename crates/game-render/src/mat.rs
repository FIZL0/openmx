//! The small amount of linear algebra a scene needs.
//!
//! Row vectors times row-major matrices, because that is how the game's own files store a transform: a
//! `.psg` node's matrix has its translation in the last four floats, so `v * M` is the file's own
//! convention and needs no transposing on the way in.
//!
//! Plain `f32` arithmetic is deliberate here. This crate draws; it does not simulate. Nothing in it feeds
//! a hash or a replay, so the PS2's rounding does not have to be reproduced — see `game-sim`'s rules for
//! the code where it does.

/// A point or direction.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Shorthand constructor, because this module is mostly made of them.
pub const fn v3(x: f32, y: f32, z: f32) -> V3 {
    V3 { x, y, z }
}

impl V3 {
    pub fn add(self, o: V3) -> V3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
    pub fn sub(self, o: V3) -> V3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
    pub fn scale(self, k: f32) -> V3 {
        v3(self.x * k, self.y * k, self.z * k)
    }
    pub fn dot(self, o: V3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: V3) -> V3 {
        v3(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    pub fn len(self) -> f32 {
        self.dot(self).sqrt()
    }
    /// Unit length, or the zero vector unchanged.
    pub fn norm(self) -> V3 {
        let l = self.len();
        if l > 1e-20 { self.scale(1.0 / l) } else { self }
    }
}

/// A 4x4 transform, row-major: `m[row][col]`, translation in row 3.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct M4(pub [[f32; 4]; 4]);

impl M4 {
    pub const IDENTITY: M4 = M4([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);

    /// Sixteen floats in the order a `.psg` node stores them.
    pub fn from_row_major(f: &[f32]) -> M4 {
        let mut m = [[0.0f32; 4]; 4];
        for r in 0..4 {
            for c in 0..4 {
                m[r][c] = f[r * 4 + c];
            }
        }
        M4(m)
    }

    /// `self` then `o` — the order a child-to-parent chain is walked in.
    pub fn mul(self, o: M4) -> M4 {
        let mut m = [[0.0f32; 4]; 4];
        for r in 0..4 {
            for c in 0..4 {
                m[r][c] = (0..4).map(|k| self.0[r][k] * o.0[k][c]).sum();
            }
        }
        M4(m)
    }

    /// A point: translation applies.
    pub fn point(self, v: V3) -> V3 {
        v3(
            v.x * self.0[0][0] + v.y * self.0[1][0] + v.z * self.0[2][0] + self.0[3][0],
            v.x * self.0[0][1] + v.y * self.0[1][1] + v.z * self.0[2][1] + self.0[3][1],
            v.x * self.0[0][2] + v.y * self.0[1][2] + v.z * self.0[2][2] + self.0[3][2],
        )
    }

    /// A direction: translation does not.
    pub fn dir(self, v: V3) -> V3 {
        v3(
            v.x * self.0[0][0] + v.y * self.0[1][0] + v.z * self.0[2][0],
            v.x * self.0[0][1] + v.y * self.0[1][1] + v.z * self.0[2][1],
            v.x * self.0[0][2] + v.y * self.0[1][2] + v.z * self.0[2][2],
        )
    }

    /// A point in clip space: `(x, y, z, w)`, w being view depth.
    pub fn clip(self, v: V3) -> [f32; 4] {
        [
            v.x * self.0[0][0] + v.y * self.0[1][0] + v.z * self.0[2][0] + self.0[3][0],
            v.x * self.0[0][1] + v.y * self.0[1][1] + v.z * self.0[2][1] + self.0[3][1],
            v.x * self.0[0][2] + v.y * self.0[1][2] + v.z * self.0[2][2] + self.0[3][2],
            v.x * self.0[0][3] + v.y * self.0[1][3] + v.z * self.0[2][3] + self.0[3][3],
        ]
    }

    /// Place a model: `look` is its +Z, `up` its +Y — the two vectors a `.scn` gives for every prop.
    pub fn placement(pos: V3, look: V3, up: V3) -> M4 {
        let z = look.norm();
        let x = up.cross(z).norm();
        let y = z.cross(x);
        M4([
            [x.x, x.y, x.z, 0.0],
            [y.x, y.y, y.z, 0.0],
            [z.x, z.y, z.z, 0.0],
            [pos.x, pos.y, pos.z, 1.0],
        ])
    }

    /// World to view, for a camera at `eye` looking at `at`.
    pub fn look_at(eye: V3, at: V3, up: V3) -> M4 {
        let z = at.sub(eye).norm();
        let x = up.cross(z).norm();
        let y = z.cross(x);
        // The inverse of a rotation-and-translation: transpose the rotation, re-project the eye.
        M4([
            [x.x, y.x, z.x, 0.0],
            [x.y, y.y, z.y, 0.0],
            [x.z, y.z, z.z, 0.0],
            [-eye.dot(x), -eye.dot(y), -eye.dot(z), 1.0],
        ])
    }

    /// View to clip. `fov_y` in radians; `w` comes out as view depth, which is what the rasteriser
    /// interpolates for perspective correction.
    pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> M4 {
        let f = 1.0 / (fov_y * 0.5).tan();
        M4([
            [f / aspect, 0.0, 0.0, 0.0],
            [0.0, f, 0.0, 0.0],
            [0.0, 0.0, far / (far - near), 1.0],
            [0.0, 0.0, -near * far / (far - near), 0.0],
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_placement_puts_the_model_where_the_scene_says() {
        let m = M4::placement(v3(10.0, 2.0, -3.0), v3(0.0, 0.0, 1.0), v3(0.0, 1.0, 0.0));
        assert_eq!(m.point(v3(0.0, 0.0, 0.0)), v3(10.0, 2.0, -3.0));
        // +Z of the model is the look vector.
        assert_eq!(m.dir(v3(0.0, 0.0, 1.0)), v3(0.0, 0.0, 1.0));
        // A quarter turn: look down -X, and the model's +Z must follow it.
        let m = M4::placement(v3(0.0, 0.0, 0.0), v3(-1.0, 0.0, 0.0), v3(0.0, 1.0, 0.0));
        let d = m.dir(v3(0.0, 0.0, 1.0));
        assert!((d.sub(v3(-1.0, 0.0, 0.0))).len() < 1e-6, "{d:?}");
    }

    #[test]
    fn the_camera_puts_what_it_looks_at_down_the_middle() {
        let view = M4::look_at(v3(0.0, 1.0, -10.0), v3(0.0, 1.0, 0.0), v3(0.0, 1.0, 0.0));
        let p = view.point(v3(0.0, 1.0, 0.0));
        assert!(p.x.abs() < 1e-6 && p.y.abs() < 1e-6, "{p:?}");
        assert!((p.z - 10.0).abs() < 1e-5, "{p:?}");
        let clip = M4::perspective(1.0, 4.0 / 3.0, 0.1, 100.0).clip(p);
        assert!((clip[3] - 10.0).abs() < 1e-5, "w is view depth: {clip:?}");
    }
}
