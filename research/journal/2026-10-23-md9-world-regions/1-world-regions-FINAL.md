# 1 — world regions (FINAL)

Task MD9. 2026-10-23.

## What I did

- `game-render/src/terrain.rs`: `Regions` (the world rectangles each tile owns — its `keep` cells
  moved by its offset — in the world's order), its `weights` / `shares` and `mirrored`; `WorldPaint`
  (each tile's MD4 `Pages`, `paint(x, z)` on the CPU) and `pack` (where each tile's pages go on the GPU).
  - Inside a tile's own cells the ground is that tile's paint, unchanged.
  - Anywhere no tile owns (a join), every tile is weighted `(1/d − 1/BLEND_REACH)²` by its distance
    `d` to the tile's rectangle (`BLEND_REACH` 1500 ft), plus a trace of dirt (`DIRT_WEIGHT`, so a
    hole far from every map fades to dirt rather than dividing by nothing). The weight grows without
    bound at a tile's edge, so the join starts on exactly the tile's colour and ends on the next one's.
  - A tile is read past its edge by mirroring the point back inside its rectangle. The mirror is
    continuous at the edge, so the join carries the tile's own texture (and its detail map) on from
    its last pixel instead of repeating one leaf's quarter-page.
- `game-render/src/gpu.rs`: `GroundPaint::World` and `ground_world_wgsl`. Each tile's numbers are
  written into the shader as one function per tile (no dynamic indexing of const arrays, which naga's
  GLSL path has trouble with). There is one page array for every tile, one leaf→page image with the
  tiles' rows stacked (layer + 1 in red and green, so more than 255 pages work) and one detail array
  (`upload_world_pages`, `upload_array`, at least two layers so GL does not take the array for a 2D
  texture). The shader runs the CPU's formula: owner, else weights, and each tile is sampled only where
  its share is over 1/512. Added to `every_shader_translates_for_webgl2`.
- `game-render/src/world.rs`: `Lighting` derives `Clone, Copy, Debug, PartialEq`, and
  `Lighting::blend` mixes colours, fog distances and the sun's direction (made unit again) by share.
- `game-render/src/scene.rs`: `Looks` — each tile's `Lighting` plus `Regions`. `at(x, z)` gives the
  mixed light and the sky of the tile with the largest share.
