//! P88 — session markers on the d-pad, ridden the way the shell and the online session ride them: a
//! tick is the pad's poll with the d-pad out of the ride (`Controls::stick_alone`, P93), the step,
//! then `markers::frame` with every rider's spot from before the tick (`Seat::pass`,
//! `net::rides::Rides::step`). The stand-in bike on a flat floor, at the 30 Hz clamp. Left and Right
//! fire on the press, the tick they go down (P93); Down and Up on the tick their held timer crosses
//! `markers::HOLD_MS`, 200 ms (P106).
//!
//! P107's at the end: the plane's own set, flown as `Seat::pass` flies it — the garage's step, then
//! `Garage::markers` — on the disc's biplane over a flat floor (skipped without a disc).

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
#[allow(dead_code)]
#[path = "../src/net/mod.rs"]
mod net;
#[path = "../src/saved_markers.rs"]
#[allow(dead_code)]
mod saved_markers;
#[path = "../src/aspect.rs"]
#[allow(dead_code)]
mod aspect;
#[path = "../src/hud.rs"]
#[allow(dead_code)]
mod hud;

use game_sim::input::{Control, PADS, Report};
use game_sim::markers::{self, Did, HOLD_MS, Pick, SLOTS, Spot};
use game_sim::ride::Controls;
use game_sim::rider::Rider;
use game_sim::trick::Set;
use game_sim::world::Flat;
use game_sim::{Sim, hash, stock};
use ps2_float::{F, Vf};

const DT: F = F(game_sim::tick::DeltaTime::CAP_30);

fn pads(buttons: u16) -> [Report; PADS] {
    let mut r = [Report::missing(); PADS];
    r[0] = Report::from_buttons(buttons);
    r
}

/// One tick of every rider, player 1 on `buttons` and the rest on nothing, then player 1's markers.
fn tick(sim: &mut Sim, buttons: u16) -> Option<Did> {
    tick_dt(sim, buttons, DT)
}

/// [`tick`] on a tick `dt` long, to bring the held timer to a millisecond the 30 Hz clamp skips.
fn tick_dt(sim: &mut Sim, buttons: u16, dt: F) -> Option<Did> {
    let spots: Vec<Spot> = (0..sim.players()).map(|i| Spot::of(sim.rider_of(i))).collect();
    let mut hands: Vec<Controls> = sim.poll_all(dt, &pads(buttons)).into_iter().map(Controls::stick_alone).collect();
    hands.resize(sim.players(), Controls::default());
    sim.step_all(dt, &hands, &mut Flat::default());
    markers::frame(sim, 0, &spots, &hands[0])
}

/// `control` pressed (on top of `with`) and held until the input layer's timer reads at least `ms` —
/// 0 is a one-tick press — then let go for a tick; what each tick's markers did.
fn hold(sim: &mut Sim, control: Control, with: u16, ms: u32) -> Vec<Option<Did>> {
    let mut did = vec![tick(sim, with | control.mask())];
    while sim.input.pad(0).control(control).held_ms < ms {
        did.push(tick(sim, with | control.mask()));
    }
    did.push(tick(sim, with));
    did
}

fn fired(did: &[Option<Did>]) -> Vec<(usize, Did)> {
    did.iter().enumerate().filter_map(|(k, d)| d.map(|d| (k, d))).collect()
}

/// What fired, whatever the tick.
fn what(did: &[Option<Did>]) -> Vec<Did> {
    did.iter().flatten().copied().collect()
}

/// The tick of a hold from rest on which the held timer first reads [`HOLD_MS`] or more at 30 Hz:
/// the press's tick reads 0.
fn hold_tick() -> usize {
    let mut sim = Sim::new(0x88);
    let mut k = 0;
    tick(&mut sim, Control::Down.mask());
    while sim.input.pad(0).control(Control::Down).held_ms < HOLD_MS {
        tick(&mut sim, Control::Down.mask());
        k += 1;
    }
    k
}

const GAS: u16 = 0x0040; // Cross, the shipped preset's gas

/// A stand-in ride with some speed on it.
fn moving() -> Sim {
    let mut sim = Sim::new(0x88);
    for _ in 0..45 {
        tick(&mut sim, GAS);
    }
    assert!(F(sim.bike_vel().z()).abs() > F(1.0), "the bike is not moving: {:?}", sim.bike_vel());
    sim
}

/// A one-tick press of Right picks the next slot on that tick, and of Left the one before: no hold.
#[test]
fn a_one_tick_press_picks_the_next_slot() {
    let mut sim = moving();
    assert_eq!(fired(&hold(&mut sim, Control::Right, GAS, 0)), vec![(0, Did::Stepped(Pick::Slot(1)))]);
    assert_eq!(sim.markers.pick(0, 1), Pick::Slot(1));
    assert_eq!(fired(&hold(&mut sim, Control::Right, GAS, 0)), vec![(0, Did::Stepped(Pick::Slot(2)))]);
    assert_eq!(fired(&hold(&mut sim, Control::Left, GAS, 0)), vec![(0, Did::Stepped(Pick::Slot(1)))]);
    // Both at once is no step.
    let both = Control::Left.mask() | Control::Right.mask();
    assert_eq!(fired(&[tick(&mut sim, GAS | both), tick(&mut sim, GAS)]), vec![]);
    assert_eq!(sim.markers.pick(0, 1), Pick::Slot(1));
}

/// Down puts a marker in the selected slot on the tick its hold reaches 200 ms, the bike as that tick
/// left it — speed and all — and fires once however long it is held after (P106).
#[test]
fn down_places_a_marker_after_its_hold() {
    assert_eq!(HOLD_MS, 200);
    let mut sim = moving();
    let mut did = vec![tick(&mut sim, GAS | Control::Down.mask())];
    let mut kept = None;
    for _ in 0..40 {
        did.push(tick(&mut sim, GAS | Control::Down.mask()));
        if did.last() == Some(&Some(Did::Placed(0))) {
            kept = Some(hash::of(&sim.rider.ride));
        }
    }
    let at = hold_tick();
    assert_eq!(at, 7, "the 30 Hz clamp crosses 200 ms on the hold's eighth tick");
    assert_eq!(fired(&did), vec![(at, Did::Placed(0))], "one placement, on the tick the hold crossed 200 ms");
    let s = sim.input.pad(0).control(Control::Down);
    assert!(s.held_ms > 1000, "held {} ms", s.held_ms);
    let marker = sim.markers.slots[0].as_ref().expect("a marker in slot 1");
    let ride = marker.ride().expect("a bike's marker");
    assert_eq!(Some(hash::of(&**marker)), kept, "the marker is the bike as the firing tick left it");
    assert!(F(ride.chassis.vel.z()).abs() > F(1.0), "the marker kept no speed");
    assert!((1..SLOTS).all(|s| !sim.markers.holds(s)));
    // Each hold is one more: let go, a second hold places again, a held Right steps once, on its press.
    assert_eq!(tick(&mut sim, GAS), None, "the release fired");
    assert_eq!(what(&hold(&mut sim, Control::Down, GAS, HOLD_MS)), vec![Did::Placed(0)]);
    assert_eq!(fired(&hold(&mut sim, Control::Right, GAS, 1000)), vec![(0, Did::Stepped(Pick::Slot(1)))]);
    assert_eq!(fired(&hold(&mut sim, Control::Up, GAS, HOLD_MS)), vec![(at, Did::Refused(Pick::Slot(1)))]);
}

