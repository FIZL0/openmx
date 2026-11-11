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
use game_sim::atv_car::{self, Call, Car, Keys, Lever, Pieces, PreloadState, SORT_HITS, SortAnswer, State, Steps, Tire, TireKeys, TireState, Update, Wing};
use game_sim::car::Surface;
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
/// Where the probe asks the ground (MD127).
const PROBE_GROUND: u32 = 0x004b_5464;
/// The sort's ray probe's place (its vt `+0xcc`) and search, and where the sort calls them (MD127).
const SORT_PLACE: u32 = 0x001b_cfe0;
const SORT_SEARCH: u32 = 0x0011_51c0;
const POST_PLACE: u32 = 0x004c_addc;
const POST_SEARCH: u32 = 0x004c_adec;
/// Free RAM (zero in the image) for the sort's hits, records, objects with their one listener, and
/// movers; the listener's vt `+0xc` is a hook at [`sort::STUB`].
mod sort {
    pub const SCRATCH: u32 = 0x01fe_0000;
    pub const FROMS: u32 = SCRATCH;
    pub const NORMALS: u32 = SCRATCH + 0x100;
    pub const POINTS: u32 = SCRATCH + 0x200;
    pub const RECORDS: u32 = SCRATCH + 0x300;
    pub fn record(i: usize) -> u32 {
        SCRATCH + 0x400 + 0x200 * i as u32
    }
    pub fn object(i: usize) -> u32 {
        SCRATCH + 0x1400 + 0x300 * i as u32
    }
    pub fn listeners(i: usize) -> u32 {
        SCRATCH + 0x2400 + 0x200 * i as u32
    }
    pub fn items(i: usize) -> u32 {
        SCRATCH + 0x3400 + 0x10 * i as u32
    }
    pub fn item(i: usize) -> u32 {
        SCRATCH + 0x3800 + 0x10 * i as u32
    }
    pub const VTABLE: u32 = SCRATCH + 0x3c00;
    pub fn mover(i: usize) -> u32 {
        SCRATCH + 0x4000 + 0x200 * i as u32
    }
    pub const STUB: u32 = SCRATCH + 0x5000;
    pub const END: u32 = SCRATCH + 0x5100;
    /// The probe's flags byte (bit `0x20`: it struck something) and its hit list; the list's count
    /// and arrays.
    pub const FLAGS: u32 = 0x126;
    pub const LIST: u32 = 0x7c;
    pub const COUNT: u32 = 0x8c;
    pub const LIST_FROMS: u32 = 0x90;
    pub const LIST_NORMALS: u32 = 0x94;
    pub const LIST_POINTS: u32 = 0x98;
    pub const LIST_RECORDS: u32 = 0x9c;
    /// A record's object and mover; an object's listener list, its count and items; a mover's
    /// kind.
    pub const RECORD_OBJECT: u32 = 0x90;
    pub const RECORD_MOVER: u32 = 0x8c;
    pub const OBJECT_LISTENERS: u32 = 0x208;
    pub const LISTENER_COUNT: u32 = 0x104;
    pub const LISTENER_ITEMS: u32 = 0x10c;
    pub const MOVER_KIND: u32 = 0x1a0;
}
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
    /// The car's sort rays and the sort's ray probe (MD127).
    pub const SORT_RAYS: u32 = 0xc4;
    pub const SORT_PROBE: u32 = 0x1264;
    /// A tire's probe words (MD127).
    pub const DROP: u32 = 0x60;
    pub const MOUNT: u32 = 0x70;
    pub const HELD: u32 = 0xac;
    pub const PROBES: u32 = 0xc8;
    pub const LEVER: u32 = 0xe0;
    pub const SURFACE: u32 = 0xf0;
    pub const GROUND_POINT: u32 = 0x100;
    pub const GROUND_NORMAL: u32 = 0x110;
    pub const CAR_NORMAL: u32 = 0x120;
    pub const UPPER: u32 = 0x140;
    pub const LOWER: u32 = 0x150;
    pub const TIRE_WORLD: u32 = 0x1a4;
    pub const HIT: u32 = 0x1b8;
    pub const PROBE_NORMAL: u32 = 0x1c0;
    pub const RADIUS: u32 = 0x1e8;
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
    sort_rays: u32,
    sort_probe: u32,
    tire_worlds: [u32; 4],
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
        sort_rays: ee.u32_at(CAR + at::SORT_RAYS),
        sort_probe: ee.u32_at(CAR + at::SORT_PROBE),
        tire_worlds: at::TIRES.map(|o| ee.u32_at(ee.u32_at(CAR + o) + at::TIRE_WORLD)),
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
        sort_rays: b(at::SORT_RAYS),
        sort: SortAnswer::default(),
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
        mount: get_v(ee, t + at::MOUNT),
        radius: F(ee.f32_at(t + at::RADIUS)),
        on_ground: b(at::TIRE_WORLD),
        probe_normal: get_v(ee, t + at::PROBE_NORMAL),
        car_normal: get_v(ee, t + at::CAR_NORMAL),
        drop: get_v(ee, t + at::DROP),
        lever: get_v(ee, t + at::LEVER),
        upper: get_v(ee, t + at::UPPER),
        lower: get_v(ee, t + at::LOWER),
        probes: ee.i32_at(t + at::PROBES),
        ground_point: get_v(ee, t + at::GROUND_POINT),
        ground_normal: get_v(ee, t + at::GROUND_NORMAL),
        held: b(at::HELD),
        surface: ee.u8_at(t + at::SURFACE),
        hit: ee.u32_at(t + at::HIT),
        query: (Vf::ZERO, Vf::ZERO),
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
    w(c(at::SORT_RAYS), if car.sort_rays { p.sort_rays } else { 0 });
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
        w(t + at::RADIUS, tire.radius.to_bits());
        w(t + at::HELD, word(tire.held));
        w(t + at::PROBES, tire.probes as u32);
        // The surface is the record's first byte; the other three the original never writes (zero
        // in the image).
        w(t + at::SURFACE, u32::from(tire.surface));
        w(t + at::HIT, tire.hit);
        for (o, v) in [
            (at::MOUNT, tire.mount),
            (at::PROBE_NORMAL, tire.probe_normal),
            (at::CAR_NORMAL, tire.car_normal),
            (at::DROP, tire.drop),
            (at::LEVER, tire.lever),
            (at::UPPER, tire.upper),
            (at::LOWER, tire.lower),
            (at::GROUND_POINT, tire.ground_point),
            (at::GROUND_NORMAL, tire.ground_normal),
        ] {
            vs.push((t + o, v));
        }
    }
    for (t, (tire, &world)) in p.tires.iter().zip(car.tires.iter().zip(&p.tire_worlds)) {
        w(t + at::TIRE_WORLD, if tire.on_ground { world } else { 0 });
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
    /// The probes' ground queries (MD127), by tire and query; how many each tire has asked; and the
    /// contact sort's search.
    ground: [[Ground; 5]; 4],
    asked: [usize; 4],
    sort: SortAnswer,
}

/// One answer of a probe's ground query: the height a step from the point asked, or from the tire's
/// contact; the normal, or the tire's last probe normal again (so two agree); the surface byte.
#[derive(Clone, Copy, Default)]
struct Ground {
    from_contact: bool,
    dy: f32,
    normal: Option<Vf>,
    surface: u8,
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

    /// The next ground query's answer for tire `k`, asked at `at`: the point moved onto the ground,
    /// the normal and the surface byte.
    fn ground(&mut self, k: usize, at: Vf, t: &Tire) -> (Vf, Vf, u8) {
        let g = self.ground[k][self.asked[k]];
        self.asked[k] += 1;
        let base = if g.from_contact { t.ground_point.y() } else { at.y() };
        let mut point = at;
        point.0[1] = base + g.dy;
        (point, g.normal.unwrap_or(t.probe_normal), g.surface)
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
            Call::TireGround(k, at) => {
                let (point, normal, surface) = self.script.ground(k, at, &car.tires[k]);
                car.tires[k].query = (point, normal);
                car.tires[k].surface = surface;
            }
            Call::SortSearch => car.sort = self.script.sort,
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
        if ee.r(31) as u32 == PROBE_GROUND {
            // A probe's (MD127), its record the tire's surface byte; the idle buggy's are left.
            let record = ee.r(8) as u32;
            let Some(k) = p.tires.iter().position(|&t| t + at::SURFACE == record) else { return };
            assert_eq!((ee.r(4) as u32, ee.r(7)), (p.tire_worlds[k], 0), "the probe's ground query's world and fourth");
            let (a1, a2) = (ee.r(5) as u32, ee.r(6) as u32);
            let asked = get_v(ee, a1);
            let t = read_tire(ee, p.tires[k]);
            let (point, normal, surface) = s.borrow_mut().ground(k, asked, &t);
            put_v(ee, a1, point);
            put_v(ee, a2, normal);
            ee.write(record, 1, u128::from(surface));
            l.borrow_mut().push(Call::TireGround(k, asked));
            return;
        }
        assert_eq!(ee.r(31) as u32, POST_GROUND, "a ground query from elsewhere");
        assert_eq!((ee.r(4) as u32, ee.r(7), ee.r(8)), (p.ground_world, 0, 0), "the ground query's world, fourth and record");
        l.borrow_mut().push(Call::GroundUnder(get_v(ee, ee.r(5) as u32)));
        put_v(ee, ee.r(6) as u32, s.borrow().normal);
    }));
    hook_sort(ee, p, log.clone(), script.clone());
    let (l, s) = (log, script);
    ee.hooks.insert(NODE_POSITION, Box::new(move |ee| {
        assert_eq!(ee.r(31) as u32, POST_NODE, "a node position from elsewhere");
        assert_eq!((ee.r(4) as u32, ee.r(5), ee.r(6) as u32), (p.node, 0, CAR + at::NODE_POS), "the node position's node and out");
        put_v(ee, CAR + at::NODE_POS, s.borrow().node_pos);
        l.borrow_mut().push(Call::NodePosition);
    }));
}

