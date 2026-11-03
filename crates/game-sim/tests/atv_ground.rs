//! MD76 — Desert Oasis's ground for the ATV, proved against MX vs. ATV's own ground query.
//!
//! The oracle is SLUS-21104 itself: MD37a's RAM image of Desert Oasis Free Ride with the atv_ quad at
//! its spawn (`context/mxvatv-ps2/md37a/atv-r-spawn.ee.bin`) holds the loaded terrain object the quad's
//! wheels ask, its cell reader's tree and blocks, and the query's ten-cell ring. The query runs there
//! as it is, nothing hooked. The port reads the same map off its file, `Env\mc02\MC02.ps2.trn` out of
//! `mc02.res` (copied into the same folder from MX vs. ATV's disc), and must answer every point the
//! same, bit for bit: the point, the normal and the record byte.
//!
//! The points are the ones the captured rides asked about: every wheel's probe end (wheel `+0x160`)
//! in every row of MD37a's fifteen captures, and as many again scattered a few feet round them, plus
//! points past the map's edges. Some calls run with the ring cold, some with the terrain object's
//! coarse and parity-split flags flipped, some with no normal or no record asked for.
//!
//! Without the image, the map or the captures every test here skips (and says so).

use game_sim::archive::Res;
use game_sim::atv_ground::{self, Terrain};
use game_sim::track;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The ground query, and the terrain object the quad's four wheels hold (wheel `+0x1e0`).
const GROUND_QUERY: u32 = 0x0048_33c0;
const TERRAIN: u32 = 0x0184_c7d0;
/// The quad and one of its wheels, to check the image is the one these come from.
const QUAD_VTABLE_AT: u32 = 0x015e_0694;
const QUAD_VTABLE: u32 = 0x003c_e250;
const LEFT_FRONT: u32 = 0x0162_a7e0;
/// The query's ring of cells (0x70 bytes each): it uses ten (the count it reads), but a cold start
/// clears the keys of 32.
const RING: u32 = 0x0054_1550;
const RING_BYTES: u32 = 32 * 0x70;
/// Where the call's buffers go: the point, the normal and the record byte.
const POINT: u32 = 0x01ff_0000;
const NORMAL: u32 = 0x01ff_0010;
const RECORD: u32 = 0x01ff_0020;
/// What the normal and record buffers hold before the call, to see whether it wrote them.
const MARK: u32 = 0x7f80_0123;
const RECORD_MARK: u8 = 0xee;

struct Data {
    ee: Ee,
    trn: Vec<u8>,
    points: Vec<Vf>,
}

fn data() -> Option<Data> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let dir = kit.ctx("mxvatv-ps2/md37a");
    let image = dir.join(format!("atv-r-spawn.ee{}", ".bin"));
    let res = dir.join("mc02.res");
    for (p, what) in [(&image, "MD37a's eeMemory.bin of atv-r-spawn.p2s"), (&res, "Desert Oasis's mc02.res off MX vs. ATV's disc (Env/MC02)")] {
        if !p.exists() {
            skip(&format!("{} is not there: {what} (memory: md37a ATV states live in s2)", p.display()));
            return None;
        }
    }
    let ee = Ee::from_ram(&image).unwrap_or_else(|e| panic!("{e}"));
    if ee.u32_at(QUAD_VTABLE_AT) != QUAD_VTABLE || ee.u32_at(LEFT_FRONT + 0x1e0) != TERRAIN {
        panic!("{} is not the MX vs. ATV quad image these addresses are from", image.display());
    }
    let file = std::fs::read(&res).unwrap();
    let archive = Res::parse(&file).unwrap();
    let entry = archive.find("Env\\mc02\\MC02.ps2.trn").expect("mc02.res holds MC02.ps2.trn");
    let trn = archive.read(&file, entry).unwrap();
    let points = probe_ends(&kit.ctx("capture/md37a"))?;
    Some(Data { ee, trn, points })
}

