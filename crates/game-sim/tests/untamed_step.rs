//! MD81a — MX vs. ATV Untamed's (SLUS-21701) bike wreck test, proved against Untamed's own code.
//!
//! Untamed keeps MX vs. ATV Unleashed's sixteen `mx_*.phy`; its eight P files raise `[Wrecks]`
//! (front 12.5 → 14, back 15 → 17, side 13 → 14.5, both downs 45/48 → 54). Whether the step that reads
//! them is SLUS-21104's is what this asks, clause by clause, of the wreck test: the oracle is
//! SLUS-21701 off a RAM image of a Free Ride in it (`context/untamed/ram/nat_or.bin`, the 32 MB
//! eeMemory of an Untamed save state; the GAME_REL.BIN overlay is in it). Its wreck test runs in the
//! interpreter with its own vtable, on the bike laid out at Untamed's offsets, and the reasons it
//! enters and the throw it writes must be [`step::wreck_test`]'s then [`step::hit_wreck`]'s on the
//! same state with Untamed's limits.
//!
//! The one change SLUS-21701 made to the clauses is the trick bound: the rider's stunt id is a trick at
//! most 48 (SLUS-21104: 40), Untamed having eight more stunts. The port reads that as
//! [`air::Step::stunt`], which the caller sets, so the generator sets it by 48 here.
//!
//! Without the image the test skips (and says so).

use game_sim::air::{self, Air, Chassis, Params};
use game_sim::handling::step;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Where the harness puts the bike and its pieces: the top of RAM, under the interpreter's stack.
const OBJ: u32 = 0x01c0_0000;
const BODY: u32 = 0x01c0_2000;
const DRIVER: u32 = 0x01c0_6000;
const RECORD: u32 = 0x01c0_6100;
const WRECK_RECORD: u32 = 0x01c0_6400;
const CONTACTS: u32 = 0x01c0_6600;
const STRUCK: u32 = 0x01c0_6c00;
const OTHERS: u32 = 0x01c0_7000;
const REGIONS: [(&str, u32, u32); 2] = [("bike", OBJ, 0x1800), ("body", BODY, 0x200)];

/// SLUS-21701's bike vtable, its wreck entry (vt `+0x368`) and wreck test (vt `+0x494`; the bike's
/// update calls it last, `f12` the frame's time), the "wrecks on" word and the class word reason 15
/// masks the record's object with.
const BIKE_VTABLE: u32 = 0x003f_1d10;
const WRECK_ENTRY: u32 = 0x004f_a390;
const WRECK_TEST: u32 = 0x004f_a5f0;
const WRECKS_ON: u32 = 0x0053_d3c8;
const STRUCK_MASK: u32 = 0x0054_5978;

