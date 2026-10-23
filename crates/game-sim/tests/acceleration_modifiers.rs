//! P95 — acceleration apart from top speed: [`Knob::BikeAccel`] for the bike, and the biplane's
//! speed split into [`Knob::PlaneAccel`] and [`Knob::PlaneTopSpeed`].
//!
//! What they hold:
//! - the bike's time from rest to 60 ft/s falls with bike acceleration up, and rises with it down,
//!   while the top speed it reaches on flat ground stays where it was;
//! - the biplane's acceleration and top speed each move without the other, on the plane's own engine
//!   and aerodynamics as the disc's file builds them (skipped without a disc);
//! - at 100% none of the three touches anything, and the old words (`bike_speed`, `bike_power`,
//!   `plane_speed`) still read and still ride the numbers they did.
//!
//! The bike is the stand-in ([`stock::BIKE`]) with `tests/ride.rs`'s ridden patches, headless; the
//! biplane is flown along a level line rather than the full flight, whose autopilot climbs away from
//! any top speed it could show (see [`level_run`]).

use game_sim::collide::{Bundle, Record};
use game_sim::archive::{Psr, Res};
use game_sim::biplane::Air;
use game_sim::flight::Flight;
use game_sim::modifiers::{self, Knob, Mods};
use game_sim::phy::{self, Phy, Plane};
use game_sim::ride::Controls;
use game_sim::rider::{Rider, Tables};
use game_sim::trick::Set;
use game_sim::world::World;
use game_sim::{Sim, stock};
use ps2_float::{F, Vf};
use ps2_oracle::{Kit, skip};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Arc;

/// The shell's fixed update's frame.
const DT: F = F(1.0 / 60.0);

/// The "0 to 60" of the bike, in feet a second: under the stand-in's top speed with room to spare.
const SIXTY: f32 = 60.0;

/// `tests/ride.rs`'s wreck limits: without them a bike at rest is a wreck that never moves.
const RIDE_LIMITS: &str = "
[Wrecks]
MaxWreckTime = 2
CannonballWreckTime = 5
FrontWreckSpeedLimit = 60
BackWreckSpeedLimit = 60
SideWreckSpeedLimit = 40
RacingDownWreckSpeedLimit = 40
FreestyleDownWreckSpeedLimit = 40
GroundPitchDownWreckAngle = 60
GroundPitchUpWreckAngle = 60
AirbornePitchDownWreckAngle = 120
AirbornePitchUpWreckAngle = 120
AirborneRollWreckAngle = 100
NoHanderWreckSpeedLimit = 400
NoHanderMaxRollAngle = 45
JoustWreckHeight = 2
CrossupWreck = 1
CrossupWreckTime = 0.3
";

/// The stand-in bike with a steering scale, the drag that lets it reach the original's top speed,
/// and the wreck limits of a bike being ridden (`tests/ride.rs`'s `ridden_bike`).
fn ridden_bike() -> String {
    let mut out = stock::BIKE.to_string();
    for (key, with) in [
        (
            "XAxisRateLimit = 6\n",
            "XAxisRateLimit = 6\nSteerScale1 = 1.0\nSteerScale2 = 0.05\nSteerSpeed1 = 5\nSteerSpeed2 = 50\n",
        ),
        ("Drag = 0.5", "Drag = 0.037"),
    ] {
        assert!(out.contains(key), "game_sim::stock::BIKE no longer has `{}`", key.trim_end());
        out = out.replace(key, with);
    }
    format!("{out}{RIDE_LIMITS}")
}

/// Flat ground at y = 0, nothing solid on it.
struct Floor;

