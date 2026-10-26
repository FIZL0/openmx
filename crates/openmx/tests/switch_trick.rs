//! P161 — **a switch mid-trick fails the trick as a bail does.**
//!
//! The switch (`Garage::switch`, from the M key, the pause's Switch To Machine, a tape, and online
//! through `switch_lane`) used to leave the bike's rider as it was: the trick running, the combo
//! open, the HUD's pop-up standing over the plane, and the combo banked on the next landing after
//! the switch back. Now the switch stops the trick machine and loses the combo as the wreck does
//! (`Rider::trick_lost`, `Score::wrecked`): no points, the pop-up plays out as a bail's and clears,
//! and the bike taken back has nothing in progress.
//!
//! The rides are the stock bike thrown high over a flat floor, so they are seconds in the air, with
//! the trick buttons pressed for real through the rider's own trick machine. No disc needed: the
//! plane is a parked one, which is all the switch needs to stand it by the bike.

#[path = "../src/menu.rs"]
#[allow(dead_code)]
mod menu;
#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;
#[path = "../src/roster.rs"]
#[allow(dead_code)]
mod roster;
#[path = "../src/chase.rs"]
#[allow(dead_code)]
mod chase;
#[path = "../src/join.rs"]
#[allow(dead_code)]
mod join;
#[path = "../src/split.rs"]
#[allow(dead_code)]
mod split;
#[path = "../src/overlay.rs"]
#[allow(dead_code)]
mod overlay;
#[path = "../src/ride_hud.rs"]
#[allow(dead_code)]
mod ride_hud;

use game_sim::Sim;
use game_sim::ride::Controls;
use game_sim::trick::{Set, Stunt, Trick};
use game_sim::world::Flat;
use ps2_float::{F, Vf};
use ride_hud::{BAIL_COLOUR, TIMES, Tracker};
use std::time::{Duration, Instant};
use vehicle::{Garage, Kind, Plane, Spawn};

const DT: F = F(game_sim::tick::DeltaTime::CAP_30);
const TICK: Duration = Duration::from_micros(33_333);

/// The stock bike 400 ft over the floor, falling, on the bike with the parked plane in its garage.
/// It rides the floor first: a bike that has not touched down since it was put on the track takes
/// no trick (`trick::Bike::fresh`).
fn thrown() -> (Sim, Garage, Flat) {
    let mut sim = Sim::new(0);
    sim.rider.set = stunts();
    let mut floor = Flat::default();
    for _ in 0..30 {
        sim.step(DT, &Controls::default(), &mut floor);
    }
    assert!(!sim.rider.state.fresh, "the bike touched down");
    let at = Vf::new(0.0, 400.0, 0.0, 1.0);
    sim.rider.ride.launch(at, Vf::new(0.0, 0.0, 1.0, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0), Vf::new(0.0, 0.0, 10.0, 0.0));
    let bay = Spawn { at: Vf::new(100.0, 0.0, -50.0, 1.0), facing: Vf::new(0.0, 0.0, -1.0, 0.0) };
    let garage = Garage::new(Kind::Bike, Some(Plane::parked(bay))).unwrap();
    (sim, garage, floor)
}

/// The rider's 24 stunts: the stock rider has none, and a gesture with no stunt in its slot never
/// starts a clock. A second each, safe from half a second, as the disc's tricks run about.
fn stunts() -> Set {
    let stunt = |i: usize| Stunt {
        name: format!("trick{i}"),
        display_name: format!("Trick {i}"),
        safe_time: F(0.5),
        hold_time: F(0.4),
        duration: F(1.0),
        for_bike: true,
        index: i as i32,
    };
    Set::new((0..24).map(stunt).collect())
}

/// A trick asked for: the first trick button and the d-pad's up, as the decoder reads a gesture.
fn trick() -> Controls {
    let mut c = Controls::default();
    c.trick[0] = true;
    c.dpad[2] = true;
    c
}

/// One tick of the seat as the shell has it: the garage's step, then the HUD's tracker a tick on.
struct Ride {
    sim: Sim,
    garage: Garage,
    floor: Flat,
    hud: Tracker,
    now: Instant,
}

impl Ride {
    fn new() -> Ride {
        let (sim, garage, floor) = thrown();
        Ride { sim, garage, floor, hud: Tracker::default(), now: Instant::now() }
    }

    fn tick(&mut self, c: &Controls) {
        self.garage.step(&mut self.sim, DT, c, &mut self.floor);
        self.now += TICK;
        self.track();
    }

    fn track(&mut self) {
        self.hud.update(self.now, &self.sim.rider.score, |id| format!("Trick {id}"));
    }

