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
        // The rear off the front half the time: the original copies the front over it (B17).
        held.rear_brake = if self.b() { held.front_brake } else { self.lever() };
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
        let rear = if s.held.rear_brake == s.held.front_brake { "rear on the front" } else { "rear off the front" };
        *tally.entry(rear).or_insert(0u32) += 1;
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
            "rear on the front",
            "rear off the front",
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

// --- MD63c: the steering hinge -------------------------------------------------------------------------

/// SLUS-21104's wheel integrate (its dispatch, which calls the steering hinge on a
/// balanced wheel), and the surface drag and the spin it calls first.
const WHEEL_INTEGRATE: u32 = 0x004a_1680;
const SURFACE_DRAG: u32 = 0x004a_1ec0;
const SPIN_INTEGRATE: u32 = 0x004a_2040;
/// How much of a wheel the steering touches: up to its angle and rate at `+0x8ac`/`+0x8b0`.
const WHEEL_LEN: u32 = 0x900;

/// A zero with a random sign, or a value from `lo..hi`.
#[allow(clippy::float_arithmetic)]
impl Gen {
    fn or_zero(&mut self, lo: f32, hi: f32) -> F {
        if self.one_in(4) { F(if self.b() { -0.0 } else { 0.0 }) } else { self.f(lo, hi) }
    }
    fn hinge(&mut self) -> (game_sim::wheel::Wheel, F) {
        let mut w = game_sim::wheel::Wheel {
            steered: !self.one_in(8),
            balancing: !self.one_in(4),
            steer_torque: self.or_zero(-60.0, 60.0),
            steer_damping: self.or_zero(0.0, 12.0),
            steer_inv_inertia: self.or_zero(0.0, 6.0),
            max_steer_angle: self.f(0.2, 0.7),
            min_steer_angle: -self.f(0.2, 0.7),
            steer_angle: self.or_zero(-0.75, 0.75),
            steer_rate: self.or_zero(-12.0, 12.0),
            ..Default::default()
        };
        if self.one_in(6) {
            w.steer_angle = if self.b() { w.max_steer_angle } else { w.min_steer_angle };
        }
        let dt = if self.one_in(10) { F::ZERO } else { self.f(0.002, 0.02) };
        (w, dt)
    }
}

fn poke_hinge(ee: &mut Ee, w: &game_sim::wheel::Wheel) {
    put_b(ee, FRONT + 0x2c, w.steered);
    put_b(ee, FRONT + 0x30, w.balancing);
    put_f(ee, FRONT + 0x60, w.steer_torque);
    put_f(ee, FRONT + 0x64, w.steer_damping);
    put_f(ee, FRONT + 0x6c, w.steer_inv_inertia);
    put_f(ee, FRONT + 0x70, w.max_steer_angle);
    put_f(ee, FRONT + 0x74, w.min_steer_angle);
    put_f(ee, FRONT + 0x8ac, w.steer_angle);
    put_f(ee, FRONT + 0x8b0, w.steer_rate);
}

fn hinge_snap(ee: &Ee) -> Vec<u32> {
    (0..WHEEL_LEN).step_by(4).map(|o| ee.u32_at(FRONT + o)).collect()
}

fn hinge_reset(ee: &mut Ee) {
    for o in (0..WHEEL_LEN).step_by(4) {
        ee.set_u32(FRONT + o, 0);
    }
}

fn hinge_path(w: &game_sim::wheel::Wheel, after: &game_sim::wheel::Wheel) -> &'static str {
    if !w.steered {
        "not steered"
    } else if !w.balancing {
        "held"
    } else if after.steer_angle == after.max_steer_angle && after.steer_rate == F::ZERO {
        "top of the lock"
    } else if after.steer_angle == after.min_steer_angle && after.steer_rate == F::ZERO {
        "bottom of the lock"
    } else {
        "free"
    }
}

/// SLUS-21104's wheel integrate on its steering, the port's [`step::wheel_integrate`] and
/// [`step::steer_integrate`]: 10,000 wheels (a quarter of each input a signed zero), the surface drag
/// and the spin hooked and logged, the wheel compared word for word.
#[test]
fn mxvatv_steer_integrate_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let log = Rc::new(RefCell::new(Vec::new()));
    for at in [SURFACE_DRAG, SPIN_INTEGRATE] {
        let log = log.clone();
        ee.hooks.insert(at, Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, FRONT, "the wheel");
            log.borrow_mut().push((at, ee.f(12)));
        }));
    }
    let mut g = Gen(0x0d63_c001);
    let mut tally = BTreeMap::new();
    let (mut signs, mut mxu_differs) = (0, 0);
    for k in 0..10_000 {
        let (w, dt) = g.hinge();
        hinge_reset(&mut ee);
        poke_hinge(&mut ee, &w);
        log.borrow_mut().clear();
        ee.call(WHEEL_INTEGRATE, &[u64::from(FRONT)], &[dt.0], 10_000);
        for (a, b) in ee.written_ranges() {
            let stack = a >= ee.stack - 0x100 && b <= ee.stack + 0x40;
            assert!(stack || (a >= FRONT && b <= FRONT + WHEEL_LEN), "state {k}: the original wrote {a:#010x}..{b:#010x}");
        }
        let was = hinge_snap(&ee);

        let mut port = w;
        let calls = step::wheel_integrate(&mut port, dt);
        let want: Vec<(u32, f32)> = calls
            .iter()
            .map(|c| match c {
                game_sim::wheel::Call::SurfaceDrag(t) => (SURFACE_DRAG, t.0),
                game_sim::wheel::Call::SpinIntegrate(t) => (SPIN_INTEGRATE, t.0),
                c => panic!("{c:?}"),
            })
            .collect();
        let got: Vec<(u32, u32)> = log.borrow().iter().map(|&(a, t)| (a, t.to_bits())).collect();
        assert_eq!(got, want.iter().map(|&(a, t)| (a, t.to_bits())).collect::<Vec<_>>(), "state {k}: the calls");
        hinge_reset(&mut ee);
        poke_hinge(&mut ee, &port);
        let now = hinge_snap(&ee);
        let path = hinge_path(&w, &port);
        if was != now {
            let i = was.iter().zip(&now).position(|(a, b)| a != b).unwrap();
            panic!(
                "state {k} ({path}): wheel + {:#x} is {:#010x} in the original and {:#010x} in the port\n{w:#?}",
                4 * i,
                was[i],
                now[i]
            );
        }
        *tally.entry(path).or_insert(0u32) += 1;
        if w.steered && w.balancing {
            let mut mxu = w;
            game_sim::wheel::integrate(&mut mxu, dt);
            let bits = |w: &game_sim::wheel::Wheel| [w.steer_angle.0.to_bits(), w.steer_rate.0.to_bits(), w.steer_torque.0.to_bits()];
            if bits(&mxu) != bits(&port) {
                mxu_differs += 1;
            }
            if [port.steer_angle, port.steer_rate].iter().any(|v| v.0 == 0.0) {
                signs += 1;
            }
        }
    }
    enough(&tally, &["not steered", "held", "top of the lock", "bottom of the lock", "free"], 500);
    assert!(signs >= 300, "only {signs} balanced wheels came out with a zero angle or rate");
    // MX Unleashed's hinge keeps a −0 where the original's `0 + x` makes it +0: the oracle has to see it.
    assert!(mxu_differs >= 100, "only {mxu_differs} wheels tell MX Unleashed's hinge from MX vs. ATV's");
}

