# 1 — native tolerance suite (FINAL)

Task F18. 2026-11-02. Built on main at 408b131e.

## What I did

- **`crates/game-sim/tests/replay.rs`, "F18: the native build's tolerance suite"**. In the exact build each of
  these tests prints a line and returns, because the exact tests already hold the same rides to the bit.
  - `native_tolerance_landings` runs `each_captured_landing_rides_like_the_original` on the 9 S67/S115
    landings, held to `S67_BOUND`.
    - That test's wheel-mount precondition now allows `NATIVE_MOUNT_ULPS` (2) in the native build only.
    - The mounts come out 1 ulp off natively. Before this change that precondition alone failed the test.
  - `native_tolerance_wrecks` calls the outcome tests as they are. Each already compares the wreck and its
    tick (±1), not bits:
    - crooked landings and backwards bails (S150);
    - roof (P147);
    - walls/nose (S138);
    - tails (S144);
    - the 2P ride's wrecks off C10's states (S175).
  - `native_tolerance_save_states` covers the six S58 cases and the four S106 cases.
    - Each is set up the way its exact test sets it up. `native_path` takes the levers, engine state and saved
      controls only for the cases that `replay_a_save_state_case` seeds that way.
    - Each is ridden through. The path is `rider.ride.origin` against the capture's `ch_xform` row 3 on every
      compared tick, skipping the `late` ticks.
    - Each case is held to its `NATIVE_PATHS` bound up to its tick. No case may wreck.
- **The gate**: `ps2kit/cargo_gate.py` has a new gate.toml section, `[[variant]]`, with `features`,
  `target_dir`, `filter` and `tests`.
  - Each variant's binaries are built with its features into its own target directory.
  - They run in the same pool as the rest, with the name filter applied.
  - Their rerun line carries all of that: `-p game-sim --features ps2-float/native --target-dir target/native
    --test replay native_tolerance`.
  - gate.toml has the native entry.
  - `ps2kit/checks.py` `--rerun` now runs each quoted binary as a cargo run of its own. Otherwise a variant's
    features and filter would reach the other binaries the runner's retest lists beside it.
  - `tests/test_gate.py` has four new checks:
    - a variant runs filtered with its features;
    - it builds into its own directory;
    - a failing variant test fails the gate with that rerun line;
    - the plain build of the same binary is unaffected.
- **Binaries**: the native run is the one binary. The brief's three groups all live in replay.rs, so I did not
  build or run the other 21 game-sim binaries natively. Their native failures are 1–500-ulp bit asserts with no
  outcome behind them. Adding one later is a line in `tests` plus `native_tolerance_*` tests in it.

## Measured (native, this worktree's captures, 408b131e)

- **Landings.** All 9 are inside S67_BOUND. The worst is 3452, at 0.12°, cofg 0.016, vel 0.117, comp 0.007,
  drawn 0.016 and aim 0.002. This matches the artifact.
- **Wrecks.** In this worktree, S138 (8), S144 (12) and S175 (39 of 41 events, 0 parted) agree. S150's
  r50/r60/r70 and P147's roof captures are not in s1's `context/`, so those tests skip here. They run wherever
  the captures are.
- **Save-state paths**, as worst ft (tick) up to the compared-through tick, then the bound:

| case | worst | through | bound | past 1 ft | at the end |
|---|---|---|---|---|---|
| turn | 0.53 (151) | end | 0.6 | never | 0.46 |
| launch | 0.82 (240) | end | 0.9 | never | 0.82 |
| brake | 1.04 (183) | end | 1.1 | 170 | 1.02 |
| roll | 0.18 (239) | end | 0.2 | never | 0.18 |
| nose | 0.22 (199) | end | 0.25 | never | 0.22 |
| side | 0.15 (199) | end | 0.2 | never | 0.15 |
| traverse | 0.52 (123) | 125 | 0.6 | 130 | 13.0 |
| roll2 | 0.14 (100) | 100 | 0.2 | 108 | 8.5 |
| steep | 0.33 (111) | 150 | 0.4 | 186 | 5.9 |
| steep2 | 0.75 (160) | 180 | 0.85 | 203 | 3.1 |

- turn, launch, brake and roll match the brief's 0.2/0.6/0.9/1.1. The brief did not measure nose, side, steep
  and steep2, so their bounds are new.
- steep and steep2 (S117's held climbs) take their own line late in the ride, like roll2 and traverse. All four
  are held on their path only up to before the split, and to their outcome after it.
- The brief gave traverse and roll2 as 24 and 11 ft at their ends. I measured 13 and 8.5 ft.

## How it was proved

- Native: `./kit check --rerun '-p game-sim --features ps2-float/native --target-dir target/native --test replay
  native_tolerance'` passes, 3 tests. This is exactly the line a failure would print.
- Perturbed: `phy::G` 32.2 → 32.25 (+0.16%) reds `native_tolerance_save_states`:
  - turn 0.71, brake 1.17, steep 0.67, steep2 21.2, roll 4.4 and roll2 0.25 ft, each past its bound.
  - The landings stay inside S67_BOUND, and the wrecks stay within their ±1 tick.

  I reverted the change.
- Exact: `each_captured_landing_rides_like_the_original` and the three `native_tolerance_*` tests pass in the exact
  build. That was a two-binary `--rerun`, which also exercised the new one-run-each path.
- `python3 tests/test_gate.py`: 25 passed, 0 failed.
- The full gate runs in `agent-runner done`.

## Not verified / not 1:1

- **The suite is coarse where the brief's groups are.** A 0.16% gravity change does not move a landing past
  S67_BOUND or a wreck by more than its ±1 tick. Only the path bounds caught it. The bounds are as tight as the
  measurements allow, with 7–30% headroom.
- **The bounds are native's numbers at 408b131e.** A real port fix that moves the native ride (most will) may
  push a path past its bound. That is the intent: the change shows as a number. Whoever lands it re-measures
  and updates `NATIVE_PATHS` and its comment.
- **The other 21 game-sim binaries are not in the native run**, and neither is openmx's suite (see above).
- **S150 r50/r60/r70 and P147 roof skip in this worktree** (no captures in s1). The artifact had them agreeing.
  I did not re-run them here.
- **The gate's build time grows** by a second native build of game-sim's replay test binary in `target/native`.
  It is incremental after the first build. It also counts toward `target/`'s 80 GB cap.
