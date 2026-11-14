//! MD81d — MX vs. ATV Untamed's (SLUS-21701) car step against MX vs. ATV Unleashed's (SLUS-21104),
//! proved where the port switches to it.
//!
//! The car's world step, its tires and its engine are SLUS-21104's with the fields moved
//! (research/untamed-machines.md, "SLUS-21701's car step against SLUS-21104's (MD81d)"). Where Untamed's
//! differs and the port follows it behind [`atv_car::Load::untamed`]:
//!
//! - the preload's cue is posted while the push is on ([`atv_car::Keys::untamed`]), where SLUS-21104's
//!   is posted while the timer is above 0: they part with `PreloadTime` 0 (Untamed's baja has its
//!   `[Preload]` off) and while the preload is armed and winding;
//! - the vehicle's force adds a zero force after the gravity ([`atv_car::Vehicle::untamed`]);
//! - the update's hazard clock is told whether a tire is down ([`atv_car::Update::untamed`]; the clock is
//!   MD81b's [`game_sim::handling::untamed::stuck`], proved in tests/untamed_atv.rs).
//!
//! The oracle is SLUS-21701's own functions off a RAM image of a Free Ride in it
//! (`context/untamed/ram/nat_or.bin`, as tests/untamed_atv.rs), run in the interpreter on a car laid out
//! at Untamed's offsets. Without the image the tests skip (and say so).

use game_sim::atv_body::Body;
use game_sim::atv_car::{self, Call, Car, Keys, Pieces, PreloadState, State, Steps, Update};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Where the harness puts the car, its body, the body's vtable and the collision record with its
/// vtable: the top of RAM, under the stack.
const OBJ: u32 = 0x01c0_0000;
const BODY: u32 = 0x01c0_2000;
const BODY_VT: u32 = 0x01c0_2400;
const RECORD: u32 = 0x01c0_2800;
const RECORD_VT: u32 = 0x01c0_2900;
const GRAVITY: u32 = 0x01c0_2a00;
/// Hooked: the record's gravity call (record vt `+0x88`) and the body's add-force when it is only
/// counted.
const RECORD_CALL: u32 = 0x01c0_2b00;
const ADD_FORCE_HOOK: u32 = 0x01c0_2b10;

/// SLUS-21701's preload (car vt `+0x474`), vehicle force, the add-force the body's vt `+0x80` is in
/// it (the vehicle base's, the force into body `+0xa0`, then each listener), and the event post; the
/// first two words of each, to check the image.
const PRELOAD: u32 = 0x004e_9210;
const PRELOAD_HEAD: [u32; 2] = [0x27bd_ffe0, 0xffbf_0010];
const VEHICLE_FORCE: u32 = 0x004b_a390;
const VEHICLE_FORCE_HEAD: [u32; 2] = [0x27bd_ffd0, 0xffbf_0010];
const ADD_FORCE: u32 = 0x004b_a450;
const ADD_FORCE_HEAD: [u32; 2] = [0x27bd_ffc0, 0xffbf_0030];
const POST_EVENT: u32 = 0x0025_e740;

/// The car's words at Untamed's offsets (SLUS-21104's less 0x10 in the `+0xb..+0x12` block, the
/// `+0x3xx` words and the body pointer +0x30).
const STATE: u32 = 0xc70;
const READY: u32 = 0xc40;
const ACTIVE: u32 = 0xc44;
const ARMED: u32 = 0xc48;
const PRELOAD_TIME: u32 = 0xc4c;
const TIMER: u32 = 0xc50;
const MIN_TIME: u32 = 0xc54;
const MAX_TIME: u32 = 0xc58;
const MAX_PRELOAD: u32 = 0xc5c;
const FORCE: u32 = 0xc60;
const CUED: u32 = 0x724;
const GROUNDED: u32 = 0x354;
const BRAKE: u32 = 0x340;
const VELOCITY: u32 = 0x380;
const LEAN: u32 = 0x11bc;
const PAD_WORD: u32 = 0x1114;
const BODY_AT: u32 = 0xe8;
const RECORD_AT: u32 = 0xf8;
const GRAVITY_AT: u32 = 0x118;
const FLAGS: u32 = 0x75;
/// The body's: enabled, its velocity, its force, its listener count, its vtable.
const B_ENABLED: u32 = 0;
const B_VELOCITY: u32 = 0x80;
const B_FORCE: u32 = 0xa0;
const B_LISTENERS: u32 = 0xe8;
const B_VT: u32 = 0x110;

