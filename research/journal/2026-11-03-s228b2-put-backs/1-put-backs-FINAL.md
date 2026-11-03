# 1 — the put-back resets less, and not to zero (FINAL)

Task S228b2. 2026-11-03.

## The spot

- `Image::rest` now seeds the put-back record (`Ride::back`) from rider `+0x3b0` (point), `+0x3c0`
  (heading) and `+0x3d0` (up). Before this the record was the port's own and lagged the image.
- The original's chain is `player_respawn` → `bike_unwreck` 0x00252ee0 → vt+0x138 0x0027b650 →
  `ghost_start` 0x00250580. ghost_start sets the node's rows with `FUN_001b1e20(node, 0, +0x3c0,
  +0x3d0, 1, 0, 0)`, which is the port's `math::basis(.., true, false)`. In the oracle, its rows equal the
  port's bit for bit at all three put-backs.
- In `ride()`, the chord (L1 0x4 / R1 0x8, one pressed with the other held) is read from tape field
  `t + 1`, a field ahead of the pad's mapping. That puts the put-back on the tick whose capture row
  shows it. The row after it is read mid-put-back: velocity 0, centre still the one before.
  `held` takes that row as `placed`.

## What the put-back writes (oracle, 0x0027b650 from the images at 138610, 139110, 139710, 136510)

All writes to the chassis, both wheels and the rider were listed, before and after:

- **Chassis.** `+0x70`, `+0x80`, `+0x130..+0x160`, `+0x260`, `+0x270`, `+0x290` and the integrator
  histories are set. The vectors become (0, 0, 0, 1). The orientation's history (`+0x2f0` dx, `+0x300`
  prev) becomes (0, 0, 0, 1) too, not zeroes.
  The quaternion `+0x1a0`, the centre `+0x60`, the angles `+0x90` and the world angular velocity `+0x280`
  are **not** written.
- **Rider.** `+0x574` on_ground = 1, `+0x57c` airborne = 0, `+0x600` crossup = 0, `+0x788` = 0.5 (the
  drawn crossup), ghost clock `+0x7dc`, `+0x5e0`, and the rider's copies at `+0x820..+0x948` and
  `+0xde0..+0xe84`. Nothing else in the air state is written.
- **Each wheel.** `+0xd0`, `+0xd4`, `+0xf0`, `+0xf8` = 0; `+0x130` and `+0x200` normals = (0, 1, 0, 1);
  `+0x140` load point, `+0x170`, `+0x180`, `+0x190`, `+0x840`, `+0x850` = (0, 0, 0, 1); `+0x1d4` spin and
  `+0x210` drive torque = 0; `+0x224`, `+0x228` = 0 (not kept in the port). `+0xf4` last_compression and
  `+0xdc` contact are not written.

## How the yaw was found

With the centre matching, the put-back tick still came out about −0.0127 rad off in yaw. I ran
`chassis_integrate` 0x0015c540 alone after the put-back in the oracle. It turned the body away from the
node's quaternion by 0.011–0.016 rad about the body's axis, with every rate zero. The fold
(`m * conj(q)`, then `d * q`) is the port's; I tried all eight product orders offline and none gave the
result. The cause was the Adams step: `x += dt/2 · (3·dx − prev)` with `prev.w = 1` takes `dt/2` off `w`,
and the normalise scales the vector part up. Setting the history to (0, 0, 0, 1) in `place_back` made the
port's yaw match the original's to 3e-7.

Then the next tick's air control added an unroll torque (z 4.38) in the port and none in the original.
The port's put-back had reset `Air` to default (on_ground false). With only the three words written,
the put-back tick's end equals the capture bit for bit at 139141→row 139143: euler, centre, velocity
and rates.

The tick after that still parted, in the wheel torque, because the port left the wheels' contact words
alone. With `Wheel::put_back` clearing what the original clears, all three stretches hold to 100.

## What changed

- `Ride::place_back` keeps the ground step's words, the air state (but for the three), and the chassis
  quaternion, centre, angles and world angular velocity. It sets the orientation history to
  (0, 0, 0, 1) and puts both wheels back (`ride::Wheel::put_back`). Dropping the centre or angle keep
  takes 138610 back to 76, so both are needed.
- `Rider::wrap_round` asks a riding chassis for `transform · cofg` instead of the centre word. In
  wrap_rest's seam test, the stale centre of a wreck carried over the seam is on the far side and lapped the
  freshly placed bike back out. That test now reads the transform's origin, the spot itself.
- HELD: 138610, 139110 and 139710 now hold to 100. 139510 holds 1, its first row. DRAWN_TICKS is
  (1952, 274), and the hard-turn lean ratio is 0.795 (the pin is ≥ 0.79).
- platforms.rs vectors were regenerated and spliced const by const: HASHES, RIDERS_HASHES, RIDERS_DIGEST,
  RIDERS_WIRE_DIGEST, TRACE_DIGEST, POS_Y, VEL_Y, FINAL_DUMP_HEAD, FINAL_DUMP_DIGEST and
  FIXTURE_TEXT_DIGEST. The canonical run respawns. The leaf count and the type hash did not move.

## Checked

- `cargo test --release -p game-sim --test fly_and_collide`: 3 passed, none skipped (the stretches ran
  and printed their GOT list while I worked).
- `./kit check -- -p game-sim --no-fail-fast`: TESTS PASSED, 1113 tests.
- `./kit check -- --no-fail-fast` (whole workspace): one failure, openmx's
  `original_on_sets_every_sub_switch_and_one_turned_back_leaves_the_rest` (menu.rs:5422, Fog
  "Whole Map" vs "Farther Fog"). That is P268/P259's menu, not game-sim.
