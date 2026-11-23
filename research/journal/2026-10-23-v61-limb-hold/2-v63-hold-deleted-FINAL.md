# 2 — V63 hold deleted (FINAL)

Task V63. 2026-11-23.

## What I did

- **Deleted the riding bike rider's stand-in hold.** Removed `hold_on`, `holds_in`, `Body::hold`,
  `Body::holds`, `Body::held_by` and `Body::feet` from `crates/game-render/src/rider.rs`, and
  `Seat::held` from `crates/game-render/src/chassis.rs`.
- **Why it is safe.** The riding ragdoll is on by default (S90; online P133; AI V133), so its pins
  hold the hands and feet to the grips and pegs.
- **Where the clip is drawn as written.** The clip is drawn untouched wherever there is no ragdoll:
  posed, before it is placed, with no ragdoll kit, or with `OPENMX_RIDING_RAGDOLL=0`.

## What I kept

- **The limb solver.** `aim`, `solve_limbs`, `hold_limbs` and `lean_in` stay in `rider.rs`.
  - The user-asked bare hands (MD41) use it. The bike has no clip for that trick.
  - The seats where no ragdoll is stepped use it: the biplane pilot, the car driver, the quad, the
    garage and `--moment`. They now reach it through `Body::seated_palette`.
- **The sim's `feet`.** `foot_down` keeps its `feet` in the state and the hash. Nothing draws from it.

## Checked

- **Game render.**
  - `body` 12 of 12 pass.
  - `trick_chain` 2 of 2 pass.
  - `whip_tricks` has 4 passing and 3 red 360 tests. They were red with the trick pack on main
    already (K87).
  - `a_360_keeps_the_hands_on_the_bars` is renamed `a_360_fades_back_to_the_airborne_row`.
- **Openmx.**
  - `foot_down`, `flip_tricks`, `switch_back_anim` and `reverse_clips`: 73 pass,
    with the trick packs in the slot (`flip_tricks` skips without).
  - `poof_cut`, `tiled_world` and `netplay`: 97 pass.

## Assertions deleted

- **Hands on grips.** The drawn-hands-on-grips assertions measured the stand-in. They were in the V20,
  V69 and V100 tests, in `flip_tricks` on riding ticks, and in `foot_down` (hands on the grips and feet
  back on the pegs).
- **Not replaced.** Only the full whip (V100) checks the riding ragdoll's hands; the 360 and moving off
  have no such check.
- **Feet after a 360.** The "feet settled on pegs within 0.05 ft after a 360" assertion
  (`a_360_throws_the_rider_off_the_pegs`) is gone.

## Pictures

- **Setup.** `free06` headless, with unmodified HEAD built beside this change.
- **Ragdoll on.** Frames 2, 6 and 120 look the same. They differ no more than two runs of one build
  differ.
- **Ragdoll off (`OPENMX_RIDING_RAGDOLL=0`).** Frames 2 and 6 now show the clip's outstretched arms.
  The stand-in used to reach for the grips there.

## Found, not fixed

- **Blend-in at the start of a ride.** A ride's first ~10 frames blend out of the bind pose. This is
  on main too, for both the clip and the ragdoll. Filed as B53 in `plan/TODO.md`.

## Next

Done as briefed.
