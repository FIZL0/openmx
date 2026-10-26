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
//!
//! **The fade itself** (S85) is [`Shade`]: the picture's side of the fader, which the simulation never
//! reads. It follows a [`Page`] a tick at a time with the front end's whole-millisecond frame time and
//! gives the alpha of the black the original draws over the viewport that tick.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::hash::{State, Walk};
use ps2_float::F;

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

    /// Whether this tick's [`Page::update`] will put the bike back: the fade out's last update (S108,
    /// for the bones, which the original steps after the fader).
    pub fn puts_back_next(&self) -> bool {
        self.fade == Fade::Out && self.updates + 1 >= Self::UPDATES
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

/// **The fader's picture** (S85): the viewport's black over a page, as the original's fader updates
/// and draws it, kept beside the [`Page`] and never read by the simulation, so no hash or snapshot has
/// it.
///
/// The original's fader (S72's journal; the update and the draw in `research/oracle.txt`) holds its
/// elapsed seconds, its duration and t. Each update while t < 1 adds the frame's time, whole
/// milliseconds as a float of seconds, to the elapsed, and t = min(elapsed / duration, 1). The draw
/// after it is black at alpha `(int)(255 t + 0.5)` fading out, and `(int)(255 - 255 t + 0.5)` fading
/// in. The fade out's draw at 255 puts the bike back and starts the fade in from elapsed 0. The fade
/// in's draw at 0 lets go of the screen.
///
/// The port's [`Page`] decides when the bike is put back: its third update, which three ticks of the
/// front end's 33 and 34 ms always reach. So the shade follows it, and does not lead it. The draw of
/// [`Fade::In`] with no updates is the black one, 255, whatever the milliseconds summed to.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shade {
    /// The page as the last step saw it. A step that finds it unchanged had no fader update.
    seen: Page,
    /// The elapsed seconds of the fade's current half.
    elapsed: F,
    /// t of the current half, `0..=1`.
    t: F,
    /// The last draw's alpha.
    alpha: u8,
}

impl Shade {
    /// Each half's duration: the original's `(float)100 / 1000.0`.
    pub const DURATION: F = F(0.1);

    /// One tick, after the page's update: `ms` is the tick's frame time in whole milliseconds. A
    /// tick where the page did not move (no page, or the rider not stepped) changes nothing.
    pub fn step(&mut self, page: &Page, ms: i32) {
        if *page == self.seen {
            return;
        }
        let half_began = page.fade != self.seen.fade || page.updates < self.seen.updates;
        self.seen = *page;
        if half_began {
            (self.elapsed, self.t) = (F::ZERO, F::ZERO);
        }
        match (page.fade, page.updates) {
            // The fade in's last update reached t = 1, and its draw at 0 let go.
            (Fade::Off, _) => {
                (self.elapsed, self.t, self.alpha) = (F::ZERO, F::ZERO, 0);
            }
            // The black draw, which started the fade in from nothing.
            (Fade::In, 0) => self.alpha = 255,
            (Fade::Out, 0) => self.alpha = 0,
            (fade, _) => {
                if self.t < F::ONE {
                    (self.elapsed, self.t) = Self::update(self.elapsed, ms);
                }
                self.alpha = match fade {
                    Fade::Out => Self::out_alpha(self.t),
                    _ => Self::in_alpha(self.t),
                };
            }
        }
    }

    /// The fader's update: `ms` added as seconds, and t off the new elapsed.
    pub fn update(elapsed: F, ms: i32) -> (F, F) {
        let elapsed = elapsed + F::from(ms) * F(0.001);
        (elapsed, (elapsed / Self::DURATION).min(F::ONE))
    }

    /// The fade out's draw at `t`.
    pub fn out_alpha(t: F) -> u8 {
        (F(0.5) + F(255.0) * t).to_i32() as u8
    }

    /// The fade in's draw at `t`.
    pub fn in_alpha(t: F) -> u8 {
        (F(0.5) + (F(255.0) - F(255.0) * t)).to_i32() as u8
    }

    /// The black's alpha over the viewport now: 0 with no page.
    pub fn alpha(&self) -> u8 {
        self.alpha
    }

    /// The elapsed seconds of the fade's current half, and its t.
    pub fn at(&self) -> (F, F) {
        (self.elapsed, self.t)
    }
}

impl State for Page {
    fn walk(&self, w: &mut Walk<'_>) {
        w.u8("fade", self.fade as u8);
        w.u8("updates", self.updates);
    }
}
