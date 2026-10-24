//! P139 — more markers: once a player's five slots are all set the selection reaches a + past the
//! last, and Down held on it adds a slot holding the spot, up to 64. Ridden as the shell rides the
//! markers (`teleport_momentum.rs`'s tick, one player): the stand-in bike on a flat floor at the
//! 30 Hz clamp, the d-pad out of the ride.
//!
//! A slot added is kept until Clear Markers, which goes back to five; a set of forty survives the
//! snapshot and the saved-markers file, and the five-slot files of the formats before it still load.

use game_sim::hash;
use game_sim::input::{Control, PADS, Report};
use game_sim::markers::{self, Did, MAX_SLOTS, Mark, Markers, Pick, SLOTS, Spot, Vehicle};
use game_sim::ride::Controls;
use game_sim::snapshot;
use game_sim::world::Flat;
use game_sim::Sim;
use ps2_float::F;

const DT: F = F(game_sim::tick::DeltaTime::CAP_30);

const GAS: u16 = 0x0040; // Cross, the shipped preset's gas

const SEED: u32 = 0x139;

const BIKE: &str = "stock";

fn pads(buttons: u16) -> [Report; PADS] {
    let mut r = [Report::missing(); PADS];
    r[0] = Report::from_buttons(buttons);
    r
}

/// One tick on `buttons`, then the player's markers.
fn tick(sim: &mut Sim, buttons: u16) -> Option<Did> {
    let spots: Vec<Spot> = (0..sim.players()).map(|i| Spot::of(sim.rider_of(i))).collect();
    let hands: Vec<Controls> = sim.poll_all(DT, &pads(buttons)).into_iter().map(Controls::stick_alone).collect();
    sim.step_all(DT, &hands, &mut Flat::default());
    markers::frame(sim, 0, &spots, &hands[0])
}

/// A press of Left or Right, let go after: where the selection went.
fn step(sim: &mut Sim, dir: Control) -> Pick {
    let did = tick(sim, dir.mask());
    tick(sim, 0);
    match did {
        Some(Did::Stepped(p)) => p,
        other => panic!("{dir:?} did {other:?}"),
    }
}

/// Down or Up held until it fires, on the gas, let go after: what it did.
fn hold(sim: &mut Sim, dir: Control) -> Did {
    for _ in 0..30 {
        if let Some(did) = tick(sim, GAS | dir.mask()) {
            tick(sim, GAS);
            return did;
        }
    }
    panic!("{dir:?} never fired");
}

/// A few ticks on the gas, so the next marker is somewhere else.
fn ride(sim: &mut Sim, ticks: usize) {
    for _ in 0..ticks {
        tick(sim, GAS);
    }
}

/// A sim with its five slots set, the selection on the last.
fn five_set() -> Sim {
    let mut sim = Sim::new(SEED);
    for s in 0..SLOTS {
        if s > 0 {
            assert_eq!(step(&mut sim, Control::Right), Pick::Slot(s));
        }
        ride(&mut sim, 3);
        assert_eq!(hold(&mut sim, Control::Down), Did::Placed(s));
    }
    sim
}

#[test]
fn there_is_no_plus_until_all_five_are_set() {
    let mut sim = Sim::new(SEED);
    // Fresh: Right goes round the five and back to the first, Left from the first to the fifth.
    let seen: Vec<Pick> = (0..SLOTS).map(|_| step(&mut sim, Control::Right)).collect();
    assert_eq!(seen, (1..SLOTS).map(Pick::Slot).chain([Pick::Slot(0)]).collect::<Vec<_>>());
    assert_eq!(step(&mut sim, Control::Left), Pick::Slot(SLOTS - 1));
    assert_eq!(step(&mut sim, Control::Right), Pick::Slot(0));
    // Four set: still no +.
    for s in 0..SLOTS - 1 {
        if s > 0 {
            step(&mut sim, Control::Right);
        }
        assert_eq!(hold(&mut sim, Control::Down), Did::Placed(s));
    }
    assert!(!sim.markers.more());
    assert_eq!(step(&mut sim, Control::Right), Pick::Slot(SLOTS - 1));
    assert_eq!(step(&mut sim, Control::Right), Pick::Slot(0), "no + with slot 5 empty");
    // The fifth set: the + is past it, and Left from the first wraps onto it.
    assert_eq!(step(&mut sim, Control::Left), Pick::Slot(SLOTS - 1));
    assert_eq!(hold(&mut sim, Control::Down), Did::Placed(SLOTS - 1));
    assert!(sim.markers.more());
    assert_eq!(step(&mut sim, Control::Right), Pick::More);
    assert_eq!(step(&mut sim, Control::Right), Pick::Slot(0));
    assert_eq!(step(&mut sim, Control::Left), Pick::More);
    assert_eq!(sim.markers.slots.len(), SLOTS, "stepping onto the + adds nothing");
}

