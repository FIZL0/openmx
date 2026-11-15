//! MD145 — what MX vs. ATV Untamed's (SLUS-21701) bike and car updates hand its hazard clock, and
//! whether its node world update reaches a car.
//!
//! Untamed's hazard clock takes a third word, `grounded` (MD81b proved the clock itself,
//! `tests/untamed_atv.rs`). Its three callers each hand their own:
//!
//! - the bike's update: either wheel's ray down (its `+0xd0`), beside `on` (that and the held word,
//!   either wheel's surface holding). Ported as [`game_sim::handling::untamed::bike_hazard`], in
//!   [`game_sim::handling::tail::vehicle_update_untamed`]; proved here as a stretch of the update, the
//!   clock run as it is and the put-back hooked.
//! - the car's update: any tire solidly down (its `+0xa4`), beside `on` (that and some tire on a
//!   hazard surface). Ported by MD81d as [`game_sim::atv_car::Update::untamed`]; proved here as a
//!   stretch of the update, the clock hooked and its arguments compared with the port's
//!   [`game_sim::atv_car::Call::UntamedHazardClock`].
//! - the quad's update: any wheel's ray down (MD81b, `quad::Tail::untamed`).
//!
//! The oracle is SLUS-21701's code off a RAM image of a Free Ride in it (`context/untamed/ram/nat_or.bin`),
//! in the interpreter, on a vehicle laid out at Untamed's offsets. Without the image the tests skip.
//!
//! The node world update: Untamed makes the topmost stale node's world frame orthonormal only when it
//! has a parent. The car's node has none in Untamed's own rides (MD164's spawn and air images), so on
//! the port's car ride, whose scene holds the car's node alone, [`game_sim::atv_moto::Scene::untamed`]
//! changes no word: [`untamed_car_node_has_no_parent`].

use game_sim::atv_car::{self, Call, Car, Update};
use game_sim::atv_body::Body;
use game_sim::handling::tail::{NO_STUCK, Out, Vehicle, Vehicles, Wheel};
use game_sim::handling::untamed;
use game_sim::quad::Surface;
use ps2_float::F;
use ps2_oracle::{DONE, Ee, Kit, skip};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

/// Where the harness puts the vehicle, its vtable, its run and its wheels: the top of RAM, under the
/// stack.
const OBJ: u32 = 0x01c0_0000;
const VTABLE: u32 = 0x01c0_2000;
const RUN: u32 = 0x01c0_2800;
const PUT_BACK: u32 = 0x01c0_2900;
const WHEELS: u32 = 0x01c0_3000;
const WHEEL_SIZE: u32 = 0x400;

/// SLUS-21701's hazard clock and its first two instructions (to check the image).
const CLOCK: u32 = 0x004c_e3a0;
const CLOCK_HEAD: [u32; 2] = [0x27bd_ffe0, 0xffbf_0010];

/// The bike's update's stretch: from the inner update's return to the clock's return. The bike is in
/// `$s4`, the frame's `dt` in `$f20`.
const BIKE_FROM: u32 = 0x004f_2278;
const BIKE_TO: u32 = 0x004f_234c;
/// The car's update's stretch: from the hazard surfaces' first read to the clock's return. The car is
/// in `$s0`, `dt` in `$f20`.
const CAR_FROM: u32 = 0x004e_9f10;
const CAR_TO: u32 = 0x004e_a0b0;

/// The vehicle's words at Untamed's offsets: the held (or on-hazard) word, the clock's stuck, free and
/// time, the run's pointer, and the surface table (12 bytes an entry: the holding or hazard word, the
/// time).
const HELD: u32 = 0xc10;
const STUCK: u32 = 0xc00;
const FREE: u32 = 0xc04;
const TIME: u32 = 0xde0;
const RUN_AT: u32 = 0x6a4;
const SURFACES: u32 = 0xde4;
/// The bike's two wheels (front, rear), each one's ray word and surface byte.
const BIKE_WHEELS: [u32; 2] = [0x17f0, 0x17f4];
const RAY: u32 = 0xd0;
const BIKE_SURFACE: u32 = 0x110;
/// The car's four tires (front left, front right, rear left, rear right), each one's solid word and
/// surface byte.
const CAR_TIRES: [u32; 4] = [0x1244, 0x1248, 0x124c, 0x1250];
const SOLID: u32 = 0xa4;
const CAR_SURFACE: u32 = 0xf0;

