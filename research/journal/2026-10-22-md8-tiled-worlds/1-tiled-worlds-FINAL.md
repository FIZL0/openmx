# 1 — tiled worlds (FINAL)

Task MD8. 2026-10-22.

## What I did

- `game-sim/src/tiles.rs`: `Tiled`, a world of terrains placed at offsets (openmx-mods' world
  standard §3: tiles with `at`/`keep`, then joins), `Land` (one track or a world, one `World`), a
  `Source` the shell or a test fills, and `tiles/generate.rs`, a `.trn` writer for tests.
- Every ground question goes to the tile that owns the point, in the tile's own coordinates:
  `Ground::at` (the suspension's blend) directly, and the original's heightfield query through a new
  `Heightfield::tile` hook in `contact_search::ground` / `nearest_surface` (a single track's field
  never answers it, so nothing changes for one).
- The edge: `World::area` is the rectangle from 0 to the far corner of everything owned, so the
  original's own edge test shoots off only at the world's outer edge. Holes inside it go through a
  new `World::off_world` (default `false`), asked from the same rider clause in `rider.rs`.
- Props: each tile's shapes are placed in world feet and swept tile by tile into one record
  (`Props::sweep_owned`), owner numbers starting at the tile's `first_owner`, so a shape's number
  never depends on what is loaded.
- Load and drop: a tile is read the first time anything asks about it; `Tiled::stream` loads ahead
  within `Reach::load` of any rider and drops past `Reach::drop` of all (desktop 6000/9000 ft, web
  2000/3000). A query on a dropped tile reads it again first, so streaming is memory, not behaviour.
- `openmx/src/worlds.rs`: reads a world mod (`kind: "world"`, `tiles` with `at`/`keep`, `joins`,
  `drop` by scene section, `sky`, `spawn`), each tile's scene and each terrain's header (a join's
  root cells off its tree), and is the `Source` that reads a piece's terrain and `.col` shapes from
  its folder. `--track <world id>` rides it: the sky tile's folder is the ride's first archive (sky,
  fog, lights, its props moved by its offset), the simulation is `Land::Tiles`, the map edge, the
  plane's area and the `map:` line are the whole world's, a `world:` line says what loaded, and the
  ride streams tiles every simulated frame. The GPU draws every piece's ground as its own chunks,
  moved and cut to the cells the simulation says it owns, unpainted (MD9 is the look).
- Decisions, since nobody was asked: the `Land` enum instead of making `Place` generic, so a disc
  track's ride goes through exactly the code it did; tile props in world feet (one sweep, one
  record) rather than moving each probe into tile space, since the bundle's centre/radius and the
  merge are world-space and the owner numbers must not move; the edge is the original's own test
  over the bounding rectangle, and holes a separate question, so a one-terrain track is untouched.

## What the original does

One terrain per track, its edge test over `256 × cells × scale` with a 150-ft margin
(`off_map.rs`). Nothing of the original covers several terrains; the per-tile query is the original's
own terrain query, moved.

## How it was proved

- `crates/game-sim/tests/tiles.rs`: three generated tiles (9, 4.5, 6 ft) and two joins (9 ft), one
  relief function in world feet. Over ~16,000 owned points each run, `Tiled::at` equals the tile
  alone's `Ground::at`, and `contact_search::ground` / `nearest_surface` through the world's field
  equal the tile alone's field, bit for bit (height, normal, weights, surfaces). The same with the
  whole world moved to (110,592, 29,952) ft.
- The move `world − at` on the EE's subtract against f64 over 20,000 points at ten offsets from 0
  to 2²⁴ − 4096: exact every time.
- Ownership precedence, holes, `off_world` with the margin, the area, refusing non-whole / negative
  / ≥ 2²⁴ offsets, and streaming (drop a, ask again: same answers, loaded again).
- Cost: a world query 761 ns against the tile alone's 635 ns (debug build; the terrain header is
  parsed again per query). The five test terrains are 246 KB; near tile c only 191 KB stays.
- `crates/openmx/tests/tiled_world.rs`, through the shell's binary headless on the disc's bike: a
  world mod of three generated track mods (9, 4.5, 6 ft) and two joins rides on the gas from tile a
  across join ab, tile b, join bc onto tile c, within 6 ft of the relief every tick, and is thrown on
  tick 410 at x 1574.9 — the outer edge 1722 less the 150-ft margin. Tile a, at the origin, rides
  the same 60+ ticks to the printed digit as `md8_a` alone. Tile b, 648 ft out, against `md8_b`
  alone from the same local spot: apart in the printed digits from tick 0, at most 0.024 ft apart
  over the ride across b. Every piece's ground read from the mod's own files equals that file alone
  at 108,000 grid points, both queries.
- `./kit check` passes whole (the disc replay hashes with it). One earlier run reddened on netplay's
  four_peers under load (the known flake); the rerun passed.

## Not verified / not 1:1

- **A ride in a world is not the tile ridden alone** except on a tile at the origin. The ground under
  each point is the tile's to the bit, but the rider's own sums are in world feet: tile b, 648 ft
  out, parts from b alone at once and drifts to 0.024 ft over ~200 ticks. At the ultra map's
  116,000 ft a position holds 1/128 ft. Nothing short of a per-tile local frame for the whole rider
  would make it 1:1, and nothing in the original covers several terrains to be 1:1 to.
- **Far from the origin is proved for the ground only** (the sim test, offset 110,592 × 29,952).
  No ride and no camera was run that far out; the shell's camera and its drawing are in world f32,
  so a picture at 100,000+ ft will jitter by 1/128 ft. Filed as MD13 (camera-relative drawing).
- **The cell a point is in**: ownership divides `x/spacing` as `Ground::at` does; the heightfield
  query multiplies by `1/spacing`. At a cell line the two can differ by one ulp, so a point exactly
  on a `keep` line could be asked of a neighbour's cell. Not seen in 230,000 test points; not ruled out.
- **The throw** aims at the middle of the world's bounding rectangle, the original's rule for one
  map; for an L-shaped world that middle can be over a hole. A wreck or a throw that comes down past
  the far edge or over a hole puts the bike back on no ground, and the shell's camera then panics
  (seen at tick ~2490 riding the 1722-ft test world on the gas; the test stops at 600). MD14.
- **The biplane** sees the tiles' ground but none of their props, and a hole as open sky. MD14.
- **Eco and paint** are not drawn in a world, and the sky tile's path cars (if it has a `.toy`) are
  drawn at its own coordinates, not moved; props are drawn for the sky tile only, and
  the software renderer draws only the sky tile's ground. Ground chunks are not stitched across a
  tile seam and every piece's ground is uploaded at the start (not streamed). MD13 / MD9.
- **Wasm**: `Reach::WEB` exists but the web build has no world loader; untested there.
- **The terrain is parsed again per query** (the `Ground` borrows the bytes): +20% a query in debug.
- **`drop`** takes a tile's scene sections out by name before the scene is read; no generated test
  has a dropped section.
- **P71** is not done here: noted on it that its spliced maps should be an MD8 world mod.

## Next

- MD9 (the look across joins), MD13 (the whole picture, streaming, camera-relative), MD14 (the
  plane and put-backs in a world).
