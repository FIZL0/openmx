//! MD93 — MX vs. ATV Untamed's (SLUS-21701) reworked bike functions, proved against Untamed's own code.
//!
//! MD81a found six of the bike step's functions rewritten from SLUS-21104's (research/untamed-machines.md).
//! These are the three [`handling::untamed`] ports so far: the preload, the wreck entry and the put-back,
//! each run on an Untamed Free Ride image (`context/untamed/ram/nat_or.bin`, as `tests/untamed_step.rs`) on
//! the bike laid out at Untamed's offsets. Every call they make out is hooked and logged; the port's calls
//! must be the same, in the same order, and every word of the bike and its body the same after.
//!
//! Without the image the test skips (and says so).

use game_sim::air::{self, Air, Chassis, Params};
use game_sim::handling::untamed::{self, Call, PutBack, WreckWords};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Where the harness puts the bike and its pieces: the top of RAM, under the interpreter's stack.
const OBJ: u32 = 0x01c0_0000;
const BODY: u32 = 0x01c0_2000;
const DRIVER: u32 = 0x01c0_6000;
/// What the put-back's ground and attachment words point at: only handed on to hooks, never read.
const GROUND: u32 = 0x01c0_6800;
const ATTACHMENT: u32 = 0x01c0_6900;
const REGIONS: [(&str, u32, u32); 3] = [("bike", OBJ, 0x1800), ("body", BODY, 0x200), ("driver", DRIVER, 0x100)];

/// SLUS-21701's bike and rigid-body vtables, the functions under test, and what they call out to:
/// the clock's event post, `rand`, the vehicle's own wreck entry, the ground query, the vehicle's
/// settle and the attachment's put-back.
const BIKE_VTABLE: u32 = 0x003f_1d10;
const BODY_VTABLE: u32 = 0x003e_f6a0;
const WRECK_ENTRY: u32 = 0x004f_a390;
const PRELOAD: u32 = 0x004f_e360;
const PUT_BACK: u32 = 0x004c_a900;
const POST_EVENT: u32 = 0x0025_e740;
const RAND: u32 = 0x0034_4360;
const BASE_WRECK: u32 = 0x004c_bc60;
const GROUND_QUERY: u32 = 0x004b_0910;
const SETTLE: u32 = 0x004c_8880;
const ATTACHMENT_PUT_BACK: u32 = 0x0051_2e90;

