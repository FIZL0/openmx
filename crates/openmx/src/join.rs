//! The front end's join screen: who is riding, on which seat, with which controller.
//!
//! opentennis's controller assignment (`crates/hst/src/play/main_menu/assign.rs` and the `Menu` it
//! hangs off) is the model, cut down to what a ride needs. Every controller drives itself on this
//! screen — there is no 1P carrying the others' devices about:
//!
//! * a controller with no seat presses **Cross** to join, and takes the first open seat;
//! * a seated one moves its seat with **Left** and **Right** (the d-pad), to the nearest open seat that
//!   way — it never pushes another player off theirs, which opentennis's swap does, because here the
//!   other seat is somebody's rider;
//! * a seated one presses **Circle** to leave, and its seat is open again;
//! * **Square** from any seated controller lets go of the seats whose controllers are gone (below);
//! * **Start** from a seated controller is done, and **Circle** from one with no seat is back.
//!
//! The seats are the riders, in order, and the split screen has one view per seat in play
//! ([`Join::views`]): open seats are skipped, so two players on seats 1 and 3 are a two-way split.
//!
//! **A controller that drops out.** A seat whose controller is unplugged is not freed: it shows as
//! gone ([`Seat::Gone`]), still counts as a view (the rider is on the track, waiting), no other
//! controller can join into it, and when the same controller is plugged back in it is that seat's
//! again — on this screen, between rides, or from the pause menu ([`Join::update`]). This is the same
//! rule as P35's `PadSlots` (which keeps a seat for its pad mid-ride), applied to the screen; only a
//! Square, or the controller itself coming back and pressing Circle, lets the seat go.
//!
//! The screen is pure state over a device id `D` (a pad's identity, or the keyboard) so it can be
//! driven and tested without a window or a device; whoever owns the devices feeds it
//! [`Join::update`] with what is plugged in and [`Join::press`] with each controller's new presses
//! ([`presses`] turns two of the game's button words into them).

use game_sim::input::Control;

/// Seats on one screen today. Eight is P35's seat count and the stretch task's (V23) screen; until
/// that lands the join screen offers four.
pub const ON_SCREEN: usize = 4;

/// The most seats the screen can be made with ([`Join::new`]): P35's eight.
pub const MOST: usize = 8;

/// What one seat holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Seat<D> {
    /// Nobody: a controller with no seat may join into it.
    Open,
    /// This controller rides it.
    Taken(D),
    /// This controller rode it and was unplugged; the seat is kept for it.
    Gone(D),
}

impl<D> Seat<D> {
    /// The controller the seat is for, whether it is in or gone.
    pub fn device(&self) -> Option<&D> {
        match self {
            Seat::Open => None,
            Seat::Taken(d) | Seat::Gone(d) => Some(d),
        }
    }

    /// Whether a rider is on it (taken, or waiting for its controller).
    pub fn in_play(&self) -> bool {
        !matches!(self, Seat::Open)
    }
}

/// A press the screen reads.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Press {
    /// Cross.
    Join,
    /// Circle.
    Leave,
    Left,
    Right,
    /// Square.
    LetGo,
    Start,
}

impl Press {
    /// The pad control this press is, in the game's own map.
    pub const fn control(self) -> Control {
        match self {
            Press::Join => Control::Cross,
            Press::Leave => Control::Circle,
            Press::Left => Control::Left,
            Press::Right => Control::Right,
            Press::LetGo => Control::Square,
            Press::Start => Control::Start,
        }
    }

    pub const ALL: [Press; 6] =
        [Press::Join, Press::Leave, Press::Left, Press::Right, Press::LetGo, Press::Start];
}

/// The presses between two of the game's button words (active high, [`Control::mask`]'s bits): the
/// controls down in `now` that were up in `before`, in [`Press::ALL`]'s order.
pub fn presses(before: u16, now: u16) -> Vec<Press> {
    let new = now & !before;
    Press::ALL.into_iter().filter(|p| new & p.control().mask() != 0).collect()
}

/// What a press asked of the front end beyond the seats.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    /// Nothing more: the seats may have changed.
    Stay,
    /// Start from a seated controller: done, ride.
    Done,
    /// Circle from a controller with no seat: back to the screen before.
    Back,
}