fn image() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("untamed/ram").join(format!("nat_or{}", ".bin"));
    if !path.exists() {
        skip(&format!(
            "{} is not there: unzip eeMemory.bin from an MX vs. ATV Untamed (SLUS-21701) Free Ride save state \
             (ps2-decomp-kit-slots/pcsx2/s9, SLUS-21701 (08FE7DFD).05.p2s) to it (s1 has it)",
            path.display()
        ));
        return None;
    }
    let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    if [ee.u32_at(CLOCK), ee.u32_at(CLOCK + 4)] != CLOCK_HEAD {
        panic!("{} is not the MX vs. ATV Untamed image these addresses are from", path.display());
    }
    Some(ee)
}

struct Gen(u64);

#[allow(clippy::float_arithmetic)]
impl Gen {
    fn bits(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }
    fn unit(&mut self) -> f32 {
        self.bits() as f32 / 4294967296.0
    }
    fn b(&mut self) -> bool {
        self.bits() & 0x10000 != 0
    }
    /// A word that is zero half the time, else 1 or some other non-zero word.
    fn word(&mut self) -> u32 {
        match self.bits() % 4 {
            0 | 1 => 0,
            2 => 1,
            _ => self.bits() | 0x100,
        }
    }
    /// A surface's time: −1 (at once), signed zeros, under and over 0.
    fn time(&mut self) -> F {
        let from = [-1.0, 0.0, -0.0, -0.5, 0.5, 1.0, 3.0];
        if self.b() { F(from[self.bits() as usize % from.len()]) } else { F(-2.0 + 6.0 * self.unit()) }
    }
    fn clock_time(&mut self) -> F {
        let from = [0.0, 0.49, 0.98, 2.99];
        if self.b() { F(3.5 * self.unit()) } else { F(from[self.bits() as usize % from.len()]) }
    }
    fn dt(&mut self) -> F {
        F([1.0 / 60.0, 1.0 / 30.0, 0.0, 0.25][self.bits() as usize % 4])
    }
}

/// The surface table, the same 16 entries laid for the original and handed to the port.
fn surfaces(g: &mut Gen) -> [(u32, F); 16] {
    std::array::from_fn(|_| (g.word(), g.time()))
}

fn clear(ee: &mut Ee) {
    for o in (0..0x1800).step_by(4) {
        ee.set_u32(OBJ + o, 0);
    }
    for o in (0..4 * WHEEL_SIZE).step_by(4) {
        ee.set_u32(WHEELS + o, 0);
    }
}

fn set_byte(ee: &mut Ee, a: u32, b: u8) {
    let w = ee.u32_at(a & !3);
    let shift = 8 * (a & 3);
    ee.set_u32(a & !3, w & !(0xff << shift) | u32::from(b) << shift);
}

fn lay_table(ee: &mut Ee, table: &[(u32, F); 16]) {
    for (k, &(word, time)) in table.iter().enumerate() {
        ee.set_u32(OBJ + SURFACES + 12 * k as u32, word);
        ee.set_f32(OBJ + SURFACES + 12 * k as u32 + 4, time.0);
    }
}

/// The port's side of the bike: its words, the put-backs counted.
struct Clock {
    v: Vehicle,
    put_backs: u32,
}

impl Vehicles for Clock {
    fn vehicle(&mut self) -> &mut Vehicle {
        &mut self.v
    }
    fn skipped(&mut self) -> bool {
        false
    }
    fn paused(&mut self) -> bool {
        false
    }
    fn rumble_ready(&mut self) -> bool {
        false
    }
    fn word(&mut self, _at: u32) -> u32 {
        0
    }
    fn out(&mut self, c: Out, _dt: F) {
        assert_eq!(c, Out::PutBack, "the hazard clock asks only for the put-back");
        self.put_backs += 1;
    }
}

fn end_stretch(ee: &mut Ee, at: u32) {
    ee.hooks.insert(at, Box::new(|ee: &mut Ee| ee.set(31, u64::from(DONE) | 0xffff_ffff_0000_0000)));
}

