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

use game_sim::air::AcosTable;
use game_sim::chassis::{ContactCall, WheelContact};
use game_sim::quad::{self, Body, Call, Contact, Params, Quad, Wheel};
use ps2_float::{F, Vf};
use ps2_oracle::{DONE, Ee, Kit, skip};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

const OBJ: u32 = 0x01c0_0000;
const BODY: u32 = 0x01c0_2000;
const WORLD: u32 = 0x01c0_2400;
const WHEEL: [u32; 4] = [0x01c0_8000, 0x01c0_9000, 0x01c0_a000, 0x01c0_b000];
/// The quad's vehicle state (its `+0x28` is what the three listeners' guard reads) and the collision
/// record its PostProcess flags the tip-over in.
const STATE_OBJ: u32 = 0x01c0_7000;
const RECORD: u32 = 0x01c0_7100;
const REGIONS: [(&str, u32, u32); 9] = [
    ("quad", OBJ, 0x1800),
    ("body", BODY, 0x200),
    ("world", WORLD, 0x50),
    ("state", STATE_OBJ, 0x40),
    ("record", RECORD, 0x40),
    ("left front", 0x01c0_8000, 0x8b0),
    ("right front", 0x01c0_9000, 0x8b0),
    ("left rear", 0x01c0_a000, 0x8b0),
    ("right rear", 0x01c0_b000, 0x8b0),
];
/// What the quad points at and the steps only hand on: its engine, its scene node, its collision
/// search and the actor list the sweep is given.
const ENGINE: u32 = 0x01c0_7300;
const NODE: u32 = 0x01c0_7340;
const ACTORS: u32 = 0x01c0_7380;
const SEARCH: u32 = 0x01c0_c000;

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
    /// The vehicle state's word, or none at all (both stop the three listeners: none, 3 and 4).
    vstate: Option<u32>,
    /// The four wheels' contact fields and the rear node the distributor keeps.
    contacts: [WheelContact; 4],
    rear_node: u32,
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
    match s.vstate {
        Some(v) => {
            ee.set_u32(OBJ + 0xbc4, STATE_OBJ);
            ee.set_u32(STATE_OBJ + 0x28, v);
        }
        None => ee.set_u32(OBJ + 0xbc4, 0),
    }
    ee.set_u32(OBJ + 0xc8, RECORD);
    ee.set_u32(OBJ + 0xe4, NODE);
    ee.set_u32(OBJ + 0xc4, ACTORS);
    ee.set_u32(OBJ + 0x142c, ENGINE);
    ee.set_u32(OBJ + 0x1430, SEARCH);
    ee.set_u32(OBJ + 0x143c, s.rear_node);

    let (p, q, b) = (&s.p, &s.q, &s.body);
    put_f(ee, OBJ + 0x139c, p.max_pitch_command);
    put_f(ee, OBJ + 0x13a0, p.pitch_gain);
    put_f(ee, OBJ + 0x13a4, p.pitch_rate_gain);
    put_f(ee, OBJ + 0x13a8, p.inv_wheelie_top_speed);
    put_f(ee, OBJ + 0x13cc, p.max_stoppie_pitch_command);
    put_f(ee, OBJ + 0x13d0, p.stoppie_pitch_gain);
    put_f(ee, OBJ + 0x13d4, p.stoppie_yaw_rate_scale);
    put_f(ee, OBJ + 0x13d8, p.stoppie_top_speed);
    put_f(ee, OBJ + 0x1300, p.steer_speed_1);
    put_f(ee, OBJ + 0x1304, p.steer_scale_1);
    put_f(ee, OBJ + 0x1308, p.steer_speed_2);
    put_f(ee, OBJ + 0x130c, p.steer_scale_2);
    put_b(ee, OBJ + 0x12f0, p.full_lock);
    put_f(ee, OBJ + 0x12f4, p.full_lock_angle);
    put_f(ee, OBJ + 0x12f8, p.full_lock_time);
    put_f(ee, OBJ + 0x122c, p.max_donut_speed);
    put_f(ee, OBJ + 0x1230, p.donut_angle);
    put_f(ee, OBJ + 0x1234, p.donut_angle_back);
    put_f(ee, OBJ + 0x137c, p.front_spring.0);
    put_f(ee, OBJ + 0x1380, p.front_spring.1);
    put_f(ee, OBJ + 0x1384, p.rear_spring.0);
    put_f(ee, OBJ + 0x1388, p.rear_spring.1);

    put_b(ee, OBJ + 0x324, q.on_ground);
    put_b(ee, OBJ + 0x1270, q.all_down);
    put_b(ee, OBJ + 0x1274, q.front_down);
    put_b(ee, OBJ + 0x1278, q.rear_down);
    ee.set_u32(OBJ + 0x1524, s.bal.0);
    ee.set_u32(OBJ + 0x152c, s.bal.1);
    put_v(ee, OBJ + 0xc30, q.contact_vel);
    put_f(ee, OBJ + 0x310, q.brake);
    put_f(ee, OBJ + 0x314, q.gas);
    put_f(ee, OBJ + 0x318, q.gas_limited);
    put_v(ee, OBJ + 0x360, q.euler);
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
    put_b(ee, OBJ + 0xc80, q.control_off);
    put_f(ee, OBJ + 0x32c, q.speed_len);
    put_f(ee, OBJ + 0x12c8, q.steer_command);
    put_f(ee, OBJ + 0x12fc, q.full_lock_timer);
    put_f(ee, OBJ + 0x12b8, q.stick_angle);
    put_f(ee, OBJ + 0x12bc, q.deflection);
    put_v(ee, OBJ + 0x1250, q.attitude);
    put_v(ee, OBJ + 0x350, q.local_vel);
    put_v(ee, OBJ + 0xbe0, q.heading);
    put_v(ee, OBJ + 0xbf0, q.up);
    put_b(ee, OBJ + 0xde4, q.was_airborne);
    put_b(ee, OBJ + 0xde0, q.airborne);
    put_f(ee, OBJ + 0x1190, q.hang_time);
    put_f(ee, OBJ + 0x330, q.crossup);
    put_b(ee, OBJ + 0x1198, q.flipping);
    put_b(ee, OBJ + 0x11d4, q.spinning);
    put_f(ee, OBJ + 0x1188, q.authority);
    put_f(ee, OBJ + 0x118c, q.authority_time);
    put_b(ee, RECORD + 0x24, q.tipped);
    put_v(ee, OBJ + 0xbd0, q.node_pos);
    put_v(ee, OBJ + 0xc40, q.node_pos_kept);
    for (w, &a) in q.wheels.iter().zip(&WHEEL) {
        put_b(ee, a + 0x2c, w.steered);
        put_b(ee, a + 0xb4, w.rear_pair);
        put_b(ee, a + 0xdc, w.touching);
        put_v(ee, a + 0x130, w.normal);
        put_v(ee, a + 0x190, w.contact_vel);
        put_b(ee, a + 0x30, w.balancing);
        put_f(ee, a + 0x70, w.max_steer_angle);
        put_f(ee, a + 0x8ac, w.steer_command);
        put_f(ee, a + 0x38, w.donut);
        put_f(ee, a + 0x48, w.spring);
        put_f(ee, a + 0x838, w.inv_radius);
    }
    for (c, &a) in s.contacts.iter().zip(&WHEEL) {
        // Its normal is the wheel's own +0x130: the contacts test copies it across.
        put_v(ee, a + 0x120, c.point);
        ee.set_u32(a + 0x1f0, c.owner);
        ee.set_u32(a + 0x1f4, c.vehicle);
        ee.set_u32(a + 0xd4, c.touching as u32);
    }

    put_v(ee, BODY + 0xa0, b.force);
    put_v(ee, BODY + 0x1b0, b.moment);
    put_v(ee, BODY + 0x190, b.orientation);
    put_v(ee, BODY + 0x1d0, b.ang_vel);
    put_v(ee, BODY + 0x80, b.velocity);
    put_v(ee, BODY + 0xc0, b.reset_each_frame);
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

