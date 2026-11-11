//! MD106 — MX vs. ATV's cars' physics frame, proved against SLUS-21104's own code.
//!
//! MD39a's buggy image (`context/mxvatv-ps2/md39a/buggy.ee.bin`, the player's buggy at its Desert
//! Oasis spawn) holds the game overlay, so the physics world's step (`atv_world_step`) runs in the
//! interpreter over the game's own world: the car's three listeners, its one rigid body, and the
//! other vehicles' PreProcess listeners. Every call the car's listeners make ([`Call`]) is hooked to
//! log what it was handed and play a generated script (the tires' contact flags, grounds and loads,
//! the ground's normal, the node's position), which the port's [`Steps`] plays the same way; each
//! generated state is poked into the image's car, tires and body on both sides. Then every word
//! [`atv_car::frame`] keeps of the car, its tires and its body equals the original's, the two call
//! logs are equal, and the original wrote nothing else in those blocks.
//!
//! The car's update (MD106d), which runs outside the world step, is proved the same way: SLUS-21104's
//! update called on the image's car with its four calls hooked, against [`atv_car::update`].
//!
//! Without the image every test here skips (and says so).

use game_sim::air::AcosTable;
use game_sim::atv_body::Body;
use game_sim::atv_car::{self, Call, Car, Keys, Pieces, PreloadState, State, Steps, Tire, Update, Wing};
use game_sim::biplane::World;
use game_sim::car::Reverse;
use game_sim::handling::seed;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};
use std::cell::RefCell;
use std::rc::Rc;

/// The player's buggy (research/oracle.txt `# MD39a`), its class's vtable, the physics world and
/// SLUS-21104's `$gp`.
const CAR: u32 = 0x0159_8e10;
const BUGGY_VTABLE: u32 = 0x003c_ec60;
const WORLD: u32 = 0x0065_bc0c;
const GP: u64 = 0x003d_baf0;
const WORLD_STEP: u32 = 0x0049_6b70;

/// The car's calls: its virtuals as the buggy's vtable has them, and the plain calls.
const CONTROLS: u32 = 0x004c_a450;
const TRAVEL_COFG: u32 = 0x004c_b000;
const SORT_CONTACTS: u32 = 0x004c_ad90;
const PROBE: u32 = 0x004b_51f0;
const VEHICLE_FORCE: u32 = 0x004a_4ef0;
const ANTI_SLIDE: u32 = 0x004c_b500;
const PRELOAD: u32 = 0x004c_b7f0;
const LAUNCH: u32 = 0x004c_ba30;
const ENGINE_STEP: u32 = 0x004b_3f30;
const TIRE_STEP: u32 = 0x004b_5ca0;
const AERO: u32 = 0x004c_b200;
const VEHICLE_POST: u32 = 0x004a_4f00;
const SHIFT: u32 = 0x004b_3c90;
const WRECK_TIMER: u32 = 0x004c_d0b0;
const NODE_LOCAL: u32 = 0x0019_5320;
const LANDING_SOUND: u32 = 0x0050_0820;
const GROUND_QUERY: u32 = 0x0048_33c0;
const NODE_POSITION: u32 = 0x0019_4c20;
/// The engine's reverse switch (a0 engine, a1 into reverse), which the controls call.
const REVERSE: u32 = 0x004b_3c20;

/// Where the car's PostProcess makes its last three calls from (their return addresses).
const POST_LANDED: [u32; 2] = [0x004c_bf60, 0x004c_bf88];
const POST_GROUND: u32 = 0x004c_c428;
const POST_NODE: u32 = 0x004c_c4c0;

/// The car's words (research/oracle.txt `# MD106`).
mod at {
    pub const BODY: u32 = 0xb8;
    pub const RECORD: u32 = 0xc8;
    pub const NODE: u32 = 0xe4;
    pub const GROUNDED: u32 = 0x324;
    pub const ONE_SIDE: u32 = 0x328;
    pub const LATCH: u32 = 0x31c;
    pub const LATCHED: u32 = 0x320;
    pub const SPEED: u32 = 0x32c;
    pub const SIDE_SLIP: u32 = 0x330;
    pub const ALPHA: u32 = 0x334;
    pub const WORLD_RATES: u32 = 0x340;
    pub const VELOCITY: u32 = 0x350;
    pub const EULER: u32 = 0x360;
    pub const MATRIX: u32 = 0x370;
    pub const GROUND_WORLD: u32 = 0x3b4;
    pub const FORWARD: u32 = 0xbe0;
    pub const UP: u32 = 0xbf0;
    pub const GROUND_VELOCITY: u32 = 0xc30;
    pub const NODE_POS: u32 = 0xc40;
    pub const STATE: u32 = 0xc80;
    pub const WAS_GROUNDED: u32 = 0x1240;
    pub const YAW_RATE_LIMIT: u32 = 0x124c;
    pub const ENGINE: u32 = 0x1250;
    pub const TIRES: [u32; 4] = [0x1254, 0x1258, 0x125c, 0x1260];
    pub const LANDING_SOUND: u32 = 0x12b8;
    pub const LATCH_COUNT: u32 = 0x12c4;
    /// The record's tipped word.
    pub const TIPPED: u32 = 0x24;
    /// The body's centre of mass in its own frame.
    pub const COFG: u32 = 0x1a0;
    /// A tire's.
    pub const SOLID: u32 = 0xa4;
    pub const SORTED: u32 = 0xa8;
    pub const TIRE_GROUNDED: u32 = 0xb0;
    pub const LONE_FRONT: u32 = 0xb4;
    pub const HANDED: u32 = 0xb8;
    pub const HANDED_LOAD: u32 = 0xbc;
    pub const HANDED_ON: u32 = 0xc0;
    pub const LOAD: u32 = 0xd4;
    pub const LOCAL_VELOCITY: u32 = 0x180;
    pub const STANDS_ON: u32 = 0x19c;
}

fn image() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2/md39a").join(format!("buggy.ee{}", ".bin"));
    if !path.exists() {
        skip(&format!(
            "{} is not there: MD39a's buggy (C29 remade it; a copy is in openmx-slots/kept-context/md39a)",
            path.display()
        ));
        return None;
    }
    let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    if ee.u32_at(CAR + 4) != BUGGY_VTABLE || ee.u32_at(ee.u32_at(WORLD + 0xb8)) != CAR + 0x34 {
        panic!("{} is not the MX vs. ATV buggy image these addresses are from", path.display());
    }
    Some(ee)
}

fn get_v(ee: &Ee, a: u32) -> Vf {
    Vf(core::array::from_fn(|k| ee.f32_at(a + 4 * k as u32)))
}
fn put_v(ee: &mut Ee, a: u32, v: Vf) {
    for (k, w) in v.to_bits().into_iter().enumerate() {
        ee.set_u32(a + 4 * k as u32, w);
    }
}
fn word(b: bool) -> u32 {
    u32::from(b)
}

/// The image's car's pointers.
#[derive(Clone, Copy)]
struct Ptrs {
    body: u32,
    record: u32,
    node: u32,
    engine: u32,
    tires: [u32; 4],
    ground_world: u32,
    landing_sound: u32,
}

fn ptrs(ee: &Ee) -> Ptrs {
    Ptrs {
        body: ee.u32_at(CAR + at::BODY),
        record: ee.u32_at(CAR + at::RECORD),
        node: ee.u32_at(CAR + at::NODE),
        engine: ee.u32_at(CAR + at::ENGINE),
        tires: at::TIRES.map(|o| ee.u32_at(CAR + o)),
        ground_world: ee.u32_at(CAR + at::GROUND_WORLD),
        landing_sound: ee.u32_at(CAR + at::LANDING_SOUND),
    }
}

/// The car as [`atv_car`] holds it, read out of RAM. The ground's normal is the query's answer and
/// lives on the stack: it reads as zero.
fn read_car(ee: &Ee, p: &Ptrs) -> Car {
    let c = |o: u32| CAR + o;
    let b = |o: u32| ee.u32_at(c(o)) != 0;
    let f = |o: u32| F(ee.f32_at(c(o)));
    let v = |o: u32| get_v(ee, c(o));
    Car {
        state: ee.u32_at(c(at::STATE)),
        yaw_rate_limit: f(at::YAW_RATE_LIMIT),
        was_grounded: b(at::WAS_GROUNDED),
        grounded: b(at::GROUNDED),
        one_side: b(at::ONE_SIDE),
        landing_sound: b(at::LANDING_SOUND),
        speed: f(at::SPEED),
        velocity: v(at::VELOCITY),
        world_rates: v(at::WORLD_RATES),
        ground_velocity: v(at::GROUND_VELOCITY),
        alpha: f(at::ALPHA),
        side_slip: f(at::SIDE_SLIP),
        euler: v(at::EULER),
        matrix: core::array::from_fn(|k| v(at::MATRIX + 0x10 * k as u32)),
        forward: v(at::FORWARD),
        up: v(at::UP),
        cofg: get_v(ee, p.body + at::COFG),
        ground_normal: Vf::ZERO,
        tipped: ee.u32_at(p.record + at::TIPPED) != 0,
        latch: b(at::LATCH),
        latched: b(at::LATCHED),
        latch_count: ee.i32_at(c(at::LATCH_COUNT)),
        node_pos: v(at::NODE_POS),
        tires: p.tires.map(|t| read_tire(ee, t)),
    }
}

fn read_tire(ee: &Ee, t: u32) -> Tire {
    let b = |o: u32| ee.u32_at(t + o) != 0;
    Tire {
        solid: b(at::SOLID),
        sorted: b(at::SORTED),
        stands_on: [ee.u32_at(t + at::STANDS_ON), ee.u32_at(t + at::STANDS_ON + 4)],
        grounded: b(at::TIRE_GROUNDED),
        lone_front: b(at::LONE_FRONT),
        handed: b(at::HANDED),
        handed_load: F(ee.f32_at(t + at::HANDED_LOAD)),
        handed_on: [ee.u32_at(t + at::HANDED_ON), ee.u32_at(t + at::HANDED_ON + 4)],
        load: F(ee.f32_at(t + at::LOAD)),
        local_velocity: get_v(ee, t + at::LOCAL_VELOCITY),
    }
}

