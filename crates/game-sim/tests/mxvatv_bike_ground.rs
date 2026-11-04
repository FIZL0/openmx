//! F31c — the ground under C15's SLUS-21104 rides, the port's against the original's on state 4.
//!
//! State 4 (`context/mxvatv-ps2/state04.ee.bin`) is a Free Ride on Nat05. The port reads the map off
//! its file, `Env\nat05\Nat05.ps2.trn` out of `nat05.res` (copied off MX vs. ATV's disc into
//! `context/mxvatv-ps2/nat05/`), and [`game_sim::handling::ground`] builds the terrain object from it.
//!
//! The interpreter replays C15's captures as `tests/mxvatv_bike_ride.rs` does (the same hooks, the
//! same rows fed in), with the ground query and the collision record's terrain query passed through:
//! each call the bike makes is logged with its buffers before and after, and the port must answer the
//! same buffers bit for bit:
//!
//! - every wheel query (the ground query with a wheel's `+0x110` as its record): the point, the normal
//!   and the record byte. Row 0 of ride-2 makes eight, two wheels by four substeps;
//! - every terrain query of the bike's collision record (flip-1's wreck rows): the point, the normal,
//!   the surfaces and the weights, and the surface the terrain service picks of them.
//!
//! The searches of the bike's rays (its two trigger rays, the riding and the wreck parts) are watched
//! too: none of them files a contact along either ride, so the terrain is all the ground they ride on.
//!
//! Without the image, the map or the captures every test here skips (and says so).

use game_sim::contact_search::Ground;
use game_sim::handling::ground::{self, WHEEL_RECORD, WHEEL_TERRAIN};
use game_sim::track;
use ps2_float::Vf;
use ps2_oracle::{Ee, Kit, skip};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

const BIKE: u32 = 0x00e7_14e0;
const SIM: u32 = 0x0066_59e0;
const FRAME: u32 = 0x0020_4040;
const DT: u32 = 0x3d08_850a;
const GP: u64 = 0x003d_baf0;
/// The bike's wheels (front, rear) and their terrain object.
const WHEELS: [u32; 2] = [0x17b0, 0x17b4];
const TERRAIN: u32 = 0x00bb_d720;
/// The ground query (MD76), the collision record's terrain query, the record's per-frame pass, the
/// world search, and the terrain service's surface pick.
const GROUND_QUERY: u32 = 0x0048_33c0;
const UNDER: u32 = 0x0048_38f0;
const RECORD_PASS: u32 = 0x0020_51f0;
const SEARCH: u32 = 0x0011_51c0;
const SURFACE: u32 = 0x0012_7b20;
/// The bike's collision record object, and its ray probes: the two trigger rays, the wreck part, the
/// riding part (bike-relative pointers).
const RECORD_OBJECT: u32 = 0xc8;
const PROBES: [u32; 3] = [0x1430, 0x1434, 0x1438];

const READER: [(u32, u32); 7] =
    [(0x310, 8), (0x12a0, 0x18), (0x12c0, 8), (0x14c8, 4), (0x14d4, 4), (0x14ec, 4), (0x17ac, 4)];
const SPOT: [(u32, u32); 1] = [(0xd38, 0x38)];
const BUDGET: u64 = 400_000_000;

type Row = HashMap<String, String>;

struct Data {
    ee: Ee,
    image: std::path::PathBuf,
    trn: Vec<u8>,
}

fn data() -> Option<Data> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let image = kit.ctx("mxvatv-ps2").join(format!("state04.ee{}", ".bin"));
    let res = kit.ctx("mxvatv-ps2/nat05").join("nat05.res");
    for (p, what) in [
        (&image, "state 4's RAM image (F31's recipe: research/journal)"),
        (&res, "Nat05's nat05.res off MX vs. ATV's disc (Env/Nat05)"),
    ] {
        if !p.exists() {
            skip(&format!("{} is not there: {what}", p.display()));
            return None;
        }
    }
    let trn = ground::terrain_file(&std::fs::read(&res).unwrap(), "Nat05").expect("nat05.res holds Nat05.ps2.trn");
    Some(Data { ee: Ee::from_ram(&image).unwrap(), image, trn })
}

