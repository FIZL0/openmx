//! MD157: MX vs. ATV's cars drawn, followed on their own cameras and heard. With `--mxvatv-car` and
//! openmx-mods' `mxvatv_cars` pack in this checkout's mods folder, the shell rides free09 headless on
//! a hand-written tape (switch to the car, the gas, then the stick hard left) and says under
//! `--trace-picture` what each drawn frame did: the car's chassis model off the pack at its scene
//! node, its tires posed from the step (`picture mxvatv car:`), by both renderers; and under
//! `--trace-camera` which preset follows it: its kind's own `Cam\FC_<kind>Default.pb`'s first, not
//! the bike's. Its driver (MD175) sits in it, posed on the plane pilot's clips, rooted in the seat
//! its model's four seat nodes make. Its engine's `carengine.aud` profile is read off the pack. Each run writes a shot,
//! looked at by eye. Picked and never switched to, nothing of it is drawn. Skips without a disc or
//! the pack.

#[allow(dead_code)]
#[path = "../src/vehicle.rs"]
mod vehicle;
#[allow(dead_code)]
#[path = "../src/chase.rs"]
mod chase;

use std::path::{Path, PathBuf};
use std::process::Command;

use game_sim::camera::{Aspect, Viewport};
use game_sim::car::MxvatvMachine;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn pack_dir() -> PathBuf {
    root().join("mods/machines/mxvatv_cars")
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md157-mxvatv-cars-drawn");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir.join(name)
}

/// The tick the tape switches to the car, and the tick it turns the stick hard left.
const IN: usize = 30;
const LEFT: usize = 110;

/// A tape of `frames` ticks: Cross (the car's gas) held throughout, the switch to the car at [`IN`],
/// the left stick hard left from [`LEFT`]; with no `switch` line, the bike all the way.
fn tape(name: &str, frames: usize, switch: bool) -> PathBuf {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape the car\n");
    for f in 0..frames {
        let lx = if f >= LEFT { "00" } else { "7f" };
        text += &format!("frame 0x3d08850a 0x0079ffbf7f7f{lx}7f000000000000ff{} -\n", "00".repeat(17));
    }
    if switch {
        text += &format!("switch {IN} mxvatv_car\n");
    }
    let path = scratch(name);
    std::fs::write(&path, text).expect("the tape is written");
    path
}

/// The shell on free09 with `car` picked, headless on `renderer`, for `frames`, writing a shot of the
/// last but one: what it said. `None` with no disc.
fn ride(car: &str, renderer: &str, frames: usize, switch: bool, shot: &Path) -> Option<String> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(root())
        .args(["--headless", "--track", "free09", "--mods", "mods", "--size", "640x448", "--overlay", "none", "--steady-clock"])
        .args(["--renderer", renderer, "--trace-picture", "--trace-camera", "--frames", &frames.to_string(), "--play"])
        .arg(tape(&format!("{car}-{renderer}-{switch}.tape"), frames + 5, switch))
        .arg("--shot")
        .arg(shot)
        .args(["--shot-frame", &(frames - 2).to_string()])
        .args(["--mxvatv-car", car])
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    Some(said)
}

/// The numbers after `key` in `line`.
fn after<const N: usize>(line: &str, key: &str) -> [f32; N] {
    let w: Vec<&str> = line.split_whitespace().collect();
    let i = w.iter().position(|x| *x == key).unwrap_or_else(|| panic!("no {key}: {line}"));
    std::array::from_fn(|j| w[i + 1 + j].parse().unwrap_or_else(|_| panic!("not a number after {key}: {line}")))
}

/// One word after `key` in `line`.
fn word<'a>(line: &'a str, key: &str) -> &'a str {
    let w: Vec<&str> = line.split_whitespace().collect();
    let i = w.iter().position(|x| *x == key).unwrap_or_else(|| panic!("no {key}: {line}"));
    w[i + 1]
}

/// A `picture mxvatv car:` line's four tires: spin, steer and compression each.
fn tires(line: &str) -> Vec<[f32; 3]> {
    let w: Vec<&str> = line.split_whitespace().collect();
    let i = w.iter().position(|x| *x == "tires").unwrap_or_else(|| panic!("no tires: {line}"));
    w[i + 1..i + 5]
        .iter()
        .map(|t| {
            let n: Vec<f32> = t.split('/').map(|v| v.parse().unwrap_or_else(|_| panic!("a tire: {line}"))).collect();
            [n[0], n[1], n[2]]
        })
        .collect()
}

