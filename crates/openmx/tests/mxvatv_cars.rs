//! MD39c: MX vs. ATV's five cars in the shell, an addition off by default. With `--mxvatv-car NAME`
//! (settings.txt's `mxvatv_car` under the front end) and openmx-mods' `mxvatv_cars` machine pack in
//! the mods folder, the pause's Switch To Machine offers that car, built off the pack's `.phy` and
//! chassis model alone (MD150's `atv_car::Load`) and ridden on SLUS-21104's step on MX Unleashed's
//! world (MD149's `atv_car::Ride`). Without it the ride is MX Unleashed's: no pack is read, a switch
//! to the car is refused, and MX Unleashed's own cars are left as they were.
//!
//! The shell's car is held to the type MD135's replays prove the step on: each of the five, switched
//! to and driven on the gas, says its body's position and velocity words as it is left, and the same
//! car made here off the same pack files at the spot the shell took it, stepped on the same pad over
//! the same free09, must come to those words bit for bit. (MD39b's drives through the Ride itself
//! are MD149a's, still open.)
//!
//! This rides the binary headless on hand-written tapes, as mxvatv_atv.rs does. It skips with no
//! disc, and the car half with no pack in this checkout's `mods/machines/`.

use game_render::disc::Disc;
use game_sim::atv_car::{self, Load};
use game_sim::bindings::Table;
use game_sim::camera::CurveTable;
use game_sim::car::MxvatvMachine;
use game_sim::input::{Manager, REPORT_BYTES, Report};
use game_sim::quad;
use game_sim::track::{Scene, Terrain as TerrainFile};
use game_sim::world::{Ground, Place, Props};
use ps2_float::{F, Vf};
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The pack's folder in this checkout.
fn pack_dir() -> PathBuf {
    root().join("mods/machines/mxvatv_cars")
}

/// Ride free09 headless for `frames` ticks on the steady clock playing `tape`, with `more` flags
/// and this checkout's mods folder, or `None` with no disc to ride.
fn ride(frames: u32, tape: &Path, more: &[&str]) -> Option<String> {
    if let Err(e) = Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(root())
        .args(["--headless", "--track", "free09", "--frames", &frames.to_string(), "--steady-clock", "--mods", "mods"])
        .args(["--play", tape.to_str().expect("a path")])
        .args(more)
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    Some(said)
}

/// Whether this checkout has the car pack; says SKIPPED when not.
fn pack() -> bool {
    let there = pack_dir().join("mod.json").is_file();
    if !there {
        eprintln!("SKIPPED: no mods/machines/mxvatv_cars (openmx-mods' import-atv cars makes it)");
    }
    there
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md39c-mxvatv-cars");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir.join(name)
}

/// The tapes' one report: Cross (the gas) held at full pressure, the sticks centred.
fn gas_report() -> String {
    format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17))
}

/// `frames` ticks of [`gas_report`], with `switches` (`30 mxvatv_car`) after them.
fn tape(name: &str, frames: usize, switches: &[&str]) -> PathBuf {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape Cross held\n");
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x{} -\n", gas_report());
    }
    for s in switches {
        text += &format!("switch {s}\n");
    }
    let path = scratch(name);
    std::fs::write(&path, text).expect("the tape is written");
    path
}

/// The machines the ride's `vehicle:` lines say it was on, in order.
fn vehicles(said: &str) -> Vec<&str> {
    said.lines().filter_map(|l| l.strip_prefix("vehicle: ")).collect()
}

/// The `ride hash` line the shell says at the ride's end.
fn ride_hash(said: &str) -> &str {
    said.lines().find(|l| l.starts_with("ride hash ")).unwrap_or_else(|| panic!("no ride hash:\n{said}"))
}

/// The tick the switches are at, and the tick the car is left.
const IN: usize = 30;
const OUT: usize = 280;

#[test]
fn off_by_default_there_is_no_mxvatv_car() {
    let switched = tape("off.tape", 300, &["30 mxvatv_car", "280 bike"]);
    let Some(said) = ride(320, &switched, &[]) else { return };
    assert!(!said.lines().any(|l| l.starts_with("mxvatv car:")), "a ride without a car picked read the car pack:\n{said}");
    assert_eq!(vehicles(&said), ["bike", "this ride has no MX vs. ATV car", "bike"], "\n{said}");
    // MX Unleashed's own buggy is still MX Unleashed's.
    let buggy = tape("off-buggy.tape", 120, &["30 buggy"]);
    let Some(mxu) = ride(120, &buggy, &[]) else { return };
    assert_eq!(vehicles(&mxu), ["bike", "buggy"], "\n{mxu}");
}

/// The split free09 as the shell reads it (game-sim's tests/mxvatv_car_mxu.rs's).
fn in_free09<R>(disc: &mut Disc, then: impl FnOnce(&mut Place<'_>) -> R) -> R {
    let parts = ["Tracks\\free09_ss.res", "Tracks\\free09_ss_2.res"].map(|n| disc.res(n).unwrap_or_else(|e| panic!("{n}: {e}")));
    let read = |want: &str| parts.iter().find_map(|r| r.read_suffix(want).ok()).ok_or_else(|| format!("no archive of this track has {want}"));
    let scene = Scene::parse(&read(".scn").unwrap()).expect("parse the scene");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let bytes = read(leaf.rsplit('\\').next().unwrap()).unwrap();
    let terrain = TerrainFile::parse(&bytes).expect("parse the terrain");
    let ground = Ground::new(&terrain, &scene.terrain);
    let (props, _) = Props::load(&scene, |w: &str| read(w));
    then(&mut Place { ground, props })
}

/// One of the five off the pack's files, at the origin facing `+z`, as the shell builds it.
fn load(machine: MxvatvMachine, name: &str, curve: &CurveTable) -> Load {
    let ini = game_sim::phy::Ini::parse_bytes(&std::fs::read(pack_dir().join(format!("Veh/{name}.phy"))).unwrap());
    let model = quad::Model::parse(&std::fs::read(pack_dir().join(format!("Veh/Anim/{name}.psg"))).unwrap()).unwrap();
    let origin = quad::spawn_frame(Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 1.0));
    let points = atv_car::tire_points(&model, &origin).unwrap();
    Load::read(&ini, machine, model.half_box(&origin), points, curve)
}

