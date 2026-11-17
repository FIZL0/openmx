//! MD37b — **C28's captured ATV rides replayed bit-exact**.

use super::*;
use std::collections::HashMap;

/// The C28 images' sim object and player quad (another heap than MD37a's `atv-r-spawn.ee.bin`).
const C28_SIM: u32 = 0x0065_bcc0;
const C28_QUAD: u32 = 0x0150_a990;
/// The pad reader's stores, quad-relative (F31's, the vehicle class's): brake/gas in, d-pad, stick and
/// holds and the stick's angle and length, clutch and reverse in, +0x14c8, the put-back request, the
/// trick byte, +0x17ac.
const READER: [(u32, u32); 7] =
    [(0x310, 8), (0x12a0, 0x20), (0x12c0, 8), (0x14c8, 4), (0x14d4, 4), (0x14ec, 4), (0x17ac, 4)];
/// The put-back spot and its orientation, quad-relative (F31's).
const SPOT: [(u32, u32); 1] = [(0xd38, 0x38)];
/// The rider's update copies its ragdoll into the quad every frame (MD148): a spin vector at
/// +0x870, a body's velocity at +0x880, bone positions and orientations at +0x930..+0x978. A row holds
/// the frame's own copy on the race- rides and the frame before's on the s- rides (MD148's phases).
const RAGDOLL: [(u32, u32); 2] = [(0x870, 0x20), (0x930, 0x48)];
/// The C library's reentrancy struct; rand()'s 64-bit state is at +0xa8 in it, stepped by the newlib
/// LCG (state × [`LCG`] + 1, the draw its high word & 0x7fff_ffff).
const REENT: u32 = 0x0039_3a90;
const LCG: u64 = 0x5851_f42d_4c95_7f2d;
/// R37: the rand() draws the original made between the image and a ride's frame that the replayed sim
/// frames do not make (the game draws rand() outside the sim frame too), as (ride, frame, draws): the
/// RNG is stepped that many times before that frame on both machines. The wreck's first frame draws
/// the rider's spin (three draws, base + spread × (2 × draw − 1) per axis) and race-jump-side's row 226
/// an idle variant (one of five); with these race-jump-side's ragdoll copies equal the capture bit for
/// bit on every row (s-jump's on all but its phase-flipped row 49), and race-jump-side's "a bone
/// touches" (+0x920) arms its put-back on the capture's frame by itself. race-jump, race-wheelie-launch
/// and s-wheelie-launch lag too (their copies part at first contact) but are not found yet; their
/// bodies match without it. Found by inverting the captured spin through the frame's own quaternion and
/// searching the LCG's stream for its draws (the journal).
const RNG_LAG: [(&str, usize, u64); 3] = [("race-jump-side", 183, 41), ("race-jump-side", 226, 1), ("s-jump", 74, 33)];
/// Quad words the sim frame leaves to code that runs between two frames, outside it, as (offset,
/// bytes, bits held against the rows): the replay runs no such code, so these bits are not compared
/// (MD121's journal):
/// - `+0x44` bit 0x400: set by the vehicle's scene-node pass (the `+0xc` node's vt +0x48, a thunk to
///   the quad's vt +0x108, which ends in the bit's setter), cleared by the vehicle's PostProcess in the
///   frame. No reader of it was found.
/// - `+0x6f8..+0x720`'s ride time, top speed, wheelie and stoppie distances and their bests: the frame
///   counts them off the player's score card (`*(+0x680)`: its flags, speed `+0x108` and step `+0x18`),
///   which the card's own update writes between frames.
/// - `+0x464`, `+0x468`: the camera link list's node pointers, which the allocator hands out; the
///   game's other per-frame code allocates and frees between frames, so its blocks are others.
/// - `+0xd90`: where the vehicle last gathered its nearby list (the sphere gather, between frames;
///   `+0xd38`, the squared distance since, is in the spot, laid).
const BETWEEN: [(u32, u32, u32); 6] =
    [(0x44, 4, !0x400), (0x6f8, 8, 0), (0x70c, 8, 0), (0x71c, 8, 0), (0x464, 8, 0), (0xd90, 0x10, 0)];
/// The rider's block in the quad, quad-relative: the rider's update writes its copies here
/// (`*(rider+0x378)` is quad +0x730). From a wreck's first row the interpreter's rider ragdoll leaves
/// the original's, so the block is not compared from there on; R37's RNG lag lines race-jump-side's
/// up again ([`RAGDOLL`]).
const RIDER_BLOCK: (u32, u32) = (0x870, 0x980);
/// The quad's wreck reason (non-zero while wrecked).
const REASON: u32 = 0xc80;

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

