//! MD149 — **MX vs. ATV's car on MX Unleashed's worlds**: [`atv_car::Ride`] built from each car's files
//! ([`atv_car::Load`], MD150's) and stepped headless on free09 (the split track the shell reads: its
//! terrain and the props of both archives) with a pad, as a shell steps it.
//!
//! The original has no car on an MX Unleashed track, so nothing here is compared with a capture: each
//! car is checked for riding. It stands, gets going on the gas, turns on the stick, leaves the ground
//! off a lip and lands, and wrecks at the map's edge, has its wreck clock run out and is put back at
//! the spot MX Unleashed's put-back record kept, riding on. Every call the ride hands on unanswered is
//! one [`atv_car::Ride`] drops on purpose.
//!
//! MD149b rigs the car's collision record (its `<car>m.col`, [`atv_car::Ride::rig`]): the record built
//! from the files is the images' (MD39a), a rigged car rolled over wrecks tipped and is put back, and
//! one driven head on into a prop stops on it. The gravity call is answered once rigged; it stays in
//! [`DROPPED`] for the un-rigged rides above.
use game_render::disc::Disc;
use game_sim::atv_car::{self, Call, Load, RIDING, WRECK_EDGE, WRECK_TIPPED};
use game_sim::bindings::{Command, Table};
use game_sim::camera::CurveTable;
use game_sim::car::MxvatvMachine;
use game_sim::input::{Manager, Report};
use game_sim::quad;
use game_sim::track::{Scene, Terrain as TerrainFile};
use game_sim::world::{Ground, Place, Props, World as _};
use ps2_float::{F, Vf};
use ps2_oracle::{Kit, skip};

/// Each car: its name, its machine, its file and its chassis model.
const CARS: [(&str, MxvatvMachine, &str, &str); 5] = [
    ("buggy", MxvatvMachine::Buggy, "buggy.phy", "buggy"),
    ("baja", MxvatvMachine::Baja, "baja.phy", "baja"),
    ("sandrail", MxvatvMachine::Sandrail, "sandrail.phy", "sandrail"),
    ("golf", MxvatvMachine::GolfCart, "golfcart.phy", "golfcart"),
    ("truck", MxvatvMachine::MonsterTruck, "monstertruck.phy", "monstertruck"),
];

/// The calls a ride on an MX Unleashed world drops on purpose ([`atv_car::Ride`]'s list), by name.
const DROPPED: &[&str] = &[
    "GravityRecord",
    "SpatialMove",
    "PartMove",
    "Gather",
    "LeaveWorld",
    "RidingLetGo",
    "ReleaseRecord",
    "Cue",
    "LandingSound",
    "ShiftSound",
    "PreloadCue",
    "Rumble",
    "SubtreeOutOfDate",
    "NodeRebound",
    "RideRumble",
    "WreckLook",
    "SortPlace",
    "CollisionOn",
    "Unlist",
    "Place",
    "ClassStep",
    "Prepare",
    "Ride",
    "Sync",
    "Snapshot",
    "Network",
    "Parts",
];

/// The tick the shell steps (the game's 30 Hz frame).
fn dt() -> F {
    F::from_bits(0x3d08_850a)
}

/// The split free09's archives, read by suffix (tests/c16_fly.rs's).
fn track_read(disc: &mut Disc) -> impl Fn(&str) -> Result<Vec<u8>, String> + use<> {
    let parts = ["Tracks\\free09_ss.res", "Tracks\\free09_ss_2.res"].map(|n| disc.res(n).unwrap_or_else(|e| panic!("{n}: {e}")));
    move |want: &str| parts.iter().find_map(|r| r.read_suffix(want).ok()).ok_or_else(|| format!("no archive of this track has {want}"))
}

