use super::*;
use game_sim::atv_car::{FrontParts, Parts, Posing};
use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::handling::node;
use std::collections::BTreeMap;

/// The buggy's class step (vt `+0x488`).
pub(crate) const CLASS_STEP: u32 = 0x004c_5510;
/// The coarse curve table's pointer, the acos table's pointer, and the swing's factor's data word.
const CURVE_TABLE: u32 = 0x003e_ab20;
const ACOS_TABLE: u32 = 0x003e_a9c0;
const SWING_FACTOR: u32 = 0x0052_52b0;

/// Where the car keeps its parts and the values they are posed by (research/oracle.txt `# MD151`).
mod cs {
    pub const WHEELS: [u32; 4] = [0x12d0, 0x12d4, 0x12d8, 0x12dc];
    pub const AXLE: u32 = 0x12e0;
    /// Each front corner: upright, arm, knuckle, the two ends, top.
    pub const FRONTS: [[u32; 6]; 2] =
        [[0x12f8, 0x12e8, 0x12e4, 0x12ec, 0x12f0, 0x12f4], [0x1310, 0x1300, 0x12fc, 0x1304, 0x1308, 0x130c]];
    pub const RODS: u32 = 0x1314;
    pub const LINKS: [u32; 4] = [0x1354, 0x1358, 0x135c, 0x1360];
    pub const SPINNERS: [u32; 2] = [0x1368, 0x136c];
    pub const SWING: u32 = 0x1370;
    pub const PIVOTS: [u32; 2] = [0x13c0, 0x13a0];
    pub const REAR_PIVOT: u32 = 0x13e0;
    pub const RPM: u32 = 0x11c;
    pub const STEER: u32 = 0x250;
    pub const ANGLE: u32 = 0x190;
    pub const COMPRESSION: u32 = 0xcc;
    pub const MAX_COMPRESSION: u32 = 0x30;
    pub const STRETCH: u32 = 0x54;
    /// A scene node's local frame, world frame and flags, as far as the step touches them.
    pub const NODE_FROM: u32 = 0x30;
    pub const NODE_TO: u32 = 0x104;
    pub const POSITION: u32 = 0x60;
    pub const FLAGS: u32 = 0x100;
}

pub(crate) fn read_parts(ee: &Ee) -> Parts {
    let c = |o: u32| ee.u32_at(car_at() + o);
    Parts {
        car: c(at::NODE),
        wheels: cs::WHEELS.map(c),
        fronts: cs::FRONTS.map(|[upright, arm, knuckle, e0, e1, top]| FrontParts {
            upright: c(upright),
            arm: c(arm),
            knuckle: c(knuckle),
            ends: [c(e0), c(e1)],
            top: c(top),
        }),
        axle: c(cs::AXLE),
        links: cs::LINKS.map(c),
        spinners: cs::SPINNERS.map(c),
        rods: core::array::from_fn(|k| [c(cs::RODS + 8 * k as u32), c(cs::RODS + 8 * k as u32 + 4)]),
    }
}

pub(crate) fn read_posing(ee: &Ee) -> Posing {
    let tires = at::TIRES.map(|o| ee.u32_at(car_at() + o));
    let f = |a: u32| F::from_bits(ee.u32_at(a));
    let t = |o: u32| tires.map(|t| f(t + o));
    Posing {
        steer: [f(tires[0] + cs::STEER), f(tires[1] + cs::STEER)],
        angle: t(cs::ANGLE),
        compression: t(cs::COMPRESSION),
        max_compression: t(cs::MAX_COMPRESSION),
        stretch: t(cs::STRETCH),
        rear_mount: get_v(ee, tires[2] + at::MOUNT),
        swing: f(car_at() + cs::SWING),
        pivots: cs::PIVOTS.map(|o| get_v(ee, car_at() + o)),
        rear_pivot: get_v(ee, car_at() + cs::REAR_PIVOT),
        rpm: f(ee.u32_at(car_at() + at::ENGINE) + cs::RPM),
    }
}

