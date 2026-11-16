//! P334: the Hall of Fame — its tables as the original makes them at boot, the name entry a new
//! record leads to and the insertion that name goes in with.
//!
//! The tables are not stored on the disc. At boot the front end makes them from a base time a track
//! and a class ([`SX_BASES`], [`NAT_BASES`]), stepping each entry on from the one before by the
//! game's `rand` ([`Dice::step`]), and names every entry after a rider drawn from the disc's
//! `rider\riders.dat` list — the second word of the rider's name ([`Dice::name`]). The first name
//! drawn seeds `rand` from the console's clock, once. The freestyle arenas' tables are fixed numbers
//! ([`ARENA_SCORES`], [`ARENA_COUNTS`], [`ARENA_TIMES`]), named the same way. [`FameBook::generate`]
//! is that, call for call.
//!
//! A record a race's finish marks ([`game_sim::race::Supercross::race_over`]) brings the Hall of
//! Fame menu up after the results: the records the seat's card beats ([`FameBook::earned`], the
//! entry page's flags), the name typed, and each record put in its table in turn
//! ([`FameBook::insert`]: the first entry the value beats moves down with the rest, the last one
//! falls off).
//!
//! The shell keeps the tables in `fame.txt`, made the first time the game starts without one.

#![allow(dead_code)] // the freestyle arenas' tables are made and kept; no freestyle record is tried yet

use super::results::time_text;
use game_sim::race::{HallOfFame, fame_table};
use game_sim::rng::Rng;
use ps2_float::{F, fpu};
use std::path::Path;

/// Entries a table keeps for a track and class (or an arena).
pub const ENTRIES: usize = 5;

/// How long a kept name's slot is, its terminating zero counted.
pub const NAME_BYTES: usize = 30;

/// The longest name the Hall of Fame's keyboard takes.
pub const NAME_KEYS: usize = 10;

/// The tracks the supercross and nationals tables have room for, and the freestyle arenas.
pub const SX_TRACKS: i32 = 24;
pub const NAT_TRACKS: i32 = 22;
pub const ARENAS: i32 = 5;

/// Kept names, every table's together.
pub const NAMES: usize = 3755;

/// Where the tables keep how many supercross and nationals tracks they were made for, in bytes.
const SX_COUNT_AT: usize = 0x1e00;
const NAT_COUNT_AT: usize = 0x3984;

/// The freestyle arenas, in the order their tables keep them (the disc's `[Freestyle Leagues]`).
pub const ARENA_TRACKS: [&str; 5] = ["free09", "free10", "free11", "free07", "free06"];

/// Each supercross track's base best lap, a class (50, 125, 250, 500) a row, in seconds.
pub const SX_BASES: [[u32; 24]; 4] = [
    [0x427e0000, 0x42773333, 0x427f3333, 0x4255999a, 0x4234cccd, 0x425a6666, 0x422e0000, 0x42773333, 0x427c0000, 0x424d3333, 0x427a6666, 0x426e6666, 0x427e0000, 0x424d3333, 0x4269999a, 0x42990000, 0x42780000, 0x427d3333, 0x42953333, 0x4255999a, 0x4235999a, 0x426acccd, 0x425f3333, 0x425d3333],
    [0x427d999a, 0x426ecccd, 0x426f999a, 0x424ccccd, 0x42340000, 0x425b999a, 0x42360000, 0x4272cccd, 0x427e6666, 0x42506666, 0x4269999a, 0x42626666, 0x426d999a, 0x42533333, 0x425acccd, 0x428f0000, 0x4271999a, 0x4275999a, 0x428f3333, 0x424ccccd, 0x422acccd, 0x4265999a, 0x4255999a, 0x425f3333],
    [0x42786666, 0x426b3333, 0x42786666, 0x424c0000, 0x422c0000, 0x42500000, 0x42320000, 0x4273999a, 0x42726666, 0x42520000, 0x426a0000, 0x425ecccd, 0x4271999a, 0x42693333, 0x42546666, 0x428c6666, 0x4265999a, 0x4273999a, 0x4281999a, 0x4249999a, 0x42253333, 0x426ccccd, 0x424c0000, 0x4257999a],
    [0x426e6666, 0x4262cccd, 0x426f3333, 0x42466666, 0x4229999a, 0x42460000, 0x421e0000, 0x42666666, 0x42700000, 0x42473333, 0x42680000, 0x425a6666, 0x4269999a, 0x42473333, 0x42460000, 0x428d0000, 0x42680000, 0x42700000, 0x42826666, 0x423e6666, 0x42266666, 0x4262cccd, 0x423b999a, 0x42526666],
];

