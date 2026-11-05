# 1 — modstore install twice (FINAL)

Task K45. 2026-11-05.

## What I did

- K42's merge gate log (`merge2/context/notes/merge-gate.log`) still had the failure:
  `modstore::tests::installs_run_a_few_at_a_time` panicked at modstore_tests.rs:763, `5 != 6`. Line 763
  is the **`music_packs(&mods).len()`** assert, not the `done` count. So the loop had counted six
  `Installed(Ok)` events, but only five mods were in `mods/music/`.
- The race: the test asks for each install twice in a row (`s.install(e); s.install(e);`). The dedup only
  works while the first one is queued or running. If the test thread is preempted between the two asks,
  the first install can finish in the meantime (they take ms), and the second ask queues a reinstall.
  The loop counted `Installed` events, not mods, so the reinstall's `Ok` counted as a seventh mod and the
  loop stopped early. At that moment either the last mod was still running, or a reinstall had its old copy
  moved aside to `.store-old-<id>` (unpack's swap). Either way `music_packs` saw five. It hangs on thread
  scheduling under load, not on CPU time. With 48 busy loops plus 8 copies (load ~130), an install still
  took ≤55 ms.
- Fix (test only, `crates/openmx/src/modstore_tests.rs`): the fake store gets `Routes::held`, so its GETs
  wait unanswered while it is set. The test holds the store while it queues all the installs, so none can
  finish between the two asks. Then it checks that exactly `PARALLEL` are running and releases the store.
  It now collects every `Installed` (key, result) and asserts it is exactly one `Ok` per mod: the dedup
  is now checked deterministically, where before it was only hoped for. The loop also stops on the event
  count, not on `busy()`, because a job clears `busy` before it pushes its event. The 30 s limit is
  unchanged and only guards against a hang.
- opentennis: nothing to take, since it has no mod store.

## How it was proved

- A/B with a 30 ms sleep put between the two asks (a stand-in for preemption). The old test failed 5/5
  (`11 != 6`: the reinstalls were counted). The fixed test passed 5/5 with the same sleep.
- The fixed test passed in 8 concurrent copies of `openmx-<hash> modstore` (32 tests each) beside 48
  busy loops, at load ~150.
- The original failure itself (a real preemption in a full gate) could not be reproduced. Six full-binary
  copies and 8×modstore beside busy loops all passed on the old code. The sleep A/B stands in for it.

## Not verified / not 1:1

- The gate's exact interleaving was not reproduced; the cause is inferred from the assert line (763 =
  `music_packs`) plus the forced-delay A/B. Reason: the merge gate's check.log was overwritten, and the
  natural preemption did not happen in ~14 loaded runs.
- No original-game behaviour is involved (test harness only).

## Next

- Nothing.