fn image(heads: &[(u32, [u32; 2])]) -> Option<Ee> {
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
    for &(at, head) in heads {
        if [ee.u32_at(at), ee.u32_at(at + 4)] != head {
            panic!("{} is not the MX vs. ATV Untamed image these addresses are from ({at:#x})", path.display());
        }
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
    fn f(&mut self, lo: f32, hi: f32) -> F {
        F(lo + (hi - lo) * self.unit())
    }
    fn b(&mut self) -> bool {
        self.bits() & 0x10000 != 0
    }
    fn one_in(&mut self, n: u32) -> bool {
        self.bits() % n == 0
    }
    fn pick(&mut self, from: &[f32]) -> F {
        F(from[self.bits() as usize % from.len()])
    }
    /// A force lane: mostly ordinary, now and then a signed zero or a denormal.
    fn lane(&mut self) -> f32 {
        match self.bits() % 6 {
            0 => -0.0,
            1 => 0.0,
            2 => f32::from_bits(0x8000_0001 | (self.bits() & 0x7f_ffff)),
            _ => self.f(-5000.0, 5000.0).0,
        }
    }
    fn quad(&mut self) -> Vf {
        Vf([self.lane(), self.lane(), self.lane(), self.lane()])
    }
}

fn put_v(ee: &mut Ee, a: u32, v: Vf) {
    for k in 0..4 {
        ee.set_f32(a + 4 * k as u32, v.0[k]);
    }
}
fn get_v(ee: &Ee, a: u32) -> [u32; 4] {
    std::array::from_fn(|k| ee.u32_at(a + 4 * k as u32))
}
fn clear(ee: &mut Ee) {
    for o in (0..0x1800).step_by(4) {
        ee.set_u32(OBJ + o, 0);
    }
    for o in (0..0xc00).step_by(4) {
        ee.set_u32(BODY + o, 0);
    }
    ee.set_u32(OBJ + BODY_AT, BODY);
    ee.set_u32(BODY + B_VT, BODY_VT);
}

/// What the port's pieces hand out: the cue counted.
#[derive(Default)]
struct Heard(Vec<Call>);

impl Steps for Heard {
    fn call(&mut self, c: Call, _car: &mut Car, _body: &mut Body) {
        self.0.push(c);
    }
}

// --- the preload ---------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct Case {
    state: u32,
    p: PreloadState,
    preload_time: F,
    min_time: F,
    max_time: F,
    max_preload: F,
    grounded: bool,
    rise: F,
    speed: F,
    brake: F,
    lean: F,
    pad_word: F,
    force: Vf,
    h: F,
}

fn case(g: &mut Gen) -> Case {
    let min_time = g.f(0.0, 30.0);
    Case {
        state: if g.one_in(10) { 1 + g.bits() % 3 } else { atv_car::RIDING },
        p: PreloadState {
            ready: g.b(),
            active: g.one_in(3),
            armed: g.b(),
            timer: if g.one_in(4) { g.pick(&[0.0, -0.0, 0.25]) } else { g.f(-0.2, 0.6) },
            force: Vf::new(g.f(-5.0, 5.0).0, g.f(0.0, 3000.0).0, g.f(-5.0, 5.0).0, g.f(-2.0, 2.0).0),
            cued: g.b(),
        },
        // Untamed's baja: `[Preload]` off, 0 throughout.
        preload_time: if g.one_in(3) { F::ZERO } else { g.f(0.0, 0.5) },
        min_time,
        max_time: if g.one_in(6) { min_time } else { g.f(10.0, 80.0) },
        max_preload: if g.one_in(4) { F::ZERO } else { g.f(0.0, 3000.0) },
        grounded: !g.one_in(6),
        rise: if g.one_in(8) { F(5.0) } else { g.f(3.0, 12.0) },
        speed: if g.one_in(8) { F(30.0) } else { g.f(20.0, 60.0) },
        brake: if g.one_in(6) { F::ONE } else { F::ZERO },
        lean: if g.one_in(5) { g.pick(&[0.8, -0.8, 0.8000001, -0.8000001]) } else { g.f(-1.2, 1.2) },
        pad_word: g.f(-90.0, 90.0),
        force: g.quad(),
        h: g.pick(&[1.0 / 60.0, 1.0 / 120.0, 1.0 / 30.0, 0.25]),
    }
}