const HALF_PI: f32 = 1.5707964;
const PI: f32 = 3.1415927;

fn with_z(v: Vf, z: F) -> Vf {
    Vf::new(v.x(), v.y(), z.0, v.w())
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
        let mut p = Params {
            max_pitch_command: self.f(0.3, 1.2),
            pitch_gain: self.f(0.2, 3.0),
            pitch_rate_gain: self.f(100.0, 1500.0),
            inv_wheelie_top_speed: self.f(0.0, 0.05),
            max_stoppie_pitch_command: self.f(0.5, 2.5),
            stoppie_pitch_gain: self.f(0.5, 3.0),
            stoppie_yaw_rate_scale: self.f(0.0, 2.0),
            stoppie_top_speed: if self.one_in(6) { F::ZERO } else { self.f(5.0, 40.0) },
            steer_speed_1: self.near(0.0, 0.0, 30.0),
            steer_scale_1: self.f(0.3, 1.2),
            steer_speed_2: self.f(30.0, 140.0),
            steer_scale_2: self.f(0.05, 0.6),
            full_lock: self.b(),
            full_lock_angle: self.f(0.0, 0.4),
            full_lock_time: if self.one_in(10) { F::ZERO } else { self.f(0.1, 1.0) },
            max_donut_speed: self.f(10.0, 60.0),
            donut_angle: if self.one_in(8) { F(if self.b() { -HALF_PI } else { 0.0 }) } else { self.f(-1.5, -0.1) },
            donut_angle_back: if self.one_in(8) { F(if self.b() { -PI } else { -HALF_PI }) } else { self.f(-3.1, -1.7) },
            front_spring: (self.f(200.0, 400.0), self.f(300.0, 500.0)),
            rear_spring: (self.f(400.0, 600.0), self.f(500.0, 700.0)),
        };
        if self.one_in(8) {
            p.steer_speed_2 = p.steer_speed_1;
        }
        let mut wheels = [Wheel::default(); 4];
        for w in &mut wheels {
            *w = Wheel {
                steered: self.b(),
                rear_pair: self.b(),
                touching: !self.one_in(3),
                normal: self.normal(),
                contact_vel: self.vec(-40.0, 40.0),
                balancing: self.b(),
                max_steer_angle: self.f(0.3, 0.7),
                steer_command: self.f(-0.7, 0.7),
                donut: self.f(0.0, 1.0),
                spring: self.f(200.0, 700.0),
                inv_radius: self.f(0.5, 1.5),
            };
        }
        let mut q = Quad {
            on_ground: !self.one_in(6),
            all_down: !self.one_in(4),
            front_down: !self.one_in(4),
            rear_down: !self.one_in(4),
            balancing: false,
            contact_vel: {
                let speed = if self.one_in(10) { F(if self.b() { 5.0 } else if self.b() { 7.0 } else { 15.0 }) } else { self.f(-20.0, 40.0) };
                with_z(self.vec(-10.0, 10.0), speed)
            },
            brake: self.or_zero(0.0, 1.0),
            gas: if self.one_in(2) { F::ZERO } else { self.f(0.0, 1.0) },
            gas_limited: self.or_zero(-0.2, 1.0),
            euler: Vf::new(self.f(-1.5, 1.5).0, self.f(-3.0, 3.0).0, self.f(-3.0, 3.0).0, self.f(-1.0, 1.0).0),
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
            active: true,
            control_off: self.one_in(4),
            speed_len: if self.one_in(8) { F(if self.b() { 14.6666 } else { 29.3332 }) } else { self.f(0.0, 60.0) },
            steer_command: self.f(-0.7, 0.7),
            full_lock_timer: self.f(0.0, 1.0),
            stick_angle: self.f(-3.1416, 3.1416),
            deflection: if self.b() { self.f(0.9, 1.0) } else { self.f(0.0, 1.0) },
            attitude: self.vec(-5.0, 5.0),
            local_vel: self.vec(-40.0, 40.0),
            heading: self.vec(-1.0, 1.0),
            up: self.vec(-1.0, 1.0),
            was_airborne: self.b(),
            airborne: self.b(),
            hang_time: self.f(0.0, 3.0),
            crossup: self.f(-1.0, 1.0),
            flipping: self.one_in(5),
            spinning: self.one_in(5),
            authority: if self.b() { F::ZERO } else { self.f(-0.2, 1.0) },
            authority_time: if self.one_in(6) { F::ZERO } else { self.f(0.2, 1.0) },
            tipped: self.b(),
            node_pos: self.vec(-1000.0, 1000.0),
            node_pos_kept: self.vec(-1000.0, 1000.0),
        };
        // The stick hard over, now and then, for the full lock.
        if self.one_in(4) {
            q.lean = if self.b() { self.f(0.987, 1.0) } else { self.f(-1.0, -0.987) };
        }
        let mut body = Body {
            force: self.vec(-100.0, 100.0),
            moment: self.vec(-100.0, 100.0),
            orientation: self.quaternion(),
            ang_vel: self.vec(-3.0, 3.0),
            velocity: self.vec(-60.0, 60.0),
            reset_each_frame: self.vec(-1.0, 1.0),
        };
        // Slow enough to stand up, now and then.
        if self.one_in(3) {
            q.contact_vel = with_z(q.contact_vel, self.f(-10.5, 10.5));
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
        let vstate = match self.bits() % 8 {
            0 => None,
            1 => Some(3),
            2 => Some(4),
            k => Some(k % 3 + if k == 7 { 5 } else { 0 }),
        };
        q.active = matches!(vstate, Some(v) if !(3..5).contains(&v));
        let mut contacts = [WheelContact::default(); 4];
        for (c, w) in contacts.iter_mut().zip(&q.wheels) {
            *c = WheelContact {
                point: self.vec(-500.0, 500.0),
                normal: w.normal,
                owner: self.bits(),
                vehicle: self.bits(),
                touching: (self.bits() % 2) as i32,
            };
        }
        State { p, q, body, bal, vstate, contacts, rear_node: self.bits(), dt: self.f(0.005, 0.04) }
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
        let path = if !q.all_down || q.balancing || q.gas != F::ZERO || !(q.speed().abs() < F(10.0)) {
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
            q.contact_vel = with_z(q.contact_vel, g.f(15.5, 40.0));
            q.lean_fore_aft = F(-1.0);
        }
        let q = &s.q;
        let wheelie = q.rear_down
            && q.gas_limited > F::ZERO
            && q.speed() > F(5.0)
            && q.rear_brake == F::ZERO
            && q.clutch_in == F::ZERO
            && q.lean_fore_aft >= F(0.5);
        let starts = q.front_down && !q.stoppie && q.front_brake > F::ZERO && q.speed() > F(15.0) && q.lean_fore_aft <= F(-1.0);
        let on = q.stoppie || starts;
        let ends = q.rear_down || q.front_brake == F::ZERO || q.lean_fore_aft > F(-0.1) || q.speed() <= F(7.0);
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

// --- MD64: the three listeners --------------------------------------------------------------------

/// The listeners themselves.
const GROUND_HEAD: u32 = 0x004b_9020;
const GROUND_STEP: u32 = 0x004b_9580;
const WHEEL_CONTACTS: u32 = 0x004b_d2d0;
const AIR_STATE: u32 = 0x004b_97a0;
/// The PreProcess's callees: two through the quad's vtable (`+0x34c`, `+0x4d0`), two direct.
const INPUT_LIMITER: u32 = 0x004e_3ea0;
const AUTO_SHIFT: u32 = 0x004e_5fe0;
const CLUTCH: u32 = 0x004e_6a60;
const HEAD_TAIL: u32 = 0x004e_4190;
/// The Force body's callees that are the bike's or the vehicle's.
const GRAVITY: u32 = 0x004a_4ef0;
const PRELOAD: u32 = 0x004e_60d0;
const BACKFLIP: u32 = 0x004e_4f20;
const SPIN_AIR: u32 = 0x004e_53d0;
const ENGINE_UPDATE: u32 = 0x0049_fbb0;
const AIR_DRAG: u32 = 0x004e_6c00;
const AIRBORNE_CONTROL: u32 = 0x004e_65d0;
const WHEEL_CLEAR: u32 = 0x004a_0c30;
const SUSPENSION: u32 = 0x004a_0cb0;
const TIRE_FORCES: u32 = 0x004a_16f0;
const REV_LIMIT: u32 = 0x0049_ff40;
const PEAK_RATES: u32 = 0x004e_6c90;
/// The distributor's sweep, and the PostProcess's two callees.
const COLLIDE: u32 = 0x0011_51c0;
const VEHICLE_POST: u32 = 0x004a_4f00;
const NODE_POSITION: u32 = 0x0019_4c20;
/// The arc cosine table's pointer and index scale, as the PostProcess reads them.
const ACOS_PTR: u32 = 0x003e_a9c0;
const ACOS_SCALE: u32 = 0x003e_a9d8;

type Log<T> = Rc<RefCell<Vec<T>>>;

fn wheel_of(ee: &Ee) -> usize {
    let a = ee.r(4) as u32;
    WHEEL.iter().position(|&w| w == a).unwrap_or_else(|| panic!("a wheel call handed {a:#x}"))
}

/// Hook each callee to log what it was handed in the port's terms, and do nothing else.
fn hook_calls(ee: &mut Ee, table: &[(u32, fn(&Ee) -> Call)]) -> Log<Call> {
    let log: Log<Call> = Rc::default();
    for &(at, f) in table {
        let l = log.clone();
        ee.hooks.insert(at, Box::new(move |ee| l.borrow_mut().push(f(ee))));
    }
    log
}

fn on_quad(ee: &Ee, c: Call) -> Call {
    assert_eq!(ee.r(4) as u32, OBJ, "{c:?} handed something other than the quad");
    c
}

fn tally(t: &mut BTreeMap<&'static str, u32>, path: &'static str) {
    *t.entry(path).or_insert(0) += 1;
}

#[test]
fn the_quads_pre_process_is_the_originals() {
    let Some(mut ee) = image() else { return };
    assert_eq!(ee.u32_at(QUAD_VTABLE + 0x34c), INPUT_LIMITER, "the quad's input-limiter slot");
    assert_eq!(ee.u32_at(QUAD_VTABLE + 0x4d0), HEAD_TAIL, "the quad's head-tail slot");
    let log = hook_calls(
        &mut ee,
        &[
            (INPUT_LIMITER, |ee| on_quad(ee, Call::InputLimiter)),
            (AUTO_SHIFT, |ee| on_quad(ee, Call::AutoShift { dt: F(ee.f(12)), inv_radius: F(ee.f(13)) })),
            (CLUTCH, |ee| on_quad(ee, Call::Clutch(F(ee.f(12))))),
            (HEAD_TAIL, |ee| on_quad(ee, Call::HeadTail)),
        ],
    );
    let port: Log<Call> = Rc::default();
    let mut g = Gen(0x0064_0001);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        let mut s = g.state();
        // The donut's gate open, a third of the time.
        if g.one_in(3) {
            s.q.all_down = true;
            s.q.speed_len = g.f(0.0, s.p.max_donut_speed.0);
            s.q.deflection = g.f(0.9001, 1.0);
            s.q.gas = g.f(0.05, 1.0);
        }
        // The back ramp's end exactly at −π, with the stick past it.
        if g.one_in(12) {
            s.p.donut_angle_back = F(-PI);
            s.q.stick_angle = g.f(-3.3, -3.1416);
        }
        let (p, q) = (&s.p, &s.q);
        if !q.active {
            tally(&mut t, "off");
        } else {
            if q.balancing {
                tally(&mut t, "balancing");
            } else if q.control_off {
                tally(&mut t, "control off");
            } else if !p.full_lock {
                tally(&mut t, if p.steer_speed_1 == p.steer_speed_2 { "steer, flat fade" } else { "steer" });
            } else {
                tally(&mut t, if q.lean.abs() < F(0.987) { "full lock, let go" } else { "full lock, held" });
            }
            let gate = q.all_down && q.speed_len < p.max_donut_speed && q.deflection > F(0.9) && q.gas != F::ZERO;
            let a = q.stick_angle;
            tally(&mut t, match (gate, a < F(-HALF_PI)) {
                (false, _) => "no donut",
                (true, false) if a < p.donut_angle => "donut, forward, ramp out",
                (true, false) if p.donut_angle == F::ZERO => "donut, forward, no ramp in",
                (true, false) => "donut, forward, ramp in",
                (true, true) if a < p.donut_angle_back && p.donut_angle_back == F(-PI) => "donut, back, past −π",
                (true, true) if a < p.donut_angle_back => "donut, back, ramp out",
                (true, true) => "donut, back, ramp in",
            });
            if !q.on_ground {
                tally(&mut t, "front brake let go");
            }
        }
        log.borrow_mut().clear();
        compare(
            &mut ee,
            k,
            &s,
            "pre-process",
            |ee| ee.call(GROUND_HEAD, &[u64::from(OBJ)], &[s.dt.0], 50_000),
            |t| {
                let mut calls = Vec::new();
                quad::ground_head(&t.p, &mut t.q, &mut t.body, t.dt, &mut calls);
                *port.borrow_mut() = calls;
            },
        );
        assert_eq!(*log.borrow(), *port.borrow(), "state {k}: the calls\n{s:#?}");
    }
    each_path_seen(
        &t,
        &[
            "off",
            "balancing",
            "control off",
            "steer",
            "steer, flat fade",
            "full lock, let go",
            "full lock, held",
            "no donut",
            "donut, forward, ramp out",
            "donut, forward, no ramp in",
            "donut, forward, ramp in",
            "donut, back, past −π",
            "donut, back, ramp out",
            "donut, back, ramp in",
            "front brake let go",
        ],
        30,
    );
}

#[test]
fn the_quads_force_body_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let log = hook_calls(
        &mut ee,
        &[
            (GRAVITY, |ee| on_quad(ee, Call::Gravity)),
            (PRELOAD, |ee| on_quad(ee, Call::Preload(F(ee.f(12))))),
            (BACKFLIP, |ee| on_quad(ee, Call::Backflip(F(ee.f(12))))),
            (SPIN_AIR, |ee| on_quad(ee, Call::SpinAir(F(ee.f(12))))),
            (ENGINE_UPDATE, |ee| {
                assert_eq!(ee.r(4) as u32, ENGINE, "the engine update's engine");
                Call::EngineUpdate(F(ee.f(12)))
            }),
            (AIR_DRAG, |ee| on_quad(ee, Call::AirDrag(F(ee.f(12))))),
            (AIRBORNE_CONTROL, |ee| on_quad(ee, Call::AirborneControl(F(ee.f(12))))),
            (WHEEL_CLEAR, |ee| Call::WheelClear(wheel_of(ee))),
            (WHEEL_CONTACTS, |ee| on_quad(ee, Call::Contacts)),
            (SUSPENSION, |ee| Call::Suspension(wheel_of(ee), F(ee.f(12)))),
            (TIRE_FORCES, |ee| Call::TireForces(wheel_of(ee), F(ee.f(12)))),
            (WHEEL_INTEGRATE, |ee| Call::WheelIntegrate(wheel_of(ee), F(ee.f(12)))),
            (REV_LIMIT, |ee| {
                assert_eq!(ee.r(4) as u32, ENGINE, "the rev limiter's engine");
                Call::RevLimit
            }),
            (PEAK_RATES, |ee| on_quad(ee, Call::PeakRates)),
        ],
    );
    let port: Log<Call> = Rc::default();
    let mut g = Gen(0x0064_0002);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        let s = g.state();
        let q = &s.q;
        tally(&mut t, match (q.active, q.control_off) {
            (false, _) => "off",
            (true, true) => "control off",
            (true, false) => "control on",
        });
        if q.active && !q.wheels[LF].touching && q.wheels[RF].touching && !q.wheels[LR].touching && !q.wheels[RR].touching {
            tally(&mut t, "on the right front alone");
        }
        log.borrow_mut().clear();
        compare(
            &mut ee,
            k,
            &s,
            "force",
            |ee| ee.call(GROUND_STEP, &[u64::from(OBJ)], &[s.dt.0], 100_000),
            |t| {
                let mut calls = Vec::new();
                quad::ground_step(&t.p, &mut t.q, &mut t.body, t.dt, &mut calls);
                *port.borrow_mut() = calls;
            },
        );
        assert_eq!(*log.borrow(), *port.borrow(), "state {k}: the calls\n{s:#?}");
    }
    each_path_seen(&t, &["off", "control off", "control on", "on the right front alone"], 30);
}

