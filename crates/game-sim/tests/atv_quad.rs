//! MD37b — the ATV's own ground steps, proved against MX vs. ATV's own code.
//!
//! MX Unleashed has no quad, so the oracle is SLUS-21104: a RAM image of MD37a's Desert Oasis Free Ride
//! with the atv_ quad at its spawn (`context/mxvatv-ps2/md37a/atv-r-spawn.ee.bin`) holds the game
//! overlay, so the quad's functions run in the interpreter as they are, with the rigid body's own
//! vtable. The harness is `tests/mxvatv_air.rs`'s: lay a quad, its body, its four wheels and the
//! physics world out in RAM, write a generated state into them, run the original, write the same state
//! again with the port's answer over it, and compare the regions word for word. The original may write
//! nothing else.
//!
//! Without the image every test here skips (and says so).

use game_sim::quad::{self, Body, Params, Quad, Wheel};
use ps2_float::{F, Vf};
use ps2_oracle::{DONE, Ee, Kit, skip};
use std::collections::BTreeMap;

const OBJ: u32 = 0x01c0_0000;
const BODY: u32 = 0x01c0_2000;
const WORLD: u32 = 0x01c0_2400;
const WHEEL: [u32; 4] = [0x01c0_2600, 0x01c0_2800, 0x01c0_2a00, 0x01c0_2c00];
const REGIONS: [(&str, u32, u32); 7] = [
    ("quad", OBJ, 0x1800),
    ("body", BODY, 0x200),
    ("world", WORLD, 0x50),
    ("left front", 0x01c0_2600, 0x140),
    ("right front", 0x01c0_2800, 0x140),
    ("left rear", 0x01c0_2a00, 0x140),
    ("right rear", 0x01c0_2c00, 0x140),
];

/// The quad's and its rigid body's vtables in the image.
const QUAD_VTABLE: u32 = 0x003c_e250;
const BODY_VTABLE: u32 = 0x003c_d200;
/// The body's world-moment entry, which must be the quaternion rotate the port copies.
const BODY_WORLD_MOMENT: u32 = 0x0049_c1a0;
/// The quad's steps, and the place in its Force body the rear-pair flag is set at: from the test after
/// the fourth tire force to the first wheel integrate, which the harness stops at.
const STAND_UPRIGHT: u32 = 0x004b_cee0;
const ROLL_PITCH: u32 = 0x004b_c9e0;
const GROUND_PUSH: u32 = 0x004b_d290;
const REAR_FLAG_FROM: u32 = 0x004b_9768;
const WHEEL_INTEGRATE: u32 = 0x004a_1680;

fn image() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2/md37a").join(format!("atv-r-spawn.ee{}", ".bin"));
    if !path.exists() {
        skip(&format!(
            "{} is not there: it is MD37a's (eeMemory.bin of atv-r-spawn.p2s; memory: md37a ATV states live in s2)",
            path.display()
        ));
        return None;
    }
    let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    if ee.u32_at(BODY_VTABLE + 0x88) != BODY_WORLD_MOMENT || ee.u32_at(0x015e_0694) != QUAD_VTABLE {
        panic!("{} is not the MX vs. ATV quad image these addresses are from", path.display());
    }
    Some(ee)
}

#[derive(Clone, Debug)]
struct State {
    p: Params,
    q: Quad,
    body: Body,
    /// The two balance words: equal is the bike's "balancing".
    bal: (u32, u32),
    dt: F,
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
    ee.set_u32(OBJ + 0x4, QUAD_VTABLE);
    ee.set_u32(OBJ + 0xb8, BODY);
    ee.set_u32(OBJ + 0xe8, WORLD);
    for (k, &w) in WHEEL.iter().enumerate() {
        ee.set_u32(OBJ + 0x17b0 + 4 * k as u32, w);
    }
    ee.set_u32(BODY + 0x110, BODY_VTABLE);
    ee.set_u32(BODY + 0xe8, 0); // no force listeners on the body

