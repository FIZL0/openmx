//! S248 — a driven car's particles: its tires' dust, dirt and chunks and its exhaust off the track's
//! `buggyparticles.dat` / `monsterparticles.dat` / `bajaparticles.dat`, run by the bike's and the
//! plane's manager on the car's four tires.
//!
//! The first half checks a car's tire and frame read as the original's car loader wires the manager
//! (`research/oracle.txt` S248), on a buggy off the disc. The second reads each car's list, its `.emt`s and free09's
//! sheet off the disc, takes the car on a flat floor through the shell's own [`vehicle::CarRide`],
//! and holds the gas: at rest nothing dusts, on the gas the tires do. Both skip without a disc. The
//! last (C18) runs the manager over PINE captures of the original's, row by row.
//!
//! `src/particles.rs` and `src/vehicle.rs` are compiled in by path, as the other tests take them.

#[allow(dead_code)]
#[path = "../src/particles.rs"]
mod particles;
#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;

use game_render::biplane;
use game_render::ini;
use game_render::mat::{M4, v3};
use game_sim::input::{EventQueue, Pad, Report};
use game_sim::collide::{Bundle, Record};
use game_sim::contact_search::Heightfield;
use game_sim::world::{Flat, World};
use particles::{Atlas, Disable, PlaneFrame, PlaneFx, TireFrame};
use ps2_float::{F, Vf};

/// The shell's frame, and the original's.
const DT: f32 = 1.0 / 30.0;

#[test]
fn the_lists_are_named_by_the_cars_directory() {
    assert_eq!(particles::car_list("buggy"), Some("buggyparticles.dat"));
    assert_eq!(particles::car_list("MonsterTruck"), Some("monsterparticles.dat"));
    assert_eq!(particles::car_list("baja"), Some("bajaparticles.dat"));
    assert_eq!(particles::car_list("biplane"), None);
}

/// A flat floor of one surface. [`Flat`] is surface 0, which the cars' `DirtSpray` and `Chunks` zone
/// descriptors (`mxparticlezones.dat`) say `none` to; a tire on any other surface throws.
struct Floor(Flat, u8);

impl World for Floor {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        self.0.ground(at)
    }
    fn surface(&mut self, _at: Vf) -> u8 {
        self.1
    }
    fn solid(&mut self, bundle: &Bundle, record: &mut Record) -> bool {
        self.0.solid(bundle, record)
    }
    fn heightfield(&mut self) -> Option<Box<dyn Heightfield + '_>> {
        self.0.heightfield()
    }
}

/// The buggy, taken on a flat floor through the shell's own [`vehicle::CarRide`], or `None` with no
/// disc to read it off.
fn buggy() -> Option<game_sim::car::Machine> {
    let Ok(mut disc) = game_render::Disc::open_configured() else {
        println!("skipped: no disc");
        return None;
    };
    let machines = disc.res("machine.res").expect("the machines' archive");
    let mut car = vehicle::CarRide::new(vehicle::Machine::Buggy, 0, None, &mut |n| machines.read_suffix(n).ok()).unwrap();
    let up = Vf::new(0.0, 1.0, 0.0, 0.0);
    car.take(&game_sim::markers::Spot::still(Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), up), &mut Flat::default()).unwrap();
    car.driven
}

/// A tire's record: solid ground the ground contact, a sorted contact the object's, slipping the
/// spin, the side ratio the side force, the surface's low byte the zone.
#[test]
fn a_cars_tire_reads_as_the_loader_wires_it() {
    let Some(m) = buggy() else { return };
    let mut w = m.wheels[0];
    w.solid = false;
    w.sorted = false;
    assert_eq!(TireFrame::car(&w).flags(), Disable::NO_CONTACT);
    w.solid = true;
    w.s.compression = F(0.75);
    w.s.surface = 3;
    w.s.slipping = true;
    w.s.side_ratio = F(-4.5);
    let t = TireFrame::car(&w);
    assert_eq!(t, TireFrame { ground: true, object: false, compression: 0.75, zone: 3, spinning: true, side: -4.5 });
    assert_eq!(t.flags(), Disable::GROUND_CONTACT);
    w.sorted = true;
    assert_eq!(TireFrame::car(&w).flags(), Disable::GROUND_CONTACT | Disable::OBJECT_CONTACT);
}