pub(crate) fn curve_table(ee: &Ee) -> CurveTable {
    let base = ee.u32_at(CURVE_TABLE);
    let entries: Vec<[f32; 2]> =
        (0..CURVE_ENTRIES as u32).map(|k| [ee.f32_at(base + 8 * k), ee.f32_at(base + 8 * k + 4)]).collect();
    CurveTable::from_entries(&entries)
}

/// The image's acos table, checked to be the one [`AcosTable::build`] makes.
pub(crate) fn acos_table(ee: &Ee) -> AcosTable {
    let built = AcosTable::build();
    let base = ee.u32_at(ACOS_TABLE);
    assert_ne!(base, 0, "the image has an acos table");
    for (k, e) in built.entries().iter().enumerate() {
        assert_eq!(ee.u32_at(base + 4 * k as u32), e.to_bits(), "acos table entry {k}");
    }
    built
}

/// The image's RAM with the port's writes laid over it: what the port wrote, word by word.
pub(crate) struct Over<'a> {
    pub ee: &'a Ee,
    pub words: BTreeMap<u32, u32>,
}

impl node::Ram for Over<'_> {
    fn word(&mut self, at: u32) -> u32 {
        self.words.get(&at).copied().unwrap_or_else(|| self.ee.u32_at(at))
    }
    fn set(&mut self, at: u32, w: u32) {
        self.words.insert(at, w);
    }
}

/// Every part node the step may touch, the car's own first.
pub(crate) fn nodes(p: &Parts) -> Vec<u32> {
    let mut v = vec![p.car];
    v.extend(p.wheels);
    for f in p.fronts {
        v.extend([f.upright, f.arm, f.knuckle, f.ends[0], f.ends[1], f.top]);
    }
    v.push(p.axle);
    v.extend(p.links);
    v.extend(p.spinners);
    v.extend(p.rods.iter().flatten());
    v.retain(|&n| n != 0);
    v.sort_unstable();
    v.dedup();
    v
}

/// The port's words equal the original's, and the original wrote nothing in the nodes the port did
/// not. Returns the bad lines.
pub(crate) fn compare_nodes(ee: &Ee, port: &BTreeMap<u32, u32>, nodes: &[u32]) -> Vec<String> {
    let mut bad: Vec<String> = port
        .iter()
        .filter(|&(&a, &x)| ee.u32_at(a) != x)
        .map(|(&a, &x)| format!("  {a:#x}: port {x:#010x}, original {:#010x}", ee.u32_at(a)))
        .collect();
    for (from, to) in ee.written_ranges() {
        for a in (from & !3..to).step_by(4) {
            if nodes.iter().any(|&n| (n..n + 0x110).contains(&a)) && !port.contains_key(&a) {
                bad.push(format!("  {a:#x}: the original wrote it, the port did not"));
            }
        }
    }
    bad
}

/// Pokes a case: the posing values into the car, its tires and engine, and each node's position row
/// moved and its current bit now and then cleared.
fn poke_case(ee: &mut Ee, pristine: &Ee, p: &Posing, nodes: &[u32], r: &mut Rng) {
    let tires = at::TIRES.map(|o| ee.u32_at(CAR + o));
    for k in 0..2 {
        ee.set_u32(tires[k] + cs::STEER, p.steer[k].to_bits());
    }
    for (k, &t) in tires.iter().enumerate() {
        ee.set_u32(t + cs::ANGLE, p.angle[k].to_bits());
        ee.set_u32(t + cs::COMPRESSION, p.compression[k].to_bits());
        ee.set_u32(t + cs::MAX_COMPRESSION, p.max_compression[k].to_bits());
        ee.set_u32(t + cs::STRETCH, p.stretch[k].to_bits());
    }
    put_v(ee, tires[2] + at::MOUNT, p.rear_mount);
    ee.set_u32(CAR + cs::SWING, p.swing.to_bits());
    for (o, v) in cs::PIVOTS.iter().zip(p.pivots) {
        put_v(ee, CAR + o, v);
    }
    put_v(ee, CAR + cs::REAR_PIVOT, p.rear_pivot);
    let engine = ee.u32_at(CAR + at::ENGINE);
    ee.set_u32(engine + cs::RPM, p.rpm.to_bits());
    for &n in nodes {
        let v = get_v(pristine, n + cs::POSITION);
        let d = r.vec(0.2, 0.0);
        put_v(ee, n + cs::POSITION, Vf::new(v.x() + d.x(), v.y() + d.y(), v.z() + d.z(), v.w()));
        let flags = pristine.u32_at(n + cs::FLAGS);
        ee.set_u32(n + cs::FLAGS, if r.chance(0.5) { flags & !1 } else { flags });
    }
}

