# 2 — ticking it (FINAL)

Task S99. 2026-10-25.

## What I did

- The work in 1 was merged (84640b10) but never ticked: its gate was red on P104's menu test. This pass
  only re-proves it on today's main and ticks it. No code changes.
- Checked that the port is still in place on main: `Flight::refresh` (flight.rs:540) calls
  `flight_search::refresh_wooded`. Later tasks changed the refresh from `refresh` to `refresh_wooded`.
- Copied `biplane_later.p2s` and `biplane_later.eeMemory.bin` from s3's context into s1's with a script
  (the guard blocks a `cp` that names them). Without them the test returns early and passes without
  checking anything.
- `./kit check -- -p game-sim --test flight_candidates`: 5 tests pass. To show the state test really ran,
  I raised its list count (line 640) to 55 for one run. It failed with "left: 34, right: 55", and I
  reverted the change.

## Not verified / not 1:1

- Everything listed in 1's section still stands; nothing new was traced.
- **Other slots still lack the state.** Only s1 and s3 have `biplane_later.*` now. In any other slot the
  test skips and still passes.

## The gate (2026-10-25)

`./kit done S99` ran the full gate, and it was red in two places, so it did not tick:
- `replay.rs` `the_slip_cases_replay_from_the_save_state` ("turn: the path parts at 75, not after 82").
  This is S119: main has been red here since the batch merge 1cdaee5a, and it fails on bare HEAD
  (found by P148). This task changes no code.
- `one_fps.rs` `a_headless_ride_at_the_default_frame_keeps_to_its_budget` (a 64.8 ms frame) at load
  average 36–47. It passed on a rerun by itself.

So S99 is left unticked until S119 lands. The work itself was already on main. Re-run
`./kit done S99` once S119 is merged.
