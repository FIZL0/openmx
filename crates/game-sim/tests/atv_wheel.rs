//! MD37b — the ATV's wheel suspension and tire forces, proved against MX vs. ATV's own code; MD74 —
//! the contact velocity and the steering hinge's force they call.
//!
//! The image is `tests/atv_quad.rs`'s (MD37a's `atv-r-spawn.ee.bin`): one wheel and the quad's rigid
//! body (with the body's own vtable, so its add-force, add-moment and add-force-at run as they are)
//! are laid out in RAM, a generated state is written into them, SLUS-21104's function runs, the same
//! state is written again with the port's answer over it, and the two regions are compared word for
//! word. The original may write nothing else. The ground query is hooked: the hook checks the
//! arguments, logs the call and serves generated answers, which the port's closure serves too. The
//! contact velocity and the hinge run as they are (their hooks only check and log the call, then run
//! the original's code); what the wheel stands on (a body, a vehicle's chassis with its transform, a
//! moving piece of scenery with two virtuals hooked to serve generated answers) is laid out in RAM.
//!
//! Without the image every test here skips (and says so).

use game_sim::atv_wheel::{self, Body, Chassis, Ground, Holder, More, Mover, Other, Spin};
use game_sim::camera::CurveTable;
use game_sim::contact::Node;
use game_sim::quad::TireSurface;
use game_sim::wheel::{Call, Wheel};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

const WHEEL: u32 = 0x01c0_8000;
const BODY: u32 = 0x01c0_2000;
/// The world the wheel asks for the ground; never read, only handed on.
const WORLD: u32 = 0x01c0_2400;
/// What the wheel stands on: a collision object, the body or the chassis it belongs to, their
/// transform, and a mover.
const OBJECT: u32 = 0x01c0_3000;
const OTHER_BODY: u32 = 0x01c0_3400;
const CHASSIS: u32 = 0x01c0_3800;
const NODE: u32 = 0x01c0_3c00;
const MOVER: u32 = 0x01c0_4000;
/// The other wheel of the pair (the wheel's `+0xbc`), and the brake lever the wheel's `+0x24` points
/// at.
const OTHER_WHEEL: u32 = 0x01c0_9000;
const BRAKE: u32 = 0x01c0_4900;
const REGIONS: [(&str, u32, u32); 8] = [
    ("wheel", WHEEL, 0x910),
    ("other wheel", OTHER_WHEEL, 0x200),
    ("body", BODY, 0x200),
    ("object", OBJECT, 0x100),
    ("other body", OTHER_BODY, 0x200),
    ("chassis", CHASSIS, 0x300),
    ("node", NODE, 0x110),
    ("mover", MOVER, 0x250),
];
/// The mover's vtable, and where its velocity and spin virtuals point (hooked, never run).
const MOVER_VTABLE: u32 = 0x01c0_4400;
const MOVER_VELOCITY: u32 = 0x01c0_4600;
const MOVER_SPIN: u32 = 0x01c0_4610;
/// Where the hinge's force is handed from.
const ARGS: u32 = 0x01c0_4800;

const BODY_VTABLE: u32 = 0x003c_d200;
const BODY_WORLD_MOMENT: u32 = 0x0049_c1a0;
const BODY_ADD_FORCE_AT: u32 = 0x0049_c0b0;
const SUSPENSION: u32 = 0x004a_0cb0;
const TIRE_FORCES: u32 = 0x004a_16f0;
const GROUND_QUERY: u32 = 0x0048_33c0;
const CONTACT_VELOCITY: u32 = 0x004a_1150;
const HINGE_FORCE: u32 = 0x004a_2aa0;
const SURFACE_DRAG: u32 = 0x004a_1ec0;
const SPIN_INTEGRATE: u32 = 0x004a_2040;
/// The axis rotation's sine and cosine table (a pointer to it) and its step and inverse step, which
/// the startup code fills in.
const CURVE_TABLE: u32 = 0x003e_ab20;
const CURVE_STEP: u32 = 0x003e_a9c8;
const CURVE_INV_STEP: u32 = 0x003e_a9d0;

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
    if ee.u32_at(BODY_VTABLE + 0x88) != BODY_WORLD_MOMENT || ee.u32_at(BODY_VTABLE + 0x84) != BODY_ADD_FORCE_AT {
        panic!("{} is not the MX vs. ATV quad image these addresses are from", path.display());
    }
    Some(ee)
}

/// One generated wheel and body, with the answers the hooked callees give, in call order.
#[derive(Clone, Debug, Default)]
struct State {
    w: Wheel,
    x: More,
    b: Body,
    dt: F,
    /// The ground query's answers: the point (whole quadword), the unnormalised normal, and the
    /// surface byte it writes into the wheel's record (none: it leaves the record).
    ground: Vec<(Vf, Vf, Option<u8>)>,
    /// The ground query hands the point back as it was given, which puts it exactly level with the
    /// probe's end.
    echo: bool,
    /// What the wheel stands on.
    ground_under: Ground,
    /// A collision object is there, and a mover too (which the object hides).
    also_mover: bool,
    /// What is laid out in RAM when the wheel stands on nothing: a wheel borrowing a load drops its
    /// own ground for its partner's, and what it stood on stays where it was.
    lay: Ground,
    /// The spin step's words, and the other wheel's; `share` is that the other wheel's ground
    /// pointers are the wheel's own (else they are null).
    spin: Spin,
    other: Other,
    share: bool,
}

