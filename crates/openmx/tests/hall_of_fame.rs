//! P334: the Hall of Fame against the original — its tables as the front end makes them at boot,
//! and the insertion a name entry leads to.
//!
//! * `hall_of_fame_defaults` over 40 makings — supercross, nationals and arena counts from none to
//!   past the tables, rider lists of 1 to 99 names spaced every way the tokenizer reads them, the
//!   clock's seed drawn or `rand` already seeded — against [`FameBook::generate`]: every word of
//!   the tables and every one of the 3,755 names. `rand`, `srand`, the clock, the allocator and
//!   `snprintf` are hooked; the maker, its step, its name draw, the list, the copy and the
//!   tokenizer run.
//! * `hall_of_fame_insert` over 20,000 inserts — every table, track and class (the arenas' with
//!   a class too), entries tied, signed zeros, denormals, odd words, values a few ulps either side
//!   of an entry — against [`FameBook::insert`]: the answer, the five words and the five names.
//! * The s196 RAM image's Hall of Fame, made again from its seed: every word and name.
//!
//! The book reads back as it was written.

#[allow(unused)]
#[path = "../src/files.rs"]
mod files;
#[allow(unused)]
#[path = "../src/menu_results.rs"]
mod results;
#[path = "../src/menu_fame.rs"]
mod fame;

use fame::{Dice, FameBook, FameTry, NAMES, name_index, riders_of, second_word};
use game_sim::race::{HallOfFame, fame_table};
use game_sim::rng::Rng;
use ps2_float::F;
use ps2_oracle::{Ee, Kit, addr, game_or_skip, skip};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const HOF: u32 = 0x0180_0000;
const APP: u32 = 0x0183_0000;
const GD: u32 = 0x0183_1000;
const LIST_AT: u32 = 0x0183_2000;
const LIST: u32 = 0x0183_3000;
const ARRAY: u32 = 0x0183_4000;
const ENTRIES_AT: u32 = 0x0184_0000;
const NAME: u32 = 0x0185_0000;
const HEAP: u32 = 0x0186_0000;
const NAMES_AT: u32 = 0x3ab8;

struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        (self.next() >> 16) % n
    }
    fn word(&mut self) -> String {
        let n = 1 + self.below(9) as usize;
        (0..n).map(|_| char::from(b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ-'."[self.below(55) as usize])).collect()
    }
    fn gap(&mut self) -> &'static str {
        [" ", " ", " ", "\n", "  ", " \n"][self.below(6) as usize]
    }
    /// A rider's name: one to four words, the gaps a space, a line end or two, a gap before the
    /// first sometimes and one after the last. (Two after the last, or nothing but gaps, runs the
    /// original's tokenizer past the end.)
    fn rider(&mut self) -> String {
        let mut s = String::new();
        if self.below(5) == 0 {
            s += self.gap();
        }
        let words = 1 + self.below(4);
        for k in 0..words {
            if k > 0 {
                s += self.gap();
            }
            s += &self.word();
        }
        if self.below(6) == 0 {
            s += [" ", "\n"][self.below(2) as usize];
        }
        if self.below(40) == 0 {
            s.clear();
        }
        s
    }
    fn name(&mut self) -> String {
        let n = [0, 1, 3, 10, 20, 29][self.below(6) as usize];
        (0..n).map(|_| char::from(b"abcdefghijklmnopqrstuvwxyz ABCDEFGHIJ0123456789"[self.below(47) as usize])).collect()
    }
}

fn c_str(ee: &Ee, mut at: u32) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let b = ee.u8_at(at);
        if b == 0 || out.len() > 0x400 {
            return out;
        }
        out.push(b);
        at += 1;
    }
}

fn put_c(ee: &mut Ee, at: u32, s: &[u8], size: u32) {
    if size == 0 {
        return;
    }
    let n = s.len().min(size as usize - 1);
    for (k, &b) in s[..n].iter().enumerate() {
        ee.write(at + k as u32, 1, u128::from(b));
    }
    ee.write(at + n as u32, 1, 0);
}

