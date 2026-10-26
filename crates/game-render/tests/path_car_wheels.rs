//! The path cars' wheels (V82) against the original: s89-along's save state of free06, its four dump
//! trucks and two scrapers (fronts and backs) driven by the original's own path car update in the
//! interpreter, beside [`toy::Wheels`] on the same cars.
//!
//! 1. **the wheels** — each car's wheel count, its one over the radius and its steer switch are the
//!    ones the original's init left in the car, out of the model's `Tire*` nodes and the `.toy`;
//! 2. **the spin** — every update the original's spin moves by minus the flat length of the step its
//!    path point made over the radius, wrapped into ±π;
//! 3. **the run** — over [`TICKS`] updates, the angle the port draws moves as the original's does;
//! 4. **the steer** — the front wheels' steer the port replays is the original's to 3·10⁻³ rad (the
//!    port's heading is its own, a few thousandths of a radian off on free06's bends);
//! 5. **the pose** — every wheel node's local matrix is [`toy::Wheels::local`] of the original's spin
//!    and steer at the node's file position.
//!
//! Needs the disc and the state, and skips with the reason when either is missing.

use game_render::disc::{Disc, Res};
use game_render::mat::{V3, v3};
use game_render::psg::Model;
use game_render::toy::{self, Path, Wheels};
use ps2_oracle::{Ee, Kit, addr, skip};

/// Two seconds of the scene's 30 updates a second.
const TICKS: usize = 60;
const DT: f32 = 1.0 / 30.0;
const BUDGET: u64 = 50_000_000;

/// The car's fields (`research/oracle.txt` S81, V82).
mod at {
    pub const NAME: u32 = 0x320;
    pub const PATH: u32 = 0x430;
    pub const POINT: u32 = 0x460;
    pub const STEERS: u32 = 0x4d8;
    pub const WHEELS: u32 = 0x4f0;
    pub const INV_RADIUS: u32 = 0x548;
    pub const SPIN: u32 = 0x550;
    pub const STEER: u32 = 0x554;
    pub const WHEEL_NODES: u32 = 0x570;
    pub const TIME_SCALE: u32 = 0x534;
    pub const LAG: u32 = 0x558;
    /// A node's local matrix.
    pub const LOCAL: u32 = 0x40;
}

fn disc() -> Option<Disc> {
    match Disc::open_configured() {
        Ok(d) => Some(d),
        Err(why) => {
            println!("SKIPPED: {why}");
            None
        }
    }
}

fn track(disc: &mut Disc, name: &str) -> Res {
    let want = format!("\\{name}.res");
    let member = disc.entries().iter().find(|e| e.name.to_ascii_lowercase().ends_with(&want)).expect("the track").name.clone();
    disc.res(&member).expect("the track's archive")
}

fn v(ee: &Ee, a: u32) -> V3 {
    v3(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8))
}