fn image() -> Option<Ee> {
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
    if ee.u32_at(BIKE_VTABLE + 0x368) != WRECK_ENTRY {
        panic!("{} is not the MX vs. ATV Untamed image these addresses are from", path.display());
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
fn get_v(ee: &Ee, a: u32) -> Vf {
    Vf(std::array::from_fn(|k| f32::from_bits(ee.u32_at(a + 4 * k as u32))))
}

fn reset(ee: &mut Ee) {
    for (_, base, len) in REGIONS {
        for o in (0..len).step_by(4) {
            ee.set_u32(base + o, 0);
        }
    }
    ee.set_u32(OBJ + 0x4, BIKE_VTABLE);
    ee.set_u32(OBJ + 0xe8, BODY);
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
    fn v(&mut self, lo: f32, hi: f32) -> Vf {
        Vf::new(self.f(lo, hi).0, self.f(lo, hi).0, self.f(lo, hi).0, self.f(lo, hi).0)
    }
}

/// Every tuning zero: the generator sets the ones the preload reads.
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

/// The event post, logged as its receiver and the four words after.
fn hook_post(ee: &mut Ee) -> Rc<RefCell<Vec<[u32; 5]>>> {
    let posts = Rc::new(RefCell::new(Vec::<[u32; 5]>::new()));
    let log = posts.clone();
    ee.hooks.insert(POST_EVENT, Box::new(move |ee| {
        log.borrow_mut().push([ee.r(5) as u32, ee.r(6) as u32, ee.r(7) as u32, ee.r(8) as u32, ee.r(9) as u32]);
    }));
    posts
}

// --- the preload --------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct PreloadState {
    p: Params,
    air: Air,
    ch: Chassis,
    st: air::Step,
    /// The analogue preload, when an action is bound to it.
    axis: Option<F>,
    cue: bool,
    /// The bike's `+0xd10`, the event's fourth argument.
    tag: u32,
}

impl Gen {
    fn preload(&mut self) -> PreloadState {
        let mut s = PreloadState {
            p: zero_params(),
            air: Air::default(),
            ch: Chassis::default(),
            st: air::Step::default(),
            axis: None,
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

        s.axis = match self.bits() % 3 {
            0 => None,
            _ => Some(match self.bits() % 5 {
                0 => F(0.001),
                1 => F::ZERO,
                2 => self.f(-0.5, 0.0),
                3 => self.f(1.0, 1.5),
                _ => self.f(0.0, 1.0),
            }),
        };

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
    put_f(ee, OBJ + 0xc4c, p.preload_time);
    put_f(ee, OBJ + 0xc54, p.min_preload_time);
    put_f(ee, OBJ + 0xc58, p.max_preload_time);
    put_f(ee, OBJ + 0xc5c, p.max_preload);
    put_f(ee, OBJ + 0x1150, p.min_dpad_preload_time);
    put_f(ee, OBJ + 0x1154, p.max_dpad_preload_time);

    put_b(ee, OBJ + 0xc40, a.preload_ready);
    put_b(ee, OBJ + 0xc44, a.preload_firing);
    put_b(ee, OBJ + 0xc48, a.preload_armed);
    put_f(ee, OBJ + 0xc50, a.preload_timer);
    put_v(ee, OBJ + 0xc60, a.preload_force);
    put_b(ee, OBJ + 0x354, a.on_ground);
    put_v(ee, OBJ + 0xc20, a.contact_vel);
    put_b(ee, OBJ + 0x724, s.cue);
    ee.set_u32(OBJ + 0xd10, s.tag);

    put_v(ee, BODY + 0x80, c.vel);
    put_v(ee, BODY + 0xa0, c.force);

    put_f(ee, OBJ + 0x340, st.brake);
    put_f(ee, OBJ + 0x129c, st.stick.1);
    put_f(ee, OBJ + 0x12a0, st.stick_hold);
    put_b(ee, OBJ + 0x1294, st.dpad_y);
    put_b(ee, OBJ + 0x12c8, s.axis.is_some());
    put_f(ee, OBJ + 0x12b4, s.axis.unwrap_or(F::ZERO));
}

/// Which way a state went, from the disassembly's branches and not the port's.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn preload_path(s: &PreloadState) -> &'static str {
    let (a, st) = (&s.air, &s.st);
    let lip = a.on_ground && s.ch.vel.y() > 5.0 && a.contact_vel.z() > 30.0 && st.brake == 0.0;
    if a.preload_firing {
        if a.preload_timer - st.dt > F::ZERO { "firing" } else { "fired out" }
    } else if !lip {
        "off the lip"
    } else if s.axis.is_some_and(|x| x > F(0.001)) {
        if st.stick.1 > 0.8 { "axis fired, then armed" } else { "axis fired" }
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

/// SLUS-21701's preload, the port's [`untamed::preload`]: 20,000 states over every path, the analogue
/// preload bound or not, the cue's event post hooked and compared with the port's answer.
#[test]
fn untamed_preload_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let posts = hook_post(&mut ee);
    let mut g = Gen(0x0d93_0001);
    let mut tally = BTreeMap::new();
    let mut cues = 0;
    for k in 0..20_000 {
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
        let posted = untamed::preload(&t.p, &mut t.air, &mut t.ch, &t.st, t.axis, &mut t.cue);
        reset(&mut ee);
        poke_preload(&mut ee, &t);
        compare(k, path, &was, &snap(&ee), &s);
        let want: Vec<[u32; 5]> = if posted { vec![[OBJ + 0x698, 8, untamed::PRELOAD_EVENT, s.tag, 0]] } else { vec![] };
        assert_eq!(*posts.borrow(), want, "state {k} ({path}): the cue's event");
        cues += u32::from(posted);
    }
    enough(
        &tally,
        &["firing", "fired out", "off the lip", "axis fired", "axis fired, then armed", "armed", "fired", "fired, equal times", "holding", "waiting"],
        100,
    );
    assert!(cues >= 1000, "only {cues} cue(s) posted");
}

// --- the wreck entry ----------------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct EntryState {
    air: Air,
    w: WreckWords,
    angular: Vf,
    reason: u32,
    /// What the three draws of `rand` answer.
    draws: [i32; 3],
}

impl Gen {
    fn draw(&mut self) -> i32 {
        match self.bits() % 8 {
            0 => i32::MAX,
            1 => 0,
            2 => i32::MIN,
            3 => self.bits() as i32,
            _ => (self.bits() >> 1) as i32,
        }
    }
    fn entry(&mut self) -> EntryState {
        let mut air = Air::default();
        air.preload_firing = self.b();
        air.preload_ready = self.b();
        air.preload_armed = self.b();
        EntryState {
            air,
            w: WreckWords { hold: self.bits(), entered: self.b(), reasoned: self.b(), count: if self.one_in(10) { u32::MAX } else { self.bits() % 50 } },
            angular: self.v(-5.0, 5.0),
            reason: match self.bits() % 3 {
                0 => untamed::SPIN_REASON,
                1 => 0,
                _ => self.bits() % 20,
            },
            draws: [self.draw(), self.draw(), self.draw()],
        }
    }
}

fn poke_entry(ee: &mut Ee, s: &EntryState) {
    put_b(ee, OBJ + 0xc40, s.air.preload_ready);
    put_b(ee, OBJ + 0xc44, s.air.preload_firing);
    put_b(ee, OBJ + 0xc48, s.air.preload_armed);
    ee.set_u32(OBJ + 0xdd8, s.w.hold);
    put_b(ee, OBJ + 0x1714, s.w.entered);
    put_b(ee, OBJ + 0x17c4, s.w.reasoned);
    ee.set_u32(OBJ + 0x708, s.w.count);
    put_v(ee, BODY + 0x1d0, s.angular);
}

/// Where a draw of `rand` falls among the calls, in both logs (no event is posted with this id).
const DRAW: Call = Call::Post(u32::MAX);

/// SLUS-21701's wreck entry (vt `+0x368`), the port's [`untamed::wreck_entry`]: 10,000 entries, a third
/// of them reason 13's random spin. Its calls (the four virtuals, `rand`, the vehicle's own entry) are
/// hooked and logged with their arguments.
#[test]
fn untamed_wreck_entry_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let calls = Rc::new(RefCell::new(Vec::<Call>::new()));
    let draws = Rc::new(RefCell::new(Vec::<i32>::new()));
    let slot = |ee: &Ee, at: u32| ee.u32_at(BIKE_VTABLE + at);
    let hooked: [(u32, fn(&Ee) -> Call); 5] = [
        (slot(&ee, 0x4a0), |_| Call::EndStunts),
        (slot(&ee, 0x4a8), |ee| {
            assert_eq!(ee.r(5) as i64, -1, "the clip's stop is handed -1");
            Call::StopClip
        }),
        (slot(&ee, 0x398), |ee| {
            assert_eq!(ee.r(5), 0, "the physics node's switch is handed 0");
            Call::PhysicsOff
        }),
        (slot(&ee, 0x484), |_| Call::WreckRays),
        (BASE_WRECK, |ee| Call::BaseWreck(ee.r(5) as u32)),
    ];
    for (at, what) in hooked {
        let log = calls.clone();
        ee.hooks.insert(at, Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, OBJ, "called on the bike");
            log.borrow_mut().push(what(ee));
        }));
    }
    let pool = draws.clone();
    let log = calls.clone();
    ee.hooks.insert(RAND, Box::new(move |ee| {
        let v = pool.borrow_mut().remove(0);
        log.borrow_mut().push(DRAW);
        ee.set(2, v as i64 as u64);
    }));
    let mut g = Gen(0x0d93_0002);
    let mut tally = BTreeMap::new();
    for k in 0..10_000 {
        let s = g.entry();
        let path = if s.reason == untamed::SPIN_REASON { "spun" } else if s.reason == 0 { "no reason" } else { "a reason" };
        *tally.entry(path).or_insert(0u32) += 1;
        reset(&mut ee);
        poke_entry(&mut ee, &s);
        calls.borrow_mut().clear();
        *draws.borrow_mut() = s.draws.to_vec();
        ee.call(WRECK_ENTRY, &[u64::from(OBJ), u64::from(s.reason)], &[], 10_000);
        only_touched_the_regions(&ee).unwrap_or_else(|e| panic!("state {k}: {e}\n{s:#?}"));
        let was = snap(&ee);
        let got = calls.borrow().clone();

        let mut t = s.clone();
        let port = RefCell::new(Vec::new());
        let mut next = s.draws.into_iter();
        untamed::wreck_entry(&mut t.air, &mut t.w, &mut t.angular, t.reason, || {
            port.borrow_mut().push(DRAW);
            next.next().unwrap()
        }, |c| port.borrow_mut().push(c));
        let port = port.into_inner();
        reset(&mut ee);
        poke_entry(&mut ee, &t);
        compare(k, path, &was, &snap(&ee), &s);
        assert_eq!(got, port, "state {k} ({path}): the calls");
    }
    enough(&tally, &["spun", "no reason", "a reason"], 1000);
}

