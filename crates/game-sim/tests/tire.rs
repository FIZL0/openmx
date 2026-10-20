//! P24 — the tire force, proved against the original.
//!
//! The harness lays a wheel out in the interpreter's RAM with one generated case in it — the torque,
//! the load, the patch's velocity, the flags and every one of the sixteen surfaces' tables — runs the
//! original's `tire_patch_force`, and compares **every word** of the wheel against the port run on the
//! same case. The three flags start out random, so a flag the original leaves alone and the port
//! writes (or the other way round) is a difference. `powf` is the game's own, interpreted.

use game_sim::phy::Surface;
use game_sim::tire::{self, Flags, Grip};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, addr, game_or_skip};

const WHEEL: u32 = 0x0130_0000;
const WHEEL_LEN: u32 = 0x900;
/// The brake word the wheel's `+0x30` points at.
const BRAKE: u32 = 0x0131_0000;

/// The per-surface tables, by where the wheel keeps them: each is sixteen floats from this offset.
fn tables(s: &Surface) -> [(u32, F); 21] {
    [
        (0x238, s.side_force),
        (0x278, s.slipping_side_force),
        (0x2b8, s.max_drive_force),
        (0x2f8, s.max_side_force),
        (0x338, s.max_slipping_side_force),
        (0x378, s.skid_drive_force),
        (0x3b8, s.skid_side_force),
        (0x3f8, s.skid_slipping_side_force),
        (0x438, s.slipping_side_force_round_off),
        (0x478, s.slipping_side_force_drop_off),
        (0x4b8, s.slipping_side_force_scale),
        (0x4f8, s.slipping_side_force_speed_scale),
        (0x538, s.braking_side_force_round_off),
        (0x578, s.braking_side_force_drop_off),
        (0x5b8, s.braking_side_force_scale),
        (0x5f8, s.braking_side_force_speed_scale),
        (0x638, s.side_force_round_off),
        (0x678, s.side_force_drop_off),
        (0x6b8, s.side_force_scale),
        (0x6f8, s.side_force_speed_scale),
        (0x738, s.donuts_side_force_speed_scale),
    ]
}

#[derive(Clone, Debug)]
struct Case {
    g: Grip,
    flags: Flags,
    /// The surface the contact is on, and all sixteen rows, so that a wrong index reads a wrong row.
    surface: usize,
    surfaces: Vec<Surface>,
    /// The patch velocity's `y`, which nothing should read.
    lift: F,
}

/// Lay a case out, in the original's offsets. Everything else in the wheel is zero.
fn poke(ee: &mut Ee, c: &Case) {
    for o in (0..WHEEL_LEN).step_by(4) {
        ee.set_u32(WHEEL + o, 0);
    }
    ee.set_u32(WHEEL + 0x30, BRAKE);
    ee.set_f32(BRAKE, c.g.brake.0);
    ee.set_u32(WHEEL + 0x40, c.g.parametric as u32);
    ee.set_f32(WHEEL + 0x44, c.g.donut.0);
    ee.set_u32(WHEEL + 0xe0, c.flags.spinning as u32);
    ee.set_u32(WHEEL + 0xe4, c.flags.locked as u32);
    ee.set_u32(WHEEL + 0xe8, c.flags.sliding as u32);
    ee.set_f32(WHEEL + 0xf8, c.g.load.0);
    // The contact record's surface byte, with junk in the three bytes beside it.
    ee.set_u32(WHEEL + 0x110, 0x5a5a_5a00 | c.surface as u32);
    ee.set_f32(WHEEL + 0x210, c.g.torque.0);
    ee.set_f32(WHEEL + 0x838, c.g.inv_radius.0);
    ee.set_f32(WHEEL + 0x840, c.g.slide.0);
    ee.set_f32(WHEEL + 0x844, c.lift.0);
    ee.set_f32(WHEEL + 0x848, c.g.roll.0);
    for (i, s) in c.surfaces.iter().enumerate() {
        for (at, v) in tables(s) {
            ee.set_f32(WHEEL + at + 4 * i as u32, v.0);
        }
    }
}

fn snapshot(ee: &Ee) -> Vec<u32> {
    (0..WHEEL_LEN).step_by(4).map(|o| ee.u32_at(WHEEL + o)).collect()
}