/// The split free09 as the shell reads it, handed to `then` (tests/atv_quad/mxu.rs's).
fn in_free09<R>(disc: &mut Disc, then: impl FnOnce(&mut Place<'_>) -> R) -> R {
    let read = track_read(disc);
    let scene = Scene::parse(&read(".scn").unwrap()).expect("parse the scene");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let bytes = read(leaf.rsplit('\\').next().unwrap()).unwrap();
    let terrain = TerrainFile::parse(&bytes).expect("parse the terrain");
    let ground = Ground::new(&terrain, &scene.terrain);
    let (props, _) = Props::load(&scene, |w: &str| read(w));
    then(&mut Place { ground, props })
}

/// Where each car is put down: on a rutted track of free09's (the quad's spot), facing `+z`.
const SPOT: (f32, f32) = (1872.0, 1900.0);

/// The phases of the ride, by tick: standing, then the gas, then the gas with the stick hard right.
const STAND: usize = 60;
const GAS: usize = 150;
const STEER: usize = 300;
/// The ticks ridden on the gas to keep a put-back spot before the car is taken to the map's edge.
const KEEP: usize = 240;

/// How much a lip rises and falls ([`lip`]), ft; the ticks ridden at it; the ticks in the air that
/// are a jump.
const LIP_RISE: f32 = 4.0;
const LIP_TICKS: usize = 240;
const AIRBORNE: usize = 6;

/// The nearest spot to [`SPOT`] (on an 8 ft grid, eight headings) where the ground rises [`LIP_RISE`]
/// over the 15 ft before it and falls as much over the 15 ft after, with 90 ft of run-up within 12 ft
/// of level; where the run-up starts, and the heading (tests/atv_quad/mxu.rs's).
fn lip(world: &Place<'_>) -> Option<((f32, f32), f32)> {
    let h = |x: f32, z: f32| world.ground.at(F(x), F(z)).map(|g| g.y.0);
    let mut best: Option<(f32, (f32, f32), f32)> = None;
    for i in -100..=100 {
        for j in -100..=100 {
            let (x, z) = (SPOT.0 + 8.0 * i as f32, SPOT.1 + 8.0 * j as f32);
            let d = (8.0 * i as f32).hypot(8.0 * j as f32);
            if best.is_some_and(|b| b.0 <= d) {
                continue;
            }
            let Some(top) = h(x, z) else { continue };
            for a in 0..8 {
                let yaw = a as f32 * std::f32::consts::FRAC_PI_4;
                let (ux, uz) = (yaw.sin(), yaw.cos());
                let at = |t: f32| h(x + ux * t, z + uz * t);
                let (Some(face), Some(after)) = (at(-15.0), at(15.0)) else { continue };
                if top - face < LIP_RISE || top - after < LIP_RISE {
                    continue;
                }
                let run: Option<Vec<f32>> = (2..=10).map(|k| at(-15.0 - 10.0 * k as f32)).collect();
                let Some(run) = run else { continue };
                let (lo, hi) = run.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
                if hi - lo < 12.0 {
                    best = Some((d, (x - ux * 105.0, z - uz * 105.0), yaw));
                }
            }
        }
    }
    best.map(|(_, start, yaw)| (start, yaw))
}

/// The quaternion turning `+z` to `yaw` about `+y`.
fn yaw_quat(yaw: f32) -> Vf {
    Vf::new(0.0, (yaw / 2.0).sin(), 0.0, (yaw / 2.0).cos())
}

/// The flattened heading of a velocity, radians.
fn heading(v: Vf) -> f32 {
    v.x().atan2(v.z())
}

/// The name of a call, its variant.
fn call_name(c: &Call) -> String {
    format!("{c:?}").split([' ', '(', '{']).next().unwrap().to_string()
}

/// A shell's pad: the gas held or not, the left stick's x byte.
struct Hands {
    input: Manager,
    table: Table,
}

impl Hands {
    fn new() -> Hands {
        Hands { input: Manager::connected(), table: Table::SHIPPED }
    }
    /// One tick of the ride with the gas held or not and the stick at `x` (0x80 the middle).
    fn step(&mut self, ride: &mut atv_car::Ride, world: &mut Place<'_>, gas: bool, x: u8) {
        let mask = if gas { self.table.control(0, Command::GAS).expect("the gas is bound").mask() } else { 0 };
        let mut r = Report::from_buttons(mask);
        r.set_axis(2, x);
        self.input.tick(dt(), &[r, Report::idle()]);
        ride.step(self.input.pad(0), &self.table, dt(), world);
    }
}

/// The context's car files, and the curve table.
struct Files {
    kit: Kit,
    curve: CurveTable,
}

impl Files {
    fn load() -> Option<Files> {
        let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
        for dir in ["mxvatv-ps2/md39/phy", "mxvatv-ps2/md39/model"] {
            if !kit.ctx(dir).exists() {
                skip(&format!("{} is not there: MD39's files", kit.ctx(dir).display()));
                return None;
            }
        }
        Some(Files { kit, curve: CurveTable::build() })
    }
    /// The car built from its files alone, at the origin facing `+z` (MD150's [`Load::read`]).
    fn load_car(&self, machine: MxvatvMachine, phy: &str, model: &str) -> Load {
        let ini = game_sim::phy::Ini::parse_bytes(&std::fs::read(self.kit.ctx("mxvatv-ps2/md39/phy").join(phy)).unwrap());
        let bytes = std::fs::read(self.kit.ctx("mxvatv-ps2/md39/model").join(format!("{model}.psg"))).unwrap();
        let model = quad::Model::parse(&bytes).unwrap();
        let origin = quad::spawn_frame(Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 1.0));
        let points = atv_car::tire_points(&model, &origin).unwrap();
        Load::read(&ini, machine, model.half_box(&origin), points, &self.curve)
    }
}

