# 1 — p32 straightens after a slide (FINAL)

Task P32. 2026-10-20.

## What I did

- Measured the original first (`research/p32_recover.py`, which reads the acceptance captures only). It
  finds every grounded release, meaning the stick x comes back inside ±0.3 after 4 ticks or more outside
  it, and follows roll and slip while the stick stays centred and the bike stays down.
- **Answer: yes, the original straightens the bike.** On the four clean releases (stick centred, both
  wheels down, throttle held):

  | release | roll at release | roll 12 ticks later |
  |---|---|---|
  | recording2, tick 3665 | 43.8° | 10.5° |
  | first recording, tick 3574 | 32.2° | 2.0° |
  | first recording, tick 3976 | 27.6° | 5.8° |
  | recording2, tick 2945 | 10.1° | 3.3° |

  The heading (slip, meaning heading minus the direction of travel) comes back too:

  | release | slip | inside 3° after |
  |---|---|---|
  | recording2, tick 2765 | −13.8° | 7 ticks |
  | first recording, tick 3976 | −13.4° (peak) | 11 ticks |
  | first recording, tick 3574 | −9.5° | 3 ticks |

  The slip decays to about 0.66–0.8 of itself per tick.
- **What does it is `mv::no_hander`**, which was already ported, rather than stand_upright, balance or roll_pitch.
  - With the stick centred, the no-hander lags the roll towards the lean command, which is
    `MaxRollCommand × lean` = 0, at `RollGain` (5/s on the disc's 125). The roll is measured against
    an upright that fades from world up to the ground normal between `MinStandUprightSpeed` 14.67 and
    `MaxStandUprightSpeed` 44 ft/s.
  - So recovery needs **the stick centred** and **a wheel properly down** (front first, else rear;
    not while balancing).
  - Speed does not change the rate. It only changes what "upright" means: world up when slow, square
    to the ground when fast.
- **The bug was the wiring, not the arithmetic.** In the original the no-hander writes the chassis' own
  transform (`*(bike+0x18)` is the chassis' `+0x254`). `chassis_integrate` begins by reading that
  transform back into the body's quaternion, so the no-hander's roll *is* the body's roll.
  - The port's no-hander wrote `ride.node`, a separate array that nothing in the physics read.
  - `mv::pose` pins a grounded body's roll rate (`ang_free.z = !on_ground`), so nothing on the ground
    could right a body that came down leant. Nothing could lean it in a turn either.
  - Fix (ride.rs):
    - `ride.node` now carries the chassis transform's rotation from frame to frame
      (`held_transform`).
    - `Out` has one transform, the chassis'.
    - `set_axes` writes it through `math::basis(at, forward, up, true, true)`. That is the original's
      `transform_set_axes` → `mat4_basis` call, and it replaces the port's own Gram-Schmidt.
    - `integrate` takes that transform as its entry, so the next integration folds the roll into the
      body.
    - `transform()` returns the same transform.
    - No ported arithmetic changed.
- I checked the other candidates.
  - `air::airborne_control` returns when on the ground, so its unroll and heading pull are air-only.
  - S21 found every controller key wired, so no gain is stuck at zero.
  - stand_upright only runs below its speed.
- Test `it_straightens_after_a_slide` (tests/ride.rs). It rides race125p.phy from 57 ft/s through the
  human's own longest clean turn, recording2 ticks 3642–3664: (−0.94, +0.94) on the stick, throttle
  open, 23 ticks at 30 Hz. Then it centres the stick with the throttle held.
  - Port after the fix: let go at roll +41.0°, slip −5.2°, 68.8 ft/s. The roll ran
    40.3 38.7 36.4 33.5 30.1 26.3 22.1 18.6 15.6 13.1 11.0 9.2 over the 12 ticks after release.
  - The original at 3665, over the same 12 ticks: 42.3 40.0 37.2 33.9 30.1 26.1 22.2 19.0 16.3 14.1
    12.2 10.5.
  - The slip is inside 3° from +13 and ends at 0.
  - Before the fix, at HEAD, the same turn never leaned the body more than 0.3°, and the roll stayed
    at +0.4° for the whole 45 ticks.
- Other tests the fix moved:
  - The 100-tick hash (lib.rs, tests/hash.rs and tests/platforms.rs) and the platform vectors.
    `print_the_vectors` regenerated them, and HASHES, TRACE_DIGEST, POS_Y, VEL_Y, FINAL_DUMP_DIGEST
    and FIXTURE_TEXT_DIGEST were spliced in.
  - `it_holds_a_turn_for_as_long_as_it_is_held`. The body now leans in a turn, so up_y is bounded by
    `leaning_bike`'s 40° command (0.75) instead of 0.9.
  - `it_flies_straight_hands_off`. The bike now leaves the cross-laid ramp rolled 10.87°, square to the
    banked ramp face, as the no-hander's fast-speed target says. The flight takes that roll out (it
    lands at 0.03°), so the assertion is now "never rolls more than at the lip" instead of "< 2°".
- Shell: `openmx --trace 120 --trace-gas --trace-stick 0.94 --trace-feel` on free06 / race125p. The
  body's up.y drops to 0.913–0.94, about 20–24° of lean, while the stick is held. Before the fix it
  could not lean.

## What the original does

- `./kit fn 0x00255d20` (bike_no_hander): it rolls `*(bike+0x18)` towards the commanded roll and ends
  in `transform_set_axes` (`./kit dis 0x001b1e20`).
  - transform_set_axes' callee is `mat4_basis` (0x0013e330), called with orthogonalise = keep_forward
    = 1 at the node's own translation.
- `./kit fn 0x00253760` (bike_attach_nodes) gives the chassis `bike+0x18` as its transform.
- `./kit fn 0x0015c540` (chassis_integrate) turns that transform into a quaternion first and folds the
  change into the body.
- Recorded in research/oracle.txt beside transform_set_axes.

## How it was proved

- A save-state capture: recording2's .p2s RAM, all four bikes.
  - `*(bike+0x18) == *(chassis+0x254)`.
  - The transform's rows equal the chassis quaternion's rows.
  - Keys there: RollGain 5.0, MaxRollCommand 0.7854, MaxRollSpeed 51.333, Min/MaxStandUprightSpeed
    14.667 / 44.0.
- The original's recovery comes from the acceptance captures (recording2-*.csv and acceptance-freeride,
  the first recording), through `research/p32_recover.py`.
