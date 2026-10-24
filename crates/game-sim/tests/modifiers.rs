//! P97 — the bike's modifiers on the bike a player rides: the disc's `race125p.phy`, flat out from a
//! standstill on an endless flat floor, at the 30 Hz step a level runs at.
//!
//! What they hold:
//! - **Bike top speed** at 200% tops out at least 1.8 times the stock bike's, and gets there no slower;
//! - **Engine power** at 200% is clearly quicker from rest to 40 ft/s, and to 100;
//! - every other bike knob moves the measure it is named for (grip the launch, suspension the sag,
//!   jump the preload, crash tolerance the wreck limits, air control and flip spin their torques);
//! - none of them is undone by a wreck's put-back, which sets the ride's speed limit back.
//! - **Biplane turn speed** (P108) at 200% pulls the disc's biplane round a full-stick turn off its
//!   top speed near twice as fast, and leaves the autopilot's banked hold as the file flies it.
//!
//! Skipped without a disc (`./kit extract MX.PSR`): the stand-in bike does not top out the way the
//! disc's does, and the stock bike's launch on it is not grip-limited the way the real one's is.

use game_sim::archive::{Psr, Res};
use game_sim::modifiers::{Knob, Mods};
use game_sim::phy::Phy;
use game_sim::ride::Controls;
use game_sim::rider::{Rider, Tables};
use game_sim::trick::Set;
use game_sim::collide::{Bundle, Record};
use game_sim::world::{Flat, World};
use ps2_float::{F, Vf};
use ps2_oracle::{Kit, skip};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

/// The 30 Hz clamp a level runs at, which every capture of the original shows.
const DT: F = F(game_sim::tick::DeltaTime::CAP_30);

/// The disc's `race125p.phy`, as the shell loads it, and the `125_faisst` model's wheel places (the
/// shell's mounts), or `None` and a note.
fn the_125(tables: &Tables) -> Option<(Phy, (Vf, Vf))> {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/iso/MX.PSR");
    let path = if here.exists() {
        here
    } else {
        match Kit::load() {
            Ok(k) => k.ctx("iso").join("MX.PSR"),
            Err(why) => {
                skip(&why);
                return None;
            }
        }
    };
    let Ok(mut file) = File::open(&path) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", path.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    file.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let mut member = |name: &str| -> Option<Vec<u8>> {
        let e = psr.find(name)?;
        file.seek(SeekFrom::Start(e.at)).ok()?;
        let mut bytes = vec![0u8; e.size as usize];
        file.read_exact(&mut bytes).ok()?;
        Some(bytes)
    };
    let leaf = |res: &Res, want: &str| {
        res.entries
            .iter()
            .find(|e| {
                let n = e.name.to_ascii_lowercase();
                n == want || n.ends_with(&format!("\\{want}"))
            })
            .cloned()
    };
    let archives: Vec<String> = psr
        .entries
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.starts_with("bike\\") && n.ends_with(".res")
        })
        .collect();
    let (mut phy, mut wheels) = (None, None);
    for archive in &archives {
        let Some(bytes) = member(archive) else { continue };
        let Ok(res) = Res::parse(&bytes) else { continue };
        if phy.is_none()
            && let Some(e) = leaf(&res, "race125p.phy")
        {
            phy = res.read(&bytes, &e).ok().map(|b| Phy::read_bytes(&b, &tables.ride.curve));
        }
        if wheels.is_none() && archive.to_ascii_lowercase().contains("125_faisst") {
            let Some(e) = leaf(&res, "bike.psg") else { continue };
            let Ok(psg) = res.read(&bytes, &e) else { continue };
            let Ok(model) = game_render::psg::Model::parse(&psg) else { continue };
            let node = |name: &str| -> Option<Vf> {
                let i = model.nodes.iter().position(|n| n.name == name)?;
                let mut root = i;
                while let Some(up) = model.nodes[root].parent {
                    root = up;
                }
                Some(Vf(model.place_in(i, root)))
            };
            wheels = node("TireFront").zip(node("TireRear"));
        }
    }
    match phy.zip(wheels) {
        Some(found) => Some(found),
        None => {
            skip("no race125p.phy or 125_faisst wheels in MX.PSR");
            None
        }
    }
}