/// Hook what the contact sort calls (MD127), for when it runs unhooked: the ray probe's place and
/// search, on the car's probe, log and the search lays the script's hits out in [`sort::SCRATCH`]; the
/// idle buggy's are skipped (its probe's flags byte says it struck nothing). Each struck object's one
/// listener logs the hit.
fn hook_sort(ee: &mut Ee, p: Ptrs, log: Shared<Vec<Call>>, script: Shared<Script>) {
    let l = log.clone();
    ee.hooks.insert(SORT_PLACE, Box::new(move |ee| {
        if ee.r(4) as u32 != p.sort_probe {
            return;
        }
        assert_eq!((ee.r(31) as u32, ee.r(5) as u32), (POST_PLACE, CAR + 0x370), "the sort probe's place");
        l.borrow_mut().push(Call::SortPlace);
    }));
    let l = log.clone();
    ee.hooks.insert(SORT_SEARCH, Box::new(move |ee| {
        if ee.r(4) as u32 != p.sort_probe {
            return;
        }
        assert_eq!((ee.r(31) as u32, ee.r(5) as u32, ee.r(6)), (POST_SEARCH, p.sort_rays, 1), "the sort probe's search");
        let a = script.borrow().sort;
        let list = ee.u32_at(p.sort_probe + sort::LIST);
        for (o, to) in [
            (sort::LIST_FROMS, sort::FROMS),
            (sort::LIST_NORMALS, sort::NORMALS),
            (sort::LIST_POINTS, sort::POINTS),
            (sort::LIST_RECORDS, sort::RECORDS),
        ] {
            ee.set_u32(list + o, to);
        }
        ee.set_u32(list + sort::COUNT, a.count as u32);
        for (i, h) in a.hits.iter().enumerate() {
            let record = sort::record(i);
            put_v(ee, sort::FROMS + 0x20 * i as u32, h.from);
            put_v(ee, sort::NORMALS + 0x10 * i as u32, h.normal);
            put_v(ee, sort::POINTS + 0x10 * i as u32, h.point);
            ee.set_u32(sort::RECORDS + 4 * i as u32, record);
            ee.set_u32(record + sort::RECORD_OBJECT, h.object);
            ee.set_u32(record + sort::RECORD_MOVER, h.mover);
            if h.mover != 0 {
                ee.set_u32(h.mover + sort::MOVER_KIND, h.mover_kind);
            }
            if h.object != 0 {
                let x = sort::listeners(i);
                ee.set_u32(h.object + sort::OBJECT_LISTENERS, x);
                ee.set_u32(x + sort::LISTENER_COUNT, 1);
                ee.set_u32(x + sort::LISTENER_ITEMS, sort::items(i));
                ee.set_u32(sort::items(i), sort::item(i));
                ee.set_u32(sort::item(i), sort::VTABLE);
            }
        }
        ee.set_u32(sort::VTABLE + 0xc, sort::STUB);
        let flags = ee.u8_at(p.sort_probe + sort::FLAGS) & !0x20 | if a.found { 0x20 } else { 0 };
        ee.write(p.sort_probe + sort::FLAGS, 1, u128::from(flags));
        l.borrow_mut().push(Call::SortSearch);
    }));
    ee.hooks.insert(sort::STUB, Box::new(move |ee| {
        assert_eq!((ee.r(5), ee.r(6)), (0, 0), "a struck listener's arguments");
        let item = ee.r(4) as u32;
        let i = (0..SORT_HITS).position(|i| sort::item(i) == item).expect("a struck listener of ours");
        log.borrow_mut().push(Call::Struck(i));
    }));
}

/// The sort's hits and the probes' ground answers (MD127), off their own stream so the other words'
/// draws stay as they were: hits struck from rays hanging either side of `±0` round the body, now and
/// then on an object (with its listener) or a mover of either kind; heights a step from the point
/// asked or the contact, at or off the normal the probe has.
fn generate_sort(r: &mut Rng, s: &mut Script, body: &Body) {
    let side = |r: &mut Rng| match r.next() % 6 {
        0 => 0.0,
        1 => -0.0,
        2 => r.range(0.0, 5.0),
        3 => -r.range(0.0, 5.0),
        _ => r.range(-5.0, 5.0),
    };
    let upish = |r: &mut Rng| {
        if r.chance(0.6) {
            let mut n = r.vec(0.4, 0.0);
            n.0[1] = r.range(0.6, 1.2);
            n
        } else {
            r.vec(1.0, 0.0)
        }
    };
    s.sort.found = r.chance(0.6);
    s.sort.count = (r.next() % (SORT_HITS as u64 + 1)) as usize;
    for (i, h) in s.sort.hits.iter_mut().enumerate() {
        h.from = Vf::new(side(r), r.range(-2.0, 2.0), side(r), r.range(-1.0, 1.0));
        h.point = plus_v(body.position, r.vec(6.0, 1.0));
        h.normal = upish(r);
        h.object = if r.chance(0.4) { sort::object(i) } else { 0 };
        h.mover = if r.chance(0.5) { sort::mover(i) } else { 0 };
        h.mover_kind = if r.chance(0.6) { 2 } else { r.word() % 5 };
        h.record = sort::record(i);
    }
    for k in 0..4 {
        for g in &mut s.ground[k] {
            g.from_contact = r.chance(0.4);
            g.dy = match r.next() % 9 {
                0 => 0.0,
                1 => 0.1,
                2 => -0.1,
                3 => 0.05,
                4 => -0.05,
                5 => 0.2,
                6 => -0.2,
                _ => r.range(-3.0, 3.0),
            };
            g.normal = if r.chance(0.35) { None } else { Some(upish(r)) };
            g.surface = r.word() as u8;
        }
    }
}

fn plus_v(a: Vf, b: Vf) -> Vf {
    Vf::new(a.x() + b.x(), a.y() + b.y(), a.z() + b.z(), a.w())
}

/// The tires' probe words (MD127): now and then no ground to ask or no sort rays, last frame's normal
/// up-ish (or anything), the terrain's hold, last frame's contact round the body.
fn generate_probe(r: &mut Rng, car: &mut Car, body: &Body) {
    car.sort_rays = !r.chance(0.1);
    if r.chance(0.5) {
        car.up = Vf::new(0.0, 1.0, 0.0, car.up.w());
    }
    for t in &mut car.tires {
        t.on_ground = !r.chance(0.08);
        t.probe_normal = if r.chance(0.6) {
            let mut n = r.vec(0.3, 0.0);
            n.0[1] = 1.0;
            crate_unit(n)
        } else {
            let w = r.range(-1.0, 1.0);
            r.vec(1.0, w)
        };
        t.held = r.chance(0.5);
        t.ground_point = plus_v(body.position, r.vec(4.0, 1.0));
        if r.chance(0.5) {
            t.ground_normal = r.vec(1.0, 0.0);
        }
        t.probes = (r.next() % 7) as i32;
        t.surface = r.word() as u8;
        t.hit = if r.chance(0.5) { 0 } else { r.word() };
        t.car_normal = r.vec(1.0, 0.0);
        t.drop = r.vec(1.5, 0.0);
        t.lever = r.vec(5.0, 1.0);
        t.upper = r.vec(2000.0, 1.0);
        t.lower = r.vec(2000.0, 1.0);
    }
}