const LF: usize = quad::LEFT_FRONT;
const RF: usize = quad::RIGHT_FRONT;
const LR: usize = quad::LEFT_REAR;
const RR: usize = quad::RIGHT_REAR;

/// The collision search and what hangs off it, laid out for the distributor to walk: the search's
/// vtable, its record and the record's four parallel arrays, the nodes, their owners with their
/// listener lists, and the vehicles. The two virtuals land on addresses with no code, hooked.
const SEARCH_VT: u32 = 0x01c0_c200;
const COLL: u32 = 0x01c0_c400;
const AT: u32 = 0x01c0_c600;
const NORMALS: u32 = 0x01c0_c700;
const POINTS: u32 = 0x01c0_c800;
const NODES: u32 = 0x01c0_c900;
const ELEM_VT: u32 = 0x01c0_c980;
const NODE_AT: u32 = 0x01c0_d000;
const OWNER_AT: u32 = 0x01c0_e000;
const VEHICLE_AT: u32 = 0x01c0_f000;
const CLEAR_FN: u32 = 0x01cf_0000;
const NOTIFY_FN: u32 = 0x01cf_0100;

fn lay_out_contacts(ee: &mut Ee, look: bool, cs: &[Contact]) {
    ee.set_u32(SEARCH + 0xc, SEARCH_VT);
    ee.set_u32(SEARCH_VT + 200, CLEAR_FN);
    // Bit 5 of the flags byte, among other bits set at random-ish.
    ee.set_u32(SEARCH + 0x124, if look { 0x0025_0000 } else { 0x00c3_0000 });
    ee.set_u32(SEARCH + 0x7c, COLL);
    ee.set_u32(COLL + 0x8c, cs.len() as u32);
    ee.set_u32(COLL + 0x90, AT);
    ee.set_u32(COLL + 0x94, NORMALS);
    ee.set_u32(COLL + 0x98, POINTS);
    ee.set_u32(COLL + 0x9c, NODES);
    ee.set_u32(ELEM_VT + 0xc, NOTIFY_FN);
    for (i, c) in cs.iter().enumerate() {
        let i = i as u32;
        put_v(ee, AT + 0x20 * i, c.at);
        put_v(ee, NORMALS + 0x10 * i, c.normal);
        put_v(ee, POINTS + 0x10 * i, c.point);
        ee.set_u32(NODES + 4 * i, c.node);
        ee.set_u32(c.node + 0x8c, c.vehicle);
        ee.set_u32(c.node + 0x90, c.owner);
        if c.owner != 0 {
            let list = c.owner + 0x300;
            ee.set_u32(c.owner + 0x208, list);
            ee.set_u32(list + 0x104, c.listeners as u32);
            ee.set_u32(list + 0x10c, c.owner + 0x380);
            for j in 0..4 {
                let elem = c.owner + 0x3c0 + 0x10 * j;
                ee.set_u32(c.owner + 0x380 + 4 * j, elem);
                ee.set_u32(elem, ELEM_VT);
            }
        }
        if c.vehicle != 0 {
            ee.set_u32(c.vehicle + 0x1a0, c.vehicle_kind as u32);
        }
    }
}