/// Every word [`read_car`] reads, as the port holds it, where it lives; the landing sound's word is
/// the image's pointer (or zero).
fn car_words(car: &Car, p: &Ptrs) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let mut w = |a: u32, x: u32| out.push((a, x));
    let c = |o: u32| CAR + o;
    w(c(at::STATE), car.state);
    w(c(at::YAW_RATE_LIMIT), car.yaw_rate_limit.to_bits());
    w(c(at::WAS_GROUNDED), word(car.was_grounded));
    w(c(at::GROUNDED), word(car.grounded));
    w(c(at::ONE_SIDE), word(car.one_side));
    w(c(at::LANDING_SOUND), if car.landing_sound { p.landing_sound } else { 0 });
    w(c(at::SPEED), car.speed.to_bits());
    w(c(at::ALPHA), car.alpha.to_bits());
    w(c(at::SIDE_SLIP), car.side_slip.to_bits());
    w(p.record + at::TIPPED, word(car.tipped));
    w(c(at::LATCH), word(car.latch));
    w(c(at::LATCHED), word(car.latched));
    w(c(at::LATCH_COUNT), car.latch_count as u32);
    let mut vs: Vec<(u32, Vf)> = vec![
        (c(at::VELOCITY), car.velocity),
        (c(at::WORLD_RATES), car.world_rates),
        (c(at::GROUND_VELOCITY), car.ground_velocity),
        (c(at::EULER), car.euler),
        (c(at::FORWARD), car.forward),
        (c(at::UP), car.up),
        (c(at::NODE_POS), car.node_pos),
        (p.body + at::COFG, car.cofg),
    ];
    for k in 0..4u32 {
        vs.push((c(at::MATRIX + 0x10 * k), car.matrix[k as usize]));
    }
    for (t, tire) in p.tires.iter().zip(&car.tires) {
        w(t + at::SOLID, word(tire.solid));
        w(t + at::SORTED, word(tire.sorted));
        w(t + at::STANDS_ON, tire.stands_on[0]);
        w(t + at::STANDS_ON + 4, tire.stands_on[1]);
        w(t + at::TIRE_GROUNDED, word(tire.grounded));
        w(t + at::LONE_FRONT, word(tire.lone_front));
        w(t + at::HANDED, word(tire.handed));
        w(t + at::HANDED_LOAD, tire.handed_load.to_bits());
        w(t + at::HANDED_ON, tire.handed_on[0]);
        w(t + at::HANDED_ON + 4, tire.handed_on[1]);
        w(t + at::LOAD, tire.load.to_bits());
        vs.push((t + at::LOCAL_VELOCITY, tire.local_velocity));
    }
    for (a, v) in vs {
        for (k, x) in v.to_bits().into_iter().enumerate() {
            w(a + 4 * k as u32, x);
        }
    }
    out
}

fn poke(ee: &mut Ee, words: &[(u32, u32)]) {
    for &(a, x) in words {
        ee.set_u32(a, x);
    }
}

/// xorshift, so a failure names a seed that repeats.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.unit()
    }
    fn word(&mut self) -> u32 {
        self.next() as u32
    }
    fn vec(&mut self, r: f32, w: f32) -> Vf {
        Vf::new(self.range(-r, r), self.range(-r, r), self.range(-r, r), w)
    }
    fn quat(&mut self) -> Vf {
        let v = [self.range(-1.0, 1.0), self.range(-1.0, 1.0), self.range(-1.0, 1.0), self.range(-1.0, 1.0)];
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-3);
        Vf::new(v[0] / n, v[1] / n, v[2] / n, v[3] / n)
    }
}

/// The ground normal's `y` that puts a level car's up row exactly on the tipped edge.
const EDGE: u32 = 0x3f34_fdf4;

/// What the hooked calls do, on both sides: each probe leaves its tire's two contact flags, each
/// tire step its grounded flag, load, ground and contact velocity (by the tire and how many times it
/// has stepped), and the two queries answer the ground's normal and the node's position.
#[derive(Clone, Default)]
struct Script {
    probe: [(bool, bool); 4],
    steps: [[(bool, F, [u32; 2], Vf); 8]; 4],
    stepped: [usize; 4],
    normal: Vf,
    node_pos: Vf,
}

impl Script {
    fn new(r: &mut Rng) -> Script {
        let mut s = Script::default();
        let down = r.range(0.2, 0.9);
        for k in 0..4 {
            s.probe[k] = (r.chance(0.4), r.chance(0.3));
            for j in 0..8 {
                let on = [if r.chance(0.5) { 0 } else { r.word() }, if r.chance(0.7) { 0 } else { r.word() }];
                // Ties now and then, signed zeros among them: the hand-off's max settles +0 against
                // -0 as +0, where a compare and select would take one front's.
                let load = if r.chance(0.1) {
                    F(500.0)
                } else if r.chance(0.3) {
                    F(if r.chance(0.5) { -0.0 } else { 0.0 })
                } else {
                    F(r.range(0.0, 900.0))
                };
                let w = r.range(-1.0, 1.0);
                s.steps[k][j] = (r.chance(down), load, on, r.vec(40.0, w));
            }
        }
        let n = r.vec(1.0, 0.0);
        let (x, z) = (r.range(-1.0, 1.0), r.range(-1.0, 1.0));
        s.normal = if r.chance(0.3) {
            Vf::new(0.0, 1.0, 0.0, 0.0)
        } else if r.chance(0.3) {
            // Under a level car, exactly on the tipped edge.
            Vf::new(x, f32::from_bits(EDGE), z, 0.0)
        } else {
            n
        };
        s.node_pos = r.vec(2000.0, 1.0);
        s
    }

    fn probe(&self, k: usize, t: &mut Tire) {
        (t.solid, t.sorted) = self.probe[k];
    }

    fn step(&mut self, k: usize, t: &mut Tire) {
        let (g, load, on, v) = self.steps[k][self.stepped[k]];
        self.stepped[k] += 1;
        (t.grounded, t.load, t.stands_on, t.local_velocity) = (g, load, on, v);
    }
}

/// The port's side of the hooked calls.
struct Port {
    log: Vec<Call>,
    script: Script,
}

impl Steps for Port {
    fn call(&mut self, c: Call, car: &mut Car, _body: &mut Body) {
        match c {
            Call::Probe(k, _) => self.script.probe(k, &mut car.tires[k]),
            Call::TireStep(k, _) => self.script.step(k, &mut car.tires[k]),
            Call::GroundUnder(_) => car.ground_normal = self.script.normal,
            Call::NodePosition => car.node_pos = self.script.node_pos,
            _ => {}
        }
        self.log.push(c);
    }
}

type Shared<T> = Rc<RefCell<T>>;

/// Hook every call the car's listeners make: log it in the port's terms and play the script. A call
/// on anything but the player's car (the idle buggy's PreProcess makes the same ones) is skipped and
/// not logged; one of the plain calls from anywhere but the car's listeners fails the test.
fn hook(ee: &mut Ee, p: Ptrs, log: Shared<Vec<Call>>, script: Shared<Script>) {
    let tire = move |ee: &Ee| p.tires.iter().position(|&t| t == ee.r(4) as u32);
    let dt = |ee: &Ee| F(ee.f(12));
    type Make = fn(&Ee, Option<usize>) -> Option<Call>;
    let on_car: [(u32, Make); 10] = [
        (CONTROLS, |ee, _| Some(Call::Controls(F(ee.f(12))))),
        (TRAVEL_COFG, |_, _| Some(Call::TravelCofg)),
        (SORT_CONTACTS, |_, _| Some(Call::SortContacts)),
        (VEHICLE_FORCE, |_, _| Some(Call::VehicleForce)),
        (ANTI_SLIDE, |ee, _| Some(Call::AntiSlide(F(ee.f(12))))),
        (PRELOAD, |ee, _| Some(Call::Preload(F(ee.f(12))))),
        (LAUNCH, |ee, _| Some(Call::Launch(F(ee.f(12))))),
        (AERO, |ee, _| Some(Call::Aero(F(ee.f(12))))),
        (VEHICLE_POST, |ee, _| Some(Call::VehiclePost(F(ee.f(12))))),
        (WRECK_TIMER, |ee, _| Some(Call::WreckTimer(F(ee.f(12))))),
    ];
    for (addr, make) in on_car {
        let l = log.clone();
        ee.hooks.insert(addr, Box::new(move |ee| {
            if ee.r(4) as u32 == CAR {
                l.borrow_mut().push(make(ee, None).unwrap());
            }
        }));
    }
    for (addr, is_step) in [(PROBE, false), (TIRE_STEP, true)] {
        let (l, s) = (log.clone(), script.clone());
        ee.hooks.insert(addr, Box::new(move |ee| {
            let Some(k) = tire(ee) else { return };
            let mut t = read_tire(ee, p.tires[k]);
            let mut s = s.borrow_mut();
            let c = if is_step {
                s.step(k, &mut t);
                Call::TireStep(k, dt(ee))
            } else {
                s.probe(k, &mut t);
                Call::Probe(k, dt(ee))
            };
            let mut car = Car::default();
            car.tires[k] = t;
            for (a, x) in car_words(&car, &p) {
                if (p.tires[k]..p.tires[k] + 0x200).contains(&a) {
                    ee.set_u32(a, x);
                }
            }
            l.borrow_mut().push(c);
        }));
    }
    for (addr, is_shift) in [(ENGINE_STEP, false), (SHIFT, true)] {
        let l = log.clone();
        ee.hooks.insert(addr, Box::new(move |ee| {
            if ee.r(4) as u32 == p.engine {
                l.borrow_mut().push(if is_shift { Call::Shift(dt(ee)) } else { Call::Engine(dt(ee)) });
            }
        }));
    }
    let l = log.clone();
    ee.hooks.insert(REVERSE, Box::new(move |ee| {
        if ee.r(4) as u32 == p.engine {
            l.borrow_mut().push(Call::Reverse(if ee.r(5) != 0 { Reverse::In } else { Reverse::Out }));
        }
    }));
    let l = log.clone();
    ee.hooks.insert(NODE_LOCAL, Box::new(move |ee| {
        assert!(POST_LANDED.contains(&(ee.r(31) as u32)), "node local from {:#x}", ee.r(31));
        assert_eq!(ee.r(5) as u32, p.node, "the landing's node");
        l.borrow_mut().push(Call::Landed(get_v(ee, ee.r(6) as u32)));
    }));
    ee.hooks.insert(LANDING_SOUND, Box::new(move |ee| {
        assert_eq!((ee.r(4) as u32, ee.r(5)), (p.landing_sound, 0), "the landing sound's object and kind");
    }));
    let (l, s) = (log.clone(), script.clone());
    ee.hooks.insert(GROUND_QUERY, Box::new(move |ee| {
        assert_eq!(ee.r(31) as u32, POST_GROUND, "a ground query from elsewhere");
        assert_eq!((ee.r(4) as u32, ee.r(7), ee.r(8)), (p.ground_world, 0, 0), "the ground query's world, fourth and record");
        l.borrow_mut().push(Call::GroundUnder(get_v(ee, ee.r(5) as u32)));
        put_v(ee, ee.r(6) as u32, s.borrow().normal);
    }));
    let (l, s) = (log, script);
    ee.hooks.insert(NODE_POSITION, Box::new(move |ee| {
        assert_eq!(ee.r(31) as u32, POST_NODE, "a node position from elsewhere");
        assert_eq!((ee.r(4) as u32, ee.r(5), ee.r(6) as u32), (p.node, 0, CAR + at::NODE_POS), "the node position's node and out");
        put_v(ee, CAR + at::NODE_POS, s.borrow().node_pos);
        l.borrow_mut().push(Call::NodePosition);
    }));
}

