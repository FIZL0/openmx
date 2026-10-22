# 1 — big terrains (FINAL)

Task MD3. 2026-10-22.

## What I did

Went through every place the brief names, checking whether the port took a free track's size as given
rather than from the terrain and scene. Most of them already read it from the files. Four did not, and
those are fixed:

- **Already sized by the files, left alone.** The sim side needs no change, so the disc tracks keep
  every replay hash:
  - `game_sim::world::Ground::new` sizes the field as `width × 16^levels` samples off the scene's
    `TerrainWidth`/`Breadth` and the `.trn`'s own levels and spacing.
  - The edge (`off_map`, S59) and its `Area` are `cells × 256 × scale` off the scene's four numbers,
    which are the original's terrain `+0x530..+0x53c`.
  - The throw aims at `Area`'s middle, with `REACH.min(mid_x)`.
  - The biplane's area uses those same four numbers.
  - The spawn is the scene's start (or a mod's `spawn`, MD2).
  - The GPU's far clip and `Graphics::Full`'s haze use the ground's own span.
  - `eco.rs` places the plants by `samples × spacing` of the terrain.
  - The splat's mask transform comes off the `.psm`.
- **The radar's world** (`main.rs`): it was `ground.spacing × 192`, the `.trn`'s spacing. The original
  (`./kit fn 35d3e0`, V26) reads `max(terrain+0x538, +0x53c) × 192`, which is the scene's
  `TerrainWidthScale`/`BreadthScale`. It now takes those. On the disc they equal the `.trn` spacing
  (free06 still prints 1,728 feet). A mod whose scene and file disagree now gets the original's
  answer.
- **The software ground's draw radius** (`ground.rs`, used by `main.rs`, `moment.rs` and `web.rs`):
  it was 44 *samples*, which at 4.5 feet a sample is half the ground. It is now `ground::REACH`,
  396 feet, turned into samples by `Ground::radius()`. That is 44 at nine feet, so the disc tracks
  draw exactly what they did.
- **The GPU's chunk size** (`ground.rs`): it was 32 cells. It is now `ground::CHUNK_FEET`, 288 feet,
  through `Ground::chunk_cells()`: 32 cells at nine feet (free06 still uploads 576 chunks) and 64 at
  4.5 feet (the 5,760-foot map uploads 400 chunks instead of 1,600).
- **The software follow camera's far plane** (`main.rs`): it was a flat 4,000 feet. It is now
  `fog_far.max(4000)`, so a mod whose fog ends further out is not clipped short of it. The disc's fog
  ends at 2,000 feet at most, so the disc tracks are unchanged.
- **game-render's `terrain.rs` height band**: it now uses the file's band, not a fit.
  - The V1 reader's "unexplained `u32 1, u32 size`" at 1073 is the tree's level count and the root
    node's offset, the same words `game_sim::track::Terrain::parse` reads.
  - The root node's second float is `height_scale`. A word's world height is
    `height_scale × spacing`, from 0.
  - `Terrain::calibrate` and its least-squares `fit` are gone. `Terrain::props_rest` reports where
    the bales stand on the file's band instead, and `first-draw` prints that.
- **The ground cache.** Nothing in the port caches the ground by a fixed size.
  - The GPU's per-view ground (`GroundView`) is keyed by the chunk count.
  - The original's "last ten cells" in the contact search is not ported (`contact_search.rs` says
    why).
  - So there was nothing to change.
- A `map:` line in the shell's log gives the edge, the margin and the radar's span, which the test
  reads.

Taken from opentennis: nothing. Its courts are one fixed size.

## What the original does

- The map edge: S59's journal and `off_map.rs`. The extent is root cells × 256 × the scene's scale,
  per axis, with a 150-foot margin.
