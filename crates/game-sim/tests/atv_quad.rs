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
use game_sim::atv_body;
use game_sim::biplane::World;
use game_sim::chassis::{ContactCall, WheelContact};
use game_sim::phy::Ini;
use game_sim::quad::{self, Body, Call, Contact, Params, Player, Quad, Race, Surface, Tail, Wheel};
use ps2_float::{F, Vf};
use ps2_oracle::{DONE, Ee, Kit, skip};
use std::cell::{Cell, RefCell};
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
/// The player the update's ride stats read, whose countdown it runs (MD64a).
const PLAYER: u32 = 0x01c0_7400;
const REGIONS: [(&str, u32, u32); 10] = [
    ("quad", OBJ, 0x1800),
    ("body", BODY, 0x230),
    ("world", WORLD, 0x50),
    ("state", STATE_OBJ, 0x40),
    ("record", RECORD, 0x40),
    ("left front", 0x01c0_8000, 0x8b0),
    ("right front", 0x01c0_9000, 0x8b0),
    ("left rear", 0x01c0_a000, 0x8b0),
    ("right rear", 0x01c0_b000, 0x8b0),
    ("player", PLAYER, 0x1e0),
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
    put_v(ee, BODY + 0x70, b.position);
    put_v(ee, BODY + 0xd0, b.correction);
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
            node_local: Vf::new(0.0, 0.0, 0.0, 1.0),
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
            ..Body::default()
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
            (INPUT_LIMITER, |ee| on_quad(ee, Call::InputLimiter(F(ee.f(12))))),
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

// --- MD64a: the update ----------------------------------------------------------------------------

/// The update, and what it calls: the bike's vehicle update, the name lookup the rider is found by,
/// the vehicle's hazard clock (quad vt +0x45c), the rider's stunt start / end (vt +0x4b8 / +0x4ac),
/// the rumble-on query and the message post.
const UPDATE: u32 = 0x004b_a9f0;
const BIKE_UPDATE: u32 = 0x004e_1b90;
const NAME_BIT: u32 = 0x0021_4b30;
const HAZARD_CLOCK: u32 = 0x004a_ad60;
const START_STUNT: u32 = 0x004e_6370;
const END_STUNT: u32 = 0x004e_4db0;
const RUMBLE_ON: u32 = 0x0051_a250;
const POST: u32 = 0x0027_2300;
/// The post's first argument is read from here.
const POST_QUEUE: u32 = 0x0044_2eb0;
/// What the update walks to: the vehicle state's vtable (its rider lookup), the rider and its hold,
/// the pad and its vtable, the race, the player's countdown vtable, its listeners and the race's
/// watchers. The virtuals land on addresses with no code, hooked.
const STATE_VT: u32 = 0x01c0_d000;
const RIDER: u32 = 0x01c0_d100;
const RIDER_HOLD: u32 = 0x01c0_d500;
const PAD: u32 = 0x01c0_d600;
const PAD_VT: u32 = 0x01c0_d680;
const COUNTDOWN_VT: u32 = 0x01c0_d700;
const LISTENER_VT: u32 = 0x01c0_d780;
const WATCHER_VT: u32 = 0x01c0_d800;
const LISTENERS: u32 = 0x01c0_e000;
const WATCHERS: u32 = 0x01c0_e400;
const RACE: u32 = 0x01c0_e800;
const RIDER_FN: u32 = 0x01cf_0200;
const RUMBLE_FN: u32 = 0x01cf_0300;
const COUNTDOWN_FN: u32 = 0x01cf_0400;
const LISTENER_FN: u32 = 0x01cf_0500;
const WATCHER_FN: u32 = 0x01cf_0600;
/// What the name lookup hands back, and the vehicle state's word the rumble query is handed.
const RIDER_NAME: u64 = 0x0052_74f3;
const RUMBLE_TOKEN: u32 = 0x0012_3450;

fn poke_tail(ee: &mut Ee, s: &State, t: &Tail) {
    poke(ee, s);
    ee.set_u32(STATE_OBJ + 0x4, STATE_VT);
    ee.set_u32(STATE_VT + 0xac, RIDER_FN);
    ee.set_u32(STATE_OBJ + 0x5c, if t.pad { PAD } else { 0 });
    ee.set_u32(STATE_OBJ + 0x6ac, RUMBLE_TOKEN);
    ee.set_u32(PAD + 0x1c, PAD_VT);
    ee.set_u32(PAD_VT + 0x4c, RUMBLE_FN);
    ee.set_u32(RIDER + 0x378, RIDER_HOLD);
    put_b(ee, RIDER_HOLD + 0x14, t.rider_held);

    put_f(ee, OBJ + 0x174c, t.stunt_speed);
    for (i, sf) in t.surfaces.iter().enumerate() {
        let a = OBJ + 0xdf4 + 0xc * i as u32;
        put_b(ee, a, sf.hazard);
        put_f(ee, a + 4, sf.time);
    }
    for (k, &w) in WHEEL.iter().enumerate() {
        ee.write(w + 0x110, 1, u128::from(t.wheel_surface[k]));
        put_b(ee, w + 0xd0, t.wheel_sunk[k]);
        put_f(ee, w + 0xf0, t.wheel_squeeze[k]);
    }
    put_b(ee, OBJ + 0xc20, t.on_hazard);
    ee.set_u32(OBJ + 0x31c, t.request);
    ee.set_u32(OBJ + 0x320, t.request_was);
    put_f(ee, OBJ + 0x1744, t.wheelie_clock);
    put_f(ee, OBJ + 0x1748, t.stoppie_clock);
    ee.set_u32(OBJ + 0x13c4, t.wheelie_stunt);
    ee.set_u32(OBJ + 0x13c8, t.stoppie_stunt);
    put_f(ee, OBJ + 0x704, t.wheelie_time);
    put_f(ee, OBJ + 0x708, t.stoppie_time);
    put_f(ee, OBJ + 0x70c, t.wheelie_dist);
    put_f(ee, OBJ + 0x710, t.stoppie_dist);
    put_f(ee, OBJ + 0x714, t.best_wheelie_time);
    put_f(ee, OBJ + 0x718, t.best_stoppie_time);
    put_f(ee, OBJ + 0x71c, t.best_wheelie_dist);
    put_f(ee, OBJ + 0x720, t.best_stoppie_dist);
    ee.set_u32(OBJ + 0x6a0, COUNTDOWN_VT);
    ee.set_u32(COUNTDOWN_VT + 0x68, COUNTDOWN_FN);
    ee.set_u32(OBJ + 0x680, if t.player.is_some() { PLAYER } else { 0 });
    if let Some(p) = t.player {
        ee.set_u32(PLAYER + 0x8, p.flags);
        put_f(ee, PLAYER + 0x18, p.step);
        ee.set_u32(PLAYER + 0x40, p.word);
        put_f(ee, PLAYER + 0x108, p.speed);
        put_f(ee, PLAYER + 0xc4, p.countdown);
        // The listeners chain through +0x1c; each has its vtable at +0x20.
        ee.set_u32(PLAYER + 0x1d4, if p.listeners == 0 { 0 } else { LISTENERS });
        ee.set_u32(LISTENER_VT + 0x68, LISTENER_FN);
        for i in 0..p.listeners {
            let e = LISTENERS + 0x40 * i;
            ee.set_u32(e + 0x1c, if i + 1 < p.listeners { e + 0x40 } else { 0 });
            ee.set_u32(e + 0x20, LISTENER_VT);
        }
    }
    ee.set_u32(OBJ + 0x684, if t.race.is_some() { RACE } else { 0 });
    if let Some(r) = t.race {
        put_f(ee, RACE + 0x29c, r.clock);
        // The watchers: the first's next is at +4, every later one's at +0x18 (the other word of each
        // left zero, so a walk through the wrong one stops short). Each has its vtable at +0x1c.
        ee.set_u32(RACE + 0x24, if r.watchers == 0 { 0 } else { WATCHERS });
        ee.set_u32(WATCHER_VT + 0x7c, WATCHER_FN);
        for i in 0..r.watchers {
            let e = WATCHERS + 0x40 * i;
            let next = if i + 1 < r.watchers { e + 0x40 } else { 0 };
            ee.set_u32(e + 4, if i == 0 { next } else { 0 });
            ee.set_u32(e + 0x18, if i == 0 { 0 } else { next });
            ee.set_u32(e + 0x1c, WATCHER_VT);
        }
    }
    put_f(ee, OBJ + 0x6f8, t.ride_time);
    put_f(ee, OBJ + 0x6fc, t.top_speed);
    put_f(ee, OBJ + 0x724, t.distance);
    ee.set_u32(OBJ + 0x6c8, t.word_falls);
    ee.set_u32(OBJ + 0x6cc, t.word_was);
    put_v(ee, NODE + 0x80, t.node_up);
    put_v(ee, NODE + 0x90, t.node_forward);
    ee.set_u32(OBJ + 0x175c, t.second_on);
    put_f(ee, OBJ + 0x1760, t.second_countdown);
    put_f(ee, OBJ + 0x1750, t.clock);
    ee.set_u32(OBJ + 0xd20, t.post);
}

#[allow(clippy::float_arithmetic)]
impl Gen {
    fn tail(&mut self, s: &mut State) -> Tail {
        let vehicle_state = match self.bits() % 12 {
            0 => None,
            1 => Some(0),
            2 => Some(2),
            3 => Some(3),
            _ => Some(1),
        };
        s.vstate = vehicle_state;
        let q = &mut s.q;
        if self.one_in(4) {
            q.hang_time = if self.one_in(3) { F(0.1) } else { self.f(0.0, 0.2) };
        }
        if self.one_in(6) {
            q.speed_len = F(10.0);
        }
        // Each pairing of the two pairs, and wrecked, about equally often.
        if self.one_in(2) {
            q.front_down = self.b();
            q.rear_down = !q.front_down;
        }
        let mut surfaces = [Surface::default(); 16];
        for sf in &mut surfaces {
            *sf = Surface {
                hazard: self.one_in(6),
                time: match self.bits() % 5 {
                    0 => F(-1.0),
                    1 => F::ZERO,
                    _ => self.f(-2.0, 5.0),
                },
            };
        }
        let squeeze = |g: &mut Gen| match g.bits() % 6 {
            0 => F(0.95),
            1 => F(0.73),
            _ => g.f(0.3, 1.1),
        };
        let clock = |g: &mut Gen| if g.one_in(4) { F::ZERO } else { g.f(0.5, 1.1) };
        let time = |g: &mut Gen| match g.bits() % 4 {
            0 => F::ZERO,
            1 => g.f(-0.5, 0.0),
            _ => g.f(0.0, 5.0),
        };
        let word = |g: &mut Gen| if g.one_in(3) { 0 } else { g.bits() | 1 };
        let player = (!self.one_in(6)).then(|| Player {
            flags: self.bits(),
            step: self.f(0.0, 3.0),
            word: word(self),
            speed: self.f(0.0, 80.0),
            countdown: match self.bits() % 4 {
                0 => F::ZERO,
                1 => self.f(-1.0, 0.0),
                _ => self.f(0.0, 0.06),
            },
            listeners: self.bits() % 4,
        });
        let race = (!self.one_in(5)).then(|| Race {
            clock: match self.bits() % 4 {
                0 => F::ZERO,
                1 => self.f(0.0, 5.0),
                _ => self.f(-5.0, 0.0),
            },
            watchers: self.bits() % 4,
        });
        let node_forward = match self.bits() % 8 {
            0 => Vf::new(self.f(-1.0, 1.0).0, [1.0, -1.0, 1.2, -1.2][(self.bits() % 4) as usize], self.f(-1.0, 1.0).0, 1.0),
            1 if self.one_in(4) => Vf::new(0.0, 0.5, 0.0, 1.0),
            _ => self.vec(-1.0, 1.0),
        };
        Tail {
            vehicle_state,
            rider_held: self.one_in(12),
            // Exactly the speed, now and then: at it is not slow.
            stunt_speed: if self.one_in(8) { s.q.speed() } else { self.f(0.0, 15.0) },
            surfaces,
            wheel_surface: [0; 4].map(|_: u8| (self.bits() % 16) as u8),
            wheel_sunk: [0; 4].map(|_: u8| self.one_in(3)),
            wheel_squeeze: [0; 4].map(|_: u8| squeeze(self)),
            on_hazard: self.b(),
            request: word(self),
            request_was: word(self),
            pad: !self.one_in(5),
            rumble_on: !self.one_in(5),
            wheelie_clock: clock(self),
            stoppie_clock: clock(self),
            wheelie_stunt: self.bits() % 64,
            stoppie_stunt: self.bits() % 64,
            wheelie_time: time(self),
            stoppie_time: time(self),
            wheelie_dist: self.f(0.0, 500.0),
            stoppie_dist: self.f(0.0, 500.0),
            best_wheelie_time: self.f(0.0, 6.0),
            best_stoppie_time: self.f(0.0, 6.0),
            best_wheelie_dist: self.f(0.0, 600.0),
            best_stoppie_dist: self.f(0.0, 600.0),
            player,
            race,
            ride_time: self.f(0.0, 1000.0),
            top_speed: self.f(0.0, 80.0),
            distance: self.f(0.0, 10000.0),
            word_falls: self.bits() % 100,
            word_was: word(self),
            node_up: self.vec(-1.0, 1.0),
            node_forward,
            second_on: word(self),
            second_countdown: if self.one_in(5) { F::ZERO } else { self.f(-0.1, 0.5) },
            clock: self.f(0.0, 100.0),
            post: self.bits(),
        }
    }
}

/// Which way the update went, for the tally (read off the state before either side runs).
#[allow(clippy::float_arithmetic)]
fn tail_paths(s: &State, tl: &Tail) -> Vec<&'static str> {
    let (q, dt) = (&s.q, s.dt);
    let Some(mode) = tl.vehicle_state else { return vec!["no vehicle state"] };
    if tl.rider_held {
        return vec!["rider held"];
    }
    let mut p = Vec::new();
    let surface = tl.wheel_surface.map(|i| tl.surfaces[i as usize]);
    p.push(match (surface.iter().any(|s| s.hazard), tl.wheel_sunk.iter().any(|&s| s)) {
        (false, _) => "no hazard",
        (true, false) => "on a hazard",
        (true, true) => "sunk in a hazard",
    });
    if tl.request != 0 && tl.request_was != 0 {
        p.push("request cleared");
    }
    if q.speed_len.0 > 10.0 && mode == 1 && !q.control_off {
        let sq = tl.wheel_squeeze.iter().map(|f| f.0).fold(f32::MIN, f32::max);
        p.push(match (sq > 0.95, sq > 0.73, tl.pad && tl.rumble_on) {
            (true, _, true) => "rumble hard",
            (false, true, true) => "rumble soft",
            (_, true, false) => "rumble, no pad or off",
            _ => "no rumble",
        });
    } else {
        p.push("rumble gated");
    }
    let slow = q.speed().0 < tl.stunt_speed.0;
    let one = |clock: F, time: F| match (clock.0 + dt.0 > 1.0, time.0 <= 0.0) {
        (false, _) => "clock",
        (true, true) => "starts",
        (true, false) => "runs",
    };
    p.push(if q.control_off {
        "wrecked"
    } else if !q.front_down && q.rear_down && !slow {
        match one(tl.wheelie_clock, tl.wheelie_time) {
            "clock" => "wheelie clock",
            "starts" => "wheelie starts",
            _ => "wheelie runs",
        }
    } else if q.front_down && !q.rear_down && !slow {
        match one(tl.stoppie_clock, tl.stoppie_time) {
            "clock" => "stoppie clock",
            "starts" => "stoppie starts",
            _ => "stoppie runs",
        }
    } else if (q.front_down && q.rear_down) || (!q.airborne && slow) || (q.airborne && q.hang_time.0 > 0.1) {
        if tl.wheelie_time.0 > 0.0 || tl.stoppie_time.0 > 0.0 { "down, stunt ended" } else { "down" }
    } else {
        "left be"
    });
    if q.speed() == tl.stunt_speed {
        p.push("speed exactly the stunt speed");
    }
    if q.airborne && q.hang_time == F(0.1) {
        p.push("hang exactly 0.1");
    }
    match tl.player {
        None => p.push("no player"),
        Some(pl) => {
            p.push(if q.front_down {
                "distance from the speed"
            } else if tl.node_forward.y() <= -1.0 || tl.node_forward.y() >= 1.0 {
                "distance along the up row"
            } else {
                "distance along the forward row"
            });
            if pl.flags & 0x200 != 0 {
                p.push("ride clock held");
            }
            if tl.word_was != 0 && pl.word == 0 {
                p.push("word fell");
            }
            if let Some(r) = tl.race.filter(|r| pl.countdown.0 > 0.0 && r.clock.0 <= 0.0) {
                p.push(if pl.countdown.0 - dt.0 <= 0.0 { if pl.listeners > 0 && r.watchers > 1 { "countdown out, told" } else { "countdown out" } } else { "countdown runs" });
            }
        }
    }
    if tl.second_on != 0 && mode == 1 {
        p.push(if tl.second_countdown.0 < 0.0 { "second countdown off" } else { "second countdown runs" });
    }
    p
}

