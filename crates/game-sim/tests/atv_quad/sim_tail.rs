//! MD105a — **the sim frame's tail on the quad in Rust**: the tail's listener walk ([`tail::walk`]),
//! the quad's update ([`quad::update`]) at its listener, under it the vehicle's inner update
//! ([`tail::inner`]) and hazard clock ([`tail::stuck`]), the wreck test and entry and the unwreck
//! ([`wreck::wreck_test`], [`wreck::enter`], [`wreck::unwreck`], [`wreck::part`]), the swap onto the
//! wreck rays and back ([`quad::swap_rays`]), the put-back ([`quad::put_back`] over
//! [`wreck::put_back_base`], the wheels' resets [`wreck::wheel_reset`]) and the animation driver
//! ([`quad::animate`]), each on the quad in the interpreter's RAM. Every other call is the
//! original's, in the interpreter.
//!
//! Each port reads its words where the image keeps them (the field maps are research/oracle.txt's
//! `# MD64a`, `# MD65`, `# MD65a`, `# MD65c`, `# F31b4a`, `# F31b4b`), and writes back those it changed.
//! [`port_tail`] stands in for the original's tail in `sim_frame.rs`'s pieces.

use super::*;
use game_sim::handling::tail::{self, Listeners, Vehicles};
use game_sim::handling::wreck::{self, Wrecks};
use game_sim::quad::{Moto, Part};

/// The quad's listener's step (its vt `+0x40`): a thunk to the quad's update with the quad.
const QUAD_STEP: u32 = 0x004b_f9a0;
/// The quad's vtable slots whose functions the ports stand for, each with its function: the hazard
/// clock, the wreck test, entry, unwreck and part toggle, the two ray swaps, the put-back and the
/// animation driver.
const PORTED: [(u32, u32); 9] = [
    (0x45c, 0x004a_ad60),
    (0x49c, 0x004e_24a0),
    (0x368, 0x004e_2260),
    (0x36c, 0x004e_2380),
    (0x398, 0x004e_1870),
    (0x48c, 0x004e_2e10),
    (0x490, 0x004e_2eb0),
    (0x3a8, 0x004b_8e30),
    (0x3e4, 0x004b_e3c0),
];
/// The inner update's calls the original runs, by the quad's vtable slot.
const TAIL_SLOTS: [(u32, tail::Out); 7] = [
    (0x3a4, tail::Out::Fade),
    (0x4f8, tail::Out::WreckFirst),
    (0x4e8, tail::Out::WreckPose),
    (0x4f0, tail::Out::Lean),
    (0x35c, tail::Out::Ride),
    (0x378, tail::Out::Sync),
    (0x37c, tail::Out::Snapshot),
];
/// The rider's hold: the name lookup and its two names.
const LOOKUP: u32 = 0x0021_4b30;
const LOOKUP_ARGS: [u64; 3] = [0x0054_23e0, 0x0052_74f3, 0];
const RUMBLE_READY: u32 = 0x0051_a250;
const NETWORK: u32 = 0x0025_e8d0;
const PARTS: u32 = 0x0049_4370;
const POST: u32 = 0x0027_2300;
const POST_QUEUE: u32 = 0x0044_2eb0;
/// The wreck's calls out (F31b4b's).
const RAND: u32 = 0x0035_61a8;
const BOUNDS: u32 = 0x0031_d410;
const UNHOOK: u32 = 0x0018_d540;
const ATTACH: u32 = 0x0011_43e0;
const RIDER_SETTLE: u32 = 0x0026_6c90;
const BONE_POSE: u32 = 0x0019_4e00;
const BONE_PLACE: u32 = 0x0019_4da0;
const TERRAIN_NORMAL: u32 = 0x0048_33c0;
const GROUND_RESET: u32 = 0x0049_2ca0;
const REHANG: u32 = 0x0019_6c10;
const TRAIL_RESET: u32 = 0x004f_a760;
const CAMERA_RESET: u32 = 0x004f_7590;
/// The animation driver's calls out (MD65c's).
const ANIM_STOP: u32 = 0x0013_e650;
const PLAY_CLIP: u32 = 0x0013_eec0;
const BLEND_CLIPS: u32 = 0x0013_f5e0;
const POSE_NODES: u32 = 0x004b_e8d0;
const ATTACHMENT_MOVED: u32 = 0x0026_6d30;
const SNAP_SLOTS: [u32; 5] = [0xcd4, 0xcdc, 0xce4, 0xcec, 0xcf4];
/// Scratch slots (inside the stack the compare leaves out) for the calls' buffers: the body's
/// quadword, the terrain's normal, the bounds test's words, the two frame buffers, the told zero.
const S_QUAD: u32 = 61;
const S_NORMAL: u32 = 62;
const S_BOUNDS: u32 = 63;
const S_FRAME: u32 = 64;
const S_TOLD: u32 = 66;

fn call(ee: &mut Ee, f: u32, ints: &[u64], floats: &[f32]) -> u64 {
    ee.set(28, GP);
    ee.call(f, ints, floats, BUDGET);
    ee.r(2)
}

fn vt(ee: &Ee, obj: u32, slot: u32) -> u32 {
    ee.u32_at(ee.u32_at(obj + 4) + slot)
}

