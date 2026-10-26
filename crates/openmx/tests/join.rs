//! P38 — the join screen, driven by pad input: join, move and leave, the split screen's count
//! following, and a seat kept for its pad across an unplug.
//!
//! The presses come in as the game's own button words (two consecutive frames' [`Report::buttons`])
//! and go through [`join::presses`], so what is tested is the screen as a pad drives it, not its
//! methods called by hand. The devices are strings standing for a pad's identity or the keyboard.

#[allow(dead_code)]
#[path = "../src/join.rs"]
mod join;

use game_sim::input::{Control, Report};
use join::{Join, ON_SCREEN, Opened, Outcome, Press, Seat};

/// One controller's pad, frame by frame: the last frame's buttons, so a hold is pressed once.
struct Pad {
    id: &'static str,
    before: u16,
}

impl Pad {
    fn new(id: &'static str) -> Pad {
        Pad { id, before: 0 }
    }

    /// A frame with `held` down, then a frame with nothing, through the report bytes; the outcomes of
    /// what the screen read.
    fn tap(&mut self, screen: &mut Join<&'static str>, held: &[Control]) -> Vec<Outcome> {
        let mut out = self.frame(screen, held);
        out.extend(self.frame(screen, &[]));
        out
    }

    fn frame(&mut self, screen: &mut Join<&'static str>, held: &[Control]) -> Vec<Outcome> {
        let now = Report::from_buttons(game_sim::input::mask_of(held)).buttons();
        let out = join::presses(self.before, now).into_iter().map(|p| screen.press(&self.id, p)).collect();
        self.before = now;
        out
    }
}