/// Where the screen was opened from. The rules are the same; the front end decides what a change
/// means (between rides the next ride starts with these seats; from the pause menu a joined seat's
/// rider is spawned and a left one's taken off when the ride resumes).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Opened {
    Menu,
    Pause,
}

/// The join screen's seats.
#[derive(Clone, Debug)]
pub struct Join<D> {
    pub seats: Vec<Seat<D>>,
    pub from: Opened,
}

impl<D: Clone + PartialEq> Join<D> {
    /// `count` open seats ([`ON_SCREEN`] today, at most [`MOST`]).
    pub fn new(count: usize, from: Opened) -> Join<D> {
        assert!((1..=MOST).contains(&count), "{count} seats: the screen has 1 to {MOST}");
        Join { seats: vec![Seat::Open; count], from }
    }

    /// The screen over seats already set (the ride's, opened from the pause menu, or the last ride's).
    /// Extra seats past the screen's are dropped; missing ones are open.
    pub fn with(seats: &[Option<D>], count: usize, from: Opened) -> Join<D> {
        let mut join = Join::new(count, from);
        for (seat, d) in join.seats.iter_mut().zip(seats) {
            if let Some(d) = d {
                *seat = Seat::Taken(d.clone());
            }
        }
        join
    }

    /// The seat `d` is on (in or gone), if any.
    pub fn seat_of(&self, d: &D) -> Option<usize> {
        self.seats.iter().position(|s| s.device() == Some(d))
    }

    /// The seats in play, in seat order: one view each on the split screen.
    pub fn views(&self) -> Vec<usize> {
        (0..self.seats.len()).filter(|&i| self.seats[i].in_play()).collect()
    }

    /// The seats as the ride takes them: the controller on each seat, gone or in, or nobody.
    pub fn riders(&self) -> Vec<Option<D>> {
        self.seats.iter().map(|s| s.device().cloned()).collect()
    }

    /// `present` is every controller plugged in. A seat whose controller is no longer in it is gone;
    /// a gone seat whose controller is back is taken again.
    pub fn update(&mut self, present: &[D]) {
        for seat in &mut self.seats {
            *seat = match std::mem::replace(seat, Seat::Open) {
                Seat::Taken(d) if !present.contains(&d) => Seat::Gone(d),
                Seat::Gone(d) if present.contains(&d) => Seat::Taken(d),
                s => s,
            };
        }
    }

    /// Controller `d` pressed `p`.
    pub fn press(&mut self, d: &D, p: Press) -> Outcome {
        let seat = self.seat_of(d).filter(|&i| matches!(self.seats[i], Seat::Taken(_)));
        match (seat, p) {
            (None, Press::Join) => {
                // a gone seat's controller pressing before `update` saw it back still goes home
                if let Some(i) = self.seat_of(d) {
                    self.seats[i] = Seat::Taken(d.clone());
                } else if let Some(i) = self.seats.iter().position(|s| *s == Seat::Open) {
                    self.seats[i] = Seat::Taken(d.clone());
                }
                Outcome::Stay
            }
            (None, Press::Leave) => Outcome::Back,
            (None, _) => Outcome::Stay,
            (Some(i), Press::Leave) => {
                self.seats[i] = Seat::Open;
                Outcome::Stay
            }
            (Some(i), Press::Left) => {
                if let Some(j) = (0..i).rev().find(|&j| self.seats[j] == Seat::Open) {
                    self.seats.swap(i, j);
                }
                Outcome::Stay
            }
            (Some(i), Press::Right) => {
                if let Some(j) = (i + 1..self.seats.len()).find(|&j| self.seats[j] == Seat::Open) {
                    self.seats.swap(i, j);
                }
                Outcome::Stay
            }
            (Some(_), Press::LetGo) => {
                for s in &mut self.seats {
                    if matches!(s, Seat::Gone(_)) {
                        *s = Seat::Open;
                    }
                }
                Outcome::Stay
            }
            (Some(_), Press::Join) => Outcome::Stay,
            (Some(_), Press::Start) => Outcome::Done,
        }
    }

    /// What seat `i`'s card says under the player number.
    pub fn label(&self, i: usize, name: impl Fn(&D) -> String) -> String {
        match &self.seats[i] {
            Seat::Open => "Press \u{2715} to join".into(),
            Seat::Taken(d) => name(d),
            Seat::Gone(d) => format!("{} - gone", name(d)),
        }
    }
}