/// Asserts the quad's vt `slot` holds the function the port stands for.
fn own(ee: &Ee, quad: u32, slot: u32) {
    let f = PORTED.iter().find(|s| s.0 == slot).unwrap().1;
    assert_eq!(vt(ee, quad, slot), f, "the quad's vt +{slot:#x}");
}

fn f_at(ee: &Ee, a: u32) -> F {
    F(f32::from_bits(ee.u32_at(a)))
}

fn set_byte(ee: &mut Ee, a: u32, b: u8) {
    let (at, shift) = (a & !3, 8 * (a & 3));
    ee.set_u32(at, ee.u32_at(at) & !(0xff << shift) | u32::from(b) << shift);
}

/// `now`'s words that are not in `last`, into RAM.
fn set_changed(ee: &mut Ee, now: &[(u32, u32)], last: &[(u32, u32)]) {
    let old: std::collections::HashSet<(u32, u32)> = last.iter().copied().collect();
    for &(a, w) in now {
        if !old.contains(&(a, w)) {
            ee.set_u32(a, w);
        }
    }
}

/// A list's length, from `first` through `next` (handed the element and its place), at most 256.
fn count(first: u32, next: impl Fn(u32, u32) -> u32) -> u32 {
    let (mut e, mut n) = (first, 0);
    while e != 0 && n < 256 {
        e = next(e, n);
        n += 1;
    }
    n
}

// --- the quad's update ------------------------------------------------------------------------------

/// The quad's [`Tail`] off RAM (`atv_quad.rs`' `poke_tail` map), but the rider's hold and the rumble's
/// on, which take calls ([`port_update`]).
fn read_tail(ee: &Ee, q: u32) -> Tail {
    let f = |o: u32| f_at(ee, q + o);
    let w = |o: u32| ee.u32_at(q + o);
    let state = w(0xbc4);
    let wheels = [0, 1, 2, 3].map(|k| w(0x17b0 + 4 * k));
    let (player, race, node) = (w(0x680), w(0x684), w(0xe4));
    Tail {
        vehicle_state: (state != 0).then(|| ee.u32_at(state + 0x28)),
        rider_held: false,
        stunt_speed: f(0x174c),
        surfaces: std::array::from_fn(|i| {
            let a = q + 0xdf4 + 0xc * i as u32;
            Surface { hazard: ee.u32_at(a) != 0, time: f_at(ee, a + 4) }
        }),
        wheel_surface: wheels.map(|a| ee.u8_at(a + 0x110)),
        wheel_sunk: wheels.map(|a| ee.u32_at(a + 0xd0) != 0),
        wheel_squeeze: wheels.map(|a| f_at(ee, a + 0xf0)),
        on_hazard: w(0xc20) != 0,
        request: w(0x31c),
        request_was: w(0x320),
        pad: state != 0 && ee.u32_at(state + 0x5c) != 0,
        rumble_on: false,
        wheelie_clock: f(0x1744),
        stoppie_clock: f(0x1748),
        wheelie_stunt: w(0x13c4),
        stoppie_stunt: w(0x13c8),
        wheelie_time: f(0x704),
        stoppie_time: f(0x708),
        wheelie_dist: f(0x70c),
        stoppie_dist: f(0x710),
        best_wheelie_time: f(0x714),
        best_stoppie_time: f(0x718),
        best_wheelie_dist: f(0x71c),
        best_stoppie_dist: f(0x720),
        player: (player != 0).then(|| Player {
            flags: ee.u32_at(player + 8),
            step: f_at(ee, player + 0x18),
            word: ee.u32_at(player + 0x40),
            speed: f_at(ee, player + 0x108),
            countdown: f_at(ee, player + 0xc4),
            listeners: count(ee.u32_at(player + 0x1d4), |e, _| ee.u32_at(e + 0x1c)),
        }),
        race: (race != 0).then(|| Race {
            clock: f_at(ee, race + 0x29c),
            watchers: count(ee.u32_at(race + 0x24), |e, n| ee.u32_at(e + if n == 0 { 4 } else { 0x18 })),
        }),
        ride_time: f(0x6f8),
        top_speed: f(0x6fc),
        distance: f(0x724),
        word_falls: w(0x6c8),
        word_was: w(0x6cc),
        node_up: vf_at(ee, node + 0x80),
        node_forward: vf_at(ee, node + 0x90),
        second_on: w(0x175c),
        second_countdown: f(0x1760),
        clock: f(0x1750),
        post: w(0xd20),
        untamed: false,
    }
}

/// The words of `t` the update writes, at the quad `q` (and its player's countdown, with a player at
/// `player`).
fn tail_words(t: &Tail, q: u32, player: u32) -> Vec<(u32, u32)> {
    let b = |x: F| x.0.to_bits();
    let mut out = vec![
        (q + 0xc20, u32::from(t.on_hazard)),
        (q + 0x31c, t.request),
        (q + 0x320, t.request_was),
        (q + 0x1744, b(t.wheelie_clock)),
        (q + 0x1748, b(t.stoppie_clock)),
        (q + 0x704, b(t.wheelie_time)),
        (q + 0x708, b(t.stoppie_time)),
        (q + 0x70c, b(t.wheelie_dist)),
        (q + 0x710, b(t.stoppie_dist)),
        (q + 0x714, b(t.best_wheelie_time)),
        (q + 0x718, b(t.best_stoppie_time)),
        (q + 0x71c, b(t.best_wheelie_dist)),
        (q + 0x720, b(t.best_stoppie_dist)),
        (q + 0x6f8, b(t.ride_time)),
        (q + 0x6fc, b(t.top_speed)),
        (q + 0x724, b(t.distance)),
        (q + 0x6c8, t.word_falls),
        (q + 0x6cc, t.word_was),
        (q + 0x175c, t.second_on),
        (q + 0x1760, b(t.second_countdown)),
        (q + 0x1750, b(t.clock)),
    ];
    if let Some(p) = t.player {
        out.push((player + 0xc4, b(p.countdown)));
    }
    out
}