/// Every byte the row holds that the machine must equal, as address → (byte, bits compared): the
/// body, engine and wheels whole, and the quad's own 0x1800 bytes but for [`BETWEEN`], the spot when
/// it is laid a row late (`spot_late`), and the rider's block once wrecked ([`RIDER_BLOCK`]).
fn want(ee: &Ee, row: &Row, spot_late: bool, wrecked: bool) -> HashMap<u32, (u8, u8)> {
    let mut want = HashMap::new();
    let mut put = |at: u32, words: Vec<u32>, mask: &dyn Fn(u32) -> u32| {
        for (i, w) in words.into_iter().enumerate() {
            let m = mask(4 * i as u32);
            for (b, (x, k)) in w.to_le_bytes().into_iter().zip(m.to_le_bytes()).enumerate() {
                if k != 0 {
                    want.insert(at + 4 * i as u32 + b as u32, (x, k));
                }
            }
        }
    };
    for (name, at, _) in blocks(ee) {
        put(at, hex_words(&row[&name]), &|_| !0);
    }
    let mask = |off: u32| {
        let skip = |(at, len): (u32, u32)| (at..at + len).contains(&off);
        if let Some(&(_, _, bits)) = BETWEEN.iter().find(|&&(at, len, _)| skip((at, len))) {
            bits
        } else if (spot_late && SPOT.iter().any(|&s| skip(s))) || (wrecked && skip((RIDER_BLOCK.0, RIDER_BLOCK.1 - RIDER_BLOCK.0))) {
            0
        } else {
            !0
        }
    };
    put(C28_QUAD, obj(row), &mask);
    want
}

