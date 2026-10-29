//! MD40 — MX vs. ATV's bike steps, proved against MX vs. ATV's own code.
//!
//! The oracle here is SLUS-21104, not MX Unleashed: a RAM image of a Free Ride in MX vs. ATV
//! Unleashed (`context/mxvatv-ps2/state04.ee.bin`, the eeMemory of PCSX2 save state 4 of copy 9 that
//! openmx-mods' M19/M21 captures started from) holds its game overlay at run time, so its functions run
//! in the interpreter as they are, with their own vtables. The harness is `tests/air.rs`'s: lay a bike
//! object out in RAM, write a generated state into it, run the original, write the same state again with
//! the port's answer over it, and compare the regions word for word. The rider's stunt calls and the
//! wreck entry are hooked and logged; the port says what it asked for and the two logs must agree.
//!
//! Without the image every test here skips (and says so).

use game_sim::air::{Air, Chassis, Params, Step};
use game_sim::handling::{self, FlipClip, FlipIn};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};
use std::cell::RefCell;
use std::rc::Rc;

/// Where the harness puts the bike and its pieces: the top of RAM, under the interpreter's stack.
const OBJ: u32 = 0x01c0_0000;
const BODY: u32 = 0x01c0_2000;
const XF: u32 = 0x01c0_2400;
const COLL: u32 = 0x01c0_2600;
const REGIONS: [(&str, u32, u32); 4] = [("bike", OBJ, 0x1800), ("body", BODY, 0x200), ("frame", XF, 0xa0), ("collision", COLL, 0x40)];

/// The bike's and its rigid body's vtables in the image, and the four calls the harness answers.
const BIKE_VTABLE: u32 = 0x003d_0450;
const BODY_VTABLE: u32 = 0x003c_d200;
const BACKFLIP: u32 = 0x004e_4f20;
const START_STUNT: u32 = 0x004e_6370;
const END_STUNT: u32 = 0x004e_4db0;
const WRECK_ENTRY: u32 = 0x004e_2260;
const STEEP: u32 = 0x004e_8370;

fn image() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2").join(format!("state04.ee{}", ".bin"));
    if !path.exists() {
        skip(&format!(
            "{} is not there: unzip eeMemory.bin from MX vs. ATV's save state 4 (SLUS-21104 (95BB1901).04.p2s, \
             ps2-decomp-kit-slots/pcsx2/s9) to it",
            path.display()
        ));
        return None;
    }
    let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    // The image must be MX vs. ATV's, overlay loaded: the bike's vtable points at its backflip's neighbours.
    if ee.u32_at(BIKE_VTABLE + 0x368) != WRECK_ENTRY {
        panic!("{} is not the MX vs. ATV image these addresses are from", path.display());
    }
    Some(ee)
}

/// What the hooked calls answer and what they were asked.
#[derive(Default, Debug)]
struct Calls {
    start_answer: bool,
    steep_answer: bool,
    started: Vec<u32>,
    ended: Vec<u32>,
    steep: u32,
    wrecks: Vec<u32>,
}

fn harness() -> Option<(Ee, Rc<RefCell<Calls>>)> {
    let mut ee = image()?;
    let calls = Rc::new(RefCell::new(Calls::default()));
    let c = calls.clone();
    ee.hooks.insert(START_STUNT, Box::new(move |ee| {
        let mut c = c.borrow_mut();
        c.started.push(ee.r(5) as u32);
        ee.set(2, u64::from(c.start_answer));
    }));
    let c = calls.clone();
    ee.hooks.insert(END_STUNT, Box::new(move |ee| c.borrow_mut().ended.push(ee.r(5) as u32)));
    let c = calls.clone();
    ee.hooks.insert(WRECK_ENTRY, Box::new(move |ee| c.borrow_mut().wrecks.push(ee.r(5) as u32)));
    let c = calls.clone();
    ee.hooks.insert(STEEP, Box::new(move |ee| {
        let mut c = c.borrow_mut();
        c.steep += 1;
        ee.set(2, u64::from(c.steep_answer));
    }));
    Some((ee, calls))
}

