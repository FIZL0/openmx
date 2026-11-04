//! MD93 — MX vs. ATV Untamed's (SLUS-21701) reworked bike functions, proved against Untamed's own code.
//!
//! MD81a found six of the bike step's functions rewritten from SLUS-21104's (research/untamed-machines.md).
//! These are the [`handling::untamed`] ports so far: the preload, the wreck entry, the put-back, the backflip (MD93a), the
//! 360 (MD93b) and the vehicle update (MD93c),
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

// --- the backflip (MD93a) -----------------------------------------------------------------------------

/// The backflip's pieces beyond the bike and its body: the bike's frame node (`+0x114`), its collision
/// record (`+0xf8`) and the controls' binding table (`+0x1510`).
const FRAME: u32 = 0x01c0_2400;
const COLL: u32 = 0x01c0_2600;
const BINDING: u32 = 0x01c0_2700;
const FLIP_REGIONS: [(&str, u32, u32); 5] =
    [("bike", OBJ, 0x1800), ("body", BODY, 0x200), ("frame", FRAME, 0x100), ("collision", COLL, 0x40), ("binding", BINDING, 0x40)];

/// The backflip and its callees: the rider's stunt start and end (vt +0x4b0, +0x4a4), the wreck's
/// steep test, and the binding's "is it pressed" (its table read and the clip store run as they are).
const BACKFLIP: u32 = 0x004f_d0d0;
const START_STUNT: u32 = 0x004f_e710;
const END_STUNT: u32 = 0x004f_cf60;
const STEEP: u32 = 0x0050_0880;
const BINDING_PRESSED: u32 = 0x0025_2640;

#[derive(Clone, Debug)]
struct FlipState {
    p: Params,
    air: Air,
    ch: Chassis,
    st: air::Step,
    inp: untamed::FlipIn,
    clip: game_sim::handling::FlipClip,
    /// The four trick buttons as the reader keeps them (bytes +18, +20, +22, +24 of the pad record).
    buttons: [u8; 4],
    /// The pad record's device word and the binding table's byte for it (non-zero: the backflip is a
    /// bound control on this device).
    device: u32,
    bound: u8,
    flip_stunt: u32,
    rate_increase: F,
    start_answer: bool,
    steep_answer: bool,
}

#[derive(Default, Debug)]
struct FlipCalls {
    start_answer: bool,
    steep_answer: bool,
    pressed_answer: bool,
    started: Vec<(u32, u32)>,
    ended: Vec<u32>,
    steep: u32,
    pressed: u32,
    wrecks: Vec<u32>,
}

impl Gen {
    fn flip(&mut self) -> FlipState {
        let mut s = FlipState {
            p: zero_params(),
            air: Air::default(),
            ch: Chassis::default(),
            st: air::Step::default(),
            inp: untamed::FlipIn::default(),
            clip: game_sim::handling::FlipClip::default(),
            buttons: [0; 4],
            device: 0,
            bound: 0,
            flip_stunt: self.bits() % 64,
            rate_increase: self.f(0.0, 1000.0),
            start_answer: self.b(),
            steep_answer: self.b(),
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
        c.moment = self.v(-500.0, 500.0);
        c.pitch = if self.one_in(8) { -s.p.airborne_pitch_down_wreck_angle } else { self.f(-2.0, 2.0) };
        c.roll = if self.one_in(6) { F(1.5707964) } else { self.f(-3.2, 3.2) };

        let st = &mut s.st;
        st.dt = self.f(0.005, 0.04);
        st.brake = if self.one_in(4) { self.f(0.0, 1.0) } else { F::ZERO };
        st.stick = (F::ZERO, if self.one_in(8) { F::ZERO } else { self.f(-1.2, 1.2) });
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
        // Each button held a third of the time, with any non-zero byte for held.
        for b in s.buttons.iter_mut() {
            *b = if self.one_in(3) { 1 + (self.bits() % 255) as u8 } else { 0 };
        }
        s.clip = game_sim::handling::FlipClip((self.bits() % 3) as i32);
        s.device = self.bits() % 4;
        s.bound = if self.one_in(4) { 1 + (self.bits() % 255) as u8 } else { 0 };
        s.inp = untamed::FlipIn {
            arm_button: s.buttons[3] != 0,
            trick_held: s.buttons.iter().any(|&b| b != 0),
            lip_y: match self.bits() % 5 {
                0 => F(if self.b() { 0.8 } else { -0.8 }),
                1 => F(if self.b() { 0.81 } else { -0.81 }),
                _ => self.f(-1.2, 1.2),
            },
            hold: self.f(-70.0, 70.0),
            button: if s.bound != 0 { Some(self.b()) } else { None },
            right_y: if self.one_in(8) { F::ZERO } else { self.f(-1.0, 1.0) },
            up_y: if self.one_in(8) { F::ZERO } else if self.one_in(6) { F(-0.79) } else { self.f(-1.0, 1.0) },
            stunt_id: match self.bits() % 4 {
                0 => 48,
                1 => 49,
                _ => (self.bits() % 80) as i32,
            },
        };
        s
    }
}

fn poke_flip(ee: &mut Ee, s: &FlipState) {
    ee.set_u32(OBJ + 0xf8, COLL);
    ee.set_u32(OBJ + 0x114, FRAME);
    ee.set_u32(OBJ + 0x1510, BINDING);

    let (p, a, c, st, inp) = (&s.p, &s.air, &s.ch, &s.st, &s.inp);
    put_f(ee, OBJ + 0x1194, p.min_backflip_time);
    put_f(ee, OBJ + 0x1198, p.max_backflip_time);
    put_f(ee, OBJ + 0x11a0, p.min_backflip_torque);
    put_f(ee, OBJ + 0x11a4, p.max_backflip_torque);
    put_f(ee, OBJ + 0x11a8, s.rate_increase);
    put_f(ee, OBJ + 0x11ac, p.backflip_torque_rate_decrease);
    put_f(ee, OBJ + 0x117c, p.control_authority_time);
    put_f(ee, OBJ + 0x1414, p.airborne_pitch_down_wreck_angle);
    put_f(ee, OBJ + 0x1418, p.airborne_pitch_up_wreck_angle);

    put_b(ee, OBJ + 0x354, a.on_ground);
    put_v(ee, OBJ + 0xc20, a.contact_vel);
    put_f(ee, OBJ + 0x1178, a.authority);
    put_b(ee, OBJ + 0x1184, a.flip_torque_applied);
    put_b(ee, OBJ + 0x1188, a.flipping);
    put_b(ee, OBJ + 0x118c, a.flip_active);
    put_b(ee, OBJ + 0x1190, a.flip_armed);
    put_f(ee, OBJ + 0x119c, a.flip_grace);
    put_v(ee, OBJ + 0x11b0, a.flip_torque);

    put_v(ee, BODY + 0x80, c.vel);
    put_v(ee, BODY + 0x1b0, c.moment);
    put_f(ee, OBJ + 0x390, c.pitch);
    put_f(ee, OBJ + 0x398, c.roll);

    put_f(ee, OBJ + 0x340, st.brake);
    put_f(ee, OBJ + 0x129c, st.stick.1);
    put_b(ee, OBJ + 0x11c0, st.spinning);
    put_b(ee, OBJ + 0x11c8, st.spin_active);
    put_b(ee, COLL + 0x34, st.touching);

    put_f(ee, OBJ + 0x12c4, inp.lip_y);
    put_f(ee, OBJ + 0x12c0, inp.hold);
    ee.set_u32(OBJ + 0x14f4, s.device);
    for (k, &b) in s.buttons.iter().enumerate() {
        ee.write(OBJ + 0x14f4 + 18 + 2 * k as u32, 1, u128::from(b));
    }
    ee.write(BINDING + 36 + s.device, 1, u128::from(s.bound));
    put_f(ee, FRAME + 0xb4, inp.right_y);
    put_f(ee, FRAME + 0xc4, inp.up_y);
    ee.set_u32(OBJ + 0x1548, inp.stunt_id as u32);
    ee.set_u32(OBJ + 0x1794, s.clip.0 as u32);
    ee.set_u32(OBJ + 0x13c8, s.flip_stunt);
}

fn flip_reset(ee: &mut Ee) {
    for (_, base, len) in FLIP_REGIONS {
        for o in (0..len).step_by(4) {
            ee.set_u32(base + o, 0);
        }
    }
    reset(ee);
}

fn flip_snap(ee: &Ee) -> Vec<u32> {
    FLIP_REGIONS.iter().flat_map(|&(_, base, len)| (0..len).step_by(4).map(move |o| base + o)).map(|a| ee.u32_at(a)).collect()
}

fn flip_where(i: usize) -> String {
    let mut i = i as u32 * 4;
    for (name, _, len) in FLIP_REGIONS {
        if i < len {
            return format!("{name} + {i:#x}");
        }
        i -= len;
    }
    "past the end".into()
}

/// Which way a state went, from the disassembly's branches and not the port's.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn flip_path(s: &FlipState) -> &'static str {
    let (a, st, inp) = (&s.air, &s.st, &s.inp);
    if !a.flip_active {
        let lip = a.on_ground && !st.spinning && !st.spin_active && !(s.ch.vel.y() <= 5.0) && !(a.contact_vel.z() <= 30.0) && st.brake == 0.0;
        if !lip {
            "off the lip"
        } else if s.bound != 0 {
            if inp.button == Some(true) { "bound, pressed" } else { "bound, not pressed" }
        } else if inp.lip_y < F(-0.8) && s.buttons[3] != 0 {
            "arming"
        } else if !(inp.lip_y <= F(0.8)) && a.flip_armed {
            "triggered"
        } else {
            "waiting"
        }
    } else if !a.flip_armed {
        if a.on_ground { "in the grace" } else { "off the lip in time" }
    } else if a.on_ground || st.touching || F(a.flip_torque.x()).abs() < s.p.min_backflip_torque {
        if s.clip.0 != 0 { "ended mid-clip" } else { "ended" }
    } else if s.buttons.iter().all(|&b| b == 0) && st.stick.1 <= 0.0 {
        "turning, decaying"
    } else if inp.right_y <= 0.0 {
        "turning, right side down"
    } else {
        "turning, right side up"
    }
}

