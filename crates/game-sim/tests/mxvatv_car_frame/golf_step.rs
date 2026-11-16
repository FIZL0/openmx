// --- MD156a: the golf cart's class step ------------------------------------------------------------

use super::class_step::{Over, curve_table};
use super::*;
use game_sim::atv_car::{GolfArm, GolfCorner, GolfParts, GolfPosing};
use std::collections::BTreeMap;

#[path = "../mxvatv_car_replay/drive.rs"]
#[allow(dead_code)]
mod drive;

/// The golf cart's class step (vt `+0x488`), and the steering frame's origin (rodata).
pub(crate) const GOLF_CLASS_STEP: u32 = 0x004c_f9b0;
const ORIGIN: u32 = 0x0039_6fc0;

/// Where the golf cart keeps its parts and the values they are posed by (research/oracle.txt
/// `# MD156a`).
mod gs {
    pub const STEERING: u32 = 0x1350;
    pub const WHEELS: [u32; 2] = [0x12d4, 0x12d8];
    /// Each front corner (in the tires' order): the three arms, the knuckle, the two ends; its
    /// block of arms' aheads and ups (each arm's ahead and up at the offsets in `ARM_AT`) and the
    /// knuckle's ahead at `+0x40` in it.
    pub const CORNERS: [([u32; 3], u32, [u32; 2], u32); 2] =
        [([0x1304, 0x1310, 0x1308], 0x12e4, [0x12e8, 0x130c], 0x1590), ([0x131c, 0x1328, 0x1320], 0x12f0, [0x12f4, 0x1324], 0x14f0)];
    pub const ARM_AT: [(u32, u32); 3] = [(0x00, 0x20), (0x60, 0x70), (0x10, 0x30)];
    pub const KNUCKLE_AHEAD: u32 = 0x40;
    pub const LINKS: [[u32; 2]; 2] = [[0x12fc, 0x1300], [0x1318, 0x1314]];
    pub const AXLE: u32 = 0x1330;
    pub const REAR_WHEEL: u32 = 0x132c;
    pub const HUB: u32 = 0x134c;
    pub const SHOCKS: [[u32; 2]; 2] = [[0x1334, 0x1338], [0x133c, 0x1340]];
    pub const UP: u32 = 0x14a0;
    pub const STEER_FRAME: u32 = 0x14b0;
    pub const AXLE_TO: u32 = 0x1490;
    pub const HUB_AT: u32 = 0x13d0;
    pub const SHOCK_TOPS: [u32; 2] = [0x1410, 0x1420];
    pub const STEER: u32 = 0x250;
    pub const ANGLE: u32 = 0x190;
    pub const COMPRESSION: u32 = 0xcc;
    pub const POSITION: u32 = 0x60;
    pub const FLAGS: u32 = 0x100;
}

pub(crate) fn read_golf_parts(ee: &Ee) -> GolfParts {
    let c = |o: u32| ee.u32_at(car_at() + o);
    GolfParts {
        car: c(at::NODE),
        steering: c(gs::STEERING),
        wheels: gs::WHEELS.map(c),
        corners: gs::CORNERS.map(|(arms, knuckle, ends, _)| GolfCorner { arms: arms.map(c), knuckle: c(knuckle), ends: ends.map(c) }),
        links: gs::LINKS.map(|l| l.map(c)),
        axle: c(gs::AXLE),
        rear_wheel: c(gs::REAR_WHEEL),
        hub: c(gs::HUB),
        shocks: gs::SHOCKS.map(|s| s.map(c)),
    }
}

pub(crate) fn read_golf_posing(ee: &Ee) -> GolfPosing {
    let tires = at::TIRES.map(|o| ee.u32_at(car_at() + o));
    let f = |a: u32| F::from_bits(ee.u32_at(a));
    let v = |o: u32| get_v(ee, car_at() + o);
    GolfPosing {
        steer: [f(tires[0] + gs::STEER), f(tires[1] + gs::STEER)],
        angle: tires.map(|t| f(t + gs::ANGLE)),
        compression: tires.map(|t| f(t + gs::COMPRESSION)),
        rear_mount: get_v(ee, tires[3] + at::MOUNT),
        up: v(gs::UP),
        steer_frame: core::array::from_fn(|r| v(gs::STEER_FRAME + 0x10 * r as u32)),
        arms: gs::CORNERS.map(|(_, _, _, block)| gs::ARM_AT.map(|(ahead, up)| GolfArm { ahead: v(block + ahead), up: v(block + up) })),
        knuckle_ahead: gs::CORNERS.map(|(_, _, _, block)| v(block + gs::KNUCKLE_AHEAD)),
        axle_to: v(gs::AXLE_TO),
        hub: v(gs::HUB_AT),
        shock_tops: gs::SHOCK_TOPS.map(v),
    }
}

/// The words the golf class step keeps on the car: the up's `w` and the steering frame.
pub(crate) fn golf_car_words(p: &GolfPosing) -> Vec<(u32, u32)> {
    let mut v = vec![(car_at() + gs::UP + 12, p.up.to_bits()[3])];
    for (r, row) in p.steer_frame.iter().enumerate() {
        for (k, w) in row.to_bits().into_iter().enumerate() {
            v.push((car_at() + gs::STEER_FRAME + 0x10 * r as u32 + 4 * k as u32, w));
        }
    }
    v
}

