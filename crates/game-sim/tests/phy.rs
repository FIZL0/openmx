//! S18 — the bike's `.phy` file, proved against the loader that reads it.
//!
//! Four proofs, in the order they were built.
//!
//! 1. **The reader's primitives**, against the game's own six INI getters, over every key of both
//!    retail Race125 files and of a generated file written to be awkward: spaces where the format
//!    removes them, quotes, duplicate keys, empty values, comments, hex and octal integers, every
//!    spelling of a flag. The getters are reached through the in-memory open the reader also has, so
//!    this proof needs no file system at all.
//! 2. **The whole loader**, run for real on the retail file: the bike, its chassis and its engine are
//!    laid out in the interpreter's RAM from zero, the game's own `.phy` loader fills them, and then
//!    the same three regions are filled again from the port's [`Phy`] at the offsets the original
//!    uses and compared word for word. Starting from zero is what makes that comparison a statement
//!    about *everything*: a word the original writes and the port does not model is a word that is
//!    nonzero on one side and zero on the other.
//! 3. **The arguments of the seven suspension, tire and steering setters and the seven collision
//!    setters**, which is where the rest of the file goes. Those objects belong to other modules
//!    (`wheel.rs`, and the collision record nothing has ported yet), so the loader's share of them is
//!    exactly what it passes, and that is what is compared: a second run with all fourteen hooked,
//!    recording the registers each was called with.
//! 4. **The bike the human rode**, out of the save state beside the recording: the port reads
//!    `Race125P.phy` off the disc and every parameter word is compared with the live bike's, with the
//!    game's own curve table taken from the same image. No interpreter, and nothing generated — this
//!    is the one that says the map is right about a bike that was being ridden.
//!
//! The ones that need the game skip without it, so a fresh checkout stays green.

use game_sim::archive::{Entry, Psr, Res};
use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::phy::{self, Ini, Phy};
use game_sim::track::{FREERIDE_ENGINE, FREERIDE_PRO, Tuning, physics_file};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, game_or_skip, skip};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::rc::Rc;

/// The object graph, laid out past anything the executable uses. The three compared regions are each
/// object's highest offset the loader touches, rounded up; the wheels and the collision record are
/// laid out too but not compared, because the loader reaches them through setters (proof 3).
const OBJ: u32 = 0x0100_0000;
const CHASSIS: u32 = 0x0101_0000;
const FRONT: u32 = 0x0102_0000;
const REAR: u32 = 0x0103_0000;
const ENGINE: u32 = 0x0104_0000;
const COLL: u32 = 0x0105_0000;
const REGIONS: [(&str, u32, u32); 3] =
    [("bike", OBJ, 0xb80), ("chassis", CHASSIS, 0x2c0), ("engine", ENGINE, 0xc0)];

/// The chassis' vtable, with the one slot the loader calls through it: the mass setter.
const CH_VTABLE: u32 = 0x0106_0000;
const MASS_SLOT: u32 = 0x78;
/// The engine's `{cos, sin}` table, built by the game itself.
const CURVE: u32 = 0x0108_0000;
/// The reader's own state, and what proof 1 hands it and reads back out of it.
const INI: u32 = 0x0110_0000;
const BLOB: u32 = 0x0111_0000;
const SEC: u32 = 0x0112_0000;
const KEY: u32 = 0x0112_0400;
const DEFAULT: u32 = 0x0112_0800;
/// A getter's destination. One word, then a quadword, then a kilobyte of string.
const DST: u32 = 0x0113_0000;
const DST_V: u32 = DST + 0x10;
const DST_S: u32 = DST + 0x20;
/// What a destination holds before a getter is called: a getter that reports the key absent must
/// leave this alone, which is the whole of the "destination untouched" rule the port relies on.
const SENTINEL: u32 = 0xdead_beef;
/// The heap every allocation in a run comes out of.
const HEAP: u32 = 0x0120_0000;
const HEAP_END: u32 = 0x01a0_0000;
/// And where the C library's own heap grows, which is a different allocator reached a different way.
const BREAK: u32 = 0x01a0_0000;
const BREAK_END: u32 = 0x01f0_0000;

/// The file the loader is told to open, and the only name the served file answers to.
const PHY: &str = "Bike\\Race125P.phy";
/// Both halves of D7's freeride set: the pro tuning the human rode and the amateur one beside it.
const RETAIL: [&str; 2] = ["race125p.phy", "race125a.phy"];

/// The player's bike in the save state beside the recording, and the name it must be carrying for the
/// image to be the one this proof is about.
const LIVE_BIKE: u32 = 0x00dad810;
const LIVE_NAME: u32 = 0x84;
const STATE: &str = "first_recording.p2m2_SaveState.eeMemory.bin";

// ---------------------------------------------------------------------------------------------------
// The disc

fn container() -> Option<File> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let at = kit.ctx("iso").join("MX.PSR");
    match File::open(&at) {
        Ok(f) => Some(f),
        Err(_) => {
            skip(&format!("no {} — `./kit extract MX.PSR` writes it", at.display()));
            None
        }
    }
}

fn member(f: &mut File, psr: &Psr, name: &str) -> Vec<u8> {
    let e = psr.find(name).unwrap_or_else(|| panic!("{name} is not in MX.PSR"));
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    bytes
}

fn directory(f: &mut File) -> Psr {
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    Psr::parse(&front).expect("MX.PSR's directory")
}