/// P106: Down held 199 ms places nothing, and 200 ms places once however long it is held after; Up
/// the same with the marker, and a tap of either does nothing. The 30 Hz clamp steps the timer past
/// both, so a 1 ms tick ([`game_sim::tick::DeltaTime::FLOOR`]) brings it to each.
#[test]
fn down_held_199_ms_places_nothing_and_200_ms_once() {
    let ms = F(0.001);
    let down = GAS | Control::Down.mask();
    let mut sim = moving();
    let timer = |sim: &Sim, c: Control| sim.input.pad(0).control(c).held_ms;
    // Up to 199 ms: the 30 Hz ticks, then 1 ms ones.
    let to_199 = |sim: &mut Sim, buttons: u16, c: Control| {
        let mut did = vec![tick(sim, buttons)];
        while timer(sim, c) + 34 < 199 {
            did.push(tick(sim, buttons));
        }
        while timer(sim, c) < 199 {
            did.push(tick_dt(sim, buttons, ms));
        }
        assert_eq!(timer(sim, c), 199);
        did
    };
    let did = to_199(&mut sim, down, Control::Down);
    assert_eq!(fired(&did), vec![], "199 ms placed");
    assert_eq!(tick(&mut sim, GAS), None, "the release fired");
    assert!(sim.markers.fresh(), "199 ms placed");
    // A tap does nothing either.
    assert_eq!(fired(&hold(&mut sim, Control::Down, GAS, 0)), vec![]);
    assert!(sim.markers.fresh());

    let mut did = to_199(&mut sim, down, Control::Down);
    did.push(tick_dt(&mut sim, down, ms));
    assert_eq!(timer(&sim, Control::Down), 200);
    assert_eq!(fired(&did), vec![(did.len() - 1, Did::Placed(0))], "200 ms, the tick it read 200");
    let kept = hash::of(&**sim.markers.slots[0].as_ref().unwrap());
    let rest: Vec<Option<Did>> = (0..90).map(|_| tick(&mut sim, down)).collect();
    assert_eq!(fired(&rest), vec![], "a hold fired twice");
    assert!(timer(&sim, Control::Down) > 3000);
    assert_eq!(hash::of(&**sim.markers.slots[0].as_ref().unwrap()), kept);
    tick(&mut sim, GAS);

    // Up the same: 199 ms rides nowhere, 200 ms rides back.
    for _ in 0..30 {
        tick(&mut sim, GAS);
    }
    let up = GAS | Control::Up.mask();
    let did = to_199(&mut sim, up, Control::Up);
    assert_eq!(fired(&did), vec![], "199 ms teleported");
    assert!(!sim.rider.ghost.on);
    tick(&mut sim, GAS);
    let mut did = to_199(&mut sim, up, Control::Up);
    did.push(tick_dt(&mut sim, up, ms));
    assert_eq!(fired(&did), vec![(did.len() - 1, Did::Teleported(Pick::Slot(0)))]);
    assert!(sim.rider.ghost.on);
}

/// Left fires on its first tick and Right on its, with no hold, and once however long they are held.
#[test]
fn left_fires_on_its_first_tick() {
    let mut sim = moving();
    assert_eq!(tick(&mut sim, GAS | Control::Left.mask()), Some(Did::Stepped(Pick::Slot(SLOTS - 1))));
    let rest: Vec<Option<Did>> = (0..30).map(|_| tick(&mut sim, GAS | Control::Left.mask())).collect();
    assert_eq!(fired(&rest), vec![]);
    tick(&mut sim, GAS);
    assert_eq!(tick(&mut sim, GAS | Control::Right.mask()), Some(Did::Stepped(Pick::Slot(0))));
}

/// The slot display's bar fills with an Up or Down hold ([`markers::holding`]): empty on the press,
/// part-way through the hold, full from 200 ms on — and Left and Right, on the press, leave it empty.
#[test]
fn the_bar_fills_with_an_up_or_down_hold() {
    let hands = Controls::default();
    let holding = |sim: &Sim| markers::holding(sim.input.pad(0), sim.table(), &hands);
    for c in [Control::Up, Control::Down] {
        let mut sim = Sim::new(0x88);
        let mut seen = vec![];
        for _ in 0..10 {
            tick(&mut sim, c.mask());
            seen.push(holding(&sim));
        }
        assert_eq!(seen[0], 0, "{c:?}: the press");
        assert!(seen.windows(2).all(|w| w[0] <= w[1]), "{c:?}: {seen:?}");
        assert!(seen[3] > 0 && seen[3] < HOLD_MS, "{c:?}: {seen:?}");
        assert_eq!(*seen.last().unwrap(), HOLD_MS, "{c:?}: {seen:?}");
        tick(&mut sim, 0);
        assert_eq!(holding(&sim), 0, "{c:?}: let go");
        // With a trick button down it is the gesture's, and nothing fills.
        let mut trick = Controls::default();
        trick.trick[0] = true;
        tick(&mut sim, c.mask());
        tick(&mut sim, c.mask());
        assert_eq!(markers::holding(sim.input.pad(0), sim.table(), &trick), 0);
    }
    for c in [Control::Left, Control::Right] {
        let mut sim = Sim::new(0x88);
        for _ in 0..10 {
            tick(&mut sim, c.mask());
        }
        assert_eq!(holding(&sim), 0, "{c:?} has no hold");
    }
    // The bar drawn: the pixels the hold changes, against the same display with none.
    let draw = |h: u32| {
        let mut f = game_render::raster::Frame::new(640, 448, [20, 60, 120]);
        hud::draw_markers(&mut f, Pick::Slot(0), [false; SLOTS], "marker 1 empty", &[], h);
        f
    };
    let empty = draw(0);
    let lit = |h: u32| draw(h).color.chunks_exact(3).zip(empty.color.chunks_exact(3)).filter(|(a, b)| a != b).count();
    let (half, full, over) = (lit(HOLD_MS / 2), lit(HOLD_MS), lit(5 * HOLD_MS));
    assert!(half > 0, "half a hold lit nothing");
    assert!(full > half && full <= 2 * half + 2, "half {half}, full {full}");
    assert_eq!(over, full, "past the hold the bar stays full");
}

/// A held d-pad moves no bike (P93): the stick alone steers. Each direction held for two seconds on
/// the gas rides the bike exactly as the gas alone does, tick for tick — where the original's reading
/// of the same pad ([`Sim::poll`], the d-pad a digital stick) leans it off that line.
#[test]
fn a_held_dpad_moves_no_bike() {
    let start = moving();
    let mut plain = start.clone();
    let rides: Vec<u64> = (0..60)
        .map(|_| {
            tick(&mut plain, GAS);
            hash::of(&plain.rider.ride)
        })
        .collect();
    for d in [Control::Left, Control::Right, Control::Up, Control::Down] {
        let mut sim = start.clone();
        for (k, want) in rides.iter().enumerate() {
            tick(&mut sim, GAS | d.mask());
            assert_eq!(hash::of(&sim.rider.ride), *want, "{d:?} held moved the bike on tick {k}");
        }
        // The control: the original's hands, d-pad and all, ride somewhere else.
        let mut original = start.clone();
        for _ in 0..60 {
            let c = original.poll(DT, &pads(GAS | d.mask()));
            original.step(DT, &c, &mut Flat::default());
        }
        assert_ne!(hash::of(&original.rider.ride), *rides.last().unwrap(), "{d:?}: the d-pad does not ride even unmasked");
    }
}

/// A hold with a trick button down is the trick's gesture (the d-pad names its direction, P73) and
/// fires nothing — and the bike reads the d-pad exactly as the original's hands do ([`Sim::poll`]):
/// with a trick button down it is not taken out of the ride (P93).
#[test]
fn a_trick_gesture_is_not_a_marker() {
    let mut sim = moving();
    let start = sim.clone();
    let mut plain = sim.clone();
    let circle = Control::Circle.mask(); // trick 1, the shipped preset's
    let did = hold(&mut sim, Control::Down, GAS | circle, 1000);
    assert_eq!(fired(&did), vec![]);
    assert!(sim.markers.fresh());
    // A ride that never had a marker step: the same pads into the same state, ridden on the plain
    // step alone, hash the same.
    let mut hands = Vec::new();
    for k in 0..did.len() {
        let b = if k + 1 < did.len() { GAS | circle | Control::Down.mask() } else { GAS | circle };
        let c = plain.poll(DT, &pads(b));
        plain.step(DT, &c, &mut Flat::default());
        hands.push(c);
    }
    assert_eq!(plain.hash(), sim.hash(), "a gesture's hold changed the ride");
    // The control: the same with no d-pad is another ride, so the gesture's d-pad was read.
    let mut circle_alone = start.clone();
    for _ in 0..did.len() {
        let c = circle_alone.poll(DT, &pads(GAS | circle));
        circle_alone.step(DT, &c, &mut Flat::default());
    }
    assert_ne!(circle_alone.hash(), sim.hash(), "the gesture's d-pad did nothing to the ride");
}

