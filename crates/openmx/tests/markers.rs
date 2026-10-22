//! P88 — session markers on the d-pad, ridden the way the shell and the online session ride them: a
//! tick is the pad's poll with the d-pad out of the ride (`Controls::stick_alone`, P93), the step,
//! then `markers::frame` with every rider's spot from before the tick (`Seat::pass`,
//! `net::rides::Rides::step`). The stand-in bike on a flat floor, at the 30 Hz clamp. Since P93 all
//! four directions fire on the press, the tick a direction goes down.

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

use game_sim::input::{Control, PADS, Report};
use game_sim::markers::{self, Did, Pick, SLOTS, Spot};
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
    let spots: Vec<Spot> = (0..sim.players()).map(|i| Spot::of(sim.rider_of(i))).collect();
    let mut hands: Vec<Controls> = sim.poll_all(DT, &pads(buttons)).into_iter().map(Controls::stick_alone).collect();
    hands.resize(sim.players(), Controls::default());
    sim.step_all(DT, &hands, &mut Flat::default());
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

/// Down puts a marker in the selected slot on the press, the bike as that tick left it — speed and
/// all — and fires once however long it is held after.
#[test]
fn down_places_a_marker_on_the_press() {
    let mut sim = moving();
    let mut did = vec![tick(&mut sim, GAS | Control::Down.mask())];
    let kept = Some(hash::of(&sim.rider.ride));
    for _ in 0..40 {
        did.push(tick(&mut sim, GAS | Control::Down.mask()));
    }
    assert_eq!(fired(&did), vec![(0, Did::Placed(0))], "one placement, on the press");
    let s = sim.input.pad(0).control(Control::Down);
    assert!(s.held_ms > 1000, "held {} ms", s.held_ms);
    let marker = sim.markers.slots[0].as_ref().expect("a marker in slot 1");
    assert_eq!(Some(hash::of(&**marker)), kept, "the marker is the bike as the firing tick left it");
    assert!(F(marker.chassis.vel.z()).abs() > F(1.0), "the marker kept no speed");
    assert!((1..SLOTS).all(|s| !sim.markers.holds(s)));
    // Each press is one more: let go, a second press places again, a held Right steps once.
    assert_eq!(tick(&mut sim, GAS), None, "the release fired");
    assert_eq!(fired(&hold(&mut sim, Control::Down, GAS, 0)), vec![(0, Did::Placed(0))]);
    assert_eq!(fired(&hold(&mut sim, Control::Right, GAS, 1000)), vec![(0, Did::Stepped(Pick::Slot(1)))]);
    assert_eq!(fired(&hold(&mut sim, Control::Up, GAS, 0)), vec![(0, Did::Refused(Pick::Slot(1)))]);
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
    let did = hold(&mut sim, Control::Down, GAS, 0);
    assert_eq!(fired(&did), vec![(0, Did::Placed(0))]);
    // The placement was on the press; the tick after it let go. Ride the one after that
    // again from a copy of the marker for the comparison below.
    let marker = (**sim.markers.slots[0].as_ref().unwrap()).clone();
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
    assert_eq!(fired(&did), vec![(0, Did::Teleported(Pick::Slot(0)))]);
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
    hold(&mut sim, Control::Down, GAS, 0);
    let mut placed = sim.clone();
    // `hold` let go on the tick after the placement: ride the placement's state forward again from
    // the marker itself, then compare that tick with one more after a teleport.
    placed.rider.ride = (**sim.markers.slots[0].as_ref().unwrap()).clone();
    let mut a = placed.clone();
    tick(&mut a, GAS);
    for _ in 0..30 {
        tick(&mut sim, GAS);
    }
    let did = hold(&mut sim, Control::Up, GAS, 0);
    assert_eq!(fired(&did), vec![(0, Did::Teleported(Pick::Slot(0)))]);
    // `hold` rode one more tick after the teleport, on gas: so did `a` after the placement.
    let mut b = placed.clone();
    b.rider.teleport((**sim.markers.slots[0].as_ref().unwrap()).clone());
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
    assert_eq!(fired(&hold(&mut sim, Control::Down, 0, 0)), vec![(0, Did::Refused(Pick::Player(1)))]);
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
    assert_eq!(fired(&did), vec![(0, Did::Teleported(Pick::Player(1)))]);
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

    hold(&mut sim, Control::Down, GAS, 0);
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
    hold(&mut three, Control::Down, 0, 0);
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
    let did = [tick_among(&mut sim, Control::Up.mask(), &riding), tick_among(&mut sim, 0, &riding)];
    assert_eq!(fired(&did), vec![(0, Did::Teleported(Pick::Player(2)))]);

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
        let mut rides = net::rides::Rides { sims: &mut sims, world: &mut world, after: None, time_scale: None };
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
    hold(&mut sim, Control::Down, GAS, 0);
    for _ in 0..2 {
        hold(&mut sim, Control::Right, GAS, 0);
    }
    for _ in 0..15 {
        tick(&mut sim, GAS);
    }
    hold(&mut sim, Control::Down, GAS, 0);
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