/// Every `.phy` on the disc whose leaf name is one of `want`, read out of whichever `Bike\*.res` holds
/// it — which is how the app layer finds a vehicle's physics file too.
fn phy_files(f: &mut File, psr: &Psr, want: &[&str]) -> Vec<(String, Vec<u8>)> {
    let archives: Vec<String> = psr
        .entries
        .iter()
        .map(|e: &Entry| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.starts_with("bike\\") && n.ends_with(".res")
        })
        .collect();
    let mut out = Vec::new();
    for archive in &archives {
        let bytes = member(f, psr, archive);
        let Ok(res) = Res::parse(&bytes) else { continue };
        for e in &res.entries {
            let leaf = e.name.to_ascii_lowercase();
            let leaf = leaf.rsplit('\\').next().unwrap_or(&leaf).to_string();
            if want.contains(&leaf.as_str()) {
                let file = res.read(&bytes, e).unwrap_or_else(|why| panic!("{}: {why}", e.name));
                // Every bike's archive carries the whole set of physics files, so the same file
                // comes off the disc two dozen times. One copy of each distinct file is what is
                // wanted; a copy that differs would be kept under its archive's name.
                if out.iter().any(|(_, b): &(String, Vec<u8>)| *b == file) {
                    continue;
                }
                let name =
                    if out.iter().any(|(n, _)| *n == leaf) { format!("{archive}: {leaf}") } else { leaf.clone() };
                out.push((name, file));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

// ---------------------------------------------------------------------------------------------------
// The machine

/// A bump allocator over every heap call the loader and the reader make, with the sizes remembered so
/// that `realloc` can copy — the reader grows its pair array one entry at a time, so a `realloc` that
/// lost the old contents would lose the file.
fn heap(ee: &mut Ee) {
    let state = Rc::new(RefCell::new((HEAP, BTreeMap::<u32, u32>::new())));
    for name in ["malloc", "new_array", "operator_new"] {
        let state = Rc::clone(&state);
        ee.hooks.insert(
            addr(name).unwrap(),
            Box::new(move |ee| {
                let size = ee.r(4) as u32;
                let at = take(&state, size);
                ee.set(2, at as u64);
            }),
        );
    }
    {
        let state = Rc::clone(&state);
        ee.hooks.insert(
            addr("realloc").unwrap(),
            Box::new(move |ee| {
                let (old, size) = (ee.r(4) as u32, ee.r(5) as u32);
                if old == 0 {
                    let at = take(&state, size);
                    ee.set(2, at as u64);
                    return;
                }
                let was = *state.borrow().1.get(&old).unwrap_or_else(|| panic!("realloc of {old:#x}, which was never allocated"));
                // The commonest case by far, and the one that keeps the heap small: the block being
                // grown is the last one handed out, so it grows where it is.
                if old + was == state.borrow().0 {
                    let mut s = state.borrow_mut();
                    s.0 = old + size.max(was);
                    s.1.insert(old, size.max(was));
                    assert!(s.0 < HEAP_END, "the heap ran out growing {old:#x} to {size}");
                    ee.set(2, old as u64);
                    return;
                }
                let at = take(&state, size);
                let keep: Vec<u8> = (0..was.min(size)).map(|k| ee.u8_at(old + k)).collect();
                ee.load(at, &keep);
                ee.set(2, at as u64);
            }),
        );
    }
    for name in ["free", "delete_array", "operator_delete"] {
        ee.hooks.insert(addr(name).unwrap(), Box::new(|ee| ee.set(2, 0)));
    }
    // The C library's own malloc, which `strtod` reaches for the digit strings it cannot round in
    // one pass, grows its heap through `sbrk`. That one runs for real — it is the library's rounding
    // and the port has to match it — so all it needs is somewhere to grow, contiguously, because
    // malloc checks that each new block sits exactly on top of the old one.
    let brk = Rc::new(RefCell::new(BREAK));
    ee.hooks.insert(
        addr("sbrk").unwrap(),
        Box::new(move |ee| {
            let incr = ee.r(5) as i32;
            let mut at = brk.borrow_mut();
            let was = *at;
            *at = was.wrapping_add(incr as u32);
            assert!((BREAK..BREAK_END).contains(&*at), "the C library's heap ran out at {incr} bytes");
            ee.set(2, was as u64);
        }),
    );
}

fn take(state: &Rc<RefCell<(u32, BTreeMap<u32, u32>)>>, size: u32) -> u32 {
    let mut s = state.borrow_mut();
    let at = s.0;
    s.0 = at + size.max(16).next_multiple_of(16);
    assert!(s.0 < HEAP_END, "the heap ran out at {size} bytes");
    s.1.insert(at, size);
    at
}

/// The two pieces of the C library an interpreter cannot run: the semaphore it takes around `strtod`,
/// which one thread has no use for, and `memset`, which broadcasts its byte with one of the EE's own
/// parallel instructions. Standing in for a fill is exact, and neither is anything the reader reads.
fn stand_ins(ee: &mut Ee) {
    for name in ["libc_lock", "libc_unlock"] {
        ee.hooks.insert(addr(name).unwrap(), Box::new(|ee| ee.set(2, 0)));
    }
    ee.hooks.insert(
        addr("memset").unwrap(),
        Box::new(|ee| {
            let (at, byte, n) = (ee.r(4) as u32, ee.r(5) as u8, ee.r(6) as u32);
            ee.load(at, &vec![byte; n as usize]);
            ee.set(2, at as u64);
        }),
    );
}

/// A machine with the heap hooked and the game's own curve table built and handed back, which is what
/// makes the two cosines the loader reads out of it bit-exact on both sides.
fn harness() -> Option<(Ee, CurveTable)> {
    let mut ee = game_or_skip()?;
    stand_ins(&mut ee);
    heap(&mut ee);
    ee.hooks.insert(addr("mem_alloc").unwrap(), Box::new(|ee| ee.set(2, CURVE as u64)));
    ee.call(addr("curve_table_build").unwrap(), &[], &[], 2_000_000_000);
    assert_eq!(ee.v0(), 1, "the game's curve table builder reported failure");
    assert_eq!(ee.u32_at(addr("curve_table_ptr").unwrap()), CURVE, "the table went somewhere else");
    heap(&mut ee);
    let curve = curve_from(&ee, CURVE);
    Some((ee, curve))
}

/// The game's `{cos, sin}` table, read out of RAM as a [`CurveTable`] the port can be handed.
fn curve_from(ee: &Ee, at: u32) -> CurveTable {
    let entries: Vec<[f32; 2]> =
        (0..CURVE_ENTRIES as u32).map(|i| [ee.f32_at(at + 8 * i), ee.f32_at(at + 8 * i + 4)]).collect();
    CurveTable::from_entries(&entries)
}

fn put_str(ee: &mut Ee, at: u32, s: &str) {
    let mut bytes = s.as_bytes().to_vec();
    bytes.push(0);
    ee.load(at, &bytes);
}

fn read_str(ee: &Ee, at: u32) -> String {
    let mut out = Vec::new();
    for k in 0..0x400 {
        let c = ee.u8_at(at + k);
        if c == 0 {
            break;
        }
        out.push(c);
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn put_f(ee: &mut Ee, at: u32, v: F) {
    ee.set_u32(at, v.to_bits());
}

fn put_v(ee: &mut Ee, at: u32, v: Vf) {
    for (k, x) in [v.x(), v.y(), v.z(), v.w()].into_iter().enumerate() {
        ee.set_u32(at + 4 * k as u32, x.to_bits());
    }
}

/// Every word of every compared region, in order.
fn snap(ee: &Ee) -> Vec<u32> {
    let mut out = Vec::new();
    for (_, base, len) in REGIONS {
        for o in (0..len).step_by(4) {
            out.push(ee.u32_at(base + o));
        }
    }
    out
}

/// Which field a differing word is, by the name of its region and its offset in it.
fn where_is(i: usize) -> String {
    let mut i = i as u32 * 4;
    for (name, _, len) in REGIONS {
        if i < len {
            return format!("{name} + {i:#x}");
        }
        i -= len;
    }
    "past the end".into()
}

/// The index of one region word in a [`snap`].
fn word_of(base: u32, off: u32) -> usize {
    let mut at = 0;
    for (_, b, len) in REGIONS {
        if b == base {
            assert!(off < len, "{off:#x} is past the end of the region at {base:#010x}");
            return at + off as usize / 4;
        }
        at += len as usize / 4;
    }
    panic!("{base:#010x} is not a compared region");
}

// ---------------------------------------------------------------------------------------------------
// Proof 1 — the reader's primitives against the game's own six getters

/// A file written to be awkward, because the retail files are not: the format removes whitespace
/// rather than trimming it, keeps it inside brackets and quotes, drops a pair with an empty value,
/// takes the first `=` and only treats `;` as a comment at the start of a line. Every line here is
/// read by the game's reader and by the port's, and the two have to agree about all of it.
const AWKWARD: &str = "\
; a comment. No file here opens with a pair: a pair before the first section has no section to go
; in, and the original indexes its section array at -1 and reads unmapped memory for it, so there is
; nothing to compare against. The port drops such a pair.
[Mass Properties]
Weight=400
  Gravity  =  1.5\t
Inertia = 22.4, 18.4, 10.9
Spaced Key = 2
\"Quoted\" = 3
Quoted Value = \"1 2 3\"
Empty =
Dup = 1
Dup = 2
MiXeD = 0.25
Hex = 0x10
Octal = 010
Neg = -12
Plus = +7
Exp = 1e3
Dot = .5
Trail = 12.5abc
Words = abc
Big = 1234567890123
Tiny = 1.1754944e-38
; and nothing out of a float's range: see the journal's Not verified
Huge = 3.4e38
MinusZero = -0.0
Tabbed\t=\t9
NoEquals
Vec2 = 1, 2
Vec1 = 7
VecRun = 1 2 3 4 5
VecSemi = 1;2;3
OnFlag = ON
OffFlag = off
TFlag = T
FFlag = f
YFlag = Y
NFlag = n
TrueFlag = TRUE
OneFlag = 1
ZeroFlag = 0
WordFlag = maybe
[Second Section]
Weight = 1
Only Here = 4.5
; a comment at the end of a section
[Mass Properties]
Weight = 2
";

/// A float no retail file holds, so a destination that still has it was left alone.
const MARK: F = F(-7.25);
const MARK_V: Vf = Vf([-7.25, -8.5, -9.75, -11.0]);

fn lanes(v: Vf) -> [u32; 4] {
    [v.x().to_bits(), v.y().to_bits(), v.z().to_bits(), v.w().to_bits()]
}

/// Hand the reader a file it already has in RAM: `(length, bytes)`, which is the path the reader has
/// for a file somebody else loaded, and the one that keeps this proof off the file system.
fn open_in_memory(ee: &mut Ee, bytes: &[u8]) {
    let mut blob = (bytes.len() as u64).to_le_bytes().to_vec();
    blob.extend_from_slice(bytes);
    ee.load(BLOB, &blob);
    put_str(ee, SEC, "a file in hand");
    ee.call(addr("ini_new").unwrap(), &[INI as u64], &[], 10_000_000);
    ee.call(
        addr("ini_open").unwrap(),
        &[INI as u64, SEC as u64, 0, u64::MAX, BLOB as u64],
        &[],
        2_000_000_000,
    );
    assert_eq!(ee.v0(), 1, "the game's reader would not take the file");
}

fn close(ee: &mut Ee) {
    ee.call(addr("ini_del").unwrap(), &[INI as u64, u64::MAX], &[], 10_000_000);
}

fn select(ee: &mut Ee, name: &str) -> bool {
    put_str(ee, SEC, name);
    ee.call(addr("ini_section").unwrap(), &[INI as u64, SEC as u64], &[], 10_000_000);
    ee.v0() != 0
}

fn their_text(ee: &mut Ee, key: &str) -> Option<String> {
    put_str(ee, KEY, key);
    ee.load(DST_S, &[0u8; 4]);
    ee.call(addr("ini_text").unwrap(), &[INI as u64, KEY as u64, DST_S as u64, 0x400], &[], 10_000_000);
    (ee.v0() != 0).then(|| read_str(ee, DST_S))
}

fn their_float(ee: &mut Ee, key: &str) -> Option<u32> {
    put_str(ee, KEY, key);
    ee.set_u32(DST, SENTINEL);
    ee.call(addr("ini_float").unwrap(), &[INI as u64, KEY as u64, DST as u64], &[], 10_000_000);
    let got = ee.u32_at(DST);
    if ee.v0() == 0 {
        assert_eq!(got, SENTINEL, "{key}: the getter reported the key absent and wrote anyway");
        return None;
    }
    Some(got)
}

fn their_float_or(ee: &mut Ee, key: &str, default: F) -> u32 {
    put_str(ee, KEY, key);
    ee.set_u32(DST, SENTINEL);
    ee.call(
        addr("ini_float_default").unwrap(),
        &[INI as u64, KEY as u64, DST as u64],
        &[default.0],
        10_000_000,
    );
    ee.u32_at(DST)
}

fn their_int(ee: &mut Ee, key: &str) -> Option<i32> {
    put_str(ee, KEY, key);
    ee.set_u32(DST, SENTINEL);
    ee.call(addr("ini_int").unwrap(), &[INI as u64, KEY as u64, DST as u64], &[], 10_000_000);
    let got = ee.u32_at(DST);
    if ee.v0() == 0 {
        assert_eq!(got, SENTINEL, "{key}: the getter reported the key absent and wrote anyway");
        return None;
    }
    Some(got as i32)
}

fn their_flag(ee: &mut Ee, key: &str, default: bool) -> u32 {
    put_str(ee, KEY, key);
    ee.set_u32(DST, SENTINEL);
    ee.call(
        addr("ini_bool").unwrap(),
        &[INI as u64, KEY as u64, u64::from(default), DST as u64],
        &[],
        10_000_000,
    );
    ee.u32_at(DST)
}

fn their_vector(ee: &mut Ee, key: &str, default: Vf) -> [u32; 4] {
    put_str(ee, KEY, key);
    put_v(ee, DEFAULT, default);
    for k in 0..4 {
        ee.set_u32(DST_V + 4 * k, SENTINEL);
    }
    ee.call(
        addr("ini_quadword").unwrap(),
        &[INI as u64, KEY as u64, DEFAULT as u64, DST_V as u64],
        &[],
        10_000_000,
    );
    [0, 1, 2, 3].map(|k| ee.u32_at(DST_V + 4 * k))
}

/// Every key of every file, read six ways by the game and six ways by the port.
#[test]
fn the_readers_primitives_are_the_games_own() {
    let Some(mut ee) = game_or_skip() else { return };
    stand_ins(&mut ee);
    heap(&mut ee);

    let mut files: Vec<(String, Vec<u8>)> = vec![("the generated file".into(), AWKWARD.into())];
    if let Some(mut f) = container() {
        let psr = directory(&mut f);
        files.extend(phy_files(&mut f, &psr, &RETAIL));
    }
    assert!(files.len() >= 2, "no retail .phy came off the disc — only the generated file was read");

    let (mut keys, mut steps) = (0usize, 0u64);
    for (what, bytes) in &files {
        let ours = Ini::parse_bytes(bytes);
        open_in_memory(&mut ee, bytes);
        steps += ee.steps;

        // Every section the port found is a section the original finds, under the same name.
        for name in ours.names() {
            assert!(select(&mut ee, name), "{what}: the original has no section called [{name}]");
        }

        let mut current = String::new();
        for (section, key, value) in ours.entries() {
            if section != current {
                assert!(select(&mut ee, section), "{what}: no section [{section}]");
                current = section.to_string();
            }
            let s = ours.section(section);
            let at = format!("{what} [{section}] {key} = {value}");
            assert_eq!(their_text(&mut ee, key).as_deref(), s.text(key), "{at}: the string");
            assert_eq!(their_float(&mut ee, key), s.float(key).map(|v| v.to_bits()), "{at}: the float");
            assert_eq!(
                their_float_or(&mut ee, key, MARK),
                s.float_or(key, MARK).to_bits(),
                "{at}: the float with a default"
            );
            assert_eq!(their_int(&mut ee, key), s.int(key), "{at}: the integer");
            // The original's flag getter writes the whole integer the value names — it is the
            // integer getter with a default bolted on — while the port narrows it to a bool, which
            // is what every one of these destinations is read as. So the truth of the word is
            // compared always, and the word itself wherever it is a 0 or a 1, which is all any
            // retail file holds.
            for default in [false, true] {
                let theirs = their_flag(&mut ee, key, default);
                assert_eq!(theirs != 0, s.flag(key, default), "{at}: the flag, defaulting to {default}");
                if theirs < 2 {
                    assert_eq!(theirs, u32::from(s.flag(key, default)), "{at}: the flag's own word");
                }
            }
            assert_eq!(their_vector(&mut ee, key, MARK_V), lanes(s.vector(key, MARK_V)), "{at}: the quadword");
            keys += 1;
            steps += ee.steps;
        }

        // And keys that are not there: every getter has to report so, and the two that have no
        // default have to leave the destination alone — which is what the loader's "absent means the
        // constructor's value stands" depends on.
        let section = ours.names().first().cloned().unwrap_or_default();
        assert!(select(&mut ee, &section));
        let s = ours.section(&section);
        for key in ["NoSuchKey", "Weigh", "WeightX", "Wei ght", "0", "-"] {
            let at = format!("{what} [{section}] {key}, which is not there");
            assert_eq!(their_text(&mut ee, key), None, "{at}: the string");
            assert_eq!(s.text(key), None, "{at}: the port's string");
            assert_eq!(their_float(&mut ee, key), None, "{at}: the float");
            assert_eq!(s.float(key), None, "{at}: the port's float");
            assert_eq!(their_int(&mut ee, key), None, "{at}: the integer");
            assert_eq!(s.int(key), None, "{at}: the port's integer");
            assert_eq!(their_float_or(&mut ee, key, MARK), MARK.to_bits(), "{at}: the float's default");
            assert_eq!(s.float_or(key, MARK).to_bits(), MARK.to_bits(), "{at}: the port's default");
            for default in [false, true] {
                assert_eq!(their_flag(&mut ee, key, default), u32::from(default), "{at}: the flag's default");
                assert_eq!(s.flag(key, default), default, "{at}: the port's flag default");
            }
            assert_eq!(their_vector(&mut ee, key, MARK_V), lanes(MARK_V), "{at}: the quadword's default");
            assert_eq!(lanes(s.vector(key, MARK_V)), lanes(MARK_V), "{at}: the port's quadword default");
        }

        // A section the file does not have. The port answers every key of it with the default; the
        // original is asked the same thing here so that a port reading a file with a section missing
        // cannot quietly read the section that happened to be selected before.
        assert!(!select(&mut ee, "No Such Section"), "{what}: the original found a section that is not there");
        let missing = ours.section("No Such Section");
        for key in ["Weight", "Gravity", "Inertia"] {
            let at = format!("{what} [No Such Section] {key}");
            assert_eq!(their_float(&mut ee, key), None, "{at}: the float");
            assert_eq!(missing.float(key), None, "{at}: the port's float");
            assert_eq!(their_float_or(&mut ee, key, MARK), MARK.to_bits(), "{at}: the default");
            assert_eq!(missing.float_or(key, MARK).to_bits(), MARK.to_bits(), "{at}: the port's default");
        }

        close(&mut ee);
    }
    println!("the reader: {keys} keys in {} files, six getters each, {steps} instructions", files.len());
    assert!(keys > 1900, "only {keys} keys were read — both retail files should be in there");
}

// ---------------------------------------------------------------------------------------------------
// Proof 2 — the whole loader

/// The seven the wheels are reached through and the seven the collision record is.
const WHEEL_SETTERS: [&str; 7] = [
    "wheel_set_suspension",
    "wheel_set_tire",
    "wheel_set_surface_drive",
    "wheel_set_surface_side",
    "wheel_set_surface_drag",
    "wheel_set_steering",
    "wheel_set_tire_derived",
];
const COLLISION_SETTERS: [&str; 7] = [
    "collision_set_mass",
    "collision_set_springs",
    "collision_set_high_speed",
    "collision_set_angular",
    "collision_set_moment_scales",
    "collision_set_friction",
    "collision_set_surface_friction",
];

/// Serve one file to the game's four file calls, under the one name the bike is carrying. Everything
/// else is reported missing, which is how the loader's second open — the wreck bike's own file, which
/// it hands to the ragdoll — is kept out of this proof.
fn serve(ee: &mut Ee, bytes: &[u8]) {
    const HANDLE: u64 = 7;
    let size = bytes.len() as u64;
    let data = bytes.to_vec();
    ee.hooks.insert(
        addr("fs_open").unwrap(),
        Box::new(move |ee| {
            let name = read_str(ee, ee.r(5) as u32);
            ee.set(2, if name.eq_ignore_ascii_case(PHY) { HANDLE } else { u64::MAX });
        }),
    );
    ee.hooks.insert(addr("fs_size").unwrap(), Box::new(move |ee| ee.set(2, size)));
    ee.hooks.insert(
        addr("fs_read").unwrap(),
        Box::new(move |ee| {
            let (dst, len) = (ee.r(6) as u32, (ee.r(7) as u32 as usize).min(data.len()));
            ee.load(dst, &data[..len]);
            ee.set(2, len as u64);
        }),
    );
    ee.hooks.insert(addr("fs_close").unwrap(), Box::new(|ee| ee.set(2, 0)));
}

/// The graph as the bike's constructor leaves it, less everything the loader writes: zeroed, with the
/// pointers between the pieces, the one vtable slot the loader calls through and the name of the file
/// it is to read.
fn lay_out(ee: &mut Ee) {
    for (base, len) in [(OBJ, 0xb80u32), (CHASSIS, 0x2c0), (FRONT, 0x8c0), (REAR, 0x8c0), (ENGINE, 0xc0), (COLL, 0x40)] {
        for o in (0..len).step_by(4) {
            ee.set_u32(base + o, 0);
        }
    }
    ee.set_u32(OBJ + 0x7a4, CHASSIS);
    ee.set_u32(OBJ + 0x7b0, FRONT);
    ee.set_u32(OBJ + 0x7b4, REAR);
    ee.set_u32(OBJ + 0x7b8, ENGINE);
    ee.set_u32(OBJ + 0x7d0, COLL);
    ee.set_u32(CHASSIS + 0x8, CH_VTABLE);
    ee.set_u32(CH_VTABLE + MASS_SLOT, addr("rigid_body_set_mass").unwrap());
    // The wheels' own back pointer to the chassis: the suspension setter reads the mass through it.
    ee.set_u32(FRONT + 0x1e4, CHASSIS);
    ee.set_u32(REAR + 0x1e4, CHASSIS);
    put_str(ee, OBJ + LIVE_NAME, PHY);
}

/// The port's answer, written into the graph at the offsets the original uses. This is the key to
/// parameter map S3 recovered by reading, and proof 2 is it being run for real.
fn poke_ours(ee: &mut Ee, p: &Phy) {
    // [Mass Properties]. The chassis' mass is the setter's, and the loader reads it straight back out
    // to turn the file's gravity into a force.
    put_v(ee, OBJ + 0x440, p.mass.inertia);
    put_v(ee, OBJ + 0x460, p.mv.cofg);
    put_v(ee, OBJ + 0x470, p.mass.wreck_cofg);
    put_v(ee, OBJ + 0x480, p.mv.balance_cofg);
    put_f(ee, OBJ + 0x698, p.mv.cofg_travel_forward);
    put_f(ee, OBJ + 0x69c, p.mv.cofg_travel_back);
    put_f(ee, OBJ + 0x6a4, p.mv.brake_cofg_travel_forward);
    put_v(ee, OBJ + 0x450, p.mv.gravity);
    put_f(ee, CHASSIS + 0x210, p.mass.mass);
    put_f(ee, CHASSIS + 0x214, F::ONE / p.mass.mass);
    put_f(ee, CHASSIS + 0x234, p.mass.yaw_rate_limit);
    put_f(ee, CHASSIS + 0x244, -p.mass.yaw_rate_limit);

    // [Front Tire] and [Rear Tire]: the four spring rates the suspension setter leaves behind, and
    // the front tire's two per-surface tables, which are the bike's and not the wheel's.
    put_f(ee, OBJ + 0x6f4, p.mv.front_spring_when_stopped);
    put_f(ee, OBJ + 0x6f8, p.mv.front_spring);
    put_f(ee, OBJ + 0x6fc, p.mv.rear_spring_when_stopped);
    put_f(ee, OBJ + 0x700, p.mv.rear_spring);
    for (i, s) in p.front.surfaces.iter().enumerate() {
        put_f(ee, OBJ + 0xafc + 4 * i as u32, s.mud_time);
        put_f(ee, OBJ + 0xb3c + 4 * i as u32, s.mud_amount);
    }

    // [Engine]. The power curve, the gearbox and the inertia go in through three setters, which is
    // why the engine's own region is compared as well as the bike's.
    let e = &p.engine;
    put_f(ee, ENGINE + 0x38, e.idle_throttle);
    put_f(ee, ENGINE + 0x3c, e.engine_braking);
    put_f(ee, ENGINE + 0x40, e.final_drive);
    put_f(ee, ENGINE + 0x48, e.inv_inertia);
    put_f(ee, ENGINE + 0x50, e.clutch_static);
    put_f(ee, ENGINE + 0x54, e.clutch_kinetic);
    put_f(ee, ENGINE + 0x60, e.min_rpm);
    ee.set_u32(ENGINE + 0x68, e.gear as u32);
    ee.set_u32(ENGINE + 0x6c, e.curve.points);
    ee.set_u32(ENGINE + 0x78, e.gears as u32);
    put_f(ee, ENGINE + 0x80, e.ratio);
    put_f(ee, ENGINE + 0x84, e.inv_ratio);
    ee.set_u32(ENGINE + 0x8c, e.curve.points);
    ee.set_u32(ENGINE + 0x90, e.curve.index);
    put_f(ee, ENGINE + 0x94, e.curve.frac());
    put_f(ee, ENGINE + 0xb0, p.peak.rpm);
    put_f(ee, ENGINE + 0xb4, e.rev_limit);
    put_f(ee, ENGINE + 0xb8, p.peak.torque);
    put_f(ee, OBJ + 0x6c8, p.mv.clutch_torque_scale_time);
    put_f(ee, OBJ + 0x6d0, p.mv.max_clutch_torque_scale_speed);
    put_f(ee, OBJ + 0x6d4, p.mv.max_clutch_torque_scale);
    put_f(ee, OBJ + 0x6a0, p.mv.clutch_cofg_travel_back);
    put_f(ee, OBJ + 0x740, p.mv.up_shift_rpm);
    put_f(ee, OBJ + 0x744, p.mv.down_shift_rpm);
    put_f(ee, OBJ + 0x544, p.mv.reverse_speed);
    put_f(ee, OBJ + 0x548, p.mv.reverse_gain);

    // [Roll Control]
    put_f(ee, OBJ + 0x6d8, p.roll.max_roll_command);
    put_f(ee, OBJ + 0x6dc, p.roll.max_roll_rate_command);
    put_f(ee, OBJ + 0x6e0, p.roll.max_roll_speed);
    put_f(ee, OBJ + 0x6e4, p.roll.max_roll_yaw_rate);
    put_f(ee, OBJ + 0x6e8, p.roll.roll_gain);
    put_f(ee, OBJ + 0x6ec, p.roll.min_stand_upright_speed);
    put_f(ee, OBJ + 0x6f0, p.roll.max_stand_upright_speed);
    put_f(ee, OBJ + 0x704, p.roll.roll_control_gain);
    put_f(ee, OBJ + 0x708, p.roll.roll_control_rate_gain);
    put_f(ee, OBJ + 0x70c, p.roll.roll_control_max_cg);
    put_f(ee, OBJ + 0x710, p.roll.max_roll_rate_disturbance);

    // [Pitch Control]
    put_f(ee, OBJ + 0x714, p.pitch.max_pitch_command);
    put_f(ee, OBJ + 0x718, p.pitch.pitch_gain);
    put_f(ee, OBJ + 0x71c, p.pitch.pitch_rate_gain);
    put_f(ee, OBJ + 0x720, p.pitch.inv_wheelie_top_speed);
    put_f(ee, OBJ + 0x72c, p.pitch.stoppie_brake);
    put_f(ee, OBJ + 0x730, p.pitch.max_stoppie_pitch_command);
    put_f(ee, OBJ + 0x734, p.pitch.stoppie_pitch_gain);
    put_f(ee, OBJ + 0x738, p.pitch.stoppie_yaw_rate_scale);
    put_f(ee, OBJ + 0x73c, p.pitch.stoppie_top_speed);

    // [Aerodynamics], [Airborne Control], [Preload], [Backflip]
    let a = &p.air;
    put_f(ee, OBJ + 0x6a8, p.mv.drag);
    put_v(ee, OBJ + 0x4d0, a.airborne_control_moment);
    put_f(ee, OBJ + 0x4e0, a.airborne_damping);
    put_f(ee, OBJ + 0x4e4, a.airborne_control_force);
    put_f(ee, OBJ + 0x6ac, a.brake_gyro_torque);
    put_f(ee, OBJ + 0x6b0, a.unroll_gain);
    put_f(ee, OBJ + 0x6b4, a.unroll_torque);
    put_f(ee, OBJ + 0x6b8, a.unyaw_gain);
    put_f(ee, OBJ + 0x6bc, a.unyaw_torque);
    put_f(ee, OBJ + 0x9e4, p.max_crossup);
    put_f(ee, OBJ + 0x4ac, a.preload_time);
    put_f(ee, OBJ + 0x4b4, a.min_preload_time);
    put_f(ee, OBJ + 0x4b8, a.max_preload_time);
    put_f(ee, OBJ + 0x4bc, a.min_dpad_preload_time);
    put_f(ee, OBJ + 0x4c0, a.max_dpad_preload_time);
    put_f(ee, OBJ + 0x4c4, a.max_preload);
    put_f(ee, OBJ + 0x504, a.min_backflip_time);
    put_f(ee, OBJ + 0x508, a.max_backflip_time);
    put_f(ee, OBJ + 0x510, a.min_backflip_torque);
    put_f(ee, OBJ + 0x514, a.max_backflip_torque);
    put_f(ee, OBJ + 0x518, a.backflip_torque_rate_increase);
    put_f(ee, OBJ + 0x51c, a.backflip_torque_rate_decrease);

    // [Joystick]
    let j = &p.joystick;
    put_f(ee, OBJ + 0x660, j.x_axis_rate_limit);
    put_f(ee, OBJ + 0x664, j.y_axis_rate_limit);
    put_f(ee, OBJ + 0x668, j.x_dpad_rate_limit);
    put_f(ee, OBJ + 0x66c, j.y_dpad_rate_limit);
    put_f(ee, OBJ + 0x678, p.mv.steer_speed_1);
    put_f(ee, OBJ + 0x67c, p.mv.steer_scale_1);
    put_f(ee, OBJ + 0x680, p.mv.steer_speed_2);
    put_f(ee, OBJ + 0x684, p.mv.steer_scale_2);
    put_f(ee, OBJ + 0x688, j.gas_rate_limit);
    put_f(ee, OBJ + 0x68c, j.brake_rate_limit);
    put_f(ee, OBJ + 0x690, j.clutch_in_rate_limit);
    put_f(ee, OBJ + 0x694, j.clutch_out_rate_limit);

    // [Donuts]
    put_f(ee, OBJ + 0x530, p.donuts.min_donut_speed);
    put_f(ee, OBJ + 0x534, p.mv.max_donut_speed);
    put_f(ee, OBJ + 0x538, p.mv.donut_angle);
    put_f(ee, OBJ + 0x53c, p.mv.donut_angle_back);
    put_f(ee, OBJ + 0x540, p.donuts.roll_angle_scale);

    // [No Hander Lander]
    let b = &p.balance;
    put_f(ee, OBJ + 0xa50, b.balance_inertia_rate);
    put_f(ee, OBJ + 0xa54, b.min_balance_inertia);
    put_f(ee, OBJ + 0xa58, b.max_balance_inertia);
    put_f(ee, OBJ + 0xa5c, b.input_inertia);
    put_f(ee, OBJ + 0xa68, b.balance_noise_amplitude);
    put_f(ee, OBJ + 0xa70, b.balance_noise_time_scale);
    put_f(ee, OBJ + 0xa74, b.max_balance_point);

    // [Wrecks]
    put_f(ee, OBJ + 0x754, a.front_wreck_speed_limit);
    put_f(ee, OBJ + 0x758, a.back_wreck_speed_limit);
    put_f(ee, OBJ + 0x75c, a.side_wreck_speed_limit);
    put_f(ee, OBJ + 0x760, a.racing_down_wreck_speed_limit);
    put_f(ee, OBJ + 0x764, a.freestyle_down_wreck_speed_limit);
    put_f(ee, OBJ + 0x768, a.ground_pitch_down_wreck_angle);
    put_f(ee, OBJ + 0x76c, a.ground_pitch_up_wreck_angle);
    put_f(ee, OBJ + 0x770, a.airborne_pitch_down_wreck_angle);
    put_f(ee, OBJ + 0x774, a.airborne_pitch_up_wreck_angle);
    put_f(ee, OBJ + 0x778, a.airborne_roll_wreck_angle);
    put_f(ee, OBJ + 0x77c, a.no_hander_wreck_speed_limit);
    put_f(ee, OBJ + 0x780, a.no_hander_max_roll_angle);
    put_f(ee, OBJ + 0x784, a.joust_wreck_height);
    put_f(ee, OBJ + 0x78c, a.crossup_wreck);
    put_f(ee, OBJ + 0x790, a.crossup_wreck_time);
    put_f(ee, OBJ + 0x798, p.wreck_times.max_wreck_time);
    put_f(ee, OBJ + 0x79c, p.wreck_times.cannonball_wreck_time);

    // [FirstPersonCamera] and [Animation]
    put_v(ee, OBJ + 0x590, p.shoulders.left);
    put_v(ee, OBJ + 0x5a0, p.shoulders.right);
    put_f(ee, OBJ + 0xa20, p.animation.stunt_overlap_time);
    put_v(ee, OBJ + 0x990, p.animation.pelvis_box_position);
    put_v(ee, OBJ + 0x9a0, p.animation.pelvis_box_size);
}

/// The words of the compared regions the port does not model, blanked on both sides. The three heap
/// pointers are addresses, which a port has no business reproducing; `EngineInertia` itself is kept
/// by the original beside the reciprocal it actually uses, and nothing reads it.
const UNMODELLED: [(u32, u32); 5] =
    [(ENGINE, 0x44), (ENGINE, 0x70), (ENGINE, 0x74), (ENGINE, 0x7c), (ENGINE, 0x98)];

fn blank(words: &mut [u32]) {
    for (base, off) in UNMODELLED {
        words[word_of(base, off)] = 0;
    }
}

/// Every word the original's `.phy` loader writes into the bike, its chassis and its engine, against
/// every word the port writes — from a zeroed graph on both sides, so a word only one of them writes
/// is a word that differs.
#[test]
fn the_whole_loader_fills_the_bike_the_way_the_original_does() {
    let Some((mut ee, curve)) = harness() else { return };
    let Some(mut f) = container() else { return };
    let psr = directory(&mut f);
    let files = phy_files(&mut f, &psr, &[RETAIL[0]]);
    let (_, bytes) = files.first().expect("Race125P.phy is on the disc");

    serve(&mut ee, bytes);
    lay_out(&mut ee);
    // The collision record is not one of the regions compared here — the loader only reaches it
    // through its setters, and proof 3 is where what they are handed is checked — so they are hooked
    // away. One of them is written in the EE's parallel instructions, which the interpreter has not
    // been taught and this proof has no reason to teach it.
    for name in COLLISION_SETTERS {
        ee.hooks.insert(addr(name).unwrap(), Box::new(|ee| ee.set(2, 0)));
    }
    ee.call(addr("bike_phy_load").unwrap(), &[OBJ as u64], &[], 2_000_000_000);
    println!("the original's loader took {} instructions", ee.steps);
    let mut theirs = snap(&ee);
    // The three arrays the engine's setters allocate and copy into. Their addresses are the
    // allocator's, so the pointers are not compared, but what they point at is: this is the torque
    // curve the file describes and the gearbox it names.
    let (rpm_at, torque_at, ratios_at) =
        (ee.u32_at(ENGINE + 0x70), ee.u32_at(ENGINE + 0x74), ee.u32_at(ENGINE + 0x7c));
    let (points, gears) = (ee.u32_at(ENGINE + 0x6c), ee.u32_at(ENGINE + 0x78));
    assert_eq!(ee.u32_at(ENGINE + 0x98), rpm_at, "the curve cursor reads a different array than the curve");
    let their_curve: Vec<(u32, u32)> =
        (0..points).map(|k| (ee.u32_at(rpm_at + 4 * k), ee.u32_at(torque_at + 4 * k))).collect();
    let their_ratios: Vec<u32> = (0..gears).map(|k| ee.u32_at(ratios_at + 4 * k)).collect();

    let p = Phy::read_bytes(bytes, &curve);
    assert_eq!(points, p.engine.curve.points, "the number of power curve points");
    assert_eq!(gears, p.engine.gears as u32, "the number of gears");
    let our_curve: Vec<(u32, u32)> = (0..points as usize)
        .map(|k| (p.engine.curve_rpm[k].to_bits(), p.engine.curve_torque[k].to_bits()))
        .collect();
    assert_eq!(their_curve, our_curve, "the power curve: rpm and the torque it is turned into");
    let our_ratios: Vec<u32> =
        (0..gears as usize).map(|k| p.engine.gear_ratios[k].to_bits()).collect();
    assert_eq!(their_ratios, our_ratios, "the gear ratios");
    lay_out(&mut ee);
    poke_ours(&mut ee, &p);
    let mut ours = snap(&ee);

    blank(&mut theirs);
    blank(&mut ours);
    let mut bad = 0;
    for (i, (&t, &o)) in theirs.iter().zip(&ours).enumerate() {
        if t != o {
            if bad < 16 {
                println!(
                    "{}: theirs {t:#010x} ({}) ours {o:#010x} ({})",
                    where_is(i),
                    f32::from_bits(t),
                    f32::from_bits(o)
                );
            }
            bad += 1;
        }
    }
    assert_eq!(bad, 0, "{bad} of {} words differ", theirs.len());
    println!("the loader: {} words of the bike, the chassis and the engine, bit-exact", theirs.len());
}

// ---------------------------------------------------------------------------------------------------
// 3. The arguments of the fourteen setters
//
// The wheels and the collision record are not filled by the loader but by setters it calls, which
// work the file's numbers into their own derived form. What the loader is responsible for is the
// arguments, so that is what is proved here: a second run with all fourteen hooked, each recording
// what it was handed and returning without doing it.

/// One call of a setter, as the EE's two argument banks hand it over: the integers from `$4` up, the
/// floats from `$f12` up, and the arguments past the eighth from the caller's stack, where each one
/// gets eight bytes whatever its size — `sp+0x0`, `+0x8`, `+0x10`, and so on.
struct Shot {
    at: u32,
    ints: [u32; 6],
    floats: [u32; 8],
    stack: [u32; 6],
}

/// Record one function's arguments and return without running it.
fn capture(ee: &mut Ee, log: &Rc<RefCell<Vec<Shot>>>, name: &str) -> u32 {
    let at = addr(name).unwrap_or_else(|e| panic!("{name} is not in the addresses file: {e}"));
    let log = Rc::clone(log);
    ee.hooks.insert(
        at,
        Box::new(move |ee| {
            let sp = ee.r(29) as u32;
            let mut shot = Shot { at, ints: [0; 6], floats: [0; 8], stack: [0; 6] };
            for (k, slot) in shot.ints.iter_mut().enumerate() {
                *slot = ee.r(4 + k) as u32;
            }
            for (k, slot) in shot.floats.iter_mut().enumerate() {
                *slot = ee.f(12 + k).to_bits();
            }
            for (k, slot) in shot.stack.iter_mut().enumerate() {
                *slot = ee.u32_at(sp + 8 * k as u32);
            }
            log.borrow_mut().push(shot);
            ee.set(2, 0);
        }),
    );
    at
}

/// A walk down the calls in the order the loader made them. Nothing is skipped and nothing is
/// searched for: a call out of place is a difference.
struct Calls<'a> {
    log: &'a [Shot],
    k: usize,
}

impl<'a> Calls<'a> {
    fn next(&mut self, what: &str, name: &str) -> &'a Shot {
        let shot = self.log.get(self.k).unwrap_or_else(|| panic!("the loader never got to {what}"));
        let at = addr(name).unwrap();
        assert_eq!(
            shot.at, at,
            "call {} should be {what} ({name} at {at:#010x}), not the one at {:#010x}",
            self.k, shot.at
        );
        self.k += 1;
        shot
    }
}

/// The floats a call was handed, against what the port says they should be.
#[track_caller]
fn floats_are(shot: &Shot, what: &str, want: &[F]) {
    for (k, w) in want.iter().enumerate() {
        let got = shot.floats[k];
        assert_eq!(
            got,
            w.to_bits(),
            "{what}: float argument {} is {} ({got:#010x}), should be {} ({:#010x})",
            k + 1,
            f32::from_bits(got),
            w.0,
            w.to_bits()
        );
    }
}

/// The same for the arguments that went on the stack, counted from the ninth.
#[track_caller]
fn stack_floats_are(shot: &Shot, what: &str, from: usize, want: &[F]) {
    for (k, w) in want.iter().enumerate() {
        let got = shot.stack[from + k];
        assert_eq!(
            got,
            w.to_bits(),
            "{what}: stack argument {} is {} ({got:#010x}), should be {} ({:#010x})",
            9 + from + k,
            f32::from_bits(got),
            w.0,
            w.to_bits()
        );
    }
}

#[track_caller]
fn ints_are(shot: &Shot, what: &str, want: &[u32]) {
    assert_eq!(&shot.ints[..want.len()], want, "{what}: the integers from $4 up");
}

/// A quadword a call pointed at, which is how every vector argument is passed.
#[track_caller]
fn vector_at(ee: &Ee, at: u32, what: &str, want: Vf) {
    let got = [ee.u32_at(at), ee.u32_at(at + 4), ee.u32_at(at + 8), ee.u32_at(at + 12)];
    assert_eq!(got, lanes(want), "{what}: the quadword at {at:#010x}");
}

/// The calls one tire's section makes, in order: the suspension — twice for the front, three times
/// for the rear, whose spring is set once more after the two rates have been read back — the tire,
/// the front's steering, the numbers the wheel derives for itself, and then three calls a surface.
fn tire_calls(c: &mut Calls, t: &phy::Tire, wheel: u32, front: bool) {
    let side = if front { "front" } else { "rear" };
    let strut = [t.strut_compression, t.max_compression_rate, t.max_decompression_rate];
    let damp = [t.bounce_damping_ratio, t.rebound_damping_ratio];
    let suspension = |spring: F| -> Vec<F> {
        strut.iter().copied().chain([spring]).chain(damp.iter().copied()).collect()
    };

    let what = format!("the {side} suspension with SpringWhenStopped");
    let shot = c.next(&what, "wheel_set_suspension");
    floats_are(shot, &what, &suspension(t.spring_when_stopped));
    ints_are(shot, &what, &[wheel]);
    for again in 0..if front { 1 } else { 2 } {
        let what = format!("the {side} suspension with Spring, call {}", again + 2);
        let shot = c.next(&what, "wheel_set_suspension");
        floats_are(shot, &what, &suspension(t.spring));
        ints_are(shot, &what, &[wheel]);
    }

    let what = format!("the {side} tire");
    let shot = c.next(&what, "wheel_set_tire");
    // The third and fourth are the loader's own constants, the same for every bike and both wheels.
    floats_are(shot, &what, &[t.tire_radius, t.tire_inertia, F(6.0), F(24.0)]);
    ints_are(shot, &what, &[wheel]);

    if front {
        let what = "the front steering".to_string();
        let shot = c.next(&what, "wheel_set_steering");
        floats_are(shot, &what, &[t.steering_damping, t.steering_inertia, t.steering_torque]);
        ints_are(shot, &what, &[wheel]);
    }

    let what = format!("what the {side} wheel derives");
    let shot = c.next(&what, "wheel_set_tire_derived");
    ints_are(shot, &what, &[wheel]);

    for (i, s) in t.surfaces.iter().enumerate() {
        let what = format!("the {side} wheel's drive forces on surface {}", i + 1);
        let shot = c.next(&what, "wheel_set_surface_drive");
        floats_are(
            shot,
            &what,
            &[
                s.side_force,
                s.slipping_side_force,
                s.max_drive_force,
                s.max_side_force,
                s.max_slipping_side_force,
                s.skid_drive_force,
                s.skid_side_force,
                s.skid_slipping_side_force,
            ],
        );
        ints_are(shot, &what, &[wheel, i as u32]);

        let what = format!("the {side} wheel's side forces on surface {}", i + 1);
        let shot = c.next(&what, "wheel_set_surface_side");
        floats_are(
            shot,
            &what,
            &[
                s.slipping_side_force_round_off,
                s.slipping_side_force_drop_off,
                s.slipping_side_force_scale,
                s.slipping_side_force_speed_scale,
                s.braking_side_force_round_off,
                s.braking_side_force_drop_off,
                s.braking_side_force_scale,
                s.braking_side_force_speed_scale,
            ],
        );
        ints_are(shot, &what, &[wheel, i as u32]);
        stack_floats_are(
            shot,
            &what,
            0,
            &[s.side_force_round_off, s.side_force_drop_off, s.side_force_scale, s.side_force_speed_scale],
        );
        // `[Front Tire]` has no DonutsSideForceSpeedScale%d key at all, and the front's thirteenth
        // argument is a zero written straight into the argument slot, which is what the port keeps
        // for it.
        stack_floats_are(shot, &what, 4, &[s.donuts_side_force_speed_scale]);

        let what = format!("the {side} wheel's braking and drag on surface {}", i + 1);
        let shot = c.next(&what, "wheel_set_surface_drag");
        floats_are(shot, &what, &[s.max_braking_torque, s.wheel_friction, s.wheel_drag]);
        ints_are(shot, &what, &[wheel, i as u32]);
    }
}

/// The seven the collision record is set through. Its own words are the setters' business; what the
/// loader owes it is these arguments, including the one number it works out itself — pounds into the
/// record's own mass unit — and the cosine it takes out of the curve table.
fn collision_calls(c: &mut Calls, ee: &Ee, k: &phy::Collisions) {
    let what = "the collision mass";
    let shot = c.next(what, "collision_set_mass");
    floats_are(shot, what, &[k.collision_mass]);
    ints_are(shot, what, &[COLL]);
    vector_at(ee, shot.ints[1], "CollisionCofGOffset", k.collision_cofg_offset);

    let what = "the collision springs";
    let shot = c.next(what, "collision_set_springs");
    floats_are(
        shot,
        what,
        &[
            k.max_compression,
            k.spring,
            k.damping,
            k.restitution,
            k.body_max_compression,
            k.body_spring,
            k.body_damping,
            k.body_restitution,
        ],
    );
    ints_are(shot, what, &[COLL]);

    let what = "the high speed collision check";
    let shot = c.next(what, "collision_set_high_speed");
    floats_are(
        shot,
        what,
        &[k.high_speed_ray_speed_scale, k.high_speed_unpenetration_distance, k.high_speed_min_collision_angle],
    );
    ints_are(shot, what, &[COLL, u32::from(k.check_for_high_speed_collisions)]);
    vector_at(ee, shot.ints[2], "HighSpeedRayOffset", k.high_speed_ray_offset);

    let what = "the angular impulse switches";
    let shot = c.next(what, "collision_set_angular");
    ints_are(
        shot,
        what,
        &[COLL, u32::from(k.angular_impulse_enabled), u32::from(k.body_angular_impulse_enabled)],
    );

    let what = "the moment scales";
    let shot = c.next(what, "collision_set_moment_scales");
    ints_are(shot, what, &[COLL]);
    for (slot, (name, want)) in [
        ("SpringMomentScale", k.spring_moment_scale),
        ("FrictionMomentScale", k.friction_moment_scale),
        ("BodySpringMomentScale", k.body_spring_moment_scale),
        ("BodyFrictionMomentScale", k.body_friction_moment_scale),
    ]
    .into_iter()
    .enumerate()
    {
        vector_at(ee, shot.ints[1 + slot], name, want);
    }

    let what = "the collision frictions";
    let shot = c.next(what, "collision_set_friction");
    floats_are(shot, what, &[k.friction, k.body_friction]);
    ints_are(shot, what, &[COLL]);

    // One friction into all sixteen surface slots, whatever NumberOfSurfaces said.
    for i in 0..phy::MAX_SURFACES {
        let what = format!("the terrain friction of surface {}", i + 1);
        let shot = c.next(&what, "collision_set_surface_friction");
        floats_are(shot, &what, &[k.terrain_friction]);
        ints_are(shot, &what, &[COLL, i as u32]);
    }
}

/// Every argument of every call the loader makes to the wheels and to the collision record, and the
/// six words it writes onto the wheels itself.
#[test]
fn the_setters_are_handed_what_the_port_works_out() {
    let Some((mut ee, curve)) = harness() else { return };
    let Some(mut f) = container() else { return };
    let psr = directory(&mut f);
    let files = phy_files(&mut f, &psr, &[RETAIL[0]]);
    let (_, bytes) = files.first().expect("Race125P.phy is on the disc");
    let p = Phy::read_bytes(bytes, &curve);

    serve(&mut ee, bytes);
    lay_out(&mut ee);
    let log = Rc::new(RefCell::new(Vec::new()));
    for name in WHEEL_SETTERS.into_iter().chain(COLLISION_SETTERS) {
        capture(&mut ee, &log, name);
    }
    ee.call(addr("bike_phy_load").unwrap(), &[OBJ as u64], &[], 2_000_000_000);

    let shots = log.borrow();
    let mut c = Calls { log: &shots, k: 0 };
    tire_calls(&mut c, &p.front, FRONT, true);
    tire_calls(&mut c, &p.rear, REAR, false);
    collision_calls(&mut c, &ee, &p.collisions);
    assert_eq!(c.k, shots.len(), "{} calls the proof has not accounted for", shots.len() - c.k);

    // The words the loader puts on a wheel without going through a setter. The steer limit arrives
    // in degrees and leaves in radians, with its own negative beside it for the other side.
    assert_eq!(
        ee.u32_at(FRONT + 0x40),
        u32::from(p.front.use_parametric_curve),
        "the front tire's UseParametricCurve"
    );
    assert_eq!(ee.u32_at(REAR + 0x40), u32::from(p.rear.use_parametric_curve), "the rear's");
    assert_eq!(ee.u32_at(FRONT + 0x7c), p.front.max_steer_angle.to_bits(), "MaxSteerAngle in radians");
    assert_eq!(ee.u32_at(FRONT + 0x80), (-p.front.max_steer_angle).to_bits(), "and its negative");
    vector_at(&ee, FRONT + 0xb0, "the front BodyTireForcePosition", p.front.body_tire_force_position);
    vector_at(&ee, REAR + 0xb0, "the rear BodyTireForcePosition", p.rear.body_tire_force_position);
    assert_eq!(
        ee.u32_at(REAR + 0xc8),
        p.rear.other_tire_normal_load_scale.to_bits(),
        "OtherTireNormalLoadScale"
    );
    println!("the fourteen setters: {} calls, every argument bit-exact", shots.len());
}

// ---------------------------------------------------------------------------------------------------
// 4. The bike the human rode

/// The words of the live bike the loader leaves one way and riding leaves another: the cursor the
/// engine walks its power curve with, at the start of the curve when the file has just been read and
/// wherever the revs had taken it by the time the image was saved.
const RIDDEN: [(u32, u32); 2] = [(ENGINE, 0x90), (ENGINE, 0x94)];

/// The EE image saved beside the human's recording holds a bike the original's loader filled, one
/// race earlier, from the file this port now reads. No interpreter here: the image is the original's
/// answer, and the comparison is every word the port claims to write.
#[test]
fn the_bike_in_the_recording_is_the_one_the_port_reads() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return;
        }
    };
    let at = kit.ctx("states").join(STATE);
    let mut img = match Ee::from_ram(&at) {
        Ok(ee) => ee,
        Err(why) => {
            skip(&why);
            return;
        }
    };

    // The bike is found by the file it is carrying, which is also the claim being checked: the
    // freeride bike the port picks is the one the human was on.
    let want = physics_file(FREERIDE_ENGINE, FREERIDE_PRO, Tuning::P);
    let name = read_str(&img, LIVE_BIKE + LIVE_NAME);
    if name != want {
        skip(&format!(
            "the object at {LIVE_BIKE:#010x} in {} is carrying {name:?}, not the {want:?} this proof \
             is about — a different moment, or a different build",
            at.display()
        ));
        return;
    }
    assert_eq!(want, PHY, "the port's freeride bike");

    // The game's own curve table, out of the same image, so the two cosines are the table's.
    let table = img.u32_at(addr("curve_table_ptr").unwrap());
    assert_ne!(table, 0, "the image was taken before the curve table was built");
    let curve = curve_from(&img, table);

    let Some(mut f) = container() else { return };
    let psr = directory(&mut f);
    let files = phy_files(&mut f, &psr, &[RETAIL[0]]);
    let (_, bytes) = files.first().expect("Race125P.phy is on the disc");
    let p = Phy::read_bytes(bytes, &curve);

    // Every word of the live bike, its chassis and its engine, read before anything is written.
    let bases = [(OBJ, LIVE_BIKE), (CHASSIS, img.u32_at(LIVE_BIKE + 0x7a4)), (ENGINE, img.u32_at(LIVE_BIKE + 0x7b8))];
    let mut rows = Vec::new();
    for (what, base, len) in REGIONS {
        let live = bases.iter().find(|(b, _)| *b == base).expect("a region with no live object").1;
        assert_ne!(live, 0, "the live bike has no {what}");
        for o in (0..len).step_by(4) {
            rows.push((what, base, o, img.u32_at(live + o)));
        }
    }
    // The port's answer goes into a copy of the graph filled with a value no float is, so the words
    // it does not write are the words it does not claim: those are the ones skipped below, and they
    // are the running state of a bike that had been ridden for a while.
    for &(_, base, o, _) in &rows {
        img.set_u32(base + o, SENTINEL);
    }
    poke_ours(&mut img, &p);

    let (mut checked, mut bad) = (0, 0);
    for &(what, base, o, theirs) in &rows {
        let ours = img.u32_at(base + o);
        if ours == SENTINEL || RIDDEN.contains(&(base, o)) {
            continue;
        }
        checked += 1;
        if ours != theirs {
            if bad < 16 {
                println!(
                    "{what}+{o:#05x}: the human's bike {theirs:#010x} ({}) ours {ours:#010x} ({})",
                    f32::from_bits(theirs),
                    f32::from_bits(ours)
                );
            }
            bad += 1;
        }
    }
    assert_eq!(bad, 0, "{bad} of {checked} words differ from the bike the human rode");
    assert!(checked > 150, "only {checked} words were compared — the port wrote almost nothing");
    println!("the human's bike: {checked} words, {} off the disc, bit-exact", RETAIL[0]);
}

