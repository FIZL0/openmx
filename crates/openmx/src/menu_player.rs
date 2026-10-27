//! The pause's music player (P174): the song playing and three controls — back to the song before,
//! pause or play, and on to the next — on the pause screen itself, so the music changes without
//! Settings > Audio. The pad works them with L1, Square and R1; a mouse click works the one under
//! it. Each takes at once, as Audio's Skip Song does, not when the pause closes.
//!
//! Where it is drawn is the stage's ([`super::screens`]), in its ems; [`Menu::click`] reads a click
//! against the same boxes.

use super::*;

/// One of the player's controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deck {
    /// Back to the song before (L1).
    Back,
    /// Pause the music, or play it on (Square).
    Hold,
    /// On to the next song (R1).
    Skip,
}

impl Deck {
    pub const ALL: [Deck; 3] = [Deck::Back, Deck::Hold, Deck::Skip];
}

/// The player's box on the stage, in ems: under the pause's side panel, over the hint tab.
pub(super) const BOX: [f32; 4] = [34.0, 29.4, 32.0, 6.4];
/// Each control's disc's radius, in ems.
pub(super) const RADIUS: f32 = 1.45;

/// The centre of `d`'s disc on the stage, the three in a row at the box's right.
pub(super) fn centre(d: Deck) -> [f32; 2] {
    let i = Deck::ALL.iter().position(|x| *x == d).unwrap_or(0) as f32;
    [BOX[0] + BOX[2] - 1.6 - RADIUS - (2.0 - i) * 3.7, BOX[1] + 2.9]
}

/// The control a pad press works on the pause, if it is one: L1, Square and R1.
pub(super) fn press(p: Press) -> Option<Deck> {
    if p.l1 {
        Some(Deck::Back)
    } else if p.delete {
        Some(Deck::Hold)
    } else if p.r1 {
        Some(Deck::Skip)
    } else {
        None
    }
}

/// Where `d`'s disc's centre is in a `size` view, in its pixels.
#[allow(dead_code)]
pub fn deck_point(d: Deck, size: (usize, usize)) -> [f32; 2] {
    let (em, o) = screens::fit(size);
    let c = centre(d);
    [o[0] + c[0] * em, o[1] + c[1] * em]
}

/// The control under `at`, a point of a `size` view the pause is drawn over, in its pixels.
pub fn under(at: [f32; 2], size: (usize, usize)) -> Option<Deck> {
    let (em, o) = screens::fit(size);
    let p = [(at[0] - o[0]) / em, (at[1] - o[1]) / em];
    // A little past the disc, so a click on its rim counts.
    let reach = RADIUS * 1.2;
    Deck::ALL.into_iter().find(|d| {
        let c = centre(*d);
        (p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2) <= reach * reach
    })
}

impl Menu {
    /// Whether the player is on the screen: the pause, with no confirm over it.
    pub fn player_shown(&self) -> bool {
        self.screen() == Some(Screen::Pause) && self.leaving.is_none()
    }

    /// Why the player's controls do nothing, if they do not: the music is off.
    pub fn player_off(&self) -> Option<&'static str> {
        (self.settings.music == "off").then_some("The music is off")
    }

    /// Work one of the player's controls: what it asks of the music goes to the shell with the
    /// frame's other tunes ([`Menu::take_tunes`]). Whether it did anything.
    pub(super) fn work_deck(&mut self, d: Deck) -> bool {
        if self.player_off().is_some() {
            return false;
        }
        match d {
            Deck::Back => self.back_song(),
            Deck::Skip => {
                self.music_held = false;
                self.skip_song();
            }
            Deck::Hold => {
                self.music_held = !self.music_held;
                self.tunes.push(Tune::Hold(self.music_held));
            }
        }
        self.changed = true;
        true
    }

    /// A mouse click at `at` in a `size` view the menu is drawn over, in its pixels (P174): the
    /// player's control under it, if the player is up, worked as its pad button works it, with the
    /// sound a press makes. Whether it hit one.
    pub fn click(&mut self, at: [f32; 2], size: (usize, usize)) -> bool {
        if !self.player_shown() {
            return false;
        }
        let Some(d) = under(at, size) else { return false };
        let cue = if self.work_deck(d) { Cue::Accept } else { Cue::Refused };
        self.cues.list.push(cue);
        true
    }
}
