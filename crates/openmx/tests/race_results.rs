//! P330: the race's end on screen against the original's own code — the results menu taking the
//! score manager's cards (`race_results_take`, its card copy and its sort), the profile's race and
//! win counts (`profile_stat_add` 9 and 10) and the Hall of Fame menu's marks (`fame_menu_mark`).
//!
//! * `race_results_sort` over 20,000 lists (no card to ten, every key from every key, zeros, 18000
//!   and ties) against [`results::sort`]: the order, each record's words and the list's key.
//! * `race_results_take` over 6,000 card sets — players, AI riders and neither, long names, times
//!   at 0, past ±5999.99 and either side of it, odd floats, metric on and off — against
//!   [`results::Board::take`]: every cell of every page (the three blank rows too), each page's row
//!   count and the records the sorts leave. `snprintf`, `strcpy`, the allocator, the interface id
//!   and the AI's name are hooked; the menu's own code runs.
//! * `profile_stat_add` stats 9 and 10 over 20,000 counts and amounts against
//!   [`results::RiderRecord`], and `fame_menu_mark` against [`results::FameMarks`].
//!
//! The records file reads back as it was written, and the tips are the popup's words.

#[allow(unused)]
#[path = "../src/files.rs"]
mod files;
#[path = "../src/menu_results.rs"]
mod results;

use game_sim::race::{EndTip, LapCard};
use ps2_float::{F, fpu};
use ps2_oracle::{Ee, addr, game_or_skip, skip};
use results::{Board, CardStats, FameMarks, Records, ResultCard, RiderRecord, Who};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const BUDGET: u64 = 4_000_000;

const MENU: u32 = 0x0180_0000;
const PAGES: u32 = 0x0181_0000;
const PAGE_SIZE: u32 = 0x1000;
const HEAP: u32 = 0x0182_0000;
const MGR: u32 = 0x0182_f000;
const CARDS: u32 = 0x0183_0000;
const OWNERS: u32 = 0x0184_0000;
const ITEMS: u32 = 0x0185_0000;
const SEATS: u32 = 0x0186_0000;
const TRACKERS: u32 = 0x0187_0000;
const PROFILES: u32 = 0x0188_0000;
const AIS: u32 = 0x0189_0000;
const ITEM_VT: u32 = 0x018a_0000;
/// Not code: the item's tracker slot lands here and is hooked.
const HOOK_TRACKER: u32 = 0x018a_1000;
const APP: u32 = 0x018b_0000;
const FRONT: u32 = 0x018b_1000;
const UNITS: u32 = 0x018b_2000;
const LIST: u32 = 0x018c_0000;
const RECORDS: u32 = 0x018c_1000;
const STATS: u32 = 0x018d_0000;
const FAME: u32 = 0x018d_1000;

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
    /// A time as a card may hold one: mostly a race's, with the edges the text turns on.
    fn time(&mut self) -> F {
        let edge = |x: f32, d: i32| F(f32::from_bits((x.to_bits() as i32 + d) as u32));
        F(match self.below(14) {
            0 => 0.0,
            1 => -0.0,
            2 => edge(5999.99, self.below(7) as i32 - 3).0,
            3 => edge(-5999.99, self.below(7) as i32 - 3).0,
            4 => f32::from_bits(self.next() as u32),
            5 => 18000.0,
            6 => [30.0, 45.5, 61.25, 0.004_999, 0.005, -0.005][self.below(6) as usize],
            7 => -(self.below(100_000) as f32) / 100.0,
            8 => self.below(1 << 24) as f32 / 1000.0,
            _ => 20.0 + self.below(30_000) as f32 / 97.0,
        })
    }
    fn count(&mut self) -> i32 {
        match self.below(8) {
            0 => self.next() as i32,
            1 => -(self.below(5) as i32),
            _ => self.below(6) as i32,
        }
    }
    fn name(&mut self) -> String {
        let n = [0, 3, 8, 23, 24, 25, 40][self.below(7) as usize];
        (0..n).map(|_| char::from(b"abcdefghijklmnopqrstuvwxyz ABCDEFGHIJ0123456789"[self.below(47) as usize])).collect()
    }
}

