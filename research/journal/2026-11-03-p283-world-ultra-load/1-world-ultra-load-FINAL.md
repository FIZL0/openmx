# 1 — world_ultra's load (FINAL)

Task P283. 2026-11-03.

## Profile (desktop, before)

These runs were `openmx --headless --mods ~/repos/openmx/mods --track world_ultra --frames 30 --timings`.
The stages were timed by stamping each log line with the wall clock and VmRSS, and sampled with
`eu-stack` every 0.5 s (perf is not allowed here; see memory profiling-without-perf).

- **0–1 s:** disc and archives.
- **1 → 25 s, `Tiled` / `close_seams`:** about 24 of the 28 s load. world_ultra wraps, and
  `generate::seam_joins` asks for the ground at one edge and then at the far edge, 35,000 ft away,
  for every sample in turn. Under the open world's reach (`Reach::DESKTOP`, 6000/9000 ft), each ask
  dropped the tiles under the other edge and read them again. The stacks were all in
  `Tiled::stream` → `Files::load` → terrain parse.
- **World look, +1.3 GB:** every map's ground pages (`Pages::of`), and the 5 disc maps' baked pages
  and sharp-splat modulate pages.
- **Upload, +1 GB RSS:** the page arrays for all 77 maps: 128 px × 1441 layers and 256 px × 3176
  layers, with mips.
- **Ground start, ~+800 MB transient:** the 88 ground pieces within 6000 ft of the start, all held
  on the CPU until the last was built.

## What I did

1. **Seams (worlds.rs `close_seams`).** Each wrapped edge's samples are now read once, in order along
   the edge, into a table. The joins read their heights from the table, which falls back to the
   ground for any point that is not on an edge. The joins are the same bytes: ride hash
   0xb363ed99829fb5f3 before and after. The load went from 29.7 s to 6.3 s.
