// ---------------------------------------------------------------------------------------------------
// S377: the Sim's race AI riders against C31's live sx01 race, tick after tick
// ---------------------------------------------------------------------------------------------------
//
// C31 recorded sx01's single race (four riders: the human idling at the gate, three AI riders round
// three laps) for 12,000 ticks from its start state: every rider's score card each tick, raw, and the
// score manager. C31 holds no bike words, so the riders' bodies cannot be ridden here; their places on
// the track are the capture's, fed to the cards as the bikes tell them. What runs is the Sim's own
// `race::Laps` over the whole race — every card's step in list order, the ranking, the clocks, the
// finish — twice: once seeded at the race's first tick and run free (order, places, laps, clocks and
// flags hold on every row), once started again from the capture wherever it lost a tick (every card
// word holds, bit for bit). Each held tick also checks what the Sim's AI lane hands the brains off
// those laps (S332): `ai::Racing::of` / `ai::racing_of` (the race, the card the race driver reads) and
// `ai::Pilot::read_laps` (the pilot's card flags and progress, live, the race phase). And
// `Sim::load_ai_net`'s net off the disc against the image's.

use super::path_branch::s309_paths;
use super::*;
use game_sim::Sim;
use game_sim::ai::{AiRider, Lane, Racing, racing_of};
use game_sim::race::{
    self, CardFeed, Course, Entry, Jumps, LapCard, LapRules, Laps, LineLists, RaceCard, RaceRules, Seen, Spot,
    Tracker, WatchRules, Warnings, card_flags, race_card_flags,
};
use ps2_oracle::{Ee, Kit};

/// C31's start state as a RAM image: the race loaded, before the countdown and before any card.
const C31_STATE: &str = "c31-sx01-ai4.ee.bin";
/// The four AI-race capture by rider id (card id = player: 0 the human, 1–3 the AI riders).
const C31_CSV: [&str; 3] = ["capture/c31/", "sx01-ai4-by-", "rider.csv"];

/// The card flags the card's owner writes outside the race handler: the listeners' bits (S13c). The
/// jump watch, which the owner drops while its rider is down and sets again at the respawn (C31
/// rider 1, frames 5915–6021), is the Sim's own (S429, [`RaceCard::riding_bit`]).
const OWNER_FLAGS: u64 = 0xe0;

/// sx01's race track as C31's image holds it, and the RAM addresses that name its paths and nodes.
struct Track {
    paths: race::TrackPaths,
    course: Course,
    recs: Vec<u32>,
    nodes: Vec<Vec<u32>>,
    /// The race track's ahead reach as the image holds it (`+0x38`).
    ahead: F,
}

impl Track {
    fn spot(&self, path: u32, node: u32, along: F) -> Option<Spot> {
        if path == 0 {
            return Some(Spot { path: 0, piece: 0, along });
        }
        let p = self.recs.iter().position(|&r| r == path)?;
        let n = if node == 0 { 0 } else { self.nodes[p].iter().position(|&a| a == node)? as u32 + 1 };
        Some(Spot { path: p as u32 + 1, piece: n, along })
    }
}

fn quad_at(ee: &Ee, at: u32) -> Vf {
    Vf(core::array::from_fn(|k| f32::from_bits(ee.u32_at(at + 4 * k as u32))))
}

/// One captured row: its frame, dt and generator, the manager's raw words and each card's.
struct Row {
    frame: u32,
    dt: F,
    rng: u64,
    mgr: Vec<u8>,
    raw: Vec<Vec<u8>>,
}