fn put_f(ee: &mut Ee, at: u32, x: F) {
    ee.set_u32(at, x.0.to_bits());
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

/// `s` into `at` as C's `snprintf` leaves it in a buffer of `size`.
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

/// A record list as the results menu keeps one: the count, the records and the word it is sorted on.
fn put_list(ee: &mut Ee, cards: &[ResultCard], key: usize) {
    zero(ee, LIST, 0x34);
    ee.set_u32(LIST, cards.len() as u32);
    for (k, c) in cards.iter().enumerate() {
        let rec = RECORDS + 0x40 * k as u32;
        ee.set_u32(LIST + 4 + 4 * k as u32, rec);
        ee.set_u32(rec, 0x100 + k as u32);
        for (w, x) in c.words.iter().enumerate() {
            put_f(ee, rec + 4 + 4 * w as u32, *x);
        }
    }
    ee.set_u32(LIST + 0x2c, key as u32);
}

#[test]
fn the_sort_matches_the_results_menus_sort() {
    let Some(mut ee) = game_or_skip() else { return };
    let sort = match addr("race_results_sort") {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let mut p = Pick(0x7d1c_3a95_e04b_2f68);
    let (mut nolap, mut moved) = (0, 0);
    for case in 0..20_000 {
        let n = p.below(11) as usize;
        let tied = p.below(3) == 0;
        let mut cards: Vec<ResultCard> = (0..n)
            .map(|_| {
                let mut words = [F(0.0); 13];
                for w in &mut words {
                    *w = if tied { F([0.0, 18000.0, 2.0][p.below(3) as usize]) } else { p.time() };
                }
                ResultCard { who: Who::Other, words }
            })
            .collect();
        let mut current = p.below(13) as usize;
        let key = if p.below(4) == 0 { current } else { p.below(13) as usize };
        put_list(&mut ee, &cards, current);
        ee.call(sort, &[u64::from(LIST), key as u64], &[], BUDGET);
        let mut order: Vec<usize> = (0..n).collect();
        let before = cards.clone();
        results::sort(&mut order, &mut cards, &mut current, key);
        let got: Vec<usize> = (0..n).map(|k| ((ee.u32_at(ee.u32_at(LIST + 4 + 4 * k as u32)) - 0x100) as usize)).collect();
        assert_eq!(got, order, "case {case}: key {key} from {}, cards {before:?}", ee.u32_at(LIST + 0x2c));
        assert_eq!(ee.u32_at(LIST + 0x2c) as usize, current, "case {case}");
        for (k, c) in cards.iter().enumerate() {
            for (w, x) in c.words.iter().enumerate() {
                let at = RECORDS + 0x40 * k as u32 + 4 + 4 * w as u32;
                assert_eq!(ee.u32_at(at), x.0.to_bits(), "case {case}: card {k} word {w}, key {key}, cards {before:?}");
            }
        }
        nolap += usize::from(key == 3 && before.iter().skip(1).any(|c| c.words[3].0 == 0.0));
        moved += usize::from(order.iter().enumerate().any(|(k, &o)| k != o));
    }
    assert!(nolap > 300 && moved > 5_000, "{nolap} lists with a card without a lap, {moved} moved");
}

/// What one card set puts in RAM, and what the port takes from the same numbers.
struct Set {
    cards: Vec<ResultCard>,
    metric: bool,
}

fn put_set(ee: &mut Ee, p: &mut Pick, ai_names: &RefCell<Vec<String>>) -> Set {
    let n = 1 + p.below(10) as usize;
    let metric = p.below(2) == 0;
    ee.set_u32(UNITS + 0x18, u32::from(metric) * (1 + p.below(3) as u32));
    let same = p.below(4) == 0;
    let mut cards = Vec::new();
    let mut names = Vec::new();
    ee.set_u32(MGR + 0x34, CARDS);
    for k in 0..n as u32 {
        let (card, owner, item, seat, tracker) =
            (CARDS + k * 0x200, OWNERS + k * 0x100, ITEMS + k * 0x100, SEATS + k * 0x800, TRACKERS + k * 0x100);
        let (profile, ai) = (PROFILES + k * 0x100, AIS + k * 0x100);
        ee.set_u32(card + 4, owner);
        ee.set_u32(card + 0x1c8, if k + 1 < n as u32 { card + 0x200 } else { 0 });
        ee.set_u32(owner + 0x24, item);
        ee.set_u32(item + 0x20, seat);
        ee.set_u32(item + 0x48, ITEM_VT);
        ee.set_u32(seat + 0x6b4, profile);
        ee.set_u32(seat + 0x6b8, ai);

        let lap = LapCard {
            place: if same { 1 } else { p.count() },
            race_time: if same { F(61.25) } else { p.time() },
            gap: p.time(),
            best_lap: if same && p.below(2) == 0 { F(0.0) } else { p.time() },
            ..LapCard::default()
        };
        ee.set_u32(card + 0x2c, lap.place as u32);
        put_f(ee, card + 0xb4, lap.race_time);
        put_f(ee, card + 0xbc, lap.gap);
        put_f(ee, card + 0x50, lap.best_lap);
        let stats = CardStats {
            laps_led: p.count(),
            times_in_lead: p.count(),
            passes: p.count(),
            times_passed: p.count(),
            wrecks: p.count(),
            preloads: p.count(),
            clutches: p.count(),
            jump: if p.below(3) == 0 { F(p.below(400) as f32 * 0.75) } else { p.time() },
            hang: p.time(),
        };
        put_f(ee, card + 0xf0, stats.jump);
        put_f(ee, card + 0x100, stats.hang);
        for (at, v) in [
            stats.laps_led,
            stats.times_in_lead,
            stats.passes,
            stats.times_passed,
            stats.wrecks,
            stats.preloads,
            stats.clutches,
        ]
        .into_iter()
        .enumerate()
        {
            ee.set_u32(tracker + 0x24 + 4 * at as u32, v as u32);
        }

        let kind = [1, 2, 2, 2, 0, 3][p.below(6) as usize];
        ee.set_u32(seat + 0x34, kind);
        let name = p.name();
        put_c(ee, profile + 0x38, name.as_bytes(), 0x100);
        names.push(name.clone());
        let who = match kind {
            1 => Who::Player(name),
            2 => Who::Ai(name),
            _ => Who::Other,
        };
        cards.push(ResultCard::take(&lap, who, &stats));
    }
    *ai_names.borrow_mut() = names;
    Set { cards, metric }
}

#[test]
fn the_board_matches_the_results_menus_pages() {
    let Some(mut ee) = game_or_skip() else { return };
    let [take, new, snprintf, strcpy, id, ai_name, app] = match addrs([
        "race_results_take",
        "operator_new",
        "snprintf",
        "strcpy",
        "interface_id",
        "ai_rider_name",
        "app_pointer",
    ]) {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let heap = Rc::new(Cell::new(HEAP));
    let made: Rc<RefCell<Vec<u32>>> = Rc::default();
    let ai_names: Rc<RefCell<Vec<String>>> = Rc::default();
    let (h, m) = (heap.clone(), made.clone());
    ee.hooks.insert(new, Box::new(move |ee| {
        let at = h.get();
        let size = (ee.r(4) as u32 + 15) & !15;
        zero(ee, at, size);
        h.set(at + size);
        m.borrow_mut().push(at);
        ee.set(2, u64::from(at));
    }));
    ee.hooks.insert(snprintf, Box::new(|ee| {
        let (dst, size, fmt, arg) = (ee.r(4) as u32, ee.r(5) as u32, ee.r(6) as u32, ee.r(7));
        let fmt = c_str(ee, fmt);
        let mut out = Vec::new();
        let mut k = 0;
        while k < fmt.len() {
            match (fmt[k], fmt.get(k + 1)) {
                (b'%', Some(b'd')) => out.extend((arg as u32 as i32).to_string().bytes()),
                (b'%', Some(b's')) => out.extend(c_str(ee, arg as u32)),
                (b'%', c) => panic!("snprintf %{c:?}"),
                (b, _) => {
                    out.push(b);
                    k += 1;
                    continue;
                }
            }
            k += 2;
        }
        put_c(ee, dst, &out, size);
        ee.set(2, out.len() as u64);
    }));
    ee.hooks.insert(strcpy, Box::new(|ee| {
        let (dst, src) = (ee.r(4) as u32, ee.r(5) as u32);
        let s = c_str(ee, src);
        put_c(ee, dst, &s, s.len() as u32 + 1);
        ee.set(2, u64::from(dst));
    }));
    ee.hooks.insert(id, Box::new(|_| {}));
    ee.hooks.insert(HOOK_TRACKER, Box::new(|ee| {
        let k = (ee.r(4) as u32 - ITEMS) / 0x100;
        ee.set(2, u64::from(TRACKERS + k * 0x100));
    }));
    let names = ai_names.clone();
    ee.hooks.insert(ai_name, Box::new(move |ee| {
        let (entry, buf, size) = (ee.r(4) as u32, ee.r(5) as u32, ee.r(6) as u32);
        let k = ((entry - AIS) / 0x100) as usize;
        let name = names.borrow()[k].clone();
        put_c(ee, buf, format!("ai {name}").as_bytes(), size);
    }));
    ee.set_u32(ITEM_VT + 0x40, HOOK_TRACKER);
    ee.set_u32(app, APP);
    ee.set_u32(APP + 0x3d4, FRONT);
    ee.set_u32(FRONT + 8, UNITS);

    let mut p = Pick(0x51e8_09ad_c3f4_7b26);
    let (mut long, mut others, mut metric, mut odd) = (0, 0, 0, 0);
    for case in 0..6_000 {
        heap.set(HEAP);
        made.borrow_mut().clear();
        ee.set_u32(MENU + 0xa8, 0);
        for (k, page) in results::PAGES.iter().enumerate() {
            let at = PAGES + PAGE_SIZE * k as u32;
            zero(&mut ee, at, 0x700);
            ee.set_u32(at + 4, page.key as u32);
            ee.set_u32(MENU + 0xac + 4 * k as u32, at);
        }
        let mut set = put_set(&mut ee, &mut p, &ai_names);
        for c in &mut set.cards {
            if let Who::Ai(n) = &mut c.who {
                *n = format!("ai {n}");
            }
        }
        ee.call(take, &[u64::from(MENU), u64::from(MGR)], &[], BUDGET);
        let board = Board::take(&set.cards, set.metric);
        let n = set.cards.len();
        for (k, page) in board.pages.iter().enumerate() {
            let at = PAGES + PAGE_SIZE * k as u32;
            assert_eq!(ee.u32_at(at + 0x20) as usize, n + 3, "case {case} page {k}");
            let columns = if k == 0 { 4 } else { 3 };
            for row in 0..n + 3 {
                let got: Vec<String> = [0x24, 0x1b4, 0x344, 0x4d4][..columns]
                    .iter()
                    .map(|c| String::from_utf8_lossy(&c_str(&ee, at + c + 25 * row as u32)).into_owned())
                    .collect();
                let want = page.get(row).cloned().unwrap_or_else(|| vec![String::new(); columns]);
                assert_eq!(got, want, "case {case} page {k} row {row}: cards {:?}, metric {}", set.cards, set.metric);
            }
        }
        assert_eq!(ee.u32_at(MENU + 0x130), 0, "case {case}");
        // The records, one per card in the manager's order after the list's own, as the sorts left them.
        let made = made.borrow();
        assert_eq!(made.len(), n + 1, "case {case}");
        assert_eq!(ee.u32_at(MENU + 0xa8), made[0], "case {case}");
        assert_eq!(ee.u32_at(made[0]) as usize, n, "case {case}");
        for (k, c) in board.cards.iter().enumerate() {
            let rec = made[k + 1];
            assert_eq!(ee.u32_at(rec), SEATS + 0x800 * k as u32, "case {case} card {k}");
            for (w, x) in c.words.iter().enumerate() {
                assert_eq!(ee.u32_at(rec + 4 + 4 * w as u32), x.0.to_bits(), "case {case} card {k} word {w}: {:?}", set.cards[k]);
            }
        }
        long += usize::from(board.pages[0].iter().any(|r| r[1].len() == results::CELL));
        others += usize::from(set.cards.iter().any(|c| c.who == Who::Other));
        metric += usize::from(set.metric);
        odd += usize::from(board.pages[0].iter().any(|r| r[2].starts_with('9') || r[2].starts_with("-9")));
    }
    assert!(long > 1_000 && others > 1_000 && metric > 2_000 && odd > 300, "{long} long names, {others} with neither, {metric} metric, {odd} past the limit");
}

#[test]
fn the_race_and_win_counts_match_the_profiles_stat_add() {
    let Some(mut ee) = game_or_skip() else { return };
    let add = match addr("profile_stat_add") {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let mut p = Pick(0x0c4e_92b7_15da_6f83);
    let mut divided = 0;
    for case in 0..20_000 {
        let slot = p.below(6) as u32;
        let small = p.below(2) == 0;
        let mut rec = RiderRecord {
            races: if small { p.below(5) as u16 } else { p.next() as u16 },
            wins: if small { p.below(5) as u16 } else { p.next() as u16 },
            win_rate: F(f32::from_bits(p.next() as u32)),
        };
        let stat = if p.below(2) == 0 { 9 } else { 10 };
        let amount = F(match p.below(5) {
            0 => f32::from_bits(p.next() as u32),
            1 => p.below(200_000) as f32 - 100_000.0,
            2 => p.below(400) as f32 / 7.0,
            _ => 1.0,
        });
        zero(&mut ee, STATS, 0x200);
        ee.write(STATS + 0x168 + 2 * slot, 2, u128::from(rec.races));
        ee.write(STATS + 0x174 + 2 * slot, 2, u128::from(rec.wins));
        put_f(&mut ee, STATS + 0x30 + 4 * slot, rec.win_rate);
        ee.call(add, &[u64::from(STATS), stat, u64::from(slot)], &[amount.0], BUDGET);
        let before = rec;
        if stat == 9 { rec.add_races(amount) } else { rec.add_wins(amount) }
        let got = (ee.u16_at(STATS + 0x168 + 2 * slot), ee.u16_at(STATS + 0x174 + 2 * slot), ee.u32_at(STATS + 0x30 + 4 * slot));
        assert_eq!(got, (rec.races, rec.wins, rec.win_rate.0.to_bits()), "case {case}: stat {stat} by {amount:?} on {before:?}");
        divided += usize::from(rec.races != rec.wins);
    }
    assert!(divided > 10_000, "{divided}");
}

#[test]
fn the_fame_marks_match_the_fame_menus_mark() {
    let Some(mut ee) = game_or_skip() else { return };
    let mark = match addr("fame_menu_mark") {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let mut p = Pick(0x9a3f_61c0_e72d_5b18);
    for case in 0..5_000 {
        let mut marks = FameMarks {
            seat0: p.below(2) == 0,
            seat1: p.below(2) == 0,
            turn: p.below(2) == 0,
            track: p.below(30) as i32,
            waiting: p.below(2) == 0,
        };
        let before = marks;
        for (at, v) in [(0xb0, marks.seat0), (0xb4, marks.seat1), (0xbc, marks.turn), (0xc0, marks.waiting)] {
            ee.set_u32(FAME + at, u32::from(v));
        }
        ee.set_u32(FAME + 0xc4, marks.track as u32);
        let seat = p.below(4) as i32 - 1;
        let track = p.below(40) as i32 - 2;
        ee.call(mark, &[u64::from(FAME), seat as i64 as u64, track as i64 as u64], &[], BUDGET);
        marks.mark(seat, track);
        let got = [0xb0, 0xb4, 0xbc, 0xc0].map(|at| ee.u32_at(FAME + at));
        let want = [marks.seat0, marks.seat1, marks.turn, marks.waiting].map(u32::from);
        assert_eq!(got, want, "case {case}: seat {seat} on {before:?}");
        assert_eq!(ee.i32_at(FAME + 0xc4), marks.track, "case {case}");
    }
}

/// The popup's words: seat 2's preload word is seat 0's clutch word, a later card writes over an
/// earlier one, and a seat with neither is not shown.
#[test]
fn the_tips_are_the_popups_words() {
    let tip = |seat, no_preload, no_clutch| EndTip { seat, no_preload, no_clutch };
    assert_eq!(results::tips_of(&[]), vec![]);
    assert_eq!(results::tips_of(&[tip(0, false, false), tip(1, false, false)]), vec![]);
    assert_eq!(results::tips_of(&[tip(1, true, false)]), vec![results::Tip { seat: 1, preload: true, clutch: false }]);
    assert_eq!(
        results::tips_of(&[tip(0, true, false), tip(2, true, false)]),
        vec![results::Tip { seat: 0, preload: true, clutch: true }]
    );
    assert_eq!(results::tips_of(&[tip(0, true, true), tip(0, false, true)]), vec![results::Tip { seat: 0, preload: false, clutch: true }]);
    assert_eq!(results::tips_of(&[tip(5, true, true), tip(-1, true, true)]), vec![results::Tip { seat: 1, preload: true, clutch: false }]);
}

/// The records file reads back exactly, the win rate bit for bit, and a line it cannot read is
/// skipped.
#[test]
fn the_records_read_back_as_they_were_written() {
    let mut r = Records::default();
    r.seat(0).add_races(F(3.0));
    r.seat(0).add_wins(F(1.0));
    r.seat(2).add_wins(F(2.0));
    assert_eq!(r.seats.len(), 3);
    assert_eq!(r.seats[0].win_rate.0.to_bits(), fpu::div(100.0, 3.0).to_bits());
    let text = r.text();
    assert_eq!(Records::parse(&text), r);
    assert_eq!(Records::parse(&format!("junk\nseat.x = 1 2 3\nseat.1 = 1\n{text}")), r);
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("p330-records");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(Records::load(&dir), Records::default());
    r.save(&dir).unwrap();
    assert_eq!(Records::load(&dir), r);
}
