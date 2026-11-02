//! MD63 — the rest of MX vs. ATV's bike step, clause by clause, proved against MX vs. ATV's own code.
//!
//! The oracle is SLUS-21104 off a RAM image of a Free Ride in it (`context/mxvatv-ps2/state04.ee.bin`,
//! `tests/mxvatv_air.rs`' image): its functions run in the interpreter as they are, with their own
//! vtables. Lay the bike out in RAM, write a generated state into it, run the original, write the same
//! state again with the port's answer over it, and compare the regions word for word; the original may
//! write nothing else. Calls out of the step are hooked and logged, and the port says what it asked for.
//!
//! MD63a adds the input limiter and the steering command's input ramp.
//!
//! Without the image the oracle tests here skip (and say so).

use game_sim::air::{self, Air, Chassis, Params};
use game_sim::handling::step::{self, GasCut, LimiterIn, SteerRamp};
use game_sim::phy::Joystick;
use game_sim::ride::{Controls, Held};
use game_sim::trick::Stick;
use game_sim::r#move as mv;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Where the harness puts the bike and its pieces: the top of RAM, under the interpreter's stack.
const OBJ: u32 = 0x01c0_0000;
const BODY: u32 = 0x01c0_2000;
const ENGINE: u32 = 0x01c0_2400;
const REGIONS: [(&str, u32, u32); 3] = [("bike", OBJ, 0x1800), ("body", BODY, 0x200), ("engine", ENGINE, 0x100)];

/// The bike's and its rigid body's vtables in the image, the steps, and the event post the preload calls.
const BIKE_VTABLE: u32 = 0x003d_0450;
const BODY_VTABLE: u32 = 0x003c_d200;
const WRECK_ENTRY: u32 = 0x004e_2260;
const CLUTCH: u32 = 0x004e_6a60;
const PRELOAD: u32 = 0x004e_60d0;
const POST_EVENT: u32 = 0x0027_2300;

fn image() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2").join(format!("state04.ee{}", ".bin"));
    if !path.exists() {
        skip(&format!(
            "{} is not there: unzip eeMemory.bin from MX vs. ATV's save state 4 (SLUS-21104 (95BB1901).04.p2s, \
             ps2-decomp-kit-slots/pcsx2/s9) to it (s1 has it)",
            path.display()
        ));
        return None;
    }
    let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    if ee.u32_at(BIKE_VTABLE + 0x368) != WRECK_ENTRY {
        panic!("{} is not the MX vs. ATV image these addresses are from", path.display());
    }
    Some(ee)
}

fn put_f(ee: &mut Ee, a: u32, v: F) {
    ee.set_f32(a, v.0);
}
fn put_b(ee: &mut Ee, a: u32, v: bool) {
    ee.set_u32(a, u32::from(v));
}
fn put_v(ee: &mut Ee, a: u32, v: Vf) {
    for k in 0..4 {
        ee.set_f32(a + 4 * k as u32, v.0[k]);
    }
}

fn reset(ee: &mut Ee) {
    for (_, base, len) in REGIONS {
        for o in (0..len).step_by(4) {
            ee.set_u32(base + o, 0);
        }
    }
    ee.set_u32(OBJ + 0x4, BIKE_VTABLE);
    ee.set_u32(OBJ + 0xb8, BODY);
    ee.set_u32(OBJ + 0x142c, ENGINE);
    ee.set_u32(BODY + 0x110, BODY_VTABLE);
    ee.set_u32(BODY + 0xe8, 0); // no force listeners on the body
}

fn snap(ee: &Ee) -> Vec<u32> {
    REGIONS.iter().flat_map(|&(_, base, len)| (0..len).step_by(4).map(move |o| base + o)).map(|a| ee.u32_at(a)).collect()
}

fn where_is(i: usize) -> String {
    let mut i = i as u32 * 4;
    for (name, _, len) in REGIONS {
        if i < len {
            return format!("{name} + {i:#x}");
        }
        i -= len;
    }
    "past the end".into()
}

fn only_touched_the_regions(ee: &Ee) -> Result<(), String> {
    let frame = (ee.stack - 0x2000, ee.stack + 0x40);
    for (a, b) in ee.written_ranges() {
        let inside = REGIONS.iter().any(|&(_, base, len)| a >= base && b <= base + len) || (a >= frame.0 && b <= frame.1);
        if !inside {
            return Err(format!("the original wrote {a:#010x}..{b:#010x}, which is not the bike or the stack"));
        }
    }
    Ok(())
}

fn compare(k: usize, path: &str, was: &[u32], now: &[u32], s: &dyn std::fmt::Debug) {
    if was != now {
        let i = was.iter().zip(now).position(|(a, b)| a != b).unwrap();
        panic!(
            "state {k} ({path}): {} is {:#010x} ({}) in the original and {:#010x} ({}) in the port\n{s:#?}",
            where_is(i),
            was[i],
            f32::from_bits(was[i]),
            now[i],
            f32::from_bits(now[i]),
        );
    }
}

fn enough(tally: &BTreeMap<&'static str, u32>, paths: &[&str], at_least: u32) {
    for want in paths {
        let n = tally.get(want).copied().unwrap_or(0);
        assert!(n >= at_least, "only {n} state(s) went down {want:?}: {tally:?}");
    }
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
    /// A value sometimes exactly on `at`, the threshold a branch compares it with.
    fn near(&mut self, at: f32, lo: f32, hi: f32) -> F {
        if self.one_in(5) { F(at) } else { self.f(lo, hi) }
    }
}

// --- the clutch ---------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
struct ClutchState {
    p: mv::Params,
    r: mv::Ride,
    eng: mv::Engine,
    s: mv::Step,
}

impl Gen {
    fn clutch(&mut self) -> ClutchState {
        let mut c = ClutchState::default();
        c.p.clutch_torque_scale_time = if self.one_in(5) { F::ZERO } else { self.f(0.1, 2.0) };
        c.p.max_clutch_torque_scale_speed = if self.one_in(5) { F::ZERO } else { self.f(5.0, 40.0) };
        c.p.max_clutch_torque_scale = self.f(0.5, 4.0);
        c.p.clutch_cofg_travel_back = self.f(0.0, 2.0);
        c.r.clutch_timer = match self.bits() % 5 {
            0 => F::ZERO,
            1 => c.p.clutch_torque_scale_time,
            // Under zero, the only timer an empty time range runs on.
            2 => self.f(-1.0, 0.0),
            _ => self.f(0.0, 2.5),
        };
        c.r.clutch_scale_start = self.f(-1.0, 4.0);
        c.r.clutch_cofg_weight = self.f(-1.0, 2.0);
        c.eng.clutch_torque_scale = self.f(-1.0, 4.0);
        c.s.clutch = match self.bits() % 3 {
            0 => F::ZERO,
            1 => F::ONE,
            _ => self.f(0.0, 1.0),
        };
        c.s.gas = if self.b() { F::ONE } else { self.f(0.0, 1.0) };
        c.s.speed = self.f(-10.0, 60.0);
        c.s.dt = self.f(0.005, 0.04);
        c
    }
}