fn name(ee: &Ee, car: u32) -> String {
    let bytes: Vec<u8> = (0..64).map(|k| ee.u8_at(car + at::NAME + k)).take_while(|&b| b != 0).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// `a` into ±π, the original's way: less its whole turns, then one more either side.
fn wrap(a: f32) -> f32 {
    let tau = 6.283_185_5f32;
    let a = a - (a * 0.159_154_94) as i32 as f32 * tau;
    if a > std::f32::consts::PI {
        a - tau
    } else if a < -std::f32::consts::PI {
        a + tau
    } else {
        a
    }
}

/// The smallest turn from `a` to `b`.
fn turn(a: f32, b: f32) -> f32 {
    let d = (b - a).rem_euclid(std::f32::consts::TAU);
    if d > std::f32::consts::PI { d - std::f32::consts::TAU } else { d }
}

struct Seen {
    car: u32,
    name: String,
    toy: toy::Car,
    path: Path,
    model: Model,
    wheels: Wheels,
}

#[test]
fn the_wheels_turn_as_the_originals_do() {
    let Some(mut disc) = disc() else { return };
    let Ok(kit) = Kit::load().map_err(|w| skip(&w)) else { return };
    let state = kit.ctx("states").join("s89-along.eeMemory.bin");
    if !state.exists() {
        skip(&format!("{} is not there: `./kit state ram` of S89's s89-along save state", state.display()));
        return;
    }
    let mut ee = Ee::from_ram(&state).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let mut archives = vec![track(&mut disc, "free06")];
    if disc.entries().iter().any(|e| e.name.to_ascii_lowercase().ends_with("\\free06_2.res")) {
        archives.push(track(&mut disc, "free06_2"));
    }
    let toy_text = archives.iter().find_map(|a| a.read_suffix("free06.toy").ok()).expect("free06.toy");
    let toys = toy::cars(&game_render::ini::text(&toy_text));
    let vtable = addr("path_car_vtable").unwrap();
    let cars: Vec<u32> = (0..32u32 << 20).step_by(4).filter(|&a| ee.u32_at(a) == vtable).map(|a| a - 8).collect();
    assert_eq!(cars.len(), 21, "the state's path cars");

    // 1. The wheels, as the init left them.
    let mut seen = Vec::new();
    for &car in &cars {
        let name = name(&ee, car);
        let stem = name.trim_end_matches(".psg");
        let bytes = archives.iter().find_map(|a| a.read_suffix(&name).ok()).unwrap_or_else(|| panic!("no {name}"));
        let mut model = Model::parse(&bytes).expect("the model");
        // The shell draws a path car without its file's root (V46).
        for node in model.nodes.iter_mut().filter(|n| n.parent.is_none()) {
            node.local = game_render::mat::M4::IDENTITY;
        }
        let (scale, lag) = (ee.u32_at(car + at::TIME_SCALE), ee.u32_at(car + at::LAG));
        let toy = toys
            .iter()
            .find(|c| c.stem().eq_ignore_ascii_case(stem) && c.time_scale.to_bits() == scale && c.lag.to_bits() == lag)
            .unwrap_or_else(|| panic!("{name} ({car:#x}) is not in free06.toy"))
            .clone();
        let path = ee.u32_at(car + at::PATH);
        let (n, keys) = (ee.u32_at(path), ee.u32_at(path + 8));
        let points = (0..n).map(|k| keys + k * 0x30).map(|k| (ee.f32_at(k), v(&ee, k + 0x10))).collect();
        let path = Path::new(points).expect("a path");
        let count = ee.u32_at(car + at::WHEELS) as usize;
        let Some(wheels) = Wheels::new(&model, &toy, &path) else {
            assert_eq!(count, 0, "{name} ({car:#x}): the original turns {count} wheels the port finds none of");
            continue;
        };
        assert_eq!(wheels.nodes.len(), count, "{name} ({car:#x}): wheels");
        assert_eq!(wheels.inv_radius.to_bits(), ee.u32_at(car + at::INV_RADIUS), "{name} ({car:#x}): one over the radius");
        assert_eq!(wheels.steer, ee.u32_at(car + at::STEERS) != 0, "{name} ({car:#x}): SteerFrontTires");
        seen.push(Seen { car, name, toy, path, model, wheels });
    }
    assert_eq!(seen.len(), 8, "free06's wheeled cars: four dump trucks, two scrapers' fronts and backs");

    // 2. Every update's spin, and the clocks the drive went through.
    let update = addr("path_car_update").unwrap();
    let clock_of = |ee: &Ee, car: u32| ee.f32_at(ee.u32_at(car + at::PATH) + 0xc);
    let start: Vec<(f32, f32)> = seen.iter().map(|s| (clock_of(&ee, s.car), ee.f32_at(s.car + at::SPIN))).collect();
    let mut turned = vec![0.0f32; seen.len()];
    for tick in 0..TICKS {
        let before: Vec<(V3, f32)> = seen.iter().map(|s| (v(&ee, s.car + at::POINT), ee.f32_at(s.car + at::SPIN))).collect();
        for &car in &cars {
            ee.call(update, &[u64::from(car)], &[DT], BUDGET);
        }
        for (k, s) in seen.iter().enumerate() {
            let (was, spin) = before[k];
            let step = v(&ee, s.car + at::POINT).sub(was);
            let flat = (step.z * step.z + step.x * step.x).sqrt();
            let want = wrap(spin - s.wheels.inv_radius * flat);
            let got = ee.f32_at(s.car + at::SPIN);
            assert!((turn(want, got)).abs() < 1e-5, "tick {tick}: {} ({:#x}) spin {got} against {want}", s.name, s.car);
            turned[k] += turn(spin, got);
        }
    }

    // 3. The run, 4. the steer and 5. the pose.
    let mut rolled = 0;
    let mut steered = 0;
    for (k, s) in seen.iter().enumerate() {
        let ts = s.toy.time_scale;
        let (c0, spin0) = start[k];
        let c1 = clock_of(&ee, s.car);
        // The scene's clock that puts the port's car at the state's clock, and two seconds on.
        let e0 = f64::from(c0) / f64::from(ts);
        let e1 = e0 + TICKS as f64 * f64::from(DT);
        let ours = s.wheels.spin_at(&s.toy, &s.path, e1) - s.wheels.spin_at(&s.toy, &s.path, e0);
        let ours = turn(0.0, ours);
        let theirs = turn(0.0, turned[k]);
        assert!(
            (ours - theirs).abs() < 0.01 + theirs.abs() * 0.005,
            "{} ({:#x}): two seconds turn the port's wheels {ours} and the original's {theirs} (spin {spin0} on)",
            s.name,
            s.car
        );
        rolled += usize::from(turned[k].abs() > 1.0);
        let steer = ee.f32_at(s.car + at::STEER);
        let our_steer = s.wheels.steer_at(&s.toy, &s.path, c1, DT * ts);
        println!("{} ({:#x}): steer {our_steer}, the original's {steer}", s.name, s.car);
        assert!((our_steer - steer).abs() < 3e-3, "{} ({:#x}): steer {our_steer} against the original's {steer}", s.name, s.car);
        steered += usize::from(steer.abs() > 1e-3);
        let spin = ee.f32_at(s.car + at::SPIN);
        let nodes = ee.u32_at(s.car + at::WHEEL_NODES);
        for (w, &i) in s.wheels.nodes.iter().enumerate() {
            let node = ee.u32_at(nodes + 4 * w as u32);
            let t = s.model.nodes[i].local.0[3];
            let ours = Wheels::local(v3(t[0], t[1], t[2]), spin, if s.wheels.steer && w < 2 { steer } else { 0.0 });
            for r in 0..4 {
                for c in 0..3 {
                    let theirs = ee.f32_at(node + at::LOCAL + 16 * r as u32 + 4 * c as u32);
                    assert!(
                        (ours.0[r][c] - theirs).abs() < 3e-3,
                        "{} ({:#x}) wheel {w} row {r} col {c}: {} against the original's {theirs}",
                        s.name,
                        s.car,
                        ours.0[r][c]
                    );
                }
            }
        }
    }
    assert!(rolled >= 4, "only {rolled} of the cars rolled a radian in two seconds");
    assert!(steered >= 1, "no car's front wheels were turned");
}