2. **Paint held near the riders.**
   - `WorldPaint.far: FarPaint`. Every map's pages are box-averaged to 16 × 16 (`far_page`), and all
     of them go in one atlas texture (77 maps' worth is 3 MB).
   - `reach: Some(drop)` turns on pooling. `WorldPaint::pool` sizes each page array to the most
     layers that the maps within `drop` (wrap-aware `rect_gap`) of any one map take, plus room for
     the largest map, capped at the device's layer limit. For world_ultra that is 1440 + 1060
     layers instead of 1441 + 3176.
   - In the GPU, `PagedPaint` gives each map a run of layers in its array: `take` / `give_back`,
     which merges free runs. The leaf image's red/green holds the near layer + 1 and its blue/alpha
     the far atlas cell + 1. In `ground_world_meshed_wgsl`, a leaf with no near layer but a far cell
     samples `far_pages` with the clamped sampler, half a texel in.
   - `look_near` reads every map as before (on every core). A map not within `load` of the start
     has its far pages made, then its pages and modulate are emptied (`hollow`) in the reader
     thread, so the load never holds every map's pages at once.
   - After `set_ground_streamed`, the shell empties the CPU copies of the maps it kept too. It also
     drops the disc maps' splat textures and light maps; only their passes and roads stay, and those
     are what the shader was generated from.
3. **The paint stream (paint_stream.rs).**
   - It uses the world's own reach: load 6000 ft, drop 9000 ft, measured to each map's owned
     rectangle, with the riders' copies across the seams (`wrapped_near`).
   - A map coming within `load` is asked of a worker thread. The worker runs `map_paint` and
     `page_layer_mips` at the array's layer size.
   - Each frame writes whole layers, the nearest map first, until 4 MB is written (at least one
     layer), then shows the map once every layer is up. A map is let go past `drop`.
   - If a map's array has no room, the map waits on its far pages until a map is let go.
4. **Ground start (worlds.rs `GroundStream::start_into`).** The builders send each piece over a
   channel, and the shell adds it to the GPU in index order (a reorder buffer), so the chunk slots
   are given out in the same order as before. `start` is unchanged for its other caller.
5. **`load:` and `memory:` lines.** These come from `/proc/self/status` VmHWM / VmRSS. The first
   frame's time is measured from `main`. A Deck log now shows both.

## Numbers (desktop, same machine and same hour; HEAD 4826a394 built in a scratch worktree)

Runs used `--frames 30`. Wall time is from launch to `warm frame:`, and the peak is VmRSS sampled
from outside. Ride hashes are the same in every pair.

| world | before | after |
|---|---|---|
| world_ultra | 29.9 s, 2671 MB | 4.4 s, 1910 MB (`load:` 4.42 s, VmHWM 1909) |
| world_mxvatv | 15.4 s, 1517 MB | 3.6 s, 1072 MB |
| world_untamed | 14.0 s, 1459 MB | 3.1 s, 1025 MB |
| world_mxu_free | 2.8 s, 647 MB | 2.9 s, 663 MB (5 maps; all held, as before) |

**Riding across world_ultra.** The run was `--world-sweep 56000,16000,110000,16000 --frames 900`:
54,000 ft from the mxvatv maps into the mxu ones.
- 21 maps were shown and 24 let go.
- At most 4.3 MB was written in a frame, 283 MB in all.
- The slowest paint update took 1.2 ms. The worst frame was 17.8 ms (present 11.0, stream 4.5, the
  ground's chunks included), and 2 frames went over 16.7 ms.
- RSS fell over the ride, from 1952 to 1685 MB.

**Pictures.** Default camera at frame 30, new against old: 440 px differ, while two runs of the old
build differ by 1200 px (plants and dither). From 6000 ft up looking over the far maps, the far
pages look the same as the full ones at that distance (`compare -metric AE` 1.5 %, all in the far
two-thirds).

**Where world_ultra's ~1.9 GB goes now.**
- About 600–750 MB is the GPU ground of the 15,423 chunks within the 6000 ft load reach. RSS counts
  it on this desktop.
- About 270 MB is glibc arena memory that the 32 look-reader threads freed but kept. With
  `MALLOC_ARENA_MAX=1` the peak is 1650 MB. `malloc_trim` is out, because the crate forbids
  `unsafe`.
- The real data the look keeps: the 9 near maps' pages 113 MB, details 55 MB, skies 115 MB, disc
  splats 88 MB (dropped after the upload), far pages 3 MB.

## Checked

- `./kit check -- -p openmx` with these tests passed: world_ultra_load, world_texture, world_look,
  world_streaming, stitched_seams, world_first_frame, wrapped_world, wrapped_world_draw,
  world_far_tiles, world_picture, disc_worlds (40 tests).
- `cargo test -p openmx --test world_ultra_load -- --nocapture` ran both tests, which were not
  skipped. In the test profile: first frame 7.28 s, peak 1878 MB; sweep 21 shown / 24 let go,
  4.3 MB a frame, 1.51 ms slowest update.
- The new game-render unit test passed.
- The full `./kit check` first went red: one failure was mine and two were there before.
  - **Mine:** `one_fps::nothing_is_read_or_decoded_after_the_first_frame` saw 1538 bytes read. That
    was the new end-of-ride `memory:` line reading `/proc/self/status` before the count was taken.
    It now comes after the count, under `--timings`. `./kit check -- -p openmx --test one_fps
    --test world_ultra_load` then passed (4 tests).
  - **Already at HEAD:** `research/oracle.txt:8523` (MD75's `atv_follow_fields` line, merge
    544ccb92) is not an address line. Every oracle-reading test failed on it: ray_probe,
    ragdoll_wrecks, ragdoll_bike and openmx audio. Filed as MD88.
  - **Known:** `trick::the_humans_tricks` is a red the memory already records.
- Follow-up for what is left (Deck, near ground, skies): P285.

## Not verified / not 1:1

- **The Deck.** Nothing was measured on the Deck (memory profiling-on-the-deck: no ssh in runs). The
  desktop peak fell from 2.7 to 1.9 GB and the load from 30 to 4.4 s. On the Deck the GPU's share
  is also counted against its 14 GB, and its load is several times slower; those Deck numbers are
  not measured.
- **Ground level of detail.** The far ground is not built: pieces past 6000 ft are not built, and the
  stand-in is drawn under them as before. The near ground is still full detail out to 6000 ft, which
  is the biggest piece left (600–750 MB). A coarser reach for the Deck (`--world-reach`) was not
  chosen for it.
- **Still loaded whole:** the detail array (73 × 512²), the 77 skies (115 MB) and the far atlas grow
  with the number of maps. They are small next to the pages, but they are not flat.
- **The 128 px array's pool on world_ultra** is 1440 of 1441 layers, because the mxu maps' 128 px
  pages all lie within 9000 ft of one another. It saves nothing there; the 256 px array's is 1060 of
  3176.
- **A map whose array has no room** stays on its far pages, silently, until one is let go. Two riders
  far apart, or a plane flown far, could do this. Neither was ridden.
- **The software renderer** reads the paint on the CPU. Paint is only held near the riders when the
  ride is on the GPU, so the software renderer still gets every map, as before. `--seams` and
  `--world-ground all` (the overview, 8.4 GB) also still read every map's paint; the overview's
  memory was not addressed.
- **A map's pages pop** from far to near when they are shown, 6000 ft away (under the fog on most
  maps). There is no blend.
- **The ride hash** was compared over 30 frames for all four worlds. The sweep only moves where the
  streams look from, not the rider.