impl World for Floor {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        (Vf::new(at.x(), 0.0, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0))
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
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

/// Player 1's bike on `mods`, loaded as the shell loads it, ridden flat out from rest for a minute:
/// the seconds it took to reach [`SIXTY`] and the top speed it reached, ft/s.
fn bike_run(mods: &Mods) -> (f32, f32) {
    let tables = Arc::new(Tables::build());
    let mut phy = Phy::read_bytes(ridden_bike().as_bytes(), &tables.ride.curve);
    mods.bike(0, &mut phy);
    let geom = Phy::read_bytes(stock::BIKE.as_bytes(), &tables.ride.curve);
    let mut rider = Rider::new(&phy, stock::mounts(&geom), stock::SPAWN, stock::FACING, Set::default());
    mods.ride(0, &mut rider.ride);
    let mut sim = Sim::with_rider(0, tables, phy, rider);
    let gas = Controls { gas: F(1.0), ..Controls::default() };
    let (mut sixty, mut top) = (None, 0.0f32);
    for i in 0..3600 {
        sim.step(DT, &gas, &mut Floor);
        let speed = sim.rider.ride.chassis.speed.0;
        if sixty.is_none() && speed >= SIXTY {
            sixty = Some((i + 1) as f32 * DT.0);
        }
        top = top.max(speed);
    }
    (sixty.unwrap_or(f32::INFINITY), top)
}

/// `Biplane\biplane.phy` out of `machine.res` (`tests/flight.rs`'s), or `None` and a note.
fn plane_file() -> Option<Vec<u8>> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/iso/MX.PSR");
    let path = if here.exists() { here } else { kit.ctx("iso").join("MX.PSR") };
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
        .find(|e| e.name.eq_ignore_ascii_case(phy::BIPLANE_PHY))
        .expect("machine.res carries the plane's file");
    Some(res.read(&bytes, e).expect("its bytes"))
}

/// The biplane on `mods`, its throttle open from rest along a level line for two minutes: the
/// seconds it took to reach `mark` ft/s and the speed it settled at.
///
/// The plane is built off the file by [`Flight::new`] and only its engine ([`game_sim::biplane::Engine::step`])
/// and its aerodynamics ([`game_sim::biplane::Aero::forces`]) are flown: each step the angle of
/// attack is found that holds the plane's weight (as near as the stall allows), so the drag is the
/// level flight's induced drag and all, and the speed moves by thrust less drag over the mass. The
/// full flight is not used because its autopilot pitches up and trades the speed for height.
fn level_run(file: &[u8], mods: &Mods, mark: f32) -> (f32, f32) {
    let mut plane = Plane::read_bytes(file);
    mods.plane(0, &mut plane);
    let mut f = Flight::new(&plane);
    let weight = -f.gravity.y();
    let mass = f.mass.to_f32();
    let most = f.aero.cm_break_alpha.to_f32();
    let dt = 1.0 / 30.0;
    let (mut v, mut at) = (0.0f32, None);
    for i in 0..30 * 120 {
        let thrust = f.engine.step(F::ONE, Vf::new(0.0, 0.0, v, 0.0), F::ZERO, F::ZERO, F(dt), &mut |_| F::ZERO).z();
        let mut air = Air { speed: F(v), ..Air::default() };
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
        let drag = -f.aero.forces(&air).0.z();
        v += (thrust - drag) / mass * dt;
        if at.is_none() && v >= mark {
            at = Some((i + 1) as f32 * dt);
        }
    }
    (at.unwrap_or(f32::INFINITY), v)
}

#[test]
fn bike_acceleration_cuts_the_zero_to_sixty_and_holds_the_top_speed() {
    let (stock_sixty, stock_top) = bike_run(&Mods::original());
    println!("stock: 0-{SIXTY} in {stock_sixty:.2} s, top {stock_top:.2} ft/s");
    assert!(stock_top > SIXTY * 1.2, "the stock bike only reached {stock_top} ft/s");
    for (pct, faster) in [(200, true), (150, true), (70, false)] {
        let (sixty, top) = bike_run(&with(Knob::BikeAccel, pct));
        println!("bike acceleration {pct}%: 0-{SIXTY} in {sixty:.2} s, top {top:.2} ft/s");
        match faster {
            true => assert!(sixty < stock_sixty * 0.85, "{pct}%: 0-{SIXTY} in {sixty} s against {stock_sixty}"),
            false => assert!(sixty > stock_sixty * 1.15, "{pct}%: 0-{SIXTY} in {sixty} s against {stock_sixty}"),
        }
        // Within 2%: the drive and the three drags scale alike, and what is left over (the tires'
        // slip under k times the torque) settled the top 1.0 to 1.4% high when this was written.
        let off = (top - stock_top).abs() / stock_top;
        assert!(off < 0.02, "{pct}%: the top speed moved to {top} ft/s from {stock_top}");
    }
    // Engine power, which the new knob is not: it raises the top speed too.
    let (_, power_top) = bike_run(&with(Knob::BikePower, 200));
    println!("engine power 200%: top {power_top:.2} ft/s");
    assert!(power_top > stock_top * 1.1, "engine power x2 left the top at {power_top}");
}