/// A set with `knob` at `pct` for every player.
fn with(knob: Knob, pct: u16) -> Mods {
    let mut mods = Mods::original();
    for seat in 0..game_sim::MAX_PLAYERS {
        mods.set(seat, knob, pct);
    }
    mods
}

/// What the rider saw on one tick.
#[derive(Clone, Copy, Debug)]
struct Tick {
    /// Along the ground, ft/s.
    speed: f32,
    /// The chassis' height and its vertical speed.
    y: f32,
    vy: f32,
    on_ground: bool,
    wreck: bool,
    flip: bool,
    /// How fast the body turns, rad/s: the length of its angular velocity.
    spin: f32,
    /// The nose above the level, degrees.
    pitch: f32,
}

/// `ticks` frames of the disc's 125 on `mods`, loaded as the shell loads it, from `at` facing +z on
/// `world`, its hands `script`'s from each entry's tick on (the first entry's from the start).
fn ride(p: &Phy, wheels: (Vf, Vf), mods: &Mods, world: &mut dyn World, at: Vf,
        script: &[(usize, Controls)], ticks: usize) -> Vec<Tick> {
    ride_at(DT, p, wheels, mods, world, at, script, ticks)
}

/// [`ride`] at a frame of `dt`.
#[allow(clippy::too_many_arguments)]
fn ride_at(dt: F, p: &Phy, wheels: (Vf, Vf), mods: &Mods, world: &mut dyn World, at: Vf,
           script: &[(usize, Controls)], ticks: usize) -> Vec<Tick> {
    let tables = Tables::build();
    let mut phy = p.clone();
    mods.bike(0, &mut phy);
    let mut rider = Rider::new(&phy, wheels, at, Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    mods.ride(0, &mut rider.ride);
    let mut now = script[0].1.clone();
    (0..ticks)
        .map(|i| {
            if let Some((_, c)) = script.iter().find(|(t, _)| *t == i) {
                now = c.clone();
            }
            let f = rider.frame(&phy, &tables, &now, dt, world);
            let r = &rider.ride;
            let (v, w) = (r.chassis.vel, r.chassis.ang_vel);
            Tick {
                speed: v.x().hypot(v.z()),
                y: r.position().y(),
                vy: v.y(),
                on_ground: r.air.on_ground,
                wreck: f.wreck.is_some(),
                flip: r.air.flip_active,
                spin: (w.x() * w.x() + w.y() * w.y() + w.z() * w.z()).sqrt(),
                pitch: r.transform().0[2][1].clamp(-1.0, 1.0).asin().to_degrees(),
            }
        })
        .collect()
}

fn floor() -> Flat {
    Flat { y: F::ZERO, normal: Vf::new(0.0, 1.0, 0.0, 0.0) }
}

/// Where a ride on the floor starts: a foot up, so the bike settles onto its wheels.
const ON_THE_FLOOR: Vf = Vf::new(0.0, 1.0, 0.0, 1.0);

fn gas() -> Controls {
    Controls { gas: F::ONE, ..Controls::default() }
}

/// The seconds a ride took to reach `speed` ft/s, infinite if it never did.
fn time_to(ticks: &[Tick], speed: f32) -> f32 {
    ticks.iter().position(|t| t.speed >= speed).map_or(f32::INFINITY, |i| (i + 1) as f32 * DT.0)
}

fn top(ticks: &[Tick]) -> f32 {
    ticks.iter().map(|t| t.speed).fold(0.0, f32::max)
}

/// `secs` of full throttle from rest on the floor.
fn flat_out(p: &Phy, wheels: (Vf, Vf), mods: &Mods, secs: usize) -> Vec<Tick> {
    ride(p, wheels, mods, &mut floor(), ON_THE_FLOOR, &[(0, gas())], secs * 30)
}

/// Flat ground with one take-off ramp across +z: rising at `slope` for `length` feet from `start`
/// (`tests/ride.rs`'s `Jump`, laid along +z from the origin).
struct Ramp {
    start: f32,
    slope: f32,
    length: f32,
}

/// `tests/ride.rs`'s flip ramp: 108 feet of run-up, then 25 feet at a slope of a half.
const THE_RAMP: Ramp = Ramp { start: 108.0, slope: 0.5, length: 25.0 };

impl World for Ramp {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        let t = at.z() - self.start;
        if t < 0.0 || t > self.length {
            return (Vf::new(at.x(), 0.0, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0));
        }
        let n = 1.0 / (1.0 + self.slope * self.slope).sqrt();
        (Vf::new(at.x(), t * self.slope, at.z(), 1.0), Vf::new(0.0, n, -self.slope * n, 0.0))
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

/// `tests/ride.rs`'s `THE_FLIP`, the stick's y over the ride at [`THE_RAMP`]: the preload's pull and
/// shove onto the lip (ticks 140 to 176), the backflip's pull off it (260 to 280).
const THE_FLIP: [(usize, f32); 6] = [(140, 1.0), (158, -1.0), (166, 1.0), (176, 0.0), (260, -1.0), (280, 0.0)];

/// The ride at [`THE_RAMP`] with the throttle pinned and the stick's y `ys`, its x `x` from tick 0, at
/// the 60 Hz frame `tests/ride.rs` rides it at, which is what [`THE_FLIP`]'s ticks are timed for.
fn at_the_ramp(p: &Phy, wheels: (Vf, Vf), mods: &Mods, ys: &[(usize, f32)], x: f32, ticks: usize) -> Vec<Tick> {
    let mut script = vec![(0, Controls { stick: (F(x), F::ZERO), ..gas() })];
    script.extend(ys.iter().map(|&(t, y)| (t, Controls { stick: (F(x), F(y)), ..gas() })));
    let mut ramp = Ramp { ..THE_RAMP };
    ride_at(F(1.0 / 60.0), p, wheels, mods, &mut ramp, Vf::new(0.0, 2.0, 0.0, 1.0), &script, ticks)
}

/// The highest the bike went, and the fastest it turned, while its wheels were off the ground.
fn in_the_air(ticks: &[Tick]) -> (f32, f32) {
    ticks
        .iter()
        .filter(|t| !t.on_ground)
        .fold((0.0f32, 0.0f32), |(y, s), t| (y.max(t.y), s.max(t.spin)))
}

/// The disc's bike, or a skip.
fn the_bike() -> Option<(Phy, (Vf, Vf))> {
    the_125(&Tables::build())
}

/// The ratio `a / b`, printed with its name.
fn ratio(name: &str, a: f32, b: f32) -> f32 {
    println!("{name}: {a:.3} against {b:.3} (x{:.3})", a / b);
    a / b
}

#[test]
fn bike_top_speed_tops_out_k_times_as_fast_on_the_flat() {
    let Some((p, wheels)) = the_bike() else { return };
    let stock = flat_out(&p, wheels, &Mods::original(), 40);
    // The disc's 125 tops out on the drag at 120, under the integrator's 135 (`tests/ride.rs`, P21).
    assert!((115.0..130.0).contains(&top(&stock)), "the stock bike topped out at {}", top(&stock));
    for pct in [150, 200, 300] {
        let k = pct as f32 / 100.0;
        let fast = flat_out(&p, wheels, &with(Knob::BikeSpeed, pct), 40);
        let x = ratio(&format!("bike top speed {pct}%: top"), top(&fast), top(&stock));
        assert!(x >= 0.9 * k && x <= 1.05 * k, "bike top speed {pct}%: the top went x{x}, not x{k}");
        // Off the line it is the stock bike: the same force at the wheel, on taller gearing.
        let off = ratio(&format!("bike top speed {pct}%: 0-40"), time_to(&fast, 40.0), time_to(&stock, 40.0));
        assert!(off < 1.05, "bike top speed {pct}%: 0-40 slowed to x{off}");
    }
    let slow = flat_out(&p, wheels, &with(Knob::BikeSpeed, 50), 40);
    let x = ratio("bike top speed 50%: top", top(&slow), top(&stock));
    assert!((0.45..0.6).contains(&x), "bike top speed 50%: the top went x{x}");
}

#[test]
fn engine_power_is_quicker_off_the_line() {
    let Some((p, wheels)) = the_bike() else { return };
    let stock = flat_out(&p, wheels, &Mods::original(), 30);
    let strong = flat_out(&p, wheels, &with(Knob::BikePower, 200), 30);
    // Before P97 the rear tire spun the doubled torque away: 0.87 s to 40 against 0.97.
    let forty = ratio("engine power 200%: 0-40", time_to(&strong, 40.0), time_to(&stock, 40.0));
    assert!(forty < 0.7, "engine power 200%: 0-40 only went x{forty}");
    let hundred = ratio("engine power 200%: 0-100", time_to(&strong, 100.0), time_to(&stock, 100.0));
    assert!(hundred < 0.5, "engine power 200%: 0-100 only went x{hundred}");
    // And faster at the top, not held at the integrator's 135 as it was.
    assert!(top(&strong) > 138.0, "engine power 200% topped out at {}", top(&strong));
    // Four times is quicker again, and still rides: no wreck, the nose not past a wheelie. Not five:
    // 450% to 500% is a cliff where the bike loops over or not on a tick's lag in the speed the
    // steering reads (S58 swapped which of the two wrecks), and 400% rides at a 12° nose either way.
    // Ridden from rest on the ground: since S67 the bottom-out's shove is the original's, and full gas
    // pinned through the foot's drop [`ON_THE_FLOOR`] starts from launches the extra torque off the
    // landing's rebound and loops the bike (83 degrees at tick 18 at 500%).
    let four = flat_out(&p, wheels, &with(Knob::BikePower, 400), 30);
    assert!(time_to(&four, 40.0) < time_to(&strong, 40.0), "engine power 400% is no quicker than 200%");
    let four = ride(&p, wheels, &with(Knob::BikePower, 400), &mut floor(), ON_THE_FLOOR,
                    &[(0, Controls::default()), (60, gas())], 32 * 30);
    assert!(!four.iter().any(|t| t.wreck), "engine power 400% wrecked on the flat");
    let pitch = four.iter().map(|t| t.pitch).fold(0.0, f32::max);
    assert!(pitch < 45.0, "engine power 400% stood the bike up to {pitch} degrees");
    // Less is slower.
    let weak = flat_out(&p, wheels, &with(Knob::BikePower, 50), 30);
    let x = ratio("engine power 50%: 0-40", time_to(&weak, 40.0), time_to(&stock, 40.0));
    assert!(x > 1.5, "engine power 50%: 0-40 went x{x}");
    assert!(top(&weak) < top(&stock) * 0.9, "engine power 50% topped out at {}", top(&weak));
}

#[test]
fn bike_acceleration_is_quicker_and_holds_the_top() {
    let Some((p, wheels)) = the_bike() else { return };
    let stock = flat_out(&p, wheels, &Mods::original(), 30);
    let quick = flat_out(&p, wheels, &with(Knob::BikeAccel, 200), 30);
    let forty = ratio("bike acceleration 200%: 0-40", time_to(&quick, 40.0), time_to(&stock, 40.0));
    assert!(forty < 0.7, "bike acceleration 200%: 0-40 only went x{forty}");
    let x = ratio("bike acceleration 200%: top", top(&quick), top(&stock));
    assert!((x - 1.0).abs() < 0.02, "bike acceleration 200%: the top went x{x}");
    let slow = flat_out(&p, wheels, &with(Knob::BikeAccel, 50), 30);
    let forty = ratio("bike acceleration 50%: 0-40", time_to(&slow, 40.0), time_to(&stock, 40.0));
    assert!(forty > 1.5, "bike acceleration 50%: 0-40 went x{forty}");
}

/// A put-back — the respawn button, here — sets the chassis' speed limit back. It used to set the
/// original's 135 ft/s whatever the ride was built with, so a faster bike was a stock-limited one
/// from its first wreck on.
#[test]
fn a_put_back_keeps_a_faster_bikes_speed_limit() {
    let Some((p, wheels)) = the_bike() else { return };
    let tables = Tables::build();
    for (mods, want) in [(with(Knob::BikeSpeed, 200), 4.0), (with(Knob::BikePower, 300), 9.0), (Mods::original(), 1.0)] {
        let mut phy = p.clone();
        mods.bike(0, &mut phy);
        let mut rider = Rider::new(&phy, wheels, ON_THE_FLOOR, Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
        mods.ride(0, &mut rider.ride);
        let built = rider.ride.chassis.max_speed_sq;
        assert_eq!(built.0, 135.0 * 135.0 * want, "{}: built with {}", mods.text(), built.0);
        let mut world = floor();
        let respawn = Controls { respawn: true, ..Controls::default() };
        let mut back = false;
        for i in 0..300 {
            let c = if i == 200 { respawn.clone() } else { gas() };
            rider.frame(&phy, &tables, &c, DT, &mut world);
            // The put-back sets the bike down stopped, where it last rode (S66).
            back |= i == 200 && rider.ride.chassis.speed.0 < 5.0;
        }
        assert!(back, "{}: the respawn did not put the bike back", mods.text());
        assert_eq!(rider.ride.chassis.max_speed_sq, built, "{}: the put-back changed the limit", mods.text());
        rider.ride.unwreck();
        assert_eq!(rider.ride.chassis.max_speed_sq, built, "{}: unwreck changed the limit", mods.text());
    }
}

#[test]
fn every_other_bike_knob_moves_what_it_is_named_for() {
    let Some((p, wheels)) = the_bike() else { return };
    let original = Mods::original();
    let check = |what: &str, stock: f32, up: f32, down: f32| {
        println!("{what}: {down:.3} at 50%, {stock:.3} stock, {up:.3} at 200%");
        assert!(up > stock * 1.1 && down < stock * 0.9, "{what}: {down} / {stock} / {up}");
    };

    // Grip: the launch, which the tire holds.
    let launch = |m: &Mods| 1.0 / time_to(&flat_out(&p, wheels, m, 3), 40.0);
    check("tyre grip, 1 / the seconds to 40", launch(&original), launch(&with(Knob::Grip, 200)), launch(&with(Knob::Grip, 50)));

    // Suspension: how high the bike stands at rest, its sag the springs hold up.
    let stands = |m: &Mods| {
        let t = ride(&p, wheels, m, &mut floor(), ON_THE_FLOOR, &[(0, Controls::default())], 90);
        t.last().expect("ridden").y + 1.0
    };
    check("suspension, the chassis' height at rest + 1 ft", stands(&original), stands(&with(Knob::Suspension, 200)), stands(&with(Knob::Suspension, 50)));

    // Jump: the preload off the lip, as high as it throws the bike.
    let preload = &THE_FLIP[..4];
    let jump = |m: &Mods| in_the_air(&at_the_ramp(&p, wheels, m, preload, 0.0, 300)).0;
    check("jump strength, the height off the preload", jump(&original), jump(&with(Knob::Jump, 200)), jump(&with(Knob::Jump, 50)));

    // Flip spin: how fast the backflip turns the bike.
    let flip = |m: &Mods| {
        let t = at_the_ramp(&p, wheels, m, &THE_FLIP, 0.0, 420);
        assert!(t.iter().any(|t| t.flip), "{}: the backflip never fired", m.text());
        in_the_air(&t).1
    };
    check("flip spin, rad/s in the air", flip(&original), flip(&with(Knob::SpinRate, 200)), flip(&with(Knob::SpinRate, 50)));

    // Air control: how fast the stick turns the bike in the air, off the same preload's jump.
    let lean = |m: &Mods| in_the_air(&at_the_ramp(&p, wheels, m, preload, 1.0, 300)).1;
    check("air control, rad/s in the air", lean(&original), lean(&with(Knob::AirControl, 200)), lean(&with(Knob::AirControl, 50)));

    // Crash tolerance: an 80 ft drop onto the wheels wrecks the stock bike and not a tougher one,
    // and a 40 ft one wrecks a weaker one and not the stock.
    let wrecks = |m: &Mods, h: f32| {
        ride(&p, wheels, m, &mut floor(), Vf::new(0.0, h, 0.0, 1.0), &[(0, Controls::default())], 120)
            .iter()
            .any(|t| t.wreck)
    };
    assert!(wrecks(&original, 80.0) && !wrecks(&with(Knob::CrashTolerance, 200), 80.0));
    assert!(!wrecks(&original, 40.0) && wrecks(&with(Knob::CrashTolerance, 50), 40.0));

    // The world's two on the bike: air drag the top speed (held to the integrator's 135 below the
    // original's drag, which P97 leaves as the original's), gravity the fall.
    let tops = |m: &Mods| top(&flat_out(&p, wheels, m, 30));
    let (stock, thick, thin) = (tops(&original), tops(&with(Knob::AirDrag, 200)), tops(&with(Knob::AirDrag, 50)));
    println!("air drag, the top speed: {thin:.2} at 50%, {stock:.2} stock, {thick:.2} at 200%");
    assert!(thick < stock * 0.9 && thin > stock * 1.1, "air drag: {thin} / {stock} / {thick}");
    let fall = |m: &Mods| {
        let t = ride(&p, wheels, m, &mut floor(), Vf::new(0.0, 60.0, 0.0, 1.0), &[(0, Controls::default())], 30);
        -t[29].vy
    };
    check("gravity, the speed a second's fall gives", fall(&original), fall(&with(Knob::Gravity, 200)), fall(&with(Knob::Gravity, 50)));
}

/// The disc's biplane file (`machine.res`'s `biplane.phy`), or `None` and a note.
fn the_plane() -> Option<game_sim::phy::Plane> {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/iso/MX.PSR");
    let path = if here.exists() {
        here
    } else {
        match Kit::load() {
            Ok(k) => k.ctx("iso").join("MX.PSR"),
            Err(why) => {
                skip(&why);
                return None;
            }
        }
    };
    let Ok(mut f) = File::open(&path) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", path.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find("machine.res").expect("machine.res is in MX.PSR");
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    let res = Res::parse(&bytes).expect("machine.res");
    let e = res
        .entries
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(game_sim::phy::BIPLANE_PHY))
        .expect("machine.res carries the plane's file");
    Some(game_sim::phy::Plane::read_bytes(&res.read(&bytes, e).expect("its bytes")))
}

/// The stock biplane's level top speed at full throttle, as `acceleration_modifiers`' level run
/// finds it: each step the angle of attack that holds the weight, and thrust less that drag.
fn top_speed(plane: &game_sim::phy::Plane) -> f32 {
    let mut f = game_sim::flight::Flight::new(plane);
    let weight = -f.gravity.y();
    let mass = f.mass.to_f32();
    let most = f.aero.cm_break_alpha.to_f32();
    let mut v = 0.0f32;
    for _ in 0..30 * 120 {
        let thrust = f.engine.step(F::ONE, Vf::new(0.0, 0.0, v, 0.0), F::ZERO, F::ZERO, DT, &mut |_| F::ZERO).z();
        let mut air = game_sim::biplane::Air { speed: F(v), ..Default::default() };
        let (mut lo, mut hi) = (0.0f32, most);
        for _ in 0..24 {
            air.alpha = F(0.5 * (lo + hi));
            if f.aero.forces(&air).0.y() < weight {
                lo = air.alpha.0;
            } else {
                hi = air.alpha.0;
            }
        }
        air.alpha = F(hi);
        v += (thrust - -f.aero.forces(&air).0.z()) / mass * DT.0;
    }
    v
}

/// What a turn did: the heading turned through in the first second of the pull and over all of it,
/// in degrees, the most bank, and the height and airspeed it started from and ended at.
#[derive(Debug)]
struct Turn {
    first: f32,
    all: f32,
    most_bank: f32,
    y: (f32, f32),
    speed: (f32, f32),
}

/// The biplane on `mods`, flown the way the shell's default controls fly it (the New scheme:
/// `openmx::vehicle`'s every-bank and round-stick overrides, on the stick a frame at a time) at full
/// throttle: from `speed` ft/s level, rolled to `bank` degrees over two seconds by a
/// hand on the aileron that then holds it there, and `secs` seconds of the stick at `pull` (1 is full
/// back) with the bank still held.
fn turn(plane: &game_sim::phy::Plane, mods: &Mods, speed: f32, bank: f32, pull: f32, secs: usize) -> Turn {
    use game_sim::biplane::Pad;
    let tables = game_sim::ride::Tables::build();
    let mut plane = *plane;
    mods.plane(0, &mut plane);
    let mut f = game_sim::flight::Flight::new(&plane);
    f.attitude.flown = true;
    f.place(Vf::new(2048.0, 3000.0, 2048.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), &tables.acos);
    f.body.velocity = Vf::new(0.0, 0.0, speed, 0.0);
    f.remeasure(&tables.acos);
    let mut ground = Flat::default();
    let mut prev = 0.0f32;
    let mut frame = |f: &mut game_sim::flight::Flight, bank: f32, elevator: f32| {
        // A positive aileron rolls the plane to a negative roll; the hand damps the roll rate.
        let roll = f.attitude.roll.to_f32();
        let rate = (roll - prev) / DT.0;
        prev = roll;
        let aileron = (-(1.5 * (-bank.to_radians() - roll) - 0.4 * rate)).clamp(-1.0, 1.0);
        let pad = Pad { forward: F::ONE, elevator: F(elevator), aileron: F(aileron), ..Pad::default() };
        // The New scheme's overrides, as `openmx::vehicle`'s plane step makes them for one frame.
        let (least, zone, forward, rate_limit) =
            (f.joystick.min_elevator, f.autopilot.aileron_dead_zone, f.autopilot.feed_forward, f.autopilot.max_pitch_rate);
        let held = ((F(2.0) + F(2.0) - pad.aileron.abs()) / F(2.0)).max(F::ZERO).min(F::ONE);
        f.joystick.min_elevator = F::ONE;
        f.autopilot.aileron_dead_zone = F(2.0);
        f.autopilot.feed_forward = forward * held;
        f.autopilot.max_pitch_rate = rate_limit * F(2.5);
        f.frame(DT, pad, &mut ground, &tables);
        (f.joystick.min_elevator, f.autopilot.aileron_dead_zone, f.autopilot.feed_forward, f.autopilot.max_pitch_rate) =
            (least, zone, forward, rate_limit);
        roll
    };
    let hz = (1.0 / DT.0).round() as usize;
    for _ in 0..2 * hz {
        frame(&mut f, bank, 0.0);
    }
    let heading = |f: &game_sim::flight::Flight| f.body.velocity.x().atan2(f.body.velocity.z());
    let (y0, s0) = (f.body.position.y(), f.measure.airspeed.to_f32());
    let (mut last, mut all, mut first, mut most_bank) = (heading(&f), 0.0f32, 0.0f32, 0.0f32);
    for i in 0..secs * hz {
        let roll = frame(&mut f, bank, -pull);
        let h = heading(&f);
        let d = (h - last + 3.0 * std::f32::consts::PI).rem_euclid(2.0 * std::f32::consts::PI) - std::f32::consts::PI;
        last = h;
        all += d;
        if i + 1 == hz {
            first = all;
        }
        most_bank = most_bank.max(roll.abs().to_degrees());
    }
    Turn {
        first: first.abs().to_degrees(),
        all: all.abs().to_degrees(),
        most_bank,
        y: (y0, f.body.position.y()),
        speed: (s0, f.measure.airspeed.to_f32()),
    }
}

#[test]
fn biplane_turn_speed_at_200_percent_pulls_round_near_twice_as_fast() {
    let Some(stock) = the_plane() else { return };
    // At 100% the file is the original's, to the bit.
    let mut back = with(Knob::PlaneTurn, 250);
    for seat in 0..game_sim::MAX_PLAYERS {
        back.set(seat, Knob::PlaneTurn, game_sim::modifiers::ORIGINAL);
    }
    assert!(back.is_original());
    let mut p = stock;
    back.plane(0, &mut p);
    assert_eq!(format!("{p:?}"), format!("{stock:?}"), "Biplane turn speed at 100% touched the biplane");
    let twice = with(Knob::PlaneTurn, 200);
    let top = top_speed(&stock);
    println!("the stock biplane's level top speed: {top:.1} ft/s");
    // Full stick on the knife edge from the top speed (the bank held at 90 degrees). With the stick
    // held the elevator sits on its stop, which holds the wing at the top of its lift curve, so the
    // stock turn is the lift's: 200% lifts and pitches twice as hard. The first second turns 1.71
    // times as far (81 degrees against 139 when this was written); over three seconds the harder pull
    // has bled the speed further (to 57 ft/s against 82), and the turn goes with the speed, so the
    // three seconds are 1.43 times (244 against 349).
    let a = turn(&stock, &Mods::original(), top, 90.0, 1.0, 3);
    let b = turn(&stock, &twice, top, 90.0, 1.0, 3);
    println!("100%: {a:?}\n200%: {b:?}");
    assert!(a.all > 180.0, "the stock plane only turned {} degrees in 3 s of full stick", a.all);
    let first = ratio("the first second", b.first, a.first);
    assert!((1.6..2.3).contains(&first), "200% turned {} degrees in the first second, 100% {}", b.first, a.first);
    let all = ratio("three seconds", b.all, a.all);
    assert!((1.35..2.3).contains(&all), "200% turned {} degrees in 3 s, 100% {}", b.all, a.all);
    assert!(b.most_bank < 95.0, "200% rolled past the knife edge to {} degrees", b.most_bank);
    // Slower, at 120 ft/s: at least half again as far.
    let a = turn(&stock, &Mods::original(), 120.0, 90.0, 1.0, 3);
    let b = turn(&stock, &twice, 120.0, 90.0, 1.0, 3);
    println!("from 120 ft/s, 100%: {a:?}\n200%: {b:?}");
    assert!(ratio("the first second from 120 ft/s", b.first, a.first) > 1.5);
    // The autopilot's banked hold, the stick let go at 45 degrees: the trim is the file's, so it flies
    // the stock plane's turn, and holds its height at least as well.
    let a = turn(&stock, &Mods::original(), top, 45.0, 0.0, 3);
    let b = turn(&stock, &twice, top, 45.0, 0.0, 3);
    println!("hands off at 45 degrees, 100%: {a:?}\n200%: {b:?}");
    assert!((ratio("the banked hold's turn", b.all, a.all) - 1.0).abs() < 0.1);
    assert!(b.y.0 - b.y.1 <= a.y.0 - a.y.1 + 1.0, "200% sank {} ft holding the bank, 100% {}", b.y.0 - b.y.1, a.y.0 - a.y.1);
    assert!((b.most_bank - 45.0).abs() < 5.0, "200% held the bank at {} degrees", b.most_bank);
}