/// Every part node the step touches, the car's own first.
pub(crate) fn golf_nodes(p: &GolfParts) -> Vec<u32> {
    let mut v = vec![p.car, p.steering, p.axle, p.rear_wheel, p.hub];
    v.extend(p.wheels);
    for c in p.corners {
        v.extend(c.arms);
        v.push(c.knuckle);
        v.extend(c.ends);
    }
    v.extend(p.links.iter().flatten());
    v.extend(p.shocks.iter().flatten());
    v.sort_unstable();
    v.dedup();
    v
}

/// The port's words (its node words and the car words it keeps) equal the original's, and the
/// original wrote nothing in the part nodes or the car that the port did not. Returns the bad
/// lines.
pub(crate) fn compare_golf(ee: &Ee, port: &BTreeMap<u32, u32>, nodes: &[u32]) -> Vec<String> {
    let mut bad: Vec<String> = port
        .iter()
        .filter(|&(&a, &x)| ee.u32_at(a) != x)
        .map(|(&a, &x)| format!("  {a:#x}: port {x:#010x}, original {:#010x}", ee.u32_at(a)))
        .collect();
    let car = car_at();
    for (from, to) in ee.written_ranges() {
        for a in (from & !3..to).step_by(4) {
            let ours = nodes.iter().any(|&n| (n..n + 0x110).contains(&a)) || (car..car + 0x1700).contains(&a);
            if ours && !port.contains_key(&a) {
                bad.push(format!("  {a:#x}: the original wrote it, the port did not"));
            }
        }
    }
    bad
}

/// The port over the RAM the original is about to run on: its node words and the car words it
/// keeps.
pub(crate) fn golf_port(ee: &Ee, curve: &game_sim::camera::CurveTable) -> BTreeMap<u32, u32> {
    let parts = read_golf_parts(ee);
    let mut posing = read_golf_posing(ee);
    let mut over = Over { ee, words: BTreeMap::new() };
    atv_car::golf_class_step(&mut over, &parts, &mut posing, curve);
    let mut port = over.words;
    port.extend(golf_car_words(&posing));
    port
}

/// The golf drive's image, its car the one [`car_at`] reads.
fn golf_image() -> Option<Ee> {
    let d = drive::GOLF;
    let ee = drive::image(d.name)?;
    assert_eq!(ee.u32_at(d.car + 4), d.vtable, "no golf cart at {:#x}", d.car);
    THE_CAR.with(|c| c.set(d.car));
    Some(ee)
}

fn put(ee: &mut Ee, o: u32, v: Vf) {
    put_v(ee, car_at() + o, v);
}

/// Pokes a case: the posing values into the car and its tires, and each node's position row moved
/// and its current bit now and then cleared.
fn poke_case(ee: &mut Ee, pristine: &Ee, p: &GolfPosing, nodes: &[u32], r: &mut Rng) {
    let tires = at::TIRES.map(|o| ee.u32_at(car_at() + o));
    for k in 0..2 {
        ee.set_u32(tires[k] + gs::STEER, p.steer[k].to_bits());
    }
    for (k, &t) in tires.iter().enumerate() {
        ee.set_u32(t + gs::ANGLE, p.angle[k].to_bits());
        ee.set_u32(t + gs::COMPRESSION, p.compression[k].to_bits());
    }
    put_v(ee, tires[3] + at::MOUNT, p.rear_mount);
    put(ee, gs::UP, p.up);
    for (k, (_, _, _, block)) in gs::CORNERS.iter().enumerate() {
        for (arm, (ahead, up)) in p.arms[k].iter().zip(gs::ARM_AT) {
            put(ee, block + ahead, arm.ahead);
            put(ee, block + up, arm.up);
        }
        put(ee, block + gs::KNUCKLE_AHEAD, p.knuckle_ahead[k]);
    }
    put(ee, gs::AXLE_TO, p.axle_to);
    put(ee, gs::HUB_AT, p.hub);
    for (o, v) in gs::SHOCK_TOPS.iter().zip(p.shock_tops) {
        put(ee, *o, v);
    }
    for &n in nodes {
        let v = get_v(pristine, n + gs::POSITION);
        let d = r.vec(0.2, 0.0);
        put_v(ee, n + gs::POSITION, Vf::new(v.x() + d.x(), v.y() + d.y(), v.z() + d.z(), v.w()));
        let flags = pristine.u32_at(n + gs::FLAGS);
        ee.set_u32(n + gs::FLAGS, if r.chance(0.5) { flags & !1 } else { flags });
    }
}

fn jiggle(r: &mut Rng, v: Vf, by: f32) -> Vf {
    let d = r.vec(by, 0.0);
    Vf::new(v.x() + d.x(), v.y() + d.y(), v.z() + d.z(), v.w())
}

