# 1 — c28 quad images (FINAL)

Task MD116. 2026-11-17.

## What I did

- `tests/atv_quad.rs` `the_quad_images_wheels_are_their_phy_files` no longer pins the quad's address. A new
  `quads_in` scans the RAM image for every object whose word at `+4` is the quad vtable (`0x003ce250`).
- Every image holds two quads: the player's (lowest, made first) and an idle one.
  - MD37a `atv-r-spawn.ee.bin`: `0x15e0690`, `0x1657090`.
  - C28 `c28-atv-{r,s}-{spawn,air}.ee.bin`: `0x150a990`, `0x1581a70` in all four.
- The test now checks every quad, not just the player's: the player's must match exactly one file of its
  image's kind (`atv_` for r, `atv_s_` for s); the idle one exactly one of all eight.
- `QUAD_IMAGES` now lists MD37a's `atv-r-spawn` and C28's four. Dropped MD37a's `atv-r-air`,
  `atv-r-spawn-late`, `atv-s-spawn`, `atv-s-air`, `atv-s-spawn-late`: their `.ee.bin` exist neither in the
  shared context nor in `~/repos/openmx-slots/kept-context/md37a`, so they only ever skipped.
- Filed MD116a for `atv_gather` / `atv_search` / `atv_spatial`, which pin the quad, its tree and the other
  vehicles (not in this task's files).
- Nothing taken from ../opentennis (no ATV images there).

## What the original does

Nothing new: the quad's tire words, read by the ported `.phy` tire reader (MD64b).

## How it was proved

Ran the test with a temporary panic listing what it saw (then removed): all ten quads matched one file each.

- `atv-r-spawn` both quads, `c28-atv-r-spawn` and `c28-atv-r-air` both quads: `atv_race250p`.
- `c28-atv-s-spawn` and `c28-atv-s-air` both quads: `atv_s_race250p`. So the idle quad is the player's kind
  (the poke changes the type name both are made from).

## Not verified / not 1:1

- That the lowest quad is the player's is read from MD37a's notes (oracle.txt: player `0x015e0690`, idle
  `0x01657090`) and C28's (player `0x150a990`), not from a pointer the game keeps. The test does not depend
  on it beyond the player-kind check, and both quads match the same file anyway.
- atv_gather / atv_search / atv_spatial still read only MD37a's image (MD116a).

## Next

MD116a.