fn put_f(ee: &mut Ee, a: u32, v: F) {
    ee.set_f32(a, v.0);
}
fn put_i(ee: &mut Ee, a: u32, v: i32) {
    ee.set_u32(a, v as u32);
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
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

fn reset(ee: &mut Ee) {
    for (_, base, len) in REGIONS {
        for o in (0..len).step_by(4) {
            ee.set_u32(base + o, 0);
        }
    }
}

/// The wheel and body at SLUS-21104's offsets (see `game_sim::atv_wheel`'s table).
fn poke(ee: &mut Ee, s: &State) {
    let (w, x, b) = (&s.w, &s.x, &s.b);
    ee.set_u32(WHEEL + 0x1e0, if w.has_ground { WORLD } else { 0 });
    ee.set_u32(WHEEL + 0x1e4, BODY);
    put_b(ee, WHEEL + 0x2c, w.steered);
    put_b(ee, WHEEL + 0x30, w.balancing);
    put_f(ee, WHEEL + 0x3c, w.max_compression);
    put_f(ee, WHEEL + 0x40, w.max_damper_speed);
    put_f(ee, WHEEL + 0x44, w.extension_rate);
    put_f(ee, WHEEL + 0x48, w.spring_rate);
    put_f(ee, WHEEL + 0x4c, w.damping_out);
    put_f(ee, WHEEL + 0x50, w.damping_in);
    put_v(ee, WHEEL + 0x80, w.probe);
    put_v(ee, WHEEL + 0x90, w.mount);
    put_v(ee, WHEEL + 0xc0, x.kept);
    put_i(ee, WHEEL + 0xd0, w.ray_hit);
    put_i(ee, WHEEL + 0xd4, w.swept_contact);
    put_i(ee, WHEEL + 0xd8, w.terrain_wins);
    put_b(ee, WHEEL + 0xdc, w.contact);
    put_i(ee, WHEEL + 0xec, w.ray_steps);
    put_f(ee, WHEEL + 0xf0, w.compression);
    put_f(ee, WHEEL + 0xf4, w.last_compression);
    put_f(ee, WHEEL + 0xf8, w.spring_force);
    put_v(ee, WHEEL + 0x100, w.probe_end);
    ee.set_u32(WHEEL + 0x110, u32::from(x.record) | 0x5a5a_5a00);
    put_v(ee, WHEEL + 0x120, w.contact_point);
    put_v(ee, WHEEL + 0x130, w.contact_normal);
    put_v(ee, WHEEL + 0x140, w.load_point);
    put_v(ee, WHEEL + 0x150, w.mount_world);
    put_v(ee, WHEEL + 0x160, w.probe_end_world);
    put_v(ee, WHEEL + 0x170, w.contact_velocity);
    put_v(ee, WHEEL + 0x180, w.chassis_velocity);
    put_v(ee, WHEEL + 0x190, x.chassis_velocity_local);
    put_i(ee, WHEEL + 0x1a0, x.ground_known);
    put_v(ee, WHEEL + 0x1b0, x.ground_velocity);
    put_v(ee, WHEEL + 0x1c0, x.ground_point_velocity);
    put_f(ee, WHEEL + 0x60, w.steer_torque);
    put_v(ee, WHEEL + 0x8c0, x.steer_axis);
    put_v(ee, WHEEL + 0x200, w.ground_normal);
    put_f(ee, WHEEL + 0x214, w.probe_length);
    put_f(ee, WHEEL + 0x8ac, w.steer_angle);
    put_f(ee, WHEEL + 0x8b0, w.steer_rate);
    poke_spin(ee, &s.spin);
    ee.set_u32(WHEEL + 0xbc, OTHER_WHEEL);
    put_b(ee, OTHER_WHEEL + 0xdc, s.other.contact);
    put_f(ee, OTHER_WHEEL + 0xf8, s.other.spring_force);
    if s.share {
        point_at(ee, OTHER_WHEEL, &s.ground_under, s.also_mover);
    }

    ee.set_u32(BODY + 0x110, BODY_VTABLE);
    ee.set_u32(BODY + 0xe8, 0); // no force listeners on the body
    put_v(ee, BODY + 0x70, b.position);
    put_v(ee, BODY + 0xa0, b.force);
    put_v(ee, BODY + 0xd0, b.correction);
    put_v(ee, BODY + 0x190, b.orientation);
    put_v(ee, BODY + 0x1a0, b.cofg);
    put_v(ee, BODY + 0x1b0, b.moment);
    put_v(ee, BODY + 0x80, b.velocity);
    put_v(ee, BODY + 0x1d0, b.spin);

    let put_node = |ee: &mut Ee, n: &Node| {
        for (k, r) in n.rows.iter().enumerate() {
            put_v(ee, NODE + 0x70 + 0x10 * k as u32, *r);
        }
        put_v(ee, NODE + 0xa0, n.origin);
        ee.set_u32(NODE + 0xf0, 0); // no parent
        ee.set_u32(NODE + 0x100, 1); // up to date
    };
    point_at(ee, WHEEL, &s.ground_under, s.also_mover);
    match if s.ground_under == Ground::Still { &s.lay } else { &s.ground_under } {
        Ground::Still => {}
        Ground::Object { cofg, cofg_world, on } => {
            put_v(ee, OBJECT + 0x80, *cofg);
            put_v(ee, OBJECT + 0x90, *cofg_world);
            match on {
                Holder::Body(o) => {
                    ee.set_u32(OBJECT + 0xe4, OTHER_BODY);
                    // A chassis and transform there too, which the body hides.
                    ee.set_u32(OBJECT + 0xe0, CHASSIS);
                    ee.set_u32(OBJECT + 0xe8, NODE);
                    put_v(ee, OTHER_BODY + 0x70, o.position);
                    put_v(ee, OTHER_BODY + 0x80, o.velocity);
                    put_v(ee, OTHER_BODY + 0x190, o.orientation);
                    put_v(ee, OTHER_BODY + 0x1d0, o.spin);
                }
                Holder::Chassis(c) => {
                    ee.set_u32(OBJECT + 0xe0, CHASSIS);
                    ee.set_u32(OBJECT + 0xe8, NODE);
                    put_v(ee, CHASSIS + 0x180, c.cofg);
                    put_v(ee, CHASSIS + 0x250, c.velocity);
                    put_v(ee, CHASSIS + 0x280, c.spin);
                    put_node(ee, &c.node);
                }
            }
        }
        Ground::Mover(m) => {
            ee.set_u32(MOVER + 4, MOVER_VTABLE);
            ee.set_u32(MOVER + 0x244, NODE);
            put_v(ee, MOVER + 0x180, m.origin);
            put_node(ee, &m.node);
            ee.set_u32(MOVER_VTABLE + 0xc0, MOVER_VELOCITY);
            ee.set_u32(MOVER_VTABLE + 0xc4, MOVER_SPIN);
        }
    }
}

/// A wheel's two ground pointers (`+0x1f0` an object, `+0x1f4` a mover) to what `g` lays out.
fn point_at(ee: &mut Ee, wheel: u32, g: &Ground, also_mover: bool) {
    match g {
        Ground::Still => {}
        Ground::Object { .. } => {
            ee.set_u32(wheel + 0x1f0, OBJECT);
            if also_mover {
                ee.set_u32(wheel + 0x1f4, MOVER);
            }
        }
        Ground::Mover(_) => ee.set_u32(wheel + 0x1f4, MOVER),
    }
}

/// The spin step's words of the wheel (see [`Spin`]'s offsets), and the brake lever.
fn poke_spin(ee: &mut Ee, p: &Spin) {
    match p.brake {
        Some(v) => {
            ee.set_u32(WHEEL + 0x24, BRAKE);
            put_f(ee, BRAKE, v);
        }
        None => ee.set_u32(WHEEL + 0x24, 0),
    }
    put_i(ee, WHEEL + 0x34, p.parametric);
    put_f(ee, WHEEL + 0x38, p.donut);
    put_f(ee, WHEEL + 0x58, p.inv_inertia);
    put_f(ee, WHEEL + 0x5c, p.tire_inertia);
    put_v(ee, WHEEL + 0xa0, p.arm);
    put_i(ee, WHEEL + 0xb0, p.no_moment);
    put_i(ee, WHEEL + 0xb4, p.rear_pair);
    put_f(ee, WHEEL + 0xb8, p.other_load_scale);
    put_b(ee, WHEEL + 0xe0, p.slip.spinning);
    put_b(ee, WHEEL + 0xe4, p.slip.locked);
    put_b(ee, WHEEL + 0xe8, p.slip.sliding);
    put_f(ee, WHEEL + 0x1d0, p.angle);
    put_f(ee, WHEEL + 0x1d4, p.spin);
    put_f(ee, WHEEL + 0x210, p.torque);
    put_f(ee, WHEEL + 0x224, p.cleared[0]);
    put_f(ee, WHEEL + 0x228, p.cleared[1]);
    put_f(ee, WHEEL + 0x234, p.side_per_load);
    for (k, r) in p.surfaces.iter().enumerate() {
        for (t, &v) in r.drive.iter().chain(&r.side).chain(&r.drag).enumerate() {
            put_f(ee, WHEEL + 0x238 + 0x40 * t as u32 + 4 * k as u32, v);
        }
    }
    put_f(ee, WHEEL + 0x838, p.inv_radius);
    put_v(ee, WHEEL + 0x840, p.local_velocity);
    put_v(ee, WHEEL + 0x850, p.local_force);
    for (k, &r) in p.frame.iter().enumerate() {
        put_v(ee, WHEEL + 0x8d0 + 0x10 * k as u32, r);
    }
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
            return Err(format!("the original wrote {a:#010x}..{b:#010x}, which is not the wheel, the body or the stack"));
        }
    }
    Ok(())
}