fn generate(r: &mut Rng, image: &GolfPosing) -> GolfPosing {
    let mut p = *image;
    let small = |r: &mut Rng, a: f32, b: f32| if r.chance(0.05) { F::ZERO } else { F(r.range(a, b)) };
    let steer = |r: &mut Rng| if r.chance(0.1) { F(r.range(-10.0, 10.0)) } else { small(r, -0.8, 0.8) };
    p.steer = [steer(r), steer(r)];
    p.angle = core::array::from_fn(|_| small(r, -40.0, 40.0));
    p.compression = core::array::from_fn(|_| small(r, -0.1, 0.6));
    p.rear_mount = jiggle(r, image.rear_mount, 0.3);
    // The up: the image's, its `w` anything (the step sets it to 1).
    p.up = jiggle(r, image.up, 0.3);
    p.up.0[3] = if r.chance(0.5) { 1.0 } else { r.range(-2.0, 2.0) };
    for k in 0..2 {
        for a in 0..3 {
            p.arms[k][a] = GolfArm { ahead: jiggle(r, image.arms[k][a].ahead, 0.3), up: jiggle(r, image.arms[k][a].up, 0.3) };
        }
        p.knuckle_ahead[k] = jiggle(r, image.knuckle_ahead[k], 0.3);
        p.shock_tops[k] = jiggle(r, image.shock_tops[k], 0.3);
    }
    p.axle_to = jiggle(r, image.axle_to, 0.3);
    p.hub = jiggle(r, image.hub, 0.3);
    p
}

/// **The golf cart's class step is the original's** (MD156a): SLUS-21104's golf cart vt `+0x488`
/// called on the golf drive's image's car, nothing hooked, against [`atv_car::golf_class_step`] over
/// the same RAM, over 10,000 generated cases (the tires' steer, angles and compressions, the mount,
/// the up, each arm's ahead and up, the knuckles' aheads, the axle's, hub's and shocks' points, each
/// part's position and current bit): every word the port writes equals the original's, and the
/// original wrote nothing in the part nodes or the car the port did not.
#[test]
fn the_golf_carts_class_step_is_the_originals() {
    let Some(pristine) = golf_image() else { return };
    let Some(mut ee) = golf_image() else { return };
    assert_eq!(ee.u32_at(drive::GOLF.vtable + 0x488), GOLF_CLASS_STEP, "the golf cart's vt +0x488");
    assert_eq!((0..4).map(|k| ee.u32_at(ORIGIN + 4 * k)).collect::<Vec<_>>(), [0, 0, 0, 0x3f80_0000], "the steering frame's origin");
    let curve = curve_table(&ee);
    let image_parts = read_golf_parts(&ee);
    let image_posing = read_golf_posing(&ee);
    let all = golf_nodes(&image_parts);
    assert!(!all.contains(&0), "the image's golf cart has all its parts: {image_parts:?}");
    let mut r = Rng(0x0156_a601_f000_0001);
    let mut seen = BTreeMap::<&str, u32>::new();
    let mut see = |what: &'static str, yes: bool| {
        if yes {
            *seen.entry(what).or_insert(0) += 1;
        }
    };
    const CASES: usize = 10_000;
    for case in 0..CASES {
        let posing = generate(&mut r, &image_posing);
        poke_case(&mut ee, &pristine, &posing, &all, &mut r);
        assert_eq!(read_golf_posing(&ee), posing);

        let port = golf_port(&ee, &curve);

        ee.set(28, GP);
        ee.call(GOLF_CLASS_STEP, &[u64::from(car_at())], &[], 2_000_000);

        let bad = compare_golf(&ee, &port, &all);
        assert!(bad.is_empty(), "case {case} ({posing:?}): {} words differ\n{}", bad.len(), bad.join("\n"));

        let pi = core::f32::consts::PI;
        see("a steer past a half turn", posing.steer.iter().any(|s| s.0.abs() > pi));
        see("a steer of 0", posing.steer.contains(&F::ZERO));
        see("the left rear compressed more", posing.compression[2] > posing.compression[3]);
        see("the right rear compressed more", posing.compression[3] > posing.compression[2]);
        see("a rear stretched", posing.compression[2].0 < 0.0 && posing.compression[3].0 < 0.0);
        see("a wheel not turned", posing.angle.contains(&F::ZERO));
        see("the up's w not 1", posing.up.w() != 1.0);

        if case == 0 {
            let (&a, &x) = port.iter().next().expect("the port wrote something");
            let mut flipped = port.clone();
            flipped.insert(a, x ^ 1);
            let red = compare_golf(&ee, &flipped, &all);
            assert_eq!(red.len(), 1, "a flipped bit: {red:?}");
        }

        for (from, to) in ee.written_ranges() {
            for a in from..to {
                ee.write(a, 1, u128::from(pristine.u8_at(a)));
            }
        }
    }
    eprintln!("MD156a: {CASES} golf class steps, seen {seen:?}");
    for (what, &n) in &seen {
        assert!(n >= 30, "{what} seen only {n} times");
    }
    assert_eq!(seen.len(), 7, "a branch was never seen: {seen:?}");
}
