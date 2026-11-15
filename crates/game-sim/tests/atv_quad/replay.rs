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

/// Whether the ride's rows were read after the game wrote the put-back spot for the next frame. The
/// spot is written outside the sim frame, off the body (on the ground under it), between two frames,
/// and the recorder reads either side of that: each race- ride's every row holds the spot written off
/// its own row's body, each s- ride's the spot off the row before's (the one its frame read). Told by
/// which body a moving spot sits nearer, over the ride's rows; a still quad's rows can go either way,
/// so the most rows decide.
fn spot_after(rows: &[Row]) -> bool {
    let at = |row: &Row, block: &str, o: usize| {
        let w = if block == "obj" { obj(row) } else { hex_words(&row[block]) };
        (f32::from_bits(w[o / 4]), f32::from_bits(w[o / 4 + 2]))
    };
    let (mut after, mut before) = (0, 0);
    for k in 1..rows.len() {
        let spot = at(&rows[k], "obj", 0xd40);
        if spot == at(&rows[k - 1], "obj", 0xd40) {
            continue;
        }
        let d = |b: (f32, f32)| (b.0 - spot.0).powi(2) + (b.1 - spot.1).powi(2);
        if d(at(&rows[k], "body", 0x70)) < d(at(&rows[k - 1], "body", 0x70)) {
            after += 1;
        } else {
            before += 1;
        }
    }
    after > before
}

/// The captured blocks' bytes as they are now, by address.
fn block_bytes(ee: &Ee) -> HashMap<u32, u8> {
    blocks(ee).into_iter().flat_map(|(_, at, len)| (at..at + len).map(|a| (a, ee.u8_at(a))).collect::<Vec<_>>()).collect()
}

/// A7's mid-frame read: the instant inside the frame just run where every captured byte of the body,
/// engine and wheels was the row's, found by laying the frame's writes ([`Ee::log`]) one by one over
/// the blocks as they were before it (`pre`). Answers how many of the frame's block writes had landed
/// and the pc of the last, at the first such instant (0 writes: the row was read before the frame).
fn mid_frame(ee: &Ee, row: &Row, pre: &HashMap<u32, u8>) -> Option<(usize, usize, u32)> {
    let mut want = HashMap::new();
    for (name, at, _) in blocks(ee) {
        for (i, w) in hex_words(&row[&name]).into_iter().enumerate() {
            for (b, x) in w.to_le_bytes().into_iter().enumerate() {
                want.insert(at + 4 * i as u32 + b as u32, x);
            }
        }
    }
    let mut now = pre.clone();
    let mut off = now.iter().filter(|(a, x)| want[a] != **x).count();
    let log = ee.log.borrow();
    let writes: Vec<_> = log.as_ref().expect("the write log is on").iter().filter(|(_, a, _)| want.contains_key(a)).collect();
    if off == 0 {
        return Some((0, writes.len(), 0));
    }
    for (k, &&(pc, a, x)) in writes.iter().enumerate() {
        let was = now.insert(a, x).unwrap();
        off = off + usize::from(x != want[&a]) - usize::from(was != want[&a]);
        if off == 0 {
            return Some((k + 1, writes.len(), pc));
        }
    }
    None
}

/// C28's states' heap (all four share it).
fn c28_heap(state: &str) -> image::Heap {
    let file: &'static str = Box::leak(format!("c28-{state}").into_boxed_str());
    image::Heap { file, sim: C28_SIM, quad: C28_QUAD, list: 0x0155_2230, probe: 0x0150_c650, trigger: 0x0155_7540, terrain: 0x0127_7a50 }
}

