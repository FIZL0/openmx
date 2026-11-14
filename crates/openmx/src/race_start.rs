//! P327: a race's start as the shell lays it out from what `game_sim::race` has — the start gate off
//! the scene's drop-gate prop ([`Supercross::place_drop_gate`], S13e), the activity's reset with the
//! track's start calls (S13a), the grid slot's lanes and spot for the human's card, and the closed
//! gate with its countdown to the drop that `Sim::race` steps (S13b).
//!
//! The port's own where the original's code was not ported (S297's prop lookup and bound): the prop is
//! the first scene model whose name has `dropgate` in it, as the original looks its props up by that
//! name, and its bound is the box round the model's finest triangles. The activity is the
//! countdown's listener, so its start timeline holds the rider from the countdown's zero to the
//! warning window (S290, S304), and the gate's watch catches a rider who rolls through after that;
//! the intro is counted as seen and not shown.

use game_render::scene::Placement;
use game_render::V3;
use game_sim::race::{self, GridStart, Prop, Race, Supercross};
use game_sim::rng::Rng;
use ps2_float::{F, Vf};

/// The scene prop the start gate stands on: its name has this in it (sx01's `THQsxdropgate`, the
/// nationals' `Nat15_Dropgate01`).
pub const DROP_GATE: &str = "dropgate";

/// What a race's start puts the human at, and the gate that holds them.
#[derive(Clone, Debug)]
pub struct Start {
    /// The grid spot before the ground snaps it, and the way it faces.
    pub spot: Vf,
    pub facing: Vf,
    /// The human's lane and starting place (from 1), of [`Start::riders`].
    pub lane: i32,
    pub place: i32,
    pub riders: i32,
    /// The start gate, closed, and the seconds from the reset to its drop.
    pub gate: race::Gate,
    pub delay: F,
    /// The race's own draws, after the reset's and the lanes'.
    pub rng: Rng,
    /// The race's activity after its reset and the lanes: the countdown's listener.
    pub activity: Supercross,
    /// The race's laps off the track's race file (P313), its cards where the riders stand: `None`, a
    /// start alone.
    pub laps: Option<race::Laps>,
}

impl Start {
    /// The race `Sim::race` steps, with the rider standing at `rider` (its bike's translation and
    /// forward row).
    pub fn race(&self, rider: (Vf, Vf)) -> Race {
        let mut race = Race::new(self.gate, &[rider], self.delay, self.rng.clone());
        race.activity = Some(self.activity.clone());
        race.laps = self.laps.clone().map(Box::new);
        race
    }
}

/// The drop-gate prop off the scene: the first placement named [`DROP_GATE`], its frame's across row
/// (`up × look`), its look and its place, and half its model's box across and up (`bound`, the
/// model's box as a centre and half extents).
pub fn drop_prop(models: &[Placement], bound: impl Fn(&str) -> Option<(V3, V3)>) -> Option<Prop> {
    let p = models.iter().find(|p| p.model.to_ascii_lowercase().contains(DROP_GATE))?;
    let (_, half) = bound(&p.model)?;
    let (u, l) = (p.up, p.look);
    let across = V3 { x: u.y * l.z - u.z * l.y, y: u.z * l.x - u.x * l.z, z: u.x * l.y - u.y * l.x };
    Some(Prop {
        across: Vf::new(across.x, across.y, across.z, 0.0),
        look: Vf::new(l.x, l.y, l.z, 0.0),
        at: Vf::new(p.pos.x, p.pos.y, p.pos.z, 1.0),
        half_width: F(half.x),
        height: F(half.y),
    })
}

/// The start calls' gate-drop times out of the race events' `event.aud`: the `SoundNGateDropTime`
/// keys of the `[RaceStart<class>]` section for the bike's class (the digits its name starts with,
/// `125sx` → 125), or of the first `RaceStart` section when there is none for it.
pub fn drop_times(event_aud: &str, bike: &str) -> Vec<F> {
    let class: String = bike.chars().take_while(|c| c.is_ascii_digit()).collect();
    let mut sections: Vec<(String, Vec<F>)> = Vec::new();
    for line in event_aud.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            sections.push((name.to_string(), Vec::new()));
            continue;
        }
        let (Some((key, value)), Some((_, times))) = (line.split_once('='), sections.last_mut()) else { continue };
        let key = key.trim();
        if key.starts_with("Sound")
            && key.ends_with("GateDropTime")
            && let Ok(t) = value.trim().parse::<f32>()
        {
            times.push(F(t));
        }
    }
    let starts = || sections.iter().filter(|(n, t)| n.starts_with("RaceStart") && !t.is_empty());
    let mine = starts().find(|(n, _)| n.eq_ignore_ascii_case(&format!("RaceStart{class}")));
    mine.or_else(|| starts().next()).map(|(_, t)| t.clone()).unwrap_or_default()
}

