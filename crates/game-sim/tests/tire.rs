//! P33 — the tire's grip, proved against the original.
//!
//! Three functions the wheel's spin step calls before it turns the wheel: the force law, the contact
//! frame and the axis-angle rotation the frame steers by. Each case lays a generated wheel out in the
//! interpreter's RAM, runs the original, and compares **every word** of the wheel against the
//! generated one with the port's answers written over it: a word the original writes and the port
//! does not predict is a difference.
//!
//! The force law calls `powf` through the executable's one thunk, which is not hooked: the original's
//! own fdlibm runs, against [`ps2_float::libm::powf`]. The contact frame asks the chassis transform to
//! turn one direction out into the world; that call is hooked, answers with a generated vector and
//! records what it was asked, which is the port's arithmetic and is compared too.

use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::phy::Surface;
use game_sim::tire::{self, Contact, Slip};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, addr, game_or_skip};
use std::cell::RefCell;
use std::rc::Rc;

const WHEEL: u32 = 0x0130_0000;
const WHEEL_LEN: u32 = 0x920;
const OWNER: u32 = 0x0131_0000;
const XFORM: u32 = 0x0132_0000;
const BRAKE: u32 = 0x0133_0000;
const OUT: u32 = 0x0134_0000;
const CURVE: u32 = 0x0140_0000;
const CASES: usize = 10_000;

/// The per-surface tables, in the order `phy::Surface` keeps them, at the offsets the wheel does.
const TABLES: [u32; 21] = [0x238, 0x278, 0x2b8, 0x2f8, 0x338, 0x378, 0x3b8, 0x3f8, 0x438, 0x478, 0x4b8,
                           0x4f8, 0x538, 0x578, 0x5b8, 0x5f8, 0x638, 0x678, 0x6b8, 0x6f8, 0x738];

fn keys(s: &Surface) -> [F; 21] {
    [s.side_force, s.slipping_side_force, s.max_drive_force, s.max_side_force,
     s.max_slipping_side_force, s.skid_drive_force, s.skid_side_force, s.skid_slipping_side_force,
     s.slipping_side_force_round_off, s.slipping_side_force_drop_off, s.slipping_side_force_scale,
     s.slipping_side_force_speed_scale, s.braking_side_force_round_off, s.braking_side_force_drop_off,
     s.braking_side_force_scale, s.braking_side_force_speed_scale, s.side_force_round_off,
     s.side_force_drop_off, s.side_force_scale, s.side_force_speed_scale,
     s.donuts_side_force_speed_scale]
}

fn put_v(ee: &mut Ee, a: u32, v: Vf) {
    for (k, x) in [v.x(), v.y(), v.z(), v.w()].into_iter().enumerate() {
        ee.set_f32(a + 4 * k as u32, x);
    }
}
fn get_v(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}
fn words(ee: &Ee, base: u32, len: u32) -> Vec<u32> {
    (0..len).step_by(4).map(|o| ee.u32_at(base + o)).collect()
}
fn clear(ee: &mut Ee, base: u32, len: u32) {
    for o in (0..len).step_by(4) {
        ee.set_u32(base + o, 0);
    }
}

/// The interpreter, with the game's own curve table where the harness wants it.
fn machine() -> Option<(Ee, CurveTable)> {
    let mut ee = game_or_skip()?;
    ee.hooks.insert(addr("mem_alloc").unwrap(), Box::new(|ee| ee.set(2, CURVE as u64)));
    ee.call(addr("curve_table_build").unwrap(), &[], &[], 2_000_000_000);
    ee.hooks.remove(&addr("mem_alloc").unwrap());
    assert_eq!(ee.u32_at(addr("curve_table_ptr").unwrap()), CURVE, "the table went somewhere else");
    let entries: Vec<[f32; 2]> =
        (0..CURVE_ENTRIES as u32).map(|i| [ee.f32_at(CURVE + 8 * i), ee.f32_at(CURVE + 8 * i + 4)]).collect();
    Some((ee, CurveTable::from_entries(&entries)))
}