/// The `n` hex words after `key` in `line` (an `along` among them skipped).
fn words(line: &str, key: &str, n: usize) -> Vec<u32> {
    let at = line.find(key).unwrap_or_else(|| panic!("no `{key}` in `{line}`")) + key.len();
    line[at..].split([' ', ',']).filter(|w| !w.is_empty() && *w != "along").take(n).map(|w| u32::from_str_radix(w, 16).expect("a hex word")).collect()
}

#[test]
fn each_mxvatv_car_drives_in_the_shell_on_the_step_it_is_proved_on() {
    if !pack() {
        return;
    }
    let cars = [
        ("buggy", MxvatvMachine::Buggy, "Buggy"),
        ("monstertruck", MxvatvMachine::MonsterTruck, "Monster Truck"),
        ("baja", MxvatvMachine::Baja, "Baja"),
        ("golfcart", MxvatvMachine::GolfCart, "Golf Cart"),
        ("sandrail", MxvatvMachine::Sandrail, "Sandrail"),
    ];
    let switched = tape("on.tape", 300, &["30 mxvatv_car", "280 bike"]);
    let mut shell = Vec::new();
    for (name, _, label) in cars {
        let Some(said) = ride(320, &switched, &["--mxvatv-car", name]) else { return };
        assert!(said.contains(&format!("mxvatv car: {label} ({name}) off mxvatv_cars (switch from the pause)\n")), "\n{said}");
        assert_eq!(vehicles(&said), ["bike", "mxvatv_car", "bike"], "{name}\n{said}");
        let line = said.lines().find(|l| l.starts_with(&format!("mxvatv car: {label} ")) && l.contains(" body ")).unwrap_or_else(|| panic!("no car line:\n{said}"));
        let gone: f32 = line[format!("mxvatv car: {label} ").len()..].split_whitespace().next().and_then(|w| w.parse().ok()).unwrap_or_else(|| panic!("no distance in `{line}`"));
        assert!(gone > 100.0, "the {name} went only {gone} ft on the gas: {line}");
        shell.push((words(line, " body ", 6), words(line, " taken at ", 5), line.to_string()));
    }

    // The same cars made here at the spots the shell took them, on the same pad, over the same world.
    let mut disc = Disc::open_configured().expect("the shell rode the disc");
    let curve = CurveTable::build();
    let hex = gas_report();
    let mut bytes = [0u8; REPORT_BYTES];
    for (k, b) in bytes.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[2 * k..2 * k + 2], 16).unwrap();
    }
    let report = Report::from_bytes(bytes);
    let dt = F::from_bits(0x3d08_850a);
    in_free09(&mut disc, |world| {
        for ((name, machine, _), (body, taken, line)) in cars.into_iter().zip(&shell) {
            let load = load(machine, name, &curve);
            let car = load.car();
            let lift = car.tires.iter().map(|t| t.radius.0 - t.mount.y()).fold(0.0f32, f32::max) + 1.0;
            let f = |i: usize| f32::from_bits(taken[i]);
            let yaw = f(3).atan2(f(4));
            let quat = Vf::new(0.0, (yaw / 2.0).sin(), 0.0, (yaw / 2.0).cos());
            let mut ride = load.ride(Vf::new(f(0), f(1) + lift, f(2), 1.0), quat, F::ZERO);
            let (mut input, table) = (Manager::connected(), Table::SHIPPED);
            for _ in 0..IN {
                input.tick(dt, &[report, Report::idle()]);
            }
            let mut met = None;
            for n in 1..=OUT - IN + 10 {
                input.tick(dt, &[report, Report::idle()]);
                ride.step(input.pad(0), &table, dt, world);
                ride.rest.clear();
                let (p, v) = (ride.body.position, ride.body.velocity);
                let got = [p.x(), p.y(), p.z(), v.x(), v.y(), v.z()].map(f32::to_bits);
                if got[..] == body[..] {
                    met = Some(n);
                    break;
                }
            }
            let n = met.unwrap_or_else(|| panic!("{name}: the Ride stepped here never came to the shell's body words: {line}"));
            assert_eq!(n, OUT - IN, "{name}: met the shell's words after {n} steps, not one a tick from the switch in to the switch out");
            eprintln!("{name}: the shell's car is atv_car::Ride's after {n} steps");
        }
    });

    // Picked and never switched to, the bike's ride is the one with none picked.
    let bike = tape("bike.tape", 200, &[]);
    let Some(none) = ride(220, &bike, &[]) else { return };
    let Some(picked) = ride(220, &bike, &["--mxvatv-car", "buggy"]) else { return };
    assert_eq!(ride_hash(&none), ride_hash(&picked));
}
