//! The frame loop's rest while the app is away (P302). Inactive (WillResignActive: the app switcher,
//! Control Center) iOS hands the shell no drawable, so nothing paces its loop: the user's 0.10.3 logs
//! (2026-11-04) show 3700-13556 fps of nothing drawn until iOS suspended the app. While away the loop
//! waits here instead, woken at once by DidBecomeActive.
//!
//! Plain logic, built everywhere, so the desktop's tests prove it; only iOS's lifecycle notices set it.

use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

/// Whether the app is away, and the wake for coming back.
pub struct Rest {
    away: Mutex<bool>,
    back: Condvar,
}

impl Rest {
    pub const fn new() -> Rest {
        Rest { away: Mutex::new(false), back: Condvar::new() }
    }

    /// The app has gone away (WillResignActive or DidEnterBackground).
    pub fn go(&self) {
        *self.away.lock().unwrap_or_else(|e| e.into_inner()) = true;
    }

    /// The app is back (DidBecomeActive): any loop resting is woken.
    pub fn come_back(&self) {
        *self.away.lock().unwrap_or_else(|e| e.into_inner()) = false;
        self.back.notify_all();
    }

    /// Whether the app is away now.
    pub fn is_away(&self) -> bool {
        *self.away.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// While the app is away, wait until it comes back or `most` has gone, whichever is first, so the
    /// loop still polls its window and answers going away now and then. How long it waited: zero when
    /// the app was not away.
    pub fn rest(&self, most: Duration) -> Duration {
        let start = Instant::now();
        let away = self.away.lock().unwrap_or_else(|e| e.into_inner());
        if !*away {
            return Duration::ZERO;
        }
        drop(self.back.wait_timeout_while(away, most, |away| *away).unwrap_or_else(|e| e.into_inner()));
        start.elapsed()
    }
}

impl Default for Rest {
    fn default() -> Rest {
        Rest::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn here_it_does_not_wait() {
        let r = Rest::new();
        assert_eq!(r.rest(Duration::from_secs(5)), Duration::ZERO);
        r.go();
        r.come_back();
        assert!(!r.is_away());
        assert_eq!(r.rest(Duration::from_secs(5)), Duration::ZERO);
    }

    #[test]
    fn away_it_waits_its_most() {
        let r = Rest::new();
        r.go();
        assert!(r.is_away());
        let waited = r.rest(Duration::from_millis(40));
        assert!(waited >= Duration::from_millis(40), "{waited:?}");
        assert!(r.is_away());
    }

    #[test]
    fn coming_back_wakes_it_at_once() {
        let r = Arc::new(Rest::new());
        r.go();
        let other = Arc::clone(&r);
        // Whichever runs first: a wake sent before the wait begins is not lost, as the flag is read
        // under the lock.
        let waiter = std::thread::spawn(move || other.rest(Duration::from_secs(60)));
        r.come_back();
        let waited = waiter.join().unwrap();
        assert!(waited < Duration::from_secs(30), "{waited:?}");
        assert!(!r.is_away());
    }
}