/// The port over the same layout: the case poked, then the port's outputs written where the original
/// keeps them.
fn port(ee: &mut Ee, c: &Case) -> Vec<u32> {
    poke(ee, c);
    let mut flags = c.flags;
    let f = tire::force(&c.g, &c.surfaces[c.surface], &mut flags);
    ee.set_f32(WHEEL + 0x850, f.side.0);
    ee.set_u32(WHEEL + 0x854, 0);
    ee.set_f32(WHEEL + 0x858, f.drive.0);
    ee.set_u32(WHEEL + 0xe0, flags.spinning as u32);
    ee.set_u32(WHEEL + 0xe4, flags.locked as u32);
    ee.set_u32(WHEEL + 0xe8, flags.sliding as u32);
    snapshot(ee)
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
    /// Zero now and then, so that every equality the original tests is taken both ways.
    fn or_zero(&mut self, lo: f32, hi: f32) -> F {
        if self.one_in(5) { F::ZERO } else { self.f(lo, hi) }
    }
    /// A unit axis: a fork's when `fork` — the chassis' `y` raked back by up to 0.7 rad and tipped a
    /// little sideways — or any direction at all.
    fn unit_axis(&mut self, fork: bool) -> Vf {
        let (x, y, z) = if fork {
            let r = self.f(0.0, 0.7).0;
            (self.f(-0.05, 0.05).0, r.cos(), -r.sin())
        } else {
            (self.f(-1.0, 1.0).0, self.f(-1.0, 1.0).0, self.f(-1.0, 1.0).0)
        };
        let n = (x * x + y * y + z * z).sqrt().max(1e-6);
        Vf::new(x / n, y / n, z / n, 0.0)
    }
    fn surface(&mut self) -> Surface {
        Surface {
            side_force: self.f(0.0, 1.0),
            slipping_side_force: self.f(0.0, 1.0),
            max_drive_force: self.or_zero(-0.5, 4.0),
            max_side_force: self.f(0.0, 8.0),
            max_slipping_side_force: self.f(0.0, 8.0),
            skid_drive_force: self.f(0.0, 4.0),
            skid_side_force: self.f(0.0, 8.0),
            skid_slipping_side_force: self.f(0.0, 8.0),
            slipping_side_force_round_off: self.f(0.0, 10.0),
            slipping_side_force_drop_off: self.f(0.3, 3.0),
            slipping_side_force_scale: self.f(0.0, 100.0),
            slipping_side_force_speed_scale: self.f(0.0, 0.05),
            braking_side_force_round_off: self.f(0.0, 10.0),
            braking_side_force_drop_off: self.f(0.3, 3.0),
            braking_side_force_scale: self.f(0.0, 100.0),
            braking_side_force_speed_scale: self.f(0.0, 0.05),
            side_force_round_off: self.f(0.0, 10.0),
            side_force_drop_off: self.f(0.3, 3.0),
            side_force_scale: self.f(0.0, 100.0),
            side_force_speed_scale: self.f(0.0, 0.05),
            donuts_side_force_speed_scale: self.or_zero(0.0, 0.05),
            mud_time: F::ZERO,
            mud_amount: F::ZERO,
            max_braking_torque: self.or_zero(0.0, 400.0),
            wheel_friction: self.or_zero(0.0, 20.0),
            wheel_drag: self.or_zero(0.0, 0.5),
        }
    }
    fn case(&mut self) -> Case {
        let surfaces: Vec<Surface> = (0..16).map(|_| self.surface()).collect();
        let slide_reach = if self.one_in(3) { 2.0 } else { 80.0 };
        let roll_reach = if self.one_in(2) { 12.0 } else { 90.0 };
        Case {
            g: Grip {
                torque: self.or_zero(-3000.0, 3000.0),
                inv_radius: self.f(0.5, 1.5),
                load: if self.one_in(10) { self.f(-50.0, 0.0) } else { self.or_zero(0.0, 600.0) },
                donut: if self.one_in(2) { F::ZERO } else { self.f(-0.5, 1.5) },
                parametric: self.one_in(2),
                brake: if self.one_in(2) { F::ZERO } else { self.f(-1.0, 1.0) },
                slide: self.or_zero(-slide_reach, slide_reach),
                roll: self.or_zero(-roll_reach, roll_reach),
            },
            flags: Flags { spinning: self.one_in(2), locked: self.one_in(2), sliding: self.one_in(2) },
            surface: (self.bits() % 16) as usize,
            surfaces,
            lift: self.f(-10.0, 10.0),
        }
    }
}