/// A generated state: the image's car with its flags, speeds and frame stirred, and its body moving.
fn generate(r: &mut Rng, image_car: &Car, image_body: &Body, p: &Ptrs) -> (Car, Body, F) {
    let mut car = *image_car;
    car.state = [0, 0, 0, 13, 14, 5][(r.next() % 6) as usize];
    car.yaw_rate_limit = F(r.range(0.5, 12.0));
    car.was_grounded = r.chance(0.5);
    car.grounded = r.chance(0.5);
    car.one_side = r.chance(0.5);
    car.landing_sound = p.landing_sound != 0 && r.chance(0.7);
    car.latch = r.chance(0.5);
    car.latched = r.chance(0.5);
    car.latch_count = (r.next() % 5) as i32;
    car.forward.0[3] = r.range(-2.0, 2.0);
    car.up.0[3] = r.range(-2.0, 2.0);
    car.cofg = r.vec(1.5, 1.0);
    for t in &mut car.tires {
        t.solid = r.chance(0.5);
        t.sorted = r.chance(0.5);
        t.lone_front = r.chance(0.5);
        t.handed = r.chance(0.5);
        t.handed_load = F(r.range(0.0, 900.0));
        t.handed_on = [r.word(), r.word()];
    }
    let mut body = *image_body;
    let reach = if r.chance(0.1) { 0.5 } else { 70.0 };
    body.velocity = r.vec(reach, 0.0);
    body.rates = r.vec(8.0, 0.0);
    if r.chance(0.3) {
        // Past the yaw-rate limit, either way.
        body.rates.0[1] = car.yaw_rate_limit.0 * r.range(1.0, 2.0) * if r.chance(0.5) { -1.0 } else { 1.0 };
    }
    body.quat = if r.chance(0.2) { Vf::new(0.0, 0.0, 0.0, 1.0) } else { r.quat() };
    body.last_quat = body.quat;
    body.position = Vf::new(
        image_body.position.x() + r.range(-50.0, 50.0),
        image_body.position.y() + r.range(-5.0, 5.0),
        image_body.position.z() + r.range(-50.0, 50.0),
        1.0,
    );
    body.force = r.vec(3000.0, 0.0);
    body.moment = r.vec(500.0, 0.0);
    if r.chance(0.15) {
        // Level and not turning, so the up row stays (0, 1, 0).
        body.quat = Vf::new(0.0, 0.0, 0.0, 1.0);
        body.last_quat = body.quat;
        body.rates = Vf::ZERO;
        body.moment = Vf::ZERO;
    }
    body.correction = if r.chance(0.5) { Vf::ZERO } else { r.vec(200.0, 0.0) };
    body.correction_peak = r.vec(50.0, 0.0);
    let dt = if r.chance(0.7) { F(1.0 / 30.0) } else { F(r.range(0.002, 0.05)) };
    (car, body, dt)
}

/// The MX Unleashed pieces' words (research/oracle.txt `# MD106a`; the keys' are MD39b's): the
/// car's, but for the vehicle state's mode, the gravity block's, the record's touching word, the left
/// front tire's ground normal, the engine's torque scale and a wing's (from its first word).
mod mx {
    pub const VEHICLE_STATE: u32 = 0xbc4;
    pub const MODE: u32 = 0x28;
    pub const LEAN: u32 = 0x11cc;
    pub const STICK: u32 = 0x11c8;
    pub const PAD_WORD: u32 = 0x1124;
    pub const CLUTCH: u32 = 0x1120;
    pub const GAS: u32 = 0x318;
    pub const VEHICLE_BRAKE: u32 = 0x310;
    pub const BRAKE: u32 = 0x1130;
    pub const BRAKE_FORCE: u32 = 0x1134;
    pub const HAND_BRAKE: u32 = 0x1138;
    pub const GRAVITY_BLOCK: u32 = 0xe8;
    pub const GRAVITY: u32 = 0x40;
    pub const TOUCHING: u32 = 0x34;
    pub const FRONT_NORMAL: u32 = 0x110;
    pub const READY: u32 = 0xc50;
    pub const ACTIVE: u32 = 0xc54;
    pub const ARMED: u32 = 0xc58;
    pub const PRELOAD_TIME: u32 = 0xc5c;
    pub const TIMER: u32 = 0xc60;
    pub const MIN_PRELOAD_TIME: u32 = 0xc64;
    pub const MAX_PRELOAD_TIME: u32 = 0xc68;
    pub const MAX_PRELOAD: u32 = 0xc6c;
    pub const FORCE: u32 = 0xc70;
    pub const CUED: u32 = 0x700;
    pub const LAUNCH_TIMER: u32 = 0x1168;
    pub const LAUNCH_START: u32 = 0x116c;
    pub const CLUTCH_TIME: u32 = 0x1170;
    pub const MAX_SCALE: u32 = 0x1174;
    pub const SCALE_SPEED: u32 = 0x1178;
    pub const BOOST: u32 = 0x60;
    pub const COFG: u32 = 0x10f0;
    pub const COFG_AI: u32 = 0x1100;
    pub const TRAVEL_FORWARD: u32 = 0x1110;
    pub const TRAVEL_BACK: u32 = 0x1114;
    pub const AREA: u32 = 0x117c;
    pub const DAMPING: u32 = 0x1180;
    pub const UNROLL_GAIN: u32 = 0x1184;
    pub const UNROLL_TORQUE: u32 = 0x1188;
    pub const UNYAW_GAIN: u32 = 0x118c;
    pub const UNYAW_TORQUE: u32 = 0x1190;
    pub const CONTROL_MOMENT: u32 = 0x11a0;
    pub const GYRO: u32 = 0x11b0;
    pub const WINGS: u32 = 0x11b4;
    pub const WING_PTR: u32 = 0x11b8;
    // The controls' (research/oracle.txt `# MD126`).
    pub const WHEEL: u32 = 0x54;
    pub const PENALTY_OBJECT: u32 = 0x680;
    pub const PENALTY: u32 = 0x1d0;
    pub const DRIVEN: u32 = 0x11bc;
    pub const SQUARE: u32 = 0x11c0;
    pub const DPAD: u32 = 0x11c4;
    pub const GAS_PEDAL: u32 = 0x314;
    pub const STEER_INPUT: u32 = 0x11d0;
    pub const STEER: u32 = 0x113c;
    pub const REAR_STEER: u32 = 0x1140;
    pub const ENGAGED_CLUTCH: u32 = 0x112c;
    pub const FADE: u32 = 0x1164;
    pub const REAR_FRICTION: u32 = 0x9a4;
    pub const HELD: u32 = 0xd7c;
    pub const THROTTLE_ENABLED: u32 = 0xdd4;
    pub const REVERSE_ENABLED: u32 = 0xdd8;
    pub const REVERSING: u32 = 0xbc;
    pub const RATES: u32 = 0x11d4;
    pub const TABLES: u32 = 0x11e8;
    pub const REAR_TABLE: u32 = 0x1230;
    pub const YAW_LIMIT: u32 = 0x1244;
    pub const HAND_BRAKE_YAW_LIMIT: u32 = 0x1248;
    pub const FRICTION_SCALE: u32 = 0x1158;
    pub const BRAKE_SCALE: u32 = 0x115c;
    pub const FADE_TIME: u32 = 0x1160;
}

/// The preload's cue (a sound's play), the global it is handed first, and where the preload calls it.
const CUE: u32 = 0x0027_2300;
const CUE_GLOBAL: u32 = 0x0044_2eb0;
const CUE_FROM: u32 = 0x004c_b99c;