/// The car's node over the ground at `(x, z)`: high enough that its lowest tire bottom is a foot
/// over it.
fn over(world: &mut Place<'_>, load: &Load, (x, z): (f32, f32)) -> Vf {
    let (floor, _) = world.ground(Vf::new(x, 1.0e4, z, 1.0));
    assert!(floor.y() > -1.0e5, "no ground at {x} {z}");
    let car = load.car();
    let lift = car.tires.iter().map(|t| t.radius.0 - t.mount.y()).fold(0.0f32, f32::max) + 1.0;
    Vf::new(x, floor.y() + lift, z, 1.0)
}

/// The calls handed on unanswered that are not dropped on purpose.
fn unexpected(handed: &std::collections::BTreeSet<String>) -> Vec<&String> {
    handed.iter().filter(|n| !DROPPED.contains(&n.as_str())).collect()
}

#[test]
fn each_car_rides_free09_jumps_wrecks_and_is_put_back() {
    let Some(files) = Files::load() else { return };
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return;
        }
    };
    in_free09(&mut disc, |world| {
        let (start, yaw) = lip(world).expect("a lip near the spot");
        let area = world.area().expect("free09 has a map edge");
        for (name, machine, phy, model) in CARS {
            let load = files.load_car(machine, phy, model);
            let mut handed = std::collections::BTreeSet::new();

            // Stand, gas, steer.
            let mut ride = load.ride(over(world, &load, SPOT), yaw_quat(0.0), F::ZERO);
            let mut hands = Hands::new();
            let at = ride.body.position;
            let (mut top, mut turn, mut was, mut lowest) = (0.0f32, 0.0f32, 0.0f32, f32::MAX);
            for tick in 0..STEER {
                let (gas, x) = (tick >= STAND, if tick >= GAS { 0xff } else { 0x80 });
                hands.step(&mut ride, world, gas, x);
                handed.extend(ride.rest.drain(..).map(|c| call_name(&c)));
                let (p, v) = (ride.body.position, ride.body.velocity);
                assert!(p.x().is_finite() && p.y().is_finite() && p.z().is_finite(), "{name}: tick {tick} at {p:?}");
                assert_eq!(ride.car.state, RIDING, "{name}: wrecked at tick {tick}");
                let (floor, _) = world.ground(Vf::new(p.x(), 1.0e4, p.z(), 1.0));
                lowest = lowest.min(p.y() - floor.y());
                let speed = v.x().hypot(v.z());
                if tick == STAND - 1 {
                    let moved = (p.x() - at.x()).hypot(p.z() - at.z());
                    assert!(moved < 3.0, "{name}: moved {moved} ft standing");
                    let wheels: Vec<String> = ride.car.tires.iter().map(|t| format!("{:.2}", t.ground_point.y() - floor.y())).collect();
                    eprintln!("{name}: standing, body {:.2} ft over the ground, tire ground points {wheels:?}, cofg {:?}", p.y() - floor.y(), ride.car.cofg);
                }
                // The heading's turn over the steer, tick by tick (each step's wrapped to ±180°).
                if tick >= GAS {
                    let d = (heading(v) - was).to_degrees();
                    turn += (d + 540.0).rem_euclid(360.0) - 180.0;
                }
                was = heading(v);
                top = top.max(speed);
            }
            eprintln!("{name}: top {top:.1} ft/s, turned {turn:.1}°, lowest body over the ground {lowest:.2} ft");
            assert!(top > 20.0, "{name}: top speed {top} ft/s on the gas");
            assert!(turn > 20.0, "{name}: turned {turn}° on the stick held right");
            assert!(lowest > -1.0, "{name}: the body went {lowest} ft under the ground");

            // Off the lip.
            let mut ride = load.ride(over(world, &load, start), yaw_quat(yaw), F::ZERO);
            let mut hands = Hands::new();
            let (mut longest, mut air, mut landed) = (0usize, 0usize, false);
            for tick in 0..LIP_TICKS {
                hands.step(&mut ride, world, true, 0x80);
                handed.extend(ride.rest.drain(..).map(|c| call_name(&c)));
                assert_eq!(ride.car.state, RIDING, "{name}: wrecked at lip tick {tick}");
                if ride.car.tires.iter().all(|t| !t.grounded) {
                    air += 1;
                } else {
                    if longest >= AIRBORNE && ride.car.tires.iter().all(|t| t.grounded) {
                        landed = true;
                    }
                    air = 0;
                }
                longest = longest.max(air);
                if landed {
                    break;
                }
            }
            eprintln!("{name}: {longest} ticks in the air off the lip, landed {landed}");
            assert!(longest >= AIRBORNE, "{name}: {longest} ticks in the air off the lip");
            assert!(landed, "{name}: never down on four tires after the jump");

            // Ridden to keep a put-back spot, then over the map's edge.
            let mut ride = load.ride(over(world, &load, SPOT), yaw_quat(0.0), F::ZERO);
            let mut hands = Hands::new();
            for _ in 0..KEEP {
                hands.step(&mut ride, world, true, 0x80);
                handed.extend(ride.rest.drain(..).map(|c| call_name(&c)));
            }
            let back = ride.back.unwrap_or_else(|| panic!("{name}: no put-back spot kept"));
            let spot = ride.spot;
            let edge = (100.0, area.depth.0 / 2.0);
            let (q, at) = (yaw_quat(0.0), over(world, &load, edge));
            ride.put(at, q);
            let (mut wrecked, mut back_at) = (None, None);
            for tick in 0..900 {
                hands.step(&mut ride, world, false, 0x80);
                handed.extend(ride.rest.drain(..).map(|c| call_name(&c)));
                if wrecked.is_none() {
                    assert!(tick < 2, "{name}: no map-edge wreck by tick {tick}");
                    if ride.car.state == WRECK_EDGE {
                        wrecked = Some(tick);
                    }
                } else if ride.car.state == RIDING {
                    back_at = Some((tick, ride.body.position));
                    break;
                }
            }
            let (put, p) = back_at.unwrap_or_else(|| panic!("{name}: never put back after the wreck at {wrecked:?}"));
            let from = (p.x() - back.at.x()).hypot(p.z() - back.at.z());
            eprintln!("{name}: wrecked at tick {wrecked:?}, put back at {put}, {from:.1} ft from the kept spot");
            assert!(from < 10.0, "{name}: put back {from} ft from the kept spot {:?}", back.at);
            assert!(ride.body.enabled, "{name}: the body is off after the put-back");
            assert!(spot.0.y() > back.at.y(), "{name}: the spot {:?} is not over the record's {:?}", spot.0, back.at);

            // And rides on.
            let p0 = ride.body.position;
            for tick in 0..120 {
                hands.step(&mut ride, world, true, 0x80);
                handed.extend(ride.rest.drain(..).map(|c| call_name(&c)));
                assert_eq!(ride.car.state, RIDING, "{name}: wrecked again at tick {tick} after the put-back");
            }
            let p1 = ride.body.position;
            let on = (p1.x() - p0.x()).hypot(p1.z() - p0.z());
            assert!(on > 10.0, "{name}: rode {on} ft on after the put-back");

            eprintln!("{name}: handed on {handed:?}");
            assert!(unexpected(&handed).is_empty(), "{name}: calls handed on unanswered: {:?}", unexpected(&handed));
        }
    });
}