// --- MD63c: the wreck test -----------------------------------------------------------------------------

/// SLUS-21104's wreck test (the bike's update calls it last, `f12` the frame's time), and what the
/// harness lays out for it: the driver (the AI's `+0xe4`), the collision record and the empty one the
/// wreck entry swaps in, the record's contact list, its object, and the other riders' bikes.
const WRECK_TEST: u32 = 0x004e_24a0;
const DRIVER: u32 = 0x01c0_6000;
const RECORD: u32 = 0x01c0_6100;
const WRECK_RECORD: u32 = 0x01c0_6400;
const CONTACTS: u32 = 0x01c0_6600;
const STRUCK: u32 = 0x01c0_6c00;
const OTHERS: u32 = 0x01c0_7000;
/// The class word reason 15 masks the record's object with, and the "wrecks on" word.
const STRUCK_MASK: u32 = 0x0054_2db0;
const WRECKS_ON: u32 = 0x0052_5350;

#[derive(Clone, Debug)]
struct WreckState {
    p: Params,
    air: Air,
    chassis: Chassis,
    step: air::Step,
    loads: air::Loads,
    w: step::WreckIn,
    /// The rider's stunt id, the balance stunt's, the one wanted.
    id: i32,
    balance: i32,
    wanted: i32,
    /// The record's object and its class bits (0: none).
    struck: u32,
    small: bool,
    height: F,
    contacts: Vec<air::Contact>,
}

