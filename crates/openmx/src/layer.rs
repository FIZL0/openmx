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
