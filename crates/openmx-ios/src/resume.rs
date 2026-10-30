//! The tap a controller waits for: after a screenshot (side + volume) iOS's preview takes the
//! system's event focus, and GameController reports every button let go and the sticks centred until
//! a touch on the screen gives focus back. The app cannot take it back itself, so it asks for the tap.
//!
//! [`Resume`] is the decision alone, built everywhere so the desktop's tests prove it: a notice (a
//! screenshot, the app resigning or becoming active) with a controller connected asks for the tap;
//! the first touch, or the first controller press that comes through, answers it.

/// Whether the screen is asking for a tap to give a controller its input back.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Resume {
    waiting: bool,
    /// A read with every button up since the notice: a press after it is a new one that came
    /// through, not a button still held from before.
    released: bool,
}

impl Resume {
    /// Not asking: what the app starts with.
    pub const fn new() -> Resume {
        Resume { waiting: false, released: false }
    }

    /// A notice that can leave the controllers without focus, with `controllers` connected.
    pub fn notice(&mut self, controllers: usize) {
        if controllers > 0 {
            *self = Resume { waiting: true, released: false };
        }
    }

    /// A finger on the screen.
    pub fn touch(&mut self) {
        *self = Resume::new();
    }

    /// The controllers as read: whether any button is down.
    pub fn pads(&mut self, any_down: bool) {
        if !self.waiting {
            return;
        }
        if !any_down {
            self.released = true;
        } else if self.released {
            *self = Resume::new();
        }
    }

    /// Whether to draw "Tap screen to resume controller".
    pub fn needed(&self) -> bool {
        self.waiting
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_screenshot_with_a_controller_asks_for_the_tap() {
        let mut r = Resume::default();
        assert!(!r.needed());
        r.notice(1);
        assert!(r.needed());
    }

    #[test]
    fn with_no_controller_it_stays_unset() {
        let mut r = Resume::default();
        r.notice(0);
        assert!(!r.needed());
        r.pads(false);
        r.pads(true);
        assert!(!r.needed());
    }

    #[test]
    fn a_touch_clears_it() {
        let mut r = Resume::default();
        r.notice(2);
        r.touch();
        assert!(!r.needed());
    }

    #[test]
    fn a_pad_press_that_comes_through_clears_it() {
        let mut r = Resume::default();
        r.notice(1);
        // GameController reports everything let go while it has no focus.
        r.pads(false);
        r.pads(false);
        assert!(r.needed());
        r.pads(true);
        assert!(!r.needed());
    }

    #[test]
    fn a_button_still_held_from_before_the_notice_does_not_clear_it() {
        let mut r = Resume::default();
        r.notice(1);
        r.pads(true);
        assert!(r.needed());
        r.pads(false);
        r.pads(true);
        assert!(!r.needed());
    }

    #[test]
    fn a_second_notice_asks_again() {
        let mut r = Resume::default();
        r.notice(1);
        r.touch();
        r.notice(1);
        assert!(r.needed());
    }
}
