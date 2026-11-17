# 1 — kiowa wreck gravity (FINAL)

Task MD63f. 2026-11-17.

## What I did

- The task's premise was wrong. The gravity rewrites MD63d found belong to the **Kiowa** (vtable
  0x003cfad0), not the bike. A scan of GAME_REL for stores to the gravity block's +0x44 with 32.2 nearby
  finds only the plane and the Kiowa. I corrected the # MD63d line in research/oracle.txt and wrote # MD63f.
- The formula was already 1:1. `Wreckage::wreck` / `unwreck` (MX Unleashed's, MD71) compute
  `gravity_force(scale, mass)` = (0, 32.2 * (-scale * mass), 0, 1), the same as SLUS, and
  `KiowaAround::crashed` already sends MX vs. ATV wrecks through `Wreckage::act`.
- Two real differences, both fixed:
  - **The clock.** MXVehicle's base wreck (0x004a83d0) zeroes the crash clock; MX Unleashed's keeps it.
    Added `Crash::wreck_mxvatv` (the wreck plus clock = 0). `Crash::step_mxvatv` uses it for the edge
    throw and zeroes the clock when `Crash::step` returns a strike or contact wreck.
  - **The spawn gravity.** In both md80a images, the player's never-wrecked Kiowa holds the unwreck's
    -1369.9998779, while the idle twin holds the load's -1370. So the player's helicopter is taken through
    the unwreck. Added `kiowa::mxvatv_taken_gravity`. `KiowaRide::built` (openmx vehicle.rs) uses it when
    the ride is MX vs. ATV's. Before this, the ride's gravity only rounded like the original's after the
    first wreck. That was the task's symptom.

## What the original does

- Wreck: `./kit dis 0x004d3f90 --len 0x110 --program atv`. Unwreck: `./kit dis 0x004d40a0 --len 0x100
  --program atv`. Their bases: `./kit dis 0x004a83d0` / `0x004a8500`. The full word list is in
  research/oracle.txt # MD63f.
- The wreck damps, scales the rates by 1.0, writes the gravity from WreckGravity * mass, starts the sound,
  throws the blades (unless reason 13), and swaps two nodes. The unwreck writes the gravity from the scale
  * mass, stops the sound, zeroes the controls and the held height, shows the rotor, stops the blades,
  calls the put-back (placement), then the base (throw scale / 0.5 for a 13, reason and clock 0, rest
  damping, inertia).

## How it was proved

- Function oracle, crates/game-sim/tests/mxvatv_kiowa.rs
  `the_mxvatv_wreck_and_unwreck_are_the_originals`: 10000 generated cases on md80a kiowa (4995 wrecks,
  1744 of them thrown; 5005 unwrecks). The port and SLUS match bit for bit on the gravity, the gravity
  scale, the throw scale, the reason, the clock, the latch, the rates, both dampings, the body inertia,
  the inverse inertia, +0xcc0, the control words, the node bits, and the order of calls out (rumble
  test, events (8, 11) / (8, 13), sound, blade throw, blade stops, put-back).
- `the_taken_mxvatv_kiowa_holds_the_unwrecks_gravity`: on kiowa and kiowa3, the port's scale and mass from
  kiowa.phy match the images. `mxvatv_taken_gravity` equals the player's quad and `part.gravity` equals the
  twin's.
- Mutants: dropping the clock zero fails case 0. Reordering the gravity multiply to (32.2 * -scale) *
  mass fails case 8.

## Not verified / not 1:1

- The placement 0x004a6f70 (in the put-back) is hooked, not compared against the port's `place()`.
- The blade throw 0x004d3720 is hooked, not compared against MX Unleashed's throw; the wreck's rotor
  node bits it sets are therefore not compared.
- The blades' vt +0x134 stop calls are only counted, not ported or compared.
- The port has no +0xbbc gate: it always damps on a wreck and an unwreck. The Kiowa's +0xbbc is 3, so this
  only matters for other kinds.
- The order differs: SLUS runs its unwreck's own writes, then the put-back, then the base; the port runs
  place, then unwreck, then spawn. The oracle hooks the put-back, so the order's effect on placement is
  unproven.
- Events (8, 11) / (8, 13), the rumble, and the sound start/stop are not ported (only seen as calls).
- Which spawn path calls the unwreck is not found. The spawn gravity is inferred from the two images
  (player against twin), not from a traced call.
- `Wreckage.wrecked` has no SLUS field it is compared to.
- MX Unleashed's Kiowa spawn gravity was not checked (left as the load's).
- The oracle excludes NaN and denormal inputs.

## Next

- Nothing for this task. Possible follow-ups: compare the placement and the blade throw against SLUS.
