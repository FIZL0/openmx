# 1 — world parked plane (FINAL)

Task MD32. 2026-10-27.

## What I did

- **Found the fix already on main.** MD28's journal (written 2026-10-25) filed this when a world's
  parked plane still stood at the home tile's machine.txt spot in that track's own feet. MD24
  (merged 2026-10-26 06:09, after that note) had already fixed it: `vehicle::world_spots` moves the
  home tile's spots as one so `[Biplane1]` stands one spacing right of the world's spawn, and every
  seat's bay and online bay is parked around those moved spots. Measured on the user's worlds
  (`--frames 1`, release build):
  - world_mxu: spawn on mxu_nat17 at 16920, 120, 22680; plane parked at 16880, 210, 22680 (dirt 209.7).
  - world_ultra: spawn on mxvatv_nat06 at 59081, 368, 22882; plane at 59116, 368, 22901.
  - world_untamed: no machine.txt, so no parked copy; the plane starts by the start line (P113).
  So no placement code changed.
- **One shell line added** (`main.rs`, seats loop): `player N: biplane parked at x, y, z` for every
  seat after the first with a bay, as the first seat's `biplane: parked by ...` line already says, so
  a test can see seat 2's plane as well.
- **The test the brief asks for**, `crates/openmx/tests/world_parked_plane.rs`: a generated two-tile
  world, tile `a` at the origin and tile `b` at 6000, 4000 (2862 ft a side), the ride spawned on and
  lit by `b`, whose scene is stemmed `FREE06` so the disc's `tracks\free06\machine.txt` parks the lot.
  Two players. Both seats' planes must stand inside `b`'s rectangle and 0–150 ft over its ground.
  Result: start 6570, 40, 4570; planes 6570, 135, 4610 and 6552, 135, 4633.
- Nothing taken from opentennis: it has no worlds or parked machines.

## What the original does

Nothing to port. The original has no worlds; where a world parks the plane is the port's own (MD24).

## How it was proved

- The new test passes. With the world move turned off by hand (`lot_spots` set to the file's own
  spots), it fails: "seat 1's plane at [2304.0, 135.0, 2281.0] is outside b's 6000..8862 x
  4000..6862". So it catches the bug the brief describes. The change was then put back.
- The three real worlds above were looked at by their load lines only, not in a picture.

## Not verified / not 1:1

- **A home tile whose terrain doesn't cover its machine.txt spots parks the plane about a million
  feet up.** `world_spots` measures each spot's clearance over the home ground at the file's own
  place, and off the terrain the probe answers about −1e6. Seen on this test's first try, with a
  1143-ft tile under free06's 2304, 2281 spot. No real world hits it (their home tiles are the disc
  maps themselves), so the test tile was made big enough, and the fix is filed as MD55 rather than
  guessed here.
- **The test's height check is loose (0–150 ft over the ground).** The plane keeps the clearance
  free06's file gives it over free06's own hills (95 ft here over the flat test ground), not a
  plane-on-its-wheels height. That is MD24's design and isn't checked further.
- **Not checked clear of props** in a real world (as MD24 already notes). The spot is the port's own.
- **Not looked at in a picture.** The plane's position was read from load lines only.

## Next

Nothing for MD32. MD55 is the follow-up.