fn rows(name: &str) -> Option<Vec<Row>> {
    let path = Kit::load().ok()?.ctx("capture/c15").join(format!("{name}{}", ".csv"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        skip(&format!("no {}", path.display()));
        return None;
    };
    let mut lines = text.lines();
    let head: Vec<String> = lines.next()?.split(',').map(String::from).collect();
    Some(lines.map(|l| head.iter().cloned().zip(l.split(',').map(String::from)).collect()).collect())
}

fn hex_words(s: &str) -> Vec<u32> {
    (0..s.len() / 8)
        .map(|i| {
            let b = |j: usize| u8::from_str_radix(&s[8 * i + 2 * j..8 * i + 2 * j + 2], 16).unwrap();
            u32::from_le_bytes([b(0), b(1), b(2), b(3)])
        })
        .collect()
}

fn poke(ee: &mut Ee, row: &Row, words: &[(u32, u32)]) {
    let o: Vec<u32> = hex_words(&row["obj0"]).into_iter().chain(hex_words(&row["obj1"])).collect();
    for &(at, len) in words {
        for i in 0..len / 4 {
            ee.set_u32(BIKE + at + 4 * i, o[(at / 4 + i) as usize]);
        }
    }
}

/// `tests/mxvatv_bike_ride.rs`'s hooks: DIntr/EIntr, every syscall stub, the network send.
fn hooked(mut ee: Ee) -> Ee {
    ee.hooks.insert(0x0034_8fa0, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    ee.hooks.insert(0x0034_8ff0, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    ee.hooks.insert(0x004e_7e70, Box::new(|_: &mut Ee| {}));
    for a in (0x0030_0000..0x0036_0000u32).step_by(4) {
        if ee.u32_at(a) >> 16 == 0x2403 && ee.u32_at(a + 4) == 0x0000_000c && ee.u32_at(a + 8) == 0x03e0_0008 {
            let n = ee.u32_at(a) & 0xffff;
            ee.hooks.insert(a, Box::new(move |ee: &mut Ee| {
                let v = if n == 0x2f { 1 } else { ee.r(4) };
                ee.set(2, v);
            }));
        }
    }
    ee
}

/// Runs `at` as it is from inside its own hook, and gives back the call's arguments; `after` sees the
/// interpreter once the call is done.
fn pass_through(ee: &mut Ee, at: u32, mut before: impl FnMut(&Ee, &[u64; 8]) + 'static, mut after: impl FnMut(&Ee, &[u64; 8]) + 'static) {
    ee.hooks.insert(at, Box::new(move |ee: &mut Ee| {
        let args = [ee.r(4), ee.r(5), ee.r(6), ee.r(7), ee.r(8), ee.r(9), ee.r(10), ee.r(11)];
        let (ra, sp, stack) = (ee.r(31), ee.r(29), ee.stack);
        before(ee, &args);
        ee.stack = (sp as u32 - 0x40) & !0xf;
        ee.call(at, &args, &[], BUDGET);
        let v0 = ee.v0();
        (ee.stack, ee.pc) = (stack, at);
        ee.set(29, sp);
        ee.set(31, ra);
        ee.set(2, v0);
        after(ee, &args);
    }));
}

fn vf(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

fn bits(v: Vf) -> [u32; 4] {
    v.0.map(f32::to_bits)
}

/// The record's terrain query's buffer as the port's [`Ground`]: the point, the normal, the surface
/// word, the weights.
fn buffer(ee: &Ee, a: u32) -> Ground {
    Ground { point: vf(ee, a), normal: vf(ee, a + 0x10), surfaces: ee.u32_at(a + 0x20).to_le_bytes(), weights: vf(ee, a + 0x30) }
}

fn ground_bits(g: &Ground) -> ([u32; 4], [u32; 4], [u8; 4], [u32; 4]) {
    (bits(g.point), bits(g.normal), g.surfaces, bits(g.weights))
}

/// A wheel's ground query: which wheel, the point, the normal and the record byte the call had, and
/// what it left them.
#[derive(Clone, Debug)]
struct WheelQuery {
    row: usize,
    wheel: usize,
    before: (Vf, Vf, u8),
    after: (Vf, Vf, u8),
}

/// A terrain query of the collision record: its buffer before and after.
#[derive(Clone, Debug)]
struct RecordQuery {
    row: usize,
    before: Ground,
    after: Ground,
}

#[derive(Default)]
struct Log {
    row: usize,
    wheels: Vec<WheelQuery>,
    records: Vec<RecordQuery>,
    in_record: u32,
    /// Searches per probe: the trigger rays, the wreck part, the riding part.
    searches: [usize; 3],
    filed: Vec<String>,
}

/// Replays `rows` (or its first `n`) with the bike's ground queries and ray searches watched.
fn watched(d: &Data, name: &str, n: usize) -> Option<Log> {
    let rows = rows(name)?;
    let mut ee = hooked(Ee::from_ram(&d.image).unwrap());
    let log = Rc::new(RefCell::new(Log::default()));
    let wheels = WHEELS.map(|o| ee.u32_at(BIKE + o));
    let record_object = ee.u32_at(BIKE + RECORD_OBJECT);
    let probes = PROBES.map(|o| ee.u32_at(BIKE + o));

    let pending: Rc<RefCell<Vec<(Vf, Vf, u8)>>> = Rc::default();
    let (p, p2, l2) = (pending.clone(), pending.clone(), log.clone());
    pass_through(
        &mut ee,
        GROUND_QUERY,
        move |ee, a| {
            p.borrow_mut().push((vf(ee, a[1] as u32), vf(ee, a[2] as u32), ee.u8_at(a[4] as u32)));
        },
        move |ee, a| {
            let before = p2.borrow_mut().pop().unwrap();
            let Some(wheel) = wheels.iter().position(|&w| a[4] as u32 == w + WHEEL_RECORD) else { return };
            assert_eq!(a[0] as u32, ee.u32_at(wheels[wheel] + WHEEL_TERRAIN), "the wheel asks its own terrain");
            let after = (vf(ee, a[1] as u32), vf(ee, a[2] as u32), ee.u8_at(a[4] as u32));
            let mut l = l2.borrow_mut();
            let row = l.row;
            l.wheels.push(WheelQuery { row, wheel, before, after });
        },
    );

    let (l, l2) = (log.clone(), log.clone());
    pass_through(
        &mut ee,
        RECORD_PASS,
        move |_, a| {
            if a[0] as u32 == record_object {
                l.borrow_mut().in_record += 1;
            }
        },
        move |_, a| {
            if a[0] as u32 == record_object {
                l2.borrow_mut().in_record -= 1;
            }
        },
    );

    let pending: Rc<RefCell<Vec<Ground>>> = Rc::default();
    let (p, p2, l) = (pending.clone(), pending.clone(), log.clone());
    pass_through(
        &mut ee,
        UNDER,
        move |ee, a| p.borrow_mut().push(buffer(ee, a[1] as u32)),
        move |ee, a| {
            let before = p2.borrow_mut().pop().unwrap();
            let mut l = l.borrow_mut();
            if l.in_record > 0 {
                assert_eq!(a[0] as u32, TERRAIN, "the record asks the wheels' terrain");
                let row = l.row;
                l.records.push(RecordQuery { row, before, after: buffer(ee, a[1] as u32) });
            }
        },
    );

    let l = log.clone();
    pass_through(&mut ee, SEARCH, |_, _| {}, move |ee, a| {
        let probe = a[0] as u32;
        if !probes.contains(&probe) {
            return;
        }
        let record = ee.u32_at(probe + 0x7c);
        let mut l = l.borrow_mut();
        l.searches[probes.iter().position(|&q| q == probe).unwrap()] += 1;
        let count = ee.u32_at(record + 0x8c);
        if count != 0 {
            let row = l.row;
            l.filed.push(format!("row {row}: probe {probe:#x} filed {count}"));
        }
    });

    for (r, row) in rows.iter().enumerate().take(n) {
        log.borrow_mut().row = r;
        poke(&mut ee, row, &READER);
        ee.set(28, GP);
        ee.call(FRAME, &[u64::from(SIM)], &[f32::from_bits(DT)], BUDGET);
        poke(&mut ee, row, &SPOT);
    }
    drop(ee);
    Some(Rc::try_unwrap(log).ok().unwrap().into_inner())
}

/// Every logged wheel query, asked of the port: the same point, normal and record byte back.
fn check_wheels(map: &track::Terrain<'_>, qs: &[WheelQuery]) {
    let t = ground::terrain(map);
    for q in qs {
        let (at, normal, mut record) = q.before;
        let (p, n) = t.ground(at, normal, &mut record);
        let want = q.after;
        assert_eq!(
            (bits(p), bits(n), record),
            (bits(want.0), bits(want.1), want.2),
            "row {} {} wheel at {at:?}: the port's {p:?} {n:?} {record} against the original's {want:?}",
            q.row,
            ["front", "rear"][q.wheel]
        );
    }
}

fn check_records(map: &track::Terrain<'_>, qs: &[RecordQuery]) {
    let t = ground::terrain(map);
    for q in qs {
        let mut g = q.before;
        ground::under(&t, &mut g);
        assert_eq!(ground_bits(&g), ground_bits(&q.after), "row {}: the record's query at {:?}", q.row, q.before.point);
    }
}

/// State 4's terrain object, the one both wheels hold, is what the port builds from Nat05's file,
/// word for word.
#[test]
fn state4s_terrain_is_nat05s_map() {
    let Some(d) = data() else { return };
    let map = track::Terrain::parse(&d.trn).unwrap();
    for w in WHEELS {
        assert_eq!(d.ee.u32_at(d.ee.u32_at(BIKE + w) + WHEEL_TERRAIN), TERRAIN, "wheel +{w:#x}'s terrain");
    }
    let ours = ground::terrain(&map);
    for (at, word) in ground::terrain_words(&ours, TERRAIN) {
        assert_eq!(d.ee.u32_at(at), word, "terrain +{:#x}", at - TERRAIN);
    }
    let read = ground::read_terrain(&|a| d.ee.u32_at(a), TERRAIN, Some(&map));
    assert_eq!(ground::terrain_words(&read, TERRAIN), ground::terrain_words(&ours, TERRAIN));
}

/// The task's check: on ride-2's first frame, the port answers every wheel query as the original
/// does, and the wheels' searches file nothing.
#[test]
fn ride2s_first_frame_wheel_queries_are_the_ports() {
    let Some(d) = data() else { return };
    let map = track::Terrain::parse(&d.trn).unwrap();
    let Some(log) = watched(&d, "ride-2", 1) else { return };
    assert_eq!(log.wheels.len(), 8, "two wheels, four substeps");
    assert_eq!(log.wheels.iter().filter(|q| q.wheel == 0).count(), 4);
    check_wheels(&map, &log.wheels);
    assert_eq!(log.searches, [4, 0, 1], "the trigger rays' search each substep, the riding part's once");
    assert!(log.filed.is_empty(), "{:?}", log.filed);
    assert!(log.records.is_empty());
}

/// All of ride-2: every wheel query, and nothing filed by any of the bike's rays.
#[test]
fn every_wheel_query_of_ride2_is_the_ports() {
    let Some(d) = data() else { return };
    let map = track::Terrain::parse(&d.trn).unwrap();
    let Some(log) = watched(&d, "ride-2", usize::MAX) else { return };
    assert_eq!(log.wheels.len(), 459 * 8);
    check_wheels(&map, &log.wheels);
    assert!(log.filed.is_empty(), "{:?}", log.filed);
    check_records(&map, &log.records);
}

/// All of flip-1, wreck and put-back included: every wheel query and every terrain query of the
/// collision record (the wreck's rows ask it), and nothing filed by any of the bike's rays.
#[test]
fn every_ground_query_of_flip1_is_the_ports() {
    let Some(d) = data() else { return };
    let map = track::Terrain::parse(&d.trn).unwrap();
    let Some(log) = watched(&d, "flip-1", usize::MAX) else { return };
    assert!(!log.wheels.is_empty());
    check_wheels(&map, &log.wheels);
    assert!(log.filed.is_empty(), "{:?}", log.filed);
    assert!(!log.records.is_empty(), "the wreck's rows ask the record's terrain query");
    check_records(&map, &log.records);
    eprintln!("flip-1: {} wheel queries, {} record queries", log.wheels.len(), log.records.len());
}

/// The record's terrain query and the service's surface pick, called on the image at points over
/// all of Nat05 and past its edges, some with the terrain read coarse or split by parity.
#[test]
fn the_records_terrain_query_is_the_originals_over_nat05() {
    let Some(mut d) = data() else { return };
    let map = track::Terrain::parse(&d.trn).unwrap();
    let ee = &mut d.ee;
    const BUF: u32 = 0x01ff_0000;
    const MARK: u32 = 0x7f80_0123;
    let (coarse0, split0) = (ee.u32_at(TERRAIN + 0x8b8), ee.u32_at(TERRAIN + 0x770));
    let mut seed = 0x4633_3163_6772_6f75u64;
    let mut unit = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 40) as f32 / (1u64 << 24) as f32
    };
    let mut surfaces = std::collections::BTreeSet::new();
    for n in 0..6000 {
        let x = unit() * 6400.0 - 320.0;
        let z = unit() * 6400.0 - 320.0;
        let k = unit();
        let (coarse, split) = if k < 0.1 { (1, split0) } else if k < 0.2 { (coarse0, 0) } else { (coarse0, split0) };
        ee.set_u32(TERRAIN + 0x8b8, coarse);
        ee.set_u32(TERRAIN + 0x770, split);
        ee.set_u32(TERRAIN + 0x9e0, 0);
        for i in 0..16 {
            ee.set_u32(BUF + 4 * i, MARK);
        }
        ee.set_f32(BUF, x);
        ee.set_f32(BUF + 8, z);
        ee.set_f32(BUF + 12, 1.0);
        let before = buffer(ee, BUF);
        ee.set(28, GP);
        ee.call(UNDER, &[u64::from(TERRAIN), u64::from(BUF)], &[], 1_000_000);
        let want = buffer(ee, BUF);
        let t = ground::read_terrain(&|a| ee.u32_at(a), TERRAIN, Some(&map));
        let mut g = before;
        ground::under(&t, &mut g);
        assert_eq!(ground_bits(&g), ground_bits(&want), "point {n} ({x}, {z}) coarse {coarse} split {split}");
        if want.weights.x().to_bits() != MARK {
            ee.call(SURFACE, &[u64::from(BUF)], &[], 10_000);
            assert_eq!(u64::from(ground::surface(&g)), ee.v0() & 0xff, "point {n}: the surface");
            surfaces.insert(ground::surface(&g));
        }
    }
    assert!(surfaces.len() > 1, "Nat05 has more than one surface: {surfaces:?}");
}