/// Every wheel's probe end, in the world, in every row of the captures.
fn probe_ends(dir: &PathBuf) -> Option<Vec<Vf>> {
    let mut names: Vec<_> = std::fs::read_dir(dir).ok().into_iter().flatten().flatten().map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "csv")).collect();
    names.sort();
    if names.len() < 15 {
        skip(&format!("{} does not hold MD37a's fifteen captures", dir.display()));
        return None;
    }
    let mut out = Vec::new();
    for name in names {
        let text = std::fs::read_to_string(&name).unwrap();
        let mut lines = text.lines();
        let head: Vec<&str> = lines.next().unwrap().split(',').collect();
        let cols: Vec<usize> = (0..4).map(|i| head.iter().position(|h| *h == format!("wheel{i}.raw")).unwrap()).collect();
        for line in lines {
            let f: Vec<&str> = line.split(',').collect();
            for &c in &cols {
                let raw = f[c];
                let word = |k: usize| {
                    let at = (0x160 + 4 * k) * 2;
                    let b: Vec<u8> = (0..4).map(|i| u8::from_str_radix(&raw[at + 2 * i..at + 2 * i + 2], 16).unwrap()).collect();
                    f32::from_le_bytes(b.try_into().unwrap())
                };
                out.push(Vf::new(word(0), word(1), word(2), word(3)));
            }
        }
    }
    Some(out)
}

/// xorshift, so a failure names a seed that repeats.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }
}

fn put(ee: &mut Ee, a: u32, v: Vf) {
    for (k, x) in v.0.iter().enumerate() {
        ee.set_f32(a + 4 * k as u32, *x);
    }
}

fn get(ee: &Ee, a: u32) -> [u32; 4] {
    std::array::from_fn(|k| ee.u32_at(a + 4 * k as u32))
}

/// The port's terrain over `map` with the image's object's own words.
fn from_image<'a>(ee: &Ee, map: &'a track::Terrain<'a>) -> Terrain<'a> {
    let v = |a: u32| Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12));
    Terrain {
        map: Some(map),
        min: v(TERRAIN + 0x30),
        max: v(TERRAIN + 0x40),
        margin: F(ee.f32_at(TERRAIN + 0x50)),
        per_sample: F(ee.f32_at(TERRAIN + 0x788)),
        height_scale: F(ee.f32_at(TERRAIN + 0x90)),
        coarse: ee.u32_at(TERRAIN + 0x8b8) != 0,
        split_one_way: ee.u32_at(TERRAIN + 0x770) == 1,
    }
}

/// The terrain object SLUS-21104 built over Desert Oasis holds what [`Terrain::of`] works out of the
/// map file: the box's `x` and `z`, the margin, samples per foot, the height scale and both flags.
#[test]
fn desert_oasis_terrain_object_is_the_maps() {
    let Some(d) = data() else { return };
    let map = track::Terrain::parse(&d.trn).unwrap();
    let ours = Terrain::of(&map);
    let image = from_image(&d.ee, &map);
    let bits = |v: Vf| [v.x().to_bits(), v.z().to_bits()];
    assert_eq!(bits(ours.min), bits(image.min), "box min");
    assert_eq!(bits(ours.max), bits(image.max), "box max");
    assert_eq!(ours.margin.0.to_bits(), image.margin.0.to_bits(), "margin");
    assert_eq!(ours.per_sample.0.to_bits(), image.per_sample.0.to_bits(), "samples per foot");
    assert_eq!(ours.height_scale.0.to_bits(), image.height_scale.0.to_bits(), "height scale");
    assert_eq!((ours.coarse, ours.split_one_way), (image.coarse, image.split_one_way), "flags");
}

