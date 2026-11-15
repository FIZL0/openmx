//! MD161 — MX vs. ATV Untamed's (SLUS-21701) monster truck: the two changes of its car step the port
//! follows behind [`atv_car::frame_untamed`], proved against SLUS-21701's own code.
//!
//! - **The body's integrate** (every machine's rigid body, vt `+0x54`): the angular damping is made per
//!   time, `powf(d, 0.333/h)` per lane when that exponent is more than a tenth off 1
//!   ([`atv_body::per_time_damping`], [`atv_body::Body::integrate_untamed`]). MD160's wreck drive shows
//!   it live (×0.85219 a tick in the air from a stored 0.999); here it is held over damping lanes
//!   below 1 and many substeps `h`, the original's powf run as it is.
//! - **The car PostProcess's yaw hold** by two compares ([`atv_car::hold_yaw_rate_untamed`]): run as a
//!   stretch of SLUS-21701's PostProcess, from its start to its gear change, the vehicle post before it
//!   answered empty.
//!
//! The oracle is SLUS-21701's code off a RAM image of a Free Ride in it (`context/untamed/ram/nat_or.bin`,
//! as tests/untamed_car.rs). Without it the tests skip (and say so).

use game_sim::atv_body::{self, Body};
use game_sim::atv_car;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};
use std::collections::BTreeMap;

/// Where the harness puts the car and its body: the top of RAM, under the stack.
const OBJ: u32 = 0x01c0_0000;
const BODY: u32 = 0x01c0_2000;

/// SLUS-21701's body vtable (the truck's body's, MD160's spawn state) and the integrate and
/// end-of-substep in it.
const BODY_VTABLE: u32 = 0x003e_f6a0;
const BODY_INTEGRATE: u32 = 0x004b_c670;
const BODY_END_SUBSTEP: u32 = 0x004c_22b0;
/// The integrate's frame (432 bytes) holds the raised damping at `+0x170`, and its `w` (never written
/// on the powf path) at `+0x17c`: 52 bytes under the caller's stack pointer.
const RAISED_W_BELOW_SP: u32 = 52;