/// Untamed's highest trick id (SLUS-21104's is 40).
const LAST_TRICK: i32 = 48;

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
    if ee.u32_at(BIKE_VTABLE + 0x368) != WRECK_ENTRY || ee.u32_at(BIKE_VTABLE + 0x494) != WRECK_TEST {
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

fn reset(ee: &mut Ee) {
    for (_, base, len) in REGIONS {
        for o in (0..len).step_by(4) {
            ee.set_u32(base + o, 0);
        }
    }
    ee.set_u32(OBJ + 0x4, BIKE_VTABLE);
    ee.set_u32(OBJ + 0xe8, BODY);
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

/// Every tuning zero: the generator sets the ones the wreck test reads.
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
        // About Untamed's P limits (mph to ft/s, 22/15), sometimes off them.
        p.front_wreck_speed_limit = self.near(20.533, 10.0, 30.0);
        p.back_wreck_speed_limit = self.near(24.933, 10.0, 30.0);
        p.side_wreck_speed_limit = self.near(21.267, 10.0, 30.0);
        p.racing_down_wreck_speed_limit = self.near(79.2, 40.0, 90.0);
        p.freestyle_down_wreck_speed_limit = self.near(79.2, 40.0, 90.0);
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
        // The ids either side of both builds' bounds: 40/41 (a trick here, not on SLUS-21104), 48/49.
        let id = match self.bits() % 8 {
            0 => balance,
            1 => 41,
            2 => 255,
            3 => 40,
            4 => 48,
            5 => 49,
            _ => (self.bits() % 60) as i32,
        };
        let wanted = if self.one_in(2) { id } else if self.b() { 0xfe } else { (self.bits() % 60) as i32 };
        let touching = self.b();
        let up_y = self.near(p.no_hander_max_roll_angle.0, -1.0, 1.0);
        let step = air::Step {
            up: Vf::new(0.0, up_y.0, 0.0, 0.0),
            stunt: id <= LAST_TRICK,
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

/// The state at SLUS-21701's offsets (SLUS-21104's, `tests/mxvatv_step.rs`' `poke_wreck`, are in the
/// MD81a journal beside them).
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
    ee.set_u32(OBJ + 0x4b0, DRIVER);
    ee.set_u32(DRIVER + 0xe4, 1);
    ee.set_u32(OBJ + 0xc0c, 1);
    ee.set_u32(OBJ + 0x338, 0);
    ee.set_u32(OBJ + 0xc70, 0);
    put_f(ee, OBJ + 0xc78, F(100.0));
    let p = &s.p;
    for (o, v) in [
        (0x13f8, p.front_wreck_speed_limit),
        (0x13fc, p.back_wreck_speed_limit),
        (0x1400, p.side_wreck_speed_limit),
        (0x1404, p.racing_down_wreck_speed_limit),
        (0x1408, p.freestyle_down_wreck_speed_limit),
        (0x140c, p.ground_pitch_down_wreck_angle),
        (0x1410, p.ground_pitch_up_wreck_angle),
        (0x1414, p.airborne_pitch_down_wreck_angle),
        (0x1418, p.airborne_pitch_up_wreck_angle),
        (0x141c, p.airborne_roll_wreck_angle),
        (0x1420, p.no_hander_wreck_speed_limit),
        (0x1424, p.no_hander_max_roll_angle),
        (0x1428, p.joust_wreck_height),
        (0x142c, s.loads.landing_crossup),
        (0x1430, p.crossup_wreck),
        (0x1434, p.crossup_wreck_time),
        (0x1240, s.loads.side),
        (0x1244, s.loads.vertical),
        (0x1248, s.loads.fore_aft),
        (0x14ec, s.loads.pose_blend),
        (0x390, s.chassis.pitch),
        (0x398, s.chassis.roll),
        (0x1180, s.air.hang_time),
    ] {
        put_f(ee, OBJ + o, v);
    }
    put_b(ee, OBJ + 0x14d4, s.loads.trick_wrecked);
    put_b(ee, OBJ + 0x147c, s.loads.racing);
    put_b(ee, OBJ + 0x354, s.air.on_ground);
    put_b(ee, OBJ + 0xdd0, s.air.airborne);
    put_b(ee, OBJ + 0xdd4, s.air.was_airborne);
    put_b(ee, OBJ + 0x1184, s.air.flip_torque_applied);
    put_b(ee, OBJ + 0x11c0, s.w.spin_applied);
    ee.set_u32(OBJ + 0x1544, s.id as u32);
    ee.set_u32(OBJ + 0x154c, s.balance as u32);
    ee.set_u32(OBJ + 0x1550, s.wanted as u32);
    put_v(ee, OBJ + 0xbe0, s.step.up);
    put_v(ee, OBJ + 0x1250, s.air.wreck_impact);
    put_v(ee, BODY + 0x80, s.chassis.vel);
    put_f(ee, BODY + 0x74, s.height);
    ee.set_u32(OBJ + 0xbb0, if s.small { 0 } else { 2 });
    ee.set_u32(OBJ + 0xf8, RECORD);
    put_b(ee, RECORD + 0x34, s.step.touching);
    ee.set_u32(RECORD + 0x20c, if s.struck != 0 { STRUCK } else { 0 });
    ee.set_u32(STRUCK + 0xc0, s.struck);
    ee.set_u32(RECORD + 0xf0, s.contacts.len() as u32);
    ee.set_u32(RECORD + 0xf8, CONTACTS);
    for (i, c) in s.contacts.iter().enumerate() {
        let at = CONTACTS + 0x90 * i as u32;
        for o in (0..0x90).step_by(4) {
            ee.set_u32(at + o, 0);
        }
        // The flag is a halfword on SLUS-21701 (a word on SLUS-21104): the low half, little-endian.
        put_b(ee, at, c.mirrored);
        put_f(ee, at + 0x24, c.reach);
        if let air::Other::Rider { height, vel } = c.other {
            let (bike, body) = (OTHERS + 0x200 * i as u32, OTHERS + 0x200 * i as u32 + 0x100);
            ee.set_u32(at + 0x80, bike);
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
        (Some(W::NoHander), _) if (41..=LAST_TRICK).contains(&s.id) => "mid trick 41-48",
        (Some(W::NoHander), _) => "mid trick",
        (Some(w), _) => panic!("{w:?}"),
        (None, Some(W::Joust)) if s.small => "hit from above, small",
        (None, Some(W::Joust)) => "hit from above",
        (None, Some(w)) => panic!("{w:?} from the hits"),
        (None, None) if s.step.touching && !s.contacts.is_empty() => "touched, rides on",
        (None, None) => "rides on",
    }
}

/// SLUS-21701's wreck test on a driven bike not yet down, the port's [`step::wreck_test`] then
/// [`step::hit_wreck`] with the trick bound at 48: 10,000 states through the original with its own
/// vtable, the wreck entry hooked to log the reason and do the record swap its collision call does
/// (the riding record out, an empty one in). The reasons entered, in order, and the throw at `+0x1250`
/// must be the port's. The original may write only the bike, its harness pieces and its stack.
#[test]
fn untamed_wreck_test_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let log = Rc::new(RefCell::new(Vec::new()));
    {
        let log = log.clone();
        ee.hooks.insert(WRECK_ENTRY, Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, OBJ, "the bike");
            let reason = ee.r(5) as u32;
            log.borrow_mut().push(reason);
            ee.set_u32(OBJ + 0xc70, reason);
            ee.set_u32(OBJ + 0xc74, 0);
            ee.set_u32(OBJ + 0xc7c, 0);
            ee.set_u32(OBJ + 0xf8, WRECK_RECORD);
        }));
    }
    let mut g = Gen(0x0d81_a001);
    let mut tally = BTreeMap::new();
    for k in 0..10_000 {
        let s = g.wreck();
        reset(&mut ee);
        poke_wreck(&mut ee, &s);
        log.borrow_mut().clear();
        ee.call(WRECK_TEST, &[u64::from(OBJ)], &[0.0], 20_000);
        let frame = (ee.stack - 0x2000, ee.stack + 0x40);
        for (a, b) in ee.written_ranges() {
            let ok = REGIONS.iter().any(|&(_, base, len)| a >= base && b <= base + len)
                || (a >= frame.0 && b <= frame.1)
                || a == WRECKS_ON
                || a == STRUCK_MASK
                || (a >= DRIVER && b <= OTHERS + 0x1000);
            assert!(ok, "state {k}: the original wrote {a:#010x}..{b:#010x}");
        }
        let got = log.borrow().clone();
        let impact: Vec<u32> = (0..4).map(|i| ee.u32_at(OBJ + 0x1250 + 4 * i)).collect();

        let mut air = s.air.clone();
        let landed = step::wreck_test(&s.p, &mut air, &s.chassis, &s.step, &s.loads, &s.w, true);
        let hit = if landed.is_some() {
            None
        } else {
            step::hit_wreck(&s.p, &mut air, s.step.touching, s.small, s.height, &s.loads, &s.contacts)
        };
        let path = wreck_path(&s, landed, hit);
        let want: Vec<u32> = landed.or(hit).map(|w| w as u32).into_iter().collect();
        assert_eq!(got, want, "state {k} ({path}): the reasons\n{s:#?}");
        let port: Vec<u32> = air.wreck_impact.0.iter().map(|v| v.to_bits()).collect();
        assert_eq!(impact, port, "state {k} ({path}): the throw\n{s:#?}");
        *tally.entry(path).or_insert(0u32) += 1;
    }
    eprintln!("{tally:?}");
    enough(
        &tally,
        &[
            "15 in the air", "15 on the loads", "back", "front", "side", "flat", "ground pitch down", "ground pitch up",
            "nose dive", "airborne pitch up", "torqued touch", "crossup", "landed onto", "no-hander roll", "mid trick",
            "hit from above", "hit from above, small", "touched, rides on", "rides on",
        ],
        30,
    );
    // The ids SLUS-21701 made tricks: a wreck mid trick here, the bike let go on SLUS-21104.
    enough(&tally, &["mid trick 41-48"], 20);
}