/// Each national's, the same.
pub const NAT_BASES: [[u32; 22]; 4] = [
    [0x42940000, 0x42a60000, 0x42d00000, 0x42b80000, 0x42740000, 0x42540000, 0x42500000, 0x428c0000, 0x42700000, 0x42c80000, 0x42a00000, 0x42900000, 0x42a00000, 0x42b40000, 0x42b80000, 0x42780000, 0x42960000, 0x42a20000, 0x42a60000, 0x42900000, 0x42b60000, 0x42080000],
    [0x42960000, 0x42aa0000, 0x42ce0000, 0x42be0000, 0x42740000, 0x42600000, 0x42500000, 0x42920000, 0x42700000, 0x42c80000, 0x42a80000, 0x42960000, 0x42a20000, 0x42b20000, 0x42b60000, 0x42740000, 0x428e0000, 0x42a00000, 0x42a00000, 0x428e0000, 0x42ae0000, 0x420c0000],
    [0x42900000, 0x42a23333, 0x42ce6666, 0x42ba999a, 0x42706666, 0x42673333, 0x424e6666, 0x42910000, 0x4271999a, 0x42c3cccd, 0x42a46666, 0x4290999a, 0x429c3333, 0x42993333, 0x42b2cccd, 0x4274cccd, 0x42930000, 0x42a50000, 0x42a6999a, 0x4293999a, 0x42bacccd, 0x4210cccd],
    [0x42900000, 0x429c0000, 0x42cc0000, 0x42b40000, 0x42680000, 0x42480000, 0x42480000, 0x428a0000, 0x42600000, 0x42ba0000, 0x429a0000, 0x428a0000, 0x42960000, 0x42aa0000, 0x42b00000, 0x42700000, 0x428e0000, 0x429a0000, 0x42a20000, 0x428a0000, 0x42b20000, 0x42080000],
];

/// The freestyle arenas' scores, an arena a row (floats).
pub const ARENA_SCORES: [[u32; 5]; 5] = [
    [0x43a3999a, 0x4396cccd, 0x438fb333, 0x438a8ccd, 0x4352b333],
    [0x43993333, 0x438f999a, 0x438aa666, 0x437ecccd, 0x4367999a],
    [0x43a6199a, 0x4396cccd, 0x438f3333, 0x437e199a, 0x434bb333],
    [0x43a54000, 0x4399199a, 0x4380c000, 0x4368199a, 0x4348b333],
    [0x439fb333, 0x4394cccd, 0x43844ccd, 0x4374cccd, 0x43500000],
];

/// Their counts (whole numbers).
pub const ARENA_COUNTS: [[u32; 5]; 5] = [
    [85550, 83250, 78500, 65200, 61000],
    [75950, 72500, 68700, 64250, 59650],
    [79856, 72550, 68953, 64532, 61230],
    [72563, 68550, 63985, 60250, 58748],
    [77650, 72684, 68954, 65500, 61300],
];

/// Their times (floats, in seconds).
pub const ARENA_TIMES: [[u32; 5]; 5] = [
    [0x43dea666, 0x4400eccd, 0x44107333, 0x4416a666, 0x441f399a],
    [0x43f9c000, 0x44021333, 0x440ccccd, 0x44132ccd, 0x44192000],
    [0x43d4a666, 0x4400eccd, 0x440e399a, 0x4410e666, 0x44154666],
    [0x43e8d99a, 0x43f3d99a, 0x44016ccd, 0x44079333, 0x440b0000],
    [0x44018000, 0x4405cccd, 0x440cb333, 0x44149333, 0x441b2ccd],
];