/// SLUS-21701's car PostProcess (car vt, the car's listener), the vehicle post it calls first (a
/// thunk), and the gear change it calls after the yaw hold, where the stretch ends.
const POST_PROCESS: u32 = 0x004e_9790;
const VEHICLE_POST: u32 = 0x004c_8b20;
const SHIFT: u32 = 0x004d_6340;
/// The car's body pointer and yaw-rate limit (Untamed's offsets), the body's rates.
const BODY_AT: u32 = 0xe8;
const YAW_LIMIT: u32 = 0x123c;
const RATES: u32 = 0x1d0;

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
    assert_eq!(
        (ee.u32_at(BODY_VTABLE + 0x54), ee.u32_at(BODY_VTABLE + 0x40)),
        (BODY_INTEGRATE, BODY_END_SUBSTEP),
        "{} is not the MX vs. ATV Untamed image these addresses are from",
        path.display()
    );
    assert_eq!([ee.u32_at(POST_PROCESS), ee.u32_at(POST_PROCESS + 4)], [0x27bd_ff20, 0xffbf_0020], "SLUS-21701's car PostProcess");
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
    fn vec(&mut self, lo: f32, hi: f32) -> Vf {
        Vf::new(self.f(lo, hi).0, self.f(lo, hi).0, self.f(lo, hi).0, self.f(lo, hi).0)
    }
    fn quaternion(&mut self) -> Vf {
        let v = [self.f(-1.0, 1.0).0, self.f(-1.0, 1.0).0, self.f(-1.0, 1.0).0, self.f(-1.0, 1.0).0];
        let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2] + v[3] * v[3]).sqrt().max(1e-3);
        Vf::new(v[0] / n, v[1] / n, v[2] / n, v[3] / n)
    }

    /// A damping lane: mostly under 1 (the wreck's 0.999, a collider's, a ragdoll's), sometimes 1
    /// (every `.phy` of the truck), 0, a signed zero, a denormal, over 1 or negative.
    fn lane(&mut self) -> f32 {
        match self.bits() % 20 {
            0..=2 => 1.0,
            3 => 0.999,
            4 => 0.0,
            5 => -0.0,
            6 => f32::from_bits(1 + self.bits() % 0x007f_ffff),
            7 => self.f(1.0, 1.2).0,
            8 => self.f(-1.0, 0.0).0,
            9 => self.f(0.0, 0.05).0,
            10..=13 => self.f(0.99, 1.0).0,
            _ => self.f(0.0, 1.0).0,
        }
    }

    /// The substep: the game's (1/120, four a frame at 30), other worlds' rates, the band where the
    /// exponent is within a tenth of 1 (0.333/1.1 ..= 0.333/0.9) and its edges, 0, a denormal and under.
    fn h(&mut self) -> F {
        match self.bits() % 16 {
            0..=3 => F(1.0 / 120.0),
            4 => F(1.0 / 115.0),
            5 => F(1.0 / 60.0),
            6 => F(1.0 / 240.0),
            7 => self.f(0.3027, 0.37),
            8 => [F(0.333 / 1.1), F(0.333 / 0.9), F(0.302_727_3), F(0.37), F(0.333)][(self.bits() % 5) as usize],
            9 => [F::ZERO, F(-0.0), F(-1.0 / 120.0), F(f32::from_bits(0x0000_1234))][(self.bits() % 4) as usize],
            10 => self.f(0.3, 2.0),
            _ => self.f(0.001, 0.05),
        }
    }

    /// The body: the angular half's paths as tests/atv_quad.rs's MD75 generator has them, the
    /// damping lanes [`Gen::lane`]'s.
    fn body(&mut self) -> Body {
        let no_limit = F(f32::MAX);
        let limit = |g: &mut Gen, lo: f32, hi: f32| if g.one_in(3) { no_limit } else { g.f(lo, hi) };
        Body {
            enabled: !self.one_in(10),
            linear_axes: [!self.one_in(6), !self.one_in(6), !self.one_in(6)],
            inv_mass: if self.one_in(10) { F::ZERO } else { self.f(0.01, 0.2) },
            damping: if self.b() { Vf::splat(1.0) } else { self.vec(0.9, 1.0) },
            last_position: self.vec(-1000.0, 1000.0),
            position: self.vec(-1000.0, 1000.0),
            velocity: self.vec(-80.0, 80.0),
            last_force: self.vec(-2000.0, 2000.0),
            force: self.vec(-2000.0, 2000.0),
            accel: self.vec(-100.0, 100.0),
            correction_peak: self.vec(-10.0, 10.0),
            correction: self.vec(-10.0, 10.0),
            max_speed: limit(self, 20.0, 120.0),
            max_accel: limit(self, 50.0, 300.0),
            angular_axes: [!self.one_in(6), !self.one_in(6), !self.one_in(6)],
            inv_inertia: if self.one_in(15) {
                Vf::new(0.0, 0.0, 0.0, 1.0)
            } else {
                Vf::new(self.f(0.01, 0.2).0, self.f(0.01, 0.2).0, self.f(0.01, 0.2).0, 1.0)
            },
            ang_damping: Vf::new(self.lane(), self.lane(), self.lane(), if self.one_in(4) { self.f(0.0, 2.0).0 } else { 1.0 }),
            last_quat: self.quaternion(),
            quat: self.quaternion(),
            moment: self.vec(-500.0, 500.0),
            last_moment: self.vec(-500.0, 500.0),
            rates: self.vec(-25.0, 25.0),
            ang_accel: self.vec(-50.0, 50.0),
            twist_peak: self.vec(-10.0, 10.0),
            twist: self.vec(-10.0, 10.0),
            max_rates: [self.f(1.0, 20.0), self.f(1.0, 20.0), self.f(1.0, 20.0)],
            max_ang_accel: limit(self, 5.0, 60.0),
        }
    }
}

fn put_v(ee: &mut Ee, a: u32, v: Vf) {
    for k in 0..4 {
        ee.set_f32(a + 4 * k as u32, v.0[k]);
    }
}

