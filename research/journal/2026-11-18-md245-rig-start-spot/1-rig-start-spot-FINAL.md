# 1 — rig start spot (FINAL)

Task MD245. 2026-11-18.

## What I did

- The fix was in the test, not the rig. `the_quad_rigged_from_its_files_is_md123as` (tests/atv_quad/mxu.rs)
  built the quad from its files at the image's quad +0x50, the spot it had settled to. MD134 showed the part
  reader reads the box at the start spot the quad was made at. The test now passes that spot.
- MD37a's `atv-r-spawn.ee.bin` keeps the start spot at 0x668e70 (point, then the 0.96/0/-0.3 heading), not at
  C28's 0x669e40. Same point bit for bit as C28's; found by searching the image for C28's 12 bytes.
- The 1e-5 relative bound on the record's inverse inertia is now a bit-for-bit assert. The doc comment no
  longer calls it one float off.
- quad.rs is unchanged: `Load::read_model` was already right since MD134; only the caller's spot was wrong.

## What the original does

- As MD134 found (research/journal/2026-11-17-md134-atv-model-load): the load path re-places the node at the
  start spot with the kept heading made unit, then the part reader reads the chassis box in that frame.

## How it was proved

- Save-state image: MD37a's `atv-r-spawn.ee.bin`. With the start spot the record's inverse inertia is the
  image's bit for bit (was 7.9e-5 relative off), and every other compared group still matches.
- `./kit check -- -p game-sim --test atv_quad mxu::` passes (11 tests).

## Not verified / not 1:1

- 0x668e70 being this image's factory descriptor: it is heap RAM, found by value, not traced. Same caveat as
  MD134's 0x669e40. Its point equals C28's start spot bit for bit and the box it gives makes the record exact.
- The test's other rigs (rays, tail, wreck) are now built at the start spot rather than the settled one. They
  were and are compared bit for bit and still match, so they do not depend on the spot.

## Next

- Nothing for this task.
