# 1 — mxvatv cars sandrail red (FINAL)

Task K55. 2026-11-16.

## What I did

- Copied s2's `mods/machines/mxvatv_cars` into this slot's (git-ignored) `mods/` and reproduced the red:
  buggy, monster truck, baja and golf cart met the shell's body words at 250 steps; the sandrail never did.
- Cause: MD149b (merge 4f96989d3) rigs every shell car with its collision record (`MxvatvCarRide::with_record`
  reads the pack's `col`, `made` calls `Ride::rig`). The test's in-process car was never rigged. On the other
  four cars' 250 ticks over free09 the record's rays touch nothing, so rigged and unrigged agree; the sandrail's
  path brings its record into contact, and only the shell's car feels it.
- Fix is in the test, not the sim: `tests/mxvatv_cars.rs` now reads each car's `col` from the pack's mod.json
  and rigs the in-process Ride the same way the shell does (no rig when the pack lists none, as the shell).
- No bisect: the cause was found by reading the shell's car build beside the test's; rigging the test's car
  turns the test green with nothing else changed.
- Nothing taken from ../opentennis (no cars there).

## How it was proved

- `./kit check -- -p openmx --test mxvatv_cars each_mxvatv`: before, sandrail fails with the task's message;
  after, passes (all five meet the shell's body words at OUT − IN = 250 steps, the test's own assertion).

## Not verified / not 1:1

- The brief says it was also red with the MD39c-era pack. I did not have that pack to check; if that pack
  listed no `col`, neither side rigs and the red there would have another cause. Only the current s2 pack was tried.
- That the sandrail's record contact is right against the original is MD149b's proof, not this task's; this task
  only makes the test build the car the way the shell does.
- Which solid or ray the sandrail's record touches on this drive was not looked at (not needed for the fix).

## Next

Nothing.
