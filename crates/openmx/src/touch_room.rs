//! Where the page's on-screen pad (P197, `web/touch.js`) stands over the frame, so the ride HUD's
//! radar and the front end's hints are drawn clear of it (P208), and the speedometer moves up to the
//! top right while it is shown (P304).
//!
//! The page lists the pad among its gamepads only while it is shown, by the name `Touch controls`
//! ([`crate::pad::TOUCH_PAD`]); the canvas's poll ([`set`]) notes the window it is laid out in
//! while it is. touch.js sizes everything in its `u`, `min(7vw, 12.5vh)`, from the window's
//! corners; the frame is drawn centred in the window, scaled to fit ([`Room`] maps one onto the
//! other). The page's safe-area insets are not known here, so a control is taken to stand from the
//! window's very edge: on a phone with an inset the pad stands further in than this says.

use std::cell::Cell;

thread_local! {
    /// The window the pad is shown over, while it is. The page's shell is one thread (`thread.rs`),
    /// so the canvas's poll and the drawing that reads it share this; a test's thread has its own.
    static WINDOW: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
}

/// The pad is shown over a window of `window` pixels, or (`None`) is not shown: the page's, or the
/// iPhone's own pad (P304, `touch_pad.rs`), laid out alike, over the room inside its kept insets.
#[cfg_attr(not(any(target_arch = "wasm32", target_os = "ios")), allow(dead_code))]
pub fn set(window: Option<(usize, usize)>) {
    WINDOW.with(|w| w.set(window.map(|(x, y)| (x.max(1), y.max(1)))));
}

/// The window the pad is shown over, if it is.
pub fn window() -> Option<(usize, usize)> {
    WINDOW.with(Cell::get)
}

/// touch.js's places, in its `u`, each with the 0.15 `u` a touch past a button's edge still hits
/// and a little room past that.
const GAP: f32 = 0.15 + 0.2;
/// Along the bottom right: MODIFIER's left edge (4.35 + 2.1 in from the right), and GAS's top
/// (0.35 + 2.5 up from the bottom), the tallest of the three pedals.
const PEDALS_LEFT: f32 = 6.45 + GAP;
const PEDALS_TOP: f32 = 2.85 + GAP;
/// Up the right side, the face diamond: △'s top (5.3 + 1.15 up) and left edge (1.6 + 1.15 in), and
/// □'s, beside ○ under it (4.2 + 1.15 up, 2.85 + 1.15 in).
const TRIANGLE_TOP: f32 = 6.45 + GAP;
const TRIANGLE_LEFT: f32 = 2.75 + GAP;
const SQUARE_TOP: f32 = 5.35 + GAP;
const SQUARE_LEFT: f32 = 4.0 + GAP;
/// Along the top, PAUSE's right edge: 0.8 + 1.6 right of the middle.
const PAUSE_RIGHT: f32 = 0.8 + 1.6 + GAP;
/// The stick's base at rest: 2.6 across, centred 1.8 in from the bottom left corner.
const STICK_EDGE: f32 = 1.8 + 1.3 + 0.2;

/// The pad over a frame: the window it is laid out in, and the frame, drawn centred in it scaled
/// to fit, both in their own pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Room {
    pub window: (f32, f32),
    pub frame: (f32, f32),
}

impl Room {
    /// The pad over a frame of `frame` pixels, while it is shown ([`set`]).
    pub fn of(frame: (usize, usize)) -> Option<Room> {
        let (w, h) = window()?;
        Some(Room { window: (w as f32, h as f32), frame: (frame.0.max(1) as f32, frame.1.max(1) as f32) })
    }

    /// touch.js's `u`, in window pixels.
    pub fn unit(self) -> f32 {
        (0.07 * self.window.0).min(0.125 * self.window.1)
    }

    /// Window pixels to a frame pixel, and how far the frame's edges stand in from the window's.
    fn fit(self) -> (f32, (f32, f32)) {
        let k = (self.window.0 / self.frame.0).min(self.window.1 / self.frame.1);
        (k, ((self.window.0 - self.frame.0 * k) / 2.0, (self.window.1 - self.frame.1 * k) / 2.0))
    }

    /// `u` many `u` in from the window's left or right edge, as frame pixels in from the frame's.
    fn across(self, u: f32) -> f32 {
        let (k, (x, _)) = self.fit();
        ((u * self.unit() - x) / k).max(0.0)
    }

    /// `u` many `u` up from the window's bottom edge, as frame pixels up from the frame's.
    fn up(self, u: f32) -> f32 {
        let (k, (_, y)) = self.fit();
        ((u * self.unit() - y) / k).max(0.0)
    }

