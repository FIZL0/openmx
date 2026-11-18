//! R17 — what MX vs. ATV scores its backflip on, run in SLUS-21104's own code.
//!
//! The scorer is the stunt start and end themselves. The start pushes a node onto the bike's score card
//! (`+0x680`, the manager `+0x684`) with its base and rate a second out of the rider's stunt table, and
//! holds it growing; the end finishes it; the stunt idle's end with id -1, after 0.2 s on the ground,
//! banks the combo at its sum times its count; the wreck's cancel loses it. Run here on state04's real
//! bike, card and table (`context/mxvatv-ps2/state04.ee.bin`), nothing hooked but the kernel's syscall
//! stubs. Without the image every test skips (and says so).
//!
//! The first four pin the original's numbers (journal 2026-11-16-r17-*). The port that scores a trick
//! pack's ride this way (S358, `game_sim::score::Score::atv_*`) is run beside the original on a
//! generated sequence of the same calls: `the_port_scores_stunts_as_the_original`.

use game_sim::score::{self, Atv, Score, Values};
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
const MARK: u32 = 0x0046_b110;
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
#[derive(Clone, Copy, Debug, PartialEq)]
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

/// The port's id for one of state 4's: its tricks 0–23 are the rider's 24 slots, and its five kept
/// stunts the port's backflip, wheelie, stoppie and 360s.
fn port_id(id: u32) -> u8 {
    match id {
        41 => score::BACKFLIP,
        42 => score::WHEELIE,
        43 => score::STOPPIE,
        44 => score::THREE_SIXTY_LEFT,
        45 => score::THREE_SIXTY_RIGHT,
        _ => id as u8,
    }
}

/// S358: the port's MX vs. ATV card against the original's, on 12 000 generated calls in runs of
/// up to 40 — stunt starts (tricks 0–23 and the five kept, held or not), ends, marks, the manager's
/// tick (state 4's dt or any up to 0.1 s), the bank and the wreck's cancel — compared after each: every
/// node's words, the combo and running flags, the total, the combo's points, and every stunt's repeat
/// scale; then one backflip banked to the repeat floor. The bike is put in the air (`+0x324` 0) so a trick's start scores, as it does in a jump.
#[test]
fn the_port_scores_stunts_as_the_original() {
    let Some(mut ee) = image() else { return };
    ee.set_u32(BIKE + 0x324, 0);
    let set = ee.u32_at(BIKE + 0x14f0);
    let entry = |id: u32| set + 0x18 + 12 * id;
    let row = |ee: &Ee, id: u32| {
        let def = ee.u32_at(entry(id));
        Values { point: F(ee.f32_at(def + 0x50)), hold: F(ee.f32_at(def + 0x54)) }
    };
    let ids: Vec<u32> = (0..24).chain(41..46).collect();
    let mut port = Score { total: F(ee.f32_at(card(&ee) + 0xdc)), ..Score::default() };
    let mut atv = Atv::default();
    let mut seed = 0x5358_u64;
    let mut pick = |n: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed % n
    };
    let mut seen = [0u32; 7];
    for step in 0..12_000u32 {
        let id = ids[pick(ids.len() as u64) as usize];
        let what = pick(if step % 40 == 39 { 1 } else { 100 });
        let kind = match what {
            0 if pick(4) == 0 => 6,
            0..=1 => 5,
            2..=24 => 0,
            25..=39 => 1,
            40..=49 => 2,
            _ => 3 + u64::from(pick(2) == 0),
        };
        seen[kind as usize] += 1;
        match kind {
            0 => {
                let held = pick(2);
                ee.call(START_STUNT, &[u64::from(BIKE), u64::from(id), held], &[], 100_000);
                port.atv_start(&atv, port_id(id), row(&ee, id), held == 1);
            }
            1 => {
                end(&mut ee, i64::from(id));
                port.atv_end(port_id(id));
            }
            2 => {
                let on = pick(2);
                ee.call(MARK, &[u64::from(manager(&ee)), u64::from(card(&ee)), on, u64::from(id), 0], &[], 100_000);
                port.atv_mark(port_id(id), on == 1);
            }
            3 | 4 => {
                let dt = if kind == 3 { f32::from_bits(DT) } else { (pick(1 << 24) as f32 / (1 << 24) as f32) * 0.1 };
                let m = manager(&ee);
                ee.call(SCORE_GROW, &[u64::from(m)], &[dt], 100_000);
                port.atv_grow(F(dt));
            }
            5 => {
                end(&mut ee, -1);
                port.atv_bank(&mut atv);
            }
            _ => {
                ee.call(CANCEL_STUNT, &[u64::from(BIKE), u64::MAX], &[], 100_000);
                port.atv_cancel(&mut atv);
            }
        }
        let c = card(&ee);
        let flags = ee.u32_at(c + 8);
        let got: Vec<Node> = port.nodes.iter().map(|n| Node {
            flags: n.flags, id: u32::from(n.id), points: n.points.to_bits(), base: n.base.to_bits(), rate: n.rate.to_bits(),
            mult: n.mult.to_bits(), extra: n.extra.to_bits(), held: n.held.to_bits(), granularity: n.granularity.to_bits(),
        }).collect();
        let want: Vec<Node> = nodes(&ee).into_iter().map(|n| Node { id: u32::from(port_id(n.id)), ..n }).collect();
        let at = format!("step {step} (call {kind}, stunt {id})");
        assert_eq!(got, want, "{at}: the nodes");
        assert_eq!((port.combo, port.scoring), (flags & 0x20 != 0, flags & 0x40 != 0), "{at}: combo open, a node running");
        assert_eq!(port.total.to_bits(), ee.u32_at(c + 0xdc), "{at}: the total");
        assert_eq!(port.combo_points.to_bits(), ee.u32_at(c + 0xe0), "{at}: the combo's points");
        for &k in &ids {
            assert_eq!(Score::atv_scale(&atv, port_id(k)).to_bits(), ee.u32_at(entry(k) + 4), "{at}: stunt {k}'s repeat scale");
        }
    }
    assert!(seen.iter().all(|&n| n > 50), "every call taken: {seen:?}");
    // The floor: one backflip banked alone ten times, which the mix above never reaches.
    for round in 0..10 {
        ee.call(START_STUNT, &[u64::from(BIKE), 41, 1], &[], 100_000);
        port.atv_start(&atv, score::BACKFLIP, row(&ee, 41), true);
        end(&mut ee, -1);
        port.atv_bank(&mut atv);
        let at = format!("backflip banked {round}");
        assert_eq!(port.total.to_bits(), ee.u32_at(card(&ee) + 0xdc), "{at}: the total");
        for &k in &ids {
            assert_eq!(Score::atv_scale(&atv, port_id(k)).to_bits(), ee.u32_at(entry(k) + 4), "{at}: stunt {k}'s repeat scale");
        }
    }
    assert_eq!(ee.u32_at(entry(41) + 4), score::ATV_REPEAT_FLOOR.to_bits(), "the backflip at the repeat floor");
}

