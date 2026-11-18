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

use game_sim::race::{EndTip, LapCard, RaceEnd, RaceOver, ResultEvent};
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

/// A rider's record as the profile keeps it: races, wins and the win percentage, and the race
/// statistics the results menu commits on its way out (P358): play time in whole seconds (stat 0),
/// feet ridden (1), preloads (2), clutch pulls (3), wrecks (4) and seconds raced (5); and the
/// points the score commit banks (P364), the profile's word the unlock shop spends.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RiderRecord {
    pub races: u16,
    pub wins: u16,
    pub win_rate: F,
    pub play_time: u16,
    pub feet: F,
    pub preloads: u16,
    pub clutches: u16,
    pub wrecks: u16,
    pub race_time: F,
    pub points: i32,
    /// P369: the unlock shop's items the rider owns, a bit an item, a word a category: the
    /// profile's unlock table, which keeps a word a slot of 50 a category.
    pub unlocked: [u64; SHOP_CATEGORIES],
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

    /// P358: one of the race statistics counted up by `amount`, as the profile's stat add does:
    /// the counts (stats 0, 2, 3, 4) by `amount` cut to a whole number and kept in 16 bits, feet
    /// ridden and seconds raced (1, 5) by `amount` itself. Any other stat is not one of these.
    pub fn add_stat(&mut self, stat: u32, amount: F) {
        let whole = fpu::to_i32(amount.0) as u16;
        match stat {
            0 => self.play_time = self.play_time.wrapping_add(whole),
            1 => self.feet = F(fpu::add(self.feet.0, amount.0)),
            2 => self.preloads = self.preloads.wrapping_add(whole),
            3 => self.clutches = self.clutches.wrapping_add(whole),
            4 => self.wrecks = self.wrecks.wrapping_add(whole),
            5 => self.race_time = F(fpu::add(self.race_time.0, amount.0)),
            _ => {}
        }
    }

    /// P364: a card's banked total added to the points, cut to a whole number (the word wraps).
    pub fn add_points(&mut self, banked: F) {
        self.points = self.points.wrapping_add(fpu::to_i32(banked.0));
    }

    /// P369: whether the rider owns the shop's item at `category`, `index`.
    pub fn owns(&self, category: usize, index: usize) -> bool {
        self.unlocked.get(category).is_some_and(|w| index < SHOP_ITEMS && w >> index & 1 != 0)
    }

    /// P369: the shop's purchase of `item` with these points, as the original's does it: an item
    /// already owned only buzzes (no dialog); else the buy check unlocks it when its price is no
    /// more than the points, and the profile's unlock marks it and takes the price off; a price
    /// over the points unlocks nothing and puts up "Not Enough Points".
    pub fn buy(&mut self, item: &ShopItem) -> Bought {
        if self.owns(item.category, item.index) {
            return Bought::Owned;
        }
        if self.points < item.price {
            return Bought::NotEnoughPoints;
        }
        self.unlock(item, true);
        Bought::Bought
    }

    /// P369: the profile's unlock of `item`: its bit set and, when `pay`, its price taken off the
    /// points (the word wraps). The career's free unlocks pass no pay; a category or slot past the
    /// table's is not kept.
    pub fn unlock(&mut self, item: &ShopItem, pay: bool) {
        if let Some(w) = self.unlocked.get_mut(item.category).filter(|_| item.index < SHOP_ITEMS) {
            *w |= 1 << item.index;
        }
        if pay {
            self.points = self.points.wrapping_sub(item.price);
        }
    }
}

/// P369: how many categories a record's unlock table keeps, and items a category (the profile's
/// table and a store category both hold 50).
pub const SHOP_CATEGORIES: usize = 16;
pub const SHOP_ITEMS: usize = 50;

/// P369: the store's kinds of category, by the `Type` its section names: a category whose type is
/// none of these is kind 5, which the original makes no items for.
pub const SHOP_KINDS: [&str; 5] = ["Track", "BikeCategory", "Bike", "Machine", "FreestyleActivity"];

/// P369: where the store's list lives: `appstart.res`'s `ui\store.dat`.
pub const SHOP_FILE: &str = "ui\\store.dat";

/// P369: what the shop's purchase answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bought {
    Bought,
    Owned,
    NotEnoughPoints,
}

/// P369: one of the store's items: its section's name, the track it names (a Track's `TrackName`,
/// a machine's or an activity's `Track`, else empty), its price (`Price`, 100 when the section has
/// none), and its category's place in the store and its own in the category.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShopItem {
    pub key: String,
    pub track: String,
    pub price: i32,
    pub category: usize,
    pub index: usize,
}

