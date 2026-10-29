//! P207 — **the new layout's keyboard: the gas is the gas and nothing else.**
//!
//! The user's report: "j is also trick now". Under P80's home row J was the gas, and the shell
//! pressed the pad's Cross beside R2 for it, as the menus' OK. With MX vs. ATV's tricks on (MD16)
//! the new layout hands the ride its Cross as the first extra trick button
//! (`Scheme::ride_tricks`), so a held gas was a held trick button: any lean in the air started a
//! pack trick. Since P207 the keyboard is modern MX games' (W the gas, S the brake, A and D the
//! steer, I and K the lean, F G E the tricks) and its gas presses the gas alone.
//!
//! These rides are the stock bike thrown high over a flat floor, driven by held keys through the
//! shell's own keyboard pad, then the seat's steps as the shell has them: `Scheme::ride_tricks`,
//! the poll, the markers' stick-alone hands, the reset chord. Each under the original's tricks and
//! under a pack's: a trick asked for with the gas held starts the same trick as without it and
//! banks on the landing, and the gas held through a lean starts none. No disc needed.

// The shell's own keyboard pad. This crate is a binary, so a test cannot `use` it; these files are
// std and `game_sim` only, and are compiled into this test as they stand (as tests/ride.rs does).
// the browser's own clock, threads and files, which are std's here (P196)
#[allow(unused)]
#[path = "../src/clock.rs"]
mod clock;
#[allow(unused)]
#[path = "../src/files.rs"]
mod files;
#[allow(dead_code)]
#[path = "../src/window.rs"]
mod window;
#[allow(dead_code)]
#[path = "../src/pad.rs"]
mod pad;

use game_sim::input::Report;
use game_sim::ride::Controls;
use game_sim::trick::{Set, Stunt, Trick};
use game_sim::world::Flat;
use game_sim::{Sim, phy, trick_pack};
use pad::{Pads, Scheme, Wiring};
use ps2_float::{F, Vf};
use window::key;

const DT: F = F(game_sim::tick::DeltaTime::CAP_30);

/// A stunt in every slot, the pack's eight extra ones too: a second each, safe from half a second.
fn stunt(i: usize) -> Stunt {
    Stunt {
        name: format!("trick{i}"),
        display_name: format!("Trick {i}"),
        safe_time: F(0.5),
        hold_time: F(0.4),
        duration: F(1.0),
        for_bike: true,
        index: i as i32,
    }
}

/// One rider on the keyboard: the sim over a flat floor and the keyboard's pad under the new
/// layout, on the modern keys (the default wiring).
struct Ride {
    sim: Sim,
    floor: Flat,
    keys: Pads,
}

impl Ride {
    /// The stock bike 400 ft over the floor, falling, with or without a trick pack's moves. It rides
    /// the floor first: a bike that has not touched down since it was put on the track takes no
    /// trick.
    fn thrown(pack: bool) -> Ride {
        let mut sim = Sim::new(0);
        let set = Set::new((0..24).map(stunt).collect());
        sim.rider.set = if pack { set.with_extras((24..32).map(stunt).collect()) } else { set };
        if pack {
            let ini = phy::Ini::parse("[Three Sixty]\nMinThreeSixtyTime = 1.0\n");
            std::sync::Arc::make_mut(&mut sim.phy).tricks = Some(trick_pack::moves(&ini, &[]).expect("a [Three Sixty]"));
        }
        let mut floor = Flat::default();
        for _ in 0..30 {
            sim.step(DT, &Controls::default(), &mut floor);
        }
        assert!(!sim.rider.state.fresh, "the bike touched down");
        let at = Vf::new(0.0, 400.0, 0.0, 1.0);
        sim.rider.ride.launch(at, Vf::new(0.0, 0.0, 1.0, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0), Vf::new(0.0, 0.0, 10.0, 0.0));
        Ride { sim, floor, keys: Pads::unplugged_on(Wiring::default(), Scheme::New) }
    }

    /// One tick on `held` keys, through the seat's steps as the shell takes them.
    fn tick(&mut self, held: &[u8]) {
        let report = self.keys.build(DT.0, &|c| held.contains(&c));
        let pack = self.sim.phy.tricks.is_some();
        let report = Scheme::New.ride_tricks(report, pack);
        let c = self.sim.poll(DT, &[report, Report::missing()]).stick_alone();
        let c = c.chord_together(self.sim.input.pad(0), self.sim.table());
        self.sim.step(DT, &c, &mut self.floor);
    }