    /// How far in from the frame's right edge something along the bottom stands clear of the
    /// pedals, and how far up from the bottom that holds: left of MODIFIER, up to GAS's top.
    pub fn left_of_the_pedals(self) -> (f32, f32) {
        (self.across(PEDALS_LEFT), self.up(PEDALS_TOP))
    }

    /// How far down from the frame's top something in its top right corner stands clear of the
    /// face diamond under it, in frame pixels: over △.
    pub fn over_the_faces(self) -> f32 {
        self.frame.1 - self.up(TRIANGLE_TOP)
    }

    /// How far down from the frame's top something left of △ stands clear of the faces: over □.
    pub fn over_the_square(self) -> f32 {
        self.frame.1 - self.up(SQUARE_TOP)
    }

    /// How far in from the frame's right edge something in its top right corner reaching `down`
    /// frame pixels from the top stands clear of the faces: left of △, or of □ once it reaches as
    /// low as □'s top.
    pub fn left_of_the_faces(self, down: f32) -> f32 {
        let wide = down > self.over_the_square();
        self.across(if wide { SQUARE_LEFT } else { TRIANGLE_LEFT })
    }

    /// How far in from the frame's left edge something along the top stands clear of the small
    /// buttons there: right of PAUSE.
    pub fn right_of_the_top(self) -> f32 {
        let (k, (x, _)) = self.fit();
        ((self.window.0 / 2.0 + PAUSE_RIGHT * self.unit() - x) / k).max(0.0)
    }

    /// How far in from the frame's left edge something along the bottom stands clear of the
    /// stick's resting base, and how far up from the bottom that holds.
    pub fn right_of_the_stick(self) -> (f32, f32) {
        (self.across(STICK_EDGE), self.up(STICK_EDGE))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The phone of the task: 844x390, the frame the window's own shape.
    fn phone() -> Room {
        Room { window: (844.0, 390.0), frame: (844.0, 390.0) }
    }

    #[test]
    fn the_unit_is_touch_js_s() {
        assert_eq!(phone().unit(), 48.75);
        assert_eq!(Room { window: (1000.0, 2000.0), frame: (1.0, 1.0) }.unit(), 70.0);
    }

    #[test]
    fn the_places_are_the_pads_on_a_phone() {
        let r = phone();
        let (x, y) = r.left_of_the_pedals();
        assert!((x - 6.8 * 48.75).abs() < 1e-3 && (y - 3.2 * 48.75).abs() < 1e-3, "{x} {y}");
        let (x, _) = r.right_of_the_stick();
        assert!((x - 3.3 * 48.75).abs() < 1e-3, "{x}");
        let y = r.over_the_faces();
        assert!((y - (390.0 - 6.8 * 48.75)).abs() < 1e-3, "{y}");
        assert!((r.left_of_the_faces(y) - 3.1 * 48.75).abs() < 1e-3);
        assert!((r.left_of_the_faces(390.0 - 5.7 * 48.75 + 1.0) - 4.35 * 48.75).abs() < 1e-3);
        assert!((r.right_of_the_top() - (422.0 + 2.75 * 48.75)).abs() < 1e-3);
    }

    #[test]
    fn a_smaller_frame_scales_and_a_barred_one_starts_in_from_the_bars() {
        // half the window's pixels: every place is half as many frame pixels
        let half = Room { window: (844.0, 390.0), frame: (422.0, 195.0) };
        assert!((half.left_of_the_pedals().0 - phone().left_of_the_pedals().0 / 2.0).abs() < 1e-3);
        // 4:3 in 844x390: 520x390 centred, its sides 162 in from the window's
        let barred = Room { window: (844.0, 390.0), frame: (520.0, 390.0) };
        assert!((barred.left_of_the_pedals().0 - (6.8 * 48.75 - 162.0)).abs() < 1e-3);
        assert_eq!(barred.right_of_the_stick().0, 0.0);
        assert!((barred.left_of_the_pedals().1 - 3.2 * 48.75).abs() < 1e-3);
    }

    #[test]
    fn the_window_is_kept_until_the_pad_goes() {
        set(Some((844, 390)));
        assert_eq!(window(), Some((844, 390)));
        assert_eq!(Room::of((844, 390)), Some(phone()));
        set(Some((0, 0)));
        assert_eq!(window(), Some((1, 1)));
        set(None);
        assert_eq!(window(), None);
        assert_eq!(Room::of((844, 390)), None);
    }
}
