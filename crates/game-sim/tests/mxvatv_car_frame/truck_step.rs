// --- MD156b: the monster truck's class step -------------------------------------------------------

use super::class_step::{Over, acos_table, curve_table};
use super::golf_step::compare_golf;
use super::*;
use game_sim::air::AcosTable;
use game_sim::atv_car::{TruckParts, TruckPosing};
use std::collections::BTreeMap;

#[path = "../mxvatv_car_replay/drive.rs"]
#[allow(dead_code)]
mod drive;

/// The monster truck's class step (vt `+0x488`), and the acos table's scale (a data word the
/// lean's lookup reads).
pub(crate) const TRUCK_CLASS_STEP: u32 = 0x004d_6f20;
const ACOS_SCALE: u32 = 0x003e_a9d8;

/// Where the monster truck keeps its parts and the values they are posed by (research/oracle.txt
/// `# MD156b`).
mod ts {
    pub const WHEELS: [u32; 4] = [0x12d0, 0x12d4, 0x12d8, 0x12dc];
    pub const UPRIGHTS: [u32; 4] = [0x12e0, 0x12e4, 0x12e8, 0x12ec];
    pub const MOUNTS: [u32; 4] = [0x12f0, 0x12f4, 0x12f8, 0x12fc];
    pub const AXLES: [u32; 2] = [0x1340, 0x1344];
    pub const SHOCKS: [u32; 2] = [0x1348, 0x134c];
    /// Each rod: the node aimed, and the node it is measured from.
    pub const RODS: [[u32; 2]; 8] = [
        [0x1300, 0x1320],
        [0x1304, 0x1324],
        [0x1308, 0x1328],
        [0x130c, 0x132c],
        [0x1310, 0x1330],
        [0x1314, 0x1334],
        [0x1318, 0x1338],
        [0x131c, 0x133c],
    ];
    pub const UPRIGHT_AT: [u32; 4] = [0x1350, 0x1360, 0x1370, 0x1380];
    pub const MOUNT_AT: [u32; 4] = [0x1390, 0x13a0, 0x13b0, 0x13c0];
    pub const AXLE_AT: [u32; 2] = [0x13d0, 0x13e0];
    /// The shocks' tops, then the rods'.
    pub const TOPS: u32 = 0x13f0;
    pub const TRACK: u32 = 0x1490;
    pub const REAR_STEER: u32 = 0x1494;
    pub const STEER: u32 = 0x250;
    pub const ANGLE: u32 = 0x190;
    pub const COMPRESSION: u32 = 0xcc;
    pub const MAX_COMPRESSION: u32 = 0x30;
    pub const STRETCH: u32 = 0x54;
    pub const POSITION: u32 = 0x60;
    pub const FLAGS: u32 = 0x100;
}

pub(crate) fn read_truck_parts(ee: &Ee) -> TruckParts {
    let c = |o: u32| ee.u32_at(car_at() + o);
    TruckParts {
        car: c(at::NODE),
        wheels: ts::WHEELS.map(c),
        uprights: ts::UPRIGHTS.map(c),
        mounts: ts::MOUNTS.map(c),
        axles: ts::AXLES.map(c),
        shocks: ts::SHOCKS.map(c),
        rods: ts::RODS.map(|r| r.map(c)),
    }
}

pub(crate) fn read_truck_posing(ee: &Ee) -> TruckPosing {
    let tires = at::TIRES.map(|o| ee.u32_at(car_at() + o));
    let f = |a: u32| F::from_bits(ee.u32_at(a));
    let t = |o: u32| tires.map(|t| f(t + o));
    let v = |o: u32| get_v(ee, car_at() + o);
    TruckPosing {
        steer: t(ts::STEER),
        angle: t(ts::ANGLE),
        compression: t(ts::COMPRESSION),
        max_compression: t(ts::MAX_COMPRESSION),
        stretch: t(ts::STRETCH),
        upright_at: ts::UPRIGHT_AT.map(v),
        mount_at: ts::MOUNT_AT.map(v),
        axle_at: ts::AXLE_AT.map(v),
        track: f(car_at() + ts::TRACK),
        rear_steer: f(car_at() + ts::REAR_STEER),
        tops: core::array::from_fn(|k| v(ts::TOPS + 0x10 * k as u32)),
    }
}

/// Every part node the step touches, the car's own first.
pub(crate) fn truck_nodes(p: &TruckParts) -> Vec<u32> {
    let mut v = vec![p.car];
    v.extend(p.wheels);
    v.extend(p.uprights);
    v.extend(p.mounts);
    v.extend(p.axles);
    v.extend(p.shocks);
    v.extend(p.rods.iter().flatten());
    v.sort_unstable();
    v.dedup();
    v
}

