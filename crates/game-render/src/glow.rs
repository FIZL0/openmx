//! Glows (V51): textured triangles in the world, tinted, laid over the scene alpha-blended or added,
//! read against its depth and writing none. The freeride activities' start zones are drawn with
//! them — a disc on the ground, a column of light round it and its words over it — and the session
//! markers as a small copy of the same.
//!
//! The pieces come in world space, each vertex with its own tint: the texel's colour times the tint's
//! colour, its alpha times the tint's alpha, unlit and unfogged (the original's zone materials say
//! `foggingOnOff = 0`). An alpha-blended glow lays `(Cs − Cd) × As + Cd`, an added one `Cs × As + Cd`.
//! Drawn after the scene and its particles, before the overlay, in the order given; the GPU's
//! ([`crate::gpu::Renderer::set_glows`]) and this module's software path draw the same list.
//!
//! The software path takes a triangle's alpha as the mean of its three vertices' for an alpha-blended
//! glow (every alpha-blended piece the zones draw has one alpha throughout), and for an added one
//! folds each vertex's alpha into its colour, which for `Cs × As + Cd` is the same sum.

use crate::pst::Texture;
use crate::raster::{Batch, Frame, Mode, Shade};
use crate::world::Camera;

/// One corner of a glow's triangle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vertex {
    /// Where it is, in the world.
    pub at: [f32; 3],
    pub uv: [f32; 2],
    /// Multiplies the texel: colour `0..1` a channel, and how opaque.
    pub tint: [f32; 4],
}

/// Triangles that share a texture and a blend.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Glow {
    /// Which of the glow textures ([`crate::gpu::Renderer::set_glow_textures`], or the list handed
    /// to [`draw`]) it is cut from.
    pub texture: usize,
    /// Added to the frame rather than mixed into it.
    pub additive: bool,
    pub tris: Vec<[Vertex; 3]>,
}

/// Draw `glows` into `frame` as `camera` sees them, each cut from its entry of `textures` (one past
/// the list is drawn white), on every core ([`Frame::draw`]). Returns how many triangles were
/// handed to the rasteriser.
pub fn draw(frame: &mut Frame, camera: &Camera, textures: &[Texture], glows: &[Glow]) -> usize {
    let (width, height) = (frame.width, frame.height);
    if width == 0 || height == 0 {
        return 0;
    }
    let m = camera.view_projection(width, height);
    let mut batch = Batch::new(width, height);
    // Each queued triangle's tag is its place here: its texture, and its alpha where it is mixed in.
    let mut tagged: Vec<(Option<&Texture>, Option<f32>)> = Vec::new();
    for glow in glows {
        let texture = textures.get(glow.texture);
        let mode = Mode { cutoff: 1, blend: true, additive: glow.additive, depth_write: false };
        for tri in &glow.tris {
            let alpha = (tri[0].tint[3] + tri[1].tint[3] + tri[2].tint[3]) / 3.0;
            if !glow.additive && alpha <= 0.0 {
                continue;
            }
            let shade = |v: &Vertex| {
                let fold = if glow.additive { v.tint[3] } else { 1.0 };
                Shade {
                    clip: m.clip(crate::mat::v3(v.at[0], v.at[1], v.at[2])),
                    uv: v.uv,
                    light: [v.tint[0] * fold, v.tint[1] * fold, v.tint[2] * fold],
                    fog: 0.0,
                }
            };
            batch.triangle([shade(&tri[0]), shade(&tri[1]), shade(&tri[2])], tagged.len() as u32, mode);
            tagged.push((texture, (!glow.additive).then_some(alpha)));
        }
    }
    let paint = |tag: u32, u: f32, v: f32| {
        let (texture, alpha) = tagged[tag as usize];
        let mut t = match texture {
            Some(t) => t.sample_smooth(u.rem_euclid(1.0), v.rem_euclid(1.0)),
            None => [255; 4],
        };
        if let Some(alpha) = alpha {
            t[3] = (t[3] as f32 * alpha).clamp(0.0, 255.0) as u8;
        }
        t
    };
    frame.draw(&batch, &paint, [0; 3]);
    tagged.len()
}