/// Calls compared by their `Debug` text, which tells `-0.0` from `0.0`.
fn texts(calls: &[Call]) -> Vec<String> {
    calls.iter().map(|c| format!("{c:?}")).collect()
}

type Log = Rc<RefCell<Vec<Call>>>;
type Answers = Rc<RefCell<VecDeque<(Vf, Vf, Option<u8>)>>>;

/// Run the original's own `addr` from inside its hook, on the arguments the hook was handed, and leave
/// the machine as a return from it would: the interpreter files the hook back under `pc`, and the
/// nested call must not forget what the outer one wrote.
fn run_inside(ee: &mut Ee, addr: u32) {
    let (args, floats) = ([ee.r(4), ee.r(5), ee.r(6), ee.r(7)], [ee.f(12)]);
    let (ra, sp, stack, written) = (ee.r(31), ee.r(29), ee.stack, ee.written.clone());
    ee.stack = (sp as u32 - 0x40) & !0xf;
    ee.call(addr, &args, &floats, 100_000);
    ee.written.extend(written);
    (ee.stack, ee.pc) = (stack, addr);
    ee.set(29, sp);
    ee.set(31, ra);
}

/// The ground query's answer to `query`.
fn answer(s: &State, i: usize, query: Vf, record: &mut u8) -> (Vf, Vf) {
    let (p, n, r) = s.ground[i];
    if let Some(r) = r {
        *record = r;
    }
    (if s.echo { query } else { p }, n)
}

