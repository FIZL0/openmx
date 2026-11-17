# 1 — mxvatv truck class step (FINAL)

Task MD156b. 2026-11-17.

## What I did

- Ported the monster truck's per-frame class step (vt +0x488, `./kit fn 0x004d6f20 -p atv`) as
  `atv_car::truck_class_step`, beside the buggy's `class_step` and the golf cart's `golf_class_step`, with its own
  `TruckParts` / `TruckPosing` and a small `truck_lean` (the axle's lean off its two tires).
- No new node calls: it uses MD151's and MD156a's (`set_position`, `spin_about`, `stretch`, `position_in`,
  `aim_squared`) and the existing acos table.
- New test `tests/mxvatv_car_frame/truck_step.rs`. `update_beside.rs` now runs the truck's step for real in its copy
  and compares its part words each update. `drives.rs` now asserts every drive (the truck too) compared the class
  step on every update.
- research/oracle.txt `# MD156b`: every word, call and constant.
- opentennis: nothing to take; scene-node posing for a car class is this game's own.

## What the original does

- Ghidra calls it 1456 bytes; it really runs to 0x004d7e9c (~3.9 KB). The decompile is whole.
- Each corner (left front, right front, right rear, left rear): the upright is set at its point raised by the
  tire's compression and spun about up by the steer (the rears by the steer times a car word, the rear share);
  the wheel is spun about the side by the tire's angle.
- Each wheel (rears first) stretched along y: the buggy's code, word for word.
- Four mounts set at their points raised by their tire's compression.
- Two axles (front, rear): set at their point raised by half the pair's summed compression, then spun about
  ahead by acos(track / |(track, d)|), d = right compression less left, clamped to −1..1 (twice), negated if
  d < 0.
- Ten aims: two shocks along (where they are − their top), then eight rods along (where their partner node is −
  their top), each squared to up in the car's frame.
- No null checks, no car words written.

## How it was proved

- Function oracle, 10,000 generated cases on md39a/truck.ee.bin, nothing hooked
  (`truck_step.rs` `the_monster_trucks_class_step_is_the_originals`): every tire's steer (10 % past ±π, some 0),
  angle, compression (pairs now and then equal), maximum compression (10 % zero), stretch 0.3..1.7; points jiggled;
  the track the image's, negated, 0, small or huge; the rear share random; part positions moved and current bits
  cleared. Every port word equals the original's and the original wrote nothing in the parts or the car block the
  port did not. Thirteen branch counters, each ≥ 133 (the lean's clamp at both ends among them). A one-bit flip goes
  red.
- The truck drive (`drives.rs` `the_trucks_drive_is_the_ports`): 730 updates, each update's class step's part words
  compared against the original run in the copy.
- Mutants (restored with cp + touch): the lean negated at d ≤ 0 instead of d < 0 → oracle red at case 5 and the
  drive red at frame 97 (31 words); the rear share dropped → oracle red at case 0 (the drive stays green, see
  below).
- `./kit check -- -p game-sim --test mxvatv_car_frame`: passes.

## Not verified / not 1:1

- The rear share, the stretch and the lean clamp on the drive: the truck image's rear share is 1.0, its rear tires
  never steer on the drive, and every stretch is 1.0, so the drive cannot tell those clauses from simpler ones.
  The oracle covers them (random shares, stretches 0.3..1.7, huge tracks for the clamp).
- The lean clamp is only reached with a track whose square saturates (≥ ~1e19); the real truck never gets there.
  It is proved by the oracle only.
- The acos lookup: the original sums frac·hi then (1−frac)·lo, the port's `AcosTable::at` the other way round.
  The two-term sum is the same either way and all 10,000 cases agree, but no case was built to tell them apart.
- A NaN compression or track: not generated (the original would index the acos table with garbage).
- Missing parts: the original has no null checks; no case has a missing part.
- Stack temporaries' lanes the original leaves as garbage are not modelled; the callees overwrite them, and the
  oracle compares every word written.

## Next

- none
