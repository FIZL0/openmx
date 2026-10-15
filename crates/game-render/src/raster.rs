//! A software rasteriser: depth-buffered, perspective-correct, textured, and about as simple as one
//! can be while still drawing a stadium correctly.
//!
//! It takes triangles already in clip space, clips them against the near plane (without which anything
//! behind the camera wraps around to the front), and fills them with an edge function. Attributes are
//! interpolated divided by `w` and put back at the end, which is what keeps a texture from swimming
//! across a large ground quad.
//!
//! There is no backface culling on purpose. The PS2 culls in VU1, using data this crate does not read,
//! and a strip's winding is not reliable without it; the depth buffer gets the same answer for a closed
//! model and a kinder one for the open ones.

use crate::pst::Texture;

/// A vertex handed to the rasteriser.
#[derive(Clone, Copy, Debug)]
pub struct Shade {
    /// Clip space: `w` is view depth.
    pub clip: [f32; 4],
    pub uv: [f32; 2],
    /// Light already applied per vertex, as a multiplier on the texture.
    pub light: [f32; 3],
    /// 0 = clear, 1 = entirely fog.
    pub fog: f32,
}

/// A colour buffer and its depth buffer.
pub struct Frame {
    pub width: usize,
    pub height: usize,
    /// RGB, row by row from the top.
    pub color: Vec<u8>,
    /// 1/w, larger being nearer. Zero is the far plane.
    pub depth: Vec<f32>,
}

/// How near the near plane is, in world units. Nothing nearer survives the clip.
const NEAR: f32 = 0.05;

impl Frame {
    pub fn new(width: usize, height: usize, clear: [u8; 3]) -> Frame {
        let mut color = Vec::with_capacity(width * height * 3);
        for _ in 0..width * height {
            color.extend_from_slice(&clear);
        }
        Frame {
            width,
            height,
            color,
            depth: vec![0.0; width * height],
        }
    }

    /// Draw one triangle. `texture` of `None` paints flat white times the vertex light.
    pub fn triangle(&mut self, v: [Shade; 3], texture: Option<&Texture>, fog: [u8; 3]) {
        // Clip against the near plane: a polygon of at most four vertices comes out.
        let mut poly: Vec<Shade> = Vec::with_capacity(4);
        for i in 0..3 {
            let (a, b) = (v[i], v[(i + 1) % 3]);
            let (da, db) = (a.clip[3] - NEAR, b.clip[3] - NEAR);
            if da >= 0.0 {
                poly.push(a);
            }
            if (da >= 0.0) != (db >= 0.0) {
                poly.push(mix(a, b, da / (da - db)));
            }
        }
        for i in 2..poly.len() {
            self.fill(&[poly[0], poly[i - 1], poly[i]], texture, fog);
        }
    }

