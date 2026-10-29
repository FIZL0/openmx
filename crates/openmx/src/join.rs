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
//! * **Start** from a seated controller is done — from the menu once at least [`FEWEST`] seats are in
//!   play, which is one (P171): a local game may start alone, since the others can drop in on the
//!   ride as it runs (P76) — and **Circle** from one with no seat is back.
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

/// The fewest seats in play a local game starts with from the menu: the host alone (P171; P51 had
/// asked for one more), since whoever comes later joins the running ride with Cross (P76).
pub const FEWEST: usize = 1;

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
#[derive(Clone, Debug, PartialEq, Eq)]
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

    /// Whether Start is done: from the menu, once [`FEWEST`] seats are in play; from the pause menu
    /// always, since the ride goes on with whoever is left in it.
    pub fn can_start(&self) -> bool {
        self.from == Opened::Pause || self.views().len() >= FEWEST
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
            (Some(_), Press::Start) if self.can_start() => Outcome::Done,
            (Some(_), Press::Start) => Outcome::Stay,
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

/// The most local players a running ride takes in (P76): the join screen's seats.
pub const IN_RIDE: usize = ON_SCREEN;

/// A change of a running freeride's local players (P76), which every per-player list of the ride
/// takes the same way ([`Change::apply`]) so that each staying player keeps all that is theirs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Change {
    /// The controller on this pad seat claimed a free seat: a new player after the rest.
    Join(usize),
    /// This player gave up their seat: they go, and the players after them move up one.
    Leave(usize),
}

impl Change {
    /// The change to one per-player list: a joiner's entry made by `make` on the end, a leaver's
    /// taken out. Nothing else in the list is touched. The joiner's index, or the leaver's.
    pub fn apply<T>(self, players: &mut Vec<T>, make: impl FnOnce() -> T) -> Option<usize> {
        match self {
            Change::Join(_) => {
                players.push(make());
                Some(players.len() - 1)
            }
            Change::Leave(i) if i < players.len() => {
                players.remove(i);
                Some(i)
            }
            Change::Leave(_) => None,
        }
    }

    /// The players' pad seats after it: player `i` reads `devices[i]`'s pad. A controller already
    /// riding joins nobody.
    pub fn devices(self, devices: &[usize]) -> Vec<usize> {
        let mut after = devices.to_vec();
        match self {
            Change::Join(d) if !devices.contains(&d) => after.push(d),
            Change::Join(_) => {}
            Change::Leave(_) => {
                self.apply(&mut after, || 0);
            }
        }
        after
    }
}

/// How long a card's slide to its new seat lasts on the Controllers screen (P210), in the menu's
/// 60 Hz ticks: opentennis's 0.3 s.
pub const SLIDE_TICKS: u32 = 18;

/// How long a pressed card glows on the Controllers screen (P210), in ticks.
pub const FLASH_TICKS: u32 = 30;

/// The Controllers screen (P210, opentennis's `assign.rs`): every device plugged in — the pads, the
/// keyboard, the browser's touch pad — on a seat or on none, each moving itself with its own
/// presses.
///
/// A device's place is a column: 0 is "no seat", `k` is seat `k - 1`. **Left** and **Right** step it
/// a column, and a device already in the column it steps into swaps with it — takes its place, and
/// it takes the mover's. **Cross** lights its card (press-to-find, the way opentennis's cards flash)
/// and, from no seat, joins the first open one. **Square** takes it off its seat. Unlike the join
/// screen, a seat here is not a rider: these are the seats `pad.rs`'s `Seats` hands out, which the
/// next ride (or this one, from the pause) reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assign<D> {
    /// What drives each seat, [`MOST`] of them.
    pub seats: Vec<Option<D>>,
    /// The devices plugged in, in the order the shell found them: no seat's column lists them so.
    pub present: Vec<D>,
    /// How many seats the screen offers: [`MOST`], or online this machine's own.
    pub open: usize,
    /// The cards lit by their own press, and the ticks left of it.
    pub flash: Vec<(D, u32)>,
    /// The cards on their way to a new column: the column they left, and the ticks left.
    pub slide: Vec<(D, usize, u32)>,
    /// Whether a press moved a device since the shell last took the seats ([`Assign::take`]).
    pub moved: bool,
    /// Whether the shell gave the screen its seats ([`Assign::with`]): until it has, the screen takes
    /// the next ones it is given.
    pub given: bool,
}

