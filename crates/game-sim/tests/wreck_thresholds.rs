//! S156: how sensitive the wreck test is — every clause of it, its threshold, where the threshold
//! comes from, and which side of it the 2p recording of record (C10) rode on.
//!
//! The wreck test itself is proven against the original over generated states (`tests/air.rs`'s
//! `the_wreck_test_is_the_originals`, `the_hits_are_the_originals`) and on every call of the 2p
//! ride's wrecks (`tests/ragdoll_wrecks.rs`'s `the_2p_wrecks_are_the_originals`). Generated states
//! straddle the thresholds only by chance; these tests put each clause's quantity *on* its threshold
//! and one float step past it, on a real bike of the 2p ride, so the strictness of every comparison
//! (`>` or `>=`, which side of a negated limit) is proven, not sampled:
//!
//! 1. [`the_2p_bikes_hold_the_ports_limits`]: both 2p bikes' wreck limits, in a RAM image of the
//!    ride, are the ones the port reads out of `Bike\Race500P.phy` (`phy::Phy`), word for word.
//! 2. [`every_wreck_clause_either_side_of_its_limit`]: the original's `bike_wreck_test` on P1's real
//!    bike with one clause's quantity on the limit (rides out) and a float step past it (wrecks), and
//!    the port's [`air::wreck_test`] then [`air::hit_wreck`] on the same inputs with the port's own
//!    limits: same reasons, every case.
//! 3. [`the_2p_ride_sits_either_side_of_the_limits`]: every tick of the 2p capture a clause judged,
//!    against the port's limit: the ridden-out ticks are on the near side, the wrecks on the far side,
//!    and the closest pairs (79.68° then 80.53° nose down, 84.85° then 85.03° of roll…) are pinned.
//!
//! The clause list, its sources and the 2p margins are tabled in
//! `research/journal/2026-10-27-s156-2p-wreck-thresholds`.

use game_render::disc::Disc;
use game_sim::air::{self, Wreck};
use game_sim::phy::Phy;
use game_sim::ride::Tables;
use game_sim::track;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

fn ctx(rel: &str) -> Option<PathBuf> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let local = kit.root.join("context").join(rel);
    let path = if local.exists() { local } else { kit.ctx(rel) };
    if !path.exists() {
        skip(&format!("{} is not there", path.display()));
        return None;
    }
    Some(path)
}

/// The game manager's slot (kit.toml's `@game_manager`).
const GAME_MANAGER: u32 = 0x004e_1e98;

/// The RAM image the clause runs start from: C10's state at frame 128466, both riders riding.
const IMAGE: u32 = 128_466;

fn image(frame: u32) -> Option<Ee> {
    let path = ctx(&format!("states/c10/c10-{frame}.ram"))?;
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    assert_eq!(ee.u32_at(addr("frame_counter").unwrap()), frame, "c10-{frame}.ram is not frame {frame}");
    Some(ee)
}

/// A rider's ridden vehicle in a 2p RAM image: the game manager's list, P1 its fourth entry, P2 its
/// third (watch/2p.toml).
fn two_p_bike(ee: &Ee, p: u32) -> u32 {
    let hops: &[u32] = if p == 1 { &[0x34, 0x34, 0x8, 0x8, 0x8, 0x0] } else { &[0x34, 0x34, 0x8, 0x8, 0x0] };
    let mut a = ee.u32_at(GAME_MANAGER);
    for &h in hops {
        a = ee.u32_at(a + h);
    }
    a
}

/// The 2p bike's physics file, read the way the port reads it.
const BIKE: &str = "Bike\\Race500P.phy";

fn race500p() -> Option<Phy> {
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let bikes: Vec<String> = disc
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.starts_with("bike\\") && n.ends_with(".res")
        })
        .collect();
    let bytes = bikes
        .iter()
        .find_map(|b| disc.res(b).ok()?.read_suffix("race500p.phy").ok())
        .expect("race500p.phy in a bike archive");
    Some(Phy::read_bytes(&bytes, &Tables::build().curve))
}