fn zero(ee: &mut Ee, at: u32, len: u32) {
    for k in (0..len).step_by(4) {
        ee.set_u32(at + k, 0);
    }
}

fn addrs<const N: usize>(names: [&str; N]) -> Result<[u32; N], String> {
    let mut out = [0; N];
    for (o, n) in out.iter_mut().zip(names) {
        *o = addr(n)?;
    }
    Ok(out)
}

/// The state that hands out `r` at its next `rand`.
fn state_before(r: i32) -> u64 {
    const M: u64 = 0x5851_f42d_4c95_7f2d;
    let mut inv = M;
    for _ in 0..6 {
        inv = inv.wrapping_mul(2u64.wrapping_sub(M.wrapping_mul(inv)));
    }
    ((r as u64) << 32).wrapping_sub(1).wrapping_mul(inv)
}

#[test]
fn state_before_hands_out_its_draw() {
    for r in [0, 1, 12345, 0x7fff_ffff] {
        assert_eq!(Rng::from_state(state_before(r)).rand(), r);
    }
}

#[test]
fn the_tokenizer_takes_the_second_word() {
    assert_eq!(second_word("Travis Preston"), Some("Preston"));
    assert_eq!(second_word("  Jean\n Michel Bayle"), Some("Michel"));
    assert_eq!(second_word("Bob "), None);
    assert_eq!(second_word(""), None);
    assert_eq!(riders_of("[Pro Riders]\r\nrider=\"Travis Preston\"\r\n[Computer Riders]\r\nRider = \"Josh Hansen\"\r\nx=1\r\n"), ["Travis Preston", "Josh Hansen"]);
}