    let (p, q, b) = (&s.p, &s.q, &s.body);
    put_f(ee, OBJ + 0x139c, p.max_pitch_command);
    put_f(ee, OBJ + 0x13a0, p.pitch_gain);
    put_f(ee, OBJ + 0x13a4, p.pitch_rate_gain);
    put_f(ee, OBJ + 0x13a8, p.inv_wheelie_top_speed);
    put_f(ee, OBJ + 0x13cc, p.max_stoppie_pitch_command);
    put_f(ee, OBJ + 0x13d0, p.stoppie_pitch_gain);
    put_f(ee, OBJ + 0x13d4, p.stoppie_yaw_rate_scale);
    put_f(ee, OBJ + 0x13d8, p.stoppie_top_speed);

    put_b(ee, OBJ + 0x324, q.on_ground);
    put_b(ee, OBJ + 0x1270, q.all_down);
    put_b(ee, OBJ + 0x1274, q.front_down);
    put_b(ee, OBJ + 0x1278, q.rear_down);
    ee.set_u32(OBJ + 0x1524, s.bal.0);
    ee.set_u32(OBJ + 0x152c, s.bal.1);
    put_f(ee, OBJ + 0xc38, q.speed);
    put_f(ee, OBJ + 0x310, q.brake);
    put_f(ee, OBJ + 0x314, q.gas);
    put_f(ee, OBJ + 0x318, q.gas_limited);
    put_f(ee, OBJ + 0x360, q.pitch);
    put_v(ee, OBJ + 0x340, q.world_ang_vel);
    put_f(ee, OBJ + 0x12cc, q.front_brake);
    put_f(ee, OBJ + 0x12d0, q.rear_brake);
    put_f(ee, OBJ + 0x12d4, q.clutch_in);
    put_f(ee, OBJ + 0x12e8, q.lean);
    put_f(ee, OBJ + 0x12ec, q.lean_fore_aft);
    put_f(ee, OBJ + 0x13ac, q.pitch_integral);
    put_b(ee, OBJ + 0x13b0, q.stoppie);
    put_b(ee, OBJ + 0xdd8, q.push);
    put_f(ee, OBJ + 0x123c, q.push_target);
    put_f(ee, OBJ + 0x1240, q.push_gain);
    put_f(ee, OBJ + 0x12c4, q.push_gate);
    put_v(ee, WORLD + 0x40, q.gravity);
    for (w, &a) in q.wheels.iter().zip(&WHEEL) {
        put_b(ee, a + 0x2c, w.steered);
        put_b(ee, a + 0xb4, w.rear_pair);
        put_b(ee, a + 0xdc, w.touching);
        put_v(ee, a + 0x130, w.normal);
    }

