# 1 — d6 freeride track (PART)

Task D6. 2026-10-15.

## What I did

`crates/game-sim/src/track.rs`: the freeride area off the disc, read through D5's archive reader. One new
module, one new line in `lib.rs`, one new integration test file; nothing existing moved or reformatted.

The track chosen from R7's `.res` list is **`Tracks\free06.res`** (with `Tracks\free06_2.res` beside it,
which holds the bikes and the physics parameter sets). It is the one freeride area whose scene, collision
and terrain are all present and self-consistent. Inside it:

```text
tracks\free06\free06.scn          the scene: CRLF INI, ~900 sections
tracks\free06\free06.ps2.trn      the ground: PLN v7 quantised heightfield, 2 031 781 bytes
col\*.col                         92 distinct collision shapes, placed 353 times
```

Note the platform tag: the scene says `Free06.trn` and `sky34.cub`, the disc stores
`tracks\free06\free06.ps2.trn` and `sky34.ps2.cub`. The reader does not do that rewrite — the test does,
because it is the app layer's job and the rule depends on the platform, not the file.

What the module gives the simulation:

- `Scene::parse` — the spawn (`start`, `start_direction`), the terrain file and its layout
  (`TerrainLayout`: width/breadth in root cells, the two scales, the border width, the minimum division
  size), every `[ModelN]` as a `Placement` and every `[ObjectN]` (the collision placements) as another.
  A placement is a file name, a position, a look and an up vector, and an optional `Type`.