/// A race's start for `riders` riders off the drop-gate prop and the start calls' drop times: the
/// activity reset (the intro counted as seen, so it is skipped), the lanes shuffled with every rider
/// a player's, and card 0's grid spot. `None` without a prop or a start call.
pub fn lay_out(prop: &Prop, drops: Vec<F>, riders: i32, seed: u32) -> Option<Start> {
    if drops.is_empty() {
        return None;
    }
    let riders = riders.clamp(1, race::LANES as i32);
    let mut start = race::Gate::new();
    Supercross::place_drop_gate(prop, None, &mut start);
    let mut s = Supercross::new();
    s.lane_offsets = Supercross::lane_table();
    s.drop_times = drops;
    s.intro_seen = 1;
    let mut rng = Rng::from_seed(seed);
    s.reset(&mut start, &mut rng);
    s.shuffle_lanes(riders as usize, false, &mut rng);
    let (spot, facing) = s.grid_spot(GridStart::Gate { centre: start.centre, normal: start.normal, width: start.width }, 0, riders);
    let (lane, place, delay) = (s.lanes[0], s.starting_place(0), s.start_delay);
    Some(Start { spot, facing, lane, place, riders, gate: start, delay, rng, activity: s, laps: None })
}

/// The finish-gate prop off the scene, read as [`drop_prop`] reads the drop gate's: the first
/// placement whose name has `finishgate` in it ([`race::FINISH_GATE`]).
pub fn finish_prop(models: &[Placement], bound: impl Fn(&str) -> Option<(V3, V3)>) -> Option<Prop> {
    let needle = std::str::from_utf8(race::FINISH_GATE).expect("an ascii name");
    let p = models.iter().find(|p| p.model.to_ascii_lowercase().contains(needle))?;
    // Read as the drop gate is, under the drop gate's name.
    drop_prop(&[Placement { model: DROP_GATE.into(), ..p.clone() }], |_| bound(&p.model))
}

/// A toy as a scene placement, for [`finish_prop`] when the finish gate is one of the track's toys
/// (sx01's `FinishGate`): its model where its spec stands it (the rows across, up, look, place).
pub fn toy_placement(model: &str, place: &game_sim::math::Mat4) -> Placement {
    let v = |r: Vf| V3 { x: r.x(), y: r.y(), z: r.z() };
    Placement { model: model.into(), pos: v(place.0[3]), look: v(place.0[2]), up: v(place.0[1]) }
}

/// The race file a track's laps are counted on: `tracks\<stem>\<stem>.tdf` in its second archive.
pub fn race_file_name(stem: &str) -> String {
    format!("tracks\\{stem}\\{stem}.tdf")
}

/// P313: a race's laps off the track's race file, as the track object and the supercross loader leave
/// it — the file read, its paths snapped to `terrain` (S295), the paths walked and their gate spots
/// found (S293), and the finish line stood in the finish-gate prop facing along the path at its new
/// spot (S297) — with a card per rider in `riders` (where each stands and its card flags). The grid
/// gate is not read: the start is the drop gate's. `None` for a file that does not read or load.
///
/// The port's own: the nodes the original raises onto the scene's props (S296a) stay on the terrain
/// (sx01's file has none), and a track with no finish-gate prop in its scene keeps the file's line
/// (the original looks in the track's toys next).
pub fn laps(file: &[u8], terrain: &mut dyn game_sim::world::World, finish: Option<&Prop>, laps: i32, riders: &[(Vf, u64)]) -> Option<race::Laps> {
    let mut t = race::TrackFile::parse(file).ok()?;
    let raised = t.snap_to_ground(Some(terrain));
    if !raised.is_empty() {
        println!("race: {} path nodes left on the ground, not raised onto the props", raised.len());
    }
    let (mut paths, spots) = race::TrackPaths::load(&t)?;
    let mut line = t.line?;
    let mut line_spot = spots[1]?;
    if let Some(finish) = finish {
        line.centre = line.centre.merge(ps2_float::Dest::XYZ, finish.at);
        game_sim::ai::follow::spot(&mut paths, line.centre, false, &mut line_spot, None);
        let mut ahead = Vf::new(0.0, 0.0, 1.0, 0.0);
        paths.path_ahead(line_spot, &mut ahead);
        Supercross::place_finish_line(finish, ahead, &mut line);
    }
    line_spot.at()?;
    let course = race::Course { line, line_spot, lap_length: paths.lap_length };
    Some(race::Laps::new(race::supercross_rules(laps), course, paths, riders))
}

/// P313: the readouts a card gives the HUD: the lap it rides (one past the laps it has run, held at
/// the race's last) and its place, a dash until the lap line ranks it.
pub fn shown(card: &race::LapCard, laps: u32) -> (Option<u32>, Option<u32>) {
    let lap = (card.laps.max(0) as u32 + 1).min(laps.max(1));
    (Some(lap), u32::try_from(card.place).ok().filter(|&p| p > 0))
}