- The radar: `./kit fn 35d3e0`. `W = max(terrain+0x538, terrain+0x53c) × 192`, where those two are
  the scales the edge also reads (V26's journal).
- The terrain band: the terrain query D6 ported (`TerrainNode::height`) multiplies the raw word by the
  node's `height_scale`, then the local height by the file's spacing. Nothing about it is per-track
  outside the file.

The MX vs. ATV numbers are from its own files in `~/repos/openmx-mods/extracted/mxvatv` (read once by a
scratch test, not committed):

| Map | Scene layout | Terrain |
|---|---|---|
| MC01 | `TerrainWidth=5`, `WidthScale=4.5` | two levels, spacing 4.5, root `height_scale` 0.00161, start at (3076, 452, 2280) |
| MC05, MC10, Nat01 | the same 5 / 4.5 | — |
| FS01 | 1 / 3.5, `TerrainBorderWidth=3000` | one level (MD12's case) |

## How it was proved

- **`crates/openmx/tests/big_terrain.rs`** (disc; three tests, through the shell's binary headless).
  It writes an MX vs. ATV-sized mod: 80×80 leaves, two levels, `TerrainWidth=5`, 4.5 feet a sample,
  flat at 0.
  - `the_big_map_is_sized_by_its_terrain_and_scene`:
    - `ground 5756 x 5756 feet in 1280 x 1280 samples`
    - `map: 5760 x 5760 feet to its edge, shot back from 150 feet short of it; radar 864 feet across`
  - `a_big_map_rides_corner_to_corner_and_shoots_off_at_its_own_edge`: `--trace-at 300,300,45` on the
    gas.
    - The bike rides the diagonal on the ground (y −0.28) at 120 ft/s, through (4362, 4348) at
      tick 1500, past where a 768-sample map would end.
    - The first wreck is on tick 1940 at (5610.4, 5587.5). That is the map's own edge less its
      margin (5,610), and the throw's 300 ft/s toward the middle is on the same row.
    - No tick before it is inside the margin.
  - `a_big_map_shoots_off_at_its_near_edge`: from (600, 2880) facing −x, thrown within 12 feet of
    x = 150.
  - These rides would also pass on the old HEAD. The sim was already sized by the files, and the test
    proves that property. The radar line is the new part.
- **`crates/game-render/tests/terrain_band.rs`** (disc): all 24 supercross terrains the V1 reader
  takes.
  - The band is the sim reader's root `height_scale × scale`, bit for bit.
  - The scene's bales (145 to 220 a track) stand a median of 1.24 to 1.57 feet above the ground. The
    spread is that tight on every track, so a misread band would show.
  - The fit it replaces was about 10 % out in scale (sx01: 1/540 fitted against 1/485 in the file),
    with a made-up offset.
  - In the scratch run, flat 0xAAAA × the file's band gives exactly 90.0 on sx01, sx05 and sx06, which is the
    start line's height there.
- The unit tests in `terrain.rs` build the synthetic patch file with a root node now:
  `the_band_is_the_root_nodes_and_the_bales_stand_on_it` replaces the calibration test.
- **Disc replay hashes are unchanged.** No game-sim file changed. MD2's free06 hash test and the
  replay suite run in the gate. free06's log still says 576 chunks and a 1,728-foot radar.
- **Looked right:** a headless `--shot` on the big map shows the bike on the dirt, with the HUD and
  radar up.

## Not verified / not 1:1

- **The radar on a mod whose scene scales differ from its `.trn` spacing.** It follows the original's
  formula (the scene's scales), but nothing on either disc has the two differ, so no ride of the
  original covers that case.
- **The V1 `.trn` reader still walks the patches in file order, not through the tree.** It also takes
  every node at full relief, ignoring `vertical_scale`. The band is now the file's, but where a
  sample's height comes from is still V1's reading. The bales seat at 1.2–1.6 feet everywhere, which
  says the reading is close. Only `first-draw` uses this reader; the freeride ground is D6's tree
  query.
- **The splat's light-map margin** (`splat.rs`, `SHADOW_MARGIN = 64/768`) is a fraction fitted on
  MXU's 768-sample fields. On a 1,280-sample MX vs. ATV map the margin may not be the same fraction.
  It is render-only and only matters to a mod that ships a light map. Left as a note per AGENT.md
  (no render chasing).
- **`REACH` (396 ft) and `CHUNK_FEET` (288 ft)** are the port's own numbers, chosen to keep the disc
  tracks' 44 samples and 32 cells. They are not the original's: the original draws its own way.
- **GPU frame time on the big map** was not measured (headless only). With 400 chunks of 64 cells the
  LOD picks per chunk as before, but nobody has watched it at 60 Hz in a window.
- **A one-level terrain** (MX vs. ATV's supercross and freestyle, FS01) still comes out 16 samples a
  side. That is MD12, filed by MD2, and not done here.

## Next

Nothing for MD3.
