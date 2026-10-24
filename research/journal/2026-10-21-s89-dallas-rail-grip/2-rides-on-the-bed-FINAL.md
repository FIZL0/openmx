# 2 — rides on the bed, captured and replayed (FINAL)

Task S89, reopened by the user on 2026-10-22: "do what can be done now, the parts that wait on another
task go to it". 2026-10-23.

## What I did

- **Two save states on the railway** in PCSX2 copy 3, both free06 on the 250 (Race250A.phy, the
  250_motoxxx model):
  - The base state is the biplane state switched to the bike from the pause menu ("Switch To Machine:
    Bike": d-pad down ×3, then cross).
  - A PINE script, paused over AT-SPI, finds the human's actors by control source 1. It shifts every
    copy of the bike's position by one delta, zeroes the chassis and vehicle velocities, holds the
    brake until 3 s before the end, lets the bike settle and saves the state.
    - **along**: on the top of the +z straight at (4591.5, 135.46, 2899.7), facing +z (5° off).
    - **across**: at (3900, 136.7, 4560), facing +z, 47 ft short of the z≈4607 straight.
  - The bed's straights, read off the disc:
    - x 4579–4603, z 2829–4330;
    - z≈2318, x 2546–3595 (a train stands near x≈2900; my first crossing landed on it, so I moved
      to the north straight);
    - z 4595–4619, x 3493–4344;
    - x≈2314, z 2506–3438.
- **Two captures with `./kit record`**, with `watch/s89-rails.toml`: s64-wheels' fields, plus the
  rider chain for this state, acceptance's pad words, and each wheel's swept contact (`+0xd4`) and
  surface byte (`+0x110`).
  - Pads: `watch/pads/s89-along.pad` and `watch/pads/s89-across.pad`.
  - **along** (600 rows): gas, then the brake, then gas with a second of half-left stick at about
    70 ft/s (the turn on the rails), drifting off the bed at row 107.
  - **across** (300 rows): gas over the bed at about 50 ft/s, then the brake.
  - The captures (context/capture/s89/) and the RAM images (context/states/s89-*.eeMemory.bin) are
    git-ignored and live only in s3.
- **The test** `tests/replay.rs` `the_dallas_rail_rides_replay_from_their_save_states`:
  - Asserts the original's behaviour off its own rows.
  - Seeds the port from the RAM image the way S64 does: chassis, wheels, balance, engine outputs,
    air and drivetrain.
  - Replays the captured pads in the free06 world (terrain plus all 353 collision meshes).
  - Compares lane by lane, and prints both sides' side speed, ray, sweep and surface every ten rows.
  - New helpers, all delegations that leave the old callers unchanged: `the_bike_named`,
    `the_wheels_of`, `in_the_world_on`, `SaveState::open_image`.

## What the original does on the bed

Read off the running game's rows:

- **The wheels stand on it through the swept contact alone**: ray_hit 0 and swept 1 on both wheels,
  along rows 0–107 and across 55–58.
- **The surface byte is the terrain's 0, never the bed's 1.** This confirms entry 1's reading of
  0x1dc960/0x25b250 against the live game, which entry 1 could not do. The test asserts 0 or 12 on
  every bed row.
- **It does not slide on the top.** Speed across the heading stays under 0.04 ft/s through row 90,
  while the bike speeds up to about 63 ft/s along it. The most is 1.24, along at 67 ft/s, at rows
  94–102, where the drifting bike rocks over the edge onto the shoulder. Across, it reaches 0.51 on
  the bed.
  - At 50 ft/s across, the front wheel takes a swept contact on the near shoulder and the bike leaves
    the 0.94 lip airborne.

## What the port does, from the same state and pads

- **Wheel mounts**: identical to the image's `+0xa0` on both wheels.
- **On the bed as long as the original, the same way**: along 105 ticks against 108 rows, across 4
  against 4. It rides through the sweep alone, on surface 0. The test asserts the count, the surface
  and the side speed.
- **No slide**: at most 1.34 ft/s along (at the same rock onto the shoulder) and 0.96 across. On the
  top it stays within 0.011 of the original's side speed. The slide the user saw is gone (entry 1's
  `belly_reach` fix).
- **The wheels' slip flags against the original's** (ratchet `S89_LANES_AGREE_THROUGH`; the test fails
  if any lane parts earlier):

  | lane | along parts at | across parts at |
  |---|---|---|
  | front / rear `sliding` | never | never |
  | front `spinning` | never | never |
  | rear `spinning` | 32 (off the bed, on the gas) | 17 (before the bed) |
  | front / rear `locked` | 280 / 278 (on the brake, off the bed) | never |
  | front / rear ray_hit and swept | 106 / 108 (dropping off the bed's edge) | 50 / 53 (the port takes the shoulder's sweep a few ticks early) |
  | front / rear surface | 165 / 167 | 60 / 63 |

- **Bit-exact it is not.** The chassis parts at tick 1 along: vel by about 1e-7, the angular velocity
  by up to 70% of a 1e-5 value. Across, it parts at tick 2. This is S107's tick-1 problem (every lane
  agrees at tick 0), which happens on bare terrain too and has nothing to do with the bed.
  `S89_AGREES_THROUGH` is `[0, 1]`.
- I proved the ratchet runs by planting front `locked` along at 280 for one run. It failed: "agrees
  through tick 279, was 280".
- `./kit check --all`: run by `./kit done` as the gate.

## Handed on

- **S107** (note added): the bit-exact acceptance. Once tick 1 agrees, raise `S89_AGREES_THROUGH`.
- **S112** (new, after S107): replace the ride's collision bundle and `belly_reach` with the original's
  collision search, then rerun these rides. The contact lanes part where the wheels go between sweep
  and ray at the bed's edges.

## Not verified / not 1:1

- **The acceptance's "bit-exact through the contact" is not met.** Both rides part at tick 1–2 for
  S107's reason, before the bed is reached. Handed to S107.
- **The wheels' slip agrees only flag by flag** (spinning, locked, sliding), not in the slip values or
  tire forces. The float lanes part at tick 1 with the chassis, so they cannot be compared until S107
  lands.
- **`belly_reach` and the four-probe bundle are still the port's own.** The contact lanes agreeing
  through the bed is evidence for them, not proof. Handed to S112.
- **Only one bike (the 250), one speed range, and two of the four straights.** There is no ride on
  the shoulder slope alone, and none at the 125's tire table that entry 1 read.
- **The "turn on the rails"** is half-left stick at about 70 ft/s. It carried the bike off the bed
  (row 107) instead of turning on it, so a turn on the top was not captured.
- **The states were teleported**, not ridden to. Every copy of the position within 12 ft was moved
  by one delta, and the velocities were zeroed. The game settled for seconds before each save, and the
  test seeds from the saved image, so this does not reach the replay. But nothing proves the moved
  state equals one ridden to the same spot.
- **The captures and states exist only in s3's context/** (git-ignored). Other slots skip the test
  with a "no …csv" line, and `watch/s89-rails.toml` says how to remake them.