#[test]
fn untamed_bike_hands_the_clock_either_rays_hit() {
    let Some(mut ee) = image() else { return };
    let put_backs = Rc::new(Cell::new(0u32));
    let count = put_backs.clone();
    ee.hooks.insert(PUT_BACK, Box::new(move |ee: &mut Ee| {
        assert_eq!(ee.r(4) as u32, OBJ, "the put-back is called on the bike");
        assert_eq!((ee.r(5), ee.r(6)), (1, 1), "the put-back is asked to place and keep");
        count.set(count.get() + 1);
    }));
    end_stretch(&mut ee, BIKE_TO);
    let mut g = Gen(0x4d44_3134_3562);
    let mut tally = BTreeMap::<&str, u32>::new();
    for k in 0..6000 {
        let table = surfaces(&mut g);
        let ray = [g.word(), g.word()];
        let surface = [(g.bits() % 16) as u8, (g.bits() % 16) as u8];
        let (dt, free, stuck, time) = (g.dt(), g.bits() & 1, g.bits() & 1, g.clock_time());
        let run_flags = if g.b() { NO_STUCK } else { 0 } | (g.bits() & 0x0f);

        clear(&mut ee);
        ee.set_u32(OBJ + 4, VTABLE);
        ee.set_u32(VTABLE + 0x3a8, PUT_BACK);
        ee.set_u32(OBJ + RUN_AT, RUN);
        ee.set_u32(RUN + 8, run_flags);
        ee.set_u32(RUN + 12, 0);
        lay_table(&mut ee, &table);
        for w in 0..2 {
            let at = WHEELS + WHEEL_SIZE * w as u32;
            ee.set_u32(OBJ + BIKE_WHEELS[w], at);
            ee.set_u32(at + RAY, ray[w]);
            set_byte(&mut ee, at + BIKE_SURFACE, surface[w]);
        }
        ee.set_u32(OBJ + STUCK, stuck);
        ee.set_u32(OBJ + FREE, free);
        ee.set_f32(OBJ + TIME, time.0);
        ee.set_u32(OBJ + HELD, g.word());
        put_backs.set(0);
        ee.set(20, u64::from(OBJ));
        ee.set_f(20, dt.0);
        ee.call(BIKE_FROM, &[], &[], 20_000);

        let wheels = std::array::from_fn(|w| {
            let (holds, limit) = table[usize::from(surface[w])];
            Wheel { ray_hit: ray[w] != 0, surface_holds: holds != 0, stuck_limit: limit, ..Wheel::default() }
        });
        let v = Vehicle { wheels, free, stuck, stuck_time: time, run_flags, ..Vehicle::default() };
        let mut c = Clock { v, put_backs: 0 };
        untamed::bike_hazard(&mut c, dt);
        let want = (ee.u32_at(OBJ + HELD), ee.u32_at(OBJ + FREE), ee.u32_at(OBJ + STUCK), ee.u32_at(OBJ + TIME), put_backs.get());
        let got = (c.v.held, c.v.free, c.v.stuck, c.v.stuck_time.0.to_bits(), c.put_backs);
        assert_eq!(got, want, "state {k}: rays {ray:x?}, surfaces {surface:?}, dt {dt:?}");

        let grounded = ray.iter().any(|&r| r != 0);
        let path = match (grounded, c.v.held != 0, got.4) {
            (false, _, 0) => "no ray down",
            (false, _, _) => "no ray down, put back",
            (true, false, _) => "a ray down, not held",
            (true, true, 0) => "held",
            (true, true, _) => "held, put back",
        };
        *tally.entry(path).or_default() += 1;
    }
    eprintln!("MD145 bike hazard: {tally:?}");
    for (path, n) in [("no ray down", 200), ("no ray down, put back", 100), ("a ray down, not held", 200), ("held", 200), ("held, put back", 100)] {
        assert!(tally.get(path).copied().unwrap_or(0) >= n, "too few states went {path:?}: {tally:?}");
    }
}