fn lay_preload(ee: &mut Ee, c: &Case) {
    clear(ee);
    ee.set_u32(OBJ + STATE, c.state);
    ee.set_u32(OBJ + READY, u32::from(c.p.ready));
    ee.set_u32(OBJ + ACTIVE, u32::from(c.p.active));
    ee.set_u32(OBJ + ARMED, u32::from(c.p.armed));
    ee.set_f32(OBJ + TIMER, c.p.timer.0);
    put_v(ee, OBJ + FORCE, c.p.force);
    ee.set_u32(OBJ + CUED, u32::from(c.p.cued));
    ee.set_f32(OBJ + PRELOAD_TIME, c.preload_time.0);
    ee.set_f32(OBJ + MIN_TIME, c.min_time.0);
    ee.set_f32(OBJ + MAX_TIME, c.max_time.0);
    ee.set_f32(OBJ + MAX_PRELOAD, c.max_preload.0);
    ee.set_u32(OBJ + GROUNDED, u32::from(c.grounded));
    ee.set_f32(BODY + B_VELOCITY + 4, c.rise.0);
    ee.set_f32(OBJ + VELOCITY + 8, c.speed.0);
    ee.set_f32(OBJ + BRAKE, c.brake.0);
    ee.set_f32(OBJ + LEAN, c.lean.0);
    ee.set_f32(OBJ + PAD_WORD, c.pad_word.0);
    put_v(ee, BODY + B_FORCE, c.force);
    ee.set_u32(BODY_VT + 0x80, ADD_FORCE_HOOK);
}

/// The port on the case: the preload's words, the body's force, whether the cue was posted.
fn port_preload(c: &Case, untamed: bool) -> (PreloadState, Vf, bool) {
    let keys = Keys {
        preload_time: c.preload_time,
        min_preload_time: c.min_time,
        max_preload_time: c.max_time,
        max_preload: c.max_preload,
        untamed,
        ..Keys::default()
    };
    let state = State { lean: c.lean, pad_word: c.pad_word, vehicle_brake: c.brake, preload: c.p, ..State::default() };
    let mut car = Car { state: c.state, grounded: c.grounded, velocity: Vf::new(0.0, 0.0, c.speed.0, 0.0), ..Car::default() };
    let mut body = Body { velocity: Vf::new(0.0, c.rise.0, 0.0, 0.0), force: c.force, ..Body::default() };
    let mut pieces = Pieces { keys, state, rest: Heard::default() };
    assert!(pieces.answer(Call::Preload(c.h), &mut car, &mut body), "the pieces answer the preload");
    let cued = pieces.rest.0.contains(&Call::PreloadCue);
    (pieces.state.preload, body.force, cued)
}

fn preload_words(p: &PreloadState) -> [u32; 10] {
    let f = p.force.0.map(f32::to_bits);
    [u32::from(p.ready), u32::from(p.active), u32::from(p.armed), p.timer.0.to_bits(), f[0], f[1], f[2], f[3], u32::from(p.cued), 0]
}

