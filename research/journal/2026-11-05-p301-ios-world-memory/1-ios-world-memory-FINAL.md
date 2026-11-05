# 1 — ios world memory (FINAL)

Task P301. 2026-11-05.

## What I did

**The phone log's memory lines.** Before this, `peak_memory` read only Linux's `/proc/self/status`,
so the iOS `load:` line said "memory not known here" and the line a second had no memory at all.

- `openmx_ios::footprint()` (sys.rs) returns three figures from `task_vm_info`:
  - `phys_footprint`, which is what iOS closes the app by;
  - `ledger_phys_footprint_peak`, the 22nd u64, present only if the kernel wrote at least 44 natural_t's;
  - `os_proc_available_memory`.

  `memory()` (the launch and exit lines) is now built on it, and reads the same as before.
- `openmx_ios::gpu_bytes()` is the system Metal device's `currentAllocatedSize`. That is the device wgpu
  draws with, so the figure is the GPU's share of the footprint.
- `peak_memory()` on iOS gives this load line:
  `load: … memory peak N MB, now N MB (N MB of it the GPU's), N MB left before iOS closes the app, cpu …`.
  On Linux the line is unchanged (VmHWM / VmRSS), so `world_ultra_load` still reads it the same way.
- `memory_now()` adds `, memory N MB (N MB left, GPU N MB)` to the line a second on iOS, before any
  `, N views` (drop_in reads the line by that ending). On the desktop it adds nothing. My first version
  read VmRSS there once a second, but that file read made `one_fps::nothing_is_read_or_decoded_after_the_first_frame`
  see bytes read after the first frame. The desktop figure was only for my measuring; the sampler below
  reads it from outside. The `memory:` line at the end of a ride uses `peak_memory`, as before, so the
  iPhone has it too.
- `crates/stitch/worlds/world_mxu_free.start.json` was re-keyed. `disc_worlds` was red on "its start
  file is stale": another merged change had moved the cache key. Only the `key` changed
  (f06b0c54375601ea → 8a2309e7bba75399); the start is the same.

**A phone holds a world nearer.** `video::world_reach` uses `Device::here()`. On a touch device, the
iPhone app or a touch browser, it returns:

| | load | drop |
|---|---|---|
| world reach | 3000 ft | 4500 ft |
| full-detail ground | 1500 ft | 2200 ft |

The desktop and the Deck keep 6000/9000 and 2500/3500. `parse_args` sets this before reading the flags,
so `--world-reach` and `--world-fine` still override it. One reach drives the sim's `Tiled` streaming,
the props and plants dressing, the paint pool and the stream, and the ground stream.

From opentennis: its `hst-base/src/ios.rs` has the same `phys_footprint` read that `memory()` already
copied (B538). It has no peak figure, no Metal figure and no per-second memory, so nothing new was taken.

## Where a world's memory goes (desktop, release build, 1280×720, headless, 300 frames)

How it was measured:
- RSS is VmRSS, sampled twice a second by a Python launcher that holds the pid.
- GPU is the process's DRM fdinfo `drm-total-vram` + `drm-total-gtt`. This desktop's GPU is an APU (Strix
  Halo, shared memory), which is the closest thing here to an iPhone's unified memory. It is not the
  same allocator, though: wgpu's Vulkan backend sub-allocates in blocks, while Metal makes one object
  per resource.
- The per-resource breakdown came from a throwaway patch. It wrapped gpu.rs's 33 `create_texture` /
  `create_buffer` sites to log each one's label and bytes. It was reverted and is not in the commit.

world_mxvatv at the desktop reach: about 800 MB of GPU resources were made in all.
- **World ground pages: 384 MB.** This is one RGBA8 array with mips. P283's pool is sized to the maps
  within the 9000 ft drop of any one map.
- **Ground vertex pages: 130 MB.** They grow by doubling, never shrink, and hold old + new during a
  grow's copy.
- Ground indices: 37 MB.
- World ground detail array: 36 MB. On world_ultra it is 97 MB: 73 layers at 512².
- Everything else is under 15 MB: offscreen/depth targets, models, skies, far atlas, ground cover.

The largest RSS+GPU seen in each run:

