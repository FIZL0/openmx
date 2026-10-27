# 1 — ground quadtree (FINAL)

Task V65. 2026-10-27.

## What I did

- `crates/game-render/src/ground_tree.rs` (new): a `GroundTree` over one ground piece's chunk grid.
  Each node holds a bounding sphere around its kids' spheres, the worst error of its chunks at each
  level, and also their least error. `select` walks the tree from the root and stamps each node, which
  takes no clearing between frames:
  - A branch out of the frustum or past reach is culled at its root.
  - A branch wholly in view becomes **whole** when its level at the near side (worst errors) equals its
    level at the far side (least errors, at distance + 2r). Every chunk under it would pick that level
    alone, so it is drawn as one run.
  - Anything else splits.
  - A balance pass keeps neighbours drawn within one level of each other. It looks neighbours up
    through parent links. When a whole node is too coarse beside a finer one, the pass splits it
    rather than lowering it, so the cascade stays local.
  - Picks come out with V32's stitching masks: the finer side bends onto the coarser one.
  - `select` returns how many nodes it looked at.
- `terrain.rs`:
  - `lod_hole_errors` and `HOLE_EDGE_WEIGHT` (⅛). A chunk with missing cells (world piece seams, holes)
    used to stay at level 0. That blocked every whole node over it and cascaded through the balance.
    Now its ragged edge counts as an error, and it coarsens like the rest.
  - `lod_indices_holed` keeps a coarse quad wherever any of the cells it covers is present.
- `gpu.rs`:
  - `ground_for` no longer loops over every chunk of every view with a fresh index list. For each piece
    it runs the tree's `select`, groups the picks into runs, and caches one index part per run. A run is
    an 8×8-chunk block (`GROUND_BLOCK`), or one whole node at least that big.
  - A part is rebuilt only when its pick list changes. Parts not picked this frame are dropped.
  - Trees are built when a piece arrives (`regrid`) and dropped with it (`forget`).
  - `Drawn::ground_visits` reports the node count. `Cull::holds` is the "wholly in view" test.
- `crates/openmx/src/main.rs`: `--timings` prints "ground nodes looked at a frame: avg on average, max
  at most".
- Streaming: the brief asks that GPU ground buffers stream like `Tiled::stream`. MD17's `GroundStream`
  already does this (load 6000 ft / drop 9000 ft on desktop): `put_ground_piece` / `forget`. It is
  unchanged. Trees and parts follow it.
- `crates/openmx/src/ground.rs` was not touched. Nothing in it needed a change.
- opentennis was checked first: it has no ground and no quadtree, so nothing was taken from it.

## What the original does

Not applicable. This is rendering: the PS2 draws its own fixed-distance ground, and this is ours.

## How it was proved

- `ground_tree.rs` unit tests:
  - `the_nodes_looked_at_grow_with_the_view_not_the_ground`: 64 vs 256 chunks a side at the same reach
    look at 229 vs 237 nodes. Infinite reach on 65,536 chunks looks at 493.
  - `no_crack_between_the_levels_drawn`: grounds with 0, 1/5 and 1/3 holed chunks, seen from several
    eyes. Neighbours drawn differ by ≤ 1 level, and each mask marks exactly its coarser drawn
    neighbours, both ways. These are V32's stitching rules.
  - `a_whole_node_is_never_coarser_than_its_chunks_would_be_alone`.
- `terrain.rs` `a_holed_chunk_keeps_every_cell_it_has_at_every_level`.
- `crates/game-render/tests/ground_tree.rs` (headless GPU), on hills 48² and 192² chunks (4× wider)
  from the same camera:
  - Far Fog: both draw the same 402 chunks. They look at 401 vs 413 nodes.
  - Whole Map on the wide one: 31,524 chunks drawn for 2,037 nodes.
- Timings: `openmx --headless --size 1920x1080 --mods ~/repos/openmx/mods --play gas.tape --timings`,
  on the HEAD binary (before) and this one (after). The tape is idle for 2 s, then gas. The machine was
  loaded (load 25–38) by other slots, so the before and after runs were paired back to back.
  Ground is the CPU `ground` stage, averaged:

  | track / graphics | ground ms before | after | triangles before | after | nodes avg / max |
  |---|---|---|---|---|---|
  | world_mxu Far Fog (run 5 / 6) | 1.76 / 1.69 | 0.19 / 0.19 | 288 k | 290 k | 323 / 360 |
  | world_mxu Whole Map (run 5 / 6) | 2.84 / 3.79 | 2.30 / 2.70 | 1.83 M | 1.21 M | ~3,500 / ~4,150 |
  | world_mxu Whole Map (pair 4) | 1.40 | 1.07 | | | |
  | free06 Whole Map (run 5 / 6) | 0.11 / 0.09 | 0.19 / 0.21 | 377 k | 396 k | |

  The ride hashes were identical before and after.
- Looked right: world_mxu Whole Map, frame 250, before vs after (`context/shots/v65/`, git-ignored).
  Same ground, no crack or hole seen.

## Not verified / not 1:1

- The timings come from a machine loaded by other slots. Pairs swing ~±30 % run to run, and the ms
  above are not clean. The trends held in every pair: Far Fog ~9× less ground time, Whole Map fewer
  triangles and less ground time.
- "Whole Map holds 16.7 ms": not proven. The frame totals on this loaded machine are set by the sim and
  load, not the ground, and windowed (vsync) play was not measured. The ground stage is ~2.5 ms of it.
- free06 (one small piece) got slightly worse: +0.1 ms ground and +5 % triangles. A whole node split
  for balance draws its kids one level finer than they would sit under the old per-chunk balance, and
  the tree walk costs more than the old loop on a map this small. Not tuned further.
- Holed chunks now coarsen. At level l their ragged edge can stray up to (2^l−1) × cell × ⅛ (≈ 6 px on
  screen by the error budget), and a coarse quad over a partly missing area can overlap the next
  world piece's ground at a seam. That could show as slight z-fighting at piece seams far away. It was
  not seen in the shot, but not searched for over a whole world.
- A whole node is drawn entirely even if a corner of it is just outside the frustum. `holds` requires
  the sphere wholly inside, so this only happens through sphere slack: a few extra triangles, not a gap.
- GPU residency is MD17's `GroundStream`, unchanged. "Far tiles not resident" holds past its 9000 ft
  drop radius. Whatever Whole Map keeps resident beyond that is as before: V65 did not change it.

## Next

Nothing. The task lands with this.
