# 1 — pair log resize stub (FINAL)

Task K64. 2026-11-17.

## What I did

- The red was not load. `the_riding_parts_listeners_are_the_originals` fails alone too, with the same seven
  words as in both MD168b gate logs (context/notes/gates/41f5bd11…failed, 6f94ef37…failed). The runner's
  "passed alone" was wrong for this binary.
- Cause: F54's test hooks the heap's resize (log + answer a fixed block), since its pair logs are made up.
  MD140 (342d23131) then made the port side of the pair log call `heap_in_ram::resize_checked`, which runs
  the original (here: the stub, no RAM change) and then Rust's real resize (writes the heap). They can only
  differ, whenever a log grows.
- Fix in `crates/game-sim/tests/mxvatv_bike_ride.rs` (`pair_log_heard_in_ram`): when the resize address is
  hooked, both sides get the stub, as before MD140. Unhooked (the replays through the part world), Rust's
  resize is still checked word for word against the original's.
- Nothing from `../opentennis`: test glue specific to this game.

## What the original does

Unchanged by this task (MD140 has the resize).

## How it was proved

- `./kit check -- -p game-sim --test mxvatv_bike_ride the_riding_parts_listeners_are_the_originals` red alone
  before the fix (same seven words as the gate logs); with the fix the whole binary passes, 147 tests.

## Not verified / not 1:1

- Why the runner's alone rerun reported a pass: not looked into (runner, not this task). The test is red
  alone on this branch every time.
- The F54 listener test no longer exercises Rust's resize: its pair logs are fake blocks, which the
  original's resize cannot take either. Rust's resize stays checked where real logs grow (atv_quad sim_frame asserts two grows, each through `resize_checked`).

## Next

- None.