fn poke_clutch(ee: &mut Ee, c: &ClutchState) {
    put_f(ee, OBJ + 0x12c0, c.s.clutch);
    put_f(ee, OBJ + 0x314, c.s.gas);
    put_f(ee, OBJ + 0xc38, c.s.speed);
    put_f(ee, OBJ + 0x1348, c.r.clutch_timer);
    put_f(ee, OBJ + 0x134c, c.r.clutch_scale_start);
    put_f(ee, OBJ + 0x1350, c.p.clutch_torque_scale_time);
    put_f(ee, OBJ + 0x1354, c.r.clutch_cofg_weight);
    put_f(ee, OBJ + 0x1358, c.p.max_clutch_torque_scale_speed);
    put_f(ee, OBJ + 0x135c, c.p.max_clutch_torque_scale);
    put_f(ee, ENGINE + 0x7c, c.eng.clutch_torque_scale);
}

/// Which way a state went, from the decompile's branches and not the port's.
fn clutch_path(c: &ClutchState) -> &'static str {
    if c.s.clutch == 1.0 && c.s.gas == 1.0 {
        "rearmed"
    } else if c.s.clutch != 0.0 || c.r.clutch_timer > c.p.clutch_torque_scale_time {
        "left alone"
    } else if c.r.clutch_timer == 0.0 {
        if c.p.max_clutch_torque_scale_speed == 0.0 { "started, no speed range" } else { "started" }
    } else if c.p.clutch_torque_scale_time == 0.0 {
        "running, no time range"
    } else {
        "running"
    }
}

/// SLUS-21104's clutch ramp, the port's [`step::clutch`]: 10,000 states over every case.
#[test]
fn mxvatv_clutch_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let mut g = Gen(0x0d63_0001);
    let mut tally = BTreeMap::new();
    for k in 0..10_000 {
        let c = g.clutch();
        let path = clutch_path(&c);
        *tally.entry(path).or_insert(0u32) += 1;
        reset(&mut ee);
        poke_clutch(&mut ee, &c);
        ee.call(CLUTCH, &[u64::from(OBJ)], &[c.s.dt.0], 10_000);
        only_touched_the_regions(&ee).unwrap_or_else(|e| panic!("state {k}: {e}\n{c:#?}"));
        let was = snap(&ee);

        let mut t = c.clone();
        step::clutch(&t.p, &mut t.r, &mut t.eng, &t.s);
        reset(&mut ee);
        poke_clutch(&mut ee, &t);
        compare(k, path, &was, &snap(&ee), &c);
    }
    enough(&tally, &["rearmed", "left alone", "started", "started, no speed range", "running", "running, no time range"], 100);
}

/// MX vs. ATV's clutch differs from MX Unleashed's where the module says: a launch moves no centre of
/// gravity, and a rearm clears what the last launch started at.
#[test]
fn mxvatv_clutch_moves_no_cofg() {
    let mut g = Gen(0x0d63_0002);
    let mut moved = 0;
    for _ in 0..2_000 {
        let mut c = g.clutch();
        c.s.clutch = F::ZERO;
        c.r.clutch_timer = F::ZERO;
        let mut ch = air_free_chassis();
        let (mut a, mut b) = (c.clone(), c.clone());
        mv::clutch(&a.p, &mut a.r, &mut ch, &mut a.eng, &a.s);
        step::clutch(&b.p, &mut b.r, &mut b.eng, &b.s);
        assert_eq!(a.eng.clutch_torque_scale.0.to_bits(), b.eng.clutch_torque_scale.0.to_bits(), "{c:#?}");
        moved += u32::from(ch.cofg.z() != 0.0);
    }
    assert!(moved > 1_000, "MX Unleashed's clutch moved the centre of gravity on only {moved} launches");
}

fn air_free_chassis() -> mv::Chassis {
    mv::Chassis::default()
}

// --- the preload --------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct PreloadState {
    p: Params,
    air: Air,
    ch: Chassis,
    st: air::Step,
    cue: bool,
    /// The bike's `+0xd20`, the event's fourth argument.
    tag: u32,
}

/// Every tuning zero: the generator sets the ones the preload reads.
/// Every tuning zero: the generator sets the ones the backflip reads.
fn zero_params() -> Params {
    Params {
        airborne_control_moment: Vf::ZERO,
        airborne_damping: F::ZERO,
        airborne_control_force: F::ZERO,
        brake_gyro_torque: F::ZERO,
        unroll_gain: F::ZERO,
        unroll_torque: F::ZERO,
        unyaw_gain: F::ZERO,
        unyaw_torque: F::ZERO,
        control_authority_time: F::ZERO,
        preload_time: F::ZERO,
        max_preload: F::ZERO,
        min_preload_time: F::ZERO,
        max_preload_time: F::ZERO,
        min_dpad_preload_time: F::ZERO,
        max_dpad_preload_time: F::ZERO,
        min_backflip_time: F::ZERO,
        max_backflip_time: F::ZERO,
        min_backflip_torque: F::ZERO,
        max_backflip_torque: F::ZERO,
        backflip_torque_rate_increase: F::ZERO,
        backflip_torque_rate_decrease: F::ZERO,
        front_wreck_speed_limit: F::ZERO,
        back_wreck_speed_limit: F::ZERO,
        side_wreck_speed_limit: F::ZERO,
        racing_down_wreck_speed_limit: F::ZERO,
        freestyle_down_wreck_speed_limit: F::ZERO,
        ground_pitch_down_wreck_angle: F::ZERO,
        ground_pitch_up_wreck_angle: F::ZERO,
        airborne_pitch_down_wreck_angle: F::ZERO,
        airborne_pitch_up_wreck_angle: F::ZERO,
        airborne_roll_wreck_angle: F::ZERO,
        no_hander_wreck_speed_limit: F::ZERO,
        no_hander_max_roll_angle: F::ZERO,
        joust_wreck_height: F::ZERO,
        crossup_wreck: F::ZERO,
        crossup_wreck_time: F::ZERO,
        rotation_speed: None,
    }
}

