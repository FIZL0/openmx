//! MD149 — **MX vs. ATV's car on MX Unleashed's worlds**: [`atv_car::Ride`] built from each car's files
//! ([`atv_car::Load`], MD150's) and stepped headless on free09 (the split track the shell reads: its
//! terrain and the props of both archives) with a pad, as a shell steps it.
//!
//! The original has no car on an MX Unleashed track, so nothing here is compared with a capture: each
//! car is checked for riding. It stands, gets going on the gas, turns on the stick, leaves the ground
//! off a lip and lands, and wrecks at the map's edge, has its wreck clock run out and is put back at
//! the spot MX Unleashed's put-back record kept, riding on. Every call the ride hands on unanswered is
//! one [`atv_car::Ride`] drops on purpose.
use game_render::disc::Disc;
use game_sim::atv_car::{self, Call, Load, RIDING, WRECK_EDGE};
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