- `Collision::parse` — a `.col` file without being told in advance what class it is. Two classes behind
  one header: `Class::Mesh` (a triangle soup in a bounding-volume tree — the world's shapes) and
  `Class::Rays` (line segments in the same tree shape — the vehicles' probes). `Mesh` and `Rays` can also
  be asked for directly, and refuse the other class with a message that says which it got.
- `Terrain` — borrowed over the file's bytes, with the game's own query on top: `leaf(sx, sz)` descends
  the 16×16-ary tree, `block(i)` resolves the leaf's shared sample block, `height(raw, base)` is the
  dequantisation, and `cell(sx, sz, step) -> Quad` is the four corner heights, four normals and four
  surface ids the collision code asks for.
- `physics_file(engine, pro, tuning)` — the sixteen `bike\*.phy` names as a class (50/125/250/500) and a
  tier (Race/Pro) and a tuning letter (A/P), plus `FREERIDE_ENGINE = Cc250`.

### The ground

`free06.ps2.trn` is a quantised heightfield in a tree of sample blocks. The header names the terrain,
carries an optional 1024-byte palette, a world scale (9.0 here), a level count (2), the offset of the
tree, and a block count; the blocks sit between the header and the tree, and the accounting is exact —
`blocks_at + blocks × 2129 == tree_at`, 842 blocks, no slack.

A block is 2129 bytes: a signed base height, then 17×17 raw `u16` samples on a stride-20 row, 17×17
three-byte normals, a 289-byte array this task did not identify, and 17×17 surface bytes.

A node is 97 bytes: four floats (`vertical_scale`, `height_scale`, `min_y`, `max_y`), a block index, a
16-word table, a flags byte and the offset of its 256 children. `root_at + 97 == root.children_at`, which
is the node size proved by the file. The tree is 1 root + 9 level-1 nodes + 2304 leaves — exactly
`TerrainWidth 3 × TerrainBreadth 3` root cells of 16×16 cells of 16×16 samples = 768×768 samples = 6912
world units square at scale 9. 832 of the 842 blocks are referenced; blocks are shared between leaves.

Dequantisation, which is the one piece of arithmetic in the module:

```text
y_local = height_scale × (base + vertical_scale × (raw − base))      the subtraction in integers
y_world = terrain_scale × y_local
```

and when `vertical_scale == 1.0` the game takes a shorter path, `height_scale × raw`, which is the same
number by identity but not by rounding, so the port keeps both branches. `vertical_scale < 1` is a leaf
whose relief has been flattened towards its block's base — which is why the normals it hands out have
their x and z scaled by `vertical_scale` and their y left alone: flattening the relief scales the
gradient.

### The spawn, and what it pins

`StartPoint = 2884, 338, 2750`, `StartDirection = 0, 0, 1`. The ground under it comes out at 336.47, one
unit of clearance under the rider — and the median of (placement y − ground y) over all 353 collision
placements is 0.005, which is the check that pins the ×9 vertical scale: get that multiplier wrong and
the whole field of shapes floats or sinks by hundreds of units.

### What the simulation actually tests against

free06 has no `[SoultreeTerrain]` section and no `.grid.col`, so the ground *is* the heightfield — there
is no second, collision-only mesh of it. The shapes are the 353 `[ObjectN]` placements drawn from 92
distinct `.col` files, all of class Mesh, every one of them a triangle soup in its own tree.

### The physics default — settled only to the class

free06_2.res ships exactly one engine class's parameter sets, the 250 pair of each tier, so freeride is
**250cc**. Which tier (Race vs Pro) and which of the A/P pair governs the player is **not** proved — see
below.

## What the original does

Read any of these with `./kit fn <addr>`; `./kit xref <addr>` for who calls them.

- `0x1195d0` — the shared container-asset header check: magic 1234 and a four-byte tag.
- `.col` mesh: `0x1ed1e0` reads the header and dispatches on the class word at +0x0c; `0x1edda0` reads the
  vertices, normals, nodes and triangles; `0x1eded0` is the tree walk. Note the transform: the loader
  reads four rows of three floats and writes 0,0,0,1 into the fourth column itself, which is why the
  reader skips four bytes there.
- `.col` rays: `0x1eae70` (load) and `0x1ec930` (query). Sixteen floats of transform, then a segment count,
  a node count, and the segment count a second time — which the port checks against the first.
- `.trn`: `0x1d9930` loads the header, `0x1d0f30` / `0x1d1550` / `0x1d1cb0` walk the tree, and
  `0x1dc960` / `0x1ddaf0` are the height-and-surface query. `0x1ddaf0` is where the formula above comes
  from, including the integer subtraction, the `vertical_scale == 1.0` short path, the four-corner output
  with a `step` of 1 or 2, the normals with x and z scaled and y untouched, and the surface id masked with
  0x7f.
- `0x3b4660` — the `.grid.col` loader, for tracks that do ship a collision-only ground. Documented in the
  module, not exercised: free06 has none.
- `0x208c50` — the physics filename table at `0x42e0d8`: sixteen names, indexed by a class field at
  config +0x10c (0..3 = 50/125/250/500) and a boolean tier field at +0x110 (Race vs Pro), with the `…A.phy`
  and `…P.phy` of a pair formatted into two 272-byte buffers together.

## How it was proved

Seven integration tests in `crates/game-sim/tests/track.rs` (plus four unit tests in the module), all
against the user's own disc through D5's reader, all skipping with a printed reason when `context/iso` is
absent. `./kit check`: 109 tests pass. Confirmed by a temporary test that the disc is really being read
(`terrain 2031781 bytes, 842 blocks, 2 levels, 2304 leaves`), then removed — the whole file runs in 0.03 s,
which looks exactly like a silent skip and isn't one.

- `the_scene_names_the_spawn_the_terrain_and_every_shape` — the spawn, the facing, the 3×3 layout, scale
  9.0, and the model and object counts checked against the scene's own `NumberOfStaticModels` /
  `NumberOfCollisionObjects` rather than against my arithmetic. Every look vector is unit length.
- `every_collision_shape_the_scene_places_reads_whole` — all 92 shapes the scene places, each read to its
  last byte (parsed end == file size, the cheap format proof), every bounding-volume child index in range,
  and every triangle vertex within 0.01 of its own stored plane. 6433 triangles, worst residual 3.97e-5.
  Each file is also read through `Collision::parse` and must come back as the class the header claims, and
  must be refused when read as the other.
- `the_vehicles_collision_files_are_segments_in_a_tree` — the `*m.col` ray files in both archives:
  `nodes + 1 == segments`, children in range, byte accounting exact. Version 1 files have no surface word,
  so their body starts four bytes earlier; the accounting is what found that.
- `the_terrains_blocks_account_for_the_space_in_front_of_its_tree` — platform tag `ps2\0`, scale 9.0,
  palette present, `root.children_at == root_at + 97`, 48×48 = 2304 leaves, one `height_scale` for the
  whole terrain.
- `every_leaf_agrees_with_the_bounds_it_declares` — the end-to-end proof of the height formula, using the
  file as its own oracle: for each of the 2304 leaves, the min and max over its 17×17 samples must
  reproduce the `min_y` / `max_y` the node declares. Within 0.01 everywhere; worst deviation 1.8e-6; zero
  failures; both the flat and the scaled branch exercised.
- `the_ground_under_the_spawn_is_where_the_scene_starts_a_rider` — the drop under the spawn is positive and
  under 4 units, the surface ids are in range, the normals have w == 1 and y > 0.
- `the_shapes_the_scene_places_sit_on_the_ground` — the cross-file check: over 300 placements, the median
  of |placement y − ground y| is under 1.0 (measured 0.005).

Not proved by oracle: `ps2-oracle` runs one function over generated inputs, and these are file loaders
over a 2 MB structure, not pure functions of a few registers. The substitutes are byte accounting, the
files' own declared bounds, and cross-file agreement — which is why each of the three appears above.

## Not verified / not 1:1

- **The 289-byte per-sample array in each block**, between the normals and the surface ids. Its position
  and size are proved by the block's byte accounting; what it holds is not. Exposed as
  `Block::unread`.
- **Three header fields of the `.trn`** are read and ignored: the one word per block before the texture
  counts, and the fact that the block count and the texture count each appear twice (the pairs are equal in
  this file, so nothing distinguishes them here).
- **Five node fields** are read and ignored, kept in `Unread` so a later task can look: the `u16` before
  the block index, the word after it (an LOD or flags word), the 16-word table, the word after that, and
  the flags byte — which is 39 on every one of the 2314 nodes, so this file cannot say what it means.
- **The ×9 world-y multiplier** is taken to be the file's own `scale` field, applied to the local height.
  The game reads a terrain-object field rather than the file's, and the spawn/placement agreement above
  pins it only to about ±1.5 units — enough to exclude 1 and 18, not enough to exclude 8.9.
- **The instruction order of the height expression** was read as C, not disassembled. If the original fuses
  the multiply-add there, the port's `F` arithmetic differs in the last bit. Cheap to settle later with
  `./kit dis` over `0x1ddaf0`; it needs doing before S4 compares positions bit for bit.
- **`.col` class 1** is rejected, not parsed. One file on the disc is one (`col\machineracegate.col`) and
  nothing in free06 places it. It gains three words before its transform, and even then neither body layout
  accounts for its bytes: 4916 of 11496, and a brute-force scan implies a triangle count the header
  contradicts. Guessing would have been worse than the error message.
- **The packed-vertex fixed-point scale is unknown.** The mesh reader handles the packed `i16` form because
  the loader does, but no file in free06 is packed, so the divisor that turns those integers into units is
  untested. `Points::Packed` hands back the raw integers.
- **The reserved word before the mesh counts** (after the transform) is read and ignored.
- **`.grid.col` is documented but unexercised** — free06 ships no collision-only ground, so the one tested
  path to the ground is the heightfield. A race track will need the other.
- **The ray variant's node fields are assumed identical to the mesh's** (same 32-byte volume/centre/
  half-extent/children layout). Consistent with every file read, but the ray query at `0x1ec930` was not
  compared field by field against the mesh query.
- **Text to float is the host's `parse::<f32>()`**, then normalised through `fpu::operand`, not the game's
  own `strtod`. Decimal strings in these files are short and exact-ish, but a value that needs the last bit
  could differ.
- **`cell` bounds-checks its sample coordinates**; the original indexes without checking. Same answers
  inside the terrain, `None` instead of garbage outside it.
- **Which `.phy` freeride actually loads is unresolved.** The class is 250 (nothing else ships in
  free06_2.res) and the sixteen-name table and its indexing are proved from the ELF, but the tier flag and
  the A/P pairing are not: Ghidra shows both path buffers written and never read in that function. It needs
  a live read of the two config fields — class at config +0x10c, tier at +0x110 — in a freeride session
  over PINE. `FREERIDE_ENGINE` says 250 and its doc comment says the tier is unproved.

## Next

Two things, in this order, and both are small:

1. **Settle the freeride `.phy`.** Start a freeride session, find the config object (xref the table read at
   `0x208c50`), and read the class word at +0x10c and the tier byte at +0x110 over PINE. Then say in
   `track.rs` which single file freeride loads instead of only its engine class.
2. **Disassemble `0x1ddaf0`'s height expression** with `./kit dis` and match the multiply/add order exactly,
   before S4 starts comparing wheel positions bit for bit.

Then S4 has what it needs: `Scene` for the spawn and the shapes, `Collision` for what to test against, and
`Terrain::cell` for the ground under a wheel.
