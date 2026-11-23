//! Which way up the game is held (M144, B694 in opentennis): landscape only (F83), either side with
//! the device, and one side while rotation lock is on.
//!
//! An iPhone's rotation lock makes UIDevice report portrait however the phone is held, so portrait
//! the accelerometer does not agree with is the lock going on, not the phone held upright. Once the
//! device has said portrait for a second while the accelerometer says the phone is sideways, that is
//! the lock, and the game turns to landscape with the camera / Dynamic Island on the left
//! ([`Hold::Locked`], UIKit's `LandscapeRight`) and stays there, held upright or put down and
//! picked up included, until the device reports a landscape turn the accelerometer agrees with (the
//! lock off). With the lock off the game follows the device between the two landscape sides, and
//! held upright it stays on the side it is on.
//!
//! The next launch starts that way too (opentennis 013ae90e): iOS cannot say whether the lock is
//! still on, so a lock held last run ([`Holder::remembering`], a `rotation-lock` marker file) starts
//! [`Hold::Locked`]; with the lock off since, until the phone is turned sideways once.

/// What the window's view controller is held to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hold {
    /// Either landscape side, following the device.
    Landscape,
    /// Rotation lock on: landscape with the camera on the left only.
    Locked,
}

/// UIDevice's orientations that say something: portrait, upside down, landscape left and right.
/// Face up, face down and unknown (0, 5, 6) say nothing.
fn turned(device: isize) -> Option<bool> {
    (1..=4).contains(&device).then_some(device >= 3)
}

/// [`Holder::step`]'s memory: the hold, the device's last reported orientation and since when, and
/// since when the device has said portrait while the accelerometer says sideways.
#[derive(Default, Debug)]
pub struct Holder {
    held: Option<Hold>,
    device: isize,
    since: f32,
    across: Option<f32>,
    /// The lock was held when the last run ended.
    was_locked: bool,
}

impl Holder {
    /// A holder for a launch after one that ended with the lock held (`was_locked`) or not.
    pub fn remembering(was_locked: bool) -> Self {
        Holder { was_locked, ..Default::default() }
    }