    fn in_air(&self) -> bool {
        !self.sim.rider.ride.air.on_ground
    }
}

/// What a ride on `script` came to: the first trick it started, and the points banked once it is
/// down and has rolled a second on the floor. The script is `(ticks, keys)` from the throw; then no
/// keys but `after`, to the floor and past it.
fn ride(pack: bool, script: &[(usize, &[u8])], after: &[u8]) -> (Option<Trick>, F) {
    let mut r = Ride::thrown(pack);
    let mut started = None;
    let note = |r: &Ride, started: &mut Option<Trick>| {
        if started.is_none() && r.sim.rider.tricks.in_progress {
            *started = Some(r.sim.rider.tricks.current);
        }
    };
    for &(ticks, held) in script {
        for _ in 0..ticks {
            r.tick(held);
            note(&r, &mut started);
        }
    }
    assert!(r.in_air(), "the script ran out in the air");
    let mut landed = None;
    for t in 0..600 {
        r.tick(after);
        note(&r, &mut started);
        if !r.in_air() && landed.is_none() {
            landed = Some(t);
        }
        if landed.is_some_and(|l| t > l + 30) {
            return (started, r.sim.rider.score.total);
        }
    }
    panic!("never landed");
}

/// The modern keys the rides press.
const GAS: u8 = key::W;
const LEAN_FORWARD: u8 = key::I;
const TRICK_1: u8 = key::F;
const TRICK_4: u8 = key::SPACE;

/// A trick on the keys — F, the first trick button, with I, the lean forward, as the stick's
/// direction — starts the same trick with W, the gas, held through it as without, and banks on
/// the landing: under the original's tricks and under a pack's.
#[test]
fn a_trick_with_the_gas_held_is_the_same_trick() {
    for pack in [false, true] {
        let settle: (usize, &[u8]) = (10, &[]);
        let bare = ride(pack, &[settle, (20, &[TRICK_1, LEAN_FORWARD])], &[]);
        let gassed = ride(pack, &[(10, &[GAS]), (20, &[GAS, TRICK_1, LEAN_FORWARD])], &[GAS]);
        let trick = bare.0.unwrap_or_else(|| panic!("pack {pack}: F and I start a trick"));
        assert!(trick.extra().is_none(), "pack {pack}: the first trick button's own trick, not a pack one: {trick:?}");
        assert_eq!(gassed.0, Some(trick), "pack {pack}: the gas changed the trick");
        assert!(bare.1 > F::ZERO && gassed.1 > F::ZERO, "pack {pack}: both bank: {:?} {:?}", bare.1, gassed.1);
    }
}

/// The gas held through a lean in the air starts no trick, with a pack's tricks or without — what
/// J, the home row's gas, did before P207. And the pack's extra trick button is still a key of its
/// own: Space with the same lean starts one of the pack's tricks, so the ride would have shown a
/// gas that pressed it.
#[test]
fn the_gas_and_a_lean_start_no_trick() {
    for pack in [false, true] {
        let (started, banked) = ride(pack, &[(40, &[GAS, LEAN_FORWARD])], &[GAS]);
        assert_eq!((started, banked), (None, F::ZERO), "pack {pack}: the gas and a lean started a trick");
    }
    let (started, _) = ride(true, &[(10, &[]), (20, &[TRICK_4, LEAN_FORWARD])], &[]);
    assert!(started.is_some_and(|t| t.extra().is_some()), "Space and I start a pack trick: {started:?}");
    let (started, _) = ride(false, &[(10, &[]), (20, &[TRICK_4, LEAN_FORWARD])], &[]);
    assert_eq!(started, None, "without a pack Space is no trick");
}

/// P80's home row, `keys = home-row`, is the same: J, its gas, is no trick button now.
#[test]
fn the_home_row_gas_is_no_trick_either() {
    let mut r = Ride::thrown(true);
    r.keys = Pads::unplugged_on(Wiring { keys: pad::Keys::HomeRow, ..Wiring::default() }, Scheme::New);
    for _ in 0..40 {
        // J the gas, W the lean forward on the home row.
        r.tick(&[key::J, key::W]);
        assert!(!r.sim.rider.tricks.in_progress, "J and W started a trick");
    }
    assert!(r.in_air());
}
