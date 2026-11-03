//! S269 — a driven car's tire tracks: the four `TireTrack` entries of each car's list
//! (`buggyparticles.dat`, `monsterparticles.dat`, `bajaparticles.dat`) laid by the bike's tracks
//! (`src/tire_tracks.rs`) on the car's four tires, read as the original's car loader wires the
//! manager's tire records (`research/oracle.txt` S248).
//!
//! The disc's half reads the lists off free09's archives; the ride takes each car on a flat floor
//! through the shell's own [`vehicle::CarRide`] and holds the gas, stepping the tracks after every
//! tick as `src/main.rs`'s `step_car_tire_marks` does; the shell's half rides the monster truck
//! headless. All skip without a disc, saying so.
//!
//! `src/tire_tracks.rs`, `src/particles.rs` and `src/vehicle.rs` are compiled in by path, as the
//! other tests take the shell's files.

#[allow(dead_code)]
#[path = "../src/tire_tracks.rs"]
mod tire_tracks;
#[allow(dead_code)]
#[path = "../src/particles.rs"]
mod particles;
#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;

use game_render::ini;
use game_sim::collide::{Bundle, Record};
use game_sim::input::{EventQueue, Pad, Report};
use game_sim::world::{Flat, World};
use ps2_float::{F, Vf};
use tire_tracks::{Bike, Disable, Params, Tracks};

/// The shell's frame, and the original's.
const DT: f32 = 1.0 / 30.0;

/// A floor of one surface (surface 0 is the one the `TireTrack` descriptor turns off): flat at 0,
/// then up a slope along +z onto a flat top, nothing solid. The foot of the slope loads the tires
/// past the lists' `MinimumCompressionThreshold`, which a flat floor never does.
struct Floor(u8);

/// Where the slope starts down +z, its rise over its run, and the top's height, in feet.
const SLOPE: (f32, f32, f32) = (60.0, 0.35, 8.0);

impl World for Floor {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        let (start, slope, top) = SLOPE;
        let y = (at.z() - start) * slope;
        if !(0.0..top).contains(&y) {
            return (Vf::new(at.x(), y.clamp(0.0, top), at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0));
        }
        let n = 1.0 / (1.0 + slope * slope).sqrt();
        (Vf::new(at.x(), y, at.z(), 1.0), Vf::new(0.0, n, -slope * n, 0.0))
    }
    fn surface(&mut self, _at: Vf) -> u8 {
        self.0
    }
    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

/// free09's archives, as the shell reads a freeride's.
fn free09() -> Option<[game_render::Res; 2]> {
    let Ok(mut disc) = game_render::Disc::open_configured() else {
        println!("skipped: no disc");
        return None;
    };
    Some([disc.res("Tracks\\free09_2.res").expect("free09's second archive"), disc.res("Tracks\\free09.res").expect("its first")])
}

/// `machine`'s tracks off free09, its list and the zones file.
fn list(archives: &[game_render::Res], machine: vehicle::Machine) -> Vec<Params> {
    let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(&format!("\\{name}")).ok());
    let zones = read(tire_tracks::ZONES).map(|z| ini::text(&z));
    let name = particles::car_list(machine.dir()).unwrap();
    tire_tracks::list(&ini::text(&read(name).unwrap_or_else(|| panic!("no {name}"))), zones.as_deref())
}

/// The ride's ticks.
const TICKS: usize = 150;

const CARS: [vehicle::Machine; 3] = [vehicle::Machine::Buggy, vehicle::Machine::Monster, vehicle::Machine::Baja];

/// Each car's list has a track on each of its four tires, the bike's tread, off on the descriptor's
/// surface 0, and off with no contact, an object contact or a wreck; the widths are each car's own.
#[test]
fn each_car_lists_a_track_a_tire() {
    let Some(archives) = free09() else { return };
    for machine in CARS {
        let l = list(&archives, machine);
        assert_eq!(l.iter().map(|p| p.tire).collect::<Vec<_>>(), [0, 1, 2, 3], "{machine:?}: {l:?}");
        println!("{machine:?}: widths {:?}, compression {:?}", l.iter().map(|p| p.width).collect::<Vec<_>>(), l.iter().map(|p| (p.min_compression, p.max_compression)).collect::<Vec<_>>());
        for p in &l {
            assert_eq!((p.material.as_str(), p.strips), ("ttrack", 32), "{machine:?}: {}", p.name);
            assert_eq!(p.disable, Disable::NO_CONTACT | Disable::OBJECT_CONTACT | Disable::WRECKING, "{machine:?}: {}", p.name);
            assert!(!p.zones[0] && p.zones[1..].iter().all(|z| *z), "{machine:?}: {}: only surface 0 is off", p.name);
        }
    }
}