    put_v(ee, BODY + 0xa0, b.force);
    put_v(ee, BODY + 0x1b0, b.moment);
    put_v(ee, BODY + 0x190, b.orientation);
    put_v(ee, BODY + 0x1d0, b.ang_vel);
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
            return Err(format!("the original wrote {a:#010x}..{b:#010x}, which is not the quad or the stack"));
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
    /// Exactly `at` one time in five, else anywhere in `lo..hi`.
    fn near(&mut self, at: f32, lo: f32, hi: f32) -> F {
        if self.one_in(5) { F(at) } else { self.f(lo, hi) }
    }
    /// Zero one time in three, else anywhere in `lo..hi`.
    fn or_zero(&mut self, lo: f32, hi: f32) -> F {
        if self.one_in(3) { F::ZERO } else { self.f(lo, hi) }
    }
    fn vec(&mut self, lo: f32, hi: f32) -> Vf {
        Vf::new(self.f(lo, hi).0, self.f(lo, hi).0, self.f(lo, hi).0, self.f(lo, hi).0)
    }
    /// A ground normal: mostly up, tilted some, `w` near 1.
    fn normal(&mut self) -> Vf {
        let (x, z) = (self.f(-0.4, 0.4).0, self.f(-0.4, 0.4).0);
        let y = (1.0 - x * x - z * z).sqrt();
        Vf::new(x, y, z, self.near(1.0, 0.99, 1.0).0)
    }
    fn quaternion(&mut self) -> Vf {
        let v = [self.f(-1.0, 1.0).0, self.f(-1.0, 1.0).0, self.f(-1.0, 1.0).0, self.f(-1.0, 1.0).0];
        let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2] + v[3] * v[3]).sqrt().max(1e-3);
        Vf::new(v[0] / n, v[1] / n, v[2] / n, v[3] / n)
    }

    fn state(&mut self) -> State {
        let p = Params {
            max_pitch_command: self.f(0.3, 1.2),
            pitch_gain: self.f(0.2, 3.0),
            pitch_rate_gain: self.f(100.0, 1500.0),
            inv_wheelie_top_speed: self.f(0.0, 0.05),
            max_stoppie_pitch_command: self.f(0.5, 2.5),
            stoppie_pitch_gain: self.f(0.5, 3.0),
            stoppie_yaw_rate_scale: self.f(0.0, 2.0),
            stoppie_top_speed: if self.one_in(6) { F::ZERO } else { self.f(5.0, 40.0) },
        };
        let mut wheels = [Wheel::default(); 4];
        for w in &mut wheels {
            *w = Wheel { steered: self.b(), rear_pair: self.b(), touching: !self.one_in(3), normal: self.normal() };
        }
        let mut q = Quad {
            on_ground: !self.one_in(6),
            all_down: !self.one_in(4),
            front_down: !self.one_in(4),
            rear_down: !self.one_in(4),
            balancing: false,
            speed: if self.one_in(10) { F(if self.b() { 5.0 } else if self.b() { 7.0 } else { 15.0 }) } else { self.f(-20.0, 40.0) },
            brake: self.or_zero(0.0, 1.0),
            gas: if self.one_in(2) { F::ZERO } else { self.f(0.0, 1.0) },
            gas_limited: self.or_zero(-0.2, 1.0),
            pitch: self.f(-1.5, 1.5),
            world_ang_vel: self.vec(-3.0, 3.0),
            front_brake: self.or_zero(-0.1, 1.0),
            rear_brake: if self.one_in(2) { F::ZERO } else { self.f(0.0, 1.0) },
            clutch_in: if self.one_in(2) { F::ZERO } else { self.f(0.0, 1.0) },
            lean: self.f(-1.0, 1.0),
            lean_fore_aft: match self.bits() % 6 {
                0 => F(-1.0),
                1 => F(0.5),
                2 => F(-0.1),
                3 => self.f(0.5, 1.0),
                4 => self.f(-1.0, -0.1),
                _ => self.f(-1.0, 1.0),
            },
            pitch_integral: self.f(-50.0, 50.0),
            stoppie: self.b(),
            push: self.b(),
            push_target: self.f(-20.0, 20.0),
            push_gain: self.f(0.0, 60.0),
            push_gate: self.or_zero(-1.0, 1.0),
            gravity: Vf::new(self.f(-5.0, 5.0).0, self.near(-432.6, -500.0, -30.0).0, self.f(-5.0, 5.0).0, self.f(0.0, 1.0).0),
            wheels,
        };
        let mut body = Body {
            force: self.vec(-100.0, 100.0),
            moment: self.vec(-100.0, 100.0),
            orientation: self.quaternion(),
            ang_vel: self.vec(-3.0, 3.0),
        };
        // Slow enough to stand up, now and then.
        if self.one_in(3) {
            q.speed = self.f(-10.5, 10.5);
            q.gas = F::ZERO;
        }
        // The forward axis straight into the ground: nothing left once it is flattened.
        if self.one_in(25) {
            body.orientation = Vf::new(0.0, 0.0, 0.0, 1.0);
            for w in &mut q.wheels {
                w.normal = Vf::new(0.0, 0.0, 1.0, 1.0);
            }
        }
        let a = self.bits() % 3;
        let bal = if self.one_in(5) { (a, a) } else { (a, a + 1) };
        q.balancing = bal.0 == bal.1;
        State { p, q, body, bal, dt: self.f(0.005, 0.04) }
    }
}

