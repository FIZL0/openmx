# 1 — s13 laps (FINAL)

Task S301. 2026-11-16.

## What I did

Started over on main. The first try (wip/S301, 11-14) built its own laps handler on `Race::laps`, and
P313 had landed the same thing first. The fix session could not join the two, so this run keeps P313's
`Laps` and adds only what P313 left out.

Already on main (P313, S293, S295), so not redone:
- One `RaceCard` per rider, run each tick after the drop (`Laps::tick` → `RaceRules::card_on_lines`).
  Its notices go to the HUD (`race_start::Watch`) and the results (`race_start::hand_over`).
- `race_over` at each `LineNotice::Finished` (`Stadium::card_finishes`, then the fireworks to
  `TrackFx::finish`), and `race_end` once at the Over.
- The lap length from the line spot's route to itself is `TrackPaths::load`. Its route call is already
  oracled against the hooked track load (tests/race.rs, the load test over generated race files) and
  against S196's sx01 image.

New here:
- **The card join** (`RaceRules::join`, crates/game-sim/src/race.rs), the score manager's reset on each
  card. Taken from wip/S301 and brought up to date with main's newer card words. The jump count and
  bests are cleared unless the manager keeps cards. The last jump, the height and speed words, the
  warnings' clocks and the time on the ground are always cleared. Take-off and landing go to vf0. The
  bike's word (`RaceCard::feed`) runs where the owner's look runs, in the middle of the join.
- `TrackPaths::holds_any`: the spot search with no out slot, which only answers "on a stretch?". Taken
  from wip/S301.
- `Laps::new` now joins each card through `RaceRules::join`, fed by a bike standing where the rider
  stands, in place of P313's hand-made card. The cards now start with the original's words:
  - to_line is 1e6 until the first step.
  - on_track comes from the ring test, not a fixed 1.
  - The flags are masked, then the grounded bit is set.
  - The human's card ends up as `HUMAN_CARD` exactly. The computer's is `COMPUTER_CARD` without
    `0x10_0000`, which only a landing sets.
- `Laps::new` takes the lap length off the track (`course.lap_length = track.lap_length`). The lap line
  reads the length the track load measured, as the original reads track `+0x54`, whatever the caller
  put in `Course`.
- `--race-win` and `Stadium::player_wins` / `Stadium::clock` / `Stadium::tick` are removed. Only real
  finishes from the lap line now fire the flashes and fireworks. The stadium unit test now finishes a
  first-place card through `card_finishes`.
- research/oracle.txt: `score_card_join`, with what it writes.

From opentennis: nothing. It has no lap or score-card code.

## What the original does

`./kit fn 0x0025f370` (the join, called from the manager reset `0x0025fad0` on every card).

First the flags: `&= 0x1a8801c`, `|= 0x800`. Then two conditional bits, for a card without `0x10`:
- `0x10000` (behind the line) in a ghost race, or when the track's `+0x34` has bit 4.
- `0x20000` (no best) when `+0x34` has bit 8.

The counts and clocks go to zero, and both distances to the line to 1e6. The best lap is kept only under
card `0x200000` or manager `0x10000`.

Then the owner and chain look at the card (vt+8, the bike's `0x0027dc80`), and last = now.

On a path race (mode 2) with a track:
- path_spot, holding stretches only.
- The kept and aim spots are set from it.
- on_track = path_find_spot(-1, -1, out null).
- Lap = path_distance to the line's `+0x94`, or the track's `+0x54` with no line.

Under manager `0x10000` clear: the odometer and the jump count and bests. Then always:
- the rest of the jump words, `+0x108` and the look / velocity / way-ahead quads (vf0)
- `+0x150..+0x168`, `+0x1b0`
- the way back to the track (0, 0, 1)
- take-off and landing (vf0)
- the records go back to the pool.
The listener calls come between these writes.

## How it was proved

- **Function oracle, 20,000 generated cases** (`the_card_join_matches_the_score_managers_join`,
  crates/game-sim/tests/race.rs, entry 51). Each case is a generated card (jump and warning words
  included), track, manager kind and flags, checkpoint list of 0–3, time limit and record pool. Checked
  against the ELF's join: every card word the port keeps, the next checkpoint, the free records, the
  paths and pools, and every listener call in order.
  - All 20,000 match.
  - Coverage, seen by raising the count assert for one run: 11,905 spots found, 15,994 with a line,
    11,187 bests kept, 11,331 behind the line.
- **sx01** (`laps_join_their_cards_with_the_live_races_flags`): `Laps::new` on sx01's loaded track and
  grid gives the live race's flags (C31's `HUMAN_CARD`), on_track 1, 1e6 to the line, and each card's lap
  equal to its path distance to the line. The test was seen to run (a temporary panic showed lap 3357.5
  on path 2).
- P313's sx01 race-to-the-Over test still passes with joined cards.
- The shell, headless (`openmx --headless --track sx01 --race --frames 900`): 4 cards, the drop at
  tick 97, places told for all four. No panic.

## Not verified / not 1:1

- **The bike's word during the join is the port's stand-in.** The oracle hooks the owner's vt+8 to do
  nothing (`feed` = `None`). `Laps::new` feeds a bike standing still where the rider stands: on the
  ground, zero velocity, zero look. Its only lasting effects are the position and the grounded bit. The
  real bike's look at a grid bike was not captured; C31's live flags agree with the result.
- **A fresh card's spot starts at the line's spot.** The original's fresh card holds no spot, so a rider
  on no stretch at the join keeps that null. A path distance from it would read a null path in the
  original. sx01's grid riders are all on a stretch, so this case never comes up there.
- **Card words the port does not keep are not joined**: `+0x3c`, `+0x44`, `+0xcc`, `+0xd8`, `+0xdc`,
  `+0xe0`, `+0x16c..+0x178`, `+0x1b8`, `+0x1c0`, the `+0xc8` list back to manager `+0x48`, and the
  `+0x110` velocity. The listener calls are not ported either (the oracle checks their order only), nor
  are the gate race's (mode 1) and the fast-lap attack's (kind 4) own work.
- **The race's reset itself (`0x0025fad0`) is not oracled whole.** Only the join it runs per card is.
  The order of the cards and the manager words it sets are P313's.
- **`card_on_lines` still reads `Course::lap_length`.** `Laps::new` now copies the track's into it, so a
  `Laps` race reads the track's. The per-card oracles of the lap line that build a `Course` by hand
  still pass their own length. Those tests are other tasks' and were left alone.
- **`race_over` in the shell gets no Hall of Fame and no seat.** `card_finishes` passes
  `HallOfFame::default()`, seat −1 and no ghost, as P313 left it. A player's win and Hall of Fame
  entries are answered but not kept anywhere (filed as P353).

## Next

Nothing for this task.