#[test]
fn biplane_acceleration_and_top_speed_move_apart() {
    let Some(file) = plane_file() else { return };
    // The mark is low on the climb to the top, where acceleration is the thrust's and not the drag's.
    let mark = 80.0;
    let (stock_at, stock_top) = level_run(&file, &Mods::original(), mark);
    println!("stock: 0-{mark} in {stock_at:.2} s, top {stock_top:.2} ft/s");
    assert!(stock_top > mark * 1.5, "the stock plane only reached {stock_top} ft/s");
    let near = |a: f32, b: f32, by: f32| (a - b).abs() / b < by;

    for pct in [200, 50] {
        let (at, top) = level_run(&file, &with(Knob::PlaneAccel, pct), mark);
        println!("biplane acceleration {pct}%: 0-{mark} in {at:.2} s, top {top:.2} ft/s");
        assert!(near(top, stock_top, 0.01), "acceleration {pct}%: the top speed moved to {top} from {stock_top}");
        match pct > 100 {
            true => assert!(at < stock_at * 0.7, "acceleration {pct}%: 0-{mark} in {at} s against {stock_at}"),
            false => assert!(at > stock_at * 1.4, "acceleration {pct}%: 0-{mark} in {at} s against {stock_at}"),
        }
    }
    for pct in [150, 70] {
        let (at, top) = level_run(&file, &with(Knob::PlaneTopSpeed, pct), mark);
        let k = pct as f32 / 100.0;
        println!("biplane top speed {pct}%: 0-{mark} in {at:.2} s, top {top:.2} ft/s (x{:.3})", top / stock_top);
        assert!(near(top, stock_top * k, 0.06), "top speed {pct}%: {top} ft/s against {} expected", stock_top * k);
        assert!(near(at, stock_at, 0.1), "top speed {pct}%: 0-{mark} moved to {at} s from {stock_at}");
    }
    // Both at once: each does its own.
    let mut both = with(Knob::PlaneAccel, 200);
    for seat in 0..game_sim::MAX_PLAYERS {
        both.set(seat, Knob::PlaneTopSpeed, 150);
    }
    let (at, top) = level_run(&file, &both, mark);
    println!("both: 0-{mark} in {at:.2} s, top {top:.2} ft/s");
    assert!(at < stock_at * 0.7 && near(top, stock_top * 1.5, 0.06), "{at} s, {top} ft/s");
}

#[test]
fn at_one_hundred_percent_the_new_knobs_touch_nothing() {
    let tables = Tables::build();
    let stock_phy = Phy::read_bytes(ridden_bike().as_bytes(), &tables.ride.curve);
    let plane = plane_file().map(|f| Plane::read_bytes(&f));
    for knob in [Knob::BikeAccel, Knob::PlaneAccel, Knob::PlaneTopSpeed] {
        let mut mods = with(knob, 250);
        assert!(!mods.is_original());
        for seat in 0..game_sim::MAX_PLAYERS {
            mods.set(seat, knob, modifiers::ORIGINAL);
        }
        assert!(mods.is_original() && mods.text().is_empty(), "{knob:?}");
        let mut phy = stock_phy.clone();
        mods.bike(0, &mut phy);
        assert_eq!(format!("{phy:?}"), format!("{stock_phy:?}"), "{knob:?} at 100% touched the bike");
        if let Some(p) = plane {
            let mut q = p;
            mods.plane(0, &mut q);
            // Debug shows every float to the bit; PartialEq would let -0 by 0.
            assert_eq!(format!("{q:?}"), format!("{p:?}"), "{knob:?} at 100% touched the biplane");
        }
        // Off it, the bike's knob reaches the bike alone and the plane's the plane alone.
        let mods = with(knob, 150);
        let mut phy = stock_phy.clone();
        mods.bike(0, &mut phy);
        let bike = format!("{phy:?}") != format!("{stock_phy:?}");
        assert_eq!(bike, knob == Knob::BikeAccel, "{knob:?} at 150% and the bike");
        if let Some(p) = plane {
            let mut q = p;
            mods.plane(0, &mut q);
            assert_eq!(q != p, knob != Knob::BikeAccel, "{knob:?} at 150% and the biplane");
        }
        assert_eq!(Mods::parse(&mods.text()).unwrap(), mods);
    }
    // The menu has a row for every knob but the old biplane speed.
    assert!(!Knob::MENU.contains(&Knob::PlaneSpeed));
    assert!(Knob::ALL.iter().all(|k| *k == Knob::PlaneSpeed || Knob::MENU.contains(k)));
}