fn word(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn fl(b: &[u8], o: usize) -> F {
    F(f32::from_bits(word(b, o)))
}
fn vq(b: &[u8], o: usize) -> Vf {
    Vf::new(fl(b, o).0, fl(b, o + 4).0, fl(b, o + 8).0, fl(b, o + 12).0)
}

/// A score card off its raw bytes, as the race handler keeps it (S13c's offsets). None for a spot that
/// names a node outside the track.
fn card_of(raw: &[u8], t: &Track) -> Option<RaceCard> {
    let u = |o: usize| word(raw, o);
    let f = |o: usize| fl(raw, o);
    let spot = |o: usize| t.spot(u(o), u(o + 4), f(o + 8));
    let jumps = Jumps {
        count: u(0xe8) as i32,
        length: f(0xec),
        best_length: f(0xf0),
        height: f(0xf4),
        best_height: f(0xf8),
        air_time: f(0xfc),
        best_air_time: f(0x100),
        rise: f(0x104),
        takeoff: vq(raw, 0x190),
        landing: vq(raw, 0x1a0),
        grounded: f(0x1b0),
    };
    let warnings = Warnings { off: f(0x150), off_blink: f(0x154), off_nag: f(0x15c), wrong: f(0x160), wrong_blink: f(0x164) };
    let lap = LapCard {
        flags: u64::from(u(8)) | u64::from(u(12)) << 32,
        to_line_kept: f(0x10),
        to_line: f(0x14),
        progress: f(0x1c),
        laps_length: f(0x24),
        lap_length: f(0x28),
        place: u(0x2c) as i32,
        laps: u(0x30) as i32,
        runs: u(0x34) as i32,
        on_track: u(0x40) as i32,
        lap_time: f(0x48),
        lap_start: f(0x4c),
        best_lap: f(0x50),
        best_lap_no: u(0x54) as i32,
        seen: Seen { now: vq(raw, 0x60), last: vq(raw, 0x70) },
        spot: spot(0x88)?,
        kept_spot: spot(0x94)?,
        race_time: f(0xb4),
        lap_clock: f(0xb8),
        gap: f(0xbc),
        off_track: f(0xc0),
        time_left: f(0xc4),
        attack_clock: f(0x168),
        record_word: u(0x108),
        ..LapCard::default()
    };
    Some(RaceCard {
        lap,
        on_path: f(0x18),
        odometer: f(0x20),
        aim: spot(0xa0)?,
        look: vq(raw, 0x120),
        ahead: vq(raw, 0x140),
        to_track: vq(raw, 0x180),
        jumps,
        warnings,
    })
}

/// The score manager's words off its raw bytes (S13c's offsets).
fn rules_of(raw: &[u8]) -> RaceRules {
    let u = |o: usize| word(raw, o);
    let f = |o: usize| fl(raw, o);
    RaceRules {
        lap: LapRules {
            flags: u(0x5c),
            kind: u(0x60) as i32,
            laps: u(0x94) as i32,
            ghost_runs: u(0xf0) as i32,
            record_limit: u(0x4c) as i32,
            // Not captured; a race of 4 riders and 3 laps never empties it.
            free_records: 64,
            finish_wait: f(0x9c),
            finish_clock: f(0x98),
            time_limit: f(0xa0),
        },
        below_limit: f(0xb8),
        time_limit: f(0xa0),
        off_limit: f(0xd4),
        aim_reach: f(0xd8),
        watch: WatchRules {
            jump_settle: f(0xf4),
            off_after: f(0xcc),
            off_blink: f(0xd0),
            off_nag: f(0xdc),
            wrong_after: f(0xe0),
            wrong_blink: f(0xe4),
            aim_ahead: f(0x104),
        },
    }
}

/// The card ids (players) in the manager's list order: the head at its `+0x34`, each card's next at
/// its `+0x1c8`; the cards sit side by side in id order, so a card's address ranks it.
fn order_of(mgr: &[u8], raws: &[Vec<u8>]) -> Vec<usize> {
    let mut by_id: Vec<u32> = std::iter::once(word(mgr, 0x34)).chain(raws.iter().map(|r| word(r, 0x1c8))).filter(|&a| a != 0).collect();
    by_id.sort_unstable();
    by_id.dedup();
    let mut list = Vec::new();
    let mut at = word(mgr, 0x34);
    while let Ok(k) = by_id.binary_search(&at) {
        if k >= raws.len() || list.len() == raws.len() {
            break;
        }
        list.push(k);
        at = word(&raws[k], 0x1c8);
    }
    list
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|k| u8::from_str_radix(&s[2 * k..2 * k + 2], 16).expect("hex")).collect()
}