fn diff(name: &str, k: usize, was: &[u32], want: &[u32], case: &dyn std::fmt::Debug) {
    if was == want {
        return;
    }
    let bad: Vec<String> = was.iter().zip(want).enumerate().filter(|(_, (a, b))| a != b)
        .map(|(i, (a, b))| format!("+{:#x}: original {a:#010x} ({}) port {b:#010x} ({})", 4 * i,
                                   f32::from_bits(*a), f32::from_bits(*b)))
        .collect();
    panic!("{name} case {k}: {}\n{case:#?}", bad.join("\n  "));
}

/// A generator; plain host arithmetic, these are only inputs.
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
    fn one_in(&mut self, n: u32) -> bool {
        self.bits() % n == 0
    }
    fn vec(&mut self, lo: f32, hi: f32) -> Vf {
        Vf::new(self.f(lo, hi).0, self.f(lo, hi).0, self.f(lo, hi).0, self.f(lo, hi).0)
    }
    fn dir(&mut self) -> Vf {
        if self.one_in(53) {
            return Vf::new(0.0, 0.0, 0.0, self.f(-1.0, 1.0).0);
        }
        let v = self.vec(-1.0, 1.0);
        let n = (v.x() * v.x() + v.y() * v.y() + v.z() * v.z()).sqrt().max(1e-6);
        Vf::new(v.x() / n, v.y() / n, v.z() / n, self.f(-1.0, 1.0).0)
    }
    /// A key: usually the size the disc's tires have, sometimes exactly zero.
    fn key(&mut self, hi: f32) -> F {
        if self.one_in(17) { F::ZERO } else { self.f(0.0, hi) }
    }
    fn surface(&mut self) -> Surface {
        let mut s = Surface::default();
        (s.side_force, s.slipping_side_force) = (self.key(2.0), self.key(2.0));
        (s.max_drive_force, s.max_side_force, s.max_slipping_side_force) =
            (self.key(3.0), self.key(5.0), self.key(5.0));
        (s.skid_drive_force, s.skid_side_force, s.skid_slipping_side_force) =
            (self.key(2.0), self.key(4.0), self.key(4.0));
        for (r, d, c, p) in [
            (&mut s.slipping_side_force_round_off, &mut s.slipping_side_force_drop_off,
             &mut s.slipping_side_force_scale, &mut s.slipping_side_force_speed_scale),
            (&mut s.braking_side_force_round_off, &mut s.braking_side_force_drop_off,
             &mut s.braking_side_force_scale, &mut s.braking_side_force_speed_scale),
            (&mut s.side_force_round_off, &mut s.side_force_drop_off, &mut s.side_force_scale,
             &mut s.side_force_speed_scale),
        ] {
            *r = if self.one_in(17) { F::ZERO } else { self.f(0.0, 2.0) };
            *d = self.f(0.5, 4.0);
            *c = self.f(0.0, 4.0);
            *p = self.f(0.0, 3.0);
        }
        s.donuts_side_force_speed_scale = self.f(0.0, 3.0);
        s
    }
}

#[derive(Debug)]
struct ForceCase {
    surfaces: Vec<Surface>,
    surface: u8,
    c: Contact,
    slip: Slip,
}

