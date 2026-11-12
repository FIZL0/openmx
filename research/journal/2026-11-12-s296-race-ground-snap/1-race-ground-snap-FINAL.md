# 1 — race ground snap (FINAL)

Task S296. 2026-11-12.

## What I did

- Ported the race activity load's ground snap as `race::TrackFile::snap_to_ground(terrain)`
  (crates/game-sim/src/race.rs). It drops every node's three points to height 0, takes each one to the
  terrain with a floor at 0, then takes the line's centre and the grid gate's centre to the terrain. It hands back the
  nodes the original goes on to raise onto the props (`word_04 & 0x80`). That raise is **not** ported
  (filed as S296a).
- Three tests in crates/game-sim/tests/race.rs:
  - `sx01s_race_file_stood_on_its_terrain_is_the_track_object_in_the_image`: sx01.tdf, snapped on
    sx01's own terrain (Tracks\sx01.res's .scn and .ps2.trn through `world::Ground`), against S196's
    image.
  - `the_race_file_stands_on_the_ground_as_the_race_activitys_load_puts_it`: an Ee oracle of the
    whole activity load.
  - `the_race_files_that_ask_for_the_prop_raise`: a scan of every race file on the disc.
- New oracle.txt names after `activity_race_load`: race_prop_raise, res_find,
  track_load, sweep_probe_ctor, sweep_probe_setup, world_layer, world_add_probe, search_setup,
  search_free. The snap itself stays a `#` comment there: as a name it would make S310's plan line
  (the same snap, filed from the AI side) an audit leak.
- Nothing taken from opentennis (it has no race files).

## What the original does

`./kit fn 0x2f8610` (activity_race_load) reads the track object, then calls `./kit fn 0x2f8b40`
(activity, track):

- For each path slot (no null check: an empty slot would read address 0x10), it walks the node chain
  (head `+0x10`, next `+0x54`). It stores 0 into each point's y (+0x14/+0x24/+0x34), and if the world
  (act+0x180) has a terrain (+0x428) it runs ground_query on each point and floors y at 0 (`c.lt.s`
  against 0.0).
- A node with `+4 & 0x80` gets, per point, a sweep probe 1000 long that starts at (x, y + 900, z),
  then `./kit fn 0x2f8820`. That function asks the search grid for nearby objects. A mesh prop (type
  2) not masked by two named classes is swept, and `*y = max(*y, hit y)`. The `&y` it is handed is
  `node + 0x14` on all three calls (`./kit dis 0x2f8dd8`, `addiu $10, $19, 20` each time), so all
  three sweeps can only lift the **first** point.
- Back in the load: the track's +0x38/+0x3c are set to 125/75 (already in `TrackPaths`'
  `ahead`/`behind`). Then ground_query runs on the line's centre (+0x5c gate +0x40) and the grid
  gate's (+0x58), with no floor and no terrain check.
- The second file (act+0x288, the AI net) is snapped the same way.

## How it was proved

- **Save-state capture**: sx01's 96 nodes, all 288 points with all four lanes, are bit for bit the
  image's track object after the snap on sx01's terrain off the disc. No sx01 node has 0x80, and the
  third gate stays as read.
- **Oracle, N = 10,000 generated tracks** through the whole of activity_race_load in Ee. The real
  snap runs; ground_query is hooked to answer from a random list (heights including −0, +0,
  denormals and negatives, or "no ground"). Compared: every point's four lanes, the line and grid
  centres, the points asked about and their order, the nodes the props raise, and for each raise the
  sweep's start (x, y+900, z) and the height pointer.
  All 10,000 match. The test asserts more than 60,000 points, more than 3,000 negative answers,
  more than 1,000 raised nodes and more than 1,000 tracks with no terrain (the exact counts are not
  printed on a pass). Its first run caught a real mismatch: the original leaves a negative denormal
  height (−9.4e-44) where it is, because `c.lt.s` reads it as zero, and the port floored it. The
  port now reads the height as an FPU operand (`F::from_bits`). `F`'s own comparisons don't read
  denormals as zero, although its doc says they do; filed as F63.
- **Disc scan**: of the 102 race files in the tracks' `_2.res` archives, those with nodes the props raise are nat03.tdf,
  nat03ai.tdf, sx06, sx08, sx11 and sx20. The test pins that list.

## Not verified / not 1:1

- **The prop raise is not ported** (S296a). nat03 (both files), sx06, sx08, sx11 and sx20 have nodes
  that need it, so on those tracks a raised node's first point stays on the terrain in the port.
  `snap_to_ground` hands those nodes back so a caller can see the gap.
- **The snapped gate centres are not seen on an image.** On sx01 the supercross loader moves the line
  and the grid gate onto their props afterwards. The oracle proves the snap with a hooked ground
  query, not with the real terrain.
- **ground_query itself is the port's `world::Ground`** (proved by S64 on the terrain). Here it is proved
  only through sx01's 288 points, which all land on the field at about 90. The off-field case (the
  original leaves the point; the port asks `World::keeps`) and the floor at 0 are proved only by the
  oracle's hooked answers, never against a real terrain.
- **With no terrain** the original still runs ground_query on the two gate centres, through a null
  terrain (it reads +0x8a8 off address 0). The port leaves the gates as read. The oracle checks only
  that the original made those two calls.
- **An empty path slot** reads address 0x10 in the original. The port skips it, and the oracle
  generates none.
- **The second (AI) file's snap** runs in the original. The port has no reader for that file yet
  (S297), so it is not called on it.

## Next

S296a: the prop raise. S310 (noted): call `snap_to_ground` on the AI net's file as well, and its
text's claim that every node is rayed is wrong.
