//! P137 — the Biplane max G modifier on the disc's `biplane.phy`, flown high over an endless flat
//! floor at the 30 Hz step a level runs at.
//!
//! What it holds:
//! - at 300% top speed a full pull from level cruise never passes the set limit on any frame, where
//!   off it pulls several times over;
//! - P160: the limit holds only the pull: a full aileron rolls (and yaws) the plane as with the limit
//!   off at the same speed, on the stock plane and the sped-up ones alike;
//! - at stock speed and settings a 9 g limit never acts: a cruise, a full pull and a full roll fly
//!   bit for bit as with it off;
//! - off is the original's plane: the file untouched, and no field of the flight's walk added;
//! - a session marker's put-back and a retune in flight keep the limit, and take it away again;
//! - P148: on the fastest plane the knobs make (500% top speed and acceleration) every limit from
//!   10 g to the knob's top, 15 g, holds a pull under it, where off the plane pulls far past 15 g.
//!
//! Skipped without a disc (`./kit extract MX.PSR`).

use game_sim::archive::{Psr, Res};
use game_sim::biplane::Pad;
use game_sim::flight::Flight;
use game_sim::modifiers::{Knob, MAX_G, MOST, Mods, ORIGINAL};
use game_sim::world::Flat;
use ps2_float::{F, Vf};
use ps2_oracle::{Kit, skip};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

const DT: F = F(game_sim::tick::DeltaTime::CAP_30);
const HZ: usize = 30;
/// Feet per second squared, the g the limit is counted in.
const G: f32 = 32.2;

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

/// A set with `knob` at `v` for every player.
fn with(mut mods: Mods, knob: Knob, v: u16) -> Mods {
    for seat in 0..game_sim::MAX_PLAYERS {
        mods.set(seat, knob, v);
    }
    mods
}