fn seats(s: &Join<&'static str>) -> Vec<Option<&'static str>> {
    s.riders()
}

#[test]
fn a_held_button_is_one_press() {
    let mut screen = Join::new(ON_SCREEN, Opened::Menu);
    let mut a = Pad::new("a");
    for _ in 0..30 {
        a.frame(&mut screen, &[Control::Cross, Control::Right]);
    }
    // one Cross joined seat 1; Right in the same frame came after it and moved to seat 2, once
    assert_eq!(seats(&screen), [None, Some("a"), None, None]);
    assert_eq!(join::presses(0, Control::Cross.mask() | Control::Up.mask()), [Press::Join]);
}

#[test]
fn join_move_and_leave() {
    let mut screen = Join::new(ON_SCREEN, Opened::Menu);
    let (mut a, mut b, mut k) = (Pad::new("a"), Pad::new("b"), Pad::new("keys"));
    assert!(screen.views().is_empty());

    // Cross joins the first open seat, in the order they press
    b.tap(&mut screen, &[Control::Cross]);
    a.tap(&mut screen, &[Control::Cross]);
    assert_eq!(seats(&screen), [Some("b"), Some("a"), None, None]);
    assert_eq!(screen.views(), [0, 1]);

    // Right moves to the nearest open seat that way, past nobody; at the end it stays
    b.tap(&mut screen, &[Control::Right]);
    assert_eq!(seats(&screen), [None, Some("a"), Some("b"), None]);
    b.tap(&mut screen, &[Control::Right]);
    b.tap(&mut screen, &[Control::Right]);
    assert_eq!(seats(&screen), [None, Some("a"), None, Some("b")]);
    // Left skips a taken seat, never swapping its rider off it
    a.tap(&mut screen, &[Control::Right]);
    assert_eq!(seats(&screen), [None, None, Some("a"), Some("b")]);
    b.tap(&mut screen, &[Control::Left]);
    assert_eq!(seats(&screen), [None, Some("b"), Some("a"), None]);
    a.tap(&mut screen, &[Control::Right]);
    a.tap(&mut screen, &[Control::Left]);
    a.tap(&mut screen, &[Control::Left]);
    assert_eq!(seats(&screen), [Some("a"), Some("b"), None, None]);
    // a seated Cross does nothing
    a.tap(&mut screen, &[Control::Cross]);
    assert_eq!(seats(&screen), [Some("a"), Some("b"), None, None]);

    // the split follows the count, open seats skipped
    k.tap(&mut screen, &[Control::Cross]);
    assert_eq!(screen.views(), [0, 1, 2]);
    // Circle leaves; the seat is open and the next Cross takes it
    b.tap(&mut screen, &[Control::Circle]);
    assert_eq!(seats(&screen), [Some("a"), None, Some("keys"), None]);
    assert_eq!(screen.views(), [0, 2]);

    // full: a fifth controller can't join a four-seat screen
    let (mut c, mut d, mut e) = (Pad::new("c"), Pad::new("d"), Pad::new("e"));
    c.tap(&mut screen, &[Control::Cross]);
    d.tap(&mut screen, &[Control::Cross]);
    e.tap(&mut screen, &[Control::Cross]);
    assert_eq!(seats(&screen), [Some("a"), Some("c"), Some("keys"), Some("d")]);
    assert_eq!(screen.views().len(), ON_SCREEN);

    // Circle with no seat is back; Start from a seated one is done, from an unseated one nothing
    assert_eq!(e.tap(&mut screen, &[Control::Circle]), [Outcome::Back]);
    assert_eq!(e.tap(&mut screen, &[Control::Start]), [Outcome::Stay]);
    assert_eq!(k.tap(&mut screen, &[Control::Start]), [Outcome::Done]);
}

#[test]
fn start_needs_only_the_host() {
    // From the menu one seat in play is a local game (P171): the others drop in on the ride. With
    // nobody in, Start is nobody's — an unseated controller's Start does nothing.
    let mut screen = Join::new(ON_SCREEN, Opened::Menu);
    let (mut a, mut b) = (Pad::new("a"), Pad::new("b"));
    assert!(!screen.can_start());
    assert_eq!(a.tap(&mut screen, &[Control::Start]), [Outcome::Stay]);
    a.tap(&mut screen, &[Control::Cross]);
    assert!(screen.can_start());
    assert_eq!(b.tap(&mut screen, &[Control::Start]), [Outcome::Stay], "not seated");
    assert_eq!(a.tap(&mut screen, &[Control::Start]), [Outcome::Done]);
    b.tap(&mut screen, &[Control::Cross]);
    assert!(screen.can_start());
    assert_eq!(a.tap(&mut screen, &[Control::Start]), [Outcome::Done]);
    // a gone seat is still a rider, and counts
    screen.update(&["a"]);
    assert_eq!(a.tap(&mut screen, &[Control::Start]), [Outcome::Done]);
    // left again: alone, which still starts
    screen.update(&["a", "b"]);
    b.tap(&mut screen, &[Control::Circle]);
    assert_eq!(screen.views().len(), 1);
    assert_eq!(a.tap(&mut screen, &[Control::Start]), [Outcome::Done]);
    // From the pause the ride goes on with whoever is left.
    let mut screen = Join::with(&[Some("a")], ON_SCREEN, Opened::Pause);
    assert_eq!(a.tap(&mut screen, &[Control::Start]), [Outcome::Done]);
}

#[test]
fn eight_seats_when_the_screen_has_them() {
    let mut screen = Join::new(join::MOST, Opened::Menu);
    let ids = ["0", "1", "2", "3", "4", "5", "6", "7", "8"];
    let mut pads: Vec<Pad> = ids.iter().map(|id| Pad::new(id)).collect();
    for p in &mut pads {
        p.tap(&mut screen, &[Control::Cross]);
    }
    assert_eq!(screen.views(), (0..8).collect::<Vec<_>>());
    assert_eq!(screen.seat_of(&"8"), None);
}

#[test]
fn the_seat_is_kept_across_an_unplug() {
    // opened from the pause menu over a ride's seats
    let mut screen = Join::with(&[Some("a"), Some("b")], ON_SCREEN, Opened::Pause);
    let (mut a, mut c) = (Pad::new("a"), Pad::new("c"));
    screen.update(&["a", "b"]);
    assert_eq!(seats(&screen), [Some("a"), Some("b"), None, None]);

    // b drops mid-ride: its seat is gone, not open, and still a view (its rider waits)
    screen.update(&["a"]);
    assert_eq!(screen.seats[1], Seat::Gone("b"));
    assert_eq!(screen.label(1, |d| d.to_string()), "b - gone");
    assert_eq!(screen.views(), [0, 1]);

    // a new pad joining takes the next open seat, never the gone one; a can't move into it either
    screen.update(&["a", "c"]);
    c.tap(&mut screen, &[Control::Cross]);
    assert_eq!(seats(&screen), [Some("a"), Some("b"), Some("c"), None]);
    a.tap(&mut screen, &[Control::Right]);
    assert_eq!(seats(&screen), [None, Some("b"), Some("c"), Some("a")]);
    a.tap(&mut screen, &[Control::Left]);
    assert_eq!(seats(&screen), [Some("a"), Some("b"), Some("c"), None]);

    // b comes back to its own seat
    screen.update(&["c", "a", "b"]);
    assert_eq!(screen.seats[1], Seat::Taken("b"));

    // gone again; this time b's Cross arrives before the update has seen it back: still home
    screen.update(&["a", "c"]);
    let mut b = Pad::new("b");
    b.tap(&mut screen, &[Control::Cross]);
    assert_eq!(screen.seats[1], Seat::Taken("b"));

    // and a seated player's Square lets a gone seat go, for the next pad to join into
    screen.update(&["a", "c"]);
    c.tap(&mut screen, &[Control::Square]);
    assert_eq!(seats(&screen), [Some("a"), None, Some("c"), None]);
    assert_eq!(screen.views(), [0, 2]);
    // b back after being let go is a new player: the first open seat
    screen.update(&["a", "c", "b"]);
    assert_eq!(screen.seat_of(&"b"), None);
    b.tap(&mut screen, &[Control::Cross]);
    assert_eq!(screen.seat_of(&"b"), Some(1));
}