/// Replays capture `name`'s first `upto` rows off C28's image `state` through [`sim_chain`] (the
/// original's whole sim frame, and with `port` the frame in pieces beside it, every RAM word equal):
/// before each row's frame the pad reader's stores and the put-back spot that frame reads
/// ([`spot_after`]) are laid on both machines, and after it every captured word of the body, engine and
/// wheels must be the row's, at the frame's end or at an instant inside it ([`mid_frame`]). The quad's own words that differ are told apart by whether the frame wrote them, and listed
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
    let after_spot = spot_after(&rows);
    let pre = RefCell::new(HashMap::new());
    let before = |r: usize, ee: &Ee| {
        *pre.borrow_mut() = block_bytes(ee);
        *ee.log.borrow_mut() = Some(Vec::new());
        // The spot frame r reads is row r-1's when the rows are read after the game writes it, row r's
        // when before (s-wheelie-launch's put-back on row 158 lands on row 158's spot).
        let spot = if after_spot { r.checked_sub(1).map_or(Vec::new(), |q| words(&rows[q], &SPOT)) } else { words(&rows[r], &SPOT) };
        [words(&rows[r], &READER), spot].concat()
    };
    let mid = RefCell::new(Vec::<(usize, usize, usize, u32)>::new());
    let outside = RefCell::new(BTreeMap::<u32, (usize, usize, u32, u32)>::new());
    let inside = RefCell::new(BTreeMap::<u32, (usize, usize, u32, u32)>::new());
    let torn = RefCell::new(Vec::<usize>::new());
    let first = RefCell::new(String::new());
    let after = |r: usize, ee: &Ee| {
        let row = &rows[r];
        let (d, quad) = diff(ee, row);
        // A row the recorder read while the game's frame ran differs at the frame's end (s-jump's
        // row 74: a substep short, its forces summed and not yet integrated), and must equal the
        // machine at an instant inside the frame. PINE reads at a fixed point in emulated time, so a
        // slower re-record reads the same rows mid-frame (C32).
        if !d.is_empty() {
            if let Some((k, of, pc)) = mid_frame(ee, row, &pre.borrow()) {
                mid.borrow_mut().push((r, k, of, pc));
                return Vec::new();
            }
            if torn.borrow().is_empty() {
                *first.borrow_mut() = format!("row {r} (vsync {}): {} words differ\n  {}", row["vsync"], d.len(), d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  "));
            }
            torn.borrow_mut().push(r);
            return Vec::new();
        }
        for (off, got, want) in quad {
            let wrote = (0..4).any(|b| ee.written.contains(&(C28_QUAD + off + b)));
            let mut m = if wrote { inside.borrow_mut() } else { outside.borrow_mut() };
            m.entry(off).or_insert((r, 0, got, want)).1 += 1;
        }
        if r % 50 == 0 {
            eprintln!("MD37b {name} row {r} equal");
        }
        Vec::new()
    };
    let seen = sim_chain(n, port, None, tail, before, after);
    let show = |m: &BTreeMap<u32, (usize, usize, u32, u32)>| {
        m.iter().map(|(off, (r, n, got, want))| format!("+{off:#x} from row {r} on {n} rows ({got:#010x} want {want:#010x})")).collect::<Vec<_>>().join("\n  ")
    };
    let (outside, inside, torn, mid) = (outside.into_inner(), inside.into_inner(), torn.into_inner(), mid.into_inner());
    assert!(torn.is_empty(), "{name}: rows {torn:?} differ from the capture at every instant of their frames; the first was {}", first.borrow());
    let say = format!(
        "MD37b {name} ({n} rows; rows read mid-frame (row: block writes landed of the frame's, at pc): {}; rows matching no instant {torn:?}): quad words the frame never wrote that differ from the capture:\n  {}\nquad words the frame wrote otherwise:\n  {}\n",
        mid.iter().map(|(r, k, of, pc)| format!("{r}: {k}/{of} {pc:#x}")).collect::<Vec<_>>().join(", "),
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
/// RAM word of the twin equal to the original's after each frame, and both equal to the capture). Every
/// ride is replayed whole but race-jump-side, to 257: there the frame puts the quad back a frame before
/// the original does, which is not a read (C32's journal, MD148). s-stoppie-short reads rows 124-133,
/// 153 and 175-299 but a few mid-frame, s-wheelie-launch 81-83, s-jump 74 (A7's journal).
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
    s_stoppie_short_replays_by_the_originals_frame, s_stoppie_short_replays_by_the_port: "s-stoppie-short" off "atv-s-spawn" upto 300;
    s_wheelie_launch_replays_by_the_originals_frame, s_wheelie_launch_replays_by_the_port: "s-wheelie-launch" off "atv-s-spawn" upto 300;
    s_still_replays_by_the_originals_frame, s_still_replays_by_the_port: "s-still" off "atv-s-spawn" upto 300;
    s_jump_replays_by_the_originals_frame, s_jump_replays_by_the_port: "s-jump" off "atv-s-air" upto 300;
    s_jump_side_replays_by_the_originals_frame, s_jump_side_replays_by_the_port: "s-jump-side" off "atv-s-air" upto 300;
}

/// MD133b — **an MX Unleashed-sized landing and a face, by the original's frame and the port's**:
/// race-jump off its air image, its pad, and on row 47 (a row before its own landing) the body's
/// velocity laid at the shell's free09 landing (the horizontal × 1.55, about 66 ft/s, and 40 ft/s
/// down), every row by the original's whole frame and the port's pieces and tail beside it, every RAM
/// word equal. The original lands it flat with a vertical load of 69 (under the `.phy`'s 80 mph ×
/// 1.46666) and the fore-aft small, and rides on; at 67 ft/s it meets the next face and wrecks front,
/// the fore-aft −33 past the front limit 12.5 mph × 1.46666, as race-jump's own capture wrecks front
/// on that face at 50 ft/s (−23.2). So the shell's quad wrecking front where it lands on free09's next
/// face at 65 ft/s (−20.5) is the original's wreck, not the port's loads (the journal).
#[test]
fn an_mxu_sized_landing_rides_out_and_its_face_wrecks_front_by_the_original_and_the_port() {
    const LAID: usize = 47;
    const FRAMES: usize = 62;
    image::set_heap(c28_heap("atv-r-air"));
    let Some(rows) = rows("race-jump") else { return };
    let pad = |row: &Row| {
        let o = obj(row);
        READER.iter().flat_map(|&(at, len)| (0..len / 4).map(|i| (C28_QUAD + at + 4 * i, o[(at / 4 + i) as usize])).collect::<Vec<_>>()).collect::<Vec<_>>()
    };
    let before = |r: usize, ee: &Ee| {
        let mut lay = pad(&rows[r]);
        if r == LAID {
            let body = ee.u32_at(C28_QUAD + 0xb8);
            let v = get_v(ee, body + 0x80);
            for (k, x) in [v.x() * 1.55, -40.0, v.z() * 1.55].into_iter().enumerate() {
                lay.push((body + 0x80 + 4 * k as u32, x.to_bits()));
            }
        }
        lay
    };
    // Each row: the body's velocity, a wheel down, the loads (+0x1250 side, vertical, fore and aft),
    // the reason, and the clauses' front, side and freestyle-down limits.
    let seen = RefCell::new(Vec::<(Vf, bool, Vf, u32, [f32; 3])>::new());
    let after = |_: usize, ee: &Ee| {
        let f = |o: u32| f32::from_bits(ee.u32_at(C28_QUAD + o));
        let body = ee.u32_at(C28_QUAD + 0xb8);
        let row = (get_v(ee, body + 0x80), ee.u32_at(C28_QUAD + 0x324) != 0, get_v(ee, C28_QUAD + 0x1250), ee.u32_at(C28_QUAD + 0xc80), [f(0x13e8), f(0x13f0), f(0x13f4)]);
        seen.borrow_mut().push(row);
        Vec::new()
    };
    let Some(_) = sim_chain(FRAMES, Some(Pieces::Port), None, true, before, after) else { return };
    let seen = seen.into_inner();
    let say = seen
        .iter()
        .enumerate()
        .skip(LAID)
        .map(|(r, (v, down, l, reason, _))| format!("{r}: vel {:.1} {:.1} {:.1} down {down} loads {:.2} {:.2} {:.2} reason {reason}", v.x(), v.y(), v.z(), l.x(), l.y(), l.z()))
        .collect::<Vec<_>>()
        .join("\n");
    eprintln!("MD133b\n{say}");
    let mph = |x: f32| (F(x) * game_sim::phy::MPH).0;
    let speed = |v: Vf| (v.x() * v.x() + v.y() * v.y() + v.z() * v.z()).sqrt();
    let [front, side, down] = seen[0].4;
    assert_eq!((front, side, down), (mph(12.5), mph(10.0), mph(80.0)), "the limits are the .phy's in ft/s");
    let landing = seen.iter().position(|s| s.1).expect("it lands");
    assert!(landing > LAID && seen[landing - 1].0.y() < -40.0 && seen[landing - 1].0.x() > 60.0, "lands at the shell's speeds:\n{say}");
    let face = seen.iter().position(|s| s.3 != 0).unwrap_or_else(|| panic!("no wreck:\n{say}"));
    let peak = seen[landing..face].iter().map(|s| s.2.y()).fold(0.0, f32::max);
    assert!(peak > 60.0 && peak <= down, "the landing's vertical peak {peak}:\n{say}");
    assert!(seen[landing..face].iter().all(|s| s.2.x().abs() <= side && s.2.z() >= -front), "the landing's loads within the limits:\n{say}");
    assert!(face >= landing + 8, "the landing rides out before the face:\n{say}");
    let (v, _, loads, reason, _) = seen[face];
    assert_eq!(reason, 1, "the face wrecks front:\n{say}");
    assert!(loads.z() < -front && speed(seen[face - 1].0) > 60.0, "fore-aft {} past −{front} at {} ft/s:\n{say}", loads.z(), speed(v));
}