fn generate(r: &mut Rng, image: &Posing) -> Posing {
    let mut p = *image;
    let small = |r: &mut Rng, a: f32, b: f32| if r.chance(0.05) { F::ZERO } else { F(r.range(a, b)) };
    p.steer = [small(r, -0.8, 0.8), small(r, -0.8, 0.8)];
    p.angle = core::array::from_fn(|_| small(r, -40.0, 40.0));
    p.compression = core::array::from_fn(|_| small(r, -0.1, 0.6));
    p.max_compression = core::array::from_fn(|_| if r.chance(0.15) { F::ZERO } else { F(r.range(0.05, 0.6)) });
    p.stretch = core::array::from_fn(|_| F(r.range(0.4, 1.6)));
    let jiggle = |r: &mut Rng, v: Vf, by: f32| {
        let d = r.vec(by, 0.0);
        Vf::new(v.x() + d.x(), v.y() + d.y(), v.z() + d.z(), v.w())
    };
    p.rear_mount = jiggle(r, image.rear_mount, 0.3);
    p.swing = F(r.range(-2.0, 2.0));
    p.pivots = image.pivots.map(|v| jiggle(r, v, 0.3));
    p.rear_pivot = jiggle(r, image.rear_pivot, 0.3);
    p.rpm = if r.chance(0.1) { F::ZERO } else { F(r.range(0.0, 9000.0)) };
    p
}