#[test]
fn the_quads_update_is_the_originals() {
    let Some(mut ee) = image() else { return };
    assert_eq!(ee.u32_at(QUAD_VTABLE + 0x45c), HAZARD_CLOCK, "the quad's hazard-clock slot");
    assert_eq!(ee.u32_at(QUAD_VTABLE + 0x4b8), START_STUNT, "the quad's stunt-start slot");
    assert_eq!(ee.u32_at(QUAD_VTABLE + 0x4ac), END_STUNT, "the quad's stunt-end slot");
    let log: Log<Call> = Rc::default();
    let hook = |ee: &mut Ee, at: u32, f: fn(&Ee) -> Call| {
        let l = log.clone();
        ee.hooks.insert(at, Box::new(move |ee| l.borrow_mut().push(f(ee))));
    };
    hook(&mut ee, BIKE_UPDATE, |ee| on_quad(ee, Call::BikeVehicleUpdate(F(ee.f(12)))));
    hook(&mut ee, HAZARD_CLOCK, |ee| {
        assert!(ee.r(5) <= 1, "the sunk flag is {:#x}", ee.r(5));
        on_quad(ee, Call::HazardSurface { dt: F(ee.f(12)), time: F(ee.f(13)), sunk: ee.r(5) == 1 })
    });
    hook(&mut ee, START_STUNT, |ee| {
        assert_eq!(ee.r(6), 1, "the stunt start's last argument");
        on_quad(ee, Call::StartStunt(ee.r(5) as u32))
    });
    hook(&mut ee, END_STUNT, |ee| {
        assert_eq!(ee.r(5) as i64, -1, "the stunt end's stunt");
        on_quad(ee, Call::EndStunt)
    });
    hook(&mut ee, RUMBLE_FN, |ee| {
        assert_eq!((ee.r(4) as u32, ee.f(12)), (PAD, 0.2), "the rumble's pad and time");
        assert!(ee.r(5) <= 1, "the rumble's soft flag is {:#x}", ee.r(5));
        Call::Rumble { soft: ee.r(5) == 1, strength: ee.r(6) as i32 }
    });
    hook(&mut ee, COUNTDOWN_FN, |ee| {
        assert_eq!((ee.r(4) as u32, ee.r(5) as u32), (OBJ + 0x680, RACE), "the countdown's arguments");
        Call::CountdownOut
    });
    hook(&mut ee, LISTENER_FN, |ee| {
        assert_eq!((ee.r(5) as u32, ee.r(6) as u32), (RACE, PLAYER), "a listener's arguments");
        Call::CountdownListener((ee.r(4) as u32 - LISTENERS) / 0x40)
    });
    hook(&mut ee, WATCHER_FN, |ee| {
        assert_eq!((ee.r(5) as u32, ee.r(6) as u32), (RACE, PLAYER), "a watcher's arguments");
        Call::CountdownWatcher((ee.r(4) as u32 - WATCHERS) / 0x40)
    });
    hook(&mut ee, POST, |ee| {
        let want = (u64::from(ee.u32_at(POST_QUEUE)), u64::from(OBJ + 0x674), 8, 4, 0);
        assert_eq!((ee.r(4), ee.r(5), ee.r(6), ee.r(7), ee.r(9)), want, "the post's arguments");
        Call::Post(ee.r(8) as u32)
    });
    ee.hooks.insert(NAME_BIT, Box::new(|ee| ee.set(2, RIDER_NAME)));
    ee.hooks.insert(
        RIDER_FN,
        Box::new(|ee| {
            assert_eq!((ee.r(4) as u32, ee.r(5)), (STATE_OBJ, RIDER_NAME), "the rider lookup's arguments");
            ee.set(2, u64::from(RIDER + 0x2ec));
        }),
    );
    let rumble_on = Rc::new(Cell::new(false));
    let on = rumble_on.clone();
    ee.hooks.insert(
        RUMBLE_ON,
        Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, RUMBLE_TOKEN, "the rumble query's argument");
            ee.set(2, u64::from(on.get()));
        }),
    );

    let mut g = Gen(0x0064_a001);
    let mut tally_of = BTreeMap::new();
    for k in 0..10_000 {
        let mut s = g.state();
        let tl = g.tail(&mut s);
        for path in tail_paths(&s, &tl) {
            tally(&mut tally_of, path);
        }
        rumble_on.set(tl.rumble_on);

        // The original.
        log.borrow_mut().clear();
        reset(&mut ee);
        poke_tail(&mut ee, &s, &tl);
        ee.call(UPDATE, &[u64::from(OBJ)], &[s.dt.0], 50_000);
        only_touched_the_regions(&ee).unwrap_or_else(|e| panic!("state {k}: {e}\n{s:#?}\n{tl:#?}"));
        let was = snap(&ee);

        // The port.
        let (mut t, mut tp) = (s.clone(), tl);
        let mut calls = Vec::new();
        quad::update(&mut tp, &mut t.q, &mut t.body, t.dt, &mut calls);
        reset(&mut ee);
        poke_tail(&mut ee, &t, &tp);
        let now = snap(&ee);
        if was != now {
            let i = was.iter().zip(&now).position(|(a, b)| a != b).unwrap();
            panic!(
                "state {k}: {} is {:#010x} ({}) in the original and {:#010x} ({}) in the port\n{s:#?}\n{tl:#?}",
                where_is(i),
                was[i],
                f32::from_bits(was[i]),
                now[i],
                f32::from_bits(now[i]),
            );
        }
        assert_eq!(*log.borrow(), calls, "state {k}: the calls\n{s:#?}\n{tl:#?}");
    }
    each_path_seen(
        &tally_of,
        &[
            "no vehicle state",
            "rider held",
            "no hazard",
            "on a hazard",
            "sunk in a hazard",
            "request cleared",
            "rumble hard",
            "rumble soft",
            "rumble, no pad or off",
            "no rumble",
            "rumble gated",
            "wrecked",
            "wheelie clock",
            "wheelie starts",
            "wheelie runs",
            "stoppie clock",
            "stoppie starts",
            "stoppie runs",
            "down, stunt ended",
            "down",
            "left be",
            "hang exactly 0.1",
            "speed exactly the stunt speed",
            "no player",
            "distance from the speed",
            "distance along the up row",
            "distance along the forward row",
            "ride clock held",
            "word fell",
            "countdown out",
            "countdown out, told",
            "countdown runs",
            "second countdown off",
            "second countdown runs",
        ],
        30,
    );
    eprintln!("{tally_of:?}");
}

// --- MD64b: the tire reader ------------------------------------------------------------------------

/// The quad's `[Front Tire]`/`[Rear Tire]` reader, the ini's section select and string lookup (the
/// flag, int, float and vector reads all come down to it), the kernel's syscall stubs, and the sweep
/// shape rebuild the wheels' derived setter calls.
const TIRE_PHY: u32 = 0x004b_b880;
const INI_SECTION: u32 = 0x0014_4140;
const INI_STRING: u32 = 0x0014_2ce0;
const SYSCALLS: (u32, u32) = (0x0033_e800, 0x0033_f400);
const SWEEP_REBUILD: u32 = 0x001b_b3f0;
/// The C library's reent pointer: the vector read's `strtok` keeps its place at `+0x5c` of it, and
/// `strtod`'s freelist is at `+0x4c`.
const IMPURE_PTR: u32 = 0x0039_3a90;
/// The ini the reader is handed (the hooks never read it) and the object the wheels' mass is read
/// off.
const INI: u32 = 0x01c0_d000;
const MASS_OBJ: u32 = 0x01c0_d100;
/// The reader's reach: the quad's tire words and the four wheels up to their steering torque.
const TIRE_REGIONS: [(&str, u32, u32); 5] = [
    ("quad", OBJ, 0x17c0),
    ("left front", 0x01c0_8000, 0x8b8),
    ("right front", 0x01c0_9000, 0x8b8),
    ("left rear", 0x01c0_a000, 0x8b8),
    ("right rear", 0x01c0_b000, 0x8b8),
];
/// The wheels' pointers in the quad, in [`quad::LEFT_FRONT`]'s order.
const QUAD_WHEELS: u32 = 0x17b0;

fn tire_cstr(ee: &Ee, mut a: u32) -> String {
    let mut s = String::new();
    loop {
        let c = ee.u8_at(a);
        if c == 0 {
            return s;
        }
        s.push(char::from(c));
        a += 1;
    }
}

fn tire_set_byte(ee: &mut Ee, a: u32, b: u8) {
    let (at, shift) = (a & !3, 8 * (a & 3));
    ee.set_u32(at, ee.u32_at(at) & !(0xff << shift) | u32::from(b) << shift);
}

/// A generated tire file: each section's keys and their text.
type TireFile = BTreeMap<&'static str, BTreeMap<String, String>>;

/// One sweep rebuild call: the shape, the 1, the segment's two ends, and the two zeros.
type SweepCall = (u32, u32, Vf, Vf, u32, u32);

/// The ini's lookups answered from `file`: the section select remembers the section, the string
/// lookup copies the value's text into the caller's buffer as the game's line reader keeps it (with
/// the whitespace squeezed out, which [`Ini::parse`] does as the original's does).
fn hook_tire_ini(ee: &mut Ee, file: &Rc<RefCell<Ini>>, sweeps: &Rc<RefCell<Vec<SweepCall>>>) {
    let section = Rc::new(RefCell::new(String::new()));
    let at = section.clone();
    ee.hooks.insert(INI_SECTION, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, INI, "the section select is not handed the ini");
        *at.borrow_mut() = tire_cstr(ee, ee.r(5) as u32);
        ee.set(2, 1);
    }));
    let f = file.clone();
    ee.hooks.insert(INI_STRING, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, INI + 0x44, "the lookup is not handed the ini's table");
        let key = tire_cstr(ee, ee.r(5) as u32);
        let (buf, len) = (ee.r(6) as u32, ee.r(7) as u32);
        let found = match f.borrow().section(section.borrow().as_str()).text(&key) {
            Some(text) => {
                assert!((text.len() as u32) < len, "{key}'s text does not fit");
                for (i, b) in text.bytes().chain([0]).enumerate() {
                    tire_set_byte(ee, buf + i as u32, b);
                }
                1
            }
            None => 0,
        };
        ee.set(2, found);
    }));
    for a in (SYSCALLS.0..SYSCALLS.1).step_by(4) {
        if ee.u32_at(a) >> 16 == 0x2403 && ee.u32_at(a + 4) == 0x0000_000c {
            ee.hooks.insert(a, Box::new(|ee| ee.set(2, 0)));
        }
    }
    let s = sweeps.clone();
    ee.hooks.insert(SWEEP_REBUILD, Box::new(move |ee| {
        let v = |a: u32| Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12));
        let ends = ee.r(6) as u32;
        s.borrow_mut().push((ee.r(4) as u32, ee.r(5) as u32, v(ends), v(ends + 16), ee.r(7) as u32, ee.r(8) as u32));
    }));
}

/// The wheel words the port's [`quad::WheelTire`] holds, as `(offset, field)`.
fn wheel_words(w: &mut quad::WheelTire) -> Vec<(u32, &mut F)> {
    vec![
        (0x3c, &mut w.strut_compression),
        (0x40, &mut w.max_compression_rate),
        (0x44, &mut w.max_decompression_rate),
        (0x48, &mut w.spring),
        (0x4c, &mut w.damping_in),
        (0x50, &mut w.damping_out),
        (0x54, &mut w.spin_inertia),
        (0x58, &mut w.inv_spin_inertia),
        (0x5c, &mut w.tire_inertia),
        (0x64, &mut w.steer_damping),
        (0x68, &mut w.steer_inertia),
        (0x6c, &mut w.inv_steer_inertia),
        (0x70, &mut w.max_steer_angle),
        (0x74, &mut w.min_steer_angle),
        (0xb8, &mut w.other_load_scale),
        (0x214, &mut w.radius),
        (0x218, &mut w.ply),
        (0x21c, &mut w.width),
        (0x220, &mut w.tire_mass),
        (0x838, &mut w.inv_radius),
        (0x8a8, &mut w.ply_scale),
        (0x8b4, &mut w.steer_torque),
    ]
}

fn wheel_vectors(w: &mut quad::WheelTire) -> [(u32, &mut Vf); 3] {
    [(0x90, &mut w.mount), (0xa0, &mut w.force_position), (0x100, &mut w.rest)]
}

/// A wheel's surface table `t` (the drive setter's eight, the side setter's thirteen, the drag
/// setter's three), surface `i`.
fn table_word(w: &mut quad::WheelTire, t: usize, i: usize) -> &mut F {
    let s = &mut w.surfaces[i];
    match t {
        0..8 => &mut s.drive[t],
        8..21 => &mut s.side[t - 8],
        _ => &mut s.drag[t - 21],
    }
}

