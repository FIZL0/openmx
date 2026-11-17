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

use game_render::gpu::Actor;
use game_render::mat::M4;
use game_render::scene::{Placement, Scene};
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
    /// P313a: every card's grid spot and facing, card 0 the human's ([`Start::spot`]) and the rest
    /// the AI riders', before the ground snaps them.
    pub grid: Vec<(Vf, Vf)>,
    /// P313a: the AI riders' bikes standing on their grid spots, card 1 first: the Sim's players
    /// after the human, in card order.
    pub ai: Vec<game_sim::rider::Rider>,
    /// P313c: the drop gate's sink, on a track whose gate sinks ([`gate_sink`]).
    pub sink: Option<game_sim::drop_gate::Sink>,
}

impl Start {
    /// The race `Sim::race` steps, with the rider standing at `rider` (its bike's translation and
    /// forward row).
    pub fn race(&self, rider: (Vf, Vf)) -> Race {
        self.race_of(&[rider])
    }

    /// [`Start::race`] with every rider on the grid in player order (P313a): the human, then the AI
    /// riders, each as its bike's translation and forward row.
    pub fn race_of(&self, riders: &[(Vf, Vf)]) -> Race {
        let mut race = Race::new(self.gate, riders, self.delay, self.rng.clone());
        race.activity = Some(self.activity.clone());
        race.laps = self.laps.clone().map(Box::new);
        race.sink = self.sink.clone().map(Box::new);
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
    let grid: Vec<(Vf, Vf)> =
        (0..riders).map(|card| s.grid_spot(GridStart::Gate { centre: start.centre, normal: start.normal, width: start.width }, card, riders)).collect();
    let (spot, facing) = grid[0];
    let (lane, place, delay) = (s.lanes[0], s.starting_place(0), s.start_delay);
    Some(Start { spot, facing, lane, place, riders, gate: start, delay, rng, activity: s, laps: None, grid, ai: Vec::new(), sink: None })
}

/// P313a: how many entries the rider database holds, which the roster's identity draws pick from
/// (the same in every RAM image read: C33's and S138's).
pub const RIDER_IDENTITIES: i32 = 99;

/// P313a: the seed of the generator the roster and the AI riders' spawns draw from. The port's own:
/// the original's comes off the wall clock.
pub const AI_SEED: u32 = 0;

/// P313a: the event's AI difficulty, the event's default (no menu here writes one).
pub const AI_DIFFICULTY: u8 = 90;

/// P313a: the event's kind word an AI rider's brain picks its mode from: a race's (0 and 1 both
/// give the race driver's mode, [`game_sim::ai::spawn_mode`]).
pub const RACE_EVENT_KIND: i32 = 0;

/// P313a: what an AI rider's seat and card start a race with ([`game_sim::ai::Entry`]): live, no
/// race phase or path net yet, and a computer's card.
pub fn ai_entry() -> game_sim::ai::Entry {
    let card = game_sim::ai::Card { flags: race::COMPUTER_CARD, ..game_sim::ai::Card::default() };
    game_sim::ai::Entry { race_phase: None, path_net: false, live: true, card: Some(card) }
}

/// P313a: the race's AI riders in `sim` at the load: the roster for `riders` racers, `humans` of
/// them local seats ([`game_sim::ai::spawn_roster`] at [`AI_DIFFICULTY`] over [`RIDER_IDENTITIES`]),
/// then each AI rider's brain off the bike's character file `file` ([`game_sim::Sim::spawn_ai`]),
/// drawing from the race's generator when the Sim has a race, else one seeded `seed`. The Sim must
/// hold every AI rider's bike after the first human's, and its seeker world (`Sim::load_seek`)
/// first. P350: the other seats' humans are Sims of their own, so in `sim` the AI riders follow the
/// one human. Answers the roster as drawn.
pub fn spawn_ai(sim: &mut game_sim::Sim, riders: i32, humans: i32, file: Option<&game_sim::phy::Ini>, seed: u32) -> game_sim::ai::Roster {
    use game_sim::ai::{RosterIn, spawn_roster};
    // S333: in a race, the roster draws on, and leaves, the race's generator: one for the load and the race.
    let mut rng = sim.race.as_ref().map_or_else(|| Rng::from_seed(seed), |r| r.rng);
    let input = RosterIn { racers: riders, humans, difficulty: AI_DIFFICULTY, identities: RIDER_IDENTITIES, all_ai: false, ghost_race: false };
    let roster = spawn_roster(&mut rng, input);
    let in_sim = game_sim::ai::Roster { humans: 1, ..roster.clone() };
    sim.spawn_ai(&in_sim, file, AI_DIFFICULTY, RACE_EVENT_KIND, &ai_entry(), rng);
    roster
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

/// S331b: the track's AI path file, beside its race file: `tracks\<stem>\<stem>AI.tdf`.
pub fn ai_file_name(stem: &str) -> String {
    format!("tracks\\{stem}\\{stem}AI.tdf")
}

/// S331b: the track's AI net off its AI file `file`, stood on `terrain` ([`game_sim::ai::race_net`]),
/// handed to `sim`'s AI lane for its race drivers to follow. Answers the net's path count, `None` for
/// a file that does not read or load, or a Sim with no lane.
pub fn give_ai_net(sim: &mut game_sim::Sim, file: &[u8], terrain: &mut dyn game_sim::world::World) -> Option<usize> {
    let t = race::TrackFile::parse(file).ok()?;
    let net = game_sim::ai::race_net(&t, Some(terrain))?;
    let paths = net.paths.len();
    sim.give_ai_net(net).then_some(paths)
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
/// Each card's statistics are its bike's tracker (S319) beside the card's longest jump and longest
/// time in the air; the human's seat reports its tracker's preloads and clutches.
pub fn hand_over(laps: &race::Laps, laps_run: u8, player: &str, overs: &[race::RaceOver]) -> crate::menu::results::HandOver {
    hand_over_seats(laps, laps_run, &[(0, player.to_string())], overs)
}

/// P350: [`hand_over`] with several local seats' humans: `players` is each seat's race player
/// ([`Laps`](race::Laps)' card player) and name, seat 0 first; every other card is the computer's.
pub fn hand_over_seats(laps: &race::Laps, laps_run: u8, players: &[(usize, String)], overs: &[race::RaceOver]) -> crate::menu::results::HandOver {
    use crate::menu::results::{CardStats, HandOver, ResultCard, Who};
    let seat_of = |player: usize| players.iter().position(|p| p.0 == player);
    let cards = laps
        .cards
        .iter()
        .map(|e| {
            let who = match seat_of(e.player) {
                Some(k) => Who::Player(players[k].1.clone()),
                None => Who::Ai(format!("Rider {}", e.player + 1)),
            };
            let t = &e.stats;
            let stats = CardStats {
                laps_led: t.laps_led,
                times_in_lead: t.times_in_lead,
                passes: t.passes,
                times_passed: t.times_passed,
                wrecks: t.wrecks,
                preloads: t.preloads,
                clutches: t.clutches,
                jump: e.card.jumps.best_length,
                hang: e.card.jumps.best_air_time,
            };
            ResultCard::take(&e.card.lap, who, &stats)
        })
        .collect();
    let seats: Vec<(i32, Option<race::EndSeat>)> = laps
        .cards
        .iter()
        .map(|e| (e.card.lap.place, seat_of(e.player).map(|k| race::EndSeat { seat: k as i32, preloads: e.stats.preloads as u32, clutches: e.stats.clutches as u32 })))
        .collect();
    let event = race::ResultEvent { activity: 2, laps: laps_run, ..race::ResultEvent::default() };
    let end = Supercross::new().race_end(&seats, &event, false);
    HandOver { cards, end, overs: overs.to_vec() }
}

/// P313b: how long the drop gate takes to swing down, in seconds. The race's drop hands the gate
/// object its turn with this fixed length. The track's `RotationRateInSeconds` is read but never used.
pub const SWING_SECONDS: f32 = 0.15;

/// P313c: what a track's `.toy` says of its drop gate's sink: `[DropGate]`'s `GateName`,
/// `MoveGateDown`, `MoveTime` and its two piece counts, and `[LowerGate]`'s `TriggerPosition`.
#[derive(Clone, Debug, PartialEq)]
pub struct SinkWords {
    pub name: String,
    pub move_down: bool,
    pub move_time: f32,
    pub soultree: i32,
    pub collision: i32,
    pub trigger_at: [f32; 3],
}

impl SinkWords {
    /// The words, with the reader's defaults: no sink, 5 seconds, no pieces, and the trigger at the
    /// origin. nat12, nat16 and nat19 spell the trigger's key `Position`, which the reader does not
    /// know, so theirs is at the origin too. `None` with no `[DropGate]`.
    pub fn read(toy: &str) -> Option<SinkWords> {
        let sections = game_render::ini::parse(toy);
        let s = sections.iter().find(|s| s.name.eq_ignore_ascii_case("DropGate"))?;
        let lower = sections.iter().find(|s| s.name.eq_ignore_ascii_case("LowerGate"));
        let trigger_at = match lower.and_then(|l| l.numbers("TriggerPosition")).as_deref() {
            Some(&[x, y, z, ..]) => [x, y, z],
            _ => [0.0; 3],
        };
        let count = |k: &str| s.get(k).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
        Some(SinkWords {
            name: s.get("GateName").unwrap_or("").trim().trim_matches('"').to_string(),
            move_down: s.get("MoveGateDown").is_some_and(|v| v.trim().eq_ignore_ascii_case("true")),
            move_time: s.number("MoveTime").unwrap_or(5.0),
            soultree: count("NumberSoultreePieces"),
            collision: count("NumberCollisionPieces"),
            trigger_at,
        })
    }

    /// The scene prop names the sink lowers.
    pub fn soultree_names(&self) -> Vec<String> {
        game_sim::drop_gate::soultree_names(&self.name, self.soultree)
    }
}

/// P313c: the drop gate's sink bound to the race's `laps`, on a track whose `.toy` sets
/// `MoveGateDown`: its collision pieces found among the scene's collision `files` by name, each
/// where the props hold it (`at_index`, [`game_sim::world::Props::load_indexed`]). `None` on any
/// other track: there the original's trigger can fire but sinks nothing.
pub fn gate_sink<'a>(toy: &str, laps: &mut race::Laps, files: impl IntoIterator<Item = &'a str>, at_index: &[Option<usize>]) -> Option<game_sim::drop_gate::Sink> {
    let w = SinkWords::read(toy).filter(|w| w.move_down)?;
    let names = game_sim::drop_gate::collision_names(&w.name, w.soultree, w.collision);
    let pieces = game_sim::drop_gate::gathered(files, &names).into_iter().filter_map(|k| at_index.get(k).copied().flatten()).collect();
    let [x, y, z] = w.trigger_at;
    Some(game_sim::drop_gate::Sink::bind(laps, Vf::new(x, y, z, 1.0), true, F(w.move_time), pieces))
}

/// P313b: the drop gate as it falls at the drop. The race activity makes a gate object from the
/// track's `.toy` `[DropGate]` section: the scene prop named `GateName`, turned about its own
/// `RotationAxisLocal` by `RotationAngleDegrees`. The drop starts the turn. Each frame after it, the
/// turn moves the prop's rotation on by the whole angle times `dt / 0.15`, until the angle is reached.
/// The steps all turn about the same axis, so the prop at time `t` is turned by
/// `angle × min(t / 0.15, 1)` from where the scene stands it.
pub struct GateSwing {
    placed: Placement,
    axis: V3,
    /// The whole turn, in radians, and how far it has gone.
    angle: f32,
    turned: f32,
    swinging: bool,
    /// The gate has dropped since it last closed.
    down: bool,
    /// P313c: on a track whose gate sinks, the prop is one of the pieces the sink lowers, and these
    /// are the scene's other pieces, taken out to be drawn lowered with it.
    lowered: bool,
    sunk: Vec<Placement>,
}

impl GateSwing {
    /// The `[DropGate]` words of a track's `.toy`: the prop's name (its quotes taken off), its local
    /// axis and the turn in degrees. A missing axis is `1,0,0` and a missing angle 45, the reader's
    /// defaults. `None` with no section: the original then makes no gate object.
    pub fn words(toy: &str) -> Option<(String, V3, f32)> {
        let sections = game_render::ini::parse(toy);
        let s = sections.iter().find(|s| s.name.eq_ignore_ascii_case("DropGate"))?;
        let name = s.get("GateName").unwrap_or("").trim().trim_matches('"').to_string();
        let axis = match s.numbers("RotationAxisLocal").as_deref() {
            Some(&[x, y, z, ..]) => V3 { x, y, z },
            _ => V3 { x: 1.0, y: 0.0, z: 0.0 },
        };
        Some((name, axis, s.number("RotationAngleDegrees").unwrap_or(45.0)))
    }

    /// The gate's prop taken out of `scene`, so the static props drawn from it never show it standing
    /// after the drop. The prop is found by the `[DropGate]` name, ignoring case. `None`, with the
    /// scene left alone, when the toy has no section or the scene has no such prop.
    pub fn take(scene: &mut Scene, toy: &str) -> Option<GateSwing> {
        let (name, axis, degrees) = Self::words(toy)?;
        let at = scene.models.iter().position(|p| p.model.eq_ignore_ascii_case(&name))?;
        let placed = scene.models.remove(at);
        // P313c: the sink's pieces, every scene prop with one of their names.
        let names = SinkWords::read(toy).filter(|w| w.move_down).map(|w| w.soultree_names()).unwrap_or_default();
        let named = |m: &str| names.iter().any(|n| n.eq_ignore_ascii_case(m));
        let lowered = named(&placed.model);
        let (sunk, kept) = std::mem::take(&mut scene.models).into_iter().partition(|p| named(&p.model));
        scene.models = kept;
        // The turn's angle as the drop gives it (degrees × 0.01745329), and its axis made unit length
        // as the rotation's builder does.
        Some(GateSwing { placed, axis: axis.norm(), angle: degrees * 0.01745329, turned: 0.0, swinging: false, down: false, lowered, sunk })
    }

    /// The prop's model name.
    pub fn model(&self) -> &str {
        &self.placed.model
    }

    /// The gate dropped: its turn starts from where the scene stands it.
    pub fn drop(&mut self) {
        self.turned = 0.0;
        self.swinging = true;
        self.down = true;
    }

    /// The gate closed again: it is drawn standing. This is the port's own. The original's reset
    /// was not traced as far as the gate object.
    pub fn close(&mut self) {
        self.turned = 0.0;
        self.swinging = false;
        self.down = false;
    }

    /// One frame of the race's gate as the sim has it: closed, it stands. On the first frame it is
    /// open, it drops, and that frame already steps the turn, as the original's does: its start gate
    /// opens and its turn takes its first step on the same frame. Returns true on the frame the turn
    /// ends.
    pub fn follow(&mut self, closed: bool, dt: f32) -> bool {
        if closed {
            self.close();
            return false;
        }
        if !self.down {
            self.drop();
        }
        self.tick(dt)
    }

    /// One frame of the turn, as the gate object's update steps it. Returns true on the frame the
    /// turn reaches its angle.
    pub fn tick(&mut self, dt: f32) -> bool {
        if !self.swinging {
            return false;
        }
        let whole = self.angle;
        let mut step = whole * (dt / SWING_SECONDS);
        let end = (whole < 0.0 && self.turned + step <= whole) || (whole > 0.0 && whole <= self.turned + step) || whole == 0.0;
        if end {
            self.swinging = false;
            step = whole - self.turned;
        }
        self.turned += step;
        end
    }

    /// How far the gate has turned, in radians.
    pub fn turned(&self) -> f32 {
        self.turned
    }

    /// Where the prop is drawn: the turn about its local axis, then the scene's placement. The rows
    /// are the rotation builder's, with `c` and `s` the turn's cosine and sine and `t = 1 − c`.
    pub fn at(&self) -> M4 {
        let place = self.placed.transform();
        if self.turned == 0.0 {
            return place;
        }
        let (s, c) = self.turned.sin_cos();
        let (a, t) = (self.axis, 1.0 - c);
        let turn = M4([
            [t * a.x * a.x + c, t * a.x * a.y + s * a.z, t * a.x * a.z - s * a.y, 0.0],
            [t * a.y * a.x - s * a.z, t * a.y * a.y + c, t * a.y * a.z + s * a.x, 0.0],
            [t * a.z * a.x + s * a.y, t * a.z * a.y - s * a.x, t * a.z * a.z + c, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        turn.mul(place)
    }

    /// P313c: every piece the gate draws and where, with the sink's group `y` below the scene (the
    /// race's [`game_sim::drop_gate::Sink::y`], 0 with no sink): the gate turned as it swings
    /// ([`GateSwing::at`]), then the other pieces where the scene stands them. The group stands at
    /// `y` with no turn of its own, so a piece hung under it is only moved down by `y`.
    pub fn pieces(&self, y: f32) -> Vec<(&str, M4)> {
        let down = |mut m: M4| {
            m.0[3][1] += y;
            m
        };
        let gate = if self.lowered { down(self.at()) } else { self.at() };
        std::iter::once((self.placed.model.as_str(), gate)).chain(self.sunk.iter().map(|p| (p.model.as_str(), down(p.transform())))).collect()
    }

    /// [`GateSwing::pieces`] as actors.
    pub fn actors(&self, y: f32) -> Vec<Actor<'_>> {
        self.pieces(y).into_iter().map(|(model, at)| Actor { model, at, pose: None, lod_from: None, alpha: 1.0, casts: false }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A race of two whose cards are moved by hand, as the lap line would move them.
    fn two_cards() -> race::Laps {
        let card = |player, flags| race::Entry { player, card: race::RaceCard { lap: race::LapCard { flags, ..race::LapCard::default() }, ..race::RaceCard::default() }, stats: race::Tracker::default() };
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
        // S319: each card's statistics are its bike's tracker and its card's longest jump and air.
        laps.cards[1].stats = race::Tracker { laps_led: 2, times_in_lead: 1, passes: 3, times_passed: 4, wrecks: 5, preloads: 6, clutches: 7, ..race::Tracker::default() };
        laps.cards[1].card.jumps.best_length = F(81.5);
        laps.cards[1].card.jumps.best_air_time = F(2.25);
        laps.cards[0].stats.preloads = 1;
        let hand = hand_over(&laps, 2, "Me", &w.overs);
        assert_eq!(hand.cards[1].words[4..], [2.0, 1.0, 81.5, 2.25, 3.0, 4.0, 5.0, 6.0, 7.0].map(F));
        assert_eq!(hand.cards[0].words[11..], [F(1.0), F::ZERO]);
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

    /// sx01's `[DropGate]` as its `.toy` has it, and its `THQsxdropgate` placement.
    const SX01_DROP_GATE: &str = "[Other]\nGateName=x\n[DropGate]\nGateName=\"THQsxdropgate\"\nInitialPosition=101.493164,90.123070,458.479828\nLookVector=0.000000,0.500000,0.866025\nUpVector=0.000000,0.866025,-0.500000\nRotationAxisLocal=1.0,0.0,0.0\nRotationAngleDegrees=-60.0\nRotationRateInSeconds=3.0\nMoveGateDown=FALSE\n;DownPosition\n";

    fn sx01_scene() -> Scene {
        let mut scene = Scene::parse("");
        let gate = Placement {
            model: "THQsxdropgate".into(),
            pos: V3 { x: 101.493164, y: 90.12307, z: 458.47986 },
            look: V3 { x: 0.0, y: 0.5, z: 0.866025 },
            up: V3 { x: 0.0, y: 0.866025, z: -0.5 },
        };
        scene.models = vec![Placement { model: "stands".into(), ..gate.clone() }, gate];
        scene
    }

    #[test]
    fn the_drop_gate_words_and_their_defaults() {
        let (name, axis, degrees) = GateSwing::words(SX01_DROP_GATE).unwrap();
        assert_eq!((name.as_str(), (axis.x, axis.y, axis.z), degrees), ("THQsxdropgate", (1.0, 0.0, 0.0), -60.0));
        let (_, axis, degrees) = GateSwing::words("[dropgate]\nGateName=g\n").unwrap();
        assert_eq!(((axis.x, axis.y, axis.z), degrees), ((1.0, 0.0, 0.0), 45.0));
        assert!(GateSwing::words("[CameraFlash]\n").is_none());
    }

    #[test]
    fn the_gate_is_taken_out_of_the_scene_and_swings_flat_at_the_drop() {
        let mut scene = sx01_scene();
        let mut g = GateSwing::take(&mut scene, SX01_DROP_GATE).unwrap();
        assert_eq!(scene.models.len(), 1, "the static props no longer draw the gate");
        assert_eq!(scene.models[0].model, "stands");
        // Standing, it is drawn where the scene put it.
        let standing = g.at();
        assert_eq!(standing, scene.models[0].transform());
        let near = |got: [f32; 4], want: [f32; 3]| (0..3).all(|i| (got[i] - want[i]).abs() < 1e-5);
        // Closed, it stays standing.
        for _ in 0..10 {
            assert!(!g.follow(true, 1.0 / 30.0));
        }
        assert_eq!(g.at(), standing);
        // Open: −60° about its across row in 0.15 s, 4.5 ticks at 30 Hz, so the fifth ends it, the
        // last step cut short to land on the angle.
        let ends: Vec<bool> = (0..6).map(|_| g.follow(false, 1.0 / 30.0)).collect();
        assert_eq!(ends, [false, false, false, false, true, false]);
        assert_eq!(g.turned(), -60.0 * 0.01745329);
        // Its 30° lean back becomes flat: up along −z, look straight up, across unchanged, in place.
        let m = g.at();
        assert!(near(m.0[0], [1.0, 0.0, 0.0]) && near(m.0[1], [0.0, 0.0, -1.0]) && near(m.0[2], [0.0, 1.0, 0.0]), "{m:?}");
        assert_eq!(m.0[3], standing.0[3]);
        // Partway the up row is between the two.
        g.close();
        g.follow(false, 1.0 / 30.0);
        let up = g.at().0[1];
        assert!(up[2] < -0.5 && up[2] > -1.0 && up[1] > 0.0 && up[1] < 0.866, "{up:?}");
    }

    /// nat19's `[DropGate]` and `[LowerGate]` as its `.toy` has them: its trigger's key is `Position`.
    const NAT19_DROP_GATE: &str = "[DropGate]\nGateName=\"Nat19_Dropgate01\"\nPosition=3336.205078,173.670685,3135.009277\nLookVector=0.000000,0.500000,0.866025\nUpVector=-0.000000,0.866025,-0.500000\nRotationAxisLocal=1.0,0.0,0.0\nRotationAngleDegrees=-60.0\nRotationRateInSeconds=.1\nMoveGateDown=TRUE\nDownPosition=3336.205078,0,3135.009277\nMoveTime=0.25\nNumberSoultreePieces=2\nNumberCollisionPieces=1\n\n[LowerGate]\nPosition=2722.225830,234.692734,3035.601074\nTriggerLook=0.000000,1.000000,0.000000\nTriggerWidth=100.0\nTriggerHeight=100.0\n";

    #[test]
    fn the_sink_words_and_their_defaults() {
        let w = SinkWords::read(NAT19_DROP_GATE).unwrap();
        assert_eq!(w, SinkWords { name: "Nat19_Dropgate01".into(), move_down: true, move_time: 0.25, soultree: 2, collision: 1, trigger_at: [0.0; 3] });
        assert_eq!(w.soultree_names(), ["Nat19_Dropgate01", "Nat19_Dropgate02"]);
        // sx17's trigger is spelled as the reader reads it; nat16 writes `True`.
        let sx17 = SinkWords::read("[DropGate]\nGateName=\"THQsxdropgate01\"\nMoveGateDown=True\n[LowerGate]\nTriggerPosition=604.6,93,704.9\n").unwrap();
        assert!(sx17.move_down);
        assert_eq!(sx17.trigger_at, [604.6, 93.0, 704.9]);
        assert_eq!((sx17.move_time, sx17.soultree, sx17.collision), (5.0, 0, 0));
        let sx01 = SinkWords::read(SX01_DROP_GATE).unwrap();
        assert!(!sx01.move_down);
        assert!(SinkWords::read("[CameraFlash]\n").is_none());
    }

    #[test]
    fn a_sinking_gate_takes_its_other_pieces_and_draws_them_lowered() {
        let mut scene = sx01_scene();
        let piece = |m: &str, x: f32| Placement { model: m.into(), pos: V3 { x, y: 170.0, z: 3135.0 }, look: V3 { x: 0.0, y: 0.0, z: 1.0 }, up: V3 { x: 0.0, y: 1.0, z: 0.0 } };
        scene.models = vec![piece("stands", 0.0), piece("Nat19_Dropgate01", 1.0), piece("NAT19_DROPGATE02", 2.0), piece("Nat19_Dropgate03", 3.0)];
        let g = GateSwing::take(&mut scene, NAT19_DROP_GATE).unwrap();
        let left: Vec<&str> = scene.models.iter().map(|p| p.model.as_str()).collect();
        assert_eq!(left, ["stands", "Nat19_Dropgate03"], "piece 03 is the collision piece's number, not a scene piece");
        let up = g.pieces(0.0);
        assert_eq!(up.iter().map(|p| p.0).collect::<Vec<_>>(), ["Nat19_Dropgate01", "NAT19_DROPGATE02"]);
        let down = g.pieces(-13.5);
        for (u, d) in up.iter().zip(&down) {
            assert_eq!(d.1.0[3][1], u.1.0[3][1] - 13.5);
            assert_eq!((d.1.0[3][0], d.1.0[3][2], &d.1.0[..3]), (u.1.0[3][0], u.1.0[3][2], &u.1.0[..3]));
        }
        assert_eq!(g.actors(-13.5).len(), 2);
        // A gate that does not sink takes only its prop, and draws it where it swings.
        let mut scene = sx01_scene();
        let g = GateSwing::take(&mut scene, SX01_DROP_GATE).unwrap();
        assert_eq!(scene.models.len(), 1);
        assert_eq!(g.pieces(-50.0), vec![("THQsxdropgate", g.at())]);
    }

    #[test]
    fn the_sink_finds_its_collision_pieces_where_the_props_hold_them() {
        let mut laps = two_cards();
        let toy = "[DropGate]\nGateName=\"THQsxdropgate01\"\nMoveGateDown=TRUE\nMoveTime=0.25\nNumberSoultreePieces=2\nNumberCollisionPieces=2\n";
        let files = ["a.col", "missing.col", "THQsxdropgate03.col", "thqsxdropgate04.col", "THQsxdropgate02.col"];
        // `missing.col` did not load, so the props after it are one place down.
        let at_index = [Some(0), None, Some(1), Some(2), Some(3)];
        let sink = gate_sink(toy, &mut laps, files, &at_index).unwrap();
        assert_eq!(sink.pieces, [1, 2]);
        assert_eq!((sink.move_time, sink.move_down, sink.armed), (F(0.25), true, true));
        // No paths to find a spot on: the port measures the trigger 0.
        assert_eq!(sink.trigger, F::ZERO);
        assert!(gate_sink(SX01_DROP_GATE, &mut laps, files, &at_index).is_none());
        // A race with a sink hands it on.
        let prop = Prop { across: Vf::new(1.0, 0.0, 0.0, 0.0), look: Vf::new(0.0, 0.0, 1.0, 0.0), at: Vf::new(0.0, 0.0, 0.0, 1.0), half_width: F(30.0), height: F(1.0) };
        let mut s = lay_out(&prop, vec![F(1.54)], 1, 1).unwrap();
        assert!(s.race((s.spot, s.facing)).sink.is_none());
        s.sink = Some(sink.clone());
        assert_eq!(s.race((s.spot, s.facing)).sink.as_deref(), Some(&sink));
    }

    #[test]
    fn no_drop_gate_section_or_prop_leaves_the_scene_alone() {
        let mut scene = sx01_scene();
        assert!(GateSwing::take(&mut scene, "[CameraFlash]\n").is_none());
        assert!(GateSwing::take(&mut scene, "[DropGate]\nGateName=Nat15_Dropgate01\n").is_none());
        assert_eq!(scene.models.len(), 2);
    }
}