#[allow(clippy::float_arithmetic)]
impl Gen {
    fn wreck(&mut self) -> WreckState {
        let mut p = zero_params();
        // About the Free Ride image's limits, sometimes off them.
        p.front_wreck_speed_limit = self.near(18.333, 10.0, 30.0);
        p.back_wreck_speed_limit = self.near(22.0, 10.0, 30.0);
        p.side_wreck_speed_limit = self.near(19.067, 10.0, 30.0);
        p.racing_down_wreck_speed_limit = self.near(70.4, 40.0, 90.0);
        p.freestyle_down_wreck_speed_limit = self.near(70.4, 40.0, 90.0);
        p.ground_pitch_down_wreck_angle = self.near(1.309, 0.8, 1.6);
        p.ground_pitch_up_wreck_angle = self.near(1.309, 0.8, 1.6);
        p.airborne_pitch_down_wreck_angle = self.near(1.484, 1.0, 1.7);
        p.airborne_pitch_up_wreck_angle = self.near(1.553, 1.0, 1.7);
        p.airborne_roll_wreck_angle = self.near(1.553, 1.0, 1.7);
        p.no_hander_wreck_speed_limit = self.near(29.333, 20.0, 40.0);
        p.no_hander_max_roll_angle = self.near(0.5, 0.2, 0.9);
        p.joust_wreck_height = self.near(2.0, 0.5, 3.0);
        p.crossup_wreck = self.near(0.99, 0.8, 1.0);
        p.crossup_wreck_time = self.near(0.75, 0.3, 1.2);
        // A load past a limit one time in six each, mostly inside them.
        let load = |g: &mut Self, lo: F, hi: F| {
            if g.one_in(14) {
                hi + g.f(0.0, 10.0)
            } else if g.one_in(13) {
                -lo - g.f(0.0, 10.0)
            } else if g.one_in(12) {
                if g.b() { hi } else { -lo }
            } else {
                g.f(-lo.0, hi.0)
            }
        };
        let fore_aft = load(self, p.front_wreck_speed_limit, p.back_wreck_speed_limit);
        let side = load(self, p.side_wreck_speed_limit, p.side_wreck_speed_limit);
        let vertical = if self.one_in(4) { self.f(20.0, 100.0) } else { self.f(-20.0, 20.0) };
        let on_ground = self.b();
        let airborne = !on_ground && !self.one_in(4);
        let air = Air {
            on_ground,
            airborne,
            was_airborne: if airborne { self.one_in(3) } else { !self.one_in(3) },
            hang_time: self.near(p.crossup_wreck_time.0, 0.0, 2.0),
            flip_torque_applied: self.one_in(6),
            wreck_impact: Vf::new(self.f(-1.0, 1.0).0, self.f(-1.0, 1.0).0, self.f(-1.0, 1.0).0, self.f(-1.0, 1.0).0),
            ..Default::default()
        };
        let angle = |g: &mut Self, lim: F| if g.one_in(8) { lim } else if g.one_in(3) { g.f(-2.0, 2.0) } else { g.f(-0.8, 0.8) };
        let pitch = if self.b() { angle(self, p.airborne_pitch_up_wreck_angle) } else { -angle(self, p.ground_pitch_down_wreck_angle) };
        let chassis = Chassis {
            pitch,
            roll: angle(self, p.airborne_roll_wreck_angle),
            vel: if self.one_in(10) {
                Vf::ZERO
            } else {
                Vf::new(self.f(-40.0, 40.0).0, self.f(-20.0, 20.0).0, self.f(-40.0, 40.0).0, self.f(-1.0, 1.0).0)
            },
            ..Default::default()
        };
        let balance = 4;
        let id = match self.bits() % 6 {
            0 => balance,
            1 => 41,
            2 => 255,
            3 => 40,
            _ => (self.bits() % 60) as i32,
        };
        let wanted = if self.one_in(2) { id } else if self.b() { 0xfe } else { (self.bits() % 60) as i32 };
        let touching = self.b();
        let up_y = self.near(p.no_hander_max_roll_angle.0, -1.0, 1.0);
        let step = air::Step {
            up: Vf::new(0.0, up_y.0, 0.0, 0.0),
            stunt: id <= 40,
            touching,
            ..Default::default()
        };
        let loads = air::Loads {
            side,
            vertical,
            fore_aft,
            landing_crossup: if self.one_in(3) { self.f(0.0, 1.0) } else { self.near(0.5, 0.0, 0.02) },
            no_hander: id == balance,
            racing: self.b(),
            pose: id,
            pose_wanted: wanted,
            pose_blend: self.near(0.95, 0.5, 1.0),
            trick_wrecked: self.one_in(4),
        };
        let struck = if self.one_in(3) { [0x20, 0x10, 0x21, 0][(self.bits() % 4) as usize] } else { 0 };
        let w = step::WreckIn { spin_applied: self.one_in(6), struck_class: touching && struck & 0x20 != 0 };
        let height = self.f(-5.0, 5.0);
        let n = if touching { self.bits() % 5 } else { 0 };
        let small = self.b();
        let contacts = (0..n)
            .map(|_| air::Contact {
                mirrored: self.one_in(4),
                reach: if self.one_in(5) { if small { step::HIT_REACH_SMALL } else { step::HIT_REACH } } else { self.f(0.0, 7.0) },
                other: if self.one_in(2) {
                    air::Other::Scenery
                } else {
                    air::Other::Rider {
                        height: if self.one_in(5) { height + p.joust_wreck_height } else { height + self.f(-4.0, 6.0) },
                        vel: Vf::new(self.f(-30.0, 30.0).0, self.f(-9.0, 9.0).0, self.f(-30.0, 30.0).0, self.f(-1.0, 1.0).0),
                    }
                },
            })
            .collect();
        let mut s = WreckState { p, air, chassis, step, loads, w, id, balance, wanted, struck, small, height, contacts };
        if self.one_in(3) {
            // A third ride along calmly, so that the hits are what is asked.
            s.loads.fore_aft = self.f(-5.0, 5.0);
            s.loads.side = self.f(-5.0, 5.0);
            s.loads.vertical = self.f(-5.0, 5.0);
            s.chassis.pitch = self.f(-0.5, 0.5);
            s.chassis.roll = self.f(-0.5, 0.5);
            s.air.flip_torque_applied = false;
            s.w.spin_applied = false;
            s.air.was_airborne = s.air.airborne;
            (s.id, s.wanted) = (255, 0xfe);
            s.step.stunt = false;
            s.loads.no_hander = false;
            (s.loads.pose, s.loads.pose_wanted) = (255, 0xfe);
        }
        s
    }
}

fn poke_wreck(ee: &mut Ee, s: &WreckState) {
    for o in (0..0x1000).step_by(4) {
        ee.set_u32(DRIVER + o, 0);
    }
    for o in (0..0x1000).step_by(4) {
        ee.set_u32(OTHERS + o, 0);
    }
    ee.set_u32(WRECKS_ON, 1);
    ee.set_u32(STRUCK_MASK, 0x20);
    // Driven by the AI, wrecks on for the bike, inside the world, not down, no striker, no node.
    ee.set_u32(OBJ + 0x48c, DRIVER);
    ee.set_u32(DRIVER + 0xe4, 1);
    ee.set_u32(OBJ + 0xc1c, 1);
    ee.set_u32(OBJ + 0x308, 0);
    ee.set_u32(OBJ + 0xc80, 0);
    put_f(ee, OBJ + 0xc88, F(100.0));
    let p = &s.p;
    for (o, v) in [
        (0x13e8, p.front_wreck_speed_limit),
        (0x13ec, p.back_wreck_speed_limit),
        (0x13f0, p.side_wreck_speed_limit),
        (0x13f4, p.racing_down_wreck_speed_limit),
        (0x13f8, p.freestyle_down_wreck_speed_limit),
        (0x13fc, p.ground_pitch_down_wreck_angle),
        (0x1400, p.ground_pitch_up_wreck_angle),
        (0x1404, p.airborne_pitch_down_wreck_angle),
        (0x1408, p.airborne_pitch_up_wreck_angle),
        (0x140c, p.airborne_roll_wreck_angle),
        (0x1410, p.no_hander_wreck_speed_limit),
        (0x1414, p.no_hander_max_roll_angle),
        (0x1418, p.joust_wreck_height),
        (0x141c, s.loads.landing_crossup),
        (0x1420, p.crossup_wreck),
        (0x1424, p.crossup_wreck_time),
        (0x1250, s.loads.side),
        (0x1254, s.loads.vertical),
        (0x1258, s.loads.fore_aft),
        (0x14e4, s.loads.pose_blend),
        (0x360, s.chassis.pitch),
        (0x368, s.chassis.roll),
        (0x1190, s.air.hang_time),
    ] {
        put_f(ee, OBJ + o, v);
    }
    put_b(ee, OBJ + 0x14cc, s.loads.trick_wrecked);
    put_b(ee, OBJ + 0x146c, s.loads.racing);
    put_b(ee, OBJ + 0x324, s.air.on_ground);
    put_b(ee, OBJ + 0xde0, s.air.airborne);
    put_b(ee, OBJ + 0xde4, s.air.was_airborne);
    put_b(ee, OBJ + 0x1194, s.air.flip_torque_applied);
    put_b(ee, OBJ + 0x11d0, s.w.spin_applied);
    ee.set_u32(OBJ + 0x1524, s.id as u32);
    ee.set_u32(OBJ + 0x152c, s.balance as u32);
    ee.set_u32(OBJ + 0x1530, s.wanted as u32);
    put_v(ee, OBJ + 0xbf0, s.step.up);
    put_v(ee, OBJ + 0x1260, s.air.wreck_impact);
    put_v(ee, BODY + 0x80, s.chassis.vel);
    put_f(ee, BODY + 0x74, s.height);
    ee.set_u32(OBJ + 0xbc0, if s.small { 0 } else { 2 });
    // The record, and the empty one the wreck entry puts in its place.
    ee.set_u32(OBJ + 0xc8, RECORD);
    ee.set_u32(OBJ + 0x1434, WRECK_RECORD);
    put_b(ee, RECORD + 0x34, s.step.touching);
    ee.set_u32(RECORD + 0x20c, if s.struck != 0 { STRUCK } else { 0 });
    ee.set_u32(STRUCK + 0x94, s.struck);
    ee.set_u32(RECORD + 0xf0, s.contacts.len() as u32);
    ee.set_u32(RECORD + 0xf8, CONTACTS);
    for (i, c) in s.contacts.iter().enumerate() {
        let at = CONTACTS + 0x90 * i as u32;
        for o in (0..0x90).step_by(4) {
            ee.set_u32(at + o, 0);
        }
        put_b(ee, at, c.mirrored);
        put_f(ee, at + 0x24, c.reach);
        if let air::Other::Rider { height, vel } = c.other {
            let (bike, body) = (OTHERS + 0x200 * i as u32, OTHERS + 0x200 * i as u32 + 0x100);
            ee.set_u32(at + 0x84, bike);
            ee.set_u32(bike + 0xe4, body);
            put_f(ee, body + 0x74, height);
            put_v(ee, body + 0x80, vel);
        }
    }
}

