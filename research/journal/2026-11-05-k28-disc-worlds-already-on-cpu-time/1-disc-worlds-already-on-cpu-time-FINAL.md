# 1 — disc worlds already on cpu time (FINAL)

Task K28. 2026-11-05.

## What I did

- K28 asked for world_mxu_nat_is_the_22_nationals and
  world_mxu_has_every_freeride_and_national_with_the_freerides_together to stop failing on a busy gate:
  `shell_within`'s 480 s wall-clock limit was exceeded at 518 s and 582 s (load ~48, P226's gate, 11-04).
  K28 duplicates the second half of K24's brief, and K24 (ff99c062, on this branch) already fixed it.
  `shell_within` calls `shell_cpu`, which reaps the shell with `wait4` and holds that one child's
  rusage (user+sys, all threads) to `COLD` = 900 s of CPU. K24 also took disc_worlds out of gate.toml's
  `[alone]`. Both tests reach `shell_within` through `first_ride`, so neither one reads the wall clock.
  P299 (start files) has since cut the cold builds as well.
- No code change. I re-ran both tests under load to confirm the fix holds:
  `./kit check -- -p openmx --test disc_worlds -- world_mxu_nat_is_the_22_nationals world_mxu_has_every_freeride`
  → `TESTS PASSED: 2 tests`. The load averages were 177 / 81 / 65 at the start and 75 / 152 / 108 at the
  end, with about 5 min 20 s of wall clock for the two together. That is far above the load ~48 where the
  wall-clock limit used to fail them.

## What the original does

Nothing of the original is involved: this is test-gate tooling.

## How it was proved

I ran both tests once through `./kit check` at load 75-177 (above). The run took about 5 minutes, so the
worlds really were built from the disc; a skip without the disc returns at once.

## Not verified / not 1:1

- The CPU seconds each ride spent in this run are not recorded. On a pass `./kit check` prints only one
  line and keeps no log, so the `eprintln!` figures were not seen. K24's journal has the figures it
  measured.
- One run, not a series. That CPU time does not grow with load is K24's measurement (7% across load
  25-55), not mine.

## Next

Nothing.
