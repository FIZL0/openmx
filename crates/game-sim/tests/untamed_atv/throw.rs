//! MD144a — **the quad's out-of-bounds test and throw on Untamed's step** ([`quad::Ride::throw`]
//! after [`quad::Ride::untamed`]).
//!
//! SLUS-21701's is SLUS-21104's on moved fields but for two things: its band inside the map's box is
//! 300 ft (Unleashed's 150), and it builds the spin in a zeroed local and stores it whole, so the
//! body's angular velocity `w` is 0 (Unleashed's 1). It is run in the interpreter off the Free Ride
//! image (`context/untamed/ram/nat_or.bin`, as the hazard clock's test) on a quad, body, world and
//! drag holder laid at Untamed's offsets, its `rand` handing the draws the port's wreck generator
//! makes, against the spawned quad on Untamed's step: whether it throws, the body's velocity and
//! angular velocity and the drag, bit for bit. Without the image or MD123's files the test skips.

use super::route::{quad_files, spawned};
use super::{Gen, image};
use game_sim::biplane::Area;
use game_sim::collide::{Bundle, Record};
use game_sim::world::World;
use ps2_float::{F, Vf};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// SLUS-21701's quad out-of-bounds test (the quad's vt `+0x16c`), its first two words, and `rand`.
const THROW: u32 = 0x004c_8b40;
const THROW_HEAD: [u32; 2] = [0x27bd_ff90, 0xffbf_0010];
const RAND: u32 = 0x0034_4360;

/// Where the harness lays the quad and what it points at, under the hazard clock test's vtable.
const OBJ: u32 = 0x01c0_0000;
const BODY: u32 = 0x01c0_1000;
const WORLD: u32 = 0x01c0_1400;
const AREA: u32 = 0x01c0_1a00;
const DRAG: u32 = 0x01c0_1b00;

/// The quad's words at Untamed's offsets: the edge armed, the world, the body, the drag's holder;
/// the world's box and its size; the body's position, velocity and angular velocity.
const ARMED: u32 = 0x338;
const WORLD_AT: u32 = 0x3e0;
const BODY_AT: u32 = 0xe8;
const DRAG_AT: u32 = 0x118;
const BOX_AT: u32 = 0x42c;
const WIDTH: u32 = 0x40;
const DEPTH: u32 = 0x48;
const POSITION: u32 = 0x70;
const VELOCITY: u32 = 0x80;
const RATES: u32 = 0x1d0;

/// A world that is only its box: no wrap, no holes.
struct Boxed(Area);

impl World for Boxed {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        (at, Vf::new(0.0, 1.0, 0.0, 1.0))
    }
    fn solid(&mut self, _: &Bundle, _: &mut Record) -> bool {
        false
    }
    fn area(&self) -> Option<Area> {
        Some(self.0)
    }
}

struct Case {
    area: Area,
    at: Vf,
    velocity: Vf,
    rates: Vf,
    drag: F,
    seed: u32,
}

/// Mostly near an edge — inside 150 ft, between 150 and 300, past it — on boxes from smaller than
/// the 600 ft the band leaves nothing of to a full map.
fn case(g: &mut Gen) -> Case {
    let area = Area { width: g.pick(&[400.0, 599.0, 600.0, 2048.0, 4096.0, 8192.0]), depth: g.pick(&[512.0, 1000.0, 3072.0, 6144.0]) };
    let mut axis = |g: &mut Gen, size: F| match g.bits() % 9 {
        0 => g.f(0.0, 150.0),
        1 => g.f(150.0, 300.0),
        2 => g.pick(&[150.0, 300.0, size.0 - 300.0, size.0 - 150.0]),
        3 => g.f(size.0 - 300.0, size.0),
        4 => g.f(-60.0, 0.0),
        _ => g.f(0.0, size.0),
    };
    let (x, z) = (axis(g, area.width), axis(g, area.depth));
    let at = Vf::new(x.0, g.f(-50.0, 400.0).0, z.0, 1.0);
    let v = |g: &mut Gen, r: f32| Vf::new(g.f(-r, r).0, g.f(-r, r).0, g.f(-r, r).0, g.f(-1.0, 1.0).0);
    Case { area, at, velocity: v(g, 80.0), rates: v(g, 5.0), drag: g.f(0.0, 0.02), seed: g.bits() }
}

