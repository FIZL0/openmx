# 5 — the parked plane's creep was the unflown controls' brake (FINAL)

Task S252b. 2026-11-03.

## What I did

- **What differed.** The four consecutive RAM images `context/states/s252-plane-11072..11075`
  show the original's plane as **flown**: the input gate +0x334 is 1, the pad +0x338..+0x348 is
  zero, the brake +0x350 is 0, the throttle +0x34c is 0, and the elevator command +0x358 and the
  pitch-rate integrator +0x584 are both 0.5236. `off_the_save_state` (S41) reads the body, the
  tires and the engine's clock and noise tables. It does not read the controls, because the C7 fixture
  takes them from its capture row. The S252 test has no such row, so its plane kept
  `Flight::new`'s unflown attitude. `biplane::controls` then holds an unflown plane's brake at 1,
  and the front tires' braking slowed the creep from 0.06 to 0.05 ft a tick.
- **The fix is in the test, not the port.** The new helper `controls_off_the_save_state`
  (`tests/replay.rs`) sets the flown flag, the five pad fields and the eight surfaces from the image
  before `remeasure`. The offsets come from oracle.txt R8 (biplane_input, biplane_controls and
  biplane_autopilot) and from watch/biplane.toml's `plane_flown` and `pitch_int`. `plane_at` gains
  `FLOWN` (0x334) and `PITCH_INTEGRAL` (0x584). The S252 test calls it, and the other
  `off_the_save_state` callers are unchanged.
- **Result.** From 11073 to 11090, the port's own plane's `body_pos` equals the capture's row
  **bit for bit** on every tick, and the test now asserts it. The free run's truck wakes on 11092,
  the original's tick, at the original's point, as the pinned run already did. The test's assertion
  that the free run wakes two ticks late is now an assertion that it wakes on the same tick.

## How it was checked

- `cargo test --release -p game-sim --test replay a_parked_plane_wakes`: passes. The free run's
  per-tick position assertion covers ticks 1..18.
- `cargo test --release -p game-sim --test replay`: 118 passed, 1 ignored (the same one as before).
- `./kit audit`: only the two plan/DONE.md leaks that are already in HEAD.

## Not verified / not 1:1

- **The tick of the push itself (11092) is not held.** The truck's push on the plane, 0.07 ft off,
  is S251's f.col ground, as in S252.
- **The engine's derived words** (power, rpm, thrust, note) are not read from the image. Each
  substep recomputes them from the throttle, and the bit-exact rows show they do not matter here.
- **No port change.** Nothing in `game_sim` sets `attitude.flown` from a seat; the shell does. A
  parked, unflown plane in the port has the brake on, as the original's controls do for an unflown
  plane. This capture's plane was flown with no pad, which is not that case.
- Nothing taken from opentennis: it has no counterpart.
