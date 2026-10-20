//! The shape of the picture (V24): what frame a window gets, which of the original's two displays
//! that frame stands for, and the field of view each view of it is drawn with.
//!
//! # The original's widescreen
//!
//! The disc has one. Its video modes are 480i, 480p, 480p widescreen, 720p, 1080i, PAL and PAL60, and
//! the display setter files the three widescreen-capable ones under a 16:9 display aspect and the rest
//! under 4:3. That aspect reaches the picture in two places, and the port follows both:
//!
//! * **The projection.** Each viewport's shape term is its own pixel shape times the display's aspect,
//!   and the projection is built from the preset's *horizontal* angle and that term. So at 16:9 the
//!   sideways sweep is the preset's and the vertical one is whatever the shape leaves — the rule the
//!   port already drew every window with, and still does ([`game_render::world::view_fov`], which
//!   only steps in past the shapes the original ever drew).
//! * **The camera presets.** At 16:9 the camera's loader reads a different copy of every preset:
//!   `Preset1w` for the full screen *and* the stacked split, `Preset1x` for side by side, `Preset1z`
//!   for any other layout ([`preset_suffix`]). And its aim skips the stacked split's pitch trim, which
//!   it only makes off the 16:9 display ([`short_viewport`]).
//!
//! The port's window is no video mode, so which of the two displays it stands for is read off the
//! frame's shape: whichever of 4:3 and 16:9 it is nearer ([`Display::of`]).
//!
//! # 4:3 kept
//!
//! `--aspect 4:3` is the shell as it was before: the largest 4:3 frame inside the window, bars round
//! it, and the television's display. `--aspect fill`, the default, draws the window's own shape.

use crate::split::Layout;

/// What shape of frame a window is drawn at: `--aspect`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Aspect {
    /// The window's own shape, whatever it is.
    #[default]
    Fill,
    /// The television's, the largest 4:3 frame that fits, with bars round it.
    Television,
}

impl Aspect {
    /// `--aspect`'s word for it.
    pub fn parse(word: &str) -> Result<Aspect, String> {
        match word.to_ascii_lowercase().as_str() {
            "fill" => Ok(Aspect::Fill),
            "4:3" | "4x3" | "43" => Ok(Aspect::Television),
            _ => Err(format!("--aspect: {word:?} is fill (the window's shape) or 4:3")),
        }
    }

    /// Its name, for the line the shell prints.
    pub fn name(self) -> &'static str {
        match self {
            Aspect::Fill => "fill",
            Aspect::Television => "4:3",
        }
    }

    /// The frame a window of `window` pixels is drawn with at `scale` window pixels to a frame pixel.
    /// Never smaller than a pixel each way, so a window shrunk to nothing still has a frame to draw.
    pub fn frame(self, window: (usize, usize), scale: usize) -> (usize, usize) {
        match self {
            Aspect::Fill => ((window.0 / scale.max(1)).max(1), (window.1 / scale.max(1)).max(1)),
            Aspect::Television => fit_frame(window, scale),
        }
    }
}

/// The largest 4:3 frame inside a window of `window` pixels, at `scale` window pixels to a frame
/// pixel: `--aspect 4:3`'s, and what every window got before V24. The rest of the window is bars
/// ([`crate::window::Window::present`]).
pub fn fit_frame(window: (usize, usize), scale: usize) -> (usize, usize) {
    let (w, h) = (window.0 / scale.max(1), window.1 / scale.max(1));
    let (w, h) = if w * 3 > h * 4 {
        (h * 4 / 3, h)
    } else {
        (w, w * 3 / 4)
    };
    (w.max(1), h.max(1))
}

/// Which of the original's two display aspects a frame stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Display {
    /// 4:3, the television's: 480i, 480p, PAL and PAL60.
    Television,
    /// 16:9: 480p widescreen, 720p and 1080i.
    Wide,
}

impl Display {
    /// The display a frame of `width` by `height` square pixels stands for: whichever of the two
    /// aspects its own is nearer, as a ratio — so 16:10 and 21:9 are wide, 3:2 and every portrait
    /// window are the television. The PS2's own 640x448 is the television's.
    pub fn of(width: usize, height: usize, aspect: Aspect) -> Display {
        if aspect == Aspect::Television {
            return Display::Television;
        }
        // The geometric mean of 4:3 and 16:9: as far as a ratio goes, the middle of the two.
        let middle = (4.0f32 / 3.0 * 16.0 / 9.0).sqrt();
        if width as f32 / height.max(1) as f32 >= middle {
            Display::Wide
        } else {
            Display::Television
        }
    }

    /// Its name, for the line the shell prints.
    pub fn name(self) -> &'static str {
        match self {
            Display::Television => "4:3",
            Display::Wide => "16:9",
        }
    }
}

/// The suffix of the camera presets a view in `layout` reads on `display`: the original loader's
/// choice. On the television it is the layout's own ([`Layout::suffix`]); at 16:9 the full screen and
/// the stacked split read `w`, side by side reads `x`, and any other layout `z` — which no file on the
/// disc has, so the port's quarters and eighths fall back to the full screen's as they do for `q`.
pub fn preset_suffix(layout: Layout, display: Display) -> &'static str {
    match display {
        Display::Television => layout.suffix(),
        Display::Wide => match layout {
            Layout::Full | Layout::Stacked => "w",
            Layout::SideBySide => "x",
            Layout::Quarters | Layout::Eighths => "z",
        },
    }
}

/// Whether the race camera's aim takes its pitch trim for a short view: a view less than 0.55 as tall
/// as it is wide, on the television — the original skips the test altogether at 16:9.
pub fn short_viewport(display: Display, width: usize, height: usize) -> bool {
    display == Display::Television && (height as f32) / (width.max(1) as f32) < 0.55
}

/// The field of view, vertical and in radians, that a view of `width` by `height` pixels is drawn with
/// from a preset's horizontal one in degrees: [`game_render::world::view_fov`] of the view's own shape.
pub fn fov(horizontal_degrees: f32, width: usize, height: usize) -> f32 {
    game_render::world::view_fov(horizontal_degrees, width as f32 / height.max(1) as f32)
}

/// A corner of a view, for what is pinned to one. The riding overlay pins itself to the bottom left;
/// the other three are there for whatever else the shell pins to a view.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Where something `size` pixels across and down goes to sit `margin` pixels in from `corner` of a
/// view of `view` pixels: its top-left pixel, in the view. Pinned to the corner whatever the view's
/// shape, so it neither stretches with the view nor sits in a 4:3 box inside it; a view too small to
/// hold it clips it at the far side rather than moving it off its corner.
pub fn corner(view: (usize, usize), size: (i32, i32), corner: Corner, margin: i32) -> (i32, i32) {
    let (w, h) = (view.0 as i32, view.1 as i32);
    let left = margin;
    let top = margin;
    let right = (w - size.0 - margin).max(margin);
    let bottom = (h - size.1 - margin).max(margin);
    match corner {
        Corner::TopLeft => (left, top),
        Corner::TopRight => (right, top),
        Corner::BottomLeft => (left, bottom),
        Corner::BottomRight => (right, bottom),
    }
}