/// The car's frame: its four tires in order, its throttle after the rate limit, the clutch the engine
/// reads, its ground speed and never a wreck (C18: the loader's wreck word is never set, so a wrecked
/// car's manager reads `NotWrecking`); no collision object, no contacts.
#[test]
fn a_cars_frame_carries_its_four_tires_and_its_pedals() {
    let Some(mut m) = buggy() else { return };
    for w in &mut m.wheels {
        w.solid = false;
    }
    m.wheels[2].solid = true;
    m.wheels[3].s.compression = F(1.25);
    m.controls.throttle = F(0.5);
    m.controls.engaged_clutch = F(1.0);
    m.ground_speed = F(22.0);
    m.wrecked = true;
    m.body.velocity = Vf::new(1.0, 2.0, 3.0, 0.0);
    let f = PlaneFrame::car(&m, vec![("Node".into(), M4::IDENTITY)], None);
    assert_eq!(f.tires.len(), 4);
    assert!(f.tires[2].ground && !f.tires[0].ground);
    assert_eq!(f.tires[3].compression, 1.25);
    assert_eq!((f.throttle, f.clutch, f.airspeed, f.wrecked), (0.5, 1.0, 22.0, false));
    assert_eq!(f.vel, v3(1.0, 2.0, 3.0));
    assert!(f.ground_contacts.is_empty() && f.object_contacts.is_empty() && f.contact_vel.is_none());
    assert_eq!(f.nodes.len(), 1);
}