impl Gen {
    fn preload(&mut self) -> PreloadState {
        let mut s = PreloadState {
            p: zero_params(),
            air: Air::default(),
            ch: Chassis::default(),
            st: air::Step::default(),
            cue: self.b(),
            tag: self.bits(),
        };
        let p = &mut s.p;
        p.preload_time = self.f(0.0, 0.5);
        p.min_preload_time = self.f(0.0, 30.0);
        p.max_preload_time = if self.one_in(6) { p.min_preload_time } else { self.f(10.0, 80.0) };
        p.min_dpad_preload_time = self.f(0.0, 30.0);
        p.max_dpad_preload_time = if self.one_in(6) { p.min_dpad_preload_time } else { self.f(10.0, 80.0) };
        p.max_preload = self.f(0.0, 3000.0);

        let a = &mut s.air;
        a.preload_firing = self.one_in(3);
        a.preload_ready = self.b();
        a.preload_armed = self.b();
        a.preload_timer = if self.one_in(5) { F::ZERO } else { self.f(-0.2, 0.6) };
        a.preload_force = Vf::new(self.f(-5.0, 5.0).0, self.f(0.0, 3000.0).0, self.f(-5.0, 5.0).0, self.f(-2.0, 2.0).0);
        a.on_ground = self.bits() % 4 != 0;
        a.contact_vel = Vf::new(self.f(-5.0, 5.0).0, self.f(-5.0, 5.0).0, self.near(30.0, 20.0, 60.0).0, 0.0);

        let c = &mut s.ch;
        c.vel = Vf::new(self.f(-10.0, 10.0).0, self.near(5.0, 0.0, 15.0).0, self.f(-40.0, 40.0).0, 0.0);
        c.force = Vf::new(self.f(-500.0, 500.0).0, self.f(-500.0, 500.0).0, self.f(-500.0, 500.0).0, self.f(-2.0, 2.0).0);

        let st = &mut s.st;
        st.dt = self.f(0.005, 0.04);
        st.brake = if self.one_in(5) { self.f(0.0, 1.0) } else { F::ZERO };
        st.stick = (F::ZERO, match self.bits() % 4 {
            0 => F(if self.b() { 0.8 } else { -0.8 }),
            _ => self.f(-1.0, 1.0),
        });
        st.stick_hold = self.f(-70.0, 70.0);
        st.dpad_y = self.b();

        // Half the rest are on a lip, stick pushed or pulled past the line: the branches that arm and fire.
        if !s.air.preload_firing && self.b() {
            s.air.on_ground = true;
            s.st.brake = F::ZERO;
            s.ch.vel = Vf::new(s.ch.vel.x(), self.f(5.5, 15.0).0, s.ch.vel.z(), 0.0);
            s.air.contact_vel = Vf::new(s.air.contact_vel.x(), s.air.contact_vel.y(), self.f(30.5, 60.0).0, 0.0);
            s.st.stick.1 = if self.b() { self.f(0.81, 1.0) } else { self.f(-1.0, -0.81) };
        }
        s
    }
}

fn poke_preload(ee: &mut Ee, s: &PreloadState) {
    let (p, a, c, st) = (&s.p, &s.air, &s.ch, &s.st);
    put_f(ee, OBJ + 0xc5c, p.preload_time);
    put_f(ee, OBJ + 0xc64, p.min_preload_time);
    put_f(ee, OBJ + 0xc68, p.max_preload_time);
    put_f(ee, OBJ + 0xc6c, p.max_preload);
    put_f(ee, OBJ + 0x1160, p.min_dpad_preload_time);
    put_f(ee, OBJ + 0x1164, p.max_dpad_preload_time);

    put_b(ee, OBJ + 0xc50, a.preload_ready);
    put_b(ee, OBJ + 0xc54, a.preload_firing);
    put_b(ee, OBJ + 0xc58, a.preload_armed);
    put_f(ee, OBJ + 0xc60, a.preload_timer);
    put_v(ee, OBJ + 0xc70, a.preload_force);
    put_b(ee, OBJ + 0x324, a.on_ground);
    put_v(ee, OBJ + 0xc30, a.contact_vel);
    put_b(ee, OBJ + 0x700, s.cue);
    ee.set_u32(OBJ + 0xd20, s.tag);

    put_v(ee, BODY + 0x80, c.vel);
    put_v(ee, BODY + 0xa0, c.force);

    put_f(ee, OBJ + 0x310, st.brake);
    put_f(ee, OBJ + 0x12ac, st.stick.1);
    put_f(ee, OBJ + 0x12b0, st.stick_hold);
    put_b(ee, OBJ + 0x12a4, st.dpad_y);
}

/// Which way a state went, from the decompile's branches and not the port's.
fn preload_path(s: &PreloadState) -> &'static str {
    let (a, st) = (&s.air, &s.st);
    if a.preload_firing {
        if a.preload_timer - st.dt > F::ZERO { "firing" } else { "fired out" }
    } else if !a.on_ground || s.ch.vel.y() <= 5.0 || a.contact_vel.z() <= 30.0 || st.brake != 0.0 {
        "off the lip"
    } else if st.stick.1 > 0.8 {
        "armed"
    } else if st.stick.1 < -0.8 && a.preload_armed {
        let (from, to) = if st.dpad_y {
            (s.p.min_dpad_preload_time, s.p.max_dpad_preload_time)
        } else {
            (s.p.min_preload_time, s.p.max_preload_time)
        };
        if from == to { "fired, equal times" } else { "fired" }
    } else if a.preload_armed {
        "holding"
    } else {
        "waiting"
    }
}

/// SLUS-21104's preload, the port's [`step::preload`]: 10,000 states over every path, the cue's
/// event post hooked and compared with the port's answer.
#[test]
fn mxvatv_preload_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let posts = Rc::new(RefCell::new(Vec::<[u32; 5]>::new()));
    let log = posts.clone();
    ee.hooks.insert(POST_EVENT, Box::new(move |ee| {
        log.borrow_mut().push([ee.r(5) as u32, ee.r(6) as u32, ee.r(7) as u32, ee.r(8) as u32, ee.r(9) as u32]);
    }));
    let mut g = Gen(0x0d63_0003);
    let mut tally = BTreeMap::new();
    let mut cues = 0;
    for k in 0..10_000 {
        let s = g.preload();
        let path = preload_path(&s);
        *tally.entry(path).or_insert(0u32) += 1;
        reset(&mut ee);
        poke_preload(&mut ee, &s);
        posts.borrow_mut().clear();
        ee.call(PRELOAD, &[u64::from(OBJ)], &[s.st.dt.0], 10_000);
        only_touched_the_regions(&ee).unwrap_or_else(|e| panic!("state {k}: {e}\n{s:#?}"));
        let was = snap(&ee);

        let mut t = s.clone();
        let posted = step::preload(&t.p, &mut t.air, &mut t.ch, &t.st, &mut t.cue);
        reset(&mut ee);
        poke_preload(&mut ee, &t);
        compare(k, path, &was, &snap(&ee), &s);
        let want: Vec<[u32; 5]> = if posted { vec![[OBJ + 0x674, 8, 14, s.tag, 0]] } else { vec![] };
        assert_eq!(*posts.borrow(), want, "state {k} ({path}): the cue's event");
        cues += u32::from(posted);
    }
    enough(&tally, &["firing", "fired out", "off the lip", "armed", "fired", "fired, equal times", "holding", "waiting"], 100);
    assert!(cues >= 500, "only {cues} cue(s) posted");
}

// --- the input limiter (MD63a) ------------------------------------------------------------------------

const INPUT_LIMITER: u32 = 0x004e_3ea0;
/// Where the harness puts the object the limiter's first call reads through the bike's `+0x680`: its
/// `+0x1d0` above zero cuts the gas (the start-gate penalty, [`Controls::gate_cut`]). Read only.
const GATE: u32 = 0x01c0_5000;

#[derive(Clone, Debug)]
struct LimiterState {
    held: Held,
    cut: GasCut,
    c: Controls,
    stick: Stick,
    j: Joystick,
    dt: F,
    inp: LimiterIn,
    /// The `+0x680` object's `+0x1d0`, or no object.
    gate: Option<F>,
}

