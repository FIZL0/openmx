# 1 — see the whole map (FINAL)

Task V32. 2026-10-21.

## What I did

- **`--graphics full|original`** and a **Graphics** row in the settings (`settings.txt`'s
  `graphics =`; `Whole map` / `Original`, default `full`). `world::Graphics` turns a scene's
  `Lighting` and the map's size into a `Fog { near, far, max, reach }` and a far clip:
  - `original`: fog from `MinimumFogDistance` to `MaximumFogDistance`, fully opaque at the far end,
    nothing drawn past `MaximumFogDistance`, and the far clip plane there too (see below).
  - `full`: a haze that starts where the original's fog ends, ramps over the map's diagonal and stops
    at 30 % of the fog colour (`HAZE_MAX`), so it tints the distance and never hides it; no reach;
    far clip at twice the diagonal (at least 10,000 ft).
  The startup line says which, with the numbers:
  `graphics: full (fog 768..10534 up to 30%, far clip 19533; the disc's fog 1..768 in [97, 93, 92])`.
  The pause's settings change it mid-ride. The software renderer keeps the original's (it cannot afford
  the whole map) and says so when `--graphics full` is asked of it.
- **Reversed depth** (clear 0, compare greater, `z' = w − z` folded into the view-projection) so the
  depth buffer holds up from 2.4 ft to 20,000 ft: the decals lying on the ground (free06's shadow and rut
  meshes) don't z-fight at the far end.
- **Per-view culling** (`gpu::Cull`): six frustum planes (the far one included) per split view, the
  fog's reach, and a sub-pixel test (`PROP_PIXELS` 0.5 px) for props.
- **Terrain LOD** (geomipmapping, `terrain.rs`): `ground.rs` now cuts the terrain into chunks of a
  power-of-two cell count with a full (cells+1)² corner grid. Level *l* keeps every 2^l-th corner;
  each level's worst vertical error is measured on the chunk's real heights (`lod_errors`), and a view
  picks the coarsest level whose error stays under `GROUND_PIXELS` (0.75 px) at that distance
  (`lod_level`). Neighbours are balanced to differ by at most one level (`lod_balance`), and a finer
  chunk's edge facing a coarser one snaps its odd corners onto the even ones (`lod_indices`), so edges
  are shared exactly — no cracks, no T-junctions. Error-driven at under a pixel, the level switch is not
  visible as a pop. A chunk with missing cells keeps its own full-detail index lists (16 stitch
  variants) and never coarsens.
- **One ground draw per view**: the visible chunks' patterns are concatenated into one per-view index
  buffer, rebuilt only when that view's (chunk, level, stitch) key changes. No base-vertex is needed
  (WebGL2 has none): indices carry the chunk's base.
- **Model LOD**: props now pick the disc's own LOD group by distance (`psg::group_at`, which
  `Model::group_for` now calls), as the bike and rider already did. Props were already instanced and
  batched one draw per (view, model, group, material) by P44; V32 keeps that.

## What the original does

- The far clip is the fog's far distance. In the save state of `recording2.p2m2` (track free09, whose
  scene says fog 10..1000 in 150,151,139) the race viewport (pointer at `0x0048a560`, viewport
  `0x016dce40`) holds near 2.4 at +0x19c and far 1000 at +0x1a0 — free09's `MaximumFogDistance`.
  `./kit fn 0x00112a80` is the near-clip setter (stores +0x19c, calls the projection with +0x1a0);
  `./kit fn 0x0020aaa0` (viewport_arrange) sets near 2.4 for freestyle, and with two views out of
  supercross scales the fog's far by 0.61 (already in research/oracle.txt). `./kit fn 0x0012f940`
  (fog_set) stores far/near/third at world+0x6c/+0x70/+0x74.
- The renderer object at `*0x0048a560` holds the fog colour at +0x28 (0xff96978b = 150,151,139 as BGR)
  and **374.4, 1000.0** at +0x48/+0x4c. The far matches; the near does **not** match the scene's 10
  (see Not verified; filed as V33).

## How it was proved

- `terrain.rs` tests: every level and every stitch tiles a chunk with no gap or fold (area and open
  edges); two neighbouring chunks at different levels share their edge exactly; the level gets coarser
  with distance; balanced neighbours differ by one at most.
