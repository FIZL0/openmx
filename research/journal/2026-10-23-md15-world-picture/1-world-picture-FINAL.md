# 1 — world picture (FINAL)

Task MD15. 2026-10-23.

The user: "other than the start one there's no props or anything", "each map's props are still missing
in the big worlds". Before this a world drew the sky tile's props only (and at the home tile's feet),
no tile's ecosystem but the home one's, no path cars, and the software ground over the home tile only.

## What I did

- **Every tile dressed, by distance** (`crates/openmx/src/world_dress.rs`, new). `DressStream` is
  MD17's `GroundStream` again for what stands on the ground: the tiles within `Reach::load` of a rider
  are built before the first frame on every core, the rest on a thread of their own nearest first,
  and a tile past `Reach::drop` of every rider is let go. Nothing waits for a tile's props: they come
  a frame or two late rather than the ride stopping. A tile's build reads its own folder only: its
  scene with the world's `drop`ped sections left out, the props' models, its ecosystem stood on its
  own terrain (`Grove::load`), and the models of the cars its `.toy` drives. Everything is moved by
  the tile's offset and every model name gets the tile's prefix (`t1::Model`), so two tiles' models of
  one name stay two models.
- **Assets taken in and let go** (`game-render/src/world.rs`): `Assets::take_in(other, prefix)`
  appends another `Assets`' textures and models and says what it took (`Taken`); `let_go` removes
  them (texture slots are blanked, not removed, so indices stay put).
- **GPU** (`game-render/src/gpu.rs`): props carry an owner (`SCENE` for the scene's own); `add_props`
  adds a tile's props and grove, `drop_owner` removes them and frees their models' vertex buffers and
  their textures' materials. Several groves draw (`groves`), each with the shared plant LOD switch.
  `props_of(owner)` says where an owner's props stand (for the test and the shell's last lines).
- **Camera-relative drawing** (gpu.rs): the view uniform's matrix is built from a camera at the
  origin; every instance (props, plants, cars, the bike) is moved by `-eye` on the CPU before upload,
  and ground and other world-space vertices subtract the eye in the shader before the multiply
  (`origin` in the `View` struct of all three shaders; fog measures from the relative eye). A ground
  vertex is still stored in absolute f32 (a step of ~0.008 ft at 116,000 ft), which is below a pixel. The
  absolute matrix is still used for culling and sky probes. Relative to the eye
  every drawn thing is a few thousand feet at most.
- **Path cars**: each dressed tile's `Traffic` drives in its own feet on the ground asked at its
  offset (`Dressed::cars`); in a world the home tile's own traffic is not loaded twice.
- **Software ground over every near tile** (`world_dress::draw_ground`, `ground::Ground::queue_piece`,
  `ground::draw_world`): each piece within `ground::REACH` is meshed from its own terrain (kept in a
  cache within 4×REACH) and painted with the world's pages, or dirt.
- **Seams** (`worlds::owns_cell`): a cell is drawn by the first piece that owns its middle, or any of
  four inset corners, so a misaligned seam (world_mxvatv's mc02, keep starting at 34,45) has no gap.
  Used by `piece_chunks` (GPU) and the software ground alike.
- **Baked paint for disc tiles** (`world_dress::bake_splat`, used by `WorldMod::look`): a tile whose
  terrain has no pages (a disc map's) gets pages baked from its splat (`Splat::paint`) times its light
  map, one 128-px page per 4×4 leaf block. Tiles are read in parallel. The GPU asks for the adapter's
  own texture-array layer limit (a disc map bakes 144 pages).

## What the original does

Nothing to compare: the original has no worlds (MD8 stitches the disc's maps). Each tile is drawn as
the tile ridden alone draws it: same scene, same ecosystem file, same `.toy`, moved by its offset.

## How it was proved

- `crates/openmx/tests/world_picture.rs`: a generated three-tile world whose tiles each place one
  disc model (the beach ball) at (100, 150) in their own feet; one headless frame with a 600-ft load
  reach. The GPU's props stand at (100, 150) for t0 and (820, 150) for t1, and t2 (out of reach) has
  none. Skips without the disc or a GPU.
- `world_dress::tests::a_far_tile_is_dropped_and_built_again_when_the_rider_comes_back` (unit test,
  no disc): a generated three-tile row; `GroundStream` and `DressStream` driven from t0's start out to
  t2 and back. t0's ground is dropped and built again with the same chunks (place and vertex count),
  and its dressing is dropped and built again; t2's dressing is dropped back at the start.
- By eye (shots in the session scratch, not kept): world_mxu_free at free10's start
  (`--trace-at 9869,3476,-30`) shows free10's power lines and buildings in GPU and software, matching
  free10 ridden alone from its own start (`--trace-at 2669,3188,-30`); logs say free06 577 props / 5600
  plants / 21 car models, free10 572 / 6160, free11 168 props / 8 cars, 29 cars on clips. world_mxvatv
  from (1500, 250) shows no seam gap; its start shows palms and the banner of the start tile.
- Baked paint: "world look: 5 of 5 tiles painted from their pages" on world_mxu_free; shaded hills
  and baked shadows in both renderers.

## Not verified / not 1:1

- **Jitter at 116,000 ft only seen still**: one GPU frame on world_ultra at (111660, 1644)
  (`--trace-at 111660,1644,40 --shot`) is clean: the bike, its rider, plants and props crisp, no
  cracks. A moving ride there was not watched, and jitter is a thing of motion.
- **Software props are not camera-relative**: the software rasteriser still transforms in absolute
  feet (f32), so far out on world_ultra its props may shimmer. The GPU is the default renderer.
- **Software plants**: drawn only when fully faded in (alpha ≥ 1), without the GPU's dithered
  fade-in; plants pop in the software renderer.
- **Baked pages are blurry and approximate**: 128 px per 64 samples (~4.5 ft a pixel) with no detail
  map, and the light is the light map divided by an estimate of the terrain's own sunlight (clamped at
  0.2) since the world shader multiplies the sun back in. Looks right at a distance; up close it is
  softer than the tile ridden alone. Not compared pixel for pixel.
- **WebGL2 has 256 array layers**: the raised layer limit is the adapter's; a browser may refuse it and
  `pack` turns the tiles that do not fit to dirt. Not tried in the web build.
- **Small GPU leaks on a drop**: a dropped tile's models keep their slot in the renderer's model list
  (an empty 4-byte buffer) and the statics palette grows when a model is uploaded again; `Assets`
  keeps blank texture slots. Bounded by tiles × models per tile; a long ride over world_ultra was not
  measured.
- **Tile files read from the tile folder only**: a tile whose eco or model lives in the disc's shared
  archives but not in its folder goes without (world_mxvatv's mc02: "ecosystem: no mc02 in the track's
  archives"). Said in the log line.
- **Overlap at misaligned seams**: two pieces can both draw a corner cell where their keeps disagree
  (the inset-corner rule prefers no gap); a z-fight there was not seen but not looked for closely.
- **Ground drop/reload in a shell ride** is proved by the stream unit test, not by a shell ride that
  turns round: a tape that rides out and back reliably was not written.

## Next

Nothing for MD15. Follow-ups worth filing if the user notices them: software props camera-relative, software plants dithered, a moving ride far out on world_ultra.
