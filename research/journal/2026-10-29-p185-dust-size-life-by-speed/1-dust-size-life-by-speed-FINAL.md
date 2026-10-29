# 1 — dust size life by speed (FINAL)

Task P185. 2026-10-29.

## What I did

- `particles::Emt` reads `InitialSizeToSpeedRatio` and `LifeToSpeedRatio` (`size_ratio`, `life_ratio`)
  and draws a new particle's size and life with `Emt::size_and_life(speed, moving, u_size, u_life)`.
  `Source::emit_over` calls it with the emitter's speed and `moving = MinimumMovementSpeed < speed`
  (the same test it already used for the share). The draw order of the port's RNG is kept: the
  life's draw is taken where it was taken before and handed in.
- A file that leaves the keys out now reads the speed emitter's own defaults: ISR 0.01, LSR 5, and
  ParticlesToDistanceRatio 0.01 (it was 0). On the disc only fireworks_* leave them out; most others
  set both ratios to 0, so nothing changes for them.
- research/oracle.txt: comment lines on particle_life_size and speed_emitter_read.
- Tests: `crates/openmx/tests/particle_size_life.rs` (new, an oracle test plus a unit test) and
  `wreck_particles.rs::the_wreck_dusts_grow_with_their_speed` (new, against a capture).

## What the original does

`./kit fn 0x00152030` (particle_life_size, arguments a0 emitter, a1 size out, a2 life out, a3 moving,
f12 speed, f13/f14 two random units). For each of size (+0x2f4 min, +0x2f8 max, +0x2fc span) and life
(+0x330.., ratio +0x3b8):
- if `moving` and the ratio (+0x3b4 for size) is above 0, the value is `ratio*speed`, clamped to
  [min, max];
- otherwise it is `min + span*u`.

Only on the emitter's other path (+0x23c set, `./kit fn 0x001512b0`) is the life then scaled by
`max(0.5, +0x254 * system+0x84)`, the view cull. `./kit fn 0x00150540` (emit) passes its
`|pos−prev|/dt` as the speed and `MinimumMovementSpeed (+0x3c8) < that` as `moving`.
`./kit fn 0x00152230` reads "General Info" with these defaults: ISR 0.01, LSR 5, PtD 0.01, PtS 0,
MES 0, MMS 0. Those overwrite the constructor's MES 25 and MMS 15.

The wreck's Vehicle Dust and Rider Dust (rider +0x1490 → +0x3c entries 0 and 2 → +0x14) carry 0.1
and 5 in RAM, as the disc's wreckdust .emt says. Their size runs 0.1..1 and their life 1200..2000 ms.
So a dust particle is 0.1 ft for each foot a second up to 1 ft, and lives 1200 ms unless the emitter
does more than 240 ft/s.

## How it was proved

- **Function oracle, 20,000 cases** (`particle_size_life.rs::a_particles_size_and_life_are_the_originals`):
  `particle_life_size` runs on the boot ELF over a made-up emitter. The inputs are random ends (some
  equal, some backwards), ratios of 0, negative, the disc's and made-up ones, speeds of every size and
  on the ends' own speeds, `moving` both ways, and random units. Every size and life matches the port
  bit for bit. Branch counts were [9932, 10068, 9923, 10077] (size by speed, size drawn, life by
  speed, life drawn), and the test asserts each is over 2,000.
- **Capture with two RAM images** (`wreck_particles.rs::the_wreck_dusts_grow_with_their_speed`):
  S109's animandpump wreck, replayed in copy 3 with P153's spec:
  ```
  WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1 SAVE=3850:1,3910:2 \
    ./kit pcsx2 with -- bash research/journal/2026-10-27-p153-wreck-particle-contacts/capture/play.sh \
    <pid> context/recordings/animandpump.p2m2 3700 230 context/capture/p185/run.csv
  ./kit state ram context/capture/p185/run-3850.p2s --out context/capture/p185/run-3850.ee.bin   # and 3910
  ```
  This gives 230 rows (3700..3929) and states at 3850 and 3910. P153's own wreck-3790 state is too
  early, because the dust starts at 3794 and none was alive in it.
  - All 171 live Vehicle and Rider Dust particles lie on the port's curve: life exactly 1200, and size
    0.3..1.0 (the 3 ft/s emission gate up to the cap). The original's random range would put them
    anywhere in 0.1..1 and 1200..2000.
  - Each (entry, age) group is one frame's particles, and its last one (highest list slot) came from
    the frame's last contact or sphere, which is the one the row records. 39 of the 141 groups match
    `size_and_life(|e_at − e_from|/dt)` from a row within one tick of the birth tick, to ≤1 ulp.
    19 of those are under the 1 ft cap, with sizes 0.31..0.75. The test asserts ≥15 under the cap,
    spread below 0.35 and above 0.7. A random size would hit within one ulp of the row's speed about
    never.

## Not verified / not 1:1

- **The capture's speed is only within 1 ulp.** The test's speed is the row's two f32 points over
  the row's dt, so its subtract, length and divide are not the game's own. Half the uncapped matches
  are 1 ulp off, none exact-by-construction. The port's `emit_over` takes its speed as before (P153),
  so its arithmetic order against the original's f27 is untested.
- **Frames whose last emitter call made no particle are only held to the curve.** The row records
  the last contact or sphere's points, so the earlier particles' speeds are not in the capture. Those
  ~100 groups are checked for life 1200 and size ≥ the gate's, but not for their exact speed.
- **The sphere and body speeds were not matched.** The capture's body_vel and sphere velocity give
  the speed to only ~1e-3, which is a phase issue (they were read at another point of the tick). I
  used the emitter's at/from instead.
- **The +0x23c path's life cull (×max(0.5, view)) and the 0x1512b0 emit path are not ported.**
  `Source::emit_over` is not that path, and no emitter the port drives sets +0x23c.
- **The reader's defaults (0.01 / 5 / 0.01) come from disassembly only**, not from an oracle run of
  the reader. They matter only for files lacking the keys (fireworks_*), and the ParticlesToDistanceRatio
  default changed 0 → 0.01 for those without any test of the fireworks' look.
- **The random units are the port's RNG, not the original's `rand`** (as before P185). Only the
  speed branch is deterministic against the original.
- **The random branch's `min + span*u` is now fpu arithmetic**, which is bit-exact to the original
  by the oracle. Earlier `rng.between` results may differ by an ulp from before (no fixture pinned
  them).
- **The capture and RAM images live only in s3's context** (context/capture/p185). The live test
  skips elsewhere.

## Next

Nothing for this task.