/// The port over the RAM the original is about to run on: its node words.
pub(crate) fn truck_port(ee: &Ee, curve: &game_sim::camera::CurveTable, acos: &AcosTable) -> BTreeMap<u32, u32> {
    let mut over = Over { ee, words: BTreeMap::new() };
    atv_car::truck_class_step(&mut over, &read_truck_parts(ee), &read_truck_posing(ee), curve, acos);
    over.words
}

/// The truck drive's image, its car the one [`car_at`] reads.
fn truck_image() -> Option<Ee> {
    let d = drive::TRUCK;
    let ee = drive::image(d.name)?;
    assert_eq!(ee.u32_at(d.car + 4), d.vtable, "no truck at {:#x}", d.car);
    THE_CAR.with(|c| c.set(d.car));
    Some(ee)
}

fn put(ee: &mut Ee, o: u32, v: Vf) {
    put_v(ee, car_at() + o, v);
}

/// Pokes a case: the posing values into the car and its tires, and each node's position row moved
/// and its current bit now and then cleared.
fn poke_case(ee: &mut Ee, pristine: &Ee, p: &TruckPosing, nodes: &[u32], r: &mut Rng) {
    let tires = at::TIRES.map(|o| ee.u32_at(car_at() + o));
    for (k, &t) in tires.iter().enumerate() {
        ee.set_u32(t + ts::STEER, p.steer[k].to_bits());
        ee.set_u32(t + ts::ANGLE, p.angle[k].to_bits());
        ee.set_u32(t + ts::COMPRESSION, p.compression[k].to_bits());
        ee.set_u32(t + ts::MAX_COMPRESSION, p.max_compression[k].to_bits());
        ee.set_u32(t + ts::STRETCH, p.stretch[k].to_bits());
    }
    for k in 0..4 {
        put(ee, ts::UPRIGHT_AT[k], p.upright_at[k]);
        put(ee, ts::MOUNT_AT[k], p.mount_at[k]);
    }
    for k in 0..2 {
        put(ee, ts::AXLE_AT[k], p.axle_at[k]);
    }
    ee.set_u32(car_at() + ts::TRACK, p.track.to_bits());
    ee.set_u32(car_at() + ts::REAR_STEER, p.rear_steer.to_bits());
    for (k, &v) in p.tops.iter().enumerate() {
        put(ee, ts::TOPS + 0x10 * k as u32, v);
    }
    for &n in nodes {
        let v = get_v(pristine, n + ts::POSITION);
        let d = r.vec(0.2, 0.0);
        put_v(ee, n + ts::POSITION, Vf::new(v.x() + d.x(), v.y() + d.y(), v.z() + d.z(), v.w()));
        let flags = pristine.u32_at(n + ts::FLAGS);
        ee.set_u32(n + ts::FLAGS, if r.chance(0.5) { flags & !1 } else { flags });
    }
}

fn jiggle(r: &mut Rng, v: Vf, by: f32) -> Vf {
    let d = r.vec(by, 0.0);
    Vf::new(v.x() + d.x(), v.y() + d.y(), v.z() + d.z(), v.w())
}

fn generate(r: &mut Rng, image: &TruckPosing) -> TruckPosing {
    let mut p = *image;
    let small = |r: &mut Rng, a: f32, b: f32| if r.chance(0.05) { F::ZERO } else { F(r.range(a, b)) };
    let steer = |r: &mut Rng| if r.chance(0.1) { F(r.range(-10.0, 10.0)) } else { small(r, -0.8, 0.8) };
    p.steer = core::array::from_fn(|_| steer(r));
    p.angle = core::array::from_fn(|_| small(r, -40.0, 40.0));
    p.compression = core::array::from_fn(|_| small(r, -0.2, 1.2));
    // Now and then a pair of tires compressed alike: the axle level (the lean's clamp's own edge).
    if r.chance(0.1) {
        p.compression[1] = p.compression[0];
    }
    if r.chance(0.1) {
        p.compression[3] = p.compression[2];
    }
    p.max_compression = core::array::from_fn(|k| if r.chance(0.1) { F::ZERO } else { F(image.max_compression[k].0 * r.range(0.5, 1.5)) });
    p.stretch = core::array::from_fn(|_| F(r.range(0.3, 1.7)));
    for k in 0..4 {
        p.upright_at[k] = jiggle(r, image.upright_at[k], 0.3);
        p.mount_at[k] = jiggle(r, image.mount_at[k], 0.3);
    }
    for k in 0..2 {
        p.axle_at[k] = jiggle(r, image.axle_at[k], 0.3);
    }
    // The track: the image's, now and then negated, small, 0 or huge.
    p.track = match r.range(0.0, 1.0) {
        x if x < 0.1 => -image.track,
        x if x < 0.15 => F::ZERO,
        x if x < 0.25 => F(r.range(-0.2, 0.2)),
        // Big enough that its square saturates: the cosine past ±1, the lean's clamp.
        x if x < 0.28 => F(r.range(1e19, 1e20) * if r.chance(0.5) { 1.0 } else { -1.0 }),
        _ => F(image.track.0 * r.range(0.5, 1.5)),
    };
    p.rear_steer = if r.chance(0.2) { image.rear_steer } else { F(r.range(-1.5, 1.5)) };
    for k in 0..10 {
        p.tops[k] = jiggle(r, image.tops[k], 0.3);
    }
    p
}

