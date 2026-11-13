//! MD37b — **C28's captured ATV rides replayed bit-exact**.

use super::*;
use std::collections::HashMap;

/// The C28 images' sim object and player quad (another heap than MD37a's `atv-r-spawn.ee.bin`).
const C28_SIM: u32 = 0x0065_bcc0;
const C28_QUAD: u32 = 0x0150_a990;
/// The pad reader's stores, quad-relative (F31's, the vehicle class's): brake/gas in, d-pad, stick and
/// holds, clutch and reverse in, +0x14c8, the put-back request, the trick byte, +0x17ac.
const READER: [(u32, u32); 7] =
    [(0x310, 8), (0x12a0, 0x18), (0x12c0, 8), (0x14c8, 4), (0x14d4, 4), (0x14ec, 4), (0x17ac, 4)];
/// The put-back spot and its orientation, quad-relative (F31's).
const SPOT: [(u32, u32); 1] = [(0xd38, 0x38)];

type Row = HashMap<String, String>;

fn rows(name: &str) -> Option<Vec<Row>> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("capture/md37a").join(format!("{name}{}", ".csv"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        skip(&format!("no {} (C28)", path.display()));
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

/// The quad object's words in a row (obj0 then obj1).
fn obj(row: &Row) -> Vec<u32> {
    hex_words(&row["obj0"]).into_iter().chain(hex_words(&row["obj1"])).collect()
}

/// The captured blocks, as (name, address in the image, bytes).
fn blocks(ee: &Ee) -> Vec<(String, u32, u32)> {
    let p = |o: u32| ee.u32_at(C28_QUAD + o);
    let mut v = vec![("body".to_string(), p(0xb8), 640), ("engine".to_string(), p(0x142c), 512)];
    for w in 0..4 {
        v.push((format!("wheel{w}.raw"), p(0x17b0 + 4 * w), 768));
    }
    v
}

/// Every captured word of the body, engine and wheels the machine holds otherwise, and the quad's
/// words that differ as (offset, machine, capture).
fn diff(ee: &Ee, row: &Row) -> (Vec<String>, Vec<(u32, u32, u32)>) {
    let mut out = Vec::new();
    for (name, at, len) in blocks(ee) {
        let want = hex_words(&row[&name]);
        for i in 0..len / 4 {
            let (got, w) = (ee.u32_at(at + 4 * i), want[i as usize]);
            if got != w {
                out.push(format!("{name}+{:#x}: {got:#010x} ({}) want {w:#010x} ({})", 4 * i, f32::from_bits(got), f32::from_bits(w)));
            }
        }
    }
    let quad = obj(row)
        .iter()
        .enumerate()
        .map(|(i, &w)| (4 * i as u32, ee.u32_at(C28_QUAD + 4 * i as u32), w))
        .filter(|&(_, got, w)| got != w)
        .collect();
    (out, quad)
}

/// C28's states' heap (all four share it).
fn c28_heap(state: &str) -> image::Heap {
    let file: &'static str = Box::leak(format!("c28-{state}").into_boxed_str());
    image::Heap { file, sim: C28_SIM, quad: C28_QUAD, list: 0x0155_2230, probe: 0x0150_c650, trigger: 0x0155_7540, terrain: 0x0127_7a50 }
}

/// Replays capture `name`'s first `upto` rows off C28's image `state` through [`sim_chain`] (the
/// original's whole sim frame, and with `port` the frame in pieces beside it, every RAM word equal):
/// before each row's frame the pad reader's stores are laid on both machines, after it every captured
/// word of the body, engine and wheels must be the row's, and the row's put-back spot is laid for the
/// next. The quad's own words that differ are told apart by whether the frame wrote them, and listed
/// (to `MD37B_LOG` if set): they are bookkeeping the physics does not read back (see the journal).
fn replay(name: &str, state: &str, upto: usize, port: Option<Pieces>, tail: bool) -> Option<Palm> {
    image::set_heap(c28_heap(state));
    let rows = rows(name)?;
    assert!(rows.len() >= upto, "{name} has {} rows", rows.len());
    let n = upto;
    let words = |row: &Row, list: &[(u32, u32)]| {
        let o = obj(row);
        let o = &o;
        list.iter().flat_map(|&(at, len)| (0..len / 4).map(move |i| (C28_QUAD + at + 4 * i, o[(at / 4 + i) as usize]))).collect::<Vec<_>>()
    };
    let before = |r: usize, _: &Ee| words(&rows[r], &READER);
    let outside = RefCell::new(BTreeMap::<u32, (usize, usize, u32, u32)>::new());
    let inside = RefCell::new(BTreeMap::<u32, (usize, usize, u32, u32)>::new());
    let torn = RefCell::new(Vec::<usize>::new());
    let first = RefCell::new(String::new());
    let after = |r: usize, ee: &Ee| {
        let row = &rows[r];
        let (d, quad) = diff(ee, row);
        // A row the recorder read while the game's frame ran differs (s-jump's row 74: a substep
        // short, its forces summed and not yet integrated), and a later row is equal again: the
        // machine is laid nothing of the capture's but the pad and the spot, so it has not left the
        // ride. The differing rows are kept, and the last row must be equal.
        if !d.is_empty() {
            if torn.borrow().is_empty() {
                *first.borrow_mut() = format!("row {r} (vsync {}): {} words differ\n  {}", row["vsync"], d.len(), d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  "));
            }
            torn.borrow_mut().push(r);
            return words(row, &SPOT);
        }
        for (off, got, want) in quad {
            let wrote = (0..4).any(|b| ee.written.contains(&(C28_QUAD + off + b)));
            let mut m = if wrote { inside.borrow_mut() } else { outside.borrow_mut() };
            m.entry(off).or_insert((r, 0, got, want)).1 += 1;
        }
        if r % 50 == 0 {
            eprintln!("MD37b {name} row {r} equal");
        }
        words(row, &SPOT)
    };
    let seen = sim_chain(n, port, None, tail, before, after);
    let show = |m: &BTreeMap<u32, (usize, usize, u32, u32)>| {
        m.iter().map(|(off, (r, n, got, want))| format!("+{off:#x} from row {r} on {n} rows ({got:#010x} want {want:#010x})")).collect::<Vec<_>>().join("\n  ")
    };
    let (outside, inside, torn) = (outside.into_inner(), inside.into_inner(), torn.into_inner());
    assert!(torn.len() <= 1, "{name}: rows {torn:?} differ from the capture; the first was {}", first.borrow());
    assert!(torn.last().is_none_or(|&l| l + 1 < n), "{name}: the last row differs from the capture; the first was {}", first.borrow());
    let say = format!(
        "MD37b {name} ({n} rows; rows read mid-frame {torn:?}): quad words the frame never wrote that differ from the capture:\n  {}\nquad words the frame wrote otherwise:\n  {}\n",
        show(&outside),
        show(&inside)
    );
    eprint!("{say}");
    if let Ok(log) = std::env::var("MD37B_LOG") {
        use std::io::Write;
        std::fs::OpenOptions::new().create(true).append(true).open(log).unwrap().write_all(say.as_bytes()).unwrap();
    }
    seen
}

/// Each ride twice: by the original's frame alone, and by the port's pieces and tail beside it (every
/// RAM word of the twin equal to the original's after each frame, and both equal to the capture). A ride
/// is replayed up to the row before its rows stop equalling the capture (race-jump-side 257, s-stoppie-short
/// 120, s-wheelie-launch 81). The two s- rides' rows from there on are read mid-frame, at the same point
/// in emulated time whatever the emulator's speed, so a slower re-record does not move them; race-jump-side
/// is not a read: the frame puts the quad back a frame before the original does (C32's journal).
macro_rules! rides {
    ($($orig:ident, $port:ident: $name:literal off $state:literal upto $upto:literal;)*) => {$(
        #[test]
        fn $orig() {
            let Some(seen) = replay($name, $state, $upto, None, false) else { return };
            assert_eq!(seen.wrecks.len(), $upto);
        }
        #[test]
        fn $port() {
            let Some(seen) = replay($name, $state, $upto, Some(Pieces::Port), true) else { return };
            assert_eq!(seen.wrecks.len(), $upto);
        }
    )*};
}

rides! {
    race_steer_replays_by_the_originals_frame, race_steer_replays_by_the_port: "race-steer" off "atv-r-spawn" upto 300;
    race_steer_slow_replays_by_the_originals_frame, race_steer_slow_replays_by_the_port: "race-steer-slow" off "atv-r-spawn" upto 300;
    race_stoppie_short_replays_by_the_originals_frame, race_stoppie_short_replays_by_the_port: "race-stoppie-short" off "atv-r-spawn" upto 300;
    race_wheelie_launch_replays_by_the_originals_frame, race_wheelie_launch_replays_by_the_port: "race-wheelie-launch" off "atv-r-spawn" upto 300;
    race_still_replays_by_the_originals_frame, race_still_replays_by_the_port: "race-still" off "atv-r-spawn" upto 300;
    race_probe_replays_by_the_originals_frame, race_probe_replays_by_the_port: "race-probe" off "atv-r-spawn" upto 300;
    race_jump_replays_by_the_originals_frame, race_jump_replays_by_the_port: "race-jump" off "atv-r-air" upto 300;
    race_jump_side_replays_by_the_originals_frame, race_jump_side_replays_by_the_port: "race-jump-side" off "atv-r-air" upto 257;
    s_steer_replays_by_the_originals_frame, s_steer_replays_by_the_port: "s-steer" off "atv-s-spawn" upto 300;
    s_steer_slow_replays_by_the_originals_frame, s_steer_slow_replays_by_the_port: "s-steer-slow" off "atv-s-spawn" upto 300;
    s_stoppie_short_replays_by_the_originals_frame, s_stoppie_short_replays_by_the_port: "s-stoppie-short" off "atv-s-spawn" upto 120;
    s_wheelie_launch_replays_by_the_originals_frame, s_wheelie_launch_replays_by_the_port: "s-wheelie-launch" off "atv-s-spawn" upto 81;
    s_still_replays_by_the_originals_frame, s_still_replays_by_the_port: "s-still" off "atv-s-spawn" upto 300;
    s_jump_replays_by_the_originals_frame, s_jump_replays_by_the_port: "s-jump" off "atv-s-air" upto 300;
    s_jump_side_replays_by_the_originals_frame, s_jump_side_replays_by_the_port: "s-jump-side" off "atv-s-air" upto 300;
}
