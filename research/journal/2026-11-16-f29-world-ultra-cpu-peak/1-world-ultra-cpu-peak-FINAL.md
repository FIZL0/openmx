# 1 — world_ultra's CPU peak

Task F29. 2026-11-16.

## Where the peak really is

The brief (from P286's journal) put the ~1.9 GB VmHWM in the look's load. Stamping each shell line
with `/proc/PID/status` (VmRSS, VmHWM, RssAnon/File/Shmem; a Python wrapper, `/usr/bin/time` is
missing) on HEAD says otherwise:

| HEAD, world_ultra, 32 cores | VmRSS | VmHWM |
|---|---|---|
| props dressed | 329 | 329 |
| `world look:` | 1108–1151 | same |
| `world paint:` (pages and detail uploaded) | 1322–1371 | 1335–1385 |
| stand-in | 1390–1414 | same |
| `world ground: … built before the ride` | 1833–1865 | 1855–1929 |
| `load:` | 1858–1879 | 1858–1929 |

All of it is RssAnon (file 44 MB, shmem 17 MB, unchanged). The peak is in the ground's first build
(88 pieces on 32 threads: 9 whole maps built at full detail at once, 8 then decimated to coarse),
on top of everything the look left held. With `taskset -c 0-7` (8 cores, the Deck's count) HEAD
peaked at 1417 MB: most of the desktop's extra is 32 threads' worth of maps in flight and their
heaps' slack.

## What I did

- **Skies near the riders (`worlds.rs`).** `look_near` with a near ring reads a sky only for the
  maps it keeps. A far map whose own archive holds its sky (`Sky::held_in`, new in `sky.rs`: the
  name lookup `Sky::load` uses, without reading) is left in `WorldLook::sky_later`. A map whose sky
  would come from the ride's other archives, or that has none, is read at the load as before.
  - `SkyStream` reads a later sky on a thread of its own once an eye comes within the world's
    `Reach::load` of the map, nearest first, and lets it go past `Reach::drop` (the renderer keeps a
    map's cube once uploaded). It mirrors `PaintStream`.
  - A view over a map whose sky is not read yet (a jump: a shot pose, a moved camera) reads it on
    the shell's thread there and then (`SkyStream::need`), so the first frame is right. A sky not
    read is never handed to `set_view_skies`, which would cache "no sky" for the map for good.
  - world_ultra: 9 skies read at the load, 68 left for later. Lines: `world look: … N with a sky of
    their own, M skies read near the riders`, `world sky: X read [at once] at tick T`, `world sky: X
    let go`, and under `--timings` `world sky: N skies read in the ride, at most M held at once`.
- **The look reads 8 maps at a time (`LOOK_READERS`).** At the look line, 743 MB against 1035 MB
  read 32 at a time (same build, `F29L` test switch, since removed). The look's own time did not
  change: 1.5–3.1 s either way across three pairs at load 31–43 (it waits on reads, not cores).
- **The CPU's paint let go before the ground is built (`main.rs`).** P283's hollowing of the maps'
  pages, and a disc map's splat textures, moved from after the ground's first build to right after
  the paint's upload, so the builders reuse that memory. Each map's detail map is dropped there too:
  the GPU's detail array holds them all, and nothing reads the CPU's copy after the upload (the
  software renderer draws only without the near ring; the paint stream writes pages only).
- `world_ultra_load.rs`: the peak bound is 2000 MB (was 2300), and the first test checks that over
  three quarters of the maps' skies are left for later.
- `world_look.rs` `a_far_maps_sky_is_read_when_a_view_jumps_over_it`: `--world-reach 100,150` on
  the two-map world leaves `b`'s sky for later; view 2 posed over `b` reads it at tick 1 and its
  shot is `b`'s blue.

opentennis has no streamed skies or memory work to take.

## Numbers (desktop, release, `--headless --frames 30 --size 640x360 --timings`, load 30–45)

| world_ultra | HEAD | F29 |
|---|---|---|
| VmRSS at `world look:` | 1108–1151 MB | 721–743 MB |
| VmHWM at `load:`, 32 cores | 1855–1929 MB | 1548–1735 MB (7 runs) |
| VmHWM at `load:`, 8 cores (`taskset -c 0-7`) | 1417 MB | 1094 MB |

The F29 peaks spread more than HEAD's: they come from when the 32 ground builders' maps overlap.

**Pictures.** `--world-sky per-map --shot-pose 80000,600,16000,90,5,60` (over mxu_nat02, 24,000 ft
from the start), 640×448: HEAD against F29 differ by 0 px at frame 1 and at frame 10 (`compare -metric
AE -fuzz 3%`). Before `need` existed frame 1 showed the home sky (29,730 px), which is why it is there.

## Checked

- `./kit check -- -p openmx --test world_look`: 11 tests passed; the new one's shot was looked at.
- `./kit check -- -p openmx --test world_ultra_load --test world_streaming --test world_first_frame
  --test wrapped_world`: 11 tests passed. `cargo test -p openmx --test world_ultra_load --
  --nocapture` ran both world_ultra tests, neither skipped: peak 1717 MB under the other slots' load.
- The full gate in `./kit done`.

## Not verified / not 1:1

- **The Deck.** Not measured on it (no `ssh deck` in runs). The 8-core number is `taskset` on the
  desktop, the Deck's thread count but not its allocator, GPU driver or disc speed.
- **The ground's first build is still the peak** on the desktop: ~500 MB over the stand-in with 32
  builders. Capping them at 8 gave 1466 MB against 1615 MB in one run, but its load-time cost could
  not be measured with the machine at load 30–140, and it was not in the brief. Filed as a follow-up.
- **The GPU's detail array still holds every map's detail** (73 layers of 512×512). Only the CPU's
  copies go now. Streaming the layers like the pages would be a renderer change; GPU memory is not
  VmHWM, so it was left.
- **glibc arenas.** Fewer look readers keep fewer arenas' slack, but freed memory is still kept
  (`malloc_trim` is out: `#![forbid(unsafe_code)]`). Not measured under `MALLOC_ARENA_MAX=1` after.
- **A sky read mid-ride on the stream's thread** was seen only through the shot-pose run (6 read at
  tick 2) and the two-map test. No ride crossed into a far map's sky reach on screen.
- **`need` reads a sky on the shell's thread**: a hitch of one archive open and a `.cub` decode when
  a camera jumps. Not timed.
- **Load time.** The wall clock could not be compared fairly (load 30–140). The look's own span is
  unchanged; CPU time to the first frame read 33–37 s for both builds.

## Next

- F29a (low): the ground's first build, now the desktop's peak.