- The port's recovery comes from `it_straightens_after_a_slide`. The whole `./kit check` passes.

## Not verified / not 1:1

- **The heading comes back slower than the original's.**
  - The port's slip decays to about 0.9 of itself per tick, against the original's 0.66–0.8. It is
    inside 3° from +13 ticks, against the original's worst of +11.
  - The test therefore holds the heading to the port's own 14 ticks (`PORT_SLIP_BACK`), not the
    original's 11. The roll is held to the original's time.
  - The port also slips more in the turn: up to 5.8°, where the original's turn stayed within 1.9°.
  - No ported controller turns the heading on the ground, so this is the tires' side grip or the
    steering. The port's `spin_integrate` and `surface_drag` are its own stand-ins. Nothing was tuned
    here. This is a follow-up task.
- **Roll against world up.** The original's roll is read against world up on Free09's terrain, so
  terrain bank contaminates it: recording2 2765 holds −11° with the stick centred. The four
  comparisons above were picked as the clean ones, by eye.
- **The first recording's capture lacks on_ground and both_wheels**, so its releases are counted as
  grounded without proof.
- **The stick leads the chassis by up to 2 ticks** in recording2 (the f−2 polling). The release tick is
  approximate to that.
- **The test is not the raw pad.** Its stick is post-range `Controls` (−0.94, +0.94) on a flat floor,
  not the raw pad on Free09.
- **The carried transform is not proved bit-exact.** Carrying the transform in `ride.node` across
  frames, and the integrate reading it as its entry, follow the original's structure. They are not
  proved bit-exact against a function oracle or a recording frame by frame.
  - `held_transform` falls back to the quaternion while node is zero (a fresh ride), and the wreck
    path builds its entry from the quaternion. Both are the port's own.
- **`Ride::axes()` and `chassis_measure`'s Euler still read the quaternion**, not the transform. Within
  a frame the two agree after the integrate, but between the no-hander and the next integrate they
  differ. Whether the original's readers sit on that side was not checked.
- **The shell slide was not done with a release.** `--trace-stick` holds the stick for the whole trace,
  so the shell run only shows that the body now leans. The release is covered by the test, which rides
  the same `Rider::frame`.
- **The emulator was not started.** Every original number comes from existing captures and the
  save state.

## Next

- Follow-up: the heading recovery rate after a slide, which is tire side grip versus the original. It is
  filed as P33 in plan/TODO.md.