- `openmx/src/worlds.rs`: `WorldMod::regions` and `WorldMod::look`, which read each tile's folder:
  its terrain's pages with its own detail map and `DetailMapTilingMultiplier`, its scene's lights
  and fog (moved by its offset), and its sky (its folder first, then the ride's other archives).
- `openmx/src/menu.rs` / `menu_pages.rs`: Settings → Display → **World Sky**, Global (default) or
  Per Map. It is saved as `world_sky = per-map` in `settings.txt`, written only when it is not the
  default. `main.rs`: `--world-sky global|per-map`, passed on from the front end like the V52 rows.
  The ride prints `world look: N of M tiles painted from their pages, K with a sky of their own`, then
  `world sky: <setting> (the whole world's from <tile>)`, and a `world sky: over <tile|the whole
  world> at x,z: sun … ambient … fog …` line whenever the look in use changes. Under Per Map the
  first view's camera eye picks the tile each frame: the light is the blend, and the sky is switched
  (`Renderer::set_sky`) when the tile with the largest share changes.
- `game-sim/src/tiles/generate.rs`: `terrain_painted`, the test generator with a palette and one
  format-39 page (`terrain` is it with `None`, byte for byte as before).
- Decisions, since nobody was asked:
  - **Global** is MD8's existing behaviour: the world mod's `sky` tile, else its first, lights the
    whole ride. No new world-mod key was needed.
  - **The camera's place, not the rider's, picks the region**: the camera is what sees the sky, and
    `--shot-pose` then tests it directly. With split screen the first view decides for every view,
    since a frame has one `Lighting`.
  - **The sky is not crossfaded**: it switches at the join's halfway point while the sun, ambient
    and fog blend. Crossfading two skyboxes needs a second sky binding in the sky shader. Noted below.

Nothing was taken from opentennis, which has no terrain or sky regions.

## What the original does

Nothing. MX Unleashed draws one map with one sky, and so does MX vs. ATV. A world of several maps is
the port's own (MD8), so everything here is by eye, which is how the brief asks for it. The simulation
is untouched: the paint and the light are read only by the renderers.

## How it was proved

`crates/openmx/tests/world_look.rs` uses a world mod of two generated track mods, each 576 ft at 9 ft a
sample and flat (so the light is the same everywhere). Tile `a` has one red-brown page, tile `b` one
green page, and a 207-ft visible join sits between them:

- `the_paint_blends_across_the_join_with_no_step`: the CPU `WorldPaint`, from the tiles' own pages
  (`Pages::of` on the generated `.trn`). Each tile's colour holds within 1.5 levels inside its cells
  (at x 10, 300, 503.9 / 711, 900, 1210). From x 400 to 800 every quarter-foot step changes a channel
  by under 1 level, and the join's middle is between the two.
- `the_ground_runs_from_one_map_to_the_next_with_no_seam`: the shell's own binary, headless on the GPU
  (`--shot-pose` 340 ft over the join, looking down). The middle row is red at one end and green at the
  other, 189 levels apart, with no step over 6 levels between neighbouring pixels. Looked at by eye:
  a smooth red → olive → green ramp, no line.
  - The first run of this test found a real 84-level step. It was a hole in my test world (the join
    owns cells only up to 711, its last sample row having no cell after it, and `b` started at 720),
    drawn in the fog colour. The world was fixed; nothing in the code changed.
- `global_keeps_one_sky_and_per_map_takes_the_tile_below`: with the camera over `b`, Global prints
  `over the whole world` and `a`'s fog, 150,151,139. Per Map prints `over b`, `b`'s fog 60,80,160 and
  `b`'s ambient, 40,40,70 of 255. Over `a`, Per Map gives `a`'s.
- `per_map_light_is_mixed_across_the_join`: `Looks::at` is `a`'s light exactly at `a`'s edge and `b`'s
  at `b`'s. Across the join the ambient never moves more than 0.01 in half a foot, and the sky is
  `a`'s nearer `a` and `b`'s nearer `b`.
- `every_shader_translates_for_webgl2` covers the new world shader (naga's WGSL → GLSL ES path).
- `./kit check` passes. The first full run failed to build: `tests/menu.rs` lists every `Settings` field, and it now sets `world_sky` (Per Map, so the round trip covers it). The run after that hit the known busy-gate flake in pace's 60 fps test, and the next run was green.

## Not verified / not 1:1

- **Nothing to be 1:1 to**: no original draws several maps. The blend's shape (inverse distance
  squared, 1500-ft reach, mirrored reads) is a choice by eye, tested on flat generated colours only.
  No real combined world from openmx-mods (C1's joins between MX vs. ATV maps) was ridden or looked
  at. None was on this disk in a world mod, and building one is openmx-mods' job.
- **The sky is switched, not blended**: under Per Map, the sun, ambient and fog mix across a join,
  but the skybox changes at once where the next tile's share passes the last one's (the join's middle).
  A crossfade needs a second sky in the sky shader.
- **The software renderer draws none of this**: it still draws the sky tile's ground only. MD15 owns
  the software ground over every tile. `WorldPaint::paint` is the colour it will need. Per Map's light
  does reach the software frame, through the same `Lighting`.
- **Tiles painted by a splat** (MXU's own maps, P71's spliced worlds) are dirt in a world. Only MD4's
  page paint is blended. A splat needs its layers' programme per tile in the world shader.
- **Device limits**: all tiles' pages share one texture array (256 layers on the WebGL2 limits the
  renderer asks for), and the leaf images are stacked no taller than the largest 2D texture (2048). A
  tile past either limit is drawn in dirt. The ultra map's dozens of 48-page tiles would hit 256 layers.
  This needs streaming the paint with the chunks (MD17) or a bigger limit on desktop. Not tested.
- **A tile with pages of another size** is resampled, nearest, to the first tile's page size. The same
  goes for detail maps of different sizes. Not tested with real mixed sizes.
- **The region picks by the first view's camera**, so split screen shares one light and sky.
- **Joins are blended only by distance to the tiles' rectangles**. A tile whose `keep` is not a
  rectangle's whole (a ragged owner after precedence) is taken as its rectangle. Two tiles that meet
  with no join between them still show their seam, since the blend runs only where no tile owns the ground.
- **Cost**: on a join the fragment shader samples every tile whose share is over 1/512 (two here).
  Inside a tile it runs the rectangle tests for every tile and samples one. No frame time was measured.
- **WebGL2 itself**: the shader is proved to translate, but no world ran in the browser (the web build
  has no world loader, MD8).

## Next

- `./kit done MD9`.
