# 1 — world ultra load cpu time (FINAL)

Task K23. 2026-11-04.

## What I did

world_ultra_load's two time budgets went red under load even though both binaries were already in
gate.toml `[alone]`. The alone rerun only holds the other *gates*. The other slots' builds keep the
machine at load 50+, and on a 32-thread machine that slows the wall clock directly. Both checks now read
CPU time, which does not go up when the shell has to wait for a core:

- `paint_stream.rs` `cpu_time(thread)` (the lib mounts paint_stream too, so it lives there): the process's CPU time from `/proc/self/stat` (utime+stime, 100 ticks
  a second), or the calling thread's in ns from `/proc/thread-self/schedstat`. It reads /proc because
  the shell is `#![forbid(unsafe_code)]`, which rules out `clock_gettime`, and adding a libc dependency
  to the shell was not worth it. Not on Linux it returns None.
- The `load:` line now ends with `, cpu 49.91 s (11.10 s on this thread)`: every thread's CPU time and
  the shell thread's own, up to the first frame. The wall clock and memory figures before it are the
  same as before.
- `PaintStream` keeps `slowest_cpu`: the most CPU time the shell's thread spent in one `update`. Where
  that figure cannot be read it falls back to the wall clock. The `world paint: … slowest update` line
  adds `, N ms of cpu`.
- The test: the first frame must come under 80 s of total CPU and 20 s of the shell thread's
  (it had to come under 20 s of wall clock). The paint update must stay under 8 ms of thread CPU
  (it was 8 ms of wall clock). The peak-memory bound is unchanged.

gate.toml: world_ultra_load leaves `[alone]` (see Gate notes).

Nothing from opentennis applied. It has no CPU-time budget test.

## What the original does

Nothing. This is test tooling for the shell's load, not a port.

## How it was proved

Ran the test binary with `--nocapture` while other slots were working:

| load avg | wall to first frame | cpu all | cpu shell thread | paint wall | paint cpu |
|---|---|---|---|---|---|
| 69 | 20.83 s | 49.64 s | — | 5.21 ms | 3.17 ms |
| 69 | 20.20 s | 48.51 s | — | 3.14 ms | 2.99 ms |
| 49 | 20.35 s | 49.91 s | 11.10 s | 3.55 ms | 2.99 ms |
| 51 | 21.80 s | 51.52 s | 11.64 s | 2.96 ms | 2.99 ms |
| 44 | 18.28 s | 48.16 s | 10.25 s | 2.87 ms | 2.99 ms |
| 27 | 14.52 s | 44.00 s | 7.75 s | 2.72 ms | 2.99 ms |

The wall clock crossed the old 20 s limit in four of six runs, and the CPU figures stayed within 44–52 s.
The paint update's CPU time stayed at about 3.0 ms while its wall clock rose to 5.2 ms. `./kit check --
-p openmx --test world_ultra_load` passed (2 tests, neither skipped).

## Not verified / not 1:1

- No run on an idle machine: the load never went under 27 during the session. The idle CPU figures
  are probably lower (less SMT sharing), which only adds margin.
- I did not build a pre-P283 shell to check that the new CPU bounds would catch that regression. That
  claim is reasoned, not run: P283 removed 24 s of serial seam reads, which ran on the shell's thread,
  so they would put it well over its 20 s bound.
- At heavy load (100+) the CPU time still grows a little because of SMT core sharing (44 s at load 27,
  52 s at load 51). The bounds give about 1.6× over the loaded runs. Load 146 was not measured with the
  final build.
- Not Linux: `cpu_time` returns None, so the load line says `cpu not known here` and the test fails on
  its missing-figure assert. The paint figure falls back to the wall clock there. The gate runs only on
  this Linux machine.

## Next

none

## Gate notes

- `./kit done` went red three times on disc_worlds' own wall-clock limits at load 50-100: world_mxu at
  561, 505 and 549 s, world_mxu_nat at 516 s, and the installed world_mxu at 73 s. That is K24's work,
  so I put those figures on K24 as a note. Once the tests passed (4782 tests) and only the audit
  stopped it.
- Done with `--skip-audit`: the audit's four leaks are in crates/game-sim/src/atv_wheel.rs and
  plan/DONE.md, which belong to other tasks and which K23 does not touch. They show up because this
  machine's oracle.txt has names for those addresses. K23's diff adds no addresses.
- I merged origin/main into task/K23 to pick up P226's disc_worlds `[alone]` stopgap, which the runner's
  merge gate already has. Then I took world_ultra_load back out of `[alone]` (P294 had put it there as a
  stopgap until K23), which the K23 line on main asked for. It runs in the pool now.