/// The words of `want` the machine holds otherwise, as text.
fn diff(ee: &Ee, want: &HashMap<u32, (u8, u8)>) -> Vec<String> {
    let mut words: Vec<u32> = want.iter().filter(|&(&a, &(x, k))| ee.u8_at(a) & k != x & k).map(|(&a, _)| a & !3).collect();
    words.sort();
    words.dedup();
    let name = |a: u32| {
        let rel = |base: u32| a.wrapping_sub(base);
        match blocks(ee).into_iter().find(|&(_, at, len)| rel(at) < len) {
            Some((n, at, _)) => format!("{n}+{:#x}", rel(at)),
            None => format!("quad+{:#x}", rel(C28_QUAD)),
        }
    };
    let byte = |a: u32| want.get(&a).map_or(0, |&(x, _)| x);
    words
        .into_iter()
        .map(|a| {
            let (got, w) = (ee.u32_at(a), u32::from_le_bytes([byte(a), byte(a + 1), byte(a + 2), byte(a + 3)]));
            format!("{}: {got:#010x} ({}) want {w:#010x} ({})", name(a), f32::from_bits(got), f32::from_bits(w))
        })
        .collect()
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

/// The captured blocks' and the quad's bytes as they are now, by address.
fn block_bytes(ee: &Ee) -> HashMap<u32, u8> {
    blocks(ee).into_iter().chain([(String::new(), C28_QUAD, 0x1800)]).flat_map(|(_, at, len)| (at..at + len).map(|a| (a, ee.u8_at(a))).collect::<Vec<_>>()).collect()
}

/// A7's mid-frame read: the instant inside the frame just run where every byte of `want` (the body,
/// engine, wheels and the quad's words) was the row's, found by laying the frame's writes
/// ([`Ee::log`]) one by one over the bytes as they were before it (`pre`). Answers how many of the
/// frame's writes to those bytes had landed and the pc of the last, at the first such instant (0
/// writes: the row was read before the frame).
fn mid_frame(ee: &Ee, want: &HashMap<u32, (u8, u8)>, pre: &HashMap<u32, u8>) -> Option<(usize, usize, u32)> {
    let off_by = |a: &u32, x: u8| want.get(a).is_some_and(|&(w, k)| x & k != w & k);
    let mut now = pre.clone();
    let mut off = now.iter().filter(|&(a, &x)| off_by(a, x)).count();
    let log = ee.log.borrow();
    let writes: Vec<_> = log.as_ref().expect("the write log is on").iter().filter(|(_, a, _)| want.contains_key(a)).collect();
    if off == 0 {
        return Some((0, writes.len(), 0));
    }
    for (k, &&(pc, a, x)) in writes.iter().enumerate() {
        let was = now.insert(a, x).unwrap();
        off = off + usize::from(off_by(&a, x)) - usize::from(off_by(&a, was));
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
/// before each row's frame the pad reader's stores, the put-back spot that frame reads
/// ([`spot_after`]) and the RNG's lag ([`RNG_LAG`]) are laid on both machines, after it every captured
/// word of the body, engine and wheels and every word of the quad ([`want`]) must be the row's, at the
/// frame's end or at an instant inside it ([`mid_frame`]), and the rider's ragdoll copies ([`RAGDOLL`])
/// are held against the row that has them.
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
        *ee.log.borrow_mut() = Some(Vec::new());
        // The spot frame r reads is row r-1's when the rows are read after the game writes it, row r's
        // when before (s-wheelie-launch's put-back on row 158 lands on row 158's spot).
        let spot = if after_spot { r.checked_sub(1).map_or(Vec::new(), |q| words(&rows[q], &SPOT)) } else { words(&rows[r], &SPOT) };
        let mut rng = Vec::new();
        for &(_, _, draws) in RNG_LAG.iter().filter(|&&(ride, at, _)| ride == name && at == r) {
            let at = ee.u32_at(REENT) + 0xa8;
            let state = (0..draws).fold(u64::from(ee.u32_at(at)) | u64::from(ee.u32_at(at + 4)) << 32, |x, _| x.wrapping_mul(LCG).wrapping_add(1));
            rng = vec![(at, state as u32), (at + 4, (state >> 32) as u32)];
        }
        let lay = [words(&rows[r], &READER), spot, rng].concat();
        let mut bytes = block_bytes(ee);
        for &(a, w) in &lay {
            for (b, x) in w.to_le_bytes().into_iter().enumerate() {
                bytes.insert(a + b as u32, x);
            }
        }
        *pre.borrow_mut() = bytes;
        lay
    };
    let mid = RefCell::new(Vec::<(usize, usize, usize, u32)>::new());
    let torn = RefCell::new(Vec::<usize>::new());
    let first = RefCell::new(String::new());
    let wrecked = Cell::new(None::<usize>);
    // The ragdoll copies the frame left in the quad, against the row that holds them.
    let late = !after_spot;
    let ragdoll = RefCell::new(Vec::<usize>::new());
    let copies = |r: usize, ee: &Ee| {
        let Some(row) = rows.get(r + usize::from(late)) else { return };
        if words(row, &RAGDOLL).iter().any(|&(a, w)| ee.u32_at(a) != w) {
            ragdoll.borrow_mut().push(r);
        }
    };
    let after = |r: usize, ee: &Ee| {
        let row = &rows[r];
        copies(r, ee);
        if wrecked.get().is_none() && (ee.u32_at(C28_QUAD + REASON) != 0 || obj(row)[REASON as usize / 4] != 0) {
            wrecked.set(Some(r));
        }
        let want = want(ee, row, after_spot, wrecked.get().is_some());
        let d = diff(ee, &want);
        // A row the recorder read while the game's frame ran differs at the frame's end (s-jump's
        // row 74: a substep short, its forces summed and not yet integrated; the s- rides' quad words
        // the frame's tail writes after the physics), and must equal the machine at an instant inside
        // the frame. PINE reads at a fixed point in emulated time, so a slower re-record reads the
        // same rows mid-frame (C32).
        if !d.is_empty() {
            if let Some((k, of, pc)) = mid_frame(ee, &want, &pre.borrow()) {
                mid.borrow_mut().push((r, k, of, pc));
                return Vec::new();
            }
            if torn.borrow().is_empty() {
                *first.borrow_mut() = format!("row {r} (vsync {}): {} words differ at the frame's end\n  {}", row["vsync"], d.len(), d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  "));
            }
            torn.borrow_mut().push(r);
            return Vec::new();
        }
        if r % 50 == 0 {
            eprintln!("MD37b {name} row {r} equal");
        }
        Vec::new()
    };
    let seen = sim_chain(n, port, None, tail, before, after);
    let (torn, mid, ragdoll) = (torn.into_inner(), mid.into_inner(), ragdoll.into_inner());
    let say = format!(
        "MD37b {name} ({n} rows, wrecked from {:?}; rows read mid-frame (row: writes landed of the frame's, at pc): {}; rows matching no instant {torn:?}; rows whose ragdoll copies differ {ragdoll:?})\n",
        wrecked.get(),
        mid.iter().map(|(r, k, of, pc)| format!("{r}: {k}/{of} {pc:#x}")).collect::<Vec<_>>().join(", "),
    );
    eprint!("{say}");
    if let Ok(log) = std::env::var("MD37B_LOG") {
        use std::io::Write;
        let all = format!("{say}{}\n", first.borrow());
        std::fs::OpenOptions::new().create(true).append(true).open(log).unwrap().write_all(all.as_bytes()).unwrap();
    }
    assert!(name != "race-jump-side" || ragdoll.is_empty(), "{name}: the rider's ragdoll copies differ from the capture on rows {ragdoll:?}");
    assert!(torn.is_empty(), "{name}: rows {torn:?} differ from the capture at every instant of their frames; the first was {}", first.borrow());
    seen
}

/// Each ride twice: by the original's frame alone, and by the port's pieces and tail beside it (every
/// RAM word of the twin equal to the original's after each frame, and both equal to the capture). Every
/// ride is replayed whole. race-jump-side's put-back is timed by the rider's ragdoll settling, which
/// follows from the RNG's lag ([`RNG_LAG`], R37). With the quad's words compared (MD121), nearly every
/// s- row is read mid-frame: the recorder reads them before the quad's last few dozen writes of the
/// frame (the race- rows at its end); A7's torn rows (s-stoppie-short's, s-wheelie-launch 81-83, s-jump
/// 74) are among them.
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
    race_jump_side_replays_by_the_originals_frame, race_jump_side_replays_by_the_port: "race-jump-side" off "atv-r-air" upto 300;
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
