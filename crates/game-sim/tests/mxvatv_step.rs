//! MD63 — the rest of MX vs. ATV's bike step, clause by clause, proved against MX vs. ATV's own code.
//!
//! The oracle is SLUS-21104 off a RAM image of a Free Ride in it (`context/mxvatv-ps2/state04.ee.bin`,
//! `tests/mxvatv_air.rs`' image): its functions run in the interpreter as they are, with their own
//! vtables. Lay the bike out in RAM, write a generated state into it, run the original, write the same
//! state again with the port's answer over it, and compare the regions word for word; the original may
//! write nothing else. Calls out of the step are hooked and logged, and the port says what it asked for.
//!
//! Without the image every test here skips (and says so).

use game_sim::air::{self, Air, Chassis, Params};
use game_sim::handling::step;
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
