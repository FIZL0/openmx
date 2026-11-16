//! S337: the car's wreck step against the original's, run in the interpreter.
//!
//! `car_wreck_step` is called on a car laid out by hand in recording2's RAM image (no save state holds
//! a car), with its area, body, collision record and vtable beside it. Each case draws the map's
//! extent, the body's place, velocity and spin, the wreck words, the stuck clock and its inputs, and the
//! generator's state; the port runs [`Machine::wreck_timer`] and acts on its answer as the shell and
//! the car's post do (the edge throw [`Machine::throw_back`], the unwreck [`Machine::unwreck`]); the
//! stuck reset is only asked for, on both sides. Every word either side writes is compared to the bit.
//!
//! The image, the disc's car files and the oracle's names are git-ignored or local: absent, the test
//! skips.

use game_sim::biplane::Area;
use game_sim::car::{Car, CarEvent, Kind, Machine};
use game_sim::rng::Rng;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::cell::Cell;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::rc::Rc;

const STATE: &str = "recording2.p2m2_SaveState.eeMemory.bin";
const CASES: usize = 12_000;

/// Where the hand-made objects go: clear of the image's heap and of the interpreter's stack.
const CAR: u32 = 0x01f0_0000;
const BODY: u32 = 0x01f0_1000;
const HOLDER: u32 = 0x01f0_2000;
const TERRAIN: u32 = 0x01f0_3000;
const VTABLE: u32 = 0x01f0_4000;
const RECORD: u32 = 0x01f0_5000;

/// The car's words the wreck step reads or writes.
mod at {
    pub const VTABLE: u32 = 0x48;
    pub const BODY: u32 = 0x160;
    pub const COLLISION: u32 = 0x170;
    pub const EDGE_SCALE: u32 = 0x194;
    pub const GROUND_VELOCITY: u32 = 0x2a0;
    /// The scene holder whose `+0x3b4` is the terrain with the map's extent at `+0x530`.
    pub const HOLDER: u32 = 0x2b4;
    pub const LATCH: u32 = 0x2c0;
    pub const REASON: u32 = 0x2c4;
    pub const LIFTED: u32 = 0x2c8;
    pub const STUCK: u32 = 0x2d0;
    pub const MAX_WRECK_TIME: u32 = 0x2d4;
    pub const GAS: u32 = 0x3e0;
    pub const GROUNDED: u32 = 0x530;
    /// The vtable's reset, wreck and unwreck.
    pub const RESET_SLOT: u32 = 0x19c;
    pub const WRECK_SLOT: u32 = 0x1d0;
    pub const UNWRECK_SLOT: u32 = 0x1d4;
    /// The body's place, velocity and rates; the collision record's touching word.
    pub const POSITION: u32 = 0x60;
    pub const VELOCITY: u32 = 0x70;
    pub const RATES: u32 = 0x1b0;
    pub const TOUCHING: u32 = 0x40;
}

struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    fn one_in(&mut self, n: u64) -> bool {
        self.next() % n == 0
    }
}

/// A flat floor whose area is the case's.
struct Edged(game_sim::world::Flat, Area);

impl game_sim::world::World for Edged {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        self.0.ground(at)
    }
    fn solid(&mut self, bundle: &game_sim::collide::Bundle, record: &mut game_sim::collide::Record) -> bool {
        self.0.solid(bundle, record)
    }
    fn area(&self) -> Option<Area> {
        Some(self.1)
    }
}

/// A member of `machine.res`, by its name.
fn machine_file(kit: &Kit, name: &str) -> Option<Vec<u8>> {
    use game_sim::archive::{Psr, Res};
    let at = kit.ctx("iso").join("MX.PSR");
    let Ok(mut f) = File::open(&at) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", at.display()));
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
    let e = res.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name)).unwrap_or_else(|| panic!("no {name} in machine.res"));
    Some(res.read(&bytes, e).expect("its bytes"))
}

fn put(ee: &mut Ee, a: u32, v: Vf) {
    for k in 0..4 {
        ee.set_f32(a + 4 * k as u32, v[k]);
    }
}

fn quad(ee: &Ee, a: u32) -> [u32; 4] {
    [0, 4, 8, 12].map(|i| ee.u32_at(a + i))
}

