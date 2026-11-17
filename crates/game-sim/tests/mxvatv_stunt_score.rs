//! R17 — what MX vs. ATV scores its backflip on, run in SLUS-21104's own code.
//!
//! The scorer is the stunt start and end themselves. The start pushes a node onto the bike's score card
//! (`+0x680`, the manager `+0x684`) with its base and rate a second out of the rider's stunt table, and
//! holds it growing; the end finishes it; the stunt idle's end with id -1, after 0.2 s on the ground,
//! banks the combo at its sum times its count; the wreck's cancel loses it. Run here on state04's real
//! bike, card and table (`context/mxvatv-ps2/state04.ee.bin`), nothing hooked but the kernel's syscall
//! stubs. Without the image every test skips (and says so).
//!
//! Nothing in the port is proved here: it still scores MX Unleashed's way (`game_sim::score`). These
//! tests pin the original's numbers for the port that follows (journal 2026-11-16-r17-*).

use ps2_float::{F, fpu};
use ps2_oracle::{Ee, Kit, skip};

/// State 4's bike, its vtable, and the calls run.
const BIKE: u32 = 0x00e7_14e0;
const BIKE_VTABLE: u32 = 0x003d_0450;
const START_STUNT: u32 = 0x004e_6370;
const END_STUNT: u32 = 0x004e_4db0;
const CANCEL_STUNT: u32 = 0x004e_4e50;
const SCORE_GROW: u32 = 0x0046_b210;
const WRECK_ENTRY: u32 = 0x004e_2260;
/// The 30 Hz tick the frame runs on (F31: these bits, not 1/30).
const DT: u32 = 0x3d08_850a;

