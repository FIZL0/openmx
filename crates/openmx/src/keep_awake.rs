//! The phone kept from locking itself while anything downloads (F83): a locked phone stops the
//! downloads. Every download — a store install from its Install press on (queued or going), the app
//! update's, Android's disc copied in — holds a [`Keep`] until it ends, fails or is dropped. The
//! first one turns the phone's auto-lock off, the last one gone turns it back on. Nothing on a
//! desktop or in the browser.

use std::sync::{Mutex, PoisonError};

/// The app's downloads, over the phone's auto-lock.
pub static AWAKE: Awake = Awake::new(screen);

/// How many downloads hold the screen awake, and what turns its auto-lock off (`true`) and back on.
pub struct Awake {
    held: Mutex<usize>,
    screen: fn(bool),
}

impl Awake {
    pub const fn new(screen: fn(bool)) -> Awake {
        Awake { held: Mutex::new(0), screen }
    }

    /// A download starts: awake until the [`Keep`] is dropped.
    pub fn keep(&'static self) -> Keep {
        // the screen is told under the lock: the last end and the next start can't swap
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        *held += 1;
        if *held == 1 {
            (self.screen)(true);
        }
        Keep(self)
    }

    /// Whether any download holds it.
    #[cfg(test)]
    pub fn on(&self) -> bool {
        *self.held.lock().unwrap_or_else(PoisonError::into_inner) > 0
    }
}

/// One download under way.
pub struct Keep(&'static Awake);

impl Drop for Keep {
    fn drop(&mut self) {
        let mut held = self.0.held.lock().unwrap_or_else(PoisonError::into_inner);
        *held -= 1;
        if *held == 0 {
            (self.0.screen)(false);
        }
    }
}

/// The phone's auto-lock off (`awake`) or back to normal.
fn screen(awake: bool) {
    #[cfg(any(target_os = "ios", target_os = "android"))]
    println!("screen: {}", if awake { "kept awake for a download" } else { "auto-lock back" });
    #[cfg(target_os = "ios")]
    openmx_ios::keep_awake(awake);
    #[cfg(target_os = "android")]
    openmx_android::keep_awake(awake);
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    let _ = awake;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_screen_is_told_at_the_first_start_and_the_last_end() {
        static TOLD: Mutex<Vec<bool>> = Mutex::new(Vec::new());
        static AWAKE: Awake = Awake::new(|on| TOLD.lock().unwrap().push(on));
        let told = || TOLD.lock().unwrap().clone();
        assert!(!AWAKE.on());
        let a = AWAKE.keep();
        assert_eq!(told(), [true]);
        let b = AWAKE.keep();
        drop(a);
        assert!(AWAKE.on());
        assert_eq!(told(), [true], "one still going");
        drop(b);
        assert!(!AWAKE.on());
        assert_eq!(told(), [true, false]);
        // the next batch, whose one download dies on its thread
        let c = AWAKE.keep();
        assert_eq!(told(), [true, false, true]);
        assert!(std::thread::spawn(move || { let _c = c; panic!("a download that panics") }).join().is_err());
        assert_eq!(told(), [true, false, true, false]);
    }
}