fn wreck_path(s: &WreckState, landed: Option<air::Wreck>, hit: Option<air::Wreck>) -> &'static str {
    use air::Wreck as W;
    match (landed, hit) {
        (Some(W::Struck), _) if s.air.airborne => "15 in the air",
        (Some(W::Struck), _) => "15 on the loads",
        (Some(W::Back), _) => "back",
        (Some(W::Front), _) if s.air.on_ground && s.chassis.pitch < -s.p.ground_pitch_down_wreck_angle => "ground pitch down",
        (Some(W::Front), _) => "front",
        (Some(W::Left | W::Right), _) if s.air.was_airborne && !s.air.airborne => "crossup",
        (Some(W::Left | W::Right), _) => "side",
        (Some(W::Flat), _) => "flat",
        (Some(W::NoseDive), _) => "nose dive",
        (Some(W::Tail), _) if s.air.on_ground && s.chassis.pitch > s.p.ground_pitch_up_wreck_angle => "ground pitch up",
        (Some(W::Tail), _) if s.step.touching && (s.air.flip_torque_applied || s.w.spin_applied) => "torqued touch",
        (Some(W::Tail), _) => "airborne pitch up",
        (Some(W::Joust), _) => "landed onto",
        (Some(W::NoHander), _) if s.id == s.balance && s.step.up.y() < s.p.no_hander_max_roll_angle.0 => "no-hander roll",
        (Some(W::NoHander), _) => "mid trick",
        (Some(w), _) => panic!("{w:?}"),
        (None, Some(W::Joust)) if s.small => "hit from above, small",
        (None, Some(W::Joust)) => "hit from above",
        (None, Some(w)) => panic!("{w:?} from the hits"),
        (None, None) if s.step.touching && !s.contacts.is_empty() => "touched, rides on",
        (None, None) => "rides on",
    }
}

