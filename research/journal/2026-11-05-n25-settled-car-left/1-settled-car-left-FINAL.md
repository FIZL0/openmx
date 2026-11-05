# 1 — settled car left (FINAL)

Task N25. 2026-11-05.

## What I did

- Found why online_car's guest-buggy test parted under a busy gate. The ride itself never parted
  (0 desyncs, same state hash before tick 300 on both machines: the failing assert was the
  `car: buggy …` line only). That line was printed by the shell from `seat.garage`'s copy of the car,
  which `Garage::follow` refreshes from the **frame drawn**: `Online::show` copies the session's
  current sims, and those run up to the prediction window past the settled tick. On the host the
  guest's switch back to the bike (tick 330) arrives late under load, so the host drew one or more
  predicted ticks with the buggy still out, kept that copy, and printed it once the rollback took the
  car away: one predicted tick further along (481.1 vs 480.1 ft, ~36 ft/s = ~1.2 ft a tick). The
  `taken_at` it measures from had the same flaw (set on the first drawn frame with the car out).
  The car sim reads nothing outside the rollback state; the report read a prediction.
- Fix (`online.rs`): the session's game is wrapped in `Watched`, which after every tick it runs
  (first runs and re-runs alike, a re-run dropping the ticks from it on) notes each seat's car lane:
  kind, where it was first seen out (carried from the tick before), and the car's machine.
  `Online::cars_left(all)` walks those notes up to `net.settled()` and says each car that was out
  after one tick and not after the next, as `CarRide::report` says it, then drops all but the last
  tick looked at. The shell (`main.rs`) prints those after each `frame_seats`, and the rest (`all`)
  before the summary; the old print from the drawn frame is gone.
- opentennis: nothing like it to take (its rallies print no per-machine object reports).

## What the original does

Nothing to port: this is the port's own diagnostic line for online play.

## How it was proved

- `./kit check -- -p openmx --test online_car`: 140 tests pass (host truck, guest buggy, two cars at
  once, the lane/bytes oracle).
- The guest-buggy test three times in a row with 24 extra busy-loop processes on top of a machine
  already at load ~31 (32 cores): passed all three.

## Not verified / not 1:1

- I did not re-run the old code under the same load to watch it fail there and then pass with the
  fix: that needs a second full build in a scratch worktree. The cause comes from reading the code
  (the report came off the drawn, predicted frame) and from the size of the gap (one tick at the
  car's speed), not from a capture of the failure.
- `taken_at` is now where the car stood after the first tick it was out on the settled ticks, the same
  meaning `Garage::follow` gave it (after the first frame drawn with it out), not the spot the
  switch stood it before that tick's step. Both machines agree on it; it is a few feet off where
  offline's switch measures from.
- A car still out when the ride ends: the shell's end-of-run `car: …` line (seat 0's garage) still
  comes from the frame drawn last, which may be predicted. No test reads it online; left as it was.
- A car whose seat was left on a tick that never settled before the run ended is printed from the
  ticks run (`cars_left(true)`), which may still be a prediction.

## Next

Nothing.