/// One row of an S411 capture (`research/s411/card.toml`): the bike's words the scorer reads, and
/// the card as the original left the frame.
struct Ride {
    vsync: u32,
    /// The rows' words, to tell a row read late (the next frame's, so the same as the next row).
    words: String,
    ground: bool,
    /// `+0x14f4`: the bank's clock.
    bank: u32,
    /// `+0x1754` the backflip's clip, `+0x1758` the 360's.
    clip: u32,
    spin: u32,
    /// The node's up and forward rows' heights (node `+0x84`, `+0x94`).
    heights: (F, F),
    /// A backflip's (`+0x1194`..) or 360's (`+0x11d8`) move on.
    move_on: bool,
    /// The backflip's torque on (`+0x1194`).
    flip_torque: bool,
    wrecks: u32,
    flags: u32,
    total: u32,
    combo: u32,
    nodes: Vec<Node>,
}

/// An S411 capture, or `None` (and a skip) without it.
fn ride(name: &str) -> Option<Vec<Ride>> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("capture").join("s411").join(format!("{name}.csv"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        skip(&format!("{} is not there: capture it with research/s411/play.sh", path.display()));
        return None;
    };
    let hex = |s: &str| -> Vec<u8> { (0..s.len() / 2).map(|k| u8::from_str_radix(&s[2 * k..2 * k + 2], 16).unwrap()).collect() };
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap().split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name).unwrap_or_else(|| panic!("no {name} column"));
    let (vs, o0, o1, nd, cd, pl) = (col("vsync"), col("obj0"), col("obj1"), col("node"), col("card"), col("pool"));
    let w = |b: &[u8], o: u32| u32::from_le_bytes(b[o as usize..o as usize + 4].try_into().unwrap());
    let rows = lines.map(|l| {
        let c: Vec<&str> = l.split(',').collect();
        let obj = [hex(c[o0]), hex(c[o1])].concat();
        let (node, card, pool) = (hex(c[nd]), hex(c[cd]), hex(c[pl]));
        let mut nodes = vec![];
        let mut n = w(&card, 0xc8);
        while n != 0 {
            let at = |o: u32| w(&pool, n - 0x012a_d6c0 + o);
            nodes.push(Node { flags: at(0), id: at(4), points: at(8), base: at(0xc), rate: at(0x10), mult: at(0x14), extra: at(0x18), held: at(0x1c), granularity: at(0x20) });
            n = at(0x24);
            assert!(nodes.len() < 100, "the node list runs on");
        }
        Ride {
            vsync: c[vs].parse().unwrap(),
            words: [c[o0], c[o1], c[nd], c[cd], c[pl]].concat(),
            ground: w(&obj, 0x324) != 0,
            bank: w(&obj, 0x14f4),
            clip: w(&obj, 0x1754),
            spin: w(&obj, 0x1758),
            heights: (F::from_bits(w(&node, 0x84)), F::from_bits(w(&node, 0x94))),
            move_on: (0..4).any(|k| w(&obj, 0x1194 + 4 * k) != 0) || w(&obj, 0x11d8) != 0,
            flip_torque: w(&obj, 0x1194) != 0,
            wrecks: w(&obj, 0x6e4),
            flags: w(&card, 8),
            total: w(&card, 0xdc),
            combo: w(&card, 0xe0),
            nodes,
        }
    });
    Some(rows.collect())
}