/// A car's tires as the tracks read them: the contact and the ground's normal, solid ground and a
/// sorted contact as the two contacts, the compression, slipping as the spin, the surface; the
/// ground speed and the wreck.
#[test]
fn a_cars_tires_read_as_the_loader_wires_them() {
    let Ok(mut disc) = game_render::Disc::open_configured() else {
        println!("skipped: no disc");
        return;
    };
    let machines = disc.res("machine.res").expect("the machines' archive");
    let mut car = vehicle::CarRide::new(vehicle::Machine::Buggy, 0, None, &mut |n| machines.read_suffix(n).ok()).unwrap();
    let up = Vf::new(0.0, 1.0, 0.0, 0.0);
    car.take(&game_sim::markers::Spot::still(Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), up), &mut Flat::default()).unwrap();
    let mut m = car.driven.unwrap();
    for w in &mut m.wheels {
        (w.solid, w.sorted) = (true, false);
    }
    m.wheels[1].solid = false;
    m.wheels[2].sorted = true;
    m.wheels[3].s.slipping = true;
    m.wheels[3].s.surface = 0x105;
    m.wrecked = true;
    let b = Bike::car(&m);
    assert_eq!(b.tires.len(), 4);
    for (t, w) in b.tires.iter().zip(&m.wheels) {
        assert_eq!((t.at.x, t.at.y, t.at.z), (w.s.contact.x(), w.s.contact.y(), w.s.contact.z()));
        assert_eq!((t.normal.x, t.normal.y, t.normal.z), (w.ground_normal.x(), w.ground_normal.y(), w.ground_normal.z()));
        assert_eq!(t.compression, w.s.compression.0);
    }
    assert_eq!(b.tires.iter().map(|t| t.flags()).collect::<Vec<_>>(), [2, 1, 6, 2]);
    assert!(b.tires[3].spinning && !b.tires[0].spinning);
    assert_eq!(b.tires[3].zone, 5);
    assert!(b.wrecking);
    assert_eq!(b.speed, m.ground_speed.0);
}

