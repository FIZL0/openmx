//! S58 — the wheel's spin step, proved against the original.
//!
//! P33 proved the three functions the spin step calls (tests/tire.rs); this proves the step itself
//! (`wheel_spin_integrate`), the surface drag that runs before it (`wheel_surface_drag`), and the two
//! numbers the spin is integrated through, which `wheel_set_tire` makes out of the radius. The steering
//! hinge's own force application (`wheel_hinge_force`) is proved through the spin step, which calls it
//! for a balanced, steered wheel.
//!
//! As in tests/tire.rs, each case lays a generated wheel out in the interpreter's RAM, runs the
//! original, and compares **every word** of the wheel and its chassis against the generated ones with
//! the port's answers written over them. The chassis transform's three entry points are hooked: each
//! answers with the port's own arithmetic on a generated matrix and records what it was asked, and
//! the order and the bits of those asks are compared too.

use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::chassis::{self, Mat3};
use game_sim::phy::Surface;
use game_sim::ride::Wheel;
use game_sim::tire::{self, Slip, Step};
use game_sim::wheel::{self, Call};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, addr, game_or_skip};
use std::cell::RefCell;
use std::rc::Rc;

const WHEEL: u32 = 0x0130_0000;
const WHEEL_LEN: u32 = 0x920;
const OWNER: u32 = 0x0131_0000;
const OWNER_LEN: u32 = 0x300;
const XFORM: u32 = 0x0132_0000;
const BRAKE: u32 = 0x0133_0000;
const OTHER: u32 = 0x0135_0000;
const CURVE: u32 = 0x0140_0000;
const CASES: usize = 10_000;

/// The per-surface tables, in the order `keys` lists them, at the offsets the wheel keeps them.
const TABLES: [u32; 24] = [0x238, 0x278, 0x2b8, 0x2f8, 0x338, 0x378, 0x3b8, 0x3f8, 0x438, 0x478, 0x4b8,
                           0x4f8, 0x538, 0x578, 0x5b8, 0x5f8, 0x638, 0x678, 0x6b8, 0x6f8, 0x738,
                           0x778, 0x7b8, 0x7f8];

