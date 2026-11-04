# 1 — native libm cost (PART)

Task F40. 2026-11-04.

## What I did

- **Measured first, as the task said.** The verdict is that libm does not matter in a native-sim
  ride, so I made **no code change**. Exact builds are untouched, and so are native builds.
- A note on the premise: under `native`, `ps2_float::libm` does not run software arithmetic. Its
  polynomials are written on `F`, so their adds and muls are already host ops in a native build.
  What a native build still pays for is fdlibm's shape: the argument reduction, the polynomial
  lengths, and `powf`'s long log/exp chain.
- **Stack samples** (profiling-without-perf recipe). I built openmx with `--features native-sim`,
  `line-tables-only`, into `target/prof`, and ran
  `openmx --headless --steady-clock --draw-fps 30 --track free06 --players 2 --controls original --play gas.tape --frames 20000`
  with `OPENMX_THREADS=1`. The tape is hand-written: Cross held from line 31. I sampled it with
  `eu-stack -s -i`. 75 samples landed inside `World::simulate`, and 1 of them was in libm (`powf`,
  called from `tire.rs`'s curve). That is too few samples to rely on, so I counted calls as well.
- **Call counts.** I temporarily put an atomic counter at the top of every `pub fn` in `libm.rs` and
  printed the counts at exit. Then I reverted it (`git checkout`); none of it was committed. Same
  ride, 3000 ticks:

  | fn | 1P calls | 2P calls | per tick (2P) |
  |---|---|---|---|
  | fmodf | 72000 | 144000 | 48 |
  | sinf | 23562 | 23566 | 7.9 |
  | powf | 5866 | 17548 | 5.8 |
  | cosf | 7 | 11 | ~0 |
  | acos (f64) / sqrt (f64) | 60000 / 29998 | the same | — |

  The `acos`/`sqrt` count does not change between 1P and 2P. These calls come from
  `AngleTable::build`, which builds the 30,000-entry trick table twice at load. They are not
  per-tick calls.
- **Cost per call** (a scratch bench on `ps2-float` with `native`, release, 4M calls each, desktop
  at load ~60–76):

  | call | ns |
  |---|---|
  | fmodf (yaw-sized) | 7.9 |
  | fmodf (traffic clock) | 13.2 |
  | sinf (small) | 10.9 |
  | sinf (to 200 rad) | 24.6 |
  | cosf | 10.6 |
  | powf | 67.6 |
  | the loop alone | 1.5 |

  The whole acos table takes 7.1 ms, so about 14 ms once, at load.
- **What that comes to.** 2P: 48×13 + 8×25 + 6×68 ≈ **1.2 µs a tick**, against 1.83 ms of sim a
  frame in the same run (`--timings` session line), which is **0.07%**. 1P: about 0.6 µs against
  1.36 ms, **0.04%**. Even a libm 10× faster would gain under 0.1% of native sim time. That is
  below the noise of any measurement on this machine, so it does not justify a second set of libm
  routines that would need their own cross-platform proof.
- I took nothing from opentennis. Its native/libm split is not like this one.

## What the original does

Not relevant: this task is about the native build's speed, not about the original's behaviour.

## How it was proved

This is a measurement, not a port. The call counts are exact (atomic counters over a 3000-tick
ride, 1P and 2P). The per-call costs come from a microbench. The libm share comes from counts ×
cost, divided by the run's sim ms a frame. The stack samples (1 of 75 in libm) agree with that
share being small.

## Not verified / not 1:1

- Only free06 on the stock bike, with one gas-held tape. Rides on other maps or machines may call
  libm more: the biplane calls `powf` a few times a tick, and maps with many path cars call `fmodf`
  per car. At these per-call costs it would take thousands of calls a tick to reach 1%, and nothing
  here comes close. I did not measure it.
- I did not test the wreck/ragdoll path in particular. `ragdoll.rs` calls `sinf` once per joint
  solve; the gas tape's ride may not have wrecked.
- Desktop only, under heavy shared load (load average 60–76). No Steam Deck numbers. The per-call
  ns are absolute timings and would be roughly 2–3× on the Deck, which is still well under 1%.
- The stack sample is small (75 samples in `simulate`). The conclusion rests on the call counts.
- No code changed, so nothing new needs proving. The full gate is not green (P294, K23; see Next).

## Next

The work is done, but the task is not ticked. `./kit done` could not pass the gate on 2026-11-04, for
two reasons this task did not cause (its commit changes only this journal):

- `web.rs` `the_browser_installs_from_the_store` is red on main since P293 (46624303). A finished
  store install now asks to reload instead of re-reading `mods/`. It fails alone in 30 s. Filed as
  **P294** (critical).
- `world_ultra_load`'s two tests red at load 50–76 (first frame 21.6 s and 34.7 s against a 20 s
  limit), and pass alone. Filed as **K23** (critical).

Also, in this slot only (git-ignored data): `kiowa_capture` needed `context/states/c14-kiowa.*`,
which I copied from `kept-context/s4`. `machine_packs` needed a current atv pack, which I copied
from s3's `mods/machines/mxvatv_atv` (the old one is in `target/tmp/mxvatv_atv.old`).

Once P294 lands, run `./kit done F40 "<msg>"` again. Nothing else is left to do. If libm ever does
show up in a native profile, `powf` is the one to replace first: it is ~68 ns, against ~10–25 ns
for the others.
