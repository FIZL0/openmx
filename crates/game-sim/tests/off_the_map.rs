//! S59 — off the map: the bike's cannonball, proved against the original.
//!
//! The original's map edge is the first clause of the bike's wreck test, and the harness runs that
//! function itself over generated bikes. It lays out the bike, its chassis (with a vtable whose speed
//! limit slot is the game's own setter), the chassis' transform node, and the terrain the extent is
//! read off. Then it runs the test with the rider's main bone where the ride state machine hands it
//! over, and stops the run where the clause ends (a hook on the two addresses its paths leave by). The
//! port does the same to the same state with [`off_map::outside`] and [`off_map::throw`], and every
//! byte of the four objects, the generator's state and the wreck asked for are compared.
//!
//! The bike's air drag (`+0x6a8`) is halved by the original's throw and doubled again by its put-back,
//! with no simulation of the bike between the two (S71). The port keeps the halving as a flag on the
//! ride and [`off_map::drag`] as the value the ground step reads, so the harness writes that value
//! back into the bike to compare it with the rest, and [`the_put_back_doubles_the_drag_back`] runs the
//! original's put-back over the same.

use game_sim::biplane::Area;
use game_sim::chassis::Chassis;
use game_sim::off_map;
use game_sim::rng::Rng;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, addr, game_or_skip};
use std::cell::RefCell;
use std::rc::Rc;

const BIKE: u32 = 0x0120_0000;
const CHASSIS: u32 = 0x0120_2000;
const CHASSIS_VTABLE: u32 = 0x0120_3000;
const NODE: u32 = 0x0120_4000;
const TERRAIN_OBJ: u32 = 0x0120_5000;
const TERRAIN: u32 = 0x0120_6000;
const VTABLE: u32 = 0x0120_7000;
/// Where the bike's "the rider came off" slot points: a hook writes the reason down.
const CAME_OFF: u32 = 0x0120_8000;
const REGIONS: [(&str, u32, u32); 4] = [
    ("bike", BIKE, 0xd50),
    ("chassis", CHASSIS, 0x2a0),
    ("node", NODE, 0x120),
    ("terrain", TERRAIN, 0x540),
];

/// The clause's two ways out: after the wreck entry (and past every gate), and the "nothing outside"
/// branch. The rest of the wreck test is `tests/air.rs`'s.
const CLAUSE_END: [u32; 2] = [0x0025_25e4, 0x0025_25e8];
/// Where the test keeps its return address, off its own stack pointer.
const RA_SLOT: u32 = 80;

/// The bike's offsets the clause reads and writes.
mod bike {
    pub const TRANSFORM: u32 = 0x18;
    pub const VTABLE: u32 = 0x48;
    pub const ARMED: u32 = 0x570;
    pub const DRAG: u32 = 0x6a8;
    pub const REASON: u32 = 0x748;
    pub const CHASSIS: u32 = 0x7a4;
    pub const TERRAIN: u32 = 0x960;
    pub const MAIN_BONE: u32 = 0xd40;
}

/// The chassis' offsets: [`Chassis`]'s fields.
mod ch {
    pub const VTABLE: u32 = 0x08;
    pub const COFG: u32 = 0x60;
    pub const MAX_SPEED_SQ: u32 = 0x220;
    pub const NODE: u32 = 0x254;
    pub const VEL: u32 = 0x260;
    pub const ANG_VEL: u32 = 0x290;
}

/// A small generator of cases: xorshift64*.
struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn one_in(&mut self, n: u64) -> bool {
        self.next() % n == 0
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        let t = (self.next() >> 40) as f32 / (1u64 << 24) as f32;
        lo + (hi - lo) * t
    }
    fn vec(&mut self, lo: f32, hi: f32) -> Vf {
        Vf::new(self.range(lo, hi), self.range(lo, hi), self.range(lo, hi), self.range(lo, hi))
    }
}

fn put(ee: &mut Ee, a: u32, v: Vf) {
    for (k, x) in v.0.into_iter().enumerate() {
        ee.set_f32(a + 4 * k as u32, x);
    }
}