fn rows_of(text: &str) -> Vec<Row> {
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap_or("").split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name).unwrap_or_else(|| panic!("C31: no column {name}"));
    let (c_frame, c_dt, c_rng, c_mgr) = (col("frame"), col("dt"), col("rng_state"), col("mgr_raw"));
    let c_raw: Vec<usize> = (0..4).map(|k| col(&format!("rider{k}_0.raw"))).collect();
    lines
        .map(|l| {
            let v: Vec<&str> = l.split(',').collect();
            Row {
                frame: v[c_frame].parse().expect("frame"),
                dt: F(v[c_dt].parse::<f32>().expect("dt")),
                rng: v[c_rng].parse().expect("rng_state"),
                mgr: hex(v[c_mgr]),
                raw: c_raw.iter().map(|&c| hex(v[c])).collect(),
            }
        })
        .collect()
}

/// The words one card is held on, named, as raw bits.
fn words(c: &RaceCard) -> Vec<(&'static str, u64)> {
    let l = &c.lap;
    let b = |x: F| u64::from(x.0.to_bits());
    let q = |x: Vf| x.to_bits().iter().take(3).fold(0u64, |h, w| h.rotate_left(21) ^ u64::from(*w));
    let s = |x: Spot| u64::from(x.path) << 48 | u64::from(x.piece) << 32 | b(x.along);
    let j = &c.jumps;
    vec![
        ("flags", l.flags & !OWNER_FLAGS),
        ("place", l.place as u64),
        ("progress", b(l.progress)),
        ("laps", l.laps as u64),
        ("laps_length", b(l.laps_length)),
        ("lap_length", b(l.lap_length)),
        ("to_line_kept", b(l.to_line_kept)),
        ("to_line", b(l.to_line)),
        ("on_path", b(c.on_path)),
        ("odometer", b(c.odometer)),
        ("on_track", l.on_track as u64),
        ("lap_time", b(l.lap_time)),
        ("lap_start", b(l.lap_start)),
        ("best_lap", b(l.best_lap)),
        ("best_lap_no", l.best_lap_no as u64),
        ("now", q(l.seen.now)),
        ("last", q(l.seen.last)),
        ("spot", s(l.spot)),
        ("kept_spot", s(l.kept_spot)),
        ("aim", s(c.aim)),
        ("race_time", b(l.race_time)),
        ("lap_clock", b(l.lap_clock)),
        ("gap", b(l.gap)),
        ("off_track", b(l.off_track)),
        ("time_left", b(l.time_left)),
        ("ahead", q(c.ahead)),
        ("to_track", q(c.to_track)),
        ("jumps", [j.count as u64, b(j.length), b(j.best_length), b(j.height), b(j.best_height), b(j.air_time), b(j.best_air_time), b(j.rise), b(j.grounded)].iter().fold(0, |h, w| h.rotate_left(13) ^ w)),
        ("warnings", [c.warnings.off, c.warnings.off_blink, c.warnings.off_nag, c.warnings.wrong, c.warnings.wrong_blink].iter().fold(0u64, |h, w| h.rotate_left(13) ^ b(*w))),
    ]
}

/// What the run went through and where it parted from the capture.
#[derive(Debug, Default)]
struct Run {
    ticks: usize,
    held: usize,
    /// Ticks some card's place for is missing from the capture.
    guessed: usize,
    laps: usize,
    /// Held rows × cards out of the race (finished).
    finished: usize,
    place_changes: usize,
    order_changes: usize,
    hand_offs: usize,
    /// Pilot reads of the laps checked.
    reads: usize,
    /// Pilot reads whose habits are in the air (air time running, ground clock 0), and on the ground
    /// after a jump (both set) (B38).
    airborne: usize,
    landed: usize,
    /// The last frame held.
    end: u32,
    /// Per word: (count, first few).
    bad: std::collections::BTreeMap<&'static str, (usize, String)>,
}