#[test]
fn untamed_car_hands_the_clock_any_tire_solid() {
    let Some(mut ee) = image() else { return };
    let seen = Rc::new(RefCell::new(None::<(u64, u64, u32, u32)>));
    let into = seen.clone();
    ee.hooks.insert(CLOCK, Box::new(move |ee: &mut Ee| {
        assert_eq!(ee.r(4) as u32, OBJ, "the clock is called on the car");
        *into.borrow_mut() = Some((ee.r(5), ee.r(6), ee.f(12).to_bits(), ee.f(13).to_bits()));
    }));
    end_stretch(&mut ee, CAR_TO);
    let mut g = Gen(0x4d44_3134_3563);
    let mut tally = BTreeMap::<&str, u32>::new();
    for k in 0..6000 {
        let mut table = surfaces(&mut g);
        if g.b() {
            // Most surfaces safe, so that four tires off a hazard come up often enough.
            for e in &mut table {
                if g.bits() % 4 != 0 {
                    e.0 = 0;
                }
            }
        }
        let solid: [u32; 4] = std::array::from_fn(|_| if g.bits() % 3 == 0 { g.word() } else { 0 });
        let surface: [u8; 4] = std::array::from_fn(|_| (g.bits() % 16) as u8);
        let dt = g.dt();

        clear(&mut ee);
        lay_table(&mut ee, &table);
        for t in 0..4 {
            let at = WHEELS + WHEEL_SIZE * t as u32;
            ee.set_u32(OBJ + CAR_TIRES[t], at);
            ee.set_u32(at + SOLID, solid[t]);
            set_byte(&mut ee, at + CAR_SURFACE, surface[t]);
        }
        ee.set_u32(OBJ + HELD, g.word());
        *seen.borrow_mut() = None;
        ee.set(16, u64::from(OBJ));
        ee.set_f(20, dt.0);
        ee.call(CAR_FROM, &[], &[], 20_000);
        let (on, grounded, dt_bits, time_bits) = seen.borrow().expect("the stretch calls the clock");

        let mut car = Car::default();
        for t in 0..4 {
            car.tires[t].solid = solid[t] != 0;
        }
        let mut u = Update {
            vehicle_state: Some(1),
            surfaces: table.map(|(hazard, time)| Surface { hazard: hazard != 0, time }),
            tire_surface: surface,
            untamed: true,
            ..Update::default()
        };
        let mut calls = Vec::<Call>::new();
        atv_car::update(&mut u, &mut car, &mut Body::default(), dt, &mut calls);
        let clock = calls.iter().find_map(|c| match *c {
            Call::UntamedHazardClock { dt, time, sunk, grounded } => Some((u64::from(sunk), u64::from(grounded), dt.0.to_bits(), time.0.to_bits())),
            _ => None,
        });
        assert_eq!(clock, Some((on, grounded, dt_bits, time_bits)), "state {k}: solid {solid:x?}, surfaces {surface:?}");
        assert_eq!(u32::from(u.on_hazard), ee.u32_at(OBJ + HELD), "state {k}: the on-hazard word");

        let path = match (grounded != 0, on != 0) {
            (false, _) => "no tire solid",
            (true, false) => "solid, off hazard",
            (true, true) => "solid, on hazard",
        };
        *tally.entry(path).or_default() += 1;
    }
    eprintln!("MD145 car hazard: {tally:?}");
    for path in ["no tire solid", "solid, off hazard", "solid, on hazard"] {
        assert!(tally.get(path).copied().unwrap_or(0) >= 300, "too few states went {path:?}: {tally:?}");
    }
}

/// The car's node in Untamed's own rides: MD164's trophy-truck spawn and in-air images (the player's
/// truck is at a fixed heap word in both, research/journal MD164). Its parent word is 0, so Untamed's
/// node update brings it up to date as SLUS-21104's does (its own frame as it is), and the scene's
/// switch changes nothing on a car scene that holds the car's node alone.
#[test]
fn untamed_car_node_has_no_parent() {
    let Ok(kit) = Kit::load().map_err(|w| skip(&w)) else { return };
    const TRUCK: u32 = 0x01be_4740;
    const NODE_AT: u32 = 0x114;
    const PARENT: u32 = 0x120;
    let mut read = 0;
    for name in ["spawn", "air"] {
        let path = kit.ctx("untamed/md164").join(format!("{name}.ee{}", ".bin"));
        if !path.exists() {
            skip(&format!("{} is not there (MD164's Untamed trophy-truck capture)", path.display()));
            continue;
        }
        let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
        let node = ee.u32_at(TRUCK + NODE_AT);
        assert_ne!(node, 0, "{name}: the truck has a node");
        assert_eq!(ee.u32_at(node + PARENT), 0, "{name}: the truck's node has a parent");
        read += 1;
    }
    if read == 0 {
        return;
    }

    // On the port's side: a car scene's node 0 (a root) brought up to date the same both ways.
    let mut g = Gen(0x4d44_3134_3564);
    for _ in 0..200 {
        let mut scene = game_sim::atv_moto::Scene::default();
        scene.nodes[0].local = std::array::from_fn(|_| ps2_float::Vf::new(g.unit(), g.unit(), g.unit(), g.unit()));
        let mut untamed = scene;
        untamed.untamed = true;
        scene.update(0);
        untamed.update(0);
        assert_eq!(untamed.nodes, scene.nodes);
    }
}