#[test]
fn untamed_car_preload_is_the_originals() {
    let Some(mut ee) = image(&[(PRELOAD, PRELOAD_HEAD)]) else { return };
    // The body's add-force, counted with what it is handed; the event post, with its words.
    let pushed = Rc::new(RefCell::new(Vec::<[u32; 4]>::new()));
    let posted = Rc::new(RefCell::new(Vec::<(u64, u64)>::new()));
    let p2 = pushed.clone();
    ee.hooks.insert(ADD_FORCE_HOOK, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, BODY, "the push goes to the car's body");
        let a = ee.r(5) as u32;
        p2.borrow_mut().push(std::array::from_fn(|k| ee.u32_at(a + 4 * k as u32)));
    }));
    let q2 = posted.clone();
    ee.hooks.insert(POST_EVENT, Box::new(move |ee| {
        assert_eq!(ee.r(5) as u32, OBJ + 0x698, "the cue is posted from the car's event source");
        q2.borrow_mut().push((ee.r(6), ee.r(7)));
    }));
    let mut g = Gen(0x4d44_3831_6400);
    let mut tally = BTreeMap::<&str, u32>::new();
    for k in 0..20_000 {
        let c = case(&mut g);
        lay_preload(&mut ee, &c);
        pushed.borrow_mut().clear();
        posted.borrow_mut().clear();
        ee.call(PRELOAD, &[u64::from(OBJ)], &[c.h.0], 10_000);

        let want_p = PreloadState {
            ready: ee.u32_at(OBJ + READY) != 0,
            active: ee.u32_at(OBJ + ACTIVE) != 0,
            armed: ee.u32_at(OBJ + ARMED) != 0,
            timer: F(f32::from_bits(ee.u32_at(OBJ + TIMER))),
            force: Vf(get_v(&ee, OBJ + FORCE).map(f32::from_bits)),
            cued: ee.u32_at(OBJ + CUED) != 0,
        };
        let posts = posted.borrow().clone();
        assert!(posts.iter().all(|&p| p == (8, 14)), "case {k}: the cue is event (8, 14): {posts:?}");
        let (got_p, got_force, got_cue) = port_preload(&c, true);
        assert_eq!(preload_words(&got_p), preload_words(&want_p), "case {k}: the preload's words: {c:?}");
        assert_eq!(got_cue, posts.len() == 1, "case {k}: the cue: {c:?}");
        // The push: the original hands the body's add-force the preload's force at most once (the
        // add itself is the vehicle force's below); the port's body has it added (`xyz`) then.
        let pushes = pushed.borrow().clone();
        assert!(pushes.len() <= 1, "case {k}: pushed {} times", pushes.len());
        let mut want_force = c.force.0.map(f32::to_bits);
        if let [f] = pushes[..] {
            for i in 0..3 {
                want_force[i] = ps2_float::vu::add(c.force.0[i], f32::from_bits(f[i])).to_bits();
            }
        }
        assert_eq!(got_force.0.map(f32::to_bits), want_force, "case {k}: the body's force: {c:?}");

        // SLUS-21104's cue on the same case: where they part.
        let (old_p, _, old_cue) = port_preload(&c, false);
        let path = match (c.state == atv_car::RIDING, got_cue == old_cue && old_p == got_p, got_cue) {
            (false, _, _) => "wrecked",
            (true, true, true) => "cued, as SLUS-21104",
            (true, true, false) => "not cued, as SLUS-21104",
            (true, false, true) => "cued where SLUS-21104 is not",
            (true, false, false) => "not cued where SLUS-21104 is",
        };
        *tally.entry(path).or_default() += 1;
        if c.preload_time == 0.0 && got_p.active && !c.p.active && got_cue {
            *tally.entry("fired with PreloadTime 0, cued").or_default() += 1;
        }
    }
    eprintln!("MD81d preload: {tally:?}");
    for want in [
        "wrecked",
        "cued, as SLUS-21104",
        "not cued, as SLUS-21104",
        "cued where SLUS-21104 is not",
        "not cued where SLUS-21104 is",
        "fired with PreloadTime 0, cued",
    ] {
        assert!(tally.get(want).copied().unwrap_or(0) >= 50, "too few cases went {want:?}: {tally:?}");
    }
}

// --- the vehicle force ---------------------------------------------------------------------------