impl Run {
    fn differ(&mut self, name: &'static str, what: String) {
        let e = self.bad.entry(name).or_insert((0, String::new()));
        if e.0 < 4 {
            e.1 += &format!(" | {what}");
        }
        e.0 += 1;
    }
}

/// The race track off C31's image.
fn track_of(ee: &Ee, activities: u32) -> Track {
    let act = ee.u32_at(ee.u32_at(ee.u32_at(activities) + 0x58));
    let at = ee.u32_at(act + 0xb8);
    let (paths, recs, nodes) = s309_paths(ee, at);
    let line_at = ee.u32_at(at + 0x5c);
    let line = race::Gate {
        flags: ee.u32_at(line_at + 0x30),
        centre: quad_at(ee, line_at + 0x40),
        normal: quad_at(ee, line_at + 0x50),
        across: quad_at(ee, line_at + 0x60),
        width: F(f32::from_bits(ee.u32_at(line_at + 0x70))),
        height: F(f32::from_bits(ee.u32_at(line_at + 0x74))),
        reach_sq: F(f32::from_bits(ee.u32_at(line_at + 0x7c))),
    };
    let ahead = paths.ahead;
    let mut t = Track { course: Course { line, line_spot: Spot::default(), lap_length: paths.lap_length }, paths, recs, nodes, ahead };
    t.course.line_spot = t
        .spot(ee.u32_at(line_at + 0x94), ee.u32_at(line_at + 0x98), F(f32::from_bits(ee.u32_at(line_at + 0x9c))))
        .expect("the line's spot is on the race track");
    t
}

/// The bike's word to its card for one tick off a captured row: where the card saw it, its look, its
/// velocity (`+0x110`) and the ground bit. C31 holds no ground under the rider, so the ground is the
/// one that leaves the captured rise.
fn feed_of(raw: &[u8], at: Vf) -> CardFeed {
    let rise = fl(raw, 0x104);
    CardFeed {
        at,
        look: vq(raw, 0x120),
        velocity: vq(raw, 0x110),
        ground: F(ps2_float::fpu::sub(at.y(), rise.0)),
        on_ground: u64::from(word(raw, 8)) & race_card_flags::GROUNDED != 0,
        // Down: the owner has dropped the jump watch every card of the race carries. C31 keeps no
        // bike word, so the captured card is the only witness of the original's wreck.
        wrecked: u64::from(word(raw, 8)) & race_card_flags::WATCHES_JUMPS == 0,
    }
}

#[test]
fn s377_the_sims_race_riders_hold_c31s_live_sx01_race() {
    let Ok(kit) = Kit::load() else { return skip("no kit.toml") };
    let image = kit.ctx("states").join(C31_STATE);
    let ee = match Ee::from_ram(&image) {
        Ok(ee) => ee,
        Err(why) => return skip(&format!("{why} — C31's sx01 start image")),
    };
    let Some(at) = named(&["activity_manager"]) else { return };
    let rel = C31_CSV.concat();
    let Ok(text) = std::fs::read_to_string(kit.ctx(&rel)) else {
        return skip(&format!("{rel} is not there: C31's sx01 capture"));
    };
    let track = track_of(&ee, at[0]);
    let rows = rows_of(&text);
    drop(text);
    let mut lane = lane_of();
    // Run free: seeded once, never again. Where the capture lost a tick its places are the next
    // row's, so a card's distances part from there (odometer, the path searches' spots and what
    // they feed); what the race is about must hold all the same.
    let free = run_free(&rows, &track, false, &mut lane);
    eprintln!("{free:#?}");
    let drift =
        ["ahead", "aim", "jumps", "kept_spot", "last", "odometer", "on_path", "racing card", "read_laps jumps", "spot", "to_line", "to_line_kept"];
    let held: std::collections::BTreeMap<_, _> = free.bad.iter().filter(|(k, _)| !drift.contains(k)).collect();
    assert!(held.is_empty(), "the free run's race parts from C31's (count, first four): {held:#?}");
    assert!(free.held > 10_000 && free.reads == 3 * free.held, "{free:?}");
    // Resynced: at every lost tick the cards and the manager start again from the next row whole,
    // and then every word of every card holds, bit for bit.
    let run = run_free(&rows, &track, true, &mut lane);
    eprintln!("{run:#?}");
    assert!(run.held > 9_500 && run.finished > 0 && run.place_changes > 0 && run.laps > 0, "{run:?}");
    // B38: the bots' habits fly and land with their cards.
    assert!(run.airborne > 0 && run.landed > 0, "{run:?}");
    assert!(run.bad.is_empty(), "words that part from C31's race (count, first four): {:#?}", run.bad);
}