/// Every threshold the wreck test reads off the bike: the `[Wrecks]` key it is loaded from, where
/// the bike holds it, and the port's copy. Speed limits are in the file in mph and held in ft/s,
/// angles in degrees and held in radians; `NoHanderMaxRollAngle` is held as its cosine.
fn limits(p: &air::Params) -> [(&'static str, u32, F); 16] {
    [
        ("FrontWreckSpeedLimit", 0x754, p.front_wreck_speed_limit),
        ("BackWreckSpeedLimit", 0x758, p.back_wreck_speed_limit),
        ("SideWreckSpeedLimit", 0x75c, p.side_wreck_speed_limit),
        ("RacingDownWreckSpeedLimit", 0x760, p.racing_down_wreck_speed_limit),
        ("FreestyleDownWreckSpeedLimit", 0x764, p.freestyle_down_wreck_speed_limit),
        ("GroundPitchDownWreckAngle", 0x768, p.ground_pitch_down_wreck_angle),
        ("GroundPitchUpWreckAngle", 0x76c, p.ground_pitch_up_wreck_angle),
        ("AirbornePitchDownWreckAngle", 0x770, p.airborne_pitch_down_wreck_angle),
        ("AirbornePitchUpWreckAngle", 0x774, p.airborne_pitch_up_wreck_angle),
        ("AirborneRollWreckAngle", 0x778, p.airborne_roll_wreck_angle),
        ("NoHanderWreckSpeedLimit", 0x77c, p.no_hander_wreck_speed_limit),
        ("NoHanderMaxRollAngle (cos)", 0x780, p.no_hander_max_roll_angle),
        ("JoustWreckHeight", 0x784, p.joust_wreck_height),
        ("CrossupWreck", 0x78c, p.crossup_wreck),
        ("CrossupWreckTime", 0x790, p.crossup_wreck_time),
        // Not a limit of its own: the hit clause's reach is 4 when the bike is a "50", else 6.
        ("(50cc reach flag)", 0x588, F(if track::fifty_reach(BIKE) { 1.0 } else { 0.0 })),
    ]
}

/// **The 2p ride's two bikes hold the limits the port loads** (S156). Both riders are on
/// `Race500P.phy` (C10); in a RAM image of the ride, each bike's sixteen wreck words are the
/// port's [`Phy`] of that file, bit for bit (the flag as a word, 1 or 0).
#[test]
fn the_2p_bikes_hold_the_ports_limits() {
    let Some(ee) = image(IMAGE) else { return };
    let Some(phy) = race500p() else { return };
    for p in [1, 2] {
        let bike = two_p_bike(&ee, p);
        for (name, at, ours) in limits(&phy.air) {
            let theirs = ee.u32_at(bike + at);
            let ours = if at == 0x588 { ours.0 as u32 } else { ours.0.to_bits() };
            eprintln!("P{p} {name:<30} +{at:#x} {theirs:#010x} ({})", f32::from_bits(theirs));
            assert_eq!(theirs, ours, "P{p}'s {name} is {theirs:#010x} ({}) in the ride, the port's {ours:#010x} ({})", f32::from_bits(theirs), f32::from_bits(ours));
        }
    }
}

fn up(x: f32) -> f32 {
    f32::from_bits(if x >= 0.0 { x.to_bits() + 1 } else { x.to_bits() - 1 })
}

fn down(x: f32) -> f32 {
    -up(-x)
}

/// A word to write before a run: a bike offset, a chassis offset, a collision record offset, or an
/// address in the scratch block the hit cases lay their contact list and actors in.
#[derive(Clone, Copy, Debug)]
enum At {
    Bike(u32),
    Chassis(u32),
    Record(u32),
    Scratch(u32),
}

#[derive(Clone, Copy, Debug)]
struct Poke(At, u32);

fn f(at: At, v: f32) -> Poke {
    Poke(at, v.to_bits())
}

fn w(at: At, v: u32) -> Poke {
    Poke(at, v)
}

use At::{Bike as B, Chassis as C, Record as R, Scratch as S};

/// The bike on the ground, square, with nothing the wreck test judges anywhere near a limit: the
/// state every case starts from, then changes one clause's inputs of.
fn quiet() -> Vec<Poke> {
    vec![
        w(B(0x748), 0),
        f(B(0x550), 0.0),
        f(B(0x554), 0.0),
        f(B(0x558), 0.0),
        f(C(0x90), 0.0),
        f(C(0x98), 0.0),
        w(B(0x574), 1),
        w(B(0x578), 1),
        w(B(0x57c), 0),
        w(B(0x580), 0),
        f(B(0x4f0), 0.0),
        w(B(0x4f4), 0),
        w(B(0x4f8), 0),
        w(R(0x40), 0),
        w(R(0x100), 0),
        w(B(0xa0c), 0),
        w(B(0xa10), 0),
        w(B(0xa44), 0),
        w(B(0x810), 0),
        w(B(0xa78), 0),
        w(B(0xa84), 1),
        f(B(0xa2c), 0.0),
        f(B(0x788), 0.5),
        f(B(0x424), 1.0),
        w(B(0x7d8), 0),
    ]
}

/// In the air, nothing touching.
fn aloft() -> Vec<Poke> {
    vec![w(B(0x574), 0), w(B(0x578), 0), w(B(0x57c), 1), w(B(0x580), 1)]
}

/// The scratch block: a contact list of one entry at +0, another actor at +0x100, its chassis or
/// rigid body at +0x200.
const LIST: u32 = 0;
const OTHER: u32 = 0x100;
const BODY: u32 = 0x200;

/// One contact found by the bike's own rays (not mirrored), reaching `reach`, on `other` (0 for
/// scenery, else the scratch actor).
fn contact(reach: f32, other: bool) -> Vec<Poke> {
    vec![w(R(0x40), 1), w(R(0x100), 1), w(S(LIST), 0), f(S(LIST + 0x24), reach), w(S(LIST + 0x74), u32::from(other))]
}

/// The original's answer and the port's to one state: the bike with `pokes` written over it.
struct Bench {
    ee: Ee,
    bike: u32,
    ch: u32,
    rec: u32,
    scratch: u32,
    params: air::Params,
    came_off: Rc<RefCell<Vec<u32>>>,
}

impl Bench {
    fn new(mut ee: Ee, params: air::Params) -> Bench {
        let bike = two_p_bike(&ee, 1);
        let (ch, rec) = (ee.u32_at(bike + 0x7a4), ee.u32_at(bike + 0x7d0));
        // A free 0x400 bytes, high in RAM, for the hit cases' list and actors.
        let scratch = (0x01f0_0000u32..0x01ff_0000)
            .step_by(0x400)
            .rev()
            .find(|&a| (0..0x400).step_by(4).all(|k| ee.u32_at(a + k) == 0))
            .expect("a zeroed 1 KB block");
        let came_off = Rc::new(RefCell::new(Vec::new()));
        let seen = Rc::clone(&came_off);
        let slot = ee.u32_at(ee.u32_at(bike + 0x48) + 0xfc);
        ee.hooks.insert(
            slot,
            Box::new(move |ee: &mut Ee| {
                if ee.r(4) as u32 == bike {
                    seen.borrow_mut().push(ee.r(5) as u32);
                }
            }),
        );
        Bench { ee, bike, ch, rec, scratch, params, came_off }
    }

    fn at(&self, at: At) -> u32 {
        match at {
            At::Bike(o) => self.bike + o,
            At::Chassis(o) => self.ch + o,
            At::Record(o) => self.rec + o,
            At::Scratch(o) => self.scratch + o,
        }
    }

    /// Run both on the bike with `pokes` over it, and put every word back after.
    fn judge(&mut self, pokes: &[Poke]) -> (Vec<u32>, Vec<u32>) {
        // The list pointer and the actor's links into the scratch block go in whatever the case.
        let mut all = vec![w(R(0x108), self.scratch + LIST)];
        all.extend_from_slice(pokes);
        let mut was = Vec::new();
        for &Poke(at, v) in &all {
            let a = self.at(at);
            was.push((a, self.ee.u32_at(a)));
            let v = match at {
                At::Scratch(o) if o == LIST + 0x74 && v != 0 => self.scratch + OTHER,
                At::Scratch(o) if (o == OTHER + 0xf0 || o == OTHER + 0xf4) && v != 0 => self.scratch + BODY,
                _ => v,
            };
            self.ee.set_u32(a, v);
        }
        self.came_off.borrow_mut().clear();
        self.ee.call(addr("bike_wreck_test").unwrap(), &[u64::from(self.bike), 0], &[], 20_000_000);
        let original = self.came_off.borrow().clone();
        let port = self.port();
        for &(a, v) in was.iter().rev() {
            self.ee.set_u32(a, v);
        }
        (original, port)
    }

    /// The port's two clauses, [`air::wreck_test`] then [`air::hit_wreck`], composed as `rider.rs`
    /// composes them, on the bike's words and the port's own limits.
    fn port(&self) -> Vec<u32> {
        let ee = &self.ee;
        let (o, ch, rec) = (self.bike, self.ch, self.rec);
        let fl = |a: u32| F(ee.f32_at(a));
        let b = |a: u32| ee.u32_at(a) != 0;
        let v = |a: u32| Vf([0, 4, 8, 12].map(|k| ee.f32_at(a + k)));
        let mut a = air::Air {
            on_ground: b(o + 0x574),
            both_wheels: b(o + 0x578),
            airborne: b(o + 0x57c),
            was_airborne: b(o + 0x580),
            hang_time: fl(o + 0x4f0),
            flip_torque_applied: b(o + 0x4f4),
            flipping: b(o + 0x4f8),
            ..air::Air::default()
        };
        let c = air::Chassis { pitch: fl(ch + 0x90), roll: fl(ch + 0x98), vel: v(ch + 0x260), ..air::Chassis::default() };
        let st = air::Step { stunt: b(o + 0xa0c), touching: b(rec + 0x40), up: v(o + 0x420), ..air::Step::default() };
        let lo = air::Loads {
            side: fl(o + 0x550),
            vertical: fl(o + 0x554),
            fore_aft: fl(o + 0x558),
            landing_crossup: fl(o + 0x788),
            no_hander: b(o + 0xa44),
            racing: b(o + 0x810),
            pose: ee.i32_at(o + 0xa78),
            pose_wanted: ee.i32_at(o + 0xa84),
            pose_blend: fl(o + 0xa2c),
            trick_wrecked: b(o + 0xa10),
        };
        let p = &self.params;
        let landed = if ee.u32_at(o + 0x748) == 0 { air::wreck_test(p, &mut a, &c, &st, &lo) } else { None };
        let list: Vec<air::Contact> = (0..ee.u32_at(rec + 0x100))
            .map(|i| {
                let e = ee.u32_at(rec + 0x108) + 0x80 * i;
                let other = match ee.u32_at(e + 0x74) {
                    0 => air::Other::Scenery,
                    x if ee.u32_at(x + 0xf0) != 0 => {
                        let body = ee.u32_at(x + 0xf0);
                        air::Other::Rider { height: fl(body + 0x64), vel: v(body + 0x260) }
                    }
                    x => {
                        let body = ee.u32_at(x + 0xf4);
                        air::Other::Object { mass: fl(body + 0xc), vel: v(body + 0x70) }
                    }
                };
                air::Contact { mirrored: ee.u32_at(e) != 0, reach: fl(e + 0x24), other }
            })
            .collect();
        let hit = air::hit_wreck(p, &mut a, st.touching, b(o + 0x588), fl(ch + 0x210), fl(ch + 0x64), &list);
        landed.into_iter().chain(hit).map(|w| w as u32).collect()
    }
}

/// **Every wreck clause, on its threshold and one float step past it** (S156). From C10's RAM image
/// of the 2p ride, P1's own bike (Race500P, its limits the port's: [`the_2p_bikes_hold_the_ports_limits`])
/// is put in a quiet state ([`quiet`]), then one clause's inputs are set: the quantity on the limit,
/// where the original rides out, and the next float past it, where it wrecks with the clause's
/// reason. The original's `bike_wreck_test` and the port's [`air::wreck_test`] + [`air::hit_wreck`]
/// (with the port's limits out of the `.phy`) must both answer what the case says, on every case.
#[test]
fn every_wreck_clause_either_side_of_its_limit() {
    let Some(ee) = image(IMAGE) else { return };
    let Some(phy) = race500p() else { return };
    let p = phy.air;
    let mut bench = Bench::new(ee, p);
    let fifty = bench.ee.u32_at(bench.bike + 0x588) != 0;
    let reach = if fifty { air::HIT_REACH_FIFTY.0 } else { air::HIT_REACH.0 };
    let other_reach = if fifty { air::HIT_REACH.0 } else { air::HIT_REACH_FIFTY.0 };
    let height = bench.ee.f32_at(bench.ch + 0x64);
    let mass = bench.ee.f32_at(bench.ch + 0x210);
    // The rider-on-top joust subtracts on the PS2's FPU: the highest other chassis that is not more
    // than JoustWreckHeight above this one, and the next float up, which is.
    let joust = p.joust_wreck_height;
    let mut theirs = (F(height) + joust).0;
    while (F(theirs) - F(height)) > joust {
        theirs = down(theirs);
    }
    while (F(up(theirs)) - F(height)) <= joust {
        theirs = up(theirs);
    }
    let cross_lo = (F::ONE - p.crossup_wreck).0;

    let l = |x: F| x.0;
    type Case = (&'static str, Vec<Poke>, Option<Wreck>);
    let cases: Vec<Case> = vec![
        // The g loads, judged every tick whatever the bike is doing.
        ("fore-aft g on BackWreckSpeedLimit", vec![f(B(0x558), l(p.back_wreck_speed_limit))], None),
        ("fore-aft g past BackWreckSpeedLimit", vec![f(B(0x558), up(l(p.back_wreck_speed_limit)))], Some(Wreck::Back)),
        ("fore-aft g on -FrontWreckSpeedLimit", vec![f(B(0x558), -l(p.front_wreck_speed_limit))], None),
        ("fore-aft g past -FrontWreckSpeedLimit", vec![f(B(0x558), down(-l(p.front_wreck_speed_limit)))], Some(Wreck::Front)),
        ("side g on SideWreckSpeedLimit", vec![f(B(0x550), l(p.side_wreck_speed_limit))], None),
        ("side g past SideWreckSpeedLimit", vec![f(B(0x550), up(l(p.side_wreck_speed_limit)))], Some(Wreck::Left)),
        ("side g on -SideWreckSpeedLimit", vec![f(B(0x550), -l(p.side_wreck_speed_limit))], None),
        ("side g past -SideWreckSpeedLimit", vec![f(B(0x550), down(-l(p.side_wreck_speed_limit)))], Some(Wreck::Right)),
        ("vertical g on FreestyleDownWreckSpeedLimit", vec![f(B(0x554), l(p.freestyle_down_wreck_speed_limit))], None),
        ("vertical g past FreestyleDownWreckSpeedLimit", vec![f(B(0x554), up(l(p.freestyle_down_wreck_speed_limit)))], Some(Wreck::Flat)),
        ("racing: vertical g on RacingDownWreckSpeedLimit", vec![w(B(0x810), 1), f(B(0x554), l(p.racing_down_wreck_speed_limit))], None),
        ("racing: vertical g past RacingDownWreckSpeedLimit", vec![w(B(0x810), 1), f(B(0x554), up(l(p.racing_down_wreck_speed_limit)))], Some(Wreck::Flat)),
        ("no-hander: vertical g on NoHanderWreckSpeedLimit", vec![w(B(0xa44), 1), f(B(0x554), l(p.no_hander_wreck_speed_limit))], None),
        ("no-hander: vertical g past NoHanderWreckSpeedLimit", vec![w(B(0xa44), 1), f(B(0x554), up(l(p.no_hander_wreck_speed_limit)))], Some(Wreck::Flat)),
        // The slope: pitch on the ground.
        ("on the ground, pitched down to GroundPitchDownWreckAngle", vec![f(C(0x90), -l(p.ground_pitch_down_wreck_angle))], None),
        ("on the ground, pitched down past GroundPitchDownWreckAngle", vec![f(C(0x90), down(-l(p.ground_pitch_down_wreck_angle)))], Some(Wreck::Front)),
        ("on the ground, pitched up to GroundPitchUpWreckAngle", vec![f(C(0x90), l(p.ground_pitch_up_wreck_angle))], None),
        ("on the ground, pitched up past GroundPitchUpWreckAngle", vec![f(C(0x90), up(l(p.ground_pitch_up_wreck_angle)))], Some(Wreck::Tail)),
        // No roll clause on the ground: C10's 131484 lands at 85.95° and rides out.
        ("on the ground, rolled 86°", vec![f(C(0x98), 1.5)], None),
        ("on the ground, rolled -86°", vec![f(C(0x98), -1.5)], None),
        // The air: pitch and roll while neither wheel touches and no backflip is turning.
        ("in the air, pitched down to AirbornePitchDownWreckAngle", [aloft(), vec![f(C(0x90), -l(p.airborne_pitch_down_wreck_angle))]].concat(), None),
        ("in the air, pitched down past AirbornePitchDownWreckAngle", [aloft(), vec![f(C(0x90), down(-l(p.airborne_pitch_down_wreck_angle)))]].concat(), Some(Wreck::NoseDive)),
        ("in the air, pitched up to AirbornePitchUpWreckAngle", [aloft(), vec![f(C(0x90), l(p.airborne_pitch_up_wreck_angle))]].concat(), None),
        ("in the air, pitched up past AirbornePitchUpWreckAngle", [aloft(), vec![f(C(0x90), up(l(p.airborne_pitch_up_wreck_angle)))]].concat(), Some(Wreck::Tail)),
        ("in the air, rolled to AirborneRollWreckAngle", [aloft(), vec![f(C(0x98), l(p.airborne_roll_wreck_angle))]].concat(), None),
        ("in the air, rolled past AirborneRollWreckAngle", [aloft(), vec![f(C(0x98), up(l(p.airborne_roll_wreck_angle)))]].concat(), Some(Wreck::NoseDive)),
        ("in the air, rolled to -AirborneRollWreckAngle", [aloft(), vec![f(C(0x98), -l(p.airborne_roll_wreck_angle))]].concat(), None),
        ("in the air, rolled past -AirborneRollWreckAngle", [aloft(), vec![f(C(0x98), down(-l(p.airborne_roll_wreck_angle)))]].concat(), Some(Wreck::NoseDive)),
        ("in a backflip, pitched down past AirbornePitchDownWreckAngle", [aloft(), vec![w(B(0x4f8), 1), f(C(0x90), down(-l(p.airborne_pitch_down_wreck_angle)))]].concat(), None),
        ("in a backflip, pitched up past AirbornePitchUpWreckAngle", [aloft(), vec![w(B(0x4f8), 1), f(C(0x90), up(l(p.airborne_pitch_up_wreck_angle)))]].concat(), None),
        ("in a backflip, rolled past AirborneRollWreckAngle", [aloft(), vec![w(B(0x4f8), 1), f(C(0x98), up(l(p.airborne_roll_wreck_angle)))]].concat(), None),
        // Tail first: a backflip still torquing when anything is touched.
        ("backflip torquing, touching nothing", [aloft(), vec![w(B(0x4f4), 1)]].concat(), None),
        ("backflip torquing, touching", [aloft(), vec![w(B(0x4f4), 1), w(R(0x40), 1)]].concat(), Some(Wreck::Tail)),
        // The landing: crossed up past CrossupWreck, after more than CrossupWreckTime in the air.
        ("landed crossed up after CrossupWreckTime", vec![w(B(0x580), 1), f(B(0x4f0), l(p.crossup_wreck_time)), f(B(0x788), 1.0)], None),
        ("landed crossed up after past CrossupWreckTime", vec![w(B(0x580), 1), f(B(0x4f0), up(l(p.crossup_wreck_time))), f(B(0x788), 1.0)], Some(Wreck::Left)),
        ("landed crossed up to CrossupWreck", vec![w(B(0x580), 1), f(B(0x4f0), 10.0), f(B(0x788), l(p.crossup_wreck))], None),
        ("landed crossed up past CrossupWreck", vec![w(B(0x580), 1), f(B(0x4f0), 10.0), f(B(0x788), up(l(p.crossup_wreck)))], Some(Wreck::Left)),
        ("landed crossed up to 1 - CrossupWreck", vec![w(B(0x580), 1), f(B(0x4f0), 10.0), f(B(0x788), cross_lo)], None),
        ("landed crossed up past 1 - CrossupWreck", vec![w(B(0x580), 1), f(B(0x4f0), 10.0), f(B(0x788), down(cross_lo))], Some(Wreck::Right)),
        ("crossed up in the air, not landed", [aloft(), vec![f(B(0x4f0), 10.0), f(B(0x788), 1.0)]].concat(), None),
        // The trick endings.
        ("a trick running in the air", [aloft(), vec![w(B(0xa0c), 1)]].concat(), None),
        ("a trick running on the ground", vec![w(B(0xa0c), 1)], Some(Wreck::NoHander)),
        ("a trick running on the ground, already let off", vec![w(B(0xa0c), 1), w(B(0xa10), 1)], None),
        ("a trick running, touching in the air", [aloft(), vec![w(B(0xa0c), 1), w(R(0x40), 1)]].concat(), Some(Wreck::NoHander)),
        ("the pose reached, blend on 0.95", vec![w(B(0xa84), 0), f(B(0xa2c), 0.95)], None),
        ("the pose reached, blend past 0.95", vec![w(B(0xa84), 0), f(B(0xa2c), up(0.95))], Some(Wreck::Joust)),
        ("no-handed, up on cos NoHanderMaxRollAngle", vec![w(B(0xa44), 1), f(B(0x424), l(p.no_hander_max_roll_angle))], None),
        ("no-handed, up past cos NoHanderMaxRollAngle", vec![w(B(0xa44), 1), f(B(0x424), down(l(p.no_hander_max_roll_angle)))], Some(Wreck::NoHander)),
        // The hits: a body ray reaching above the bike's reach, on scenery, a rider, an object.
        ("a ray on scenery reaching to the reach", contact(reach, false), None),
        ("a ray on scenery reaching past the reach", contact(up(reach), false), Some(Wreck::Scenery)),
        ("a mirrored entry reaching past the reach", [contact(up(reach), false), vec![w(S(LIST), 1)]].concat(), None),
        ("the other reach flag: a ray to its reach", [contact(other_reach, false), vec![w(B(0x588), u32::from(!fifty))]].concat(), None),
        ("the other reach flag: a ray past its reach", [contact(up(other_reach), false), vec![w(B(0x588), u32::from(!fifty))]].concat(), Some(Wreck::Scenery)),
        (
            "a rider JoustWreckHeight above",
            [contact(up(reach), true), vec![w(S(OTHER + 0xf0), 1), f(S(BODY + 0x64), theirs)]].concat(),
            None,
        ),
        (
            "a rider past JoustWreckHeight above",
            [contact(up(reach), true), vec![w(S(OTHER + 0xf0), 1), f(S(BODY + 0x64), up(theirs))]].concat(),
            Some(Wreck::Joust),
        ),
        (
            "an object as heavy as the chassis",
            [contact(up(reach), true), vec![w(S(OTHER + 0xf0), 0), w(S(OTHER + 0xf4), 1), f(S(BODY + 0xc), mass)]].concat(),
            None,
        ),
        (
            "an object heavier than the chassis",
            [contact(up(reach), true), vec![w(S(OTHER + 0xf0), 0), w(S(OTHER + 0xf4), 1), f(S(BODY + 0xc), up(mass))]].concat(),
            Some(Wreck::Joust),
        ),
    ];

    let (original, port) = bench.judge(&quiet());
    assert!(original.is_empty() && port.is_empty(), "the quiet bike wrecks: the original {original:?}, the port {port:?}");
    for (name, pokes, want) in &cases {
        let (original, port) = bench.judge(&[quiet(), pokes.clone()].concat());
        let want: Vec<u32> = want.map(|w| w as u32).into_iter().collect();
        eprintln!("{name:<62} original {original:?} port {port:?}");
        assert_eq!(port, original, "{name}: the port answers {port:?}, the original {original:?}");
        assert_eq!(original, want, "{name}: the original answers {original:?}, the case says {want:?}");
    }
    eprintln!("{} cases, reach {reach}, joust: height {height}, the other's {theirs}; mass {mass}", cases.len());
}

// --- the 2p ride against the limits --------------------------------------------------------------

/// The capture's columns the margins read.
const COLS: [&str; 14] = [
    "wreck_reason", "mode", "on_ground", "airborne", "flip_ramp", "trick_running", "trick_safe",
    "ch_euler.x", "ch_euler.z", "g_peak.x", "g_peak.y", "g_peak.z", "hang_time", "frame_counter",
];

/// C10's 2p capture (`context/capture/2p/2p.csv`), the [`COLS`] of rider `p`, row by row.
fn capture(p: u32) -> Option<Vec<HashMap<&'static str, f64>>> {
    use std::io::BufRead;
    let file = std::fs::File::open(ctx("capture/2p/2p.csv")?).expect("open the 2p capture");
    let mut lines = std::io::BufReader::new(file).lines().map(|l| l.expect("read the 2p capture"));
    let head = lines.next().expect("a header");
    let header: Vec<&str> = head.split(',').collect();
    let at: Vec<usize> = COLS
        .iter()
        .map(|c| {
            let name = if *c == "frame_counter" { c.to_string() } else { format!("p{p}_{c}") };
            header.iter().position(|h| *h == name).unwrap_or_else(|| panic!("no column {name}"))
        })
        .collect();
    Some(
        lines
            .map(|l| {
                let all: Vec<&str> = l.split(',').collect();
                COLS.iter().zip(&at).map(|(&c, &i)| (c, all[i].parse::<f64>().unwrap_or_else(|_| panic!("{c}: {}", all[i])))).collect()
            })
            .collect(),
    )
}

/// One clause the capture can show: which ticks it judged, the quantity it judged (as the f32 the
/// test compared), the port's limit, and the reason it enters past it.
struct Judged {
    name: &'static str,
    judges: fn(&HashMap<&'static str, f64>) -> bool,
    value: fn(&HashMap<&'static str, f64>) -> f32,
    limit: F,
    reason: u32,
}

/// **The 2p ride sits either side of the port's limits** (S156). Every tick of C10's capture a
/// clause judged (the rider not wrecked going in, riding or in a trick the row before; the clause's
/// own gate — on the ground, or airborne with no backflip turning): each tick ridden out is at or
/// inside the port's limit, and each tick that wrecked with the clause's reason past it. The
/// closest ridden-out and wrecked ticks of each clause are printed and the pairs that bracket a
/// limit are pinned. The g loads, ground pitch up, crossup and the hits never came near in this
/// ride (the journal's table).
#[test]
fn the_2p_ride_sits_either_side_of_the_limits() {
    let Some(phy) = race500p() else { return };
    let p = phy.air;
    fn n(r: &HashMap<&str, f64>, c: &str) -> f32 {
        r[c] as f32
    }
    fn ground(r: &HashMap<&str, f64>) -> bool {
        r["on_ground"] != 0.0
    }
    fn air(r: &HashMap<&str, f64>) -> bool {
        r["airborne"] != 0.0 && r["flip_ramp"] == 0.0
    }
    fn any(_: &HashMap<&str, f64>) -> bool {
        true
    }
    let clauses = [
        Judged { name: "fore-aft g (back)", judges: any, value: |r| n(r, "g_peak.z"), limit: p.back_wreck_speed_limit, reason: 4 },
        Judged { name: "fore-aft g (front)", judges: any, value: |r| -n(r, "g_peak.z"), limit: p.front_wreck_speed_limit, reason: 1 },
        Judged { name: "side g (left)", judges: any, value: |r| n(r, "g_peak.x"), limit: p.side_wreck_speed_limit, reason: 2 },
        Judged { name: "side g (right)", judges: any, value: |r| -n(r, "g_peak.x"), limit: p.side_wreck_speed_limit, reason: 3 },
        Judged { name: "vertical g", judges: any, value: |r| n(r, "g_peak.y"), limit: p.freestyle_down_wreck_speed_limit, reason: 5 },
        Judged { name: "ground pitch down", judges: ground, value: |r| -n(r, "ch_euler.x"), limit: p.ground_pitch_down_wreck_angle, reason: 1 },
        Judged { name: "ground pitch up", judges: ground, value: |r| n(r, "ch_euler.x"), limit: p.ground_pitch_up_wreck_angle, reason: 11 },
        Judged { name: "air pitch down", judges: air, value: |r| -n(r, "ch_euler.x"), limit: p.airborne_pitch_down_wreck_angle, reason: 8 },
        Judged { name: "air pitch up", judges: air, value: |r| n(r, "ch_euler.x"), limit: p.airborne_pitch_up_wreck_angle, reason: 11 },
        Judged { name: "air roll", judges: air, value: |r| n(r, "ch_euler.z").abs(), limit: p.airborne_roll_wreck_angle, reason: 8 },
    ];
    // The pairs that bracket a limit: (rider, clause, last tick ridden out, the wreck's tick).
    let pinned: [(u32, &str, u32, u32); 6] = [
        (1, "air pitch down", 129944, 129945),
        (1, "air pitch up", 130965, 130966),
        (1, "air roll", 128994, 128995),
        (2, "air roll", 133799, 133800),
        (2, "ground pitch down", 131594, 131595),
        (1, "air roll", 131483, 0),
    ];
    let mut seen_pins = 0;
    for rider in [1, 2] {
        let Some(rows) = capture(rider) else { return };
        assert!(rows.len() > 12_000, "the 2p capture has {} rows", rows.len());
        for c in &clauses {
            let (mut ridden, mut wrecked): (Vec<(f32, u32)>, Vec<(f32, u32)>) = (Vec::new(), Vec::new());
            for k in 1..rows.len() {
                let (prev, row) = (&rows[k - 1], &rows[k]);
                if prev["wreck_reason"] != 0.0 || !(prev["mode"] == 1.0 || prev["mode"] == 3.0) || !(c.judges)(row) {
                    continue;
                }
                let (v, fc, reason) = ((c.value)(row), row["frame_counter"] as u32, row["wreck_reason"] as u32);
                if reason == 0 {
                    ridden.push((v, fc));
                } else if reason == c.reason && v > c.limit.0 {
                    wrecked.push((v, fc));
                }
            }
            ridden.sort_by(|a, b| b.0.total_cmp(&a.0));
            let deg = |v: f32| if c.name.contains("g") && !c.name.contains("pitch") { v } else { v.to_degrees() };
            eprintln!(
                "P{rider} {:<20} limit {:>8.3}: ridden out up to {:?}; wrecked past it {:?}",
                c.name,
                deg(c.limit.0),
                ridden.iter().take(2).map(|&(v, fc)| (deg(v), fc)).collect::<Vec<_>>(),
                wrecked.iter().map(|&(v, fc)| (deg(v), fc)).collect::<Vec<_>>()
            );
            if let Some(&(v, fc)) = ridden.first() {
                assert!(v <= c.limit.0, "P{rider} {}: {fc} rode out at {} past the limit {}", c.name, deg(v), deg(c.limit.0));
            }
            for &(r, name, last, wreck) in &pinned {
                if r != rider || name != c.name {
                    continue;
                }
                seen_pins += 1;
                assert!(ridden.iter().any(|&(_, fc)| fc == last), "P{rider} {name}: {last} is not a tick ridden out");
                if wreck != 0 {
                    assert!(wrecked.iter().any(|&(_, fc)| fc == wreck), "P{rider} {name}: {wreck} is not a wreck past the limit");
                }
            }
        }
        // Ground roll has no clause: the highest roll ridden out on the ground is past the airborne
        // limit (P1's landing at 131484, 85.95°).
        if rider == 1 {
            let row = rows.iter().find(|r| r["frame_counter"] as u32 == 131_484).expect("row 131484");
            assert!(ground(row) && !air(row) && row["wreck_reason"] == 0.0, "131484 is not a grounded ride");
            assert!(n(row, "ch_euler.z").abs() > p.airborne_roll_wreck_angle.0, "131484's roll is inside the airborne limit");
        }
        // The landing crossup: the crossup itself (+0x788) is not in the capture, but every crossup
        // wreck (Left or Right with no g load past its limit) came after more than CrossupWreckTime
        // in the air, which the capture does hold. C10: P1's 132997 and 139659, P2's 140363.
        let mut crossups = 0;
        for k in 1..rows.len() {
            let (prev, row) = (&rows[k - 1], &rows[k]);
            let reason = row["wreck_reason"] as u32;
            if prev["wreck_reason"] != 0.0 || !(reason == 2 || reason == 3) || n(row, "g_peak.x").abs() > p.side_wreck_speed_limit.0 {
                continue;
            }
            let hang = n(row, "hang_time");
            eprintln!("P{rider} {}: crossup wreck {reason} after {hang} s in the air", row["frame_counter"]);
            assert!(hang > p.crossup_wreck_time.0, "P{rider} {}: a crossup wreck after only {hang} s", row["frame_counter"]);
            crossups += 1;
        }
        assert_eq!(crossups, if rider == 1 { 2 } else { 1 }, "P{rider}'s crossup wrecks");
        // The trick endings: a trick still running when the bike is on the ground wrecks
        // (NoHander), on its row or the next (a capture row reads at one of three phases).
        let mut endings = 0;
        for k in 1..rows.len() - 1 {
            let (prev, row) = (&rows[k - 1], &rows[k]);
            if prev["wreck_reason"] != 0.0 || prev["mode"] == 0.0 || prev["mode"] == 2.0 {
                continue;
            }
            if row["trick_running"] != 0.0 && row["trick_safe"] == 0.0 && ground(row) {
                let next = rows[k + 1]["wreck_reason"] as u32;
                let here = row["wreck_reason"] as u32;
                assert!(here == 12 || (here == 0 && next == 12), "P{rider} {}: a trick on the ground answered {here} then {next}", row["frame_counter"]);
                endings += u32::from(here == 12);
            }
        }
        // C10: P1's six NoHander wrecks out of a trick (133313 … 136559), P2's one (134315).
        assert_eq!(endings, if rider == 1 { 6 } else { 1 }, "P{rider}'s wrecks out of a trick still running");
    }
    assert_eq!(seen_pins, pinned.len(), "a pinned pair's clause is not in the list");
}