// ---------------------------------------------------------------------------------------------------
// 5. The numbers themselves

/// The retail values, off the disc, for the pair the freeride bike is. This is the test that would
/// have been a table of hand-typed constants: `MaxCrossupAngle` is the one S6 could not prove, and
/// the 45 degrees it finds here is the file's, turned into radians the way the loader turns it.
#[test]
fn the_race125_pair_holds_the_retail_numbers() {
    let Some(mut f) = container() else { return };
    let psr = directory(&mut f);
    let files = phy_files(&mut f, &psr, &RETAIL);
    assert_eq!(files.len(), RETAIL.len(), "both Race125 files should be in Bike\\*.res");

    // The port's own table, which is not bit-exact against the game's: nothing asserted below comes
    // through it. The proofs that do go through it take the table out of the game's RAM.
    let curve = CurveTable::build();
    for (what, bytes) in &files {
        let p = Phy::read_bytes(bytes, &curve);
        assert_eq!(p.mass.weight, F(400.0), "{what}: Weight, in pounds");
        assert_eq!(p.mass.mass, F(400.0 / 32.2), "{what}: the mass the chassis gets");
        assert_eq!(p.engine.curve.points, 10, "{what}: NumberOfRPMPoints");
        assert_eq!(p.engine.gears, 5, "{what}: NumberOfGears");
        assert_eq!(p.front.surfaces.len(), phy::MAX_SURFACES, "{what}: the front's surfaces");
        assert_eq!(p.rear.surfaces.len(), phy::MAX_SURFACES, "{what}: the rear's");
        assert_eq!(p.max_crossup.to_bits(), 0x3f49_0fd9, "{what}: MaxCrossupAngle, 45 degrees");
        // 0.025 in the file is the float *below* a fortieth: the conversion truncates.
        assert_eq!(p.mv.drag.to_bits(), 0x3ccc_cccc, "{what}: Drag");
        // The two sets of parameters the rest of the port has been waiting for, both filled from the
        // same file by the same pass.
        assert_ne!(p.mv.gravity, Vf::new(0.0, 0.0, 0.0, 0.0), "{what}: the ground step's gravity");
        assert_ne!(p.air.airborne_control_force, F::ZERO, "{what}: the air step's control force");
        assert_ne!(p.mv.up_shift_rpm, F::ZERO, "{what}: UpShiftRPM");
        println!("{what}: {} pounds, {} gears, crossup {} rad", p.mass.weight.0, p.engine.gears, p.max_crossup.0);
    }
}

