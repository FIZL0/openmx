//! A card dragged with the mouse on the Controllers screen (P214, opentennis's B597 `assign_drag.rs`):
//! the left button down on a card and the pointer moved past [`DRAG_EMS`] lifts it, and it follows the
//! pointer over the other cards. The column under the pointer it would go to rings, and letting go
//! there moves the card's device to it as its own Left/Right would: onto an empty seat, or swapping
//! with the device on it, or onto no seat. Let go anywhere else and nothing moves. A press that never
//! moves that far is no drag, and changes nothing.
//!
//! The rules are [`join::Assign::drop_on`]'s, the pads' own: a seat kept for an unplugged pad (P38)
//! takes nobody, an unplugged card is not lifted, and online only this machine's seats are offered.
//! Points are the stage's ems ([`super::screens`]), which [`Menu::mouse`] maps a view's pixels into.

use super::*;

/// What the mouse's left button did: the shell's `window::Mouse`, which the menus' includers may not
/// have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mouse {
    Down,
    Move,
    Up,
}

/// How far, in stage ems, the pointer goes with the button down before a press on a card is a drag.
pub const DRAG_EMS: f32 = 0.6;

/// The card the mouse has hold of.
#[derive(Clone, Debug, PartialEq)]
pub struct CardDrag {
    /// Whose card it is.
    pub dev: Dev,
    /// Where the button went down, and where the pointer is now, in stage ems.
    pub grab: [f32; 2],
    pub at: [f32; 2],
    /// Whether it has moved far enough to be a drag: until it has, the card stays put.
    pub lifted: bool,
}

impl Menu {
    /// The mouse's left button went down, moved while held, or came up at `at` in a `size` view the
    /// Controllers screen is drawn in, in its pixels (P214). Whether it did anything to a card.
    pub fn mouse(&mut self, what: Mouse, at: [f32; 2], size: (usize, usize)) -> bool {
        if self.screen() != Some(Screen::Assign) {
            self.card_drag = None;
            return false;
        }
        let (em, o) = screens::fit(size);
        let p = [(at[0] - o[0]) / em, (at[1] - o[1]) / em];
        match what {
            Mouse::Down => {
                self.card_drag = self.card_at(p).map(|dev| CardDrag { dev, grab: p, at: p, lifted: false });
                self.card_drag.is_some()
            }
            Mouse::Move => {
                let Some(g) = self.card_drag.as_mut() else { return false };
                g.at = p;
                if !g.lifted && (p[0] - g.grab[0]).hypot(p[1] - g.grab[1]) >= DRAG_EMS {
                    g.lifted = true;
                    self.cues.list.push(Cue::Move);
                }
                self.changed = true;
                true
            }
            Mouse::Up => {
                let Some(mut g) = self.card_drag.take() else { return false };
                g.at = p;
                if !g.lifted {
                    return true;
                }
                let to = screens::assign_spots(&self.assign).column_at(p);
                let moved = to.is_some_and(|c| self.assign.drop_on(&g.dev, c));
                self.cues.list.push(if moved { Cue::Accept } else { Cue::Refused });
                self.changed = true;
                true
            }
        }
    }

    /// The plugged-in device whose card is at `p` (stage ems), where it stands now.
    fn card_at(&self, p: [f32; 2]) -> Option<Dev> {
        let a = &self.assign;
        let spots = screens::assign_spots(a);
        let inside = |b: ink::Box4| p[0] >= b[0] && p[1] >= b[1] && p[0] < b[0] + b[2] && p[1] < b[1] + b[3];
        let seated = a.seats.iter().enumerate().filter_map(|(i, d)| Some((d.clone()?, spots.seats[i])));
        let loose = a.loose().into_iter().enumerate().map(|(k, d)| (d, spots.loose_box(k)));
        seated.chain(loose).find(|(d, b)| a.plugged(d) && inside(*b)).map(|(d, _)| d)
    }

    /// The column a lifted card would go to if let go now (P214): 0 for no seat, else a seat plus one.
    /// `None` with no card lifted, or over nowhere it may go.
    pub fn drop_column(&self) -> Option<usize> {
        let g = self.card_drag.as_ref().filter(|g| g.lifted)?;
        let c = screens::assign_spots(&self.assign).column_at(g.at)?;
        self.assign.may_drop(&g.dev, c).then_some(c)
    }

    /// The middle of column `c`'s place for a card in a `size` view, in its pixels (P214): a seat's
    /// box, or no seat's first place. For the tests, which drag a card by it.
    pub fn assign_point(&self, c: usize, size: (usize, usize)) -> Option<[f32; 2]> {
        let spots = screens::assign_spots(&self.assign);
        let b = if c == 0 { spots.loose_box(0) } else { *spots.seats.get(c - 1)? };
        let (em, o) = screens::fit(size);
        Some([o[0] + (b[0] + b[2] / 2.0) * em, o[1] + (b[1] + b[3] / 2.0) * em])
    }

    /// The middle of `d`'s card in a `size` view, in its pixels, where it stands now (P214).
    pub fn card_point(&self, d: &Dev, size: (usize, usize)) -> Option<[f32; 2]> {
        let a = &self.assign;
        let spots = screens::assign_spots(a);
        let b = match a.column(d) {
            0 => spots.loose_box(a.loose().iter().position(|l| l == d)?),
            c => spots.seats[c - 1],
        };
        let (em, o) = screens::fit(size);
        Some([o[0] + (b[0] + b[2] / 2.0) * em, o[1] + (b[1] + b[3] / 2.0) * em])
    }
}
