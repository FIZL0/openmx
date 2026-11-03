# 1. The full whip, captured and compared

## Why

S253 hung the ragdoll's target under the bike's transform. Since then, at a full whip the ragdoll's
pelvis has no longer moved the way `lean_in`'s stand-in does (cosine -0.2, was over 0.75;
whip_tricks `a_full_whip_keeps_the_hands_on_the_bars`). No capture said which way the original's
goes.

## The capture

- **Tape.** `capture/make_tape.py` starts from the human's recording2 (free09, 250sx, thq1). It uses
  two fields a tick, with field ≈ 2·(tick − 2377).
  - C8's fix is kept: flip 1's Taunt is lifted on fields 335..349, so jump 1 lands clean.
  - Jump 2's cancan is made a full whip. Circle is lifted on 549..600, and the left stick's x is
    set to 255 over 549..700, which covers take-off to past the landing. The stick's y stays the
    human's.
  - The script asserts that no trick button is held from 549 to the end (field 860).
  - Run: `python3 -B capture/make_tape.py context/recordings/c20/recording2.p2m2
    context/recordings/c20/c20.p2m2`. recording2's save state is copied as
    `c20.p2m2_SaveState.p2s`.
- **Playback.** `capture/play_whip.sh` is S201's play_entry.sh with the attach field passed in.
  It runs `entry_watch.py --state 0` and logs the rider's nodes, joints, request block, crossup
  lag record and body mode once a tick.
  - Run: `KIT_PCSX2=3 ./kit pcsx2 with -- bash capture/play_whip.sh 3344016
    context/recordings/c20/c20.p2m2 2600 40 context/c20/whip-right.txt`.
  - It recorded 204 ticks (2600..2803), torn 0, gaps 0.
  - The file is in slot s3 only and is git-ignored.
- **The ride.** The bike leaves the lip at tick 2651. The lip tick reads crossup phase 0 from the
  human's steer to the left. The whip then sweeps through and is all the way out (phase < 0.01)
  on 2691..=2726, 36 ticks. The bike lands crossed up and wrecks.

## The comparison

The comparison is `riding_spring` `a_full_whip_hangs_the_riding_body_as_the_originals`.

- **Setup.** The rider is S74's thq1: the capture's non-part joint offsets equal S74's held
  skeleton's within 1e-3. S74's skeleton therefore rebuilds the clip's target from the
  capture's joints.
- **Run.** The port's riding body is carried from the opening's settle (S253's `ride_watched`) on
  the original's request, step and clip.
- **Lean.** For each full-whip tick, the test measures how far the pelvis stands off the clip's
  pelvis, in the bike's frame (the request's drawn transform).

| | pelvis off the clip's, bike frame (ft) | length |
|---|---|---|
| original, mean over 2691..=2726 | (-0.093, +0.345, +0.133) | 0.381 |
| port's riding body, same ticks | (-0.093, +0.345, +0.132) | 0.381 |
| worst gap between them | 0.0043 ft at 2691 | |
| `lean_in`'s stand-in (whip_tricks ramp ride) | (+0.255, -0.005, +0.196) | 0.32 |
| whip_tricks ragdoll, 1/60 step | (-0.095, +0.482, -0.024) | 0.49 |
| whip_tricks ragdoll, 1/30 step (probe) | (-0.095, +0.386, -0.025) | 0.39 |

- **Over the whole stretch** (2631..), the port matches the original:
  - 97 ticks;
  - worst part 0.070 ft (frame 2641, part 3);
  - worst turn 0.101 (2663, part 17);
  - mean 0.0137 ft.
- **Hands.** The original's hands are at most 0.158 and 0.174 ft off the grips. The clip's are
  0.232 and 0.366.

## What this settles

- **The original's body rises at a full whip.** Its pelvis goes up along the bike's own up by
  about 0.35 ft. It does not slide toward the bars or sideways.
- **Not gravity.** World up swings round in the bike's frame as the bike rolls through the flight,
  but the lean stays on the bike's up. So it is not gravity pulling the body.
- **The port's riding body is the original's here.** It is within 0.005 ft of the original on every
  full-whip tick.
- **S253's ragdoll is right and `lean_in` is wrong.**
  - whip_tricks' ramp-ride ragdoll goes the original's way (cosine ~0.88).
  - `lean_in` moves the body sideways and forward (cosine ~0.05). Filed as **B21**.
- **Most of the ragdoll's extra rise in whip_tricks is its 1/60 step.** At 1/30, the shell's and
  the original's tick, the rise is 0.39 ft against the original's 0.38. The remaining z gap
  (+0.13 against -0.02) is a difference between the rides: the original's bike rolls through
  the flight, while the ramp bike's attitude is constant.
- whip_tricks' direction check can come back against the captured lean. Filed with the 1/60 step
  as **T4**. whip_tricks itself is unchanged here; the 1/30 numbers come from a probe that was
  reverted.

## Checked

- `./kit check -- -p game-sim --test riding_spring`: the new test and the file's other tests pass.
- The numbers above are from the test binary's `--nocapture` output.
- `./kit audit`.

## Not verified / not 1:1

- Only one capture was made: a right whip, on one jump (free09 jump 2), with one rider and bike
  (thq1 on the 250sx). Left whips, other bikes and other riders are not captured.
- Nothing here was looked at on screen. Only the RAM logs were compared.
- The landing wrecks the bike, so nothing after the full whip (the recovery onto the bars) is
  compared.
- whip_tricks' ride is a different ride from the capture (a ramp, constant attitude). The cosines
  and the 1/30 numbers in the table compare two different rides, not the same one.
- Copy 3 ran with emulation speed NominalScalar = 0.25 (its PCSX2.ini, as in S30/S58/S93), not full speed. Whether that
  touches the rider's body was not checked.
- The test's `WHIP_MEAN_AT` (0.02) is looser than the settled `MEAN_AT`. The crossup sweep
  2652..2675 leaves the pelvis up to 0.044 ft off. Where that gap comes from was not traced.
- The capture and the tape are in slot s3's `context/` only. They are not committed.