/// Run `original` on the state, then `port` on a copy, and compare the regions word for word.
fn compare(ee: &mut Ee, k: usize, s: &State, label: &str, original: impl Fn(&mut Ee), port: impl Fn(&mut State)) {
    reset(ee);
    poke(ee, s);
    original(ee);
    only_touched_the_regions(ee).unwrap_or_else(|e| panic!("state {k} ({label}): {e}\n{s:#?}"));
    let was = snap(ee);
    let mut t = s.clone();
    port(&mut t);
    reset(ee);
    poke(ee, &t);
    let now = snap(ee);
    if was != now {
        let i = was.iter().zip(&now).position(|(a, b)| a != b).unwrap();
        panic!(
            "state {k} ({label}): {} is {:#010x} ({}) in the original and {:#010x} ({}) in the port\n{s:#?}",
            where_is(i),
            was[i],
            f32::from_bits(was[i]),
            now[i],
            f32::from_bits(now[i]),
        );
    }
}

fn each_path_seen(tally: &BTreeMap<&'static str, u32>, paths: &[&str], at_least: u32) {
    for want in paths {
        let n = tally.get(want).copied().unwrap_or(0);
        assert!(n >= at_least, "only {n} state(s) went down {want:?}: {tally:?}");
    }
}

#[test]
fn the_quads_stand_upright_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let mut g = Gen(0x37b0_0001);
    let mut tally = BTreeMap::new();
    for k in 0..10_000 {
        let s = g.state();
        let q = &s.q;
        let path = if !q.all_down || q.balancing || q.gas != F::ZERO || !(q.speed.abs() < F(10.0)) {
            "off"
        } else if s.body.orientation == Vf::new(0.0, 0.0, 0.0, 1.0) && q.wheels[0].normal == Vf::new(0.0, 0.0, 1.0, 1.0) {
            "no forward left"
        } else if q.brake != F::ZERO {
            "braked"
        } else {
            "rolling"
        };
        *tally.entry(path).or_insert(0u32) += 1;
        compare(
            &mut ee,
            k,
            &s,
            path,
            |ee| ee.call(STAND_UPRIGHT, &[u64::from(OBJ)], &[], 20_000),
            |t| quad::stand_upright(&t.q, &mut t.body),
        );
    }
    each_path_seen(&tally, &["off", "no forward left", "braked", "rolling"], 30);
}

#[test]
fn the_quads_wheelie_and_stoppie_are_the_originals() {
    let Some(mut ee) = image() else { return };
    let mut g = Gen(0x37b0_0002);
    let mut tally = BTreeMap::new();
    for k in 0..10_000 {
        let mut s = g.state();
        // On the front brake, fast and leant all the way forward, rear up: a stoppie about to start.
        if g.one_in(10) {
            let q = &mut s.q;
            (q.on_ground, q.front_down, q.rear_down, q.stoppie) = (true, true, false, false);
            q.front_brake = g.f(0.1, 1.0);
            q.speed = g.f(15.5, 40.0);
            q.lean_fore_aft = F(-1.0);
        }
        let q = &s.q;
        let wheelie = q.rear_down
            && q.gas_limited > F::ZERO
            && q.speed > F(5.0)
            && q.rear_brake == F::ZERO
            && q.clutch_in == F::ZERO
            && q.lean_fore_aft >= F(0.5);
        let starts = q.front_down && !q.stoppie && q.front_brake > F::ZERO && q.speed > F(15.0) && q.lean_fore_aft <= F(-1.0);
        let on = q.stoppie || starts;
        let ends = q.rear_down || q.front_brake == F::ZERO || q.lean_fore_aft > F(-0.1) || q.speed <= F(7.0);
        let path = match (q.on_ground, wheelie, on, ends) {
            (false, ..) => "airborne",
            (true, true, false, _) => "wheelie",
            (true, false, false, _) => "neither",
            (true, _, true, true) => "stoppie, ends",
            (true, _, true, false) if starts => "stoppie, starts",
            (true, _, true, false) => "stoppie, held",
        };
        *tally.entry(path).or_insert(0u32) += 1;
        compare(
            &mut ee,
            k,
            &s,
            path,
            |ee| ee.call(ROLL_PITCH, &[u64::from(OBJ)], &[s.dt.0], 20_000),
            |t| quad::roll_pitch(&t.p, &mut t.q, &mut t.body, t.dt),
        );
    }
    each_path_seen(&tally, &["airborne", "wheelie", "neither", "stoppie, ends", "stoppie, starts", "stoppie, held"], 30);
}