/// The [`Quad`] and [`Body`] words the update reads, off RAM.
fn read_update_quad(ee: &Ee, quad: u32, q: &mut Quad, body: &mut Body) {
    let b = |o: u32| ee.u32_at(quad + o) != 0;
    q.contact_vel = vf_at(ee, quad + 0xc30);
    q.speed_len = f_at(ee, quad + 0x32c);
    q.control_off = b(0xc80);
    q.front_down = b(0x1274);
    q.rear_down = b(0x1278);
    q.airborne = b(0xde0);
    q.hang_time = f_at(ee, quad + 0x1190);
    body.velocity = vf_at(ee, ee.u32_at(quad + 0xb8) + 0x80);
}

/// The quad's update on the quad in RAM: its [`Tail`] handed out before each call and read back
/// after ([`Moto::tail_out`], [`Moto::tail_in`]), the [`Quad`] and [`Body`] words it reads read again
/// after each call.
struct UpdateInRam<'a> {
    ee: &'a mut Ee,
    quad: u32,
    last: Vec<(u32, u32)>,
}

impl UpdateInRam<'_> {
    fn player(&self) -> u32 {
        self.ee.u32_at(self.quad + 0x680)
    }
}

impl Moto for UpdateInRam<'_> {
    fn tail_out(&mut self, t: &Tail) {
        let now = tail_words(t, self.quad, self.player());
        set_changed(self.ee, &now, &self.last);
        self.last = now;
    }
    fn tail_in(&mut self, t: &mut Tail) {
        let (held, on) = (t.rider_held, t.rumble_on);
        *t = read_tail(self.ee, self.quad);
        (t.rider_held, t.rumble_on) = (held, on);
        self.last = tail_words(t, self.quad, self.player());
    }
    fn call(&mut self, c: Call, q: &mut Quad, body: &mut Body) {
        let (ee, quad) = (&mut *self.ee, self.quad);
        let qa = u64::from(quad);
        let state = ee.u32_at(quad + 0xbc4);
        let (player, race) = (ee.u32_at(quad + 0x680), ee.u32_at(quad + 0x684));
        match c {
            Call::BikeVehicleUpdate(dt) => {
                let mut m = TailInRam::new(ee, quad);
                tail::inner(&mut m, dt);
                m.write();
            }
            Call::HazardSurface { dt, time, sunk } => {
                own(ee, quad, 0x45c);
                let mut m = TailInRam::new(ee, quad);
                tail::stuck(&mut m, sunk, dt, time);
                m.write();
            }
            Call::Rumble { soft, strength } => {
                let pad = ee.u32_at(state + 0x5c);
                let f = ee.u32_at(ee.u32_at(pad + 0x1c) + 0x4c);
                call(ee, f, &[u64::from(pad), u64::from(soft), strength as i64 as u64], &[tail::RUMBLE_TIME]);
            }
            Call::StartStunt(id) => {
                call(ee, vt(ee, quad, 0x4b8), &[qa, u64::from(id), 1], &[]);
            }
            Call::EndStunt => {
                call(ee, vt(ee, quad, 0x4ac), &[qa, u64::MAX], &[]);
            }
            Call::CountdownOut => {
                let f = ee.u32_at(ee.u32_at(quad + 0x6a0) + 0x68);
                call(ee, f, &[qa + 0x680, u64::from(race), u64::from(player)], &[]);
            }
            Call::CountdownListener(i) => {
                let mut l = ee.u32_at(player + 0x1d4);
                for _ in 0..i {
                    l = ee.u32_at(l + 0x1c);
                }
                let f = ee.u32_at(ee.u32_at(l + 0x20) + 0x68);
                call(ee, f, &[u64::from(l), u64::from(race), u64::from(player)], &[]);
            }
            Call::CountdownWatcher(i) => {
                let mut w = ee.u32_at(race + 0x24);
                for n in 0..i {
                    w = ee.u32_at(w + if n == 0 { 4 } else { 0x18 });
                }
                let f = ee.u32_at(ee.u32_at(w + 0x1c) + 0x7c);
                call(ee, f, &[u64::from(w), u64::from(race), u64::from(player)], &[]);
            }
            Call::Post(id) => {
                let queue = u64::from(ee.u32_at(POST_QUEUE));
                call(ee, POST, &[queue, qa + 0x674, 8, 4, u64::from(id), 0], &[]);
            }
            c => panic!("the quad's update makes no {c:?}"),
        }
        read_update_quad(self.ee, quad, q, body);
    }
}