/// The pieces' keys and state as the image holds them: [`Keys`] with its wing count as written (the
/// image's one wing kept in RAM whatever the count), [`State`], and the vehicle state's pointer (the
/// image's object, or none) and mode.
#[derive(Clone)]
struct Mx {
    keys: Keys,
    wing: Wing,
    wing_count: i32,
    wing_ptr: u32,
    vstate_object: u32,
    vstate: bool,
    mode: u32,
    /// The vehicle state's wheel word, the start penalty's object (the image's, or none) and its clock.
    wheel_word: u32,
    penalty_object: u32,
    penalty: bool,
    penalty_clock: F,
    state: State,
}

impl Mx {
    fn keys(&self) -> Keys {
        Keys { wings: if self.wing_count > 0 { vec![self.wing; self.wing_count as usize] } else { Vec::new() }, ..self.keys.clone() }
    }
}

fn read_mx(ee: &Ee, p: &Ptrs) -> Mx {
    let c = |o: u32| CAR + o;
    let f = |o: u32| F(ee.f32_at(c(o)));
    let v = |o: u32| get_v(ee, c(o));
    let b = |o: u32| ee.u32_at(c(o)) != 0;
    let vstate_object = ee.u32_at(c(mx::VEHICLE_STATE));
    assert_ne!(vstate_object, 0, "the image's vehicle state");
    let mode = ee.u32_at(vstate_object + mx::MODE);
    let wing_ptr = ee.u32_at(c(mx::WING_PTR));
    let wf = |o: u32| F(ee.f32_at(wing_ptr + o));
    let wing = Wing { lift_slope: wf(0), angle_of_attack: wf(4), basic_drag: wf(8), induced_drag: wf(0xc), position: get_v(ee, wing_ptr + 0x10) };
    let ff = |a: u32| F(ee.f32_at(a));
    let table = |a: u32| -> [[F; 2]; 3] { core::array::from_fn(|i| [ff(a + 8 * i as u32), ff(a + 8 * i as u32 + 4)]) };
    let rate = |i: u32| f(mx::RATES + 4 * i);
    let keys = Keys {
        square_steering: b(mx::SQUARE),
        brake_rate_limit: rate(0),
        gas_rate_limit: rate(1),
        x_dpad_rate_limit_on: rate(2),
        x_dpad_rate_limit_off: rate(3),
        x_axis_rate_limit: rate(4),
        steer: table(c(mx::TABLES)),
        wheel_steer: table(c(mx::TABLES + 0x18)),
        dpad_steer: table(c(mx::TABLES + 0x30)),
        rear_steer: {
            let [a, b, _] = table(c(mx::REAR_TABLE));
            [a, b]
        },
        yaw_rate_limit: f(mx::YAW_LIMIT),
        hand_brake_yaw_rate_limit: f(mx::HAND_BRAKE_YAW_LIMIT),
        hand_brake_friction_scale: f(mx::FRICTION_SCALE),
        hand_brake_brake_scale: f(mx::BRAKE_SCALE),
        hand_brake_fade_time: f(mx::FADE_TIME),
        cofg: v(mx::COFG),
        cofg_ai: v(mx::COFG_AI),
        cofg_travel_forward: f(mx::TRAVEL_FORWARD),
        cofg_travel_back: f(mx::TRAVEL_BACK),
        preload_time: f(mx::PRELOAD_TIME),
        min_preload_time: f(mx::MIN_PRELOAD_TIME),
        max_preload_time: f(mx::MAX_PRELOAD_TIME),
        max_preload: f(mx::MAX_PRELOAD),
        clutch_torque_scale_time: f(mx::CLUTCH_TIME),
        max_clutch_torque_scale: f(mx::MAX_SCALE),
        max_clutch_torque_scale_speed: f(mx::SCALE_SPEED),
        aero_surface_area: f(mx::AREA),
        airborne_damping: f(mx::DAMPING),
        unroll_gain: f(mx::UNROLL_GAIN),
        unroll_torque: f(mx::UNROLL_TORQUE),
        unyaw_gain: f(mx::UNYAW_GAIN),
        unyaw_torque: f(mx::UNYAW_TORQUE),
        airborne_control_moment: v(mx::CONTROL_MOMENT),
        brake_gyro_torque: f(mx::GYRO),
        wings: Vec::new(),
    };
    let wheel_word = ee.u32_at(vstate_object + mx::WHEEL);
    let penalty_object = ee.u32_at(c(mx::PENALTY_OBJECT));
    assert_ne!(penalty_object, 0, "the image's start penalty");
    let penalty_clock = F(ee.f32_at(penalty_object + mx::PENALTY));
    let state = State {
        human: mode == 1,
        driven: b(mx::DRIVEN),
        wheel: wheel_word != 0,
        dpad: b(mx::DPAD),
        start_penalty: penalty_clock > 0.0,
        gas_pedal: f(mx::GAS_PEDAL),
        steer_input: f(mx::STEER_INPUT),
        steer: f(mx::STEER),
        rear_steer: f(mx::REAR_STEER),
        engaged_clutch: f(mx::ENGAGED_CLUTCH),
        hand_brake_fade: f(mx::FADE),
        rear_friction: [2, 3].map(|t| F(ee.f32_at(p.tires[t] + mx::REAR_FRICTION))),
        held: b(mx::HELD),
        reverse_enabled: b(mx::REVERSE_ENABLED),
        throttle_enabled: b(mx::THROTTLE_ENABLED),
        reversing: ee.u32_at(p.engine + mx::REVERSING) != 0,
        lean: f(mx::LEAN),
        stick: f(mx::STICK),
        pad_word: f(mx::PAD_WORD),
        clutch: f(mx::CLUTCH),
        gas: f(mx::GAS),
        vehicle_brake: f(mx::VEHICLE_BRAKE),
        brake: f(mx::BRAKE),
        brake_force: f(mx::BRAKE_FORCE),
        hand_brake: f(mx::HAND_BRAKE),
        gravity: get_v(ee, ee.u32_at(c(mx::GRAVITY_BLOCK)) + mx::GRAVITY),
        touching: ee.u32_at(p.record + mx::TOUCHING) != 0,
        front_normal: get_v(ee, p.tires[0] + mx::FRONT_NORMAL),
        preload: PreloadState {
            ready: b(mx::READY),
            active: b(mx::ACTIVE),
            armed: b(mx::ARMED),
            timer: f(mx::TIMER),
            force: v(mx::FORCE),
            cued: b(mx::CUED),
        },
        launch_timer: f(mx::LAUNCH_TIMER),
        launch_start: f(mx::LAUNCH_START),
        launch_boost: F(ee.f32_at(p.engine + mx::BOOST)),
    };
    Mx {
        keys,
        wing,
        wing_count: ee.i32_at(c(mx::WINGS)),
        wing_ptr,
        vstate_object,
        vstate: true,
        mode,
        wheel_word,
        penalty_object,
        penalty: true,
        penalty_clock,
        state,
    }
}

/// Every word [`read_mx`] reads but the gravity block's, where it lives.
fn mx_words(m: &Mx, p: &Ptrs) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let c = |o: u32| CAR + o;
    let (k, s) = (&m.keys, &m.state);
    let mut fs: Vec<(u32, F)> = vec![
        (c(mx::TRAVEL_FORWARD), k.cofg_travel_forward),
        (c(mx::TRAVEL_BACK), k.cofg_travel_back),
        (c(mx::PRELOAD_TIME), k.preload_time),
        (c(mx::MIN_PRELOAD_TIME), k.min_preload_time),
        (c(mx::MAX_PRELOAD_TIME), k.max_preload_time),
        (c(mx::MAX_PRELOAD), k.max_preload),
        (c(mx::CLUTCH_TIME), k.clutch_torque_scale_time),
        (c(mx::MAX_SCALE), k.max_clutch_torque_scale),
        (c(mx::SCALE_SPEED), k.max_clutch_torque_scale_speed),
        (c(mx::AREA), k.aero_surface_area),
        (c(mx::DAMPING), k.airborne_damping),
        (c(mx::UNROLL_GAIN), k.unroll_gain),
        (c(mx::UNROLL_TORQUE), k.unroll_torque),
        (c(mx::UNYAW_GAIN), k.unyaw_gain),
        (c(mx::UNYAW_TORQUE), k.unyaw_torque),
        (c(mx::GYRO), k.brake_gyro_torque),
        (m.wing_ptr, m.wing.lift_slope),
        (m.wing_ptr + 4, m.wing.angle_of_attack),
        (m.wing_ptr + 8, m.wing.basic_drag),
        (m.wing_ptr + 0xc, m.wing.induced_drag),
        (c(mx::LEAN), s.lean),
        (c(mx::STICK), s.stick),
        (c(mx::PAD_WORD), s.pad_word),
        (c(mx::CLUTCH), s.clutch),
        (c(mx::GAS), s.gas),
        (c(mx::VEHICLE_BRAKE), s.vehicle_brake),
        (c(mx::BRAKE), s.brake),
        (c(mx::BRAKE_FORCE), s.brake_force),
        (c(mx::HAND_BRAKE), s.hand_brake),
        (c(mx::TIMER), s.preload.timer),
        (c(mx::LAUNCH_TIMER), s.launch_timer),
        (c(mx::LAUNCH_START), s.launch_start),
        (p.engine + mx::BOOST, s.launch_boost),
        (c(mx::RATES), k.brake_rate_limit),
        (c(mx::RATES + 4), k.gas_rate_limit),
        (c(mx::RATES + 8), k.x_dpad_rate_limit_on),
        (c(mx::RATES + 0xc), k.x_dpad_rate_limit_off),
        (c(mx::RATES + 0x10), k.x_axis_rate_limit),
        (c(mx::YAW_LIMIT), k.yaw_rate_limit),
        (c(mx::HAND_BRAKE_YAW_LIMIT), k.hand_brake_yaw_rate_limit),
        (c(mx::FRICTION_SCALE), k.hand_brake_friction_scale),
        (c(mx::BRAKE_SCALE), k.hand_brake_brake_scale),
        (c(mx::FADE_TIME), k.hand_brake_fade_time),
        (m.penalty_object + mx::PENALTY, m.penalty_clock),
        (c(mx::GAS_PEDAL), s.gas_pedal),
        (c(mx::STEER_INPUT), s.steer_input),
        (c(mx::STEER), s.steer),
        (c(mx::REAR_STEER), s.rear_steer),
        (c(mx::ENGAGED_CLUTCH), s.engaged_clutch),
        (c(mx::FADE), s.hand_brake_fade),
        (p.tires[2] + mx::REAR_FRICTION, s.rear_friction[0]),
        (p.tires[3] + mx::REAR_FRICTION, s.rear_friction[1]),
    ];
    for (t, table) in [k.steer, k.wheel_steer, k.dpad_steer].iter().enumerate() {
        for (i, [speed, scale]) in table.iter().enumerate() {
            let a = c(mx::TABLES + 0x18 * t as u32 + 8 * i as u32);
            fs.extend([(a, *speed), (a + 4, *scale)]);
        }
    }
    for (i, [speed, scale]) in k.rear_steer.iter().enumerate() {
        let a = c(mx::REAR_TABLE + 8 * i as u32);
        fs.extend([(a, *speed), (a + 4, *scale)]);
    }
    let vs: [(u32, Vf); 6] = [
        (c(mx::COFG), k.cofg),
        (c(mx::COFG_AI), k.cofg_ai),
        (c(mx::CONTROL_MOMENT), k.airborne_control_moment),
        (m.wing_ptr + 0x10, m.wing.position),
        (p.tires[0] + mx::FRONT_NORMAL, s.front_normal),
        (c(mx::FORCE), s.preload.force),
    ];
    for (a, v) in vs {
        for (j, x) in v.0.into_iter().enumerate() {
            fs.push((a + 4 * j as u32, F(x)));
        }
    }
    out.extend(fs.into_iter().map(|(a, x)| (a, x.to_bits())));
    let words = [
        (c(mx::READY), word(s.preload.ready)),
        (c(mx::ACTIVE), word(s.preload.active)),
        (c(mx::ARMED), word(s.preload.armed)),
        (c(mx::CUED), word(s.preload.cued)),
        (c(mx::WINGS), m.wing_count as u32),
        (c(mx::VEHICLE_STATE), if m.vstate { m.vstate_object } else { 0 }),
        (m.vstate_object + mx::MODE, m.mode),
        (p.record + mx::TOUCHING, word(s.touching)),
        (m.vstate_object + mx::WHEEL, m.wheel_word),
        (c(mx::PENALTY_OBJECT), if m.penalty { m.penalty_object } else { 0 }),
        (c(mx::SQUARE), word(k.square_steering)),
        (c(mx::DRIVEN), word(s.driven)),
        (c(mx::DPAD), word(s.dpad)),
        (c(mx::HELD), word(s.held)),
        (c(mx::REVERSE_ENABLED), word(s.reverse_enabled)),
        (c(mx::THROTTLE_ENABLED), word(s.throttle_enabled)),
        (p.engine + mx::REVERSING, word(s.reversing)),
    ];
    out.extend(words);
    out
}