    fn fill(&mut self, v: &[Shade; 3], texture: Option<&Texture>, fog: [u8; 3]) {
        let (w, h) = (self.width as f32, self.height as f32);
        // To screen, keeping 1/w and attribute/w for the perspective-correct interpolation.
        let mut p = [[0.0f32; 3]; 3];
        let mut iw = [0.0f32; 3];
        for i in 0..3 {
            let c = v[i].clip;
            let inv = 1.0 / c[3];
            iw[i] = inv;
            p[i] = [
                (c[0] * inv * 0.5 + 0.5) * w,
                (0.5 - c[1] * inv * 0.5) * h,
                c[2] * inv,
            ];
        }
        let area = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[1][1] - p[0][1]) * (p[2][0] - p[0][0]);
        if area.abs() < 1e-7 {
            return;
        }
        let inv_area = 1.0 / area;
        let min_x = p.iter().map(|q| q[0]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
        let max_x = (p.iter().map(|q| q[0]).fold(f32::MIN, f32::max).ceil()).clamp(0.0, w - 1.0) as usize;
        let min_y = p.iter().map(|q| q[1]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
        let max_y = (p.iter().map(|q| q[1]).fold(f32::MIN, f32::max).ceil()).clamp(0.0, h - 1.0) as usize;
        if min_x > max_x || min_y > max_y {
            return;
        }
        for y in min_y..=max_y {
            let py = y as f32 + 0.5;
            for x in min_x..=max_x {
                let px = x as f32 + 0.5;
                // Barycentric weights from the three edge functions.
                let w0 = ((p[1][0] - px) * (p[2][1] - py) - (p[1][1] - py) * (p[2][0] - px)) * inv_area;
                let w1 = ((p[2][0] - px) * (p[0][1] - py) - (p[2][1] - py) * (p[0][0] - px)) * inv_area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let at = y * self.width + x;
                let inv_w = w0 * iw[0] + w1 * iw[1] + w2 * iw[2];
                if inv_w <= self.depth[at] {
                    continue;
                }
                let scale = 1.0 / inv_w;
                let attr = |f: &dyn Fn(&Shade) -> f32| {
                    (w0 * f(&v[0]) * iw[0] + w1 * f(&v[1]) * iw[1] + w2 * f(&v[2]) * iw[2]) * scale
                };
                let (u, vv) = (attr(&|s: &Shade| s.uv[0]), attr(&|s: &Shade| s.uv[1]));
                let texel = match texture {
                    Some(t) => t.sample(u, vv),
                    None => [255, 255, 255, 255],
                };
                if texel[3] < 32 {
                    continue; // a cut-out texture: banners and fences are full of them
                }
                let fogged = attr(&|s: &Shade| s.fog).clamp(0.0, 1.0);
                let mut rgb = [0u8; 3];
                for (k, slot) in rgb.iter_mut().enumerate() {
                    let light = attr(&|s: &Shade| s.light[k]);
                    let lit = texel[k] as f32 * light;
                    *slot = (lit * (1.0 - fogged) + fog[k] as f32 * fogged).clamp(0.0, 255.0) as u8;
                }
                self.color[at * 3..at * 3 + 3].copy_from_slice(&rgb);
                self.depth[at] = inv_w;
            }
        }
    }
}

/// Linear blend of two vertices, for the near-plane clip.
fn mix(a: Shade, b: Shade, t: f32) -> Shade {
    let l = |x: f32, y: f32| x + (y - x) * t;
    Shade {
        clip: [
            l(a.clip[0], b.clip[0]),
            l(a.clip[1], b.clip[1]),
            l(a.clip[2], b.clip[2]),
            l(a.clip[3], b.clip[3]),
        ],
        uv: [l(a.uv[0], b.uv[0]), l(a.uv[1], b.uv[1])],
        light: [
            l(a.light[0], b.light[0]),
            l(a.light[1], b.light[1]),
            l(a.light[2], b.light[2]),
        ],
        fog: l(a.fog, b.fog),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(f: &Frame, x: usize, y: usize) -> [u8; 3] {
        let i = (y * f.width + x) * 3;
        [f.color[i], f.color[i + 1], f.color[i + 2]]
    }

    fn vertex(x: f32, y: f32, w: f32) -> Shade {
        Shade {
            clip: [x * w, y * w, 0.0, w],
            uv: [0.0, 0.0],
            light: [1.0, 1.0, 1.0],
            fog: 0.0,
        }
    }

    #[test]
    fn a_triangle_covers_the_pixels_inside_it_and_no_others() {
        let mut f = Frame::new(16, 16, [0, 0, 0]);
        f.triangle(
            [
                vertex(-0.9, -0.9, 1.0),
                vertex(0.9, -0.9, 1.0),
                vertex(-0.9, 0.9, 1.0),
            ],
            None,
            [0, 0, 0],
        );
        assert_eq!(at(&f, 2, 8), [255, 255, 255], "inside");
        assert_eq!(at(&f, 14, 2), [0, 0, 0], "outside the hypotenuse");
    }

    #[test]
    fn the_nearer_triangle_wins_whichever_order_they_are_drawn_in() {
        for near_first in [true, false] {
            let mut f = Frame::new(8, 8, [0, 0, 0]);
            let far = [vertex(-1.0, -1.0, 8.0), vertex(1.0, -1.0, 8.0), vertex(-1.0, 1.0, 8.0)];
            let mut near = far;
            for v in near.iter_mut() {
                v.clip = [v.clip[0] / 8.0 * 2.0, v.clip[1] / 8.0 * 2.0, 0.0, 2.0];
                v.light = [0.5, 0.5, 0.5];
            }
            let (a, b) = if near_first { (near, far) } else { (far, near) };
            f.triangle(a, None, [0, 0, 0]);
            f.triangle(b, None, [0, 0, 0]);
            assert_eq!(at(&f, 1, 4), [127, 127, 127], "the nearer one is the one that shows");
        }
    }

    #[test]
    fn a_triangle_through_the_camera_is_clipped_not_wrapped() {
        let mut f = Frame::new(16, 16, [0, 0, 0]);
        // One vertex behind the eye: without the near clip this draws a mirrored mess.
        f.triangle(
            [
                Shade { clip: [0.0, 0.0, 0.0, -1.0], ..vertex(0.0, 0.0, 1.0) },
                vertex(0.5, -0.5, 1.0),
                vertex(-0.5, -0.5, 1.0),
            ],
            None,
            [0, 0, 0],
        );
        assert_eq!(at(&f, 8, 2), [0, 0, 0], "nothing above the clipped edge");
        assert_eq!(at(&f, 8, 11), [0, 0, 0], "nor above the edge the clip left behind");
        assert_eq!(at(&f, 8, 13), [255, 255, 255], "the part in front still draws");
    }
}