// --- MD149b: the car's collision record ------------------------------------------------------------

/// Each car's MD39a image (`mxvatv-ps2/md39a/<name>.ee.bin`) and where its car is
/// (tests/mxvatv_car_captures.rs's), in [`CARS`] order.
const IMAGES: [(&str, u32); 5] = [("buggy", 0x0159_8e10), ("baja", 0x0159_8e10), ("sandrail", 0x01b1_58a0), ("golf", 0x01b0_dac0), ("truck", 0x0159_8e10)];

impl Files {
    /// The car's `<model>m.col`, the ray probe its record runs (MXVSATV.PSR's `Veh\\Col`, MD39's).
    fn riding(&self, model: &str) -> game_sim::track::Rays {
        let path = self.kit.ctx("mxvatv-ps2/md39/model").join(format!("{model}m.col"));
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        game_sim::track::Rays::parse(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }
    /// The car built from its files and rigged with its record, at `at` turned to `quat`.
    fn rigged(&self, machine: MxvatvMachine, phy: &str, model: &str, world: &mut Place<'_>, at: (f32, f32), quat: Vf) -> (atv_car::Ride, Load) {
        let load = self.load_car(machine, phy, model);
        let mut ride = load.ride(over(world, &load, at), quat, F::ZERO);
        ride.rig(&load, &self.riding(model));
        (ride, load)
    }
}

/// Each car's record made from its files ([`atv_car::Load::record`] on its `.phy` and `m.col`) is
/// the one its MD39a image holds, in every word the ride does not move: the record's flags and
/// tuning, its resolution's fields, and the riding rays bit for bit (the image's active part's). The
/// record's last sweep and contacts, and its resolution's speeds, are state; its inverse inertia is
/// the part reader's off the model's box (MD150's, within 1%).
#[test]
fn each_cars_record_from_its_files_is_its_images() {
    use game_sim::handling::record;
    let Some(files) = Files::load() else { return };
    let mut seen = 0;
    for ((name, machine, phy, model), (image, car)) in CARS.into_iter().zip(IMAGES) {
        let path = files.kit.ctx("mxvatv-ps2/md39a").join(format!("{image}.ee.bin"));
        if !path.exists() {
            skip(&format!("{} is not there: MD39a's {image} (C29 remade it)", path.display()));
            continue;
        }
        let pre = ps2_oracle::Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
        let at = pre.u32_at(car + 0xc8);
        let w = |a: u32| pre.u32_at(a);
        let (theirs, their_held) = (record::read(&w, at), record::held(&w, at).expect("the image's record is held"));
        let part = pre.u32_at(at + 0x200);
        let their_rays: Vec<u32> = (0..pre.u32_at(part + 0x178) * 8).map(|k| pre.u32_at(pre.u32_at(part + 0x180) + 4 * k)).collect();

        let load = files.load_car(machine, phy, model);
        let (ours, our_held, riding) = load.record(&files.riding(model));
        let still = |r: &record::Record| {
            let mut r = r.clone();
            (r.swept_from, r.swept_to, r.contacts) = (Vf::ZERO, Vf::ZERO, Vec::new());
            r
        };
        assert_eq!(still(&ours), still(&theirs), "{name}: the record");
        let collider = |h: &game_sim::atv_contact::Held| {
            let mut c = h.record.clone();
            (c.collider.cofg_world, c.collider.normal_speed, c.collider.relative, c.collider.inv_inertia) = (Vf::ZERO, F::ZERO, Vf::ZERO, Vf::ZERO);
            (c, h.mass, h.entries.len(), h.movers.len())
        };
        assert_eq!(collider(&our_held), collider(&their_held), "{name}: the record's resolution fields");
        let (ii, ij) = (our_held.record.collider.inv_inertia, their_held.record.collider.inv_inertia);
        let off = ii.0.iter().zip(ij.0).map(|(x, y)| (x - y).abs() / y.abs().max(1e-6)).fold(0.0f32, f32::max);
        // The box the part reader takes the inertia from is the model's, in a frame the image does not
        // keep: within 1% (tests/mxvatv_car_load.rs, `the_models_box_is_the_records_within_a_percent`).
        assert!(off < 1e-2, "{name}: the record's inverse inertia: ours {ii:?}, the image's {ij:?}");
        let ours_rays: Vec<u32> = riding.iter().flat_map(|s| s.iter().flat_map(|v| v.0.map(f32::to_bits))).collect();
        assert_eq!(ours_rays, their_rays, "{name}: the riding rays");
        eprintln!("{name}: the record from the files is the image's; {} riding rays, inverse inertia {off} off", riding.len());
        seen += 1;
    }
    eprintln!("MD149b: {seen} cars' records against their images");
}

/// The quaternion turning `+z` to `yaw` about `+y` and then rolling the car `roll` radians about its
/// own forward axis.
fn rolled(yaw: f32, roll: f32) -> Vf {
    let (sy, cy) = (yaw / 2.0).sin_cos();
    let (sr, cr) = (roll / 2.0).sin_cos();
    // (the yaw) · (the roll about z), Hamilton's product.
    Vf::new(-sy * sr, sy * cr, cy * sr, cy * cr)
}

/// How long a tipped car gets to wreck, and then to be put back.
const TIP_TICKS: usize = 150;
const BACK_TICKS: usize = 600;

/// Each car, rigged, ridden to keep a put-back spot and then rolled over on free09 and let go: its
/// record's heightfield rays hold it on the ground (its body never sinks under it), it wrecks tipped
/// ([`WRECK_TIPPED`]), is put back upright at the kept spot when its clock runs out, and rides on.
///
/// Rolled onto its roof, not its side: on an MX Unleashed world a tire's ground is the world's
/// straight-down query ([`game_sim::quad::world_ground`]), so the tires of a car on its side read
/// ground under them, the car counts as grounded on both sides and its suspension stands it back up
/// before the tipped clause can hold.
#[test]
fn each_car_rolled_over_wrecks_tipped_and_is_put_back() {
    let Some(files) = Files::load() else { return };
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return;
        }
    };
    in_free09(&mut disc, |world| {
        for (name, machine, phy, model) in CARS {
            let (mut ride, load) = files.rigged(machine, phy, model, world, SPOT, yaw_quat(0.0));
            let mut hands = Hands::new();
            let mut handed = std::collections::BTreeSet::new();
            for _ in 0..KEEP {
                hands.step(&mut ride, world, true, 0x80);
                handed.extend(ride.rest.drain(..).map(|c| call_name(&c)));
            }
            let back = ride.back.unwrap_or_else(|| panic!("{name}: no put-back spot kept"));
            assert_eq!(ride.car.state, RIDING, "{name}: wrecked riding to keep a spot");

            // Rolled onto its roof, a little off the ground, where it is.
            let p = ride.body.position;
            let lift = Vf::new(p.x(), over(world, &load, (p.x(), p.z())).y(), p.z(), 1.0);
            ride.put(lift, rolled(0.0, std::f32::consts::PI));
            let (mut tipped, mut touched, mut lowest) = (None, 0usize, f32::MAX);
            for tick in 0..TIP_TICKS {
                hands.step(&mut ride, world, false, 0x80);
                handed.extend(ride.rest.drain(..).map(|c| call_name(&c)));
                let p = ride.body.position;
                assert!(p.0.iter().all(|v| v.is_finite()), "{name}: tick {tick} at {p:?}");
                let (floor, _) = world.ground(Vf::new(p.x(), 1.0e4, p.z(), 1.0));
                lowest = lowest.min(p.y() - floor.y());
                let r = &ride.recorded.as_ref().unwrap().record;
                touched += usize::from(r.on_ground);
                if ride.car.state == WRECK_TIPPED {
                    tipped = Some(tick);
                    break;
                }
            }
            eprintln!("{name}: rolled over, the record on the ground {touched} ticks, lowest body {lowest:.2} ft over the ground, tipped wreck at {tipped:?}");
            assert!(touched > 0, "{name}: the record's heightfield rays never touched the ground");
            assert!(lowest > -0.5, "{name}: the body sank {lowest} ft under the ground rolled over");
            let tipped = tipped.unwrap_or_else(|| panic!("{name}: no tipped wreck in {TIP_TICKS} ticks rolled over"));
            assert!(ride.vehicle.tipped_wreck, "{name}: the tipped wreck's flag is not set");

            let mut put = None;
            for tick in 0..BACK_TICKS {
                hands.step(&mut ride, world, false, 0x80);
                handed.extend(ride.rest.drain(..).map(|c| call_name(&c)));
                if ride.car.state == RIDING {
                    put = Some((tick, ride.body.position));
                    break;
                }
            }
            let (put, p) = put.unwrap_or_else(|| panic!("{name}: never put back after the tipped wreck at {tipped}"));
            let from = (p.x() - back.at.x()).hypot(p.z() - back.at.z());
            // The node is placed by the next tick's post; the body is put back already.
            let q = ride.body.quat;
            let up = 1.0 - 2.0 * (q.x() * q.x() + q.z() * q.z());
            eprintln!("{name}: put back {put} ticks after the tipped wreck, {from:.1} ft from the kept spot, up {up:.3}");
            assert!(from < 10.0, "{name}: put back {from} ft from the kept spot {:?}", back.at);
            assert!(up > 0.9, "{name}: put back with its up row's y {up}");
            assert!(!ride.vehicle.tipped_wreck, "{name}: the tipped wreck's flag is still set");

            let p0 = ride.body.position;
            for tick in 0..120 {
                hands.step(&mut ride, world, true, 0x80);
                handed.extend(ride.rest.drain(..).map(|c| call_name(&c)));
                assert_eq!(ride.car.state, RIDING, "{name}: wrecked again at tick {tick} after the put-back");
            }
            let p1 = ride.body.position;
            let on = (p1.x() - p0.x()).hypot(p1.z() - p0.z());
            assert!(on > 10.0, "{name}: rode {on} ft on after the put-back");
            assert!(!handed.contains("GravityRecord"), "{name}: the rigged car's gravity call went unanswered");
            assert!(unexpected(&handed).is_empty(), "{name}: calls handed on unanswered: {:?}", unexpected(&handed));
        }
    });
}