#[test]
fn untamed_vehicle_force_is_the_originals() {
    let Some(mut ee) = image(&[(VEHICLE_FORCE, VEHICLE_FORCE_HEAD), (ADD_FORCE, ADD_FORCE_HEAD)]) else { return };
    let recorded = Rc::new(RefCell::new(0u32));
    let r2 = recorded.clone();
    ee.hooks.insert(RECORD_CALL, Box::new(move |ee| {
        assert_eq!((ee.r(4) as u32, ee.r(5)), (RECORD, 1), "the record's gravity call");
        *r2.borrow_mut() += 1;
    }));
    let mut g = Gen(0x4d44_3831_6401);
    let mut tally = BTreeMap::<&str, u32>::new();
    for k in 0..20_000 {
        let (force, gravity) = (g.quad(), g.quad());
        let (enabled, record, flags) = (!g.one_in(8), g.b(), (g.bits() & 0xff) as u8);
        clear(&mut ee);
        // The real add-force (the body's vt `+0x80`), with no listeners.
        ee.set_u32(BODY_VT + 0x80, ADD_FORCE);
        ee.set_u32(BODY + B_ENABLED, u32::from(enabled));
        ee.set_u32(BODY + B_LISTENERS, 0);
        put_v(&mut ee, BODY + B_FORCE, force);
        ee.set_u32(OBJ + GRAVITY_AT, GRAVITY - 0x40);
        put_v(&mut ee, GRAVITY, gravity);
        ee.set_u32(OBJ + RECORD_AT, if record { RECORD } else { 0 });
        ee.set_u32(RECORD + 4, RECORD_VT);
        ee.set_u32(RECORD_VT + 0x88, RECORD_CALL);
        ee.write(OBJ + FLAGS, 1, u128::from(flags));
        *recorded.borrow_mut() = 0;
        ee.call(VEHICLE_FORCE, &[u64::from(OBJ)], &[], 10_000);

        let mut v = atv_car::Vehicle { gravity, record, flags, untamed: true, ..Default::default() };
        let mut body = Body { enabled, force, ..Body::default() };
        let mut car = Car::default();
        let mut heard = Heard::default();
        atv_car::vehicle_force(&mut atv_car::Own { v: &mut v, rest: &mut heard }, &mut body, &mut car);
        let want = (get_v(&ee, BODY + B_FORCE), ee.u8_at(OBJ + FLAGS), *recorded.borrow());
        let got = (body.force.0.map(f32::to_bits), v.flags, heard.0.iter().filter(|c| **c == Call::GravityRecord).count() as u32);
        assert_eq!(got, want, "case {k}: force {force:?} gravity {gravity:?} enabled {enabled} record {record}");

        let mut old_v = atv_car::Vehicle { gravity, record, flags, ..Default::default() };
        let mut old = Body { enabled, force, ..Body::default() };
        atv_car::vehicle_force(&mut atv_car::Own { v: &mut old_v, rest: &mut Heard::default() }, &mut old, &mut car);
        let path = match (enabled, old.force.0.map(f32::to_bits) == got.0) {
            (false, _) => "off",
            (true, true) => "as SLUS-21104",
            (true, false) => "the zero add tells",
        };
        *tally.entry(path).or_default() += 1;
    }
    eprintln!("MD81d vehicle force: {tally:?}");
    for want in ["off", "as SLUS-21104", "the zero add tells"] {
        assert!(tally.get(want).copied().unwrap_or(0) >= 100, "too few cases went {want:?}: {tally:?}");
    }
}

// --- the update's hazard clock -------------------------------------------------------------------

/// The car's update asks Untamed's clock, with the grounded word, only when switched to Untamed; and
/// SLUS-21104's, as before, when not.
#[test]
fn untamed_car_update_asks_the_untamed_clock() {
    let mut g = Gen(0x4d44_3831_6402);
    let mut seen = BTreeMap::<(bool, bool, bool), u32>::new();
    for _ in 0..400 {
        let untamed = g.b();
        let mut u = Update { vehicle_state: Some(1), untamed, ..Update::default() };
        for s in &mut u.surfaces {
            (s.hazard, s.time) = (g.one_in(3), g.f(0.0, 3.0));
        }
        u.tire_surface = std::array::from_fn(|_| (g.bits() % 16) as u8);
        let mut car = Car::default();
        for t in &mut car.tires {
            t.solid = g.one_in(3);
        }
        let mut heard = Heard::default();
        atv_car::update(&mut u, &mut car, &mut Body::default(), F(1.0 / 60.0), &mut heard);
        let grounded = car.tires.iter().any(|t| t.solid);
        let sunk = u.on_hazard && grounded;
        let clock = heard.0.iter().find(|c| matches!(c, Call::HazardClock { .. } | Call::UntamedHazardClock { .. }));
        match (untamed, clock) {
            (true, Some(Call::UntamedHazardClock { sunk: s, grounded: gr, .. })) => assert_eq!((*s, *gr), (sunk, grounded)),
            (false, Some(Call::HazardClock { sunk: s, .. })) => assert_eq!(*s, sunk),
            (u, c) => panic!("untamed {u}: the clock asked was {c:?}"),
        }
        *seen.entry((untamed, sunk, grounded)).or_default() += 1;
    }
    for untamed in [false, true] {
        for (sunk, grounded) in [(false, false), (false, true), (true, true)] {
            assert!(seen.get(&(untamed, sunk, grounded)).copied().unwrap_or(0) >= 10, "{seen:?}");
        }
    }
}