/// The callees hooked: the ground query checks what it is handed, logs it and writes the next answer;
/// the contact velocity and the hinge check and log theirs and run as they are; the mover's two
/// virtuals answer.
fn hook(ee: &mut Ee, log: &Log, ground: &Answers, echo: bool, mover: (Vf, Vf), hinge_f12: f32) {
    let (l, g) = (log.clone(), ground.clone());
    ee.hooks.insert(
        GROUND_QUERY,
        Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, WORLD, "the ground query's world");
            assert_eq!((ee.r(7), ee.r(8) as u32), (0, WHEEL + 0x110), "the ground query's last two arguments");
            let (p, n) = (ee.r(5) as u32, ee.r(6) as u32);
            l.borrow_mut().push(Call::GroundQuery(get_v(ee, p)));
            let (point, normal, record) = g.borrow_mut().pop_front().expect("more ground queries than answers");
            if !echo {
                put_v(ee, p, point);
            }
            put_v(ee, n, normal);
            if let Some(r) = record {
                ee.write(WHEEL + 0x110, 1, u128::from(r));
            }
        }),
    );
    let l = log.clone();
    ee.hooks.insert(
        CONTACT_VELOCITY,
        Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, WHEEL, "the contact velocity's wheel");
            l.borrow_mut().push(Call::ContactVelocity);
            run_inside(ee, CONTACT_VELOCITY);
        }),
    );
    for (at, v) in [(MOVER_VELOCITY, mover.0), (MOVER_SPIN, mover.1)] {
        ee.hooks.insert(
            at,
            Box::new(move |ee| {
                assert_eq!(ee.r(5) as u32, MOVER, "the mover's virtual's object");
                let out = ee.r(4);
                put_v(ee, out as u32, v);
                ee.set(2, out);
            }),
        );
    }
    let l = log.clone();
    ee.hooks.insert(
        HINGE_FORCE,
        Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, WHEEL, "the hinge's wheel");
            assert_eq!(ee.f(12).to_bits(), hinge_f12.to_bits(), "the hinge's f12");
            let (f, p) = (get_v(ee, ee.r(5) as u32), get_v(ee, ee.r(6) as u32));
            assert_eq!(ee.r(6) as u32, WHEEL + 0x140, "the hinge's point");
            l.borrow_mut().push(Call::HingeForce(f, p));
            run_inside(ee, HINGE_FORCE);
        }),
    );
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
    /// A direction: mostly up, sometimes anywhere, not quite unit length, `w` near 1.
    fn normal(&mut self) -> Vf {
        let tilt = if self.one_in(4) { 1.0 } else { 0.5 };
        let (x, z) = (self.f(-tilt, tilt).0, self.f(-tilt, tilt).0);
        let y = if self.one_in(6) { self.f(-1.0, 1.0).0 } else { (1.0 - (x * x + z * z).min(0.99)).sqrt() };
        let s = self.f(0.9, 1.1).0;
        Vf::new(x * s, y * s, z * s, self.f(0.99, 1.0).0)
    }
    fn quaternion(&mut self) -> Vf {
        // Mostly near upright, so that the suspension points down.
        let mut v = [self.f(-0.3, 0.3).0, self.f(-1.0, 1.0).0, self.f(-0.3, 0.3).0, self.f(0.5, 1.0).0];
        if self.one_in(5) {
            v = [self.f(-1.0, 1.0).0, self.f(-1.0, 1.0).0, self.f(-1.0, 1.0).0, self.f(-1.0, 1.0).0];
        }
        let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2] + v[3] * v[3]).sqrt().max(1e-3);
        Vf::new(v[0] / n, v[1] / n, v[2] / n, v[3] / n)
    }

    fn state(&mut self) -> State {
        let mut w = Wheel {
            has_ground: !self.one_in(20),
            steered: self.b(),
            balancing: self.b(),
            max_compression: self.f(0.2, 3.0),
            max_damper_speed: self.f(2.0, 30.0),
            extension_rate: self.f(1.0, 20.0),
            spring_rate: self.f(100.0, 3000.0),
            damping_out: self.f(10.0, 300.0),
            damping_in: self.f(10.0, 300.0),
            // Mostly down the body, as the suspension leaves it.
            probe: if self.one_in(6) {
                self.vec(-2.0, 2.0)
            } else {
                Vf::new(self.f(-0.3, 0.3).0, self.f(-2.5, -0.3).0, self.f(-0.3, 0.3).0, self.f(0.0, 1.0).0)
            },
            mount: Vf::new(self.f(-2.0, 2.0).0, self.f(-1.0, 0.5).0, self.f(-2.5, 2.5).0, self.f(0.0, 1.0).0),
            ray_hit: if self.one_in(3) { 0 } else { 1 },
            swept_contact: if self.b() { 1 } else { 0 },
            terrain_wins: if self.b() { 1 } else { 0 },
            contact: self.b(),
            ray_steps: self.bits() as i32 % 7,
            compression: self.f(-1.0, 1.0),
            last_compression: if self.one_in(4) { F::ZERO } else { self.f(0.0, 1.5) },
            spring_force: self.f(-100.0, 100.0),
            probe_end: self.vec(-5.0, 5.0),
            load_point: self.vec(-5.0, 5.0),
            mount_world: self.vec(-5.0, 5.0),
            probe_end_world: self.vec(-5.0, 5.0),
            contact_velocity: self.vec(-5.0, 5.0),
            chassis_velocity: self.vec(-5.0, 5.0),
            ground_normal: self.normal(),
            probe_length: self.f(0.5, 2.5),
            ..Wheel::default()
        };
        if self.one_in(10) {
            w.max_compression = F::ZERO;
        }
        let b = self.body();
        // The contact sits around the wheel: somewhere about the body's centre, a few feet down.
        let around = |g: &mut Gen| {
            Vf::new(
                b.position.x() + g.f(-3.0, 3.0).0,
                b.position.y() + g.f(-4.0, 1.0).0,
                b.position.z() + g.f(-3.0, 3.0).0,
                g.f(0.0, 1.0).0,
            )
        };
        w.contact_point = around(self);
        w.contact_normal = self.normal();
        if self.one_in(15) {
            // Next to nothing: the suspension line is as good as parallel to it.
            w.contact_normal = w.contact_normal.mul(Vf::splat(1e-7));
        }
        let point = around(self);
        // The same point every time it asks: only the normals move it on.
        let n = self.normal();
        let ground = (0..5)
            .map(|_| {
                let n = if self.b() { n } else if self.one_in(3) { w.ground_normal } else { self.normal() };
                (point, n, if self.one_in(4) { None } else { Some(1 + (self.bits() % 255) as u8) })
            })
            .collect();
        // A swept contact a tenth either side of the ray's answer, now and then exactly.
        if self.one_in(3) {
            let d = match self.bits() % 4 {
                0 => 0.1,
                1 => -0.1,
                _ => self.f(-0.3, 0.3).0,
            };
            w.contact_point = Vf::new(w.contact_point.x(), (F(point.y()) + d).0, w.contact_point.z(), w.contact_point.w());
        }
        let x = More {
            record: self.bits() as u8,
            kept: if self.one_in(10) { Vf::ZERO } else { self.vec(-3.0, 3.0) },
            chassis_velocity_local: self.vec(-5.0, 5.0),
            ground_known: if self.one_in(5) { 1 } else { 0 },
            ground_velocity: self.vec(-5.0, 5.0),
            ground_point_velocity: self.vec(-5.0, 5.0),
            steer_axis: self.normal(),
        };
        w.steer_torque = self.f(-50.0, 50.0);
        let ground_under = self.ground_under(&w);
        State {
            w,
            x,
            b,
            dt: self.f(0.001, 0.04),
            ground,
            echo: self.one_in(8),
            ground_under,
            also_mover: self.b(),
            ..State::default()
        }
    }

    /// One surface's row of tables, with its caps sometimes not positive.
    fn surface(&mut self) -> TireSurface {
        let mut drive = [F::ZERO; 8];
        for (k, d) in drive.iter_mut().enumerate() {
            *d = match k {
                2 if self.one_in(10) => self.f(-1.0, 0.0),
                0 | 1 => self.f(0.1, 3.0),
                _ => self.f(0.05, 2.0),
            };
        }
        let mut side = [F::ZERO; 13];
        for (k, v) in side.iter_mut().enumerate() {
            *v = match k % 4 {
                0 if k < 12 => self.f(0.01, 1.0),
                1 if k < 12 => self.f(0.3, 3.0),
                _ => self.f(0.1, 3.0),
            };
        }
        TireSurface { drive, side, drag: [self.f(0.0, 3000.0), self.f(0.0, 300.0), self.f(0.0, 2.0)] }
    }

    /// A state for the wheel integrate's two steps: the wheel's [`Spin`] words, a surface under
    /// sixteen, an other wheel, and contact velocities both slow and fast along the tire.
    fn spin_state(&mut self) -> State {
        let mut s = self.state();
        s.lay = s.ground_under;
        s.x.record = (self.bits() % 16) as u8;
        s.w.steer_angle = if self.one_in(5) { self.f(-12.0, 12.0) } else { self.f(-1.0, 1.0) };
        s.w.steer_rate = self.f(-5.0, 5.0);
        s.w.contact_velocity = if self.b() { self.vec(-12.0, 12.0) } else { self.vec(-60.0, 60.0) };
        s.w.spring_force = if self.one_in(6) { self.f(-200.0, 0.0) } else { self.f(0.0, 1500.0) };
        s.x.chassis_velocity_local = if self.b() { self.vec(-6.0, 6.0) } else { self.vec(-60.0, 60.0) };
        s.other = Other { contact: self.b(), spring_force: self.f(-100.0, 1500.0), ground: Ground::Still };
        s.share = self.b();
        let law = s.w.contact || s.other.contact;
        let mut surfaces = [TireSurface::default(); 16];
        for r in &mut surfaces {
            *r = self.surface();
        }
        s.spin = Spin {
            brake: match self.bits() % 5 {
                0 if !law => None,
                0..=2 => Some(F::ZERO),
                3 => Some(self.f(0.0, 1.0)),
                _ => Some(self.f(-1.0, 0.0)),
            },
            parametric: if self.b() { 0 } else { 1 + (self.bits() % 3) as i32 },
            donut: if self.one_in(3) { F::ZERO } else { self.f(-0.3, 1.3) },
            inv_inertia: self.f(0.1, 2.0),
            tire_inertia: self.f(0.1, 5.0),
            arm: self.vec(-2.0, 2.0),
            no_moment: i32::from(self.one_in(4)),
            rear_pair: i32::from(self.b()),
            other_load_scale: self.f(0.5, 1.5),
            slip: game_sim::tire::Slip { spinning: self.b(), locked: self.one_in(3), sliding: self.b() },
            angle: self.f(-3.2, 3.2),
            spin: if self.b() { self.f(-1.5, 1.5) } else { self.f(-80.0, 80.0) },
            torque: if self.b() { self.f(-300.0, 300.0) } else { self.f(-4000.0, 4000.0) },
            cleared: [self.f(-1.0, 1.0), self.f(-1.0, 1.0)],
            side_per_load: self.f(-1.0, 1.0),
            surfaces,
            inv_radius: self.f(0.4, 2.0),
            local_velocity: self.vec(-5.0, 5.0),
            local_force: self.vec(-500.0, 500.0),
            frame: [self.normal(), self.normal(), self.normal(), self.vec(-1.0, 1.0)],
        };
        s
    }

    fn node(&mut self) -> Node {
        let q = Body { orientation: self.quaternion(), ..Body::default() };
        let r = game_sim::quad::Body { orientation: q.orientation, ..Default::default() };
        let row = |v: Vf, g: &mut Gen| {
            let v = r.to_world(v);
            Vf::new(v.x(), v.y(), v.z(), g.f(-1.0, 1.0).0)
        };
        Node {
            rows: [row(Vf::new(1.0, 0.0, 0.0, 0.0), self), row(Vf::new(0.0, 1.0, 0.0, 0.0), self), row(Vf::new(0.0, 0.0, 1.0, 0.0), self)],
            origin: self.vec(-50.0, 50.0),
        }
    }

    /// A body of the quad's own kind: anywhere, moving and turning.
    fn body(&mut self) -> Body {
        Body {
            position: self.vec(-50.0, 50.0),
            velocity: self.vec(-30.0, 30.0),
            force: self.vec(-500.0, 500.0),
            moment: self.vec(-500.0, 500.0),
            correction: self.vec(-5.0, 5.0),
            orientation: self.quaternion(),
            cofg: self.vec(-0.5, 0.5),
            spin: self.vec(-5.0, 5.0),
        }
    }

    fn ground_under(&mut self, _w: &Wheel) -> Ground {
        match self.bits() % 4 {
            0 => Ground::Still,
            1 => Ground::Object {
                cofg: self.vec(-2.0, 2.0),
                cofg_world: self.vec(-2.0, 2.0),
                on: Holder::Body(self.body()),
            },
            2 => Ground::Object {
                cofg: self.vec(-2.0, 2.0),
                cofg_world: self.vec(-2.0, 2.0),
                on: Holder::Chassis(Chassis {
                    cofg: self.vec(-1.0, 1.0),
                    velocity: self.vec(-30.0, 30.0),
                    spin: self.vec(-3.0, 3.0),
                    node: self.node(),
                }),
            },
            _ => Ground::Mover(Mover {
                node: self.node(),
                origin: self.vec(-5.0, 5.0),
                velocity: self.vec(-30.0, 30.0),
                spin: self.vec(-3.0, 3.0),
            }),
        }
    }
}