/// The same states with SLUS-21104's bound (a trick at most 40): the original must answer otherwise
/// on stunt ids 41-48 mid trick — that the oracle can tell the two builds apart.
#[test]
fn untamed_tricks_41_to_48_are_tricks() {
    let Some(mut ee) = image() else { return };
    let log = Rc::new(RefCell::new(Vec::new()));
    {
        let log = log.clone();
        ee.hooks.insert(WRECK_ENTRY, Box::new(move |ee| {
            log.borrow_mut().push(ee.r(5) as u32);
            ee.set_u32(OBJ + 0xc70, ee.r(5) as u32);
            ee.set_u32(OBJ + 0xf8, WRECK_RECORD);
        }));
    }
    let mut g = Gen(0x0d81_a002);
    let mut differs = 0;
    for _ in 0..4_000 {
        let mut s = g.wreck();
        if !(41..=LAST_TRICK).contains(&s.id) {
            continue;
        }
        reset(&mut ee);
        poke_wreck(&mut ee, &s);
        log.borrow_mut().clear();
        ee.call(WRECK_TEST, &[u64::from(OBJ)], &[0.0], 20_000);
        s.step.stunt = false;
        let mut air = s.air.clone();
        let landed = step::wreck_test(&s.p, &mut air, &s.chassis, &s.step, &s.loads, &s.w, true);
        let hit = if landed.is_some() {
            None
        } else {
            step::hit_wreck(&s.p, &mut air, s.step.touching, s.small, s.height, &s.loads, &s.contacts)
        };
        let old: Vec<u32> = landed.or(hit).map(|w| w as u32).into_iter().collect();
        if *log.borrow() != old {
            differs += 1;
        }
    }
    assert!(differs >= 20, "only {differs} state(s) told SLUS-21701's bound from SLUS-21104's");
}