/// A generated set of keys and pad, vehicle and pieces' state about the image's: the leans and
/// stick about their thresholds (on them now and then), the preload's flags and timer, the remaps'
/// input ranges degenerate now and then, the wing count none, one or negative; and the car's words
/// the pieces read from the last frame (its speed, velocity, flow angles, roll and up row) stirred;
/// a third of them a fast, rising, grounded car off the brake, where the preload may wind up.
fn generate_mx(r: &mut Rng, image: &Mx, car: &mut Car, body: &mut Body) -> Mx {
    let w = car.velocity.0[3];
    car.velocity = r.vec(70.0, w);
    if r.chance(0.4) {
        car.velocity.0[2] = r.range(30.0, 70.0);
    }
    car.speed = if r.chance(0.2) { F(r.range(0.0, 10.0)) } else { F(r.range(10.0, 90.0)) };
    car.alpha = F(r.range(-1.5, 1.5));
    car.side_slip = F(r.range(-1.5, 1.5));
    car.euler.0[2] = r.range(-3.0, 3.0);
    car.up.0[1] = r.range(-1.0, 1.0);
    let mut m = image.clone();
    let either = |r: &mut Rng, x: f32| if r.chance(0.5) { -x } else { x };
    let lean_edge = f32::from_bits(0x3f4c_cccd);
    let k = &mut m.keys;
    k.cofg = r.vec(1.5, 1.0);
    k.cofg_ai = r.vec(1.5, 1.0);
    k.cofg_travel_forward = F(r.range(0.0, 0.6));
    k.cofg_travel_back = F(r.range(0.0, 0.6));
    k.preload_time = F(r.range(0.05, 0.5));
    k.min_preload_time = F(r.range(0.0, 40.0));
    k.max_preload_time = if r.chance(0.4) { k.min_preload_time } else { F(k.min_preload_time.0 + r.range(1.0, 80.0)) };
    k.max_preload = F(r.range(0.0, 3000.0));
    k.clutch_torque_scale_time = F(r.range(0.1, 3.0));
    k.max_clutch_torque_scale = F(r.range(1.0, 7.0));
    k.max_clutch_torque_scale_speed = if r.chance(0.15) { F::ZERO } else { F(r.range(5.0, 60.0)) };
    k.aero_surface_area = F(r.range(0.0, 6.0));
    k.airborne_damping = F(r.range(0.0, 3.0));
    k.unroll_gain = F(r.range(-4.0, 4.0));
    k.unroll_torque = F(r.range(-3000.0, 3000.0));
    k.unyaw_gain = F(r.range(-4.0, 4.0));
    k.unyaw_torque = F(r.range(-3000.0, 3000.0));
    let w = r.range(-2.0, 2.0);
    k.airborne_control_moment = r.vec(4000.0, w);
    k.brake_gyro_torque = F(r.range(-2000.0, 2000.0));
    m.wing = Wing {
        lift_slope: F(r.range(0.0, 6.0)),
        angle_of_attack: F(r.range(-0.3, 0.3)),
        basic_drag: F(r.range(0.0, 0.2)),
        induced_drag: F(r.range(0.0, 0.3)),
        position: Vf::ZERO,
    };
    let w = r.range(-2.0, 2.0);
    m.wing.position = r.vec(2.5, w);
    m.wing_count = [1, 1, 0, -1][(r.next() % 4) as usize];
    m.vstate = !r.chance(0.2);
    m.mode = if r.chance(0.7) { 1 } else { [0, 2, 3][(r.next() % 3) as usize] };
    let s = &mut m.state;
    s.human = m.vstate && m.mode == 1;
    s.lean = F(match r.next() % 8 {
        0 => either(r, lean_edge),
        1 => 0.0,
        2 | 3 => {
            let x = r.range(0.0, lean_edge);
            either(r, x)
        }
        4 | 5 => -r.range(lean_edge, 1.0),
        _ => r.range(lean_edge, 1.0),
    });
    s.stick = F(match r.next() % 5 {
        0 => either(r, f32::from_bits(0x3dcc_cccd)),
        1 | 2 => {
            let x = r.range(0.0, 0.1);
            either(r, x)
        }
        _ => r.range(-1.0, 1.0),
    });
    s.pad_word = F(r.range(-100.0, 100.0));
    s.clutch = F([0.0, 0.0, 1.0, r.range(0.0, 1.0)][(r.next() % 4) as usize]);
    let off = |r: &mut Rng, p: f32| F(if r.chance(p) { 0.0 } else { r.range(0.0, 1.0) });
    s.gas = off(r, 0.6);
    s.vehicle_brake = off(r, 0.7);
    s.brake = off(r, 0.6);
    s.brake_force = off(r, 0.6);
    s.hand_brake = off(r, 0.7);
    s.touching = r.chance(0.25);
    s.front_normal = if r.chance(0.3) { Vf::new(0.0, 1.0, 0.0, 0.0) } else { r.vec(1.0, 0.0) };
    let pr = &mut s.preload;
    pr.ready = r.chance(0.5);
    pr.active = r.chance(0.3);
    pr.armed = r.chance(0.6);
    pr.timer = F(if r.chance(0.3) { 0.0 } else { r.range(-0.05, 0.4) });
    pr.force = Vf::new(r.range(-50.0, 50.0), r.range(0.0, 3000.0), r.range(-50.0, 50.0), if r.chance(0.5) { 0.0 } else { 1.0 });
    pr.cued = r.chance(0.5);
    if r.chance(0.35) {
        car.grounded = true;
        car.velocity.0[2] = r.range(30.0, 70.0);
        body.velocity.0[1] = r.range(5.0, 30.0);
        s.vehicle_brake = F::ZERO;
        pr.active = r.chance(0.1);
        pr.armed = r.chance(0.5);
    }
    s.launch_timer = F(if r.chance(0.35) { 0.0 } else { r.range(0.0, 3.0) });
    s.launch_start = F(r.range(1.0, 7.0));
    s.launch_boost = F(r.range(1.0, 7.0));
    generate_controls(r, &mut m);
    m
}

