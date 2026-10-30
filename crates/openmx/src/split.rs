//! The split screen (V22): one view of the frame per local player.
//!
//! # The original's layouts
//!
//! The original's race view knows three layouts, and its viewport setup lays each one out by halving
//! the screen it was given (V22's reading of the setup; the journal entry has where):
//!
//! * **0, full screen**: one viewport, the whole frame.
//! * **1, stacked**: two viewports the frame's full width and half its height, the first on top.
//! * **2, side by side**: two viewports half the frame's width and its full height, the first on the
//!   left.
//!
//! The layout's number is also what the camera calls its viewport index, and it is what picks which
//! copy of each camera preset the camera's loader reads out of `cpresets.ini`: `Preset1` for layout 0,
//! `Preset1h` for 1, `Preset1v` for 2, and `Preset1q` for any other index. (Under the display-mode class
//! the video setter writes 3 for its modes 3 to 5 the same three read `w`, `w` and `x`, and any other
//! `z`; the shell's window is not one of those modes and reads the first set.) So `h` is the stacked
//! split — two views each a wide strip, "split horizontally" — and `v` is the side-by-side one.
//!
//! # The port's own
//!
//! The original never draws more than two. Three and four players are drawn here as quarters, and five
//! to eight as eighths — four columns and two rows — which are this port's own layouts. Both read the
//! presets' `q` copies, because an index past the original's three is what reads `q` in its loader;
//! a file with no `q` copies falls back to the full screen's (see [`Layout::suffix`]).
//!
//! A view is drawn into a frame of its own size and put into the window's frame afterwards
//! ([`put`]), so everything that reads the shape of the frame — the field of view, the camera's
//! short-viewport pitch trim, the rider's level of detail — reads its own view's.

use game_render::raster::Frame;

/// How two players share the screen, which the original lets the player choose: `--split`. With no
/// choice made, a pair rides side by side (user, 2026-10-30, P239).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Split {
    /// The original's layout 1: one view above the other.
    Stacked,
    /// The original's layout 2: one view beside the other.
    #[default]
    SideBySide,
}

impl Split {
    /// `--split`'s word for it: the preset suffix each layout reads, or its name.
    pub fn parse(word: &str) -> Result<Split, String> {
        match word.to_ascii_lowercase().as_str() {
            "h" | "horizontal" | "stacked" => Ok(Split::Stacked),
            "v" | "vertical" | "side" => Ok(Split::SideBySide),
            _ => Err(format!("--split: {word:?} is h (one above the other) or v (side by side)")),
        }
    }
}

/// The most local players the shell draws.
pub const MOST: usize = 8;

/// A layout of the frame into views.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// The original's layout 0.
    Full,
    /// The original's layout 1.
    Stacked,
    /// The original's layout 2.
    SideBySide,
    /// The port's own: two by two.
    Quarters,
    /// The port's own: four across, two down.
    Eighths,
}

/// A view's place in the frame, in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

impl Layout {
    /// The layout `players` local players are drawn in, with `split` saying which of the original's
    /// two a pair gets.
    pub fn for_players(players: usize, split: Split) -> Result<Layout, String> {
        match players {
            1 => Ok(Layout::Full),
            2 => Ok(match split {
                Split::Stacked => Layout::Stacked,
                Split::SideBySide => Layout::SideBySide,
            }),
            3 | 4 => Ok(Layout::Quarters),
            5..=MOST => Ok(Layout::Eighths),
            _ => Err(format!("--players: 1 to {MOST}, not {players}")),
        }
    }

    /// The suffix of the camera presets a view in this layout reads: the original's loader's choice
    /// by viewport index, for an index of the original's own, and `q` for the port's own layouts,
    /// because `q` is what the loader reads for every index past its three.
    pub fn suffix(self) -> &'static str {
        match self {
            Layout::Full => "",
            Layout::Stacked => "h",
            Layout::SideBySide => "v",
            Layout::Quarters | Layout::Eighths => "q",
        }
    }

    /// Its name, for the line the shell prints.
    pub fn name(self) -> &'static str {
        match self {
            Layout::Full => "full screen",
            Layout::Stacked => "stacked (the original's h)",
            Layout::SideBySide => "side by side (the original's v)",
            Layout::Quarters => "quarters (the port's own)",
            Layout::Eighths => "eighths (the port's own)",
        }
    }

    /// The (columns, rows) the frame is cut into.
    fn grid(self) -> (usize, usize) {
        match self {
            Layout::Full => (1, 1),
            Layout::Stacked => (1, 2),
            Layout::SideBySide => (2, 1),
            Layout::Quarters => (2, 2),
            Layout::Eighths => (4, 2),
        }
    }

    /// Every view this layout has room for, in player order: left to right along a row, then the
    /// next row down, which for the original's two is its own first-on-top and first-on-the-left.
    ///
    /// The original halves its screen with a shift, so on an odd extent its two halves are one pixel
    /// short of the whole; here the last row and column take the remainder, so the views always cover
    /// the frame. On an even extent, which every PS2 mode and every `fit_frame` frame is, the two are
    /// the same.
    pub fn rects(self, width: usize, height: usize) -> Vec<Rect> {
        let (cols, rows) = self.grid();
        let edge = |i: usize, n: usize, of: usize| i * of / n;
        let mut out = Vec::with_capacity(cols * rows);
        for row in 0..rows {
            for col in 0..cols {
                let (x, y) = (edge(col, cols, width), edge(row, rows, height));
                out.push(Rect {
                    x,
                    y,
                    w: edge(col + 1, cols, width) - x,
                    h: edge(row + 1, rows, height) - y,
                });
            }
        }
        out
    }
}