// --- the put-back -------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct PutBackState {
    pb: PutBack,
    snap: bool,
    keep: bool,
    /// What the ground query leaves in the position and answers as the normal.
    ground_position: Vf,
    normal: Vf,
    tag: u32,
}

impl Gen {
    fn word(&mut self) -> u32 {
        if self.one_in(3) { 0 } else { self.bits() }
    }
    fn put_back(&mut self) -> PutBackState {
        let pb = PutBack {
            position: self.v(-500.0, 500.0),
            orientation: self.v(-1.0, 1.0),
            up: self.v(-1.0, 1.0),
            saved_position: self.v(-500.0, 500.0),
            saved_orientation: self.v(-1.0, 1.0),
            saved_up: self.v(-1.0, 1.0),
            respawn: [self.word(), self.word(), self.word()],
            driver_respawn: if self.one_in(4) { None } else { Some([self.bits(), self.bits(), self.bits()]) },
            hold: self.bits(),
            saved_hold: self.bits(),
            has_ground: self.bits() % 4 != 0,
            has_attachment: self.b(),
        };
        PutBackState {
            pb,
            snap: self.b(),
            keep: self.b(),
            ground_position: self.v(-500.0, 500.0),
            normal: self.v(-1.0, 1.0),
            tag: self.bits(),
        }
    }
}