/// Up pressed lands the rider on the marker: the bike exactly as it was kept, its speed too, the
/// rider back on and ghosted (P57's poof). The tick after rides as the tick after the placement did.
#[test]
fn up_lands_on_the_marker_ghosted() {
    let mut sim = moving();
    let did = hold(&mut sim, Control::Down, GAS, HOLD_MS);
    assert_eq!(what(&did), vec![Did::Placed(0)]);
    // The placement was on the hold's last tick; the tick after it let go. Ride the one after that
    // again from a copy of the marker for the comparison below.
    let marker = sim.markers.slots[0].as_ref().unwrap().ride().unwrap().clone();
    // Away from it, and down in a heap: the teleport takes the rider out of a wreck too.
    for _ in 0..60 {
        tick(&mut sim, GAS);
    }
    let away = sim.bike_pos();
    assert!(F(away.z() - marker.origin.z()).abs() > F(5.0));
    sim.rider.sense.wrecks_enabled = true;
    assert!(!sim.rider.ghost.on);

    let mut did = vec![tick(&mut sim, Control::Up.mask())];
    while did.last() == Some(&None) {
        did.push(tick(&mut sim, Control::Up.mask()));
    }
    assert_eq!(fired(&did), vec![(hold_tick(), Did::Teleported(Pick::Slot(0)))]);
    assert_eq!(hash::of(&sim.rider.ride), hash::of(&marker), "the bike is not the marker's");
    assert_eq!(sim.bike_pos().to_bits(), marker.origin.to_bits());
    assert_eq!(sim.bike_vel().to_bits(), marker.chassis.vel.to_bits(), "the run-up's speed was not kept");
    assert!(sim.rider.ghost.on, "not ghosted");
    assert_eq!(sim.rider.ghost.alpha, game_sim::ghost::Ghost::START, "not the put-back's half see-through");
    assert!(sim.rider.state.wreck.is_none() && sim.rider.crash.is_none());
    assert!(sim.markers.holds(0), "a teleport keeps the marker");
    // The ghost runs its whole fade from here, as after a put-back.
    let mut ticks = 0;
    while sim.rider.ghost.on {
        tick(&mut sim, 0);
        ticks += 1;
        assert!(ticks <= 91);
    }
    assert_eq!(ticks, 91);
}

/// A marker repeats: the tick after a teleport is the tick after the placement, bit for bit, on the
/// same hands.
#[test]
fn the_tick_after_a_teleport_is_the_tick_after_the_placement() {
    let mut sim = moving();
    hold(&mut sim, Control::Down, GAS, HOLD_MS);
    let mut placed = sim.clone();
    // `hold` let go on the tick after the placement: ride the placement's state forward again from
    // the marker itself, then compare that tick with one more after a teleport.
    placed.rider.ride = sim.markers.slots[0].as_ref().unwrap().ride().unwrap().clone();
    let mut a = placed.clone();
    tick(&mut a, GAS);
    for _ in 0..30 {
        tick(&mut sim, GAS);
    }
    let did = hold(&mut sim, Control::Up, GAS, HOLD_MS);
    assert_eq!(what(&did), vec![Did::Teleported(Pick::Slot(0))]);
    // `hold` rode one more tick after the teleport, on gas: so did `a` after the placement.
    let mut b = placed.clone();
    b.rider.teleport(sim.markers.slots[0].as_ref().unwrap().ride().unwrap().clone());
    tick(&mut b, GAS);
    assert_eq!(hash::of(&a.rider.ride), hash::of(&b.rider.ride), "the run-up did not repeat");
}

/// Left and Right step through the five slots, then each other player, then round; each player
/// sees everybody but themselves.
#[test]
fn left_and_right_reach_every_slot_and_then_the_other_players() {
    let mut sim = three_riders();
    let mut seen = vec![sim.markers.pick(0, 3)];
    for _ in 0..SLOTS + 2 {
        let did = hold(&mut sim, Control::Right, 0, 0);
        let [(0, Did::Stepped(p))] = fired(&did)[..] else { panic!("{did:?}") };
        seen.push(p);
    }
    let slots = (0..SLOTS).map(Pick::Slot);
    let want: Vec<Pick> = slots.chain([Pick::Player(1), Pick::Player(2), Pick::Slot(0)]).collect();
    assert_eq!(seen, want);
    // Left from the first slot is the last player.
    let did = hold(&mut sim, Control::Left, 0, 0);
    assert_eq!(fired(&did), vec![(0, Did::Stepped(Pick::Player(2)))]);
    // Player 2's own list leaves them out.
    let m = markers::Markers { at: SLOTS as u8, ..Default::default() };
    assert_eq!((m.pick(1, 3), markers::Markers { at: SLOTS as u8 + 1, ..Default::default() }.pick(1, 3)),
               (Pick::Player(0), Pick::Player(2)));
    // A selection past a player who left is the first slot.
    assert_eq!(markers::Markers { at: SLOTS as u8 + 1, ..Default::default() }.pick(0, 2), Pick::Slot(0));
    // Down on a player places nothing.
    hold(&mut sim, Control::Left, 0, 0);
    assert_eq!(sim.markers.pick(0, 3), Pick::Player(1));
    assert_eq!(what(&hold(&mut sim, Control::Down, 0, HOLD_MS)), vec![Did::Refused(Pick::Player(1))]);
    assert!((0..SLOTS).all(|s| !sim.markers.holds(s)));
}

/// Three riders in a row on the flat, 12 ft apart, wrecks off (the stand-in's own first-tick wreck,
/// `Sim::new`).
fn three_riders() -> Sim {
    let mut sim = Sim::new(0x88);
    let mounts = stock::mounts(&sim.phy);
    for i in 1..3 {
        let at = Vf::new(stock::SPAWN.x() + 12.0 * i as f32, stock::SPAWN.y(), stock::SPAWN.z(), 1.0);
        let mut r = Rider::new(&sim.phy, mounts, at, stock::FACING, Set::default());
        r.sense.wrecks_enabled = false;
        let phy = (*sim.phy).clone();
        sim.add_player(phy, r);
    }
    sim
}