/// What changed on the race's lap line since the shell last looked ([`Watch::look`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// Player `player` started lap `lap` (from 1).
    Lap { player: usize, lap: i32 },
    /// Player `player` is now in place `place`.
    Place { player: usize, place: i32 },
    /// Player `player`'s card ran its laps: done at the line, in its place.
    Finished { player: usize, place: i32 },
    /// The race is over (the lap line told the Over).
    Over,
}

/// P313: the shell's view of the race's lap line, which the Sim runs and does not hand on: each
/// card's laps, place and whether it is out, and whether the Over was told, to tell what changed.
#[derive(Clone, Debug, Default)]
pub struct Watch {
    seen: Vec<(usize, i32, i32, bool)>,
    over: bool,
    /// Seconds since the Over, for the outro's wait.
    pub since_over: Option<f32>,
    /// What `race_over` answered at each card's finish, in finishing order.
    pub overs: Vec<race::RaceOver>,
}

/// How long the original's race holds its end before the outro (its race-end wait): the results
/// come up then.
pub const OUTRO_WAIT: f32 = 7.0;

impl Watch {
    /// What changed on `laps` since the last look, in the cards' list order, the Over last.
    pub fn look(&mut self, laps: &race::Laps) -> Vec<Change> {
        let mut out = Vec::new();
        for e in &laps.cards {
            let c = &e.card.lap;
            let now = (e.player, c.laps, c.place, c.flags & race::card_flags::OUT != 0);
            let was = self.seen.iter_mut().find(|s| s.0 == e.player);
            let Some(was) = was else {
                self.seen.push(now);
                continue;
            };
            if now.1 > was.1 && now.1 < laps.rules.lap.laps {
                out.push(Change::Lap { player: e.player, lap: now.1 + 1 });
            }
            if now.2 != was.2 && now.2 > 0 {
                out.push(Change::Place { player: e.player, place: now.2 });
            }
            if now.3 && !was.3 && now.1 >= laps.rules.lap.laps {
                out.push(Change::Finished { player: e.player, place: now.2 });
            }
            *was = now;
        }
        if laps.over() && !self.over {
            self.over = true;
            self.since_over = Some(0.0);
            out.push(Change::Over);
        }
        out
    }

    /// A tick of the outro's wait: whether it ran out this tick.
    pub fn wait(&mut self, dt: f32) -> bool {
        let Some(t) = &mut self.since_over else { return false };
        let was = *t;
        *t += dt;
        was < OUTRO_WAIT && *t >= OUTRO_WAIT
    }
}