#[test]
fn the_defaults_match_the_original() {
    let Some(mut ee) = game_or_skip() else { return };
    let [defaults, rand, srand, clock, alloc, free, snprintf, app_pointer, rand_seeded] = match addrs([
        "hall_of_fame_defaults",
        "rand",
        "srand",
        "cd_read_clock",
        "operator_new_array",
        "operator_delete_array",
        "snprintf",
        "app_pointer",
        "rand_seeded",
    ]) {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let rng = Rc::new(RefCell::new(Rng::new()));
    let r = rng.clone();
    ee.hooks.insert(rand, Box::new(move |ee| {
        let v = r.borrow_mut().rand();
        ee.set(2, v as u32 as u64);
    }));
    let r = rng.clone();
    ee.hooks.insert(srand, Box::new(move |ee| r.borrow_mut().seed(ee.r(4) as u32)));
    let time = Rc::new(Cell::new([0u8; 4]));
    let t = time.clone();
    ee.hooks.insert(clock, Box::new(move |ee| {
        let at = ee.r(4) as u32;
        let [s, m, h, d] = t.get();
        for (k, b) in [0, s, m, h, 0, d, 1, 0x26].into_iter().enumerate() {
            ee.write(at + k as u32, 1, u128::from(b));
        }
        ee.set(2, 1);
    }));
    let heap = Rc::new(Cell::new(HEAP));
    let h = heap.clone();
    ee.hooks.insert(alloc, Box::new(move |ee| {
        let at = h.get();
        h.set(at + ((ee.r(4) as u32 + 15) & !15));
        ee.set(2, u64::from(at));
    }));
    ee.hooks.insert(free, Box::new(|_| {}));
    ee.hooks.insert(snprintf, Box::new(|ee| {
        let (dst, size, fmt, arg) = (ee.r(4) as u32, ee.r(5) as u32, ee.r(6) as u32, ee.r(7) as u32);
        assert_eq!(c_str(ee, fmt), b"%s");
        let out = if arg == 0 { b"(null)".to_vec() } else { c_str(ee, arg) };
        put_c(ee, dst, &out, size);
        ee.set(2, out.len() as u64);
    }));
    ee.set_u32(app_pointer, APP);
    ee.set_u32(APP + 0x3d8, GD);
    ee.set_u32(GD + 0x14, LIST_AT);
    ee.set_u32(LIST_AT, LIST);
    ee.set_u32(LIST + 8, ARRAY);
    let mut pick = Pick(0x9e37_79b9_7f4a_7c15);
    for case in 0..40 {
        let riders: Vec<String> = (0..[1, 2, 7, 99][case % 4]).map(|_| pick.rider()).collect();
        ee.set_u32(LIST, riders.len() as u32);
        for (k, name) in riders.iter().enumerate() {
            let entry = ENTRIES_AT + 0x10 * k as u32;
            let text = NAME + 0x40 * k as u32;
            ee.set_u32(ARRAY + 4 * k as u32, entry);
            ee.set_u32(entry, text);
            put_c(&mut ee, text, name.as_bytes(), 0x40);
        }
        let (sx, nat, arenas) = match case {
            0 => (24, 22, 5),
            1 => (0, 0, 0),
            2 => (24, 22, 7),
            _ => (pick.below(25) as i32, pick.below(23) as i32, pick.below(8) as i32),
        };
        zero(&mut ee, HOF, 0x1f2d0);
        ee.set_u32(HOF + 0x1e00, sx as u32);
        ee.set_u32(HOF + 0x3984, nat as u32);
        ee.set_u32(HOF + 0x3ab4, arenas as u32);
        ee.set_u32(HOF + 0x1f2c4, riders.len() as u32);
        // Half the makings draw the clock's seed at their first name; the rest find `rand` seeded.
        let start = pick.next();
        let seeded = if case % 2 == 0 { 0 } else { (pick.next() as u32) | 1 };
        let bcd = |v: u64| ((v / 10) << 4 | v % 10) as u8;
        let when = [bcd(pick.below(60)), bcd(pick.below(60)), bcd(pick.below(24)), bcd(1 + pick.below(31))];
        time.set(when);
        ee.set_u32(rand_seeded, seeded);
        *rng.borrow_mut() = Rng::from_state(start);
        heap.set(HEAP);
        ee.call(defaults, &[u64::from(HOF)], &[], 200_000_000);

        let mut dice = Dice { rng: Rng::from_state(start), seeded };
        let [s, m, h, d] = when.map(u32::from);
        let mut clock = || s + m * 60 + h * 0x168 + d * 0x15180;
        let book = FameBook::generate(sx, nat, arenas, &riders, &mut dice, &mut clock);
        assert_eq!(dice.seeded, ee.u32_at(rand_seeded), "case {case}: the seed word");
        assert_eq!(dice.rng.state(), rng.borrow().state(), "case {case}: rand's state after");
        for (k, &w) in book.tables.words.iter().enumerate() {
            assert_eq!(w, ee.u32_at(HOF + 4 * k as u32), "case {case}: word +{:#x} ({sx}/{nat}/{arenas})", 4 * k);
        }
        for (i, n) in book.names.iter().enumerate() {
            let got = c_str(&ee, HOF + NAMES_AT + 30 * i as u32);
            assert_eq!(n.as_bytes(), got, "case {case}: name {i} ({sx}/{nat}/{arenas})");
        }
    }
}

/// A word an entry or a value may hold for `table`.
fn value(pick: &mut Pick, table: u32, near: &[u32]) -> u32 {
    let ulps = |w: u32, d: i32| (w as i32).wrapping_add(d) as u32;
    match pick.below(12) {
        0 => 0,
        1 => 0x8000_0000,
        2 => 1 + pick.below(0x7f_ffff) as u32,
        3 => pick.next() as u32,
        4 | 5 if !near.is_empty() => near[pick.below(near.len() as u64) as usize],
        6 | 7 if !near.is_empty() => ulps(near[pick.below(near.len() as u64) as usize], pick.below(9) as i32 - 4),
        _ if table == fame_table::COUNT => (pick.below(100_000) as f32).to_bits(),
        _ => (pick.below(80_000) as f32 / 97.0).to_bits(),
    }
}

#[test]
fn insertion_matches_the_original() {
    let Some(mut ee) = game_or_skip() else { return };
    let [insert, strcpy] = match addrs(["hall_of_fame_insert", "strcpy"]) {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    ee.hooks.insert(strcpy, Box::new(|ee| {
        let (dst, src) = (ee.r(4) as u32, ee.r(5) as u32);
        let s = c_str(ee, src);
        put_c(ee, dst, &s, s.len() as u32 + 1);
        ee.set(2, u64::from(dst));
    }));
    let mut pick = Pick(0x0123_4567_89ab_cdef);
    let mut went = [0usize; 2];
    for case in 0..20_000 {
        let table = pick.below(11) as u32;
        let tracks = match table {
            0..=3 => 24,
            4..=7 => 22,
            _ => 5,
        };
        let track = pick.below(tracks) as i32;
        let class = pick.below(4) as i32;
        let at = HallOfFame::offset(table, track, class).unwrap();
        let n = name_index(table, track, class).unwrap();
        let mut book = FameBook::default();
        let mut words = Vec::new();
        for k in 0..5 {
            let w = if table == fame_table::COUNT && pick.below(3) > 0 { pick.below(100_000) as u32 } else { value(&mut pick, table, &words) };
            words.push(w);
            book.tables.words[at / 4 + k] = w;
            book.names[n + k] = pick.name();
            ee.set_u32(HOF + (at + 4 * k) as u32, w);
            put_c(&mut ee, HOF + NAMES_AT + 30 * (n + k) as u32, book.names[n + k].as_bytes(), 30);
        }
        let near: Vec<u32> = if table == fame_table::COUNT { words.iter().map(|&w| (w as i32 as f32).to_bits()).collect() } else { words.clone() };
        let v = value(&mut pick, table, &near);
        let name = pick.name();
        put_c(&mut ee, NAME, name.as_bytes(), 30);
        ee.call(insert, &[u64::from(HOF), u64::from(table), track as i64 as u64, class as i64 as u64, u64::from(NAME)], &[f32::from_bits(v)], 4_000_000);
        let want = book.insert(F(f32::from_bits(v)), table, track, class, &name);
        went[usize::from(want)] += 1;
        assert_eq!(ee.v0() as u32, u32::from(want), "case {case}: table {table} value {v:#x} entries {words:x?}");
        for k in 0..5 {
            assert_eq!(book.tables.words[at / 4 + k], ee.u32_at(HOF + (at + 4 * k) as u32), "case {case}: table {table} entry {k}, value {v:#x} entries {words:x?}");
            assert_eq!(book.names[n + k].as_bytes(), c_str(&ee, HOF + NAMES_AT + 30 * (n + k) as u32), "case {case}: table {table} name {k}");
        }
    }
    assert!(went[0] > 2000 && went[1] > 2000, "inserts missed {} and went in {}", went[0], went[1]);
}

/// The s196 image's Hall of Fame, as the game made it at that boot: its seed is in the image, and
/// the one draw before the seed (the first step) is solved for from the first entry.
#[test]
fn the_s196_image_is_made_again_from_its_seed() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => return skip(&why),
    };
    let path = kit.ctx("states").join(["s196", "sx01.ee.bin"].join("-"));
    let Ok(ram) = std::fs::read(&path) else { return skip(&format!("{} is not there: S196's RAM image of sx01", path.display())) };
    let w = |a: u32| u32::from_le_bytes(ram[(a & 0x1ff_ffff) as usize..][..4].try_into().unwrap());
    let s = |a: u32| {
        let a = (a & 0x1ff_ffff) as usize;
        let end = ram[a..].iter().position(|&b| b == 0).unwrap();
        String::from_utf8_lossy(&ram[a..a + end]).into_owned()
    };
    let app = w(addr("app_pointer").unwrap_or(0x0041_f770));
    let hof = w(app + 0x3e0);
    let seed = w(addr("rand_seeded").unwrap_or(0x0045_fa10));
    let list = w(w(w(app + 0x3d8) + 0x14));
    let riders: Vec<String> = (0..w(list)).map(|k| s(w(w(w(list + 8) + 4 * k)))).collect();
    assert_eq!((w(hof + 0x1e00), w(hof + 0x3984), w(hof + 0x3ab4), w(hof + 0x1f2c4)), (24, 22, 5, 99));
    // The list is the disc's riders.dat, in its order.
    if let Ok(dat) = std::fs::read(kit.ctx("res/appstart/rider/riders.dat")) {
        assert_eq!(riders_of(&String::from_utf8_lossy(&dat)), riders);
    }
    let first = w(hof);
    let base = F(f32::from_bits(fame::SX_BASES[0][0]));
    let guess = ((f32::from_bits(first) - base.0) / 7.0 * 2f32.powi(31)) as i64;
    let r = (guess - 4096..guess + 4096)
        .map(|r| r.clamp(0, 0x7fff_ffff) as i32)
        .find(|&r| Dice::new(Rng::from_state(state_before(r))).step(base).0.to_bits() == first)
        .expect("a draw that steps the base to the first entry");
    let mut dice = Dice::new(Rng::from_state(state_before(r)));
    let book = FameBook::generate(24, 22, 5, &riders, &mut dice, &mut || seed);
    for (k, &want) in book.tables.words.iter().enumerate() {
        assert_eq!(want, w(hof + 4 * k as u32), "word +{:#x}", 4 * k);
    }
    for (i, n) in book.names.iter().enumerate() {
        assert_eq!(*n, s(hof + NAMES_AT + 30 * i as u32), "name {i}");
    }
}

