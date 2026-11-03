# 1 — the strike, the wreck and the put-back (FINAL)

Task C19. 2026-11-03.

## The captures

- **C17's capture** (in s3's context): `capture/c17/strike.csv`. It holds frames 1204..1502 of
  `recordings/c17/strike.p2m2`, played from C14's state.
- **C19's capture**: `capture/c19/probe.csv` (and `.json`). It is the same tape on PCSX2 copy 3, read
  with `watch/c19-kiowa-strike.toml` through `research/c19/capture.sh`.
  - The spec is `c14-kiowa.toml` plus these fields:
    - the rotor probe's flags (probe `+0x114`);
    - its ray record (probe `+0x6c`): `+0x8c` the count, `+0x9c` the objects, `+0x98` the points,
      `+0xac` the segments;
    - the struck object's category (`+0x84`) and its plant position (`+0x1d0`).
  - Every column the two captures share is the same on all 299 rows except 1204, the first row,
    whose command words lag. So C17's run was not torn where it matters.
- **What the record shows.**
  - Rows 1295–1297: the count is 0 and flag bit 0x200000 is down.
  - Row 1298: the flag goes up (0x320000).
    - The count is 1. `objects[0]` is 0xdec610, category 4 (a trunk).
    - The plant stands at (3007.3359, 126.7734, 4049.6836).
    - The hit point is (3007.8188, 144.10242, 4051.0874).
  - The record is not searched again while the helicopter is wrecked: it keeps the same values to
    the put-back.

## The port

- `Collision::rotor_first_hit` (`crates/game-sim/src/kiowa.rs`) is `rotor_struck`'s search,
  returning the first hit. `rotor_struck` now calls it, with no change in behaviour.
- In the test, `Struck`'s `strike` keeps that hit.
  - On 1298 the port's first hit is tree 138's trunk.
  - Its point is bit-equal to the record's first point.
- `Struck` wraps C14a's `Grounded`. Its `crashed` builds `Throw::of` from `kiowa.psg`'s nodes at the
  frame's rotor angle, then calls `Wreckage::act`.
  - The `Wreckage` is built from the image: the built inertia `+0x410`, the place `+0xe0`, the spawn
    `+0xcd0`, and owner 0.
  - The blades come from the disc's `wreckblade.phy`, `bladebentm.psg`/`.ray` and
    `bladebentf.col`, with the first wrapper's edge.
- The blades run in the loop as `KiowaRide::step` runs them: search and step before the
  helicopter's tick, `post` after it. The put-back's `put_back_controls` and refresh come after the
  tick.

## The generator

These are draws per row, counted by walking the 64-bit LCG from each row's `rng_state` to the next
row's.

| Rows | Draws per row | What they are |
| --- | --- | --- |
| 1205–1297 | 2 (18 on 1237, 3 on 1268) | Not the port's frame: it draws nothing in flight, which C14 established. |
| 1298 | exactly 12 | The four blades' three each (`wreck_mx`'s throw). The flight's 2 are not drawn that row. |
| 1299–1309 (the fall) | 0 | |
| 1310–1448 (landed and sliding) | 0 to 48, mostly multiples of 4 | Likely the four blades' ground effects or sounds. The port's frame does not draw them. |
| 1449 | 0 | The put-back. |
| 1450 on | 2 again | |

The test asserts that the port's frame draws only on 1298.

## What does not replay: the wrecked fall

- **1299.** The original's body takes a sideways push the port's does not.
  - last_force: game (−19.71, −4112.52, −135.55), port (0, −4110.0, 0).
  - last_moment: game (186.96, 2065.43, 713.22), port 0.
  - Velocity is ~1 ft/s, so this is not drag.
  - From 1299 on, the x/z rates flip sign tick to tick (+0.036, −0.035): contact jitter.
  - The helicopter's own touching flag (C14's `pilot_flag` row and the crash test's record) is 0
    on 1299.
- **The likely cause** is the four thrown blades. They start at the rotor hub and touch the hull
  through shared contacts that push both bodies.
- **Not confirmed.** The spec's contact fields assume the collision object is at heli `+0x170` (and a
  blade wrapper's at `+0x120`). The capture shows those are wrong: the "count" words are pointers
  and the flags are always 0. The spec says so next to those fields.
- **The port has no such contact.** `Blades::search` offers the blades only the terrain, the props
  and the trees, never the helicopter's mesh. The blades also step apart from the helicopter.
  `prop_world::Collection::search_among` / `step_among` (S197/S243) is the machinery a port would
  use.
- **The put-back (1449).** The original places the body and then steps it once more in the same
  frame:
  - its 1449 row is (3025.993, 132.645, 4053.999), velocity (−0.388, −0.158, 0.000);
  - the port's body is still, at (3026, 132.647, 4054).
  - That step is pushed by the contacts left from the slide, so it depends on the fall too. The
    test checks the place to 0.01 ft and the quaternion to 2e-4.
  - The controller's state after the put-back (e.g. the collective, 0.292 game vs. 0.307 port) also
    carries the fall's history. So the rows from 1449 are not compared.

## How it was checked

- `./kit check -- -p game-sim --test kiowa_capture -- a_captured_rotor_strike --nocapture` passes.
- A temporary panic in place of the final `println!` showed the test runs and does not skip. Its
  log printed:
  - "tick 1298: the rotor probe struck tree 138 at [3007.8188, 144.10242, 4051.0874]";
  - "1205..=1502 … 4 blades thrown on the original's 12 draws, put back on 1449".
- Comparing every row after 1298 fails on 1299's last_force[0] (above).
- Comparing from 1449 on fails on the collective.