#[test]
fn untamed_backflip_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let calls = Rc::new(RefCell::new(FlipCalls::default()));
    let c = calls.clone();
    ee.hooks.insert(START_STUNT, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, OBJ);
        let mut c = c.borrow_mut();
        c.started.push((ee.r(5) as u32, ee.r(6) as u32));
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
    let c = calls.clone();
    ee.hooks.insert(BINDING_PRESSED, Box::new(move |ee| {
        assert_eq!((ee.r(4) as u32, ee.r(5) as u32), (BINDING, OBJ + 0x14f4), "the binding's arguments");
        let mut c = c.borrow_mut();
        c.pressed += 1;
        ee.set(2, u64::from(c.pressed_answer));
    }));

    let mut g = Gen(0x0d93_a001);
    let mut tally = BTreeMap::new();
    for k in 0..20_000 {
        let s = g.flip();
        let path = flip_path(&s);
        *tally.entry(path).or_insert(0u32) += 1;
        flip_reset(&mut ee);
        poke_flip(&mut ee, &s);
        *calls.borrow_mut() = FlipCalls {
            start_answer: s.start_answer,
            steep_answer: s.steep_answer,
            pressed_answer: s.inp.button == Some(true),
            ..FlipCalls::default()
        };
        ee.call(BACKFLIP, &[u64::from(OBJ)], &[s.st.dt.0], 20_000);
        let frame = (ee.stack - 0x2000, ee.stack + 0x40);
        for (a, b) in ee.written_ranges() {
            let inside = FLIP_REGIONS.iter().any(|&(_, base, len)| a >= base && b <= base + len) || (a >= frame.0 && b <= frame.1);
            assert!(inside, "state {k}: the original wrote {a:#010x}..{b:#010x}, which is not the bike or the stack\n{s:#?}");
        }
        let was = flip_snap(&ee);
        let orig = std::mem::take(&mut *calls.borrow_mut());

        let mut t = s.clone();
        let (mut started, mut steeped) = (false, false);
        let out = untamed::backflip(&t.p, &mut t.air, &mut t.ch, &t.st, &t.inp, &mut t.clip, || {
            started = true;
            s.start_answer
        }, || {
            steeped = true;
            s.steep_answer
        });
        if out.stunt_cleared {
            t.inp.stunt_id = 255;
        }
        flip_reset(&mut ee);
        poke_flip(&mut ee, &t);
        let now = flip_snap(&ee);
        if was != now {
            let i = was.iter().zip(&now).position(|(a, b)| a != b).unwrap();
            panic!(
                "state {k} ({path}): {} is {:#010x} ({}) in the original and {:#010x} ({}) in the port\n{s:#?}",
                flip_where(i),
                was[i],
                f32::from_bits(was[i]),
                now[i],
                f32::from_bits(now[i]),
            );
        }
        let want_started: Vec<(u32, u32)> = if started { vec![(s.flip_stunt, 1)] } else { vec![] };
        assert_eq!(orig.started, want_started, "state {k} ({path}): the stunt start");
        assert_eq!(out.stunt_started, started && s.start_answer, "state {k}");
        let want_ended: Vec<u32> = if out.stunt_ended { vec![s.flip_stunt] } else { vec![] };
        assert_eq!(orig.ended, want_ended, "state {k} ({path}): the stunt end");
        assert_eq!(orig.steep, u32::from(steeped), "state {k} ({path}): the steep test");
        assert_eq!(orig.wrecks, out.wreck.map(u32::from).into_iter().collect::<Vec<_>>(), "state {k} ({path}): the wreck");
        // The binding is asked only on a lip with the backflip bound.
        assert_eq!(orig.pressed, u32::from(path.starts_with("bound")), "state {k} ({path}): the binding asked");
    }
    enough(
        &tally,
        &[
            "off the lip",
            "bound, pressed",
            "bound, not pressed",
            "arming",
            "triggered",
            "waiting",
            "in the grace",
            "off the lip in time",
            "ended",
            "ended mid-clip",
            "turning, decaying",
            "turning, right side down",
            "turning, right side up",
        ],
        100,
    );
}