fn tally(t: &mut BTreeMap<&'static str, u32>, path: &'static str) {
    *t.entry(path).or_insert(0) += 1;
}

fn each_path_seen(tally: &BTreeMap<&'static str, u32>, paths: &[&str], at_least: u32) {
    for want in paths {
        let n = tally.get(want).copied().unwrap_or(0);
        assert!(n >= at_least, "only {n} state(s) went down {want:?}: {tally:?}");
    }
}

/// The mover's two virtuals' answers in `s` (the velocity and the spin), if there is a mover.
fn mover_answers(s: &State) -> (Vf, Vf) {
    match s.ground_under {
        Ground::Mover(m) => (m.velocity, m.spin),
        _ => (Vf::ZERO, Vf::ZERO),
    }
}

/// Run `addr` on the state, then the port, and compare the regions and the calls.
fn compare(ee: &mut Ee, k: usize, s: &State, addr: u32, port: impl Fn(&mut State) -> Vec<Call>) {
    compare_with(ee, k, s, addr, &[u64::from(WHEEL)], &[s.dt.0], port);
}

fn compare_with(ee: &mut Ee, k: usize, s: &State, addr: u32, ints: &[u64], floats: &[f32],
    port: impl Fn(&mut State) -> Vec<Call>) {
    let log: Log = Rc::default();
    let ground: Answers = Rc::new(RefCell::new(s.ground.iter().copied().collect()));
    // The tire forces hand the hinge 1.0; called straight, it gets what it is handed.
    // The spin step, a quarter.
    let hinge_f12 = match addr {
        HINGE_FORCE => floats[0],
        SPIN_INTEGRATE => 0.25,
        _ => 1.0,
    };
    hook(ee, &log, &ground, s.echo, mover_answers(s), hinge_f12);
    reset(ee);
    poke(ee, s);
    ee.call(addr, ints, floats, 100_000);
    only_touched_the_regions(ee).unwrap_or_else(|e| panic!("state {k}: {e}\n{s:#?}"));
    let was = snap(ee);
    let mut t = s.clone();
    let calls = port(&mut t);
    reset(ee);
    poke(ee, &t);
    let now = snap(ee);
    if let Some(i) = was.iter().zip(&now).position(|(a, b)| a != b) {
        panic!(
            "state {k}: {} is {:#010x} ({}) in the original and {:#010x} ({}) in the port\n{s:#?}",
            where_is(i),
            was[i],
            f32::from_bits(was[i]),
            now[i],
            f32::from_bits(now[i]),
        );
    }
    assert_eq!(texts(&log.borrow()), texts(&calls), "state {k}: the calls\n{s:#?}");
}

#[test]
fn the_atv_wheel_suspension_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let mut g = Gen(0x0037_b001);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        let s = g.state();
        let mut d = s.clone();
        let mut asked = 0;
        let calls = atv_wheel::suspension(&mut d.w, &mut d.x, &d.b, &mut |q, r| {
            asked += 1;
            answer(&s, asked - 1, q, r)
        });
        // The record as the last query left it, before a miss under a swept contact zeroes it.
        let queried = s.ground[..asked].iter().fold(s.x.record, |r, a| a.2.unwrap_or(r));
        tally(&mut t, match asked {
            0 => "no world",
            1 => "one ray",
            5 => "five rays",
            _ => "two to four rays",
        });
        if asked > 0 {
            tally(&mut t, match (d.w.ray_hit, s.w.swept_contact, d.w.swept_contact) {
                (0, 0, _) => "missed",
                (0, _, _) if d.x.record != queried => "missed under a swept contact",
                (0, _, _) if d.w.terrain_wins != s.w.terrain_wins => "swept kept, terrain dropped",
                (0, _, _) => "swept kept",
                (_, 0, _) => "ray hit",
                (_, _, 0) if s.w.terrain_wins == 0 => "terrain won",
                _ => "terrain kept",
            });
            if s.echo {
                tally(&mut t, "level with the probe");
            }
        }
        compare(&mut ee, k, &s, SUSPENSION, |t| {
            let mut i = 0;
            let u = t.clone();
            atv_wheel::suspension(&mut t.w, &mut t.x, &t.b, &mut |q, r| {
                i += 1;
                answer(&u, i - 1, q, r)
            })
        });
        assert_eq!(texts(&calls).len(), asked);
    }
    each_path_seen(
        &t,
        &[
            "no world",
            "one ray",
            "two to four rays",
            "five rays",
            "missed",
            "missed under a swept contact",
            "swept kept",
            "swept kept, terrain dropped",
            "ray hit",
            "terrain won",
            "terrain kept",
            "level with the probe",
        ],
        100,
    );
}

