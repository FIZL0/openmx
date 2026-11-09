//! The activity going away and coming back (P311), as the iPhone's resign/background counts are
//! (P298, `openmx-ios`): the activity's Pause counts one going away for the shell to answer by
//! pausing the ride and saving the settings, its Stop (no longer seen) one entering the background,
//! which gives the ride's GPU models back. While it is paused the sound stands still
//! (`audio.rs`'s cpal stream paused), so leaving the app does not play on like a music player.
//!
//! The patched winit's lifecycle hook (`android.rs`) calls [`pause`], [`stop`] and [`resume`]; the
//! shell reads the rest from any thread. Plain logic, built everywhere and tested on the desktop.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// The counts, for the process.
struct Away {
    paused: AtomicBool,
    stopped: AtomicBool,
    away: AtomicUsize,
    answered: AtomicUsize,
    backgrounds: AtomicUsize,
}

impl Away {
    const fn new() -> Away {
        Away {
            paused: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
            away: AtomicUsize::new(0),
            answered: AtomicUsize::new(0),
            backgrounds: AtomicUsize::new(0),
        }
    }

    fn pause(&self) -> bool {
        let newly = !self.paused.swap(true, Ordering::Relaxed);
        if newly {
            self.away.fetch_add(1, Ordering::Relaxed);
        }
        newly
    }

    fn stop(&self) -> bool {
        let newly = self.pause();
        if !self.stopped.swap(true, Ordering::Relaxed) {
            self.backgrounds.fetch_add(1, Ordering::Relaxed);
            // asked again, as iOS's DidEnterBackground is: the Pause's answer came before this
            // background, whose models the shell gives back only when it answers
            if !newly {
                self.away.fetch_add(1, Ordering::Relaxed);
            }
        }
        newly
    }

    fn resume(&self) -> bool {
        self.stopped.store(false, Ordering::Relaxed);
        self.paused.swap(false, Ordering::Relaxed)
    }

    fn away(&self) -> Option<usize> {
        let asked = self.away.load(Ordering::Relaxed);
        (asked > self.answered.load(Ordering::Relaxed)).then_some(asked)
    }

    fn answered(&self, asked: usize) {
        self.answered.fetch_max(asked, Ordering::Relaxed);
    }
}

static AWAY: Away = Away::new();

/// The activity paused (home, recents, the screen off, the file picker or a dialog over it): one
/// going away, once until it is resumed. Whether it was in front till now.
pub fn pause() -> bool {
    AWAY.pause()
}

/// The activity stopped, no longer seen: gone away if it was not already, and one entering the
/// background (asked again, for the shell to give the models back), once until it is resumed. Whether it was in front till now.
pub fn stop() -> bool {
    AWAY.stop()
}

/// The activity in front again. Whether it was away till now.
pub fn resume() -> bool {
    AWAY.resume()
}

/// Whether the activity is away now (paused or stopped, not yet resumed).
pub fn paused() -> bool {
    AWAY.paused.load(Ordering::Relaxed)
}

/// The activity has gone away since the shell last answered: the count to hand [`answered`] once
/// the ride is paused and the settings saved.
pub fn away() -> Option<usize> {
    AWAY.away()
}

/// The shell has done what going away asks, up to [`away`]'s count `asked`.
pub fn answered(asked: usize) {
    AWAY.answered(asked)
}

/// How many times the activity has entered the background since the app opened.
pub fn backgrounds() -> usize {
    AWAY.backgrounds.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::Away;

    /// A trip to the home screen is Pause then Stop, and back Start then Resume: the Pause asks
    /// (pause the ride, save), the Stop asks again (give the models back), as iOS's resign and
    /// background do; a dialog over the app is a Pause alone.
    #[test]
    fn a_pause_then_a_stop_ask_twice_and_a_dialog_once() {
        use std::sync::atomic::Ordering::Relaxed;
        let a = Away::new();
        assert_eq!(a.away(), None);
        assert!(a.pause());
        assert_eq!(a.away(), Some(1));
        a.answered(1);
        assert!(!a.stop(), "the stop after a pause is not a new going away");
        assert_eq!((a.away(), a.backgrounds.load(Relaxed)), (Some(2), 1), "but the background asks again");
        a.answered(2);
        assert_eq!(a.away(), None, "answered, nothing more to do while it stays away");
        assert!(!a.pause() && !a.stop(), "a second Pause or Stop while away counts nothing");
        assert_eq!((a.away(), a.backgrounds.load(Relaxed)), (None, 1));
        assert!(a.resume());
        assert!(!a.resume(), "a second Resume is not a return");
        assert!(a.pause(), "a dialog over the app");
        assert_eq!(a.away(), Some(3));
        assert_eq!(a.backgrounds.load(Relaxed), 1, "a pause alone is not the background");
        a.answered(3);
        a.resume();
        assert!(a.stop(), "a Stop with no Pause before it goes away too");
        assert_eq!((a.away(), a.backgrounds.load(Relaxed)), (Some(4), 2), "once");
    }

    /// An answer for an older count does not answer a newer one.
    #[test]
    fn an_old_answer_leaves_a_newer_going_away_asked() {
        let a = Away::new();
        a.pause();
        let asked = a.away().unwrap();
        a.resume();
        a.pause();
        a.answered(asked);
        assert_eq!(a.away(), Some(2));
        a.answered(1);
        assert_eq!(a.away(), Some(2), "answers only go up");
    }
}
