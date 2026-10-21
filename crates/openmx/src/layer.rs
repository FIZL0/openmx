//! What the CPU still draws over the GPU's picture (P44): the HUD and the pause menu, as patches.
//!
//! They are drawn by the same code as ever, into a frame — twice, once over black and once over
//! white, which is how a layer that fills, darkens and mixes over whatever is under it comes apart
//! into a multiply and an add ([`Patch`]).

use game_render::gpu::Patch;
use game_render::raster::Frame;

/// The two frames one layer is drawn into, kept between frames.
pub struct Layer {
    black: Frame,
    white: Frame,
}

impl Layer {
    pub fn new() -> Layer {
        Layer {
            black: Frame::new(0, 0, [0, 0, 0]),
            white: Frame::new(0, 0, [255, 255, 255]),
        }
    }

    /// The patch `draw` makes over a `width` by `height` area whose top left is `at` in the frame,
    /// or `None` where it drew nothing.
    pub fn patch(&mut self, width: usize, height: usize, at: (usize, usize), draw: impl Fn(&mut Frame)) -> Option<Patch> {
        if (self.black.width, self.black.height) != (width, height) {
            self.black = Frame::new(width, height, [0, 0, 0]);
            self.white = Frame::new(width, height, [255, 255, 255]);
        }
        self.black.color.fill(0);
        self.white.color.fill(255);
        self.black.depth.fill(0.0);
        self.white.depth.fill(0.0);
        draw(&mut self.black);
        draw(&mut self.white);
        Patch::of(&self.black.color, &self.white.color, width, height, (at.0 as u32, at.1 as u32))
    }
}

/// `top` over every patch in `under` (V28): the GPU writes each patch's rectangle whole, so a later
/// patch that covers an earlier one — the pause, whose rectangle is the whole frame, over each view's
/// HUD — would put its own "leave it be" over the HUD's pixels instead of laying itself on top. Where
/// they overlap, the earlier patches are folded into `top`, `under * mul / 255 + add` applied twice.
pub fn over(under: &[Patch], mut top: Patch) -> Patch {
    let [tx, ty, tw, th] = top.rect;
    for u in under {
        let [ux, uy, uw, uh] = u.rect;
        let (x0, x1) = (tx.max(ux), (tx + tw).min(ux + uw));
        let (y0, y1) = (ty.max(uy), (ty + th).min(uy + uh));
        for y in y0..y1 {
            for x in x0..x1 {
                let t = (((y - ty) * tw + (x - tx)) * 3) as usize;
                let b = (((y - uy) * uw + (x - ux)) * 3) as usize;
                for c in 0..3 {
                    let m = top.mul[t + c] as u32;
                    top.add[t + c] = (u.add[b + c] as u32 * m / 255 + top.add[t + c] as u32).min(255) as u8;
                    top.mul[t + c] = (u.mul[b + c] as u32 * m / 255) as u8;
                }
            }
        }
    }
    top
}
