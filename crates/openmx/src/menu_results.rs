//! P330: the race's end on screen — what the original's results menu, preload and clutch tip popup,
//! Hall of Fame menu and rider profile make of what the race's two answers hand them
//! (`game_sim::race::Supercross::race_over` at each card's finish, `race_end` once at the Over).
//!
//! The results menu takes the score manager's cards in the manager's list order, copies each into a
//! record of thirteen words ([`ResultCard`]), and writes its eleven pages' text once, there and then:
//! the race's own page in list order, and every other page after sorting the records on that page's
//! word. The sort is the original's: an insertion sort over the one list, each page starting from
//! the order the page before left it in, skipped when the word is the one the list is already in.
//! [`Board::take`] is that, word for word; the screen draws what it wrote.
//!
//! The popup's words, the Hall of Fame menu's marks and the win counted in a rider's record are the
//! original's too ([`tips_of`], [`FameMarks::mark`], [`RiderRecord::add_wins`]). What a mark leads
//! to — the Hall of Fame's name entry and its tables' insertion — and the popup's own pictures are
//! not ported; the screen says what the original's would.

#![allow(dead_code)] // the shell hands the race's end over once the Sim runs it (S301); the tests do now

use game_sim::race::{EndTip, LapCard, RaceEnd, RaceOver};
use ps2_float::{F, fpu};
use std::path::Path;

/// How many cards the results menu's record list holds.
pub const MOST_CARDS: usize = 10;

/// What the results menu's columns are 25 bytes long for: the longest text a cell keeps.
pub const CELL: usize = 24;

/// A rider's words the results menu reads from the rider's statistics tracker, and two of the
/// card's own the lap line does not keep: the longest jump (`+0xf0`, in feet) and the longest hang
/// time (`+0x100`, in seconds).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CardStats {
    /// The tracker's `+0x24` to `+0x3c`, whole counts.
    pub laps_led: i32,
    pub times_in_lead: i32,
    pub passes: i32,
    pub times_passed: i32,
    pub wrecks: i32,
    pub preloads: i32,
    pub clutches: i32,
    pub jump: F,
    pub hang: F,
}

/// Whose a card is, as the results menu names it: a player's (the seat item's `+0x34` is 1) by its
/// profile's name, an AI rider's (2) by the AI's own; anything else leaves the cell as the page
/// was made, empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Who {
    Player(String),
    Ai(String),
    Other,
}

/// One card as the results menu copies it: who, and its thirteen words — the place, the race time,
/// the gap to the leader (none below 0), the best lap, then the tracker's laps led and times in
/// the lead, the jump and the hang time, and the tracker's passes, times passed, wrecks, preloads
/// and clutches. The counts are kept as floats, as the original keeps them.
#[derive(Clone, Debug, PartialEq)]
pub struct ResultCard {
    pub who: Who,
    pub words: [F; 13],
}

impl ResultCard {
    pub fn take(card: &LapCard, who: Who, stats: &CardStats) -> ResultCard {
        let n = |v: i32| F(fpu::from_i32(v));
        ResultCard {
            who,
            words: [
                n(card.place),
                card.race_time,
                F(fpu::max(0.0, card.gap.0)),
                card.best_lap,
                n(stats.laps_led),
                n(stats.times_in_lead),
                stats.jump,
                stats.hang,
                n(stats.passes),
                n(stats.times_passed),
                n(stats.wrecks),
                n(stats.preloads),
                n(stats.clutches),
            ],
        }
    }
}

/// One of the results menu's pages: its title, the record word its list is sorted on (an index
/// into [`ResultCard::words`]) and its column heads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Page {
    pub title: &'static str,
    pub key: usize,
    pub heads: &'static [&'static str],
}

/// What a page's column of values is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Column {
    Time,
    Distance,
    Count,
}