#[test]
fn the_book_reads_back_and_a_name_goes_in() {
    let riders: Vec<String> = ["Travis Preston", "Josh Hansen", "Danny Smith"].map(String::from).to_vec();
    let mut dice = Dice::new(Rng::new());
    let mut book = FameBook::generate(24, 22, 5, &riders, &mut dice, &mut || 180_724);
    assert_eq!(dice.seeded, 180_724);
    assert!(book.names.iter().all(|n| ["Preston", "Hansen", "Smith"].contains(&n.as_str())));
    assert_eq!(book.names.len(), NAMES);
    assert_eq!(FameBook::parse(&book.text()), Some(book.clone()));
    // A best lap a second under the table's best earns it, and goes in first; a 3-lap race time a
    // second under the 3-lap table's earns that too; the 5-lap table is not tried in a 3-lap race.
    let best = f32::from_bits(book.tables.words[HallOfFame::offset(0, 3, 2).unwrap() / 4]);
    let three = f32::from_bits(book.tables.words[HallOfFame::offset(1, 3, 2).unwrap() / 4]);
    let t = FameTry { seat: 0, league: 0, track: 3, class: 2, laps: 3, best_lap: F(best - 1.0), race_time: F(three - 1.0) };
    assert_eq!(book.earned(&t).iter().map(|e| e.0).collect::<Vec<_>>(), [0, 1]);
    assert_eq!(book.enter(&t, "Me"), [(0, Some(0)), (1, Some(0))]);
    assert_eq!(book.entries(0, 3, 2)[0], ((best - 1.0).to_bits(), "Me".to_string()));
    assert_eq!(book.last_name, "Me");
    // The supercross flags read whole seconds: the same time earns the best lap again (it beats the
    // entry it pushed down), though it cannot beat itself.
    assert_eq!(book.earned(&t).first().map(|e| e.0), Some(0));
    assert_eq!(book.enter(&t, "Me").first(), Some(&(0, Some(1))));
    // Off the race leagues nothing is tried.
    assert!(book.earned(&FameTry { league: 2, ..t }).is_empty());
    assert_eq!(FameBook::parse(&book.text()), Some(book.clone()));
    assert_eq!(FameBook::parse("w.0 = 0\n"), None);
    assert_eq!(fame::fame_track("sx07"), Some((0, 6)));
    assert_eq!(fame::fame_track("nat22"), Some((1, 21)));
    assert_eq!(fame::fame_track("nat23"), None);
    assert_eq!(fame::fame_track("free09"), None);
}