#[test]
fn the_atv_wheel_tire_forces_are_the_originals() {
    let Some(mut ee) = image() else { return };
    let mut g = Gen(0x0037_b002);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        let mut s = g.state();
        let port = |t: &mut State| {
            let (mut x, mut under) = (t.x, t.ground_under);
            let calls = atv_wheel::tire_forces(&mut t.w, &mut t.b, t.dt, &mut |c, w, b, x| {
                atv_wheel::run(c, w, b, x, &mut under);
            }, &mut x);
            (t.x, t.ground_under) = (x, under);
            calls
        };
        // The ties, found by a first run of the port: the contact exactly on the probe's end (no
        // travel at all), and the travel exactly at the suspension's limit.
        if g.one_in(8) && (s.w.ray_hit != 0 || s.w.swept_contact != 0) {
            let mut d = s.clone();
            port(&mut d);
            s.w.contact_point = d.w.probe_end_world;
            tally(&mut t, "on the probe's end");
        } else if g.one_in(7) {
            s.w.last_compression = F::ZERO;
            let mut d = s.clone();
            port(&mut d);
            if d.w.contact && d.w.compression < s.w.max_compression {
                s.w.max_compression = d.w.compression;
                tally(&mut t, "at the limit");
            }
        }
        let mut d = s.clone();
        let calls = port(&mut d);
        tally(&mut t, match () {
            _ if s.w.ray_hit == 0 && s.w.swept_contact == 0 => "no contact",
            _ if !d.w.contact => "no load",
            _ => "loaded",
        });
        if (s.w.ray_hit != 0 || s.w.swept_contact != 0) && d.w.load_point == s.w.load_point {
            tally(&mut t, "no load point");
        }
        if d.w.contact {
            let bottomed = calls.iter().filter(|c| **c == Call::ContactVelocity).count() == 2;
            tally(&mut t, match (bottomed, d.b.position != s.b.position) {
                (false, _) => "sprung",
                (true, true) => "bottomed out, shoved",
                (true, false) => "bottomed out, no shove",
            });
            tally(&mut t, if calls.iter().any(|c| matches!(c, Call::HingeForce(..))) { "through the hinge" } else { "into the body" });
            tally(&mut t, if d.w.spring_force == F::ZERO { "no spring force" } else { "spring force" });
        }
        tally(&mut t, if d.w.compression != s.w.last_compression && d.w.compression > d.w.last_compression.min(d.w.compression) {
            "extension limited"
        } else {
            "extension free"
        });
        compare(&mut ee, k, &s, TIRE_FORCES, port);
    }
    each_path_seen(
        &t,
        &[
            "no contact",
            "no load",
            "no load point",
            "on the probe's end",
            "at the limit",
            "loaded",
            "sprung",
            "bottomed out, shoved",
            "bottomed out, no shove",
            "through the hinge",
            "into the body",
            "no spring force",
            "spring force",
        ],
        100,
    );
}

#[test]
fn the_atv_wheel_contact_velocity_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let mut g = Gen(0x0074_c001);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        let s = g.state();
        tally(&mut t, match (s.x.ground_known, &s.ground_under) {
            (1, _) => "known",
            (_, Ground::Still) => "still",
            (_, Ground::Object { on: Holder::Body(_), .. }) => "a body",
            (_, Ground::Object { on: Holder::Chassis(_), .. }) => "a chassis",
            (_, Ground::Mover(_)) => "a mover",
        });
        if s.x.ground_known == 0 && s.also_mover && matches!(s.ground_under, Ground::Object { .. }) {
            tally(&mut t, "an object over a mover");
        }
        // Called straight, the original goes through its own hook, which logs it.
        compare(&mut ee, k, &s, CONTACT_VELOCITY, |t| {
            atv_wheel::contact_velocity(&mut t.w, &mut t.x, &t.b, &mut t.ground_under);
            vec![Call::ContactVelocity]
        });
    }
    each_path_seen(&t, &["known", "still", "a body", "a chassis", "a mover", "an object over a mover"], 500);
}

#[test]
fn the_atv_wheel_hinge_force_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let mut g = Gen(0x0074_c002);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        let s = g.state();
        let force = g.vec(-3000.0, 3000.0);
        let scale = if g.one_in(4) { g.f(-2.0, 2.0) } else { F::ONE };
        tally(&mut t, if scale == F::ONE { "scale one" } else { "other scale" });
        put_v(&mut ee, ARGS, force);
        // Called straight, the original goes through its own hook, which logs it.
        compare_with(&mut ee, k, &s, HINGE_FORCE, &[WHEEL, ARGS, WHEEL + 0x140].map(u64::from), &[scale.0], |t| {
            let point = t.w.load_point;
            atv_wheel::hinge_force(&mut t.w, &t.x, &mut t.b, force, point, scale);
            vec![Call::HingeForce(force, point)]
        });
    }
    each_path_seen(&t, &["scale one", "other scale"], 1000);
}

/// MD76a — the terrain object the quad's wheels hold in the image (wheel `+0x1e0`): Desert Oasis's.
const TERRAIN: u32 = 0x0184_c7d0;
/// The ground query's ring of cells (32 keys cleared on a cold start) and the terrain object's ring
/// words, which the original's query writes as it goes.
const RING: (u32, u32) = (0x0054_1550, 0x0054_1550 + 32 * 0x70);
const TERRAIN_RING: (u32, u32) = (TERRAIN + 0x9e0, TERRAIN + 0x9e8);

/// One captured wheel: its words (`+0..+0x300`) and its quad's rigid body's (`+0..+0x280`).
struct Captured {
    wheel: Vec<u8>,
    body: Vec<u8>,
}

/// Desert Oasis's terrain file and every wheel of every row of MD37a's fifteen captures.
fn desert_oasis() -> Option<(Vec<u8>, Vec<Captured>)> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let res = kit.ctx("mxvatv-ps2/md37a").join("mc02.res");
    if !res.exists() {
        skip(&format!("{} is not there: Desert Oasis's mc02.res off MX vs. ATV's disc (Env/MC02; journal md76-atv-ground)", res.display()));
        return None;
    }
    let file = std::fs::read(&res).unwrap();
    let archive = game_sim::archive::Res::parse(&file).unwrap();
    let trn = archive.read(&file, archive.find("Env\\mc02\\MC02.ps2.trn").expect("mc02.res holds MC02.ps2.trn")).unwrap();
    let dir = kit.ctx("capture/md37a");
    let mut names: Vec<_> = std::fs::read_dir(&dir).ok().into_iter().flatten().flatten().map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "csv")).collect();
    names.sort();
    if names.len() < 15 {
        skip(&format!("{} does not hold MD37a's fifteen captures", dir.display()));
        return None;
    }
    let hex = |s: &str| (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect::<Vec<u8>>();
    let mut out = Vec::new();
    for name in names {
        let text = std::fs::read_to_string(&name).unwrap();
        let mut lines = text.lines();
        let head: Vec<&str> = lines.next().unwrap().split(',').collect();
        let col = |n: &str| head.iter().position(|h| *h == n).unwrap();
        let (body, wheels) = (col("body"), (0..4).map(|i| col(&format!("wheel{i}.raw"))).collect::<Vec<_>>());
        for line in lines {
            let f: Vec<&str> = line.split(',').collect();
            for &c in &wheels {
                out.push(Captured { wheel: hex(f[c]), body: hex(f[body]) });
            }
        }
    }
    Some((trn, out))
}