/// MD93a's wiring: [`untamed::ride_backflip`] reads the ride's trick buttons as Untamed's reader keeps
/// them. R1 with another trick button arms the flip on a lip, where SLUS-21104's wants R1 alone. The
/// rider's side (the clip, the stunt id, the mid-clip wreck) is as [`game_sim::handling::ride_backflip`]'s.
#[test]
fn untamed_ride_backflip_arms_on_r1_with_another_button() {
    use game_sim::chassis::Mat3;
    use game_sim::handling::{Flip, ride_backflip};
    let mut p = zero_params();
    p.min_backflip_torque = F(500.0);
    p.max_backflip_torque = F(1500.0);
    p.max_backflip_time = F(30.0);
    let lip = || {
        let mut air = Air::default();
        air.on_ground = true;
        air.contact_vel = Vf::new(0.0, 0.0, 40.0, 0.0);
        let ch = Chassis { vel: Vf::new(0.0, 8.0, 30.0, 0.0), ..Chassis::default() };
        (air, ch)
    };
    let st = air::Step { stick: (F::ZERO, F(-1.0)), ..air::Step::default() };
    let level = Mat3::new(Vf::new(1.0, 0.0, 0.0, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0), Vf::ZERO);
    let both = [true, false, true];
    let (mut air, mut ch) = lip();
    untamed::ride_backflip(&p, &mut air, &mut ch, &st, both, &level, &mut Flip::default(), || panic!("steep"));
    assert!(air.flip_armed, "Untamed arms on R1 with another button");
    let (mut air, mut ch) = lip();
    ride_backflip(&p, &mut air, &mut ch, &st, both, &level, &mut Flip::default(), || panic!("steep"));
    assert!(!air.flip_armed, "SLUS-21104 does not");

    // Over on its back with the torque in: the clip starts, and the stunt is the flip's with no trick held.
    let over = Mat3::new(Vf::new(1.0, -0.1, 0.0, 0.0), Vf::new(0.0, 0.5, 0.0, 0.0), Vf::ZERO);
    for trick_held in [false, true] {
        let mut air = Air::default();
        air.flip_active = true;
        air.flip_armed = true;
        air.flip_torque = Vf::new(-1000.0, 0.0, 0.0, 0.0);
        let (mut ch, mut f) = (Chassis::default(), Flip::default());
        let st = air::Step { stunt: trick_held, ..air::Step::default() };
        untamed::ride_backflip(&p, &mut air, &mut ch, &st, [false; 3], &over, &mut f, || panic!("steep"));
        assert_eq!(f.clip, 1, "the clip starts, trick held {trick_held}");
        assert_eq!(f.stunt, !trick_held, "the stunt id is the flip's only with no trick held");
        for steep in [false, true] {
            let (mut air, mut ch, mut f) = (air, ch, f);
            air.on_ground = true;
            untamed::ride_backflip(&p, &mut air, &mut ch, &air::Step::default(), [false; 3], &over, &mut f, || steep);
            assert!(f.wrecked && f.struck == steep && f.clip == 0 && !f.stunt, "{f:?}");
        }
    }
    assert_eq!(untamed::stunt_stand_in(false, true, false, false), 49);
    assert_eq!(untamed::stunt_stand_in(false, false, false, false), 54);
    assert_eq!(untamed::stunt_stand_in(false, false, false, true), 0xff);
    assert_eq!(untamed::stunt_stand_in(true, true, false, false), 0);
}

// --- the 360 (MD93b) ----------------------------------------------------------------------------------

/// The 360 and what it calls beyond the backflip's: the frame node's axes (hooked: it answers the
/// forward axis), and the bound control's "pressed left" and "pressed right".
const SPIN: u32 = 0x004f_d630;
const NODE_AXES: u32 = 0x0019_0580;
const BINDING_LEFT: u32 = 0x0025_2690;
const BINDING_RIGHT: u32 = 0x0025_26e0;
/// Where the game keeps its arc cosine table and its lookup scale.
const ACOS_PTR: u32 = 0x003f_f538;
const ACOS_SCALE: u32 = 0x003f_f550;

#[derive(Clone, Debug)]
struct SpinState {
    t: air::SpinTuning,
    p: Params,
    air: Air,
    spin: air::Spin,
    ch: Chassis,
    st: air::Step,
    inp: untamed::SpinIn,
    /// The four trick buttons, the device and the binding byte, as the backflip's [`FlipState`].
    buttons: [u8; 4],
    device: u32,
    bound: u8,
    /// The binding pressed left and right (only asked when bound; right only when not left).
    left: bool,
    right: bool,
    /// The two 360 stunt words (`+0x13cc` for a direction under 1, `+0x13d0` else).
    stunts: [u32; 2],
    start_answer: bool,
    steep_answer: bool,
}

#[derive(Default, Debug)]
struct SpinCalls {
    forward: Vf,
    start_answer: bool,
    steep_answer: bool,
    left_answer: bool,
    right_answer: bool,
    axes: u32,
    left: u32,
    right: u32,
    started: Vec<(u32, u32)>,
    ended: Vec<u32>,
    steep: u32,
    wrecks: Vec<u32>,
}