fn crate_unit(v: Vf) -> Vf {
    let n = (v.x() * v.x() + v.y() * v.y() + v.z() * v.z()).sqrt();
    Vf::new(v.x() / n, v.y() / n, v.z() / n, v.w())
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
/// car's, but for the vehicle state's mode, the gravity block's, the record's touching word, the
/// engine's torque scale and a wing's (from its first word). The left front tire's ground normal is
/// the tire's ([`at::GROUND_NORMAL`]).
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
        tires: core::array::from_fn(|k| read_tire_keys(ee, p.tires[k])),
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
        tires: core::array::from_fn(|k| read_tire_state(ee, p.tires[k])),
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
    let vs: [(u32, Vf); 5] = [
        (c(mx::COFG), k.cofg),
        (c(mx::COFG_AI), k.cofg_ai),
        (c(mx::CONTROL_MOMENT), k.airborne_control_moment),
        (m.wing_ptr + 0x10, m.wing.position),
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
    for (i, &t) in p.tires.iter().enumerate() {
        out.extend(tire_words(&k.tires[i], &s.tires[i], t));
    }
    out
}

/// The tire step's words (research/oracle.txt `# MD124`; the keys' are MD39b's): a tire's, and the
/// car's two steer words its steer pointers name.
mod tire {
    pub const STEER_WORD: u32 = 0x113c;
    pub const REAR_STEER_WORD: u32 = 0x1140;
    pub const BRAKE_PTR: u32 = 0x24;
    pub const STEER_PTR: u32 = 0x28;
    pub const MAX_COMPRESSION: u32 = 0x30;
    pub const MAX_COMPRESSION_RATE: u32 = 0x34;
    pub const MAX_DECOMPRESSION_RATE: u32 = 0x38;
    pub const SPRING: u32 = 0x3c;
    pub const BOUNCE: u32 = 0x40;
    pub const REBOUND: u32 = 0x44;
    pub const INV_INERTIA: u32 = 0x48;
    pub const MAX_STEER: u32 = 0x50;
    pub const DROP: u32 = 0x60;
    pub const MOUNT: u32 = 0x70;
    pub const TOP_SPEED: u32 = 0x90;
    pub const ANGLE_RATE: u32 = 0x94;
    pub const ABS: u32 = 0x98;
    pub const MIN_SLIP: u32 = 0x9c;
    pub const MAX_SLIP: u32 = 0xa0;
    pub const COMPRESSION: u32 = 0xcc;
    pub const LAST_COMPRESSION: u32 = 0xd0;
    pub const LEVER: u32 = 0xe0;
    pub const SURFACE: u32 = 0xf0;
    pub const GROUND_POINT: u32 = 0x100;
    pub const GROUND_NORMAL: u32 = 0x110;
    pub const CAR_NORMAL: u32 = 0x120;
    pub const CONTACT: u32 = 0x130;
    pub const UPPER: u32 = 0x140;
    pub const LOWER: u32 = 0x150;
    pub const POINT_VELOCITY: u32 = 0x160;
    pub const BODY_VELOCITY: u32 = 0x170;
    pub const ANGLE: u32 = 0x190;
    pub const SPIN: u32 = 0x194;
    pub const GRIP: u32 = 0x1d0;
    pub const SLIPPING: u32 = 0x1d4;
    pub const SLIDING: u32 = 0x1d8;
    pub const LOCKED: u32 = 0x1dc;
    pub const TORQUE: u32 = 0x1e0;
    pub const LAST_TORQUE: u32 = 0x1e4;
    pub const RADIUS: u32 = 0x1e8;
    pub const SLIP_RATIO: u32 = 0x1fc;
    pub const SLIP_ANGLE: u32 = 0x200;
    pub const SLIP_TAN: u32 = 0x204;
    pub const VELOCITY: u32 = 0x210;
    pub const LAST_VELOCITY: u32 = 0x220;
    pub const FORCE: u32 = 0x230;
    pub const LAST_FORCE: u32 = 0x240;
    pub const STEER: u32 = 0x250;
    pub const FRAME: u32 = 0x260;
    pub const SIDE_RATIO: u32 = 0x2a0;
    /// Sixteen surfaces' tables, one after another, each of sixteen words (one per surface).
    pub const SURFACES: u32 = 0x5a4;
    pub const FRICTION_SCALE: u32 = 0x9a4;
    pub const CAMBER: u32 = 0x9ac;
    pub const ROLL_RATIO: u32 = 0x9b0;
    /// How much of a tire the step may write.
    pub const SIZE: u32 = 0x9b4;
}

/// Where a lever's word lives.
fn lever_at(l: Option<Lever>) -> u32 {
    match l {
        None => 0,
        Some(Lever::Brake) => CAR + mx::BRAKE,
        Some(Lever::BrakeForce) => CAR + mx::BRAKE_FORCE,
        Some(Lever::Steer) => CAR + tire::STEER_WORD,
        Some(Lever::RearSteer) => CAR + tire::REAR_STEER_WORD,
    }
}

fn lever(a: u32) -> Option<Lever> {
    [Lever::Brake, Lever::BrakeForce, Lever::Steer, Lever::RearSteer]
        .into_iter()
        .find(|&l| lever_at(Some(l)) == a)
        .or_else(|| {
            assert_eq!(a, 0, "a tire's lever pointer");
            None
        })
}

/// A surface's sixteen words, in their tables' order.
fn surface_fields(s: &mut Surface) -> [&mut F; 16] {
    [
        &mut s.max_braking_torque,
        &mut s.wheel_friction,
        &mut s.c1,
        &mut s.c2,
        &mut s.c3,
        &mut s.c4,
        &mut s.a0,
        &mut s.a1,
        &mut s.a3,
        &mut s.inv_a2,
        &mut s.inv_a4,
        &mut s.moment_scale,
        &mut s.cornering_stiffness,
        &mut s.max_normal_load,
        &mut s.friction_peak,
        &mut s.friction_end_point,
    ]
}

fn read_tire_keys(ee: &Ee, t: u32) -> TireKeys {
    let f = |o: u32| F(ee.f32_at(t + o));
    let mut k = TireKeys {
        brake: lever(ee.u32_at(t + tire::BRAKE_PTR)),
        steer: lever(ee.u32_at(t + tire::STEER_PTR)),
        max_compression: f(tire::MAX_COMPRESSION),
        max_compression_rate: f(tire::MAX_COMPRESSION_RATE),
        max_decompression_rate: f(tire::MAX_DECOMPRESSION_RATE),
        spring: f(tire::SPRING),
        bounce_damping: f(tire::BOUNCE),
        rebound_damping: f(tire::REBOUND),
        inv_inertia: f(tire::INV_INERTIA),
        max_steer_angle: f(tire::MAX_STEER),
        top_wheel_speed: f(tire::TOP_SPEED),
        max_tire_angle_rate: f(tire::ANGLE_RATE),
        abs: ee.u32_at(t + tire::ABS) != 0,
        min_braking_slip: f(tire::MIN_SLIP),
        max_braking_slip: f(tire::MAX_SLIP),
        friction_scale: f(tire::FRICTION_SCALE),
        camber_roll_ratio: f(tire::ROLL_RATIO),
        surfaces: [Surface::default(); 16],
    };
    for (i, s) in k.surfaces.iter_mut().enumerate() {
        for (j, x) in surface_fields(s).into_iter().enumerate() {
            *x = f(tire::SURFACES + 0x40 * j as u32 + 4 * i as u32);
        }
    }
    k
}

fn read_tire_state(ee: &Ee, t: u32) -> TireState {
    let f = |o: u32| F(ee.f32_at(t + o));
    let b = |o: u32| ee.u32_at(t + o) != 0;
    let v = |o: u32| get_v(ee, t + o);
    let surface = ee.u32_at(t + tire::SURFACE);
    assert!(surface < 16, "a tire's surface word {surface:#x}");
    TireState {
        compression: f(tire::COMPRESSION),
        last_compression: f(tire::LAST_COMPRESSION),
        contact: v(tire::CONTACT),
        point_velocity: v(tire::POINT_VELOCITY),
        body_velocity: v(tire::BODY_VELOCITY),
        angle: f(tire::ANGLE),
        spin: f(tire::SPIN),
        low_speed_grip: b(tire::GRIP),
        slipping: b(tire::SLIPPING),
        sliding: b(tire::SLIDING),
        locked: b(tire::LOCKED),
        torque: f(tire::TORQUE),
        last_torque: f(tire::LAST_TORQUE),
        slip_ratio: f(tire::SLIP_RATIO),
        slip_angle: f(tire::SLIP_ANGLE),
        slip_tan: f(tire::SLIP_TAN),
        velocity: v(tire::VELOCITY),
        last_velocity: v(tire::LAST_VELOCITY),
        force: v(tire::FORCE),
        last_force: v(tire::LAST_FORCE),
        steer: f(tire::STEER),
        frame: core::array::from_fn(|k| v(tire::FRAME + 0x10 * k as u32)),
        side_ratio: f(tire::SIDE_RATIO),
        camber: f(tire::CAMBER),
    }
}

/// Every word [`read_tire_keys`] and [`read_tire_state`] read, where it lives.
fn tire_words(k: &TireKeys, s: &TireState, t: u32) -> Vec<(u32, u32)> {
    let mut fs: Vec<(u32, F)> = vec![
        (tire::MAX_COMPRESSION, k.max_compression),
        (tire::MAX_COMPRESSION_RATE, k.max_compression_rate),
        (tire::MAX_DECOMPRESSION_RATE, k.max_decompression_rate),
        (tire::SPRING, k.spring),
        (tire::BOUNCE, k.bounce_damping),
        (tire::REBOUND, k.rebound_damping),
        (tire::INV_INERTIA, k.inv_inertia),
        (tire::MAX_STEER, k.max_steer_angle),
        (tire::TOP_SPEED, k.top_wheel_speed),
        (tire::ANGLE_RATE, k.max_tire_angle_rate),
        (tire::MIN_SLIP, k.min_braking_slip),
        (tire::MAX_SLIP, k.max_braking_slip),
        (tire::FRICTION_SCALE, k.friction_scale),
        (tire::ROLL_RATIO, k.camber_roll_ratio),
        (tire::COMPRESSION, s.compression),
        (tire::LAST_COMPRESSION, s.last_compression),
        (tire::ANGLE, s.angle),
        (tire::SPIN, s.spin),
        (tire::TORQUE, s.torque),
        (tire::LAST_TORQUE, s.last_torque),
        (tire::SLIP_RATIO, s.slip_ratio),
        (tire::SLIP_ANGLE, s.slip_angle),
        (tire::SLIP_TAN, s.slip_tan),
        (tire::STEER, s.steer),
        (tire::SIDE_RATIO, s.side_ratio),
        (tire::CAMBER, s.camber),
    ];
    let mut surfaces = k.surfaces;
    for (i, sf) in surfaces.iter_mut().enumerate() {
        for (j, x) in surface_fields(sf).into_iter().enumerate() {
            fs.push((tire::SURFACES + 0x40 * j as u32 + 4 * i as u32, *x));
        }
    }
    let mut vs: Vec<(u32, Vf)> = vec![
        (tire::CONTACT, s.contact),
        (tire::POINT_VELOCITY, s.point_velocity),
        (tire::BODY_VELOCITY, s.body_velocity),
        (tire::VELOCITY, s.velocity),
        (tire::LAST_VELOCITY, s.last_velocity),
        (tire::FORCE, s.force),
        (tire::LAST_FORCE, s.last_force),
    ];
    for (j, r) in s.frame.iter().enumerate() {
        vs.push((tire::FRAME + 0x10 * j as u32, *r));
    }
    for (a, v) in vs {
        for (j, x) in v.0.into_iter().enumerate() {
            fs.push((a + 4 * j as u32, F(x)));
        }
    }
    let mut out: Vec<(u32, u32)> = fs.into_iter().map(|(o, x)| (t + o, x.to_bits())).collect();
    out.extend([
        (t + tire::BRAKE_PTR, lever_at(k.brake)),
        (t + tire::STEER_PTR, lever_at(k.steer)),
        (t + tire::ABS, word(k.abs)),
        (t + tire::GRIP, word(s.low_speed_grip)),
        (t + tire::SLIPPING, word(s.slipping)),
        (t + tire::SLIDING, word(s.sliding)),
        (t + tire::LOCKED, word(s.locked)),
    ]);
    out
}

/// `v` turned by the unit quaternion `q` (`x, y, z` the axis part), natively: only to aim generated
/// grounds at the wheels.
fn rotate(q: Vf, v: [f32; 3]) -> [f32; 3] {
    let u = [q.x(), q.y(), q.z()];
    let w = q.w();
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let t = cross(u, v).map(|x| 2.0 * x);
    let tt = cross(u, t);
    core::array::from_fn(|k| v[k] + w * t[k] + tt[k])
}

/// The tires' keys and state stirred about the image's, after [`generate_mx`]: each tire's ground
/// aimed along its travel (short of it, within it, bottomed past it, past twice it), its normal
/// near the car's up, slip ratios near locking, the ABS on with its slips degenerate now and then,
/// steer and brake pointers null now and then, every surface's model stirred; a third of the bodies
/// slowed into the slip's slow model. The fronts are handed no mover (the rears' come from them).
fn generate_tires(r: &mut Rng, m: &mut Mx, car: &mut Car, body: &mut Body) {
    if r.chance(0.35) {
        let v = body.velocity;
        let n = (v.x() * v.x() + v.y() * v.y() + v.z() * v.z()).sqrt().max(1e-3);
        let to = if r.chance(0.5) { r.range(0.0, 6.0) } else { r.range(0.0, 25.0) };
        body.velocity = Vf::new(v.x() * to / n, v.y() * to / n, v.z() * to / n, v.w());
        if r.chance(0.3) {
            body.rates = Vf::new(body.rates.x() * 0.05, body.rates.y() * 0.05, body.rates.z() * 0.05, body.rates.w());
        }
    }
    for t in &mut car.tires[..atv_car::REAR_LEFT] {
        t.handed_on = [0, 0];
    }
    let up = rotate(body.quat, [0.0, 1.0, 0.0]);
    let cofg = car.cofg;
    let s = &mut m.state;
    s.steer = F(if r.chance(0.3) { 0.0 } else { r.range(-1.0, 1.0) });
    s.rear_steer = F(if r.chance(0.5) { 0.0 } else { r.range(-1.0, 1.0) });
    for i in 0..4 {
        let (k, st, ct) = (&mut m.keys.tires[i], &mut s.tires[i], &mut car.tires[i]);
        let stir = |r: &mut Rng, x: F| F(x.0 * r.range(0.6, 1.4));
        if r.chance(0.1) {
            k.brake = None;
        }
        if r.chance(0.1) {
            k.steer = None;
        }
        k.max_compression = stir(r, k.max_compression);
        k.max_compression_rate = F(r.range(2.0, 20.0));
        k.max_decompression_rate = F(r.range(2.0, 20.0));
        k.spring = F(r.range(300.0, 1500.0));
        k.bounce_damping = F(r.range(20.0, 200.0));
        k.rebound_damping = F(r.range(20.0, 200.0));
        k.inv_inertia = F(r.range(0.02, 0.5));
        k.max_steer_angle = F(r.range(0.0, 0.8));
        k.top_wheel_speed = F(r.range(20.0, 300.0));
        k.max_tire_angle_rate = if r.chance(0.5) { F(f32::MAX) } else { F(r.range(5.0, 100.0)) };
        k.abs = r.chance(0.5);
        k.min_braking_slip = F(r.range(0.0, 0.3));
        k.max_braking_slip = if r.chance(0.2) { k.min_braking_slip } else { F(k.min_braking_slip.0 + r.range(0.05, 0.4)) };
        ct.radius = stir(r, ct.radius);
        k.friction_scale = F(r.range(0.5, 1.5));
        k.camber_roll_ratio = F(r.range(0.0, 1.0));
        let base = k.surfaces[0];
        for sf in &mut k.surfaces {
            *sf = base;
            for x in surface_fields(sf) {
                *x = F(x.0 * r.range(0.5, 1.5));
            }
            sf.wheel_friction = F(r.range(0.0, 200.0));
            sf.max_braking_torque = F(r.range(0.0, 3000.0));
            sf.max_normal_load = if r.chance(0.1) { F::ZERO } else { F(r.range(100.0, 2500.0)) };
        }
        ct.surface = (r.next() % 16) as u8;
        // The normal near the car's up (or anywhere), and the ground point at a chosen share of the
        // wheel's travel from its lower end.
        let n = if r.chance(0.15) {
            let v = r.vec(1.0, 0.0);
            [v.x(), v.y(), v.z()]
        } else {
            let a = up.map(|x| x + r.range(-0.2, 0.2));
            let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-3);
            a.map(|x| x / l)
        };
        ct.ground_normal = Vf::new(n[0], n[1], n[2], 0.0);
        let rel = |v: Vf| [v.x() - cofg.x(), v.y() - cofg.y(), v.z() - cofg.z()];
        let lower_local = rel(Vf::new(ct.mount.x() + ct.drop.x(), ct.mount.y() + ct.drop.y(), ct.mount.z() + ct.drop.z(), 0.0));
        let upper_local = rel(ct.mount);
        let (lo, hi) = (rotate(body.quat, lower_local), rotate(body.quat, upper_local));
        let pos = [body.position.x(), body.position.y(), body.position.z()];
        let ratio = match r.next() % 6 {
            0 => r.range(-0.5, 0.0),
            1 | 2 => r.range(0.0, 1.0),
            3 | 4 => r.range(1.0, 2.0),
            _ => r.range(2.0, 3.0),
        };
        let side = r.vec(3.0, 0.0);
        let point: [f32; 3] = core::array::from_fn(|j| pos[j] + lo[j] + (hi[j] - lo[j]) * ratio + side.0[j]);
        ct.ground_point = Vf::new(point[0], point[1], point[2], r.range(-2.0, 2.0));
        let (nx, ny) = (r.range(-0.6, 0.6), r.range(0.3, 1.0));
        ct.car_normal = Vf::new(nx, ny, ct.car_normal.z(), ct.car_normal.w());
        st.compression = F(r.range(0.0, 2.0));
        st.last_compression = F(r.range(0.0, 2.5));
        for v in [&mut ct.lever, &mut st.contact, &mut ct.upper, &mut ct.lower, &mut st.point_velocity, &mut st.body_velocity] {
            let w = r.range(-2.0, 2.0);
            *v = r.vec(50.0, w);
        }
        st.angle = F(r.range(-3.1, 3.1));
        st.spin = F(r.range(-300.0, 300.0));
        st.low_speed_grip = r.chance(0.5);
        st.slipping = r.chance(0.5);
        st.sliding = r.chance(0.5);
        st.locked = r.chance(0.5);
        st.torque = F(r.range(-3000.0, 3000.0));
        st.last_torque = F(r.range(-3000.0, 3000.0));
        st.slip_ratio = F(if r.chance(0.3) {
            let x = r.range(0.9, 1.0);
            if r.chance(0.8) { x } else { -x }
        } else {
            r.range(-1.0, 1.0)
        });
        st.slip_angle = F(r.range(-1.6, 1.6));
        st.slip_tan = F(r.range(-3.0, 3.0));
        for v in [&mut st.velocity, &mut st.last_velocity, &mut st.force, &mut st.last_force] {
            let w = r.range(-2.0, 2.0);
            *v = r.vec(40.0, w);
        }
        st.last_force.0[2] = r.range(-1500.0, 1500.0);
        st.steer = F(if r.chance(0.3) { 0.0 } else { r.range(-0.6, 0.6) });
        for v in &mut st.frame {
            let w = r.range(-2.0, 2.0);
            *v = r.vec(1.0, w);
        }
        st.side_ratio = F(r.range(-2.0, 2.0));
        st.camber = F(r.range(-0.1, 0.1));
    }
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
    // The left front tire's ground normal (the car's, MD127).
    car.tires[0].ground_normal = if r.chance(0.3) { Vf::new(0.0, 1.0, 0.0, 0.0) } else { r.vec(1.0, 0.0) };
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
            Call::TireStep(..) => TIRE_STEP,
            Call::SortContacts => SORT_CONTACTS,
            Call::Probe(..) => PROBE,
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
        let (force, log, position) = (body.force, self.pieces.rest.log.len(), body.position);
        let car0 = *car;
        assert!(self.pieces.answer(c, car, body), "{c:?} unanswered");
        let calls = &self.pieces.rest.log[log..];
        match c {
            Call::TireStep(i, _) => {
                tire_tags(&k.tires[i], &s0, &self.pieces.state, i, &car0.tires[i], &car.tires[i], body.position != position, &mut tags);
            }
            Call::SortContacts if !car0.sort_rays => tags.push("sort: no rays"),
            Call::SortContacts if !car.sort.found => tags.push("sort: nothing found"),
            Call::SortContacts => {
                let a = car.sort;
                for h in &a.hits[..a.count] {
                    tags.push(match (h.from.z() <= 0.0, h.from.x() <= 0.0) {
                        (false, false) => "sort: front right",
                        (false, true) => "sort: front left",
                        (true, false) => "sort: rear right",
                        (true, true) => "sort: rear left",
                    });
                    if h.from.z() == 0.0 || h.from.x() == 0.0 {
                        tags.push("sort: on a zero");
                    }
                    if h.object != 0 {
                        tags.push("sort: struck");
                    }
                    if h.mover != 0 {
                        tags.push(if h.mover_kind == 2 { "sort: mover kept" } else { "sort: mover dropped" });
                    }
                }
            }
            Call::Probe(k, _) => {
                let (t0, t) = (car0.tires[k], car.tires[k]);
                let asked = calls.iter().filter(|c| matches!(c, Call::TireGround(..))).count();
                if !t0.on_ground {
                    tags.push("probe: no ground");
                } else {
                    tags.push(match asked {
                        1 => "probe: asked once",
                        5 => "probe: asked five times",
                        _ => "probe: asked again",
                    });
                    let dropped = t0.sorted && car0.up.x() * t0.ground_normal.x() + car0.up.y() * t0.ground_normal.y() + car0.up.z() * t0.ground_normal.z() < f32::from_bits(0x3f34_fdf4);
                    if dropped {
                        tags.push("probe: sorted dropped by up");
                    }
                    let below = t.query.0.y() < t.lower.y();
                    if below {
                        tags.push(if t0.sorted && !dropped { "probe: below, surface cleared" } else { "probe: below" });
                    } else if t0.sorted && !dropped {
                        let a = [t.upper.x() - t.lower.x(), t.upper.y() - t.lower.y(), t.upper.z() - t.lower.z()];
                        let n = t0.ground_normal;
                        let facing = (a[0] * n.x() + a[1] * n.y() + a[2] * n.z()) / (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
                        if facing < 0.7 {
                            tags.push("probe: held by the axle");
                        }
                        tags.push(match (t0.held, t.held, t.solid) {
                            (_, true, true) if t0.held => "probe: held kept",
                            (_, true, true) => "probe: held taken",
                            (true, false, false) => "probe: let go",
                            (_, _, false) => "probe: sorted kept",
                            _ => "probe: held",
                        });
                    } else {
                        tags.push("probe: solid on terrain");
                    }
                }
            }
            _ => {}
        }
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

/// What one tire step found: its suspension's, roll's and slip's branches.
#[allow(clippy::too_many_arguments)]
fn tire_tags(k: &TireKeys, s0: &State, s1: &State, i: usize, t0: &Tire, t: &Tire, pushed: bool, tags: &mut Vec<&'static str>) {
    let (st0, st) = (&s0.tires[i], &s1.tires[i]);
    let read = |l: Option<Lever>| {
        l.map(|l| match l {
            Lever::Brake => s0.brake,
            Lever::BrakeForce => s0.brake_force,
            Lever::Steer => s0.steer,
            Lever::RearSteer => s0.rear_steer,
        })
    };
    let brake = read(k.brake);
    let mut tag = |what: &'static str, yes: bool| {
        if yes {
            tags.push(what);
        }
    };
    tag("tire: down", t.grounded);
    tag("tire: bottomed, pushed", pushed);
    tag("tire: bottomed, held", t.grounded && !pushed && st.compression == k.max_compression && !t.solid);
    tag("tire: handed", !t.grounded && t0.handed);
    tag("tire: in the air", !t.grounded && !t0.handed);
    tag("tire: unloaded", t.grounded && t.load == 0.0);
    tag("tire: lone front", t.grounded && t.lone_front);
    tag("tire: no pointer", k.brake.is_none() || k.steer.is_none());
    let braked = brake.is_some_and(|b| b > 0.0);
    tag("tire: braked", braked);
    tag("tire: abs", braked && k.abs && t.grounded && k.min_braking_slip != k.max_braking_slip);
    tag("tire: abs, degenerate", braked && k.abs && t.grounded && k.min_braking_slip == k.max_braking_slip);
    tag("tire: steered", t.grounded && st.steer != 0.0);
    tag("tire: straight", t.grounded && st.steer == 0.0);
    tag("tire: locked", st.locked);
    let speed = st.velocity.z().abs();
    tag("tire: fast", speed > 20.0);
    tag("tire: slow", speed <= 20.0 && !(speed < 5.0));
    tag("tire: crawling", speed < 5.0);
    tag("tire: low-speed grip", speed <= 20.0 && st.low_speed_grip);
    tag("tire: grip let go", st0.low_speed_grip && !st.low_speed_grip);
    tag("tire: wheel wraps", (st.angle.0 - st0.angle.0).abs() > 3.0);
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
    pieces_over(
        6000,
        0x0126_0002,
        &[CONTROLS, TRAVEL_COFG, ANTI_SLIDE, PRELOAD, LAUNCH, AERO],
        &["controls", "travel", "anti-slide", "preload", "launch", "aero"],
        48,
    );
}

/// Each piece unhooked alone, and all five, under the frame's oracle: the original's own code for
/// the piece against [`Pieces`]'s port, every word and call equal; each of the piece's branches seen.
fn pieces(seed: u64, unhooked: &[u32], prefixes: &[&str], want: usize) {
    pieces_over(3000, seed, unhooked, prefixes, want);
}

/// [`pieces`] over `cases` frames. The six- and eight-piece runs need 6,000: "preload: fired, degenerate" is
/// rare (it averaged about 17 per 3,000 frames once MD126's controls joined the stream), too close
/// to the floor of 20 for any seed to stay above it (B36).
fn pieces_over(cases: usize, seed: u64, unhooked: &[u32], prefixes: &[&str], want: usize) {
    let Some((_, seen)) = run(seed, cases, unhooked) else { return };
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
fn tire_step_is_mx_unleasheds() {
    pieces(0x0124_a001, &[TIRE_STEP], &["tire"], 20);
}

#[test]
fn contact_sort_is_mx_unleasheds() {
    pieces(0x0127_0001, &[SORT_CONTACTS], &["sort"], 10);
}

#[test]
fn sort_and_probes_are_mx_unleasheds() {
    pieces(0x0127_0002, &[SORT_CONTACTS, PROBE], &["sort", "probe"], 10 + 13);
}

#[test]
fn all_five_pieces_are_mx_unleasheds() {
    pieces(0x0106_a006, &[TRAVEL_COFG, ANTI_SLIDE, PRELOAD, LAUNCH, AERO], &["travel", "anti-slide", "preload", "launch", "aero"], 26);
}

#[test]
fn all_six_pieces_with_the_tire_step_are_mx_unleasheds() {
    let all = [TRAVEL_COFG, ANTI_SLIDE, PRELOAD, LAUNCH, AERO, TIRE_STEP];
    pieces_over(6000, 0x0124_a002, &all, &["travel", "anti-slide", "preload", "launch", "aero", "tire"], 46);
}

#[test]
fn all_eight_pieces_are_mx_unleasheds() {
    // 6,000 frames, as the six-piece run: the degenerate preload fell to 19 in 3,000 (F31b5l).
    pieces_over(
        6000,
        0x0127_0003,
        &[TRAVEL_COFG, SORT_CONTACTS, PROBE, ANTI_SLIDE, PRELOAD, LAUNCH, AERO, TIRE_STEP],
        &["travel", "sort", "probe", "anti-slide", "preload", "launch", "aero", "tire"],
        // Every tag but the sort's struck object and kept mover.
        46 + 10 + 13 - 2,
    );
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
    let mut r2 = Rng(seed ^ 0x0127_5eed_0127_5eed);
    let mut seen = Seen::new();
    let mut pieces_seen = Seen::new();
    let mut see = |what: &'static str, yes: bool| {
        if yes {
            *seen.entry(what).or_insert(0) += 1;
        }
    };
    let blocks: Vec<(u32, u32)> = [(CAR, 0x1500), (p.body, 0x230), (p.record + at::TIPPED, 4), (p.engine + mx::BOOST, 4)]
        .into_iter()
        .chain(p.tires.map(|t| (t, tire::SIZE)))
        .collect();
    for case in 0..cases {
        let (mut car, mut body, dt) = generate(&mut r, &image_car, &image_body, &p);
        let mut m = generate_mx(&mut r, &image_mx, &mut car, &mut body);
        generate_tires(&mut r, &mut m, &mut car, &mut body);
        // The rears' friction scales (MD126) are their tires' (MD124): one word each.
        m.state.rear_friction = [2, 3].map(|t| m.keys.tires[t].friction_scale);
        let mut s = Script::new(&mut r);
        if unhooked.contains(&SORT_CONTACTS) || unhooked.contains(&PROBE) {
            generate_probe(&mut r2, &mut car, &body);
            generate_sort(&mut r2, &mut s, &body);
            // With the tire step unhooked too, what the port's step leaves to the rest or does
            // not prove (MD124) stays out: a tire standing on a struck object or a kept mover, and
            // a surface byte past the sixteen tables.
            if unhooked.contains(&TIRE_STEP) {
                for h in &mut s.sort.hits {
                    h.object = 0;
                    if h.mover_kind == 2 {
                        h.mover_kind = 3;
                    }
                }
                for g in s.ground.iter_mut().flatten() {
                    g.surface &= 15;
                }
                for t in &mut car.tires {
                    t.surface &= 15;
                }
            }
        }
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
        let pm = Mx { keys: Keys { tires: port.pieces.keys.tires, ..m.keys.clone() }, state: port.pieces.state, ..m };
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
        let (u, hazard_words, mut car, dt) = generate_update(&mut r, &image_car);
        // The update's surface index is the tire's surface byte (MD127's [`Tire::surface`]).
        for (t, &k) in car.tires.iter_mut().zip(&u.tire_surface) {
            t.surface = k;
        }

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

// MD106b — SLUS-21104's own calls in the frame, unhooked.

/// What the five calls MD106b ports hand out in turn: the map-edge test, the rumble's switch, the
/// sound cue, the wrecked look on and off, the car's respawn, the body's reset, the spatial update,
/// the collision sync and the node's bounds.
const EDGE_TEST: u32 = 0x004a_4f20;
const RUMBLE_ON: u32 = 0x0051_a250;
const RUMBLE_RA: u32 = 0x004a_841c;
const LOOK_ON: u32 = 0x0019_86c0;
const LOOK_OFF: u32 = 0x004b_82c0;
const RESPAWN: u32 = 0x004c_ab10;
const BODY_RESET: u32 = 0x0049_4ae0;
const SPATIAL_UPDATE: u32 = 0x0049_2c40;
const COLLISION_SYNC: u32 = 0x0049_42c0;
const NODE_BOUNDS: u32 = 0x0019_6e80;
const SUBTREE_OUT_OF_DATE: u32 = 0x0019_3d70;

/// The vehicle's words MD106b's calls keep (research/oracle.txt `# MD106b`).
mod own {
    pub const FLAGS_WORD: u32 = 0x44;
    pub const WORLD: u32 = 0xe8;
    pub const BOUNDED: u32 = 0x308;
    pub const KIND: u32 = 0xbbc;
    pub const VEHICLE_STATE: u32 = 0xbc4;
    pub const CLOCK: u32 = 0xc84;
    pub const MAX_WRECK_TIME: u32 = 0xc88;
    pub const COUNTING: u32 = 0xc8c;
    pub const WRECK_DAMPING: u32 = 0xca0;
    pub const WRECK_INERTIA: u32 = 0xcb0;
    pub const INERTIA: u32 = 0xcc0;
    pub const CUE_TAG: u32 = 0xd20;
    pub const CUE_SOURCE: u32 = 0x674;
    pub const TIPPED_WRECK: u32 = 0x1118;
    pub const WRECKS_ON: u32 = 0x111c;
    pub const LOOK: u32 = 0x12b0;
    /// The vehicle state's mode (1 the player's) and pad.
    pub const MODE: u32 = 0x28;
    pub const PAD: u32 = 0x5c;
    /// The record's touching word; the body's inertia; the world's drag, gravity, floor and box.
    pub const TOUCHING: u32 = 0x34;
    pub const BODY_INERTIA: u32 = 0x130;
    pub const DRAG: u32 = 4;
    pub const GRAVITY: u32 = 0x40;
    pub const FLOOR: u32 = 0x98;
    pub const AREA: u32 = 0xa8;
}

/// The car's node and its parents.
fn car_scene_nodes(ee: &Ee, first: u32) -> Vec<u32> {
    let mut out = vec![first];
    while let n @ 1.. = ee.u32_at(out[out.len() - 1] + 0xf0) {
        assert!(!out.contains(&n), "a loop of parents");
        out.push(n);
    }
    out
}

/// The image's car's MD106b pointers.
#[derive(Clone)]
struct OwnPtrs {
    world: u32,
    state: u32,
    look: u32,
    area: u32,
    nodes: Vec<u32>,
}

fn own_ptrs(ee: &Ee, p: &Ptrs) -> OwnPtrs {
    let world = ee.u32_at(CAR + own::WORLD);
    let nodes = car_scene_nodes(ee, p.node);
    assert!(nodes.len() <= 4, "the car's node has {} parents, past the 3 a Scene holds", nodes.len() - 1);
    OwnPtrs {
        world,
        state: ee.u32_at(CAR + own::VEHICLE_STATE),
        look: ee.u32_at(CAR + own::LOOK),
        area: ee.u32_at(world + own::AREA),
        nodes,
    }
}

fn read_vehicle(ee: &Ee, p: &Ptrs, o: &OwnPtrs) -> atv_car::Vehicle {
    let b = |a: u32| ee.u32_at(a) != 0;
    let index = |a: u32| o.nodes.iter().position(|&n| n == a);
    let mut nodes = [game_sim::atv_moto::Node::default(); 4];
    for (n, &a) in nodes.iter_mut().zip(&o.nodes) {
        *n = game_sim::atv_moto::Node {
            local: std::array::from_fn(|r| get_v(ee, a + 0x30 + 0x10 * r as u32)),
            world: std::array::from_fn(|r| get_v(ee, a + 0x70 + 0x10 * r as u32)),
            parent: index(ee.u32_at(a + 0xf0)),
            child: index(ee.u32_at(a + 0xf4)),
            sibling: index(ee.u32_at(a + 0xf8)),
            flags: ee.u8_at(a + 0x100),
            ..Default::default()
        };
    }
    let mode = ee.u32_at(o.state + own::MODE);
    atv_car::Vehicle {
        wrecks_on: b(CAR + own::WRECKS_ON),
        bounded: b(CAR + own::BOUNDED),
        clock: F(ee.f32_at(CAR + own::CLOCK)),
        max_wreck_time: F(ee.f32_at(CAR + own::MAX_WRECK_TIME)),
        counting: b(CAR + own::COUNTING),
        tipped_wreck: b(CAR + own::TIPPED_WRECK),
        kind: ee.u32_at(CAR + own::KIND),
        wreck_damping: get_v(ee, CAR + own::WRECK_DAMPING),
        wreck_inertia: get_v(ee, CAR + own::WRECK_INERTIA),
        inertia: get_v(ee, CAR + own::INERTIA),
        body_inertia: get_v(ee, p.body + own::BODY_INERTIA),
        rumble: o.state != 0 && mode == 1 && b(o.state + own::PAD),
        look: b(CAR + own::LOOK),
        touching: b(p.record + own::TOUCHING),
        record: p.record != 0,
        gravity: get_v(ee, o.world + own::GRAVITY),
        drag: F(ee.f32_at(o.world + own::DRAG)),
        flags: ee.u8_at(CAR + own::FLAGS_WORD + 1),
        scene: game_sim::atv_moto::Scene { nodes },
        subtree: ee.u32_at(p.node + 0xf4) != 0,
        bounds: game_sim::atv_moto::Bounds {
            floor: F(ee.f32_at(o.world + own::FLOOR)),
            area: (o.area != 0).then(|| (get_v(ee, o.area + 0x30), get_v(ee, o.area + 0x40))),
        },
        out_of_bounds: false,
    }
}

/// Every word [`read_vehicle`] reads, where it lives; words a byte of which is the port's take their
/// other bytes from `ee` (the image).
fn vehicle_words(ee: &Ee, v: &atv_car::Vehicle, p: &Ptrs, o: &OwnPtrs) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let mut w = |a: u32, x: u32| out.push((a, x));
    let byte = |a: u32, b: u8| (ee.u32_at(a & !3) & !(0xff << (8 * (a & 3)))) | u32::from(b) << (8 * (a & 3));
    w(CAR + own::WRECKS_ON, word(v.wrecks_on));
    w(CAR + own::BOUNDED, word(v.bounded));
    w(CAR + own::CLOCK, v.clock.to_bits());
    w(CAR + own::MAX_WRECK_TIME, v.max_wreck_time.to_bits());
    w(CAR + own::COUNTING, word(v.counting));
    w(CAR + own::TIPPED_WRECK, word(v.tipped_wreck));
    w(CAR + own::KIND, v.kind);
    w(CAR + own::LOOK, if v.look { o.look } else { 0 });
    w(o.state + own::MODE, if v.rumble { 1 } else { 0 });
    w(p.record + own::TOUCHING, word(v.touching));
    w(o.world + own::DRAG, v.drag.to_bits());
    w(o.world + own::FLOOR, v.bounds.floor.to_bits());
    w(CAR + own::FLAGS_WORD, byte(CAR + own::FLAGS_WORD + 1, v.flags));
    let mut vs = vec![
        (CAR + own::WRECK_DAMPING, v.wreck_damping),
        (CAR + own::WRECK_INERTIA, v.wreck_inertia),
        (CAR + own::INERTIA, v.inertia),
        (p.body + own::BODY_INERTIA, v.body_inertia),
        (o.world + own::GRAVITY, v.gravity),
    ];
    if let Some((lo, hi)) = v.bounds.area {
        vs.push((o.area + 0x30, lo));
        vs.push((o.area + 0x40, hi));
    }
    for (n, &a) in v.scene.nodes.iter().zip(&o.nodes) {
        for r in 0..4u32 {
            vs.push((a + 0x30 + 0x10 * r, n.local[r as usize]));
            vs.push((a + 0x70 + 0x10 * r, n.world[r as usize]));
        }
        w(a + 0x100, byte(a + 0x100, n.flags));
    }
    for (a, x) in vs {
        for (k, b) in x.to_bits().into_iter().enumerate() {
            w(a + 4 * k as u32, b);
        }
    }
    out
}

/// Unhook MD106b's five calls (the vehicle force and post, the wreck timer, the node-local turn and
/// the node's position) and hook what they hand out instead; the landing sound now logs what it is
/// handed. A call from anything but the car (or its node, record or look) fails the test: these
/// hooks run nothing, for every caller.
fn hook_own(ee: &mut Ee, p: Ptrs, o: &OwnPtrs, log: Shared<Vec<Call>>, out: Shared<bool>) {
    for a in [VEHICLE_FORCE, VEHICLE_POST, WRECK_TIMER, NODE_LOCAL, NODE_POSITION] {
        ee.hooks.remove(&a).expect("hooked");
    }
    let record_tell = ee.u32_at(ee.u32_at(p.record + 4) + 0x88);
    let on: [(u32, u32, Call); 8] = [
        (SUBTREE_OUT_OF_DATE, ee.u32_at(p.node + 0xf4), Call::SubtreeOutOfDate),
        (record_tell, p.record, Call::GravityRecord),
        (BODY_RESET, CAR, Call::BodyReset),
        (RESPAWN, CAR, Call::Respawn),
        (SPATIAL_UPDATE, CAR, Call::SpatialUpdate),
        (NODE_BOUNDS, p.node, Call::NodeBounds),
        (COLLISION_SYNC, CAR, Call::CollisionSync),
        (LOOK_OFF, o.look, Call::WreckLook(false)),
    ];
    for (addr, this, c) in on {
        let l = log.clone();
        ee.hooks.insert(addr, Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, this, "{c:?} ({addr:#x}) on something else, from {:#x}", ee.r(31));
            if c == Call::GravityRecord {
                assert_eq!(ee.r(5), 1, "the record's gravity word");
            }
            l.borrow_mut().push(c);
        }));
    }
    let (l, look) = (log.clone(), o.look);
    ee.hooks.insert(LOOK_ON, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, look, "the wrecked look on something else, from {:#x}", ee.r(31));
        l.borrow_mut().push(Call::WreckLook(true));
    }));
    let l = log.clone();
    ee.hooks.insert(EDGE_TEST, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, CAR, "the map-edge test on something else");
        l.borrow_mut().push(Call::OutOfBounds);
        ee.set(2, u64::from(*out.borrow()));
    }));
    let l = log.clone();
    ee.hooks.insert(RUMBLE_ON, Box::new(move |ee| {
        assert_eq!(ee.r(31) as u32, RUMBLE_RA, "the rumble switch from elsewhere");
        l.borrow_mut().push(Call::Rumble);
        ee.set(2, 0);
    }));
    let l = log.clone();
    ee.hooks.insert(CUE, Box::new(move |ee| {
        assert_eq!((ee.r(5) as u32, ee.r(6)), (CAR + own::CUE_SOURCE, 8), "a cue from something else, from {:#x}", ee.r(31));
        let kind = ee.r(7) as u32;
        let tag = if kind == 12 { 0 } else { ee.u32_at(CAR + own::CUE_TAG) };
        assert_eq!(ee.r(8) as u32, tag, "cue {kind}'s tag");
        l.borrow_mut().push(Call::Cue(kind));
    }));
    let l = log;
    ee.hooks.insert(LANDING_SOUND, Box::new(move |ee| {
        assert_eq!((ee.r(4) as u32, ee.r(5)), (p.landing_sound, 0), "the landing sound's object and kind");
        l.borrow_mut().push(Call::LandingSound(get_v(ee, ee.r(6) as u32)));
    }));
}

