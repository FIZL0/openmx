# 1 — md real 360 (FINAL)

Task MD30. 2026-10-24.

## What I did

- **Took MD25 out, all of it.** I ran `git revert --no-commit 607d39b7`, then put back MD25's plan lines and its journal folder from HEAD, because history stays. Removed:
  - from `air.rs`: `SpinStyle`, the `EASY_SPIN_*` constants, `easy_spin_yaw_limit`, `Spin.easy`/`progress`, `Step.easy_spin`, and the easy path in `three_sixty` and `airborne_control`;
  - from `ride.rs`: the yaw-limit override;
  - the snapshot wire's two extra Spin fields;
  - the `spin_style` hashing in `trick_pack.rs`;
  - `--three-sixty` in `main.rs`;
  - the menu's `ThreeSixty` departure row (`Departure::ALL` is back to 6), its settings key and the about text;
  - `Pack.spin_style` in `trick_packs.rs`;
  - MD25's tests in tests/air.rs, ride.rs, rotation_modifier.rs and openmx tests/menu.rs.

  A settings.txt that still carries `three_sixty = …` reads fine: the key falls to `_ => {}`. New test `an_old_three_sixty_line_is_read_and_ignored` checks this.
- **MX vs. ATV rides hash as before the removal.** I kept `md25_mx_vs_atv_rides_as_before` (now on `md16_pack(&p, true)` alone):
  - `MD25_GESTURE_FOLD` is unchanged.
  - `MD25_DIAGONAL_FOLD` was also unchanged with the removal alone (I ran it before the next step).