#[test]
fn the_tire_force_law_matches_the_original() {
    let Some((mut ee, _)) = machine() else { return };
    let mut g = Gen(0x9e37_79b9_7f4a_7c15);
    let entry = addr("wheel_tire_drive_force").unwrap();
    // spinning, locked, neither; brake, slipping, side; parametric or linear; slow; sliding written.
    let mut seen = [0usize; 10];
    for k in 0..CASES {
        let surfaces: Vec<Surface> = (0..4).map(|_| g.surface()).collect();
        let surface = (g.bits() % 4) as u8;
        let load = if g.one_in(23) { F::ZERO } else { g.f(0.0, 1500.0) };
        let drive_torque = g.f(-3000.0, 3000.0);
        let donut = if g.one_in(2) { F::ZERO } else { g.f(-0.5, 1.5) };
        let brake = if g.one_in(3) { g.f(0.0, 1.0) } else { F::ZERO };
        let along = if g.one_in(2) { g.f(-12.0, 12.0) } else { g.f(-60.0, 60.0) };
        let velocity = Vf::new(g.f(-30.0, 30.0).0, g.f(-5.0, 5.0).0, along.0, g.f(-1.0, 1.0).0);
        let c = Contact { load, drive_torque, inv_radius: g.f(0.5, 1.5), donut, brake,
                          parametric: g.one_in(2), velocity };
        let slip = Slip { spinning: g.one_in(2), locked: g.one_in(2), sliding: g.one_in(2) };
        let case = ForceCase { surfaces, surface, c, slip };

        clear(&mut ee, WHEEL, WHEEL_LEN);
        for (i, s) in case.surfaces.iter().enumerate() {
            for (off, v) in TABLES.iter().zip(keys(s)) {
                ee.set_f32(WHEEL + off + 4 * i as u32, v.0);
            }
        }
        ee.set_u32(WHEEL + 0x30, BRAKE);
        ee.set_f32(BRAKE, c.brake.0);
        ee.set_u32(WHEEL + 0x40, c.parametric as u32);
        ee.set_f32(WHEEL + 0x44, c.donut.0);
        ee.set_u32(WHEEL + 0xe0, slip.spinning as u32);
        ee.set_u32(WHEEL + 0xe4, slip.locked as u32);
        ee.set_u32(WHEEL + 0xe8, slip.sliding as u32);
        ee.set_f32(WHEEL + 0xf8, c.load.0);
        ee.set_u32(WHEEL + 0x110, surface as u32);
        ee.set_f32(WHEEL + 0x210, c.drive_torque.0);
        ee.set_f32(WHEEL + 0x838, c.inv_radius.0);
        put_v(&mut ee, WHEEL + 0x840, c.velocity);
        // Something the law must leave alone, so a stray write shows.
        ee.set_u32(WHEEL + 0x854, 0xdead_beef);
        ee.set_u32(WHEEL + 0x85c, 0x1234_5678);
        let before = words(&ee, WHEEL, WHEEL_LEN);
        ee.call(entry, &[WHEEL as u64], &[], 200_000);
        let was = words(&ee, WHEEL, WHEEL_LEN);

        let mut port = slip;
        let (side, fwd) = tire::force(&case.surfaces[surface as usize], &c, &mut port);
        let mut want = before;
        want[0xe0 / 4] = port.spinning as u32;
        want[0xe4 / 4] = port.locked as u32;
        want[0xe8 / 4] = port.sliding as u32;
        want[0x850 / 4] = side.to_bits();
        want[0x854 / 4] = 0;
        want[0x858 / 4] = fwd.to_bits();
        diff("tire force", k, &was, &want, &case);

        let curve = c.parametric;
        let set = if c.brake != F::ZERO { 3 } else if port.spinning || port.locked { 4 } else { 5 };
        seen[if port.spinning { 0 } else if port.locked { 1 } else { 2 }] += 1;
        seen[set] += 1;
        seen[if curve { 6 } else { 7 }] += 1;
        seen[8] += (F(velocity.z()).abs() < F(10.0)) as usize;
        seen[9] += (!curve && port.sliding) as usize;
    }
    println!("spinning/locked/neither, brake/slipping/side, curve/linear, slow, slid: {seen:?}");
    assert!(seen.iter().all(|&n| n > CASES / 50), "a branch went untested: {seen:?}");
}

#[test]
fn the_axis_rotation_matches_the_original() {
    let Some((mut ee, curve)) = machine() else { return };
    let mut g = Gen(0x2545_f491_4f6c_dd1d);
    let entry = addr("axis_rotation_matrix").unwrap();
    for k in 0..CASES {
        let angle = if g.one_in(5) { g.f(-40.0, 40.0) } else { g.f(-1.2, 1.2) };
        let axis = g.dir();
        put_v(&mut ee, OUT + 0x100, axis);
        for o in (0..0x40).step_by(4) {
            ee.set_u32(OUT + o, 0x7f80_0001);
        }
        ee.call(entry, &[(OUT + 0x100) as u64, OUT as u64], &[angle.0], 10_000);
        let was = words(&ee, OUT, 0x40);
        let m = tire::axis_rotation(angle, axis, &curve);
        let mut want = vec![0x7f80_0001u32; 16];
        for (r, row) in m.iter().enumerate() {
            for (l, x) in [row.x(), row.y(), row.z()].into_iter().enumerate() {
                want[4 * r + l] = x.to_bits();
            }
        }
        diff("axis rotation", k, &was, &want, &(angle, axis));
    }
}