impl Gen {
    fn spin(&mut self) -> SpinState {
        let min_time = if self.one_in(4) { F::ZERO } else { self.f(0.0, 30.0) };
        let mut s = SpinState {
            t: air::SpinTuning {
                min_time,
                max_time: if self.one_in(8) { min_time } else { self.f(10.0, 80.0) },
                min_torque_x: self.f(-200.0, 900.0),
                max_torque_x: self.f(300.0, 1400.0),
                min_torque_y: self.f(200.0, 1200.0),
                max_torque_y: self.f(600.0, 2000.0),
                rate_increase: self.f(0.0, 1000.0),
                rate_decrease: self.f(0.0, 3000.0),
            },
            p: zero_params(),
            air: Air::default(),
            spin: air::Spin::default(),
            ch: Chassis::default(),
            st: air::Step::default(),
            inp: untamed::SpinIn::default(),
            buttons: [0; 4],
            device: self.bits() % 4,
            bound: 0,
            left: self.one_in(3),
            right: self.one_in(3),
            stunts: [self.bits() % 64, self.bits() % 64],
            start_answer: self.b(),
            steep_answer: self.b(),
        };
        s.p.control_authority_time = self.f(0.0, 2.0);

        let a = &mut s.air;
        a.on_ground = self.b();
        a.flip_torque_applied = self.one_in(8);
        a.flip_active = self.one_in(8);
        a.authority = self.f(-0.5, 2.0);
        a.contact_vel = Vf::new(self.f(-5.0, 5.0).0, self.f(-5.0, 5.0).0, self.near(30.0, 20.0, 60.0).0, 0.0);

        let sp = &mut s.spin;
        sp.applied = self.b();
        sp.ramp = self.b();
        sp.active = self.b();
        sp.armed = self.b();
        sp.grace = self.near(0.0, -0.1, 0.95);
        sp.dir = match self.bits() % 8 {
            0 => 0,
            1 => 2,
            k => if k % 2 == 0 { 1 } else { -1 },
        };
        sp.start = self.f(0.0, 6.2831855);
        let yaw = if self.one_in(6) { s.t.min_torque_y } else { self.f(0.0, 2000.0) };
        sp.torque = Vf::new(self.f(-1400.0, 1400.0).0, (if self.b() { -yaw } else { yaw }).0, self.f(-50.0, 50.0).0, self.f(-2.0, 2.0).0);
        sp.clip = (self.bits() % 3) as u8;

        let c = &mut s.ch;
        c.vel = Vf::new(self.f(-10.0, 10.0).0, self.near(5.0, -5.0, 15.0).0, self.f(-40.0, 40.0).0, 0.0);
        c.moment = self.v(-500.0, 500.0);

        let st = &mut s.st;
        st.dt = self.f(0.005, 0.04);
        st.brake = if self.one_in(4) { self.f(0.0, 1.0) } else { F::ZERO };
        st.touching = self.one_in(6);

        // Half the idle states are on a lip that takes a 360, the branches that arm and fire it.
        if !s.spin.active && self.b() {
            s.air.on_ground = true;
            s.air.flip_torque_applied = false;
            s.air.flip_active = false;
            s.st.brake = F::ZERO;
            s.ch.vel = Vf::new(s.ch.vel.x(), self.f(5.5, 15.0).0, s.ch.vel.z(), 0.0);
            s.air.contact_vel = Vf::new(s.air.contact_vel.x(), s.air.contact_vel.y(), self.f(30.5, 60.0).0, 0.0);
        }
        // Each button held a third of the time, with any non-zero byte for held.
        for b in s.buttons.iter_mut() {
            *b = if self.one_in(3) { 1 + (self.bits() % 255) as u8 } else { 0 };
        }
        // Half the turning states are in the air with nothing held, the branch that decays the torque.
        if s.spin.active && s.spin.armed && self.b() {
            s.air.on_ground = false;
            s.buttons = [0; 4];
        }
        s.bound = if self.one_in(4) { 1 + (self.bits() % 255) as u8 } else { 0 };
        let stick = |g: &mut Gen| match g.bits() % 6 {
            0 => F(if g.b() { 0.7 } else { -0.7 }),
            1 => F(if g.b() { 0.71 } else { -0.71 }),
            2 => F::ZERO,
            _ => g.f(-1.2, 1.2),
        };
        let lip_x = stick(self);
        // With the 360 on, a third of the sticks are back at centre, where the torque does not decay.
        let x = if s.spin.active && self.one_in(3) { F::ZERO } else { stick(self) };
        // The forward axis: any way round, sometimes along an axis or upright.
        let forward = match self.bits() % 8 {
            0 => Vf::new(0.0, 1.0, 0.0, 0.0),
            1 => Vf::new(0.0, self.f(-1.0, 1.0).0, if self.b() { 1.0 } else { -1.0 }, 0.0),
            2 => Vf::new(if self.b() { 1.0 } else { -1.0 }, self.f(-1.0, 1.0).0, 0.0, 0.0),
            _ => Vf::new(self.f(-1.0, 1.0).0, self.f(-1.0, 1.0).0, self.f(-1.0, 1.0).0, 0.0),
        };
        s.inp = untamed::SpinIn {
            arm_button: s.buttons[3] != 0,
            trick_held: s.buttons.iter().any(|&b| b != 0),
            lip_x,
            x,
            hold: self.f(-70.0, 70.0),
            button: (s.bound != 0).then_some(if s.left { -1 } else if s.right { 1 } else { 0 }),
            forward,
            stunt_id: match self.bits() % 4 {
                0 => 48,
                1 => 49,
                _ => (self.bits() % 80) as i32,
            },
        };
        s
    }
}

fn poke_spin(ee: &mut Ee, s: &SpinState) {
    ee.set_u32(OBJ + 0xf8, COLL);
    ee.set_u32(OBJ + 0x114, FRAME);
    ee.set_u32(OBJ + 0x1510, BINDING);

    let (t, a, sp, c, st, inp) = (&s.t, &s.air, &s.spin, &s.ch, &s.st, &s.inp);
    put_f(ee, OBJ + 0x11d0, t.min_time);
    put_f(ee, OBJ + 0x11d4, t.max_time);
    put_f(ee, OBJ + 0x11dc, t.min_torque_x);
    put_f(ee, OBJ + 0x11e0, t.max_torque_x);
    put_f(ee, OBJ + 0x11e4, t.min_torque_y);
    put_f(ee, OBJ + 0x11e8, t.max_torque_y);
    put_f(ee, OBJ + 0x11ec, t.rate_increase);
    put_f(ee, OBJ + 0x11f0, t.rate_decrease);
    put_f(ee, OBJ + 0x117c, s.p.control_authority_time);

    put_b(ee, OBJ + 0x354, a.on_ground);
    put_v(ee, OBJ + 0xc20, a.contact_vel);
    put_f(ee, OBJ + 0x1178, a.authority);
    put_b(ee, OBJ + 0x1184, a.flip_torque_applied);
    put_b(ee, OBJ + 0x118c, a.flip_active);

    put_b(ee, OBJ + 0x11c0, sp.applied);
    put_b(ee, OBJ + 0x11c4, sp.ramp);
    put_b(ee, OBJ + 0x11c8, sp.active);
    put_b(ee, OBJ + 0x11cc, sp.armed);
    put_f(ee, OBJ + 0x11d8, sp.grace);
    put_v(ee, OBJ + 0x1200, sp.torque);
    put_f(ee, OBJ + 0x1210, sp.start);
    ee.set_u32(OBJ + 0x1214, sp.dir as u32);
    ee.set_u32(OBJ + 0x1798, u32::from(sp.clip));

    put_v(ee, BODY + 0x80, c.vel);
    put_v(ee, BODY + 0x1b0, c.moment);
    put_f(ee, OBJ + 0x340, st.brake);
    put_b(ee, COLL + 0x34, st.touching);

    put_f(ee, OBJ + 0x12bc, inp.lip_x);
    put_f(ee, OBJ + 0x1298, inp.x);
    put_f(ee, OBJ + 0x12b8, inp.hold);
    ee.set_u32(OBJ + 0x14f4, s.device);
    for (k, &b) in s.buttons.iter().enumerate() {
        ee.write(OBJ + 0x14f4 + 18 + 2 * k as u32, 1, u128::from(b));
    }
    ee.write(BINDING + 36 + s.device, 1, u128::from(s.bound));
    ee.set_u32(OBJ + 0x1548, inp.stunt_id as u32);
    ee.set_u32(OBJ + 0x13cc, s.stunts[0]);
    ee.set_u32(OBJ + 0x13d0, s.stunts[1]);
}