/// Each car's list off free09's archive, run on its car driven through the shell's own step on a flat
/// dirt floor ([`Floor`]): a second at rest, then four on the gas. At rest nothing comes off the tires;
/// on the gas each car's tire dust rises (past its 15 ft/s gate, with the strut compressed), and its
/// spinning rear tires throw dirt as the throttle spins them up.
#[test]
fn a_car_on_the_gas_dusts() {
    let Ok(mut disc) = game_render::Disc::open_configured() else {
        println!("skipped the disc's half: no disc");
        return;
    };
    let machines = disc.res("machine.res").expect("the machines' archive");
    let track = disc.res("Tracks\\free09_2.res").expect("free09's second archive");
    let first = disc.res("Tracks\\free09.res").expect("free09's first archive");
    let read = |name: &str| [&track, &first].iter().find_map(|a| a.read_suffix(&format!("\\{name}")).ok());
    let sheet = read(&particles::sheet("free09")).expect("free09's particle sheet");
    let texture = game_render::pst::Texture::parse(&read(&particles::sheet_texture("free09")).expect("its texture")).unwrap();
    let atlas = Atlas::parse(&ini::text(&sheet), texture.width, texture.height);
    let tables = game_sim::rider::Tables::build();
    let table = game_sim::bindings::Table::SHIPPED;

    for machine in [vehicle::Machine::Buggy, vehicle::Machine::Monster, vehicle::Machine::Baja] {
        let list = particles::car_list(machine.dir()).unwrap();
        let mut fx = PlaneFx::new(&biplane::emitters(&ini::text(&read(list).unwrap())), atlas.clone(), read);
        let names: Vec<&str> = fx.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.iter().filter(|n| n.ends_with("Tire Dust")).count() == 4, "{machine:?}: four tire dusts in {names:?}");
        assert!(fx.skipped.iter().all(|(_, why)| why == "TireTrack"), "{machine:?}: only the tire tracks are left out: {:?}", fx.skipped);

        // The model's nodes, all on its root, where the car's matrix puts them.
        let psg = machines.read_suffix(&format!("{}\\anim\\{}.psg", machine.dir(), machine.models()[0])).unwrap();
        let model = game_render::psg::Model::parse(&psg).unwrap();
        let mut binds: Vec<(String, M4)> = Vec::new();
        for name in fx.entries.iter().filter_map(|e| e.node.clone()) {
            let i = model.nodes.iter().position(|n| n.name.eq_ignore_ascii_case(&name)).unwrap_or_else(|| panic!("{machine:?}: no node {name}"));
            if !binds.iter().any(|(n, _)| n.eq_ignore_ascii_case(&name)) {
                binds.push((name, model.world(i)));
            }
        }

        let mut car = vehicle::CarRide::new(machine, 0, None, &mut |n| machines.read_suffix(n).ok()).unwrap();
        let mut floor = Floor(Flat::default(), 1);
        let up = Vf::new(0.0, 1.0, 0.0, 0.0);
        car.take(&game_sim::markers::Spot::still(Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), up), &mut floor).unwrap();
        let mut pad = Pad::new(0);
        pad.connected = true;
        let mut events = EventQueue::new();
        let count = |fx: &PlaneFx, end: &str| -> usize { fx.counts().iter().filter(|(n, _)| n.ends_with(end)).map(|(_, c)| c).sum() };
        let (mut dust, mut dirt, mut top) = (0usize, 0usize, 0f32);
        for tick in 0..150 {
            let buttons = if tick >= 30 { game_sim::input::Control::Cross.mask() } else { 0 };
            pad.poll(F(DT), &Report::from_buttons(buttons), &mut events);
            car.step(&pad, &table, &tables.ride, F(DT), &mut floor);
            let m = car.driven.as_ref().unwrap();
            let at = M4(std::array::from_fn(|r| std::array::from_fn(|c| m.matrix.0[r].0[c])));
            let nodes = binds.iter().map(|(n, b)| (n.clone(), b.mul(at))).collect();
            fx.step_over(&PlaneFrame::car(m, nodes, car.struck.as_ref()), DT, &mut |_| Some((0.0, 0)));
            if tick == 29 {
                assert_eq!(count(&fx, "Tire Dust"), 0, "{machine:?}: a car at rest raises no dust: {:?}", fx.counts());
                assert_eq!(count(&fx, "Dirt Spray"), 0, "{machine:?}: nor throws dirt: {:?}", fx.counts());
            }
            dust = dust.max(count(&fx, "Tire Dust"));
            dirt = dirt.max(count(&fx, "Dirt Spray"));
            top = top.max(m.ground_speed.0);
        }
        let compression: Vec<f32> = car.driven.as_ref().unwrap().wheels.iter().map(|w| w.s.compression.0).collect();
        println!("{machine:?}: {top:.1} ft/s on the gas, compression {compression:?}; at most {dust} dust and {dirt} dirt in the air");
        assert!(top > 15.0, "{machine:?}: the gas takes it past the dust's gate");
        assert!(dust > 0, "{machine:?}: a car on the gas dusts: {:?}", fx.counts());
        assert!(dirt > 0, "{machine:?}: its tires throw dirt: {:?}", fx.counts());
        // Every particle starts at the car: none is made at the world's origin by a missing node.
        let (m, _) = car.place().unwrap();
        assert!(fx.particles.iter().all(|p| (p.at.x - m.x()).hypot(p.at.z - m.z()) < 200.0));
    }
}

/// The shell, headless as a user would ride it, on a hand-written tape of full throttle that switches
/// into free09's monster truck at tick 30 and stays in it: each car's list loads with only its tire
/// tracks left out, and the truck ends the ride with its four tires' dust in the air.
#[test]
fn the_shell_dusts_a_driven_car() {
    if game_render::Disc::open_configured().is_err() {
        println!("skipped: no disc");
        return;
    }
    let frames = 200;
    let report = format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17));
    let mut tape = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape the monster on the gas\n");
    for _ in 0..frames {
        tape += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    tape += "switch 30 monster\n";
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("s248-car-particles");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join("monster.tape");
    std::fs::write(&path, tape).expect("the tape is written");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--track", "free09", "--frames", "220", "--steady-clock", "--play"])
        .arg(&path)
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));

    for (dir, list) in [("buggy", "buggyparticles.dat"), ("monstertruck", "monsterparticles.dat"), ("baja", "bajaparticles.dat")] {
        let line = said.lines().find(|l| l.starts_with(&format!("car: {dir}: particles: "))).unwrap_or_else(|| panic!("no {dir} particles:\n{said}"));
        assert!(line.contains(list) && line.contains("4 not run (Front Left Tire Track, Front Right Tire Track, Rear Left Tire Track, Rear Right Tire Track)"), "{line}");
    }
    let left = said.lines().rev().find(|l| l.starts_with("car: particles: ")).unwrap_or_else(|| panic!("no particles at the end:\n{said}"));
    println!("{left}");
    for tire in ["Front Left", "Front Right", "Rear Left", "Rear Right"] {
        assert!(left.contains(&format!("{tire} Tire Dust ")), "the truck's {tire} tire dusts: {left}");
    }
}