/// A generated state of MD106b's words: wrecks on or off, the clock about `MaxWreckTime`, the kinds,
/// the flag byte's bits, the world's box round the body (in and out), the rumble and the look.
fn generate_own(r: &mut Rng, image: &atv_car::Vehicle, car: &mut Car, body: &Body, s: &mut Script) -> (atv_car::Vehicle, bool) {
    let mut v = *image;
    v.wrecks_on = r.chance(0.9);
    v.bounded = r.chance(0.7);
    v.max_wreck_time = F(r.range(0.2, 6.0));
    v.clock = if r.chance(0.5) { F(v.max_wreck_time.0 - r.range(-0.08, 0.06)) } else { F(r.range(0.0, v.max_wreck_time.0)) };
    v.counting = r.chance(0.4);
    v.tipped_wreck = r.chance(0.3);
    v.kind = [0, 1, 4, 4, 2][(r.next() % 5) as usize];
    v.wreck_damping = Vf::new(r.range(0.9, 1.0), r.range(0.9, 1.0), r.range(0.9, 1.0), 1.0);
    v.wreck_inertia = Vf::new(r.range(0.05, 1.0), r.range(0.05, 1.0), r.range(0.05, 1.0), r.range(0.5, 2.0));
    let mut lane = |r: &mut Rng| if r.chance(0.08) { [0.0, -0.0, -3.0][(r.next() % 3) as usize] } else { r.range(100.0, 1500.0) };
    v.inertia = Vf::new(lane(r), lane(r), lane(r), r.range(0.5, 2.0));
    v.body_inertia = Vf::new(lane(r), lane(r), lane(r), r.range(0.5, 2.0));
    v.rumble = r.chance(0.5);
    v.look = r.chance(0.85);
    v.touching = r.chance(0.7);
    v.gravity = Vf::new(r.range(-50.0, 50.0), r.range(-3000.0, -1000.0), r.range(-50.0, 50.0), 1.0);
    v.drag = F(r.range(0.0, 0.2));
    v.flags = (image.flags & !0x67) | (r.word() as u8 & 0x67);
    v.bounds.floor = F(body.position.y() + r.range(-8.0, 8.0));
    if v.bounds.area.is_some() {
        let (px, pz) = (body.position.x(), body.position.z());
        let lo = Vf::new(px - r.range(-10.0, 60.0), r.range(-100.0, 100.0), pz - r.range(-10.0, 60.0), 1.0);
        let hi = Vf::new(px + r.range(-10.0, 60.0), r.range(-100.0, 100.0), pz + r.range(-10.0, 60.0), 1.0);
        v.bounds.area = Some((lo, hi));
    }
    for n in &mut v.scene.nodes {
        if r.chance(0.5) {
            n.flags ^= 1;
        }
    }
    let mut out = r.chance(0.25);
    if !v.look {
        // The wreck calls the look without testing it is there: no wreck can start.
        car.state = [atv_car::WRECK_EDGE, atv_car::WRECK_TIPPED][(r.next() % 2) as usize];
        out = false;
    } else if r.chance(0.35) {
        car.state = atv_car::RIDING;
    }
    if r.chance(0.5) {
        // Slow, for the still tests.
        for t in &mut s.steps {
            for j in t.iter_mut() {
                j.3 = Vf::new(j.3.x() * 0.1, j.3.y() * 0.1, j.3.z() * 0.1, j.3.w());
            }
        }
    }
    (v, out)
}