/// Which way a state went, from the disassembly's branches and not the port's.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn spin_path(s: &SpinState) -> &'static str {
    let (a, sp, st, inp) = (&s.air, &s.spin, &s.st, &s.inp);
    let sign = |x: F| if x < F::ZERO { -1 } else { 1 };
    if !sp.active {
        let lip = a.on_ground
            && !a.flip_torque_applied
            && !a.flip_active
            && !(s.ch.vel.y() <= 5.0)
            && !(a.contact_vel.z() <= 30.0)
            && st.brake == 0.0;
        if !lip {
            "off the lip"
        } else if s.bound != 0 {
            if s.left {
                "bound, left"
            } else if s.right {
                "bound, right"
            } else {
                "bound, neither"
            }
        } else if sp.armed && !(inp.lip_x.abs() <= F(0.7)) && sign(inp.lip_x) == sp.dir {
            "fired"
        } else if s.buttons[3] != 0 && inp.lip_x < F(-0.7) {
            "arming left"
        } else if s.buttons[3] != 0 && !(inp.lip_x <= F(0.7)) {
            "arming right"
        } else {
            "waiting"
        }
    } else if !sp.armed {
        if a.on_ground { "in the grace" } else { "off the lip in time" }
    } else if a.on_ground || st.touching || F(sp.torque.y()).abs() < s.t.min_torque_y {
        if sp.clip != 0 { "ended mid-clip" } else { "ended" }
    } else if s.buttons.iter().all(|&b| b == 0) && inp.x != 0.0 && sign(inp.x) != sp.dir {
        "turning, decaying"
    } else if s.buttons.iter().all(|&b| b == 0) && inp.x == 0.0 {
        "turning, stick centred"
    } else {
        "turning"
    }
}

#[test]
fn untamed_spin_is_the_originals() {
    let Some(mut ee) = image() else { return };
    // The table and scale the original reads are the ones the game built at startup: they must be the port's.
    let table = air::AcosTable::build();
    assert_eq!(ee.f32_at(ACOS_SCALE), 14999.5, "the arc cosine table's scale");
    let at = ee.u32_at(ACOS_PTR);
    for (i, &e) in table.entries().iter().enumerate() {
        assert_eq!(ee.f32_at(at + 4 * i as u32).to_bits(), e.to_bits(), "arc cosine entry {i}");
    }
    let calls = Rc::new(RefCell::new(SpinCalls::default()));
    let c = calls.clone();
    ee.hooks.insert(NODE_AXES, Box::new(move |ee| {
        assert_eq!((ee.r(4) as u32, ee.r(5)), (FRAME, 0), "the node's arguments");
        let mut c = c.borrow_mut();
        c.axes += 1;
        put_v(ee, ee.r(6) as u32, c.forward);
        put_v(ee, ee.r(7) as u32, Vf::new(0.0, 1.0, 0.0, 0.0));
    }));
    let c = calls.clone();
    ee.hooks.insert(START_STUNT, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, OBJ);
        let mut c = c.borrow_mut();
        c.started.push((ee.r(5) as u32, ee.r(6) as u32));
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
    let c = calls.clone();
    ee.hooks.insert(BINDING_LEFT, Box::new(move |ee| {
        assert_eq!((ee.r(4) as u32, ee.r(5) as u32), (BINDING, OBJ + 0x14f4), "the binding's arguments");
        let mut c = c.borrow_mut();
        c.left += 1;
        ee.set(2, u64::from(c.left_answer));
    }));
    let c = calls.clone();
    ee.hooks.insert(BINDING_RIGHT, Box::new(move |ee| {
        assert_eq!((ee.r(4) as u32, ee.r(5) as u32), (BINDING, OBJ + 0x14f4), "the binding's arguments");
        let mut c = c.borrow_mut();
        c.right += 1;
        ee.set(2, u64::from(c.right_answer));
    }));

    let mut g = Gen(0x0d93_b001);
    let mut tally = BTreeMap::new();
    for k in 0..20_000 {
        let s = g.spin();
        let path = spin_path(&s);
        *tally.entry(path).or_insert(0u32) += 1;
        flip_reset(&mut ee);
        poke_spin(&mut ee, &s);
        *calls.borrow_mut() = SpinCalls {
            forward: s.inp.forward,
            start_answer: s.start_answer,
            steep_answer: s.steep_answer,
            left_answer: s.left,
            right_answer: s.right,
            ..SpinCalls::default()
        };
        ee.call(SPIN, &[u64::from(OBJ)], &[s.st.dt.0], 20_000);
        let frame = (ee.stack - 0x2000, ee.stack + 0x40);
        for (a, b) in ee.written_ranges() {
            let inside = FLIP_REGIONS.iter().any(|&(_, base, len)| a >= base && b <= base + len) || (a >= frame.0 && b <= frame.1);
            assert!(inside, "state {k}: the original wrote {a:#010x}..{b:#010x}, which is not the bike or the stack\n{s:#?}");
        }
        let was = flip_snap(&ee);
        let orig = std::mem::take(&mut *calls.borrow_mut());

        let mut t = s.clone();
        let (mut started, mut steeped) = (None, false);
        let out = untamed::spin(&t.t, &t.p, &mut t.air, &mut t.spin, &mut t.ch, &t.st, &t.inp, &table, |dir| {
            started = Some(dir);
            s.start_answer
        }, || {
            steeped = true;
            s.steep_answer
        });
        if out.stunt_cleared {
            t.inp.stunt_id = 255;
        }
        flip_reset(&mut ee);
        poke_spin(&mut ee, &t);
        let now = flip_snap(&ee);
        if was != now {
            let i = was.iter().zip(&now).position(|(a, b)| a != b).unwrap();
            panic!(
                "state {k} ({path}): {} is {:#010x} ({}) in the original and {:#010x} ({}) in the port\n{s:#?}",
                flip_where(i),
                was[i],
                f32::from_bits(was[i]),
                now[i],
                f32::from_bits(now[i]),
            );
        }
        let word = |dir: i32| s.stunts[usize::from(dir >= 1)];
        let want_started: Vec<(u32, u32)> = started.map(|d| (word(d), 1)).into_iter().collect();
        assert_eq!(orig.started, want_started, "state {k} ({path}): the stunt start");
        assert_eq!(out.stunt_started, started.is_some() && s.start_answer, "state {k}");
        let want_ended: Vec<u32> = if out.stunt_ended { vec![word(s.spin.dir)] } else { vec![] };
        assert_eq!(orig.ended, want_ended, "state {k} ({path}): the stunt end");
        assert_eq!(orig.steep, u32::from(steeped), "state {k} ({path}): the steep test");
        assert_eq!(orig.wrecks, out.wreck.map(u32::from).into_iter().collect::<Vec<_>>(), "state {k} ({path}): the wreck");
        // The node is asked on every call with the 360 on; the binding only on a lip with the 360 bound,
        // right only when left is not pressed.
        assert_eq!(orig.axes, u32::from(s.spin.active), "state {k} ({path}): the node asked");
        let bound = path.starts_with("bound");
        assert_eq!((orig.left, orig.right), (u32::from(bound), u32::from(bound && !s.left)), "state {k} ({path}): the binding asked");
        for clip_path in [(s.spin.clip, t.spin.clip, out.stunt_started, out.stunt_ended)] {
            let name = match clip_path {
                (0, 1, true, _) => Some("clip started"),
                (1, 2, ..) => Some("clip playing"),
                (2, 0, _, true) => Some("clip came round"),
                _ => None,
            };
            if let Some(n) = name {
                *tally.entry(n).or_insert(0u32) += 1;
            }
        }
    }
    enough(
        &tally,
        &[
            "off the lip",
            "bound, left",
            "bound, right",
            "bound, neither",
            "fired",
            "arming left",
            "arming right",
            "waiting",
            "in the grace",
            "off the lip in time",
            "ended",
            "ended mid-clip",
            "turning, decaying",
            "turning, stick centred",
            "turning",
            "clip started",
            "clip playing",
            "clip came round",
        ],
        100,
    );
}