/// SLUS-21104's wreck test on a driven bike not yet down, the port's [`step::wreck_entries`] (its
/// [`step::wreck_test`] then [`step::hit_wreck`] whatever the first answered): 10,000 states through
/// the original with its own vtable, the wreck entry hooked to log the reason and set the reason
/// words. The hook leaves the record where it is, as the original's entry does
/// ([`an_entry_leaves_the_record_touched`], B35), so a clause and a hit both enter. The reasons
/// entered, in order, and the throw at `+0x1260` must be the port's.
#[test]
fn mxvatv_wreck_test_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let log = Rc::new(RefCell::new(Vec::new()));
    {
        let log = log.clone();
        ee.hooks.insert(WRECK_ENTRY, Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, OBJ, "the bike");
            let reason = ee.r(5) as u32;
            log.borrow_mut().push(reason);
            ee.set_u32(OBJ + 0xc80, reason);
            ee.set_u32(OBJ + 0xc84, 0);
            ee.set_u32(OBJ + 0xc8c, 0);
        }));
    }
    let mut g = Gen(0x0d63_c002);
    let mut tally = BTreeMap::new();
    let mut mxu_differs = BTreeMap::new();
    for k in 0..10_000 {
        let s = g.wreck();
        reset(&mut ee);
        poke_wreck(&mut ee, &s);
        log.borrow_mut().clear();
        ee.call(WRECK_TEST, &[u64::from(OBJ)], &[0.0], 20_000);
        let frame = (ee.stack - 0x2000, ee.stack + 0x40);
        for (a, b) in ee.written_ranges() {
            let ok = (a >= OBJ && b <= OBJ + 0x1800) || (a >= frame.0 && b <= frame.1) || a == WRECKS_ON || a == STRUCK_MASK
                || (a >= DRIVER && b <= OTHERS + 0x1000);
            assert!(ok, "state {k}: the original wrote {a:#010x}..{b:#010x}");
        }
        let got = log.borrow().clone();
        let impact: Vec<u32> = (0..4).map(|i| ee.u32_at(OBJ + 0x1260 + 4 * i)).collect();

        let mut air = s.air.clone();
        let (landed, hit) =
            step::wreck_entries(&s.p, &mut air, &s.chassis, &s.step, &s.loads, &s.w, true, s.small, s.height, &s.contacts);
        let path = wreck_path(&s, landed, hit);
        let want: Vec<u32> = landed.into_iter().chain(hit).map(|w| w as u32).collect();
        assert_eq!(got, want, "state {k} ({path}): the reasons\n{s:#?}");
        let port: Vec<u32> = air.wreck_impact.0.iter().map(|v| v.to_bits()).collect();
        assert_eq!(impact, port, "state {k} ({path}): the throw\n{s:#?}");
        *tally.entry(path).or_insert(0u32) += 1;
        if want.len() == 2 {
            *tally.entry(if hit == Some(air::Wreck::Joust) { "a clause, then a joust" } else { "a clause, then scenery" })
                .or_insert(0) += 1;
        }
        let want: Vec<u32> = want.last().copied().into_iter().collect();
        // MX Unleashed's test on the same state, its 360 the applied one: how often it answers otherwise.
        let mut mxu = s.air.clone();
        let st = air::Step { spinning: s.w.spin_applied, ..s.step.clone() };
        let landed = air::wreck_test_in(&s.p, &mut mxu, &s.chassis, &st, &s.loads, true);
        let hit = if landed.is_some() {
            None
        } else {
            air::hit_wreck(&s.p, &mut mxu, s.step.touching, s.small, F::ONE, s.height, &s.contacts)
        };
        if landed.or(hit).map(|w| w as u32).into_iter().collect::<Vec<_>>() != want {
            *mxu_differs.entry(path).or_insert(0u32) += 1;
        }
    }
    eprintln!("{tally:?}\nMX Unleashed's answers otherwise: {mxu_differs:?}");
    enough(
        &tally,
        &[
            "15 in the air", "15 on the loads", "back", "front", "side", "flat", "ground pitch down", "ground pitch up",
            "nose dive", "airborne pitch up", "torqued touch", "crossup", "landed onto", "no-hander roll", "mid trick",
            "hit from above", "hit from above, small", "touched, rides on", "rides on",
        ],
        50,
    );
    enough(&tally, &["a clause, then a joust", "a clause, then scenery"], 10);
    // What SLUS-21104 changed is in the states: reason 15, the 360's torque, the trick endings' gate
    // and the hits' reach.
    enough(&mxu_differs, &["15 in the air", "15 on the loads", "torqued touch", "mid trick", "touched, rides on"], 50);
    enough(&mxu_differs, &["hit from above", "hit from above, small"], 10);
}