/// Twelve thousand cases, every word of the wheel, and a count of which ways out each took so that
/// "all passed" means every branch was taken.
#[test]
fn the_tire_force_is_the_originals() {
    let Some(mut ee) = game_or_skip() else { return };
    let entry = addr("tire_patch_force").unwrap();
    let mut g = Gen(0x7124_0024);
    // spinning, locked, neither; brake/slipping/plain; linear-within/linear-skid/parametric.
    let mut seen = [[0u32; 3]; 4];
    for k in 0..12_000 {
        let c = g.case();
        poke(&mut ee, &c);
        ee.call(entry, &[WHEEL as u64], &[], 100_000);
        for (a, b) in ee.written_ranges() {
            let inside = (a >= WHEEL && b <= WHEEL + WHEEL_LEN) || (a >= ee.stack - 0x2000 && b <= ee.stack + 0x40);
            assert!(inside, "case {k}: the original wrote {a:#x}..{b:#x}");
        }
        let was = snapshot(&ee);
        let now = port(&mut ee, &c);
        if was != now {
            let i = was.iter().zip(&now).position(|(a, b)| a != b).unwrap();
            panic!("case {k}: wheel+{:#x} is {:#010x} ({}) in the original and {:#010x} ({}) in the port\n{c:#?}",
                   4 * i, was[i], f32::from_bits(was[i]), now[i], f32::from_bits(now[i]));
        }
        let (spin, lock, slid) = (was[0xe0 / 4], was[0xe4 / 4], was[0xe8 / 4]);
        seen[0][if spin != 0 { 0 } else if lock != 0 { 1 } else { 2 }] += 1;
        let family = if c.g.brake != F::ZERO { 0 } else if spin != 0 || lock != 0 { 1 } else { 2 };
        seen[1][family] += 1;
        let shape = if c.g.parametric { 2 } else if slid != 0 { 1 } else { 0 };
        seen[2 + (family == 2) as usize][shape] += 1;
    }
    for row in seen {
        for n in row {
            assert!(n > 100, "a way through the tire force was barely taken: {seen:?}");
        }
    }
}

/// The owner and its transform, for the surface drag: the drag goes through the transform into the
/// owner's force accumulator.
const OWNER: u32 = 0x0132_0000;
const XFORM: u32 = 0x0133_0000;

/// The surface drag over twelve thousand cases: the accumulator it leaves, and the vector it hands the
/// transform (the hook answers it unchanged and records it), against [`tire::surface_drag`].
#[test]
fn the_surface_drag_is_the_originals() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let Some(mut ee) = game_or_skip() else { return };
    let entry = addr("wheel_surface_drag").unwrap();
    let asked: Rc<RefCell<Vec<[u32; 4]>>> = Rc::default();
    let a = asked.clone();
    ee.hooks.insert(
        addr("transform_rotate").unwrap(),
        Box::new(move |ee| {
            let (out, v) = (ee.r(4) as u32, ee.r(6) as u32);
            let w = [0, 4, 8, 12].map(|k| ee.u32_at(v + k));
            a.borrow_mut().push(w);
            for (k, x) in w.into_iter().enumerate() {
                ee.set_u32(out + 4 * k as u32, x);
            }
        }),
    );
    let mut g = Gen(0x5afe_0024);
    let mut seen = [0u32; 4];
    for k in 0..12_000 {
        let c = g.case();
        let contact = g.one_in(3) || !g.one_in(2);
        let forward = if g.one_in(3) { g.or_zero(-4.0, 4.0) } else { g.or_zero(-90.0, 90.0) };
        let spin = if g.one_in(2) { g.or_zero(-2.0, 2.0) } else { g.or_zero(-90.0, 90.0) };
        let wired = !g.one_in(8);
        poke(&mut ee, &c);
        for (i, s) in c.surfaces.iter().enumerate() {
            for (at, v) in [(0x778, s.max_braking_torque), (0x7b8, s.wheel_friction), (0x7f8, s.wheel_drag)] {
                ee.set_f32(WHEEL + at + 4 * i as u32, v.0);
            }
        }
        if !wired {
            ee.set_u32(WHEEL + 0x30, 0);
        }
        ee.set_u32(WHEEL + 0xdc, contact as u32);
        ee.set_f32(WHEEL + 0x198, forward.0);
        ee.set_f32(WHEEL + 0x1d4, spin.0);
        ee.set_u32(WHEEL + 0x1e4, OWNER);
        ee.set_u32(OWNER + 0x254, XFORM);
        asked.borrow_mut().clear();
        ee.call(entry, &[WHEEL as u64], &[0.0], 100_000);
        let s = &c.surfaces[c.surface];
        let d = tire::surface_drag(c.g.torque, contact, forward, spin, wired.then_some(c.g.brake), s);
        let was = ee.u32_at(WHEEL + 0x210);
        assert_eq!(was, d.torque.0.to_bits(),
                   "case {k}: the accumulator is {} in the original and {} in the port\n{c:#?}",
                   f32::from_bits(was), d.torque.0);
        let want: Vec<[u32; 4]> =
            d.force_z.map(|z| [0, 0, z.0.to_bits(), 1.0f32.to_bits()]).into_iter().collect();
        assert_eq!(*asked.borrow(), want, "case {k}: the drag handed to the transform differs");
        seen[contact as usize] += 1;
        seen[2 + (wired && c.g.brake > F::ZERO) as usize] += 1;
    }
    assert!(seen.iter().all(|&n| n > 1000), "{seen:?}");
}