/// MD93b's wiring: [`untamed::ride_spin`] arms on a lip with R1 and another trick button held, where
/// MX vs. ATV's ([`air::three_sixty`], R1 alone) does not; in the air with the stick against the turn
/// Untamed's torque decays only with no trick button held, and a landing mid-clip is the 360's wreck as
/// on the pack ride.
#[test]
fn untamed_ride_spin_arms_on_r1_with_another_button() {
    let table = air::AcosTable::build();
    let p = zero_params();
    let t = air::SpinTuning {
        min_time: F::ZERO,
        max_time: F(30.0),
        min_torque_x: F(600.0),
        max_torque_x: F(1100.0),
        min_torque_y: F(500.0),
        max_torque_y: F(1500.0),
        rate_increase: F::ZERO,
        rate_decrease: F(1000.0),
    };
    let lip = || {
        let mut air = Air::default();
        air.on_ground = true;
        air.contact_vel = Vf::new(0.0, 0.0, 40.0, 0.0);
        let ch = Chassis { vel: Vf::new(0.0, 8.0, 30.0, 0.0), ..Chassis::default() };
        (air, ch)
    };
    let st = air::Step { stick: (F(-1.0), F::ZERO), heading: Vf::new(0.0, 0.0, 1.0, 0.0), ..air::Step::default() };
    let both = [true, false, true];
    let (mut air, mut ch) = lip();
    let mut s = air::Spin::default();
    untamed::ride_spin(&t, &p, &mut air, &mut s, &mut ch, &st, both, F(10.0), &table);
    assert!(s.armed && s.dir == 1, "Untamed arms on R1 with another button: {s:?}");
    let (mut air, mut ch) = lip();
    let mut s = air::Spin::default();
    let mxvatv = air::Step { modifier: false, ..st };
    air::three_sixty(&t, &p, &mut air, &mut s, &mut ch, &mxvatv, F(10.0), &table);
    assert!(!s.armed, "MX vs. ATV's does not");

    // Turning in the air, the stick against the turn: a trick button held keeps the torque, none decays it.
    let turning = air::Spin { active: true, armed: true, dir: -1, torque: Vf::new(-700.0, -1000.0, 0.0, 0.0), ..air::Spin::default() };
    let against = air::Step { dt: F(0.01), stick: (F(1.0), F::ZERO), heading: Vf::new(0.0, 0.0, 1.0, 0.0), ..air::Step::default() };
    let mut s = turning;
    let (mut air, mut ch) = (Air::default(), Chassis::default());
    untamed::ride_spin(&t, &p, &mut air, &mut s, &mut ch, &against, [true, false, false], F::ZERO, &table);
    assert_eq!(s.torque.to_bits(), turning.torque.to_bits(), "a button held: no decay");
    assert!(s.applied && s.ramp);
    assert_eq!(ch.moment.y(), -1000.0, "the torque went in");
    let mut s = turning;
    let (mut air, mut ch) = (Air::default(), Chassis::default());
    untamed::ride_spin(&t, &p, &mut air, &mut s, &mut ch, &against, [false; 3], F::ZERO, &table);
    assert!(s.torque.y() > -1000.0 && s.torque.x() < -700.0, "nothing held: the torque decays, {:?}", s.torque);

    // Half way round with the clip playing and then down: the 360's wreck, not cut short in the air.
    let mut s = air::Spin { clip: 2, ..turning };
    let mut air = Air { on_ground: true, ..Air::default() };
    untamed::ride_spin(&t, &p, &mut air, &mut s, &mut ch, &against, [false; 3], F::ZERO, &table);
    assert!(s.wrecked && !s.cut && s.clip == 0 && !s.active, "{s:?}");
}

// --- the vehicle update (MD93c) -----------------------------------------------------------------------

/// SLUS-21701's bike vehicle update, the vehicle's own (base) update, and the physics listeners' step.
const VEHICLE_UPDATE: u32 = 0x004f_9bd0;
const BASE_UPDATE: u32 = 0x004c_b340;
const LISTENERS: u32 = 0x0012_6c50;
/// Where the game keeps the pointer to the flag record whose byte `+0x11d` bit 3 skips the rider's half.
const FLAGS_PTR: u32 = 0x003f_f6b8;
/// The harness's vehicle state, the record the timer's object hangs off (`+0x30`), the object, its
/// vtable, and the address its `+0x90` slot points at (only ever hooked).
const VSTATE: u32 = 0x01c0_6a00;
const TIMED: u32 = 0x01c0_6a40;
const TIMED_OBJ: u32 = 0x01c0_6a80;
const TIMED_VT: u32 = 0x01c0_6ac0;
const TIMER_OUT: u32 = 0x01c0_6b80;
/// The `dt` a hooked call is handed (`$f12`).
fn dt(ee: &Ee) -> F {
    F(ee.f(12))
}
const UPDATE_REGIONS: [(&str, u32, u32); 2] = [("bike", OBJ, 0x1800), ("vehicle state", VSTATE, 0x40)];