/// The line starting `start`, or a panic with everything said.
fn line<'a>(said: &'a str, start: &str) -> &'a str {
    said.lines().find(|l| l.starts_with(start)).unwrap_or_else(|| panic!("no `{start}` line:\n{said}"))
}

/// The preset each `camera 0` line of `--trace-camera` follows on, in tick order.
fn presets(said: &str) -> Vec<String> {
    said.lines()
        .filter(|l| l.starts_with("camera 0 "))
        .map(|l| l.split('"').nth(1).unwrap_or_else(|| panic!("no preset: {l}")).to_string())
        .collect()
}

/// A png's width and height, read off its header.
fn png_size(path: &Path) -> (u32, u32) {
    let png = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(&png[1..4], b"PNG", "{}: not a png", path.display());
    let be = |at: usize| u32::from_be_bytes(png[at..at + 4].try_into().unwrap());
    (be(16), be(20))
}

#[test]
fn a_switched_in_car_draws_its_chassis_on_its_own_camera() {
    if !pack_dir().join("mod.json").is_file() {
        eprintln!("SKIPPED: no mods/machines/mxvatv_cars (openmx-mods' import-atv cars makes it)");
        return;
    }
    for machine in MxvatvMachine::ALL {
        let car = vehicle::mxvatv_car_profile(machine).to_ascii_lowercase();
        // The software path is slow: the GPU for every car, the software one for the buggy.
        let renderers: &[&str] = if machine == MxvatvMachine::Buggy { &["gpu", "software"] } else { &["gpu"] };
        for &renderer in renderers {
            let shot = scratch(&format!("{car}-{renderer}.png"));
            let Some(said) = ride(&car, renderer, 150, true, &shot) else { return };
            let what = format!("{car} {renderer}");
            assert!(said.lines().any(|l| l == "vehicle: mxvatv_car"), "{what}: never in the car:\n{said}");
            assert_eq!(png_size(&shot), (640, 448), "{}", shot.display());

            // Its chassis off the pack, every material textured.
            let drawn = line(&said, "mxvatv car: drawn as ");
            assert!(drawn.starts_with(&format!("mxvatv car: drawn as Veh/Anim/{car}.psg off mxvatv_cars (")), "{what}: {drawn}");
            let (textured, slots) = drawn.split_once(" materials textured").and_then(|(h, _)| h.rsplit_once(", ")).and_then(|(_, t)| t.split_once(" of ")).expect("the textured count");
            assert!(slots.parse::<usize>().unwrap() > 0 && textured == slots, "{what}: {drawn}");

            // Drawn every frame it is out, from the switch on and never before.
            let model = format!("mxvatv_car:mxvatv_cars:{car}");
            let picture = format!("picture mxvatv car: seat 0 {renderer} ");
            let at = said.lines().position(|l| l == "vehicle: mxvatv_car").unwrap();
            assert!(!said.lines().take(at).any(|l| l.starts_with("picture mxvatv car:")), "{what}: drawn before it was taken:\n{said}");
            let frames: Vec<&str> = said.lines().filter(|l| l.starts_with(&picture)).collect();
            assert!(frames.len() >= 100, "{what}: the car was hardly drawn ({} frames):\n{said}", frames.len());
            assert!(frames.iter().all(|l| word(l, renderer) == model), "{what}: another model: {}", frames[0]);
            assert!(frames.iter().all(|l| word(l, "triangles").parse::<usize>().unwrap() > 1000), "{what}: hardly anything of it: {}", frames[0]);

            // Posed from the step: the springs take its weight, the gas turns every tire, the stick
            // steers the fronts.
            let posed: Vec<Vec<[f32; 3]>> = frames.iter().map(|l| tires(l)).collect();
            assert!(posed.iter().any(|w| w.iter().all(|t| t[2] > 0.05)), "{what}: the springs never took the weight");
            for k in 0..4 {
                assert!(posed.iter().any(|w| (w[k][0] - posed[0][k][0]).abs() > 0.5), "{what}: tire {k} never turned");
            }
            let last = &posed[posed.len() - 1];
            assert!(last[0][1].abs() > 0.05 && last[1][1].abs() > 0.05, "{what}: the fronts are not steered: {}", frames[frames.len() - 1]);
            let (first, moved): ([f32; 3], [f32; 3]) = (after(frames[0], "at"), after(frames[frames.len() - 1], "at"));
            assert!((moved[0] - first[0]).hypot(moved[2] - first[2]) > 10.0, "{what}: the gas never moved it");

            // Its driver (MD175), every frame it is drawn, rooted in its seat: the middle of the
            // model's four seat nodes, as the golf cart or the others spell them, and the plane's
            // offset from them; the body's middle in the cab, by the seat.
            let psg = std::fs::read(pack_dir().join(format!("Veh/Anim/{car}.psg"))).expect("the chassis model");
            let model = game_render::psg::Model::parse(&psg).expect("the chassis parses");
            let nodes: Vec<game_render::V3> = game_render::car::SEAT
                .iter()
                .map(|n| game_render::car::node_at(&model, n).or_else(|| game_render::car::node_at(&model, n.strip_suffix("_pos").unwrap())).unwrap_or_else(|| panic!("{what}: no {n}")))
                .collect();
            let seat = nodes[0].add(nodes[1]).add(nodes[2]).add(nodes[3]).scale(0.25).add(game_render::car::SEAT_FROM_LIMBS);
            let pilot = format!("pilot: seat 0 {renderer} ");
            assert!(!said.lines().take(at).any(|l| l.starts_with(&pilot)), "{what}: a driver before the switch:\n{said}");
            let pilots: Vec<&str> = said.lines().filter(|l| l.starts_with(&pilot)).collect();
            assert_eq!(pilots.len(), frames.len(), "{what}: frames without a driver:\n{said}");
            for l in &pilots {
                let root: [f32; 3] = after(l, "root");
                let off = [root[0] - seat.x, root[1] - seat.y, root[2] - seat.z];
                assert!(off.iter().all(|d| d.abs() < 1e-2), "{what}: the driver's root is not in the seat {seat:?}: {l}");
                assert!(word(l, "of").parse::<usize>().unwrap() > 0, "{what}: nothing of the driver was drawn: {l}");
                let middle: [f32; 3] = after(l, "middle");
                let away = [middle[0] - seat.x, middle[1] - seat.y, middle[2] - seat.z];
                assert!(away.iter().map(|d| d * d).sum::<f32>().sqrt() < 3.0, "{what}: the driver is not in the cab, seat {seat:?}: {l}");
            }

            // Its own camera: its kind's FC_ file off the pack, the first of its presets from the
            // switch on, the bike's own before.
            let file = chase::mod_presets(chase::mxvatv_car_machine(machine));
            let pb = std::fs::read(pack_dir().join(file.replace('\\', "/"))).unwrap_or_else(|e| panic!("{what}: the pack's {file}: {e}"));
            let own = chase::flier_presets(&pb, Aspect::Standard, Viewport::Full);
            assert!(!own.is_empty(), "{what}: {file} holds no presets");
            assert_eq!(line(&said, "mxvatv_car: its camera starts on"), format!("mxvatv_car: its camera starts on {}, {} presets in {file}", own[0].name, own.len()));
            let followed = presets(&said);
            assert!(followed.len() >= 140, "{what}: {} camera lines", followed.len());
            let bike = &followed[IN - 5];
            assert_ne!(bike, &own[0].name, "{what}: the bike already follows on the car's preset");
            assert!(followed[IN + 5..].iter().all(|p| *p == own[0].name), "{what}: the car is not followed on {}: {:?}", own[0].name, &followed[IN..]);

            // Its engine: its own profile's keyed loops in the pack's carengine.aud.
            let profile = vehicle::mxvatv_car_profile(machine);
            let engine = line(&said, "mxvatv car: mxvatv_cars's engine, ");
            assert!(engine.starts_with(&format!("mxvatv car: mxvatv_cars's engine, [{profile}]'s ")) && engine.contains(" loops in Aud/carengine.aud"), "{what}: {engine}");
            let n: usize = word(engine, &format!("[{profile}]'s")).parse().unwrap();
            assert!(n > 0, "{what}: {engine}");
            assert!(!said.contains("engine: Aud"), "{what}: a loop not read:\n{said}");
        }
    }

    // Picked and never switched to: read, and never drawn.
    let shot = scratch("off.png");
    let Some(said) = ride("buggy", "gpu", 60, false, &shot) else { return };
    assert!(said.lines().any(|l| l.starts_with("mxvatv car: drawn as ")), "the car was not read:\n{said}");
    assert!(!said.lines().any(|l| l.starts_with("picture mxvatv car:")), "a car never taken was drawn:\n{said}");
    assert!(!said.lines().any(|l| l.starts_with("pilot: ")), "a car never taken has a driver:\n{said}");
}