/// **The quad's update** (its listener's step) on the quad in RAM: the rider's hold through the
/// original's lookups (the name, the vehicle state's vt `+0xac`, the rider's hold word) and the
/// rumble's on through its query, then [`quad::update`] through [`UpdateInRam`].
fn port_update(ee: &mut Ee, quad: u32, dt: F) {
    let mut t = read_tail(ee, quad);
    if t.vehicle_state.is_some() {
        let state = ee.u32_at(quad + 0xbc4);
        call(ee, LOOKUP, &LOOKUP_ARGS, &[dt.0]);
        let name = ee.r(2);
        let g = call(ee, vt(ee, state, 0xac), &[u64::from(state), name], &[dt.0]) as u32;
        let rider = if g == 0 { 0 } else { g - 0x2ec };
        t.rider_held = ee.u32_at(ee.u32_at(rider + 0x378) + 0x14) != 0;
        if !t.rider_held && t.pad {
            let token = ee.u32_at(state + 0x6ac);
            t.rumble_on = call(ee, RUMBLE_READY, &[u64::from(token)], &[]) as u32 != 0;
        }
    }
    let (mut q, mut body) = (Quad::default(), Body::default());
    read_update_quad(ee, quad, &mut q, &mut body);
    let player = ee.u32_at(quad + 0x680);
    let mut m = UpdateInRam { ee, quad, last: tail_words(&t, quad, player) };
    quad::update(&mut t, &mut q, &mut body, dt, &mut m);
    m.tail_out(&t);
}

/// The quad in RAM as the vehicle's inner update sees it ([`tail::Vehicle`]'s words, the bike's map:
/// the quad is a vehicle of the same base class), written back before each call and read again after.
struct TailInRam<'a> {
    ee: &'a mut Ee,
    quad: u32,
    v: tail::Vehicle,
    last: Vec<(u32, u32)>,
}

impl<'a> TailInRam<'a> {
    fn new(ee: &'a mut Ee, quad: u32) -> TailInRam<'a> {
        let mut m = TailInRam { ee, quad, v: tail::Vehicle::default(), last: Vec::new() };
        m.read();
        m
    }
    fn read(&mut self) {
        let ee = &*self.ee;
        self.v = tail::read(&|a| ee.u32_at(a), self.quad);
        self.last = tail::words(&self.v, self.quad);
    }
    fn write(&mut self) {
        let now = tail::words(&self.v, self.quad);
        set_changed(self.ee, &now, &self.last);
        self.last = now;
    }
}

impl Vehicles for TailInRam<'_> {
    fn vehicle(&mut self) -> &mut tail::Vehicle {
        &mut self.v
    }
    fn skipped(&mut self) -> bool {
        self.ee.u8_at(self.ee.u32_at(SKIP_FLAG) + 0x225) & 8 != 0
    }
    fn paused(&mut self) -> bool {
        unreachable!("the quad's update asks no pause through the vehicle's")
    }
    fn rumble_ready(&mut self) -> bool {
        unreachable!("the quad's update asks no rumble through the vehicle's")
    }
    fn word(&mut self, at: u32) -> u32 {
        self.write();
        self.ee.u32_at(at)
    }
    fn out(&mut self, c: tail::Out, dt: F) {
        use tail::Out;
        self.write();
        let (ee, quad) = (&mut *self.ee, self.quad);
        let qa = u64::from(quad);
        match c {
            Out::WreckTest => {
                own(ee, quad, 0x49c);
                wreck::wreck_test(&mut WreckInRam { ee, quad, x: dt.0 }, 0, dt);
            }
            Out::PutBack => {
                own(ee, quad, 0x3a8);
                port_put_back(ee, quad, true, true, dt.0);
            }
            Out::Prepare => {
                own(ee, quad, 0x3e4);
                port_animate(ee, quad, dt);
            }
            Out::OwnListeners => tail::walk(&mut Tree { ee, quad, quads: 0, others: Vec::new() }, quad + 0xc, dt),
            Out::WreckLast { reason } => {
                call(ee, vt(ee, quad, 0x4a0), &[qa, u64::from(reason)], &[dt.0]);
            }
            Out::FadeOver(o) => {
                call(ee, vt(ee, o, 0x90), &[u64::from(o)], &[dt.0]);
            }
            Out::Network => {
                call(ee, NETWORK, &[qa + 0x400], &[dt.0]);
            }
            Out::Parts => {
                call(ee, PARTS, &[qa], &[dt.0]);
            }
            c => {
                let Some(&(slot, _)) = TAIL_SLOTS.iter().find(|s| s.1 == c) else {
                    panic!("the quad's inner update makes no {c:?}")
                };
                call(ee, vt(ee, quad, slot), &[qa], &[dt.0]);
            }
        }
        self.read();
    }
}

// --- the wreck --------------------------------------------------------------------------------------

/// The quad in RAM as the wreck's port sees it (`tests/mxvatv_bike_ride.rs`' F31b4b glue): every word
/// read and written in place; the wreck entry, unwreck and part toggle, the engine's and wheels'
/// resets the port's, the ray swaps [`port_swap`] and the put-back [`port_put_back`]; every other call
/// out the original's.
struct WreckInRam<'a> {
    ee: &'a mut Ee,
    quad: u32,
    /// What `$f12` is handed with the calls that take no float of their own.
    x: f32,
}