fn get_v(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

/// What the RAM holds at the port's fields, for the quad at `obj`.
fn read_tire(ee: &Ee, obj: u32) -> quad::TireSetup {
    let mut t = quad::TireSetup::default();
    for (k, w) in t.wheels.iter_mut().enumerate() {
        let base = ee.u32_at(obj + QUAD_WHEELS + 4 * k as u32);
        w.parametric = ee.u32_at(base + 0x34) as i32;
        w.sweep = ee.u32_at(base + 0x1dc) != 0;
        for (o, f) in wheel_words(w) {
            *f = F(ee.f32_at(base + o));
        }
        for (o, v) in wheel_vectors(w) {
            *v = get_v(ee, base + o);
        }
        for tb in 0..24 {
            for i in 0..16 {
                *table_word(w, tb, i) = F(ee.f32_at(base + 0x238 + 0x40 * tb as u32 + 4 * i as u32));
            }
        }
    }
    t.front_spring = (F(ee.f32_at(obj + 0x137c)), F(ee.f32_at(obj + 0x1380)));
    t.rear_spring = (F(ee.f32_at(obj + 0x1384)), F(ee.f32_at(obj + 0x1388)));
    for i in 0..16 {
        t.mud_time[i] = F(ee.f32_at(obj + 0x1654 + 4 * i as u32));
        t.mud_amount[i] = F(ee.f32_at(obj + 0x1694 + 4 * i as u32));
    }
    t
}

/// The port's fields written back over the RAM.
fn write_tire(ee: &mut Ee, t: &quad::TireSetup) {
    let mut t = *t;
    for (k, w) in t.wheels.iter_mut().enumerate() {
        let base = ee.u32_at(OBJ + QUAD_WHEELS + 4 * k as u32);
        ee.set_u32(base + 0x34, w.parametric as u32);
        for (o, f) in wheel_words(w) {
            ee.set_f32(base + o, f.0);
        }
        for (o, v) in wheel_vectors(w) {
            put_v(ee, base + o, *v);
        }
        for tb in 0..24 {
            for i in 0..16 {
                ee.set_f32(base + 0x238 + 0x40 * tb as u32 + 4 * i as u32, table_word(w, tb, i).0);
            }
        }
    }
    put_f(ee, OBJ + 0x137c, t.front_spring.0);
    put_f(ee, OBJ + 0x1380, t.front_spring.1);
    put_f(ee, OBJ + 0x1384, t.rear_spring.0);
    put_f(ee, OBJ + 0x1388, t.rear_spring.1);
    for i in 0..16 {
        put_f(ee, OBJ + 0x1654 + 4 * i as u32, t.mud_time[i]);
        put_f(ee, OBJ + 0x1694 + 4 * i as u32, t.mud_amount[i]);
    }
}

/// The quad and its wheels as a generated case lays them out: every word a finite number in a few
/// hundred, then the wheel pointers, the mass, the shapes and the mounts.
struct TireLayout {
    words: Vec<u32>,
    mass: F,
    shapes: [u32; 4],
}

#[allow(clippy::float_arithmetic)]
impl Gen {
    /// A value as a file writes one: whole, a few places, many, signed, sometimes on 0 or 1.
    fn number(&mut self, lo: f32, hi: f32) -> String {
        let v = lo + (hi - lo) * self.unit();
        match self.bits() % 8 {
            0 => "0".into(),
            1 => "1".into(),
            2 => format!("{}", v.round() as i32),
            3 => format!("{v:.1}"),
            4 => format!("{v:.2}"),
            5 => format!("{v:.4}"),
            6 => format!("{v:.7}"),
            _ => format!("{v}"),
        }
    }
    fn surface_count(&mut self) -> String {
        match self.bits() % 12 {
            0 => "16".into(),
            1 => "0".into(),
            2 => "-2".into(),
            3 => "1".into(),
            4 => format!("{}", 2 + self.bits() % 14),
            _ => format!("{}", 1 + self.bits() % 3),
        }
    }
    /// One section of a tire file, each key the reader asks for present four times in five.
    fn tire_section(&mut self, front: bool) -> BTreeMap<String, String> {
        let mut m = BTreeMap::new();
        let put = |g: &mut Gen, m: &mut BTreeMap<String, String>, k: &str, v: String| {
            if !g.one_in(5) {
                m.insert(k.to_string(), v);
            }
        };
        let flag = ["1", "0", "2", "-3", "0x10", "True", "false", "Yes", "n", "On", "OFF", "T", "F"];
        let word = flag[self.bits() as usize % flag.len()].to_string();
        put(self, &mut m, "UseParametricCurve", word);
        for (k, lo, hi) in [
            ("StrutCompression", 0.0, 2.0),
            ("MaxCompressionRate", 0.0, 40.0),
            ("MaxDecompressionRate", 0.0, 40.0),
            ("TireRadius", -0.5, 1.5),
            ("TireInertia", -1.0, 3.0),
            ("Spring", -10.0, 60.0),
            ("SpringWhenStopped", -10.0, 60.0),
            ("BounceDampingRatio", 0.0, 2.0),
            ("ReboundDampingRatio", 0.0, 2.0),
        ] {
            let v = self.number(lo, hi);
            put(self, &mut m, k, v);
        }
        let v = if self.b() {
            format!("{}, {}, {}", self.number(-2.0, 2.0), self.number(-2.0, 2.0), self.number(-2.0, 2.0))
        } else {
            format!("{},{}, {} ,{}", self.number(-2.0, 2.0), self.number(-2.0, 2.0), self.number(-2.0, 2.0), self.number(-1.0, 2.0))
        };
        put(self, &mut m, "BodyTireForcePosition", v);
        if front {
            for (k, lo, hi) in [
                ("SteeringTorque", -50.0, 400.0),
                ("SteeringDamping", -5.0, 40.0),
                ("SteeringInertia", -1.0, 2.0),
                ("MaxSteerAngle", -10.0, 60.0),
            ] {
                let v = self.number(lo, hi);
                put(self, &mut m, k, v);
            }
            m.insert("NumberOfSurfaces".into(), self.surface_count());
        } else {
            let v = self.number(0.0, 2.0);
            put(self, &mut m, "OtherTireNormalLoadScale", v);
            if !self.one_in(4) {
                m.insert("NumberOfSurfaces".into(), self.surface_count());
            }
        }
        let mut keys = vec![
            "SideForce", "SlippingSideForce", "MaxDriveForce", "MaxSideForce", "MaxSlippingSideForce",
            "SkidDriveForce", "SkidSideForce", "SkidSlippingSideForce", "SlippingSideForceRoundOff",
            "SlippingSideForceDropOff", "SlippingSideForceScale", "SlippingSideForceSpeedScale",
            "BrakingSideForceRoundOff", "BrakingSideForceDropOff", "BrakingSideForceScale",
            "BrakingSideForceSpeedScale", "SideForceRoundOff", "SideForceDropOff", "SideForceScale",
            "SideForceSpeedScale", "MaxBrakingTorque", "WheelFriction", "WheelDrag",
        ];
        // Each side's own: the reader asks the front for mud and the rear for the donut scale; the
        // other key is written too, to show it is not read.
        keys.extend(["MudTime", "MudAmount", "DonutsSideForceSpeedScale"]);
        for i in 1..=16 {
            for k in &keys {
                let v = self.number(-20.0, 200.0);
                put(self, &mut m, &format!("{k}{i}"), v);
            }
        }
        m
    }
    fn tire_file(&mut self) -> TireFile {
        let mut f = TireFile::new();
        if !self.one_in(50) {
            f.insert("Front Tire", self.tire_section(true));
        } else {
            // No front section: only its count is asked for (the missing int leaves a stack word).
            f.insert("Front Tire", BTreeMap::from([("NumberOfSurfaces".to_string(), self.surface_count())]));
        }
        if !self.one_in(50) {
            f.insert("Rear Tire", self.tire_section(false));
        }
        f
    }
    fn tire_layout(&mut self) -> TireLayout {
        let mut words = vec![];
        for &(_, _, len) in &TIRE_REGIONS {
            for _ in (0..len).step_by(4) {
                words.push(self.f(-300.0, 300.0).0.to_bits());
            }
        }
        let shapes = std::array::from_fn(|_| if self.b() { 0 } else { 0x01c0_e000 + (self.bits() & 0xff0) });
        TireLayout { words, mass: self.f(5.0, 40.0), shapes }
    }
}

fn tire_text(file: &TireFile) -> String {
    let mut t = String::new();
    for (name, keys) in file {
        t.push_str(&format!("[{name}]\n"));
        for (k, v) in keys {
            t.push_str(&format!("{k} = {v}\n"));
        }
    }
    t
}

fn lay_out_tires(ee: &mut Ee, l: &TireLayout) {
    let mut words = l.words.iter();
    for &(_, base, len) in &TIRE_REGIONS {
        for o in (0..len).step_by(4) {
            ee.set_u32(base + o, *words.next().unwrap());
        }
    }
    ee.set_u32(OBJ + 0x4, QUAD_VTABLE);
    for (k, &w) in WHEEL.iter().enumerate() {
        ee.set_u32(OBJ + QUAD_WHEELS + 4 * k as u32, w);
        ee.set_u32(w + 0x1dc, l.shapes[k]);
        ee.set_u32(w + 0x1e4, MASS_OBJ);
    }
    put_f(ee, MASS_OBJ + 0x18, l.mass);
}

fn tire_snap(ee: &Ee) -> Vec<u32> {
    TIRE_REGIONS.iter().flat_map(|&(_, base, len)| (0..len).step_by(4).map(move |o| base + o)).map(|a| ee.u32_at(a)).collect()
}

fn tire_where(i: usize) -> String {
    let mut i = i as u32 * 4;
    for (name, _, len) in TIRE_REGIONS {
        if i < len {
            return format!("{name} + {i:#x}");
        }
        i -= len;
    }
    "past the end".into()
}

fn tire_paths(file: &TireFile, l: &TireLayout, t: &quad::TireSetup) -> Vec<&'static str> {
    let front = &file["Front Tire"];
    let rear = file.get("Rear Tire");
    fn count(s: Option<&BTreeMap<String, String>>) -> Option<&str> {
        s.and_then(|s| s.get("NumberOfSurfaces")).map(String::as_str)
    }
    let mut p = vec![];
    p.push(match count(Some(front)) {
        Some("16") => "front 16 surfaces",
        Some("0") => "front no surfaces",
        Some("-2") => "front negative count",
        _ => "front some surfaces",
    });
    p.push(match count(rear) {
        None => "rear count is the front's",
        Some("16") => "rear 16 surfaces",
        _ => "rear own count",
    });
    if rear.is_none() {
        p.push("no rear section");
    }
    if !front.contains_key("TireRadius") {
        p.push("no front radius");
    }
    for (k, w) in t.wheels.iter().enumerate() {
        p.push(if w.radius.0 > 0.0 { "radius > 0" } else { "radius <= 0" });
        if k == 0 {
            p.push(if w.steer_inertia.0 > 0.0 && front.contains_key("SteeringInertia") {
                "steering inertia > 0"
            } else {
                "steering inertia kept"
            });
        }
        p.push(if l.shapes[k] != 0 { "sweep rebuilt" } else { "no sweep" });
    }
    p.push(match front.get("UseParametricCurve").map(String::as_str) {
        None => "flag missing",
        Some("True" | "T" | "Yes" | "On") => "flag word true",
        Some("false" | "F" | "n" | "OFF") => "flag word false",
        Some(_) => "flag number",
    });
    p
}

/// SLUS-21104's quad tire reader, [`quad::tire_phy`]: 10,000 generated `[Front Tire]`/`[Rear Tire]`
/// files over quads and wheels filled with noise. Every word of the quad and the four wheels is
/// compared (what the reader does not reach must be left alone), the sweep rebuilds must match, and
/// the original may write nothing but them and its stack.
#[test]
fn the_quads_tire_reader_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let file = Rc::new(RefCell::new(Ini::default()));
    let sweeps = Rc::new(RefCell::new(vec![]));
    hook_tire_ini(&mut ee, &file, &sweeps);
    let mut g = Gen(0x64b0_0001);
    let mut tally = BTreeMap::new();
    let strtok_last = ee.u32_at(IMPURE_PTR) + 0x5c;
    // `strtod`'s big numbers come out of a pool the image has already allocated: its freelist and the
    // blocks after it.
    let pool = ee.u32_at(ee.u32_at(IMPURE_PTR) + 0x4c);
    for k in 0..10_000 {
        let f = g.tire_file();
        let l = g.tire_layout();
        let ini = Ini::parse(&tire_text(&f));
        *file.borrow_mut() = ini.clone();
        sweeps.borrow_mut().clear();
        lay_out_tires(&mut ee, &l);
        // A missing front count leaves the reader's stack word: make it a known zero.
        let frame = ee.stack - 0x2000;
        for o in (0..0x2000).step_by(4) {
            ee.set_u32(frame + o, 0);
        }
        ee.call(TIRE_PHY, &[u64::from(OBJ), u64::from(INI)], &[], 20_000_000);
        for (a, b) in ee.written_ranges() {
            let inside = TIRE_REGIONS.iter().any(|&(_, base, len)| a >= base && b <= base + len)
                || (a >= frame && b <= ee.stack + 0x40)
                || (a, b) == (strtok_last, strtok_last + 4)
                || (a >= pool && b <= pool + 0x400);
            assert!(inside, "file {k}: the original wrote {a:#010x}..{b:#010x}\n{}", tire_text(&f));
        }
        let was = tire_snap(&ee);
        let original_sweeps = sweeps.borrow().clone();

        lay_out_tires(&mut ee, &l);
        let mut t = read_tire(&ee, OBJ);
        let rebuilt = quad::tire_phy(&mut t, &ini, l.mass);
        write_tire(&mut ee, &t);
        let now = tire_snap(&ee);
        if was != now {
            let i = was.iter().zip(&now).position(|(a, b)| a != b).unwrap();
            panic!(
                "file {k}: {} is {:#010x} ({}) in the original and {:#010x} ({}) in the port\n{}",
                tire_where(i),
                was[i],
                f32::from_bits(was[i]),
                now[i],
                f32::from_bits(now[i]),
                tire_text(&f),
            );
        }
        let port_sweeps: Vec<SweepCall> = rebuilt
            .iter()
            .zip(l.shapes)
            .filter_map(|(r, shape)| r.map(|[a, b]| (shape, 1, a, b, 0, 0)))
            .collect();
        let bits = |c: &[SweepCall]| c.iter().map(|&(s, o, a, b, x, y)| (s, o, a.0.map(f32::to_bits), b.0.map(f32::to_bits), x, y)).collect::<Vec<_>>();
        assert_eq!(bits(&original_sweeps), bits(&port_sweeps), "file {k}: the sweep rebuilds differ\n{}", tire_text(&f));
        for p in tire_paths(&f, &l, &t) {
            *tally.entry(p).or_insert(0u32) += 1;
        }
    }
    each_path_seen(
        &tally,
        &[
            "front 16 surfaces",
            "front no surfaces",
            "front negative count",
            "front some surfaces",
            "rear count is the front's",
            "rear 16 surfaces",
            "rear own count",
            "no rear section",
            "no front radius",
            "radius > 0",
            "radius <= 0",
            "steering inertia > 0",
            "steering inertia kept",
            "sweep rebuilt",
            "no sweep",
            "flag missing",
            "flag word true",
            "flag word false",
            "flag number",
        ],
        50,
    );
    eprintln!("{tally:?}");
}

/// The quad in each of MD37a's RAM images, and the files its `.phy` may be: the `r` images ride the
/// `atv_` quad, the `s` ones the `atv_s_` (MD37a's poke), each a Race or a Pro, A or P.
const QUAD_IMAGES: [(&str, u32, &str); 6] = [
    ("atv-r-spawn", 0x015e_0690, "atv_"),
    ("atv-r-air", 0x015e_0690, "atv_"),
    ("atv-r-spawn-late", 0x015e_0690, "atv_"),
    ("atv-s-spawn", 0x015e_0b90, "atv_s_"),
    ("atv-s-air", 0x015e_0b90, "atv_s_"),
    ("atv-s-spawn-late", 0x015e_0b90, "atv_s_"),
];
const QUAD_FILES: [&str; 4] = ["race250a", "race250p", "pro250a", "pro250p"];

/// The reader's words a ride moves on: the spring the ground step swaps between stopped and moving,
/// and the rest point.
fn settled(mut t: quad::TireSetup) -> quad::TireSetup {
    for w in &mut t.wheels {
        w.spring = F::ZERO;
        w.rest = Vf::ZERO;
    }
    t
}

/// The image's setup with every scalar word the reader writes made a NaN, so that a word the port
/// fails to write cannot pass on the image's own value. Kept: what the reader reads (mount, sweep
/// shape), the rears' steering lock (their constructor's, which the right rear copies), the left
/// rear's steering (the reader never sets it) and the fronts' `OtherTireNormalLoadScale` (likewise).
fn poisoned(image: &quad::TireSetup) -> quad::TireSetup {
    let mut t = *image;
    let nan = F(f32::NAN);
    for (k, w) in t.wheels.iter_mut().enumerate() {
        let keep = *w;
        for (_, f) in wheel_words(w) {
            *f = nan;
        }
        w.parametric = -7;
        w.force_position = Vf::splat(f32::NAN);
        if k >= quad::LEFT_REAR {
            (w.max_steer_angle, w.min_steer_angle) = (keep.max_steer_angle, keep.min_steer_angle);
        } else {
            w.other_load_scale = keep.other_load_scale;
        }
        if k == quad::LEFT_REAR {
            (w.steer_damping, w.steer_inertia, w.inv_steer_inertia) = (keep.steer_damping, keep.steer_inertia, keep.inv_steer_inertia);
            w.steer_torque = keep.steer_torque;
        }
    }
    (t.front_spring, t.rear_spring) = ((nan, nan), (nan, nan));
    t
}