const fn page(title: &'static str, key: usize, head: &'static [&'static str]) -> Page {
    Page { title, key, heads: head }
}

/// The eleven pages in the original's order, Left and Right stepping round them.
pub const PAGES: [Page; 11] = [
    page("Race Results", 0, &["#", "Name", "Total Race Time", "Time Behind Leader"]),
    page("Fastest Lap", 3, &["#", "Name", "Time"]),
    page("Laps Led", 4, &["#", "Name", "Laps"]),
    page("Times In Lead", 5, &["#", "Name", "Total"]),
    page("Longest Jump Distance", 6, &["#", "Name", "Distance"]),
    page("Longest Hang Time", 7, &["#", "Name", "Time"]),
    page("Total Passes", 8, &["#", "Name", "Passes"]),
    page("Times Passed", 9, &["#", "Name", "Total"]),
    page("Total Wrecks", 10, &["#", "Name", "Wrecks"]),
    page("Total Preloads", 11, &["#", "Name", "Preloads"]),
    page("Total Clutches", 12, &["#", "Name", "Clutches"]),
];

fn column(key: usize) -> Column {
    match key {
        3 | 7 => Column::Time,
        6 => Column::Distance,
        _ => Column::Count,
    }
}

/// The best lap's stand-in while it is sorted: a card with none goes last.
const NO_LAP: F = F(18000.0);

/// `a < b` as `c.olt.s` reads the two words.
fn lt(a: F, b: F) -> bool {
    fpu::operand(a.0.to_bits()) < fpu::operand(b.0.to_bits())
}

/// `a == b` as `c.eq.s` reads them.
fn eq(a: F, b: F) -> bool {
    fpu::operand(a.0.to_bits()) == fpu::operand(b.0.to_bits())
}

/// The results menu's sort of its list (`order`, indexes into `cards`, already sorted on `*current`)
/// on word `key`: nothing when it is that word already. The place, the race time and the gap go
/// up, and the best lap too with a card that has none (0) taken as [`NO_LAP`] while it moves —
/// except the list's first card, which the sort never moves — and written back as 0 if it ends
/// [`NO_LAP`], which a real 18000 s lap does too. Every other word goes down. Each card moves back
/// past every card it is less than (more than, going down), so equal words keep their order.
pub fn sort(order: &mut [usize], cards: &mut [ResultCard], current: &mut usize, key: usize) {
    if *current == key {
        return;
    }
    *current = key;
    let up = key <= 3;
    for i in 1..order.len() {
        let v = order[i];
        if key == 3 && eq(cards[v].words[3], F(0.0)) {
            cards[v].words[3] = NO_LAP;
        }
        let mut j = i;
        while j > 0 {
            let (a, b) = (cards[v].words[key], cards[order[j - 1]].words[key]);
            if !(if up { lt(a, b) } else { lt(b, a) }) {
                break;
            }
            order[j] = order[j - 1];
            j -= 1;
        }
        if key == 3 && eq(cards[v].words[3], NO_LAP) {
            cards[v].words[3] = F(0.0);
        }
        order[j] = v;
    }
}

/// A time as the results menu writes it, `mm:ss.cc` rounded to the hundredth: dashes for 0, and
/// `99:99.99` (or `-99:99.99`) past 5999.99 s either way.
pub fn time_text(t: F) -> String {
    if eq(t, F(0.0)) {
        return "--:--.--".into();
    }
    let most = F(5999.99);
    if !(fpu::operand(t.0.to_bits()) <= fpu::operand(most.0.to_bits())) {
        return "99:99.99".into();
    }
    if lt(t, F(-5999.99)) {
        return "-99:99.99".into();
    }
    let cs = fpu::to_i32(fpu::add(fpu::mul(100.0, t.0), 0.5));
    let sign = if cs < 0 { "-" } else { "" };
    let cs = cs.unsigned_abs();
    let digit = |n: u32| char::from(b'0' + (n % 10) as u8);
    format!(
        "{sign}{}{}:{}{}.{}{}",
        digit(cs / 60000),
        digit(cs / 6000),
        digit(cs / 100 % 60 / 10),
        digit(cs / 100),
        digit(cs % 100 / 10),
        digit(cs)
    )
}

/// What a cell keeps of `s`: its first [`CELL`] bytes.
fn cell(s: &str) -> String {
    let mut end = s.len().min(CELL);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

/// The results menu's pages as it wrote them: per page, a row per card — the row's number, the name
/// and the page's values. The cards are the ones handed to it, records and all, after the pages'
/// sorts (a best lap of 18000 s is 0 now).
#[derive(Clone, Debug, PartialEq)]
pub struct Board {
    pub pages: Vec<Vec<Vec<String>>>,
    pub cards: Vec<ResultCard>,
}

impl Board {
    /// The results menu taking the cards (at most [`MOST_CARDS`], in the score manager's list
    /// order), its distances in metres when `metric` (the front end's units word; feet otherwise).
    pub fn take(cards: &[ResultCard], metric: bool) -> Board {
        let mut cards: Vec<ResultCard> = cards.iter().take(MOST_CARDS).cloned().collect();
        let mut order: Vec<usize> = (0..cards.len()).collect();
        let mut current = 0;
        let mut pages = Vec::with_capacity(PAGES.len());
        for (p, page) in PAGES.iter().enumerate() {
            sort(&mut order, &mut cards, &mut current, page.key);
            let rows = order
                .iter()
                .enumerate()
                .map(|(i, &k)| {
                    let number = (i + 1).to_string();
                    let c = &cards[k];
                    let name = match &c.who {
                        Who::Player(n) | Who::Ai(n) => n.clone(),
                        Who::Other => String::new(),
                    };
                    let mut row = vec![cell(&number), cell(&name)];
                    if p == 0 {
                        row.push(time_text(c.words[1]));
                        row.push(time_text(c.words[2]));
                    } else {
                        let v = c.words[page.key];
                        row.push(cell(&match column(page.key) {
                            Column::Time => time_text(v),
                            Column::Distance if metric => fpu::to_i32(fpu::mul(0.3048, v.0)).to_string(),
                            Column::Distance | Column::Count => fpu::to_i32(v.0).to_string(),
                        }));
                    }
                    row
                })
                .collect();
            pages.push(rows);
        }
        sort(&mut order, &mut cards, &mut current, PAGES[0].key);
        Board { pages, cards }
    }
}

/// A seat's line on the preload and clutch tip popup: the tips it is shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tip {
    pub seat: i32,
    pub preload: bool,
    pub clutch: bool,
}

/// The popup's words per seat as [`RaceEnd::tips`] writes them, in order: a later card on a seat
/// writes over an earlier one. The popup's two words per seat are two seats long, so seat 2's
/// preload word lands on seat 0's clutch word, as the original's does; a seat past that is
/// dropped. Seats with neither tip set are left out.
pub fn tips_of(tips: &[EndTip]) -> Vec<Tip> {
    // The popup's +0xc0 (preload, seats 0 and 1) and +0xc8 (clutch, seats 0 and 1), one run.
    let mut words = [false; 4];
    for t in tips {
        for (at, v) in [(t.seat, t.no_preload), (t.seat + 2, t.no_clutch)] {
            if let Some(w) = usize::try_from(at).ok().and_then(|a| words.get_mut(a)) {
                *w = v;
            }
        }
    }
    (0..2).map(|s| Tip { seat: s as i32, preload: words[s], clutch: words[s + 2] }).filter(|t| t.preload || t.clutch).collect()
}

/// The Hall of Fame menu's marks, as a new entry leaves them: seat 0's (`+0xb0`) and seat 1's
/// (`+0xb4`), the word seat 0's mark clears and seat 1's leaves (`+0xbc`, which seat's name the
/// menu asks first was not traced), the track (`+0xc4`) and that one is waiting (`+0xc0`), which the
/// results menu's way out reads to go to the Hall of Fame first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FameMarks {
    pub seat0: bool,
    pub seat1: bool,
    pub turn: bool,
    pub track: i32,
    pub waiting: bool,
}

