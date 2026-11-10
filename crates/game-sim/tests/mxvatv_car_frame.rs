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
//! Without the image every test here skips (and says so).

use game_sim::air::AcosTable;
use game_sim::atv_body::Body;
use game_sim::atv_car::{self, Call, Car, Steps, Tire};
use game_sim::biplane::World;
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

/// **The car's frame is the original's**: SLUS-21104's world step over the buggy image's world, with
/// every call of the car's three listeners hooked, against [`atv_car::frame`] with a [`Steps`] that
/// plays the same script, over 3,000 generated states: every word of the car, its tires and its body
/// equal, the same calls in the same order with the same arguments, and nothing else written in those
/// blocks. Each branch of the listeners is seen; then a word of the port's result is changed, to see
/// the comparison go red there.
#[test]
fn the_cars_frame_is_the_originals() {
    let Some(pristine) = image() else { return };
    let Some(mut ee) = image() else { return };
    let p = ptrs(&ee);
    let image_car = read_car(&ee, &p);
    let image_body = seed::body(&|a| ee.u32_at(a), p.body);
    assert!(image_body.enabled, "the image's body is enabled");
    let log: Shared<Vec<Call>> = Rc::default();
    let script: Shared<Script> = Rc::default();
    hook(&mut ee, p, log.clone(), script.clone());
    let world = World { iterations: 1, rate: F(115.0) };
    let table = AcosTable::build();
    let mut r = Rng(0x0106_ca7f_0000_0001);
    let mut seen = std::collections::BTreeMap::<&str, u32>::new();
    let mut see = |what: &'static str, yes: bool| {
        if yes {
            *seen.entry(what).or_insert(0) += 1;
        }
    };
    let blocks: Vec<(u32, u32)> = [(CAR, 0x1500), (p.body, 0x230), (p.record + at::TIPPED, 4)]
        .into_iter()
        .chain(p.tires.map(|t| (t, 0x200)))
        .collect();
    const CASES: usize = 3000;
    for case in 0..CASES {
        let (car, body, dt) = generate(&mut r, &image_car, &image_body, &p);
        let s = Script::new(&mut r);
        let before = [car_words(&car, &p), seed::body_words(&body, p.body)].concat();

        // The original.
        poke(&mut ee, &before);
        *script.borrow_mut() = s.clone();
        log.borrow_mut().clear();
        ee.set(28, GP);
        ee.call(WORLD_STEP, &[u64::from(WORLD)], &[dt.0], 20_000_000);
        let written = ee.written_ranges();

        // The port.
        let (mut pc, mut pb) = (car, body);
        let mut port = Port { log: Vec::new(), script: s };
        atv_car::frame(&world, &mut pc, &mut pb, dt, &table, &mut port);
        let after = [car_words(&pc, &p), seed::body_words(&pb, p.body)].concat();

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
    eprintln!("MD106: {CASES} frames, seen {seen:?}");
    for (what, &k) in &seen {
        assert!(k >= 30, "{what} seen only {k} times");
    }
    assert_eq!(seen.len(), 19, "a branch was never seen: {seen:?}");
}

fn s_any(car: &Car, f: impl Fn(&Tire) -> bool) -> bool {
    car.tires.iter().any(f)
}