#[derive(Clone, Debug)]
struct State {
    p: Params,
    air: Air,
    ch: Chassis,
    st: Step,
    inp: FlipIn,
    clip: FlipClip,
    /// The bits of the trick-button byte above the nibble the step reads.
    high: u8,
    flip_stunt: u32,
    rate_increase: F,
    start_answer: bool,
    steep_answer: bool,
}

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
}

fn poke(ee: &mut Ee, s: &State) {
    ee.set_u32(OBJ + 0x4, BIKE_VTABLE);
    ee.set_u32(OBJ + 0xb8, BODY);
    ee.set_u32(OBJ + 0xc8, COLL);
    ee.set_u32(OBJ + 0xe4, XF);
    ee.set_u32(BODY + 0x110, BODY_VTABLE);
    ee.set_u32(BODY + 0xe8, 0); // no force listeners on the body

    let (p, air, ch, st, inp) = (&s.p, &s.air, &s.ch, &s.st, &s.inp);
    put_f(ee, OBJ + 0x11a4, p.min_backflip_time);
    put_f(ee, OBJ + 0x11a8, p.max_backflip_time);
    put_f(ee, OBJ + 0x11b0, p.min_backflip_torque);
    put_f(ee, OBJ + 0x11b4, p.max_backflip_torque);
    put_f(ee, OBJ + 0x11b8, s.rate_increase);
    put_f(ee, OBJ + 0x11bc, p.backflip_torque_rate_decrease);
    put_f(ee, OBJ + 0x118c, p.control_authority_time);
    put_f(ee, OBJ + 0x1404, p.airborne_pitch_down_wreck_angle);
    put_f(ee, OBJ + 0x1408, p.airborne_pitch_up_wreck_angle);

    put_b(ee, OBJ + 0x324, air.on_ground);
    put_v(ee, OBJ + 0xc30, air.contact_vel);
    put_f(ee, OBJ + 0x1188, air.authority);
    put_b(ee, OBJ + 0x1194, air.flip_torque_applied);
    put_b(ee, OBJ + 0x1198, air.flipping);
    put_b(ee, OBJ + 0x119c, air.flip_active);
    put_b(ee, OBJ + 0x11a0, air.flip_armed);
    put_f(ee, OBJ + 0x11ac, air.flip_grace);
    put_v(ee, OBJ + 0x11c0, air.flip_torque);

    put_v(ee, BODY + 0x80, ch.vel);
    put_v(ee, BODY + 0x1b0, ch.moment);
    put_f(ee, OBJ + 0x360, ch.pitch);
    put_f(ee, OBJ + 0x368, ch.roll);

    put_f(ee, OBJ + 0x310, st.brake);
    put_f(ee, OBJ + 0x12ac, st.stick.1);
    put_f(ee, OBJ + 0x12b0, st.stick_hold);
    put_b(ee, OBJ + 0x11d0, st.spinning);
    put_b(ee, OBJ + 0x11d8, st.spin_active);
    put_b(ee, COLL + 0x34, st.touching);

    ee.set_u32(OBJ + 0x14ec, u32::from(inp.buttons | s.high << 4));
    put_f(ee, XF + 0x84, inp.nose_y);
    put_f(ee, XF + 0x94, inp.up_y);
    ee.set_u32(OBJ + 0x1528, inp.stunt_id as u32);
    ee.set_u32(OBJ + 0x1754, s.clip.0 as u32);
    ee.set_u32(OBJ + 0x13b8, s.flip_stunt);
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
    fn vec(&mut self, lo: f32, hi: f32) -> Vf {
        Vf::new(self.f(lo, hi).0, self.f(lo, hi).0, self.f(lo, hi).0, self.f(lo, hi).0)
    }

    fn state(&mut self) -> State {
        let mut s = State {
            p: zero_params(),
            air: Air::default(),
            ch: Chassis::default(),
            st: Step::default(),
            inp: FlipIn::default(),
            clip: FlipClip::default(),
            high: 0,
            flip_stunt: 0,
            rate_increase: F::ZERO,
            start_answer: false,
            steep_answer: false,
        };
        let p = &mut s.p;
        p.min_backflip_time = if self.one_in(4) { F::ZERO } else { self.f(0.0, 30.0) };
        p.max_backflip_time = if self.one_in(8) { p.min_backflip_time } else { self.f(10.0, 80.0) };
        p.min_backflip_torque = self.f(300.0, 1200.0);
        p.max_backflip_torque = self.f(600.0, 1800.0);
        p.backflip_torque_rate_decrease = self.f(0.0, 3000.0);
        p.control_authority_time = self.f(0.0, 2.0);
        p.airborne_pitch_down_wreck_angle = self.f(0.8, 1.6);
        p.airborne_pitch_up_wreck_angle = self.f(0.8, 1.6);
        s.rate_increase = self.f(0.0, 1000.0);

        // Lean the flags towards the lip and the flight, the branches with something to compute.
        let a = &mut s.air;
        a.on_ground = self.b();
        a.flip_active = self.bits() % 3 != 0;
        a.flip_armed = self.b();
        a.flipping = self.b();
        a.flip_torque_applied = self.b();
        a.flip_grace = self.near(0.0, -0.1, 0.7);
        a.authority = self.f(-0.5, 2.0);
        a.contact_vel = Vf::new(self.f(-5.0, 5.0).0, self.f(-5.0, 5.0).0, self.near(30.0, 20.0, 60.0).0, 0.0);
        let mag = self.f(0.0, 1800.0);
        let t = if self.one_in(6) { s.p.min_backflip_torque } else { mag };
        a.flip_torque = Vf::new((if self.b() { -t } else { t }).0, self.f(-50.0, 50.0).0, self.f(-50.0, 50.0).0, self.f(-2.0, 2.0).0);

        let c = &mut s.ch;
        c.vel = Vf::new(self.f(-10.0, 10.0).0, self.near(5.0, -5.0, 15.0).0, self.f(-40.0, 40.0).0, 0.0);
        c.moment = self.vec(-500.0, 500.0);
        c.pitch = self.f(-2.0, 2.0);
        c.roll = if self.one_in(6) { F(1.5707964) } else { self.f(-3.2, 3.2) };

        let st = &mut s.st;
        st.dt = self.f(0.005, 0.04);
        st.brake = if self.bits() % 4 == 0 { self.f(0.0, 1.0) } else { F::ZERO };
        st.stick = (self.f(-1.0, 1.0), if self.one_in(4) { F(if self.b() { 0.8 } else { -0.8 }) } else { self.f(-1.2, 1.2) });
        if self.one_in(10) {
            st.stick.1 = F::ZERO;
        }
        st.stick_hold = self.f(-70.0, 70.0);
        st.spinning = self.one_in(8);
        st.spin_active = self.one_in(8);
        st.touching = self.one_in(6);

        // A third of the states are on a lip that takes a flip, the branches that arm and trigger it.
        if !s.air.flip_active && self.one_in(2) {
            s.air.on_ground = true;
            s.st.spinning = false;
            s.st.spin_active = false;
            s.st.brake = F::ZERO;
            s.ch.vel = Vf::new(s.ch.vel.x(), self.f(5.5, 15.0).0, s.ch.vel.z(), 0.0);
            s.air.contact_vel = Vf::new(s.air.contact_vel.x(), s.air.contact_vel.y(), self.f(30.5, 60.0).0, 0.0);
        }
        s.inp = FlipIn {
            buttons: if self.b() { handling::R1_ALONE } else if self.b() { 0 } else { (self.bits() & 0xf) as u8 },
            nose_y: if self.one_in(8) { F::ZERO } else { self.f(-1.0, 1.0) },
            up_y: if self.one_in(8) { F::ZERO } else if self.one_in(6) { F(-0.79) } else { self.f(-1.0, 1.0) },
            stunt_id: (self.bits() % 80) as i32,
        };
        s.high = (self.bits() & 0xf) as u8;
        s.clip = FlipClip((self.bits() % 3) as i32);
        s.flip_stunt = self.bits() % 64;
        s.start_answer = self.b();
        s.steep_answer = self.b();
        s
    }
}