/// How far the car runs up to a prop, ft, and how long it is driven at it.
const RUN_UP: f32 = 60.0;
const PROP_TICKS: usize = 240;
/// How long after the strike the car is looked at, and the speed it has to fall under, ft/s.
const HELD: usize = 30;
const STOPPED: f32 = 5.0;

/// The static prop nearest [`SPOT`] with a face a car's height tall and near upright, met head on by
/// a run up to it on the ground (tests/atv_quad/mxu.rs's search, then turned along the face's normal):
/// a level ray from the run's start at one and at four feet strikes this prop first, near the far end,
/// on a face whose level normal points back along the run. The prop's index, and the run's start and
/// heading.
fn prop_run(world: &mut Place<'_>) -> (usize, (f32, f32), f32) {
    let level = |world: &mut Place<'_>, a: Vf, b: Vf| {
        use game_sim::collide::{Bundle, Probe, Record};
        let mut bundle = Bundle { count: 1, centre: a, radius: F(RUN_UP + 40.0), ..Bundle::default() };
        bundle.probes[0] = Probe::between(a, b);
        let mut record = Record::new();
        world.solid(&bundle, &mut record);
        record.hit[0].then_some((record.fraction[0].0, record.owner[0], record.normal[0], record.point[0]))
    };
    let mut candidates: Vec<(usize, Vf, f32)> = world
        .props
        .props
        .iter()
        .enumerate()
        .filter(|(_, p)| p.moving.is_none() && (1.0..20.0).contains(&p.radius.0))
        .map(|(i, p)| {
            let (dx, dz) = (SPOT.0 - p.centre.x(), SPOT.1 - p.centre.z());
            (i, p.centre, (dx * dx + dz * dz).sqrt())
        })
        .collect();
    candidates.sort_by(|a, b| a.2.total_cmp(&b.2));
    candidates
        .iter()
        .take(400)
        .find_map(|&(i, c, d)| {
            let r = world.props.props[i].radius.0;
            let (ux, uz) = ((SPOT.0 - c.x()) / d.max(1.0), (SPOT.1 - c.z()) / d.max(1.0));
            let run = r + RUN_UP;
            let first = (c.x() + ux * run, c.z() + uz * run);
            let g = world.ground.at(F(first.0), F(first.1))?.y.0;
            let (_, owner, n, at) = level(world, Vf::new(first.0, g + 1.0, first.1, 1.0), Vf::new(c.x(), g + 1.0, c.z(), 1.0))?;
            let flat = n.x().hypot(n.z());
            if owner != i as i32 || flat < 0.95 {
                return None;
            }
            // Back along the face's level normal from where the ray struck it.
            let (nx, nz) = (n.x() / flat, n.z() / flat);
            let start = (at.x() + nx * RUN_UP, at.z() + nz * RUN_UP);
            let g = world.ground.at(F(start.0), F(start.1))?.y.0;
            let ok = [1.0f32, 4.0].iter().all(|&h| {
                let a = Vf::new(start.0, g + h, start.1, 1.0);
                let b = Vf::new(at.x() - nx * 10.0, g + h, at.z() - nz * 10.0, 1.0);
                level(world, a, b).is_some_and(|(f, owner, m, _)| owner == i as i32 && f * (RUN_UP + 10.0) > RUN_UP - 10.0 && (m.x() * nx + m.z() * nz) > 0.95 * m.x().hypot(m.z()).max(1e-6))
            });
            ok.then_some((i, start, (-nx).atan2(-nz)))
        })
        .expect("a tall upright prop face on free09's ground with a clear run up to it")
}