/// The plane's level top speed at full throttle on `plane`: each step the angle of attack that holds
/// the weight, and thrust less that drag (as `modifiers`' P108 test finds it).
fn top_speed(plane: &game_sim::phy::Plane) -> f32 {
    let mut f = Flight::new(plane);
    let weight = -f.gravity.y();
    let mass = f.mass.to_f32();
    let most = f.aero.cm_break_alpha.to_f32();
    let mut v = 0.0f32;
    for _ in 0..HZ * 200 {
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

/// How the stick is flown.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Scheme {
    /// The file's autopilot as it is: with the aileron in its dead zone the pitch-rate loop.
    File,
    /// The shell's default New scheme (`openmx::vehicle`'s every-bank and round-stick overrides).
    New,
}

/// What a frame left: the wing's load in g (the last substep's lift over the weight at 1 g), the roll
/// angle, the airspeed and the flight's state hash.
#[derive(Clone, Copy, Debug)]
struct Seen {
    g: f32,
    roll: f32,
    speed: f32,
    hash: u64,
}

/// The plane on `mods`, from `speed` ft/s level and wings level at full throttle, `script` frame by
/// frame (elevator, aileron: the pad's, 1 is full nose-down / full left).
fn fly(stock: &game_sim::phy::Plane, mods: &Mods, scheme: Scheme, speed: f32, script: &dyn Fn(usize, &Flight) -> (f32, f32), frames: usize) -> Vec<Seen> {
    let tables = game_sim::ride::Tables::build();
    let mut plane = *stock;
    mods.plane(0, &mut plane);
    let mut f = Flight::new(&plane);
    f.attitude.flown = true;
    f.place(Vf::new(2048.0, 6000.0, 2048.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), &tables.acos);
    f.body.velocity = Vf::new(0.0, 0.0, speed, 0.0);
    f.remeasure(&tables.acos);
    let mut ground = Flat::default();
    (0..frames)
        .map(|i| {
            let (elevator, aileron) = script(i, &f);
            step(&mut f, scheme, elevator, aileron, &mut ground, &tables);
            seen(&f)
        })
        .collect()
}

/// One frame of `f` at full throttle.
fn step(f: &mut Flight, scheme: Scheme, elevator: f32, aileron: f32, ground: &mut Flat, tables: &game_sim::ride::Tables) {
    let pad = Pad { forward: F::ONE, elevator: F(elevator), aileron: F(aileron), ..Pad::default() };
    if scheme == Scheme::File {
        f.frame(DT, pad, ground, tables);
        return;
    }
    let (least, zone, forward, rate_limit) =
        (f.joystick.min_elevator, f.autopilot.aileron_dead_zone, f.autopilot.feed_forward, f.autopilot.max_pitch_rate);
    let held = ((F(2.0) + F(2.0) - pad.aileron.abs()) / F(2.0)).max(F::ZERO).min(F::ONE);
    f.joystick.min_elevator = F::ONE;
    f.autopilot.aileron_dead_zone = F(2.0);
    f.autopilot.feed_forward = forward * held;
    f.autopilot.max_pitch_rate = rate_limit * F(2.5);
    f.frame(DT, pad, ground, tables);
    (f.joystick.min_elevator, f.autopilot.aileron_dead_zone, f.autopilot.feed_forward, f.autopilot.max_pitch_rate) =
        (least, zone, forward, rate_limit);
}

fn seen(f: &Flight) -> Seen {
    let v = f.air.speed.to_f32();
    let pressure = f.aero.area.to_f32() * 0.5 * f.aero.density.to_f32() * v * v;
    Seen {
        g: f.aero.lift.to_f32() * pressure / (f.mass.to_f32() * G),
        roll: f.attitude.roll.to_f32(),
        speed: v,
        // The flight's walk with the limit itself left out: a held plane that flew as the file's
        // does hashes as the file's.
        hash: game_sim::hash::of(&Flight { max_g: F::ZERO, ..f.clone() }),
    }
}

/// The most g each way over a run.
fn most_g(run: &[Seen]) -> (f32, f32) {
    run.iter().fold((0.0f32, 0.0f32), |(lo, hi), s| (lo.min(s.g), hi.max(s.g)))
}

/// The roll rate a full aileron reaches, degrees a second: the fastest over a frame in the first
/// second and a half (the roll from level, before it comes round).
fn roll_rate(run: &[Seen]) -> f32 {
    run.windows(2)
        .take(HZ * 3 / 2)
        .map(|w| {
            let d = (w[1].roll - w[0].roll + 3.0 * std::f32::consts::PI).rem_euclid(2.0 * std::f32::consts::PI) - std::f32::consts::PI;
            d.abs().to_degrees() / DT.0
        })
        .fold(0.0, f32::max)
}

const PULL: &dyn Fn(usize, &Flight) -> (f32, f32) = &|_, _| (-1.0, 0.0);
const PUSH: &dyn Fn(usize, &Flight) -> (f32, f32) = &|_, _| (1.0, 0.0);
const ROLL: &dyn Fn(usize, &Flight) -> (f32, f32) = &|_, _| (0.0, 1.0);

/// About the stock plane's top speed, ft/s: the speed [`KNIFE`]'s hand is set for.
const HAND_SPEED: f32 = 157.0;

/// Rolled to the knife edge by a hand on the aileron that then holds it there, the stick full back
/// from the second second: the aileron off its dead zone, so the elevator is the stick's, open loop.
/// The hand is gentler on a faster plane, by `(HAND_SPEED / speed)^2`, as the aileron's moment goes
/// with the speed squared (P160: the limit no longer eases the roll, so a hand set for the stock
/// plane swings a 500% one past the knife edge and back four times a second, pulling ±18 g with the
/// stick centred, limit or not).
const KNIFE: &dyn Fn(usize, &Flight) -> (f32, f32) = &|i, f| {
    let roll = f.attitude.roll.to_f32();
    let rate = -f.body.rates.z();
    let hand = (HAND_SPEED / f.air.speed.to_f32()).powi(2).min(1.0);
    let aileron = (-hand * (1.5 * (-90f32.to_radians() - roll) - 0.4 * rate)).clamp(-1.0, 1.0);
    (if i >= HZ { -1.0 } else { 0.0 }, aileron)
};

/// The stock plane and the plane on 300% Biplane top speed, with their top speeds.
fn planes() -> Option<(game_sim::phy::Plane, f32, Mods, f32)> {
    let stock = the_plane()?;
    let fast = with(Mods::original(), Knob::PlaneTopSpeed, 300);
    let mut p = stock;
    fast.plane(0, &mut p);
    Some((stock, top_speed(&stock), fast, top_speed(&p)))
}

/// A full pull, a full push and a pull at the knife edge from level cruise at 300% top speed never
/// pass the set limit on any frame, flown either way, where off the same pulls go far past 9 g; a
/// steady pull comes up to within a few percent under it. And at 200%, whose speed the 3 g limit
/// already holds hard.
#[test]
fn a_fast_pull_never_passes_the_limit() {
    let Some((stock, _, fast, fast_top)) = planes() else { return };
    let mut p = stock;
    let faster = with(Mods::original(), Knob::PlaneTopSpeed, 200);
    faster.plane(0, &mut p);
    let faster_top = top_speed(&p);
    let (mut all, mut bad) = (String::new(), String::new());
    for scheme in [Scheme::File, Scheme::New] {
        let (lo, hi) = most_g(&fly(&stock, &fast, scheme, fast_top, PULL, HZ * 6));
        assert!(-lo.min(-hi) > 15.0, "{scheme:?}: off, the 300% plane pulls {hi:.1} g / pushes {lo:.1} g");
        for (mods, speed) in [(fast, fast_top), (faster, faster_top)] {
            for g in 3..=9u16 {
                let m = with(mods, Knob::PlaneMaxG, g);
                for (name, script) in [("pull", PULL), ("push", PUSH), ("knife-edge pull", KNIFE)] {
                    let run = fly(&stock, &m, scheme, speed, script, HZ * 6);
                    let (lo, hi) = most_g(&run);
                    let most = hi.max(-lo);
                    let line = format!("{scheme:?} at {speed:.0} ft/s, {g} g: a {name} reaches {most:.3} g\n");
                    all += &line;
                    if most > g as f32 || (name == "pull" && hi < 0.92 * g as f32) {
                        bad += &line;
                    }
                }
            }
        }
    }
    assert!(bad.is_empty(), "past the limit or well short of it:\n{bad}\nall:\n{all}");
}

/// P160: a full aileron's roll rate with each limit is the limit-off roll rate at the same speed,
/// on the stock plane at its top speed and on the 200% and 300% top-speed planes at theirs, within
/// half a percent: what is left is the elevator hold's, which at speed still trims the pitch in the
/// roll, and the pitch couples into it (P137's easing took up to two thirds off it at 300%). The sped-up plane does roll faster
/// than the stock one: the limit does not take that away.
#[test]
fn a_fast_roll_is_the_limit_off_roll() {
    let Some((stock, top, fast, fast_top)) = planes() else { return };
    let mut p = stock;
    let faster = with(Mods::original(), Knob::PlaneTopSpeed, 200);
    faster.plane(0, &mut p);
    let faster_top = top_speed(&p);
    let mut bad = String::new();
    for scheme in [Scheme::File, Scheme::New] {
        let slow = roll_rate(&fly(&stock, &Mods::original(), scheme, top, ROLL, HZ * 2));
        let quick = roll_rate(&fly(&stock, &fast, scheme, fast_top, ROLL, HZ * 2));
        assert!(quick > 3.0 * slow, "{scheme:?}: off, the 300% plane rolls {quick:.0} deg/s against {slow:.0}");
        for (mods, speed) in [(Mods::original(), top), (faster, faster_top), (fast, fast_top)] {
            let off = fly(&stock, &mods, scheme, speed, ROLL, HZ * 2);
            let off_rate = roll_rate(&off);
            for g in MAX_G.0..=MAX_G.1 {
                let on = fly(&stock, &with(mods, Knob::PlaneMaxG, g), scheme, speed, ROLL, HZ * 2);
                let rate = roll_rate(&on);
                if (rate - off_rate).abs() > 0.005 * off_rate {
                    bad += &format!("{scheme:?} at {speed:.0} ft/s, {g} g: rolls {rate:.3} deg/s against off's {off_rate:.3}\n");
                }
            }
        }
    }
    assert!(bad.is_empty(), "the limit moves the roll:\n{bad}");
}

/// At stock speed and settings a 9 g limit never acts: a cruise, a full pull and push, a full roll
/// and a knife-edge pull fly bit for bit as with it off, frame by frame.
#[test]
fn the_stock_plane_is_never_held_at_9g() {
    let Some((stock, top, _, _)) = planes() else { return };
    let held = with(Mods::original(), Knob::PlaneMaxG, 9);
    let cruise: &dyn Fn(usize, &Flight) -> (f32, f32) = &|_, _| (0.0, 0.0);
    for scheme in [Scheme::File, Scheme::New] {
        for (name, script, frames) in [("cruise", cruise, HZ * 20), ("pull", PULL, HZ * 8), ("push", PUSH, HZ * 8), ("roll", ROLL, HZ * 8), ("knife-edge pull", KNIFE, HZ * 8)] {
            let off = fly(&stock, &Mods::original(), scheme, top, script, frames);
            let on = fly(&stock, &held, scheme, top, script, frames);
            let parted = off.iter().zip(&on).position(|(a, b)| a.hash != b.hash);
            assert_eq!(parted, None, "{scheme:?} {name}: 9 g parts from off");
        }
    }
}

/// Off is the original's plane: the knob at its default leaves the file as read, the flight carries
/// no limit, and its walk has no field the file's does not.
#[test]
fn off_flies_the_original() {
    let Some((stock, _, _, _)) = planes() else { return };
    let mut p = stock;
    Mods::original().plane(0, &mut p);
    let off = Flight::new(&p);
    assert_eq!(off.max_g, F::ZERO);
    let names = |f: &Flight| -> Vec<String> { game_sim::hash::Dump::of(f).fields.into_iter().map(|f| f.path).collect() };
    let file = Flight::new(&stock);
    assert_eq!(names(&off), names(&file));
    assert_eq!(game_sim::hash::of(&off), game_sim::hash::of(&file));
    assert!(!names(&off).iter().any(|n| n.contains("max_g")));
    let on = Flight::new(&{
        let mut p = stock;
        with(Mods::original(), Knob::PlaneMaxG, 6).plane(0, &mut p);
        p
    });
    assert_eq!(on.max_g, F(6.0));
    assert!(names(&on).iter().any(|n| n.ends_with("max_g")));
}

/// A session marker's put-back keeps the flight's limit, and a retune in flight (P98's
/// [`Mods::replane`]) sets it and takes it away again.
#[test]
fn a_marker_and_a_retune_keep_the_limit() {
    let Some((stock, _, fast, _)) = planes() else { return };
    let six = with(fast, Knob::PlaneMaxG, 6);
    let mut flight = Flight::new(&stock);
    let mark = flight.clone();
    six.replane(0, &stock, &mut flight);
    assert_eq!(flight.max_g, F(6.0));
    // Back to a marker set before the knob moved: the state is the marker's, the limit the knob's.
    flight.put_back(&mark);
    assert_eq!(flight.max_g, F(6.0));
    // And the other way: a marker set while it was on, put back after it went off.
    let held = flight.clone();
    Mods::original().replane(0, &stock, &mut flight);
    assert_eq!(flight.max_g, F::ZERO);
    flight.put_back(&held);
    assert_eq!(flight.max_g, F::ZERO);
    // Retuned on and back off, the flight is the stock one's again.
    six.replane(0, &stock, &mut flight);
    Mods::original().replane(0, &stock, &mut flight);
    assert_eq!(game_sim::hash::of(&flight), game_sim::hash::of(&Flight::new(&stock)));
    assert!(!game_sim::hash::Dump::of(&flight).fields.iter().any(|f| f.path.contains("max_g")));
}

/// The knob's settings: off and 3 g to 15 g (P148; 9 g before it), a g a step, off past 15 and back;
/// the text keeps it per seat, and a value off its range is refused.
#[test]
fn the_knob_steps_and_keeps() {
    let k = Knob::PlaneMaxG;
    assert_eq!(Mods::original().get(0, k), ORIGINAL);
    assert_eq!((k.least(), k.most()), (3, 15));
    assert_eq!(k.step(ORIGINAL, false), 15);
    assert_eq!(k.step(15, true), ORIGINAL);
    assert_eq!(k.step(15, false), 14);
    assert_eq!(k.step(9, true), 10);
    assert_eq!(k.step(10, false), 9);
    assert_eq!(k.step(3, false), ORIGINAL);
    assert_eq!(k.step(ORIGINAL, true), 3);
    assert_eq!(k.step(3, true), 4);
    // Round the whole ring: off, then every g from 3 to 15, then off again.
    let ring: Vec<u16> = std::iter::successors(Some(ORIGINAL), |&v| Some(k.step(v, true))).take(15).collect();
    assert_eq!(ring, [ORIGINAL].into_iter().chain(3..=15).chain([ORIGINAL]).collect::<Vec<_>>());
    assert_eq!((k.shown(ORIGINAL).as_str(), k.shown(6).as_str(), k.shown(15).as_str()), ("Off", "6 g", "15 g"));
    let mut m = Mods::original();
    m.set(1, k, 7);
    assert_eq!(m.max_g(1), Some(F(7.0)));
    assert_eq!(m.max_g(0), None);
    let text = m.text();
    assert!(text.contains("plane_max_g"), "{text}");
    assert_eq!(Mods::parse(&text).expect("its own text"), m);
    m.set(1, k, 15);
    assert_eq!(Mods::parse(&m.text()).expect("its own text").max_g(1), Some(F(15.0)));
    assert_eq!(Mods::parse("plane_max_g=12").expect("12 g is on the knob").max_g(0), Some(F(12.0)));
    assert!(Mods::parse("plane_max_g=16").is_err());
    assert!(Mods::parse("plane_max_g=2").is_err());
    m.set(1, k, 20);
    assert_eq!(m.get(1, k), 15);
}

/// P148: the fastest plane the knobs make — 500% Biplane top speed and acceleration, at its own top
/// speed — pulls far past the knob's top with the limit off, and every step above P137's 9 g, the
/// top 15 g among them, holds a full pull, push and knife-edge pull under the limit and under the
/// off pull, a steady pull coming up to within a few percent of it; and so at 300% and 400% top speed
/// (where the plane may not reach the higher limits at all, so only "under" is asked).
#[test]
fn the_top_limit_holds_the_fastest_plane() {
    let Some(stock) = the_plane() else { return };
    let (mut all, mut bad) = (String::new(), String::new());
    for top in [MOST, 400, 300] {
        let mods = with(with(Mods::original(), Knob::PlaneTopSpeed, top), Knob::PlaneAccel, MOST);
        let mut p = stock;
        mods.plane(0, &mut p);
        let speed = top_speed(&p);
        for scheme in [Scheme::File, Scheme::New] {
            let (lo, hi) = most_g(&fly(&stock, &mods, scheme, speed, PULL, HZ * 6));
            let off = hi.max(-lo);
            all += &format!("{scheme:?} at {speed:.0} ft/s ({top}%), off: a pull reaches {off:.1} g\n");
            if top == MOST {
                assert!(off > 1.5 * MAX_G.1 as f32, "{scheme:?}: off, the fastest plane pulls only {hi:.1} g / pushes {lo:.1} g");
            }
            for g in 10..=MAX_G.1 {
                let m = with(mods, Knob::PlaneMaxG, g);
                for (name, script) in [("pull", PULL), ("push", PUSH), ("knife-edge pull", KNIFE)] {
                    let (lo, hi) = most_g(&fly(&stock, &m, scheme, speed, script, HZ * 6));
                    let most = hi.max(-lo);
                    let line = format!("{scheme:?} at {speed:.0} ft/s ({top}%), {g} g: a {name} reaches {most:.3} g\n");
                    all += &line;
                    if most > g as f32 || (top == MOST && (most >= off || (name == "pull" && hi < 0.92 * g as f32))) {
                        bad += &line;
                    }
                }
            }
        }
    }
    eprintln!("{all}");
    assert!(bad.is_empty(), "past the limit, past off, or well short of it:\n{bad}\nall:\n{all}");
}