/// The tire reader on the quad files themselves: in each of MD37a's six RAM images the four wheels'
/// tire words, the two pairs' springs and the mud are what [`quad::tire_phy`] reads out of one of the
/// candidate `.phy` files (with the image's own mass, and from [`poisoned`] words), but for the spring
/// and rest point a ride moves. Every image must match exactly one file.
#[test]
fn the_quad_images_wheels_are_their_phy_files() {
    let Ok(kit) = Kit::load().map_err(|w| skip(&w)) else { return };
    let dir = kit.ctx("mxvatv-ps2/md37a");
    let phy = dir.join("phy");
    if !phy.exists() {
        skip(&format!(
            "{} is not there: it is MX vs. ATV's Veh\\atv_*250*.phy out of MXVSATV.PSR's vehicletype_1.res \
             (research/journal/2026-11-02-md64b-atv-tire-phy)",
            phy.display()
        ));
        return;
    }
    let mut seen = 0;
    for (name, obj, prefix) in QUAD_IMAGES {
        let path = dir.join(format!("{name}.ee{}", ".bin"));
        if !path.exists() {
            skip(&format!("{} is not there (MD37a's; memory: md37a ATV states live in s2)", path.display()));
            continue;
        }
        let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(ee.u32_at(obj + 4), QUAD_VTABLE, "{name}: no quad at {obj:#x}");
        let image = read_tire(&ee, obj);
        let mass = F(ee.f32_at(ee.u32_at(ee.u32_at(obj + QUAD_WHEELS) + 0x1e4) + 0x18));
        let mut matched = vec![];
        let mut first_miss = String::new();
        for file in QUAD_FILES {
            let at = phy.join(format!("{prefix}{file}.phy"));
            let bytes = std::fs::read(&at).unwrap_or_else(|e| panic!("{}: {e}", at.display()));
            let mut t = poisoned(&image);
            quad::tire_phy(&mut t, &Ini::parse_bytes(&bytes), mass);
            if format!("{:?}", settled(t)) == format!("{:?}", settled(image)) {
                matched.push(file);
            } else if first_miss.is_empty() {
                first_miss = format!("{prefix}{file}: {:?}", first_difference(&settled(image), &settled(t)));
            }
        }
        assert!(matched.len() == 1, "{name}: matched {matched:?} (first miss {first_miss})");
        println!("{name}: {prefix}{} (mass {})", matched[0], mass.0);
        seen += 1;
    }
    assert!(seen > 0 || QUAD_IMAGES.is_empty());
}

/// Where two setups first differ, for the failure message.
fn first_difference(a: &quad::TireSetup, b: &quad::TireSetup) -> String {
    for k in 0..4 {
        let (mut x, mut y) = (a.wheels[k], b.wheels[k]);
        for ((o, p), (_, q)) in wheel_words(&mut x).into_iter().zip(wheel_words(&mut y)) {
            if p.0.to_bits() != q.0.to_bits() {
                return format!("wheel {k} + {o:#x}: image {} port {}", p.0, q.0);
            }
        }
        for tb in 0..24 {
            for i in 0..16 {
                let (p, q) = (table_word(&mut x, tb, i).0, table_word(&mut y, tb, i).0);
                if p.to_bits() != q.to_bits() {
                    return format!("wheel {k} table {tb} surface {i}: image {p} port {q}");
                }
            }
        }
        if x != y {
            return format!("wheel {k}: {x:?} vs {y:?}");
        }
    }
    format!("quad: springs {:?} {:?} vs {:?} {:?}", a.front_spring, a.rear_spring, b.front_spring, b.rear_spring)
}

// --- MD65: the put-back and the net unpack ---------------------------------------------------------

/// The quad's put-back and net unpack, and what they call.
const PUT_BACK: u32 = 0x004b_8e30;
const WHEEL_RESET: u32 = 0x004a_04c0;
const DETACH: u32 = 0x0018_d540;
const VEHICLE_PUT_BACK: u32 = 0x004a_6f70;
const REATTACH: u32 = 0x0026_6c90;
const READ_FRAME: u32 = 0x0019_4e00;
const SET_FRAME: u32 = 0x0019_4da0;
const NET_PACK: u32 = 0x004e_2f50;
const ACTOR_MOVED: u32 = 0x004f_7590;
const NET_UNPACK: u32 = 0x004b_dd20;
const NODE_ORIENTATION: u32 = 0x0019_4650;
const NODE_LOCAL: u32 = 0x0019_5320;
const NET_POST: u32 = 0x004a_8c40;

/// What the put-back only hands on or reads: its attachment and what that hangs on, the two frame
/// nodes, and the hung list's nodes and kinds.
const ATTACH: u32 = 0x01c0_d300;
const HANG: u32 = 0x01c0_d340;
const FRAME_NODE: [u32; 2] = [0x01c0_d380, 0x01c0_d3c0];
const HUNG_NODES: u32 = 0x01c0_d400;
const HUNG_KINDS: u32 = 0x01c0_d500;
/// A hung node's own (made-up) address, by its place in the list.
fn hung_node(i: usize) -> u32 {
    0x0fa0_0000 + 0x40 * i as u32
}
/// The marker matrix the hooked frame read writes for frame node `f`.
fn frame_word(f: usize, k: u32) -> u32 {
    0x3f80_0000 + 0x100 * f as u32 + k
}

#[derive(Clone, Debug)]
struct PutBackCase {
    s: State,
    pb: quad::PutBack,
    place: bool,
    keep: bool,
}

fn poke_put_back(ee: &mut Ee, c: &PutBackCase) {
    poke(ee, &c.s);
    let pb = &c.pb;
    tire_set_byte(ee, OBJ + 0x45, pb.flags);
    ee.set_u32(OBJ + 0x1480, ATTACH);
    ee.set_u32(OBJ + 0x1284, HANG);
    ee.set_u32(OBJ + 0x1104, FRAME_NODE[0]);
    ee.set_u32(OBJ + 0x1108, FRAME_NODE[1]);
    ee.set_u32(OBJ + 0x1470, pb.hung.len() as u32);
    ee.set_u32(OBJ + 0x1474, HUNG_NODES);
    ee.set_u32(OBJ + 0x1478, HUNG_KINDS);
    for (i, &kind) in pb.hung.iter().enumerate() {
        ee.set_u32(HUNG_NODES + 4 * i as u32, hung_node(i));
        ee.set_u32(HUNG_KINDS + 4 * i as u32, kind as u32);
    }
    for (k, &w) in pb.respawn.iter().enumerate() {
        ee.set_u32(OBJ + 0x1738 + 4 * k as u32, w);
    }
    match pb.player_respawn {
        Some(p) => {
            ee.set_u32(OBJ + 0x680, PLAYER);
            for (k, &w) in p.iter().enumerate() {
                ee.set_u32(PLAYER + 0x94 + 4 * k as u32, w);
            }
        }
        None => ee.set_u32(OBJ + 0x680, 0),
    }
    put_f(ee, OBJ + 0x141c, pb.settle);
    ee.set_u32(OBJ + 0x31c, pb.request);
}

impl Gen {
    fn put_back_case(&mut self) -> PutBackCase {
        let s = self.state();
        let n = match self.bits() % 6 {
            0 => 0,
            1 => 7,
            _ => self.bits() % 12,
        };
        let hung = (0..n).map(|_| if self.one_in(5) { (self.bits() % 5) as i32 - 2 } else { (self.bits() % 2) as i32 }).collect();
        // The float word goes through the FPU's registers: a NaN or denormal there would only test the
        // copy, so it is an ordinary float.
        let respawn = [if self.one_in(3) { 0 } else { self.bits() | 1 }, self.bits(), self.f(-2000.0, 2000.0).0.to_bits()];
        let player_respawn = self.b().then(|| [self.bits(), self.bits(), self.f(-2000.0, 2000.0).0.to_bits()]);
        PutBackCase {
            s,
            pb: quad::PutBack {
                flags: (self.bits() & 0xff) as u8,
                hung,
                respawn,
                player_respawn,
                settle: self.f(-1.0, 1.0),
                request: self.bits() % 3,
            },
            place: self.b(),
            keep: self.b(),
        }
    }
}

fn frame_of(ee: &Ee, a: u32) -> usize {
    let f = (0..2).find(|&f| (0..16).all(|k| ee.u32_at(a + 4 * k) == frame_word(f, k)));
    f.unwrap_or_else(|| panic!("a node was handed a frame neither frame node wrote ({a:#x})"))
}

#[test]
fn the_quads_put_back_is_the_originals() {
    let Some(mut ee) = image() else { return };
    assert_eq!(ee.u32_at(QUAD_VTABLE + 0x378), NET_PACK, "the quad's snapshot-pack slot");
    let log = hook_calls(
        &mut ee,
        &[
            (WHEEL_RESET, |ee| Call::WheelReset(wheel_of(ee))),
            (DETACH, |ee| {
                assert_eq!(ee.r(4) as u32, ATTACH, "the detach's attachment");
                Call::Detach
            }),
            (VEHICLE_PUT_BACK, |ee| {
                let (place, keep) = (ee.r(5), ee.r(6));
                assert!(place <= 1 && keep <= 1, "the vehicle's put-back was handed {place:#x}, {keep:#x}");
                on_quad(ee, Call::VehiclePutBack { place: place != 0, keep: keep != 0, flags: ee.u8_at(OBJ + 0x45) })
            }),
            (REATTACH, |ee| {
                assert_eq!((ee.r(4) as u32, ee.r(5) as u32, ee.r(6), ee.r(7)), (ATTACH, HANG, 1, 1), "the reattach's arguments");
                Call::Reattach
            }),
            (SET_FRAME, |ee| {
                let a = ee.r(4) as u32;
                let hung = (0..16).position(|i| hung_node(i) == a).unwrap_or_else(|| panic!("a frame set on {a:#x}"));
                Call::SetFrame { hung, frame: frame_of(ee, ee.r(5) as u32) }
            }),
            (NET_PACK, |ee| on_quad(ee, Call::NetPack)),
            (ACTOR_MOVED, |ee| {
                assert_eq!(ee.r(4) as u32, OBJ + 0x43c, "the actor-moved notify's actor");
                Call::ActorMoved
            }),
        ],
    );
    // The frame read writes its node's marker matrix where it is asked to.
    let l = log.clone();
    ee.hooks.insert(
        READ_FRAME,
        Box::new(move |ee| {
            let a = ee.r(4) as u32;
            let f = FRAME_NODE.iter().position(|&n| n == a).unwrap_or_else(|| panic!("a frame read from {a:#x}"));
            assert_eq!(ee.r(5), 0, "the frame read's second argument");
            let out = ee.r(6) as u32;
            for k in 0..16 {
                ee.set_u32(out + 4 * k, frame_word(f, k));
            }
            l.borrow_mut().push(Call::ReadFrame(f));
        }),
    );
    let mut g = Gen(0x0065_0001);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        let c = g.put_back_case();
        let pb = &c.pb;
        tally(&mut t, if pb.flags & quad::HIDDEN_BIT != 0 { "hidden" } else { "shown" });
        tally(&mut t, match pb.player_respawn {
            None => "no player",
            Some(_) if pb.respawn[0] == 0 => "player, no respawn point",
            Some(_) => "player respawned",
        });
        for &kind in &pb.hung {
            tally(&mut t, match kind {
                0 => "hung on the first frame",
                1 => "hung on the second frame",
                _ => "hung on neither",
            });
        }
        if pb.hung.is_empty() {
            tally(&mut t, "nothing hung");
        }

        log.borrow_mut().clear();
        reset(&mut ee);
        poke_put_back(&mut ee, &c);
        ee.call(PUT_BACK, &[u64::from(OBJ), u64::from(c.place), u64::from(c.keep)], &[], 50_000);
        only_touched_the_regions(&ee).unwrap_or_else(|e| panic!("state {k}: {e}\n{c:#?}"));
        let was = snap(&ee);
        let mut d = c.clone();
        let mut calls = Vec::new();
        quad::put_back(&mut d.pb, &mut d.s.q, &mut d.s.body, d.place, d.keep, &mut calls);
        reset(&mut ee);
        poke_put_back(&mut ee, &d);
        let now = snap(&ee);
        if let Some(i) = was.iter().zip(&now).position(|(a, b)| a != b) {
            panic!("state {k}: {} is {:#010x} in the original and {:#010x} in the port\n{c:#?}", where_is(i), was[i], now[i]);
        }
        assert_eq!(*log.borrow(), calls, "state {k}: the calls\n{c:#?}");
    }
    each_path_seen(
        &t,
        &[
            "hidden",
            "shown",
            "no player",
            "player, no respawn point",
            "player respawned",
            "hung on the first frame",
            "hung on the second frame",
            "hung on neither",
            "nothing hung",
        ],
        100,
    );
}

/// Where the net unpack's snapshot and its three quadwords are laid out (only read), and the steering
/// filter it writes (laid in the player region, which nothing else here pokes).
const SNAPSHOT: u32 = 0x01c0_d600;
const STEER_FILTER: u32 = PLAYER + 0x100;

#[derive(Clone, Debug)]
struct NetCase {
    s: State,
    r: quad::Remote,
    /// What the hooked node hands back for the body's quadword in its frame.
    node_says: Vf,
}

fn poke_net(ee: &mut Ee, c: &NetCase) {
    poke(ee, &c.s);
    let r = &c.r;
    ee.set_u32(OBJ + 0xbcc, SNAPSHOT);
    for (k, v) in [r.position, r.orientation, r.velocity].into_iter().enumerate() {
        let at = SNAPSHOT + 0x10 + 0x10 * k as u32;
        ee.set_u32(SNAPSHOT + 4 * k as u32, at);
        put_v(ee, at, v);
    }
    put_v(ee, BODY + 0x1a0, r.com_offset);
    put_v(ee, BODY + 0x70, r.com);
    ee.set_u32(OBJ + 0xd1c, STEER_FILTER);
    put_f(ee, STEER_FILTER, r.steer);
    put_f(ee, STEER_FILTER + 4, r.steer_time);
    put_f(ee, STEER_FILTER + 8, r.steer_fraction);
    put_f(ee, WHEEL[LF] + 0x74, r.min_steer_angle);
    for (w, &a) in WHEEL.iter().enumerate() {
        put_f(ee, a + 0x1d0, r.spin[w]);
        put_f(ee, a + 0x1d4, r.spin_rate[w]);
    }
}

impl Gen {
    fn angle(&mut self) -> F {
        match self.bits() % 8 {
            0 => self.f(-60.0, 60.0),
            1 => F(if self.b() { PI } else { -PI }),
            2 => self.f(PI, 2.0 * PI),
            3 => self.f(-2.0 * PI, -PI),
            _ => self.f(-PI, PI),
        }
    }

    fn net_case(&mut self) -> NetCase {
        let mut s = self.state();
        // Still, now and then, so the angles stay where they were.
        if self.one_in(10) {
            s.body.velocity = Vf::new(0.0, 0.0, 0.0, self.f(-1.0, 1.0).0);
        }
        let r = quad::Remote {
            position: self.vec(-2000.0, 2000.0),
            orientation: self.quaternion(),
            velocity: if self.one_in(10) { Vf::new(0.0, 0.0, 0.0, 1.0) } else { self.vec(-60.0, 60.0) },
            com_offset: self.vec(-3.0, 3.0),
            com: self.vec(-2000.0, 2000.0),
            steer: self.f(-0.8, 0.8),
            steer_time: self.near(0.1, 0.005, 0.5),
            steer_fraction: self.f(0.0, 1.0),
            min_steer_angle: self.f(-0.7, -0.3),
            spin: [self.angle(), self.angle(), self.angle(), self.angle()],
            spin_rate: [self.f(-90.0, 90.0), self.f(-90.0, 90.0), self.f(-90.0, 90.0), self.f(-90.0, 90.0)],
        };
        let node_says = Vf::new(self.f(-3.0, 3.0).0, self.f(-3.0, 3.0).0, self.f(-3.0, 3.0).0, self.f(-1.0, 1.0).0);
        NetCase { s, r, node_says }
    }
}

/// The net unpack's callees as the port's: logged, and the node's answer written where it asks.
struct Net(Vec<Call>, Vf);

impl quad::Moto for Net {
    fn call(&mut self, c: Call, q: &mut Quad, _body: &mut Body) {
        if c == Call::NodeLocal {
            q.node_local = self.1;
        }
        self.0.push(c);
    }
}