    /// One look, `now` seconds into the run: UIDevice's `orientation` (its raw number), and whether
    /// the accelerometer has the phone sideways (`None` lying flat or with no accelerometer). A new
    /// hold when it changes.
    pub fn step(&mut self, now: f32, device: isize, gravity: Option<bool>) -> Option<Hold> {
        if device != self.device {
            self.device = device;
            self.since = now;
        }
        let turned = turned(device);
        // Rotation lock: portrait from the device with the phone held sideways, for longer than a
        // turn takes to report.
        if turned == Some(false) && gravity == Some(true) {
            self.across.get_or_insert(now);
        } else {
            self.across = None;
        }
        let locked = self.across.is_some_and(|t| now - t >= 1.0);
        // The accelerometer has a moment to agree with a new orientation: turning the phone past
        // 45° fires it. Under the lock the device only ever says portrait (or face up/down: put
        // down and picked up again), so only a landscape report, the lock off, lets go of `Locked`.
        let want = match (self.held, turned) {
            (held, _) if locked => (held != Some(Hold::Locked)).then_some(Hold::Locked),
            // The lock was on last time: iOS can't say whether it still is, so start as it was.
            (None, _) if self.was_locked => Some(Hold::Locked),
            (None, _) => Some(Hold::Landscape),
            (Some(Hold::Locked), Some(true)) if now - self.since < 0.5 && gravity.is_none_or(|g| g) => Some(Hold::Landscape),
            _ => None,
        };
        if want.is_some() {
            self.held = want;
        }
        want
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PORTRAIT: isize = 1;
    const LEFT: isize = 3;
    const RIGHT: isize = 4;
    const FLAT: isize = 5;

    #[test]
    fn starts_landscape_however_it_is_held() {
        assert_eq!(Holder::default().step(0.0, PORTRAIT, Some(false)), Some(Hold::Landscape));
        assert_eq!(Holder::default().step(0.0, LEFT, Some(true)), Some(Hold::Landscape));
    }

    /// F83: held upright, upside down or flat, the lock on last time or not, it asks for nothing but
    /// the two landscape holds, and the device upright is no turn at all.
    #[test]
    fn never_answers_portrait() {
        for was_locked in [false, true] {
            for device in 0..=6 {
                for gravity in [None, Some(false), Some(true)] {
                    let mut h = Holder::remembering(was_locked);
                    h.step(0.0, LEFT, Some(true));
                    for k in 1..40 {
                        let got = h.step(k as f32 * 0.1, device, gravity);
                        assert!(matches!(got, None | Some(Hold::Landscape | Hold::Locked)), "{got:?}");
                        assert!(got.is_none() || device >= 3 || gravity == Some(true), "{device} {gravity:?}: {got:?}");
                    }
                }
            }
        }
    }

    /// F83: the apps list no portrait for the system to turn to.
    #[test]
    fn the_apps_allow_landscape_only() {
        let plist = include_str!("../../../tools/ios-ipa.sh");
        assert!(plist.contains("UIInterfaceOrientationLandscapeLeft") && !plist.contains("UIInterfaceOrientationPortrait"));
        assert!(include_str!("../../../tools/apk.sh").contains("android:screenOrientation=\"sensorLandscape\""));
    }

    #[test]
    fn a_turn_the_accelerometer_disputes_is_ignored() {
        let mut h = Holder::default();
        h.step(0.0, LEFT, Some(true));
        // The lock goes on: the device says portrait, the phone is still sideways.
        assert_eq!(h.step(1.0, PORTRAIT, Some(true)), None);
        assert_eq!(h.step(1.5, PORTRAIT, Some(true)), None);
    }

    #[test]
    fn rotation_lock_holds_the_camera_left_landscape() {
        let mut h = Holder::default();
        // Locked in portrait from the start, the phone held sideways.
        assert_eq!(h.step(0.0, PORTRAIT, Some(true)), Some(Hold::Landscape));
        assert_eq!(h.step(0.9, PORTRAIT, Some(true)), None);
        assert_eq!(h.step(1.0, PORTRAIT, Some(true)), Some(Hold::Locked));
        // Held upright again under the lock: it stays.
        assert_eq!(h.step(3.0, PORTRAIT, Some(false)), None);
        assert_eq!(h.step(4.0, PORTRAIT, Some(true)), None);
        // The lock off: the device reports a turn the accelerometer agrees with.
        assert_eq!(h.step(6.0, LEFT, Some(true)), Some(Hold::Landscape));
    }

    #[test]
    fn face_up_then_picked_up_keeps_the_side() {
        let mut h = Holder::default();
        h.step(0.0, PORTRAIT, Some(true));
        assert_eq!(h.step(1.0, PORTRAIT, Some(true)), Some(Hold::Locked));
        // Put down face up, then picked up and held upright: the device says portrait again, fresh,
        // and the accelerometer agrees, but under the lock that is no turn.
        assert_eq!(h.step(3.0, FLAT, None), None);
        assert_eq!(h.step(5.0, PORTRAIT, Some(false)), None);
        assert_eq!(h.step(5.2, PORTRAIT, Some(false)), None);
        // Sideways again: still held.
        assert_eq!(h.step(6.0, PORTRAIT, Some(true)), None);
        // The lock off: a landscape turn lets go.
        assert_eq!(h.step(8.0, RIGHT, Some(true)), Some(Hold::Landscape));
    }

    #[test]
    fn a_lock_from_the_last_run_starts_locked() {
        // Launched held upright, the lock remembered: its side from the first look, before the
        // accelerometer could tell.
        let mut h = Holder::remembering(true);
        assert_eq!(h.step(0.0, PORTRAIT, Some(false)), Some(Hold::Locked));
        assert_eq!(h.step(0.2, PORTRAIT, Some(false)), None);
        assert_eq!(h.step(2.0, PORTRAIT, Some(true)), None);
        // The lock was turned off since: the first sideways turn lets go, and upright changes nothing.
        assert_eq!(h.step(4.0, LEFT, Some(true)), Some(Hold::Landscape));
        assert_eq!(h.step(6.0, PORTRAIT, Some(false)), None);
    }

    #[test]
    fn a_slow_report_is_not_a_turn() {
        let mut h = Holder::remembering(true);
        h.step(0.0, PORTRAIT, Some(false));
        h.step(1.0, LEFT, Some(false));
        // The accelerometer only agrees 0.6 s after the report: too late.
        assert_eq!(h.step(1.6, LEFT, Some(true)), None);
    }
}