#[test]
fn the_old_words_still_load_and_ride_as_they_did() {
    let mods = Mods::parse("bike_speed=130 bike_power=120 plane_speed=150 plane_speed.1=170").unwrap();
    assert_eq!(mods.get(0, Knob::BikeSpeed), 130);
    assert_eq!(mods.get(3, Knob::BikePower), 120);
    assert_eq!(mods.get(0, Knob::PlaneSpeed), 170);
    assert_eq!(mods.get(1, Knob::PlaneSpeed), 150);
    assert_eq!((mods.get(0, Knob::PlaneAccel), mods.get(0, Knob::PlaneTopSpeed)), (100, 100));
    assert_eq!(mods.get(0, Knob::BikeAccel), 100);
    assert_eq!(Mods::parse(&mods.text()).unwrap(), mods);
    // `plane_speed` is P55's sums on the file, as P55 wrote them.
    let Some(file) = plane_file() else { return };
    let p = Plane::read_bytes(&file);
    let k = F(150.0) / F(100.0);
    let mut old = p;
    old.engine.max_power = old.engine.max_power * k;
    old.engine.min_prop_efficiency_speed = old.engine.min_prop_efficiency_speed * k;
    old.aero.cd0 = old.aero.cd0 / (k * k);
    let mut q = p;
    mods.plane(1, &mut q);
    assert_eq!(format!("{q:?}"), format!("{old:?}"));
}

/// The height of Free06's runway, where the biplane stands (C7's state): the engine's power fades with
/// height to nothing at `MaxAltitude`, and here it gives 77% of its sea-level thrust, which a run on
/// a floor at 0 would not.
const RUNWAY: f32 = 138.09;

/// The biplane on `mods` flown as the ride flies it ([`Flight::frame`], 30 Hz) on a level runway at
/// [`RUNWAY`]: from rest, Cross held until it rolls at `mark` ft/s, then Square held — the throttle
/// closing and the wheels braked. The ticks the run up took, the ticks from Square to a stop (under
/// 1.5 ft/s; held on, the throttle's one step under zero rolls it back at a crawl), the top speed
/// on the way, and the lowest the nose went (the nose's `y`, +0.18 standing on three wheels). `None`
/// for a plane that left the ground.
fn runway_run(file: &[u8], mods: &Mods, mark: f32) -> Option<(u32, u32, f32, f32)> {
    let tables = game_sim::ride::Tables::build();
    let mut plane = Plane::read_bytes(file);
    mods.plane(0, &mut plane);
    let mut f = Flight::new(&plane);
    f.attitude.flown = true;
    let lowest = f.tires.iter().map(|t| F(t.center.y()) - t.radius).fold(F(f32::MAX), |a, b| a.min(b));
    f.place(Vf::new(0.0, RUNWAY - lowest.to_f32(), 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), &tables.acos);
    let mut ground = game_sim::world::Flat { y: F(RUNWAY), ..Default::default() };
    let dt = F(1.0 / 30.0);
    let gas = game_sim::biplane::Pad { forward: F::ONE, ..Default::default() };
    let back = game_sim::biplane::Pad { back: F::ONE, ..Default::default() };
    for _ in 0..30 {
        f.frame(dt, game_sim::biplane::Pad::default(), &mut ground, &tables);
    }
    let mut up = 0;
    while f.measure.airspeed.to_f32() < mark {
        f.frame(dt, gas, &mut ground, &tables);
        up += 1;
        assert!(up < 30 * 60, "a minute of gas and the plane never rolled at {mark} ft/s");
    }
    let (mut stop, mut top, mut nose) = (0, 0.0f32, 1.0f32);
    while f.measure.airspeed.to_f32() >= 1.5 {
        f.frame(dt, back, &mut ground, &tables);
        stop += 1;
        top = top.max(f.measure.airspeed.to_f32());
        nose = nose.min(f.measure.forward.y());
        assert!(stop < 30 * 60, "a minute of Square and the plane never stopped");
        if !f.measure.grounded {
            return None;
        }
    }
    Some((up, stop, top, nose))
}