fn run_free(rows: &[Row], track: &Track, resync: bool, lane: &mut Lane) -> Run {
    let race_time = |r: &Row, k: usize| fl(&r.raw[k], 0xb4);
    let started = |r: &Row| word(&r.mgr, 0x5c) & game_sim::ai::RACE_STARTED != 0;
    // The seed: the last row before the race clocks first run.
    let first = rows.iter().position(|r| started(r) && (0..4).any(|k| race_time(r, k) != F::ZERO)).expect("the race starts");
    let seed = &rows[first - 1];
    assert!(started(seed), "the seed row is after the drop");
    let order = order_of(&seed.mgr, &seed.raw);
    let cards: Vec<Entry> =
        order.iter().map(|&k| Entry { player: k, card: card_of(&seed.raw[k], track).expect("a seed card on the track"), stats: Tracker::default() }).collect();
    let mut paths = track.paths.clone();
    (paths.ahead, paths.behind) = race::RACE_REACH;
    let mut laps = Laps {
        rules: rules_of(&seed.mgr),
        course: track.course,
        lists: LineLists { reach: F(20.0), ..LineLists::default() },
        cards,
        track: std::sync::Arc::new(paths),
    };
    // Which tick a row holds goes by the race clock, which every racing card steps by the tick's dt.
    // A row is read at no fixed point of the frame: about one in thirteen holds the tick before's again
    // (read late), some hold one card already a tick on (read in the next frame's handler), a few
    // frames have no row. So each card's place for a tick comes from the first row whose card has that
    // tick's clock, and a row is held against the run only when every card still racing in it has.
    let mut run = Run::default();
    let mut rng = game_sim::rng::Rng::from_state(seed.rng);
    let mut next = first;
    let after = |t: F, dt: F| F(ps2_float::fpu::add(t.0, dt.0));
    let racing = |flags: u64| flags & card_flags::TAKING_PART != 0 && flags & card_flags::OUT == 0;
    let flags_of = |r: &Row, k: usize| u64::from(word(&r.raw[k], 8)) | u64::from(word(&r.raw[k], 12)) << 32;
    let settled = |r: &Row, k: usize| r.raw[k][0x60..0x6c] == r.raw[k][0x70..0x7c];
    // The tick a row holds whole: every card still racing on the same clock, and settled.
    let whole = |r: &Row| -> Option<F> {
        let ks: Vec<usize> = (0..4).filter(|&k| racing(flags_of(r, k))).collect();
        let t = race_time(r, *ks.first()?);
        ks.iter().all(|&k| race_time(r, k) == t && settled(r, k)).then_some(t)
    };
    loop {
        // The run's clock: its cards still racing (they share one).
        let Some(t) = laps.cards.iter().find(|e| racing(e.card.lap.flags)).map(|e| e.card.lap.race_time) else { break };
        // Rows wholly at or before the run's tick are behind it.
        let behind = |r: &Row| (0..4).filter(|&k| racing(flags_of(r, k))).all(|k| race_time(r, k).0 <= t.0);
        while next < rows.len() && behind(&rows[next]) {
            next += 1;
        }
        let Some(r) = rows.get(next) else { break };
        let (dt, tick) = (r.dt, after(t, r.dt));
        // Each card's row for this tick, in the next few rows.
        let window = &rows[next..rows.len().min(next + 6)];
        let src: Vec<Option<&Row>> = (0..4)
            .map(|k| {
                let ours = laps.cards.iter().find(|e| e.player == k).unwrap();
                if !racing(ours.card.lap.flags) {
                    return Some(r);
                }
                window.iter().find(|w| race_time(w, k) == tick && settled(w, k))
            })
            .collect();
        let held = whole(r) == Some(tick);
        if src.iter().any(Option::is_none) {
            run.guessed += 1;
            let Some(g) = rows[next..].iter().position(|w| whole(w).is_some_and(|w| w.0 > t.0)).map(|g| next + g) else { break };
            if resync {
                // A card's place this tick is nowhere in the capture: the cards and the manager start
                // again from the next whole row; the track's search scratch stays the run's.
                let w = &rows[g];
                let order = order_of(&w.mgr, &w.raw);
                laps.cards = order
                    .iter()
                    .map(|&j| Entry { player: j, card: card_of(&w.raw[j], track).expect("a card on the track"), stats: Tracker::default() })
                    .collect();
                laps.rules = rules_of(&w.mgr);
                next = g + 1;
                continue;
            }
        }
        // A card whose place is lost rides where the next whole row has it.
        let feeds: Vec<CardFeed> = (0..4)
            .map(|k| {
                let w = src[k].unwrap_or_else(|| rows[next..].iter().find(|w| whole(w).is_some_and(|w| w.0 > t.0)).unwrap());
                feed_of(&w.raw[k], vq(&w.raw[k], 0x60))
            })
            .collect();
        // The owner's bits, as its listeners left them.
        for e in &mut laps.cards {
            let w = src[e.player].unwrap_or(r);
            e.card.lap.flags = e.card.lap.flags & !OWNER_FLAGS | flags_of(w, e.player) & OWNER_FLAGS;
        }
        let src = r;
        let before: Vec<(usize, i32, i32)> = laps.cards.iter().map(|e| (e.player, e.card.lap.place, e.card.lap.laps)).collect();
        let _ = laps.tick(dt, &feeds, &mut rng);
        run.ticks += 1;
        for (e, b) in laps.cards.iter().zip(&before) {
            run.order_changes += usize::from(e.player != b.0);
        }
        for e in &laps.cards {
            let &(_, place, l) = before.iter().find(|b| b.0 == e.player).unwrap();
            run.place_changes += usize::from(e.card.lap.place != place);
            run.laps += usize::from(e.card.lap.laps != l);
        }
        if !held {
            continue;
        }
        let row = src;
        let f = row.frame;
        run.held += 1;
        run.end = f;
        // The list order: places, once ranked.
        let want = order_of(&row.mgr, &row.raw);
        let got: Vec<usize> = laps.cards.iter().map(|e| e.player).collect();
        if got != want {
            run.differ("order", format!("frame {f}: got {got:?} want {want:?}"));
        }
        if laps.rules.lap.flags != word(&row.mgr, 0x5c) {
            run.differ("manager flags", format!("frame {f}: got {:#x} want {:#x}", laps.rules.lap.flags, word(&row.mgr, 0x5c)));
        }
        for k in 0..4 {
            let theirs = card_of(&row.raw[k], track).expect("a captured card on the track");
            let ours = &laps.cards.iter().find(|e| e.player == k).unwrap().card;
            for ((name, g), (_, w)) in words(ours).into_iter().zip(words(&theirs)) {
                if g != w {
                    run.differ(name, format!("frame {f} rider {k}: got {g:#x} want {w:#x}"));
                }
            }
            run.finished += usize::from(ours.lap.flags & card_flags::OUT != 0);
            hand_off(&laps, k, &theirs, &row.mgr, track, &mut run, f);
        }
        // The lane's pilots read their race again before they think: their card's flags and progress,
        // live, the race phase.
        for (p, pilot) in &mut lane.pilots {
            pilot.read_laps(&laps, true);
            let theirs = &card_of(&row.raw[*p], track).expect("a captured card").lap;
            let got = (pilot.card.map(|c| (c.flags, c.progress.0.to_bits())), pilot.seat.live, pilot.seat.race_phase, pilot.seat.path_net);
            let want = (Some((theirs.flags, theirs.progress.0.to_bits())), started(row), Some(word(&row.mgr, 0x60) as i32), true);
            if got != want {
                run.differ("read_laps", format!("frame {f} rider {p}: got {got:x?} want {want:x?}"));
            }
            // B38: the habits' air time, ground clock and racing bit, as the race's jump watch left
            // the card.
            let h = &pilot.racer.habits;
            let jumps = card_of(&row.raw[*p], track).expect("a captured card").jumps;
            let got = (h.air_time.0.to_bits(), h.lag_clock.0.to_bits(), h.racing);
            let want = (jumps.air_time.0.to_bits(), jumps.grounded.0.to_bits(), theirs.flags & race_card_flags::WATCHES_JUMPS != 0);
            if got != want {
                run.differ("read_laps jumps", format!("frame {f} rider {p}: got {got:x?} want {want:x?}"));
            }
            run.airborne += usize::from(h.air_time != F::ZERO && h.lag_clock == F::ZERO);
            run.landed += usize::from(h.air_time != F::ZERO && h.lag_clock != F::ZERO);
            run.reads += 1;
        }
    }
    run
}