impl Gen {
    fn lever(&mut self) -> F {
        match self.bits() % 4 {
            0 => F::ZERO,
            1 => F::ONE,
            _ => self.f(0.0, 1.0),
        }
    }
    fn rate(&mut self) -> F {
        self.f(0.5, 20.0)
    }
    fn limiter(&mut self) -> LimiterState {
        let gate = match self.bits() % 4 {
            0 => None,
            1 => Some(if self.b() { F::ZERO } else { self.f(-2.0, 0.0) }),
            2 => Some(self.f(0.01, 2.0)),
            _ => Some(F::ZERO),
        };
        let mut c = Controls { gas: self.lever(), brake: self.lever(), clutch: self.lever(), ..Controls::default() };
        c.hands_off = self.one_in(6);
        c.gas_cut = gate.is_some_and(|v| F::ZERO < v);
        let stick = Stick {
            x: if self.one_in(4) { F(if self.b() { 1.0 } else { -1.0 }) } else { self.f(-1.0, 1.0) },
            y: if self.one_in(4) { F(if self.b() { 1.0 } else { -1.0 }) } else { self.f(-1.0, 1.0) },
            digital: [self.one_in(3), self.one_in(3)],
        };
        let mut held = Held::default();
        held.lean = (self.f(-1.0, 1.0), self.f(-1.0, 1.0));
        held.gas = self.lever();
        held.front_brake = self.lever();
        held.rear_brake = held.front_brake;
        held.clutch = self.lever();
        held.reverse = if self.one_in(4) { F::ONE } else { F::ZERO };
        held.lock.steer = self.one_in(4);
        held.lock.throttle = self.one_in(4);
        held.lock.steer_was = self.one_in(4);
        held.lock.latch = self.one_in(3);
        let cut = GasCut {
            latched: self.b(),
            timer: match self.bits() % 4 {
                0 => F::ZERO,
                1 => self.f(-0.05, 0.0),
                _ => self.f(0.0, 0.2),
            },
        };
        let inp = LimiterIn {
            airborne: self.one_in(3),
            cut_time: if self.b() { F::ZERO } else { self.f(-0.05, 0.2) },
        };
        // Lean rates of zero never move, in both; the levers' are the port's "no limit" (ride.rs's
        // `toward`), which no file the games ship has, so they stay above zero here.
        let j = Joystick {
            x_axis_rate_limit: if self.one_in(10) { F::ZERO } else { self.rate() },
            y_axis_rate_limit: self.rate(),
            x_dpad_rate_limit: self.rate(),
            y_dpad_rate_limit: if self.one_in(10) { F::ZERO } else { self.rate() },
            gas_rate_limit: self.rate(),
            brake_rate_limit: self.rate(),
            clutch_in_rate_limit: self.rate(),
            clutch_out_rate_limit: self.rate(),
        };
        LimiterState { held, cut, c, stick, j, dt: self.f(0.005, 0.04), inp, gate }
    }
}

fn poke_limiter(ee: &mut Ee, s: &LimiterState) {
    let (h, c, j) = (&s.held, &s.c, &s.j);
    put_f(ee, OBJ + 0x314, c.gas);
    put_f(ee, OBJ + 0x310, c.brake);
    put_f(ee, OBJ + 0x12c0, c.clutch);
    put_f(ee, OBJ + 0x12c4, h.reverse);
    put_f(ee, OBJ + 0x318, h.gas);
    put_f(ee, OBJ + 0x12cc, h.front_brake);
    put_f(ee, OBJ + 0x12d0, h.rear_brake);
    put_f(ee, OBJ + 0x12d4, h.clutch);
    put_f(ee, OBJ + 0x12a8, s.stick.x);
    put_f(ee, OBJ + 0x12ac, s.stick.y);
    put_b(ee, OBJ + 0x12a0, s.stick.digital[0]);
    put_b(ee, OBJ + 0x12a4, s.stick.digital[1]);
    put_f(ee, OBJ + 0x12e8, h.lean.0);
    put_f(ee, OBJ + 0x12ec, h.lean.1);
    put_b(ee, OBJ + 0xdd0, !h.lock.steer);
    put_b(ee, OBJ + 0xdd4, !h.lock.throttle);
    put_b(ee, OBJ + 0x1298, !h.lock.steer_was);
    put_b(ee, OBJ + 0x129c, h.lock.latch);
    put_b(ee, OBJ + 0x1764, s.cut.latched);
    put_f(ee, OBJ + 0x1768, s.cut.timer);
    put_f(ee, OBJ + 0x176c, s.inp.cut_time);
    // The rider's stunt and the balance stunt: equal is the gas's zero gate.
    ee.set_u32(OBJ + 0x1524, if c.hands_off { 4 } else { 0xff });
    ee.set_u32(OBJ + 0x152c, 4);
    put_b(ee, OBJ + 0xde0, s.inp.airborne);
    put_f(ee, OBJ + 0x12d8, j.x_axis_rate_limit);
    put_f(ee, OBJ + 0x12dc, j.y_axis_rate_limit);
    put_f(ee, OBJ + 0x12e0, j.x_dpad_rate_limit);
    put_f(ee, OBJ + 0x12e4, j.y_dpad_rate_limit);
    put_f(ee, OBJ + 0x1310, j.gas_rate_limit);
    put_f(ee, OBJ + 0x1314, j.brake_rate_limit);
    put_f(ee, OBJ + 0x1318, j.clutch_in_rate_limit);
    put_f(ee, OBJ + 0x131c, j.clutch_out_rate_limit);
    match s.gate {
        Some(v) => {
            ee.set_u32(OBJ + 0x680, GATE);
            put_f(ee, GATE + 0x1d0, v);
        }
        None => ee.set_u32(OBJ + 0x680, 0),
    }
}

/// Which way the gas went, from the decompile's branches and not the port's.
fn gas_path(s: &LimiterState) -> &'static str {
    let gas = if s.c.gas_cut { F::ZERO } else { s.c.gas };
    if s.held.lock.throttle {
        "throttle off, cut armed"
    } else if s.cut.latched {
        if gas == F::ZERO {
            "cut let go, no gas"
        } else if s.inp.cut_time < s.cut.timer {
            "cut let go, timed out"
        } else {
            "cut holding"
        }
    } else if s.c.hands_off {
        "balance stunt"
    } else if s.held.reverse != F::ZERO && !s.inp.airborne {
        "reverse on the ground"
    } else if s.c.gas_cut {
        "gate cut"
    } else {
        "gas walks"
    }
}

/// SLUS-21104's input limiter, the port's [`step::input_limiter`]: 10,000 states over every branch,
/// the bike's region compared word for word.
#[test]
fn mxvatv_input_limiter_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let mut g = Gen(0x0d63_a001);
    let mut tally = BTreeMap::new();
    for k in 0..10_000 {
        let s = g.limiter();
        let path = gas_path(&s);
        *tally.entry(path).or_insert(0u32) += 1;
        let l = &s.held.lock;
        let latch = if l.latch || (l.steer && !l.steer_was) { "start latch" } else { "no start latch" };
        *tally.entry(latch).or_insert(0u32) += 1;
        reset(&mut ee);
        poke_limiter(&mut ee, &s);
        ee.call(INPUT_LIMITER, &[u64::from(OBJ)], &[s.dt.0], 10_000);
        only_touched_the_regions(&ee).unwrap_or_else(|e| panic!("state {k}: {e}\n{s:#?}"));
        let was = snap(&ee);

        let mut t = s.clone();
        t.c = step::input_limiter(&mut t.held, &mut t.cut, &s.c, s.stick, &s.j, s.dt, &s.inp);
        t.stick = Stick { x: t.held.stick.0, y: t.held.stick.1, digital: t.held.digital };
        // The gate cut is in the limiter's first call, on the gas lever it leaves: what the port
        // answers, so the lever word compares as is.
        reset(&mut ee);
        poke_limiter(&mut ee, &t);
        compare(k, path, &was, &snap(&ee), &s);
    }
    enough(
        &tally,
        &[
            "throttle off, cut armed",
            "cut let go, no gas",
            "cut let go, timed out",
            "cut holding",
            "balance stunt",
            "reverse on the ground",
            "gate cut",
            "gas walks",
            "start latch",
            "no start latch",
        ],
        100,
    );
}