    /// Idle ticks until `done`, or a panic after `most`.
    fn until(&mut self, most: usize, what: &str, done: impl Fn(&Sim) -> bool) {
        for _ in 0..most {
            if done(&self.sim) {
                return;
            }
            self.tick(&Controls::default());
        }
        panic!("never {what}");
    }

    /// A trick pressed for a tick, and ridden until it scores.
    fn press(&mut self) {
        self.until(30, "settled into the fall", |s| !s.rider.ride.air.on_ground && !s.rider.tricks.in_progress);
        self.tick(&trick());
        self.until(10, "started the trick", |s| s.rider.score.scoring);
    }

    fn switch(&mut self, to: Kind) {
        assert_eq!(self.garage.switch(to, &mut self.sim, &mut self.floor), Ok(to));
        self.track();
    }
}

/// The control: the same fall and the same trick, no switch, banks its points on the landing — so
/// what the switch loses below is points the ride would have had.
#[test]
fn without_a_switch_the_trick_banks() {
    let mut r = Ride::new();
    r.press();
    r.until(400, "landed and banked", |s| s.rider.score.total > F::ZERO);
}

/// Mid-trick: the switch onto the plane banks nothing, the pop-up plays out as a bail's and is
/// gone, and the switch back finds no trick in progress — nor banks anything when the bike rides on.
#[test]
fn a_switch_mid_trick_loses_it_as_a_bail() {
    let mut r = Ride::new();
    r.press();
    assert!(r.sim.rider.tricks.in_progress, "the trick runs");
    assert_eq!(r.hud.popup(r.now).and_then(|p| p.outcome), None, "the pop-up shows the trick going");
    for _ in 0..5 {
        r.tick(&Controls::default());
    }
    assert!(r.sim.rider.score.combo_points > F::ZERO || r.sim.rider.score.nodes.iter().any(|n| n.points > F::ZERO), "it grew points");

    r.switch(Kind::Biplane);
    lost(&mut r);
}

/// Mid-combo: a first trick ridden out and a second started, then the switch — the whole chain is
/// lost, the first trick's points with it.
#[test]
fn a_switch_mid_combo_loses_the_whole_chain() {
    let mut r = Ride::new();
    r.press();
    r.until(200, "finished the first trick", |s| s.rider.score.combo && !s.rider.score.scoring);
    assert!(r.sim.rider.score.combo_points > F::ZERO, "the first trick scored");
    r.press();
    assert_eq!(r.sim.rider.score.nodes.len(), 2, "a combo of two");
    let popup = r.hud.popup(r.now).expect("the chain is up");
    assert_eq!((popup.outcome, popup.mult), (None, Some(2)));

    r.switch(Kind::Biplane);
    lost(&mut r);
}

/// What a lost trick leaves, from the switch onto the plane on: nothing banked, the bail's pop-up
/// played out and cleared over the plane, and the bike back with nothing going.
fn lost(r: &mut Ride) {
    let s = &r.sim.rider.score;
    assert!(!s.combo && !s.scoring && s.nodes.is_empty(), "the combo is closed: {s:?}");
    assert_eq!((s.total, s.combo_points), (F::ZERO, F::ZERO), "nothing banked");
    let t = &r.sim.rider.tricks;
    assert!(!t.in_progress && t.current == Trick::NONE && t.queue.is_empty(), "the trick machine stopped");

    let popup = r.hud.popup(r.now).expect("the pop-up plays out");
    assert_eq!(popup.outcome, Some(false), "as a bail");
    assert_eq!(popup.colour, BAIL_COLOUR);
    assert!(!popup.names.iter().any(|n| n.contains("Stunt Combo Total")), "no landed chain's total");
    // The plane flies on (a parked one stands) while the play-out runs, and then it is gone.
    let out = TIMES[2].as_micros() / TICK.as_micros() + 2;
    for _ in 0..out {
        r.tick(&Controls::default());
    }
    assert_eq!(r.hud.popup(r.now), None, "the pop-up cleared over the plane");

    r.switch(Kind::Bike);
    let s = &r.sim.rider.score;
    assert!(!s.combo && !s.scoring && s.flip == 0, "no combo on the bike taken back");
    let t = &r.sim.rider.tricks;
    assert!(!t.in_progress && t.current == Trick::NONE, "no trick on the bike taken back");
    for _ in 0..90 {
        r.tick(&Controls::default());
    }
    assert_eq!(r.sim.rider.score.total, F::ZERO, "nothing banked riding on");
    assert_eq!(r.hud.popup(r.now), None);
}
