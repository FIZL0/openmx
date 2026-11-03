# P283 — world_ultra's load, for a Steam Deck

2026-11-03. Task: `./kit plan P283`.

world_ultra ('Every game: one world', 77 maps) loaded forever on the Deck and then crashed. On the
desktop its first ride frame came 29.9 s after launch, at a 2671 MB peak. It now comes at 4.4 s and
a ~1910 MB peak, and the ride is the same to the bit. The memory held while riding no longer grows
with the world: a map's ground paint is held only near the riders, and its ground pieces already
were (MD17).

Code:
- `crates/openmx/src/worlds.rs`: the `close_seams` edge walk, `look_near`, `map_paint`, `hollow`,
  `GroundStream::start_into`.
- `crates/openmx/src/paint_stream.rs`, new.
- `crates/game-render/src/gpu.rs`: the pooled world page arrays with the far atlas.
- `crates/game-render/src/terrain.rs`: `FarPaint`, `pool`, `far_page`, `rect_gap`.
- `crates/openmx/src/main.rs`: the `load:` and `memory:` lines.

Tests:
- `crates/openmx/tests/world_ultra_load.rs`.
- `game-render` `terrain::tests::a_world_held_near_its_riders_pools_the_near_maps_layers`.

Entry: [1-world-ultra-load-FINAL.md](1-world-ultra-load-FINAL.md).
