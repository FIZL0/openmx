//! Text chat in the lobby and the ride (opentennis's `hst-base/src/chat.rs`, M20: its cleaning and rate limit; the
//! box that types and shows it is the front end's). A line goes over the peer connection as `Msg::Chat` through the
//! host, which cleans it, rate-limits it and passes it on with the sender's lobby name. It never enters the ride's
//! input, so the simulation and the replays never see it.

use n0_future::time::{Duration, Instant};

/// A line's most characters.
pub const MAX: usize = 80;
/// At most this many lines per peer in any [`WINDOW`].
pub const BURST: usize = 4;
pub const WINDOW: Duration = Duration::from_secs(8);

/// `text` as a chat line: control and bidi-override characters out, spaces trimmed, at most [`MAX`] characters;
/// `None` if nothing is left.
pub fn clean(text: &str) -> Option<String> {
    let bidi = |c: char| matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{200e}' | '\u{200f}');
    let s: String = text.chars().map(|c| if c.is_whitespace() { ' ' } else { c }).filter(|&c| !c.is_control() && !bidi(c)).collect();
    let s: String = s.trim().chars().take(MAX).collect();
    (!s.is_empty()).then_some(s)
}

/// Whether one more line may go now, `sent` being that peer's recent lines' times (kept to the window; `now` added
/// when it may).
pub fn allow(sent: &mut Vec<Instant>, now: Instant) -> bool {
    sent.retain(|t| now.saturating_duration_since(*t) < WINDOW);
    let ok = sent.len() < BURST;
    if ok {
        sent.push(now);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_cleaned_and_limited() {
        assert_eq!(clean("  hi\tthere\u{202e}\u{7}  ").as_deref(), Some("hi there"));
        assert_eq!(clean(" \n "), None);
        assert_eq!(clean(&"x".repeat(200)).map(|s| s.len()), Some(MAX));
        let t = Instant::now();
        let mut sent = Vec::new();
        assert!((0..BURST).all(|_| allow(&mut sent, t)));
        assert!(!allow(&mut sent, t + Duration::from_secs(1)));
        assert!(allow(&mut sent, t + WINDOW));
    }
}