fn image() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2").join(format!("state04.ee{}", ".bin"));
    if !path.exists() {
        skip(&format!("{} is not there: unzip eeMemory.bin from MX vs. ATV's save state 4 to it", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    if ee.u32_at(BIKE + 4) != BIKE_VTABLE || ee.u32_at(BIKE_VTABLE + 0x368) != WRECK_ENTRY {
        panic!("{} is not the MX vs. ATV image these addresses are from", path.display());
    }
    // Every `addiu $3, $zero, n; syscall; jr $ra`: the answer is a0, the thread id (0x2f) 1.
    for a in (0x0030_0000..0x0036_0000u32).step_by(4) {
        if ee.u32_at(a) >> 16 == 0x2403 && ee.u32_at(a + 4) == 0x0000_000c && ee.u32_at(a + 8) == 0x03e0_0008 {
            let n = ee.u32_at(a) & 0xffff;
            ee.hooks.insert(a, Box::new(move |ee: &mut Ee| {
                let v = if n == 0x2f { 1 } else { ee.r(4) };
                ee.set(2, v);
            }));
        }
    }
    Some(ee)
}

fn card(ee: &Ee) -> u32 {
    ee.u32_at(BIKE + 0x680)
}
fn manager(ee: &Ee) -> u32 {
    ee.u32_at(BIKE + 0x684)
}

/// A node as the card holds it: flags, id, points, base, rate, mult, extra, held, granularity.
#[derive(Debug, PartialEq)]
struct Node {
    flags: u32,
    id: u32,
    points: u32,
    base: u32,
    rate: u32,
    mult: u32,
    extra: u32,
    held: u32,
    granularity: u32,
}

fn nodes(ee: &Ee) -> Vec<Node> {
    let mut out = vec![];
    let mut n = ee.u32_at(card(ee) + 0xc8);
    while n != 0 {
        let w = |o: u32| ee.u32_at(n + o);
        out.push(Node { flags: w(0), id: w(4), points: w(8), base: w(0xc), rate: w(0x10), mult: w(0x14), extra: w(0x18), held: w(0x1c), granularity: w(0x20) });
        n = w(0x24);
        assert!(out.len() < 100, "the node list runs on");
    }
    out
}

fn start(ee: &mut Ee, id: u32) {
    ee.call(START_STUNT, &[u64::from(BIKE), u64::from(id), 1], &[], 100_000);
    assert_eq!(ee.r(2) as u32, 1, "the stunt start answered no");
}
fn end(ee: &mut Ee, id: i64) {
    ee.call(END_STUNT, &[u64::from(BIKE), id as u64], &[], 100_000);
}
fn grow(ee: &mut Ee, ticks: u32) {
    for _ in 0..ticks {
        let m = manager(ee);
        ee.call(SCORE_GROW, &[u64::from(m)], &[f32::from_bits(DT)], 100_000);
    }
}

/// The growth's sum: `adda 0, base; madd held, rate`, then `* mult`, then down to the granularity
/// (flags 0xc: round down).
fn grown(base: f32, rate: f32, ticks: u32) -> u32 {
    let mut held = F(0.0);
    let mut p = F(0.0);
    for _ in 0..ticks {
        held = held + F::from_bits(DT);
        p = F(fpu::madd(fpu::adda(0.0, base), held.0, rate)) * F(1.0);
        p = F(fpu::from_i32((p / F(1.0)).to_i32())) * F(1.0);
    }
    p.0.to_bits()
}

/// The table's numbers, and the five flip-family ids state 4's bike carries.
#[test]
fn the_backflip_reads_its_points_off_the_stunt_table() {
    let Some(ee) = image() else { return };
    // +0x13b8 the backflip, +0x13bc/+0x13c0 the two others the backflip step's family takes, +0x13c4 the
    // wheelie, +0x13c8 the stoppie (the update starts those two off a wheel's second alone).
    let ids: Vec<u32> = (0..5).map(|k| ee.u32_at(BIKE + 0x13b8 + 4 * k)).collect();
    assert_eq!(ids, [41, 44, 45, 42, 43]);
    let set = ee.u32_at(BIKE + 0x14f0);
    let entry = |id: u32| set + 0x18 + 12 * id;
    let def = |id: u32| ee.u32_at(entry(id));
    let at = |id: u32, o: u32| ee.f32_at(def(id) + o);
    for id in 0..41 {
        assert_eq!((at(id, 0x50), at(id, 0x54), ee.f32_at(entry(id) + 4)), (1000.0, 1000.0, 1.0), "trick {id}");
    }
    assert_eq!((at(41, 0x50), at(41, 0x54)), (2000.0, 10000.0), "the backflip");
    assert_eq!((at(42, 0x50), at(42, 0x54), at(43, 0x50), at(43, 0x54)), (1000.0, 1000.0, 1000.0, 1000.0));
    assert_eq!((at(44, 0x50), at(44, 0x54), at(45, 0x50), at(45, 0x54)), (2000.0, 15000.0, 2000.0, 15000.0));
    assert_eq!(ee.f32_at(manager(&ee) + 0x2a8), 1.0, "the total's granularity");
}

/// One backflip, alone: its node opens at the start at 2000, grows 10 000 a second, is finished at the
/// end, and the bank adds it times the combo's count (1).
#[test]
fn a_backflip_scores_its_held_time_and_banks_on_the_end() {
    let Some(mut ee) = image() else { return };
    let total0 = ee.f32_at(card(&ee) + 0xdc);
    start(&mut ee, 41);
    let c = card(&ee);
    assert_eq!(ee.u32_at(c + 8) & 0x60, 0x60, "combo open, a node scoring");
    let one = 1.0f32.to_bits();
    assert_eq!(nodes(&ee), [Node {
        flags: 0x3d, id: 41, points: 2000.0f32.to_bits(), base: 2000.0f32.to_bits(), rate: 10000.0f32.to_bits(),
        mult: one, extra: 0, held: 0, granularity: one,
    }]);
    let mut ticks = 0;
    for more in [1, 13, 1] {
        grow(&mut ee, more);
        ticks += more;
        assert_eq!(nodes(&ee)[0].points, grown(2000.0, 10000.0, ticks), "after {ticks} ticks");
    }
    let held = nodes(&ee)[0].points;
    assert_eq!(f32::from_bits(held), 6999.0, "half a second's flip: the held time falls short of 0.5 and the floor takes it to 6999");
    end(&mut ee, 41);
    let n = &nodes(&ee)[0];
    assert_eq!((n.flags, n.points), (0x0d, held), "finished: live and growing bits off, x1, floored");
    grow(&mut ee, 5);
    assert_eq!(nodes(&ee)[0].points, held, "a finished node does not grow");
    end(&mut ee, -1);
    assert_eq!(ee.f32_at(c + 0xdc), total0 + f32::from_bits(held), "banked x1");
    assert_eq!(ee.u32_at(c + 8) & 0x60, 0, "combo closed");
    assert!(nodes(&ee).is_empty(), "nodes back to the pool");
    // The bank's watcher lowers the repeat scale of each stunt it banked (the table's +8, 0.8 on
    // state04, no lower than its +0xc): the next backflip opens at 1600 and grows 8000 a second.
    let scale = ee.u32_at(BIKE + 0x14f0) + 0x18 + 12 * 41 + 4;
    assert_eq!(ee.f32_at(scale), 0.8);
    start(&mut ee, 41);
    let n = &nodes(&ee)[0];
    assert_eq!((f32::from_bits(n.base), f32::from_bits(n.rate)), (2000.0 * 0.8, 10000.0 * 0.8), "a repeated backflip");
}

/// A backflip and a wheelie in one combo: the bank adds their sum times two.
#[test]
fn a_combo_banks_its_sum_times_its_count() {
    let Some(mut ee) = image() else { return };
    let total0 = ee.f32_at(card(&ee) + 0xdc);
    start(&mut ee, 41);
    grow(&mut ee, 30);
    end(&mut ee, 41);
    start(&mut ee, 42);
    grow(&mut ee, 30);
    end(&mut ee, 42);
    let p: Vec<f32> = nodes(&ee).iter().map(|n| f32::from_bits(n.points)).collect();
    assert_eq!(p, [f32::from_bits(grown(1000.0, 1000.0, 30)), f32::from_bits(grown(2000.0, 10000.0, 30))]);
    end(&mut ee, -1);
    assert_eq!(ee.f32_at(card(&ee) + 0xdc), total0 + (p[0] + p[1]) * 2.0);
}

/// The wreck's cancel (bike vt +0x4b0, id -1) loses the open combo: nothing banked.
#[test]
fn a_wreck_loses_the_backflip() {
    let Some(mut ee) = image() else { return };
    let total0 = ee.f32_at(card(&ee) + 0xdc);
    start(&mut ee, 41);
    grow(&mut ee, 20);
    ee.call(CANCEL_STUNT, &[u64::from(BIKE), u64::MAX], &[], 100_000);
    assert_eq!(ee.f32_at(card(&ee) + 0xdc), total0);
    assert_eq!(ee.u32_at(card(&ee) + 8) & 0x60, 0, "combo closed");
}