/// Every distinct `.phy` on the disc, not just the pair [`RETAIL`] names: the copy proved below is
/// the same copy for every bike, and a file that leaves one of these keys out is the only way to see
/// that the loader's default travels with it.
fn every_phy_file(f: &mut File, psr: &Psr) -> Vec<(String, Vec<u8>)> {
    let archives: Vec<String> = psr
        .entries
        .iter()
        .map(|e: &Entry| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.starts_with("bike\\") && n.ends_with(".res")
        })
        .collect();
    let mut leaves: Vec<String> = Vec::new();
    for archive in &archives {
        let bytes = member(f, psr, archive);
        let Ok(res) = Res::parse(&bytes) else { continue };
        for e in &res.entries {
            let leaf = e.name.to_ascii_lowercase();
            let leaf = leaf.rsplit('\\').next().unwrap_or(&leaf).to_string();
            if leaf.ends_with(".phy") && !leaves.contains(&leaf) {
                leaves.push(leaf);
            }
        }
    }
    let want: Vec<&str> = leaves.iter().map(|s| s.as_str()).collect();
    phy_files(f, psr, &want)
}

/// S21 — the twenty-five words the four rider controllers read, on a bike that came off the disc.
///
/// The reader takes them into its own `[Roll Control]`, `[Pitch Control]`, `[Donuts]` and
/// `[No Hander Lander]` structs, which
/// [`the_whole_loader_fills_the_bike_the_way_the_original_does`] already compares against the
/// original loader's own bike, word for word at the offsets the original writes. So what is left to
/// say is that the ground step's copy is that same word and not a second reading of the file — which
/// is what this asserts, bit for bit, for every bike on the disc.
///
/// `MaxRollRateCommand`, `MaxRollYawRate` and `StoppieBrake` are the three keys of those sections
/// that are deliberately *not* here: nothing on the ground reads the first two, and `StoppieBrake`
/// belongs to the pad layer (see [`phy::Pitch::stoppie_brake`]).
#[test]
fn the_ground_step_gets_the_controller_keys_off_the_disc() {
    let Some(mut f) = container() else { return };
    let psr = directory(&mut f);
    let files = every_phy_file(&mut f, &psr);
    assert!(files.len() >= RETAIL.len(), "the disc should hold at least the Race125 pair");

    // The port's own table, which nothing asserted here goes through — none of the twenty-five is a
    // curve lookup.
    let curve = CurveTable::build();
    // One flag per key: a key that is zero in every file on the disc would make its assertion hold
    // whether the copy happened or not, so each one has to be seen carrying a number somewhere.
    let mut carried: BTreeMap<&str, bool> = BTreeMap::new();
    for (what, bytes) in &files {
        let p = Phy::read_bytes(bytes, &curve);
        let m = &p.mv;
        let (r, pc, d, b) = (&p.roll, &p.pitch, &p.donuts, &p.balance);
        let keys: [(&str, F, F); 25] = [
            // [Roll Control]
            ("MaxRollCommand", m.max_roll_command, r.max_roll_command),
            ("MaxRollSpeed", m.max_roll_speed, r.max_roll_speed),
            ("RollGain", m.roll_gain, r.roll_gain),
            ("MinStandUprightSpeed", m.min_stand_upright_speed, r.min_stand_upright_speed),
            ("MaxStandUprightSpeed", m.max_stand_upright_speed, r.max_stand_upright_speed),
            ("RollControlGain", m.roll_control_gain, r.roll_control_gain),
            ("RollControlRateGain", m.roll_control_rate_gain, r.roll_control_rate_gain),
            ("RollControlMaxCG", m.roll_control_max_cg, r.roll_control_max_cg),
            ("MaxRollRateDisturbance", m.max_roll_rate_disturbance, r.max_roll_rate_disturbance),
            // [Pitch Control]
            ("MaxPitchCommand", m.max_pitch_command, pc.max_pitch_command),
            ("PitchGain", m.pitch_gain, pc.pitch_gain),
            ("PitchRateGain", m.pitch_rate_gain, pc.pitch_rate_gain),
            ("WheelieTopSpeed", m.inv_wheelie_top_speed, pc.inv_wheelie_top_speed),
            ("MaxStoppiePitchCommand", m.max_stoppie_pitch_command, pc.max_stoppie_pitch_command),
            ("StoppiePitchGain", m.stoppie_pitch_gain, pc.stoppie_pitch_gain),
            ("StoppieYawRateScale", m.stoppie_yaw_rate_scale, pc.stoppie_yaw_rate_scale),
            ("StoppieTopSpeed", m.stoppie_top_speed, pc.stoppie_top_speed),
            // [Donuts]
            ("DonutRollAngleScale", m.donut_roll_angle_scale, d.roll_angle_scale),
            // [No Hander Lander]
            ("MinBalanceInertia", m.min_balance_inertia, b.min_balance_inertia),
            ("MaxBalanceInertia", m.max_balance_inertia, b.max_balance_inertia),
            ("BalanceInertiaRate", m.balance_inertia_rate, b.balance_inertia_rate),
            ("InputInertia", m.input_inertia, b.input_inertia),
            ("MaxBalancePoint", m.max_balance_point, b.max_balance_point),
            ("BalanceNoiseAmplitude", m.balance_noise_amplitude, b.balance_noise_amplitude),
            ("BalanceNoiseTimeScale", m.balance_noise_time_scale, b.balance_noise_time_scale),
        ];
        for (key, got, want) in keys {
            assert_eq!(
                got.to_bits(),
                want.to_bits(),
                "{what}: {key} reached the ground step as {} and the loader read {}",
                got.0,
                want.0
            );
            *carried.entry(key).or_insert(false) |= want.to_bits() != 0;
        }
    }
    assert_eq!(carried.len(), 25, "twenty-five keys, each named once");
    for (key, seen) in &carried {
        assert!(seen, "{key} is zero in every file on the disc, so its copy proves nothing");
    }
    println!("{} .phy files on the disc, 25 controller keys each, bit-exact", files.len());
}