#[derive(Debug)]
struct FrameCase {
    row2: Vf,
    steered: bool,
    contact: bool,
    steer: F,
    axis: Vf,
    normal: Vf,
    velocity: Vf,
    answer: Vf,
}

#[test]
fn the_contact_frame_matches_the_original() {
    let Some((mut ee, curve)) = machine() else { return };
    let asked: Rc<RefCell<Vec<Vf>>> = Rc::default();
    let answer: Rc<RefCell<Vf>> = Rc::new(RefCell::new(Vf::ZERO));
    let (a, b) = (asked.clone(), answer.clone());
    ee.hooks.insert(
        addr("transform_rotate").unwrap(),
        Box::new(move |ee| {
            let (out, v) = (ee.r(4) as u32, ee.r(6) as u32);
            assert_eq!(ee.r(5) as u32, XFORM, "the rotate went to something that is not the transform");
            a.borrow_mut().push(get_v(ee, v));
            let ans = *b.borrow();
            put_v(ee, out, ans);
            ee.set(2, out as u64);
        }),
    );
    let mut g = Gen(0x5851_f42d_4c95_7f2d);
    let entry = addr("wheel_contact_frame").unwrap();
    let mut seen = [0usize; 2];
    for k in 0..CASES {
        let case = FrameCase {
            row2: g.dir(),
            steered: g.one_in(2),
            contact: !g.one_in(4),
            steer: if g.one_in(5) { g.f(-20.0, 20.0) } else { g.f(-0.8, 0.8) },
            axis: g.dir(),
            normal: g.dir(),
            velocity: g.vec(-60.0, 60.0),
            answer: g.dir(),
        };
        clear(&mut ee, WHEEL, WHEEL_LEN);
        clear(&mut ee, XFORM, 0xc0);
        ee.set_u32(WHEEL + 0x1e4, OWNER);
        ee.set_u32(OWNER + 0x254, XFORM);
        put_v(&mut ee, XFORM + 0xa0, case.row2);
        ee.set_u32(WHEEL + 0xdc, case.contact as u32);
        ee.set_u32(WHEEL + 0x38, case.steered as u32);
        ee.set_f32(WHEEL + 0x8ac, case.steer.0);
        put_v(&mut ee, WHEEL + 0x8c0, case.axis);
        put_v(&mut ee, WHEEL + 0x130, case.normal);
        put_v(&mut ee, WHEEL + 0x170, case.velocity);
        *answer.borrow_mut() = case.answer;
        asked.borrow_mut().clear();
        let before = words(&ee, WHEEL, WHEEL_LEN);
        ee.call(entry, &[WHEEL as u64], &[], 20_000);
        let was = words(&ee, WHEEL, WHEEL_LEN);

        let mut port_asked = Vec::new();
        let (frame, local) = tire::contact_frame(case.row2, case.steered, case.contact, case.steer,
                                                 case.axis, case.normal, case.velocity, &curve,
                                                 &mut |v| {
                                                     port_asked.push(v);
                                                     case.answer
                                                 });
        let bits = |v: Vf| [v.x(), v.y(), v.z(), v.w()].map(f32::to_bits);
        assert_eq!(asked.borrow().iter().map(|&v| bits(v)).collect::<Vec<_>>(),
                   port_asked.iter().map(|&v| bits(v)).collect::<Vec<_>>(), "case {k}: what was rotated");
        let mut want = before;
        for (r, row) in frame.0.iter().enumerate() {
            want[(0x8d0 / 4 + 4 * r)..(0x8d0 / 4 + 4 * r + 4)].copy_from_slice(&bits(*row));
        }
        want[0x840 / 4..0x850 / 4].copy_from_slice(&bits(local));
        diff("contact frame", k, &was, &want, &case);
        seen[(case.contact && case.steered) as usize] += 1;
    }
    println!("chassis forward / steered: {seen:?}");
}
