//! What the CPU still draws over the GPU's picture (P44): the debug HUD, as patches (the original's HUD and
//! the pause are sprites since V30).
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
    /// What the last drawing touched, x, y, width, height in the layer (V30): all that has to be put
    /// back before the next. A HUD in one corner of a 4K frame no longer clears the whole of it.
    touched: Option<[usize; 4]>,
}

impl Layer {
    pub fn new() -> Layer {
        Layer {
            black: Frame::new(0, 0, [0, 0, 0]),
            white: Frame::new(0, 0, [255, 255, 255]),
            touched: None,
        }
    }

    /// The patch `draw` makes over a `width` by `height` area whose top left is `at` in the frame,
    /// or `None` where it drew nothing.
    pub fn patch(&mut self, width: usize, height: usize, at: (usize, usize), draw: impl Fn(&mut Frame)) -> Option<Patch> {
        if (self.black.width, self.black.height) != (width, height) {
            self.black = Frame::new(width, height, [0, 0, 0]);
            self.white = Frame::new(width, height, [255, 255, 255]);
            self.black.depth.fill(0.0);
            self.white.depth.fill(0.0);
            self.touched = None;
        }
        // Only what the last drawing touched is put back; the rest is still black, white and 0 deep
        // (a layer's flat drawing never writes depth, and a 3D one could only where it is 0 already).
        if let Some([x, y, w, h]) = self.touched.take() {
            for row in y..y + h {
                let (c, d) = ((row * width + x) * 3, row * width + x);
                self.black.color[c..c + w * 3].fill(0);
                self.white.color[c..c + w * 3].fill(255);
                self.black.depth[d..d + w].fill(0.0);
                self.white.depth[d..d + w].fill(0.0);
            }
        }
        draw(&mut self.black);
        draw(&mut self.white);
        let patch = Patch::of(&self.black.color, &self.white.color, width, height, (at.0 as u32, at.1 as u32));
        self.touched = patch.as_ref().map(|p| {
            let [x, y, w, h] = p.rect.map(|n| n as usize);
            [x - at.0, y - at.1, w, h]
        });
        patch
    }
}