#[derive(Clone, Debug)]
struct UpdateState {
    u: untamed::Update,
    dt: F,
    /// What the hooked calls change, so the reads after them are checked: the ghost the vehicle state
    /// (`Some(None)` takes it away), the wreck test and the listeners the countdown, the stunt idle and
    /// the node the timer, the stunt clocks the reason, the remote wreck the request, the timer's object
    /// the timer, and the vehicle's own update the cleared word.
    ghost: Option<Option<u32>>,
    countdown: Option<i32>,
    timer: Option<F>,
    reason: Option<u32>,
    request: Option<u32>,
    timer_out: F,
    cleared: u32,
}

/// What a hooked call does to the bike, the same on both sides.
fn update_mutate(c: untamed::UpdateCall, u: &mut untamed::Update, s: &UpdateState) {
    use untamed::UpdateCall as C;
    match c {
        C::Ghost(_) => {
            if let Some(k) = s.ghost {
                u.vehicle_state = k;
            }
        }
        C::WreckTest(_) | C::Listeners(_) => u.request_countdown = s.countdown.unwrap_or(u.request_countdown),
        C::StuntIdle(_) | C::NodeFromBody => u.timer = s.timer.unwrap_or(u.timer),
        C::StuntClocks(_) => u.wreck_reason = s.reason.unwrap_or(u.wreck_reason),
        C::RemoteWreck { .. } => u.request = s.request.unwrap_or(u.request),
        C::TimerOut => u.timer = s.timer_out,
        C::VehicleUpdate(_) => u.cleared = s.cleared,
    }
}

impl Gen {
    fn kind(&mut self) -> u32 {
        [0, 1, 2, 3, 4, 5, u32::MAX][(self.bits() % 7) as usize]
    }
    fn update(&mut self) -> UpdateState {
        let dt = if self.one_in(4) { F(1.0 / 30.0) } else { self.f(0.001, 0.05) };
        let timer = match self.bits() % 6 {
            0 => F::ZERO,
            1 => dt,
            2 => self.f(-1.0, 0.0),
            3 => self.f(0.0, 0.05),
            _ => self.f(0.0, 2.0),
        };
        let countdown = |g: &mut Gen| match g.bits() % 8 {
            0 => i32::MIN,
            1 => i32::MAX,
            2 => 1,
            _ => (g.bits() % 7) as i32 - 3,
        };
        let u = untamed::Update {
            vehicle_state: if self.one_in(10) { None } else { Some(self.kind()) },
            skip: self.one_in(5),
            stick: (self.f(-1.0, 1.0), self.f(-1.0, 1.0)),
            stick_angle: self.f(-4.0, 4.0),
            stick_length: self.f(0.0, 1.5),
            brake: self.f(0.0, 1.0),
            input: self.bits(),
            wreck_reason: self.bits() % 20,
            request_countdown: countdown(self),
            request: if self.b() { self.bits() % 2 } else { self.bits() },
            timer,
            timed: self.b(),
            cleared: self.bits(),
        };
        UpdateState {
            u,
            dt,
            ghost: match self.bits() % 6 {
                0 => Some(None),
                1 => Some(Some(self.kind())),
                _ => None,
            },
            countdown: if self.one_in(3) { Some(countdown(self)) } else { None },
            timer: if self.one_in(3) { Some(self.f(-0.1, 0.1)) } else { None },
            reason: if self.one_in(3) { Some(self.bits() % 20) } else { None },
            request: if self.one_in(3) { Some(self.bits()) } else { None },
            timer_out: self.f(-1.0, 1.0),
            cleared: self.bits(),
        }
    }
}

fn update_reset(ee: &mut Ee) {
    for (_, base, len) in UPDATE_REGIONS {
        for o in (0..len).step_by(4) {
            ee.set_u32(base + o, 0);
        }
    }
    ee.set_u32(OBJ + 0x4, BIKE_VTABLE);
    ee.set_u32(OBJ + 0x17b8, TIMED);
    ee.set_u32(TIMED_OBJ + 0x4, TIMED_VT);
    ee.set_u32(TIMED_VT + 0x90, TIMER_OUT);
}

/// The words a hooked call may change (see [`update_mutate`]).
fn poke_update_mutable(ee: &mut Ee, u: &untamed::Update) {
    match u.vehicle_state {
        Some(k) => {
            ee.set_u32(OBJ + 0xbb4, VSTATE);
            ee.set_u32(VSTATE + 0x28, k);
        }
        None => {
            ee.set_u32(OBJ + 0xbb4, 0);
            ee.set_u32(VSTATE + 0x28, 0);
        }
    }
    ee.set_u32(OBJ + 0x17b0, u.request_countdown as u32);
    put_f(ee, OBJ + 0x17bc, u.timer);
    ee.set_u32(OBJ + 0xc70, u.wreck_reason);
    ee.set_u32(OBJ + 0x34c, u.request);
    ee.set_u32(OBJ + 0x1690, u.cleared);
}

fn peek_update(ee: &Ee) -> untamed::Update {
    let flags = ee.u32_at(FLAGS_PTR) + 0x11d;
    untamed::Update {
        vehicle_state: (ee.u32_at(OBJ + 0xbb4) != 0).then(|| ee.u32_at(VSTATE + 0x28)),
        skip: ee.u8_at(flags) & 8 != 0,
        stick: (F::from_bits(ee.u32_at(OBJ + 0x1298)), F::from_bits(ee.u32_at(OBJ + 0x129c))),
        stick_angle: F::from_bits(ee.u32_at(OBJ + 0x12a8)),
        stick_length: F::from_bits(ee.u32_at(OBJ + 0x12ac)),
        brake: F::from_bits(ee.u32_at(OBJ + 0x340)),
        input: ee.u32_at(OBJ + 0x12d0),
        wreck_reason: ee.u32_at(OBJ + 0xc70),
        request_countdown: ee.i32_at(OBJ + 0x17b0),
        request: ee.u32_at(OBJ + 0x34c),
        timer: F::from_bits(ee.u32_at(OBJ + 0x17bc)),
        timed: ee.u32_at(TIMED + 0x30) != 0,
        cleared: ee.u32_at(OBJ + 0x1690),
    }
}