impl<D: Clone + PartialEq> Default for Assign<D> {
    fn default() -> Assign<D> {
        Assign { seats: vec![None; MOST], present: Vec::new(), open: MOST, flash: Vec::new(), slide: Vec::new(), moved: false, given: false }
    }
}

impl<D: Clone + PartialEq> Assign<D> {
    /// The screen over the seats as they are (missing ones empty), with `present` plugged in.
    pub fn with(seats: &[Option<D>], present: &[D], open: usize) -> Assign<D> {
        let mut a = Assign { open: open.clamp(1, MOST), given: true, ..Assign::default() };
        for (s, d) in a.seats.iter_mut().zip(seats) {
            *s = d.clone();
        }
        a.present = present.to_vec();
        a
    }

    /// `d`'s column: 0 for none, else its seat plus one.
    pub fn column(&self, d: &D) -> usize {
        self.seats.iter().position(|s| s.as_ref() == Some(d)).map_or(0, |i| i + 1)
    }

    /// The devices with no seat, in the order plugged in.
    pub fn loose(&self) -> Vec<D> {
        self.present.iter().filter(|d| self.column(d) == 0).cloned().collect()
    }

    /// Whether `d` is plugged in (a seat's device may be out: its seat waits for it).
    pub fn plugged(&self, d: &D) -> bool {
        self.present.contains(d)
    }

    /// The devices plugged in now. One unplugged keeps its seat, as `PadSlots` keeps it.
    pub fn update(&mut self, present: &[D]) {
        self.present = present.to_vec();
        self.flash.retain(|(d, _)| present.contains(d));
    }

    /// `d` to column `to`, whoever is there to `d`'s column.
    fn move_to(&mut self, d: &D, to: usize) {
        let from = self.column(d);
        if to == from || to > self.open {
            return;
        }
        let other = if to == 0 { None } else { self.seats[to - 1].take() };
        if from > 0 {
            self.seats[from - 1] = other.clone();
        }
        if to > 0 {
            self.seats[to - 1] = Some(d.clone());
        }
        self.start_slide(d, from);
        if let Some(o) = other.filter(|_| from > 0) {
            self.start_slide(&o, to);
        }
        self.moved = true;
    }

    fn start_slide(&mut self, d: &D, from: usize) {
        self.slide.retain(|(s, _, _)| s != d);
        self.slide.push((d.clone(), from, SLIDE_TICKS));
    }

    /// Light `d`'s card.
    pub fn light(&mut self, d: &D) {
        self.flash.retain(|(f, _)| f != d);
        self.flash.push((d.clone(), FLASH_TICKS));
    }

    /// Device `d` pressed `p`. Start and Circle are the screen's to close, not this.
    pub fn press(&mut self, d: &D, p: Press) {
        if !self.plugged(d) {
            return;
        }
        let at = self.column(d);
        match p {
            Press::Left if at > 0 => self.move_to(d, at - 1),
            Press::Right => self.move_to(d, at + 1),
            Press::Join => {
                if at == 0
                    && let Some(i) = self.seats[..self.open].iter().position(Option::is_none)
                {
                    self.move_to(d, i + 1);
                }
                self.light(d);
            }
            Press::LetGo => self.move_to(d, 0),
            _ => {}
        }
    }

    /// `ticks` of the menu's clock: the glows fade and the slides land.
    pub fn tick(&mut self, ticks: u32) {
        for (_, t) in &mut self.flash {
            *t = t.saturating_sub(ticks);
        }
        self.flash.retain(|(_, t)| *t > 0);
        for (_, _, t) in &mut self.slide {
            *t = t.saturating_sub(ticks);
        }
        self.slide.retain(|(_, _, t)| *t > 0);
    }

    /// How lit `d`'s card is, 1 just pressed to 0.
    pub fn glow(&self, d: &D) -> f32 {
        self.flash.iter().find(|(f, _)| f == d).map_or(0.0, |(_, t)| *t as f32 / FLASH_TICKS as f32)
    }

    /// Where `d`'s card is drawn between its old column and its own, 0 there to 1 home (eased).
    pub fn slid(&self, d: &D) -> Option<(usize, f32)> {
        let (_, from, t) = self.slide.iter().find(|(s, _, _)| s == d)?;
        let x = 1.0 - *t as f32 / SLIDE_TICKS as f32;
        Some((*from, 1.0 - (1.0 - x) * (1.0 - x)))
    }

    /// The seats, once a press moved one since the last time this was asked.
    pub fn take(&mut self) -> Option<Vec<Option<D>>> {
        std::mem::take(&mut self.moved).then(|| self.seats.clone())
    }
}