/// The controls' share of [`generate_mx`]: who drives and how (no driver, the d-pad, a wheel, the
/// stick squared or not), the steer tables with their segments flat now and then, the gas pedal
/// (down with the brake, for reverse, now and then), the hold, the vehicle's switches, the start penalty (its object
/// gone now and then) and the hand brake's fade about its ends.
fn generate_controls(r: &mut Rng, m: &mut Mx) {
    let either = |r: &mut Rng, x: f32| if r.chance(0.5) { -x } else { x };
    let k = &mut m.keys;
    k.square_steering = r.chance(0.5);
    for rate in [&mut k.brake_rate_limit, &mut k.gas_rate_limit, &mut k.x_dpad_rate_limit_on, &mut k.x_dpad_rate_limit_off, &mut k.x_axis_rate_limit] {
        *rate = F(r.range(0.0, 20.0));
    }
    let mut table = |r: &mut Rng| -> [[F; 2]; 3] {
        let a = r.range(0.0, 20.0);
        let b = if r.chance(0.2) { a } else { a + r.range(1.0, 40.0) };
        let c = if r.chance(0.2) { b } else { b + r.range(1.0, 40.0) };
        [a, b, c].map(|x| [F(x), F(r.range(0.0, 1.5))])
    };
    (k.steer, k.wheel_steer, k.dpad_steer) = (table(r), table(r), table(r));
    let [a, b, _] = table(r);
    k.rear_steer = [a, [b[0], F(r.range(-1.0, 1.0))]];
    k.yaw_rate_limit = F(r.range(0.5, 5.0));
    k.hand_brake_yaw_rate_limit = F(r.range(0.5, 5.0));
    k.hand_brake_friction_scale = F(r.range(0.0, 1.5));
    k.hand_brake_brake_scale = F(r.range(0.0, 1.5));
    k.hand_brake_fade_time = F(if r.chance(0.15) { 0.0 } else { r.range(0.1, 1.0) });
    m.wheel_word = if r.chance(0.3) { 1 } else { 0 };
    m.penalty = !r.chance(0.1);
    m.penalty_clock = F(if r.chance(0.8) { either(r, 0.0) } else { r.range(-1.0, 3.0) });
    let s = &mut m.state;
    s.driven = !r.chance(0.12);
    s.wheel = m.vstate && m.wheel_word != 0;
    s.dpad = r.chance(0.35);
    s.start_penalty = m.penalty && m.penalty_clock > 0.0;
    if s.dpad && r.chance(0.3) {
        s.stick = F(either(r, 0.0));
    }
    s.gas_pedal = F(if r.chance(0.4) { 0.0 } else { r.range(0.0, 1.0) });
    if s.vehicle_brake != 0.0 && r.chance(0.6) {
        s.gas_pedal = F(r.range(0.05, 1.0));
    }
    s.steer_input = F(r.range(-1.0, 1.0));
    s.steer = F(r.range(-1.0, 1.0));
    s.rear_steer = F(r.range(-1.0, 1.0));
    s.engaged_clutch = F(r.range(0.0, 1.0));
    s.hand_brake_fade = F(if r.chance(0.3) { 0.0 } else { r.range(-0.05, 1.0) });
    s.rear_friction = [F(r.range(0.0, 1.5)), F(r.range(0.0, 1.5))];
    s.held = r.chance(0.25);
    s.reverse_enabled = !r.chance(0.2);
    s.throttle_enabled = !r.chance(0.2);
    s.reversing = r.chance(0.3);
}

/// Hook the preload's cue: log it, from the preload, on its fixed arguments.
fn hook_cue(ee: &mut Ee, log: Shared<Vec<Call>>) {
    ee.hooks.insert(CUE, Box::new(move |ee| {
        assert_eq!(ee.r(31) as u32, CUE_FROM, "a cue from elsewhere");
        let want = [ee.u32_at(CUE_GLOBAL), CAR + 0x674, 8, 14, ee.u32_at(CAR + 0xd20), 0];
        let got = [4, 5, 6, 7, 8, 9].map(|k| ee.r(k) as u32);
        assert_eq!(got, want, "the cue's arguments");
        log.borrow_mut().push(Call::PreloadCue);
    }));
}

/// The port's side with the pieces at `unhooked` answered by MX Unleashed's [`Pieces`] (and not
/// logged, as the original's own code runs them unhooked), every other call by the [`Port`]; what
/// each answered call found is seen.
struct Unhooked<'a> {
    pieces: Pieces<Port>,
    unhooked: &'a [u32],
    seen: Seen,
}

impl Steps for Unhooked<'_> {
    fn call(&mut self, c: Call, car: &mut Car, body: &mut Body) {
        let addr = match c {
            Call::Controls(_) => CONTROLS,
            Call::TravelCofg => TRAVEL_COFG,
            Call::AntiSlide(_) => ANTI_SLIDE,
            Call::Preload(_) => PRELOAD,
            Call::Launch(_) => LAUNCH,
            Call::Aero(_) => AERO,
            _ => 0,
        };
        if !self.unhooked.contains(&addr) {
            self.pieces.rest.call(c, car, body);
            return;
        }
        let (k, s0) = (self.pieces.keys.clone(), self.pieces.state);
        let riding = car.state == atv_car::RIDING;
        let mut tags: Vec<&'static str> = Vec::new();
        match c {
            Call::Controls(_) => controls_tags(&k, &s0, car, &mut tags),
            Call::TravelCofg => {
                tags.push(if s0.human { "travel: a person's" } else { "travel: the AI's" });
                tags.push(if s0.lean < 0.0 { "travel: forward" } else { "travel: back" });
            }
            Call::Aero(_) => {
                if riding && car.velocity.z() > 0.0 && !k.wings.is_empty() {
                    tags.push("aero: wings");
                }
                if riding && !car.grounded && !s0.touching && car.speed > 10.0 {
                    tags.push("aero: air control");
                    if s0.stick.abs() < f32::from_bits(0x3dcc_cccd) && car.up.y() > f32::from_bits(0x3eae_147b) {
                        tags.push("aero: unroll");
                    }
                    if s0.brake != 0.0 || s0.brake_force != 0.0 {
                        tags.push("aero: gyro");
                    }
                }
            }
            _ => {}
        }
        let (force, log) = (body.force, self.pieces.rest.log.len());
        assert!(self.pieces.answer(c, car, body), "{c:?} unanswered");
        let s = self.pieces.state;
        let (p0, p) = (s0.preload, s.preload);
        match c {
            Call::AntiSlide(_) => tags.push(match (body.force != force, s0.vehicle_brake != 0.0 || s0.hand_brake != 0.0) {
                (false, _) => "anti-slide: idle",
                (true, false) => "anti-slide: holds",
                (true, true) => "anti-slide: brakes",
            }),
            Call::Preload(_) if riding => {
                tags.push(match (p0.active, p.active) {
                    (true, true) => "preload: pushing",
                    (true, false) => "preload: ran out",
                    (false, true) if k.min_preload_time == k.max_preload_time => "preload: fired, degenerate",
                    (false, true) => "preload: fired",
                    (false, false) if !p.ready => "preload: not ready",
                    (false, false) if !p0.armed && p.armed => "preload: armed",
                    (false, false) if p.armed && p.timer != p0.timer => "preload: wound",
                    (false, false) => "preload: ready, idle",
                });
                if self.pieces.rest.log.len() > log {
                    tags.push("preload: cue");
                } else if p0.cued && !p.cued {
                    tags.push("preload: cue forgotten");
                }
            }
            // Never wrecked: the Force body calls the preload only riding.
            Call::Preload(_) => tags.push("preload: wrecked"),
            Call::Launch(_) => tags.push(if s0.clutch == 1.0 {
                "launch: clutch in"
            } else if s.launch_timer == s0.launch_timer {
                "launch: idle"
            } else if s0.launch_timer == 0.0 && k.max_clutch_torque_scale_speed == 0.0 {
                "launch: starts, degenerate"
            } else if s0.launch_timer == 0.0 {
                "launch: starts"
            } else {
                "launch: eases"
            }),
            _ => {}
        }
        for t in tags {
            *self.seen.entry(t).or_insert(0) += 1;
        }
    }
}

/// The branches the controls take from `s` (the state they are handed) and `car`.
fn controls_tags(k: &Keys, s: &State, car: &Car, tags: &mut Vec<&'static str>) {
    if s.start_penalty {
        tags.push("controls: start penalty");
    }
    if !s.driven {
        tags.push("controls: no driver");
        return;
    }
    let speed = car.velocity.z();
    let table = if s.dpad {
        tags.push(if s.stick.abs() == 0.0 { "controls: d-pad, centred" } else { "controls: d-pad" });
        &k.dpad_steer
    } else if s.wheel {
        tags.push("controls: wheel");
        &k.wheel_steer
    } else {
        tags.push(if k.square_steering { "controls: stick, squared" } else { "controls: stick" });
        &k.steer
    };
    let [a, b, c] = *table;
    tags.push(match speed < b[0].0 {
        true if a[0] == b[0] => "controls: low segment, flat",
        true => "controls: low segment",
        false if b[0] == c[0] => "controls: high segment, flat",
        false => "controls: high segment",
    });
    if k.rear_steer[0][0] == k.rear_steer[1][0] {
        tags.push("controls: rear steer flat");
    }
    let gas = if s.start_penalty { 0.0 } else { s.gas_pedal.0 };
    if s.held && !(gas > 0.0) {
        tags.push("controls: held");
    } else {
        if gas != 0.0 && s.vehicle_brake != 0.0 && s.reverse_enabled {
            tags.push("controls: into reverse");
        } else {
            if s.reversing {
                tags.push("controls: out of reverse");
            }
            tags.push(if s.vehicle_brake <= s.brake { "controls: brake let off" } else { "controls: brake eases on" });
        }
        if !s.throttle_enabled {
            tags.push("controls: no throttle");
        }
    }
    tags.push(match (s.hand_brake != 0.0, s.hand_brake_fade > 0.0) {
        (true, _) => "controls: hand brake",
        (false, true) if k.hand_brake_fade_time == 0.0 => "controls: fading, flat",
        (false, true) => "controls: fading",
        (false, false) => "controls: faded",
    });
}

#[test]
fn controls_are_mx_unleasheds() {
    pieces(0x0126_0001, &[CONTROLS], &["controls"], 22);
}

#[test]
fn all_six_pieces_are_mx_unleasheds() {
    pieces(
        0x0126_0002,
        &[CONTROLS, TRAVEL_COFG, ANTI_SLIDE, PRELOAD, LAUNCH, AERO],
        &["controls", "travel", "anti-slide", "preload", "launch", "aero"],
        48,
    );
}