impl Wrecks for WreckInRam<'_> {
    fn bike(&self) -> u32 {
        self.quad
    }
    fn word(&mut self, at: u32) -> u32 {
        self.ee.u32_at(at)
    }
    fn set(&mut self, at: u32, w: u32) {
        self.ee.set_u32(at, w);
    }
    fn normal(&mut self, terrain: u32, at: u32) -> Vf {
        let out = self.ee.scratch(S_NORMAL);
        call(self.ee, TERRAIN_NORMAL, &[u64::from(terrain), u64::from(at), u64::from(out), 0, 0], &[self.x]);
        Vf::from_bits([0, 4, 8, 12].map(|k| self.ee.u32_at(out + k)))
    }
    fn out(&mut self, c: wreck::Out) -> u32 {
        use wreck::Out;
        let (quad, x) = (self.quad, self.x);
        let qa = u64::from(quad);
        let ee = &*self.ee;
        let slot = |s: u32| vt(ee, quad, s);
        let body = ee.u32_at(quad + wreck::BODY);
        let body_fn = |s: u32| ee.u32_at(ee.u32_at(body + 0x110) + s);
        let part_fn = |p: u32, s: u32| ee.u32_at(ee.u32_at(p + 0x28) + s);
        let (f, ints, x) = match c {
            Out::Enter(r) => {
                own(self.ee, quad, 0x368);
                wreck::enter(self, r);
                return 0;
            }
            Out::Unwreck(kind) => {
                own(self.ee, quad, 0x36c);
                wreck::unwreck(self, kind);
                return 0;
            }
            Out::Part(on) => {
                own(self.ee, quad, 0x398);
                wreck::part(self, on);
                return 0;
            }
            Out::ToWreck | Out::ToRide => {
                own(self.ee, quad, if c == Out::ToWreck { 0x48c } else { 0x490 });
                port_swap(self.ee, quad, c == Out::ToWreck, x);
                return 0;
            }
            Out::PutBack { snap, kind } => {
                own(self.ee, quad, 0x3a8);
                assert!(snap <= 1 && kind <= 1, "the put-back handed {snap:#x}, {kind:#x}");
                port_put_back(self.ee, quad, snap != 0, kind != 0, x);
                return 0;
            }
            Out::Engine(at) => {
                wreck::engine_reset(self, at);
                return 0;
            }
            Out::Wheel(at) => {
                wreck::wheel_reset(self, at);
                return 0;
            }
            Out::Sync => (slot(0x378), vec![qa], x),
            Out::OutOfBounds => (slot(0x16c), vec![qa], x),
            Out::Bounds { node, at } => {
                let out = u64::from(ee.scratch(S_BOUNDS));
                (BOUNDS, vec![u64::from(node), u64::from(at), out + 4, out, 0], 150.0)
            }
            Out::Settle => (slot(0x4a8), vec![qa], x),
            Out::Stance(i) => (slot(0x4b0), vec![qa, i as i64 as u64], x),
            Out::Told(l) => {
                let zero = ee.scratch(S_TOLD);
                let f = ee.u32_at(ee.u32_at(l + 0x20) + 0xa0);
                self.ee.set_u32(zero, 0);
                (f, vec![u64::from(l), 5, u64::from(zero), 0, 0], x)
            }
            Out::Rand => (RAND, vec![], x),
            Out::Post { id, arg } => (POST, vec![u64::from(ee.u32_at(POST_QUEUE)), qa + 0x674, 8, u64::from(id), u64::from(arg), 0], x),
            Out::RumbleReady(at) => (RUMBLE_READY, vec![u64::from(at)], x),
            Out::Rumble(r) => (ee.u32_at(ee.u32_at(r + 0x1c) + 0x4c), vec![u64::from(r), 0, 200], 1.0),
            Out::BodyAt { slot, at } => (body_fn(slot), vec![u64::from(body), u64::from(at)], x),
            Out::BodyQuad { slot, q } => {
                let at = ee.scratch(S_QUAD);
                let f = body_fn(slot);
                for (k, w) in q.to_bits().into_iter().enumerate() {
                    self.ee.set_u32(at + 4 * k as u32, w);
                }
                (f, vec![u64::from(body), u64::from(at)], x)
            }
            Out::PartOn(p) => (part_fn(p, 0x20), vec![u64::from(p + 0x24)], x),
            Out::PartOff(p) => (part_fn(p, 0x18), vec![u64::from(p + 0x24)], x),
            Out::Unhook(at) => (UNHOOK, vec![u64::from(at)], x),
            Out::Attach { part, a, b } => (ATTACH, vec![u64::from(part), u64::from(a), u64::from(b), 1], x),
            Out::Rider { a, b } => (RIDER_SETTLE, vec![u64::from(a), u64::from(b), 1, 1], x),
            Out::Pose { bone, buf } => (BONE_POSE, vec![u64::from(bone), 0, u64::from(ee.scratch(S_FRAME + buf))], x),
            Out::Place { part, buf } => (BONE_PLACE, vec![u64::from(part), u64::from(ee.scratch(S_FRAME + buf))], x),
            Out::Ground => (GROUND_RESET, vec![qa], x),
            Out::Rehang { node, to } => (REHANG, vec![u64::from(node), u64::from(to), 1, 1], x),
            Out::Kind(set) => (slot(if set { 0x3a0 } else { 0x39c }), vec![qa], x),
            Out::Trail(at) => (TRAIL_RESET, vec![u64::from(at)], x),
            Out::Camera(at) => (CAMERA_RESET, vec![u64::from(at)], x),
            c => unreachable!("{c:?}: the ground reset is the original's, and the normal is asked through normal()"),
        };
        call(self.ee, f, &ints, &[x]) as u32
    }
}

