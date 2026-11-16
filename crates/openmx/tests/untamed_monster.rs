//! MD161b: MX vs. ATV Untamed's monster truck in the shell, an addition off by default. openmx-mods'
//! `untamed_cars` machine pack (import-atv untamed-cars) lists the monster truck beside the baja, the
//! golf cart and the sandrail (SLUS-21701's `res\veh\VehicleType_5.res` whole) with `"step": "untamed"`;
//! `--mxvatv-car monstertruck@untamed_cars` offers it at the pause's Switch To Machine, built off the
//! pack's `Veh/monstertruck.phy.bpb` and chassis model alone and ridden on Untamed's car step
//! ([`atv_car::Load::untamed`], MD161). A bare `monstertruck` stays MX vs. ATV Unleashed's (`mxvatv_cars`).
//!
//! Untamed's monstertruck.phy turns `[Preload]` off (MaxPreload 2500 and MaxPreloadTime 60 down to 0),
//! and weighs 3500 against 2600. The shell's truck, switched to and driven on the gas, must say the
//! body words the same car made here off the same pack files with `Load::untamed`, stepped on the same
//! pad over the same free09, comes to bit for bit (untamed_cars.rs's test of the baja). MD160's drives
//! of the original's truck replayed through the step wait on MD161a's harness.
//!
//! This rides the binary headless on hand-written tapes. It skips with no disc, and with no
//! `untamed_cars` pack in this checkout's `mods/machines/`.

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

fn packs() -> PathBuf {
    root().join("mods/machines")
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

/// Whether this checkout has pack `id`; says SKIPPED when not.
fn pack(id: &str) -> bool {
    let there = packs().join(id).join("mod.json").is_file();
    if !there {
        eprintln!("SKIPPED: no mods/machines/{id} (openmx-mods' import-atv makes it)");
    }
    there
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md161b-untamed-monster");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir.join(name)
}

/// Cross's pressure byte on the tape: a full press.
const FULL: u8 = 0xff;

/// The tapes' one report: Cross (the gas) held at `pressure`, the sticks centred.
fn gas_report(pressure: u8) -> String {
    format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, pressure, "00".repeat(17))
}

/// `frames` ticks of [`gas_report`] at `pressure`, with `switches` (`30 mxvatv_car`) after them.
fn tape(name: &str, frames: usize, pressure: u8, switches: &[&str]) -> PathBuf {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape Cross held at {pressure:#04x}\n");
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x{} -\n", gas_report(pressure));
    }
    for s in switches {
        text += &format!("switch {s}\n");
    }
    let path = scratch(name);
    std::fs::write(&path, text).expect("the tape is written");
    path
}

fn vehicles(said: &str) -> Vec<&str> {
    said.lines().filter_map(|l| l.strip_prefix("vehicle: ")).collect()
}

/// The `n` hex words after `key` in `line` (an `along` among them skipped).
fn words(line: &str, key: &str, n: usize) -> Vec<u32> {
    let at = line.find(key).unwrap_or_else(|| panic!("no `{key}` in `{line}`")) + key.len();
    line[at..].split([' ', ',']).filter(|w| !w.is_empty() && *w != "along").take(n).map(|w| u32::from_str_radix(w, 16).expect("a hex word")).collect()
}

const LABEL: &str = "mxvatv car: Monster Truck ";

/// The car line the shell says as the truck is left: its body words, how far it went, and where it
/// was taken.
fn car_line(said: &str) -> (Vec<u32>, f32, Vec<u32>, String) {
    let line = said.lines().find(|l| l.starts_with(LABEL) && l.contains(" body ")).unwrap_or_else(|| panic!("no car line:\n{said}"));
    let gone: f32 = line[LABEL.len()..].split_whitespace().next().and_then(|w| w.parse().ok()).unwrap_or_else(|| panic!("no distance in `{line}`"));
    (words(line, " body ", 6), gone, words(line, " taken at ", 5), line.to_string())
}

const IN: usize = 30;
const OUT: usize = 280;

/// The split free09 as the shell reads it (mxvatv_cars.rs's).
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

/// The pack's monster truck off its BPB `.phy` and chassis model, at the origin facing `+z`, on Untamed's
/// step.
fn load(curve: &CurveTable) -> Load {
    let dir = packs().join("untamed_cars");
    let ini = game_sim::bpb::ini(&std::fs::read(dir.join("Veh/monstertruck.phy.bpb")).unwrap()).unwrap();
    let model = quad::Model::parse(&std::fs::read(dir.join("Veh/Anim/monstertruck.psg")).unwrap()).unwrap();
    let origin = quad::spawn_frame(Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 1.0));
    let points = atv_car::tire_points(&model, &origin).unwrap();
    let mut load = Load::read(&ini, MxvatvMachine::MonsterTruck, model.half_box(&origin), points, curve);
    load.untamed = true;
    load
}

/// **The Untamed monster truck drives in the shell on Untamed's step**, bit for bit the
/// Ride's made here off the pack with `Load::untamed`.
#[test]
fn the_untamed_monster_truck_drives_in_the_shell_on_untameds_step() {
    if !pack("untamed_cars") {
        return;
    }
    let switched = tape("on.tape", 300, FULL, &["30 mxvatv_car", "280 bike"]);
    let Some(said) = ride(320, &switched, &["--mxvatv-car", "monstertruck@untamed_cars"]) else { return };
    assert!(said.contains("mxvatv car: Monster Truck (monstertruck) off untamed_cars, on MX vs. ATV Untamed's step (switch from the pause)\n"), "\n{said}");
    assert_eq!(vehicles(&said), ["bike", "mxvatv_car", "bike"], "\n{said}");
    let (body, gone, taken, line) = car_line(&said);
    assert!(gone > 100.0, "the monster truck went only {gone} ft on the gas: {line}");

    let mut disc = Disc::open_configured().expect("the shell rode the disc");
    let curve = CurveTable::build();
    let hex = gas_report(FULL);
    let mut bytes = [0u8; REPORT_BYTES];
    for (k, b) in bytes.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[2 * k..2 * k + 2], 16).unwrap();
    }
    let report = Report::from_bytes(bytes);
    let dt = F::from_bits(0x3d08_850a);
    let load = load(&curve);
    assert!(load.untamed && load.keys().untamed, "Load::untamed reaches the keys");
    let k = load.keys();
    assert_eq!([k.max_preload.0, k.max_preload_time.0], [0.0, 0.0], "Untamed's monstertruck.phy: [Preload] off");
    in_free09(&mut disc, |world| {
        let car = load.car();
        let lift = car.tires.iter().map(|t| t.radius.0 - t.mount.y()).fold(0.0f32, f32::max) + 1.0;
        let f = |i: usize| f32::from_bits(taken[i]);
        let yaw = f(3).atan2(f(4));
        let quat = Vf::new(0.0, (yaw / 2.0).sin(), 0.0, (yaw / 2.0).cos());
        let mut ride = load.ride(Vf::new(f(0), f(1) + lift, f(2), 1.0), quat, F::ZERO);
        // The shell rigs every car it makes on its pack's collision record (MD149b).
        let col = std::fs::read(packs().join("untamed_cars/Veh/Col/MonsterTruckM.col")).unwrap();
        ride.rig(&load, &game_sim::track::Rays::parse(&col).unwrap());
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
        let n = met.unwrap_or_else(|| panic!("the Ride stepped here never came to the shell's body words: {line}"));
        assert_eq!(n, OUT - IN, "met the shell's words after {n} steps, not one a tick from the switch in to the switch out");
    });
}