/// The port's card run over a captured ride, row by row, as the rider runs it on a pack ride: the
/// 360's node off its clip, then the frame (the bank's clock and the backflip's watch), then the
/// manager's growth; and compared on every row with the original's card and bank clock. A frame
/// the recorder missed is run on the next row's words, and must change none of them; a row read late
/// holds the next frame (the next row's words exactly), and the next row is no frame on.
fn ride_the_port(name: &str, rows: &[Ride]) {
    let mut frame: Vec<u32> = rows.iter().map(|r| r.vsync / 2).collect();
    for k in (0..rows.len() - 1).rev() {
        if rows[k].words == rows[k + 1].words {
            frame[k] = frame[k + 1];
        }
    }
    let mut port = Score { total: F::from_bits(rows[0].total), ..Score::default() };
    let mut atv = Atv::default();
    let mut front = 0;
    let mut hang = F::ZERO;
    let dt = F::from_bits(DT);
    for k in 1..rows.len() {
        let (was, r) = (&rows[k - 1], &rows[k]);
        assert_eq!(r.wrecks, rows[0].wrecks, "{name}: no wreck in these rides");
        let ticks = frame[k] - frame[k - 1];
        if ticks > 1 {
            let same = |r: &Ride| (r.ground, r.clip, r.spin, r.move_on, r.flags & 0x60, r.nodes.len());
            assert_eq!(same(was), same(r), "{name}: the recorder missed a frame that changed something at {}", r.vsync);
        }
        for _ in 0..ticks {
            let id = score::THREE_SIXTY_RIGHT;
            let live = port.nodes.iter().any(|n| n.id == id && n.flags & score::atv_flag::LIVE != 0);
            if r.spin != 0 && !live {
                port.atv_start(&atv, id, score::ATV_THREE_SIXTY, true);
            } else if r.spin == 0 && live {
                port.atv_end(id);
            }
            hang = if r.ground { F::ZERO } else { hang + dt };
            let s = score::Sense { front: r.ground, rear: r.ground, airborne: !r.ground, hang_time: hang, up_y: r.heights.0, forward_y: r.heights.1, ..Default::default() };
            let rows = score::AtvRows {
                backflip: score::ATV_BACKFLIP,
                wheelie: score::ATV_WHEEL,
                stoppie: score::ATV_WHEEL,
                trick_running: false,
                heights: was.heights,
                move_on: r.move_on,
                flip_torque: r.flip_torque,
                front_torque: false,
            };
            port.atv_frame(&mut atv, dt, &s, rows, &mut front);
            port.atv_grow(dt);
        }
        let at = format!("{name} at {} (row {k})", r.vsync);
        let got: Vec<Node> = port.nodes.iter().map(|n| Node {
            flags: n.flags, id: u32::from(n.id), points: n.points.to_bits(), base: n.base.to_bits(), rate: n.rate.to_bits(),
            mult: n.mult.to_bits(), extra: n.extra.to_bits(), held: n.held.to_bits(), granularity: n.granularity.to_bits(),
        }).collect();
        let want: Vec<Node> = r.nodes.iter().map(|n| Node { id: u32::from(port_id(n.id)), ..*n }).collect();
        assert_eq!(got, want, "{at}: the nodes");
        assert_eq!((port.combo, port.scoring), (r.flags & 0x20 != 0, r.flags & 0x40 != 0), "{at}: combo open, a node running");
        assert_eq!(port.total.to_bits(), r.total, "{at}: the total");
        assert_eq!(port.combo_points.to_bits(), r.combo, "{at}: the combo's points");
        assert_eq!(atv.bank.to_bits(), r.bank, "{at}: the bank's clock");
    }
}