#[test]
fn the_quads_net_unpack_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let log = hook_calls(
        &mut ee,
        &[
            (NODE_ORIENTATION, |ee| {
                assert_eq!((ee.r(4) as u32, ee.r(5), ee.r(6) as u32), (NODE, 0, BODY + 0x190), "the node orientation's arguments");
                Call::NodeOrientation
            }),
            (WHEEL_CLEAR, |ee| Call::WheelClear(wheel_of(ee))),
            (WHEEL_CONTACTS, |ee| on_quad(ee, Call::Contacts)),
            (SUSPENSION, |ee| Call::Suspension(wheel_of(ee), F(ee.f(12)))),
            (TIRE_FORCES, |ee| Call::TireForces(wheel_of(ee), F(ee.f(12)))),
            (NET_POST, |ee| on_quad(ee, Call::NetPost(F(ee.f(12))))),
        ],
    );
    // The node-local transform writes this state's answer where it is asked to.
    let says = Rc::new(Cell::new(Vf::new(0.0, 0.0, 0.0, 0.0)));
    let (l, a) = (log.clone(), says.clone());
    ee.hooks.insert(
        NODE_LOCAL,
        Box::new(move |ee| {
            assert_eq!((ee.r(5) as u32, ee.r(6) as u32), (NODE, BODY + 0xb0), "the node-local transform's arguments");
            put_v(ee, ee.r(4) as u32, a.get());
            l.borrow_mut().push(Call::NodeLocal);
        }),
    );
    let mut g = Gen(0x0065_0002);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        let c = g.net_case();
        says.set(c.node_says);
        log.borrow_mut().clear();
        reset(&mut ee);
        poke_net(&mut ee, &c);
        ee.call(NET_UNPACK, &[u64::from(OBJ)], &[c.s.dt.0], 100_000);
        only_touched_the_regions(&ee).unwrap_or_else(|e| panic!("state {k}: {e}\n{c:#?}"));
        let was = snap(&ee);
        let mut d = c.clone();
        let mut net = Net(Vec::new(), c.node_says);
        quad::net_unpack(&mut d.r, &mut d.s.q, &mut d.s.body, d.s.dt, &mut net);
        reset(&mut ee);
        poke_net(&mut ee, &d);
        let now = snap(&ee);
        if let Some(i) = was.iter().zip(&now).position(|(a, b)| a != b) {
            panic!(
                "state {k}: {} is {:#010x} ({}) in the original and {:#010x} ({}) in the port\n{c:#?}",
                where_is(i),
                was[i],
                f32::from_bits(was[i]),
                now[i],
                f32::from_bits(now[i])
            );
        }
        assert_eq!(*log.borrow(), net.0, "state {k}: the calls\n{c:#?}");

        // The paths, from the port's own steps (it matched the original's above).
        #[allow(clippy::float_arithmetic)]
        for w in 0..4 {
            let pre = c.r.spin[w].0 + d.r.spin_rate[w].0 * c.s.dt.0;
            let less = pre - (pre / (2.0 * PI)).trunc() * 2.0 * PI;
            tally(&mut t, if pre.abs() > 2.0 * PI { "whole turns" } else if less > PI { "wrapped down" } else if less < -PI { "wrapped up" } else { "inside" });
        }
        let target = F(0.5) * F(c.node_says.x());
        let lock = c.s.q.wheels[LF].max_steer_angle;
        tally(&mut t, if target < c.r.min_steer_angle { "steer at the left lock" } else if target > lock { "steer at the right lock" } else { "steer inside" });
        tally(&mut t, if c.s.dt < c.r.steer_time { "filter part way" } else { "filter all the way" });
        let touching = c.s.q.wheels.iter().filter(|w| w.touching).count();
        tally(&mut t, match touching {
            0 => "airborne",
            4 => "all down",
            _ => "some down",
        });
    }
    each_path_seen(
        &t,
        &[
            "whole turns",
            "wrapped down",
            "wrapped up",
            "inside",
            "steer at the left lock",
            "steer at the right lock",
            "steer inside",
            "filter part way",
            "filter all the way",
            "airborne",
            "all down",
            "some down",
        ],
        100,
    );
}

// --- MD75: the rigid body and the physics frame ----------------------------------------------------

/// The body's integrate and end-of-substep, as its vtable has them.
const BODY_INTEGRATE: u32 = 0x0049_6240;
const BODY_END_SUBSTEP: u32 = 0x0049_bde0;
/// The world step, and a world of the quad alone laid out for it: its five lists' counts and arrays,
/// the arrays, and the wrapper the Force list holds (with the image's wrapper vtable), which hands
/// the substep on to the quad's own Force interface.
const WORLD_STEP: u32 = 0x0049_6b70;
const DWORLD: u32 = 0x01c1_0000;
const WRAP: u32 = 0x01c1_0200;
const WRAP_VTABLE: u32 = 0x003c_d150;
/// The quad's three listener interfaces (PreProcess, PostProcess, Force) and their vtables.
const QUAD_LISTENERS: [(u32, u32); 3] = [(0x30, 0x003c_e2dc), (0x34, 0x003c_e2e8), (0x3c, 0x003c_e304)];
const GAME_WORLD: World = World { iterations: 1, rate: F(115.0) };

/// The whole body: its word layout (`# MD75`).
fn put_body(ee: &mut Ee, b: &atv_body::Body) {
    put_b(ee, BODY, b.enabled);
    for k in 0..3 {
        put_b(ee, BODY + 0x4 + 4 * k as u32, b.linear_axes[k]);
        put_b(ee, BODY + 0x120 + 4 * k as u32, b.angular_axes[k]);
        put_f(ee, BODY + 0x210 + 4 * k as u32, b.max_rates[k]);
    }
    put_f(ee, BODY + 0x1c, b.inv_mass);
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
    put_f(ee, BODY + 0xe0, b.max_speed);
    put_f(ee, BODY + 0xe4, b.max_accel);
    put_f(ee, BODY + 0x220, b.max_ang_accel);
    ee.set_u32(BODY + 0x110, BODY_VTABLE);
}

const NO_LIMIT: F = F(f32::MAX);
const PIN: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);

impl Gen {
    fn body(&mut self) -> atv_body::Body {
        let limit = |g: &mut Gen, lo: f32, hi: f32| if g.b() { NO_LIMIT } else { g.f(lo, hi) };
        let peak = |g: &mut Gen, v: Vf| {
            let mut p = g.vec(-10.0, 10.0);
            // The same magnitude either way now and then: the peak is kept.
            for k in 0..3 {
                if g.one_in(8) {
                    p[k] = if g.b() { v[k] } else { (-F(v[k])).0 };
                }
            }
            p
        };
        let correction = self.vec(-10.0, 10.0);
        let twist = self.vec(-10.0, 10.0);
        atv_body::Body {
            enabled: !self.one_in(10),
            linear_axes: [!self.one_in(6), !self.one_in(6), !self.one_in(6)],
            inv_mass: match self.bits() % 40 {
                0..=3 => F::ZERO,
                4 => F(-0.0),
                _ => self.f(0.01, 0.2),
            },
            damping: if self.b() { Vf::splat(1.0) } else { self.vec(0.9, 1.0) },
            last_position: self.vec(-1000.0, 1000.0),
            position: self.vec(-1000.0, 1000.0),
            velocity: self.vec(-80.0, 80.0),
            last_force: self.vec(-2000.0, 2000.0),
            force: self.vec(-2000.0, 2000.0),
            accel: self.vec(-100.0, 100.0),
            correction_peak: peak(self, correction),
            correction,
            max_speed: limit(self, 20.0, 120.0),
            max_accel: limit(self, 50.0, 300.0),
            angular_axes: [!self.one_in(6), !self.one_in(6), !self.one_in(6)],
            inv_inertia: match self.bits() % 40 {
                0..=3 => PIN,
                4 => Vf::new(-0.0, 0.0, 0.0, 1.0),
                _ => Vf::new(self.f(0.01, 0.2).0, self.f(0.01, 0.2).0, self.f(0.01, 0.2).0, self.near(1.0, 0.0, 2.0).0),
            },
            ang_damping: if self.b() { Vf::splat(1.0) } else { self.vec(0.9, 1.0) },
            last_quat: self.quaternion(),
            quat: self.quaternion(),
            moment: self.vec(-500.0, 500.0),
            last_moment: self.vec(-500.0, 500.0),
            rates: self.vec(-25.0, 25.0),
            ang_accel: self.vec(-50.0, 50.0),
            twist_peak: peak(self, twist),
            twist,
            max_rates: [self.f(1.0, 20.0), self.f(1.0, 20.0), self.f(1.0, 20.0)],
            max_ang_accel: limit(self, 5.0, 60.0),
        }
    }
}

/// Which of the body step's paths `b` goes down, by the port's own sums (the comparison proves them).
#[allow(clippy::float_arithmetic)]
fn body_paths(b: &atv_body::Body, h: F) -> Vec<&'static str> {
    let mut paths = Vec::new();
    let len = |v: Vf| (v.x() * v.x() + v.y() * v.y() + v.z() * v.z()).sqrt();
    if b.inv_mass == 0.0 {
        paths.push("pinned");
    } else {
        if b.linear_axes.contains(&false) {
            paths.push("linear axis locked");
        }
        let mut a = b.force.mul(Vf::splat(b.inv_mass.0));
        (0..3).filter(|&k| !b.linear_axes[k]).for_each(|k| a[k] = 0.0);
        if b.max_accel != NO_LIMIT && len(a) > b.max_accel.0 {
            paths.push("accel clamped");
        }
        let (mut t, mut u) = (*b, *b);
        t.advance(h);
        u.max_speed = NO_LIMIT;
        u.advance(h);
        if u.velocity != t.velocity {
            paths.push("speed clamped");
        }
    }
    if b.inv_inertia.to_bits() == PIN.to_bits() {
        paths.push("angular pinned");
        return paths;
    }
    if b.inv_inertia.x().to_bits() != 0 && b.inv_inertia.x() == 0.0 {
        paths.push("angular -0 not pinned");
    }
    if b.angular_axes.contains(&false) {
        paths.push("angular axis locked");
    }
    let mut alpha = b.inv_inertia.mul(b.moment);
    (0..3).filter(|&k| !b.angular_axes[k]).for_each(|k| alpha[k] = 0.0);
    if b.max_ang_accel != NO_LIMIT && len(alpha) > b.max_ang_accel.0 {
        paths.push("angular accel clamped");
    }
    let mut t = *b;
    t.integrate(h);
    for k in 0..3 {
        if t.rates[k] == b.max_rates[k].0 {
            paths.push("rate held at the top");
        } else if t.rates[k] == -b.max_rates[k].0 {
            paths.push("rate held at the bottom");
        }
    }
    paths
}

#[test]
fn the_quads_rigid_body_step_is_the_originals() {
    let Some(mut ee) = image() else { return };
    assert_eq!(ee.u32_at(BODY_VTABLE + 0x54), BODY_INTEGRATE, "the body's integrate slot");
    assert_eq!(ee.u32_at(BODY_VTABLE + 0x40), BODY_END_SUBSTEP, "the body's end-of-substep slot");
    let mut g = Gen(0x0075_0001);
    let mut t = BTreeMap::new();
    let snap_body = |ee: &Ee| (0..0x230).step_by(4).map(|o| ee.u32_at(BODY + o)).collect::<Vec<u32>>();
    for k in 0..10_000 {
        let b = g.body();
        let h = if g.one_in(4) { F(1.0 / 120.0) } else { g.f(0.002, 0.03) };
        // What the two calls do not read: arbitrary, and the same both times.
        let filler: Vec<u32> = (0..0x230 / 4).map(|_| g.bits()).collect();
        let lay = |ee: &mut Ee, b: &atv_body::Body| {
            for (i, &w) in filler.iter().enumerate() {
                ee.set_u32(BODY + 4 * i as u32, w);
            }
            put_body(ee, b);
        };
        for p in body_paths(&b, h) {
            tally(&mut t, p);
        }
        lay(&mut ee, &b);
        let mark = ee.written_ranges().len();
        ee.call(BODY_INTEGRATE, &[u64::from(BODY)], &[h.0], 20_000);
        ee.call(BODY_END_SUBSTEP, &[u64::from(BODY)], &[], 20_000);
        for (a, e) in ee.written_ranges().into_iter().skip(mark) {
            let inside = (a >= BODY && e <= BODY + 0x230) || (a >= ee.stack - 0x2000 && e <= ee.stack + 0x40);
            assert!(inside, "state {k}: the original wrote {a:#010x}..{e:#010x}");
        }
        let was = snap_body(&ee);
        let mut port = b;
        port.integrate(h);
        port.end_substep();
        lay(&mut ee, &port);
        let now = snap_body(&ee);
        if let Some(i) = was.iter().zip(&now).position(|(a, b)| a != b) {
            panic!(
                "state {k}: body + {:#x} is {:#010x} ({}) in the original and {:#010x} ({}) in the port\nh {h:?}\n{b:#?}",
                4 * i,
                was[i],
                f32::from_bits(was[i]),
                now[i],
                f32::from_bits(now[i]),
            );
        }
    }
    each_path_seen(
        &t,
        &[
            "pinned",
            "linear axis locked",
            "accel clamped",
            "speed clamped",
            "angular pinned",
            "angular -0 not pinned",
            "angular axis locked",
            "angular accel clamped",
            "rate held at the top",
            "rate held at the bottom",
        ],
        30,
    );
}

/// The world of the quad alone, as [`GAME_WORLD`]: one iteration at 115 a second, the body, the
/// quad's PreProcess, Force and PostProcess, no constraints.
fn lay_out_world(ee: &mut Ee) {
    ee.set_u32(DWORLD, 1);
    ee.set_f32(DWORLD + 4, 115.0);
    let lists = [(0x50, Some(BODY)), (0x68, Some(WRAP)), (0x80, None), (0x98, Some(OBJ + 0x30)), (0xb0, Some(OBJ + 0x34))];
    for (i, (at, elt)) in lists.into_iter().enumerate() {
        let array = DWORLD + 0x100 + 0x10 * i as u32;
        ee.set_u32(DWORLD + at, u32::from(elt.is_some()));
        ee.set_u32(DWORLD + at + 8, array);
        ee.set_u32(array, elt.unwrap_or(0));
    }
    ee.set_u32(WRAP + 8, WRAP_VTABLE);
    ee.set_u32(WRAP + 0xc, OBJ + 0x3c);
}

/// The quad's listener interfaces, in the quad (whose region every state empties).
fn put_listeners(ee: &mut Ee) {
    for (o, vt) in QUAD_LISTENERS {
        ee.set_u32(OBJ + o, vt);
    }
}

#[test]
fn the_quads_physics_frame_is_the_originals() {
    let Some(mut ee) = image() else { return };
    // The world the game runs the quad in is the one laid out here, but for the other riders.
    assert_eq!((ee.u32_at(0x0065_bc0c), ee.f32_at(0x0065_bc10)), (1, 115.0), "the game world's iterations and rate");
    assert_eq!(ee.u32_at(0x015e_0690 + 0xbc), 0x0159_bee0, "the quad's Force wrapper");
    assert_eq!((ee.u32_at(0x0159_bee8), ee.u32_at(0x0159_beec)), (WRAP_VTABLE, 0x015e_0690 + 0x3c), "the wrapper's vtable and listener");
    for (o, vt) in QUAD_LISTENERS {
        assert_eq!(ee.u32_at(0x015e_0690 + o), vt, "the quad's listener at +{o:#x}");
    }
    let table = AcosTable::build();
    let log = hook_calls(
        &mut ee,
        &[
            (INPUT_LIMITER, |ee| on_quad(ee, Call::InputLimiter(F(ee.f(12))))),
            (AUTO_SHIFT, |ee| on_quad(ee, Call::AutoShift { dt: F(ee.f(12)), inv_radius: F(ee.f(13)) })),
            (CLUTCH, |ee| on_quad(ee, Call::Clutch(F(ee.f(12))))),
            (HEAD_TAIL, |ee| on_quad(ee, Call::HeadTail)),
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
            (VEHICLE_POST, |ee| on_quad(ee, Call::VehiclePost(F(ee.f(12))))),
            (NODE_POSITION, |_| Call::NodePosition),
        ],
    );
    let write = ee.hooks.remove(&NODE_POSITION).unwrap();
    let mut write = Some(write);
    ee.hooks.insert(
        NODE_POSITION,
        Box::new(move |ee| {
            write.as_mut().unwrap()(ee);
            put_v(ee, OBJ + 0xbd0, NODE_SAYS);
        }),
    );
    lay_out_world(&mut ee);
    let mut g = Gen(0x0075_0002);
    let mut t = BTreeMap::new();
    for k in 0..3_000 {
        let mut s = g.state();
        if g.one_in(3) {
            s.dt = F(1.0 / 30.0);
        }
        let mut b = g.body();
        b.set_view(&s.body);
        let (n, _) = GAME_WORLD.substeps(s.dt);
        tally(&mut t, ["1 substep", "2 substeps", "3 substeps", "4 substeps", "5 substeps"][(n - 1).clamp(0, 4) as usize]);
        tally(&mut t, match (s.q.active, b.enabled) {
            (false, _) => "quad off",
            (true, false) => "body off",
            (true, true) => "both on",
        });
        let lay = |ee: &mut Ee, s: &State, b: &atv_body::Body| {
            reset(ee);
            poke(ee, s);
            put_body(ee, b);
            put_listeners(ee);
        };
        log.borrow_mut().clear();
        lay(&mut ee, &s, &b);
        ee.call(WORLD_STEP, &[u64::from(DWORLD)], &[s.dt.0], 2_000_000);
        only_touched_the_regions(&ee).unwrap_or_else(|e| panic!("state {k}: {e}\n{s:#?}"));
        let was = snap(&ee);
        let mut ts = s.clone();
        let mut tb = b;
        let mut post = Post(Vec::new());
        quad::frame(&GAME_WORLD, &ts.p, &mut ts.q, &mut tb, ts.dt, &table, &mut post);
        ts.body = tb.view();
        lay(&mut ee, &ts, &tb);
        let now = snap(&ee);
        if let Some(i) = was.iter().zip(&now).position(|(a, b)| a != b) {
            panic!(
                "state {k}: {} is {:#010x} ({}) in the original and {:#010x} ({}) in the port\n{s:#?}\n{b:#?}",
                where_is(i),
                was[i],
                f32::from_bits(was[i]),
                now[i],
                f32::from_bits(now[i]),
            );
        }
        assert_eq!(*log.borrow(), post.0, "state {k}: the calls\n{s:#?}");
    }
    each_path_seen(
        &t,
        &["1 substep", "2 substeps", "3 substeps", "4 substeps", "5 substeps", "quad off", "body off", "both on"],
        30,
    );
}