/// Where `table`'s names start among the kept names: the tables' names lie in the order 5 laps,
/// best lap, 3 laps, 10 laps (supercross, then nationals), then the arenas' scores, counts and
/// times. `None` past the last table.
pub fn name_block(table: u32) -> Option<usize> {
    Some(match table {
        fame_table::SX_5_LAPS => 0,
        fame_table::SX_BEST_LAP => 480,
        fame_table::SX_3_LAPS => 960,
        fame_table::SX_10_LAPS => 1440,
        fame_table::NATIONALS_BEST_LAP => 1920,
        fame_table::NATIONALS_5_LAPS => 2360,
        fame_table::NATIONALS_3_LAPS => 2800,
        fame_table::NATIONALS_10_LAPS => 3240,
        fame_table::SCORE => 3680,
        fame_table::COUNT => 3705,
        fame_table::TIME => 3730,
        _ => return None,
    })
}

/// The first of the five names `table` keeps for `track` and `class` (an arena's tables have no
/// class), as [`HallOfFame::offset`] finds their entries; `None` where it finds none.
pub fn name_index(table: u32, track: i32, class: i32) -> Option<usize> {
    HallOfFame::offset(table, track, class)?;
    let block = name_block(table)? as i64;
    let at = if table >= fame_table::SCORE { block + i64::from(track) * 5 } else { block + i64::from(track) * 20 + i64::from(class) * 5 };
    usize::try_from(at).ok().filter(|&at| at + ENTRIES <= NAMES)
}

/// A name as the original's `snprintf` into a 30-byte slot leaves it: 29 bytes at most.
pub fn slot_name(name: &str) -> String {
    let mut end = name.len().min(NAME_BYTES - 1);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    name[..end].to_string()
}

/// The second word of a rider's full name, as the original's tokenizer splits it at spaces and
/// line ends: delimiters before a word skipped, `None` when there is no second word.
pub fn second_word(full: &str) -> Option<&str> {
    let mut words = full.split([' ', '\n']).filter(|w| !w.is_empty());
    words.next()?;
    words.next()
}

/// The riders' full names in the order the game's list keeps them: every `rider="…"` line of the
/// disc's `rider\riders.dat`, the professionals' section first.
pub fn riders_of(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let (key, value) = line.trim().split_once('=')?;
            key.trim().eq_ignore_ascii_case("rider").then(|| value.trim().trim_matches('"').to_string())
        })
        .collect()
}

/// A number as the console's clock keeps it: two decimal digits in a byte.
pub fn bcd(v: u32) -> u32 {
    (v / 10 % 10) << 4 | v % 10
}

/// The seed the first name draws when nothing has seeded `rand` yet: the clock's second, minute,
/// hour and day of the month, each as the clock's byte (decimal digits, [`bcd`]), weighted as the
/// original weighs them.
pub fn clock_seed(second: u32, minute: u32, hour: u32, day: u32) -> u32 {
    bcd(second).wrapping_add(bcd(minute).wrapping_mul(60)).wrapping_add(bcd(hour).wrapping_mul(0x168)).wrapping_add(bcd(day).wrapping_mul(0x15180))
}

/// [`clock_seed`] of the time now (UTC: the console's clock is its own).
pub fn clock_now() -> u32 {
    // The browser's std clock panics; web-time reads the page's (as crate::clock does, which the
    // test crates that mount this module do not have).
    #[cfg(not(target_arch = "wasm32"))]
    use std::time::{SystemTime, UNIX_EPOCH};
    #[cfg(target_arch = "wasm32")]
    use web_time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (days, rest) = (secs / 86_400, secs % 86_400);
    // The day of the month, from the days since 1970 (Hinnant's civil-from-days).
    let z = days as i64 + 719_468;
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    clock_seed((rest % 60) as u32, (rest / 60 % 60) as u32, (rest / 3600) as u32, day)
}

/// The game's `rand` as the tables' maker draws on it, and the word that says whether a name has
/// seeded it (`0` until one has; the seed after).
#[derive(Clone, Debug, PartialEq)]
pub struct Dice {
    pub rng: Rng,
    pub seeded: u32,
}