/// Each piece unhooked alone, and all five, under the frame's oracle: the original's own code for
/// the piece against [`Pieces`]'s port, every word and call equal; each of the piece's branches seen.
fn pieces(seed: u64, unhooked: &[u32], prefixes: &[&str], want: usize) {
    let Some((_, seen)) = run(seed, 3000, unhooked) else { return };
    let mine: Vec<(&&str, &u32)> = seen.iter().filter(|(w, _)| prefixes.iter().any(|p| w.starts_with(p))).collect();
    for &(what, &k) in &mine {
        assert!(k >= 20, "{what} seen only {k} times");
    }
    assert_eq!(mine.len(), want, "a branch was never seen: {seen:?}");
}

#[test]
fn travel_cofg_is_mx_unleasheds() {
    pieces(0x0106_a001, &[TRAVEL_COFG], &["travel"], 4);
}

#[test]
fn anti_slide_is_mx_unleasheds() {
    pieces(0x0106_a002, &[ANTI_SLIDE], &["anti-slide"], 3);
}

#[test]
fn preload_is_mx_unleasheds() {
    pieces(0x0106_a003, &[PRELOAD], &["preload"], 10);
}

#[test]
fn launch_is_mx_unleasheds() {
    pieces(0x0106_a004, &[LAUNCH], &["launch"], 5);
}

#[test]
fn aero_is_mx_unleasheds() {
    pieces(0x0106_a005, &[AERO], &["aero"], 4);
}

#[test]
fn all_five_pieces_are_mx_unleasheds() {
    pieces(0x0106_a006, &[TRAVEL_COFG, ANTI_SLIDE, PRELOAD, LAUNCH, AERO], &["travel", "anti-slide", "preload", "launch", "aero"], 26);
}

/// **The car's frame is the original's**: SLUS-21104's world step over the buggy image's world, with
/// every call of the car's three listeners hooked, against [`atv_car::frame`] with a [`Steps`] that
/// plays the same script, over 3,000 generated states: every word of the car, its tires and its body
/// equal, the same calls in the same order with the same arguments, and nothing else written in those
/// blocks. Each branch of the listeners is seen; then a word of the port's result is changed, to see
/// the comparison go red there.
#[test]
fn the_cars_frame_is_the_originals() {
    let Some((seen, _)) = run(0x0106_ca7f_0000_0001, 3000, &[]) else { return };
    for (what, &k) in &seen {
        assert!(k >= 30, "{what} seen only {k} times");
    }
    assert_eq!(seen.len(), 19, "a branch was never seen: {seen:?}");
}

type Seen = std::collections::BTreeMap<&'static str, u32>;

/// The world step's frame, `cases` generated states from `seed`, with the calls at `unhooked` left to
/// SLUS-21104's own code and the port's answered by MX Unleashed's [`Pieces`]: the listeners' branches
/// seen, and the pieces'.
fn run(seed: u64, cases: usize, unhooked: &[u32]) -> Option<(Seen, Seen)> {
    let pristine = image()?;
    let mut ee = image()?;
    let p = ptrs(&ee);
    let image_car = read_car(&ee, &p);
    let image_body = seed::body(&|a| ee.u32_at(a), p.body);
    let image_mx = read_mx(&ee, &p);
    assert!(image_body.enabled, "the image's body is enabled");
    let log: Shared<Vec<Call>> = Rc::default();
    let script: Shared<Script> = Rc::default();
    hook(&mut ee, p, log.clone(), script.clone());
    hook_cue(&mut ee, log.clone());
    for a in unhooked {
        assert!(ee.hooks.remove(a).is_some(), "{a:#x} was not hooked");
    }
    let world = World { iterations: 1, rate: F(115.0) };
    let table = AcosTable::build();
    let mut r = Rng(seed);
    let mut seen = Seen::new();
    let mut pieces_seen = Seen::new();
    let mut see = |what: &'static str, yes: bool| {
        if yes {
            *seen.entry(what).or_insert(0) += 1;
        }
    };
    let blocks: Vec<(u32, u32)> = [(CAR, 0x1500), (p.body, 0x230), (p.record + at::TIPPED, 4), (p.engine + mx::BOOST, 4)]
        .into_iter()
        .chain([2, 3].map(|t| (p.tires[t] + mx::REAR_FRICTION, 4)))
        .chain(p.tires.map(|t| (t, 0x200)))
        .collect();
    for case in 0..cases {
        let (mut car, mut body, dt) = generate(&mut r, &image_car, &image_body, &p);
        let m = generate_mx(&mut r, &image_mx, &mut car, &mut body);
        let s = Script::new(&mut r);
        let before = [car_words(&car, &p), seed::body_words(&body, p.body), mx_words(&m, &p)].concat();

        // The original.
        poke(&mut ee, &before);
        *script.borrow_mut() = s.clone();
        log.borrow_mut().clear();
        ee.set(28, GP);
        ee.call(WORLD_STEP, &[u64::from(WORLD)], &[dt.0], 20_000_000);
        let written = ee.written_ranges();

        // The port.
        let (mut pc, mut pb) = (car, body);
        let port = Port { log: Vec::new(), script: s };
        let mut port = Unhooked { pieces: Pieces { keys: m.keys(), state: m.state, rest: port }, unhooked, seen: Seen::new() };
        atv_car::frame(&world, &mut pc, &mut pb, dt, &table, &mut port);
        for (what, k) in std::mem::take(&mut port.seen) {
            *pieces_seen.entry(what).or_insert(0) += k;
        }
        let pm = Mx { state: port.pieces.state, ..m };
        let port = port.pieces.rest;
        let after = [car_words(&pc, &p), seed::body_words(&pb, p.body), mx_words(&pm, &p)].concat();

        let bad: Vec<String> = after
            .iter()
            .filter(|&&(a, x)| ee.u32_at(a) != x)
            .map(|&(a, x)| format!("  {a:#x}: port {x:#010x} ({}), original {:#010x} ({})", f32::from_bits(x), ee.u32_at(a), ee.f32_at(a)))
            .collect();
        assert!(bad.is_empty(), "case {case} (dt {dt:?}): {} words differ\n{}", bad.len(), bad.join("\n"));
        let theirs = log.borrow().iter().map(|c| format!("{c:?}")).collect::<Vec<_>>();
        let ours = port.log.iter().map(|c| format!("{c:?}")).collect::<Vec<_>>();
        assert_eq!(ours, theirs, "case {case}: the calls");
        let compared: std::collections::BTreeSet<u32> = after.iter().map(|&(a, _)| a).collect();
        for &(from, to) in &written {
            for a in (from & !3..to).step_by(4) {
                let ours = blocks.iter().any(|&(b, n)| (b..b + n).contains(&a));
                assert!(!ours || compared.contains(&a), "case {case}: the original wrote {a:#x}, which is not compared");
            }
        }

        let n = ours.len();
        see("riding", car.state == atv_car::RIDING);
        see("wrecked", car.state != atv_car::RIDING);
        see("lone front", pc.tires[0].lone_front);
        see("no front alone", !pc.tires[0].lone_front && (s_any(&pc, |t| t.solid || t.sorted)));
        see("handed by the left", pc.tires[2].handed && pc.tires[0].grounded);
        see("handed by the right", pc.tires[2].handed && !pc.tires[0].grounded);
        see("nothing handed", !pc.tires[2].handed);
        let landed = ours.iter().any(|c| c.starts_with("Landed"));
        let shock = pb.correction_peak.to_bits() != [0, 0, 0, 1.0f32.to_bits()];
        see("landed on the shock", landed && shock);
        see("landed on the velocity", landed && !shock);
        see("one side", pc.one_side && pc.grounded);
        see("airborne", !pc.grounded);
        see("yaw held", pb.rates.y().abs() == pc.yaw_rate_limit.0);
        see("tipped", pc.tipped);
        see("on the tipped edge", pc.up.to_bits()[..3] == [0, 1.0f32.to_bits(), 0] && port.script.normal.y().to_bits() == EDGE);
        see("tied on signed zeros", pc.tires[2].handed && pc.tires[0].load == 0.0 && pc.tires[1].load == 0.0 && pc.tires[0].load.to_bits() != pc.tires[1].load.to_bits());
        see("upright", !pc.tipped);
        see("latch let go", car.latch && car.latched);
        see("ground velocity of a tire", pc.grounded);
        see("more substeps", n > 40);

        if case == 0 {
            // One flipped bit of the port's result goes red in exactly that word.
            let (a, _) = after[after.len() / 2];
            let red: Vec<u32> = after
                .iter()
                .map(|&(b, y)| if b == a { (b, y ^ 1) } else { (b, y) })
                .filter(|&(b, y)| ee.u32_at(b) != y)
                .map(|(b, _)| b)
                .collect();
            assert_eq!(red, [a], "a flipped bit");
        }

        // Put the image back.
        for (from, to) in written {
            for a in from..to {
                ee.write(a, 1, u128::from(pristine.u8_at(a)));
            }
        }
    }
    eprintln!("MD106 (unhooked {unhooked:x?}): {cases} frames, seen {seen:?}, pieces {pieces_seen:?}");
    Some((seen, pieces_seen))
}

fn s_any(car: &Car, f: impl Fn(&Tire) -> bool) -> bool {
    car.tires.iter().any(f)
}

/// The car's update (the car class's vt `+0x10c`; the buggy's own slot runs its odometer, collision
/// and rumble first, then this) and its calls: the ghost step (the buggy's vt `+0x3a4`, a jump), the
/// hazard clock (vt `+0x45c`), the buggy's own step (vt `+0x488`) and the vehicle's base update.
const UPDATE: u32 = 0x004c_c4e0;
const GHOST: u32 = 0x004c_d4e0;
const HAZARD_CLOCK: u32 = 0x004a_ad60;
const CLASS_STEP: u32 = 0x004c_5510;
const VEHICLE_UPDATE: u32 = 0x004a_79c0;
/// The game's settings pointer, and the still-camera switch's byte and bit there.
const SETTINGS: u32 = 0x003e_ab60;
const STILL_BYTE: u32 = 0x225;
const STILL_BIT: u8 = 0x08;