| ride | desktop reach | phone reach | ride hash, both |
|---|---|---|---|
| world_mxvatv | 2233 MB (RSS 946 + GPU 1287) | 1644 MB (792 + 852) | 0xcba98e7da17eb592 |
| world_ultra | 3351 MB (1837 + 1514) | 2774 MB (1402 + 1372) | 0x6524290303994f6a |
| world_mxu (MX Unleashed: WORLD), joins cached | 3345 MB (1691 + 1654) | 2318 MB (1079 + 1239) | 0xa55d799967850173 |
| world_mxu, joins built from cold | — | 2335 MB (1221 + 1114) | 0xa55d799967850173 |
| world_ultra sweep, 900 frames | 3603 MB (1826 + 1777) | 3030 MB (1399 + 1631) | — |

- The plane hash was 0x3fcc295c14e13abd in every run.
- In the sweep, the ground vertex room went to 238 MB at the desktop reach and 181 MB at the phone reach.
  RSS stayed flat over the ride: 1399 MB at the phone reach. At the desktop reach it fell from 1826 to
  1649 MB.
- The browser's reach (2000/3000, fine 1000/1500) only went to 2359 MB on world_ultra. Most of the
  remainder is not the reach.
- `MALLOC_ARENA_MAX=2` took world_ultra's RSS from 1402 to 1186 MB at the phone reach. That is glibc's
  arenas, and iOS's malloc has none.
- MX Unleashed: WORLD's first-time joins build peaked at 1221 MB of RSS, no higher than the ride after it.
  The build's buffers are not what is held.

## How it was proved

- `video::tests` covers the reach: a phone gets 3000/4500 and 1500/2200; the desktop and the Deck get none.
  `./kit check -- -p openmx --bin openmx -- video`: 3 passed.
- The sim is untouched: the ride and plane hashes above are the same at both reaches on three worlds,
  300 frames each.
- `PATH=$HOME/.cargo/bin:$PATH tools/ios-build.sh check -p openmx --bin openmx` builds for
  aarch64-apple-ios. That is a check, not a link.
- The Linux load line is unchanged. `./kit check -- -p openmx --test drop_in --test one_fps --test
  disc_worlds`: drop_in 4 and disc_worlds 4 passed. one_fps's frame budget read 87.8 ms on the software
  rasteriser at load average 57. It is an `[alone]` binary, so its rerun in `./kit done` holds the
  machine. In the first done run that rerun passed the budget. Nothing of this change runs a frame on
  the desktop.

## Not verified / not 1:1

- **Nothing was run on an iPhone.** There is no device here.
  - The 5.2 GiB footprint was not reproduced. Whether the phone reach brings a world ride under ~2 GiB on
    an iPhone is not known. On the desktop, RSS + GPU at the phone reach is 1.6 GB (world_mxvatv), 2.3 GB
    (world_mxu) and 2.8 GB (world_ultra). The desktop never came near 5.2 GB, so something phone-only adds
    the rest. The new lines (footprint, peak, GPU) are there to find it. HUMAN.md has the steps, and
    follow-up P301a cuts what they show.
- **The iOS lines' figures are untested.**
  - The `task_vm_info` peak field's offset (`ledger_phys_footprint_peak`, the 22nd u64) is from the XNU
    header layout, not checked on a device. If the kernel writes fewer words, the peak falls back to the
    footprint now.
  - That `MTLCreateSystemDefaultDevice` is the device wgpu uses comes from wgpu-hal's Metal backend; iOS
    has one GPU. It was not checked on a device either.
  - The iOS build was `check`ed, not linked. Metal.framework is already linked by wgpu.
- **The phone reach's picture was not looked at.** Past 3000 ft the stand-in's coarse ground is drawn, and
  past 1500 ft the coarse chunks (P286). Phone fog is the Far preset's, but where it ends against 3000 ft
  was not measured. A ride across a map edge was not watched for pop-in: the sweep has the numbers, and no
  frame waited on a piece.
- **GPU figures are a desktop APU's fdinfo.** Vulkan sub-allocation blocks inflate them against the
  per-resource tally (about 800 MB made against 1287 MB in fdinfo). Metal's figure will differ.
- **A touch browser gets the phone reach too** (`Device::touch`). It was not run in one.

## Next

- P301a: the user's next phone log, then cut what it shows. The biggest remaining pieces on the desktop
  are:
  - the world page pool, 301–384 MB of RGBA8 (a smaller pool, or ASTC, on a phone);
  - the detail array at 512² (97 MB on world_ultra);
  - the ground vertex pages' doubling grow.