/// **The buggy's class step is the original's** (MD151): SLUS-21104's vt `+0x488` called on the
/// buggy image's car, nothing hooked, against [`atv_car::class_step`] over the same RAM, over
/// 10,000 generated cases (the tires' steer, angles, compressions and stretches, the mount, swing,
/// pivots and rpm, each part's position and current bit, links, spinners and odd rods now and then
/// missing): every word the port writes equals the original's, and the original wrote nothing in
/// the part nodes the port did not. Each branch is seen.
#[test]
fn the_buggys_class_step_is_the_originals() {
    let Some(pristine) = image() else { return };
    let Some(mut ee) = image() else { return };
    assert_eq!(ee.u32_at(BUGGY_VTABLE + 0x488), CLASS_STEP, "the buggy's vt +0x488");
    assert_eq!(ee.f32_at(SWING_FACTOR), 1.25, "the swing's factor");
    let curve = curve_table(&ee);
    let acos = acos_table(&ee);
    let image_parts = read_parts(&ee);
    let image_posing = read_posing(&ee);
    let all = nodes(&image_parts);
    for n in [image_parts.car, image_parts.axle].into_iter().chain(image_parts.wheels).chain(image_parts.rods.into_iter().flatten()) {
        assert_ne!(n, 0, "the image's buggy has its wheels, axle and rods: {image_parts:?}");
    }
    // The buggy has no links or spinners: the cases borrow the first three rod pairs' nodes.
    assert_eq!((image_parts.links, image_parts.spinners), ([0; 4], [0; 2]), "the image's buggy's links and spinners");
    let r = image_parts.rods;
    let links = [r[0][0], r[0][1], r[1][0], r[1][1]];
    let spinners = r[2];
    let mut r = Rng(0x0151_c1a5_5000_0001);
    let mut seen = BTreeMap::<&str, u32>::new();
    let mut see = |what: &'static str, yes: bool| {
        if yes {
            *seen.entry(what).or_insert(0) += 1;
        }
    };
    const CASES: usize = 10_000;
    for case in 0..CASES {
        let posing = generate(&mut r, &image_posing);
        let mut parts = Parts { links, spinners, ..image_parts };
        if r.chance(0.2) {
            parts.links[(r.next() % 4) as usize] = 0;
        }
        if r.chance(0.2) {
            parts.spinners[(r.next() % 2) as usize] = 0;
        }
        for k in (1..8).step_by(2) {
            if r.chance(0.2) {
                parts.rods[k][(r.next() % 2) as usize] = 0;
            }
        }
        poke_case(&mut ee, &pristine, &posing, &all, &mut r);
        for (o, n) in cs::LINKS.iter().zip(parts.links) {
            ee.set_u32(CAR + o, n);
        }
        for (o, n) in cs::SPINNERS.iter().zip(parts.spinners) {
            ee.set_u32(CAR + o, n);
        }
        for (k, pair) in parts.rods.iter().enumerate() {
            ee.set_u32(CAR + cs::RODS + 8 * k as u32, pair[0]);
            ee.set_u32(CAR + cs::RODS + 8 * k as u32 + 4, pair[1]);
        }
        assert_eq!(read_parts(&ee), parts);
        assert_eq!(read_posing(&ee), posing);

        // The port, over the RAM the original is about to run on.
        let mut over = Over { ee: &ee, words: BTreeMap::new() };
        atv_car::class_step(&mut over, &parts, &posing, &curve, &acos);
        let port = over.words;

        // The original.
        ee.set(28, GP);
        ee.call(CLASS_STEP, &[u64::from(CAR)], &[], 2_000_000);

        let bad = compare_nodes(&ee, &port, &all);
        assert!(bad.is_empty(), "case {case} ({posing:?}, {parts:?}): {} words differ\n{}", bad.len(), bad.join("\n"));

        see("links aimed", parts.links.iter().all(|&n| n != 0));
        see("links missing", parts.links.contains(&0));
        see("spinners turned", parts.spinners.iter().all(|&n| n != 0) && posing.rpm != F::ZERO);
        see("spinners still", parts.spinners.iter().all(|&n| n != 0) && posing.rpm == F::ZERO);
        see("spinners missing", parts.spinners.contains(&0));
        see("odd rods aimed", (1..8).step_by(2).all(|k| parts.rods[k].iter().all(|&n| n != 0)));
        see("an odd rod missing", (1..8).step_by(2).any(|k| parts.rods[k].contains(&0)));
        see("a wheel unstretchable", posing.max_compression.contains(&F::ZERO));
        see("a wheel stretched past its stretch", (0..4).any(|k| posing.max_compression[k] != F::ZERO && posing.compression[k] > posing.max_compression[k]));
        see("a wheel squeezed below 1", (0..4).any(|k| posing.max_compression[k] != F::ZERO && posing.compression[k] < F::ZERO));
        see("a wheel not turned", posing.angle.contains(&F::ZERO));

        if case == 0 {
            let (&a, &x) = port.iter().next().expect("the port wrote something");
            let mut flipped = port.clone();
            flipped.insert(a, x ^ 1);
            let red = compare_nodes(&ee, &flipped, &all);
            assert_eq!(red.len(), 1, "a flipped bit: {red:?}");
        }

        for (from, to) in ee.written_ranges() {
            for a in from..to {
                ee.write(a, 1, u128::from(pristine.u8_at(a)));
            }
        }
    }
    eprintln!("MD151: {CASES} class steps, seen {seen:?}");
    for (what, &n) in &seen {
        assert!(n >= 30, "{what} seen only {n} times");
    }
    assert_eq!(seen.len(), 11, "a branch was never seen: {seen:?}");
}