/// The ground query's answers, the original's against the port's, on the captured rides' points.
#[test]
fn the_atv_ground_query_is_the_originals_along_md37as_rides() {
    let Some(mut d) = data() else { return };
    let map = track::Terrain::parse(&d.trn).unwrap();
    let mut rng = Rng(0x4d44_3736_6772_6f75);
    let ee = &mut d.ee;
    let (coarse0, split0) = (ee.u32_at(TERRAIN + 0x8b8), ee.u32_at(TERRAIN + 0x770));
    let (lo, hi) = (F(ee.f32_at(TERRAIN + 0x30)), F(ee.f32_at(TERRAIN + 0x40)));
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();

    let mut points: Vec<(Vf, &str)> = d.points.iter().map(|&p| (p, "probe end")).collect();
    for &p in &d.points {
        let q = Vf::new(p.x() + rng.range(-12.0, 12.0), p.y() + rng.range(-20.0, 20.0), p.z() + rng.range(-12.0, 12.0), 1.0);
        points.push((q, "near a probe end"));
    }
    for _ in 0..2000 {
        let far = |r: &mut Rng| if r.chance(0.5) { r.range(-300.0, 0.0) } else { r.range(5760.0, 6100.0) };
        let x = if rng.chance(0.5) { far(&mut rng) } else { rng.range(0.0, 5760.0) };
        let z = if rng.chance(0.5) { far(&mut rng) } else { rng.range(0.0, 5760.0) };
        let z = if x >= lo.0 && x <= hi.0 && z >= lo.0 && z <= hi.0 { -z - 1.0 } else { z };
        points.push((Vf::new(x, rng.range(0.0, 400.0), z, 1.0), "past an edge"));
    }
    // On a cell's midlines or corners, where two corners are as near as each other: the record
    // takes the first of them.
    for &p in d.points.iter().step_by(4) {
        let on = |c: f32, r: &mut Rng| {
            let s = (c / 4.5).floor();
            (s + [0.0, 0.5, 1.0][(r.next() % 3) as usize]) * 4.5
        };
        let q = Vf::new(on(p.x(), &mut rng), p.y(), on(p.z(), &mut rng), 1.0);
        points.push((q, "on a midline or corner"));
    }
    assert!(points.len() >= 10_000, "{} points", points.len());

    for (n, &(p, kind)) in points.iter().enumerate() {
        // The flags: the image's on most calls; the other split or the coarse read on some (the ring
        // is keyed by the sample alone, so it starts cold whenever they change).
        let coarse = if rng.chance(0.1) { 1 } else { coarse0 };
        let split = if rng.chance(0.15) { 0 } else { split0 };
        let flipped = (coarse, split) != (ee.u32_at(TERRAIN + 0x8b8), ee.u32_at(TERRAIN + 0x770));
        ee.set_u32(TERRAIN + 0x8b8, coarse);
        ee.set_u32(TERRAIN + 0x770, split);
        let cold = flipped || rng.chance(0.3);
        if cold {
            ee.set_u32(TERRAIN + 0x9e0, 0);
        }
        let want_normal = !rng.chance(0.1);
        let want_record = !rng.chance(0.1);
        put(ee, POINT, p);
        ee.set_u32(NORMAL, MARK);
        ee.set_u32(NORMAL + 4, MARK);
        ee.set_u32(NORMAL + 8, MARK);
        ee.set_u32(NORMAL + 12, MARK);
        ee.write(RECORD, 1, u128::from(RECORD_MARK));
        let args = [
            u64::from(TERRAIN),
            u64::from(POINT),
            if want_normal { u64::from(NORMAL) } else { 0 },
            0,
            if want_record { u64::from(RECORD) } else { 0 },
        ];
        ee.call(GROUND_QUERY, &args, &[], 200_000);
        for (a, e) in ee.written_ranges() {
            let ok = (POINT..RECORD + 1).contains(&a) && e <= RECORD + 1
                || (RING..RING + RING_BYTES).contains(&a) && e <= RING + RING_BYTES
                || a >= TERRAIN + 0x9e0 && e <= TERRAIN + 0x9e8
                || a >= ee.stack - 0x1000;
            assert!(ok, "point {n} {p:?}: the original wrote {a:#x}..{e:#x}");
        }

        let t = from_image(ee, &map);
        let mut point = p;
        let mut normal = Vf(std::array::from_fn(|_| f32::from_bits(MARK)));
        let mut record = RECORD_MARK;
        atv_ground::query(&t, &mut point, want_normal.then_some(&mut normal), want_record.then_some(&mut record));

        let ctx = format!("point {n} ({kind}) {p:?}, coarse {coarse} split {split} cold {cold}");
        assert_eq!(get(ee, POINT), point.0.map(f32::to_bits), "{ctx}: point");
        if want_normal {
            assert_eq!(get(ee, NORMAL), normal.0.map(f32::to_bits), "{ctx}: normal");
        }
        assert_eq!(ee.u8_at(RECORD), record, "{ctx}: record");

        // Which way the port went, the comparison having proved it went the original's.
        *seen.entry(kind).or_default() += 1;
        let held = !(F(p.x()) > lo + t.margin && F(p.x()) < hi - t.margin && F(p.z()) > lo + t.margin && F(p.z()) < hi - t.margin);
        if held {
            *seen.entry("held in the box").or_default() += 1;
        }
        *seen.entry(if cold { "ring cold" } else { "ring warm" }).or_default() += 1;
        if coarse != 0 {
            *seen.entry("coarse").or_default() += 1;
        }
        if point.y() == p.y() && normal.0.map(f32::to_bits) == atv_ground::UP.0.map(f32::to_bits) {
            *seen.entry("no cell").or_default() += 1;
        }
        let fx = (t.per_sample * F(p.x()).max(lo + t.margin).min(hi - t.margin)).max(F::ZERO);
        let fz = (t.per_sample * F(p.z()).max(lo + t.margin).min(hi - t.margin)).max(F::ZERO);
        let step = if coarse != 0 { 2 } else { 1 };
        let (ix, iz) = (fx.to_i32() & !(step - 1), fz.to_i32() & !(step - 1));
        let u = (fx - F::from(ix)) / F::from(step);
        let v = (fz - F::from(iz)) / F::from(step);
        let (nu, nv) = (F::ONE - u, F::ONE - v);
        let one_way = split == 1 || (ix & 1) != (iz & 1);
        let path = match (one_way, if one_way { u * u + v * v < nu * nu + nv * nv } else { nu * nu + v * v < u * u + nv * nv }) {
            (true, true) => "split one way, near the sample",
            (true, false) => "split one way, near the far corner",
            (false, true) => "split the other way, near +x",
            (false, false) => "split the other way, near +z",
        };
        *seen.entry(path).or_default() += 1;
        let d = [u * u + v * v, nu * nu + v * v, u * u + nv * nv, nu * nu + nv * nv];
        let least = d.iter().fold(F::from_bits(0x7f7f_ffff), |a, &b| if b < a { b } else { a });
        if d.iter().filter(|&&x| x == least).count() > 1 {
            *seen.entry("a tie for nearest").or_default() += 1;
        }
        if !want_normal {
            *seen.entry("no normal asked").or_default() += 1;
        }
        if !want_record {
            *seen.entry("no record asked").or_default() += 1;
        }
    }
    ee.set_u32(TERRAIN + 0x8b8, coarse0);
    ee.set_u32(TERRAIN + 0x770, split0);
    eprintln!("{} points: {seen:?}", points.len());
    for path in [
        "probe end", "near a probe end", "past an edge", "on a midline or corner", "a tie for nearest", "held in the box", "ring cold", "ring warm", "coarse",
        "split one way, near the sample", "split one way, near the far corner", "split the other way, near +x",
        "split the other way, near +z", "no normal asked", "no record asked",
    ] {
        assert!(seen.get(path).copied().unwrap_or(0) >= 100, "{path}: seen {:?} times", seen.get(path));
    }
}