/// Each car off the disc on the gas over a surface-1 [`Floor`]: nothing laid at rest; the launch's
/// wheelspin lays the driven tires' tracks (on the flat the loads are under the lists'
/// `MinimumCompressionThreshold`); then the slope's foot loads all four and every tire lays. Every
/// row is by its own tire's contact. Over surface 0 none is laid.
///
/// Not a drop: a tire that has left the ground fades in again only after a tick on the ground it
/// does not mark (`research/oracle.txt` V40, the original's `last = tire while the strip is ended`),
/// so a landing straight into a hard load lays nothing.
#[test]
fn a_car_on_the_gas_leaves_four_tracks() {
    let Some(archives) = free09() else { return };
    let Ok(mut disc) = game_render::Disc::open_configured() else { return };
    let machines = disc.res("machine.res").expect("the machines' archive");
    let tables = game_sim::rider::Tables::build();
    let table = game_sim::bindings::Table::SHIPPED;
    for machine in CARS {
        let l = list(&archives, machine);
        for surface in [1u8, 0] {
            let mut car = vehicle::CarRide::new(machine, 0, None, &mut |n| machines.read_suffix(n).ok()).unwrap();
            let mut floor = Floor(surface);
            let up = Vf::new(0.0, 1.0, 0.0, 0.0);
            car.take(&game_sim::markers::Spot::still(Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), up), &mut floor).unwrap();
            let mut tracks = Tracks::new(&l, 7);
            let mut pad = Pad::new(0);
            pad.connected = true;
            let mut events = EventQueue::new();
            let laid = |t: &Tracks| t.tracks.iter().map(|t| t.strip.ring.triangles().len()).collect::<Vec<_>>();
            let mut far = 0f32;
            let mut launch = Vec::new();
            for tick in 0..TICKS {
                if launch.is_empty() && car.driven.as_ref().unwrap().wheels[0].s.contact.z() > SLOPE.0 - 1.0 {
                    launch = laid(&tracks);
                }
                let buttons = if tick >= 30 { game_sim::input::Control::Cross.mask() } else { 0 };
                pad.poll(F(DT), &Report::from_buttons(buttons), &mut events);
                car.step(&pad, &table, &tables.ride, F(DT), &mut floor);
                let m = car.driven.as_ref().unwrap();
                let before: Vec<usize> = tracks.tracks.iter().map(|t| t.strip.ring.batches.iter().flatten().flatten().count()).collect();
                tracks.step(&Bike::car(m));
                // A row laid this tick is by its own tire.
                for (k, t) in tracks.tracks.iter().enumerate() {
                    if t.strip.ring.batches.iter().flatten().flatten().count() != before[k] {
                        let c = m.wheels[t.params.tire].s.contact;
                        let d = t.strip.last_lay.sub(game_render::mat::v3(c.x(), c.y(), c.z())).len();
                        far = far.max(d);
                    }
                }
                if tick == 29 {
                    assert!(laid(&tracks).iter().all(|n| *n == 0), "{machine:?}: a car at rest lays nothing: {:?}", laid(&tracks));
                }
            }
            let n = laid(&tracks);
            let speed = car.driven.as_ref().unwrap().ground_speed.0;
            println!("{machine:?} on surface {surface}: {speed:.1} ft/s, triangles {launch:?} at the slope's foot and {n:?} at the end, rows at most {far:.3} ft from their tire");
            if surface == 0 {
                assert!(n.iter().all(|n| *n == 0), "{machine:?}: surface 0 is off: {n:?}");
            } else {
                // The launch spins the driven tires; the landing loads all four past the threshold.
                assert!(launch.iter().any(|n| *n > 0), "{machine:?}: the launch's spin lays a track: {launch:?}");
                assert!(n.iter().zip(&launch).all(|(n, l)| n > l), "{machine:?}: the landing lays all four: {launch:?} then {n:?}");
                assert!(far < 1e-3, "{machine:?}: a row {far} ft off its tire");
            }
        }
    }
}

/// The shell, headless, on a tape of full throttle that switches into free09's buggy at tick 30: every
/// car's list gives the tracks their four, and the buggy ends with four laid. Not the monster truck:
/// it is switched in above the ground, lands on its rears past their threshold (which lays nothing,
/// as [`a_car_on_the_gas_leaves_four_tracks`] says) and on free09's flat start never loads them that
/// hard again, so only its fronts lay.
#[test]
fn the_shell_lays_a_driven_cars_tracks() {
    if game_render::Disc::open_configured().is_err() {
        println!("skipped: no disc");
        return;
    }
    let frames = 200;
    let report = format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17));
    let mut tape = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape the buggy on the gas\n");
    for _ in 0..frames {
        tape += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    tape += "switch 30 buggy\n";
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("s269-car-tire-tracks");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join("buggy.tape");
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
    for dir in ["buggy", "monstertruck", "baja"] {
        let line = format!("car: {dir}: tire tracks: 4 on tires 0, 1, 2, 3");
        assert!(said.lines().any(|l| l == line), "no {line:?}:\n{said}");
    }
    let left = said.lines().rev().find(|l| l.starts_with("car: tire tracks: ")).unwrap_or_else(|| panic!("no tracks at the end:\n{said}"));
    println!("{left}");
    for tire in ["Front Left", "Front Right", "Rear Left", "Rear Right"] {
        let n: usize = left
            .split(", ")
            .find_map(|p| p.trim_start_matches("car: tire tracks: ").strip_prefix(&format!("{tire} Tire Track ")))
            .and_then(|n| n.parse().ok())
            .unwrap_or_else(|| panic!("no {tire}: {left}"));
        assert!(n > 0, "the buggy's {tire} tire lays a track: {left}");
    }
}