#[test]
fn the_quads_ground_push_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let mut g = Gen(0x37b0_0003);
    let mut tally = BTreeMap::new();
    for k in 0..10_000 {
        let s = g.state();
        let q = &s.q;
        let path = match (q.rear_down && q.push_gate != F::ZERO, q.push) {
            (false, _) => "gated",
            (true, false) => "no push",
            (true, true) => "pushed",
        };
        *tally.entry(path).or_insert(0u32) += 1;
        compare(
            &mut ee,
            k,
            &s,
            path,
            |ee| ee.call(GROUND_PUSH, &[u64::from(OBJ)], &[s.dt.0], 20_000),
            |t| quad::ground_pitch_torque(&t.q, &mut t.body),
        );
    }
    each_path_seen(&tally, &["gated", "no push", "pushed"], 100);
}

#[test]
fn the_rear_pair_flag_is_the_originals() {
    let Some(mut ee) = image() else { return };
    // Stop at the first wheel integrate after the flag: the hook returns to the harness, not the body.
    ee.hooks.insert(WHEEL_INTEGRATE, Box::new(|ee| ee.set(31, DONE as i32 as i64 as u64)));
    let mut g = Gen(0x37b0_0004);
    let mut seen = [0u32; 16];
    for k in 0..2_000 {
        let s = g.state();
        let touch = s.q.wheels.iter().enumerate().fold(0, |m, (i, w)| m | (usize::from(w.touching) << i));
        seen[touch] += 1;
        compare(
            &mut ee,
            k,
            &s,
            "flag",
            |ee| {
                ee.set(16, u64::from(OBJ));
                ee.call(REAR_FLAG_FROM, &[u64::from(OBJ)], &[], 200);
            },
            |t| quad::rear_pair_flag(&mut t.q),
        );
    }
    assert!(seen.iter().all(|&n| n >= 5), "every touch pattern: {seen:?}");
}

/// The field map against a real quad: the spawn's own object read into the port's types and stepped by
/// both, so the offsets the generated tests write are where SLUS-21104's quad keeps them.
#[test]
fn the_spawned_quad_reads_as_the_port_expects() {
    let Some(ee) = image() else { return };
    let obj = 0x015e_0690;
    let f = |a: u32| F(ee.f32_at(a));
    let v = |a: u32| Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12));
    let b = |a: u32| ee.u32_at(a) != 0;
    let body = ee.u32_at(obj + 0xb8);
    assert_eq!(ee.u32_at(body + 0x110), BODY_VTABLE, "the quad's rigid body");
    let wheels: Vec<u32> = (0..4).map(|k| ee.u32_at(obj + 0x17b0 + 4 * k)).collect();
    // At the spawn the four sit on the ground, both fronts take the steering, and nothing is on its nose.
    for (k, &w) in wheels.iter().enumerate() {
        assert!(b(w + 0xdc), "wheel {k} touching");
        assert!(!b(w + 0xb4), "wheel {k} rear-pair flag");
        assert_eq!(b(w + 0x2c), k < 2, "wheel {k} steered");
        let n = v(w + 0x130);
        assert!(n.y() > 0.99, "wheel {k}'s ground normal is up: {n:?}");
    }
    assert!(b(obj + 0x324) && b(obj + 0x1270) && b(obj + 0x1274) && b(obj + 0x1278), "on the ground, all down");
    // The [Wheelie] and [Stoppie] keys of atv_ Race 250: MaxPitchCommand 40 degrees, the gains, the top
    // speeds as their inverse / in ft/s.
    assert_eq!(f(obj + 0x139c), F(0.69813162));
    assert_eq!(f(obj + 0x13a4), F(750.0));
    assert_eq!(f(obj + 0x13d8), F(14.6666));
    let g = v(ee.u32_at(obj + 0xe8) + 0x40);
    assert!(g.x() == 0.0 && g.z() == 0.0 && g.w() == 1.0 && (-432.61..-432.59).contains(&g.y()), "the world's gravity: {g:?}");
}