/// **The ray swap** (the wreck entry's and the unwreck's) as [`quad::swap_rays`], on the quad in RAM.
fn port_swap(ee: &mut Ee, quad: u32, wrecked: bool, x: f32) {
    let record = ee.u32_at(quad + 0xc8);
    let w = |o: u32| ee.u32_at(quad + o);
    let start = quad::Rays { active: w(0xcc), active_too: w(0xd8), record: ee.u32_at(record + 0x200), wreck: w(0x1434), riding: w(0x1438) };
    let to = if wrecked { start.wreck } else { start.riding };
    let in_world = ee.u32_at(to + 0x20) != 0;
    let mut r = start;
    quad::swap_rays(&mut r, wrecked, in_world, &mut Quad::default(), &mut Body::default(), &mut SwapInRam { ee: &mut *ee, quad, x, rays: start });
    set_changed(ee, &[(quad + 0xcc, r.active), (quad + 0xd8, r.active_too), (record + 0x200, r.record)], &[(quad + 0xcc, start.active), (quad + 0xd8, start.active_too), (record + 0x200, start.record)]);
}

struct SwapInRam<'a> {
    ee: &'a mut Ee,
    quad: u32,
    x: f32,
    rays: quad::Rays,
}

impl Moto for SwapInRam<'_> {
    fn call(&mut self, c: Call, _: &mut Quad, _: &mut Body) {
        let ee = &mut *self.ee;
        let part = |p: Part| match p {
            Part::Wreck => self.rays.wreck,
            Part::Riding => self.rays.riding,
            p => panic!("the swap has no {p:?} part"),
        };
        let part_fn = |ee: &Ee, p: u32, s: u32| ee.u32_at(ee.u32_at(p + 0x28) + s);
        let (f, ints) = match c {
            Call::PartHide(p) => (part_fn(ee, part(p), 0x18), vec![u64::from(part(p) + 0x24)]),
            Call::PartShow(p) => (part_fn(ee, part(p), 0x20), vec![u64::from(part(p) + 0x24)]),
            Call::PartOffWorld(p) => (UNHOOK, vec![u64::from(part(p) + 0x18)]),
            Call::PartInWorld(p) => {
                let (world, class) = (ee.u32_at(self.quad + 0xb4), ee.u32_at(self.quad + 0xc0));
                (ATTACH, vec![u64::from(part(p)), u64::from(world), u64::from(class), 1])
            }
            c => panic!("the ray swap makes no {c:?}"),
        };
        call(ee, f, &ints, &[self.x]);
    }
}

// --- the put-back and the animation driver --------------------------------------------------------

/// A frame node's world frame read into its scratch buffer, or handed to a hung node: the put-back's
/// and the animation driver's [`Call::ReadFrame`] and [`Call::SetFrame`].
fn frame_call(ee: &mut Ee, quad: u32, c: Call, x: f32) -> bool {
    match c {
        Call::ReadFrame(k) => {
            let node = ee.u32_at(quad + 0x1104 + 4 * k as u32);
            let buf = ee.scratch(S_FRAME + k as u32);
            call(ee, BONE_POSE, &[u64::from(node), 0, u64::from(buf)], &[x]);
        }
        Call::SetFrame { hung, frame } => {
            let node = ee.u32_at(ee.u32_at(quad + 0x1474) + 4 * hung as u32);
            let buf = ee.scratch(S_FRAME + frame as u32);
            call(ee, BONE_PLACE, &[u64::from(node), u64::from(buf)], &[x]);
        }
        _ => return false,
    }
    true
}

/// The three [`Quad`] words the put-back writes: on the ground, airborne, the crossup.
fn put_back_quad(q: &Quad, quad: u32) -> [(u32, u32); 3] {
    [(quad + 0x324, u32::from(q.on_ground)), (quad + 0xde0, u32::from(q.airborne)), (quad + 0x330, q.crossup.0.to_bits())]
}

fn read_put_back_quad(ee: &Ee, quad: u32, q: &mut Quad) {
    q.on_ground = ee.u32_at(quad + 0x324) != 0;
    q.airborne = ee.u32_at(quad + 0xde0) != 0;
    q.crossup = f_at(ee, quad + 0x330);
}

/// The put-back's calls on the quad in RAM: the wheels' resets and the vehicle's put-back the ports'
/// ([`wreck::wheel_reset`], [`wreck::put_back_base`]), the rest the original's. The [`Quad`] words
/// are written before each call and read back after.
struct PutBackInRam<'a> {
    ee: &'a mut Ee,
    quad: u32,
    x: f32,
    last: [(u32, u32); 3],
    flags_set: bool,
}

