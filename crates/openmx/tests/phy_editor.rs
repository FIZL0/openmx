//! P387 — the physics editor: a knob changes the bike's `.phy` as text and the ported loader reads it.
//!
//! `src/phy_editor.rs` is compiled in by path, as `tests/garage.rs` takes `roster.rs`. The bike is
//! `game_sim::stock::BIKE`, the harness's hand-written `.phy`, so nothing here needs the disc. What is
//! asserted: every knob at the disc's rides bit for bit as the file read the usual way; a knob turned
//! mid-ride is in the very next tick's state; a saved mod reads back to the same bike and knobs; and
//! the online rule.

#[allow(unused)]
#[path = "../src/files.rs"]
mod files;
#[allow(dead_code)]
#[path = "../src/phy_editor.rs"]
mod phy_editor;

use game_sim::collide::{Bundle, Record};
use game_sim::input::{Control, PADS, Report};
use game_sim::phy::Phy;
use game_sim::rider::{Rider, Tables};
use game_sim::trick::Set as Stunts;
use game_sim::world::World;
use game_sim::{Sim, stock};
use phy_editor::{Editor, Edits, STOCK};
use ps2_float::{F, Vf};
use std::sync::Arc;

const DT: F = F(1.0 / 60.0);

/// Flat ground at y = 0 and nothing solid on it.
struct Floor;