/// What the race hands the results screen at its Over: the cards in the lap line's list order (the
/// human's named `player`, the rest the computer's), `race_end`'s answer, and the finishes' answers.
///
/// The port's own: the riders' statistics trackers are not kept (their words are 0), and the human's
/// seat reports no preloads or clutches.
pub fn hand_over(laps: &race::Laps, laps_run: u8, player: &str, overs: &[race::RaceOver]) -> crate::menu::results::HandOver {
    use crate::menu::results::{CardStats, HandOver, ResultCard, Who};
    let cards = laps
        .cards
        .iter()
        .map(|e| {
            let who = if e.player == 0 { Who::Player(player.to_string()) } else { Who::Ai(format!("Rider {}", e.player + 1)) };
            ResultCard::take(&e.card.lap, who, &CardStats::default())
        })
        .collect();
    let seats: Vec<(i32, Option<race::EndSeat>)> = laps
        .cards
        .iter()
        .map(|e| (e.card.lap.place, (e.player == 0).then_some(race::EndSeat { seat: 0, preloads: 0, clutches: 0 })))
        .collect();
    let event = race::ResultEvent { activity: 2, laps: laps_run, ..race::ResultEvent::default() };
    let end = Supercross::new().race_end(&seats, &event, false);
    HandOver { cards, end, overs: overs.to_vec() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A race of two whose cards are moved by hand, as the lap line would move them.
    fn two_cards() -> race::Laps {
        let card = |player, flags| race::Entry { player, card: race::RaceCard { lap: race::LapCard { flags, ..race::LapCard::default() }, ..race::RaceCard::default() } };
        race::Laps {
            rules: race::supercross_rules(2),
            course: race::Course { line: race::Gate::new(), line_spot: race::Spot::default(), lap_length: F(100.0) },
            lists: race::LineLists::default(),
            cards: vec![card(0, race::HUMAN_CARD), card(1, race::COMPUTER_CARD)],
            track: std::sync::Arc::new(race::TrackPaths::default()),
        }
    }

    /// P313: the watch says each lap, place and finish once, the Over once, and the outro's wait
    /// runs out once, seven seconds on; the hand-over takes the cards in list order, the human named.
    #[test]
    fn the_watch_tells_laps_places_the_finish_and_the_over() {
        let mut laps = two_cards();
        let mut w = Watch::default();
        assert!(w.look(&laps).is_empty(), "the first look only sees");
        laps.cards[0].card.lap.place = 1;
        laps.cards[1].card.lap.place = 2;
        laps.cards[0].card.lap.laps = 1;
        assert_eq!(w.look(&laps), vec![Change::Lap { player: 0, lap: 2 }, Change::Place { player: 0, place: 1 }, Change::Place { player: 1, place: 2 }]);
        assert!(w.look(&laps).is_empty());
        assert_eq!(shown(&laps.cards[0].card.lap, 2), (Some(2), Some(1)));
        // The last lap is no new lap: it is the finish.
        laps.cards[0].card.lap.laps = 2;
        laps.cards[0].card.lap.flags |= race::card_flags::OUT;
        assert_eq!(w.look(&laps), vec![Change::Finished { player: 0, place: 1 }]);
        assert_eq!(shown(&laps.cards[0].card.lap, 2), (Some(2), Some(1)));
        laps.cards[1].card.lap.laps = 2;
        laps.cards[1].card.lap.flags |= race::card_flags::OUT;
        laps.rules.lap.flags |= race::race_flags::OVER_TOLD;
        // Its laps counted from none to the last at once: a finish, no lap between.
        assert_eq!(w.look(&laps), vec![Change::Finished { player: 1, place: 2 }, Change::Over]);
        assert!(w.look(&laps).is_empty());
        let ticks = (0..300).filter(|_| w.wait(1.0 / 30.0)).count();
        assert_eq!(ticks, 1);
        assert!(w.since_over.is_some_and(|t| t >= OUTRO_WAIT));
        let hand = hand_over(&laps, 2, "Me", &w.overs);
        assert!(hand.end.results, "a single race shows its results");
        assert_eq!(hand.cards.len(), 2);
        assert_eq!(hand.cards[0].who, crate::menu::results::Who::Player("Me".into()));
        assert_eq!(hand.cards[1].who, crate::menu::results::Who::Ai("Rider 2".into()));
        assert_eq!(hand.cards[1].words[0], F(2.0));
    }

    #[test]
    fn the_bikes_class_picks_its_start_calls() {
        let aud = "[RaceStart50]\nSound1GateDropTime=1.9\nSound2GateDropTime=2.1\n[RaceStart125]\nSound1 = x\nSound1GateDropTime = 1.54\n[Other]\nSound1GateDropTime=9\n";
        assert_eq!(drop_times(aud, "125sx"), vec![F(1.54)]);
        assert_eq!(drop_times(aud, "50cc"), vec![F(1.9), F(2.1)]);
        assert_eq!(drop_times(aud, "250f"), vec![F(1.9), F(2.1)]);
        assert!(drop_times("", "125sx").is_empty());
    }

    #[test]
    fn the_sx01_prop_stands_the_gate_where_the_image_has_it() {
        // sx01's `THQsxdropgate` placement, and its box's half extents as S196's image holds them.
        let p = Placement {
            model: "THQsxdropgate".into(),
            pos: V3 { x: 101.493164, y: 90.12307, z: 458.47986 },
            look: V3 { x: 0.0, y: 0.5, z: 0.866025 },
            up: V3 { x: 0.0, y: 0.866025, z: -0.5 },
        };
        let prop = drop_prop(&[p], |_| Some((V3 { x: 0.0, y: 0.0, z: 0.0 }, V3 { x: 59.273155, y: 1.3735971, z: 1.0 }))).unwrap();
        let s = lay_out(&prop, vec![F(1.54)], 1, 1).unwrap();
        let want = [101.49316, 87.87306, 454.5827];
        for (got, want) in [s.gate.centre.x(), s.gate.centre.y(), s.gate.centre.z()].into_iter().zip(want) {
            assert!((got - want).abs() < 1e-3, "{:?}", s.gate.centre);
        }
        assert!((s.gate.across.x() - 1.0).abs() < 1e-6);
        assert!(s.gate.flags & race::flags::CLOSED != 0);
        assert_eq!(s.delay, F(1.5) + F(1.54));
        assert_eq!((s.lane, s.place), (0, 1));
        // Behind the gate, facing through it.
        assert!(s.spot.z() < s.gate.centre.z() && s.facing.z() > 0.9);
        // The race listens to the start's timeline, which holds the rider from the countdown's zero.
        let race = s.race((s.spot, s.facing));
        assert_eq!(race.activity.as_ref().map(|a| a.start_delay), Some(s.delay));
        assert_eq!(race.countdown.length, s.delay);
    }
}