/// Up on another player lands the rider where they stand, ghosted, and the two collide with nothing:
/// stepped together for a second, inside each other, each rides exactly as it does with the other
/// far away — and the ghost holds its clock while player 2 pulls out of it, so they were inside each
/// other.
#[test]
fn a_teleport_onto_another_rider_collides_with_nothing() {
    let mut sim = three_riders();
    for _ in 0..SLOTS {
        hold(&mut sim, Control::Right, 0, 0);
    }
    assert_eq!(sim.markers.pick(0, 3), Pick::Player(1));
    // Player 2 on the move, so the landing is on a rider and not a parked bike.
    sim.others[0].rider.ride.chassis.vel = Vf::new(0.0, 0.0, 6.0, 0.0);
    let mut did = vec![tick(&mut sim, Control::Up.mask())];
    while did.last() == Some(&None) {
        did.push(tick(&mut sim, Control::Up.mask()));
    }
    assert_eq!(fired(&did), vec![(hold_tick(), Did::Teleported(Pick::Player(1)))]);
    // Where player 2 stood before the tick (the spots are taken before anybody moves), facing their way.
    assert!(sim.rider.ghost.on);
    let (me, them) = (sim.rider.ride.position(), sim.others[0].rider.ride.position());
    assert!(F(me.z() - them.z()).abs() < F(1.0) && F(me.x() - them.x()).abs() < F(0.01), "{me:?} {them:?}");
    assert!(game_sim::ghost::overlaps(&sim.rider.ride.rival(), &[sim.others[0].rider.ride.rival()]));
    // Not ghosted, the same two bodies strike: the head ray finds the other rider.
    let hits = game_sim::ride::hits_among(&sim.rider.ride, &mut Flat::default(), &[sim.others[0].rider.ride.rival()]);
    assert!(hits.iter().any(|c| matches!(c.other, game_sim::air::Other::Rider { .. })), "the control: no contact");

    // Together, against the same two with player 2 put 500 ft off: nobody rides any differently.
    let mut apart = sim.clone();
    let far = Vf::new(stock::SPAWN.x() + 500.0, stock::SPAWN.y(), stock::SPAWN.z(), 1.0);
    let mut solo_two = sim.clone();
    apart.others[0].rider.ride.place(far, stock::FACING);
    solo_two.rider.ride.place(far, stock::FACING);
    solo_two.rider.ghost = Default::default();
    for k in 0..30 {
        for s in [&mut sim, &mut apart, &mut solo_two] {
            tick(s, GAS);
        }
        assert_eq!(hash::of(&sim.rider.ride), hash::of(&apart.rider.ride), "tick {k}: the ghost touched player 2");
        assert_eq!(hash::of(&sim.others[0].rider.ride), hash::of(&solo_two.others[0].rider.ride), "tick {k}: player 2 touched the ghost");
        assert!(sim.rider.state.wreck.is_none() && sim.others[0].rider.state.wreck.is_none());
        if k < 5 {
            assert_eq!(sim.rider.ghost.clock, game_sim::ghost::Ghost::LENGTH, "tick {k}: the clock ran inside player 2");
        }
    }
}

/// The markers are the simulation's: a snapshot's bytes carry them and put them back, and a state
/// that never touched them encodes to the bytes and hashes to the number it did before there were any.
#[test]
fn markers_go_with_a_snapshot_and_cost_nothing_untouched() {
    let mut sim = moving();
    let untouched = game_sim::snapshot::encode(&sim);
    let before = sim.hash();
    let mut copy = sim.clone();
    copy.markers = Default::default();
    assert_eq!(game_sim::snapshot::encode(&copy), untouched);
    assert_eq!(copy.hash(), before);

    hold(&mut sim, Control::Down, GAS, HOLD_MS);
    hold(&mut sim, Control::Right, GAS, 0);
    assert_ne!(sim.hash(), {
        let mut s = sim.clone();
        s.markers = Default::default();
        s.hash()
    }, "the markers are not in the hash");
    let bytes = game_sim::snapshot::encode(&sim);
    let mut back = moving();
    game_sim::snapshot::decode(&mut back, &bytes).unwrap();
    assert_eq!(back.hash(), sim.hash());
    assert!(back.markers.holds(0) && back.markers.at == 1);

    // With other players too: their section and the markers' both read back.
    let mut three = three_riders();
    hold(&mut three, Control::Down, 0, HOLD_MS);
    let bytes = game_sim::snapshot::encode(&three);
    let mut back = three_riders();
    game_sim::snapshot::decode(&mut back, &bytes).unwrap();
    assert_eq!(back.hash(), three.hash());
}

/// One tick as [`tick`], player 1's markers told who is in the ride (`riding`, P94).
fn tick_among(sim: &mut Sim, buttons: u16, riding: &[bool]) -> Option<Did> {
    let spots: Vec<Spot> = (0..sim.players()).map(|i| Spot::of(sim.rider_of(i))).collect();
    let mut hands: Vec<Controls> = sim.poll_all(DT, &pads(buttons)).into_iter().map(Controls::stick_alone).collect();
    hands.resize(sim.players(), Controls::default());
    sim.step_all(DT, &hands, &mut Flat::default());
    markers::frame_among(sim, 0, &spots, riding, &hands[0])
}

/// P94: a seat not in the ride is never a pick. With seat 2 of three empty, Right steps through the
/// five slots, then seat 3, then round, and Left from the first slot is seat 3 — seat 2 is never on
/// it — and every other player's list leaves it out too. A selection left on a seat that stops
/// riding moves to the next rider, or to the first slot when there is none.
#[test]
fn a_seat_not_riding_is_never_a_pick() {
    let mut sim = three_riders();
    let riding = [true, false, true];
    let mut seen = vec![sim.markers.pick_among(0, &riding)];
    for _ in 0..SLOTS + 2 {
        let did = [tick_among(&mut sim, Control::Right.mask(), &riding), tick_among(&mut sim, 0, &riding)];
        let [(0, Did::Stepped(p))] = fired(&did)[..] else { panic!("{did:?}") };
        seen.push(p);
    }
    let want: Vec<Pick> = (0..SLOTS).map(Pick::Slot).chain([Pick::Player(2), Pick::Slot(0), Pick::Slot(1)]).collect();
    assert_eq!(seen, want);
    for _ in 0..2 {
        tick_among(&mut sim, Control::Left.mask(), &riding);
        tick_among(&mut sim, 0, &riding);
    }
    assert_eq!(sim.markers.pick_among(0, &riding), Pick::Player(2), "Left from the first slot");
    assert_eq!(markers::Markers::others(0, 3, &riding), vec![2]);
    assert_eq!(markers::Markers::others(2, 3, &riding), vec![0]);
    // Up on the empty seat cannot happen; Up on seat 3 lands there.
    let mut did = vec![tick_among(&mut sim, Control::Up.mask(), &riding)];
    while did.len() < 30 && did.last() == Some(&None) {
        did.push(tick_among(&mut sim, Control::Up.mask(), &riding));
    }
    assert_eq!(fired(&did), vec![(hold_tick(), Did::Teleported(Pick::Player(2)))]);

    // Everybody riding, the first place past the slots is seat 2; with seat 2 out it is seat 3, and
    // with both out the first slot.
    let m = markers::Markers { at: SLOTS as u8, ..Default::default() };
    assert_eq!(m.pick_among(0, &[true; 3]), Pick::Player(1));
    assert_eq!(m.pick_among(0, &riding), Pick::Player(2));
    assert_eq!(m.pick_among(0, &[true, false, false]), Pick::Slot(0));
    // Nobody else in the ride: the slots alone, round.
    let mut alone = three_riders();
    for _ in 0..SLOTS {
        tick_among(&mut alone, Control::Right.mask(), &[true, false, false]);
        tick_among(&mut alone, 0, &[true, false, false]);
    }
    assert_eq!(alone.markers.pick_among(0, &[true, false, false]), Pick::Slot(0));
}

/// P94, online: a seat nobody sits in has the missing pad (`net::Game::step`), and the session's
/// tick ([`net::rides::Rides`]) leaves it out of the markers' places — the same count on every
/// machine, as the pads are the session's. Seven presses of Right from the first slot are round to
/// the second slot with seat 2 empty, and to the first with everybody in.
#[test]
fn online_an_empty_seat_is_never_a_pick() {
    use net::Game;
    let ride = |seat2: Report| {
        let first = Sim::new(1);
        let mut sims: Vec<Sim> = (0..3).map(|_| first.clone()).collect();
        let mut world = Flat::default();
        let mut rides = net::rides::Rides { sims: &mut sims, world: &mut world, after: None, time_scale: None, plane: None };
        let mut seen = Vec::new();
        for t in 0..2 * (SLOTS + 2) {
            let me = if t % 2 == 0 { Report::from_buttons(Control::Right.mask()) } else { Report::from_buttons(0) };
            rides.step(t, &[me, seat2, Report::from_buttons(0)]);
            if t % 2 == 0 {
                seen.push(rides.sims[0].markers.at);
            }
        }
        seen
    };
    // Seven places with everybody in (five slots, two players), round to the first on the seventh.
    assert_eq!(ride(Report::from_buttons(0)), vec![1, 2, 3, 4, 5, 6, 0], "everybody riding");
    assert_eq!(ride(Report::missing()), vec![1, 2, 3, 4, 5, 0, 1], "seat 2 empty: one place fewer, round");
}