fn put_v(ee: &mut ps2_oracle::Ee, a: u32, v: Vf) {
    for k in 0..4 {
        ee.set_f32(a + 4 * k as u32, v.0[k]);
    }
}

fn get_v(ee: &ps2_oracle::Ee, a: u32) -> [u32; 4] {
    std::array::from_fn(|k| ee.u32_at(a + 4 * k as u32))
}

fn bits(v: Vf) -> [u32; 4] {
    v.0.map(f32::to_bits)
}

#[test]
fn untamed_quad_throw_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let Some(files) = quad_files() else { return };
    if [ee.u32_at(THROW), ee.u32_at(THROW + 4)] != THROW_HEAD {
        panic!("the image's quad out-of-bounds test is not at {THROW:#x}");
    }
    let draws = Rc::new(RefCell::new(Vec::<i32>::new()));
    let pool = draws.clone();
    ee.hooks.insert(RAND, Box::new(move |ee| {
        let v = pool.borrow_mut().remove(0);
        ee.set(2, v as i64 as u64);
    }));
    let mut base = spawned(&files);
    base.untamed();
    let mut g = Gen(0x4d44_3134_3461);
    let mut tally = BTreeMap::<&str, u32>::new();
    for k in 0..4000 {
        let c = case(&mut g);
        let rng = game_sim::rng::Rng::from_seed(c.seed);

        let mut ours = base.clone();
        ours.wrecked = Some(game_sim::quad::Wrecked { rng, edge: true, wrecks: true, ..Default::default() });
        (ours.body.position, ours.body.velocity, ours.body.rates) = (c.at, c.velocity, c.rates);
        ours.bike.clutch_keys.drag = c.drag;
        let thrown = ours.throw(&Boxed(c.area));

        let mut r = rng;
        *draws.borrow_mut() = (0..3).map(|_| r.rand()).collect();
        for o in (0..0x400).step_by(4) {
            ee.set_u32(OBJ + o, 0);
        }
        ee.set_u32(OBJ + ARMED, 1);
        ee.set_u32(OBJ + WORLD_AT, WORLD);
        ee.set_u32(OBJ + BODY_AT, BODY);
        ee.set_u32(OBJ + DRAG_AT, DRAG);
        ee.set_u32(WORLD + BOX_AT, AREA);
        ee.set_f32(AREA + WIDTH, c.area.width.0);
        ee.set_f32(AREA + DEPTH, c.area.depth.0);
        put_v(&mut ee, BODY + POSITION, c.at);
        put_v(&mut ee, BODY + VELOCITY, c.velocity);
        put_v(&mut ee, BODY + RATES, c.rates);
        ee.set_f32(DRAG + 4, c.drag.0);
        ee.call(THROW, &[u64::from(OBJ)], &[], 10_000);
        let theirs = ee.r(2) != 0;

        let what = format!("case {k}: box {}×{} at {:?}", c.area.width.0, c.area.depth.0, c.at);
        assert_eq!(thrown, theirs, "{what}: thrown");
        assert_eq!(bits(ours.body.velocity), get_v(&ee, BODY + VELOCITY), "{what}: velocity");
        assert_eq!(bits(ours.body.rates), get_v(&ee, BODY + RATES), "{what}: angular velocity");
        assert_eq!(ours.bike.clutch_keys.drag.0.to_bits(), ee.u32_at(DRAG + 4), "{what}: drag");
        let out = |p: f32, size: F| p < 150.0 || size.0 - 150.0 < p;
        let in_band = !(out(c.at.x(), c.area.width) || out(c.at.z(), c.area.depth));
        *tally.entry(match (thrown, in_band) {
            (false, _) => "kept",
            (true, true) => "thrown in Untamed's wider band",
            (true, false) => "thrown",
        }).or_default() += 1;
    }
    println!("Untamed's quad throw: 4000 cases bit-exact, {tally:?}");
    for want in ["kept", "thrown", "thrown in Untamed's wider band"] {
        assert!(tally.get(want).copied().unwrap_or(0) >= 200, "too few cases went {want:?}: {tally:?}");
    }
}