/// MX vs. ATV's limiter leaves the stick as it is, where MX Unleashed's squares it: at 0.85 its lean
/// walks to 0.85, not 0.7225.
#[test]
fn mxvatv_input_limiter_does_not_square_the_stick() {
    let j = Joystick { x_axis_rate_limit: F(1000.0), y_axis_rate_limit: F(1000.0), ..Joystick::default() };
    let c = Controls { stick: (F(0.85), F(-0.5)), ..Controls::default() };
    let stick = game_sim::trick::stick(c.stick.0, c.stick.1, c.dpad);
    let (mut a, mut b) = (Held::default(), Held::default());
    a.step(&c, &j, F(0.03));
    step::input_limiter(&mut b, &mut GasCut::default(), &c, stick, &j, F(0.03), &LimiterIn::default());
    assert_eq!(a.stick, (F(0.85) * F(0.85), F(-0.5) * F(0.5)));
    assert_eq!(b.stick, (F(0.85), F(-0.5)));
    assert_eq!(b.lean, (F(0.85), F(-0.5)));
}

/// The gas cut on a ride's own cut time: with the throttle back on, two frames of no gas, then the
/// lever's walk.
#[test]
fn mxvatv_gas_cut_holds_two_frames() {
    let j = Joystick { gas_rate_limit: F(1000.0), ..Joystick::default() };
    let c = Controls { gas: F::ONE, ..Controls::default() };
    let stick = game_sim::trick::stick(F::ZERO, F::ZERO, c.dpad);
    let inp = LimiterIn { airborne: false, cut_time: step::GAS_CUT_TIME };
    let (mut h, mut cut) = (Held::default(), GasCut::default());
    h.lock.throttle = true;
    step::input_limiter(&mut h, &mut cut, &c, stick, &j, F(0.03), &inp);
    assert!(cut.latched && h.gas == F::ZERO);
    h.lock.throttle = false;
    let gas: Vec<F> = (0..3).map(|_| {
        step::input_limiter(&mut h, &mut cut, &c, stick, &j, F(0.03), &inp);
        h.gas
    }).collect();
    assert_eq!(gas, [F::ZERO, F::ZERO, F::ONE]);
}

// --- the steer ramp (MD63a) ---------------------------------------------------------------------------

const GROUND_HEAD: u32 = 0x004d_8b20;
const FRONT: u32 = 0x01c0_3000;
const REAR: u32 = 0x01c0_4000;
/// The vehicle state the ground head checks first (`+0xbc4`; its `+0x28` 3 or 4 returns at once).
const VEHICLE_STATE: u32 = 0x01c0_5800;
const AUTO_SHIFT: u32 = 0x004e_5fe0;
const HEAD_TAIL: u32 = 0x004e_4190;

#[derive(Clone, Debug)]
struct RampState {
    ramp: SteerRamp,
    p: mv::Params,
    lock: F,
    lean: F,
    speed: F,
    timer: F,
    dt: F,
}

impl Gen {
    fn ramp(&mut self) -> RampState {
        let mut p = mv::Params::default();
        p.steer_speed_1 = self.f(0.0, 30.0);
        p.steer_speed_2 = if self.one_in(5) { p.steer_speed_1 } else { self.f(20.0, 180.0) };
        p.steer_scale_1 = self.f(0.2, 1.2);
        p.steer_scale_2 = self.f(0.1, 1.0);
        let lean = match self.bits() % 5 {
            0 => F(if self.b() { 0.987 } else { -0.987 }),
            1 | 2 => if self.b() { self.f(0.987, 1.0) } else { self.f(-1.0, -0.987) },
            _ => self.f(-1.0, 1.0),
        };
        RampState {
            ramp: SteerRamp {
                on: !self.one_in(4),
                amount: self.f(0.0, 0.5),
                time: if self.one_in(10) { self.f(0.0, 0.05) } else { self.f(0.1, 1.5) },
            },
            p,
            lock: self.f(0.2, 0.8),
            lean,
            speed: self.f(-10.0, 200.0),
            timer: match self.bits() % 4 {
                0 => F::ZERO,
                1 => self.f(1.0, 3.0),
                _ => self.f(0.0, 1.0),
            },
            dt: self.f(0.005, 0.04),
        }
    }
}

fn poke_ramp(ee: &mut Ee, s: &RampState) {
    ee.set_u32(OBJ + 0xbc4, VEHICLE_STATE);
    ee.set_u32(VEHICLE_STATE + 0x28, 0);
    ee.set_u32(OBJ + 0x17b0, FRONT);
    ee.set_u32(OBJ + 0x17b4, REAR);
    // On the ground, steering, not balancing: the command's branch.
    put_b(ee, OBJ + 0x324, true);
    ee.set_u32(OBJ + 0x1524, 0xff);
    ee.set_u32(OBJ + 0x152c, 4);
    put_b(ee, OBJ + 0xc80, false);
    put_f(ee, OBJ + 0x1300, s.p.steer_speed_1);
    put_f(ee, OBJ + 0x1304, s.p.steer_scale_1);
    put_f(ee, OBJ + 0x1308, s.p.steer_speed_2);
    put_f(ee, OBJ + 0x130c, s.p.steer_scale_2);
    put_f(ee, OBJ + 0xc38, s.speed);
    put_f(ee, FRONT + 0x70, s.lock);
    put_f(ee, OBJ + 0x12e8, s.lean);
    put_b(ee, OBJ + 0x12f0, s.ramp.on);
    put_f(ee, OBJ + 0x12f4, s.ramp.amount);
    put_f(ee, OBJ + 0x12f8, s.ramp.time);
    put_f(ee, OBJ + 0x12fc, s.timer);
}