/// The biplane on `mods` along [`level_run`]'s level line: at its top speed with the throttle open, then
/// the throttle closed at its own `ThrottleIncrement` a second as Square closes it. The seconds to
/// half the top speed.
fn level_slow(file: &[u8], mods: &Mods) -> f32 {
    let (_, top) = level_run(file, mods, f32::MAX);
    let mut plane = Plane::read_bytes(file);
    mods.plane(0, &mut plane);
    let mut f = Flight::new(&plane);
    let weight = -f.gravity.y();
    let mass = f.mass.to_f32();
    let most = f.aero.cm_break_alpha.to_f32();
    let dt = 1.0 / 30.0;
    let (mut v, mut throttle) = (top, F::ONE);
    for i in 0..30 * 120 {
        throttle = (throttle - f.joystick.throttle_increment * F(dt)).max(F::ZERO);
        let thrust = f.engine.step(throttle, Vf::new(0.0, 0.0, v, 0.0), F::ZERO, F::ZERO, F(dt), &mut |_| F::ZERO).z();
        let mut air = Air { speed: F(v), ..Air::default() };
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
        let drag = -f.aero.forces(&air).0.z();
        v += (thrust - drag) / mass * dt;
        if v < top * 0.5 {
            return (i + 1) as f32 * dt;
        }
    }
    f32::INFINITY
}

/// **P116**: the biplane's acceleration is its braking too. At k times the acceleration the plane
/// gets up to speed on the runway in about 1 / k the time, and from there, Square held, stops in about
/// 1 / k the time; along a level line in the air the closed throttle halves its top speed in about 1 / k
/// the time. Before P116 the 200% plane took *longer* to stop on the runway than the file's (252 ticks
/// against 162 from 50 ft/s, on a floor at 0): its doubled power held off brakes that were the file's
/// while the throttle took the file's 2.9 s to close. And braking harder does not stand it on its nose:
/// twice the file's braking at the wheels did (the nose went to -0.998), so the extra goes through the
/// centre of gravity, and at the slider's top (500%) the nose stays near level.
///
/// The mark is 30 ft/s, under the lift-off: a 200% plane is at 45 ft/s before its tail has come up,
/// and on three wheels the wing's angle lifts it off there. At the slider's top it is 20: from 25 the
/// 500% plane overshoots into the air while its throttle closes (the lift is the file's).
#[test]
fn biplane_acceleration_brakes_with_it() {
    let Some(file) = plane_file() else { return };
    for (pct, mark) in [(200u16, 30.0), (50, 30.0), (modifiers::MOST, 20.0)] {
        let (up, stop, top, nose) = runway_run(&file, &Mods::original(), mark).expect("the file's plane left the runway");
        let k = pct as f32 / 100.0;
        let mods = with(Knob::PlaneAccel, pct);
        let (u, s, t, n) = runway_run(&file, &mods, mark).unwrap_or_else(|| panic!("{pct}%: the plane left the runway"));
        println!(
            "{pct}%: 0-{mark} in {u} ticks against 100%'s {up} (x{:.3}), Square to a stop in {s} against {stop} (x{:.3}); top {t:.1} ft/s against {top:.1}, nose down to {n:.3} against {nose:.3}",
            u as f32 / up as f32,
            s as f32 / stop as f32,
        );
        // Held back, it tops out about where the file's does: the same plane on a faster clock.
        // Within 10%, as are the times below: what is not scaled is gravity, so how the plane sits on
        // its wheels (at 50% the tail stays down through the stop, the wing at a draggier angle, and
        // it stopped in x1.83 the file's time when this was written) and the wheels' springs.
        // 500% rises a little higher (12% when this was written): the wheels all but leave the runway.
        let near = if pct == modifiers::MOST { 0.15 } else { 0.1 };
        assert!((t - top).abs() / top < near, "{pct}%: held back, it topped out at {t} ft/s against {top}");
        // Twice the file's braking at the contacts put it to -0.998; through the centre of gravity
        // 200% went to -0.146 and 500% not below level when this was written.
        assert!(n > -0.2, "{pct}%: braking put the nose down to {n} (the file's {nose})");
        // 500%'s runs are a dozen ticks or so, and ticks are whole: within a tick and a half.
        let by = if pct == modifiers::MOST { 0.12 } else { 0.1 };
        for (what, ratio) in [("the run up", u as f32 / up as f32), ("the stop", s as f32 / stop as f32)] {
            assert!((ratio * k - 1.0).abs() < by, "{pct}%: {what} took x{ratio} the file's");
        }
    }
    let half = level_slow(&file, &Mods::original());
    for pct in [200u16, 50, modifiers::MOST] {
        let k = pct as f32 / 100.0;
        let h = level_slow(&file, &with(Knob::PlaneAccel, pct));
        println!("{pct}%: in the air, the throttle closing, to half the top speed in {h:.2} s against 100%'s {half:.2} (x{:.3})", h / half);
        assert!((h / half * k - 1.0).abs() < 0.05, "{pct}%: in the air, x{} the file's", h / half);
    }
}