/// The suspension on Desert Oasis with nothing hooked but a log on the ground query, which then runs
/// as the original's own over the image's terrain object: every captured wheel of MD37a's rides,
/// with its own mount, probe, ground normal and contact and its quad's body, against the port's
/// suspension asking [`atv_ground::Terrain::ground`] over the map file. The record byte (wheel
/// `+0x110`, the query's `t0`) is compared with the rest of the wheel. Half the wheels get a swept
/// contact round the ground (the captures have almost none), some are lifted or sunk so the ray
/// misses or the terrain wins.
#[test]
fn the_atv_wheel_suspension_on_desert_oasis_is_the_originals() {
    use game_sim::atv_ground::Terrain;
    let Some(mut ee) = image() else { return };
    let Some((trn, rows)) = desert_oasis() else { return };
    assert_eq!(ee.u32_at(0x0162_a7e0 + 0x1e0), TERRAIN, "the left front wheel's terrain object");
    let map = game_sim::track::Terrain::parse(&trn).unwrap();
    let terrain = Terrain::of(&map);
    let log: Log = Rc::default();
    let l = log.clone();
    ee.hooks.insert(
        GROUND_QUERY,
        Box::new(move |ee| {
            assert_eq!((ee.r(4) as u32, ee.r(7), ee.r(8) as u32), (TERRAIN, 0, WHEEL + 0x110), "the ground query's arguments");
            l.borrow_mut().push(Call::GroundQuery(get_v(ee, ee.r(5) as u32)));
            let (args, ra, sp, stack, written) = ([ee.r(4), ee.r(5), ee.r(6), ee.r(7), ee.r(8)], ee.r(31), ee.r(29), ee.stack, ee.written.clone());
            ee.stack = (sp as u32 - 0x40) & !0xf;
            ee.call(GROUND_QUERY, &args, &[], 100_000);
            ee.written.extend(written);
            (ee.stack, ee.pc) = (stack, GROUND_QUERY);
            ee.set(29, sp);
            ee.set(31, ra);
        }),
    );
    let word = |b: &[u8], o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let quad = |b: &[u8], o: usize| Vf::new(word(b, o), word(b, o + 4), word(b, o + 8), word(b, o + 12));
    let int = |b: &[u8], o: usize| i32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let mut g = Gen(0x0076_a001);
    let mut t = BTreeMap::new();
    for (k, c) in rows.iter().enumerate() {
        let mut s = g.state();
        s.ground_under = Ground::Still;
        let (w, x, b) = (&mut s.w, &mut s.x, &mut s.b);
        w.has_ground = true;
        w.mount = quad(&c.wheel, 0x90);
        w.probe = quad(&c.wheel, 0x80);
        w.ground_normal = quad(&c.wheel, 0x200);
        w.probe_length = F(word(&c.wheel, 0x214));
        w.swept_contact = int(&c.wheel, 0xd4);
        w.terrain_wins = int(&c.wheel, 0xd8);
        w.contact_point = quad(&c.wheel, 0x120);
        w.contact_normal = quad(&c.wheel, 0x130);
        x.record = c.wheel[0x110];
        // A record left from elsewhere, for the query to write over; a ground normal off the
        // terrain's, for the ray to aim again.
        if g.b() {
            x.record = 0x80 | g.bits() as u8;
        }
        if g.one_in(3) {
            w.ground_normal = g.normal();
        }
        (b.position, b.orientation, b.cofg) = (quad(&c.body, 0x70), quad(&c.body, 0x190), quad(&c.body, 0x1a0));
        match g.bits() % 6 {
            0 => b.position = Vf::new(b.position.x(), b.position.y() + g.f(0.5, 3.0).0, b.position.z(), b.position.w()),
            1 => b.position = Vf::new(b.position.x(), b.position.y() - g.f(0.0, 1.0).0, b.position.z(), b.position.w()),
            _ => {}
        }
        if g.b() {
            w.swept_contact = 1;
            w.terrain_wins = i32::from(g.b());
            let under = point_to_world_of(b, w.mount);
            w.contact_point = Vf::new(under.x(), under.y() - g.f(0.0, 2.5).0, under.z(), 1.0);
            w.contact_normal = if g.one_in(4) { g.normal() } else { w.ground_normal };
        }
        let mut d = s.clone();
        let mut asked = 0;
        atv_wheel::suspension(&mut d.w, &mut d.x, &d.b, &mut |q, r| {
            asked += 1;
            terrain.ground(q, Vf::ZERO, r)
        });
        tally(&mut t, match (d.w.ray_hit, s.w.swept_contact, d.w.terrain_wins) {
            (0, 0, _) => "missed",
            (0, _, _) if d.x.record == 0 && s.x.record != 0 => "missed under a swept contact, record cleared",
            (0, _, w) if w != s.w.terrain_wins => "swept kept, terrain changed",
            (0, _, _) => "swept kept",
            (_, 0, _) => "ray hit",
            _ => "terrain won",
        });
        tally(&mut t, if asked > 1 { "aimed again" } else { "one ray" });
        if d.x.record != s.x.record {
            tally(&mut t, "record changed");
        }

        log.borrow_mut().clear();
        reset(&mut ee);
        poke(&mut ee, &s);
        ee.set_u32(WHEEL + 0x1e0, TERRAIN);
        ee.call(SUSPENSION, &[u64::from(WHEEL)], &[s.dt.0], 1_000_000);
        let frame = (ee.stack - 0x2000, ee.stack + 0x40);
        for (a, z) in ee.written_ranges() {
            let inside = REGIONS.iter().any(|&(_, base, len)| a >= base && z <= base + len)
                || (a >= frame.0 && z <= frame.1)
                || (a >= RING.0 && z <= RING.1)
                || (a >= TERRAIN_RING.0 && z <= TERRAIN_RING.1);
            assert!(inside, "row {k}: the original wrote {a:#010x}..{z:#010x}");
        }
        let was = snap(&ee);
        reset(&mut ee);
        poke(&mut ee, &d);
        ee.set_u32(WHEEL + 0x1e0, TERRAIN);
        let now = snap(&ee);
        if let Some(i) = was.iter().zip(&now).position(|(a, b)| a != b) {
            panic!("row {k}: {} is {:#010x} ({}) in the original and {:#010x} ({}) in the port", where_is(i), was[i], f32::from_bits(was[i]), now[i], f32::from_bits(now[i]));
        }
        let calls: Vec<Call> = (0..asked).map(|_| Call::GroundQuery(Vf::ZERO)).collect();
        assert_eq!(log.borrow().len(), calls.len(), "row {k}: the ground queries");
    }
    each_path_seen(
        &t,
        &["missed", "missed under a swept contact, record cleared", "swept kept", "ray hit", "terrain won", "aimed again", "record changed"],
        30,
    );
}