/// `wheel_set_tire`'s inverse spin inertia over ten thousand radii, zero and negative ones among them.
#[test]
fn the_spin_inertia_is_the_originals() {
    let Some(mut ee) = game_or_skip() else { return };
    let entry = addr("wheel_set_tire").unwrap();
    let mut g = Gen(0x1e57_0024);
    for k in 0..10_000 {
        let r = if g.one_in(20) { g.or_zero(-1.0, 0.0) } else { g.f(0.01, 4.0) };
        ee.set_u32(WHEEL + 0x64, 0);
        ee.call(entry, &[WHEEL as u64], &[r.0, 0.7, 6.0, 24.0], 10_000);
        let was = ee.u32_at(WHEEL + 0x64);
        assert_eq!(was, tire::spin_inv_inertia(r).0.to_bits(), "case {k}: radius {}", r.0);
    }
}

/// The game's own `{cos, sin}` table, built by the game in the interpreter and handed to the port, as
/// `tests/camera.rs` does: a rotation proof is bit-exact only on the same table.
fn curve(ee: &mut Ee) -> game_sim::camera::CurveTable {
    const CURVE: u32 = 0x0108_0000;
    ee.hooks.insert(addr("mem_alloc").unwrap(), Box::new(|ee| ee.set(2, CURVE as u64)));
    ee.call(addr("curve_table_build").unwrap(), &[], &[], 2_000_000_000);
    ee.hooks.remove(&addr("mem_alloc").unwrap());
    let entries: Vec<[f32; 2]> =
        (0..6000u32).map(|i| [ee.f32_at(CURVE + 8 * i), ee.f32_at(CURVE + 8 * i + 4)]).collect();
    game_sim::camera::CurveTable::from_entries(&entries)
}

/// The steered wheel's forward, `axis_angle_matrix`'s third row, over twelve thousand angles and
/// axes: fork-like axes near the chassis' `y`, any unit axis, and the angles past a turn that the
/// double wrap is there for.
#[test]
fn the_steered_forward_is_the_originals() {
    let Some(mut ee) = game_or_skip() else { return };
    let table = curve(&mut ee);
    let entry = addr("axis_angle_matrix").unwrap();
    let (axis_at, out) = (WHEEL, WHEEL + 0x100);
    let mut g = Gen(0x1e57_0124);
    for k in 0..12_000 {
        let angle = if g.one_in(4) { g.f(-20.0, 20.0) } else { g.f(-0.6, 0.6) };
        let fork = !g.one_in(3);
        let axis = g.unit_axis(fork);
        for (i, v) in [axis.x(), axis.y(), axis.z(), 0.0].into_iter().enumerate() {
            ee.set_f32(axis_at + 4 * i as u32, v);
        }
        ee.call(entry, &[axis_at as u64, out as u64], &[angle.0], 10_000);
        let theirs: Vec<u32> = (0..3).map(|i| ee.u32_at(out + 0x20 + 4 * i)).collect();
        let ours = tire::steered_forward(angle, axis, &table);
        assert_eq!(theirs, [ours.x().to_bits(), ours.y().to_bits(), ours.z().to_bits()],
                   "case {k}: angle {}, axis {axis:?}", angle.0);
    }
}
