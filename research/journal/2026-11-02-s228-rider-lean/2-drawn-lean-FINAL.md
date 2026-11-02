# 2 — drawn lean (FINAL)

Task S228a (P227's render half). 2026-11-02.

## What I did

I compared the drawn rider against the original's on S228's capture of the fly_and_collide tape. The
capture is context/capture/s228/fly.csv, with the images in context/states/s228/; both were copied
from s3. I added two tests, and changed no product code.

- **The clip layer**: `the_fly_riders_drawn_lean_is_the_originals` (crates/game-sim/tests/rider_anim.rs).
  - It runs V90's replay of P1's body off the capture's request words and poses it each tick with
    `posed_by`.
  - `two_p_poses::Tick` gained a `pose`, the drawn pose, which the test needs.
  - On every leaning tick (riding, on the original's state, the eased lean axis 0.15 or more off 0.5)
    it compares the head's FK place over the pelvis with the same from the original's p1_j0..21. It
    takes the best of rows t−1..t+1.
- **The drawn layer**: `the_drawn_rider_leans_as_the_original_through_the_turns`
  (crates/game-sim/tests/fly_and_collide.rs). `ride_drawn` rides each HELD stretch with the drawn
  rider built beside it, as the shell does:
  - thq1's body and its riding-ragdoll rig, plus the race500p chassis.
  - `body.ride`, then the ragdoll's skeleton from `ragdoll_posed`.
  - The chassis's `want_under` / `riding_under` / `frame`, then the holds from `grips`.
  - It reads the riding ragdoll's 18 part centres after each step.
  - The lean is head (part 3) less pelvis (part 0), in the drawn bike root's frame. It is compared with
    the original's p1_bone3/p1_bone0 `.pos` in p1_bike_xform's frame, taking the best of rows t+1..t+3.
  - The first 30 ticks after each image are skipped: the port's ragdoll starts fresh there, while the
    original's has a history.

## What the original does

- **p1_bike_xform is the drawn bike's root node, not the chassis** (S8 found this).
  - In turns it rolls far past the physics body: at 137181, up is (0.67, 0.63, 0.39) against the
    ride's (0.18, 0.91, 0.38).
  - I first measured the port's lean in `placed(&ride)`'s frame, and it looked 1.3 ft short in turns.
    That was the frame's fault. Measured in each side's own drawn root (`chassis.world[root]` ×
    `placed`), or in world space, the gap is about a quarter of that.
- **Clip layer**: on 937 of the 1153 leaning ticks, the port's drawn head is where the original's is,
  to 0.01 ft; the median is 1e-5. The other 216 are in 17 short spans, the worst being 138164..138232
  and 139630..139655 at 0.72–0.77 ft.
  - On those spans the whole body differs: up to 0.42 on a quaternion lane (LwrArmL), and on Head,
    UprTorso and the legs too.
  - On all of them the state and mode are the original's, the eased mix and phase are bit for bit and
    the ramp is 0.
  - Seeding the clip time from m_clock (V109's `Seeds { time }`) changes nothing.
  - So it is not the lean request, and not a fade. It is filed as V121.
- **Drawn layer**: 1201 ticks are compared. The figures below are distances in ft (median / p90 / p99 /
  worst), except the bike root's up, which is the angle between the two in radians.

  | measure | median | p90 | p99 | worst |
  |---|---|---|---|---|
  | lean to a side | 0.031 | 0.235 | 0.440 | 0.650 |
  | lean as a vector | 0.042 | 0.306 | 0.602 | 0.899 |
  | the drawn bike root's up (rad) | 0.027 | 0.095 | 0.166 | 0.195 |

  - Where it parts, the port leans less. On the 213 hard-turn ticks (the original's head 0.5 ft or
    more to a side), the port's mean lean to a side is 0.717 ft against the original's 0.893, a ratio
    of 0.803.
  - The worst is 137240..137248, just before the 137249 crossup: the port's lean is −1.19 ft and the
    original's −1.66.
  - The drawn root's roll off the ride is 0.226 against 0.251, a ratio of 0.898.
  - At rest the port's root has no roll at all, while the original's keeps 0.02–0.04 rad. That is S8's
    1.49°.
  - So the user's "doesn't lean enough" is real but smaller than the screen may suggest. The drawn
    rider reaches about 80% of the original's lean in hard turns, and the bike under it about 90%.

## How it was proved

`cargo test --release -p game-sim --test fly_and_collide --test rider_anim`: 3 and 17 pass. The two
new tests were also run with `--nocapture`, and they print the figures above.

The tolerances held:

- **Clip layer**: the 1153 leaning ticks are pinned. At least 937 must be within 0.01 ft (this may
  only go up), the median at most 1e-3, and the worst at most 0.8 ft.
- **Drawn layer**:
  - (1201, 213) ticks are pinned.
  - Lean to a side: median ≤ 0.05, p90 ≤ 0.25 and every tick ≤ 0.7 ft.
  - Lean as a vector: every tick ≤ 0.95 ft.
  - Root up: p90 ≤ 0.1 and every tick ≤ 0.25 rad.
  - The hard-turn lean ratio must be ≥ 0.80 (this may only go up).

## Not verified / not 1:1

- **The 20% under-lean was not fixed** (S253). It may come from the vehicle animation's lean blend
  (`Chassis::riding` and its lags), the ragdoll's give, or the holds. I measured it but did not trace
  its cause.
  - The missing rest roll on the drawn root is likely part of the same thing.
- **The clip-layer partings** (V121) were not traced. They also feed the drawn-layer numbers, for
  example 139658..139661.
- The shell itself was not run, and nothing was looked at on screen. `ride_drawn` copies main.rs's
  per-frame order rather than calling it.
- Only P1 was compared, and only on HELD stretches that last past the settle window.