/// A point of the body's own frame into the world, as the suspension puts the mount there.
fn point_to_world_of(b: &Body, p: Vf) -> Vf {
    let r = game_sim::quad::Body { orientation: b.orientation, ..Default::default() };
    let v = r.to_world(Vf::new((F(p.x()) - F(b.cofg.x())).0, (F(p.y()) - F(b.cofg.y())).0, (F(p.z()) - F(b.cofg.z())).0, 0.0));
    Vf::new((F(v.x()) + F(b.position.x())).0, (F(v.y()) + F(b.position.y())).0, (F(v.z()) + F(b.position.z())).0, 1.0)
}

/// MD75 — the image's own sine and cosine table, which must be the one the port builds.
fn curve(ee: &Ee) -> CurveTable {
    let built = CurveTable::build();
    assert_eq!(ee.f32_at(CURVE_STEP).to_bits(), 0.0010473721f32.to_bits(), "the curve table's step");
    assert_eq!(ee.f32_at(CURVE_INV_STEP).to_bits(), 954.77045f32.to_bits(), "the curve table's inverse step");
    let at = ee.u32_at(CURVE_TABLE);
    for (i, e) in built.entries().iter().enumerate().take(game_sim::camera::CURVE_ENTRIES) {
        let got = [ee.f32_at(at + 8 * i as u32), ee.f32_at(at + 8 * i as u32 + 4)];
        assert_eq!(got.map(f32::to_bits), e.map(f32::to_bits), "curve table entry {i}");
    }
    built
}

/// MD75 — `atv_wheel_surface_drag`, called straight on generated states.
#[test]
fn the_atv_wheel_surface_drag_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let mut g = Gen(0x0075_d001);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        let s = g.spin_state();
        tally(&mut t, match (s.w.contact, s.x.chassis_velocity_local.z() < 0.0) {
            (false, _) => "in the air",
            (true, true) => "rolling back",
            (true, false) => "rolling on",
        });
        tally(&mut t, match s.spin.brake {
            None => "no brake",
            Some(v) if v <= F::ZERO => "brake off",
            Some(_) => "braking",
        });
        compare(&mut ee, k, &s, SURFACE_DRAG, |t| {
            atv_wheel::surface_drag(&t.w, &t.x, &mut t.spin, &mut t.b);
            vec![]
        });
    }
    each_path_seen(&t, &["in the air", "rolling back", "rolling on", "no brake", "brake off", "braking"], 500);
}

/// The paths a spin step takes, read off its state before and after (the port's).
#[allow(clippy::float_arithmetic)]
fn spin_paths(s: &State, d: &State) -> Vec<&'static str> {
    let mut p = Vec::new();
    let (w, sp) = (&s.w, &s.spin);
    let law = w.contact || s.other.contact;
    if !law {
        p.push("both up");
    } else {
        if sp.rear_pair != 0 && !w.contact {
            p.push(if s.share { "borrowed, shared ground" } else { "borrowed, no ground" });
        }
        p.push(if w.contact && w.steered { "frame steered" } else { "frame on the body" });
        let sl = d.spin.slip;
        p.push(match (sl.spinning, sl.locked) {
            (true, _) => "spinning",
            (_, true) => "locked",
            _ => "gripping",
        });
        if d.spin.local_velocity.z().abs() < F(10.0) {
            p.push("slow");
        }
        let side = match (sp.brake != Some(F::ZERO), sl.spinning || sl.locked) {
            (true, _) => "braking",
            (_, true) => "slipping",
            _ => "plain",
        };
        let linear = sp.parametric == 0;
        p.push(match (side, linear) {
            ("braking", true) => "braking linear",
            ("braking", false) => "braking curve",
            ("slipping", true) => "slipping linear",
            ("slipping", false) => "slipping curve",
            (_, true) => "plain linear",
            _ => "plain curve",
        });
        if linear {
            p.push(if sl.sliding { "sliding" } else { "not sliding" });
        }
    }
    p.push(match (w.balancing && w.steered, sp.no_moment) {
        (true, _) => "into the hinge",
        (_, 0) => "with its moment",
        _ => "without a moment",
    });
    let sl = if law { d.spin.slip } else { sp.slip };
    p.push(match (sl.locked, sl.spinning, w.contact) {
        (true, _, _) => "spin locked",
        (_, false, true) => "spin rolling",
        _ => "spin integrated",
    });
    let a = sp.angle.0 + d.spin.spin.0 * s.dt.0;
    if a.abs() > 3.1415927 {
        p.push("angle wrapped");
    }
    p.push(match (w.balancing, w.steered) {
        (false, _) => "no gyro",
        (true, true) => "steered gyro",
        (true, false) => "gyro",
    });
    if d.w.spring_force <= F::ZERO {
        p.push("no load");
    }
    p
}

/// MD75 — `atv_wheel_spin_integrate` with its contact frame, force law and gyro, called straight on
/// generated states; the contact velocity and the hinge run as they are.
#[test]
fn the_atv_wheel_spin_integrate_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let curve = curve(&ee);
    let mut g = Gen(0x0075_d002);
    let mut t = BTreeMap::new();
    let port = |t: &mut State| {
        let other = Other { ground: if t.share { t.ground_under } else { Ground::Still }, ..t.other };
        atv_wheel::spin_integrate(&mut t.w, &mut t.x, &mut t.spin, &mut t.b, t.dt, &other, &mut t.ground_under, &curve)
    };
    for k in 0..20_000 {
        let s = g.spin_state();
        let mut d = s.clone();
        port(&mut d);
        for path in spin_paths(&s, &d) {
            tally(&mut t, path);
        }
        compare(&mut ee, k, &s, SPIN_INTEGRATE, port);
    }
    each_path_seen(
        &t,
        &[
            "both up",
            "borrowed, shared ground",
            "borrowed, no ground",
            "frame steered",
            "frame on the body",
            "spinning",
            "locked",
            "gripping",
            "slow",
            "braking linear",
            "braking curve",
            "slipping linear",
            "slipping curve",
            "plain linear",
            "plain curve",
            "sliding",
            "not sliding",
            "into the hinge",
            "with its moment",
            "without a moment",
            "spin locked",
            "spin rolling",
            "spin integrated",
            "angle wrapped",
            "no gyro",
            "steered gyro",
            "gyro",
            "no load",
        ],
        100,
    );
}