impl Moto for PutBackInRam<'_> {
    fn call(&mut self, c: Call, q: &mut Quad, _: &mut Body) {
        let now = put_back_quad(q, self.quad);
        set_changed(self.ee, &now, &self.last);
        let (ee, quad, x) = (&mut *self.ee, self.quad, self.x);
        let w = |ee: &Ee, o: u32| u64::from(ee.u32_at(quad + o));
        if !frame_call(ee, quad, c, x) {
            match c {
                Call::WheelReset(k) => {
                    let wheel = ee.u32_at(quad + 0x17b0 + 4 * k as u32);
                    wreck::wheel_reset(&mut WreckInRam { ee, quad, x }, wheel);
                }
                Call::Detach => {
                    let a = w(ee, 0x1480);
                    call(ee, UNHOOK, &[a], &[x]);
                }
                Call::VehiclePutBack { place, keep, flags } => {
                    if ee.u8_at(quad + 0x45) != flags {
                        set_byte(ee, quad + 0x45, flags);
                        self.flags_set = true;
                    }
                    wreck::put_back_base(&mut WreckInRam { ee, quad, x }, u32::from(place), u32::from(keep));
                }
                Call::Reattach => {
                    let (a, b) = (w(ee, 0x1480), w(ee, 0x1284));
                    call(ee, RIDER_SETTLE, &[a, b, 1, 1], &[x]);
                }
                Call::NetPack => {
                    call(ee, vt(ee, quad, 0x378), &[u64::from(quad)], &[x]);
                }
                Call::ActorMoved => {
                    call(ee, CAMERA_RESET, &[u64::from(quad + 0x43c)], &[x]);
                }
                c => panic!("the put-back makes no {c:?}"),
            }
        }
        read_put_back_quad(self.ee, quad, q);
        self.last = put_back_quad(q, quad);
    }
}

/// **The quad's put-back** (its vt `+0x3a8`) as [`quad::put_back`], on the quad in RAM: what it reads
/// off the quad (`atv_quad.rs`' `poke_put_back` map), and what it changed written back.
fn port_put_back(ee: &mut Ee, quad: u32, place: bool, keep: bool, x: f32) {
    let (_, mut pb, _, _) = animate::read_anim(ee, quad);
    pb.flags = ee.u8_at(quad + 0x45);
    pb.respawn = [0, 4, 8].map(|k| ee.u32_at(quad + 0x1738 + k));
    let player = ee.u32_at(quad + 0x680);
    pb.player_respawn = (player != 0).then(|| [0, 4, 8].map(|k| ee.u32_at(player + 0x94 + k)));
    let start = pb.clone();
    let (mut q, mut body) = (Quad::default(), Body::default());
    read_put_back_quad(ee, quad, &mut q);
    let last = put_back_quad(&q, quad);
    let mut m = PutBackInRam { ee: &mut *ee, quad, x, last, flags_set: false };
    quad::put_back(&mut pb, &mut q, &mut body, place, keep, &mut m);
    let (last, flags_set) = (m.last, m.flags_set);
    set_changed(ee, &put_back_quad(&q, quad), &last);
    if flags_set {
        set_byte(ee, quad + 0x45, pb.flags);
    }
    let b = |x: F| x.0.to_bits();
    set_changed(ee, &[(quad + 0x141c, b(pb.settle)), (quad + 0x31c, pb.request)], &[(quad + 0x141c, b(start.settle)), (quad + 0x31c, start.request)]);
    if let (Some(now), Some(was)) = (pb.player_respawn, start.player_respawn) {
        let at = |r: [u32; 3]| [0, 1, 2].map(|k| (player + 0x94 + 4 * k, r[k as usize]));
        set_changed(ee, &at(now), &at(was));
    }
}

/// The stunt table's two answers for the one stunt the driver asks about, read before it runs
/// (`animate.rs`' [`animate::Table`]).
struct Clips {
    stunt: i32,
    ready: bool,
    length: F,
}

impl quad::StuntClips for Clips {
    fn ready(&mut self, stunt: i32) -> bool {
        assert_eq!(stunt, self.stunt, "the stunt the driver asks about");
        self.ready
    }
    fn length(&mut self, stunt: i32) -> F {
        assert_eq!(stunt, self.stunt, "the stunt the driver asks about");
        self.length
    }
}

/// The [`Quad`] words the driver reads (the stick, the crossup) and writes (the world angular velocity).
fn read_anim_quad(ee: &Ee, quad: u32, q: &mut Quad) {
    q.lean = f_at(ee, quad + 0x12e8);
    q.lean_fore_aft = f_at(ee, quad + 0x12ec);
    q.crossup = f_at(ee, quad + 0x330);
    q.world_ang_vel = vf_at(ee, quad + 0x340);
}

fn anim_quad(q: &Quad, quad: u32) -> Vec<(u32, u32)> {
    q.world_ang_vel.to_bits().into_iter().enumerate().map(|(k, w)| (quad + 0x340 + 4 * k as u32, w)).collect()
}

/// The animation driver's calls on the quad in RAM, all the original's; the world angular velocity
/// written before each and the [`Quad`] words read back after.
struct AnimInRam<'a> {
    ee: &'a mut Ee,
    quad: u32,
    x: f32,
    last: Vec<(u32, u32)>,
}