fn poke_put_back(ee: &mut Ee, s: &PutBackState) {
    let pb = &s.pb;
    put_v(ee, OBJ + 0x80, pb.position);
    put_v(ee, OBJ + 0x90, pb.orientation);
    put_v(ee, OBJ + 0xa0, pb.up);
    put_v(ee, OBJ + 0xd30, pb.saved_position);
    put_v(ee, OBJ + 0xd40, pb.saved_orientation);
    put_v(ee, OBJ + 0xd50, pb.saved_up);
    for (i, w) in pb.respawn.iter().enumerate() {
        ee.set_u32(OBJ + 0xd60 + 4 * i as u32, *w);
    }
    if let Some(d) = pb.driver_respawn {
        ee.set_u32(OBJ + 0x6a4, DRIVER);
        for (i, w) in d.iter().enumerate() {
            ee.set_u32(DRIVER + 0x94 + 4 * i as u32, *w);
        }
    }
    ee.set_u32(OBJ + 0xdd8, pb.hold);
    ee.set_u32(OBJ + 0xddc, pb.saved_hold);
    ee.set_u32(OBJ + 0x3e4, if pb.has_ground { GROUND } else { 0 });
    ee.set_u32(OBJ + 0x10cc, if pb.has_attachment { ATTACHMENT } else { 0 });
    ee.set_u32(OBJ + 0xd10, s.tag);
}