/// What the Sim's AI lane hands player `k`'s brain off the laps this tick, against the original's card
/// and manager words the brain reads through its card and race pointers.
fn hand_off(laps: &Laps, k: usize, theirs: &RaceCard, mgr: &[u8], track: &Track, run: &mut Run, f: u32) {
    let a = Racing::of(Some(laps), k, None);
    let b = racing_of(laps, k, None);
    let race = a.race.expect("a race");
    let card = a.card.expect("a card");
    let words_of = |r: &Racing| (r.race, r.card, r.gate, r.line, r.race_word, r.track.map(|t| t.paths.len()));
    if words_of(&a) != words_of(&b) {
        run.differ("Racing::of vs racing_of", format!("frame {f} rider {k}"));
    }
    let l = &theirs.lap;
    let want_card = (l.flags, l.to_line_kept.0.to_bits(), l.to_line.0.to_bits(), l.on_track as u32, l.seen.now.to_bits(), l.seen.last.to_bits(), l.kept_spot);
    let got_card = (card.flags, card.to_line_kept.0.to_bits(), card.to_line.0.to_bits(), card.on_track, card.seen.now.to_bits(), card.seen.last.to_bits(), card.kept_spot);
    if (got_card.0 & !OWNER_FLAGS, got_card.1, got_card.2, got_card.3, got_card.4, got_card.5, got_card.6)
        != (want_card.0 & !OWNER_FLAGS, want_card.1, want_card.2, want_card.3, want_card.4, want_card.5, want_card.6)
    {
        run.differ("racing card", format!("frame {f} rider {k}: got {got_card:x?} want {want_card:x?}"));
    }
    let want_race = (word(mgr, 0x60) as i32, track.ahead.0.to_bits(), track.course.line_spot, word(mgr, 0x5c));
    let got_race = (race.kind, race.track_reach.0.to_bits(), race.line_spot, a.race_word.unwrap_or(0));
    if got_race != want_race {
        run.differ("racing race", format!("frame {f} rider {k}: got {got_race:x?} want {want_race:x?}"));
    }
    if a.line != Some((track.course.line.centre, track.course.line.normal)) || a.gate.is_some() || card.gate.is_some() {
        run.differ("racing line/gate", format!("frame {f} rider {k}"));
    }
    run.hand_offs += 1;
}

