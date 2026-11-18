# 1 — race riders over c31 (FINAL)

Task S377. 2026-11-17.

## What I did

- `crates/game-sim/tests/ai/race_sx01.rs`: the Sim's race laps (S332's lap cards, ranking, clocks,
  finish) run over C31's whole live sx01 race, 12,000 capture rows, three AI riders and the idle human.
  - **Free run.** Seeded once from the row before the race clocks start, then never touched again.
  - **Resync run.** Restarts cards and manager from the next whole row wherever the capture lost a tick.
  - On every held tick, both runs also check the AI lane's view of the laps:
    - `Racing::of` against `racing_of`, and both against the card and manager words;
    - `Pilot::read_laps` for players 1–3 (card flags, progress, live, race phase, path net).
- Second test, `s377_the_sims_ai_net_is_c31s`: `Sim::load_ai_net` on sx01AI.tdf off the disc, stood on
  sx01's ground, against the AI net in C31's start image. It checks the head words and every path's flags,
  length, word, links and nodes.
- Fix in `Sim::load_ai_net` (tick.rs): the net was built with `TrackPaths::new`, so its lap length
  was 0. The image's AI net has it measured (0x45067666). It now builds with `TrackPaths::load`, like
  S331b's `race_net`. If a file has no line to measure from, it falls back to the old build (lap 0).
- `path_nets.rs`: `sx01_files` made `pub(crate)` so the new test can read the disc files.

## What the original does

- **A card's `last` equals `now` at the end of each step.** A capture row therefore holds only where the
  rider is after the tick. A tick that has no row leaves no trace of where the rider was during it.
- C31's rows are read at no fixed point in the frame:
  - about 1 in 13 repeats the tick before (read late);
  - some rows have one card already a tick ahead (read inside the next handler);
  - 6 frames have no row at all.
- **The card's owner drops the jump watch while its rider is down.** It clears card bit 0x80000 (and
  JUMP_TO_JUDGE goes with it), freezes the card's speed word, and sets the bit again at the respawn.
  Seen on C31 rider 1, frames 5915–6021: speed held at 31.07, then a teleport to speed 0 at 6021.

## How it was proved

- Capture: `context/capture/c31/sx01-ai4-by-rider.csv` with its start image `c31-sx01-ai4.ee.bin`.
- Tick matching goes by the race clock (+0xb4), which every racing card steps by the tick's dt.
  - A card's place for a tick comes from the first row in which that card has that tick's clock and
    is settled (now == last).
  - A row is compared only when it is "whole": every card still racing has the tick's clock and is
    settled.
- **Resync run.** 9872 held ticks to frame 12006, and 919 ticks where some card's place is missing.
  - Every card word matches C31 bit for bit on every held tick, for all four cards: flags, distances,
    progress, odometer, laps, place, clocks, gap, spots, aim, look-ahead, jumps, warnings.
  - The list order and the manager flags match too.
  - 29,616 `read_laps` checks and 39,488 hand-off checks, all equal.
  - The race finishes: cards go out, places change 60 times, 4 laps are counted in the stretches held.
- **Free run.** 10,785 held ticks.
  - The list order, places, laps, progress, lap and race clocks, gap, card flags and manager flags all
    hold on every held tick.
  - 32,355 `read_laps` checks, all equal.
  - Only the distance words drift, listed below.
- AI net: head words (word_34, reach 125/75, pick 3, lap length) and all 38 paths equal the image's.

## Not verified / not 1:1

- **The riders' bodies are not ridden.**
  - C31 holds no bike words, so the AI brains' steering and throttle are not driven over the race.
  - Each card is fed the captured position, look, velocity and rise.
  - C33/race4 already shows the Sim's AI bodies parting from the original's after ~28 ticks on free09.
- **Lost ticks.** Where the capture has no row for a tick (919 ticks), that tick's position is unknown,
  because `last` equals `now`.
  - The free run feeds the next row's position for the lost tick. From there its odometer, on_path,
    to_line_kept and the path searches' spots (spot, kept_spot, last, to_line, aim, ahead) part by a
    few ulps or a different search spot. Jumps part too.
  - The free run's assert exempts exactly these words, for this reason. The resync run holds them
    all bit for bit.
- **Owner bits are copied from the capture, not made by the Sim.** These are the listener bits 0xe0
  (as in S13c) and the jump watch 0x80000.
  - The Sim's card owner keeps the jump watch on through a wreck.
  - The original drops it while the rider is down and sets it again at the respawn (rider 1,
    5915–6021).
  - The test marks a card wrecked while the captured card lacks the bit.
  - Follow-up: S429.
- **The human's card** (rider 0, idling at the gate) is checked as a card. No pilot reads it.
- **rng draws at the finish** are not compared. C31's rng column is used only as the seed.
- **The AI net's search scratch** (dist, best, free pools) and its line spots are not compared with the
  image.
- **Only sx01** is covered: one track, one race kind (RACE), one AI count (3).
- **Nothing came from ../opentennis**, which has no AI riders.
- **The gate (`./kit check --all`) had one red test, not from S377:** atv_quad
  `mxu::the_quad_rigged_from_its_files_is_md123as`.
  - The record's inverse inertia is 7.9e-5 relative off, against the test's 1e-5.
  - It is red alone on base e63b3835a, and origin/main has no fix.
  - Nothing in it calls `load_ai_net`, the only Sim code S377 changed.
  - Filed as K75.

## Next

Nothing for S377.
