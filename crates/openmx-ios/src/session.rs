//! The audio session's bookkeeping (P296): what the app asks of AVAudioSession at launch, and what its
//! notices mean for the sound — whether the game's music is to be silent for another app's, and when
//! the output has to be started again. Plain logic, built everywhere, so the desktop's tests prove it;
//! the calls themselves are in `sys.rs`.

/// The category set at launch: Playback, which sounds with the ring/silent switch on (the user,
/// 2026-11-04: "make audio play even on silent"), as opentennis's `play_on_silent` sets it. The
/// value of AVFAudio's `AVAudioSessionCategoryPlayback` is its own name.
pub const CATEGORY: &str = "AVAudioSessionCategoryPlayback";

/// `AVAudioSessionCategoryOptionMixWithOthers`: another app's music plays along with ours rather than
/// interrupting it (the default for Playback, and SoloAmbient's, is to stop the other or be stopped).
pub const MIX_WITH_OTHERS: usize = 0x1;

/// The options set with [`CATEGORY`].
pub const OPTIONS: usize = MIX_WITH_OTHERS;

/// The options the session is made active with for a moment when the player takes the music back
/// from another app (P339): none, so ours interrupts the other app's, which stops it; [`OPTIONS`]
/// are set again straight after, so sound from apps started later mixes with ours as before.
pub const TAKE_OVER_OPTIONS: usize = 0;

/// What the session hears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// `AVAudioSessionInterruptionNotification`, began: a call, Siri, an alarm; iOS stops our output.
    InterruptionBegan,
    /// The same notice, ended: the session is to be made active and the output started again.
    InterruptionEnded,
    /// `AVAudioSessionMediaServicesWereResetNotification`: every audio object is gone.
    ServicesReset,
    /// The secondary-audio hint's notice (`AVAudioSessionSilenceSecondaryAudioHintNotification`):
    /// another app began playing, or has stopped.
    OtherAudio(bool),
    /// The `secondaryAudioShouldBeSilencedHint` property read (launch, the app back, the session made
    /// active). After a take-over its `true` is passed over, as it may still be the app just stopped
    /// (P339): only a new notice holds the music again.
    HintRead(bool),
    /// `UIApplicationDidEnterBackgroundNotification`.
    Background,
    /// The player took the music back (P339): the other app's is stopped, and the output is started
    /// again in case changing the category stopped ours.
    TookOver,
    /// `UIApplicationDidBecomeActiveNotification`.
    Active,
}

/// The session as its notices leave it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Session {
    /// An interruption began and its end has not come (iOS need not send it, for one).
    interrupted: bool,
    /// The app went to the background since it was last active.
    away: bool,
    /// Another app's music is playing: ours is silent, the rest of the game's sound is not.
    pub other_audio: bool,
    /// The player took the music back (P339), and no notice of another app's has come since.
    took_over: bool,
    /// How many times the output is to have been started again; the output side compares it with
    /// the count it has done.
    pub restarts: usize,
}

impl Session {
    /// Launch: nothing heard yet.
    pub const START: Session = Session { interrupted: false, away: false, other_audio: false, took_over: false, restarts: 0 };

    pub fn on(&mut self, event: Event) {
        match event {
            Event::InterruptionBegan => self.interrupted = true,
            Event::InterruptionEnded | Event::ServicesReset => {
                self.interrupted = false;
                self.restarts += 1;
            }
            Event::OtherAudio(on) => {
                self.other_audio = on;
                self.took_over = false;
            }
            Event::HintRead(on) => self.other_audio = on && !self.took_over,
            Event::Background => self.away = true,
            Event::TookOver => {
                self.other_audio = false;
                self.took_over = true;
                self.restarts += 1;
            }
            // Coming back after an interruption whose end never came, or from the background, starts
            // the output again, so the sound always returns; a pull of Control Centre (resign, then
            // active) leaves it be.
            Event::Active => {
                if self.interrupted || self.away {
                    self.interrupted = false;
                    self.away = false;
                    self.restarts += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Event::*, *};

    #[test]
    fn launch_asks_for_playback_mixing_with_others() {
        assert_eq!(CATEGORY, "AVAudioSessionCategoryPlayback");
        assert_eq!(OPTIONS & MIX_WITH_OTHERS, MIX_WITH_OTHERS);
        assert_eq!(OPTIONS, 0x1, "MixWithOthers alone: no ducking, no Bluetooth or AirPlay flags");
    }

    #[test]
    fn another_apps_music_silences_ours_until_it_stops() {
        let mut s = Session::default();
        s.on(OtherAudio(true));
        assert!(s.other_audio);
        assert_eq!(s.restarts, 0, "a hint does not touch the output");
        s.on(OtherAudio(false));
        assert!(!s.other_audio);
    }

    #[test]
    fn taking_the_music_back_ends_the_hint_and_restarts_the_output() {
        assert_eq!(TAKE_OVER_OPTIONS & MIX_WITH_OTHERS, 0, "not mixing, so the other app is stopped");
        let mut s = Session::default();
        s.on(OtherAudio(true));
        s.on(TookOver);
        assert!(!s.other_audio);
        assert_eq!(s.restarts, 1);
        // the output's restart reads the hint again: a stale `true` does not hold the music again
        s.on(HintRead(true));
        assert!(!s.other_audio);
        s.on(Active);
        s.on(HintRead(true));
        assert!(!s.other_audio);
        // the other app's music started again later: its notice holds ours once more
        s.on(OtherAudio(true));
        assert!(s.other_audio);
        s.on(HintRead(true));
        assert!(s.other_audio, "and with no take-over since, a read says so too");
        s.on(HintRead(false));
        assert!(!s.other_audio);
    }

    #[test]
    fn an_interruptions_end_restarts_the_output() {
        let mut s = Session::default();
        s.on(InterruptionBegan);
        assert_eq!(s.restarts, 0);
        s.on(InterruptionEnded);
        assert_eq!(s.restarts, 1);
        // its end seen, coming back to the app does not start it a second time
        s.on(Active);
        assert_eq!(s.restarts, 1);
        s.on(ServicesReset);
        assert_eq!(s.restarts, 2);
    }

    #[test]
    fn coming_back_restarts_it_when_the_end_never_came_or_the_app_was_away() {
        let mut s = Session::default();
        s.on(Active);
        assert_eq!(s.restarts, 0, "a plain resign and back leaves the sound be");
        s.on(InterruptionBegan);
        s.on(Active);
        assert_eq!(s.restarts, 1);
        s.on(Background);
        s.on(Active);
        assert_eq!(s.restarts, 2);
        s.on(Active);
        assert_eq!(s.restarts, 2);
    }
}
