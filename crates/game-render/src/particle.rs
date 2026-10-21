//! Particles in software (P62): the GPU's camera-facing squares ([`crate::gpu::Particle`], P58)
//! drawn by the software renderer, for `--renderer software` and a machine with no GPU to be had.
//!
//! The same picture by the same rules: a square `size` feet across centred on its point, its sides
//! along the view's right and up, cut from the sheet at `uv` with the sheet's top towards the view's
//! up and its left to the view's left. The texel's colour unlit and fogged as the models are here (the
//! scene's own fog, by distance from the eye), its alpha times the particle's, doubled, laid over the
//! frame by `(Cs − Cd) × As + Cd` with the depth test on and no depth written. Drawn after the scene
//! and before the overlay, in the order given.
//!
//! A square that faces the camera lies in a plane parallel to the screen, so all of it is at one view
//! depth and it projects to a rectangle lined up with the pixels: it is filled as that rectangle, a
//! pixel drawn when its centre is inside, rather than as the GPU's two triangles. The rasteriser fills
//! a pixel on an edge two triangles share twice, which for a blended square is a darker diagonal.
//! The texel mixed from its four nearest, as the GPU's linear sampler does, held half a texel inside the
//! particle's own cell of the sheet so it never mixes in a neighbour's.

use crate::mat::v3;
use crate::pst::Texture;
use crate::raster::Frame;
use crate::world::{Camera, Lighting};

/// One particle to draw: [`crate::gpu::Particle`] without the GPU's padding.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Particle {
    /// Its centre, in the world.
    pub at: [f32; 3],
    /// Feet across.
    pub size: f32,
    /// `[left, top, right, bottom]` of the sheet, `0..1`.
    pub uv: [f32; 4],
    /// How opaque, `0..1`, over the texel's own alpha.
    pub alpha: f32,
}

/// Draw `particles` into `frame` as `camera` sees them, cut from `sheet` and fogged by `light`.
/// Returns how many squares reached the frame.
pub fn draw(frame: &mut Frame, camera: &Camera, sheet: &Texture, particles: &[Particle], light: &Lighting) -> usize {
    let (width, height) = (frame.width, frame.height);
    if particles.is_empty() || width == 0 || height == 0 {
        return 0;
    }
    let view_projection = camera.view_projection(width, height);
    let focal = camera.focal(height);
    let mut drawn = 0;
    for p in particles {
        if p.alpha <= 0.0 || p.size <= 0.0 {
            continue;
        }
        let at = v3(p.at[0], p.at[1], p.at[2]);
        let clip = view_projection.clip(at);
        // `w` is the view depth of the whole square; one in front of the near plane is drawn whole.
        let w = clip[3];
        if w <= camera.near {
            continue;
        }
        let (cx, cy) = ((clip[0] / w * 0.5 + 0.5) * width as f32, (0.5 - clip[1] / w * 0.5) * height as f32);
        let half = p.size * 0.5 * focal / w;
        let (x0, x1, y0, y1) = (cx - half, cx + half, cy - half, cy + half);
        // The pixels whose centres are in `[x0, x1)` by `[y0, y1)`.
        let first = |a: f32| (a - 0.5).ceil().max(0.0) as usize;
        let past = |b: f32, n: usize| ((b - 0.5).ceil().max(0.0) as usize).min(n);
        let (left, right, top, bottom) = (first(x0), past(x1, width), first(y0), past(y1, height));
        if left >= right || top >= bottom {
            continue;
        }
        drawn += 1;
        let distance = at.sub(camera.eye).len();
        let fog = ((distance - light.fog_near) / (light.fog_far - light.fog_near).max(1.0)).clamp(0.0, 1.0);
        let inv_w = 1.0 / w;
        // Held half a texel inside its own cell: any nearer the edge mixes in the next cell's.
        let (du, dv) = (0.5 / sheet.width.max(1) as f32, 0.5 / sheet.height.max(1) as f32);
        let (u0, u1) = (p.uv[0].min(p.uv[2]) + du, p.uv[0].max(p.uv[2]) - du);
        let (v0, v1) = (p.uv[1].min(p.uv[3]) + dv, p.uv[1].max(p.uv[3]) - dv);
        for y in top..bottom {
            let t = (y as f32 + 0.5 - y0) / (y1 - y0);
            let v = (p.uv[1] + (p.uv[3] - p.uv[1]) * t).clamp(v0, v1.max(v0));
            for x in left..right {
                let at = y * width + x;
                if inv_w <= frame.depth[at] {
                    continue;
                }
                let s = (x as f32 + 0.5 - x0) / (x1 - x0);
                let u = (p.uv[0] + (p.uv[2] - p.uv[0]) * s).clamp(u0, u1.max(u0));
                let texel = sheet.sample_smooth(u, v);
                let a = (texel[3] as f32 * p.alpha * 2.0).min(255.0) as u8;
                if a == 0 {
                    continue;
                }
                let a = a as f32 / 255.0;
                for k in 0..3 {
                    let fogged = (texel[k] as f32 * (1.0 - fog) + light.fog_color[k] as f32 * fog).clamp(0.0, 255.0);
                    let under = frame.color[at * 3 + k] as f32;
                    frame.color[at * 3 + k] = (under + (fogged - under) * a) as u8;
                }
            }
        }
    }
    drawn
}
