# 1 — floating trees (FINAL)

Task V125. 2026-11-10.

## What I did

- **The cause: a world kept a tile's plants outside the tile's keep.** `world_dress::build` stood each
  tile's `.esb` plants on the tile's own terrain and moved them by the tile's offset, but it never cut
  them to the tile's kept rectangle. In world_mxu the stitcher cuts every national down to its track, and
  the rest of the map's ground belongs to a join's blend (or a neighbour). So most of a national's trees
  stood over ground that was not theirs. A probe over world_mxu: nat01 had 16,660 of its 20,422 plants
  outside the keep, and 14,866 of all its plants were more than 2 ft off the world's ground. Every
  national was alike (nat15: 16,455 of 20,560 outside). Free-ride tiles keep their whole map, so they had
  none outside. In all, 213,745 of world_mxu's 279,936 plants stood outside a keep.
- `crates/game-render/src/eco.rs`: added `Grove::retain(keep)`. It drops plants and keeps each one's own
  turn and jitter entry lined up with it (those are per-plant vectors made in `Grove::new`).
- `crates/openmx/src/world_dress.rs`: `build` now drops what stands outside the tile's keep. That is the
  plants (`Grove::retain` with `worlds::inside`) and also the props (`worlds::cut_look`), which were not
  cut either. That is MD27's rule, which the simulation's scene already follows for collision shapes and
  models in `worlds::read`. The tile's `world props:` log line counts what was left out.
- **Single maps were already right.** A survey of all 30 disc tracks with an ecosystem found every
  plant with the stand bit within one 65536th of the map under the ground. Every other plant keeps the
  file's height, and only 15 plants on six maps are more than 2 ft off (free07, free10, nat05, nat12,
  nat18, nat21; worst a free07 rescuetreeH 9.9 ft up and a nat18 bush 9.8 ft down). The original's plant
  reader has no path that moves a plant without the bit, so it has them there too.
- Took nothing from opentennis: it has no ecosystem.

## What the original does

- Plant reader (`./kit fn 0x12b470`), re-read for "which it skips": it skips none. It loops over the
  file's whole count and writes every plant. The species byte goes through an optional remap table at
  eco+0x100, and only the bit-7 / `param_3` snap moves a plant (V49's reading). The original has no
  worlds, so the keep cut is ours: it removes plants whose ground the world no longer has.

## How it was proved

- `crates/game-render/tests/ecosystem.rs`
  `every_tracks_plants_stand_where_the_originals_reader_puts_them`: all 30 disc tracks with an `.esb`
  (293,634 plants). The loaded set is the file's set, plant by plant (species, x, z, size). Plants with
  the bit stand on the ported point query to within one 65536th of the map. Plants without it keep the
  file's y. The 19 plants more than 2 ft off (15 plus the free07_ss/free10_ss copies) are pinned.
- Same file, `free10s_and_nat11s_plants_are_the_originals`: the original's own plant words in S133's
  RAM images `context/states/s133-free10.ee.bin` (6,160 plants, none stood) and `s133-nat11.ee.bin`
  (10,929, all stood). Count and every x/y/z word are equal. Together with V49's free06 and free09
  checks, that makes four maps proved against RAM.
- `crates/openmx/src/world_dress.rs` `world_mxu_plants_stand_on_the_ground_their_map_keeps`: builds
  world_mxu off the disc (20–40 s) and dresses all 26 tiles that have plants. Each tile's kept plants
  are exactly its map's plants inside its keep (66,191). Every one stands the same distance off the
  world's ground (`Tiled::at`) as off its own map's, within 1e-3 ft, so the ground under it is its own.
  No prop stands outside a keep.
- By eye: shell shots of world_mxu on nat01 (`--trace-at 7700,5857,270`, and a raised `--shot-pose
  7750,600,5857,270,-20,70`) show the tree stands on the ground and no trees out on the join. The log
  says `mxu_nat01 built, 247 props, 3762 plants (0 models and 16660 plants outside its keep left out)`.
  The shots were kept in the scratchpad only.

## Not verified / not 1:1

- **No PCSX2 shots side by side.** The original has no worlds, so there is nothing in it to shoot for
  the world bug. For single maps the placement is proved against the original's own RAM words on
  free06, free09, free10 and nat11, which is exact where a shot is only by eye. Getting PCSX2 onto
  free07/nat21 at the off-ground plants would take a tape and a teleport, so I did not do it.
- **The 15 off-ground file-height plants** (free07, free10, nat05, nat12, nat18, nat21) are kept where
  the file puts them, because the reader has no other path. Only free10 among those maps has a RAM
  state, and its off-ground Choia matches. The other five maps were not checked against RAM: no state.
- **A world's national loses the trees outside its keep.** About 76 % of a national's plants go, the
  ones around the track on ground the world gave to a join. That is not the original (which has no
  worlds) but a choice: placing them on the join's blend would be inventing plants. The joins get no
  plants of their own.
- **world_mxu_free and world_mxu_nat were not run in the test.** world_mxu holds every free ride and
  every national, so it covers both, but those two worlds' own keeps (their specs cut differently)
  were not measured. The code path is the same for every world.
- **Mod worlds (world_ultra, mxvatv, untamed)** were not run. Their tiles go through the same `build`.
- The test compares against `game_sim::world::Ground::at` on both sides, not the eco point query.
  The two differ by up to about 0.06 ft on free06 (the triangle split), which this task did not look into.

## Next

Nothing for V125.