#[test]
fn down_on_the_plus_adds_slot_six_holding_the_spot_and_up_rides_back_to_it() {
    let mut sim = five_set();
    assert_eq!(step(&mut sim, Control::Right), Pick::More);
    // Up on the + does nothing.
    let before = hash::of(&sim.rider.ride);
    let kept = hash::of(&sim.markers);
    assert_eq!(hold(&mut sim, Control::Up), Did::Refused(Pick::More));
    assert_eq!(hash::of(&sim.markers), kept, "Up on the + changes no marker");
    assert_ne!(hash::of(&sim.rider.ride), before, "and the bike rode on, not put anywhere");
    assert_eq!(sim.markers.slots.len(), SLOTS);

    // Down on it: slot 6, holding the bike as the tick it fired left it, and the selection on it.
    ride(&mut sim, 4);
    let mut placed = None;
    for _ in 0..30 {
        if let Some(did) = tick(&mut sim, GAS | Control::Down.mask()) {
            placed = Some((did, hash::of(&sim.rider.ride), sim.rider.ride.position()));
            break;
        }
    }
    let (did, at_placement, spot) = placed.expect("Down fired");
    tick(&mut sim, GAS);
    assert_eq!(did, Did::Placed(SLOTS));
    assert_eq!(sim.markers.slots.len(), SLOTS + 1);
    assert_eq!(sim.markers.pick(0, 1), Pick::Slot(SLOTS), "the selection moved onto slot 6");
    assert_eq!(hash::of(&**sim.markers.slots[SLOTS].as_ref().unwrap()), at_placement);
    // The + is one on again.
    assert!(sim.markers.more());
    assert_eq!(step(&mut sim, Control::Right), Pick::More);
    assert_eq!(step(&mut sim, Control::Left), Pick::Slot(SLOTS));

    // Away and back: Up on slot 6 puts the bike where it was.
    ride(&mut sim, 20);
    assert_ne!(sim.rider.ride.position(), spot);
    let mut back = None;
    for _ in 0..30 {
        if let Some(did) = tick(&mut sim, Control::Up.mask()) {
            back = Some((did, sim.rider.ride.position()));
            break;
        }
    }
    let (did, at) = back.expect("Up fired");
    assert_eq!(did, Did::Teleported(Pick::Slot(SLOTS)));
    assert_eq!(at, spot, "rode back to slot 6's spot");
}

#[test]
fn sixty_four_set_shows_no_plus() {
    let mut sim = five_set();
    step(&mut sim, Control::Right);
    for n in SLOTS..MAX_SLOTS {
        assert_eq!(sim.markers.pick(0, 1), if n == SLOTS { Pick::More } else { Pick::Slot(n - 1) });
        if n > SLOTS {
            assert_eq!(step(&mut sim, Control::Right), Pick::More, "the + at {n}");
        }
        assert_eq!(hold(&mut sim, Control::Down), Did::Placed(n));
    }
    assert_eq!(sim.markers.slots.len(), MAX_SLOTS);
    assert!(sim.markers.slots.iter().all(Option::is_some) && !sim.markers.more());
    assert_eq!(sim.markers.pick(0, 1), Pick::Slot(MAX_SLOTS - 1));
    assert_eq!(step(&mut sim, Control::Right), Pick::Slot(0), "no + past 64");
    assert_eq!(step(&mut sim, Control::Left), Pick::Slot(MAX_SLOTS - 1));
    // A Down on the last slot places over it, adding nothing.
    assert_eq!(hold(&mut sim, Control::Down), Did::Placed(MAX_SLOTS - 1));
    assert_eq!(sim.markers.slots.len(), MAX_SLOTS);
}

#[test]
fn clear_markers_goes_back_to_five() {
    let mut sim = five_set();
    step(&mut sim, Control::Right);
    hold(&mut sim, Control::Down);
    step(&mut sim, Control::Right);
    hold(&mut sim, Control::Down);
    assert_eq!((sim.markers.slots.len(), sim.markers.pick(0, 1)), (SLOTS + 2, Pick::Slot(SLOTS + 1)));
    sim.markers.clear();
    assert_eq!(sim.markers.slots.len(), SLOTS);
    assert!(sim.markers.slots.iter().all(Option::is_none) && !sim.markers.more());
    assert_eq!(sim.markers.pick(0, 1), Pick::Slot(0), "a slot past the five goes to the first");
    assert!(sim.markers.fresh());

    // A selection on a player stays on that player; a plane's grown set goes back to five too.
    let full = || vec![Some(Box::new(Mark::Bike(sim.rider.ride.clone()))); SLOTS + 2];
    let riding = [true, true, true];
    let mut m = Markers { slots: full(), at: (SLOTS + 2 + 1 + 1) as u8, ..Markers::default() };
    assert_eq!(m.pick_among(0, &riding), Pick::Player(2));
    m.up(Vehicle::Plane);
    m.slots = full();
    m.at = (SLOTS + 2) as u8;
    assert_eq!(m.pick_among(0, &riding), Pick::More);
    m.clear();
    assert_eq!((m.slots.len(), m.pick_among(0, &riding)), (SLOTS, Pick::Slot(0)));
    m.up(Vehicle::Bike);
    assert_eq!((m.slots.len(), m.pick_among(0, &riding)), (SLOTS, Pick::Player(2)));
    assert!(m.slots_of(Vehicle::Plane).is_none(), "the cleared plane's set is gone");
}

