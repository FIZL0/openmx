//! P88 — session markers on the d-pad, ridden the way the shell and the online session ride them: a
//! tick is the pad's poll with the d-pad out of the ride (`Controls::stick_alone`, P93), the step,
//! then `markers::frame` with every rider's spot from before the tick (`Seat::pass`,
//! `net::rides::Rides::step`). The stand-in bike on a flat floor, at the 30 Hz clamp. Left and Right
//! fire on the press, the tick they go down (P93); Down and Up on the tick their held timer crosses
//! `markers::HOLD_MS`, 200 ms (P106). P135 at the end: the spawn, the bar's first entry, and its clear.
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
// P125: the shell's pad layouts, and the jump test its picture cuts on.
#[allow(dead_code)]
#[path = "../src/window.rs"]
mod window;
#[allow(dead_code)]
#[path = "../src/pad.rs"]
mod pad;
#[allow(dead_code)]
#[path = "../src/pace.rs"]
mod pace;

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
    // Left from slot 1 is the spawn (P135), the bar's first entry.
    assert_eq!(tick(&mut sim, GAS | Control::Left.mask()), Some(Did::Stepped(Pick::Spawn)));
    let rest: Vec<Option<Did>> = (0..30).map(|_| tick(&mut sim, GAS | Control::Left.mask())).collect();
    assert_eq!(fired(&rest), vec![]);
    tick(&mut sim, GAS);
    assert_eq!(tick(&mut sim, GAS | Control::Right.mask()), Some(Did::Stepped(Pick::Slot(0))));
}

