# 1 — merge2 gate swap (FINAL)

Task K66. 2026-11-17.

## What I did

- Looked for what `-p game-sim --test roof_landing` could race. It races nothing:
  - its only outside input is the ISO, read through `Disc::open_configured` (fixed file);
  - game-sim reads no clock (`Instant`/`SystemTime`), spawns no threads and has no shared mutable
    statics (only `OnceLock` tables and per-thread caches);
  - so one binary gives one answer, loaded or not.
- The failure was not a timeout either: `off_wrecks_a_roof_landing_on_the_head` wrecked with reason 11
  (Tail) instead of 6 (Scenery), and `a_flip_onto_the_roof_wrecks_the_same_on_and_off` failed beside it.
  That is a different program, not a slow one.
- Found the cause in the runner's log (`context/notes/run.log`, 11-17):
  - 12:01:25 MD168b's turn adopted a gate an earlier run left going in the `merge2` worktree.
  - 12:02:06 a new merge-ahead (F49 … R38, MD72) started in the same `merge2` worktree: `clean`,
    `checkout`, merges. 12:06:58 a fix session resolved MD72's conflict in `rider.rs` /
    `rider_contact.rs` there.
  - So MD168b's gate built and ran binaries from a half-done merge, and its failures (roof_landing,
    ride's s154, mxvatv_bike_ride's heap) were read as MD168b's. Both gates' `.failed` files hold the
    same output, and both name `check-merge2.log`.
- The runner fix is already in agent-runner `027ce7f` (0.37.1, pushed 11-17 13:33): a merge built
  ahead waits while a gate still runs in `merge2`. Its message names K64, K65, K66.
- No change to the test: there is nothing in it to make robust.

## What the original does

Not touched: a test-gate task.

## How it was proved

- `./kit check -- -p game-sim --test roof_landing` on this branch (main at 180e17ab1): 4 tests pass.
- The same on 0808b90dd, the batch rebuilt from the 12:02 merge-ahead (MD72 with its conflict fix,
  F49, P347, R38 …), in a scratch worktree: 4 tests pass. So no merged tree breaks the test; only the
  half-merged checkout did.

## Not verified / not 1:1

- The exact tree the bad binaries came from: `merge2` was reset since and the 12:02 check log
  overwritten. The swap is read from the runner's log times and the identical failure files, not
  rebuilt.
- The installed runner is still 0.37.0 (without `027ce7f`). I did not reinstall it: the live runner
  runs from that install, and swapping it under a running process is the owner's call. Until it is
  installed and the runner restarted, an adopted `merge2` gate can be swapped the same way again.

## Next

- Install agent-runner 0.37.1 (`uv tool install --reinstall ~/repos/agent-runner`) and restart the runner.
