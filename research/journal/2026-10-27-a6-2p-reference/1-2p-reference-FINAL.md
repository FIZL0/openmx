# 1 — 2p reference (FINAL)

Task A6. 2026-10-27.

## What I did

- **One place names the 2p ride**: `game_sim::replay::two_p` (end of `crates/game-sim/src/replay.rs`).
  Context-relative names for the tape (`recordings/split/2p.p2m2`), its PCSX2 state, the capture of
  record (`capture/2p/2p.csv`), the tape's start image (`states/c10/2p-start.ram`), the per-event RAM
  images and saves (`ram_at(frame)`, `p2s_at(frame)`, C10's `states/c10/` subfolder), the event index
  (`research/c10/events.json`), `FIRST_FRAME` 127882, `ROWS` 12917 and the measured `POLLED_FIELD_LAG`.
  Its doc says it is the recording of record and which subjects must read it.
- **Readers**, all in the same module or beside it:
  - `two_p::read(BufRead, keep)`: streams the ~215 MB capture a line at a time and keeps only the columns
    asked for (a whole `Capture::parse` would hold ~17.7 M strings). `two_p::no_bodies` is the keep-all
    but the joints and ragdoll bodies.
  - `two_p::lane(cap, Rider)`: either rider's lane as a one-rider `Capture` — the lane's prefix dropped,
    the other lane left out, the shared columns kept, and `pad_a`/`pad_b`/`stamp_a`/`stamp_b` naming the
    rider's own pad — so `Capture::float("bike_xform.m30", …)`, `PadTape::from_capture` and every older
    one-rider reader take it unchanged (the test feeds P1's lane to the acceptance tests' world finder).
  - `two_p::tape(cap)`: both pads off the capture, each from its older-stamp half, with the engine's own
    `dt` (`PadTape::from_capture`'s rules, for two pads). `two_p::tape_of(cap, Rider)`: one rider's pad on
    port 0.
  - `P2m2::tape_per_field(dt)`: the `.p2m2` read a field a tick (the 2p state ticks at 60 Hz; `P2m2::tape`
    takes one field in two, which is the one-player ride's rate). `PadTape::alone(pad)`: one pad of a
    two-pad tape moved to port 0.
- **The 2p case beside `the_human_recording_in_context_drives_the_port`**:
  `tests/replay.rs::the_2p_recording_drives_both_riders_from_its_state`. Checks the tape (from a state,
  12,917 fields, pressures agree on both pads every field, both ports written), the capture's tape against
  the `.p2m2`'s fields (offset measured, both riders), each lane's guards on every row and its tape = that
  pad alone; then seats both riders from `2p-start.ram` (SaveState chains: list entry 3 = P1, entry 2 = P2,
  as `watch/2p.toml`; `seed_ride` for each, the state's own wheel mounts, the disc's `race500p.phy`, the
  bike's body rays), plugs pad 1, and rides the whole tape over free09 with `poll_all` + `step_all`,
  asserting on every tick that each player's pad holds what its tape frame held, the run covers all 12,916
  ticks, and both bikes moved off their start (1559 ft and 694 ft). 6.9 s.
- **split.rs**: `the_2p_reference_has_both_riders_lanes_as_these_rides_do` — the 2p capture through the
  shared reader, both lanes carrying S93's guards (type 2, player index, local human, bike type 4, bike's
  player) on all 12,917 contiguous rows, each lane's tape live. The module doc points at the reference.
- **two_p.rs** (C10's) reads `two_p::CAPTURE` instead of its own path. Its own reader is left as it was.
- **plan/HUMAN.md**: a "human recording of record" section, and the ask for a second 2p recording with
  rider-vs-rider contacts, a truck ride and prop hits, which C10 found missing. **docs/PROVING.md**: "The
  human reference: the 2p ride" under the replay harness — what to read it with and that new
  wreck/collision/ragdoll/platform/threshold/animation tests read it.
- Took nothing from opentennis: it has no two-pad human-reference fixture to copy.

## What the original does

- The free09 split-screen state ticks at 60 Hz (C10). Measured here: the capture's row `r` polled-pad half
  (the older stamp's) is the `.p2m2`'s written field `r − 2` on 25,808 of 25,832 pads (P1 12,906, P2
  12,902 of 12,916 each); offsets −1 and −3 agree on ~9,200–9,900, 0 on ~7,300–8,300.
- `recorded_on` puts P1's lane on `tracks\free09\free09.scn` with 1 row of 12,917 more than 2 ft under its
  ground (the other freeride areas bury 2,429..6,321 rows), so the split scene's ground is free09's.
- `race500p.phy` comes out of `Bike\500_01.res`.

## How it was proved

- `./kit check -- -p game-sim --test split --test replay -- the_2p`: 2 tests passed; `--test split --test
  two_p`: 11 passed.
- Not skipped: the replay test's `--nocapture` output prints the offset table, the area choice and the
  port/original positions; for split's I raised the pad-change bound to 100,000,000 once and it failed
  ("P1's pad changes on 196 ticks"), then put it back to 100.
- `./kit done`'s gate: 2453 tests passed. Landed with `--skip-audit`: the audit's only leaks are
  plan/DONE.md:445 (S39's biplane_tire_* line, 2026-10-20), on main already and not this task's files.

## Not verified / not 1:1

- **The port's 2p ride is not held to the original's.** The test asserts the input half (each pad reaches
  its player every tick, the run is whole, both bikes ride) and prints positions. The port follows the
  original within ~1.5 ft to tick 300 and parts afterwards (tick 3000: hundreds of ft): which clause parts
  it is the wreck/collision tasks' work (S155..S160), not this task's.
- **Which field the tick acted on is not pinned.** `POLLED_FIELD_LAG` is where the capture's polled half
  sits against the tape; the drive uses the capture's pads at `PadTape::from_capture`'s numbering (row `k`'s
  pad as tick `k`'s input). S67 found recording2's tick reads `k − 2`; whether the 2p ride does the same
  is not measured.
- **The world is free09's freeride scene** as `recorded_on` picks it, not the split scene's own
  (`Tracks\Free09_SS\`) files, and has no path cars in it; the ride has no car contact (C10), so nothing
  here depends on them.
- **The seeded state is the ride only**: `seed_ride` (chassis, wheels, balance, engine, air, drivetrain,
  held) for each rider. The rider body, animation machine, trick and wreck state, and the input layer's
  timers start as the port builds them, not from the image. Pad 1 is plugged by hand (`Sim::with_rider`
  plugs pad 0 alone).
- **The bike model** for each rider is not read (C10 left 500_01/500_02 unmapped); the state's own wheel
  mounts are used, so the model only names the world-finder's fallback.
- **`dt`** comes from the capture; `tape_per_field` takes a `dt` argument and the test reads its fields at a
  flat 1/60, which is not the engine's alternating 0.0166667/0.0167 — it is only compared on buttons and
  sticks.
- `cargo doc` was not run, so the intra-doc links in the new docs are unchecked.

## Next

- S155/S156/S160/V90: start from `the_2p_recording_drives_both_riders_from_its_state`; pin the tick's
  field (`k − 1` vs `k − 2`) against P1's first 300 ticks first.