// --- MD75a: the bike's steps on the quad, each run as the original's own code --------------------------

/// What the input limiter's first call reads through the quad's `+0x680` (its `+0x1d0` above zero
/// cuts the gas), and the thing the collision record's `+0x20c` points at, whose `+0x94` class the
/// backflip's steep test holds against the game's mask.
const GATE: u32 = 0x01c0_5000;
const THING: u32 = 0x01c0_6000;
const STRUCK_MASK: u32 = 0x0054_2db0;
/// What the bike steps call in turn: the wreck, the wheels' ground query, surface drag and spin.
const WRECK_ENTRY: u32 = 0x004e_2260;
const GROUND_QUERY: u32 = 0x0048_33c0;
const SURFACE_DRAG: u32 = 0x004a_1ec0;
const SPIN_INTEGRATE: u32 = 0x004a_2040;
/// The words the steps write past [`REGIONS`]: the engine's clutch multiplier (`+0x7c`), and each
/// wheel's steering rate (`+0x8b0`) and axis (`+0x8c0`).
const MORE_REGIONS: [(&str, u32, u32); 5] = [
    ("engine", ENGINE + 0x70, 0x10),
    ("left front's steering", 0x01c0_88b0, 0x20),
    ("right front's steering", 0x01c0_98b0, 0x20),
    ("left rear's steering", 0x01c0_a8b0, 0x20),
    ("right rear's steering", 0x01c0_b8b0, 0x20),
];

fn all_regions() -> impl Iterator<Item = (&'static str, u32, u32)> {
    REGIONS.into_iter().chain(MORE_REGIONS)
}

fn reset_all(ee: &mut Ee) {
    for (_, base, len) in all_regions() {
        for o in (0..len).step_by(4) {
            ee.set_u32(base + o, 0);
        }
    }
}

fn snap_all(ee: &Ee) -> Vec<u32> {
    all_regions().flat_map(|(_, base, len)| (0..len).step_by(4).map(move |o| base + o)).map(|a| ee.u32_at(a)).collect()
}

fn where_in_all(i: usize) -> String {
    let mut i = i as u32 * 4;
    for (name, _, len) in all_regions() {
        if i < len {
            return format!("{name} + {i:#x}");
        }
        i -= len;
    }
    "past the end".into()
}

fn only_touched_all(ee: &Ee) -> Result<(), String> {
    let frame = (ee.stack - 0x2000, ee.stack + 0x40);
    for (a, b) in ee.written_ranges() {
        // Word by word: a wheel's steering words run on from its region into the next.
        let inside = (a >= frame.0 && b <= frame.1)
            || (a..b).step_by(4).all(|w| all_regions().any(|(_, base, len)| w >= base && w < base + len));
        if !inside {
            return Err(format!("the original wrote {a:#010x}..{b:#010x}, which is not the quad or the stack"));
        }
    }
    Ok(())
}

/// A quad state with the bike steps' own fields ([`quad::Bike`]) and what their callees answer.
#[derive(Clone, Debug)]
struct Ride {
    s: State,
    bike: quad::Bike,
    /// The `+0x680` object's `+0x1d0`, or no object.
    gate: Option<F>,
    /// The struck thing's class word, the bits of the trick-button byte above the step's nibble, and
    /// the bike's `+0xd20` the preload's cue hands on.
    class: u32,
    high: u8,
    tag: u32,
    /// The ground query's answers in the order the four suspensions ask, and whether it hands the
    /// point back as it was given.
    answers: Vec<(Vf, Vf)>,
    echo: bool,
}