/// Put a view's picture into the frame at its place. The view's depth is not wanted once it is
/// drawn: the overlay is already on it and nothing draws over the composed frame but the window.
pub fn put(frame: &mut Frame, view: &Frame, at: Rect) {
    let w = at.w.min(view.width).min(frame.width.saturating_sub(at.x));
    for row in 0..at.h.min(view.height).min(frame.height.saturating_sub(at.y)) {
        let from = row * view.width * 3;
        let to = ((at.y + row) * frame.width + at.x) * 3;
        frame.color[to..to + w * 3].copy_from_slice(&view.color[from..from + w * 3]);
    }
}

/// Fill a place in the frame with one colour: a view no player has, in a layout with room for more
/// players than there are.
pub fn blank(frame: &mut Frame, at: Rect, color: [u8; 3]) {
    for row in at.y..(at.y + at.h).min(frame.height) {
        for col in at.x..(at.x + at.w).min(frame.width) {
            let i = (row * frame.width + col) * 3;
            frame.color[i..i + 3].copy_from_slice(&color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The original's two splits, on its own frame: halves by the shift, the first on top or on the
    /// left — exactly the four calls its setup makes to place them.
    #[test]
    fn a_pair_with_no_split_chosen_is_side_by_side() {
        // P239: nothing chosen is the original's v; `--split h` still stacks the two.
        assert_eq!(Layout::for_players(2, Split::default()), Ok(Layout::SideBySide));
        assert_eq!(Layout::for_players(2, Split::parse("h").unwrap()), Ok(Layout::Stacked));
        assert_eq!(Layout::for_players(2, Split::parse("v").unwrap()), Ok(Layout::SideBySide));
    }

    #[test]
    fn the_two_splits_are_the_originals_halves() {
        assert_eq!(
            Layout::Stacked.rects(640, 448),
            vec![Rect { x: 0, y: 0, w: 640, h: 224 }, Rect { x: 0, y: 224, w: 640, h: 224 }]
        );
        assert_eq!(
            Layout::SideBySide.rects(640, 448),
            vec![Rect { x: 0, y: 0, w: 320, h: 448 }, Rect { x: 320, y: 0, w: 320, h: 448 }]
        );
        assert_eq!(Layout::Full.rects(640, 448), vec![Rect { x: 0, y: 0, w: 640, h: 448 }]);
    }

    /// Every layout covers the frame once: no pixel in two views, none in no view, odd sizes too.
    #[test]
    fn every_layout_tiles_the_frame() {
        for players in 1..=MOST {
            for split in [Split::Stacked, Split::SideBySide] {
                let layout = Layout::for_players(players, split).unwrap();
                for (w, h) in [(640, 448), (641, 449), (37, 23)] {
                    let rects = layout.rects(w, h);
                    assert!(rects.len() >= players, "{layout:?} has room for {} of {players}", rects.len());
                    let mut seen = vec![0u8; w * h];
                    for r in &rects {
                        for y in r.y..r.y + r.h {
                            for x in r.x..r.x + r.w {
                                seen[y * w + x] += 1;
                            }
                        }
                    }
                    assert!(seen.iter().all(|&n| n == 1), "{layout:?} at {w}x{h} does not tile the frame");
                }
            }
        }
        assert!(Layout::for_players(0, Split::Stacked).is_err());
        assert!(Layout::for_players(MOST + 1, Split::Stacked).is_err());
    }

    /// The camera copies each layout reads are the loader's by viewport index.
    #[test]
    fn each_layout_reads_its_loaders_copy() {
        let suffix = |n, s| Layout::for_players(n, s).unwrap().suffix();
        assert_eq!(suffix(1, Split::Stacked), "");
        assert_eq!(suffix(2, Split::Stacked), "h");
        assert_eq!(suffix(2, Split::SideBySide), "v");
        assert_eq!(suffix(4, Split::Stacked), "q");
        assert_eq!(suffix(8, Split::SideBySide), "q");
    }

    /// A view put into the frame lands at its place and nowhere else.
    #[test]
    fn a_view_lands_where_it_is_put() {
        let mut frame = Frame::new(8, 6, [0, 0, 0]);
        let view = Frame::new(4, 3, [9, 8, 7]);
        put(&mut frame, &view, Rect { x: 4, y: 3, w: 4, h: 3 });
        for y in 0..6 {
            for x in 0..8 {
                let i = (y * 8 + x) * 3;
                let want = if x >= 4 && y >= 3 { [9, 8, 7] } else { [0, 0, 0] };
                assert_eq!(frame.color[i..i + 3], want, "pixel {x},{y}");
            }
        }
    }
}
