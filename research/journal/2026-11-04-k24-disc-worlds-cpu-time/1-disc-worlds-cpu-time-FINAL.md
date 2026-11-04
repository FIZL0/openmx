# 1 — disc worlds cpu time (FINAL)

Task K24. 2026-11-04.

## What I did

- `crates/openmx/tests/disc_worlds.rs`: every shell the test runs goes through `shell_cpu`, which spawns
  the shell, drains its stdout/stderr and reaps it with `libc::wait4`, reading that one child's rusage
  (user + system, every thread). The rusage is per child, so the other tests' shells running in the same
  binary at once are not counted (unlike `getrusage(RUSAGE_CHILDREN)`). All three limits — `COLD` (first
  ride, joins built), `WARM` (ride on a built world with `--seams`), `INSTALLED` (plain ride on the main
  checkout's installed world) — now read that CPU time. The wall clock is still printed beside it on
  stderr and held to nothing. `libc = "0.2"` is a dev-dependency of openmx (already in the lock via
  openmx-ios).
- `gate.toml`: disc_worlds out of `[alone]` again, as the task asked.
- K23 took the other route (the shell reports its own CPU on its `load:` line); here the test reads the
  child's CPU from outside, which needs no shell change and covers every ride the test makes.

Measured, two runs of `./kit check -- -p openmx --test disc_worlds` (test build), CPU / wall:

| ride | run 1 (load ~50) | run 2 (load 46→25) |
|---|---|---|
| world_mxu cold (`--seams`) | 1475 / 296 s | 1379 / 178 s |
| world_mxu_nat cold | 843 / 249 s | 801 / 150 s |
| world_mxu_free cold | 52 / 38 s | 52 / 25 s |
| world_mxu_free warm (built / installed) | 38, 32 s | 33, 29 s |
| installed world_mxu, plain | 43 / 24 s | 43 / 15 s |
| installed world_mxu_free, plain | 28 / 15 s | 27 / 11 s |

The wall clock moved 1.7x between the runs, the CPU 7% at most. Limits: COLD 3600 s, WARM 240 s,
INSTALLED 120 s of CPU (2.4x, 4.6x and 2.8x the largest seen).

## What the original does

Not applicable: test tooling only.

## How it was proved

Two passing runs of the binary in the pool at load 25-55, the CPU figures above logged per shell
(temporary scratch-file logging, removed before commit). The full gate passes in `./kit done`.

## Not verified / not 1:1

- CPU time is not fully load-invariant: SMT sharing and the stitcher's worker threads spinning while they
  wait both grow it on a busy machine. Seen 7% here between load 25 and 55; not measured on an idle machine
  or above load 56. The limits leave 2.4x and more for that.
- The limits sum every thread, so on a machine with many more cores the stitcher's spin overhead could grow
  the cold builds' CPU; only this 32-thread machine was measured.
- The old wall-clock limits also caught "the user waits too long" (P260's MX WORLD that "is not loading");
  a CPU limit only catches the shell doing more work, not it parallelising worse. Nothing holds the wall
  clock any more.
- K23 (not yet on main when this was done) edits the same `[alone]` `tests =` line of gate.toml, so the
  merge of the two conflicts on that line: the result should be one_fps and ecosystem only.

## Next

None.
