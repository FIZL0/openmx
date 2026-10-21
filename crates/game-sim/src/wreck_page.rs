//! **The wreck page** (S72): what the ride state machine's [`crate::state::Reset::Page`] asks for,
//! and the put-back it hands back three ticks later.
//!
//! The "page" is not a page. The reset request, given a page and an owner with a screen, registers
//! the bike as a listener on its screen's viewport fader and starts that fader's fade to black: 100 ms
//! out, then 100 ms back in (S57 named the two calls). The bike is put back by the fader, not by the
//! machine: the fader's draw, the tick its alpha reaches 255, tells its listeners the screen is black,
//! and the bike's listener is the respawn. The fade back in starts on the same draw.
//!
//! Measured live (`research/s72_page.py`, `watch/s72-page.toml`): the fader's update runs after the
//! bike's in the tick, so the request's own tick is the first of the fade's three; each update adds
//! the front end's own frame time, whole milliseconds (34 and 33 at 30 Hz, so any three make 100 or
//! 101 ms), and the third makes t = 0.99999994 or more, which draws as 255. So the respawn comes at
//! the end of the second tick after the request, and the first tick the bike rides is the third: the
//! S59 captures' "put back 3 rows after the page", every time. The fade back in is three more updates,
//! and its draw at 0 lets go of the screen.
//!
//! While the fade runs the screen counts as having a page up — the machine's query reads the fader's
//! "fading out" and "held" words, and "held" is on from the black draw to the end of the fade back
//! in — so the machine asks no second time. The port counts the fader's updates rather than its
//! milliseconds: the simulation's tick is a fixed 30 Hz, and three updates reach black whichever way
//! the 34/33 rounding falls.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::hash::{State, Walk};

/// Where the fade is.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Fade {
    /// No page: the fader is idle.
    #[default]
    Off,
    /// Fading to black. The bike is still down.
    Out,
    /// Black reached and the bike put back; fading in again. The screen is still held.
    In,
}

/// One rider's screen's wreck page: its viewport fader, as far as the simulation sees it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Page {
    /// Where the fade is.
    pub fade: Fade,
    /// The fader's updates since this half of the fade began.
    pub updates: u8,
}

impl Page {
    /// The fader's updates in each half: 100 ms at the front end's 34 or 33 ms a tick.
    pub const UPDATES: u8 = 3;

    /// The request with a page: the fade to black starts from nothing. The original's start resets
    /// the fader whatever it was doing, which the machine never lets happen while one runs.
    pub fn raise(&mut self) {
        *self = Page { fade: Fade::Out, updates: 0 };
    }

    /// Whether the screen has a page up: what the ride state machine asks before raising one.
    pub fn up(&self) -> bool {
        self.fade != Fade::Off
    }

    /// One tick of the fader, after the bike's update, with its draw. Answers whether this tick's
    /// draw put the bike back.
    pub fn update(&mut self) -> bool {
        match self.fade {
            Fade::Off => false,
            Fade::Out => {
                self.updates += 1;
                if self.updates < Self::UPDATES {
                    return false;
                }
                *self = Page { fade: Fade::In, updates: 0 };
                true
            }
            Fade::In => {
                self.updates += 1;
                if self.updates >= Self::UPDATES {
                    *self = Page::default();
                }
                false
            }
        }
    }
}

impl State for Page {
    fn walk(&self, w: &mut Walk<'_>) {
        w.u8("fade", self.fade as u8);
        w.u8("updates", self.updates);
    }
}
