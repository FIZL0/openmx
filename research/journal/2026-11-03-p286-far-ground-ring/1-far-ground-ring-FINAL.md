# 1 — world_ultra's far ground, coarse (FINAL)

Task P286. 2026-11-03.

## Where world_ultra's ground went

P283 left world_ultra at a 4.4 s load and a ~1.9 GB VmHWM. It said the biggest part left was the
full-detail GPU ground out to the 6000 ft load reach: 15,423 chunks, 600–750 MB.

I measured that ground from the process's DRM fdinfo (`/proc/PID/fdinfo/*`, `drm-total-vram`),
read after the `load:` line. VmRSS does not count it on this desktop: the 15,423 chunks are in
VRAM and GTT, not in RSS. The 1.9 GB VmHWM is CPU memory, and it peaks in the look's load, before
the ground is built. On the Deck, VRAM and GTT come out of the same 16 GB, so the ground counts
there in full.

## What I did

- **`coarse` (worlds.rs).** It takes a piece's chunks as `piece_chunks` built them and decimates
  them by `COARSE_BY` = 4 each way.
  - The chunks keep the same cells per chunk, so the GPU's slots and LOD patterns are unchanged.
    Each chunk is 4× as wide, so there are a sixteenth as many.
  - A corner is the sample at every 4th point. It comes from the chunk that draws a cell beside
    it, before one that does not.
  - A far cell is left out only when every sample cell under it is. A cut between two pieces
    therefore overlaps rather than leaves a gap.
  - Corners past the field's edge stop at the edge, as the full chunks' do.
- **`GroundStream` has two levels.**
  - Each piece is held not at all, coarse, or fine (`held`), and is asked at one level (`asked`).
  - `wanted` picks the level. A piece goes fine within `FINE_REACH.load` (2500 ft) and back to
    coarse past `FINE_REACH.drop` (3500 ft). It is held at all within the world's `Reach`, as
    before.
  - A level change is a fresh build on the stream's thread. When it arrives, its parts replace
    whatever of the old level is still pending.
  - A piece that would not build is marked `failed`, and is not asked again, as before.
  - Coarse ground under a rider is waited for, as a missing piece was.
- **The shell.** `--world-fine LOAD,DROP|off` sets the reach; `off` builds everything at full
  detail, as before. `--world-ground all` is always full detail. The `world ground: … built before
  the ride` line says how many pieces are coarse.

## Numbers (desktop, release build, `--headless --frames 30/600 --timings`)

| world_ultra | full detail (`--world-fine off`) | coarse past 2500 ft |
|---|---|---|
| chunks at the start (room) | 15,423 (18,462) | 3105 (4768) |
| GPU memory (`drm-total-vram`), after the load | 1988 MB | 1464 MB |
| GTT resident | 1410 MB | 885 MB |
| VmHWM | 1878–1900 MB | 1871–1909 MB (no change, see above) |
| first frame (`load:`) | 4.41 s | 4.35 s |
| ride hash (30 frames) | 0xb363ed99829fb5f3 | 0xb363ed99829fb5f3 |

**Sweep.** The run was `--world-sweep 56000,16000,110000,16000 --frames 900`.
- Chunks held at once fell from 20,521 to 8575. The room fell from 20,540 to 9536; the ground's
  vertex pages never shrink, so the room is the peak.
- Chunks written in the ride fell from 43,171 to 22,678. The coarse ring did 417 coarse builds
  and 248 full ones. No frame waited on a piece.
- The ground pass took 0.39–0.83 ms a frame against 0.63–1.04 ms. The worst frame was 17.55 ms
  (stream 5.0) against 17.76 ms (stream 4.6), with 1 over 16.7 ms in each.
- The ride hash was 0xc80a89a6f717548d in both.

**Pictures.** All are 640×448, the full-detail run against the coarse one (`compare -metric AE
-fuzz 3%`). Two runs of one build differ by about 1200 px from plants and dither (P283).

| shot | px that differ |
|---|---|
| default camera, frame 30 | 0 |
| 500 ft up, pitch −6°, yaw 31° | 77 |
| 500 ft up, pitch −6°, yaw 211° | 153 |
| 2600 ft up, pitch −25° | 42 |

The ground's own LOD already drew those chunks at its coarser levels at that distance.

## Checked

- `./kit check -- -p openmx --bin openmx -- far_ring heading_order`: 4 tests passed.
- `./kit check -- -p openmx` with world_ultra_load, world_streaming, world_far_tiles,
  world_first_frame, wrapped_world, wrapped_world_draw, world_picture, stitched_seams,
  world_texture, world_look, disc_worlds and one_fps: 42 tests passed.
- `cargo test -p openmx --test world_ultra_load -- --nocapture` ran both tests, and neither was
  skipped. They now also check the coarse count, under 6000 chunks at the start and under 12,000
  in the sweep.

## Not verified / not 1:1

- **The Deck.** Nothing was measured on it (memory profiling-on-the-deck: no `ssh deck` in a run).
  The GPU's −524 MB is a desktop number from fdinfo. On the Deck that memory is shared with the
  CPU, so the saving should land in full, but that is not measured. Filed for a person as R23 (later).
- **The swap between levels pops.** Adding the new level drops the old piece's chunks at once
  (`put_ground_piece` with `fresh`). The stand-in then shows under the new chunks while they fade
  in (V92), so the order is coarse → stand-in → fine. This happens at 2500 ft coming in, and at
  3500 ft going out. It was not watched on screen, only in the sweep's numbers and the stills
  above. It is a render difference, so per AGENT.md no task was filed for it.
- **Seams between a coarse and a fine piece** are not stitched. They were not stitched between
  pieces before either (`add_ground_piece`'s note), but a coarse edge leaves wider slivers: up to
  4 samples' worth of height difference along it, 2500 ft or more away. No crack was seen in the
  stills.
- **The CPU still builds every far piece at full detail** before decimating it. The work moved to
  the stream's thread, and the load time is unchanged.
- **VmHWM is still ~1.9 GB.** Not done here:
  - the skies (115 MB, all 77) and the detail array, which still load whole;
  - ~270 MB of glibc arenas kept by the 32 look readers. `MALLOC_ARENA_MAX=1` gave 1650 MB (P283).
  - Both are filed as F29 (low).
- **The 2500 / 3500 ft fine reach** was chosen by these stills, not tuned per map or for a Deck.
  It can be changed with `--world-fine`.