/// The whole body at SLUS-21701's offsets (SLUS-21104's, research/oracle.txt `# MD75`: Untamed's
/// body did not move them, `# MD81c`).
fn put_body(ee: &mut Ee, b: &Body) {
    ee.set_u32(BODY, u32::from(b.enabled));
    for k in 0..3 {
        ee.set_u32(BODY + 0x4 + 4 * k as u32, u32::from(b.linear_axes[k]));
        ee.set_u32(BODY + 0x120 + 4 * k as u32, u32::from(b.angular_axes[k]));
        ee.set_f32(BODY + 0x210 + 4 * k as u32, b.max_rates[k].0);
    }
    ee.set_f32(BODY + 0x1c, b.inv_mass.0);
    for (o, v) in [
        (0x30, b.damping),
        (0x60, b.last_position),
        (0x70, b.position),
        (0x80, b.velocity),
        (0x90, b.last_force),
        (0xa0, b.force),
        (0xb0, b.accel),
        (0xc0, b.correction_peak),
        (0xd0, b.correction),
        (0x140, b.inv_inertia),
        (0x150, b.ang_damping),
        (0x170, b.last_quat),
        (0x190, b.quat),
        (0x1b0, b.moment),
        (0x1c0, b.last_moment),
        (0x1d0, b.rates),
        (0x1e0, b.ang_accel),
        (0x1f0, b.twist_peak),
        (0x200, b.twist),
    ] {
        put_v(ee, BODY + o, v);
    }
    ee.set_f32(BODY + 0xe0, b.max_speed.0);
    ee.set_f32(BODY + 0xe4, b.max_accel.0);
    ee.set_f32(BODY + 0x220, b.max_ang_accel.0);
    ee.set_u32(BODY + 0x110, BODY_VTABLE);
}

fn tally(t: &mut BTreeMap<&'static str, u32>, path: &'static str) {
    *t.entry(path).or_insert(0) += 1;
}

fn each_path_seen(t: &BTreeMap<&'static str, u32>, paths: &[&str], at_least: u32) {
    for want in paths {
        let n = t.get(want).copied().unwrap_or(0);
        assert!(n >= at_least, "only {n} case(s) went down {want:?}: {t:?}");
    }
}

/// Which of the damping's paths `(d, h)` goes down, by the port's own sums (the comparison proves
/// them).
#[allow(clippy::float_arithmetic)]
fn damping_paths(b: &Body, h: F) -> Vec<&'static str> {
    let mut p = Vec::new();
    if b.inv_inertia.to_bits() == Vf::new(0.0, 0.0, 0.0, 1.0).to_bits() {
        p.push("angular pinned");
        return p;
    }
    let d = b.ang_damping;
    let raised = atv_body::per_time_damping(d, h);
    if F::from_bits(h.to_bits()) <= 0.0 {
        p.push("h 0 or under: (1, 1, 1, 1)");
    } else if raised.to_bits() == d.to_bits() && (0.333 / h.0 - 1.0).abs() <= 0.1 {
        p.push("exponent near 1: as it is");
    } else {
        p.push("raised");
        for k in 0..3 {
            let x = d.0[k];
            if x.to_bits() == 0 || x.to_bits() == 0x8000_0000 || x.is_subnormal() {
                p.push("raised: a zero or denormal lane");
            } else if x == 1.0 {
                p.push("raised: a lane of 1");
            } else if x < 0.0 {
                p.push("raised: a negative lane");
            } else if x > 1.0 {
                p.push("raised: a lane over 1");
            } else {
                p.push("raised: a lane under 1");
            }
        }
        if h == F(1.0 / 120.0) && (0..3).any(|k| d.0[k] > 0.0 && d.0[k] < 1.0) {
            p.push("raised at the game's substep");
        }
    }
    // SLUS-21104's plain damping would not give the same rates.
    let (mut old, mut new) = (*b, *b);
    old.integrate(h);
    new.integrate_untamed(h);
    if (0..3).any(|k| old.rates.0[k].to_bits() != new.rates.0[k].to_bits()) {
        p.push("parts from SLUS-21104's");
    }
    p
}