- `gpu.rs` tests: `the_original_is_the_discs_fog_and_far_clip` (a scene parsed from `[Fog]` 25..1000
  gives fog 25..1000, opaque, reach and far clip 1000; `full` has no reach, a haze ≤ 30 % and a far clip
  past the map); `the_whole_map_sees_its_far_edge_and_culls_behind` (a quarter-of-4K view: a chunk at the
  far edge of a 6,400 ft map is seen under `full`, not under `original`; one behind the camera is culled
  under both); `the_gpu_draws_the_far_edge_under_full_and_not_under_original` (the real renderer on this
  box's GPU — it ran, not skipped — draws 2 of 3 chunks under `full` and 1 under `original`, and back);
  the existing GPU-vs-rasteriser test runs under `original` and still matches (2 of 19,200 pixels off).
- `tests/menu.rs`: the Graphics row steps both ways and round-trips through `settings.txt`.
- Looked: 1280x720 shots of free06's start under both (the whole map with its pylons to the far edge
  under `full`; the original's brown wall at 768 ft), and a 4K crop of the horizon under `full`: no
  cracks or sky showing between chunks.

### Frame times, free06's start, 3840x2160, `--headless --fps --overlay none`, AMD Radeon 8060S

The ride is paced to 60 fps, so the number to read is the frame's own work (sim + draw + present,
present including the wait for the GPU), from the `--fps` session line over 300 frames:

| | triangles a frame | draw ms | present ms | frame ms (avg/worst of a second) |
|---|---|---|---|---|
| before V32, 1 view (original fog only) | ~51 k | 0.29 | 1.14 | — |
| before V32, 4 views | ~65 k a view | 0.54 | 1.33 | — |
| V32 `original`, 1 view | 49.5 k | 0.46 | 1.16 | 1.84–2.16 / 3.86 |
| V32 `original`, 4 views | 258 k | 0.41 | 1.26 | 1.78–2.21 / 2.58 |
| V32 `full`, 1 view (whole map) | 374 k | 0.29 | 1.00 | 1.42–1.44 / 2.57 |
| V32 `full`, 4 views (whole map) | 1,424 k | 0.57 | 1.47 | 2.43–2.53 / 3.35 |

So the whole map in four 4K views costs about 2.5 ms a frame, room for ~400 fps — well above V30's
"well above 60 at 4K with four views". P47's numbers (33 ms at 1890x1018) were the software path and the
overlay; with `--overlay original` at 4K the frame is still ~80 ms of CPU in the overlay's compositing,
which is V30's problem and the same before and after V32.

### Decisions

- No generated mesh simplification or impostors for props: free06's 577 props over the whole map come to
  ~69 k triangles (137 models, every prop a single LOD group on the disc), which the GPU draws in well
  under a millisecond. The disc's own LOD groups plus the sub-pixel cull are enough; building
  simplifiers for this would be work with no frame to win.
- 32-cell chunks (576 on free06): the per-view index rebuild is cheap and only happens when a level
  changes; the draw count is one per view for the ground either way.
- Nothing taken from opentennis: it has no terrain LOD or fog setting to borrow.

## Not verified / not 1:1

- **The original's fog near distance.** The save state's renderer holds 374.4..1000 on free09, whose
  scene says 10..1000; `original` fogs from the scene's 10. What moves it is not found (V33).
- **Far clip = MaximumFogDistance** is shown on one save state (free09, one view). The two-view 0.61 and
  other tracks' values are inferred from the decompilation, not watched.
- **`full`'s haze** (starts at the original's fog end, 30 % at most, over the map diagonal) is a choice,
  not anything on the disc; near ground under `full` is less fogged than the original's.
- **No popping** is argued from the error metric (level error under 0.75 px) and one still 4K crop; I did
  not watch a moving ride at 4K for pops. Normals are per corner and coarser levels skip corners, so
  shading can shift slightly at a switch.
- **Holes:** the missing-cells path (chunks with no ground in some cells keep full detail) is covered by
  the stitch test's generic code but I did not check whether any free06 chunk has missing cells, so it may not be exercised on disc data.
- **Only free06 measured**, headless (offscreen), on this machine shared with other slots; windowed
  present at 4K was not measured. The "before" row is from this task's first session's run of the
  pre-V32 build, same command.
- **Software renderer** keeps the original's fog and reach whatever `--graphics` says.

## Next

Nothing for V32. V33 (the fog's near distance) is filed.
