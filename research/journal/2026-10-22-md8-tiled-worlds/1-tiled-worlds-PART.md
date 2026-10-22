# 1 — tiled worlds (PART)

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

## Not verified / not 1:1

-

## Next

-
