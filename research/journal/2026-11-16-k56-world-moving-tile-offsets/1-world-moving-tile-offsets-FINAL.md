# 1 — world moving tile offsets (FINAL)

Task K56. 2026-11-16.

## What I did

- `-p openmx --test world_moving` failed in all three tests: each bike ended far from the place it aimed at
  (the train test 297 ft past the line).
- The cause is data, not a code merge. MD103 (today) reinstalled the main checkout's
  `mods/tracks/world_mxu_free` from openmx's disc build. That build puts every tile 720 ft lower in x and z
  (free06 at (-576, -720), it was at (144, 0); free09 at (15840, -720), it was at (16560, 0)). The test
  hard-coded the old offsets, so it started its rides in the wrong places.
- I skipped the bisect: no code merge moved the world, and the old mod.json (kept in
  `~/repos/openmx/context/md103-old/world_mxu_free`) gives the same rides (below).
- Fix, in the test only: `tile_at` reads each tile's `at` from the installed mod.json. The rides now take a
  tile name and a place in that tile's own feet. The truck test's start "3950,2960" became free06's (3806, 2960),
  which is the same place on the old world.
- No other test hard-codes these offsets (grep for 16560, 11088, 22032, FREE06_X).
- Nothing taken from ../opentennis: it has no worlds.

## How it was proved

- `./kit check -- -p openmx --test world_moving`: 3 passed.
- Ran the test binary with `--nocapture`: no `skipped`, and the truck woke (`ctruck body 3371.8,2221.3`).
- Same binary with `OPENMX_WORLDS` set to a scratch folder holding the pre-MD103 mod.json and joins (tiles linked
  from the install): 3 passed, truck at `4091.8,2941.3`. That is the same ride, shifted by exactly 720 ft. So
  the reinstall's offsets were the only change.

## Not verified / not 1:1

- No bisect over merges. The world data changed outside git (in the gitignored `mods/`), so there was no code
  merge to find. The ride on the old world shows that the move was the whole cause.

## Next

none