/// SLUS-21701's put-back (the vehicle's, which the bike's override calls), the port's
/// [`untamed::put_back`]: 10,000 states over snap, keep, ground, driver and attachment. Its calls are
/// hooked; the ground query writes a generated position and normal, as the port's `ground` does.
#[test]
fn untamed_put_back_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let posts = hook_post(&mut ee);
    let calls = Rc::new(RefCell::new(Vec::<Call>::new()));
    let ground = Rc::new(RefCell::new((Vf::ZERO, Vf::ZERO)));
    let slot = |ee: &Ee, at: u32| ee.u32_at(BIKE_VTABLE + at);
    let hooked: [(u32, fn(&Ee) -> Call); 3] = [
        (SETTLE, |_| Call::Settle),
        (slot(&ee, 0x39c), |_| Call::RideRays),
        (slot(&ee, 0x3a0), |_| Call::KeptRays),
    ];
    for (at, what) in hooked {
        let log = calls.clone();
        ee.hooks.insert(at, Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, OBJ, "called on the bike");
            log.borrow_mut().push(what(ee));
        }));
    }
    let log = calls.clone();
    ee.hooks.insert(ATTACHMENT_PUT_BACK, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, ATTACHMENT, "the attachment's put-back is handed the attachment");
        log.borrow_mut().push(Call::Attachment);
    }));
    let log = calls.clone();
    let answer = ground.clone();
    ee.hooks.insert(GROUND_QUERY, Box::new(move |ee| {
        assert_eq!((ee.r(4) as u32, ee.r(5) as u32, ee.r(7), ee.r(8)), (GROUND, OBJ + 0x80, 0, 0), "the ground query's arguments");
        let (at, normal) = *answer.borrow();
        put_v(ee, OBJ + 0x80, at);
        put_v(ee, ee.r(6) as u32, normal);
        log.borrow_mut().push(Call::Ground);
    }));
    let mut g = Gen(0x0d93_0003);
    let mut tally = BTreeMap::new();
    for k in 0..10_000 {
        let s = g.put_back();
        let path = match (s.snap && s.pb.has_ground, s.pb.driver_respawn.is_some() && s.pb.respawn[0] != 0 && s.pb.respawn[1] != 0) {
            (true, true) => "snapped, respawn handed",
            (true, false) => "snapped",
            (false, true) => "saved, respawn handed",
            (false, false) => "saved",
        };
        *tally.entry(path).or_insert(0u32) += 1;
        *tally.entry(if s.keep { "kept" } else { "not kept" }).or_insert(0u32) += 1;
        *tally.entry(if s.pb.has_attachment { "attachment" } else { "none" }).or_insert(0u32) += 1;
        *ground.borrow_mut() = (s.ground_position, s.normal);
        reset(&mut ee);
        poke_put_back(&mut ee, &s);
        calls.borrow_mut().clear();
        posts.borrow_mut().clear();
        ee.call(PUT_BACK, &[u64::from(OBJ), u64::from(s.snap), u64::from(s.keep)], &[], 10_000);
        only_touched_the_regions(&ee).unwrap_or_else(|e| panic!("state {k}: {e}\n{s:#?}"));
        let was = snap(&ee);
        let mut got = calls.borrow().clone();
        got.extend(posts.borrow().iter().map(|p| {
            assert_eq!(*p, [OBJ + 0x698, 8, untamed::PUT_BACK_EVENT, s.tag, 0], "state {k}: the event's words");
            Call::Post(p[2])
        }));

        let mut t = s.clone();
        let mut port = Vec::new();
        untamed::put_back(&mut t.pb, t.snap, t.keep, |at| {
            *at = s.ground_position;
            s.normal
        }, |c| port.push(c));
        reset(&mut ee);
        poke_put_back(&mut ee, &t);
        compare(k, path, &was, &snap(&ee), &s);
        assert_eq!(got, port, "state {k} ({path}): the calls");
        // The bike's up after a snap is the normal the ground answered, w and all.
        if s.snap && s.pb.has_ground {
            assert_eq!(get_v(&ee, OBJ + 0xa0).to_bits(), s.normal.to_bits());
        }
    }
    enough(&tally, &["snapped, respawn handed", "snapped", "saved, respawn handed", "saved", "kept", "not kept", "attachment", "none"], 500);
}