/// One of the map's coordinates: past an edge, near one, on the margin, the middle, or anywhere.
fn coord(extent: f32, pick: &mut Pick) -> f32 {
    match pick.next() % 4 {
        0 => pick.range(-200.0, 400.0),
        1 => extent + pick.range(-400.0, 200.0),
        2 => [150.0, extent - 150.0, 0.5 * extent][(pick.next() % 3) as usize],
        _ => pick.range(0.0, extent),
    }
}

#[test]
fn the_cars_wreck_step_matches_the_original() {
    let Some(kit) = Kit::load().map_err(|w| skip(&w)).ok() else { return };
    let image = kit.ctx("states").join(STATE);
    if !image.exists() {
        skip(&format!("{} is not there: `./kit state ram` of recording2.p2m2_SaveState.p2s", image.display()));
        return;
    }
    let (Some(phy), Some(psg)) = (machine_file(&kit, Kind::Buggy.phy()), machine_file(&kit, "buggy\\anim\\buggy.psg")) else { return };
    let mut car = Car::read_bytes(&phy, Kind::Buggy);
    let model = game_sim::car::CarModel::read(&psg).unwrap();
    let names = ["car_wreck_step", "car_reset", "car_wreck", "car_unwreck", "rand_state", "boot_gp"].map(addr);
    let [Ok(step), Ok(reset), Ok(wreck), Ok(unwreck), Ok(rand_state), Ok(gp)] = names else {
        skip(&format!("research/oracle.txt lacks a name: {names:?}"));
        return;
    };
    let mut ee = Ee::from_ram(&image).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(gp));
    ee.set_u32(CAR + at::VTABLE, VTABLE);
    ee.set_u32(CAR + at::BODY, BODY);
    ee.set_u32(CAR + at::COLLISION, RECORD);
    ee.set_u32(CAR + at::HOLDER, HOLDER);
    ee.set_u32(HOLDER + 0x3b4, TERRAIN);
    ee.set_u32(VTABLE + at::RESET_SLOT, reset);
    ee.set_u32(VTABLE + at::WRECK_SLOT, wreck);
    ee.set_u32(VTABLE + at::UNWRECK_SLOT, unwreck);
    let asked = Rc::new(Cell::new(false));
    let a = asked.clone();
    ee.hooks.insert(reset, Box::new(move |_| a.set(true)));

    let mut pick = Pick(0x51f3_3a7c_0e2d_9b41);
    let mut seen = [0usize; 4];
    for case in 0..CASES {
        let extent: [f32; 4] = [3.0, 3.0, 9.0, 9.0].map(|x| if pick.one_in(2) { x } else { x * pick.range(0.25, 2.0) });
        for (k, x) in extent.iter().enumerate() {
            ee.set_f32(TERRAIN + 0x530 + 4 * k as u32, *x);
        }
        let area = Area::new(F(extent[0]), F(extent[1]), F(extent[2]), F(extent[3]));
        let (w, d) = (area.width.0, area.depth.0);
        let pos = Vf::new(coord(w, &mut pick), pick.range(-100.0, 800.0), coord(d, &mut pick), 1.0);
        let pos = if pick.one_in(200) { Vf::new(0.5 * w, pos.y(), 0.5 * d, 1.0) } else { pos };
        let vel = Vf::new(pick.range(-200.0, 200.0), pick.range(-100.0, 100.0), pick.range(-200.0, 200.0), pick.range(-1.0, 1.0));
        let rates = Vf::new(pick.range(-3.0, 3.0), pick.range(-3.0, 3.0), pick.range(-3.0, 3.0), pick.range(-1.0, 1.0));
        let ground_velocity = if pick.one_in(2) {
            Vf::new(pick.range(-8.0, 8.0), pick.range(-3.0, 3.0), pick.range(-8.0, 8.0), 1.0)
        } else {
            Vf::new(pick.range(-80.0, 80.0), pick.range(-30.0, 30.0), pick.range(-80.0, 80.0), 1.0)
        };
        let wrecked = pick.one_in(3);
        let lifted = pick.one_in(2);
        let grounded = pick.one_in(2);
        let touching = pick.one_in(2);
        let latch = pick.one_in(2);
        let gas = [0.0, 1.0, pick.unit(), -pick.unit()][(pick.next() % 4) as usize];
        let max_wreck_time = pick.range(0.0, 6.0);
        let stuck = if pick.one_in(4) { 0.0 } else { pick.range(0.0, 6.5) };
        let edge_scale = [20.0, 10.0, 40.0, pick.range(-50.0, 50.0)][(pick.next() % 4) as usize];
        let dt = [1.0 / 30.0, 1.0 / 60.0, pick.range(0.0, 0.1)][(pick.next() % 3) as usize];
        let seed = pick.next();

        put(&mut ee, BODY + at::POSITION, pos);
        put(&mut ee, BODY + at::VELOCITY, vel);
        put(&mut ee, BODY + at::RATES, rates);
        put(&mut ee, CAR + at::GROUND_VELOCITY, ground_velocity);
        ee.set_u32(CAR + at::REASON, if wrecked { 13 } else { 0 });
        ee.set_u32(CAR + at::LIFTED, lifted as u32);
        ee.set_u32(CAR + at::GROUNDED, grounded as u32);
        ee.set_u32(RECORD + at::TOUCHING, touching as u32);
        ee.set_u32(CAR + at::LATCH, latch as u32);
        ee.set_f32(CAR + at::GAS, gas);
        ee.set_f32(CAR + at::MAX_WRECK_TIME, max_wreck_time);
        ee.set_f32(CAR + at::STUCK, stuck);
        ee.set_f32(CAR + at::EDGE_SCALE, edge_scale);
        ee.write(rand_state, 8, u128::from(seed));
        asked.set(false);
        ee.call(step, &[u64::from(CAR)], &[dt], 1_000_000);

        let mut m = Machine::new(&car, &model, pos, Vf::new(0.0, 0.0, 1.0, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0));
        m.body.position = pos;
        m.body.velocity = vel;
        m.body.rates = rates;
        m.ground_velocity = ground_velocity;
        m.wrecked = wrecked;
        m.wreck_lifted = lifted;
        m.grounded = grounded;
        m.touching = touching;
        m.latch = latch;
        m.controls.gas = F(gas);
        m.stuck_time = F(stuck);
        m.edge_scale = F(edge_scale);
        car.max_wreck_time = F(max_wreck_time);
        let mut rng = Rng::from_state(seed);
        let world = Edged(game_sim::world::Flat::default(), area);
        let event = m.wreck_timer(&car, &world, F(dt));
        match event {
            Some(CarEvent::Thrown) => m.throw_back(&area, &mut rng),
            Some(CarEvent::Unwreck) => m.unwreck(),
            _ => {}
        }
        seen[match event {
            None => 0,
            Some(CarEvent::Thrown) => 1,
            Some(CarEvent::Unwreck) => 2,
            Some(CarEvent::Reset) => 3,
        }] += 1;

        let ctx = || format!("case {case}: {event:?} at {pos:?} on {w}x{d}, vel {vel:?}, wrecked {wrecked} lifted {lifted} grounded {grounded} touching {touching}");
        assert_eq!(asked.get(), event == Some(CarEvent::Reset), "{}: the reset", ctx());
        assert_eq!(quad(&ee, BODY + at::VELOCITY), m.body.velocity.to_bits(), "{}: velocity {:?}", ctx(), m.body.velocity);
        assert_eq!(quad(&ee, BODY + at::RATES), m.body.rates.to_bits(), "{}: rates", ctx());
        assert_eq!(quad(&ee, BODY + at::POSITION), pos.to_bits(), "{}: the step moved the body", ctx());
        let words = [
            ("edge scale", ee.u32_at(CAR + at::EDGE_SCALE), m.edge_scale.to_bits()),
            ("reason", ee.u32_at(CAR + at::REASON), if m.wrecked { 13 } else { 0 }),
            ("lifted", ee.u32_at(CAR + at::LIFTED), m.wreck_lifted as u32),
            ("latch", ee.u32_at(CAR + at::LATCH), m.latch as u32),
            ("stuck clock", ee.u32_at(CAR + at::STUCK), m.stuck_time.to_bits()),
        ];
        for (name, want, got) in words {
            assert_eq!(got, want, "{}: {name}", ctx());
        }
        assert_eq!(ee.read(rand_state, 8) as u64, rng.state(), "{}: the generator", ctx());
    }
    println!("car wreck step: {CASES} cases, none/thrown/unwreck/reset {seen:?}");
    assert!(seen.iter().all(|&n| n > 100), "every branch is reached: {seen:?}");
}