impl World for Floor {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        (Vf::new(at.x(), 0.0, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0))
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

/// `tests/remodify.rs`'s bike: the stand-in with the drag that lets it reach the original's top
/// speed and the wreck limits of a bike being ridden (without them it wrecks at once).
fn bike() -> String {
    let out = stock::BIKE.replace("Drag = 0.5", "Drag = 0.037");
    format!(
        "{out}
[Wrecks]
MaxWreckTime = 2
CannonballWreckTime = 5
FrontWreckSpeedLimit = 60
BackWreckSpeedLimit = 60
SideWreckSpeedLimit = 40
RacingDownWreckSpeedLimit = 40
FreestyleDownWreckSpeedLimit = 40
GroundPitchDownWreckAngle = 60
GroundPitchUpWreckAngle = 60
AirbornePitchDownWreckAngle = 120
AirbornePitchUpWreckAngle = 120
AirborneRollWreckAngle = 100
NoHanderWreckSpeedLimit = 400
NoHanderMaxRollAngle = 45
JoustWreckHeight = 2
CrossupWreck = 1
CrossupWreckTime = 0.3
"
    )
}

/// The gas (Cross) held.
fn gas() -> Report {
    Report::from_buttons(Control::Cross.mask())
}

fn sim_on(phy: Phy) -> Sim {
    let tables = Arc::new(Tables::build());
    let geom = Phy::read_bytes(stock::BIKE.as_bytes(), &tables.ride.curve);
    let rider = Rider::new(&phy, stock::mounts(&geom), stock::SPAWN, stock::FACING, Stunts::default());
    Sim::with_rider(0, tables, phy, rider)
}

fn tick(sim: &mut Sim) {
    let reports: [Report; PADS] = [gas(), Report::missing()];
    let controls = sim.poll(DT, &reports);
    sim.step(DT, &controls, &mut Floor);
}

/// Each tick's state hash over `frames` flat out.
fn ride(sim: &mut Sim, frames: usize) -> Vec<u64> {
    (0..frames)
        .map(|_| {
            tick(sim);
            sim.hash()
        })
        .collect()
}

fn curve() -> game_sim::camera::CurveTable {
    Tables::build().ride.curve
}

fn knob(label: &str) -> usize {
    phy_editor::knobs().position(|k| k.label == label).unwrap_or_else(|| panic!("no knob {label}"))
}

#[test]
fn the_stock_phy_rides_bit_identical_with_the_editor_closed() {
    let editor = Editor::new("Bike\\Race125P.phy", bike().as_bytes());
    assert!(editor.edits.is_stock());
    let disc = Phy::read_bytes(bike().as_bytes(), &curve());
    assert_eq!(format!("{:?}", editor.phy(&curve())), format!("{disc:?}"), "every knob at 100% is the file as the loader reads it");
    let (mut a, mut b) = (sim_on(disc), sim_on(editor.phy(&curve())));
    assert_eq!(ride(&mut a, 240), ride(&mut b, 240));
    // a knob turned and turned back is the disc's again
    let mut back = editor.clone();
    back.edits.step(knob("Engine Power"), true);
    back.edits.step(knob("Engine Power"), false);
    assert!(back.edits.is_stock());
    assert_eq!(format!("{:?}", back.phy(&curve())), format!("{:?}", editor.phy(&curve())));
}

#[test]
fn an_edited_value_reaches_the_sim_next_tick() {
    let mut editor = Editor::new("Bike\\Race125P.phy", bike().as_bytes());
    let mut sim = sim_on(editor.phy(&curve()));
    ride(&mut sim, 90);
    let mut stock_ride = sim.clone();
    for _ in 0..4 {
        editor.edits.step(knob("Engine Power"), true);
    }
    assert_eq!(editor.edits.0[knob("Engine Power")], STOCK + 4 * phy_editor::STEP);
    assert_eq!(editor.value(knob("Engine Power")).as_deref(), Some("13.2"), "HP1 = 11 at 120%");
    // the shell's swap at a tick: the seat's bike is the edited file from the next tick on, under the
    // ride's modifiers and retuned where it is, as a change of modifiers mid-ride is (P98)
    let mods = game_sim::modifiers::Mods::default();
    mods.resim(&mods, 0, &editor.phy(&curve()), &mut sim);
    assert_ne!(ride(&mut sim, 1), ride(&mut stock_ride, 1), "the edit is in the very next tick");
    ride(&mut sim, 60);
    ride(&mut stock_ride, 60);
    let (edited, stock) = (sim.rider.ride.chassis.speed.0, stock_ride.rider.ride.chassis.speed.0);
    assert!(edited > stock + 1.0, "more power, faster: {edited} vs {stock} ft/s a second on");
}

#[test]
fn every_knob_moves_values_the_stock_file_has() {
    let editor = Editor::new("race125p.phy", bike().as_bytes());
    for (i, k) in phy_editor::knobs().enumerate() {
        if !editor.has(i) {
            continue;
        }
        let mut e = editor.clone();
        e.edits.0[i] = 150;
        let (was, now) = (editor.ini(), e.ini());
        let moved: Vec<_> = was.entries().zip(now.entries()).filter(|(a, b)| a != b).map(|(a, _)| (a.0.to_string(), a.1.to_string())).collect();
        assert!(!moved.is_empty(), "{}", k.label);
        assert!(moved.iter().all(|(s, key)| k.takes(s, key)), "{}: only its own keys: {moved:?}", k.label);
    }
    assert!(editor.has(knob("Gears")) && editor.has(knob("Brakes")) && editor.has(knob("Inertia")));
    assert_eq!(phy_editor::scaled("0.0,1.6,-2", 1.5), "0,2.4,-3");
    assert_eq!(phy_editor::scaled("0.00000005", 2.0), "0.0000001");
    assert_eq!(phy_editor::scaled("30;deg", 0.5), "15;deg");
}

#[test]
fn an_edited_bike_saves_and_reloads() {
    let mods = std::env::temp_dir().join(format!("openmx-p387-{}", std::process::id())).join("mods");
    let mut editor = Editor::new("Bike\\Race125P.phy", bike().as_bytes());
    editor.edits.0[knob("Gravity")] = 50;
    editor.edits.0[knob("Brakes")] = 200;
    editor.edits.step(knob("Inertia"), false);
    let folder = editor.save(&mods, "Moon Bike!").unwrap();
    assert!(folder.ends_with("handling/phy_moon_bike"));
    let saved = phy_editor::load(&folder).unwrap();
    assert_eq!((saved.id.as_str(), saved.name.as_str(), saved.bike.as_str()), ("phy_moon_bike", "Moon Bike!", "race125p.phy"));
    assert_eq!(saved.edits, editor.edits, "the knobs come back");
    let file = Phy::read_bytes(&std::fs::read(folder.join("race125p.phy")).unwrap(), &curve());
    assert_eq!(format!("{file:?}"), format!("{:?}", editor.phy(&curve())), "the saved file is the edited bike");
    // and the knobs taken over a fresh editor on the disc's file build it again
    let mut again = Editor::new("Bike\\Race125P.phy", bike().as_bytes());
    again.take(&saved);
    assert_eq!(format!("{:?}", again.phy(&curve())), format!("{:?}", editor.phy(&curve())));
    assert_eq!(phy_editor::listed(&mods, "RACE125P.phy").len(), 1);
    assert!(phy_editor::listed(&mods, "pro250p.phy").is_empty());
    let (mut a, mut b) = (sim_on(file), sim_on(editor.phy(&curve())));
    assert_eq!(ride(&mut a, 120), ride(&mut b, 120));
    std::fs::remove_dir_all(mods.parent().unwrap()).unwrap();
}

#[test]
fn an_edited_bike_is_refused_online() {
    let (bytes, curve) = (bike(), curve());
    let disc = Phy::read_bytes(bytes.as_bytes(), &curve);
    assert_eq!(phy_editor::held(false, false, &disc, bytes.as_bytes(), &curve), None, "a free ride on the disc's file");
    assert!(phy_editor::held(true, false, &disc, bytes.as_bytes(), &curve).is_some(), "online every machine rides the room's files");
    assert!(phy_editor::held(false, true, &disc, bytes.as_bytes(), &curve).is_some(), "a tape rides the file it was written on");
    let mut laid = disc.clone();
    laid.handling = game_sim::handling::Handling::MxVsAtv;
    assert!(phy_editor::held(false, false, &laid, bytes.as_bytes(), &curve).is_some(), "another game's handling is no file the knobs edit");
    assert_eq!(Edits::default().changed(), 0);
}

/// P387a: a saved mod rides online where every seat holds it. Two machines, each with its own copy
/// of the mod and its own disc file, build every seat's bike from the pick's `handling/<id>`: the
/// seats ride bit for bit the same on both, and not as the disc's file. A machine whose mods folder
/// lacks the mod is refused by the handshake, with the mod named.
#[test]
fn two_seats_with_the_mod_ride_bit_identical_and_one_without_is_refused() {
    let root = std::env::temp_dir().join(format!("openmx-p387a-{}", std::process::id()));
    let (here, there) = (root.join("a").join("mods"), root.join("b").join("mods"));
    let mut editor = Editor::new("Bike\\Race125P.phy", bike().as_bytes());
    editor.edits.0[knob("Engine Power")] = 150;
    editor.edits.0[knob("Gravity")] = 70;
    let folder = editor.save(&here, "Loose").unwrap();
    // the other machine saved the same knobs on its own disc's file
    editor.save(&there, "Loose").unwrap();
    assert_eq!(phy_editor::matching(&here, "race125p.phy", &editor.edits).map(|s| s.id).as_deref(), Some("phy_loose"), "the editor on the saved knobs is on the mod");
    assert!(phy_editor::matching(&here, "race125p.phy", &Edits::default()).is_none(), "the disc's knobs are no mod");
    let packs = "machine/mxvatv_atv,handling/phy_loose";
    assert_eq!(phy_editor::picked(packs), Some("phy_loose"));
    assert_eq!(phy_editor::picked("machine/mxvatv_atv"), None);
    let curve = curve();
    let seat = |mods: &std::path::Path| phy_editor::seat_phy(Some(mods), packs, "Bike\\Race125P.phy", bike().as_bytes(), &curve).unwrap().expect("the pick's mod");
    let rides: Vec<Vec<u64>> = [&here, &here, &there, &there].into_iter().map(|m| ride(&mut sim_on(seat(m)), 240)).collect();
    assert!(rides.iter().all(|r| *r == rides[0]), "both seats on both machines ride the same bike");
    assert_eq!(rides[0], ride(&mut sim_on(editor.phy(&curve)), 240), "the mod's knobs over the disc's file");
    assert_ne!(rides[0], ride(&mut sim_on(Phy::read_bytes(bike().as_bytes(), &curve)), 240), "not the disc's file");
    assert!(phy_editor::seat_phy(Some(&here), "", "Bike\\Race125P.phy", bike().as_bytes(), &curve).unwrap().is_none(), "no mod picked, the disc's file");
    assert!(phy_editor::seat_phy(Some(&here), packs, "Bike\\Pro250P.phy", bike().as_bytes(), &curve).is_err(), "the mod edits another bike");
    assert!(phy_editor::seat_phy(None, packs, "Bike\\Race125P.phy", bike().as_bytes(), &curve).is_err(), "mods off");

    // the handshake: each machine says the packs it holds, and the seats' picks name the mod
    struct Holds(Vec<openmx_net::Pack>);
    impl openmx_net::Disc for Holds {
        fn packs(&self) -> Vec<openmx_net::Pack> {
            self.0.clone()
        }
    }
    let mut hash = openmx_net::handshake::fnv(&std::fs::read(folder.join("mod.json")).unwrap());
    hash = openmx_net::handshake::fnv_on(hash, &std::fs::read(folder.join("race125p.phy")).unwrap());
    let held = openmx_net::Pack { kind: phy_editor::KIND.into(), id: "phy_loose".into(), hash, name: "Loose 1.0".into(), how: String::new() };
    let pick = openmx_net::Pick { packs: "handling/phy_loose".into(), ..openmx_net::Pick::new("thq1", "race125p") };
    let mut room = openmx_net::Room::new("h", "free06", 2);
    room.take(openmx_net::HOST, std::slice::from_ref(&pick));
    room.take("g", &[pick]);
    let host = openmx_net::Handshake::of(&room, &Holds(vec![held.clone()]));
    assert_eq!(host.needs, vec!["handling/phy_loose".to_string()]);
    assert_eq!(openmx_net::Handshake::of(&room, &Holds(vec![held.clone()])).check(&host), Ok(()), "a seat with the mod rides");
    assert_eq!(
        openmx_net::Handshake::of(&room, &Holds(vec![])).check(&host),
        Err("Can't ride: a rider is on the pack Loose 1.0 (handling/phy_loose), and it isn't in your mods folder.".into()),
        "a seat without it is refused"
    );
    let other = openmx_net::Pack { hash: hash ^ 1, ..held };
    assert!(openmx_net::Handshake::of(&room, &Holds(vec![other])).check(&host).unwrap_err().contains("isn't the same files"), "nor one with other knobs");
    std::fs::remove_dir_all(&root).unwrap();
}