// C18: the car's manager against the original's, tick by tick. `context/capture/c18/` holds S149a's
// free07 drives played back in PCSX2 with the car's particle manager read every frame (journal
// 2026-11-03-c18-car-particles-capture): `truck.csv` and `baja.csv` on the gas, `truck-edge.csv`
// the truck parked on a map shortened under it, so the edge throws it (wreck reason 13) and it
// tumbles for 211 frames. Each row's state is the manager's after it read the row before's words.
// Not in the repo: without them the test says so and passes.

type Row = std::collections::HashMap<String, String>;

fn c18_capture(name: &str) -> Option<Vec<Row>> {
    let dir = match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => std::path::PathBuf::from(dir),
        None => std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context"),
    };
    let path = dir.join(format!("capture/c18/{name}.csv"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        println!("skipped {name}: no {}", path.display());
        return None;
    };
    let mut lines = text.lines();
    let head: Vec<String> = lines.next()?.split(',').map(str::to_string).collect();
    Some(lines.map(|l| head.iter().cloned().zip(l.split(',').map(str::to_string)).collect()).collect())
}

fn num(r: &Row, k: &str) -> f32 {
    r[k].parse().unwrap_or_else(|_| panic!("{k} = {:?}", r[k]))
}

fn word(r: &Row, k: &str) -> u32 {
    r[k].parse().unwrap_or_else(|_| panic!("{k} = {:?}", r[k]))
}

/// Row `r`'s words written into the car where the loader's records point: each tire's solid ground,
/// sorted contact, compression, surface, slipping and side ratio, the throttle, clutch and ground
/// speed, and the wreck reason (which the frame does not read).
fn drive(m: &mut game_sim::car::Machine, r: &Row) {
    for (t, w) in m.wheels.iter_mut().enumerate() {
        w.solid = word(r, &format!("t{t}_solid")) != 0;
        w.sorted = word(r, &format!("t{t}_sorted")) != 0;
        w.s.compression = F(num(r, &format!("t{t}_comp")));
        w.s.surface = word(r, &format!("t{t}_surface")) as usize;
        w.s.slipping = word(r, &format!("t{t}_slip")) != 0;
        w.s.side_ratio = F(num(r, &format!("t{t}_side")));
    }
    m.controls.throttle = F(num(r, "throttle"));
    m.controls.engaged_clutch = F(num(r, "clutch"));
    m.ground_speed = F(num(r, "speed"));
    m.wrecked = word(r, "wreck_reason") != 0;
    m.body.velocity = Vf::new(num(r, "body_vel.x"), num(r, "body_vel.y"), num(r, "body_vel.z"), 0.0);
}

/// Row `b` read the same frame's car as row `a`: the car and its emitters did not move (the game ran a
/// frame without stepping it).
fn repeat(a: &Row, b: &Row) -> bool {
    ["body_pos.x", "body_pos.z", "t0_comp", "t3_comp", "e2_carry", "e4_carry"].iter().all(|k| a[*k] == b[*k])
}

/// Entry `i`'s emitter where row `r` has it, as its node.
fn emitter_node(r: &Row, i: usize) -> M4 {
    let mut m = M4::IDENTITY;
    m.0[3] = [num(r, &format!("e{i}_at.x")), num(r, &format!("e{i}_at.y")), num(r, &format!("e{i}_at.z")), 1.0];
    m
}

/// Switched by the manager's flag alone (an exhaust, the generic dust spray, a spray): its on is the
/// port's `lit`. A tire dust's emitter stays on and its own gate (latch, compression) says when it
/// emits; a collision model's emits at contacts the capture does not hold.
fn switched(e: &particles::Entry) -> bool {
    matches!(e.kind, particles::Kind::Generic | particles::Kind::Exhaust { .. } | particles::Kind::Spray(_))
}

fn spray(e: &particles::Entry) -> bool {
    matches!(e.kind, particles::Kind::Spray(_))
}