/// P94: a filled slot has a radar mark and an empty one has none — each in its slot's colour, at
/// where its bike was put down, the selected one ringed — and a post in the world that stands over
/// it in a view looking at it, and not in one looking away.
#[test]
fn a_filled_slot_has_a_radar_mark() {
    let mut sim = moving();
    hold(&mut sim, Control::Down, GAS, HOLD_MS);
    for _ in 0..2 {
        hold(&mut sim, Control::Right, GAS, 0);
    }
    for _ in 0..15 {
        tick(&mut sim, GAS);
    }
    hold(&mut sim, Control::Down, GAS, HOLD_MS);
    assert!(sim.markers.holds(0) && sim.markers.holds(2) && !sim.markers.holds(1));
    let pick = sim.markers.pick(0, 1);
    assert_eq!(pick, Pick::Slot(2));
    let blips = overlay::marker_blips(&sim.markers, pick);
    let got: Vec<(usize, bool)> = blips
        .iter()
        .map(|b| match b.icon {
            overlay::Icon::Marker { slot, selected } => (slot, selected),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(got, vec![(0, false), (2, true)], "a mark a filled slot, the selected one ringed");
    for (b, (slot, _)) in blips.iter().zip(&got) {
        let at = sim.markers.slots[*slot].as_ref().unwrap().position();
        assert_eq!(b.at, [at.x(), at.z()]);
        let span = overlay::radar_span(2.0);
        let placed = overlay::radar_icon([at.x(), at.z() - 20.0], [0.0, 1.0], *b, span);
        assert_eq!(placed.tint, overlay::MARKER_COLOURS[*slot]);
        assert!((placed.at.1 - overlay::RADAR_AT.1) < -1.0, "a marker ahead is drawn above the middle: {placed:?}");
    }
    assert_ne!(overlay::MARKER_COLOURS[0], overlay::MARKER_COLOURS[2]);
    assert!(overlay::marker_blips(&markers::Markers::default(), Pick::Slot(0)).is_empty());

    // Its post, from 60 ft behind the marker looking at it, and from beside it looking away.
    let at = sim.markers.slots[0].as_ref().unwrap().position();
    let foot = game_render::v3(at.x(), at.y(), at.z());
    let toward = game_render::world::Camera { eye: game_render::v3(at.x(), at.y() + 8.0, at.z() - 60.0), at: foot, fov: 1.0, near: 0.05, far: 4000.0 };
    let post = overlay::marker_post(&toward, (640, 448), [at.x(), at.y(), at.z()]).expect("a post in view");
    assert!(post.top.1 < post.foot.1 - 20.0, "the post stands up: {post:?}");
    assert!((post.foot.0 - 320.0).abs() < 2.0 && (post.top.0 - 320.0).abs() < 2.0, "{post:?}");
    assert!((post.distance - 60.5).abs() < 1.0);
    let away = game_render::world::Camera { at: game_render::v3(at.x(), at.y() + 8.0, at.z() - 120.0), ..toward };
    assert_eq!(overlay::marker_post(&away, (640, 448), [at.x(), at.y(), at.z()]), None, "behind the eye");
}

// ---------------------------------------------------------------------------------------------------
// P107: a set per machine.
// ---------------------------------------------------------------------------------------------------

use vehicle::{Garage, Kind, Plane, Spawn};

/// The disc's biplane, or `None` (said) without a disc.
fn the_discs_plane() -> Option<game_sim::flight::Flight> {
    use game_render::disc::Disc;
    let mut disc = match Disc::open_configured() {
        Ok(disc) => disc,
        Err(e) => {
            eprintln!("skipped: {e}");
            return None;
        }
    };
    let name = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("machine.res"))?.name.clone();
    let res = disc.res(&name).expect("machine.res");
    let bytes = res.read_suffix(game_sim::phy::BIPLANE_PHY).expect("machine.res carries biplane.phy");
    Some(game_sim::flight::Flight::new(&game_sim::phy::Plane::read_bytes(&bytes)))
}

/// A stand-in bike and the disc's plane parked beside it on the flat floor, the garage on `up`.
fn hangar(up: Kind) -> Option<(Sim, Garage)> {
    let flight = the_discs_plane()?;
    let lowest = flight.tires.iter().map(|t| t.center.y() - t.radius.0).fold(f32::MAX, f32::min);
    let spawn = Spawn { at: Vf::new(400.0, -lowest, -300.0, 1.0), facing: Vf::new(0.0, 0.0, -1.0, 0.0) };
    Some((Sim::new(0x107), Garage::new(up, Some(Plane::flying(spawn, flight))).unwrap()))
}

/// One shell pass for player 1 as `Seat::pass` runs it: the poll with the d-pad out of the ride, the
/// garage's step on whichever machine is up, then the bike's markers or the plane's.
fn pass(sim: &mut Sim, garage: &mut Garage, report: Report) -> Option<Did> {
    let mut world = Flat::default();
    let spots = [garage.spot(sim, &mut world)];
    let reports = [report, Report::missing()];
    let controls = sim.poll(DT, &reports).stick_alone();
    garage.step(sim, DT, &controls, &mut world);
    if garage.up == Kind::Bike {
        return markers::frame(sim, 0, &spots, &controls);
    }
    garage.markers(sim, 0, &spots, &[], &mut world)
}

/// Down or Up held through [`pass`]es until it fires at [`HOLD_MS`] (P106), the way a player puts a
/// marker down or rides back: what fired, or `None` if nothing did in a second.
fn hold_pass(sim: &mut Sim, garage: &mut Garage, buttons: u16) -> Option<Did> {
    (0..30).find_map(|_| pass(sim, garage, Report::from_buttons(buttons)))
}

fn flight(garage: &Garage) -> &game_sim::flight::Flight {
    garage.plane.as_ref().and_then(|p| p.flight.as_ref()).expect("a flight")
}

fn stick(buttons: u16, y: u8) -> Report {
    let mut r = Report::from_buttons(buttons);
    r.set_axis(3, y);
    r
}

/// Thirty seconds of gas from the spawn: the plane in the air.
fn airborne(sim: &mut Sim, garage: &mut Garage) {
    for _ in 0..30 * 30 {
        pass(sim, garage, Report::from_buttons(GAS));
    }
    let plane = garage.plane.as_ref().unwrap();
    assert!(!plane.on_ground() && flight(garage).crash.reason == 0, "thirty seconds of gas and the plane is not flying");
}

/// P107: a plane's marker is its whole flight. Down in the air keeps the flight as the tick left it;
/// flown away and wrecked into the floor, Up puts the plane back into it — out of the wreck, in the
/// air, bit for bit — and the ticks after it are the ticks after the placement flown again, hash
/// for hash. And the d-pad flies nothing: the plane on the same stick with the d-pad held is the
/// same plane.
#[test]
fn a_plane_marker_restores_the_flight_bit_for_bit() {
    let Some((mut sim, mut garage)) = hangar(Kind::Biplane) else { return };
    airborne(&mut sim, &mut garage);
    assert_eq!(hold_pass(&mut sim, &mut garage, GAS | Control::Down.mask()), Some(Did::Placed(0)));
    let kept = flight(&garage).clone();
    let mark = sim.markers.slots[0].as_deref().expect("a marker in slot 1");
    assert_eq!(mark.vehicle(), markers::Vehicle::Plane);
    assert!(mark.flight() == Some(&kept), "the marker is not the flight as the tick left it");
    assert_eq!(hash::of(mark), hash::of(&kept));
    assert!(!sim.markers.any_on(markers::Vehicle::Bike), "the plane's marker went in the bike's set");

    // The ride on from the placement, pulling: what the put-back has to fly again.
    let script: Vec<Report> = (0..40).map(|k| stick(GAS, if k < 20 { 0xff } else { 0x60 })).collect();
    let ride_on = |sim: &mut Sim, garage: &mut Garage| -> Vec<u64> {
        script.iter().map(|&r| {
            pass(sim, garage, r);
            hash::of(flight(garage))
        }).collect()
    };
    let (mut a, mut b) = (sim.clone(), garage.clone());
    let want = ride_on(&mut a, &mut b);
    // The same ride with the whole d-pad held is the same flight: the plane does not read it.
    let (mut a, mut b) = (sim.clone(), garage.clone());
    let dpad = Control::Left.mask() | Control::Up.mask();
    let with_dpad: Vec<u64> = script.iter().map(|r| {
        let mut held = Report::from_buttons(r.buttons() | dpad);
        held.set_axis(3, r.axis(3));
        pass(&mut a, &mut b, held);
        hash::of(flight(&b))
    }).collect();
    assert_eq!(with_dpad, want, "the d-pad flew the plane");

    // Away, nose down to the floor, and wrecked. The plane here has no ray probe (the disc's
    // collision object is not loaded), so the floor stops its wheels but nothing wrecks it: the wreck
    // is put on by hand, the state the put-back has to take it out of.
    for _ in 0..30 {
        pass(&mut sim, &mut garage, stick(GAS, 0xff));
    }
    let ticks = 90;
    for _ in 0..ticks {
        pass(&mut sim, &mut garage, stick(GAS, 0x00));
    }
    let wrecked_at = flight(&garage).pose().1;
    garage.plane.as_mut().unwrap().flight.as_mut().unwrap().crash.reason = 1;

    // Up: back into the flight it kept, out of the wreck, in the air.
    assert_eq!(hold_pass(&mut sim, &mut garage, GAS | Control::Up.mask()), Some(Did::Teleported(Pick::Slot(0))));
    assert!(*flight(&garage) == kept, "the put-back is not the flight it kept");
    assert_eq!(hash::of(flight(&garage)), hash::of(&kept));
    let plane = garage.plane.as_ref().unwrap();
    assert!(!plane.on_ground() && flight(&garage).crash.reason == 0);
    assert_eq!(plane.at.to_bits()[..3], markers::Mark::Plane(kept.clone()).position().to_bits()[..3], "drawn where it was put back");
    assert_ne!(wrecked_at.to_bits(), kept.pose().1.to_bits());
    assert_eq!(ride_on(&mut sim, &mut garage), want, "the ticks after the put-back are not the ticks after the placement");
    println!("plane marker: kept at {:?}, wrecked {ticks} ticks into a dive, put back and flown {} ticks bit for bit", kept.pose().1, want.len());
}

/// P107: the bike and the plane have five slots and a selection each, never shared. Switching
/// machine shows the other's set — its slots, its selection, its radar marks — and an Up on a slot
/// only ever finds that machine's marker.
#[test]
fn bike_and_plane_slots_do_not_share_and_a_switch_shows_the_other_set() {
    let Some((mut sim, mut garage)) = hangar(Kind::Bike) else { return };
    for _ in 0..45 {
        pass(&mut sim, &mut garage, Report::from_buttons(GAS));
    }
    // The bike: a marker in slot 1, the selection stepped to slot 2.
    assert_eq!(hold_pass(&mut sim, &mut garage, GAS | Control::Down.mask()), Some(Did::Placed(0)));
    pass(&mut sim, &mut garage, Report::from_buttons(GAS));
    assert_eq!(pass(&mut sim, &mut garage, Report::from_buttons(GAS | Control::Right.mask())), Some(Did::Stepped(Pick::Slot(1))));
    let bike_mark = hash::of(sim.markers.slots[0].as_deref().unwrap());
    assert!(sim.markers.slots[0].as_deref().unwrap().ride().is_some());

    // Onto the plane: its own set, empty and on its first slot.
    let mut world = Flat::default();
    garage.switch(Kind::Biplane, &mut sim, &mut world).unwrap();
    assert_eq!(sim.markers.on, markers::Vehicle::Plane);
    assert!(sim.markers.slots.iter().all(Option::is_none) && sim.markers.at == 0, "the plane shows the bike's set");
    assert!(overlay::marker_blips(&sim.markers, sim.markers.pick(0, 1)).is_empty(), "the bike's marker is on the plane's radar");
    for _ in 0..3 {
        pass(&mut sim, &mut garage, Report::from_buttons(GAS));
    }
    // Up on the plane's slot 1 finds nothing: the bike's marker there is not the plane's.
    assert_eq!(hold_pass(&mut sim, &mut garage, GAS | Control::Up.mask()), Some(Did::Refused(Pick::Slot(0))));
    // The plane's own: two steps right, a marker in slot 3.
    for _ in 0..2 {
        pass(&mut sim, &mut garage, Report::from_buttons(GAS | Control::Right.mask()));
        pass(&mut sim, &mut garage, Report::from_buttons(GAS));
    }
    assert_eq!(hold_pass(&mut sim, &mut garage, GAS | Control::Down.mask()), Some(Did::Placed(2)));
    let plane_mark = hash::of(sim.markers.slots[2].as_deref().unwrap());
    let blips = overlay::marker_blips(&sim.markers, sim.markers.pick(0, 1));
    assert_eq!(blips.len(), 1, "the plane's radar has its one marker");
    assert!(!sim.markers.holds(0) && sim.markers.holds(2) && sim.markers.at == 2);

    // Back on the bike: the bike's set as it was left, the plane's waiting beside it.
    pass(&mut sim, &mut garage, Report::from_buttons(GAS));
    garage.switch(Kind::Bike, &mut sim, &mut world).unwrap();
    assert_eq!(sim.markers.on, markers::Vehicle::Bike);
    assert!(sim.markers.holds(0) && !sim.markers.holds(2) && sim.markers.at == 1, "the bike's set is not as it was left");
    assert_eq!(hash::of(sim.markers.slots[0].as_deref().unwrap()), bike_mark);
    let plane_slots = sim.markers.slots_of(markers::Vehicle::Plane).expect("the plane's set kept");
    assert_eq!(plane_slots[2].as_deref().map(hash::of), Some(plane_mark));
    assert_eq!(overlay::marker_blips(&sim.markers, sim.markers.pick(0, 1)).len(), 1, "the bike's radar has its one marker");
    // Up on the bike's slot 1 rides the bike back; on the plane's slot 3 would be nothing here.
    pass(&mut sim, &mut garage, Report::from_buttons(GAS | Control::Left.mask()));
    pass(&mut sim, &mut garage, Report::from_buttons(GAS));
    assert_eq!(hold_pass(&mut sim, &mut garage, GAS | Control::Up.mask()), Some(Did::Teleported(Pick::Slot(0))));
    pass(&mut sim, &mut garage, Report::from_buttons(GAS));
    for _ in 0..2 {
        pass(&mut sim, &mut garage, Report::from_buttons(GAS | Control::Right.mask()));
        pass(&mut sim, &mut garage, Report::from_buttons(GAS));
    }
    assert_eq!(hold_pass(&mut sim, &mut garage, GAS | Control::Up.mask()), Some(Did::Refused(Pick::Slot(2))));
    // And back on the plane, its selection is where it was left too.
    garage.switch(Kind::Biplane, &mut sim, &mut world).unwrap();
    assert!(sim.markers.holds(2) && !sim.markers.holds(0) && sim.markers.at == 2);
}

/// P107: saved and loaded per machine. The plane's markers are kept in a file of their own beside
/// the bike's (`<profile>.biplane.markers`), named by the plane's `.phy`; a placement on one machine
/// writes only its file; a ride loads both back, each into its own set. A tape and a pick carry
/// both, and with the bike's alone carry the bytes they did before (P99's).
#[test]
fn saved_and_reloaded_per_machine() {
    let Some((mut sim, mut garage)) = hangar(Kind::Bike) else { return };
    assert_eq!(saved_markers::bike_word(game_sim::phy::BIPLANE_PHY), markers::PLANE, "the plane's markers are not named by its .phy");
    let dir = std::env::temp_dir().join(format!("openmx-p107-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    const TRACK: &str = "free06";
    const BIKE: &str = "race125p";
    let mut kept = saved_markers::Kept::new(TRACK, "1P", BIKE, &sim.markers);
    let bike_file = saved_markers::path_of(&dir, TRACK, "1P", markers::Vehicle::Bike);
    let plane_file = saved_markers::path_of(&dir, TRACK, "1P", markers::Vehicle::Plane);
    assert_eq!(bike_file, saved_markers::path(&dir, TRACK, "1P"));
    assert_ne!(bike_file, plane_file);

    for _ in 0..45 {
        pass(&mut sim, &mut garage, Report::from_buttons(GAS));
    }
    hold_pass(&mut sim, &mut garage, GAS | Control::Down.mask());
    assert!(kept.frame(&dir, &sim.markers).is_some());
    assert!(bike_file.exists() && !plane_file.exists(), "a bike marker wrote the plane's file");
    let bike_bytes = std::fs::read(&bike_file).unwrap();
    // The bike's alone on a tape or a pick: its file, as P99 carried it.
    assert_eq!(sim.markers.to_files(&saved_markers::names(BIKE)), sim.markers.to_file(BIKE));

    let mut world = Flat::default();
    garage.switch(Kind::Biplane, &mut sim, &mut world).unwrap();
    for _ in 0..3 {
        pass(&mut sim, &mut garage, Report::from_buttons(GAS));
    }
    pass(&mut sim, &mut garage, Report::from_buttons(GAS | Control::Right.mask()));
    hold_pass(&mut sim, &mut garage, GAS | Control::Down.mask());
    assert!(sim.markers.holds(1));
    assert_eq!(kept.frame(&dir, &sim.markers).as_deref().map(|l| l.lines().count()), Some(1), "one file written");
    assert!(plane_file.exists());
    assert_eq!(std::fs::read(&bike_file).unwrap(), bike_bytes, "a plane marker rewrote the bike's file");

    // A ride loads both, each its own machine's, on its first slot.
    let words = saved_markers::signatures(&sim.markers);
    let mut back = saved_markers::load(&dir, TRACK, "1P", BIKE);
    assert_eq!(saved_markers::signatures(&back), words);
    assert_eq!(back.on, markers::Vehicle::Bike);
    assert!(back.holds(0) && back.slots[0].as_deref().unwrap().ride().is_some());
    back.up(markers::Vehicle::Plane);
    assert!(back.holds(1) && !back.holds(0) && back.slots[1].as_deref().unwrap().flight().is_some());
    // The plane's kept flight is the flight, every field the codec carries.
    let live = sim.markers.slots[1].as_deref().unwrap().flight().unwrap();
    let read = back.slots[1].as_deref().unwrap().flight().unwrap();
    assert_eq!(hash::of(read), hash::of(live));
    // Another bike's file is not loaded; the plane's still is.
    let other = saved_markers::load(&dir, TRACK, "1P", "race250f");
    assert!(!other.any_on(markers::Vehicle::Bike) && other.any_on(markers::Vehicle::Plane));

    // A tape and a pick carry both.
    let tape = sim.markers.to_files(&saved_markers::names(BIKE));
    let (_, from_tape) = markers::Markers::read_file(&tape).unwrap();
    assert_eq!(saved_markers::signatures(&from_tape), words);
    let pick = saved_markers::to_pick(&sim.markers, BIKE, TRACK);
    assert_eq!(saved_markers::signatures(&saved_markers::from_pick(&pick, BIKE, TRACK)), words);
    let wire = saved_markers::to_wire(&sim.markers, BIKE, TRACK);
    assert_eq!(saved_markers::signatures(&saved_markers::from_wire(&wire, BIKE, TRACK)), words);
    // A cut bundle reads as nothing, never a crash.
    for cut in [8, 12, tape.len() / 2, tape.len() - 1] {
        assert!(markers::Markers::read_file(&tape[..cut]).is_err(), "cut at {cut} read");
    }
    // P99's version-1 bike file still reads, as the bike's.
    let mut v1 = sim.markers.to_file(BIKE);
    v1[8..12].copy_from_slice(&1u32.to_le_bytes());
    let kind_at = 16 + BIKE.len();
    v1.remove(kind_at);
    let old = markers::Markers::from_file(&v1, BIKE).expect("a P99 file");
    assert_eq!(saved_markers::signatures(&old)[0], words[0]);

    // Cleared, both files go.
    saved_markers::save(&dir, TRACK, "1P", BIKE, &Default::default()).unwrap();
    assert!(!bike_file.exists() && !plane_file.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// P107 online: a seat's plane markers are its `Sim`'s, as the bike's are — placed and put back
/// through the lane (`vehicle::fly_lane`, `vehicle::lane_markers`), carried by a snapshot both sets
/// at once, and hashed.
#[test]
fn online_a_planes_markers_ride_the_lane_and_the_snapshot() {
    let Some((mut sim, garage)) = hangar(Kind::Biplane) else { return };
    let template = garage.plane.as_ref().unwrap().flight.clone().unwrap();
    let bay = garage.plane.as_ref().unwrap().spawn;
    vehicle::start_lane(&template, bay, &mut sim);
    let wanted = vehicle::Wanted::default();
    let mut world = Flat::default();
    let mut tick = |sim: &mut Sim, report: Report| {
        let spots = [vehicle::lane_spot(sim, &mut world)];
        sim.poll(DT, &[report, Report::missing()]);
        vehicle::fly_lane(sim, DT, wanted, &mut world);
        vehicle::lane_markers(sim, 0, &spots, &[true], &mut world)
    };
    for _ in 0..30 * 30 {
        tick(&mut sim, Report::from_buttons(GAS));
    }
    assert_eq!((0..30).find_map(|_| tick(&mut sim, Report::from_buttons(GAS | Control::Down.mask()))), Some(Did::Placed(0)));
    let kept = sim.plane.clone().unwrap();
    let before = sim.hash();
    assert_ne!(before, { let mut s = sim.clone(); s.markers = Default::default(); s.hash() }, "the plane's markers are not in the hash");

    // A snapshot carries the plane's set and the bike's waiting one.
    let mut both = sim.clone();
    let mut bike_set = markers::Set { on: markers::Vehicle::Bike, ..Default::default() };
    bike_set.slots[3] = Some(Box::new(markers::Mark::Bike(sim.rider.ride.clone())));
    both.markers.put(bike_set);
    let bytes = game_sim::snapshot::encode(&both);
    let mut back = sim.clone();
    back.markers = Default::default();
    game_sim::snapshot::decode(&mut back, &bytes).unwrap();
    assert_eq!(back.hash(), both.hash());
    assert!(back.markers.slots_of(markers::Vehicle::Bike).unwrap()[3].is_some());

    for _ in 0..30 {
        tick(&mut sim, stick(GAS, 0x00));
    }
    assert_eq!((0..30).find_map(|_| tick(&mut sim, Report::from_buttons(GAS | Control::Up.mask()))), Some(Did::Teleported(Pick::Slot(0))));
    assert!(sim.plane.as_ref() == Some(&kept), "the lane's put-back is not the flight it kept");
}

// ---------------------------------------------------------------------------------------------------
// P124: the slot display on every machine.
// ---------------------------------------------------------------------------------------------------

use hud::{MarkerBanner, MarkerBar};
use std::time::{Duration, Instant};

/// [`pass`] for seat `me` of a ride of `garages.len()` local players, everybody riding, every spot
/// from before the tick: what fired and the hands the poll gave, which the shell keeps as the seat's
/// (`Seat::hands`) and reads the slot display with.
fn seat_pass(seats: &mut [(Sim, Garage)], me: usize, report: Report) -> (Option<Did>, Controls) {
    let mut world = Flat::default();
    let spots: Vec<Spot> = seats.iter().map(|(sim, garage)| garage.spot(sim, &mut world)).collect();
    let riding = vec![true; seats.len()];
    let (sim, garage) = &mut seats[me];
    let controls = sim.poll(DT, &[report, Report::missing()]).stick_alone();
    garage.step(sim, DT, &controls, &mut world);
    let did = match garage.up {
        Kind::Bike => markers::frame_among(sim, me, &spots, &riding, &controls),
        _ => garage.markers(sim, me, &spots, &riding, &mut world),
    };
    (did, controls)
}

/// Seat `me`'s slot display as the shell reads it (P124): offline, so every view shows its own.
fn bar(seats: &[(Sim, Garage)], me: usize, hands: &Controls) -> MarkerBar {
    assert!(hud::markers_shown_in(me, false));
    let sim = &seats[me].0;
    MarkerBar::read(&sim.markers, me, &vec![true; seats.len()], sim.input.pad(0), sim.table(), hands, true)
}

/// The label the display shows with it, offline (no names).
fn label(b: &MarkerBar) -> String {
    hud::marker_label(b.pick, b.filled, &[])
}

/// P124: flying the plane, the slot display is the plane's. A Left press puts the bar up on the
/// plane's last slot; a held Down fills the hold bar to `HOLD_MS` — with a trick button down too,
/// which on the plane is no gesture, as its markers read the pad — and the bar marks the slot
/// filled, with its radar icon; switching to the bike puts the bar up on the bike's own set, where
/// the same trick button makes the d-pad a gesture again and fills nothing.
#[test]
fn flying_the_plane_shows_the_planes_bar() {
    let Some(hangar) = hangar(Kind::Biplane) else { return };
    let mut seats = vec![hangar];
    let mut hands = Controls::default();
    let mut banner = MarkerBanner::default();
    let mut now = Instant::now();
    let step = Duration::from_millis(33);
    let mut show = |b: &MarkerBar, now: Instant| banner.update(now, b.pick, b.filled, b.held);
    for _ in 0..30 {
        hands = seat_pass(&mut seats, 0, Report::from_buttons(GAS)).1;
        now += step;
        assert!(!show(&bar(&seats, 0, &hands), now), "the bar is up untouched");
    }
    assert_eq!(seats[0].0.markers.on, markers::Vehicle::Plane);

    // Left: the plane's last slot, the bar up.
    let (did, h) = seat_pass(&mut seats, 0, Report::from_buttons(GAS | Control::Left.mask()));
    assert_eq!(did, Some(Did::Stepped(Pick::Slot(SLOTS - 1))));
    now += step;
    let b = bar(&seats, 0, &h);
    assert!(show(&b, now), "a Left press on the plane shows no bar");
    assert_eq!((b.pick, b.filled, b.held, b.holding), (Pick::Slot(SLOTS - 1), [false; SLOTS], true, 0));
    assert_eq!(label(&b), "marker 5 empty");
    seat_pass(&mut seats, 0, Report::from_buttons(GAS));

    // Down held, Circle (trick 1 on the bike) with it: the hold bar fills and the slot is set.
    let circle = Control::Circle.mask();
    let mut seen = vec![];
    let placed = (0..30).find_map(|_| {
        let (did, h) = seat_pass(&mut seats, 0, Report::from_buttons(GAS | circle | Control::Down.mask()));
        hands = h;
        let b = bar(&seats, 0, &hands);
        seen.push(b.holding);
        now += step;
        assert!(show(&b, now), "the bar is down while Down is held");
        did
    });
    assert_eq!(placed, Some(Did::Placed(SLOTS - 1)));
    assert!(seen.windows(2).all(|w| w[0] <= w[1]) && seen[0] == 0 && seen.iter().any(|&h| h > 0 && h < HOLD_MS), "{seen:?}");
    assert_eq!(*seen.last().unwrap(), HOLD_MS, "{seen:?}");
    let b = bar(&seats, 0, &hands);
    assert!(b.filled[SLOTS - 1] && b.filled.iter().filter(|&&f| f).count() == 1);
    assert_eq!(label(&b), "marker 5 set");
    assert_eq!(overlay::marker_blips(&seats[0].0.markers, b.pick).len(), 1, "the plane's marker has no radar icon");

    // Onto the bike: its own set, empty and on its first slot, and the bar up to say so.
    hands = seat_pass(&mut seats, 0, Report::from_buttons(GAS)).1;
    let (sim, garage) = &mut seats[0];
    garage.switch(Kind::Bike, sim, &mut Flat::default()).unwrap();
    now += step;
    let b = bar(&seats, 0, &hands);
    assert_eq!((b.pick, b.filled), (Pick::Slot(0), [false; SLOTS]), "the bike shows the plane's set");
    assert!(show(&b, now), "the switch's new set put no bar up");
    assert_eq!(label(&b), "marker 1 empty");
    assert!(overlay::marker_blips(&seats[0].0.markers, b.pick).is_empty());
    let mut seen = vec![];
    for _ in 0..10 {
        hands = seat_pass(&mut seats, 0, Report::from_buttons(GAS | circle | Control::Down.mask())).1;
        seen.push(bar(&seats, 0, &hands).holding);
    }
    assert!(seen.iter().all(|&h| h == 0), "the bike's trick gesture filled the hold bar: {seen:?}");
    assert!(!bar(&seats, 0, &hands).filled.iter().any(|&f| f));
}

/// P124 in a two-player split: player 1 rides the bike while player 2 flies the plane, each view its
/// own player's bar. Player 2's Left reaches player 1 ("to P1", the one other box), a second Left
/// the plane's last slot, and a held Down fills player 2's hold bar and sets the plane's slot —
/// player 1's bike bar staying empty all along. Online only this machine's view shows a bar.
#[test]
fn a_split_shows_each_players_own_machines_bar() {
    let (Some(one), Some(two)) = (hangar(Kind::Bike), hangar(Kind::Biplane)) else { return };
    let mut seats = vec![one, two];
    let mut hands = [Controls::default(), Controls::default()];
    let mut both = |seats: &mut [(Sim, Garage)], b2: u16| {
        hands[0] = seat_pass(seats, 0, Report::from_buttons(GAS)).1;
        let (did, h) = seat_pass(seats, 1, Report::from_buttons(b2));
        hands[1] = h;
        (did, bar(seats, 0, &hands[0]), bar(seats, 1, &hands[1]))
    };
    for _ in 0..30 {
        both(&mut seats, GAS);
    }
    let (did, _, b) = both(&mut seats, GAS | Control::Left.mask());
    assert_eq!(did, Some(Did::Stepped(Pick::Player(0))));
    assert_eq!(label(&b), "to P1");
    assert_eq!(markers::Markers::others(1, 2, &[true, true]), vec![0]);
    both(&mut seats, GAS);
    let (did, _, b) = both(&mut seats, GAS | Control::Left.mask());
    assert_eq!(did, Some(Did::Stepped(Pick::Slot(SLOTS - 1))));
    assert_eq!(label(&b), "marker 5 empty");
    both(&mut seats, GAS);
    let mut most = 0;
    let placed = (0..30).find_map(|_| {
        let (did, one, two) = both(&mut seats, GAS | Control::Down.mask());
        assert_eq!((one.pick, one.filled, one.holding), (Pick::Slot(0), [false; SLOTS], 0), "player 2's hold is on player 1's bar");
        most = most.max(two.holding);
        did
    });
    assert_eq!(placed, Some(Did::Placed(SLOTS - 1)));
    assert_eq!(most, HOLD_MS);
    let (_, one, two) = both(&mut seats, GAS);
    assert!(two.filled[SLOTS - 1] && !one.filled.iter().any(|&f| f));
    assert_eq!(seats[1].0.markers.on, markers::Vehicle::Plane);
    assert_eq!(seats[0].0.markers.on, markers::Vehicle::Bike);
    // Online: this machine's own view, the first, and no other.
    assert!(hud::markers_shown_in(0, true) && !hud::markers_shown_in(1, true));
}
