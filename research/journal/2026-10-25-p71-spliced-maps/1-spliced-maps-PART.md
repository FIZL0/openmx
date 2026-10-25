# 1 — spliced maps (PART)

Task P71. 2026-10-25.

## What I did

- `--track free_all` ("Every free ride", crates/openmx/src/spliced.rs) is a world (MD8) built in
  code off the disc, with no mod folder: the five freeride maps in menu order, west to east —
  free06, free07, free09, free10, free11. It rides on the existing world machinery (tiles, joins,
  `Tiled`, `off_world`, world_dress, the per-tile look and sky), so nothing in game-sim changed.
- Probe first (a scratch test, deleted): all five maps are one layout — 3×3 terrain, 768×768
  samples at 9 ft, 6903 ft across — and all have the same ring: a plateau at about 190 ft out to
  the map's edge, a ridge inside it (west peak about sample 84–88, ~260 ft; east peak about sample
  680, ~252 ft), the valley the map is ridden in inside that.
- The cut (the user's MD27 wish, "right at the peak of mountains"): each map's keep ends at the
  peak column of the ridge it shares with its neighbour. The peak is the column with the highest
  mean height over the middle half of the map's depth, found per map from its own ground. The outer
  sides of free06 and free11 and every map's north and south side keep all their ground, so the
  outer shootoff is the original's own plateau and margin.
- Between two maps a generated join strip one leaf wide (15 cells, 135 ft) runs from the west
  map's peak column to the east map's peak column, the height a straight line across the two peak
  profiles sample by sample, the surface the nearer map's. Each seam is 4 joins along z (16 leaves,
  16, 16 and the last shifted back to end on the map's last cell), 16 joins in all.
- Layout as the load prints it: free06 at 0 keeps x 0..6129; free07 at 5481 keeps 6264..11610;
  free09 at 10962, 11745..17091; free10 at 16443, 17226..22572; free11 at 21924, 22707..28827; every
  map z 0..6903. The world is 28827 × 6903 ft, and `world holes: none, so only the outer edge throws`.
- Objects: each map's collision shapes and models are kept only if they stand on ground the map
  still owns (the cut ridge's far side and its plateau go). Props, eco and traffic are dressed per
  map by world_dress as for any world.
- Emitters: `Sound::set_map_emitters` reads every map's own `.aud` fixed emitters, moves each by its
  map's offset and keeps those on owned ground. The freeride maps hold almost none: 0, 0, 0, 1 and 0
  (free10's one now sounds at its spot). Random ambience and the crowd stay the home map's (free06):
  they are not placed, and five maps' worth at once would play each five times as often.
- Ground and sky: each map is painted by its own terrain textures and splat as MD9/MD31 do for
  any world tile; the sky and fog are the home map's (free06), the world's `lit by`.
- To make a world tile come from the disc and not a folder, `WorldPiece` got an `origin` (Folder,
  Disc, or Made bytes for a generated join) with `archives()`, `res()` and `terrain()`; worlds.rs
  and world_dress.rs ask those instead of `Res::folder(dir)`. Folder worlds behave as before.
- Follow-up filed: P149, a row and card for free_all in the front end's track pick.

## What the original does

Nothing like it: the original rides one map at a time. Its shootoff is the per-map margin test S59
ported (`off_map`), which the world asks of the whole world's rectangle and of holes inside it.

## How it was proved

crates/openmx/tests/spliced_maps.rs, on the disc (it skips without it):

- `a_ride_crosses_every_seam_with_no_shootoff`: 16 rides on the gas, every seam both ways at 1/4
  and 1/2 of the depth, from 150 ft short of the join strip: none thrown, and each past 150 ft on
  the far side before any wreck (ticks 314–334 of 600). The bike climbs the ridge, rides its top
  across the seam (y ~254 at the peak) and down; on the steep way down it wrecks at the foot, well
  past the seam — a ridden wreck, not a throw.
- `riding_off_the_outer_edge_shoots_you_off`: from 400 ft inside each of the four sides: thrown on
  tick 112 at x 148.9, x 28677.3, z 149.0, z 6753.8 — each the side's 150 ft margin.
- Both made to fail once on purpose to see they ran and did not skip (a pass shows no log).
- `./kit check --all` (through `./kit done`, 2026-10-25): red on one test that is not P71's,
  `game-sim --test replay` `the_dallas_rail_rides_replay_from_their_save_states` (`along:
  rider.ride.front.slip.locked agrees through tick 278, was 279`). It fails the same way on bare
  HEAD ecd23368 in a scratch worktree with s3's context linked in; it is S120 (filed by MD27), and
  only s3's context holds S89's capture, so other slots skip it. Nothing else was red (pace's 60 fps
  test passed its lone rerun, a known load flake).
- A windowed 5 s run with sound: `sound: 1 fixed emitters on 5 maps (free06 0, free07 0, free09 0,
  free10 1, free11 0)`.

## Not verified / not 1:1

- The join ground is made, not the original's: a straight line between the two peak profiles in
  host floats, quantised to the .trn's 1/64 ft. Its meeting with each map's own peak column was
  ridden over (no bump that wrecked or threw) but not measured sample by sample.
- The peak column is chosen by mean height over the middle half of the depth; near the north and
  south ends the ridge may curve away from that column, so the seam there may sit off the true peak.
  Not looked at in a picture (no `./kit shot` of a seam was taken).
- The ring "first difference" probe showed small height differences between the maps at the very
  edge (d = 0); not investigated, since the outer edges keep each map's own ground.
- The world's far x and z edges are at 6903 ft past their map's origin, not the 6912 the original's
  margin test uses for one map, so a ride off the east, north or south side is thrown 9 ft earlier
  than on the map alone. Making the area 6912 would turn the 6903..6912 strip into a hole.
- The throw aims at the world's middle (MD8's choice), not at the map's own middle as the original's.
- Only free06's hit.dat zones (the home map's) are read; the other maps' zone spots are not placed.
- Random ambience and the crowd are only free06's (see above); the other maps' are left out.
- Sky, fog and light are free06's for the whole world; each map's ground is its own.
- Not tried in the web build: `Origin::Disc` opens the disc with `Disc::open_configured`, which the
  wasm shell may not have.
- Not reachable from the menus yet (P149).
- Not ridden with more than one rider, nor online.

## Next

P71's work is complete and committed on task/P71, pushed by hand, but not ticked: `./kit done` gates
on the full suite and main is red on S120 (above). Once S120 lands, rerun
`./kit done P71 "<msg>"` to tick. P149 is the menu entry.