/// Every air key zero: the generator sets the ones the steps read.
fn air_keys_zero() -> game_sim::air::Params {
    game_sim::air::Params {
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

#[allow(clippy::float_arithmetic)]
impl Gen {
    fn lever(&mut self) -> F {
        match self.bits() % 4 {
            0 => F::ZERO,
            1 => F::ONE,
            _ => self.f(0.0, 1.0),
        }
    }

    /// A quad state as [`Gen::state`] leaves it, with the bike's own fields over it: mostly
    /// switched on with control, the rider's stunt word one the airborne control tells apart, the
    /// body somewhere, and the four wheels as `tests/atv_wheel.rs` makes them, standing on still
    /// ground.
    fn ride(&mut self, mask: u32) -> Ride {
        use game_sim::{handling, r#move as mv, wheel};
        let mut s = self.state();
        if !self.one_in(8) {
            s.vstate = Some(self.bits() % 3);
            s.q.active = true;
            s.q.control_off = self.one_in(10);
        }
        let stunt = match self.bits() % 6 {
            0 => self.bits() % 41,
            1 => 41 + self.bits() % 5,
            2 => 0x2e,
            3 => 0xff,
            4 => if self.b() { 40 } else { 45 },
            _ => 46 + self.bits() % 250,
        };
        s.bal = if self.one_in(5) { (stunt, stunt) } else { (stunt, stunt ^ 1) };
        s.q.balancing = s.bal.0 == s.bal.1;
        s.body.position = self.vec(-50.0, 50.0);
        s.body.correction = self.vec(-5.0, 5.0);
        if self.b() {
            // Mostly upright, so that the suspension points down.
            let v = [self.f(-0.3, 0.3).0, self.f(-1.0, 1.0).0, self.f(-0.3, 0.3).0, self.f(0.5, 1.0).0];
            let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2] + v[3] * v[3]).sqrt();
            s.body.orientation = Vf::new(v[0] / n, v[1] / n, v[2] / n, v[3] / n);
        }

        let gate = match self.bits() % 4 {
            0 => None,
            1 => Some(if self.b() { F::ZERO } else { self.f(-2.0, 0.0) }),
            2 => Some(self.f(0.01, 2.0)),
            _ => Some(F::ZERO),
        };
        let mut held = game_sim::ride::Held::default();
        held.lock.steer = self.one_in(4);
        held.lock.throttle = self.one_in(4);
        held.lock.steer_was = self.one_in(4);
        held.lock.latch = self.one_in(3);
        let rate = |g: &mut Gen| g.f(0.5, 20.0);
        let joystick = game_sim::phy::Joystick {
            x_axis_rate_limit: if self.one_in(10) { F::ZERO } else { rate(self) },
            y_axis_rate_limit: rate(self),
            x_dpad_rate_limit: rate(self),
            y_dpad_rate_limit: if self.one_in(10) { F::ZERO } else { rate(self) },
            gas_rate_limit: rate(self),
            brake_rate_limit: rate(self),
            clutch_in_rate_limit: rate(self),
            clutch_out_rate_limit: rate(self),
        };
        let stick = game_sim::trick::Stick {
            x: if self.one_in(4) { F(if self.b() { 1.0 } else { -1.0 }) } else { self.f(-1.0, 1.0) },
            y: match self.bits() % 4 {
                0 => F(if self.b() { 0.8 } else { -0.8 }),
                1 => if self.b() { self.f(0.81, 1.0) } else { self.f(-1.0, -0.81) },
                _ => self.f(-1.0, 1.0),
            },
            digital: [self.one_in(3), self.one_in(3)],
        };

        let mut clutch_keys = mv::Params::default();
        clutch_keys.clutch_torque_scale_time = if self.one_in(5) { F::ZERO } else { self.f(0.1, 2.0) };
        clutch_keys.max_clutch_torque_scale_speed = if self.one_in(5) { F::ZERO } else { self.f(5.0, 40.0) };
        clutch_keys.max_clutch_torque_scale = self.f(0.5, 4.0);
        let mut ride = mv::Ride::default();
        ride.clutch_timer = match self.bits() % 5 {
            0 => F::ZERO,
            1 => clutch_keys.clutch_torque_scale_time,
            2 => self.f(-1.0, 0.0),
            _ => self.f(0.0, 2.5),
        };
        ride.clutch_scale_start = self.f(-1.0, 4.0);
        ride.clutch_cofg_weight = self.f(-1.0, 2.0);
        let engine = mv::Engine { clutch_torque_scale: self.f(-1.0, 4.0), ..mv::Engine::default() };

        let mut k = air_keys_zero();
        k.preload_time = self.f(0.0, 0.5);
        k.min_preload_time = self.f(0.0, 30.0);
        k.max_preload_time = if self.one_in(6) { k.min_preload_time } else { self.f(10.0, 80.0) };
        k.min_dpad_preload_time = self.f(0.0, 30.0);
        k.max_dpad_preload_time = if self.one_in(6) { k.min_dpad_preload_time } else { self.f(10.0, 80.0) };
        k.max_preload = self.f(0.0, 3000.0);
        k.min_backflip_time = if self.one_in(4) { F::ZERO } else { self.f(0.0, 30.0) };
        k.max_backflip_time = if self.one_in(8) { k.min_backflip_time } else { self.f(10.0, 80.0) };
        k.min_backflip_torque = self.f(300.0, 1200.0);
        k.max_backflip_torque = self.f(600.0, 1800.0);
        k.backflip_torque_rate_increase = self.f(0.0, 1000.0);
        k.backflip_torque_rate_decrease = self.f(0.0, 3000.0);
        k.airborne_pitch_down_wreck_angle = self.f(0.8, 1.6);
        k.airborne_pitch_up_wreck_angle = self.f(0.8, 1.6);
        k.front_wreck_speed_limit = self.f(0.5, 8.0);
        k.back_wreck_speed_limit = self.f(0.5, 8.0);
        k.side_wreck_speed_limit = self.f(0.5, 8.0);
        k.airborne_control_moment = Vf::new(self.f(-600.0, 0.0).0, self.f(0.0, 800.0).0, self.f(-300.0, 0.0).0, self.f(0.0, 2.0).0);
        k.airborne_damping = self.f(0.0, 600.0);
        k.airborne_control_force = self.f(0.0, 300.0);
        k.brake_gyro_torque = self.f(0.0, 600.0);
        k.unroll_gain = self.f(0.0, 6.0);
        k.unroll_torque = self.f(0.0, 100.0);
        k.unyaw_gain = self.f(0.0, 6.0);
        k.unyaw_torque = self.f(0.0, 800.0);

        let mut air = game_sim::air::Air::default();
        air.preload_firing = self.one_in(3);
        air.preload_ready = self.b();
        air.preload_armed = self.b();
        air.preload_timer = if self.one_in(5) { F::ZERO } else { self.f(-0.2, 0.6) };
        air.preload_force = Vf::new(self.f(-5.0, 5.0).0, self.f(0.0, 3000.0).0, self.f(-5.0, 5.0).0, self.f(-2.0, 2.0).0);
        air.flip_active = self.bits() % 3 != 0;
        air.flip_armed = self.b();
        air.flip_torque_applied = self.b();
        air.flip_grace = self.near(0.0, -0.1, 0.7);
        let t = if self.one_in(6) { k.min_backflip_torque } else { self.f(0.0, 1800.0) };
        air.flip_torque = Vf::new((if self.b() { -t } else { t }).0, self.f(-50.0, 50.0).0, self.f(-50.0, 50.0).0, self.f(-2.0, 2.0).0);

        // A lip, a third of the time: on the ground, rising, fast and off the brake.
        if self.one_in(3) {
            s.q.on_ground = true;
            s.q.brake = F::ZERO;
            s.body.velocity = Vf::new(s.body.velocity.x(), self.f(5.5, 15.0).0, s.body.velocity.z(), 0.0);
            s.q.contact_vel = with_z(s.q.contact_vel, self.f(30.5, 60.0));
        }
        if self.one_in(6) {
            s.body.velocity = Vf::new(s.body.velocity.x(), 5.0, s.body.velocity.z(), s.body.velocity.w());
        }
        if self.one_in(6) {
            s.q.contact_vel = with_z(s.q.contact_vel, F(30.0));
        }
        // Level enough to auto-level, half the time.
        let hy = if self.one_in(8) { F(0.9396926) } else { self.f(-0.9, 0.9) };
        if self.b() {
            s.q.heading = Vf::new(s.q.heading.x(), hy.0, s.q.heading.z(), s.q.heading.w());
        }
        if self.b() {
            s.q.lean = self.near(0.1, -0.1, 0.1);
        }

        let touching = self.one_in(4);
        let class = if self.b() { mask } else if self.b() { !mask } else { self.bits() };
        let bike_wheels = self.wheels(&s);
        let mut more = [game_sim::atv_wheel::More::default(); 4];
        for (x, w) in more.iter_mut().zip(&s.q.wheels) {
            *x = game_sim::atv_wheel::More {
                record: self.bits() as u8,
                kept: if self.one_in(10) { Vf::ZERO } else { self.vec(-3.0, 3.0) },
                chassis_velocity_local: w.contact_vel,
                ground_known: if self.one_in(5) { 1 } else { 0 },
                ground_velocity: self.vec(-5.0, 5.0),
                ground_point_velocity: self.vec(-5.0, 5.0),
                steer_axis: self.normal(),
            };
        }
        for (c, w) in s.contacts.iter_mut().zip(&bike_wheels) {
            (c.owner, c.vehicle, c.point, c.touching) = (0, 0, w.contact_point, w.swept_contact);
        }
        let p = s.body.position;
        let mut answers = Vec::new();
        for w in &bike_wheels {
            let point = Vf::new(p.x() + self.f(-3.0, 3.0).0, p.y() + self.f(-4.0, 1.0).0, p.z() + self.f(-3.0, 3.0).0, self.f(0.0, 1.0).0);
            let n = self.normal();
            for _ in 0..5 {
                answers.push((point, if self.b() { n } else if self.one_in(3) { w.ground_normal } else { self.normal() }));
            }
        }
        let bike = quad::Bike {
            held,
            gas_cut: handling::step::GasCut {
                latched: self.b(),
                timer: match self.bits() % 4 {
                    0 => F::ZERO,
                    1 => self.f(-0.05, 0.0),
                    _ => self.f(0.0, 0.2),
                },
            },
            cut_time: if self.b() { F::ZERO } else { self.f(-0.05, 0.2) },
            joystick,
            stick,
            clutch: self.lever(),
            gate_cut: gate.is_some_and(|v| F::ZERO < v),
            clutch_keys,
            ride,
            engine,
            air_keys: k,
            air,
            stick_hold: self.f(-70.0, 70.0),
            preload_cue: self.b(),
            spin_applied: self.one_in(8),
            spin_active: self.one_in(8),
            touching,
            struck_class: touching && class & mask != 0,
            buttons: if self.b() { handling::R1_ALONE } else if self.b() { 0 } else { (self.bits() & 0xf) as u8 },
            right_y: if self.one_in(8) { F::ZERO } else { self.f(-1.0, 1.0) },
            up_y: if self.one_in(8) { F::ZERO } else if self.one_in(6) { F(-0.79) } else { self.f(-1.0, 1.0) },
            stunt_id: (self.bits() % 80) as i32,
            rider_stunt: stunt as i32,
            flip_clip: handling::FlipClip((self.bits() % 3) as i32),
            flip_stunt: self.bits() % 64,
            gas_gyro_torque: self.f(0.0, 400.0),
            gyro: handling::Gyro { brake: self.bits() % 4 != 0, gas: self.bits() % 4 != 0 },
            wheels: bike_wheels,
            more,
            ground: [game_sim::atv_wheel::Ground::Still; 4],
            cofg: self.vec(-0.5, 0.5),
        };
        let _: &[wheel::Wheel; 4] = &bike.wheels;
        Ride { s, bike, gate, class, high: (self.bits() & 0xf) as u8, tag: self.bits(), answers, echo: self.one_in(8) }
    }

    /// The four wheels' step fields, `tests/atv_wheel.rs`'s ranges, with what the quad keeps of
    /// them from the quad's.
    fn wheels(&mut self, s: &State) -> [game_sim::wheel::Wheel; 4] {
        let p = s.body.position;
        let mut out = [game_sim::wheel::Wheel::default(); 4];
        for (w, qw) in out.iter_mut().zip(&s.q.wheels) {
            *w = game_sim::wheel::Wheel {
                has_ground: !self.one_in(20),
                steered: qw.steered,
                balancing: qw.balancing,
                contact: qw.touching,
                contact_normal: qw.normal,
                max_steer_angle: qw.max_steer_angle,
                steer_angle: qw.steer_command,
                spring_rate: qw.spring,
                max_compression: if self.one_in(10) { F::ZERO } else { self.f(0.2, 3.0) },
                max_damper_speed: self.f(2.0, 30.0),
                extension_rate: self.f(1.0, 20.0),
                damping_out: self.f(10.0, 300.0),
                damping_in: self.f(10.0, 300.0),
                probe: if self.one_in(6) {
                    self.vec(-2.0, 2.0)
                } else {
                    Vf::new(self.f(-0.3, 0.3).0, self.f(-2.5, -0.3).0, self.f(-0.3, 0.3).0, self.f(0.0, 1.0).0)
                },
                mount: Vf::new(self.f(-2.0, 2.0).0, self.f(-1.0, 0.5).0, self.f(-2.5, 2.5).0, self.f(0.0, 1.0).0),
                ray_hit: if self.one_in(3) { 0 } else { 1 },
                swept_contact: if self.b() { 1 } else { 0 },
                terrain_wins: if self.b() { 1 } else { 0 },
                ray_steps: self.bits() as i32 % 7,
                compression: self.f(-1.0, 1.0),
                last_compression: if self.one_in(4) { F::ZERO } else { self.f(0.0, 1.5) },
                spring_force: self.f(-100.0, 100.0),
                probe_end: self.vec(-5.0, 5.0),
                contact_point: Vf::new(p.x() + self.f(-3.0, 3.0).0, p.y() + self.f(-4.0, 1.0).0, p.z() + self.f(-3.0, 3.0).0, self.f(0.0, 1.0).0),
                load_point: self.vec(-5.0, 5.0),
                mount_world: self.vec(-5.0, 5.0),
                probe_end_world: self.vec(-5.0, 5.0),
                contact_velocity: self.vec(-5.0, 5.0),
                chassis_velocity: self.vec(-5.0, 5.0),
                ground_normal: self.normal(),
                probe_length: self.f(0.5, 2.5),
                steer_torque: if self.one_in(4) { F::ZERO } else { self.f(-60.0, 60.0) },
                steer_damping: if self.one_in(4) { F::ZERO } else { self.f(0.0, 12.0) },
                steer_inv_inertia: if self.one_in(4) { F::ZERO } else { self.f(0.0, 6.0) },
                min_steer_angle: -self.f(0.2, 0.7),
                steer_rate: if self.one_in(4) { F::ZERO } else { self.f(-12.0, 12.0) },
                ..Default::default()
            };
        }
        out
    }
}

/// The quad state, then the bike's own words over it (the words the quad keeps too are the quad's,
/// written by [`poke`]).
fn poke_ride(ee: &mut Ee, r: &Ride) {
    poke(ee, &r.s);
    let b = &r.bike;
    let (h, j) = (&b.held, &b.joystick);
    // The input limiter's.
    put_f(ee, OBJ + 0x12c0, b.clutch);
    put_f(ee, OBJ + 0x12a8, b.stick.x);
    put_f(ee, OBJ + 0x12ac, b.stick.y);
    put_b(ee, OBJ + 0x12a0, b.stick.digital[0]);
    put_b(ee, OBJ + 0x12a4, b.stick.digital[1]);
    put_b(ee, OBJ + 0xdd0, !h.lock.steer);
    put_b(ee, OBJ + 0xdd4, !h.lock.throttle);
    put_b(ee, OBJ + 0x1298, !h.lock.steer_was);
    put_b(ee, OBJ + 0x129c, h.lock.latch);
    put_b(ee, OBJ + 0x1764, b.gas_cut.latched);
    put_f(ee, OBJ + 0x1768, b.gas_cut.timer);
    put_f(ee, OBJ + 0x176c, b.cut_time);
    put_f(ee, OBJ + 0x12d8, j.x_axis_rate_limit);
    put_f(ee, OBJ + 0x12dc, j.y_axis_rate_limit);
    put_f(ee, OBJ + 0x12e0, j.x_dpad_rate_limit);
    put_f(ee, OBJ + 0x12e4, j.y_dpad_rate_limit);
    put_f(ee, OBJ + 0x1310, j.gas_rate_limit);
    put_f(ee, OBJ + 0x1314, j.brake_rate_limit);
    put_f(ee, OBJ + 0x1318, j.clutch_in_rate_limit);
    put_f(ee, OBJ + 0x131c, j.clutch_out_rate_limit);
    match r.gate {
        Some(v) => {
            ee.set_u32(OBJ + 0x680, GATE);
            put_f(ee, GATE + 0x1d0, v);
        }
        None => ee.set_u32(OBJ + 0x680, 0),
    }
    // The clutch ramp's.
    put_f(ee, OBJ + 0x1348, b.ride.clutch_timer);
    put_f(ee, OBJ + 0x134c, b.ride.clutch_scale_start);
    put_f(ee, OBJ + 0x1350, b.clutch_keys.clutch_torque_scale_time);
    put_f(ee, OBJ + 0x1354, b.ride.clutch_cofg_weight);
    put_f(ee, OBJ + 0x1358, b.clutch_keys.max_clutch_torque_scale_speed);
    put_f(ee, OBJ + 0x135c, b.clutch_keys.max_clutch_torque_scale);
    put_f(ee, ENGINE + 0x7c, b.engine.clutch_torque_scale);
    // The preload's.
    let (k, a) = (&b.air_keys, &b.air);
    put_f(ee, OBJ + 0xc5c, k.preload_time);
    put_f(ee, OBJ + 0xc64, k.min_preload_time);
    put_f(ee, OBJ + 0xc68, k.max_preload_time);
    put_f(ee, OBJ + 0xc6c, k.max_preload);
    put_f(ee, OBJ + 0x1160, k.min_dpad_preload_time);
    put_f(ee, OBJ + 0x1164, k.max_dpad_preload_time);
    put_b(ee, OBJ + 0xc50, a.preload_ready);
    put_b(ee, OBJ + 0xc54, a.preload_firing);
    put_b(ee, OBJ + 0xc58, a.preload_armed);
    put_f(ee, OBJ + 0xc60, a.preload_timer);
    put_v(ee, OBJ + 0xc70, a.preload_force);
    put_b(ee, OBJ + 0x700, b.preload_cue);
    ee.set_u32(OBJ + 0xd20, r.tag);
    put_f(ee, OBJ + 0x12b0, b.stick_hold);
    // The backflip's, and its steep test's.
    put_f(ee, OBJ + 0x11a4, k.min_backflip_time);
    put_f(ee, OBJ + 0x11a8, k.max_backflip_time);
    put_f(ee, OBJ + 0x11b0, k.min_backflip_torque);
    put_f(ee, OBJ + 0x11b4, k.max_backflip_torque);
    put_f(ee, OBJ + 0x11b8, k.backflip_torque_rate_increase);
    put_f(ee, OBJ + 0x11bc, k.backflip_torque_rate_decrease);
    put_f(ee, OBJ + 0x1404, k.airborne_pitch_down_wreck_angle);
    put_f(ee, OBJ + 0x1408, k.airborne_pitch_up_wreck_angle);
    put_f(ee, OBJ + 0x13e8, k.front_wreck_speed_limit);
    put_f(ee, OBJ + 0x13ec, k.back_wreck_speed_limit);
    put_f(ee, OBJ + 0x13f0, k.side_wreck_speed_limit);
    put_b(ee, OBJ + 0x1194, a.flip_torque_applied);
    put_b(ee, OBJ + 0x119c, a.flip_active);
    put_b(ee, OBJ + 0x11a0, a.flip_armed);
    put_f(ee, OBJ + 0x11ac, a.flip_grace);
    put_v(ee, OBJ + 0x11c0, a.flip_torque);
    put_b(ee, OBJ + 0x11d0, b.spin_applied);
    put_b(ee, OBJ + 0x11d8, b.spin_active);
    put_b(ee, RECORD + 0x34, b.touching);
    ee.set_u32(RECORD + 0x20c, THING);
    ee.set_u32(THING + 0x94, r.class);
    ee.set_u32(OBJ + 0x14ec, u32::from(b.buttons | r.high << 4));
    put_f(ee, NODE + 0x84, b.right_y);
    put_f(ee, NODE + 0x94, b.up_y);
    ee.set_u32(OBJ + 0x1528, b.stunt_id as u32);
    ee.set_u32(OBJ + 0x1754, b.flip_clip.0 as u32);
    ee.set_u32(OBJ + 0x13b8, b.flip_stunt);
    // The airborne control's (its stunt word, `+0x1524`, is the first balance word).
    assert_eq!(b.rider_stunt as u32, r.s.bal.0, "the rider's stunt is the first balance word");
    put_v(ee, OBJ + 0x1170, k.airborne_control_moment);
    put_f(ee, OBJ + 0x1180, k.airborne_damping);
    put_f(ee, OBJ + 0x1184, k.airborne_control_force);
    put_f(ee, OBJ + 0x1330, k.brake_gyro_torque);
    put_f(ee, OBJ + 0x1334, b.gas_gyro_torque);
    put_f(ee, OBJ + 0x1338, k.unroll_gain);
    put_f(ee, OBJ + 0x133c, k.unroll_torque);
    put_f(ee, OBJ + 0x1340, k.unyaw_gain);
    put_f(ee, OBJ + 0x1344, k.unyaw_torque);
    put_b(ee, OBJ + 0x1290, b.gyro.brake);
    put_b(ee, OBJ + 0x1294, b.gyro.gas);
    // The wheels', at `tests/atv_wheel.rs`'s offsets, on still ground.
    put_v(ee, BODY + 0x1a0, b.cofg);
    for ((w, x), &at) in b.wheels.iter().zip(&b.more).zip(&WHEEL) {
        ee.set_u32(at + 0x1e0, if w.has_ground { WORLD } else { 0 });
        ee.set_u32(at + 0x1e4, BODY);
        put_f(ee, at + 0x3c, w.max_compression);
        put_f(ee, at + 0x40, w.max_damper_speed);
        put_f(ee, at + 0x44, w.extension_rate);
        put_f(ee, at + 0x4c, w.damping_out);
        put_f(ee, at + 0x50, w.damping_in);
        put_v(ee, at + 0x80, w.probe);
        put_v(ee, at + 0x90, w.mount);
        put_v(ee, at + 0xc0, x.kept);
        ee.set_u32(at + 0xd0, w.ray_hit as u32);
        ee.set_u32(at + 0xd4, w.swept_contact as u32);
        ee.set_u32(at + 0xd8, w.terrain_wins as u32);
        ee.set_u32(at + 0xec, w.ray_steps as u32);
        put_f(ee, at + 0xf0, w.compression);
        put_f(ee, at + 0xf4, w.last_compression);
        put_f(ee, at + 0xf8, w.spring_force);
        put_v(ee, at + 0x100, w.probe_end);
        ee.set_u32(at + 0x110, u32::from(x.record) | 0x5a5a_5a00);
        put_v(ee, at + 0x120, w.contact_point);
        put_v(ee, at + 0x140, w.load_point);
        put_v(ee, at + 0x150, w.mount_world);
        put_v(ee, at + 0x160, w.probe_end_world);
        put_v(ee, at + 0x170, w.contact_velocity);
        put_v(ee, at + 0x180, w.chassis_velocity);
        ee.set_u32(at + 0x1a0, x.ground_known as u32);
        put_v(ee, at + 0x1b0, x.ground_velocity);
        put_v(ee, at + 0x1c0, x.ground_point_velocity);
        ee.set_u32(at + 0x1f0, 0);
        ee.set_u32(at + 0x1f4, 0);
        put_v(ee, at + 0x200, w.ground_normal);
        put_f(ee, at + 0x214, w.probe_length);
        put_f(ee, at + 0x60, w.steer_torque);
        put_f(ee, at + 0x64, w.steer_damping);
        put_f(ee, at + 0x6c, w.steer_inv_inertia);
        put_f(ee, at + 0x74, w.min_steer_angle);
        put_f(ee, at + 0x8b0, w.steer_rate);
        put_v(ee, at + 0x8c0, x.steer_axis);
    }
}

/// The quad's call to the original's address it goes to.
fn address_of(c: &Call) -> u32 {
    match c {
        Call::InputLimiter(_) => INPUT_LIMITER,
        Call::Clutch(_) => CLUTCH,
        Call::Preload(_) => PRELOAD,
        Call::Backflip(_) => BACKFLIP,
        Call::AirborneControl(_) => AIRBORNE_CONTROL,
        Call::Suspension(..) => SUSPENSION,
        Call::TireForces(..) => TIRE_FORCES,
        Call::WheelIntegrate(..) => WHEEL_INTEGRATE,
        _ => 0,
    }
}

/// The port's side: the calls whose addresses are `picks` go to [`quad::Steps`], the rest are listed
/// where the steps list what they call in turn.
struct Only<'a, G> {
    picks: &'a [u32],
    steps: quad::Steps<Vec<Call>, G>,
}

impl<G: FnMut(Vf) -> (Vf, Vf)> quad::Moto for Only<'_, G> {
    fn call(&mut self, c: Call, q: &mut Quad, body: &mut Body) {
        if self.picks.contains(&address_of(&c)) {
            self.steps.call(c, q, body);
        } else {
            self.steps.rest.push(c);
        }
    }
}

type Answers = Rc<RefCell<std::collections::VecDeque<(Vf, Vf)>>>;