impl Dice {
    pub fn new(rng: Rng) -> Dice {
        Dice { rng, seeded: 0 }
    }

    /// The next entry after `x`: `x` plus seven times a draw, the draw no less than 0.1.
    pub fn step(&mut self, x: F) -> F {
        let u = self.rng.unit();
        let u = if fpu::operand(u.0.to_bits()) < fpu::operand(0.1f32.to_bits()) { F(0.1) } else { u };
        F(fpu::add(x.0, fpu::mul(7.0, u.0)))
    }

    /// An entry's name: `rand` seeded from `clock` first if nothing has seeded it yet, a rider
    /// drawn from `riders`, and the second word of their name cut to a slot (newlib prints a
    /// missing word as `(null)`).
    pub fn name(&mut self, riders: &[String], clock: &mut dyn FnMut() -> u32) -> String {
        if self.seeded == 0 {
            self.seeded = clock();
            self.rng.seed(self.seeded);
        }
        let k = self.rng.index(riders.len() as i32);
        let full = usize::try_from(k).ok().and_then(|k| riders.get(k)).map_or("", String::as_str);
        slot_name(second_word(full).unwrap_or("(null)"))
    }
}

/// A seat's card as the Hall of Fame menu reads it: the event's league (0 supercross, 1 nationals),
/// the track (its number in the league, from 0), the bike class and the race's laps, and the card's
/// best lap and race time.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FameTry {
    pub seat: i32,
    pub league: i32,
    pub track: i32,
    pub class: i32,
    pub laps: u8,
    pub best_lap: F,
    pub race_time: F,
}

/// A track's league and number for the Hall of Fame (`sx07` is supercross track 6, `nat03`
/// nationals track 2), or `None` for a track with no race tables.
pub fn fame_track(track: &str) -> Option<(i32, i32)> {
    let t = track.to_ascii_lowercase();
    let (league, digits) = if let Some(d) = t.strip_prefix("sx") { (0, d) } else { (1, t.strip_prefix("nat")?) };
    let n: i32 = digits.parse().ok()?;
    let most = if league == 0 { SX_TRACKS } else { NAT_TRACKS };
    (1..=most).contains(&n).then_some((league, n - 1))
}

/// What a table's entries are, for the screen.
pub fn table_title(table: u32) -> &'static str {
    match table {
        fame_table::SX_BEST_LAP | fame_table::NATIONALS_BEST_LAP => "Best Lap",
        fame_table::SX_3_LAPS | fame_table::NATIONALS_3_LAPS => "3 Lap Race",
        fame_table::SX_5_LAPS | fame_table::NATIONALS_5_LAPS => "5 Lap Race",
        fame_table::SX_10_LAPS | fame_table::NATIONALS_10_LAPS => "10 Lap Race",
        fame_table::SCORE => "Score",
        fame_table::COUNT => "Count",
        _ => "Time",
    }
}

/// The Hall of Fame: its tables word for word, every entry's name, and the name last typed.
#[derive(Clone, Debug, PartialEq)]
pub struct FameBook {
    pub tables: HallOfFame,
    pub names: Vec<String>,
    pub last_name: String,
}

impl Default for FameBook {
    fn default() -> FameBook {
        FameBook { tables: HallOfFame::new(), names: vec![String::new(); NAMES], last_name: String::new() }
    }
}

pub const FAME_FILE: &str = "fame.txt";