/// **The car's frame with SLUS-21104's own calls in it is the original's** (MD106b): the frame test
/// with the vehicle force and post, the wreck timer, the landing's node-local turn and the node's
/// position no longer hooked, against [`atv_car::Own`] answering them over the same script, 3,000
/// generated states: the car, its tires and body, the vehicle's wreck words, the world's drag and the
/// scene's nodes word for word, the same calls handed out, nothing else written in those blocks, and
/// every branch of the five seen.
#[test]
fn the_cars_own_calls_are_the_originals() {
    let Some(pristine) = image() else { return };
    let Some(mut ee) = image() else { return };
    let p = ptrs(&ee);
    let o = own_ptrs(&ee, &p);
    let image_car = read_car(&ee, &p);
    let image_body = seed::body(&|a| ee.u32_at(a), p.body);
    let image_v = read_vehicle(&ee, &p, &o);
    assert!(image_v.look && image_v.record && o.state != 0 && ee.u32_at(o.state + own::PAD) != 0, "the image's look, record and pad");
    let log: Shared<Vec<Call>> = Rc::default();
    let script: Shared<Script> = Rc::default();
    let out: Shared<bool> = Rc::default();
    hook(&mut ee, p, log.clone(), script.clone());
    hook_own(&mut ee, p, &o, log.clone(), out.clone());
    let world = World { iterations: 1, rate: F(115.0) };
    let table = AcosTable::build();
    let mut r = Rng(0x0106_b0b0_0000_0001);
    let mut seen = std::collections::BTreeMap::<&str, u32>::new();
    let mut see = |what: &'static str, yes: bool| {
        if yes {
            *seen.entry(what).or_insert(0) += 1;
        }
    };
    let mut blocks: Vec<(u32, u32)> = [(CAR, 0x1500), (p.body, 0x230), (p.record + at::TIPPED, 4), (p.record + own::TOUCHING, 4), (o.world, 0xb0)]
        .into_iter()
        .chain(p.tires.map(|t| (t, 0x200)))
        .collect();
    blocks.extend(o.nodes.iter().map(|&n| (n, 0x110)));
    const CASES: usize = 3000;
    for case in 0..CASES {
        let (mut car, body, dt) = generate(&mut r, &image_car, &image_body, &p);
        let mut s = Script::new(&mut r);
        let (v, edge) = generate_own(&mut r, &image_v, &mut car, &body, &mut s);
        let before = [car_words(&car, &p), seed::body_words(&body, p.body), vehicle_words(&pristine, &v, &p, &o)].concat();

        // The original.
        poke(&mut ee, &before);
        *script.borrow_mut() = s.clone();
        *out.borrow_mut() = edge;
        log.borrow_mut().clear();
        ee.set(28, GP);
        ee.call(WORLD_STEP, &[u64::from(WORLD)], &[dt.0], 20_000_000);
        let written = ee.written_ranges();

        // The port.
        let (mut pc, mut pb, mut pv) = (car, body, v);
        pv.out_of_bounds = edge;
        let mut port = Port { log: Vec::new(), script: s };
        atv_car::frame(&world, &mut pc, &mut pb, dt, &table, &mut atv_car::Own { v: &mut pv, rest: &mut port });
        let after = [car_words(&pc, &p), seed::body_words(&pb, p.body), vehicle_words(&pristine, &pv, &p, &o)].concat();

        let bad: Vec<String> = after
            .iter()
            .filter(|&&(a, x)| ee.u32_at(a) != x)
            .map(|&(a, x)| format!("  {a:#x}: port {x:#010x} ({}), original {:#010x} ({})", f32::from_bits(x), ee.u32_at(a), ee.f32_at(a)))
            .collect();
        let theirs = log.borrow().iter().map(|c| format!("{c:?}")).collect::<Vec<_>>();
        let ours = port.log.iter().map(|c| format!("{c:?}")).collect::<Vec<_>>();
        assert!(bad.is_empty(), "case {case} (dt {dt:?}, state {} -> {}): {} words differ\n{}\ncalls {ours:?}", car.state, pc.state, bad.len(), bad.join("\n"));
        assert_eq!(ours, theirs, "case {case}: the calls");
        let compared: std::collections::BTreeSet<u32> = after.iter().map(|&(a, _)| a).collect();
        for &(from, to) in &written {
            for a in (from & !3..to).step_by(4) {
                let ours = blocks.iter().any(|&(b, n)| (b..b + n).contains(&a));
                assert!(!ours || compared.contains(&a), "case {case}: the original wrote {a:#x}, which is not compared");
            }
        }

        let has = |what: &str| ours.iter().any(|c| c.starts_with(what));
        let wreck = car.state == atv_car::RIDING && pc.state != atv_car::RIDING || has("OutOfBounds") && edge;
        let unwreck = car.state != atv_car::RIDING && pc.state == atv_car::RIDING;
        see("wrecks off", !v.wrecks_on);
        see("edge asked", has("OutOfBounds"));
        see("edge wreck", has("OutOfBounds") && edge);
        see("tipped wreck", pc.state == atv_car::WRECK_TIPPED && car.state == atv_car::RIDING);
        see("riding, not tipped", v.wrecks_on && car.state == atv_car::RIDING && pc.state == atv_car::RIDING);
        see("edge clock started", has("Cue(12)"));
        see("edge unwreck", unwreck && car.state == atv_car::WRECK_EDGE);
        see("tipped unwreck", unwreck && car.state == atv_car::WRECK_TIPPED);
        see("wrecked, waiting", v.wrecks_on && car.state != atv_car::RIDING && pc.state == car.state);
        see("wreck of kind 0 or 1", wreck && v.kind < 2);
        see("wreck of another kind", wreck && v.kind >= 2);
        see("unwreck of kind 0 or 1", unwreck && v.kind < 2);
        see("unwreck of another kind", unwreck && v.kind >= 2);
        see("unwreck without a look", unwreck && !v.look);
        see("rumble", has("Rumble"));
        see("outside the world", has("BodyReset"));
        see("inside the world", !has("BodyReset"));
        see("node bounds", has("NodeBounds"));
        see("landing sound", has("LandingSound"));
        see("inertia lane not above 0", (wreck || unwreck) && v.kind >= 2 && pv.body_inertia.0[..3].iter().any(|&x| x <= 0.0));

        // Put the image back.
        for (from, to) in written {
            for a in from..to {
                ee.write(a, 1, u128::from(pristine.u8_at(a)));
            }
        }
    }
    eprintln!("MD106b: {CASES} frames, seen {seen:?}");
    for (what, &k) in &seen {
        assert!(k >= 30, "{what} seen only {k} times");
    }
    assert_eq!(seen.len(), 20, "a branch was never seen: {seen:?}");
}