fn get(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

fn snapshot(ee: &Ee) -> Vec<(&'static str, u32, u32)> {
    REGIONS
        .iter()
        .flat_map(|&(name, base, len)| (0..len).step_by(4).map(move |o| (name, o, base)))
        .map(|(name, o, base)| (name, o, ee.u32_at(base + o)))
        .collect()
}

/// One coordinate across a map `extent` wide: off either edge, inside the margin of either, exactly on
/// a margin or the middle, or anywhere.
fn coord(extent: f32, pick: &mut Pick) -> f32 {
    match pick.next() % 5 {
        0 => pick.range(-300.0, 400.0),
        1 => extent + pick.range(-400.0, 300.0),
        2 => [150.0, extent - 150.0, 0.5 * extent, 149.99998][(pick.next() % 4) as usize],
        _ => pick.range(0.0, extent),
    }
}

#[test]
fn the_map_edge_is_the_originals() {
    let Some(mut ee) = game_or_skip() else { return };
    let test = addr("bike_wreck_test").unwrap();
    let flag = addr("wreck_enabled_flag").unwrap();
    let state = addr("rand_state").unwrap();
    let asked = Rc::new(RefCell::new(Vec::new()));
    let a = asked.clone();
    ee.hooks.insert(CAME_OFF, Box::new(move |ee| a.borrow_mut().push(ee.r(5) as u32)));
    for end in CLAUSE_END {
        ee.hooks.insert(end, Box::new(|ee| {
            let ra = ee.read(ee.r(29) as u32 + RA_SLOT, 8) as u64;
            ee.set(31, ra);
        }));
    }
    let nothing = Vf::new(0.0, 0.0, 0.0, 1.0);
    let mut pick = Pick(0x51e5_9c0f_f00d_2059);
    let (mut thrown, mut kept, mut by_bone) = (0, 0, 0);
    const CASES: u32 = 12_000;
    for case in 0..CASES {
        for &(_, base, len) in &REGIONS {
            for o in (0..len).step_by(4) {
                ee.set_u32(base + o, if pick.one_in(2) { 0 } else { pick.next() as u32 });
            }
        }
        ee.set_u32(BIKE + bike::VTABLE, VTABLE);
        ee.set_u32(VTABLE + 0xfc, CAME_OFF);
        ee.set_u32(BIKE + bike::CHASSIS, CHASSIS);
        ee.set_u32(BIKE + bike::TRANSFORM, NODE);
        ee.set_u32(BIKE + bike::TERRAIN, TERRAIN_OBJ);
        ee.set_u32(TERRAIN_OBJ + 0x3b4, TERRAIN);
        ee.set_u32(CHASSIS + ch::VTABLE, CHASSIS_VTABLE);
        ee.set_u32(CHASSIS_VTABLE + 0x80, addr("chassis_set_max_speed").unwrap());
        ee.set_u32(CHASSIS + ch::NODE, NODE);
        // The node's "moved" byte clear: its refresh is the renderer's, not the throw's.
        ee.set_u32(NODE + 0x110, 0);

        // The terrain: root cells and spacing, mostly whole numbers as every map has them.
        let counts = [1.0f32, 2.0, 3.0, 4.0];
        let ext: [f32; 4] = [
            if pick.one_in(8) { pick.range(0.5, 5.0) } else { counts[(pick.next() % 4) as usize] },
            if pick.one_in(8) { pick.range(0.5, 5.0) } else { counts[(pick.next() % 4) as usize] },
            if pick.one_in(4) { pick.range(1.0, 16.0) } else { 9.0 },
            if pick.one_in(4) { pick.range(1.0, 16.0) } else { 9.0 },
        ];
        for (k, x) in ext.iter().enumerate() {
            ee.set_f32(TERRAIN + 0x530 + 4 * k as u32, *x);
        }
        let area = Area::new(F(ext[0]), F(ext[1]), F(ext[2]), F(ext[3]));
        let (w, d) = (area.width.0, area.depth.0);

        let cofg = Vf::new(coord(w, &mut pick), pick.range(-100.0, 800.0), coord(d, &mut pick), pick.range(-2.0, 2.0));
        let cofg = if pick.one_in(300) { Vf::new(0.5 * w, cofg.y(), 0.5 * d, 1.0) } else { cofg };
        let bone = match pick.next() % 6 {
            0 => nothing,
            1 => Vf::new(f32::NAN, cofg.y(), cofg.z(), 1.0),
            2 => Vf::new(cofg.x() + pick.range(-3.0, 3.0), cofg.y() + 1.7, cofg.z() + pick.range(-3.0, 3.0), 1.0),
            _ => Vf::new(coord(w, &mut pick), pick.range(-100.0, 800.0), coord(d, &mut pick), 1.0),
        };
        let enabled = !pick.one_in(8);
        let armed = !pick.one_in(8);
        let reason = [0u32, 0, 0, 5, 13, 1][(pick.next() % 6) as usize];
        put(&mut ee, CHASSIS + ch::COFG, cofg);
        put(&mut ee, BIKE + bike::MAIN_BONE, bone);
        ee.set_u32(flag, u32::from(enabled));
        ee.set_u32(BIKE + bike::ARMED, if armed { (pick.next() as u32) | 1 } else { 0 });
        ee.set_u32(BIKE + bike::REASON, reason);
        // A rider who is up and stays inside goes on into the rest of the wreck test, which is not
        // this file's: those cases are given a reason so the test ends at the clause.
        let inside = !off_map::outside(&area, bone, cofg);
        if reason == 0 && inside {
            ee.set_u32(BIKE + bike::REASON, 5);
        }
        let seed = pick.next();
        ee.write(state, 8, u128::from(seed));

        let before = snapshot(&ee);
        let mut port = Chassis {
            cofg_world: get(&ee, CHASSIS + ch::COFG),
            max_speed_sq: F(ee.f32_at(CHASSIS + ch::MAX_SPEED_SQ)),
            vel: get(&ee, CHASSIS + ch::VEL),
            ang_vel: get(&ee, CHASSIS + ch::ANG_VEL),
            ..Chassis::default()
        };
        let mut origin = get(&ee, NODE + 0x70);
        let drag = ee.f32_at(BIKE + bike::DRAG);
        let mut rng = Rng::from_state(seed);

        asked.borrow_mut().clear();
        ee.call(test, &[BIKE as u64, (BIKE + bike::MAIN_BONE) as u64], &[F(1.0 / 30.0).0], 40_000);

        let gate = enabled && armed && reason != 13;
        let throws = gate && !inside;
        if throws {
            off_map::throw(&area, &mut port, &mut origin, &mut rng);
            thrown += 1;
            if !area.outside(cofg, off_map::MARGIN) {
                by_bone += 1;
            }
        } else {
            kept += 1;
        }
        let want: Vec<u32> = if throws { vec![13] } else { vec![] };
        assert_eq!(*asked.borrow(), want, "case {case}: the wreck asked for (cofg {cofg:?}, bone {bone:?})");
        assert_eq!(ee.read(state, 8) as u64, rng.state(), "case {case}: the generator");

        // The port's answer written over the state going in, then compared word for word with what
        // the original left.
        let after = snapshot(&ee);
        for &(name, o, v) in &before {
            let base = REGIONS.iter().find(|r| r.0 == name).unwrap().1;
            ee.set_u32(base + o, v);
        }
        // Only a throw writes anything (the rest of the state went in as raw words, which a `Vf` would
        // not carry through unchanged).
        if throws {
            put(&mut ee, CHASSIS + ch::VEL, port.vel);
            put(&mut ee, CHASSIS + ch::ANG_VEL, port.ang_vel);
            ee.set_f32(CHASSIS + ch::MAX_SPEED_SQ, port.max_speed_sq.0);
            for k in 0..3 {
                ee.set_f32(NODE + 0x70 + 4 * k, origin.0[k as usize]);
            }
            ee.set_f32(BIKE + bike::DRAG, off_map::drag(F(drag), true).0);
        }
        let ported = snapshot(&ee);
        for (o, p) in after.iter().zip(&ported) {
            assert_eq!(o.2, p.2, "case {case}: {} +{:#x}: original {:#010x}, port {:#010x}", o.0, o.1, o.2, p.2);
        }
    }
    println!("{CASES} cases: {thrown} thrown ({by_bone} by the bone alone), {kept} kept");
    assert!(thrown > 3000 && kept > 3000 && by_bone > 200, "{thrown} thrown, {by_bone} by the bone, {kept} kept");
}

/// The entry's half: three draws, `x`, `y`, `z`, units, and the `w` handed through.
#[test]
fn the_entry_spin_is_three_units() {
    let mut rng = Rng::from_state(0x1234_5678_9abc_def0);
    let mut copy = rng;
    let v = off_map::entry_spin(&mut rng, 0.25);
    assert_eq!(v.0, [copy.unit().0, copy.unit().0, copy.unit().0, 0.25]);
    assert_eq!(rng, copy);
}

/// **The put-back's half of the drag** (S71): the original's bike unwreck run over 12,000 bikes, each
/// with its drag where the port says it is — [`off_map::drag`] of a `.phy` drag, halved when the bike
/// is down as a cannonball (reason 13) and not otherwise — leaves the `.phy`'s drag, bit for bit, the
/// value the port reads once its flag is down. The drags are the stock bikes' kind, any bit pattern, and
/// the smallest normals, whose halving the PS2 flushes to zero: there the original's put-back leaves a
/// zero where the port has the drag back, and those are counted (no `.phy` has a drag that small), as are
/// the top-exponent patterns, which the doubling overflows and the FPU clamps (no `.phy` drag either). The
/// unwreck's other work is not this file's: its slot calls and the trick queue's reset are stubbed.
#[test]
fn the_put_back_doubles_the_drag_back() {
    let Some(mut ee) = game_or_skip() else { return };
    let unwreck = addr("bike_unwreck").unwrap();
    const WRECK: u32 = 0x0120_9000;
    const WRECK_VTABLE: u32 = 0x0120_a000;
    const STUB: u32 = 0x0120_b000;
    /// The trick queue's reset, which the unwreck runs on the bike's queue.
    const QUEUE_RESET: u32 = 0x0025_a4b0;
    ee.hooks.insert(STUB, Box::new(|_| {}));
    ee.hooks.insert(QUEUE_RESET, Box::new(|_| {}));
    let mut pick = Pick(0x5710_d7a6_0b1c_e5ed);
    let (mut thrown, mut flushed, mut clamped, mut other) = (0, 0, 0, 0);
    const CASES: u32 = 12_000;
    for case in 0..CASES {
        for &(_, base, len) in &REGIONS[..2] {
            for o in (0..len).step_by(4) {
                ee.set_u32(base + o, if pick.one_in(2) { 0 } else { pick.next() as u32 });
            }
        }
        ee.set_u32(BIKE + bike::VTABLE, VTABLE);
        for slot in [0x114, 0x118, 0x138] {
            ee.set_u32(VTABLE + slot, STUB);
        }
        ee.set_u32(BIKE + bike::CHASSIS, CHASSIS);
        ee.set_u32(BIKE + 0x7a8, WRECK);
        ee.set_u32(WRECK + 0x38, WRECK_VTABLE);
        ee.set_u32(WRECK_VTABLE + 0x120, STUB);

        let riding = match pick.next() % 4 {
            0 => F(pick.range(0.0, 0.2)),
            1 => F(f32::from_bits(pick.next() as u32)),
            2 => F(f32::from_bits(0x0080_0000 + (pick.next() as u32 % 0x0180_0000))),
            _ => F([0.025f32, 0.02, 0.05, 0.0][(pick.next() % 4) as usize]),
        };
        let reason = [13u32, 13, 13, 0, 5, 1][(pick.next() % 6) as usize];
        let halved = reason == 13;
        ee.set_u32(BIKE + bike::REASON, reason);
        ee.set_f32(BIKE + bike::DRAG, off_map::drag(riding, halved).0);
        ee.call(unwreck, &[BIKE as u64, 1], &[], 10_000);

        let original = ee.u32_at(BIKE + bike::DRAG);
        assert_eq!(ee.u32_at(BIKE + bike::REASON), 0, "case {case}: the reason cleared");
        let port = off_map::drag(riding, false).0.to_bits();
        if halved && off_map::drag(riding, true).0.abs() == 0.0 && riding.0.abs() != 0.0 {
            // The halving flushed: the original divides a zero.
            assert_eq!(original & 0x7fff_ffff, 0, "case {case}: {riding:?} flushed");
            assert!(riding.0.abs() < f32::from_bits(0x0100_0000), "case {case}: {riding:?} is no small normal");
            flushed += 1;
            continue;
        }
        if halved && riding.0.to_bits() & 0x7f80_0000 == 0x7f80_0000 {
            // The top exponent, which the PS2 reads as a number: halved it is one, doubled back it
            // overflows, and the FPU clamps it to the largest of its sign.
            assert_eq!(original, riding.0.to_bits() & 0x8000_0000 | 0x7f7f_ffff, "case {case}: {riding:?} clamped");
            clamped += 1;
            continue;
        }
        assert_eq!(original, port, "case {case}: reason {reason}, drag {riding:?} ({:#010x})", riding.0.to_bits());
        if halved { thrown += 1 } else { other += 1 }
    }
    println!(
        "{CASES} put-backs: {thrown} cannonballs doubled back, {other} others untouched, {flushed} flushed by the \
         halving, {clamped} top-exponent drags clamped by the doubling"
    );
    assert!(thrown > 4000 && other > 4000 && flushed > 50, "{thrown} doubled back, {other} untouched, {flushed} flushed");
}

/// One capture of `watch/s59-edge.toml`, by column name.
struct Cells {
    cap: game_sim::replay::Capture,
    at: std::collections::HashMap<String, usize>,
}

impl Cells {
    fn cell(&self, row: usize, col: &str) -> &str {
        &self.cap.rows[row][self.at[col]]
    }
    fn f(&self, row: usize, col: &str) -> F {
        let cell = self.cell(row, col);
        F(cell.parse::<f64>().unwrap_or_else(|_| panic!("{col} row {row}: '{cell}'")) as f32)
    }
    fn v(&self, row: usize, col: &str) -> Vf {
        let l = |c: &str| self.f(row, &format!("{col}.{c}")).0;
        Vf::new(l("x"), l("y"), l("z"), l("w"))
    }
    fn u(&self, row: usize, col: &str) -> u64 {
        self.cell(row, col).parse().unwrap_or_else(|_| panic!("{col} row {row}"))
    }
}

fn bits(v: Vf) -> [u32; 4] {
    v.0.map(f32::to_bits)
}

/// **The original riding off free09's four edges, and over one in the air** (`research/s59_edge.py`:
/// the human's bike steered over the pad from save state 7 until it is thrown, then 600 frames more;
/// `airborne` is the north edge again with the chassis kicked 30 ft/s upward 126 ft short of the
/// margin, so the bike crosses it 10 ft up and falling). The port is given the same inputs on every
/// tick from the edge to the put-back.
///
/// - **The edge.** The row before the throw is inside free09's extent (3 × 256 × 9 feet both ways)
///   by both points, and the throw's row is outside by the centre of gravity, which is the value the
///   throw read: the chassis is not stepped again until the put-back. Every one of the five was
///   thrown by the centre of gravity; the bone's own clause is proved by the generated cases only.
/// - **The throw**, bit for bit: [`off_map::throw`] from that centre of gravity gives the row's
///   velocity, the 350² limit and the spin, whose two draws are found in the generator's stream after
///   the previous row's state (the other actors draw in between), and the entry's three draws follow
///   them and are the wreck body's spin.
/// - **The state machine** ([`game_sim::state::Ride`]) through the reset: entered on the throw's
///   tick and stepped with the settle gate the rows show, its clock and latch are every row's, bit for
///   bit; it asks for the wreck page as the clock passes `CannonballWreckTime`, and the put-back's
///   tick reads the speed limit [`off_map::unwreck`] gives.
/// - **The page** ([`game_sim::wreck_page::Page`], S72): stepped with the machine from the tick it
///   was asked for on, it puts the bike back on its third update, and the machine's clock there —
///   which the put-back leaves alone — is the clock the put-back's row shows, bit for bit.
/// - **The drag** (S71): the bike's air drag is the riding one on the row before the throw, the
///   port's halved [`off_map::drag`] on every row from the throw's to the one before the put-back,
///   and the riding one again from the put-back's row, bit for bit.
///
/// Each capture missing is skipped, and the test with them.
#[test]
fn the_original_rides_off_each_edge() {
    use game_sim::state::{self, Kind, Owner, Reset, Sense};
    use game_sim::trick::Wreck;
    let dir = match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => std::path::PathBuf::from(dir).join("capture"),
        None => std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/capture"),
    }
    .join("s59");
    let area = Area::new(F(3.0), F(3.0), F(9.0), F(9.0));
    let mut ran = 0;
    for name in ["north", "east", "west", "south", "airborne"] {
        let path = dir.join(format!("{name}.csv"));
        let Ok(text) = std::fs::read_to_string(&path) else {
            println!("skipped {name}: no {} — research/s59_edge.py makes it", path.display());
            continue;
        };
        let cap = game_sim::replay::Capture::parse(&text).expect("the recorder's format");
        let at = cap.columns.iter().enumerate().map(|(i, c)| (c.clone(), i)).collect();
        let c = Cells { cap, at };
        let n = c.cap.rows.len();
        let reason = |r: usize| c.u(r, "reason");
        let t = (0..n).find(|&r| reason(r) == 13).unwrap_or_else(|| panic!("{name}: no cannonball"));
        let before = t - 1;
        assert_eq!(reason(before), 0, "{name}: riding before the throw");

        // The edge.
        let cofg = c.v(t, "cofg");
        assert!(!off_map::outside(&area, c.v(before, "main_bone"), c.v(before, "cofg")), "{name}: inside before");
        assert!(off_map::outside(&area, game_sim::r#move::NOTHING, cofg), "{name}: the cofg is outside");
        assert_eq!(bits(c.v(t + 1, "cofg")), bits(cofg), "{name}: the chassis stands still after");

        // The throw: the draws are searched for after the previous row's generator.
        let target = (c.v(t, "ang_vel").x().to_bits(), c.v(t, "ang_vel").y().to_bits());
        let mut found = None;
        let mut rng = Rng::from_state(c.u(before, "rng_state"));
        for k in 0..20_000 {
            let mut probe = rng;
            let mut ch = Chassis { cofg_world: cofg, ..Chassis::default() };
            let mut origin = Vf::new(0.0, 0.0, 0.0, 1.0);
            off_map::throw(&area, &mut ch, &mut origin, &mut probe);
            if (ch.ang_vel.x().to_bits(), ch.ang_vel.y().to_bits()) == target {
                found = Some((k, ch, origin, probe));
                break;
            }
            rng.rand();
        }
        let (k, ch, origin, mut after) = found.unwrap_or_else(|| panic!("{name}: the spin's draws are not in the stream"));
        assert_eq!(bits(ch.vel), bits(c.v(t, "vel")), "{name}: the throw's velocity");
        assert_eq!(bits(ch.ang_vel), bits(c.v(t, "ang_vel")), "{name}: the throw's spin");
        assert_eq!(ch.max_speed_sq.0.to_bits(), c.f(t, "max_speed_sq").0.to_bits(), "{name}: 350²");
        assert_eq!(bits(origin)[..3], bits(cofg)[..3], "{name}: the transform onto the cofg");
        let riding = c.f(before, "drag");
        assert_eq!(c.f(t, "drag").0.to_bits(), off_map::drag(riding, true).0.to_bits(), "{name}: the throw halves the drag");
        let spin = off_map::entry_spin(&mut after, 1.0);
        let body = c.v(t, "wreck_spin");
        assert_eq!(
            [spin.x(), spin.y(), spin.z()].map(f32::to_bits),
            [body.x(), body.y(), body.z()].map(f32::to_bits),
            "{name}: the entry's three draws are the body's spin"
        );

        // The state machine, from the throw to the put-back. The rows are samples of it, not its
        // ticks: the recorder's read drifts against the game's tick, so a row may repeat the one
        // before, skip one, or land mid-tick (west's throw row: the entry has cleared the clock and
        // the machine has not added to it yet, and its bone is the row before's). So the port's
        // states are made one a tick, after a first one for that mid-tick read, and each row must be
        // the state the last row was or one of the next two, in order. The clock only rises within a
        // wreck, but for the arming, which the latch tells apart, so no row can match two states.
        let p = state::Params {
            max_wreck_time: c.f(t, "max_wreck_time"),
            cannonball_wreck_time: c.f(t, "cannonball_wreck_time"),
        };
        let row = |r: usize| (c.f(r, "clock").0.to_bits(), c.u(r, "latch") != 0);
        let key = |m: &state::Ride| (m.elapsed.0.to_bits(), m.counting);
        let mut m = state::Ride::default();
        m.enter(Wreck(0));
        // One machine a tick, and the tick the page was asked for on. The settle gate is the one input
        // the rows cannot pin: the body writes it after the machine in the tick, so a row read at the
        // tick's end shows the value the next tick's machine reads, and a row read between the two
        // shows the one this tick's read. The rows before are tried first and this row's after.
        let mut ticks = vec![m];
        let mut page: Option<usize> = None;
        let step = |m: &mut state::Ride, j: usize, settles: bool, dt: F, paged: bool| {
            let sense = Sense {
                ai_route: false,
                owner: Some(Owner { human: true, screen: true }),
                wrecks_enabled: true,
                air_wrecks: true,
                settles,
            };
            m.step(dt, sense, p, |_| (j == 1).then_some(Wreck::CANNONBALL), || paged).reset
        };
        let (mut at, mut back, mut asked) = (0, None, None);
        let (mut repeated, mut skipped, mut late_gate) = (0, 0, 0);
        for r in t..n {
            if reason(r) == 0 {
                back = Some(r);
                break;
            }
            let found = (0..3).find_map(|d| {
                let mut ms = ticks[..=at].to_vec();
                let mut asks = page;
                for j in at + 1..=at + d {
                    let mut tick = ms[j - 1];
                    let ask = step(&mut tick, j, c.u(r - 1, "settles") != 0, c.f(r, "dt"), asks.is_some());
                    ms.push(tick);
                    asks = asks.or(ask.map(|a| {
                        assert_eq!(a, Reset::Page, "{name} tick {j}");
                        j
                    }));
                }
                if key(&ms[at + d]) == row(r) {
                    return Some((d, ms, asks, false));
                }
                // The gate as this row shows it, on the last tick.
                if d > 0 {
                    let mut tick = ms[at + d - 1];
                    let ask = step(&mut tick, at + d, c.u(r, "settles") != 0, c.f(r, "dt"), asks.is_some());
                    if key(&tick) == row(r) {
                        ms[at + d] = tick;
                        return Some((d, ms, asks.or(ask.map(|_| at + d)), true));
                    }
                }
                None
            });
            let (d, ms, asks, late) = found.unwrap_or_else(|| {
                panic!("{name} row {r}: {:?} is not the port's tick {at} {:?} or the two after", row(r), key(&ticks[at]))
            });
            match d {
                0 if r > t => repeated += 1,
                2 => skipped += 1,
                _ => {}
            }
            late_gate += usize::from(late);
            ticks = ms;
            page = asks;
            at += d;
            if asked.is_none() && page.is_some_and(|j| at >= j) {
                asked = Some(r);
            }
        }
        let mut m = ticks[at];
        let back = back.unwrap_or_else(|| panic!("{name}: not put back in the capture"));
        assert!(4 * (repeated + skipped) < back - t, "{name}: {repeated} rows repeated, {skipped} skipped one");
        assert!(late_gate <= 2, "{name}: the gate read off the row itself {late_gate} times");
        let asked = asked.unwrap_or_else(|| panic!("{name}: no reset asked for"));
        // The page, from the tick it was asked for on: the fader's first update is that tick's, and
        // the machine runs on (asking nothing, the page being up) until the draw that puts it back.
        let j = page.unwrap_or_else(|| panic!("{name}: no page"));
        let mut paged = ticks[j];
        let mut fader = game_sim::wreck_page::Page::default();
        fader.raise();
        let mut updates = 1;
        while !fader.update() {
            let ask = step(&mut paged, j + updates, true, c.f(back, "dt"), fader.up());
            assert_eq!(ask, None, "{name}: asked again with the page up");
            updates += 1;
        }
        assert_eq!(updates, 3, "{name}: the page's put-back is its third update");
        assert_eq!(paged.elapsed.0.to_bits(), c.f(back, "clock").0.to_bits(), "{name}: the clock the put-back left");
        assert_eq!(m.respawn(), Kind::Wrecked);
        let mut ch = Chassis { max_speed_sq: c.f(back - 1, "max_speed_sq"), ..Chassis::default() };
        assert_eq!(ch.max_speed_sq.0, 122500.0, "{name}: thrown until the put-back");
        off_map::unwreck(&mut ch);
        assert_eq!(ch.max_speed_sq.0.to_bits(), c.f(back, "max_speed_sq").0.to_bits(), "{name}: 135² again");
        for r in t..back {
            assert_eq!(c.f(r, "drag").0.to_bits(), off_map::drag(riding, true).0.to_bits(), "{name} row {r}: halved");
        }
        assert_eq!(c.f(back, "drag").0.to_bits(), off_map::drag(riding, false).0.to_bits(), "{name}: the drag doubled back");
        println!(
            "{name}: thrown at frame {} by the cofg {:?} ({k} draws in); {at} ticks to the page's put-back \
             (the throw's is tick 1), latched on tick {}, the page asked for on tick {}, and the put-back {} rows after it \
             ({repeated} rows repeated the last, {skipped} skipped one, {late_gate} armed on the row's own gate)",
            c.u(t, "frame_counter"),
            cofg.0,
            ticks.iter().position(|m| m.counting).unwrap_or(0),
            page.unwrap_or(0),
            back - asked,
        );
        ran += 1;
    }
    if ran == 0 {
        println!("skipped: no S59 captures");
    }
}

/// A wreck body and the rider's half of a wreck, plausible numbers in the files' shape (tests/ride.rs'
/// own, not the disc's).
const WRECK_PART: &str = "
[PhysicsPart1]
Type = RigidBody
Name = wreck_physics
Weight = 250.0
CollisionWeight = 0.0001
GravityScale = 1.0
InertiaScale = 1.0, 1.0, 1.0
SpringStiffness = 5.0
SpringDamping = 0.4
MaxPenetration = 0.5
CofG = 0.0, 1.5, 0.0
SpringMomentScale = 1.0, 1.0, 1.0
FrictionMomentScale = 1.0, 1.0, 1.0
FrictionZone1 = 0.5
Radius = 1.0
AccelerationThreshold = 0.01
AngularAccelerationThreshold = 0.01
";
const RIDER_WRECKS: &str = "
[Wrecks]
HitBottomForceTime = 0.1
HitBottomHandHoldTime = 0.3
HitBottomForce = 0.0, 800.0, 0.0
GroundAngularRateDamping = 1.0
RiderFlailHeight = 3.0
RiderRollTime = 0.75
RiderRollSpeed = 0.0
";

/// A flat floor with free09's extent: [`Flat`] answers no area, so the edge never runs on it.
struct Edged(game_sim::world::Flat);

impl game_sim::world::World for Edged {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        self.0.ground(at)
    }
    fn solid(&mut self, bundle: &game_sim::collide::Bundle, record: &mut game_sim::collide::Record) -> bool {
        self.0.solid(bundle, record)
    }
    fn area(&self) -> Option<Area> {
        Some(Area::new(F(3.0), F(3.0), F(9.0), F(9.0)))
    }
}

/// **The clause is wired into the rider.** A bike set down past the margin is thrown on its first
/// frame as a cannonball: toward the map's middle, with the thrown speed limit. The reset chord
/// cannot cut it short, and a put-back gives the chassis the riding limit again, as the bike's own does.
#[test]
fn a_rider_past_the_margin_is_thrown_back() {
    use game_sim::phy::Phy;
    use game_sim::ride::Controls;
    use game_sim::rider::{self, Rider};
    use game_sim::trick::{Set, Wreck};

    let tables = rider::Tables::build();
    let p = Phy::read_bytes(include_str!("freeride-bike.phy.txt").as_bytes(), &tables.ride.curve);
    let at = Vf::new(3456.0, 1.0, 100.0, 1.0);
    let mut r = Rider::new(&p, game_sim::stock::MOUNTS, at, Vf::new(0.0, 0.0, -1.0, 0.0), Set::default());
    // A wreck body, so that the clock can put the bike back: tests/ride.rs' made-up one.
    let part = game_sim::phy::Part::read_bytes(WRECK_PART.as_bytes(), "PhysicsPart1");
    let wrecks = game_sim::phy::Wrecks::read_bytes(RIDER_WRECKS.as_bytes());
    r.carries_wreck(part, wrecks, part.inertia(Vf::new(2.5, 4.0, 7.0, 1.0)));
    let mut world = Edged(game_sim::world::Flat { y: F::ZERO, normal: Vf::new(0.0, 1.0, 0.0, 0.0) });
    let dt = F(1.0 / 60.0);

    let f = r.frame(&p, &tables, &Controls::default(), dt, &mut world);
    assert_eq!(f.wreck, Some(Wreck::CANNONBALL));
    assert_eq!(r.ride.chassis.max_speed_sq.0.to_bits(), (off_map::THROWN_MAX_SPEED * off_map::THROWN_MAX_SPEED).0.to_bits());
    let v = r.ride.chassis.vel;
    assert!(v.z() > 0.0 && v.y() > 0.0, "thrown {v:?}: not toward the middle, nor up");
    assert!(r.ride.drag_halved, "the throw halves the bike's drag");

    // The chord does not cut a cannonball short: only the clock puts this bike back.
    for k in 1..120 {
        let hands = Controls { respawn: k % 10 == 0, ..Controls::default() };
        assert!(!r.frame(&p, &tables, &hands, dt, &mut world).respawned, "the chord cut the cannonball short at {k}");
        assert!(r.ride.drag_halved, "the drag doubled back while down, at {k}");
    }
    // The clock's put-back doubles it back.
    let mut k = 0;
    while !r.frame(&p, &tables, &Controls::default(), dt, &mut world).respawned {
        k += 1;
        assert!(k < 3000, "no put-back in 50 s");
    }
    assert!(!r.ride.drag_halved, "the put-back left the drag halved");

    // The put-back gives the chassis the riding speed limit again, whatever it was: here a chord on
    // a rider who is riding, with the thrown limit left on the chassis.
    let mut r = Rider::new(&p, game_sim::stock::MOUNTS, Vf::new(3456.0, 1.0, 3456.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    r.ride.chassis.max_speed_sq = off_map::THROWN_MAX_SPEED * off_map::THROWN_MAX_SPEED;
    let hands = Controls { respawn: true, ..Controls::default() };
    assert!(r.frame(&p, &tables, &hands, dt, &mut world).respawned, "the chord never put the bike back");
    let riding = off_map::RIDING_MAX_SPEED * off_map::RIDING_MAX_SPEED;
    assert_eq!(r.ride.chassis.max_speed_sq.0.to_bits(), riding.0.to_bits());
}

/// **The put-back after a shootoff lands where the bike came down** (S77): the wreck bike keeps the
/// put-back record while it touches the ground, as the original's cannonball branch does, so the
/// clock's put-back sets the bike down on the last spot the wreck bike touched — far inside the map,
/// not back at the edge it was thrown from.
#[test]
fn the_put_back_after_a_cannonball_lands_where_the_bike_came_down() {
    use game_sim::phy::Phy;
    use game_sim::ride::Controls;
    use game_sim::rider::{self, Rider};
    use game_sim::trick::{Set, Wreck};

    let tables = rider::Tables::build();
    let p = Phy::read_bytes(include_str!("freeride-bike.phy.txt").as_bytes(), &tables.ride.curve);
    let edge = Vf::new(3456.0, 1.0, 100.0, 1.0);
    let mut r = Rider::new(&p, game_sim::stock::MOUNTS, edge, Vf::new(0.0, 0.0, -1.0, 0.0), Set::default());
    let part = game_sim::phy::Part::read_bytes(WRECK_PART.as_bytes(), "PhysicsPart1");
    let wrecks = game_sim::phy::Wrecks::read_bytes(RIDER_WRECKS.as_bytes());
    r.carries_wreck(part, wrecks, part.inertia(Vf::new(2.5, 4.0, 7.0, 1.0)));
    let mut world = Edged(game_sim::world::Flat { y: F::ZERO, normal: Vf::new(0.0, 1.0, 0.0, 0.0) });
    let dt = F(1.0 / 60.0);

    assert_eq!(r.frame(&p, &tables, &Controls::default(), dt, &mut world).wreck, Some(Wreck::CANNONBALL));
    let kept_at_the_edge = r.ride.back;
    let (mut landed, mut touched) = (None, 0);
    let mut k = 0;
    while !r.frame(&p, &tables, &Controls::default(), dt, &mut world).respawned {
        let down = r.crash.as_ref().expect("down until the put-back");
        if down.bike.record.touching {
            landed = Some(down.bike.record.node.origin);
            touched += 1;
            assert_eq!(r.ride.back.at.x().to_bits(), down.bike.record.node.origin.x().to_bits(), "the record off the wreck bike at {k}");
            assert_eq!(r.ride.back.at.z().to_bits(), down.bike.record.node.origin.z().to_bits(), "the record off the wreck bike at {k}");
        } else if touched == 0 {
            assert_eq!(r.ride.back, kept_at_the_edge, "the record moved in the air at {k}");
        }
        k += 1;
        assert!(k < 3000, "no put-back in 50 s");
    }
    let landed = landed.expect("the wreck bike never came down");
    let at = r.ride.position();
    assert_eq!((at.x().to_bits(), at.z().to_bits()), (landed.x().to_bits(), landed.z().to_bits()), "put back off the landing");
    assert!(at.z() - edge.z() > 100.0, "put back at {at:?}, beside the edge it left ({edge:?})");
    println!("thrown from z {}, came down {touched} ticks, put back at {at:?}", edge.z());
}