/// The update's words (research/oracle.txt `# MD106d`).
mod up {
    pub const VEHICLE_STATE: u32 = 0xbc4;
    pub const MODE: u32 = 0x28;
    pub const ON_HAZARD: u32 = 0xc20;
    pub const SURFACES: u32 = 0xdf4;
    /// A tire's surface byte.
    pub const TIRE_SURFACE: u32 = 0xf0;
}

/// Hook the update's four calls: log them in the port's terms. Each must be on the player's car.
fn hook_update(ee: &mut Ee, log: Shared<Vec<Call>>) {
    type Make = fn(&Ee) -> Call;
    let calls: [(u32, Make); 4] = [
        (GHOST, |ee| Call::Ghost(F(ee.f(12)))),
        (HAZARD_CLOCK, |ee| {
            assert!(ee.r(5) < 2, "the hazard clock's sunk word {:#x}", ee.r(5));
            Call::HazardClock { dt: F(ee.f(12)), time: F(ee.f(13)), sunk: ee.r(5) != 0 }
        }),
        (CLASS_STEP, |_| Call::ClassStep),
        (VEHICLE_UPDATE, |ee| Call::VehicleUpdate(F(ee.f(12)))),
    ];
    for (addr, make) in calls {
        let l = log.clone();
        ee.hooks.insert(addr, Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, CAR, "a call of the update on another object");
            l.borrow_mut().push(make(ee));
        }));
    }
}

/// A generated update: the image's car with its latch, tires' contact flags and surfaces stirred,
/// the still camera and the vehicle state's mode chosen, and a surface table (with the hazard words
/// as written: any set word is a hazard).
fn generate_update(r: &mut Rng, image_car: &Car) -> (Update, [u32; 16], Car, F) {
    let mut car = *image_car;
    car.latch = r.chance(0.6);
    car.latched = r.chance(0.5);
    car.latch_count = (r.next() % 8) as i32 - 2;
    let solid = [0.0, 0.2, 0.6][(r.next() % 3) as usize];
    for t in &mut car.tires {
        t.solid = r.chance(solid);
    }
    let mut u = Update {
        still_camera: r.chance(0.15),
        vehicle_state: if r.chance(0.15) { None } else { Some([3, 4, 1, 1, 0, 2, r.word()][(r.next() % 7) as usize]) },
        on_hazard: r.chance(0.5),
        ..Update::default()
    };
    let hazards = [0.0, 0.05, 0.4][(r.next() % 3) as usize];
    let mut words = [0u32; 16];
    for (w, s) in words.iter_mut().zip(&mut u.surfaces) {
        *w = if r.chance(hazards) { [1, 0x8000_0000, r.word() | 1][(r.next() % 3) as usize] } else { 0 };
        s.hazard = *w != 0;
        s.time = F(if r.chance(0.1) {
            -1.0
        } else if r.chance(0.15) {
            if r.chance(0.5) { -0.0 } else { 0.0 }
        } else {
            r.range(-1.0, 6.0)
        });
    }
    // Now and then all four on one surface, so the longest time is a tie.
    let one = (r.next() % 16) as u8;
    u.tire_surface = core::array::from_fn(|_| if r.chance(0.3) { one } else { (r.next() % 16) as u8 });
    if r.chance(0.1) {
        // The four tires' times all zeros of either sign: the longest is a `max.s` of signed zeros.
        for k in u.tire_surface {
            u.surfaces[usize::from(k)].time = F(if r.chance(0.5) { -0.0 } else { 0.0 });
        }
    }
    let dt = if r.chance(0.7) { F(1.0 / 30.0) } else { F(r.range(0.002, 0.05)) };
    (u, words, car, dt)
}

/// **The car's update is the original's**: SLUS-21104's update on the buggy image's car, its four
/// calls hooked, against [`atv_car::update`] over 10,000 generated updates: every word of the car and
/// its tires and the hazard flag equal, the same calls in the same order with the same arguments, and
/// nothing else written in those blocks. Each branch is seen.
#[test]
fn the_cars_update_is_the_originals() {
    let Some(pristine) = image() else { return };
    let Some(mut ee) = image() else { return };
    let p = ptrs(&ee);
    let image_car = read_car(&ee, &p);
    let state = ee.u32_at(CAR + up::VEHICLE_STATE);
    assert_ne!(state, 0, "the image's car has a vehicle state");
    let still_at = ee.u32_at(SETTINGS) + STILL_BYTE;
    let log: Shared<Vec<Call>> = Rc::default();
    hook_update(&mut ee, log.clone());
    let mut r = Rng(0x0106_d0da_7e00_0001);
    let mut seen = std::collections::BTreeMap::<&str, u32>::new();
    let mut see = |what: &'static str, yes: bool| {
        if yes {
            *seen.entry(what).or_insert(0) += 1;
        }
    };
    let blocks: Vec<(u32, u32)> = std::iter::once((CAR, 0x1500)).chain(p.tires.map(|t| (t, 0x200))).collect();
    const CASES: usize = 10_000;
    for case in 0..CASES {
        let (u, hazard_words, car, dt) = generate_update(&mut r, &image_car);

        // The original.
        poke(&mut ee, &car_words(&car, &p));
        let still = ee.u8_at(still_at) & !STILL_BIT | if u.still_camera { STILL_BIT } else { 0 };
        ee.write(still_at, 1, u128::from(still));
        ee.set_u32(CAR + up::VEHICLE_STATE, if u.vehicle_state.is_some() { state } else { 0 });
        ee.set_u32(state + up::MODE, u.vehicle_state.unwrap_or(0));
        ee.set_u32(CAR + up::ON_HAZARD, word(u.on_hazard));
        for (k, (s, w)) in u.surfaces.iter().zip(hazard_words).enumerate() {
            let a = CAR + up::SURFACES + 0xc * k as u32;
            ee.set_u32(a, w);
            ee.set_u32(a + 4, s.time.to_bits());
        }
        for (t, &k) in p.tires.iter().zip(&u.tire_surface) {
            ee.write(t + up::TIRE_SURFACE, 1, u128::from(k));
        }
        log.borrow_mut().clear();
        ee.set(28, GP);
        ee.call(UPDATE, &[u64::from(CAR)], &[dt.0], 1_000_000);
        let written = ee.written_ranges();

        // The port.
        let (mut pu, mut pc, mut pb) = (u, car, Body::default());
        let mut port = Port { log: Vec::new(), script: Script::default() };
        atv_car::update(&mut pu, &mut pc, &mut pb, dt, &mut port);
        let mut after = car_words(&pc, &p);
        after.push((CAR + up::ON_HAZARD, word(pu.on_hazard)));

        let bad: Vec<String> = after
            .iter()
            .filter(|&&(a, x)| ee.u32_at(a) != x)
            .map(|&(a, x)| format!("  {a:#x}: port {x:#010x}, original {:#010x}", ee.u32_at(a)))
            .collect();
        assert!(bad.is_empty(), "case {case} ({u:?}, dt {dt:?}): {} words differ\n{}", bad.len(), bad.join("\n"));
        let theirs = log.borrow().iter().map(|c| format!("{c:?}")).collect::<Vec<_>>();
        let ours = port.log.iter().map(|c| format!("{c:?}")).collect::<Vec<_>>();
        assert_eq!(ours, theirs, "case {case} ({u:?}): the calls");
        let compared: std::collections::BTreeSet<u32> = after.iter().map(|&(a, _)| a).collect();
        for &(from, to) in &written {
            for a in (from & !3..to).step_by(4) {
                let ours = blocks.iter().any(|&(b, n)| (b..b + n).contains(&a));
                assert!(!ours || compared.contains(&a), "case {case}: the original wrote {a:#x}, which is not compared");
            }
        }

        let latching = !u.still_camera && matches!(u.vehicle_state, None | Some(3 | 4));
        let measuring = !u.still_camera && !latching;
        let sunk = port.log.iter().any(|c| matches!(c, Call::HazardClock { sunk: true, .. }));
        see("still camera", u.still_camera);
        see("no vehicle state", latching && u.vehicle_state.is_none());
        see("mode 3", latching && u.vehicle_state == Some(3));
        see("mode 4", latching && u.vehicle_state == Some(4));
        see("latch counted", latching && car.latch && pc.latch);
        see("latch counted up from below zero", latching && car.latch && car.latch_count < 0);
        see("latch let go", latching && car.latch && !pc.latch);
        see("no latch", latching && !car.latch);
        see("measured", measuring);
        see("on a hazard, sunk", measuring && pu.on_hazard && sunk);
        see("on a hazard, not sunk", measuring && pu.on_hazard && !sunk);
        see("on no hazard, a tire solid", measuring && !pu.on_hazard && pc.tires.iter().any(|t| t.solid));
        see("a sign-bit hazard word", measuring && hazard_words.contains(&0x8000_0000) && pu.on_hazard);
        see("longest time a tie of signed zeros", measuring && {
            let t = u.tire_surface.map(|k| u.surfaces[usize::from(k)].time);
            t.iter().all(|x| *x == 0.0) && t.iter().any(|x| x.to_bits() != t[0].to_bits())
        });

        if case == 0 {
            let (a, _) = after[after.len() - 1];
            let red: Vec<u32> = after
                .iter()
                .map(|&(b, y)| if b == a { (b, y ^ 1) } else { (b, y) })
                .filter(|&(b, y)| ee.u32_at(b) != y)
                .map(|(b, _)| b)
                .collect();
            assert_eq!(red, [a], "a flipped bit");
        }

        for (from, to) in written {
            for a in from..to {
                ee.write(a, 1, u128::from(pristine.u8_at(a)));
            }
        }
    }
    eprintln!("MD106d: {CASES} updates, seen {seen:?}");
    for (what, &k) in &seen {
        assert!(k >= 30, "{what} seen only {k} times");
    }
    assert_eq!(seen.len(), 14, "a branch was never seen: {seen:?}");
}