/// What the capture says of entry `i` on row `r`, and the port's: the flag byte, a switched entry's
/// on (switch byte bit 2), a spray's fade clock to 2 ms.
#[derive(Debug, PartialEq)]
struct Seen {
    flag: u8,
    on: Option<bool>,
    clock: Option<i32>,
}

fn seen(e: &particles::Entry, r: &Row, i: usize) -> Seen {
    Seen {
        flag: word(r, &format!("e{i}_flag")) as u8,
        on: switched(e).then(|| word(r, &format!("e{i}_on")) & 4 != 0),
        clock: spray(e).then(|| (num(r, &format!("e{i}_clock")) * 500.0).round() as i32),
    }
}

fn port(e: &particles::Entry) -> Seen {
    Seen { flag: e.flag, on: switched(e).then_some(e.lit), clock: spray(e).then(|| (e.clock * 500.0).round() as i32) }
}

/// Puts the port's entry `i` where row `r` has it.
fn sync(e: &mut particles::Entry, r: &Row, i: usize) {
    e.flag = word(r, &format!("e{i}_flag")) as u8;
    if switched(e) {
        e.lit = word(r, &format!("e{i}_on")) & 4 != 0;
        e.sources.resize_with(1, Default::default);
        e.sources[0].last = e.lit.then(|| emitter_node(r, i).point(v3(0.0, 0.0, 0.0)));
    }
    if spray(e) {
        e.clock = num(r, &format!("e{i}_clock"));
    }
}

/// free07's list for `machine` (the captures' track), its `.emt`s and zones, and its sheet.
fn free07_fx(machine: vehicle::Machine) -> Option<PlaneFx> {
    let mut disc = game_render::Disc::open_configured().ok()?;
    let track = disc.res("Tracks\\free07_2.res").expect("free07's second archive");
    let first = disc.res("Tracks\\free07.res").expect("free07's first archive");
    let read = |name: &str| [&track, &first].iter().find_map(|a| a.read_suffix(&format!("\\{name}")).ok());
    let sheet = read(&particles::sheet("free07")).expect("free07's particle sheet");
    let texture = game_render::pst::Texture::parse(&read(&particles::sheet_texture("free07")).expect("its texture")).unwrap();
    let atlas = Atlas::parse(&ini::text(&sheet), texture.width, texture.height);
    let list = particles::car_list(machine.dir()).unwrap();
    Some(PlaneFx::new(&biplane::emitters(&ini::text(&read(list).unwrap())), atlas, read))
}