#[test]
fn the_quads_contact_distributor_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let log: Log<ContactCall> = Rc::default();
    let l = log.clone();
    ee.hooks.insert(
        CLEAR_FN,
        Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, SEARCH, "the record clear's search");
            l.borrow_mut().push(ContactCall::ClearRecord)
        }),
    );
    let l = log.clone();
    ee.hooks.insert(
        COLLIDE,
        Box::new(move |ee| {
            assert_eq!((ee.r(4) as u32, ee.r(5) as u32, ee.r(6)), (SEARCH, ACTORS, 1), "the sweep's arguments");
            l.borrow_mut().push(ContactCall::Collide)
        }),
    );
    let l = log.clone();
    ee.hooks.insert(
        NOTIFY_FN,
        Box::new(move |ee| {
            assert_eq!((ee.r(5), ee.r(6)), (0, 0), "a listener's arguments");
            let elem = ee.r(4) as u32;
            let owner = elem & !0x3ff;
            l.borrow_mut().push(ContactCall::Notify(owner, ((elem - owner - 0x3c0) / 0x10) as i32))
        }),
    );
    let port: Log<ContactCall> = Rc::default();
    let mut g = Gen(0x0064_0003);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        let s = g.state();
        let look = !g.one_in(8);
        let listeners = [g.bits() % 4, g.bits() % 4, g.bits() % 4];
        let kinds = [if g.one_in(4) { 1 } else { 2 }, if g.one_in(4) { 3 } else { 2 }];
        let n = g.bits() % 7;
        let cs: Vec<Contact> = (0..n)
            .map(|i| {
                let owner = match g.bits() % 4 {
                    3 => 0,
                    o => OWNER_AT + 0x400 * o,
                };
                let vehicle = match g.bits() % 3 {
                    2 => 0,
                    v => VEHICLE_AT + 0x200 * v,
                };
                let axis = |g: &mut Gen| if g.one_in(6) { F::ZERO } else { g.f(-3.0, 3.0) };
                let at = Vf::new(axis(&mut g).0, g.f(-1.0, 1.0).0, axis(&mut g).0, g.f(-1.0, 1.0).0);
                Contact {
                    at,
                    normal: g.normal(),
                    point: g.vec(-500.0, 500.0),
                    node: NODE_AT + 0x100 * i,
                    owner,
                    listeners: if owner == 0 { 0 } else { listeners[((owner - OWNER_AT) / 0x400) as usize] as i32 },
                    vehicle,
                    vehicle_kind: if vehicle == 0 { 0 } else { kinds[((vehicle - VEHICLE_AT) / 0x200) as usize] },
                }
            })
            .collect();
        if !look {
            tally(&mut t, "nothing to look at");
        } else if cs.is_empty() {
            tally(&mut t, "no contacts");
        }
        if look {
            for c in &cs {
                let front = !(c.at.z() <= 0.0);
                let right = !(c.at.x() <= 0.0);
                tally(&mut t, match (front, right) {
                    (true, false) => "left front",
                    (true, true) => "right front",
                    (false, false) => "left rear",
                    (false, true) => "right rear",
                });
                if c.vehicle != 0 {
                    tally(&mut t, if c.vehicle_kind == 2 { "vehicle" } else { "not a vehicle" });
                }
                if c.listeners > 0 {
                    tally(&mut t, "listeners told");
                }
            }
        }
        log.borrow_mut().clear();
        compare(
            &mut ee,
            k,
            &s,
            "contacts",
            |ee| {
                lay_out_contacts(ee, look, &cs);
                ee.call(WHEEL_CONTACTS, &[u64::from(OBJ)], &[s.dt.0], 50_000);
            },
            |t| {
                let calls = quad::wheel_contacts(&mut t.contacts, &mut t.rear_node, look, &cs);
                for (w, c) in t.q.wheels.iter_mut().zip(&t.contacts) {
                    w.normal = c.normal;
                }
                *port.borrow_mut() = calls;
            },
        );
        assert_eq!(*log.borrow(), *port.borrow(), "state {k}: the calls\n{cs:#?}");
    }
    each_path_seen(
        &t,
        &["nothing to look at", "no contacts", "left front", "right front", "left rear", "right rear", "vehicle", "not a vehicle", "listeners told"],
        30,
    );
}