impl FameMarks {
    /// A new entry for `seat` (any seat but 0 marks seat 1) on `track`.
    pub fn mark(&mut self, seat: i32, track: i32) {
        if seat == 0 {
            self.seat0 = true;
            self.turn = false;
        } else {
            self.seat1 = true;
        }
        self.track = track;
        self.waiting = true;
    }
}

/// A rider's record as the profile keeps it: races, wins and the win percentage.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RiderRecord {
    pub races: u16,
    pub wins: u16,
    pub win_rate: F,
}

impl RiderRecord {
    /// The profile's win percentage after either count moved: 100 when they are equal, else
    /// `100 · wins / races` (no races: the FPU's largest float).
    fn rate(&mut self) {
        self.win_rate = if self.wins == self.races {
            F(100.0)
        } else {
            F(fpu::div(fpu::mul(100.0, fpu::from_i32(i32::from(self.wins))), fpu::from_i32(i32::from(self.races))))
        };
    }

    /// The profile's wins stat counted up by `amount` (cut to a whole number, kept in 16 bits).
    pub fn add_wins(&mut self, amount: F) {
        self.wins = self.wins.wrapping_add(fpu::to_i32(amount.0) as u16);
        self.rate();
    }

    /// The profile's races stat, the same way.
    pub fn add_races(&mut self, amount: F) {
        self.races = self.races.wrapping_add(fpu::to_i32(amount.0) as u16);
        self.rate();
    }
}

/// The riders' records, one per seat, kept in `records.txt`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Records {
    pub seats: Vec<RiderRecord>,
}

pub const RECORDS_FILE: &str = "records.txt";

impl Records {
    /// Seat `seat`'s record, made if it has none yet.
    pub fn seat(&mut self, seat: usize) -> &mut RiderRecord {
        if self.seats.len() <= seat {
            self.seats.resize(seat + 1, RiderRecord::default());
        }
        &mut self.seats[seat]
    }

    /// `seat.N = races wins rate` a line, the rate as its float's bits so it reads back exactly.
    pub fn text(&self) -> String {
        self.seats
            .iter()
            .enumerate()
            .map(|(k, r)| format!("seat.{k} = {} {} {:08x}\n", r.races, r.wins, r.win_rate.0.to_bits()))
            .collect()
    }

    pub fn parse(text: &str) -> Records {
        let mut out = Records::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            let Some(k) = key.trim().strip_prefix("seat.").and_then(|k| k.parse::<usize>().ok()).filter(|&k| k < 16) else { continue };
            let w: Vec<&str> = value.split_whitespace().collect();
            if let [races, wins, rate] = w[..]
                && let (Ok(races), Ok(wins), Ok(rate)) = (races.parse(), wins.parse(), u32::from_str_radix(rate, 16))
            {
                *out.seat(k) = RiderRecord { races, wins, win_rate: F(f32::from_bits(rate)) };
            }
        }
        out
    }

    /// `records.txt` out of `dir`; none is no records.
    pub fn load(dir: &Path) -> Records {
        Records::parse(&crate::files::read_to_string(dir.join(RECORDS_FILE)).unwrap_or_default())
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let path = dir.join(RECORDS_FILE);
        crate::files::write(&path, self.text()).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// What the race hands the front end at its end: the score manager's cards as the results menu
/// copies them (in the manager's list order), `race_end`'s answer, and `race_over`'s for every card
/// that finished at the line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HandOver {
    pub cards: Vec<ResultCard>,
    pub end: RaceEnd,
    pub overs: Vec<RaceOver>,
}