/// The slot display's bar fills with an Up or Down hold ([`markers::holding`]): empty on the press,
/// part-way through the hold, full from 200 ms on — and Left and Right, on the press, leave it empty.
#[test]
fn the_bar_fills_with_an_up_or_down_hold() {
    let holding = |sim: &Sim| markers::holding(sim.input.pad(0), sim.table());
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
        // With a trick button down it fills all the same (P129): the d-pad is no gesture's.
        let circle = Control::Circle.mask();
        tick(&mut sim, circle | c.mask());
        tick(&mut sim, circle | c.mask());
        assert!(holding(&sim) > 0, "{c:?}: a trick button held the bar");
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
        hud::draw_markers(&mut f, Pick::Slot(0), &[false; SLOTS], "marker 1 empty", &[], h);
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

/// P129: a trick button held with the d-pad makes it no trick's gesture: the hold is the markers'
/// all the same — Down places at [`HOLD_MS`], Left steps — and the bike rides exactly as on the
/// trick button and the gas alone, tick for tick, the d-pad read as let go.
#[test]
fn a_trick_button_does_not_take_the_dpad_from_the_markers() {
    let circle = Control::Circle.mask(); // trick 1, the shipped preset's
    let mut sim = moving();
    let mut plain = sim.clone();
    let did = hold(&mut sim, Control::Down, GAS | circle, HOLD_MS);
    assert_eq!(fired(&did), vec![(hold_tick(), Did::Placed(0))], "a trick button stopped the marker");
    for k in 0..did.len() {
        tick(&mut plain, GAS | circle);
        if k + 1 == did.len() {
            assert_eq!(hash::of(&plain.rider.ride), hash::of(&sim.rider.ride), "the d-pad rode with the trick button");
        }
    }
    let did = hold(&mut sim, Control::Left, GAS | circle, 0);
    assert_eq!(fired(&did), vec![(0, Did::Stepped(Pick::Spawn))]);
    // The hands the bike was stepped with have no d-pad in them, a trick button down or not.
    let c = sim.poll(DT, &pads(GAS | circle | Control::Up.mask())).stick_alone();
    assert!(c.trick[0], "the trick button is still read");
    assert_eq!((c.dpad_y, c.dpad_dir, c.dpad), (false, 0, [false; 4]));
}

/// P129: with the player's Markers switch off (the original's controls, Original's sub-switch) the
/// d-pad fires no marker and rides as the original's: the shell's pass is the original's hands
/// ([`Controls::through`], [`vehicle::hands`] with markers off) stepped, tick for tick the same ride as
/// `Sim::poll` and `Sim::step` alone — d-pad Left steering the bike off the gas-alone line, and
/// Down with a trick button held a second the gesture the bike reads. With markers on the same pads
/// move no bike and the hold places a marker.
#[test]
fn markers_off_the_dpad_rides_as_the_originals() {
    let circle = Control::Circle.mask();
    let start = moving();
    let pass_with = |sim: &mut Sim, garage: &mut Garage, buttons: u16, on: bool| {
        let mut world = Flat::default();
        let spots = [garage.spot(sim, &mut world)];
        let controls = vehicle::hands(sim.poll(DT, &pads(buttons)), on);
        garage.pass_marked(sim, DT, &controls, 0, &spots, &mut world, on).1
    };
    for buttons in [GAS | Control::Left.mask(), GAS | circle | Control::Down.mask()] {
        let (mut off, mut on, mut original, mut plain) = (start.clone(), start.clone(), start.clone(), start.clone());
        let (mut g_off, mut g_on) = (Garage::new(Kind::Bike, None).unwrap(), Garage::new(Kind::Bike, None).unwrap());
        let mut did_on = vec![];
        for k in 0..30 {
            assert_eq!(pass_with(&mut off, &mut g_off, buttons, false), None, "{buttons:#x}: markers off fired on tick {k}");
            did_on.push(pass_with(&mut on, &mut g_on, buttons, true));
            let c = original.poll(DT, &pads(buttons));
            original.step(DT, &c, &mut Flat::default());
            let c = plain.poll(DT, &pads(buttons & !(Control::Left.mask() | Control::Down.mask()))).stick_alone();
            plain.step(DT, &c, &mut Flat::default());
            assert_eq!(hash::of(&off.rider.ride), hash::of(&original.rider.ride), "{buttons:#x}: tick {k} is not the original's");
            assert_eq!(hash::of(&on.rider.ride), hash::of(&plain.rider.ride), "{buttons:#x}: markers on, the d-pad rode on tick {k}");
        }
        assert!(off.markers.fresh(), "{buttons:#x}: markers off touched the markers");
        assert_ne!(hash::of(&off.rider.ride), hash::of(&on.rider.ride), "{buttons:#x}: the d-pad did nothing with markers off");
        let want = if buttons & Control::Down.mask() != 0 { vec![Did::Placed(0)] } else { vec![Did::Stepped(Pick::Spawn)] };
        assert_eq!(what(&did_on), want, "{buttons:#x}: markers on");
    }
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

/// Left and Right step through the five slots, then each other player, then the spawn (P135) and
/// round; each player sees everybody but themselves.
#[test]
fn left_and_right_reach_every_slot_and_then_the_other_players() {
    let mut sim = three_riders();
    let mut seen = vec![sim.markers.pick(0, 3)];
    for _ in 0..SLOTS + 3 {
        let did = hold(&mut sim, Control::Right, 0, 0);
        let [(0, Did::Stepped(p))] = fired(&did)[..] else { panic!("{did:?}") };
        seen.push(p);
    }
    let slots = (0..SLOTS).map(Pick::Slot);
    let want: Vec<Pick> = slots.chain([Pick::Player(1), Pick::Player(2), Pick::Spawn, Pick::Slot(0)]).collect();
    assert_eq!(seen, want);
    // Left from the first slot is the spawn, and from the spawn the last player.
    let did = hold(&mut sim, Control::Left, 0, 0);
    assert_eq!(fired(&did), vec![(0, Did::Stepped(Pick::Spawn))]);
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
/// five slots, then seat 3, then the spawn (P135) and round, and Left from the spawn is seat 3 — seat
/// 2 is never on it — and every other player's list leaves it out too. A selection left on a seat that stops
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
    let want: Vec<Pick> = (0..SLOTS).map(Pick::Slot).chain([Pick::Player(2), Pick::Spawn, Pick::Slot(0)]).collect();
    assert_eq!(seen, want);
    for _ in 0..2 {
        tick_among(&mut sim, Control::Left.mask(), &riding);
        tick_among(&mut sim, 0, &riding);
    }
    assert_eq!(sim.markers.pick_among(0, &riding), Pick::Player(2), "Left from the first slot, past the spawn");
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
    // Nobody else in the ride: the slots and the spawn alone, round.
    let mut alone = three_riders();
    for _ in 0..SLOTS + 1 {
        tick_among(&mut alone, Control::Right.mask(), &[true, false, false]);
        tick_among(&mut alone, 0, &[true, false, false]);
    }
    assert_eq!(alone.markers.pick_among(0, &[true, false, false]), Pick::Slot(0));
}

/// P94, online: a seat nobody sits in has the missing pad (`net::Game::step`), and the session's
/// tick ([`net::rides::Rides`]) leaves it out of the markers' places — the same count on every
/// machine, as the pads are the session's. Eight presses of Right from the first slot are round to
/// the second slot with seat 2 empty, and to the first with everybody in: the spawn (P135,
/// [`markers::SPAWN`]) is the place before the first slot.
#[test]
fn online_an_empty_seat_is_never_a_pick() {
    use net::Game;
    let ride = |seat2: Report| {
        let first = Sim::new(1);
        let mut sims: Vec<Sim> = (0..3).map(|_| first.clone()).collect();
        let mut world = Flat::default();
        let mut rides = net::rides::Rides { sims: &mut sims, world: &mut world, after: None, time_scale: None, plane: None };
        let mut seen = Vec::new();
        for t in 0..2 * (SLOTS + 3) {
            let me = if t % 2 == 0 { Report::from_buttons(Control::Right.mask()) } else { Report::from_buttons(0) };
            rides.step(t, &[me, seat2, Report::from_buttons(0)]);
            if t % 2 == 0 {
                seen.push(rides.sims[0].markers.at);
            }
        }
        seen
    };
    // Eight places with everybody in (the spawn, five slots, two players), round to the first on the
    // eighth.
    let spawn = markers::SPAWN;
    assert_eq!(ride(Report::from_buttons(0)), vec![1, 2, 3, 4, 5, 6, spawn, 0], "everybody riding");
    assert_eq!(ride(Report::missing()), vec![1, 2, 3, 4, 5, spawn, 0, 1], "seat 2 empty: one place fewer, round");
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
// P125: the plane's markers in the shell.
// ---------------------------------------------------------------------------------------------------

use game_sim::camera::{Preset, Race, Surface, Switches, View};

/// The shell's plane camera constants (`main.rs`): its reverse threshold, speed reference, near clip
/// and starting preset.
const REVERSE_SPEED: f32 = 5.0;
const SPEED_REFERENCE: f32 = 117.332_8;
const NEAR_CLIP: f32 = 2.4;
const PLANE_CAMERA: &str = "Flight Cam 1";

/// The plane camera's presets off the disc (`BiplaneCPresets.ini`, in free06's archive), or `None`.
fn plane_presets() -> Option<Vec<Preset>> {
    let mut disc = game_render::disc::Disc::open_configured().ok()?;
    let names: Vec<String> = disc.entries().iter().map(|e| e.name.clone()).filter(|n| n.to_ascii_lowercase().contains("free06")).collect();
    let bytes = names.iter().find_map(|n| disc.res(n).ok()?.read_suffix("biplanecpresets.ini").ok())?;
    let presets = Preset::read_bytes(&bytes, "");
    (!presets.is_empty()).then_some(presets)
}

/// No ground under the camera: the floor never lifts the eye.
struct Below;

impl Surface for Below {
    fn on_surface(&mut self, point: Vf) -> Vf {
        Vf::new(point.x(), -1.0e6, point.z(), point.w())
    }
}

/// A player in the shell: their ride, their garage, and their plane's camera as the shell builds it.
struct Player {
    sim: Sim,
    garage: Garage,
    cam: Race,
    /// What the camera showed after the last tick.
    view: Option<View>,
}

impl Player {
    fn new(sim: Sim, garage: Garage, presets: Vec<Preset>) -> Player {
        let mut cam = Race::new(presets, F(REVERSE_SPEED));
        if let Some(start) = Preset::find(&cam.cam.presets, PLANE_CAMERA) {
            cam.cam.set_preset(start);
        }
        cam.cam.snap = true;
        cam.cam.speed_reference = F(SPEED_REFERENCE);
        Player { sim, garage, cam, view: None }
    }

    fn plane(&self) -> &Plane {
        self.garage.plane.as_ref().expect("a plane")
    }

    /// The camera after a tick, as the shell runs it: a plane that jumped takes it along
    /// (`chase::follow_jump`, P125), unless `carried` is off — the shell before P125.
    fn film(&mut self, before: Vf, carried: bool) {
        let plane = self.plane();
        let at = |v: Vf| game_render::mat::V3 { x: v.x(), y: v.y(), z: v.z() };
        let jump = pace::jump(at(before), at(plane.at), plane.speed().0 * DT.0);
        let speed = plane.speed();
        if let (Some(by), true) = (jump, carried) {
            chase::follow_jump(&mut self.cam.cam, [by.x, by.y, by.z], speed);
        }
        let rider = chase::ridden(&self.garage, &self.sim.rider.ride);
        let switches = Switches { no_look_ahead: false, short_viewport: false };
        self.view = Some(self.cam.update(DT, &rider, F(NEAR_CLIP), &self.sim.tables.ride.curve, switches, &mut Below));
    }

    /// The plane as the camera shows it: how far it is from the eye, feet, and how far off the
    /// view's middle, radians.
    fn in_view(&self) -> (f32, f32) {
        let view = self.view.as_ref().expect("a filmed tick");
        let to = self.plane().at.sub(view.position);
        let dir = view.look_at.sub(view.position);
        let len = |v: Vf| (v.x() * v.x() + v.y() * v.y() + v.z() * v.z()).sqrt();
        let cos = (to.x() * dir.x() + to.y() * dir.y() + to.z() * dir.z()) / (len(to) * len(dir));
        (len(to), cos.clamp(-1.0, 1.0).acos())
    }
}

/// Whether the camera shows the plane: in front of the eye, near the view's middle, and close.
fn shown((far, off): (f32, f32)) -> bool {
    far < 150.0 && off < 0.5
}

/// One shell tick for every player, as `World::simulate` runs them: every player's spot first, then
/// each one's poll and [`Garage::pass`] in seat order, then the cameras. What each player's d-pad did.
fn shell_tick(players: &mut [Player], reports: &[Report], carried: bool) -> Vec<Option<Did>> {
    let mut world = Flat::default();
    let spots: Vec<Spot> = players.iter().map(|p| p.garage.spot(&p.sim, &mut world)).collect();
    let mut out = Vec::new();
    for (me, (p, report)) in players.iter_mut().zip(reports).enumerate() {
        let before = p.garage.plane.as_ref().map_or(Vf::new(0.0, 0.0, 0.0, 1.0), |plane| plane.at);
        let controls = p.sim.poll(DT, &[*report, Report::missing()]).stick_alone();
        out.push(p.garage.pass(&mut p.sim, DT, &controls, me, &spots, &mut world).1);
        if p.garage.up == Kind::Biplane {
            p.film(before, carried);
        }
    }
    out
}

/// The pad's report as the shell hands it to a seat on the plane (main.rs's live pass): through the
/// seat's layout, then the plane's scheme the right way up. `buttons` are the pad's own.
fn shell_report(layout: pad::Scheme, buttons: u16, stick_y: u8) -> Report {
    let mut r = Report::from_buttons(buttons);
    r.set_axis(3, stick_y);
    pad::PlaneScheme::default().fly(layout.ride(r), false)
}

/// Where a layout has the plane's throttle on the pad: R2 under the new one (P40), Cross under the
/// original's.
fn pad_gas(layout: pad::Scheme) -> u16 {
    match layout {
        pad::Scheme::Original => Control::Cross.mask(),
        _ => Control::R2.mask(),
    }
}

const LAYOUTS: [pad::Scheme; 2] = [pad::Scheme::New, pad::Scheme::Original];

/// P125: on the plane, through the shell's own pass under both layouts, a pad's held Down puts the
/// flight in slot 1; flown away, a held Up puts the plane back into it — the flight it kept, bit for
/// bit, drawn where it was — and the plane's camera goes with it, so the plane is on the screen the
/// tick it is put back and every tick after. Before P125 the camera stayed where the plane had been,
/// looking away from it: the put-back happened, and nobody could see it.
#[test]
fn the_shells_plane_places_and_puts_back_in_view_under_both_layouts() {
    let Some(presets) = plane_presets() else {
        eprintln!("skipped: no disc");
        return;
    };
    for layout in LAYOUTS {
        let Some((sim, garage)) = hangar(Kind::Biplane) else { return };
        let mut players = [Player::new(sim, garage, presets.clone())];
        let gas = pad_gas(layout);
        let tick = |players: &mut [Player], buttons: u16, y: u8| shell_tick(players, &[shell_report(layout, buttons, y)], true)[0];
        for _ in 0..30 * 30 {
            tick(&mut players, gas, 0x80);
        }
        assert!(!players[0].plane().on_ground(), "{layout:?}: thirty seconds of gas and the plane is not flying");
        let placed = (0..30).find_map(|_| tick(&mut players, gas | Control::Down.mask(), 0x80));
        assert_eq!(placed, Some(Did::Placed(0)), "{layout:?}: a held Down on the plane");
        let kept = flight(&players[0].garage).clone();
        let mark_at = markers::Mark::Plane(kept.clone()).position();

        // Away: a pull, then level, until the plane is well clear of the marker.
        for k in 0..300 {
            tick(&mut players, gas, if k < 40 { 0xff } else { 0x80 });
        }
        let away = players[0].plane().at.sub(mark_at);
        let away = (away.x() * away.x() + away.y() * away.y() + away.z() * away.z()).sqrt();
        assert!(away > 300.0, "{layout:?}: only {away} ft from the marker");
        assert!(shown(players[0].in_view()), "{layout:?}: the camera lost the plane in plain flight: {:?}", players[0].in_view());

        // Up, held: the tick it fires, the plane is the flight it kept, where it kept it — and on
        // the screen. The shell before P125, on the same tick, filmed empty sky.
        let mut fired = None;
        for _ in 0..30 {
            let mut old = [Player { sim: players[0].sim.clone(), garage: players[0].garage.clone(), cam: Race { cam: players[0].cam.cam.clone() }, view: None }];
            fired = tick(&mut players, gas | Control::Up.mask(), 0x80);
            if fired.is_some() {
                shell_tick(&mut old, &[shell_report(layout, gas | Control::Up.mask(), 0x80)], false);
                assert!(!shown(old[0].in_view()), "{layout:?}: the uncarried camera saw the put-back after all: {:?}", old[0].in_view());
                break;
            }
        }
        assert_eq!(fired, Some(Did::Teleported(Pick::Slot(0))), "{layout:?}: a held Up on the plane");
        assert!(*flight(&players[0].garage) == kept, "{layout:?}: the put-back is not the flight it kept");
        assert_eq!(players[0].plane().at.to_bits()[..3], mark_at.to_bits()[..3], "{layout:?}: not drawn where it was put back");
        assert!(shown(players[0].in_view()), "{layout:?}: the put-back is off the screen: {:?}", players[0].in_view());
        for k in 0..60 {
            tick(&mut players, gas, 0x80);
            assert!(shown(players[0].in_view()), "{layout:?}: {k} ticks after the put-back the plane is off the screen: {:?}", players[0].in_view());
        }
        println!("P125 {layout:?}: placed, flown {away:.0} ft away, put back and filmed, {:?} after it", players[0].in_view());
    }
}

/// P125, a split: player 1 flies the plane and player 2 rides the bike. On the plane a Left press
/// steps the selection off the first slot onto the other player, and a held Up lands the plane by
/// them, standing on the ground where their bike is, with the camera snapped onto it there.
#[test]
fn in_a_split_left_picks_the_other_player_and_up_lands_by_them() {
    let Some(presets) = plane_presets() else {
        eprintln!("skipped: no disc");
        return;
    };
    for layout in LAYOUTS {
        let Some((sim, garage)) = hangar(Kind::Biplane) else { return };
        let bike = Sim::new(0x125);
        let mut players = [Player::new(sim, garage, presets.clone()), Player::new(bike, Garage::new(Kind::Bike, None).unwrap(), presets.clone())];
        let gas = pad_gas(layout);
        let idle = shell_report(layout, 0, 0x80);
        let tick = |players: &mut [Player], buttons: u16| shell_tick(players, &[shell_report(layout, buttons, 0x80), idle], true)[0];
        for _ in 0..30 * 30 {
            tick(&mut players, gas);
        }
        assert!(!players[0].plane().on_ground(), "{layout:?}: not flying");
        assert_eq!(tick(&mut players, gas | Control::Left.mask()), Some(Did::Stepped(Pick::Spawn)), "{layout:?}: Left from slot 1");
        tick(&mut players, gas);
        assert_eq!(tick(&mut players, gas | Control::Left.mask()), Some(Did::Stepped(Pick::Player(1))), "{layout:?}: Left from the spawn");
        tick(&mut players, gas);
        let fired = (0..30).find_map(|_| tick(&mut players, gas | Control::Up.mask()));
        assert_eq!(fired, Some(Did::Teleported(Pick::Player(1))), "{layout:?}: a held Up onto the other player");
        tick(&mut players, 0);
        let them = Spot::of(&players[1].sim.rider).at;
        let plane = players[0].plane();
        assert!(plane.on_ground(), "{layout:?}: the plane is not landed by them");
        let (dx, dz) = (plane.at.x() - them.x(), plane.at.z() - them.z());
        assert!(dx.hypot(dz) < 1.0, "{layout:?}: landed {dx}, {dz} ft from the other player");
        assert!(shown(players[0].in_view()), "{layout:?}: the landed plane is off the screen: {:?}", players[0].in_view());
    }
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

/// One shell pass for player 1 as `Seat::pass` runs it: the poll with the d-pad out of the ride, then
/// the shell's own [`Garage::pass`] — the garage's step on whichever machine is up, then the bike's
/// markers or the plane's (P125).
fn pass(sim: &mut Sim, garage: &mut Garage, report: Report) -> Option<Did> {
    let mut world = Flat::default();
    let spots = [garage.spot(sim, &mut world)];
    let reports = [report, Report::missing()];
    let controls = sim.poll(DT, &reports).stick_alone();
    garage.pass(sim, DT, &controls, 0, &spots, &mut world).1
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
    // Its five slots uncounted (P139 counts them): the codec's magic, then the slots.
    v1.drain(kind_at + 4..kind_at + 12);
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
    let _ = hands; // since P129 the trick buttons say nothing to the bar
    MarkerBar::read(&sim.markers, me, &vec![true; seats.len()], sim.input.pad(0), sim.table(), true)
}

/// The label the display shows with it, offline (no names).
fn label(b: &MarkerBar) -> String {
    hud::marker_label(b.pick, &b.filled, &[])
}

/// P124: flying the plane, the slot display is the plane's. A Left press puts the bar up on the
/// plane's last slot; a held Down fills the hold bar to `HOLD_MS` — with a trick button down too,
/// which on the plane is no gesture, as its markers read the pad — and the bar marks the slot
/// filled, with its radar icon; switching to the bike puts the bar up on the bike's own set, where
/// since P129 the same trick button fills it too and sets the bike's slot.
#[test]
fn flying_the_plane_shows_the_planes_bar() {
    let Some(hangar) = hangar(Kind::Biplane) else { return };
    let mut seats = vec![hangar];
    let mut hands = Controls::default();
    let mut banner = MarkerBanner::default();
    let mut now = Instant::now();
    let step = Duration::from_millis(33);
    let mut show = |b: &MarkerBar, now: Instant| banner.update(now, b.pick, &b.filled, b.held);
    for _ in 0..30 {
        hands = seat_pass(&mut seats, 0, Report::from_buttons(GAS)).1;
        now += step;
        assert!(!show(&bar(&seats, 0, &hands), now), "the bar is up untouched");
    }
    assert_eq!(seats[0].0.markers.on, markers::Vehicle::Plane);

    // Left: the plane's spawn (P135), then its last slot, the bar up.
    let (did, h) = seat_pass(&mut seats, 0, Report::from_buttons(GAS | Control::Left.mask()));
    assert_eq!(did, Some(Did::Stepped(Pick::Spawn)));
    now += step;
    let b = bar(&seats, 0, &h);
    assert!(show(&b, now), "a Left press onto the spawn shows no bar");
    assert_eq!(label(&b), "spawn");
    seat_pass(&mut seats, 0, Report::from_buttons(GAS));
    let (did, h) = seat_pass(&mut seats, 0, Report::from_buttons(GAS | Control::Left.mask()));
    assert_eq!(did, Some(Did::Stepped(Pick::Slot(SLOTS - 1))));
    now += step;
    let b = bar(&seats, 0, &h);
    assert!(show(&b, now), "a Left press on the plane shows no bar");
    assert_eq!((b.pick, b.filled.clone(), b.held, b.holding), (Pick::Slot(SLOTS - 1), vec![false; SLOTS], true, 0));
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
    assert_eq!((b.pick, b.filled.clone()), (Pick::Slot(0), vec![false; SLOTS]), "the bike shows the plane's set");
    assert!(show(&b, now), "the switch's new set put no bar up");
    assert_eq!(label(&b), "marker 1 empty");
    assert!(overlay::marker_blips(&seats[0].0.markers, b.pick).is_empty());
    let mut seen = vec![];
    let mut placed = None;
    for _ in 0..10 {
        let did;
        (did, hands) = seat_pass(&mut seats, 0, Report::from_buttons(GAS | circle | Control::Down.mask()));
        placed = placed.or(did);
        seen.push(bar(&seats, 0, &hands).holding);
    }
    assert_eq!(*seen.last().unwrap(), HOLD_MS, "a trick button held the bike's hold bar: {seen:?}");
    assert_eq!(placed, Some(Did::Placed(0)));
    assert!(bar(&seats, 0, &hands).filled[0]);
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
    assert_eq!(did, Some(Did::Stepped(Pick::Spawn)));
    assert_eq!(label(&b), "spawn");
    both(&mut seats, GAS);
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
        assert_eq!((one.pick, one.filled.clone(), one.holding), (Pick::Slot(0), vec![false; SLOTS], 0), "player 2's hold is on player 1's bar");
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

// ---------------------------------------------------------------------------------------------------
// P112: a marker keeps the real bike's momentum.
// ---------------------------------------------------------------------------------------------------

/// The disc's `race125p.phy` (the freeride's bike, P25) and where the `125_faisst` model hangs its
/// wheels, as `ride.rs`'s `the_disc_125` reads them; `None` (said) without a disc.
fn the_disc_125(tables: &game_sim::rider::Tables) -> Option<(game_sim::phy::Phy, (Vf, Vf))> {
    use game_render::disc::Disc;
    let mut disc = match Disc::open_configured() {
        Ok(disc) => disc,
        Err(e) => {
            eprintln!("skipped: {e}");
            return None;
        }
    };
    let archives: Vec<String> = disc
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.contains("bike\\") && n.ends_with(".res")
        })
        .collect();
    let (mut phy, mut wheels) = (None, None);
    for archive in &archives {
        let Ok(res) = disc.res(archive) else { continue };
        if phy.is_none() {
            phy = res.read_suffix("race125p.phy").ok().map(|b| game_sim::phy::Phy::read_bytes(&b, &tables.ride.curve));
        }
        if wheels.is_none() && archive.to_ascii_lowercase().contains("125_faisst") {
            let Ok(psg) = res.read_suffix("bike.psg") else { continue };
            let Ok(model) = game_render::psg::Model::parse(&psg) else { continue };
            let node = |name: &str| -> Option<Vf> {
                let i = model.nodes.iter().position(|n| n.name == name)?;
                let mut root = i;
                while let Some(up) = model.nodes[root].parent {
                    root = up;
                }
                Some(Vf(model.place_in(i, root)))
            };
            wheels = node("TireFront").zip(node("TireRear"));
        }
    }
    if phy.is_none() || wheels.is_none() {
        eprintln!("skipped: no race125p.phy or 125_faisst wheels on the disc");
    }
    Some((phy?, wheels?))
}

/// The real bike on the flat floor, wrecks on as the shell rides by default.
fn the_real_bike() -> Option<Sim> {
    let tables = std::sync::Arc::new(game_sim::rider::Tables::build());
    let (phy, wheels) = the_disc_125(&tables)?;
    let mut rider = Rider::new(&phy, wheels, Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    rider.sense.wrecks_enabled = true;
    Some(Sim::with_rider(0x112, tables, phy, rider))
}

/// A report with the left stick's x at `x` on top of `buttons`.
fn steer(buttons: u16, x: u8) -> Report {
    let mut r = Report::from_buttons(buttons);
    r.set_axis(2, x);
    r
}

/// One shell pass on the bike for a lone rider: `Seat::pass`'s poll, step and markers.
fn ride_pass(sim: &mut Sim, report: Report) -> Option<Did> {
    let spots = [Spot::of(&sim.rider)];
    let controls = sim.poll(DT, &[report, Report::missing()]).stick_alone();
    sim.step(DT, &controls, &mut Flat::default());
    markers::frame(sim, 0, &spots, &controls)
}

/// The hands both rides are stepped with: gas on, the stick swinging left and right a second at a
/// time, so the ride turns, leans and the wheels slip rather than running a straight line.
fn weaving(i: usize) -> Report {
    steer(GAS, if i % 60 > 30 { 0xc0 } else { 0x60 })
}

/// The real bike ridden up to speed and a marker placed in slot 1 with Down held: the ride as it was
/// on the Down's tick, and the ride ridden on two seconds past it — out of a wreck when `wrecked`.
fn placed_at_speed(wrecked: bool) -> Option<(Sim, Sim)> {
    let mut sim = the_real_bike()?;
    for i in 0..90 {
        ride_pass(&mut sim, weaving(i));
    }
    let placed = (0..30).find_map(|_| ride_pass(&mut sim, steer(GAS | Control::Down.mask(), 0xc0)).map(|d| (d, sim.clone())));
    let Some((Did::Placed(0), placed)) = placed else { panic!("Down held did not place slot 1: {placed:?}", placed = placed.map(|p| p.0)) };
    // At speed, not rolling: the weave has the bike well past 40 ft/s by now.
    let v = placed.bike_vel();
    let speed = (v.x() * v.x() + v.y() * v.y() + v.z() * v.z()).sqrt();
    assert!(speed > 40.0, "the marker was placed at {speed} ft/s");
    for i in 0..60 {
        ride_pass(&mut sim, weaving(i));
    }
    if wrecked {
        sim.rider.state.enter(game_sim::trick::Wreck::ATTITUDE);
        for i in 0..20 {
            ride_pass(&mut sim, weaving(i));
        }
        assert!(sim.rider.state.wreck.is_some(), "the forced wreck did not take");
    }
    Some((placed, sim))
}

/// Up held onto slot 1, then the motion the teleport came out with against the placement's, and
/// the two rides stepped on together for four seconds, bit for bit.
fn rides_out_as_placed(placed: Sim, mut sim: Sim) {
    let did = (0..30).find_map(|_| ride_pass(&mut sim, steer(GAS | Control::Up.mask(), 0xc0)));
    assert_eq!(did, Some(Did::Teleported(Pick::Slot(0))));
    let bits = |v: Vf| [v.x(), v.y(), v.z()].map(f32::to_bits);
    let (a, b) = (&placed.rider.ride, &sim.rider.ride);
    assert_eq!(bits(sim.bike_vel()), bits(placed.bike_vel()), "the body's velocity");
    assert_eq!(bits(b.chassis.vel), bits(a.chassis.vel), "the chassis' velocity");
    assert_eq!(bits(b.chassis.ang_vel), bits(a.chassis.ang_vel), "the angular velocity");
    assert_eq!(bits(b.chassis.world_ang_vel), bits(a.chassis.world_ang_vel), "the world angular velocity");
    assert_eq!(b.front.mv.spin.to_bits(), a.front.mv.spin.to_bits(), "the front wheel's spin");
    assert_eq!(b.rear.mv.spin.to_bits(), a.rear.mv.spin.to_bits(), "the rear wheel's spin");
    assert!(a.rear.mv.spin.abs() > 1.0, "the rear wheel was not turning");
    assert!(sim.rider.crash.is_none() && sim.rider.state.wreck.is_none(), "still down after the teleport");
    let (mut a, mut b) = (placed, sim);
    for i in 0..120 {
        ride_pass(&mut a, weaving(i));
        ride_pass(&mut b, weaving(i));
        assert_eq!(hash::of(&b.rider.ride), hash::of(&a.rider.ride), "the ride after the teleport parted from the ride after the placement {i} ticks on");
    }
}

/// P112: a marker placed at speed on the real bike (`race125p.phy`) and ridden back to comes out
/// with the body's velocity, angular velocity and wheel spin it was placed with, and rides on bit
/// for bit as the ride after the placement did.
#[test]
fn a_marker_rides_out_at_its_speed() {
    let Some((placed, sim)) = placed_at_speed(false) else { return };
    rides_out_as_placed(placed, sim);
}

/// P112: the same out of a wreck — the teleport's put-back does not stop the bike.
#[test]
fn a_marker_rides_out_of_a_wreck_at_its_speed() {
    let Some((placed, sim)) = placed_at_speed(true) else { return };
    rides_out_as_placed(placed, sim);
}

/// P112: the same after the markers file — the slots saved where the shell keeps them, and loaded
/// back the way a new ride loads them — ridden back to out of a wreck.
#[test]
fn a_saved_marker_rides_out_at_its_speed() {
    let Some((placed, mut sim)) = placed_at_speed(true) else { return };
    let dir = std::env::temp_dir().join(format!("openmx-p112-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    saved_markers::save(&dir, "free06", "1P", "race125p", &sim.markers).unwrap();
    let loaded = saved_markers::load(&dir, "free06", "1P", "race125p");
    let _ = std::fs::remove_dir_all(&dir);
    let (kept, back) = (sim.markers.slots[0].as_ref().and_then(|m| m.ride()), loaded.slots[0].as_ref().and_then(|m| m.ride()));
    assert_eq!(back.map(hash::of), kept.map(hash::of), "the file did not bring slot 1's ride back whole");
    sim.markers = loaded;
    rides_out_as_placed(placed, sim);
}

/// Flat ground at `y = 0`, for the camera.
struct Ground;

impl game_sim::camera::Surface for Ground {
    fn on_surface(&mut self, point: Vf) -> Vf {
        Vf::new(point.x(), 0.0, point.z(), point.w())
    }
}

/// P112: the follow camera rides out of a marker's Up with the bike. A camera that has followed a
/// bike along `z` for seconds, and one that followed it somewhere else and is snapped onto it as a
/// teleport lands and ridden out (`chase::ride_out`), show the same eye and look-at from there on;
/// snapped alone, a camera starts feet away from where the riding one is and lags over to it.
#[test]
fn the_camera_rides_out_with_the_bike() {
    use game_sim::camera::{CurveTable, Preset, Race, Ridden, Switches, Target};
    let lags = Vf::new(0.2, 0.2, 0.2, 0.0);
    let follow = || {
        let preset = Preset {
            name: "Race Cam 1".into(),
            look_filters: lags,
            pos_filters: lags,
            offset_from_body_point: Vf::new(0.0, 3.0, 0.0, 1.0),
            angle_x: F(0.2),
            following_distance: F(15.0),
            fov: F(80.0),
            filter_this_preset: true,
            cycle_thru_this_preset: true,
            min_following_distance: F(5.0),
            max_following_distance: F(50.0),
            min_fov: F(40.0),
            max_fov: F(100.0),
            ..Preset::default()
        };
        let mut race = Race::new(vec![preset], F(1.0));
        race.cam.set_preset(0);
        race.cam.snap = true;
        race
    };
    let speed = 60.0;
    let at = |z: f32| Ridden {
        target: Target {
            body_point: Vf::new(10.0, 3.0, z, 1.0),
            forward: Vf::new(0.0, 0.0, 1.0, 0.0),
            up: Vf::new(0.0, 1.0, 0.0, 0.0),
            vel: Vf::new(0.0, 0.0, speed, 0.0),
            lean: F::ZERO,
        },
        airborne: false,
        speed: F(speed),
    };
    let curve = CurveTable::build();
    let frame = |cam: &mut Race, z: f32| cam.update(DT, &at(z), F(2.4), &curve, Switches::default(), &mut Ground);
    let step = speed * DT.0;
    let (mut riding, mut ridden_out, mut snapped) = (follow(), follow(), follow());
    let mut z = 0.0;
    for _ in 0..300 {
        frame(&mut riding, z);
        frame(&mut ridden_out, z - 2000.0);
        frame(&mut snapped, z - 2000.0);
        z += step;
    }
    // The teleport's frame: the bike lands where the first camera's bike is.
    let steady = frame(&mut riding, z);
    ridden_out.cam.snap = true;
    snapped.cam.snap = true;
    let mut out = frame(&mut ridden_out, z);
    chase::ride_out(&mut ridden_out, &mut out, at(z).target.vel, DT);
    let jump = frame(&mut snapped, z);
    let near = |a: Vf, b: Vf| (a.x() - b.x()).abs().max((a.y() - b.y()).abs()).max((a.z() - b.z()).abs());
    assert!(near(out.position, steady.position) < 0.01 && near(out.look_at, steady.look_at) < 0.01, "ridden out {out:?}, riding {steady:?}");
    assert!(near(jump.position, steady.position) > 1.0, "a plain snap already sat where the riding camera does: {jump:?}");
    let mut was = (out.position, jump.position);
    for _ in 0..60 {
        z += step;
        let steady = frame(&mut riding, z);
        let out = frame(&mut ridden_out, z);
        let jump = frame(&mut snapped, z);
        assert!(near(out.position, steady.position) < 0.01 && near(out.look_at, steady.look_at) < 0.01, "ridden out {out:?}, riding {steady:?}");
        // The ridden-out eye moves with the bike from the first frame; the snapped one creeps.
        assert!((out.position.z() - was.0.z() - step).abs() < 0.01);
        was = (out.position, jump.position);
    }
}

// ---------------------------------------------------------------------------------------------------
// P135: the spawn is its own entry before the slots (P132's start), and Down held on it clears.
// ---------------------------------------------------------------------------------------------------

/// The stand-in ride with its map's start as the shell derives it (`vehicle::home`): the stand-in
/// bike's spawn and, given, the plane's bay.
fn homed(mut sim: Sim, bays: &[Spawn]) -> Sim {
    sim.home = std::sync::Arc::new(vehicle::home(&[stock::SPAWN], stock::FACING, bays));
    sim
}

/// The bike where it stands and how it is turned, bit for bit, and its speed and spin.
fn stood(sim: &Sim) -> ([u32; 4], [u32; 4], [u32; 4], [u32; 4]) {
    let c = &sim.rider.ride.chassis;
    (sim.rider.ride.position().to_bits(), c.quat.to_bits(), c.vel.to_bits(), c.ang_vel.to_bits())
}

/// Left from slot 1, a press and a let-go: the selection onto the spawn.
fn onto_spawn(sim: &mut Sim) {
    assert_eq!(fired(&hold(sim, Control::Left, 0, 0)), vec![(0, Did::Stepped(Pick::Spawn))]);
}

/// `control` held, on top of `with`, until the input layer's timer reads exactly `ms` — the 30 Hz
/// ticks, then 1 ms ones to land on it — without letting go: what each tick's markers did.
fn hold_to(sim: &mut Sim, control: Control, with: u16, ms: u32) -> Vec<Option<Did>> {
    let timer = |sim: &Sim| sim.input.pad(0).control(control).held_ms;
    let buttons = with | control.mask();
    let mut did = vec![tick(sim, buttons)];
    while timer(sim) + 34 < ms {
        did.push(tick(sim, buttons));
    }
    while timer(sim) < ms {
        did.push(tick_dt(sim, buttons, F(0.001)));
    }
    assert_eq!(timer(sim), ms);
    did
}

/// P135 (P132's ride home, off slot 1 onto its own entry): Left from slot 1 is the spawn, and Up
/// held on it rides the bike back to the start at rest — where the ride began and turned as it was,
/// bit for bit, with no speed and no spin, though it was ridden away at speed — and places nothing.
/// Down's slot hold there does nothing. Slot 1 is the player's own again: Up on it empty does
/// nothing, Down puts a marker there and Up rides back to it, the spawn still the start. A ride with
/// no start (no `Home`) refuses.
#[test]
fn up_on_the_spawn_rides_the_bike_back_at_rest() {
    let start = homed(Sim::new(0x88), &[]);
    let spawn = stood(&start);
    let zero = Vf::new(0.0, 0.0, 0.0, 0.0).to_bits();
    assert_eq!((spawn.2, spawn.3), (zero, zero), "the stand-in does not start at rest");
    assert_eq!(start.markers.pick(0, 1), Pick::Slot(0), "a ride starts on slot 1, not on the spawn");
    let mut back = start.clone();
    back.rider.teleport_to(Spot::still(stock::SPAWN, stock::FACING, Vf::new(0.0, 1.0, 0.0, 0.0)));
    assert_eq!(stood(&back), spawn, "a teleport to the spawn is not the spawn");

    let mut sim = start.clone();
    for _ in 0..45 {
        tick(&mut sim, GAS);
    }
    assert!(F(sim.bike_vel().z()).abs() > F(1.0), "the bike is not moving");
    // Slot 1, empty, is no spawn now.
    assert_eq!(what(&hold(&mut sim, Control::Up, GAS, HOLD_MS)), vec![Did::Refused(Pick::Slot(0))]);
    onto_spawn(&mut sim);
    assert_ne!(stood(&sim).0, spawn.0);
    let did = (0..30).find_map(|_| tick(&mut sim, Control::Up.mask()));
    assert_eq!(did, Some(Did::Teleported(Pick::Spawn)));
    assert_eq!(stood(&sim), spawn, "Up on the spawn is not the spawn at rest");
    assert!(sim.rider.ghost.on, "the spawn's teleport does not ghost the bike as a teleport does");
    assert!(!sim.markers.any(), "riding to the spawn placed something");
    tick(&mut sim, 0);

    // Down's 200 ms on the spawn places nothing.
    for _ in 0..30 {
        tick(&mut sim, GAS);
    }
    assert_eq!(fired(&hold_to(&mut sim, Control::Down, GAS, HOLD_MS + 100)), vec![]);
    tick(&mut sim, GAS);
    assert!(!sim.markers.any(), "Down on the spawn placed a marker");

    // The five slots still place and ride back.
    for s in 0..SLOTS {
        assert_eq!(fired(&hold(&mut sim, Control::Right, GAS, 0)), vec![(0, Did::Stepped(Pick::Slot(s)))]);
        for _ in 0..10 {
            tick(&mut sim, GAS);
        }
        assert_eq!(what(&hold(&mut sim, Control::Down, GAS, HOLD_MS)), vec![Did::Placed(s)]);
    }
    assert_eq!(sim.markers.placed(), SLOTS);
    for s in (0..SLOTS).rev() {
        let kept = sim.markers.slots[s].as_deref().and_then(markers::Mark::ride).cloned().unwrap();
        for _ in 0..10 {
            tick(&mut sim, GAS);
        }
        let did = (0..30).find_map(|_| tick(&mut sim, Control::Up.mask()));
        assert_eq!(did, Some(Did::Teleported(Pick::Slot(s))));
        assert_eq!(sim.rider.ride.position().to_bits(), kept.position().to_bits(), "Up did not go to slot {}", s + 1);
        tick(&mut sim, 0);
        hold(&mut sim, Control::Left, 0, 0);
    }
    // Back on the spawn with every slot full: it is still the start.
    assert_eq!(sim.markers.pick(0, 1), Pick::Spawn);
    let did = (0..30).find_map(|_| tick(&mut sim, Control::Up.mask()));
    assert_eq!(did, Some(Did::Teleported(Pick::Spawn)));
    assert_eq!(stood(&sim).0, spawn.0, "a full slot 1 took the spawn's place");

    // No start: the spawn refuses.
    let mut bare = Sim::new(0x88);
    onto_spawn(&mut bare);
    assert_eq!(what(&hold(&mut bare, Control::Up, 0, HOLD_MS)), vec![Did::Refused(Pick::Spawn)]);
    println!("spawn marker: Up on the spawn stood the bike at {:?} at rest", sim.rider.ride.position());
}

/// P135: Down held 499 ms on the spawn clears nothing; the tick its timer reads 500 it empties every
/// slot on every machine — the bike's up and the plane's away — once, however long it is held after,
/// as the pause's Clear Markers does (`Markers::clear`), the selection left on the spawn. A Down
/// let go before 500 ms does nothing, and a shorter tap nothing either.
#[test]
fn down_held_499_ms_on_the_spawn_clears_nothing_and_500_ms_every_slot() {
    assert_eq!(markers::CLEAR_MS, 500);
    let mut sim = moving();
    for s in [0, 2] {
        while sim.markers.pick(0, 1) != Pick::Slot(s) {
            hold(&mut sim, Control::Right, GAS, 0);
        }
        assert_eq!(what(&hold(&mut sim, Control::Down, GAS, HOLD_MS)), vec![Did::Placed(s)]);
    }
    // The plane's set, away (a default flight: the clear only empties it).
    let plane_slots = (0..SLOTS).map(|s| (s % 2 == 1).then(|| Box::new(markers::Mark::Plane(Default::default())))).collect();
    sim.markers.put(markers::Set { on: markers::Vehicle::Plane, slots: plane_slots, at: 3 });
    assert_eq!(sim.markers.placed(), 4);
    while sim.markers.pick(0, 1) != Pick::Spawn {
        hold(&mut sim, Control::Left, GAS, 0);
    }
    let before = hash::of(&sim.markers);

    let did = hold_to(&mut sim, Control::Down, GAS, 499);
    assert_eq!(fired(&did), vec![], "499 ms cleared");
    assert_eq!(tick(&mut sim, GAS), None, "the release fired");
    assert_eq!(hash::of(&sim.markers), before, "a Down let go at 499 ms touched the markers");
    assert_eq!(fired(&hold(&mut sim, Control::Down, GAS, 0)), vec![], "a tap fired");
    assert_eq!(hash::of(&sim.markers), before);

    let mut did = hold_to(&mut sim, Control::Down, GAS, 499);
    did.push(tick_dt(&mut sim, GAS | Control::Down.mask(), F(0.001)));
    assert_eq!(sim.input.pad(0).control(Control::Down).held_ms, 500);
    assert_eq!(fired(&did), vec![(did.len() - 1, Did::Cleared)], "500 ms, the tick it read 500");
    assert!(!sim.markers.any(), "a slot survived the clear");
    assert!(!sim.markers.any_on(markers::Vehicle::Plane), "the plane's slots survived the clear");
    assert_eq!(sim.markers.pick(0, 1), Pick::Spawn, "the clear moved the selection");
    let rest: Vec<Option<Did>> = (0..60).map(|_| tick(&mut sim, GAS | Control::Down.mask())).collect();
    assert_eq!(fired(&rest), vec![], "a clear fired twice");
    tick(&mut sim, GAS);
    // And on a slot the long hold is a placement, no clear.
    hold(&mut sim, Control::Right, GAS, 0);
    assert_eq!(what(&hold(&mut sim, Control::Down, GAS, 1000)), vec![Did::Placed(0)]);
    assert_eq!(sim.markers.placed(), 1);
}

/// P135 on the plane: flying, the spawn is its bay (P132's) — Up there stands the plane back in its
/// bay at rest, unwrecked, as an online seat starts in it — and Down held 499 ms there clears
/// nothing, 500 ms every slot, the plane's and the bike's away. Online the lane's plane does the
/// same.
#[test]
fn the_planes_spawn_is_its_bay_and_down_clears_there() {
    let Some((sim, mut garage)) = hangar(Kind::Biplane) else {
        eprintln!("skipped: no disc");
        return;
    };
    let bay = garage.plane.as_ref().unwrap().bay;
    let mut sim = homed(sim, &[bay]);
    let mut want = flight(&garage).clone();
    want.reset(bay.at, bay.facing, &sim.tables.ride.acos);

    airborne(&mut sim, &mut garage);
    // Slot 1 empty is no bay now.
    assert_eq!(hold_pass(&mut sim, &mut garage, GAS | Control::Up.mask()), Some(Did::Refused(Pick::Slot(0))));
    pass(&mut sim, &mut garage, Report::from_buttons(GAS));
    assert_eq!(hold_pass(&mut sim, &mut garage, GAS | Control::Down.mask()), Some(Did::Placed(0)));
    pass(&mut sim, &mut garage, Report::from_buttons(GAS));
    let bike = Box::new(markers::Mark::Bike(sim.rider.ride.clone()));
    sim.markers.put(markers::Set { on: markers::Vehicle::Bike, slots: vec![Some(bike), None, None, None, None], at: 0 });
    assert_eq!(sim.markers.placed(), 2);
    assert_eq!(pass(&mut sim, &mut garage, Report::from_buttons(GAS | Control::Left.mask())), Some(Did::Stepped(Pick::Spawn)));
    pass(&mut sim, &mut garage, Report::from_buttons(GAS));

    assert_eq!(hold_pass(&mut sim, &mut garage, Control::Up.mask()), Some(Did::Teleported(Pick::Spawn)));
    let f = flight(&garage);
    assert_eq!(f.body.position.to_bits(), want.body.position.to_bits(), "the plane is not back in its bay");
    assert_eq!(f.body.quat.to_bits(), want.body.quat.to_bits());
    for v in [f.body.velocity, f.body.rates] {
        assert_eq!([v.x(), v.y(), v.z()], [0.0; 3], "the plane is not at rest");
    }
    assert_eq!(f.crash.reason, 0);
    assert_eq!(sim.markers.placed(), 2, "riding to the bay touched the markers");
    pass(&mut sim, &mut garage, Report::missing());

    // Down: 499 ms nothing, 500 ms every slot.
    let down = Control::Down.mask();
    let timer = |sim: &Sim| sim.input.pad(0).control(Control::Down).held_ms;
    let pass_dt = |sim: &mut Sim, garage: &mut Garage, dt: F| {
        let mut world = Flat::default();
        let spots = [garage.spot(sim, &mut world)];
        let controls = sim.poll(dt, &[Report::from_buttons(down), Report::missing()]).stick_alone();
        garage.pass(sim, dt, &controls, 0, &spots, &mut world).1
    };
    let mut did = vec![pass(&mut sim, &mut garage, Report::from_buttons(down))];
    while timer(&sim) + 34 < 499 {
        did.push(pass(&mut sim, &mut garage, Report::from_buttons(down)));
    }
    while timer(&sim) < 499 {
        did.push(pass_dt(&mut sim, &mut garage, F(0.001)));
    }
    assert_eq!(timer(&sim), 499);
    assert_eq!(fired(&did), vec![], "499 ms on the plane's spawn fired");
    assert_eq!(sim.markers.placed(), 2);
    assert_eq!(pass_dt(&mut sim, &mut garage, F(0.001)), Some(Did::Cleared));
    assert!(!sim.markers.any(), "the plane's spawn left a marker");

    // Online: the lane's plane goes to the seat's bay the same way.
    let Some((lane, garage)) = hangar(Kind::Biplane) else { return };
    let mut lane = homed(lane, &[bay]);
    let template = garage.plane.as_ref().unwrap().flight.clone().unwrap();
    vehicle::start_lane(&template, bay, &mut lane);
    let mut world = Flat::default();
    let mut tick = |sim: &mut Sim, report: Report| {
        let spots = [vehicle::lane_spot(sim, &mut world)];
        sim.poll(DT, &[report, Report::missing()]);
        vehicle::fly_lane(sim, DT, vehicle::Wanted::default(), &mut world);
        vehicle::lane_markers(sim, 0, &spots, &[true], &mut world)
    };
    for _ in 0..30 * 30 {
        tick(&mut lane, Report::from_buttons(GAS));
    }
    assert_eq!(tick(&mut lane, Report::from_buttons(GAS | Control::Left.mask())), Some(Did::Stepped(Pick::Spawn)));
    tick(&mut lane, Report::from_buttons(GAS));
    assert_eq!((0..30).find_map(|_| tick(&mut lane, Report::from_buttons(Control::Up.mask()))), Some(Did::Teleported(Pick::Spawn)));
    let f = lane.plane.as_ref().unwrap();
    assert_eq!(f.body.position.to_bits(), want.body.position.to_bits(), "the lane's plane is not back in its bay");
    assert_eq!([f.body.velocity.x(), f.body.velocity.y(), f.body.velocity.z()], [0.0; 3]);
    println!("spawn marker: the plane stood back in its bay at {:?}", bay.at);
}

/// P132: the start is the map's, never a peer's. A ride's state hashes and encodes the same with
/// and without one, so peers never send or compare it, and a resync keeps this machine's own (the
/// constant it derived from the map), as it keeps the bike's numbers.
#[test]
fn the_spawn_is_derived_never_sent() {
    let mut sim = homed(Sim::new(0x88), &[]);
    for _ in 0..20 {
        tick(&mut sim, GAS);
    }
    let mut bare = sim.clone();
    bare.home = Default::default();
    assert_eq!(sim.hash(), bare.hash(), "the spawn is in the state hash");
    let bytes = game_sim::snapshot::encode(&sim);
    assert_eq!(bytes, game_sim::snapshot::encode(&bare), "the spawn is in the state's bytes");
    let mut here = homed(Sim::new(0x88), &[]);
    let home = here.home.clone();
    game_sim::snapshot::decode(&mut here, &bytes).unwrap();
    assert_eq!(here.home, home, "a resync dropped this machine's spawn");
    // And riding to it is the same on every machine: two peers, each its own derived start.
    let (mut a, mut b) = (sim.clone(), homed(bare, &[]));
    for s in [&mut a, &mut b] {
        onto_spawn(s);
        assert_eq!((0..30).find_map(|_| tick(s, Control::Up.mask())), Some(Did::Teleported(Pick::Spawn)));
    }
    assert_eq!(a.hash(), b.hash());
    // A selection on the spawn goes with a resync.
    let mut c = Sim::new(0x88);
    game_sim::snapshot::decode(&mut c, &game_sim::snapshot::encode(&a)).unwrap();
    assert_eq!((c.markers.at, c.hash()), (markers::SPAWN, a.hash()));
}

/// P132: in a split each seat's spawn is its own start: seat 2 rides back beside the first, where
/// it was stood, not onto the first's spawn.
#[test]
fn each_seat_has_its_own_spawn() {
    let up = Vf::new(0.0, 1.0, 0.0, 0.0);
    let second = Vf::new(stock::SPAWN.x() + 12.0, stock::SPAWN.y(), stock::SPAWN.z(), 1.0);
    let home = std::sync::Arc::new(vehicle::home(&[stock::SPAWN, second], stock::FACING, &[]));
    let mut sims: Vec<Sim> = (0..2).map(|_| Sim::new(0x132)).collect();
    sims[1].rider.teleport_to(Spot::still(second, stock::FACING, up));
    let at_two = sims[1].rider.ride.position().to_bits();
    for sim in &mut sims {
        sim.home = home.clone();
    }
    for _ in 0..45 {
        split_seat(&mut sims, 1, GAS);
    }
    assert_ne!(sims[1].rider.ride.position().to_bits(), at_two);
    assert_eq!(split_seat(&mut sims, 1, Control::Left.mask()), Some(Did::Stepped(Pick::Spawn)));
    split_seat(&mut sims, 1, 0);
    assert_eq!((0..30).find_map(|_| split_seat(&mut sims, 1, Control::Up.mask())), Some(Did::Teleported(Pick::Spawn)));
    assert_eq!(sims[1].rider.ride.position().to_bits(), at_two, "seat 2 rode to somebody else's spawn");
    assert_eq!(sims[1].bike_vel().to_bits(), Vf::new(0.0, 0.0, 0.0, 0.0).to_bits());
    // An online ride's seats all start on the first's bike start, each in its own bay.
    let online = home.online(vec![(second, stock::FACING)]);
    assert_eq!(online.bike(5).map(|s| s.at.to_bits()), Some(stock::SPAWN.to_bits()));
    assert_eq!(online.plane(0).map(|p| p.0.to_bits()), Some(second.to_bits()));
    assert_eq!(online.plane(1), None);
}

/// One tick of seat `me` of a split, each seat its own `Sim`, as `Seat::pass` runs it.
fn split_seat(sims: &mut [Sim], me: usize, buttons: u16) -> Option<Did> {
    let spots: Vec<Spot> = sims.iter().map(|s| Spot::of(&s.rider)).collect();
    let riding = vec![true; sims.len()];
    let sim = &mut sims[me];
    let hands = sim.poll(DT, &pads(buttons)).stick_alone();
    sim.step(DT, &hands, &mut Flat::default());
    markers::frame_among(sim, me, &spots, &riding, &hands)
}

/// P135: a clear is the clearing player's alone. In a split, seat 2's Down held on its spawn empties
/// seat 2's slots and leaves seat 1's; online, through the session's tick (`net::rides::Rides`, every
/// seat's pad the session's), seat 2's clear empties seat 2's and every other seat keeps its own.
#[test]
fn another_seats_markers_survive_a_clear() {
    let mut sims: Vec<Sim> = (0..2).map(|_| Sim::new(0x135)).collect();
    for me in 0..2 {
        for _ in 0..20 {
            split_seat(&mut sims, me, GAS);
        }
        assert_eq!((0..30).find_map(|_| split_seat(&mut sims, me, GAS | Control::Down.mask())), Some(Did::Placed(0)));
        split_seat(&mut sims, me, GAS);
    }
    assert_eq!(split_seat(&mut sims, 1, Control::Left.mask()), Some(Did::Stepped(Pick::Spawn)));
    split_seat(&mut sims, 1, 0);
    assert_eq!((0..30).find_map(|_| split_seat(&mut sims, 1, Control::Down.mask())), Some(Did::Cleared));
    assert!(!sims[1].markers.any(), "seat 2's clear left its own");
    assert!(sims[0].markers.holds(0), "seat 2's clear took seat 1's");

    // Online: three seats on one session tick.
    use net::Game;
    let first = Sim::new(1);
    let mut sims: Vec<Sim> = (0..3).map(|_| first.clone()).collect();
    let mut world = Flat::default();
    let mut rides = net::rides::Rides { sims: &mut sims, world: &mut world, after: None, time_scale: None, plane: None };
    let idle = Report::from_buttons(0);
    let down = Report::from_buttons(Control::Down.mask());
    let mut t = 0;
    let mut run = |rides: &mut net::rides::Rides, pads: [Report; 3], n: usize| {
        for _ in 0..n {
            rides.step(t, &pads);
            t += 1;
        }
    };
    // Every seat puts a marker in slot 1; seat 2 steps onto its spawn and holds Down a second.
    run(&mut rides, [down; 3], 15);
    run(&mut rides, [idle; 3], 1);
    assert!(rides.sims.iter().all(|s| s.markers.holds(0)), "a seat placed nothing");
    run(&mut rides, [idle, Report::from_buttons(Control::Left.mask()), idle], 1);
    run(&mut rides, [idle; 3], 1);
    assert_eq!(rides.sims[1].markers.pick(1, 3), Pick::Spawn);
    run(&mut rides, [idle, down, idle], 45);
    assert!(!rides.sims[1].markers.any(), "seat 2's clear left its own online");
    assert!(rides.sims[0].markers.holds(0) && rides.sims[2].markers.holds(0), "seat 2's clear took another seat's online");
}

/// P135: the bar on the spawn. It reads "spawn" and fills its hold bar with Down over the clear's
/// 500 ms — half at 250, full only at 500 — while Up there and Down on a slot fill over 200; after
/// the clear it says Cleared until the selection moves off the spawn. Drawn, the spawn's box is its
/// own, ringed when selected and marked where it has a start.
#[test]
fn the_bar_shows_the_spawn_and_the_clear() {
    let mut sim = homed(moving(), &[]);
    let read = |sim: &Sim| MarkerBar::read(&sim.markers, 0, &[true], sim.input.pad(0), sim.table(), true).homed(sim.home.has(sim.markers.on, 0));
    let b = read(&sim);
    assert!(b.home);
    assert_eq!((b.pick, b.label(&[])), (Pick::Slot(0), "marker 1 empty".to_string()));
    hold_to(&mut sim, Control::Down, GAS, 100);
    assert_eq!(read(&sim).holding, 100, "a slot's Down fills over 200 ms");
    tick(&mut sim, GAS);
    onto_spawn(&mut sim);
    let b = read(&sim);
    assert_eq!((b.pick, b.label(&[])), (Pick::Spawn, "spawn".to_string()));
    hold_to(&mut sim, Control::Up, 0, 100);
    assert_eq!(read(&sim).holding, 100, "the spawn's Up fills over 200 ms");
    tick(&mut sim, 0);
    for _ in 0..10 {
        tick(&mut sim, GAS);
    }

    let mut banner = MarkerBanner::default();
    let mut now = Instant::now();
    let mut clears = 0;
    let mut frame = |sim: &mut Sim, buttons: u16, banner: &mut MarkerBanner| {
        let did = tick(sim, buttons);
        clears += u32::from(did == Some(Did::Cleared));
        now += Duration::from_millis(33);
        banner.cleared(now, clears);
        let b = read(sim);
        let shown = banner.update(now, b.pick, &b.filled, b.held);
        (did, shown, b.cleared(banner.says_cleared()))
    };
    let mut fills = vec![];
    let mut done = None;
    for _ in 0..30 {
        let (did, _, b) = frame(&mut sim, GAS | Control::Down.mask(), &mut banner);
        let held = sim.input.pad(0).control(Control::Down).held_ms;
        fills.push((held, b.holding));
        if did == Some(Did::Cleared) {
            done = Some(b);
            break;
        }
        assert_eq!(b.label(&[]), "spawn", "the bar said Cleared before the clear");
    }
    for &(held, fill) in &fills {
        assert_eq!(fill, held.min(500) * HOLD_MS / 500, "{held} ms held on the spawn");
        assert!(held >= 500 || fill < HOLD_MS, "the bar was full at {held} ms");
    }
    let b = done.expect("no clear in a second");
    assert_eq!((b.holding, b.label(&[])), (HOLD_MS, "cleared".to_string()));
    let (_, shown, b) = frame(&mut sim, GAS, &mut banner);
    assert!(shown && b.label(&[]) == "cleared", "the bar does not say Cleared after");
    let (_, _, b) = frame(&mut sim, GAS | Control::Right.mask(), &mut banner);
    assert_eq!(b.label(&[]), "marker 1 empty", "Cleared stayed past the spawn");

    // Drawn: the spawn selected is not slot 1 selected, and a spawn with a start not one without.
    let draw = |pick: Pick, home: bool| {
        let mut f = game_render::raster::Frame::new(320, 224, [0, 0, 0]);
        hud::draw_markers_homed(&mut f, pick, &[false; SLOTS], home, "marker", &[], 0, 1.0);
        f.color
    };
    assert_ne!(draw(Pick::Spawn, true), draw(Pick::Slot(0), true));
    assert_ne!(draw(Pick::Slot(0), true), draw(Pick::Slot(0), false));
}

/// The software HUD's bar with 63 slots set (P139): it scrolls so whichever slot or the + is
/// selected is drawn, and the + reads "marker 64 new".
#[test]
fn the_software_bar_scrolls_to_the_selection_among_sixty_three() {
    let filled = vec![true; game_sim::markers::MAX_SLOTS - 1];
    assert_eq!(hud::marker_label(Pick::More, &filled, &[]), "marker 64 new");
    let draw = |pick: Pick| {
        let mut f = game_render::raster::Frame::new(640, 448, [20, 60, 120]);
        hud::draw_markers(&mut f, pick, &filled, "", &[], 0);
        f.color
    };
    let frames: Vec<Vec<u8>> = [Pick::Slot(0), Pick::Slot(31), Pick::Slot(62), Pick::More].into_iter().map(draw).collect();
    for (i, f) in frames.iter().enumerate() {
        assert!(frames[..i].iter().all(|o| o != f), "selection {i} draws as another");
    }
}