impl FameBook {
    /// The tables as the original makes them at boot, for `sx` supercross tracks, `nat` nationals
    /// and `arenas` freestyle arenas (each at most what the tables hold), naming the entries after
    /// `riders`.
    pub fn generate(sx: i32, nat: i32, arenas: i32, riders: &[String], dice: &mut Dice, clock: &mut dyn FnMut() -> u32) -> FameBook {
        let mut book = FameBook::default();
        book.set_word(SX_COUNT_AT, sx as u32);
        book.set_word(NAT_COUNT_AT, nat as u32);
        let leagues: [(i32, u32, &dyn Fn(usize, usize) -> u32); 2] =
            [(sx.min(SX_TRACKS), fame_table::SX_BEST_LAP, &|c, t| SX_BASES[c][t]), (nat.min(NAT_TRACKS), fame_table::NATIONALS_BEST_LAP, &|c, t| NAT_BASES[c][t])];
        for (count, best, base) in leagues {
            for track in 0..count {
                for class in 0..4 {
                    let mut put = |book: &mut FameBook, dice: &mut Dice, table: u32, k: usize, v: F| {
                        let at = HallOfFame::offset(table, track, class).unwrap() + 4 * k;
                        book.set_word(at, v.0.to_bits());
                        let name = dice.name(riders, clock);
                        book.names[name_index(table, track, class).unwrap() + k] = name;
                    };
                    let mut x = F(f32::from_bits(base(class as usize, track as usize)));
                    let mut first = x;
                    for k in 0..ENTRIES {
                        x = dice.step(x);
                        if k == 0 {
                            first = x;
                        }
                        put(&mut book, dice, best, k, x);
                    }
                    for (more, laps) in [(1, 3.0f32), (2, 5.0), (3, 10.0)] {
                        let mut x = F(fpu::mul(first.0, laps));
                        for k in 0..ENTRIES {
                            x = dice.step(x);
                            put(&mut book, dice, best + more, k, x);
                        }
                    }
                }
            }
        }
        for arena in 0..arenas.min(ARENAS) {
            let a = arena as usize;
            for (table, words) in [(fame_table::SCORE, ARENA_SCORES[a]), (fame_table::COUNT, ARENA_COUNTS[a]), (fame_table::TIME, ARENA_TIMES[a])] {
                let at = HallOfFame::offset(table, arena, 0).unwrap();
                let names = name_index(table, arena, 0).unwrap();
                for (k, w) in words.into_iter().enumerate() {
                    book.set_word(at + 4 * k, w);
                    book.names[names + k] = dice.name(riders, clock);
                }
            }
        }
        book
    }

    fn set_word(&mut self, at: usize, w: u32) {
        self.tables.words[at / 4] = w;
    }

    /// The five entries `table` keeps for `track` and `class`, with their names; none where the
    /// tables have none.
    pub fn entries(&self, table: u32, track: i32, class: i32) -> Vec<(u32, String)> {
        let (Some(at), Some(n)) = (HallOfFame::offset(table, track, class), name_index(table, track, class)) else { return Vec::new() };
        (0..ENTRIES).map(|k| (self.tables.words[at / 4 + k], self.names[n + k].clone())).collect()
    }

    /// `value` put in `table`'s entries for `track` and `class` under `name`, as the original's
    /// insertion does it: the first entry it beats (a time less, a score more, a count more as whole
    /// numbers) moves down a place with every one after it, the last falling off, and the value
    /// (a count cut to a whole number) and name take its place. Whether it beat one.
    pub fn insert(&mut self, value: F, table: u32, track: i32, class: i32, name: &str) -> bool {
        self.insert_at(value, table, track, class, name).is_some()
    }

    /// [`FameBook::insert`], with the place the value took.
    pub fn insert_at(&mut self, value: F, table: u32, track: i32, class: i32, name: &str) -> Option<usize> {
        let (Some(at), Some(n)) = (HallOfFame::offset(table, track, class), name_index(table, track, class)) else { return None };
        let w = at / 4;
        let v = fpu::operand(value.0.to_bits());
        let whole = fpu::to_i32(value.0);
        let beats = |kept: u32| match table {
            fame_table::SCORE => !(v <= fpu::operand(kept)),
            fame_table::COUNT => (kept as i32) < whole,
            _ => v < fpu::operand(kept),
        };
        let i = (0..ENTRIES).find(|&k| beats(self.tables.words[w + k]))?;
        for j in (i + 1..ENTRIES).rev() {
            self.tables.words[w + j] = self.tables.words[w + j - 1];
            self.names[n + j] = self.names[n + j - 1].clone();
        }
        self.tables.words[w + i] = if table == fame_table::COUNT { whole as u32 } else { value.0.to_bits() };
        self.names[n + i] = name.to_string();
        Some(i)
    }