/// A sim with forty slots set, each a different spot.
fn forty() -> Sim {
    let mut sim = five_set();
    step(&mut sim, Control::Right);
    while sim.markers.slots.len() < 40 {
        ride(&mut sim, 1);
        assert_eq!(hold(&mut sim, Control::Down), Did::Placed(sim.markers.slots.len() - 1));
        step(&mut sim, Control::Right);
    }
    assert_eq!(sim.markers.pick(0, 1), Pick::More);
    sim
}

#[test]
fn forty_slots_survive_a_snapshot_round_trip_and_a_saved_file() {
    let sim = forty();
    let slots: Vec<u64> = sim.markers.slots.iter().map(|s| hash::of(&**s.as_ref().unwrap())).collect();
    assert_eq!(slots.len(), 40);
    assert!((1..slots.len()).all(|i| !slots[..i].contains(&slots[i])), "forty different markers");

    // The snapshot, and the resync's bytes.
    let saved = snapshot::save(&sim);
    let mut there = Sim::new(SEED);
    snapshot::restore(&mut there, &saved);
    assert_eq!(hash::of(&there), hash::of(&sim));
    let mut there = Sim::new(SEED);
    snapshot::decode(&mut there, &snapshot::encode(&sim)).unwrap();
    assert_eq!(there.markers.slots.len(), 40);
    assert_eq!(hash::of(&there), hash::of(&sim), "the resync carries all forty");
    assert_eq!(there.markers.pick(0, 1), Pick::More);
    // The hash counts them: a set one shorter is another state.
    let mut short = sim.clone();
    short.markers.slots.pop();
    assert_ne!(hash::of(&short), hash::of(&sim));

    // The saved file.
    let file = sim.markers.to_file(BIKE);
    let back = Markers::from_file(&file, BIKE).unwrap();
    assert_eq!(back.slots.len(), 40);
    let read: Vec<u64> = back.slots.iter().map(|s| hash::of(&**s.as_ref().unwrap())).collect();
    assert_eq!(read, slots);
    assert_eq!(back.at, 0, "a file starts on the first slot");
}

#[test]
fn a_set_too_long_or_too_short_is_refused() {
    let sim = forty();
    let mut long = sim.clone();
    let one = long.markers.slots[0].clone();
    long.markers.slots.resize(MAX_SLOTS + 1, one);
    let mut there = Sim::new(SEED);
    assert!(snapshot::decode(&mut there, &snapshot::encode(&long)).is_err());
    assert!(Markers::from_file(&long.markers.to_file(BIKE), BIKE).is_err());
    let mut short = sim.clone();
    short.markers.slots.truncate(SLOTS - 1);
    assert!(snapshot::decode(&mut there, &snapshot::encode(&short)).is_err());
    assert!(Markers::from_file(&short.markers.to_file(BIKE), BIKE).is_err());
    assert_eq!(hash::of(&there), hash::of(&Sim::new(SEED)), "nothing written on a refusal");
}

/// The file P107's version 2 wrote of `markers`' five bike slots: the version, then the slots
/// uncounted. Version 1 (P99's) has no machine byte either.
fn old_file(markers: &Markers, version: u32) -> Vec<u8> {
    let new = markers.to_file(BIKE);
    let head = 8 + 4 + 4 + BIKE.len();
    let mut old = new[..head].to_vec();
    old[8..12].copy_from_slice(&version.to_le_bytes());
    if version >= 2 {
        old.push(new[head]);
    }
    // The slots: the codec's magic, then without the count.
    let slots = &new[head + 1..];
    assert_eq!(u64::from_le_bytes(slots[4..12].try_into().unwrap()), SLOTS as u64);
    old.extend_from_slice(&slots[..4]);
    old.extend_from_slice(&slots[12..]);
    old
}

#[test]
fn an_old_five_slot_file_loads() {
    let sim = five_set();
    let want: Vec<u64> = sim.markers.slots.iter().map(|s| hash::of(&**s.as_ref().unwrap())).collect();
    assert_eq!(markers::FILE_VERSION, 3);
    for version in [1, 2] {
        let back = Markers::from_file(&old_file(&sim.markers, version), BIKE).unwrap_or_else(|e| panic!("version {version}: {e}"));
        let read: Vec<u64> = back.slots.iter().map(|s| hash::of(&**s.as_ref().unwrap())).collect();
        assert_eq!(read, want, "version {version}");
        assert!(back.more(), "five set: the + is there");
    }
    // An old file read as the new one, or a new one claiming the old version, is refused.
    let mut wrong = sim.markers.to_file(BIKE);
    wrong[8..12].copy_from_slice(&2u32.to_le_bytes());
    assert!(Markers::from_file(&wrong, BIKE).is_err());
}