fn poke_update(ee: &mut Ee, u: &untamed::Update) {
    poke_update_mutable(ee, u);
    put_f(ee, OBJ + 0x1298, u.stick.0);
    put_f(ee, OBJ + 0x129c, u.stick.1);
    put_f(ee, OBJ + 0x12a8, u.stick_angle);
    put_f(ee, OBJ + 0x12ac, u.stick_length);
    put_f(ee, OBJ + 0x340, u.brake);
    ee.set_u32(OBJ + 0x12d0, u.input);
    ee.set_u32(TIMED + 0x30, if u.timed { TIMED_OBJ } else { 0 });
}

fn update_snap(ee: &Ee) -> Vec<u32> {
    UPDATE_REGIONS.iter().flat_map(|&(_, base, len)| (0..len).step_by(4).map(move |o| base + o)).map(|a| ee.u32_at(a)).collect()
}

/// The paths a state goes down, off the original's branches (several per state).
fn update_paths(s: &UpdateState) -> Vec<&'static str> {
    let mut p = Vec::new();
    let Some(_) = s.u.vehicle_state else { return vec!["no state"] };
    if s.u.skip {
        p.push("skipped");
    } else {
        let k = s.ghost.unwrap_or(s.u.vehicle_state);
        match s.ghost {
            Some(None) => p.push("the ghost took the state"),
            Some(Some(_)) => p.push("the ghost changed the kind"),
            None => {}
        }
        p.push(if k.is_some_and(|k| k == 3 || k == 4) { "remote" } else { "local" });
    }
    p
}

/// SLUS-21701's bike vehicle update, the port's [`untamed::vehicle_update`]: 20,000 generated states.
/// Every call it makes is hooked, logged with its arguments, and changes the bike as the port's `call`
/// does ([`update_mutate`]), so the words read after a call must be read after it on both sides.
#[test]
fn untamed_vehicle_update_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let table = air::AcosTable::build();
    assert_eq!(ee.f32_at(ACOS_SCALE), 14999.5, "the arc cosine table's scale");
    let at = ee.u32_at(ACOS_PTR);
    for (i, &e) in table.entries().iter().enumerate() {
        assert_eq!(ee.f32_at(at + 4 * i as u32).to_bits(), e.to_bits(), "arc cosine entry {i}");
    }
    use untamed::UpdateCall as C;
    let calls = Rc::new(RefCell::new(Vec::<C>::new()));
    let current = Rc::new(RefCell::new(None::<UpdateState>));
    let slot = |ee: &Ee, at: u32| ee.u32_at(BIKE_VTABLE + at);
    let hooked: [(u32, u32, fn(&Ee) -> C); 9] = [
        (slot(&ee, 0x3a4), OBJ, |ee| C::Ghost(dt(ee))),
        (slot(&ee, 0x494), OBJ, |ee| {
            assert_eq!(ee.r(5), 0, "the wreck test is handed 0");
            C::WreckTest(dt(ee))
        }),
        (slot(&ee, 0x4e4), OBJ, |ee| C::StuntIdle(dt(ee))),
        (LISTENERS, OBJ + 0x30, |ee| C::Listeners(dt(ee))),
        (slot(&ee, 0x4ec), OBJ, |_| C::NodeFromBody),
        (slot(&ee, 0x4e0), OBJ, |ee| C::StuntClocks(dt(ee))),
        (slot(&ee, 0x498), OBJ, |ee| C::RemoteWreck { dt: dt(ee), reason: ee.r(5) as u32 }),
        (TIMER_OUT, TIMED_OBJ, |_| C::TimerOut),
        (BASE_UPDATE, OBJ, |ee| C::VehicleUpdate(dt(ee))),
    ];
    for (at, a0, what) in hooked {
        let log = calls.clone();
        let s = current.clone();
        ee.hooks.insert(at, Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, a0, "{at:#x} called on {a0:#x}");
            let c = what(ee);
            log.borrow_mut().push(c);
            let mut u = peek_update(ee);
            update_mutate(c, &mut u, s.borrow().as_ref().unwrap());
            poke_update_mutable(ee, &u);
        }));
    }
    let flags = ee.u32_at(FLAGS_PTR) + 0x11d;
    let mut g = Gen(0x0d93_000c);
    let mut tally = BTreeMap::new();
    for k in 0..20_000 {
        let s = g.update();
        let mut paths = update_paths(&s);
        update_reset(&mut ee);
        poke_update(&mut ee, &s.u);
        let byte = ee.u8_at(flags) & !8 | if s.u.skip { 8 } else { 0 };
        ee.write(flags, 1, u128::from(byte));
        *current.borrow_mut() = Some(s.clone());
        calls.borrow_mut().clear();
        ee.call(VEHICLE_UPDATE, &[u64::from(OBJ)], &[s.dt.0], 10_000);
        let frame = (ee.stack - 0x2000, ee.stack + 0x40);
        for (a, b) in ee.written_ranges() {
            let inside = UPDATE_REGIONS.iter().any(|&(_, base, len)| a >= base && b <= base + len) || (a >= frame.0 && b <= frame.1);
            assert!(inside, "state {k}: the original wrote {a:#010x}..{b:#010x}, which is not the bike or the stack\n{s:#?}");
        }
        let was = update_snap(&ee);
        let got = calls.borrow().clone();

        let mut u = s.u;
        let port = RefCell::new(Vec::new());
        untamed::vehicle_update(&mut u, s.dt, &table, |c, u| {
            port.borrow_mut().push(c);
            update_mutate(c, u, &s);
        });
        let port = port.into_inner();
        if port.contains(&C::TimerOut) {
            paths.push("timer out, told");
        } else if s.u.vehicle_state.is_some() && u.timer == F::ZERO && s.u.timer.to_bits() != 0 && !(s.u.timer <= F::ZERO) {
            paths.push("timer out, alone");
        }
        if u.request_countdown == 0 && u.request == 1 && s.u.vehicle_state.is_some() {
            paths.push("request raised");
        }
        for p in &paths {
            *tally.entry(*p).or_insert(0u32) += 1;
        }
        update_reset(&mut ee);
        poke_update(&mut ee, &u);
        let now = update_snap(&ee);
        if was != now {
            let i = was.iter().zip(&now).position(|(a, b)| a != b).unwrap();
            panic!(
                "state {k} ({paths:?}): word {:#x} of the regions is {:#010x} ({}) in the original and {:#010x} ({}) in the port\n{s:#?}",
                4 * i,
                was[i],
                f32::from_bits(was[i]),
                now[i],
                f32::from_bits(now[i]),
            );
        }
        assert_eq!(got, port, "state {k} ({paths:?}): the calls\n{s:#?}");
    }
    enough(
        &tally,
        &["no state", "skipped", "local", "remote", "the ghost took the state", "the ghost changed the kind",
          "timer out, told", "timer out, alone", "request raised"],
        100,
    );
}