/// Every callee of the PreProcess and the Force body hooked to log its call (as the two listeners'
/// tests do), and what the bike steps call in turn: the stunt start (answering yes), its end, the
/// wreck, the preload's cue, each wheel's surface drag and spin, and the ground query (serving
/// `answers`, the point too unless `echo`). Then the hooks at `picks` are taken off, so that the
/// original's own code runs there.
fn hook_steps(ee: &mut Ee, picks: &[u32], answers: &Answers, echo: &Rc<Cell<bool>>) -> Log<Call> {
    let log = hook_calls(
        ee,
        &[
            (INPUT_LIMITER, |ee| on_quad(ee, Call::InputLimiter(F(ee.f(12))))),
            (AUTO_SHIFT, |ee| on_quad(ee, Call::AutoShift { dt: F(ee.f(12)), inv_radius: F(ee.f(13)) })),
            (CLUTCH, |ee| on_quad(ee, Call::Clutch(F(ee.f(12))))),
            (HEAD_TAIL, |ee| on_quad(ee, Call::HeadTail)),
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
            (END_STUNT, |ee| on_quad(ee, Call::EndStunt)),
            (WRECK_ENTRY, |ee| on_quad(ee, Call::Wreck(ee.r(5) as u8))),
            (POST, |ee| {
                assert_eq!([ee.r(5) as u32, ee.r(6) as u32, ee.r(7) as u32, ee.r(9) as u32], [OBJ + 0x674, 8, 14, 0], "the preload cue's post");
                Call::PreloadCue
            }),
            (SURFACE_DRAG, |ee| Call::SurfaceDrag(wheel_of(ee), F(ee.f(12)))),
            (SPIN_INTEGRATE, |ee| Call::SpinIntegrate(wheel_of(ee), F(ee.f(12)))),
        ],
    );
    let l = log.clone();
    ee.hooks.insert(START_STUNT, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, OBJ, "the stunt start's quad");
        l.borrow_mut().push(Call::StartStunt(ee.r(5) as u32));
        ee.set(2, 1);
    }));
    let (a, e) = (answers.clone(), echo.clone());
    ee.hooks.insert(GROUND_QUERY, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, WORLD, "the ground query's world");
        let (p, n) = (ee.r(5) as u32, ee.r(6) as u32);
        let wheel = ee.r(8) as u32 - 0x110;
        assert!(WHEEL.contains(&wheel) && ee.r(7) == 0, "the ground query's last two arguments");
        let (point, normal) = a.borrow_mut().pop_front().expect("more ground queries than answers");
        if !e.get() {
            put_v(ee, p, point);
        }
        put_v(ee, n, normal);
    }));
    for at in picks {
        assert!(ee.hooks.remove(at).is_some(), "{at:#x} was not hooked");
    }
    log
}

/// `n` generated rides through the PreProcess (`head`) or the Force body with the calls at `picks`
/// unhooked, the original's own code answering them, and [`quad::Steps`] answering them in the port:
/// every region equal, and the calls left over equal. `tweak` shapes each state, `paths` names the
/// ways it goes (from the decompile's branches, not the port's), each of `want` seen `at_least` times.
#[allow(clippy::too_many_arguments)]
fn each_unhooked(
    seed: u64,
    n: usize,
    head: bool,
    picks: &[u32],
    tweak: impl Fn(&mut Gen, &mut Ride),
    paths: impl Fn(&Ride) -> Vec<&'static str>,
    want: &[&str],
    at_least: u32,
) {
    let Some(mut ee) = image() else { return };
    let mask = ee.u32_at(STRUCK_MASK);
    assert_ne!(mask, 0, "the struck class mask is 0 in the image");
    let answers: Answers = Rc::default();
    let echo = Rc::new(Cell::new(false));
    let log = hook_steps(&mut ee, picks, &answers, &echo);
    let mut g = Gen(seed);
    let mut t = BTreeMap::new();
    for k in 0..n {
        let mut r = g.ride(mask);
        tweak(&mut g, &mut r);
        for p in paths(&r) {
            tally(&mut t, p);
        }
        log.borrow_mut().clear();
        *answers.borrow_mut() = r.answers.iter().copied().collect();
        echo.set(r.echo);
        reset_all(&mut ee);
        poke_ride(&mut ee, &r);
        if head {
            ee.call(GROUND_HEAD, &[u64::from(OBJ)], &[r.s.dt.0], 200_000);
        } else {
            ee.call(GROUND_STEP, &[u64::from(OBJ)], &[r.s.dt.0], 2_000_000);
        }
        only_touched_all(&ee).unwrap_or_else(|e| panic!("state {k}: {e}\n{r:#?}"));
        let was = snap_all(&ee);

        let mut u = r.clone();
        let mut queue: std::collections::VecDeque<(Vf, Vf)> = r.answers.iter().copied().collect();
        let echo_now = r.echo;
        let ground = move |q: Vf| {
            let (p, n) = queue.pop_front().expect("more ground queries than answers");
            (if echo_now { q } else { p }, n)
        };
        let mut m = Only { picks, steps: quad::Steps { bike: u.bike.clone(), rest: Vec::new(), ground } };
        if head {
            quad::ground_head(&u.s.p, &mut u.s.q, &mut u.s.body, u.s.dt, &mut m);
        } else {
            quad::ground_step(&u.s.p, &mut u.s.q, &mut u.s.body, u.s.dt, &mut m);
        }
        u.bike = m.steps.bike;
        // The backflip's stunt id and the rider's stunt word are the bike's; the quad's poke writes
        // the balance word, which the port's quad does not change.
        reset_all(&mut ee);
        poke_ride(&mut ee, &u);
        let now = snap_all(&ee);
        if let Some(i) = was.iter().zip(&now).position(|(a, b)| a != b) {
            panic!(
                "state {k}: {} is {:#010x} ({}) in the original and {:#010x} ({}) in the port\n{r:#?}",
                where_in_all(i),
                was[i],
                f32::from_bits(was[i]),
                now[i],
                f32::from_bits(now[i]),
            );
        }
        assert_eq!(*log.borrow(), m.steps.rest, "state {k}: the calls\n{r:#?}");
        for c in &m.steps.rest {
            match c {
                Call::PreloadCue => tally(&mut t, "cue posted"),
                Call::StartStunt(_) => tally(&mut t, "stunt started"),
                Call::EndStunt => tally(&mut t, "stunt ended"),
                Call::Wreck(_) => tally(&mut t, "wrecked"),
                _ => {}
            }
        }
    }
    each_path_seen(&t, want, at_least);
}

/// Which way the input limiter's gas went, from the decompile's branches (`tests/mxvatv_step.rs`'s).
fn limiter_path(r: &Ride) -> &'static str {
    let (q, b) = (&r.s.q, &r.bike);
    let gas = if b.gate_cut { F::ZERO } else { q.gas };
    if !q.active {
        "off"
    } else if b.held.lock.throttle {
        "throttle off, cut armed"
    } else if b.gas_cut.latched {
        if gas == F::ZERO {
            "cut let go, no gas"
        } else if b.cut_time < b.gas_cut.timer {
            "cut let go, timed out"
        } else {
            "cut holding"
        }
    } else if q.balancing {
        "balance stunt"
    } else if q.push_gate != F::ZERO && !q.airborne {
        "reverse on the ground"
    } else if b.gate_cut {
        "gate cut"
    } else {
        "gas walks"
    }
}

const LIMITER_PATHS: [&str; 9] = [
    "off",
    "throttle off, cut armed",
    "cut let go, no gas",
    "cut let go, timed out",
    "cut holding",
    "balance stunt",
    "reverse on the ground",
    "gate cut",
    "gas walks",
];

/// Which way the clutch ramp went, on the levers as they were poked.
fn clutch_path(r: &Ride) -> &'static str {
    let (q, b) = (&r.s.q, &r.bike);
    let (k, ride) = (&b.clutch_keys, &b.ride);
    if !q.active {
        "off"
    } else if b.clutch == F::ONE && q.gas == F::ONE {
        "rearmed"
    } else if b.clutch != F::ZERO || ride.clutch_timer > k.clutch_torque_scale_time {
        "left alone"
    } else if ride.clutch_timer == F::ZERO {
        "started"
    } else {
        "running"
    }
}

/// With control on: the preload's way, from the decompile's branches (`tests/mxvatv_step.rs`'s).
fn preload_path(r: &Ride) -> &'static str {
    let (q, b, a) = (&r.s.q, &r.bike, &r.bike.air);
    let y = b.stick.y;
    if !q.active || q.control_off {
        "off"
    } else if a.preload_firing {
        if a.preload_timer - r.s.dt > F::ZERO { "firing" } else { "fired out" }
    } else if !q.on_ground || r.s.body.velocity.y() <= 5.0 || q.contact_vel.z() <= 30.0 || q.brake != F::ZERO {
        "off the lip"
    } else if y > F(0.8) {
        "armed"
    } else if y < F(-0.8) && a.preload_armed {
        "fired"
    } else {
        "waiting"
    }
}

/// With control on: the backflip's way (`tests/mxvatv_air.rs`'s).
fn backflip_path(r: &Ride) -> &'static str {
    let (q, b, a) = (&r.s.q, &r.bike, &r.bike.air);
    if !q.active || q.control_off {
        return "off";
    }
    if !a.flip_active {
        let lip = q.on_ground && !b.spin_applied && !b.spin_active && r.s.body.velocity.y() > 5.0 && q.contact_vel.z() > 30.0 && q.brake == F::ZERO;
        if !lip {
            "off the lip"
        } else if b.stick.y < F(-0.8) && b.buttons == game_sim::handling::R1_ALONE {
            "arming"
        } else if b.stick.y > F(0.8) && a.flip_armed {
            "triggered"
        } else {
            "waiting"
        }
    } else if !a.flip_armed {
        if q.on_ground { "in the grace" } else { "off the lip in time" }
    } else if q.on_ground || b.touching || F(a.flip_torque.x()).abs() < b.air_keys.min_backflip_torque {
        if b.flip_clip.0 != 0 { "ended mid-clip" } else { "ended" }
    } else {
        "turning"
    }
}

/// With control on and in the air: the airborne control's ways (`tests/mxvatv_air.rs`'s).
fn airborne_paths(r: &Ride) -> Vec<&'static str> {
    let (q, b) = (&r.s.q, &r.bike);
    if !q.active || q.control_off {
        return vec!["off"];
    }
    if q.on_ground {
        return vec!["on the ground"];
    }
    let mut v = vec![if b.air.flip_torque_applied && b.spin_applied {
        "stick left out"
    } else if b.rider_stunt < 41 {
        "stick scaled"
    } else {
        "stick whole"
    }];
    let idle = b.rider_stunt >= 46;
    let brake = q.front_brake != F::ZERO || q.rear_brake != F::ZERO;
    v.push(if idle && b.gyro.brake && brake {
        "brake gyro"
    } else if idle && b.gyro.gas && q.gas_limited != F::ZERO {
        "gas gyro"
    } else {
        "no gyro"
    });
    v.push(if idle && !b.air.flip_torque_applied && !b.spin_applied && q.speed_len > F(10.0) { "lean force" } else { "no lean force" });
    v
}

fn on(r: &Ride) -> bool {
    r.s.q.active
}

#[test]
fn the_quads_input_limiter_is_the_bikes() {
    each_unhooked(0x0075_a001, 10_000, true, &[INPUT_LIMITER], |_, _| {}, |r| vec![limiter_path(r)], &LIMITER_PATHS, 30);
}

#[test]
fn the_quads_clutch_is_the_bikes() {
    let want = ["off", "rearmed", "left alone", "started", "running"];
    let tweak = |g: &mut Gen, r: &mut Ride| {
        if g.one_in(4) {
            (r.bike.clutch, r.s.q.gas) = (F::ONE, F::ONE);
        } else if g.one_in(3) {
            r.bike.clutch = F::ZERO;
        }
    };
    each_unhooked(0x0075_a002, 10_000, true, &[CLUTCH], tweak, |r| vec![clutch_path(r)], &want, 30);
}

#[test]
fn the_quads_pre_process_runs_the_bikes_steps() {
    each_unhooked(0x0075_a003, 5_000, true, &[INPUT_LIMITER, CLUTCH], |_, _| {}, |r| vec![limiter_path(r)], &LIMITER_PATHS, 15);
}

#[test]
fn the_quads_preload_is_the_bikes() {
    let want = ["off", "firing", "fired out", "off the lip", "armed", "fired", "waiting", "cue posted"];
    each_unhooked(0x0075_a004, 10_000, false, &[PRELOAD], |_, _| {}, |r| vec![preload_path(r)], &want, 30);
}

/// A quarter of the time, a flip turning in the air (where its stunt starts, its clip turns over and
/// its stunt ends); a quarter, on a lip with the stick past the line (where it arms and triggers).
fn turning_flip(g: &mut Gen, r: &mut Ride) {
    let (q, b) = (&mut r.s.q, &mut r.bike);
    match g.bits() % 4 {
        0 => {
            (b.air.flip_active, b.air.flip_armed, q.on_ground, b.touching) = (true, true, false, false);
            let t = b.air_keys.min_backflip_torque.0 + g.f(0.0, 500.0).0;
            b.air.flip_torque = Vf::new(-t, b.air.flip_torque.y(), b.air.flip_torque.z(), b.air.flip_torque.w());
            (b.right_y, b.up_y, b.flip_clip.0) = match g.bits() % 3 {
                0 => (g.f(-1.0, 0.0), g.f(0.01, 1.0), 0),
                1 => (g.f(-1.0, 0.0), g.f(-1.0, -0.01), 1),
                _ => (g.f(0.01, 1.0), g.f(-0.78, 1.0), 2),
            };
        }
        1 => {
            (b.air.flip_active, q.on_ground, b.spin_applied, b.spin_active, q.brake) = (false, true, false, false, F::ZERO);
            r.s.body.velocity = Vf::new(r.s.body.velocity.x(), g.f(5.5, 15.0).0, r.s.body.velocity.z(), 0.0);
            q.contact_vel = with_z(q.contact_vel, g.f(30.5, 60.0));
            if g.b() {
                (b.stick.y, b.buttons) = (g.f(-1.0, -0.81), game_sim::handling::R1_ALONE);
            } else {
                (b.stick.y, b.air.flip_armed) = (g.f(0.81, 1.0), true);
            }
        }
        _ => {}
    }
}

#[test]
fn the_quads_backflip_is_the_bikes() {
    let want = [
        "off",
        "off the lip",
        "arming",
        "triggered",
        "waiting",
        "in the grace",
        "off the lip in time",
        "ended",
        "ended mid-clip",
        "turning",
        "stunt started",
        "stunt ended",
        "wrecked",
    ];
    each_unhooked(0x0075_a005, 10_000, false, &[BACKFLIP], turning_flip, |r| vec![backflip_path(r)], &want, 30);
}

#[test]
fn the_quads_airborne_control_is_the_bikes() {
    let want = ["off", "on the ground", "stick left out", "stick scaled", "stick whole", "brake gyro", "gas gyro", "no gyro", "lean force", "no lean force"];
    let tweak = |g: &mut Gen, r: &mut Ride| {
        if g.b() {
            r.s.q.on_ground = false;
        }
    };
    each_unhooked(0x0075_a006, 10_000, false, &[AIRBORNE_CONTROL], tweak, airborne_paths, &want, 30);
}

#[test]
fn the_quads_suspension_is_the_wheels() {
    let paths = |r: &Ride| {
        if !on(r) {
            return vec!["off"];
        }
        r.bike.wheels.iter().map(|w| if w.has_ground { "asks the ground" } else { "no world" }).collect()
    };
    let tweak = |g: &mut Gen, r: &mut Ride| {
        if g.one_in(8) {
            r.echo = true;
        }
    };
    each_unhooked(0x0075_a007, 5_000, false, &[SUSPENSION], tweak, paths, &["off", "asks the ground", "no world"], 30);
}

#[test]
fn the_quads_tire_forces_are_the_wheels() {
    let paths = |r: &Ride| {
        if !on(r) {
            return vec!["off"];
        }
        r.bike.wheels.iter().map(|w| if w.ray_hit != 0 || w.swept_contact != 0 { "in contact" } else { "clear" }).collect()
    };
    each_unhooked(0x0075_a008, 5_000, false, &[TIRE_FORCES], |_, _| {}, paths, &["off", "in contact", "clear"], 30);
}

#[test]
fn the_quads_wheel_integrate_is_the_wheels() {
    let paths = |r: &Ride| {
        if !on(r) {
            return vec!["off"];
        }
        r.s.q.wheels.iter().map(|w| if !w.steered { "not steered" } else if !w.balancing { "held" } else { "free" }).collect()
    };
    each_unhooked(0x0075_a009, 5_000, false, &[WHEEL_INTEGRATE], |_, _| {}, paths, &["off", "not steered", "held", "free"], 30);
}

/// All six of the Force body's ported calls at once: the original's own code for each, the port's
/// [`quad::Steps`] for each, one after the other on the same quad.
#[test]
fn the_quads_force_body_runs_the_bikes_steps() {
    let picks = [PRELOAD, BACKFLIP, AIRBORNE_CONTROL, SUSPENSION, TIRE_FORCES, WHEEL_INTEGRATE];
    let paths = |r: &Ride| vec![preload_path(r), backflip_path(r)];
    let want = ["off", "firing", "armed", "fired", "arming", "triggered", "turning", "ended", "cue posted", "stunt started"];
    each_unhooked(0x0075_a00a, 5_000, false, &picks, turning_flip, paths, &want, 15);
}