/// The original's car manager, row by row, against the port's fed through [`PlaneFrame::car`] with
/// the row's words in a car: which entries are live or zone-off, which switched entries are on, a
/// spray's fade, and on which ticks each emitter emits (its carry moves). Over the wreck the
/// original's manager never reads a wreck (its wreck word is never set): the exhausts, whose only
/// `Disable` is `Wrecking`, stay live through all of it, and so do the port's.
#[test]
fn the_cars_emitters_follow_the_original() {
    if game_render::Disc::open_configured().is_err() {
        println!("skipped: no disc");
        return;
    }
    let mut ran = 0;
    for (name, machine) in [("truck", vehicle::Machine::Monster), ("truck-edge", vehicle::Machine::Monster), ("baja", vehicle::Machine::Baja)] {
        let Some(rows) = c18_capture(name) else { continue };
        let mut fx = free07_fx(machine).expect("free07 off the disc");
        let mut car = buggy().expect("a car to write the words into");
        let n = fx.entries.len();
        // The four tire tracks are the list's last and not the port's: its entries are the capture's first.
        assert!(rows[0].contains_key(&format!("e{}_flag", n + 3)) && !rows[0].contains_key(&format!("e{}_flag", n + 4)), "{name}: {n} entries");
        for (i, e) in fx.entries.iter_mut().enumerate() {
            sync(e, &rows[0], i);
        }
        let (mut compared, mut missed, mut switches, mut wreck_rows, mut repeats, mut torn) = (0, Vec::new(), Vec::new(), 0, 0, 0);
        let (mut ours, mut theirs) = (vec![0u32; n], vec![0u32; n]);
        let mut emitted = vec![0usize; n];
        for (a, b) in rows.iter().zip(&rows[1..]) {
            let row = word(b, "frame_counter");
            assert_eq!(row, word(a, "frame_counter") + 1, "{name}: a row a frame");
            if repeat(a, b) {
                repeats += 1;
                continue;
            }
            drive(&mut car, b);
            let nodes = fx.entries.iter().enumerate().filter_map(|(i, e)| Some((e.node.clone()?, emitter_node(b, i)))).collect();
            let frame = PlaneFrame::car(&car, nodes, None);
            assert!(!frame.wrecked);
            let carry_before: Vec<f32> = fx.entries.iter().map(|e| e.sources.first().map_or(0.0, |s| s.carry)).collect();
            // The particles already in the air go first, in order, and the frame's new ones after them.
            let dt = num(b, "dt");
            let before = fx.particles.iter().filter(|p| (*p).clone().step(&fx.entries[p.emitter].emt, dt)).count();
            fx.step(&frame, dt);
            for p in &fx.particles[before..] {
                emitted[p.emitter] += 1;
            }
            if (0..4).any(|t| b[&format!("r{t}_zone")] != b[&format!("t{t}_surface")]) {
                torn += 1;
                for (i, e) in fx.entries.iter_mut().enumerate() {
                    sync(e, b, i);
                }
                continue;
            }
            compared += 1;
            if word(a, "wreck_reason") != 0 {
                wreck_rows += 1;
            }
            for i in 0..n {
                let e = &fx.entries[i];
                let (want, got) = (seen(e, b, i), port(e));
                let moved_theirs = num(a, &format!("e{i}_carry")) != num(b, &format!("e{i}_carry"));
                let moved_ours = carry_before[i] != e.sources.first().map_or(0.0, |s| s.carry);
                theirs[i] += moved_theirs as u32;
                ours[i] += moved_ours as u32;
                let carry_differs = !matches!(e.kind, particles::Kind::CollisionModel) && moved_theirs != moved_ours;
                if want.flag != got.flag || want.on != got.on {
                    switches.push(format!("{row} e{i} {}: port {got:?}, original {want:?}", e.name));
                }
                if want != got || carry_differs {
                    missed.push(format!("{row} e{i} {}: port {got:?} emitted {moved_ours}, original {want:?} emitted {moved_theirs}", e.name));
                    sync(&mut fx.entries[i], b, i);
                }
            }
        }
        let names: Vec<&str> = fx.entries.iter().map(|e| e.name.as_str()).collect();
        println!("{name}: {compared} rows compared ({wreck_rows} wrecked), {repeats} repeats, {torn} torn, {} misses", missed.len());
        println!("  ticks emitted, original: {theirs:?}");
        println!("  ticks emitted, port:     {ours:?}");
        println!("  particles the port made: {emitted:?}");
        println!("  entries: {names:?}");
        for m in missed.iter().take(30) {
            println!("  {m}");
        }
        assert!(compared >= 560, "{name}: {compared} rows");
        // Every entry's flag and every switched entry's on, on every row compared.
        assert!(switches.is_empty(), "{name}: {} flag or switch misses: {:?}", switches.len(), &switches[..switches.len().min(10)]);
        // The ticks each emitter emits on and the sprays' fades: within one in a hundred entry-ticks
        // (the rest are the original reading a word of the frame before as the car leaves the ground
        // or a spray's spin starts: journal C18).
        assert!(missed.len() * 100 < compared * n, "{name}: {} misses in {} entry-ticks", missed.len(), compared * n);
        if name == "truck-edge" {
            assert!(wreck_rows > 150, "the edge throws the truck: {wreck_rows} wrecked rows compared");
            // Through the wreck the exhausts stay live in both.
            let exhausts: Vec<usize> = (0..n).filter(|&i| matches!(fx.entries[i].kind, particles::Kind::Exhaust { .. })).collect();
            assert!(!exhausts.is_empty());
            for r in rows.iter().filter(|r| word(r, "wreck_reason") != 0) {
                assert!(exhausts.iter().all(|&i| word(r, &format!("e{i}_flag")) == 1), "{}: an exhaust off in the wreck", r["frame_counter"]);
            }
        }
        ran += 1;
    }
    if ran == 0 {
        println!("skipped: no C18 capture");
    }
}