/// Where the hooked scene node says the quad is.
const NODE_SAYS: Vf = Vf::new(812.5, -36.25, 1904.75, 1.0);

/// The PostProcess's callees as the port's: logged, and the node's position written where it asks.
struct Post(Vec<Call>);

impl quad::Moto for Post {
    fn call(&mut self, c: Call, q: &mut Quad, _body: &mut Body) {
        if c == Call::NodePosition {
            q.node_pos = NODE_SAYS;
        }
        self.0.push(c);
    }
}

#[test]
fn the_quads_post_process_is_the_originals() {
    let Some(mut ee) = image() else { return };
    // The table the original reads is the one the game built at startup: it must be the port's.
    let table = AcosTable::build();
    assert_eq!(ee.f32_at(ACOS_SCALE), 14999.5, "the arc cosine table's scale");
    let at = ee.u32_at(ACOS_PTR);
    for (i, &e) in table.entries().iter().enumerate() {
        assert_eq!(ee.f32_at(at + 4 * i as u32).to_bits(), e.to_bits(), "arc cosine entry {i}");
    }
    let log = hook_calls(
        &mut ee,
        &[
            (VEHICLE_POST, |ee| on_quad(ee, Call::VehiclePost(F(ee.f(12))))),
            (NODE_POSITION, |ee| {
                assert_eq!((ee.r(4) as u32, ee.r(5), ee.r(6) as u32), (NODE, 0, OBJ + 0xbd0), "the node's arguments");
                Call::NodePosition
            }),
        ],
    );
    // The hook writes where the node is: hooks cannot reach the quad but through RAM.
    let write = ee.hooks.remove(&NODE_POSITION).unwrap();
    let mut write = Some(write);
    ee.hooks.insert(
        NODE_POSITION,
        Box::new(move |ee| {
            write.as_mut().unwrap()(ee);
            put_v(ee, OBJ + 0xbd0, NODE_SAYS);
        }),
    );
    let port: Log<Call> = Rc::default();
    let at_quarter = up_at_a_quarter();
    let mut g = Gen(0x0064_0004);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        let mut s = g.state();
        // A quaternion or two that leaves the quad on its side or over, and a crawl now and then.
        if g.one_in(10) {
            s.body.velocity = g.vec(-9.0, 9.0);
        }
        if g.one_in(8) {
            s.body.velocity = Vf::new(g.f(-30.0, 30.0).0, g.f(-30.0, 30.0).0, g.f(-30.0, 30.0).0, 1.0);
            s.q.wheels.iter_mut().for_each(|w| w.touching = false);
        }
        // Up exactly a quarter: still tipped.
        if g.one_in(15) {
            s.body.orientation = Vf::new(at_quarter.x(), at_quarter.y(), at_quarter.z(), g.f(-1.0, 1.0).0);
        }
        let q = &s.q;
        if !q.active {
            tally(&mut t, "off");
        } else {
            let touching: Vec<bool> = q.wheels.iter().map(|w| w.touching).collect();
            let airborne = !touching.iter().any(|&b| b);
            tally(&mut t, match touching.iter().position(|&b| b) {
                None => "contact from the body",
                Some(0) => "contact from the left front",
                Some(1) => "contact from the right front",
                Some(2) => "contact from the left rear",
                _ => "contact from the right rear",
            });
            if airborne && !q.airborne {
                tally(&mut t, "takes off");
            }
            let v = s.body.velocity;
            #[allow(clippy::float_arithmetic)]
            let fast = (v.x() * v.x() + v.y() * v.y() + v.z() * v.z()).sqrt() > 10.0;
            if q.flipping || q.spinning || !fast {
                tally(&mut t, "no crossup");
            } else {
                tally(&mut t, "crossup");
            }
            tally(&mut t, match (q.authority > F::ZERO, q.authority_time == F::ZERO) {
                (false, _) => "full authority",
                (true, true) => "no authority time",
                (true, false) => "authority ramp",
            });
            let up = game_sim::math::quat_axes(s.body.orientation).1[1];
            tally(&mut t, if up == F(0.25) { "up at a quarter" } else if up < F(0.25) { "tipped" } else { "upright" });
        }
        log.borrow_mut().clear();
        compare(
            &mut ee,
            k,
            &s,
            "post-process",
            |ee| ee.call(AIR_STATE, &[u64::from(OBJ)], &[s.dt.0], 50_000),
            |t| {
                let mut post = Post(Vec::new());
                quad::air_state(&mut t.q, &mut t.body, t.dt, &table, &mut post);
                *port.borrow_mut() = post.0;
            },
        );
        assert_eq!(*log.borrow(), *port.borrow(), "state {k}: the calls\n{s:#?}");
    }
    each_path_seen(
        &t,
        &[
            "off",
            "contact from the body",
            "contact from the left front",
            "contact from the right front",
            "contact from the left rear",
            "contact from the right rear",
            "takes off",
            "no crossup",
            "crossup",
            "full authority",
            "no authority time",
            "authority ramp",
            "up at a quarter",
            "tipped",
            "upright",
        ],
        30,
    );
}

/// A quaternion whose up axis's `y` comes out exactly 0.25: `x` a half and `z` near `√⅛`, searched
/// a few ulps either way. `w` does not enter it.
fn up_at_a_quarter() -> Vf {
    let z0 = F(0.35355339).0.to_bits();
    for d in 0..200u32 {
        for z in [z0 + d, z0 - d] {
            let q = Vf::new(0.5, 0.0, f32::from_bits(z), 0.5);
            if game_sim::math::quat_axes(q).1[1] == F(0.25) {
                return q;
            }
        }
    }
    panic!("no quaternion near x = ½, z = √⅛ has up.y exactly 0.25");
}
