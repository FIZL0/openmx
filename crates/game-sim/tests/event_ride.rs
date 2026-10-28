//! **The freestyle event on a ride** (P155): `Sim::event` starts the stunt competition when a rider
//! in a stunt zone holds Circle for a second, gates the card, runs the clock out, gives the verdict, takes it down, and
//! is held by a snapshot and a resync like the rest of the state.

use game_sim::event::{self, Event, Verdict, Zone};
use game_sim::score::Sense;
use game_sim::input::Control;
use game_sim::{Input, Sim, snapshot};
use ps2_float::F;

/// A ride on the stand-in bike with a stunt zone `off` feet beside where it stands.
fn ride(off: f32, target: i32) -> Sim {
    let mut sim = Sim::new(7);
    let at = sim.bike_pos();
    let zone = Zone::new([at.x() + off, at.y(), at.z()], [0.0, 1.0], [60.0, 60.0]);
    sim.event = Event::on_track(vec![zone], target);
    sim
}

/// Circle held.
fn circle() -> Input {
    Input { buttons: Control::Circle.mask(), ..Input::default() }
}

/// Hold Circle until the event starts: the ticks it took.
fn hold(sim: &mut Sim) -> u32 {
    let mut n = 0;
    while sim.event.on.is_none() {
        sim.tick(circle());
        n += 1;
        assert!(n < 100, "the hold never started the event");
    }
    n
}

/// Whether a trick starting in the air now would open a combo on player 1's card.
fn opens(sim: &Sim) -> bool {
    let mut card = sim.rider.score.clone();
    card.combo = false;
    card.start(0, &Sense { airborne: true, ..Sense::default() });
    card.combo
}

#[test]
fn a_zone_the_rider_is_not_in_starts_nothing() {
    let mut sim = ride(500.0, 1000);
    let before = {
        let mut plain = Sim::new(7);
        (0..60).for_each(|_| plain.tick(circle()));
        plain.hash()
    };
    (0..60).for_each(|_| sim.tick(circle()));
    assert!(sim.event.on.is_none() && sim.event.fresh());
    assert_eq!(sim.hash(), before, "a fresh event leaves the hash as it was");
    assert!(opens(&sim));
}

#[test]
fn riding_into_the_zone_runs_the_event_to_its_verdict() {
    let mut sim = ride(0.0, 1000);
    sim.rider.score.total = F(4321.5);
    // In the zone and riding nothing starts without the hold, and a hold let go starts again.
    (0..20).for_each(|_| sim.tick(Input::default()));
    assert!(sim.event.on.is_none());
    (0..20).for_each(|_| sim.tick(circle()));
    assert!(sim.event.held(0) > F(0.5) && sim.event.on.is_none());
    sim.tick(Input::default());
    assert_eq!(sim.event.held(0), F::ZERO);
    // Thirty of the 30 Hz clamp's 0.03333 s fall short of 1.0: the 31st tick starts it.
    assert_eq!(hold(&mut sim), 31, "a second's hold at 30 Hz");
    let c = sim.event.on.as_ref().expect("the hold started the event");
    assert_eq!(c.target, 1000);
    assert_eq!(c.seats[0].saved, 4321, "the freeride total is put aside as a whole number");
    assert_eq!(sim.rider.score.total, F::ZERO);
    assert!(c.counting && !c.running);
    assert!(!opens(&sim), "no combo opens in the countdown");

    // The countdown: 3.33 s, the limit set and the manager running on the same tick.
    let mut ticks = 1;
    while !sim.event.on.as_ref().unwrap().running {
        sim.tick(Input::default());
        ticks += 1;
        assert!(ticks < 200, "the countdown never ended");
    }
    assert_eq!(ticks, 101, "the countdown's 3.33 s at 30 Hz");
    assert_eq!(sim.event.on.as_ref().unwrap().limit, event::LIMIT);
    assert!(opens(&sim), "combos open once the clock runs");

    // Some points in the event, past the target.
    sim.rider.score.total = F(1500.25);
    sim.tick(Input::default());
    ticks += 1;
    assert!(sim.event.on.as_ref().unwrap().seats[0].reached, "the target-reached line");

    // A snapshot mid-event and a resync both carry it.
    let saved = snapshot::save(&sim);
    let bytes = snapshot::encode(&sim);
    let mut other = ride(0.0, 1000);
    snapshot::decode(&mut other, &bytes).expect("the state decodes");
    assert_eq!(other.hash(), sim.hash());
    assert_eq!(other.event.on, sim.event.on);

    // The clock runs out at 120 s.
    let mut warned = false;
    while sim.event.verdicts.first().is_none_or(Option::is_none) {
        sim.tick(Input::default());
        ticks += 1;
        warned |= sim.event.on.as_ref().unwrap().seats[0].warning;
        assert!(ticks < 4000, "the clock never ran out");
    }
    assert!(!warned, "no warning with the target reached");
    let c = sim.event.on.as_ref().unwrap();
    assert!(c.seats[0].out && c.seats[0].left == F::ZERO);
    // 3600 of the clamp's 0.03333 s leave 0.012 s on the clock: the 3601st runs it out.
    assert_eq!(ticks, 101 + 3601, "out 120 s after the clock started");
    assert_eq!(sim.event.verdicts[0], Some((Verdict::Win, 4321 + 1500)));
    assert_eq!(sim.rider.score.total, F(1500.25) + F(4321.0), "the freeride total put back");
    assert!(!opens(&sim), "no combo opens out of time");

    // The verdict stays up, then the ride is a free ride again; a Circle held all along does not
    // start another until it has been held a second after.
    while sim.event.on.is_some() {
        sim.tick(circle());
        ticks += 1;
        assert!(ticks < 4100, "the results never came down");
    }
    assert!(!sim.rider.score.shut && !sim.rider.score.out);
    assert!(opens(&sim));
    (0..20).for_each(|_| sim.tick(circle()));
    assert!(sim.event.on.is_none(), "no second event before a whole hold");

    // Rolled back to mid-event, it runs the same way again.
    let mut back = ride(0.0, 1000);
    snapshot::restore(&mut back, &saved);
    assert_eq!(back.event.on.as_ref().unwrap().clock, saved.sim().event.on.as_ref().unwrap().clock);
}

#[test]
fn short_of_the_target_is_a_loss_with_the_warning_up() {
    let mut sim = ride(0.0, 100_000);
    hold(&mut sim);
    let mut ticks = 1;
    let mut warned_at = None;
    while sim.event.verdicts.first().is_none_or(Option::is_none) {
        sim.tick(Input::default());
        ticks += 1;
        if warned_at.is_none() && sim.event.on.as_ref().is_some_and(|c| c.seats[0].warning) {
            warned_at = Some(ticks);
        }
        assert!(ticks < 4000);
    }
    // The warning shows once under 5 s are left: 115 s into the clock.
    let warned_at = warned_at.expect("the last-seconds warning");
    assert!((101 + 3449..=101 + 3452).contains(&warned_at), "warning at tick {warned_at}");
    assert_eq!(sim.event.verdicts[0], Some((Verdict::Lose, 0)));
}