/// **The monster truck's class step is the original's** (MD156b): SLUS-21104's monster truck vt
/// `+0x488` called on the truck drive's image's car, nothing hooked, against
/// [`atv_car::truck_class_step`] over the same RAM, over 10,000 generated cases (every tire's steer,
/// angle, compression, maximum compression and stretch, the uprights', mounts' and axles' points,
/// the track, the rear share of the steer, the shocks' and rods' tops, each part's position and
/// current bit): every word the port writes equals the original's, and the original wrote nothing
/// in the part nodes or the car the port did not.
#[test]
fn the_monster_trucks_class_step_is_the_originals() {
    let Some(pristine) = truck_image() else { return };
    let Some(mut ee) = truck_image() else { return };
    assert_eq!(ee.u32_at(drive::TRUCK.vtable + 0x488), TRUCK_CLASS_STEP, "the truck's vt +0x488");
    assert_eq!(ee.f32_at(ACOS_SCALE), 14999.5, "the acos table's scale");
    let curve = curve_table(&ee);
    let acos = acos_table(&ee);
    let image_parts = read_truck_parts(&ee);
    let image_posing = read_truck_posing(&ee);
    eprintln!("MD156b: the image's truck {image_parts:x?}, {image_posing:?}");
    let all = truck_nodes(&image_parts);
    assert!(!all.contains(&0), "the image's truck has all its parts: {image_parts:x?}");
    let mut r = Rng(0x0156_b7c0_f000_0001);
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
        assert_eq!(read_truck_posing(&ee), posing);

        let port = truck_port(&ee, &curve, &acos);

        ee.set(28, GP);
        ee.call(TRUCK_CLASS_STEP, &[u64::from(car_at())], &[], 4_000_000);

        let bad = compare_golf(&ee, &port, &all);
        assert!(bad.is_empty(), "case {case} ({posing:?}): {} words differ\n{}", bad.len(), bad.join("\n"));

        let pi = core::f32::consts::PI;
        let c = posing.compression;
        see("a steer past a half turn", posing.steer.iter().any(|s| s.0.abs() > pi));
        see("a steer of 0", posing.steer.contains(&F::ZERO));
        see("a wheel not turned", posing.angle.contains(&F::ZERO));
        see("no maximum compression", posing.max_compression.contains(&F::ZERO));
        see("a stretch held at its least", (0..4).any(|k| {
            let full = posing.max_compression[k];
            full != F::ZERO && (c[k] / full).0 * (posing.stretch[k].0 - 1.0) + 1.0 < posing.stretch[k].0.min(1.0)
        }));
        see("a stretch held at its most", (0..4).any(|k| {
            let full = posing.max_compression[k];
            full != F::ZERO && (c[k] / full).0 * (posing.stretch[k].0 - 1.0) + 1.0 > posing.stretch[k].0.max(1.0)
        }));
        see("an axle leaned left", c[1] < c[0] || c[3] < c[2]);
        see("an axle leaned right", c[1] > c[0] || c[3] > c[2]);
        see("an axle level", c[1] == c[0] || c[3] == c[2]);
        // The lean's cosine as the port and the original take it: its clamp's two sides.
        let lean = |l: usize, r: usize| {
            let (t, d) = (posing.track, c[r] - c[l]);
            t * F::ONE.rsqrt(ps2_float::Acc::mula(t, t).madd(d, d))
        };
        let leans = [lean(0, 1), lean(2, 3)];
        see("a lean past 1", leans.iter().any(|&x| x > F::ONE));
        see("a lean below -1", leans.iter().any(|&x| x < F(-1.0)));
        see("a track of 0", posing.track == F::ZERO);
        see("a track below 0", posing.track.0 < 0.0);

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
    eprintln!("MD156b: {CASES} truck class steps, seen {seen:?}");
    for (what, &n) in &seen {
        assert!(n >= 30, "{what} seen only {n} times");
    }
    assert_eq!(seen.len(), 13, "a branch was never seen: {seen:?}");
}