/// P369: one of the store's categories: its name, its kind ([`SHOP_KINDS`], 5 for none) and its
/// items in the file's order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShopCategory {
    pub name: String,
    pub kind: usize,
    pub items: Vec<ShopItem>,
}

/// P369: the unlock shop's list, as the original's store reader makes it from `store.dat`:
/// `[Store] NumberOfCategories` categories named `Category1`.., each a section whose `Type` gives
/// its kind and whose `NumberOfItems` items are named `Item1`.., each a section with its `Price`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shop {
    pub categories: Vec<ShopCategory>,
}

impl Shop {
    /// The store read out of `store.dat`'s text. A kind-5 category keeps no items (the original
    /// makes none for it), and a category keeps its first [`SHOP_ITEMS`].
    pub fn parse(text: &str) -> Shop {
        let ini = game_sim::phy::Ini::parse(text);
        let store = ini.section("Store");
        let count = store.int("NumberOfCategories").unwrap_or(0).max(0) as usize;
        let mut categories = Vec::new();
        for c in 0..count {
            let name = store.text(&format!("Category{}", c + 1)).unwrap_or_default().to_string();
            let sec = ini.section(&name);
            let ty = sec.text("Type").unwrap_or_default();
            let kind = SHOP_KINDS.iter().position(|k| k.eq_ignore_ascii_case(ty)).unwrap_or(SHOP_KINDS.len());
            let n = sec.int("NumberOfItems").unwrap_or(0).clamp(0, SHOP_ITEMS as i32) as usize;
            let items = if kind == SHOP_KINDS.len() {
                Vec::new()
            } else {
                (0..n)
                    .map(|i| {
                        let key = sec.text(&format!("Item{}", i + 1)).unwrap_or_default().to_string();
                        let it = ini.section(&key);
                        let track = it.text(if kind == 0 { "TrackName" } else { "Track" }).unwrap_or_default().to_ascii_lowercase();
                        ShopItem { key, track, price: it.int("Price").unwrap_or(100), category: c, index: i }
                    })
                    .collect()
            };
            categories.push(ShopCategory { name, kind, items });
        }
        Shop { categories }
    }
}

/// What the results menu's stats commit reads of a race (P358): the event's activity (only a
/// single race, 2, counts seconds raced), the profile slot of player 0 — which the commit counts
/// every player in — the score manager's clock, the seconds it has run, and each player seat's
/// tally, seat 0 first.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RaceTally {
    pub activity: i32,
    pub profile: Option<usize>,
    pub clock: F,
    pub seats: Vec<SeatTally>,
}