/// SLUS-21104's ground head, run whole with its calls hooked, against the port's
/// [`step::steer_command`]: the command it leaves on the bike (`+0x12c8`) and the front wheel
/// (`+0x8ac`), and the ramp's timer, over 10,000 states.
#[test]
fn mxvatv_steer_ramp_is_the_originals() {
    let Some(mut ee) = image() else { return };
    assert_eq!(ee.u32_at(BIKE_VTABLE + 0x34c), INPUT_LIMITER, "the bike's input-limiter slot");
    assert_eq!(ee.u32_at(BIKE_VTABLE + 0x4d0), HEAD_TAIL, "the bike's head-tail slot");
    let body_call = ee.u32_at(BODY_VTABLE + 0x64);
    for at in [INPUT_LIMITER, HEAD_TAIL, AUTO_SHIFT, CLUTCH, body_call] {
        ee.hooks.insert(at, Box::new(|_| {}));
    }
    let mut g = Gen(0x0d63_a002);
    let mut tally = BTreeMap::new();
    for k in 0..10_000 {
        let s = g.ramp();
        let path = if !s.ramp.on {
            "no ramp"
        } else if s.lean.abs() < F(0.987) {
            "ramp, let go"
        } else if s.timer + s.dt < s.ramp.time {
            "ramp, coming in"
        } else {
            "ramp, full"
        };
        *tally.entry(path).or_insert(0u32) += 1;
        if s.p.steer_speed_1 == s.p.steer_speed_2 {
            *tally.entry("flat fade").or_insert(0u32) += 1;
        }
        reset(&mut ee);
        for o in (0..0x900).step_by(4) {
            ee.set_u32(FRONT + o, 0);
            ee.set_u32(REAR + o, 0);
        }
        poke_ramp(&mut ee, &s);
        ee.call(GROUND_HEAD, &[u64::from(OBJ)], &[s.dt.0], 50_000);
        let was = [ee.u32_at(OBJ + 0x12c8), ee.u32_at(FRONT + 0x8ac), ee.u32_at(OBJ + 0x12fc)];

        let mut timer = s.timer;
        let fade = mv::steer_scale(&s.p, s.speed);
        let cmd = step::steer_command(&s.ramp, s.lock, s.lean, fade, &mut timer, s.dt);
        let now = [cmd.0.to_bits(), cmd.0.to_bits(), timer.0.to_bits()];
        assert_eq!(
            was.map(f32::from_bits),
            now.map(f32::from_bits),
            "state {k} ({path}): [+0x12c8, front +0x8ac, +0x12fc]\n{s:#?}"
        );
        assert_eq!(was, now, "state {k} ({path}): the bits\n{s:#?}");
    }
    enough(&tally, &["no ramp", "ramp, let go", "ramp, coming in", "ramp, full", "flat fade"], 100);
}

/// The file's two keys as the reader makes them: the amount from degrees with the reader's own
/// constant, the option off.
#[test]
fn mxvatv_steer_ramp_reads_the_file() {
    let r = SteerRamp::of_file(F(12.0), F(0.6));
    assert_eq!(r.amount.0.to_bits(), (F(12.0) * F(0.01745329)).0.to_bits());
    assert_eq!(r.time, F(0.6));
    assert!(!r.on);
}

// --- the rider's controllers (MD63b) -----------------------------------------------------------------

const STAND_UPRIGHT: u32 = 0x004d_c970;
const BALANCE: u32 = 0x004d_c240;
const ROLL_PITCH: u32 = 0x004d_c4f0;
/// The engine's smooth noise, which only the balance asks for.
const SMOOTH_NOISE: u32 = 0x0013_4580;
/// The physics world the bike's `+0xe8` names; only its gravity (`+0x40`) is read.
const WORLD: u32 = 0x01c0_5a00;
/// The arc cosine table's pointer and scale, which the balance reads.
const ACOS_TABLE: u32 = 0x003e_a9c0;
const ACOS_SCALE: u32 = 0x003e_a9d8;

/// A stand-in for the smooth noise, both sides alike: `tests/move.rs`' table, reaching `±1` so the
/// command meets its clamp.
fn fake_noise(x: f32) -> f32 {
    const WAVE: [f32; 8] = [0.0, 1.0, -1.0, 0.5, -0.25, 0.937_5, -0.812_5, 0.125];
    WAVE[(x.to_bits() >> 3) as usize & 7]
}

/// The port's side of the calls: only the noise reaches it (the frames come off the quaternion).
struct Noise;

impl mv::Outside for Noise {
    fn unrotate(&mut self, _: mv::Frame, _: Vf) -> Vf {
        unreachable!("MX vs. ATV's controllers rotate by the body's quaternion")
    }
    fn forward(&mut self, _: mv::Frame) -> Vf {
        unreachable!("MX vs. ATV's controllers take the forward axis from the body's quaternion")
    }
    fn noise(&mut self, t: F) -> F {
        F(fake_noise(t.0))
    }
    fn set_axes(&mut self, _: Vf, _: Vf) {
        unreachable!("no controller here points the node")
    }
}

#[derive(Clone, Debug, Default)]
struct RiderState {
    p: mv::Params,
    r: mv::Ride,
    ch: mv::Chassis,
    front: mv::Wheel,
    rear: mv::Wheel,
    eng: mv::Engine,
    s: mv::Step,
    q: Vf,
}

#[allow(clippy::float_arithmetic)]
impl Gen {
    fn vec(&mut self, lo: f32, hi: f32) -> Vf {
        Vf::new(self.f(lo, hi).0, self.f(lo, hi).0, self.f(lo, hi).0, self.f(-2.0, 2.0).0)
    }
    /// A unit vector, mostly up: a ground normal.
    fn normal(&mut self) -> Vf {
        let (x, y, z) = (self.f(-0.7, 0.7).0, self.f(0.2, 1.0).0, self.f(-0.7, 0.7).0);
        let k = 1.0 / (x * x + y * y + z * z).sqrt();
        Vf::new(x * k, y * k, z * k, self.f(-1.0, 1.0).0)
    }
    fn quat(&mut self) -> Vf {
        let v: [f32; 4] = std::array::from_fn(|_| self.f(-1.0, 1.0).0);
        let k = 1.0 / v.iter().map(|a| a * a).sum::<f32>().sqrt();
        Vf::new(v[0] * k, v[1] * k, v[2] * k, v[3] * k)
    }

