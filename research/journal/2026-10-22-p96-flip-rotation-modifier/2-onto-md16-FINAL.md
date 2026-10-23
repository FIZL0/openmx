# 2 — onto MD16's moves (FINAL)

Task P96. 2026-10-23.

## What happened

The first attempt (part 1, `wip/P96`) was finished and its gate passed, but the runner could not merge
it. While it ran, main took MD16 (MX vs. ATV's trick pack: an R1 front flip row, `air::FRONT_FLIP`, and
the 360, `air::three_sixty`), P98/P101 (`Mods::rebike`, a ride moved onto new modifiers mid-ride) and
P108 (`Knob::PlaneTurn`). The merged build failed with three errors. The main one was a name clash:
part 1's factor was `air::Spin`, and MD16 had added its own `air::Spin`, the 360's state.

I cherry-picked part 1 onto main and redid it there:

- **Renamed** the factor `air::RotationSpeed` and its field `Params::rotation_speed`. Its maths and
  read sites are as part 1 describes.
- **The R1 front flip** is a `Rotation` row, and `rotate`'s fire branch now reads it through
  `Rotation::tuned` alongside MD16's `modifier` guard. Nothing else was needed; this is the shared hook
  doing its job.
- **MX vs. ATV's 360** is not a row. Its `SpinTuning` gets the same hook, `SpinTuning::tuned(&Params)`
  (with `RotationSpeed::three_sixty`: torques k², rates k³, flick times untouched). `three_sixty` calls
  it on its first line, so every caller gets it. A trick pack's whip gains (`UnrollGain`/`UnyawGain`)
  are laid before `Mods::bike` in the shell (`main.rs`, `pack.lay` then `mods.bike`), so the k on the
  gains applies to them too.
- **Mid-ride retune (P98)**: `Ride::retune` does not rebuild the chassis' angular windows. So
  `Mods::rebike` now rebuilds them from the file (`ride::bike_ang_limits`, now `pub(crate)`) and moves
  them again, through a new `Mods::windows` that `Mods::ride` shares. It only does this when the
  rotation factor itself changed, so every other rebike is unchanged.
- `KNOBS` is 17, because P108's `PlaneTurn` is in too.

## Proof

`crates/game-sim/tests/rotation_modifier.rs` has two new tests, the R1 front flip and the R1 360, each
ridden off the lip on the stand-in bike with the test's own `[Three Sixty]` pack. Ticks for a full turn:

| rotation | 100% | 150% | ratio | 70% |
|---|---|---|---|---|
| backflip | 118 | 78 | 0.661 | 169 |
| front flip (stick, in a drop) | 380 | 253 | 0.666 | 543 |
| 360 (stick) | 189 | 126 | 0.667 | 270 |
| R1 front flip (MD16 row) | 117 | 78 | 0.667 | 167 |
| R1 360 (MX vs. ATV) | 189 | 126 | 0.667 | 270 |

Both 360s are held by the yaw window, so a full turn alone does not show that the 360's own torque
moved. The R1 360 test therefore also asserts that the torque it fires with at 150% is exactly
2.25 × the 100% one.

The other tests changed too:
- `every_row_reads_the_factor` now covers `UNLEASHED_ROTATIONS` and the 360's `SpinTuning`.
- `at_one_hundred_percent_nothing_moves` checks that both are untouched at 100%.

Where a new rotation plugs in:
- a new `Rotation` row in any set is covered by `Rotation::tuned` and by these tests as they stand;
- a rotation that is not a row reads its tuning through a `tuned(&Params)` the way `SpinTuning` does.

## Not verified / not 1:1

- **Part 1's list still holds**: the modifier is ours, so nothing is checked against the original away
  from 100%; "about" 2/3 because flight is not sped up; flick times and ground grace are not scaled;
  ground rotation is untouched; the menu row was not checked on screen.
- **Not ridden on the disc's bike or a real MX vs. ATV pack.** The new tests use the stand-in bike and
  `[Three Sixty]` numbers I wrote, with the magnitudes of MD16's generated pack. ride.rs's MD16 disc
  tests do not run with the modifier.
- **The 360's ground grace (0.9 s) and its 35° clip are not scaled.** The grace is time to leave the
  ground and the clip is an angle, so neither is a rate.
- **`Mods::rebike` window rebuild has no test of its own.** It is covered only by reading the code: it
  rebuilds from `bike_ang_limits(file)`, which is what `Ride::new` uses, and no knob writes
  `YawRateLimit`. P98's existing rebike tests still pass in the gate.