/// A lane of three AI pilots in players 1–3, built as the Sim's load builds one (no character file).
fn lane_of() -> Lane {
    let riders: Vec<(usize, AiRider)> = (1..4).map(|p| (p, AiRider { identity: game_sim::ai::FIRST_AI_IDENTITY, skill: 0 })).collect();
    let entry = game_sim::ai::Entry { race_phase: None, path_net: false, live: false, card: None };
    Lane::spawn(
        game_sim::ai::seek::World::default(),
        std::sync::Arc::new(game_sim::ragdoll::Tables::build()),
        Rng::from_state(1),
        &riders,
        None,
        0,
        0,
        &entry,
    )
}

/// `Sim::load_ai_net` (S332) on sx01AI.tdf off the disc, stood on sx01's ground, against the AI net
/// in C31's start image: its head words (the race reach, the fork pick, the lap length) and every path's
/// flags, length, word, links and nodes.
#[test]
fn s377_the_sims_ai_net_is_c31s() {
    use super::path_nets::sx01_files;
    let Some(files) = sx01_files("Tracks\\sx01_2.res", &["tracks\\sx01\\sx01ai.tdf"]) else { return };
    let Some(track) = sx01_files("Tracks\\sx01.res", &["tracks\\sx01\\sx01.scn"]) else { return };
    let scene = game_sim::track::Scene::parse(&track[0]).expect("sx01's scene");
    let trn = scene.terrain_file.to_ascii_lowercase().replace(".ps2.trn", ".trn").replace(".trn", ".ps2.trn");
    let trn = sx01_files("Tracks\\sx01.res", &[&trn]).expect("sx01.res read once already");
    let terrain = game_sim::track::Terrain::parse(&trn[0]).expect("sx01's terrain");
    let mut ground = game_sim::world::Ground::new(&terrain, &scene.terrain);
    let Ok(kit) = Kit::load() else { return skip("no kit.toml") };
    let ee = match Ee::from_ram(&kit.ctx("states").join(C31_STATE)) {
        Ok(ee) => ee,
        Err(why) => return skip(&format!("{why} — C31's sx01 start image")),
    };
    let Some(at) = named(&["activity_manager"]) else { return };
    let act = ee.u32_at(ee.u32_at(ee.u32_at(at[0]) + 0x58));
    let (theirs, _, _) = s309_paths(&ee, ee.u32_at(act + 0xbc));

    let file = race::TrackFile::parse(&files[0]).expect("sx01AI.tdf reads");
    let mut sim = Sim::new(0);
    assert!(!sim.load_ai_net(&file, Some(&mut ground), None), "no lane, no net");
    sim.ai = Some(Box::new(lane_of()));
    assert!(sim.load_ai_net(&file, Some(&mut ground), None), "sx01AI.tdf's paths walk");
    let mine = sim.ai.as_ref().and_then(|l| l.net.clone()).expect("the lane holds the net");
    let head = |t: &race::TrackPaths| (t.word_34, t.ahead.0.to_bits(), t.behind.0.to_bits(), t.pick, t.lap_length.0.to_bits());
    assert_eq!(head(&mine), head(&theirs), "sx01AI.tdf: reach, pick, tag word and lap length");
    assert_eq!(mine.paths.len(), theirs.paths.len(), "sx01AI.tdf: the path count");
    for (k, (m, t)) in mine.paths.iter().zip(&theirs.paths).enumerate() {
        assert_eq!(
            (m.flags, m.length.0.to_bits(), m.word_0c, &m.links, format!("{:?}", m.nodes)),
            (t.flags, t.length.0.to_bits(), t.word_0c, &t.links, format!("{:?}", t.nodes)),
            "sx01AI.tdf: path {k}"
        );
    }
}