    fn rider(&mut self) -> RiderState {
        let mut t = RiderState::default();
        let p = &mut t.p;
        p.gravity = self.vec(-500.0, 500.0);
        p.balance_inertia_rate = self.f(0.0, 8.0);
        p.min_balance_inertia = self.f(0.0, 1.0);
        p.max_balance_inertia = if self.one_in(6) { p.min_balance_inertia } else { self.f(1.0, 4.0) };
        p.input_inertia = self.f(0.0, 5.0);
        p.max_balance_point = self.f(0.0, 1.5);
        p.balance_noise_amplitude = if self.one_in(5) { F::ZERO } else { self.f(0.0, 2.0) };
        p.balance_noise_time_scale = self.f(0.0, 4.0);
        p.max_roll_rate_disturbance = self.f(0.0, 4.0);
        p.roll_control_gain = self.f(0.0, 40.0);
        p.roll_control_rate_gain = if self.one_in(3) { self.f(0.0, 2.0) } else { self.f(0.0, 0.02) };
        p.roll_control_max_cg = self.f(0.0, 0.6);
        p.max_pitch_command = self.f(0.0, 0.9);
        p.pitch_gain = self.f(0.0, 60.0);
        p.pitch_rate_gain = self.f(0.0, 200.0);
        p.inv_wheelie_top_speed = self.f(0.02, 0.2);
        p.max_stoppie_pitch_command = self.f(0.0, 0.9);
        p.stoppie_pitch_gain = self.f(0.0, 60.0);
        p.stoppie_yaw_rate_scale = self.f(0.0, 4.0);
        p.stoppie_top_speed = if self.one_in(5) { F::ZERO } else { self.f(0.0, 40.0) };

        let r = &mut t.r;
        r.balance_inertia = self.f(-1.0, 5.0);
        r.balance_command = self.f(-1.2, 1.2);
        r.balance_point = self.f(-2.0, 2.0);
        r.balance_noise_time = self.f(0.0, 30.0);
        r.pitch_integral = self.f(-50.0, 50.0);
        r.stoppie = self.b();
        r.clutch_in = if self.one_in(4) { self.f(0.0, 1.0) } else { F::ZERO };

        let c = &mut t.ch;
        c.force = self.vec(-500.0, 500.0);
        c.torque = self.vec(-500.0, 500.0);
        c.cofg = self.vec(-1.0, 1.0);
        c.euler = self.vec(-1.0, 1.0);
        c.world_ang_vel = self.vec(-5.0, 5.0);
        c.ang_vel = self.vec(-5.0, 5.0);

        for w in [&mut t.front, &mut t.rear] {
            w.contact = self.bits() % 4 != 0;
            w.contact_normal = self.normal();
            w.steered = self.b();
        }

        let s = &mut t.s;
        s.on_ground = self.bits() % 8 != 0;
        s.both_wheels = self.bits() % 4 != 0;
        s.balancing = self.one_in(4);
        s.gas = if self.one_in(4) { self.f(0.0, 1.0) } else { F::ZERO };
        s.brake = if self.b() { self.f(0.0, 1.0) } else { F::ZERO };
        s.speed = match self.bits() % 6 {
            0 => F(if self.b() { 5.0 } else { 15.0 }),
            1 => F(if self.b() { 7.0 } else { -10.0 }),
            2 => self.f(-12.0, 12.0),
            _ => self.f(-10.0, 50.0),
        };
        s.stick_x = self.f(-1.0, 1.0);
        s.lean = self.f(-1.0, 1.0);
        s.lean_fore_aft = match self.bits() % 7 {
            // A full forward lean is the only one that starts a stoppie.
            5 | 6 => F(-1.0),
            0 => F(*[0.5, -1.0, -0.1].get((self.bits() % 3) as usize).unwrap()),
            1 => self.f(0.5, 1.0),
            2 => self.f(-1.0, -0.1),
            _ => self.f(-1.0, 1.0),
        };
        s.gas_rate_limited = if self.one_in(4) { F::ZERO } else { self.f(0.0, 1.0) };
        s.rear_brake = if self.one_in(4) { self.f(0.0, 1.0) } else { F::ZERO };
        s.front_brake = if self.b() { self.f(0.0, 1.0) } else { F::ZERO };
        s.dt = self.f(0.005, 0.02);
        t.q = self.quat();
        t
    }
}

fn poke_rider(ee: &mut Ee, t: &RiderState) {
    let (p, r, c, s) = (&t.p, &t.r, &t.ch, &t.s);
    ee.set_u32(OBJ + 0xe8, WORLD);
    put_v(ee, WORLD + 0x40, p.gravity);
    ee.set_u32(OBJ + 0x17b0, FRONT);
    ee.set_u32(OBJ + 0x17b4, REAR);
    for (w, at) in [(&t.front, FRONT), (&t.rear, REAR)] {
        put_b(ee, at + 0xdc, w.contact);
        put_v(ee, at + 0x130, w.contact_normal);
        put_b(ee, at + 0x2c, w.steered);
    }

    put_f(ee, OBJ + 0x14fc, p.balance_inertia_rate);
    put_f(ee, OBJ + 0x1500, p.min_balance_inertia);
    put_f(ee, OBJ + 0x1504, p.max_balance_inertia);
    put_f(ee, OBJ + 0x1508, p.input_inertia);
    put_f(ee, OBJ + 0x1514, p.balance_noise_amplitude);
    put_f(ee, OBJ + 0x151c, p.balance_noise_time_scale);
    put_f(ee, OBJ + 0x1520, p.max_balance_point);
    put_f(ee, OBJ + 0x1398, p.max_roll_rate_disturbance);
    put_f(ee, OBJ + 0x138c, p.roll_control_gain);
    put_f(ee, OBJ + 0x1390, p.roll_control_rate_gain);
    put_f(ee, OBJ + 0x1394, p.roll_control_max_cg);
    put_f(ee, OBJ + 0x139c, p.max_pitch_command);
    put_f(ee, OBJ + 0x13a0, p.pitch_gain);
    put_f(ee, OBJ + 0x13a4, p.pitch_rate_gain);
    put_f(ee, OBJ + 0x13a8, p.inv_wheelie_top_speed);
    put_f(ee, OBJ + 0x13cc, p.max_stoppie_pitch_command);
    put_f(ee, OBJ + 0x13d0, p.stoppie_pitch_gain);
    put_f(ee, OBJ + 0x13d4, p.stoppie_yaw_rate_scale);
    put_f(ee, OBJ + 0x13d8, p.stoppie_top_speed);

    put_f(ee, OBJ + 0x14f8, r.balance_inertia);
    put_f(ee, OBJ + 0x150c, r.balance_command);
    put_f(ee, OBJ + 0x1510, r.balance_point);
    put_f(ee, OBJ + 0x1518, r.balance_noise_time);
    put_f(ee, OBJ + 0x13ac, r.pitch_integral);
    put_b(ee, OBJ + 0x13b0, r.stoppie);
    put_f(ee, OBJ + 0x12d4, r.clutch_in);

    put_v(ee, BODY + 0xa0, c.force);
    put_v(ee, BODY + 0x1b0, c.torque);
    put_v(ee, BODY + 0x1a0, c.cofg);
    put_v(ee, BODY + 0x1d0, c.ang_vel);
    put_v(ee, BODY + 0x190, t.q);
    put_v(ee, OBJ + 0x360, c.euler);
    put_v(ee, OBJ + 0x340, c.world_ang_vel);

    put_b(ee, OBJ + 0x324, s.on_ground);
    put_b(ee, OBJ + 0x1270, s.both_wheels);
    // The rider's stunt against the balance stunt: equal is balancing.
    ee.set_u32(OBJ + 0x1524, if s.balancing { 4 } else { 0xff });
    ee.set_u32(OBJ + 0x152c, 4);
    put_f(ee, OBJ + 0x314, s.gas);
    put_f(ee, OBJ + 0x310, s.brake);
    put_f(ee, OBJ + 0xc38, s.speed);
    put_f(ee, OBJ + 0x12a8, s.stick_x);
    put_f(ee, OBJ + 0x12e8, s.lean);
    put_f(ee, OBJ + 0x12ec, s.lean_fore_aft);
    put_f(ee, OBJ + 0x318, s.gas_rate_limited);
    put_f(ee, OBJ + 0x12cc, s.front_brake);
    put_f(ee, OBJ + 0x12d0, s.rear_brake);
}

/// The image with the noise hooked, its arc cosine table checked against the port's, and the wheels'
/// words zeroed with the rest before each state.
fn rider_image() -> Option<(Ee, air::AcosTable)> {
    let mut ee = image()?;
    ee.hooks.insert(SMOOTH_NOISE, Box::new(|ee| {
        let x = ee.f(12);
        ee.set_f(0, fake_noise(x));
    }));
    let table = air::AcosTable::build();
    assert_eq!(ee.f32_at(ACOS_SCALE), 14999.5, "the arc cosine's scale");
    let at = ee.u32_at(ACOS_TABLE);
    for (i, e) in table.entries().iter().enumerate() {
        assert_eq!(ee.u32_at(at + 4 * i as u32), e.to_bits(), "arc cosine entry {i}");
    }
    Some((ee, table))
}