impl Moto for AnimInRam<'_> {
    fn call(&mut self, c: Call, q: &mut Quad, _: &mut Body) {
        let now = anim_quad(q, self.quad);
        set_changed(self.ee, &now, &self.last);
        let (ee, quad, x) = (&mut *self.ee, self.quad, self.x);
        let qa = u64::from(quad);
        let controller = u64::from(ee.u32_at(quad + 0x1428));
        let handle = |ee: &Ee, i: i32| ee.u32_at(quad.wrapping_add(0x1534).wrapping_add(4 * i as u32)) as i32 as i64 as u64;
        if !frame_call(ee, quad, c, x) {
            match c {
                Call::NetInterpolate { older, newer, t } => {
                    let snap = |i: usize| u64::from(ee.u32_at(quad + SNAP_SLOTS[i]));
                    let (o, n) = (snap(older), snap(newer));
                    call(ee, vt(ee, quad, 0x3ec), &[qa, o, n], &[x, t.0]);
                }
                Call::NetUnpack(dt) => {
                    call(ee, vt(ee, quad, 0x3f0), &[qa], &[dt.0]);
                }
                Call::AnimStop => {
                    call(ee, ANIM_STOP, &[controller, 0, 0, 0, 0], &[x]);
                }
                Call::PlayClip { clip, weight, fade } => {
                    let h = handle(ee, clip);
                    call(ee, PLAY_CLIP, &[controller, h, 0, 0, 0, 0, 0, 0], &[weight.0, fade.0]);
                }
                Call::BlendClips { clips, weight, mix, fade } => {
                    let (h0, h1) = (handle(ee, clips[0]), handle(ee, clips[1]));
                    call(ee, BLEND_CLIPS, &[controller, h0, h1, 0, 0, 0, 0, 0], &[weight.0, mix.0, fade.0]);
                }
                Call::PoseNodes => {
                    call(ee, POSE_NODES, &[qa], &[x]);
                }
                Call::AttachmentMoved => {
                    let a = u64::from(ee.u32_at(quad + 0x1480));
                    call(ee, ATTACHMENT_MOVED, &[a], &[x]);
                }
                c => panic!("the animation driver makes no {c:?}"),
            }
        }
        read_anim_quad(self.ee, quad, q);
        self.last = anim_quad(q, quad);
    }
}

/// **The animation driver** (the quad's vt `+0x3e4`, the inner update's prepare) as [`quad::animate`],
/// on the quad in RAM (`animate.rs`' map), the nine words it writes put back where they changed.
fn port_animate(ee: &mut Ee, quad: u32, dt: F) {
    let (mut a, mut pb, _, net) = animate::read_anim(ee, quad);
    let start = (a, pb.clone());
    let mut q = Quad::default();
    read_anim_quad(ee, quad, &mut q);
    let stunt = a.next_stunt;
    let mut clips = Clips { stunt, ready: false, length: F::ZERO };
    if (0..quad::RIDING_POSE).contains(&stunt) {
        let mut t = animate::Table(ee, quad);
        clips.ready = quad::StuntClips::ready(&mut t, stunt);
        if clips.ready {
            clips.length = quad::StuntClips::length(&mut t, stunt);
        }
    }
    let last = anim_quad(&q, quad);
    let mut m = AnimInRam { ee: &mut *ee, quad, x: dt.0, last };
    quad::animate(&mut a, &mut pb, &mut q, &mut Body::default(), &net, dt, &mut clips, &mut m);
    let last = m.last;
    set_changed(ee, &anim_quad(&q, quad), &last);
    let words = |a: &quad::Anim, pb: &quad::PutBack| {
        let b = |x: F| x.0.to_bits();
        [(quad + 0x14bc, b(a.fade)), (quad + 0x1524, a.stunt as u32), (quad + 0x14b4, b(a.crossup_blend)), (quad + 0x14b8, b(a.fore_aft_blend)), (quad + 0x141c, b(pb.settle))]
    };
    set_changed(ee, &words(&a, &pb), &words(&start.0, &start.1));
}

// --- the walk -------------------------------------------------------------------------------------

/// The sim's listener tree in RAM, walked by [`tail::walk`]: a group's step (the walk itself) is the
/// port's walk one level down, the quad's step [`port_update`], any other listener's the original's.
struct Tree<'a> {
    ee: &'a mut Ee,
    quad: u32,
    quads: u32,
    others: Vec<u32>,
}

impl Listeners for Tree<'_> {
    fn word(&mut self, at: u32) -> u32 {
        self.ee.u32_at(at)
    }
    fn step(&mut self, e: u32, dt: F) {
        let f = vt(self.ee, e, 0x40);
        if f == TAIL {
            tail::walk(self, e, dt);
        } else if e == self.quad + 0xc {
            assert_eq!(f, QUAD_STEP, "the quad's listener's step");
            port_update(self.ee, self.quad, dt);
            self.quads += 1;
        } else {
            self.others.push(e);
            call(self.ee, f, &[u64::from(e)], &[dt.0]);
        }
    }
}

/// **The frame's tail** on the sim, the port's ([`Tree`]): how many times the quad stepped, and the
/// other listeners stepped (the original's).
pub(super) fn port_tail(ee: &mut Ee, quad: u32, dt: F) -> (u32, Vec<u32>) {
    let mut t = Tree { ee, quad, quads: 0, others: Vec::new() };
    tail::walk(&mut t, image::heap().sim, dt);
    (t.quads, t.others)
}