/// **Untamed's body integrate is the port's**: 20,000 bodies and substeps, the original's integrate and
/// end-of-substep (its powf run as it is) against [`Body::integrate_untamed`] and
/// [`Body::end_substep`], every word of the body by bits. On the powf path the rates' `w` is the
/// integrate's own stack word (never written there): the test plants a word of its own there and
/// checks the original carried it, and compares the port's `w` (the damping's) on the other paths.
#[test]
fn untamed_body_integrate_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let mut g = Gen(0x0161_0001);
    let mut t = BTreeMap::new();
    let snap = |ee: &Ee| (0..0x230).step_by(4).map(|o| ee.u32_at(BODY + o)).collect::<Vec<u32>>();
    for k in 0..20_000 {
        let b = g.body();
        let h = g.h();
        let filler: Vec<u32> = (0..0x230 / 4).map(|_| g.bits()).collect();
        let planted = g.bits() | 0x3f00_0000;
        let lay = |ee: &mut Ee, b: &Body| {
            for (i, &w) in filler.iter().enumerate() {
                ee.set_u32(BODY + 4 * i as u32, w);
            }
            put_body(ee, b);
        };
        let paths = damping_paths(&b, h);
        let raised = paths.contains(&"raised");
        for p in &paths {
            tally(&mut t, p);
        }
        lay(&mut ee, &b);
        let slot = ee.stack - RAISED_W_BELOW_SP;
        ee.set_u32(slot, planted);
        let mark = ee.written_ranges().len();
        ee.call(BODY_INTEGRATE, &[u64::from(BODY)], &[h.0], 200_000);
        ee.call(BODY_END_SUBSTEP, &[u64::from(BODY)], &[], 20_000);
        for (a, e) in ee.written_ranges().into_iter().skip(mark) {
            let inside = (a >= BODY && e <= BODY + 0x230) || (a >= ee.stack - 0x2000 && e <= ee.stack + 0x40);
            assert!(inside, "case {k}: the original wrote {a:#010x}..{e:#010x}");
        }
        let mut was = snap(&ee);
        let mut port = b;
        port.integrate_untamed(h);
        port.end_substep();
        lay(&mut ee, &port);
        let now = snap(&ee);
        if raised {
            let w = (RATES + 0xc) as usize / 4;
            assert_eq!(was[w], planted, "case {k}: the raised path's rates w is not the integrate's stack word");
            tally(&mut t, "raised: rates w the stack word");
            was[w] = now[w];
        }
        if let Some(i) = was.iter().zip(&now).position(|(a, b)| a != b) {
            panic!(
                "case {k}: body + {:#x} is {:#010x} ({}) in the original and {:#010x} ({}) in the port\nh {h:?} ({:#010x}), paths {paths:?}\n{b:#?}",
                4 * i,
                was[i],
                f32::from_bits(was[i]),
                now[i],
                f32::from_bits(now[i]),
                h.0.to_bits(),
            );
        }
    }
    eprintln!("MD161 body integrate: {t:?}");
    each_path_seen(
        &t,
        &[
            "angular pinned",
            "h 0 or under: (1, 1, 1, 1)",
            "exponent near 1: as it is",
            "raised",
            "raised: a zero or denormal lane",
            "raised: a lane of 1",
            "raised: a negative lane",
            "raised: a lane over 1",
            "raised: a lane under 1",
            "raised at the game's substep",
            "raised: rates w the stack word",
            "parts from SLUS-21104's",
        ],
        30,
    );
}

/// **MD160's wreck decay is the port's**: a body at rest in the air, its damping the truck's
/// WreckDamping (0.999), four substeps of 1/120 a frame: the rates fall ×0.85219 a frame, as the
/// captured wreck rows 23–44 do (research/journal/2026-11-14-md160-untamed-monster-capture), where
/// SLUS-21104's integrate keeps 0.996 of them.
#[test]
#[allow(clippy::float_arithmetic)]
fn untamed_wreck_damping_falls_as_captured() {
    let mut b = Body { enabled: true, ang_damping: Vf::new(0.999, 0.999, 0.999, 1.0), rates: Vf::new(4.0, -2.0, 1.0, 1.0), quat: Vf::new(0.0, 0.0, 0.0, 1.0), ..Body::default() };
    (b.inv_inertia, b.max_rates, b.max_speed, b.max_accel, b.max_ang_accel) = (Vf::new(0.1, 0.1, 0.1, 1.0), [F(12.0); 3], F(f32::MAX), F(f32::MAX), F(f32::MAX));
    b.angular_axes = [true; 3];
    let mut old = b;
    for _ in 0..4 {
        b.integrate_untamed(F(1.0 / 120.0));
        old.integrate(F(1.0 / 120.0));
    }
    let ratio = b.rates.x() / 4.0;
    assert!((ratio - 0.85219).abs() < 5e-5, "a frame keeps {ratio}"); // 0.999^(4·0.333·120) = 0.852214
    assert!((old.rates.x() / 4.0 - 0.996).abs() < 1e-5);
    assert_eq!(b.rates.w(), 1.0, "the rates keep a w of 1, as the capture's");
}