    /// The records a seat's card earns, as the Hall of Fame menu's entry page flags them: the
    /// league's best lap table for the best lap, and the table of the race's length (3, 5 or 10
    /// laps) for the race time, each that [`HallOfFame::beats`] says it beats. A race in no
    /// league with race tables earns none.
    pub fn earned(&self, t: &FameTry) -> Vec<(u32, F)> {
        let base = match t.league {
            0 => fame_table::SX_BEST_LAP,
            1 => fame_table::NATIONALS_BEST_LAP,
            _ => return Vec::new(),
        };
        let mut out = Vec::new();
        if self.tables.beats(base, t.track, t.class, t.best_lap) {
            out.push((base, t.best_lap));
        }
        for (more, laps) in [(1, 3), (2, 5), (3, 10)] {
            if self.tables.beats(base + more, t.track, t.class, t.race_time) && t.laps == laps {
                out.push((base + more, t.race_time));
            }
        }
        out
    }

    /// The name typed for a seat's records: kept (cut to a slot) as the name last typed, and put
    /// in each table [`FameBook::earned`] flags, in that order. Each table, and the place the value
    /// took in it, if it beat an entry (a supercross time earns its flag on whole seconds but goes in
    /// on the time itself, so one can earn a flag and miss).
    pub fn enter(&mut self, t: &FameTry, name: &str) -> Vec<(u32, Option<usize>)> {
        let name = slot_name(name);
        self.last_name = name.clone();
        let earned = self.earned(t);
        earned.into_iter().map(|(table, value)| (table, self.insert_at(value, table, t.track, t.class, &name))).collect()
    }

    /// An entry's value as the screen shows it: a count whole, a score to a tenth, a time as the
    /// results show times.
    pub fn value_text(table: u32, word: u32) -> String {
        match table {
            fame_table::COUNT => (word as i32).to_string(),
            fame_table::SCORE => format!("{:.1}", f32::from_bits(word)),
            _ => time_text(F(f32::from_bits(word))),
        }
    }

    /// `w.K = …` sixteen words a line in hex, `n.I = name` for every name kept, and `last = name`.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for (k, row) in self.tables.words.chunks(16).enumerate() {
            let words: Vec<String> = row.iter().map(|w| format!("{w:08x}")).collect();
            out += &format!("w.{k} = {}\n", words.join(" "));
        }
        for (i, n) in self.names.iter().enumerate().filter(|(_, n)| !n.is_empty()) {
            out += &format!("n.{i} = {n}\n");
        }
        out += &format!("last = {}\n", self.last_name);
        out
    }

    /// What [`FameBook::text`] wrote; `None` unless every word is there.
    pub fn parse(text: &str) -> Option<FameBook> {
        let mut book = FameBook::default();
        let rows = book.tables.words.len().div_ceil(16);
        let mut seen = vec![false; rows];
        for line in text.lines() {
            let Some((key, value)) = line.split_once(" = ").or_else(|| line.split_once('=')) else { continue };
            let key = key.trim();
            if let Some(k) = key.strip_prefix("w.").and_then(|k| k.parse::<usize>().ok()).filter(|&k| k < rows) {
                let words: Vec<u32> = value.split_whitespace().filter_map(|w| u32::from_str_radix(w, 16).ok()).collect();
                let want = (book.tables.words.len() - 16 * k).min(16);
                if words.len() == want {
                    book.tables.words[16 * k..16 * k + want].copy_from_slice(&words);
                    seen[k] = true;
                }
            } else if let Some(i) = key.strip_prefix("n.").and_then(|i| i.parse::<usize>().ok()).filter(|&i| i < NAMES) {
                book.names[i] = slot_name(value);
            } else if key == "last" {
                book.last_name = slot_name(value);
            }
        }
        seen.iter().all(|&s| s).then_some(book)
    }

    /// `fame.txt` out of `dir`, if there is a whole one.
    pub fn load(dir: &Path) -> Option<FameBook> {
        FameBook::parse(&crate::files::read_to_string(dir.join(FAME_FILE)).ok()?)
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let path = dir.join(FAME_FILE);
        crate::files::write(&path, self.text()).map_err(|e| format!("{}: {e}", path.display()))
    }
}