/// The first row at or after `from` where `f` holds.
fn first(rows: &[Ride], from: usize, f: impl Fn(&Ride) -> bool) -> usize {
    (from..rows.len()).find(|&k| f(&rows[k])).expect("the ride gets there")
}

/// S411: when the original's backflip calls come on a ride — a backflip off state 4's session's
/// first lip that comes round, lands and banks (`flip2`: `research/s411/flip.pad`). The clip starts
/// (the stunt start: node 41 open and grown one frame), goes over, and comes round (the stunt end:
/// node finished) each a frame after the node's heights pass the rule — the up row's height at or
/// under 0 with the forward row's over 0; both under 0; the up row's over 0 with the forward row's
/// over −0.79 — so the backflip reads the frame before. The bank's clock is 0.2 from the frame
/// after the start, through the stunt word held past the clip until the move ends on the ground,
/// counts down from the first frame down, and banks the sixth frame after the word goes. Then the
/// port's card, run on the same rows, matches the original's on every row.
#[test]
fn the_backflip_calls_come_when_the_originals_do() {
    let Some(rows) = ride("flip2") else { return };
    let start = first(&rows, 1, |r| r.clip == 1);
    let over = first(&rows, start, |r| r.clip == 2);
    let round = first(&rows, over, |r| r.clip == 0);
    for k in [start, over, round] {
        assert_eq!(rows[k].vsync - rows[k - 2].vsync, 4, "the rows either side of {k} are all there");
    }
    let h = |k: usize| rows[k].heights;
    assert!(h(start - 1).0 <= F::ZERO && F::ZERO < h(start - 1).1 && F::ZERO < h(start - 2).0, "the start: {:?} {:?}", h(start - 2), h(start - 1));
    assert!(h(over - 1).1 < F::ZERO && F::ZERO <= h(over - 2).1, "over: {:?} {:?}", h(over - 2), h(over - 1));
    assert!(F::ZERO < h(round - 1).0 && F(-0.79) < h(round - 1).1 && h(round - 2).1 <= F(-0.79), "round: {:?} {:?}", h(round - 2), h(round - 1));
    let n = &rows[start].nodes;
    assert_eq!((n.len(), n[0].id, n[0].flags, f32::from_bits(n[0].points)), (1, 41, 0x3d, 2333.0), "opened at the start");
    assert_eq!(rows[round].nodes[0].flags, 0x0d, "finished on the clip's end");
    assert!(!rows[round].ground, "in the air");
    // The bank's clock: 0 on the start, 0.2 the frame after, counting from the first frame down.
    assert_eq!((rows[start].bank, rows[start + 1].bank), (0, 0x3e4c_cccd));
    let down = first(&rows, round, |r| r.ground);
    let gone = first(&rows, down, |r| !r.move_on);
    let banked = first(&rows, down, |r| r.flags & 0x20 == 0);
    assert_eq!((gone - down, banked - gone), (1, 6), "the move ends a frame down, the bank six frames on");
    assert_eq!(rows[banked].total, rows[round].nodes[0].points, "banked x1");
    ride_the_port("flip2", &rows);
}

/// S411: the 360's held start on the original's ride (`spin3`: `research/s411/spin3.pad`, a right
/// 360 off the same lip that comes round and banks). Its node opens held on the frame the 360's clip
/// leaves 0 (grown one frame), and is finished on the frame it comes back to 0, in the air; the bank's
/// clock holds through the move and banks as the backflip's does. The port's card, run on the same
/// rows with its 360 node off the clip, matches the original's on every row.
#[test]
fn the_three_sixty_starts_held_on_its_clip() {
    let Some(rows) = ride("spin3") else { return };
    let start = first(&rows, 1, |r| r.spin != 0);
    let round = first(&rows, start, |r| r.spin == 0);
    let n = &rows[start].nodes;
    assert_eq!((n.len(), n[0].id, n[0].flags, f32::from_bits(n[0].points)), (1, 45, 0x3d, 2499.0), "opened held at the start");
    assert!(rows[start - 1].nodes.is_empty(), "and not before");
    assert_eq!(rows[round].nodes[0].flags, 0x0d, "finished on the clip's end");
    assert!(!rows[round].ground, "in the air");
    let down = first(&rows, round, |r| r.ground);
    let gone = first(&rows, down, |r| !r.move_on);
    let banked = first(&rows, down, |r| r.flags & 0x20 == 0);
    assert_eq!((gone - down, banked - gone), (1, 6), "the move ends a frame down, the bank six frames on");
    ride_the_port("spin3", &rows);
}
