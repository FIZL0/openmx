# 1 — ios world memory lean (FINAL)

Task P301a. 2026-11-05.

## What I did

**No phone log to go by.** The task wanted the user's next phone log with P301's memory lines. There
is none: the newest logs (`~/Downloads/openmx-log*.txt`) are from 2026-10-30, before P301, and have
only the launch line's footprint. So this step cuts what P301's desktop breakdown named as the
biggest GPU pieces left, phone-only, and leaves checking against the phone to the next log (HUMAN.md's
P301 steps are unchanged).

**A world's ground held lean on a phone (`game_render::gpu::WORLD_LEAN`).** The shell sets it on a
touch device (`video::world_lean`, beside P301's `world_reach`), and `--world-lean` sets it anywhere,
for measuring. When it is set:
- **World page arrays: layers at most 128 px a side (`LEAN_PAGE`).** A mod map's 256² pages go to 128²,
  and the disc maps' baked pages are 128² already. `page_layer_mips` takes a page bigger than its layer
  by halves as its own mip chain from the layer's size down. That is the same bytes the desktop's
  array has at mip 1, not a resample. The stream (`paint_stream.rs`) already writes at
  `world_paint_layer_size`, so it follows with no change. The pool's layer count is P283's, as before.
  A quarter of the bytes: the pool was 301-384 MB.
- **Detail array: layers at most 256 px (`LEAN_DETAIL`).** `upload_array_shaped` uploads it at a
  `world_detail_shape`. `upload_array` is now that with its own shape, and reads layers through
  `page_layer_mips`, which resamples exactly as `upload_array` did whenever a layer is not that size.
  world_ultra's 97 MB becomes about 24.
- **Ground vertex pages: at most 32 MiB each (`LEAN_VERTEX_PAGE`).** `per_page` was the device's largest
  buffer, so the last page doubled up to that, holding old and new during each grow's copy. Now
  a new page starts once one is 32 MiB, so a grow copies at most 16 MiB and leaves at most one page's
  room unused. The ground already draws from several pages (MD17).
- The `world paint:` load line gives the sizes as held, so a phone log shows `pages 128x128`.

From opentennis: nothing. Its iOS memory work stops at the model texture cap that P223 copied
(`TEXTURE_CAP`). It has no streamed world.

## Measured (desktop, release, 1280×720 headless, phone reach `--world-reach 3000,4500 --world-fine 1500,2200`)

Sampled as in P301: VmRSS plus the DRM fdinfo's `drm-total-vram` + `drm-total-gtt` for the launched
pid, twice a second, the largest RSS+GPU kept. The sampler was a throwaway script in the session's
scratchpad. The machine was at load 100-118 throughout.

| ride | phone reach | + `--world-lean` | ride hash, both |
|---|---|---|---|
| world_mxvatv, 300 frames | 1734 MB (RSS 819 + GPU 915) | 1436 MB (777 + 659) | 0xcba98e7da17eb592 |
| world_mxu, 300 frames | 1881 MB (768 + 1113) | 1634 MB (777 + 857) | 0x0be9ed4cbbfc1b79 |
| world_ultra, 300 frames | 2795 MB (1417 + 1378) | 2293 MB (1375 + 918) | 0x6524290303994f6a |
| world_ultra sweep, 900 frames | 3014 MB (1386 + 1628) | 2498 MB (1377 + 1121) | 0xc80a89a6f717548d |

- The plane hash was 0x3fcc295c14e13abd in every run.
- world_mxu's ride hash is not P301's 0xa55d799967850173. Something merged since has changed that ride.
  Both of this task's runs agree.
- In the sweep, the ground's vertex room went to 7256 chunks without lean and 4361 with it, with
  4472 / 4348 held at most. At ~26 KB a chunk that is ~190 MB against ~114 MB.
- `world paint:` lines with lean: world_ultra `pages 128x128 x 1441, 128x128 x 3176, detail 256x256 x 73`.

## How it was proved

- `gpu::tests::a_lean_page_is_its_own_smaller_level`:
  - `capped_size` halves to the cap;
  - a 256 page in a 128 layer is exactly `mips(page)[1..]`;
  - a page the layer's size is its whole chain;
  - a smaller one is still resampled up.

  `./kit check -- -p game-render --lib -- gpu::tests`: 25 passed.
- `video::tests` checks that a phone is lean and the desktop and the Deck are not.
  `./kit check -- -p openmx --bin openmx -- video`: 3 passed.
- The sim is untouched: the ride and plane hashes in the table are the same lean and not, on three
  worlds and the sweep.
- `tools/ios-build.sh check -p openmx --bin openmx` builds for aarch64-apple-ios. That is a check, not
  a link.
- I looked at it: world_mxvatv at frame 190, 960×540, a shot lean and one not. The lean ground near the
  rider is a little softer and reads slightly darker, because the detail map's finest grain is gone.
  Further out it looks the same.

## Not verified / not 1:1

- **No phone log, and nothing run on an iPhone.** There is no device here, and the user's log with
  P301's lines has not come back. Whether a world ride now stays under ~2 GiB on an iPhone 15 Pro is
  not known. P301 found the desktop never near the phone's 5.2 GiB, so something phone-only may still
  hold the rest. The desktop figures are 1.4 / 1.6 / 2.3 GB, and 2.5 GB in the world_ultra sweep.
- **The cuts are guessed from the desktop's breakdown, not from the phone's.** Metal makes one object
  per resource where Vulkan sub-allocates, so the phone's GPU share and what it is made of may differ.
- **The cut pieces were not measured one at a time.** The table is all three together. The vertex
  cap's share is estimated from the chunk room (~75 MB in the sweep). The pages' and the detail's
  shares are worked out from their sizes, not sampled.
- **The lean picture differs from the desktop's by eye.** Pages are at half resolution and the detail
  map's finest level is gone, which shows near the rider as softer, slightly darker ground. This is
  rendering, so it is not held to 1:1, but it was looked at only on the desktop, on one map, at one spot.
- **ASTC was not tried.** Compressing the pages on a phone would keep their resolution at a quarter of
  RGBA8's bytes. But it needs an encoder on the stream's thread and the ASTC feature on the device,
  and neither can be checked here. Halving the size gives the same bytes and needs neither.
- **A touch browser goes lean too** (`Device::touch`, like P301's reach). It was not run in one.

## Next

- The user's next phone log, with HUMAN.md's P301 steps. If it is still over ~2000 MB, the load line's
  GPU share against the rest says where to cut next. RSS was ~1.4 GB on world_ultra at the phone reach,
  so the CPU-side world (tiles, props, joins) is the next candidate.