/// **Untamed's yaw hold is the port's**: 20,000 rates and limits (normal, denormal, negative, signed
/// zeros, the rate past the limit either way) through SLUS-21701's PostProcess from its start to its
/// gear change, against [`atv_car::hold_yaw_rate_untamed`], the body's rates by bits.
#[test]
fn untamed_yaw_hold_is_the_originals() {
    let Some(mut ee) = image() else { return };
    ee.hooks.insert(VEHICLE_POST, Box::new(|_: &mut Ee| {}));
    ee.hooks.insert(SHIFT, Box::new(|ee: &mut Ee| ee.set(31, u64::from(ps2_oracle::DONE) | 0xffff_ffff_0000_0000)));
    let mut g = Gen(0x0161_0002);
    let mut t = BTreeMap::new();
    let mut old_parts = 0;
    for k in 0..20_000 {
        let limit = match g.bits() % 10 {
            0 => F(f32::from_bits(1 + g.bits() % 0x007f_ffff)),
            1 => F(-f32::from_bits(1 + g.bits() % 0x007f_ffff)),
            2 => -g.f(0.1, 5.0),
            3 => [F::ZERO, F(-0.0)][(g.bits() % 2) as usize],
            _ => g.f(0.1, 5.0),
        };
        let y = match g.bits() % 8 {
            0 => limit,
            1 => -limit,
            2 => [F::ZERO, F(-0.0)][(g.bits() % 2) as usize],
            3 => F(f32::from_bits(1 + g.bits() % 0x007f_ffff)),
            _ => g.f(-8.0, 8.0),
        };
        let rates = Vf::new(g.f(-5.0, 5.0).0, y.0, g.f(-5.0, 5.0).0, g.f(0.0, 2.0).0);
        ee.set_u32(OBJ + BODY_AT, BODY);
        ee.set_f32(OBJ + YAW_LIMIT, limit.0);
        put_v(&mut ee, BODY + RATES, rates);
        ee.call(POST_PROCESS, &[u64::from(OBJ)], &[0.0333], 20_000);
        let was: Vec<u32> = (0..4).map(|i| ee.u32_at(BODY + RATES + 4 * i)).collect();
        let mut port = rates;
        atv_car::hold_yaw_rate_untamed(&mut port, limit);
        let now: Vec<u32> = (0..4).map(|i| port.0[i].to_bits()).collect();
        assert_eq!(was, now, "case {k}: rates {rates:?}, limit {limit:?} ({:#010x})", limit.0.to_bits());
        tally(
            &mut t,
            match () {
                _ if limit.0.is_subnormal() => "denormal limit",
                _ if limit.0 < 0.0 => "negative limit",
                _ if was[1] != rates.0[1].to_bits() && was[1] == limit.0.to_bits() => "held at the limit",
                _ if was[1] != rates.0[1].to_bits() => "held at minus the limit",
                _ => "kept",
            },
        );
        // SLUS-21104's hold (the port's private one, by its sum: `limit * sign`).
        let mut old = rates;
        #[allow(clippy::float_arithmetic, clippy::neg_cmp_op_on_partial_ord)]
        if !(F(old.y()).abs() <= limit) {
            old.0[1] = (limit * F(if old.y() < 0.0 { -1.0 } else { 1.0 })).0;
        }
        if old.0[1].to_bits() != port.0[1].to_bits() {
            old_parts += 1;
        }
    }
    eprintln!("MD161 yaw hold: {t:?}, {old_parts} where SLUS-21104's parts");
    each_path_seen(&t, &["denormal limit", "negative limit", "held at the limit", "held at minus the limit", "kept"], 100);
    assert!(old_parts > 100, "the cases never tell the two holds apart");
}
