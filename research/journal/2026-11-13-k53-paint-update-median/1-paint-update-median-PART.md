# 1 — paint update median (PART)

Task K53. 2026-11-13.

## What I did

- `world_ultra_load`'s sweep test judged the single slowest paint update's thread CPU time against
  8 ms; under the other slots' gates one update read 8.89 ms (MD120, P315d) and the test passed alone.
  Thread CPU time leaves out waiting for a core (K23) but still counts what contention costs on the
  thread itself: page faults, caches another process emptied, a slower clock.
- `PaintStream` now keeps the CPU time of every update that wrote pages (`writing_cpu`) and the shell
  prints `world paint: N updates wrote pages, their cpu X ms at the median and Y ms at the 90th
  percentile`. The test judges the median and the 90th percentile against the same 8 ms (no bigger
  number), and wants at least 10 writing updates. The slowest is still printed but not judged: one
  contended frame moves it, a write that grew slow every frame moves both of the judged ones.
- Updates that wrote nothing are left out of the spread, so a ride's many idle updates cannot pull the
  median down and hide a slow write.
- Nothing from opentennis: it has no streamed world.

## What the original does

Nothing here: kit/test robustness only, no game behaviour.

## How it was proved

`./kit check -- --test world_ultra_load` at load ~36 (other slots running): 75 writing updates, 0.99
ms median, 1.00 ms 90th percentile, slowest 2.01 ms of cpu; both tests pass.

## Not verified / not 1:1

- Not reproduced at the load that gave the 8.89 ms (the gates of all slots at once); the claim that the
  median/90th percentile stay far under 8 ms there rests on the 0.99/1.00 ms seen at load 36 and on
  the failure being a lone update.

## Next

-