/// Which way a state went through the step, written from the decompile's branches and not from the port.
fn path(s: &State) -> &'static str {
    let a = &s.air;
    if !a.flip_active {
        let lip = a.on_ground && !s.st.spinning && !s.st.spin_active && s.ch.vel.y() > 5.0 && a.contact_vel.z() > 30.0 && s.st.brake == 0.0;
        if !lip {
            "off the lip"
        } else if s.st.stick.1 < F(-0.8) && s.inp.buttons == handling::R1_ALONE {
            "arming"
        } else if s.st.stick.1 > F(0.8) && a.flip_armed {
            "triggered"
        } else {
            "waiting"
        }
    } else if !a.flip_armed {
        if a.on_ground { "in the grace" } else { "off the lip in time" }
    } else if a.on_ground || s.st.touching || F(a.flip_torque.x()).abs() < s.p.min_backflip_torque {
        if s.clip.0 != 0 { "ended mid-clip" } else { "ended" }
    } else if s.inp.nose_y <= 0.0 {
        "turning, nose down"
    } else {
        "turning, nose up"
    }
}

#[test]
fn mxvatv_backflip_is_the_originals() {
    let Some((mut ee, calls)) = harness() else { return };
    let mut g = Gen(0x0d40_0001);
    let mut tally = std::collections::BTreeMap::new();
    for k in 0..10_000 {
        let s = g.state();
        *tally.entry(path(&s)).or_insert(0u32) += 1;
        reset(&mut ee);
        poke(&mut ee, &s);
        {
            let mut c = calls.borrow_mut();
            *c = Calls { start_answer: s.start_answer, steep_answer: s.steep_answer, ..Calls::default() };
        }
        ee.call(BACKFLIP, &[u64::from(OBJ)], &[s.st.dt.0], 20_000);
        only_touched_the_regions(&ee).unwrap_or_else(|e| panic!("state {k}: {e}\n{s:#?}"));
        let was = snap(&ee);
        let orig = std::mem::take(&mut *calls.borrow_mut());

        let mut t = s.clone();
        let (mut started, mut steeped) = (false, false);
        let out = handling::backflip(
            &t.p,
            &mut t.air,
            &mut t.ch,
            &t.st,
            &t.inp,
            &mut t.clip,
            || {
                started = true;
                s.start_answer
            },
            || {
                steeped = true;
                s.steep_answer
            },
        );
        if out.stunt_cleared {
            t.inp.stunt_id = 255;
        }
        reset(&mut ee);
        poke(&mut ee, &t);
        let now = snap(&ee);
        if was != now {
            let i = was.iter().zip(&now).position(|(a, b)| a != b).unwrap();
            panic!(
                "state {k} ({}): {} is {:#010x} ({}) in the original and {:#010x} ({}) in the port\n{s:#?}",
                path(&s),
                where_is(i),
                was[i],
                f32::from_bits(was[i]),
                now[i],
                f32::from_bits(now[i]),
            );
        }
        let want_started: Vec<u32> = if started { vec![s.flip_stunt] } else { vec![] };
        assert_eq!(orig.started, want_started, "state {k}: the stunt start");
        assert_eq!(out.stunt_started, started && s.start_answer, "state {k}");
        let want_ended: Vec<u32> = if out.stunt_ended { vec![s.flip_stunt] } else { vec![] };
        assert_eq!(orig.ended, want_ended, "state {k}: the stunt end");
        assert_eq!(orig.steep, u32::from(steeped), "state {k}: the steep test");
        assert_eq!(orig.wrecks, out.wreck.map(u32::from).into_iter().collect::<Vec<_>>(), "state {k}: the wreck");
    }
    for want in ["off the lip", "arming", "triggered", "waiting", "in the grace", "off the lip in time", "ended", "ended mid-clip", "turning, nose down", "turning, nose up"] {
        let n = tally.get(want).copied().unwrap_or(0);
        assert!(n >= 50, "only {n} state(s) went down {want:?}: {tally:?}");
    }
}