fn keys(s: &Surface) -> [F; 24] {
    [s.side_force, s.slipping_side_force, s.max_drive_force, s.max_side_force,
     s.max_slipping_side_force, s.skid_drive_force, s.skid_side_force, s.skid_slipping_side_force,
     s.slipping_side_force_round_off, s.slipping_side_force_drop_off, s.slipping_side_force_scale,
     s.slipping_side_force_speed_scale, s.braking_side_force_round_off, s.braking_side_force_drop_off,
     s.braking_side_force_scale, s.braking_side_force_speed_scale, s.side_force_round_off,
     s.side_force_drop_off, s.side_force_scale, s.side_force_speed_scale,
     s.donuts_side_force_speed_scale, s.max_braking_torque, s.wheel_friction, s.wheel_drag]
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
fn bits(v: Vf) -> [u32; 4] {
    [v.x(), v.y(), v.z(), v.w()].map(f32::to_bits)
}
fn put_want(want: &mut [u32], off: u32, v: Vf) {
    want[off as usize / 4..off as usize / 4 + 4].copy_from_slice(&bits(v));
}

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

/// The chassis transform as the port answers it, on a generated matrix.
fn transform(m: &[Vf; 4], call: Call) -> Vf {
    let rot = Mat3::new(m[0], m[1], m[2]);
    match call {
        Call::Rotate(v) => chassis::rotate(&rot, v),
        Call::Unrotate(v) => chassis::unrotate(&rot, v),
        Call::Unpoint(v) => chassis::unrotate(&rot, v.merge(ps2_float::Dest::XYZ, v.sub(m[3]))),
        other => panic!("the transform was asked {other:?}"),
    }
}

type Asked = Rc<RefCell<Vec<(u8, [u32; 4])>>>;

fn tag(call: Call) -> (u8, Vf) {
    match call {
        Call::Rotate(v) => (0, v),
        Call::Unrotate(v) => (1, v),
        Call::Unpoint(v) => (2, v),
        other => panic!("not a transform call: {other:?}"),
    }
}

/// Hooks the three transform entry points: each answers with [`transform`] on `m` and records the ask.
fn hook_transform(ee: &mut Ee, m: &Rc<RefCell<[Vf; 4]>>, asked: &Asked) {
    for (name, make) in [("transform_rotate", Call::Rotate as fn(Vf) -> Call),
                         ("transform_unrotate", Call::Unrotate),
                         ("transform_unpoint", Call::Unpoint)] {
        let (m, asked) = (m.clone(), asked.clone());
        ee.hooks.insert(
            addr(name).unwrap(),
            Box::new(move |ee| {
                let (out, v) = (ee.r(4) as u32, ee.r(6) as u32);
                assert_eq!(ee.r(5) as u32, XFORM, "{name} went to something that is not the transform");
                let call = make(get_v(ee, v));
                let (t, v) = tag(call);
                asked.borrow_mut().push((t, bits(v)));
                let ans = transform(&m.borrow(), call);
                put_v(ee, out, ans);
                ee.set(2, out as u64);
            }),
        );
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
    fn one_in(&mut self, n: u32) -> bool {
        self.bits() % n == 0
    }
    fn vec(&mut self, lo: f32, hi: f32) -> Vf {
        Vf::new(self.f(lo, hi).0, self.f(lo, hi).0, self.f(lo, hi).0, self.f(lo, hi).0)
    }
    fn dir(&mut self) -> Vf {
        let v = self.vec(-1.0, 1.0);
        let n = (v.x() * v.x() + v.y() * v.y() + v.z() * v.z()).sqrt().max(1e-6);
        Vf::new(v.x() / n, v.y() / n, v.z() / n, self.f(-1.0, 1.0).0)
    }
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
        (s.max_braking_torque, s.wheel_friction, s.wheel_drag) =
            (self.key(3000.0), self.key(200.0), self.key(2.0));
        s
    }
    fn matrix(&mut self) -> [Vf; 4] {
        [self.dir(), self.dir(), self.dir(), self.vec(-500.0, 500.0)]
    }
}

fn lay_surfaces(ee: &mut Ee, surfaces: &[Surface]) {
    for (i, s) in surfaces.iter().enumerate() {
        for (off, v) in TABLES.iter().zip(keys(s)) {
            ee.set_f32(WHEEL + off + 4 * i as u32, v.0);
        }
    }
}

#[test]
fn the_spin_inertia_matches_wheel_set_tire() {
    let Some((mut ee, _)) = machine() else { return };
    let mut g = Gen(0x6a09_e667_f3bc_c908);
    let entry = addr("wheel_set_tire").unwrap();
    let mut seen = [0usize; 2];
    for k in 0..CASES {
        let radius = match g.bits() % 8 {
            0 => F::ZERO,
            1 => g.f(-2.0, 0.0),
            2 => F(1.2),
            _ => g.f(0.05, 4.0),
        };
        let inertia = g.f(0.0, 3.0);
        clear(&mut ee, WHEEL, WHEEL_LEN);
        let before = words(&ee, WHEEL, WHEEL_LEN);
        ee.call(entry, &[WHEEL as u64], &[radius.0, inertia.0, 6.0, 24.0], 10_000);
        let was = words(&ee, WHEEL, WHEEL_LEN);

        let mut want = before;
        want[0x214 / 4] = radius.to_bits();
        want[0x21c / 4] = 6.0f32.to_bits();
        want[0x218 / 4] = 24.0f32.to_bits();
        want[0x68 / 4] = inertia.to_bits();
        want[0x8a8 / 4] = tire::tire_ply_scale().to_bits();
        want[0x220 / 4] = tire::tire_mass(radius).to_bits();
        if let Some((inv_r, i, inv_i)) = tire::spin_inertia(radius) {
            want[0x838 / 4] = inv_r.to_bits();
            want[0x60 / 4] = i.to_bits();
            want[0x64 / 4] = inv_i.to_bits();
        }
        diff("wheel_set_tire", k, &was, &want, &(radius, inertia));
        seen[tire::spin_inertia(radius).is_some() as usize] += 1;
    }
    println!("no radius / a radius: {seen:?}");
    assert!(seen.iter().all(|&n| n > CASES / 20), "{seen:?}");
}

#[derive(Debug)]
struct DragCase {
    surfaces: Vec<Surface>,
    surface: u8,
    contact: bool,
    local_vel: Vf,
    spin: F,
    torque: F,
    brake: Option<F>,
    force: Vf,
    m: [Vf; 4],
}

#[test]
fn the_surface_drag_matches_the_original() {
    let Some((mut ee, _)) = machine() else { return };
    let m = Rc::new(RefCell::new([Vf::ZERO; 4]));
    let asked: Asked = Rc::default();
    hook_transform(&mut ee, &m, &asked);
    let mut g = Gen(0xbb67_ae85_84ca_a73b);
    let entry = addr("wheel_surface_drag").unwrap();
    // ground / air; brake on / off its stop / no lever; the fade held at either end.
    let mut seen = [0usize; 6];
    for k in 0..CASES {
        let surfaces: Vec<Surface> = (0..4).map(|_| g.surface()).collect();
        let vz = if g.one_in(2) { g.f(-5.0, 5.0) } else { g.f(-80.0, 80.0) };
        let case = DragCase {
            surface: (g.bits() % 4) as u8,
            surfaces,
            contact: !g.one_in(3),
            local_vel: Vf::new(g.f(-30.0, 30.0).0, g.f(-5.0, 5.0).0, vz.0, g.f(-1.0, 1.0).0),
            spin: if g.one_in(2) { g.f(-2.0, 2.0) } else { g.f(-60.0, 60.0) },
            torque: g.f(-3000.0, 3000.0),
            brake: match g.bits() % 4 {
                0 => None,
                1 => Some(g.f(-1.0, 0.0)),
                _ => Some(g.f(0.0, 1.0)),
            },
            force: g.vec(-2000.0, 2000.0),
            m: g.matrix(),
        };
        clear(&mut ee, WHEEL, WHEEL_LEN);
        clear(&mut ee, OWNER, OWNER_LEN);
        lay_surfaces(&mut ee, &case.surfaces);
        ee.set_u32(WHEEL + 0x1e4, OWNER);
        ee.set_u32(OWNER + 0x254, XFORM);
        ee.set_u32(WHEEL + 0xdc, case.contact as u32);
        ee.set_u32(WHEEL + 0x110, case.surface as u32);
        put_v(&mut ee, WHEEL + 0x190, case.local_vel);
        ee.set_f32(WHEEL + 0x1d4, case.spin.0);
        ee.set_f32(WHEEL + 0x210, case.torque.0);
        if let Some(b) = case.brake {
            ee.set_u32(WHEEL + 0x30, BRAKE);
            ee.set_f32(BRAKE, b.0);
        }
        put_v(&mut ee, OWNER + 0x130, case.force);
        *m.borrow_mut() = case.m;
        asked.borrow_mut().clear();
        let (before_w, before_o) = (words(&ee, WHEEL, WHEEL_LEN), words(&ee, OWNER, OWNER_LEN));
        ee.call(entry, &[WHEEL as u64], &[0.0166], 20_000);
        let (was_w, was_o) = (words(&ee, WHEEL, WHEEL_LEN), words(&ee, OWNER, OWNER_LEN));

        let mut wh = Wheel::default();
        wh.w.contact = case.contact;
        wh.chassis_local_velocity = case.local_vel;
        wh.mv.spin = case.spin;
        wh.mv.drive_torque = case.torque;
        let mut c = wheel::Chassis { force: case.force, ..wheel::Chassis::default() };
        let mut port_asked = Vec::new();
        tire::surface_drag(&mut wh, &case.surfaces[case.surface as usize], case.brake.unwrap_or(F::ZERO),
                           &mut c, &mut |v| {
                               port_asked.push((0u8, bits(v)));
                               transform(&case.m, Call::Rotate(v))
                           });
        assert_eq!(*asked.borrow(), port_asked, "case {k}: what was asked of the transform");
        let mut want_w = before_w;
        want_w[0x210 / 4] = wh.mv.drive_torque.to_bits();
        let mut want_o = before_o;
        put_want(&mut want_o, 0x130, c.force);
        diff("surface drag (wheel)", k, &was_w, &want_w, &case);
        diff("surface drag (chassis)", k, &was_o, &want_o, &case);

        seen[if case.contact { 0 } else { 1 }] += 1;
        match case.brake {
            Some(b) if b > F::ZERO => seen[2] += 1,
            Some(_) => seen[3] += 1,
            None => seen[4] += 1,
        }
        let lead = if case.contact { F(0.3) * F(vz.0) } else { case.spin };
        seen[5] += (lead.abs() > F::ONE) as usize;
    }
    println!("ground/air, brake on/stopped/none, fade held: {seen:?}");
    assert!(seen.iter().all(|&n| n > CASES / 20), "a branch went untested: {seen:?}");
}

#[derive(Debug)]
struct SpinCase {
    surfaces: Vec<Surface>,
    surface: u8,
    contact: bool,
    other_contact: bool,
    steered: bool,
    balancing: bool,
    parametric: bool,
    slip: Slip,
    load: F,
    donut: F,
    brake: F,
    dt: F,
    spin: F,
    angle: F,
    torque: F,
    radius: F,
    inv_radius: F,
    inv_inertia: F,
    gyro: F,
    steer_angle: F,
    steer_rate: F,
    steer_torque: F,
    axis: Vf,
    normal: Vf,
    contact_vel: Vf,
    load_point: Vf,
    mount_world: Vf,
    arm: Vf,
    old_force: Vf,
    old_frame: [Vf; 4],
    forward: Vf,
    ang_vel: Vf,
    cofg: Vf,
    force: Vf,
    torque_acc: Vf,
    m: [Vf; 4],
}

#[test]
fn the_spin_step_matches_the_original() {
    let Some((mut ee, curve)) = machine() else { return };
    let m = Rc::new(RefCell::new([Vf::ZERO; 4]));
    let asked: Asked = Rc::default();
    hook_transform(&mut ee, &m, &asked);
    let mut g = Gen(0x3c6e_f372_fe94_f82b);
    let entry = addr("wheel_spin_integrate").unwrap();
    // locked / rolling / spun up; both in the air / only the other down; through the hinge (and the
    // steered gyro);
    // gyro unsteered; wrapped.
    let mut seen = [0usize; 8];
    for k in 0..CASES {
        let surfaces: Vec<Surface> = (0..4).map(|_| g.surface()).collect();
        let along = if g.one_in(2) { g.f(-12.0, 12.0) } else { g.f(-60.0, 60.0) };
        let contact = !g.one_in(4);
        let case = SpinCase {
            surface: (g.bits() % 4) as u8,
            surfaces,
            contact,
            other_contact: g.one_in(2),
            steered: g.one_in(2),
            balancing: g.one_in(3),
            parametric: g.one_in(2),
            slip: Slip { spinning: g.one_in(3), locked: g.one_in(4), sliding: g.one_in(2) },
            load: if g.one_in(13) { F::ZERO } else if contact { g.f(0.0, 1500.0) } else { g.f(-10.0, 10.0) },
            donut: if g.one_in(2) { F::ZERO } else { g.f(-0.5, 1.5) },
            brake: if g.one_in(3) { g.f(0.0, 1.0) } else { F::ZERO },
            dt: g.f(0.002, 0.02),
            spin: g.f(-60.0, 60.0),
            angle: if g.one_in(5) { g.f(-40.0, 40.0) } else { g.f(-3.3, 3.3) },
            torque: g.f(-3000.0, 3000.0),
            radius: g.f(0.5, 1.5),
            inv_radius: g.f(0.5, 1.5),
            inv_inertia: g.f(0.1, 2.0),
            gyro: g.f(0.0, 2.0),
            steer_angle: g.f(-0.8, 0.8),
            steer_rate: g.f(-10.0, 10.0),
            steer_torque: g.f(-100.0, 100.0),
            axis: g.dir(),
            normal: g.dir(),
            contact_vel: Vf::new(g.f(-30.0, 30.0).0, g.f(-5.0, 5.0).0, along.0, g.f(-1.0, 1.0).0),
            load_point: g.vec(-500.0, 500.0),
            mount_world: g.vec(-500.0, 500.0),
            arm: g.vec(-3.0, 3.0),
            old_force: g.vec(-500.0, 500.0),
            old_frame: [g.dir(), g.dir(), g.dir(), g.vec(-1.0, 1.0)],
            forward: g.dir(),
            ang_vel: g.vec(-5.0, 5.0),
            cofg: g.vec(-2.0, 2.0),
            force: g.vec(-2000.0, 2000.0),
            torque_acc: g.vec(-2000.0, 2000.0),
            m: g.matrix(),
        };
        clear(&mut ee, WHEEL, WHEEL_LEN);
        clear(&mut ee, OWNER, OWNER_LEN);
        clear(&mut ee, XFORM, 0xc0);
        clear(&mut ee, OTHER, 0x100);
        lay_surfaces(&mut ee, &case.surfaces);
        ee.set_u32(WHEEL + 0x1e4, OWNER);
        ee.set_u32(OWNER + 0x254, XFORM);
        put_v(&mut ee, XFORM + 0xa0, case.forward);
        ee.set_u32(WHEEL + 0xcc, OTHER);
        ee.set_u32(OTHER + 0xdc, case.other_contact as u32);
        ee.set_u32(WHEEL + 0x30, BRAKE);
        ee.set_f32(BRAKE, case.brake.0);
        ee.set_u32(WHEEL + 0x38, case.steered as u32);
        ee.set_u32(WHEEL + 0x3c, case.balancing as u32);
        ee.set_u32(WHEEL + 0x40, case.parametric as u32);
        ee.set_f32(WHEEL + 0x44, case.donut.0);
        ee.set_f32(WHEEL + 0x64, case.inv_inertia.0);
        ee.set_f32(WHEEL + 0x68, case.gyro.0);
        ee.set_f32(WHEEL + 0x6c, case.steer_torque.0);
        put_v(&mut ee, WHEEL + 0xb0, case.arm);
        ee.set_u32(WHEEL + 0xdc, case.contact as u32);
        ee.set_u32(WHEEL + 0xe0, case.slip.spinning as u32);
        ee.set_u32(WHEEL + 0xe4, case.slip.locked as u32);
        ee.set_u32(WHEEL + 0xe8, case.slip.sliding as u32);
        ee.set_f32(WHEEL + 0xf8, case.load.0);
        ee.set_u32(WHEEL + 0x110, case.surface as u32);
        put_v(&mut ee, WHEEL + 0x130, case.normal);
        put_v(&mut ee, WHEEL + 0x140, case.load_point);
        put_v(&mut ee, WHEEL + 0x150, case.mount_world);
        put_v(&mut ee, WHEEL + 0x170, case.contact_vel);
        ee.set_f32(WHEEL + 0x1d0, case.angle.0);
        ee.set_f32(WHEEL + 0x1d4, case.spin.0);
        ee.set_f32(WHEEL + 0x210, case.torque.0);
        ee.set_f32(WHEEL + 0x214, case.radius.0);
        // What the both-in-the-air case zeroes, so its write shows.
        ee.set_u32(WHEEL + 0x224, 0x1234_5678);
        ee.set_u32(WHEEL + 0x228, 0x1234_5678);
        ee.set_f32(WHEEL + 0x838, case.inv_radius.0);
        put_v(&mut ee, WHEEL + 0x850, case.old_force);
        ee.set_f32(WHEEL + 0x8ac, case.steer_angle.0);
        ee.set_f32(WHEEL + 0x8b0, case.steer_rate.0);
        put_v(&mut ee, WHEEL + 0x8c0, case.axis);
        for (r, row) in case.old_frame.iter().enumerate() {
            put_v(&mut ee, WHEEL + 0x8d0 + 16 * r as u32, *row);
        }
        put_v(&mut ee, OWNER + 0x130, case.force);
        put_v(&mut ee, OWNER + 0x140, case.torque_acc);
        put_v(&mut ee, OWNER + 0x190, case.cofg);
        put_v(&mut ee, OWNER + 0x290, case.ang_vel);
        *m.borrow_mut() = case.m;
        asked.borrow_mut().clear();
        let (before_w, before_o) = (words(&ee, WHEEL, WHEEL_LEN), words(&ee, OWNER, OWNER_LEN));
        ee.call(entry, &[WHEEL as u64], &[case.dt.0], 400_000);
        let (was_w, was_o) = (words(&ee, WHEEL, WHEEL_LEN), words(&ee, OWNER, OWNER_LEN));

        let mut wh = Wheel::default();
        wh.w.contact = case.contact;
        wh.w.steered = case.steered;
        wh.w.balancing = case.balancing;
        wh.w.spring_force = case.load;
        wh.w.contact_normal = case.normal;
        wh.w.load_point = case.load_point;
        wh.w.mount_world = case.mount_world;
        wh.w.contact_velocity = case.contact_vel;
        wh.w.steer_angle = case.steer_angle;
        wh.w.steer_rate = case.steer_rate;
        wh.w.steer_torque = case.steer_torque;
        wh.mv.donut_scale = case.donut;
        wh.mv.spin = case.spin;
        wh.mv.drive_torque = case.torque;
        wh.mv.radius = case.radius;
        wh.mv.inv_radius = case.inv_radius;
        wh.inv_inertia = case.inv_inertia;
        wh.gyro = case.gyro;
        wh.angle = case.angle;
        wh.steer_axis = case.axis;
        wh.slip = case.slip;
        wh.local_force = case.old_force;
        wh.frame = case.old_frame;
        let mut c = wheel::Chassis { force: case.force, torque: case.torque_acc, cofg: case.cofg,
                                     ..wheel::Chassis::default() };
        let st = Step { dt: case.dt, brake: case.brake, parametric: case.parametric,
                        other_contact: case.other_contact, arm: case.arm, forward: case.forward,
                        ang_vel: case.ang_vel };
        let mut port_asked = Vec::new();
        tire::spin_integrate(&mut wh, &case.surfaces[case.surface as usize], &st, &mut c, &curve,
                             &mut |call| {
                                 let (t, v) = tag(call);
                                 port_asked.push((t, bits(v)));
                                 transform(&case.m, call)
                             });
        assert_eq!(*asked.borrow(), port_asked, "case {k}: what was asked of the transform");

        let mut want_w = before_w;
        want_w[0x6c / 4] = wh.w.steer_torque.to_bits();
        want_w[0xe0 / 4] = wh.slip.spinning as u32;
        want_w[0xe4 / 4] = wh.slip.locked as u32;
        want_w[0xe8 / 4] = wh.slip.sliding as u32;
        want_w[0x1d0 / 4] = wh.angle.to_bits();
        want_w[0x1d4 / 4] = wh.mv.spin.to_bits();
        want_w[0x210 / 4] = wh.mv.drive_torque.to_bits();
        want_w[0x234 / 4] = wh.side_per_load.to_bits();
        if !case.contact && !case.other_contact {
            want_w[0x224 / 4] = 0;
            want_w[0x228 / 4] = 0;
        }
        put_want(&mut want_w, 0x840, wh.local_velocity);
        put_want(&mut want_w, 0x850, wh.local_force);
        for (r, row) in wh.frame.iter().enumerate() {
            put_want(&mut want_w, 0x8d0 + 16 * r as u32, *row);
        }
        let mut want_o = before_o;
        put_want(&mut want_o, 0x130, c.force);
        put_want(&mut want_o, 0x140, c.torque);
        diff("spin step (wheel)", k, &was_w, &want_w, &case);
        diff("spin step (chassis)", k, &was_o, &want_o, &case);
        assert_eq!(wh.mv.spin_from_torque, wh.slip.spinning);
        assert_eq!(wh.mv.contact_force_fwd.to_bits(), wh.local_force.z().to_bits());

        let ran = case.contact || case.other_contact;
        seen[if wh.slip.locked { 0 } else if !wh.slip.spinning && case.contact { 1 } else { 2 }] += 1;
        seen[if ran { 4 } else { 3 }] += (!case.contact) as usize;
        seen[5] += (case.balancing && case.steered) as usize;
        seen[6] += (case.balancing && !case.steered) as usize;
        seen[7] += ((case.angle + wh.mv.spin * case.dt).abs() > F(std::f32::consts::PI)) as usize;
    }
    println!("locked/rolling/spun, air both/other down, hinge, gyro unsteered, wrapped: {seen:?}");
    assert!(seen.iter().all(|&n| n > CASES / 50), "a branch went untested: {seen:?}");
}
