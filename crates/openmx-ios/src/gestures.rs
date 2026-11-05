//! What UIKit reads from the window when a swipe comes in from an edge (P305), and its log line.

use std::fmt;

/// The deferred edges and home indicator the view controller UIKit asks reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gestures {
    /// `UIRectEdge` bits it defers: top 1, left 2, bottom 4, right 8.
    pub deferred: usize,
    /// Whether it asks for the home indicator to be hidden.
    pub indicator_hidden: bool,
    /// Whether it is winit's view controller, the one the window's attributes went to.
    pub ours: bool,
}

impl Gestures {
    /// Whether a first swipe up from the home bar stays in the game: the bottom edge deferred,
    /// and the indicator not hidden (iOS ignores the deferral while it is).
    pub fn holds_the_home_bar(self) -> bool {
        self.deferred & 4 != 0 && !self.indicator_hidden
    }
}

impl fmt::Display for Gestures {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let edges: Vec<_> = [(4, "bottom"), (1, "top"), (2, "left"), (8, "right")]
            .into_iter()
            .filter(|&(bit, _)| self.deferred & bit != 0)
            .map(|(_, name)| name)
            .collect();
        let edges = if edges.is_empty() { "no edge".to_string() } else { edges.join(", ") };
        write!(f, "system gestures deferred on {edges}")?;
        write!(f, ", home indicator {}", if self.indicator_hidden { "hidden" } else { "shown" })?;
        if !self.ours {
            write!(f, " (asked of another view controller than the window's)")?;
        }
        if !self.holds_the_home_bar() {
            write!(f, ": a first swipe up leaves")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_says_which_edges_and_whether_the_home_bar_holds() {
        let all = Gestures { deferred: 15, indicator_hidden: false, ours: true };
        assert_eq!(all.to_string(), "system gestures deferred on bottom, top, left, right, home indicator shown");
        assert!(all.holds_the_home_bar());
        let hidden = Gestures { indicator_hidden: true, ..all };
        assert_eq!(
            hidden.to_string(),
            "system gestures deferred on bottom, top, left, right, home indicator hidden: a first swipe up leaves"
        );
        let none = Gestures { deferred: 0, indicator_hidden: false, ours: false };
        assert_eq!(
            none.to_string(),
            "system gestures deferred on no edge, home indicator shown (asked of another view controller than the window's): a first swipe up leaves"
        );
        assert!(!Gestures { deferred: 11, ..all }.holds_the_home_bar());
    }
}