fn rider_reset(ee: &mut Ee) {
    reset(ee);
    for o in (0..0x200).step_by(4) {
        ee.set_u32(FRONT + o, 0);
        ee.set_u32(REAR + o, 0);
    }
}

/// The bike, its body and engine, and both wheels' first 0x200 bytes.
fn rider_snap(ee: &Ee) -> Vec<u32> {
    let mut v = snap(ee);
    v.extend((0..0x200).step_by(4).map(|o| ee.u32_at(FRONT + o)));
    v.extend((0..0x200).step_by(4).map(|o| ee.u32_at(REAR + o)));
    v
}

fn rider_touched_only_its_own(ee: &Ee) -> Result<(), String> {
    let frame = (ee.stack - 0x2000, ee.stack + 0x40);
    let wheels = [(FRONT, 0x200), (REAR, 0x200)];
    for (a, b) in ee.written_ranges() {
        let inside = REGIONS.iter().map(|&(_, base, len)| (base, len)).chain(wheels).any(|(base, len)| a >= base && b <= base + len)
            || (a >= frame.0 && b <= frame.1);
        if !inside {
            return Err(format!("the original wrote {a:#010x}..{b:#010x}, which is not the bike or the stack"));
        }
    }
    Ok(())
}

/// Run one controller both ways over 10,000 states and compare everything it can write.
fn rider_oracle(
    seed: u64,
    at: u32,
    path: impl Fn(&RiderState) -> &'static str,
    port: impl Fn(&mut RiderState, &air::AcosTable),
) -> BTreeMap<&'static str, u32> {
    let mut tally = BTreeMap::new();
    let Some((mut ee, table)) = rider_image() else { return tally };
    let mut g = Gen(seed);
    for k in 0..10_000 {
        let s = g.rider();
        let way = path(&s);
        *tally.entry(way).or_insert(0u32) += 1;
        rider_reset(&mut ee);
        poke_rider(&mut ee, &s);
        ee.call(at, &[u64::from(OBJ)], &[s.s.dt.0], 50_000);
        rider_touched_only_its_own(&ee).unwrap_or_else(|e| panic!("state {k}: {e}\n{s:#?}"));
        let was = rider_snap(&ee);

        let mut t = s.clone();
        port(&mut t, &table);
        rider_reset(&mut ee);
        poke_rider(&mut ee, &t);
        let now = rider_snap(&ee);
        if was != now {
            let i = was.iter().zip(&now).position(|(a, b)| a != b).unwrap();
            let n = snap(&ee).len();
            let place = if i < n {
                where_is(i)
            } else if i < n + 0x80 {
                format!("front + {:#x}", (i - n) * 4)
            } else {
                format!("rear + {:#x}", (i - n - 0x80) * 4)
            };
            panic!(
                "state {k} ({way}): {place} is {:#010x} ({}) in the original and {:#010x} ({}) in the port\n{s:#?}",
                was[i],
                f32::from_bits(was[i]),
                now[i],
                f32::from_bits(now[i]),
            );
        }
    }
    tally
}

macro_rules! rider_bike {
    ($t:expr) => {
        mv::Bike { ride: &mut $t.r, chassis: &mut $t.ch, front: &mut $t.front, rear: &mut $t.rear, engine: &mut $t.eng }
    };
}

/// SLUS-21104's low-speed upright controller, the port's [`step::stand_upright`]: 10,000 states, the
/// forward axis taken out of the body's quaternion by the original's own code and the force added
/// through the body's own add-force.
#[test]
#[allow(clippy::float_arithmetic)]
fn mxvatv_stand_upright_is_the_originals() {
    let tally = rider_oracle(
        0x0d63_b001,
        STAND_UPRIGHT,
        |t| {
            let s = &t.s;
            if !s.both_wheels || s.balancing || s.gas != 0.0 || !(s.speed.abs() < 10.0) {
                "left alone"
            } else if s.brake != 0.0 {
                "held, braked"
            } else {
                "held"
            }
        },
        |t, _| {
            let (s, q) = (t.s.clone(), t.q);
            step::stand_upright(&t.p.clone(), &mut rider_bike!(t), &s, &mut Noise, q);
        },
    );
    if tally.is_empty() {
        return;
    }
    enough(&tally, &["left alone", "held", "held, braked"], 500);
}

/// SLUS-21104's balance, the port's [`step::balance`]: 10,000 states, the noise hooked on both
/// sides alike and the arc cosine read from the image's own table.
#[test]
#[allow(clippy::float_arithmetic)]
fn mxvatv_balance_is_the_originals() {
    let tally = rider_oracle(
        0x0d63_b002,
        BALANCE,
        |t| {
            if !t.front.contact {
                return "no front";
            }
            // The roll angle's sign is the normal's x in the bike: the one branch inside.
            let local = game_sim::quad::Body { orientation: t.q, ..Default::default() }.to_local(t.front.contact_normal);
            if local.x() < 0.0 { "balanced, ground to one side" } else { "balanced, to the other" }
        },
        |t, table| {
            let (s, q) = (t.s.clone(), t.q);
            let curve = game_sim::camera::CurveTable::build();
            let tables = mv::Tables { acos: table, curve: &curve };
            step::balance(&t.p.clone(), &mut rider_bike!(t), &s, &tables, &mut Noise, q, s.dt);
        },
    );
    if tally.is_empty() {
        return;
    }
    enough(&tally, &["no front", "balanced, ground to one side", "balanced, to the other"], 500);
}

/// Which way the roll/pitch goes, from the listing's branches.
#[allow(clippy::float_arithmetic)]
fn roll_pitch_path(t: &RiderState) -> &'static str {
    let s = &t.s;
    if !s.on_ground {
        return "airborne";
    }
    let wheelie = t.rear.contact
        && s.gas_rate_limited > 0.0
        && s.speed > 5.0
        && s.rear_brake == 0.0
        && t.r.clutch_in == 0.0
        && s.lean_fore_aft >= 0.5;
    let starts = t.front.contact && !t.r.stoppie && s.front_brake > 0.0 && s.speed > 15.0 && s.lean_fore_aft <= -1.0;
    match (wheelie, t.r.stoppie || starts) {
        (true, _) => "wheelie",
        (false, false) => "neither",
        (false, true) if starts => "stoppie starts",
        (false, true) if t.p.stoppie_top_speed == 0.0 => "stoppie, no top speed",
        (false, true) => "stoppie held",
    }
}

/// SLUS-21104's wheelie and stoppie, the port's [`step::roll_pitch`]: 10,000 states, the moments put
/// through the body's own add-moment and world-moment calls.
#[test]
fn mxvatv_roll_pitch_is_the_originals() {
    let tally = rider_oracle(0x0d63_b003, ROLL_PITCH, roll_pitch_path, |t, _| {
        let (s, q) = (t.s.clone(), t.q);
        step::roll_pitch(&t.p.clone(), &mut rider_bike!(t), &s, q, s.dt);
    });
    if tally.is_empty() {
        return;
    }
    enough(&tally, &["airborne", "wheelie", "neither", "stoppie starts", "stoppie held", "stoppie, no top speed"], 100);
}
