# 1 — s13 laps (FINAL)

Task S301. 2026-11-14.

## What I did

This is the second try. The first one (wip/S301, 11-13) could not be merged: P313 had already put a
race's laps into the Sim and the shell, on the same `Race::laps` field, and the two clashed (fixer.log,
11-14 01:11). This time I built on P313's `race::Laps` and kept only what it did not have.

- **The card join**: `RaceRules::join` (crates/game-sim/src/race.rs), taken from wip/S301. Two
  changes, because S302 added words to the card since then:
  - it now clears the jump words and the warning clocks as the original does;
  - it sets take-off and landing to `(0, 0, 0, 1)`.
  `TrackPaths::holds_any` is the join's on-track word: the spot search with no out pointer.
- `Laps::new` joins every card with `RaceRules::join` instead of P313's hand-built card. Then it
  puts back the card's `GROUNDED` bit (`0x400`), which the bike's look at its card sets in the
  original (the join's first call). The card's spot search starts from the line's spot, so a rider
  on no path stays on the line.
- **The whole race handler**: `RaceRules::tick` runs the waited-for look, each card's step and the
  close over the `Entry` list, and turns the notices' card indices into players. `Laps::tick` first
  tells each card where its rider is, then calls it. The body is P313's old `Laps::tick` loop, moved.
- **Lap length**: P313's `TrackPaths::load` already measures it with the track load's route call,
  and that is oracled (race.rs tests 34 and 35). The lap line (`near_line`) now reads
  `TrackPaths::lap_length` when it sets a card back on the line after a lap. Before, it read
  `Course::lap_length`. Only the ghost reset in `LapRules::line` still reads `Course::lap_length`.
- **Shell**: P313 already calls `race_over` for each finish and `race_end` once at the Over
  (`Stadium::card_finishes`, `race_start::hand_over`). What was left of V127: I removed
  `--race-win` and `Stadium::player_wins` (and the stadium's clock, which only they used). The
  camera_flashes test now finishes a real card through `card_finishes`.
- From opentennis: nothing. It has no race handler.

### The duplicated finish code (fixer.log, 11-13)

`RaceRules` and `LapRules` both have a version of the over check, the made-up finish times and the
time limit. I did not merge them. The running race uses one path: `RaceRules::card_on_lines`, then
`LapRules::close`, and both are oracled against the same handler. Merging the other copies would
move code that this task did not come for.

## What the original does

- Card join: `./kit fn 0x25f370`. The flags and words it sets are listed in research/oracle.txt
  under `score_card_join`. After wip/S301 I also found:
  - without manager flag `0x10000`, it clears the jump count and the three bests;
  - it always clears the last jump, the height (`+0x104`), the time since landing (`+0x1b0`) and
    the warnings (`+0x150..+0x164`);
  - after the listener calls, it sets take-off and landing (`+0x190`, `+0x1a0`) to vf0;
  - in kind 4 it sets `0x300` on the gate chain's cards (not ported).
- Whole tick: `./kit fn 0x262aa0`. For each card: owner vt+8 (the bike's look, `./kit fn 0x27dc80`),
  then the card step. After the cards comes the close.

## How it was proved

- `the_card_join_matches_the_score_managers_join` (crates/game-sim/tests/race.rs): an oracle over
  20,000 generated cards, tracks and managers. It compares every card word, including the jump and
  warning words, plus the next checkpoint, the free records, the paths and the listener calls in order.
- `the_race_handler_matches_the_score_managers_whole_tick`: an oracle over 10,000 races of 1–5
  cards. The close runs live, and the S302 helpers are switched on (jump watch, computer aim,
  warnings, with the manager's watch words in RAM). It compares the listener calls, list order,
  every card, the manager's flags, the countdown, the free records, the random generator and the
  paths. Its hooks now cover listener slots up to 0xa0, because the helpers call slots past 0x80.
- The rest of tests/race.rs, and the shell's camera_flashes test, through the gate.

## Not verified / not 1:1

- No PINE capture of a whole race was compared. The oracles prove the join and one tick, not a
  whole ridden race.
- Each tick, the bike's look at its card (owner vt+8) sets the card's look (`+0x120`), speed,
  height (`+0x104`) and `GROUNDED` bit from the bike. `Laps::tick` gives the card only the rider's
  position, so `look` stays the join's `(0, 0, 0, 1)` and `GROUNDED` stays as `HUMAN_CARD` set it.
  This was already true under P313: the jump watch sees a rider who never leaves the ground.
- Before the join, the card's spot is the line's spot. What the original's card holds there before
  the manager's reset was not read. It only matters for a rider standing on no path.
- These join branches are not ported: mode 1 (gate chain length), kind 4 (`0x300` on the chain's
  cards), allocating the checkpoint words (the port makes them in place), and the listener calls,
  which are recorded in the oracle but nothing acts on them.
- Ghost cards are not carried by the running race, so a ghost race's tick has no ghost.
- `race_over` gets the Hall of Fame's default tables and track -1. `race_end`'s preload and clutch
  counts are 0. All of that is P313's shell code and was not changed here.
- `--race-win` is gone. A look at the fireworks now needs a race ridden to its finish (`--race --laps 1`).
- The duplicated finish code in `RaceRules` and `LapRules` is still there (see above).

## Next

S333: feed the bike's look into `Laps::tick` (look, height, grounded) so the jump watch sees real jumps.