/// B35: SLUS-21104's whole wreck entry, run unhooked on the Free Ride image's own bike (its record
/// given a touch and a contact), leaves the record where it was, touching, its contact count kept: it
/// writes the record's part word (`+0x200`) and nothing else on it. MX Unleashed's entry clears both
/// (S191a); this is why SLUS-21104's hits still answer after a clause's entry.
#[test]
fn an_entry_leaves_the_record_touched() {
    const BIKE: u32 = 0x00e7_14e0;
    const GP: u64 = 0x003d_baf0;
    let Some(mut ee) = image() else { return };
    ee.hooks.insert(0x0034_8fa0, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    ee.hooks.insert(0x0034_8ff0, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    ee.hooks.insert(0x004e_7e70, Box::new(|_: &mut Ee| {}));
    // Every `addiu $3, $zero, n; syscall; jr $ra`: the answer is a0, the thread id (0x2f) 1.
    for a in (0x0030_0000..0x0036_0000u32).step_by(4) {
        if ee.u32_at(a) >> 16 == 0x2403 && ee.u32_at(a + 4) == 0x0000_000c && ee.u32_at(a + 8) == 0x03e0_0008 {
            let n = ee.u32_at(a) & 0xffff;
            ee.hooks.insert(a, Box::new(move |ee: &mut Ee| {
                let v = if n == 0x2f { 1 } else { ee.r(4) };
                ee.set(2, v);
            }));
        }
    }
    assert_eq!(ee.u32_at(BIKE + 4), BIKE_VTABLE, "the image's bike");
    assert_eq!(ee.u32_at(BIKE + 0xc80), 0, "the image's bike is up");
    let rec = ee.u32_at(BIKE + 0xc8);
    ee.set_u32(rec + 0x34, 1);
    ee.set_u32(rec + 0xf0, 1);
    let before: Vec<u32> = (0..0x300).step_by(4).map(|o| ee.u32_at(rec + o)).collect();
    ee.set(28, GP);
    ee.call(WRECK_ENTRY, &[u64::from(BIKE), 4], &[], 50_000_000);
    assert_eq!(ee.u32_at(BIKE + 0xc80), 4, "the entry ran");
    assert_eq!(ee.u32_at(BIKE + 0xc8), rec, "the record stays");
    assert_eq!(ee.u32_at(BIKE + 0xcc), ee.u32_at(BIKE + 0x1434), "the record's part is the wreck one");
    for (i, w) in before.iter().enumerate() {
        let o = 4 * i as u32;
        let want = if o == 0x200 { ee.u32_at(BIKE + 0x1434) } else { *w };
        assert_eq!(ee.u32_at(rec + o), want, "the record's +{o:#x}");
    }
}

/// B35, the typed step on its own: a frame where a clause and a hit both hold enters twice and ends on
/// the hit, as [`game_sim::handling::wreck::wreck_test`] does. Loads past the back limit (a 4) and a
/// rider landing on the bike from above end on the joust and its throw; the same loads and scenery
/// end on the scenery hit, the throw left as it was.
#[test]
fn a_clause_then_a_hit_ends_on_the_hit() {
    let mut p = zero_params();
    p.front_wreck_speed_limit = F(18.0);
    p.back_wreck_speed_limit = F(22.0);
    p.side_wreck_speed_limit = F(19.0);
    p.racing_down_wreck_speed_limit = F(70.0);
    p.freestyle_down_wreck_speed_limit = F(70.0);
    p.no_hander_wreck_speed_limit = F(70.0);
    p.joust_wreck_height = F(1.0);
    let loads = air::Loads { fore_aft: F(30.0), pose: 41, pose_wanted: 42, ..Default::default() };
    let step = air::Step { touching: true, ..Default::default() };
    let w = step::WreckIn { spin_applied: false, struck_class: false };
    let (chassis, before) = (Chassis::default(), Vf::new(1.0, 2.0, 3.0, 4.0));
    let vel = Vf::new(5.0, -6.0, 7.0, 0.0);
    for (other, hit, throw) in [
        (air::Other::Rider { height: F(3.0), vel }, air::Wreck::Joust, vel),
        (air::Other::Scenery, air::Wreck::Scenery, before),
    ] {
        let mut a = Air { on_ground: true, wreck_impact: before, ..Default::default() };
        let contacts = [air::Contact { mirrored: false, reach: F(5.0), other }];
        let entries = step::wreck_entries(&p, &mut a, &chassis, &step, &loads, &w, true, false, F(0.5), &contacts);
        assert_eq!(entries, (Some(air::Wreck::Back), Some(hit)));
        assert_eq!(a.wreck_impact, throw, "{hit:?}'s throw");
    }
}

/// Reason 14, the bike with no driver (no AI driving it, `+0x48c`'s `+0xe4`, and no vehicle state of
/// kind 1, `+0xbc4`'s `+0x28`): SLUS-21104's branch is MX Unleashed's machine
/// ([`game_sim::state::Ride::step`]'s `NO_CONTROL`) — entered only on a bike not already down, the
/// clock preloaded to `MaxWreckTime` (`+0xc88`) and counting, the wreck count up one. 2,000 states.
#[test]
fn mxvatv_no_control_is_the_machines() {
    use game_sim::state;
    let Some(mut ee) = image() else { return };
    let log = Rc::new(RefCell::new(Vec::new()));
    {
        let log = log.clone();
        ee.hooks.insert(WRECK_ENTRY, Box::new(move |ee| {
            let reason = ee.r(5) as u32;
            log.borrow_mut().push(reason);
            ee.set_u32(OBJ + 0xc80, reason);
            ee.set_u32(OBJ + 0xc84, 0);
            ee.set_u32(OBJ + 0xc8c, 0);
        }));
    }
    let mut g = Gen(0x0d63_c003);
    let mut entered = 0;
    for k in 0..2_000 {
        let prior = if g.b() { 0 } else { 1 + g.bits() % 15 };
        let (elapsed, counting, max) = (g.f(0.0, 5.0), g.b(), g.f(0.5, 5.0));
        reset(&mut ee);
        for o in (0..0x100).step_by(4) {
            ee.set_u32(DRIVER + o, 0);
        }
        // No driver, either way the original asks it.
        match g.bits() % 4 {
            0 => ee.set_u32(OBJ + 0x48c, 0),
            1 => ee.set_u32(OBJ + 0x48c, DRIVER),
            2 => {
                ee.set_u32(OBJ + 0xbc4, DRIVER);
                ee.set_u32(DRIVER + 0x28, 2);
            }
            _ => {
                ee.set_u32(OBJ + 0x48c, DRIVER);
                ee.set_u32(OBJ + 0xbc4, DRIVER);
                ee.set_u32(DRIVER + 0x28, 3);
            }
        }
        ee.set_u32(OBJ + 0xc80, prior);
        put_f(&mut ee, OBJ + 0xc84, elapsed);
        put_b(&mut ee, OBJ + 0xc8c, counting);
        put_f(&mut ee, OBJ + 0xc88, max);
        log.borrow_mut().clear();
        ee.call(WRECK_TEST, &[u64::from(OBJ)], &[g.f(0.0, 0.05).0], 20_000);
        only_touched_the_regions(&ee).unwrap_or_else(|e| panic!("state {k}: {e}"));

        let mut ride = state::Ride {
            wreck: (prior != 0).then_some(game_sim::trick::Wreck(prior as u8)),
            elapsed,
            counting,
            ..Default::default()
        };
        let p = state::Params { max_wreck_time: max, cannonball_wreck_time: F::ZERO };
        let out = ride.step(F::ZERO, state::Sense::default(), p, |_| panic!("no test without a driver"), || false);
        let want: Vec<u32> = out.wreck.map(|w| u32::from(w.0)).into_iter().collect();
        assert_eq!(*log.borrow(), want, "state {k}: the reasons");
        assert_eq!(ee.u32_at(OBJ + 0xc80), ride.wreck.map_or(0, |w| u32::from(w.0)), "state {k}: the reason");
        assert_eq!(ee.u32_at(OBJ + 0xc84), ride.elapsed.0.to_bits(), "state {k}: the clock");
        assert_eq!(ee.u32_at(OBJ + 0xc8c), u32::from(ride.counting), "state {k}: the latch");
        assert_eq!(ee.u32_at(OBJ + 0x6c4) as i32, ride.wrecks, "state {k}: the wreck count");
        entered += want.len();
    }
    assert!(entered >= 500, "only {entered} states entered reason 14");
}

// --- MD71: the wreck clock and put-back ----------------------------------------------------------------

/// The unwreck's calls out (vtable `+0x4a8`, `+0x4b0`, `+0x490`, the placement `+0x3a8`, and the
/// engine's reset), the out-of-bounds ask (`+0x16c`), the race record whose riding bit it sets, and
/// the word a cannonball's put-back divides its gravity in.
const UNWRECK: u32 = 0x004e_2380;
const UNWRECK_CALLS: [u32; 3] = [0x004e_4cd0, 0x004e_4e50, 0x004e_2eb0];
const PLACE: u32 = 0x004d_8940;
const ENGINE_RESET: u32 = 0x0049_f7f0;
const OUT_OF_BOUNDS: u32 = 0x004a_4f20;
const RACE: u32 = 0x01c0_7800;
const GRAVITY: u32 = 0x01c0_7900;
const RIDING_BIT: u64 = 0x40000;

/// What one frame of the harness hands the machine.
#[derive(Clone, Copy, Debug)]
struct ClockFrame {
    dt: F,
    driven: bool,
    /// The global and the bike's own wrecks-on words.
    wrecks: (bool, bool),
    settles: bool,
    /// The bike is past the edge (`+0x308` on and `+0x16c` answering 1): reason 13.
    out: bool,
}

#[allow(clippy::float_arithmetic)]
impl Gen {
    fn clock_frame(&mut self) -> ClockFrame {
        let dt = match self.bits() % 4 {
            0 => F(1.0 / 60.0),
            1 => F(1.0 / 30.0),
            _ => self.f(0.0, 0.1),
        };
        ClockFrame {
            dt,
            driven: !self.one_in(12),
            wrecks: (!self.one_in(10), !self.one_in(10)),
            settles: !self.one_in(3),
            out: self.one_in(25),
        }
    }
}

fn clock_poke(ee: &mut Ee, f: &ClockFrame) {
    ee.set_u32(OBJ + 0x48c, if f.driven { DRIVER } else { 0 });
    ee.set_u32(DRIVER + 0xe4, 1);
    ee.set_u32(OBJ + 0xbc4, 0);
    ee.set_u32(WRECKS_ON, u32::from(f.wrecks.0));
    ee.set_u32(OBJ + 0xc1c, u32::from(f.wrecks.1));
    ee.set_u32(OBJ + 0x308, u32::from(f.out));
    put_b(ee, OBJ + 0x920, f.settles);
}

/// SLUS-21104's ride machine, frame by frame: the wreck test's tail (`+0xc84` clock, `+0xc8c` latch,
/// `+0xc88` MaxWreckTime, `+0x920` settled) over the real unwreck (`vt +0x36c`, its base half
/// included; only its calls out are stubbed and logged), against [`step::machine`]. 3,000 runs of 1 to
/// 120 frames from a generated reason, clock, latch and limit; each frame a generated `dt`, driver,
/// wrecks-on words, settle flag and out-of-bounds answer (the one clause the harness lets wreck, 13,
/// so the chain is never asked on a zeroed bike). Every frame the reason, clock, latch, wreck count,
/// riding bit, the entries, the put-backs and their kind, and the events posted must be the port's.
#[test]
fn mxvatv_wreck_clock_is_the_originals() {
    use game_sim::state;
    use game_sim::trick::Wreck as Reason;
    let Some(mut ee) = image() else { return };
    #[derive(Debug, PartialEq)]
    enum Call {
        Entry(u32),
        Unwreck(u32),
        Place(u32),
        Post(u32, u32),
    }
    let log = Rc::new(RefCell::new(Vec::new()));
    let out_of_bounds = Rc::new(RefCell::new(false));
    {
        let log = log.clone();
        ee.hooks.insert(WRECK_ENTRY, Box::new(move |ee| {
            let reason = ee.r(5) as u32;
            log.borrow_mut().push(Call::Entry(reason));
            ee.set_u32(OBJ + 0xc80, reason);
            ee.set_u32(OBJ + 0xc84, 0);
            ee.set_u32(OBJ + 0xc8c, 0);
        }));
    }
    {
        let log = log.clone();
        let seen = log.clone();
        ee.hooks.insert(PLACE, Box::new(move |ee| {
            assert_eq!(ee.r(5), 0, "the placement's first argument");
            seen.borrow_mut().push(Call::Place(ee.r(6) as u32));
        }));
        ee.hooks.insert(POST_EVENT, Box::new(move |ee| {
            assert_eq!(ee.r(5) as u32, OBJ + 0x674, "the bike's event source");
            log.borrow_mut().push(Call::Post(ee.r(6) as u32, ee.r(7) as u32));
        }));
    }
    for a in UNWRECK_CALLS.into_iter().chain([ENGINE_RESET]) {
        ee.hooks.insert(a, Box::new(|_| {}));
    }
    {
        let out_of_bounds = out_of_bounds.clone();
        ee.hooks.insert(OUT_OF_BOUNDS, Box::new(move |ee| ee.set(2, u64::from(*out_of_bounds.borrow()))));
    }
    // The unwreck is reached through the vtable and runs; its first call out (`+0x4a8`, which only it
    // makes here) logs that it was reached.
    {
        let log = log.clone();
        ee.hooks.insert(UNWRECK_CALLS[0], Box::new(move |ee| log.borrow_mut().push(Call::Unwreck(ee.r(4) as u32))));
    }
    assert_eq!(ee.u32_at(BIKE_VTABLE + 0x36c), UNWRECK);
    assert_eq!(ee.u32_at(BIKE_VTABLE + 0x16c), OUT_OF_BOUNDS);
    assert_eq!(ee.u32_at(BIKE_VTABLE + 0x3a8), PLACE);

    let mut g = Gen(0x0d71_0001);
    let mut tally: BTreeMap<&'static str, u32> = BTreeMap::new();
    let mut mxu_differs = 0u32;
    for run in 0..3_000 {
        reset(&mut ee);
        for o in (0..0x100).step_by(4) {
            ee.set_u32(DRIVER + o, 0);
        }
        for o in (0..0x200).step_by(4) {
            ee.set_u32(RACE + o, 0);
        }
        let prior = match g.bits() % 5 {
            0 => 0,
            1 => 13,
            2 => 14,
            _ => 1 + g.bits() % 15,
        };
        let max = match g.bits() % 8 {
            0 => F(-0.1),
            1 => F::ZERO,
            _ => g.f(0.2, 4.0),
        };
        let counting = g.b();
        let elapsed = match g.bits() % 5 {
            0 => F(0.5),
            1 => F(2.5),
            2 => max,
            _ => g.f(0.0, 4.0),
        };
        let wrecks = (g.bits() % 100) as i32;
        let race = u64::from(g.bits()) << 32 | u64::from(g.bits());
        ee.set_u32(OBJ + 0xc80, prior);
        put_f(&mut ee, OBJ + 0xc84, elapsed);
        put_b(&mut ee, OBJ + 0xc8c, counting);
        put_f(&mut ee, OBJ + 0xc88, max);
        ee.set_u32(OBJ + 0x6c4, wrecks as u32);
        ee.set_u32(OBJ + 0x680, RACE);
        ee.write(RACE + 8, 8, u128::from(race));
        ee.set_u32(OBJ + 0xe8, GRAVITY);
        put_f(&mut ee, GRAVITY + 4, F(1.0));
        let mut ride = state::Ride {
            wreck: (prior != 0).then_some(Reason(prior as u8)),
            elapsed,
            counting,
            wrecks,
            ..Default::default()
        };
        let mut bit = race & RIDING_BIT != 0;
        let frames = if g.b() { 1 } else { 1 + g.bits() % 120 };
        for frame in 0..frames {
            let f = g.clock_frame();
            clock_poke(&mut ee, &f);
            let was = ride.wreck;
            // The original asks the edge only when its clause is reached (prior reason not 13).
            *out_of_bounds.borrow_mut() = f.out;
            log.borrow_mut().clear();
            ee.call(WRECK_TEST, &[u64::from(OBJ), 0], &[f.dt.0], 50_000);
            let frame_at = (ee.stack - 0x2000, ee.stack + 0x40);
            for (a, b) in ee.written_ranges() {
                let ok = REGIONS.iter().any(|&(_, base, len)| a >= base && b <= base + len)
                    || (a >= frame_at.0 && b <= frame_at.1)
                    || (a >= RACE && b <= RACE + 0x200);
                assert!(ok, "run {run} frame {frame}: the original wrote {a:#010x}..{b:#010x}");
            }

            let sense = state::Sense {
                ai_route: f.driven,
                wrecks_enabled: f.wrecks.0 && f.wrecks.1,
                settles: f.settles,
                ..Default::default()
            };
            let before = ride;
            let (out, armed) = step::machine(&mut ride, f.dt, sense, max, |allowed| {
                (allowed.wrecks && f.out && was != Some(Reason::CANNONBALL)).then_some(Reason::CANNONBALL)
            });
            let mut want = Vec::new();
            if let Some(w) = out.wreck {
                want.push(Call::Entry(u32::from(w.0)));
            }
            if armed {
                want.push(Call::Post(8, 12));
            }
            if out.reset.is_some() {
                assert_eq!(out.reset, Some(state::Reset::Now));
                want.extend([Call::Unwreck(OBJ), Call::Place(state::Kind::Wrecked as u32), Call::Post(8, 13)]);
            }
            let here = format!("run {run} frame {frame}: {f:?} from {before:?}");
            assert_eq!(*log.borrow(), want, "{here}: the calls");
            assert_eq!(ee.u32_at(OBJ + 0xc80), ride.wreck.map_or(0, |w| u32::from(w.0)), "{here}: the reason");
            assert_eq!(ee.u32_at(OBJ + 0xc84), ride.elapsed.0.to_bits(), "{here}: the clock {} vs {}", ee.f32_at(OBJ + 0xc84), ride.elapsed.0);
            assert_eq!(ee.u32_at(OBJ + 0xc8c), u32::from(ride.counting), "{here}: the latch");
            assert_eq!(ee.u32_at(OBJ + 0x6c4) as i32, ride.wrecks, "{here}: the wreck count");
            if let Some(b) = out.riding_bit {
                bit = b;
            }
            let now = ee.read(RACE + 8, 8) as u64;
            assert_eq!(now & RIDING_BIT != 0, bit, "{here}: the riding bit");
            assert_eq!(now & !RIDING_BIT, race & !RIDING_BIT, "{here}: the rest of the race record");

            // MX Unleashed's machine on the same frame, its cannonball time the same limit.
            let mut mxu = before;
            let p = state::Params { max_wreck_time: max, cannonball_wreck_time: max };
            let o = mxu.step(f.dt, sense, p, |allowed| {
                (allowed.wrecks && f.out && was != Some(Reason::CANNONBALL)).then_some(Reason::CANNONBALL)
            }, || false);
            if (mxu.wreck, mxu.elapsed.0.to_bits(), mxu.counting, o.reset.is_some()) != (ride.wreck, ride.elapsed.0.to_bits(), ride.counting, out.reset.is_some()) {
                mxu_differs += 1;
            }

            let path = match (out.wreck, armed, out.reset) {
                (Some(Reason::NO_CONTROL), ..) => "no control",
                (Some(_), ..) if was.is_some() => "re-wrecked while down",
                (Some(_), ..) => "wrecked",
                (None, true, Some(_)) => "armed and put back",
                (None, true, None) if was == Some(Reason::CANNONBALL) => "armed, cannonball",
                (None, true, None) => "armed",
                (None, false, Some(_)) if was == Some(Reason::CANNONBALL) => "put back, cannonball",
                (None, false, Some(_)) if was == Some(Reason::NO_CONTROL) => "put back, no control",
                (None, false, Some(_)) => "put back",
                (None, false, None) if was.is_some() && ride.counting => "counting",
                (None, false, None) if was.is_some() && !f.settles => "down, not settled",
                (None, false, None) if was.is_some() => "settling",
                (None, false, None) => "riding",
            };
            *tally.entry(path).or_insert(0) += 1;
        }
    }
    eprintln!("{tally:?}\nMX Unleashed's machine answers otherwise on {mxu_differs} frames");
    enough(
        &tally,
        &[
            "no control", "re-wrecked while down", "wrecked", "armed and put back", "armed", "armed, cannonball",
            "put back", "put back, cannonball", "put back, no control", "counting", "down, not settled", "settling", "riding",
        ],
        50,
    );
    assert!(mxu_differs >= 1_000, "only {mxu_differs} frames tell MX Unleashed's machine from MX vs. ATV's");
}