/// One player's words the stats commit reads from their statistics tracker: feet ridden (`+0x78`),
/// preloads (`+0x38`), clutch pulls (`+0x3c`) and wrecks (`+0x34`); and what the score commit
/// reads (P364): the player's own profile slot, if they have one, and their card's banked total.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SeatTally {
    pub feet: F,
    pub preloads: i32,
    pub clutches: i32,
    pub wrecks: i32,
    pub profile: Option<usize>,
    pub banked: F,
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

    /// The race the gate's drop counts (P335; the original counts races at the drop, not the
    /// finish): in play mode 0 on a single race (activity 2), every player seat with a profile —
    /// `profiles[k]` its record's slot — has one race added. Whether any record moved.
    pub fn count_race(&mut self, event: &ResultEvent, profiles: &[Option<usize>]) -> bool {
        if event.mode != 0 || event.activity != 2 {
            return false;
        }
        let mut moved = false;
        for slot in profiles.iter().flatten() {
            self.seat(*slot).add_races(F::ONE);
            moved = true;
        }
        moved
    }

    /// P364: the results menu's score commit, which its stats commit runs first: every player seat
    /// with a profile has its card's banked total added to the points of its own profile (not
    /// player 0's). Whether any record moved.
    pub fn commit_score(&mut self, tally: &RaceTally) -> bool {
        let mut moved = false;
        for s in &tally.seats {
            if let Some(slot) = s.profile {
                self.seat(slot).add_points(s.banked);
                moved = true;
            }
        }
        moved
    }

    /// P358: the results menu's stats commit, as Restart and Quit's yes run it on their way out of
    /// a race: the score commit ([`Records::commit_score`]), then for each player seat, seconds
    /// raced (a single race only), then play time, feet ridden, preloads, clutch pulls and wrecks,
    /// every seat's counted in player 0's profile slot. No profile for player 0: no stats.
    /// Whether any record moved.
    pub fn commit_race(&mut self, tally: &RaceTally) -> bool {
        let scored = self.commit_score(tally);
        let Some(slot) = tally.profile else { return scored };
        let n = |v: i32| F(fpu::from_i32(v));
        for s in &tally.seats {
            let r = self.seat(slot);
            if tally.activity == 2 {
                r.add_stat(5, tally.clock);
            }
            r.add_stat(0, tally.clock);
            r.add_stat(1, s.feet);
            r.add_stat(2, n(s.preloads));
            r.add_stat(3, n(s.clutches));
            r.add_stat(4, n(s.wrecks));
        }
        scored || !tally.seats.is_empty()
    }

    /// `seat.N = races wins rate play feet preloads clutches wrecks raced points` a line, the floats
    /// as their bits so they read back exactly. A line of the first three alone (before P358), or
    /// without the points (before P364), reads with the rest 0.
    pub fn text(&self) -> String {
        self.seats
            .iter()
            .enumerate()
            .map(|(k, r)| {
                format!(
                    "seat.{k} = {} {} {:08x} {} {:08x} {} {} {} {:08x} {}\n",
                    r.races,
                    r.wins,
                    r.win_rate.0.to_bits(),
                    r.play_time,
                    r.feet.0.to_bits(),
                    r.preloads,
                    r.clutches,
                    r.wrecks,
                    r.race_time.0.to_bits(),
                    r.points
                ) + &unlocked_line(k, &r.unlocked)
            })
            .collect()
    }

    pub fn parse(text: &str) -> Records {
        let mut out = Records::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            // P369: the items a record owns, `unlocked.N = category:index ..`.
            if let Some(k) = key.trim().strip_prefix("unlocked.").and_then(|k| k.parse::<usize>().ok()).filter(|&k| k < 16) {
                let r = out.seat(k);
                for pair in value.split_whitespace() {
                    if let Some((c, i)) = pair.split_once(':')
                        && let (Ok(c), Ok(i)) = (c.parse::<usize>(), i.parse::<usize>())
                        && c < SHOP_CATEGORIES
                        && i < SHOP_ITEMS
                    {
                        r.unlocked[c] |= 1 << i;
                    }
                }
                continue;
            }
            let Some(k) = key.trim().strip_prefix("seat.").and_then(|k| k.parse::<usize>().ok()).filter(|&k| k < 16) else { continue };
            let w: Vec<&str> = value.split_whitespace().collect();
            let bits = |s: &str| u32::from_str_radix(s, 16).ok().map(|b| F(f32::from_bits(b)));
            let Some(([races, wins, rate], rest)) = w.split_first_chunk::<3>() else { continue };
            let (Ok(races), Ok(wins), Some(win_rate)) = (races.parse(), wins.parse(), bits(rate)) else { continue };
            let unlocked = out.seats.get(k).map_or([0; SHOP_CATEGORIES], |r| r.unlocked);
            let mut r = RiderRecord { races, wins, win_rate, unlocked, ..RiderRecord::default() };
            if let [play, feet, preloads, clutches, wrecks, raced, ref points @ ..] = rest[..] {
                let (Ok(play_time), Some(feet), Ok(preloads), Ok(clutches), Ok(wrecks), Some(race_time)) =
                    (play.parse(), bits(feet), preloads.parse(), clutches.parse(), wrecks.parse(), bits(raced))
                else {
                    continue;
                };
                r = RiderRecord { play_time, feet, preloads, clutches, wrecks, race_time, ..r };
                match points {
                    [] => {}
                    [p] => match p.parse() {
                        Ok(points) => r.points = points,
                        Err(_) => continue,
                    },
                    _ => continue,
                }
            } else if !rest.is_empty() {
                continue;
            }
            *out.seat(k) = r;
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

/// P369: a record's `unlocked.N` line, none when it owns nothing.
fn unlocked_line(k: usize, unlocked: &[u64; SHOP_CATEGORIES]) -> String {
    let pairs: Vec<String> = (0..SHOP_CATEGORIES).flat_map(|c| (0..SHOP_ITEMS).filter(move |&i| unlocked[c] >> i & 1 != 0).map(move |i| format!("{c}:{i}"))).collect();
    if pairs.is_empty() { String::new() } else { format!("unlocked.{k} = {}\n", pairs.join(" ")) }
}

/// What the race hands the front end at its end: the score manager's cards as the results menu
/// copies them (in the manager's list order), `race_end`'s answer, and `race_over`'s for every card
/// that finished at the line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HandOver {
    pub cards: Vec<ResultCard>,
    pub end: RaceEnd,
    pub overs: Vec<RaceOver>,
    /// P358: what the stats commit on the way out of the results counts.
    pub tally: RaceTally,
}