- **Checked `air::three_sixty` line by line** against openmx-mods `notes/tricks.md` "The 360" and `./kit fn 0x4e53d0` (MX vs. ATV's kit: `KIT_CONFIG=~/repos/openmx-mods-slots/s1/extracted/mxvatv/kit.toml`). Each of these matches:
  - the lip test and the backflip lock-out;
  - R1 alone arms it, and it fires past 0.7 of the stick the armed way;
  - map_clamp flick scaling of both torques;
  - the 0.9 s grace;
  - the heading record at the fire;
  - the end conditions: |yaw torque| < MinY, a contact, or landing;
  - the decay: only with no trick button held and the stick against the spin;
  - the clip states and the 35° rule;
  - the wreck when it ends with the clip ≠ 0;
  - the ramp → authority hand-off.

  I changed nothing in `three_sixty`.
- **Reproduced M21's `spin` and `spinpull`** in the sim off a 1.6 s lip (tests/ride.rs `md30_m21_spin_lands_part_way_round_and_wrecks` and `md30_m21_spinpull_comes_round_and_lands`). The port first reached the disc's steady rates exactly (−2.44 / +2.44 rad/s). So **neither YawRateLimit nor the air damping cuts the spin short**:
  - The spin settles where torque balances damping, ω = τ/damping per axis. Both games agree on that: 1100/450 = 2.44.
  - The 125's YawRateLimit of 3 sits above 2.44, so it never clips.
  - What the port did get wrong was `spinpull`: it would not pitch faster. MD16 silenced the stick under any 360.
- **Fixed the air control under a 360** (`air::airborne_control`, new `Step::spin_ramp`, set from `Spin::ramp` in `ride.rs`). Per `./kit fn 0x4e65d0` (tail at 0x4e6720), MX vs. ATV:
  - computes the stick moment unless *both* `+0x1194` (backflip applied) and `+0x11d0` (360 applied) are set;
  - scales the moment 0.5/0.4 when `+0x1524` < 0x29, and leaves it full otherwise;
  - zeroes its yaw/roll only while the backflip ramp `+0x1198` is set;
  - takes the damped branch (moment −= ω·`+0x1180`), not auto-level, while the 360's ramp `+0x11d4` is set, as well as when steep, when the stick is ≥ 0.1, when the backflip ramp is set, or when authority > 0. So after a 360 ends in the air, the bike keeps damping instead of levelling until the wheels are down;
  - applies the lean force only with both applied flags clear.

  Before the fix, the port silenced the stick whenever `spinning`, and only the backflip's `flipping` chose damping. Now the stick is silenced only by the backflip's applied torque, or by the 360 while the backflip is flipping/active. The second case is MD20's at-once state, which never happens on the disc (see below). The 360's ramp damps. tests/air.rs `md30_a_three_sixty_alone_keeps_the_stick_and_damps_to_the_ground` checks this over 10,000 generated airborne states.
- **The fix changes MD25_DIAGONAL_FOLD** from 0x13fe6e4514be885d to 0xc39acc450c4488a5. That ride pushes the stick diagonally through a backflip-then-360, so the change is expected. With only the stick part and not the damping gate, it took a third value, so both halves move it. The doc comment on the constant says so.
- **Retimed `md16_three_sixty_comes_round_or_wrecks`'s "ended" push** from 260–300 to 270–290. The stick's own yaw moment now pushes against the turn as well, so a shorter push ends it.
- **MD20's departures are kept unchanged:**
  - `BOTH_AT_ONCE_UNDER_R1`: the backflip and 360 can both run under R1.
  - `SPIN_PITCH_FOLLOWS_THE_FLIP`.

  Their tests (`md20_*`) pass. The new `silenced` term keeps the stick out of that at-once state as MD20 had it, so their landings didn't move.

## What the original does

- **The 360 step** (`./kit fn 0x4e53d0` in MX vs. ATV's kit):
  - On a lip, with no backflip running, R1 alone arms it.
  - It fires once the stick passes 0.7 the armed way, with yaw and pitch torques scaled by the flick.
  - It keeps its torque, with a 0.9 s grace, until |yaw torque| < MinY, a contact, or landing.
  - It decays only with no trick button held and the stick against the turn.
  - Landing more than 35° off the heading at the fire, with the clip still running, wrecks.
- **The air control** (`./kit fn 0x4e65d0`) works as listed above:
  - The 360 doesn't silence the stick.
  - The stick's full `AirborneControlMoment` adds to the 360's torque. That's −400 of pitch in `spinpull`: −3.33 = (−1100 − 400)/450.
  - The 360's ramp keeps the damped branch on until touchdown.

## How it was proved

- **M21's captures** (`~/repos/openmx-mods-slots/s1/extracted/mxvatv/capture/m21-spin.csv`, `m21-spinpull.csv`, mx_Race250P, state 4's first lip): I read body rates from the quaternion and summed them as `both_sum.py` does. These are the sim reproductions off the generated lip (108, 0.31, 25):

  | | disc | port |
  |---|---|---|
  | spin: air | ~1.63 s | 1.57 s |
  | spin: steady pitch/yaw rad/s | −2.44 / +2.44 | −2.44 / +2.44 |
  | spin: x / y / path | −212 / 217 / 306° | −216 / 207 / 309° |
  | spin: landing | 37° off, wreck | 44° off, clip 2, wreck |
  | spinpull: air | ~1.63 s | 1.60 s |
  | spinpull: steady rates | −3.33 / +2.44 | −3.33 / +2.45 |
  | spinpull: x / y / path / net | −289 / 217 / 365 / 10° | −294 / 215 / 371 / 5° |
  | spinpull: landing | 9° off, clean | 2° off, clean, scores 360 right |

  Both tests assert air ±0.1 s, rates ±0.02, angles ±15°, and the outcome.
- **The air-control rule:** a function check over 10,000 generated airborne states (tests/air.rs `md30_*`). It compares against the plain damped and turning moments with the same stick.
- **MX vs. ATV rides:** `md25_mx_vs_atv_rides_as_before`. The gesture fold is unchanged. The diagonal fold changed, for the reason above.
- **Gate:** `./kit check --all` from `./kit done`.

## Not verified / not 1:1

- **The lip is generated, not the disc's map.** Its slope of 0.31 was fitted to give the capture's 1.6 s of air. So take-off speed and pitch differ, and the angles agree only to a few degrees. The spin landing sits near the 35° edge (slope 0.32 brought it round in an air substep). That sensitivity is faithful, since the flattened heading jumps once the bike is pitched past vertical. But it means the outcome depends on the lip.
- **The rider/bike is MXU's 125 with the MD16 pack, not mx_Race250P.** The steady rates match because they are torque/damping, and both games have 450 and ±1100. The inertia-driven ramp-up to that rate isn't proved against the disc.
- **The minimum of up.y in flight differs**: port −0.15 / −0.44, disc +0.07 / −0.22. The capture's "up" may come from a different node row than the port's. I didn't run that down.
- **The stunt-scaling condition**: the original tests `+0x1524` < 0x29. The port uses its own `step.stunt` as before. I didn't check whether they agree in every state.
- **The airborne pitch/roll wreck limits**: the port skips them while `spinning`. I didn't trace the disc's matching gate in its wreck test.
- **Crossup paths**: the ramp-gated crossup calculations elsewhere in the air code (decomp game.c around lines 430682 and 447961) weren't traced.
- **The stick-against-spin ending**: the decay with no trick button held isn't in any M21 capture. It is checked against the decompile and notes only, through md16's retimed test.
- **MD20's departures are kept on purpose** (`BOTH_AT_ONCE_UNDER_R1`, `SPIN_PITCH_FOLLOWS_THE_FLIP`). Under MX vs. ATV tricks, MXU's backflip remains alongside the 360, and the silenced-stick rule for the at-once state is the port's own. The disc can't reach that state.
- **MD16's and MD20's test timings** were retimed (the md16 push), so their old numbers aren't comparable.
- **Opentennis**: not applicable. This is the bike's air physics.

## Next

Nothing for MD30. If someone wants the 360 proved on the disc's own lip, that needs a capture of the original's
lip geometry (or MXU's bike on the disc), which is a new task.