/// Each car, rigged, driven on the gas straight at a free09 prop: its record's world rays strike it
/// (the record on an object, the prop's owner number plus one struck), and the car stops on its face —
/// its body never gets past the face, and is all but still against it at the end.
#[test]
fn each_car_driven_into_a_prop_stops_on_it() {
    let Some(files) = Files::load() else { return };
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return;
        }
    };
    in_free09(&mut disc, |world| {
        let (prop, start, yaw) = prop_run(world);
        let (ux, uz) = (yaw.sin(), yaw.cos());
        let face = RUN_UP;
        eprintln!("MD149b: prop {prop} at {:?}, its face {RUN_UP} ft along a run from {start:?} yaw {yaw}", world.props.props[prop].centre);
        for (name, machine, phy, model) in CARS {
            let (mut ride, _) = files.rigged(machine, phy, model, world, start, yaw_quat(yaw));
            let mut hands = Hands::new();
            let mut handed = std::collections::BTreeSet::new();
            let (mut on_prop, mut struck, mut furthest, mut top) = (0usize, 0u32, 0.0f32, 0.0f32);
            // The first strike's tick, and the first tick after it slower than STOPPED.
            let (mut hit, mut stopped) = (None, None);
            for tick in 0..PROP_TICKS {
                hands.step(&mut ride, world, true, 0x80);
                handed.extend(ride.rest.drain(..).map(|c| call_name(&c)));
                let p = ride.body.position;
                assert!(p.0.iter().all(|v| v.is_finite()), "{name}: tick {tick} at {p:?}");
                let r = &ride.recorded.as_ref().unwrap().record;
                if r.on_object {
                    on_prop += 1;
                    if hit.is_none() {
                        (hit, struck) = (Some(tick), r.struck);
                    }
                }
                let v = ride.body.velocity;
                let speed = v.x().hypot(v.z());
                top = top.max(speed);
                // Held against it, on the gas, a car may climb the face and slide off round its
                // side (the golf cart does); only the meeting is looked at.
                if hit.is_some_and(|h| tick <= h + HELD) {
                    furthest = furthest.max((p.x() - start.0) * ux + (p.z() - start.1) * uz);
                    if speed < STOPPED && stopped.is_none() {
                        stopped = Some(tick);
                    }
                }
            }
            let hit = hit.unwrap_or_else(|| panic!("{name}: the record never touched the prop"));
            eprintln!("{name}: top {top:.1} ft/s, the record on the prop {on_prop} ticks (struck {struck} at tick {hit}, stopped at {stopped:?}), body furthest {furthest:.1} ft along the run in the {HELD} ticks after, the face at {face:.1}");
            assert!(top > 20.0, "{name}: only {top} ft/s at the prop");
            assert_eq!(struck, prop as u32 + 1, "{name}: struck {struck}, not the prop");
            assert!(furthest < face, "{name}: its body went {furthest} ft along the run, past the prop's face at {face}");
            assert!(stopped.is_some(), "{name}: not under {STOPPED} ft/s in the {HELD} ticks after it struck the prop");
            assert!(!handed.contains("GravityRecord"), "{name}: the rigged car's gravity call went unanswered");
            assert!(unexpected(&handed).is_empty(), "{name}: calls handed on unanswered: {:?}", unexpected(&handed));
        }
    });
}
