# 1 — world texture quality (PART)

Task MD31. 2026-10-24.

## What I did

**Where the resolution went.** A world's GPU paint (MD9) put every map's ground pages into one
texture array, and `upload_array` made that array the size of the *first* map's pages, resampling
every other map's to it, nearest. Since MD15 a disc map's ground is baked into pages of
`world_dress::BAKED_PAGE` = 128 pixels, and `world_ultra`, `world_mxu` and `world_mxu_free` all
start with `mxu_free06`, a disc map. So every MX vs. ATV / Untamed / MXU-national map in those
worlds had its 256-pixel pages cut to 128 by nearest sampling — half the resolution, aliased on
top. Ridden alone (`upload_pages`, one map) the pages were always at their own 256. `world_mxvatv`
and `world_untamed` start with a mod map and were not hit. I read the page sizes of the user's own
mod folder with a scratch probe: every mxvatv/untamed `.trn` page is 256 x 256 (one mip), every
`<stem>_d.pst` detail map 512 x 512.

**The fix** (`terrain.rs`, `gpu.rs`):
- `WorldPaint::pack` puts each map's pages in the array of their own size: `Packed::array`, up to
  `PAGE_ARRAYS` (4) sizes, each with its own `max_layers`. `WorldPaint::array_sizes` names them.
- `upload_world_pages` uploads one array a page size (bindings 3, then 6, 7, …); the world shader
  declares `pages`, `pages1`, … and each map's block samples its own.
- `upload_array` (the world's page arrays and its detail array) now takes the *largest* layer's
  size and resamples smaller layers up bilinearly with wrap, so no detail map is ever shrunk.
  `array_shape` / `world_paint_arrays` describe exactly what is uploaded, for the test and the log.
- Page-painted ground (a mod map alone, and a world) is sampled through a new anisotropic sampler
  (`GROUND_ANISOTROPY` = 16; wgpu clamps it to 1 on a device without it). The splat ground keeps
  the plain sampler.
- The shell logs `world paint: N of M paged maps on the GPU, pages 128x128 x 720, 256x256 x 3176,
  detail 512x512 x 72` (that is `world_ultra` here), so a map dropped to dirt by the layer limit is
  visible.

**Checked and not the cause:**
- Mip chain: full, box-filtered, both paths (`mips()`); the world's mip gradient (`gx * k`,
  `k = inner / (64 * spacing)`) is the same as alone's `dpdx(samples * inner / 64)`.
- Detail tiling: each map's `detail_repeat = spacing * DetailMapTilingMultiplier`, sampled in
  tile-local feet in the world shader as alone; the test checks the CPU world paint equals the
  map's own paint at four points.
- LOD (the user's hint): a world piece's chunk centre is moved by the piece's offset
  (`worlds::piece_chunks`) and the level comes from the eye in world feet, as alone; props pick
  their model group from `centre - eye` in world feet too. The ground LOD is geometry only — the
  page sampling is per pixel whatever the level. The near-ground shots below match alone.
- Software path: `WorldPaint::paint` reads each map's source pages directly (`sample_smooth`), at
  source resolution, with the same detail repeat; nothing was lost there.
- Props: same model/texture upload path as alone (`Assets::take_in`, mip-mapped); no downscale
  anywhere in `world_dress`. In the side-by-side shots the trees/bushes match.

From opentennis: nothing — it has no terrain pages.

## What the original does

Nothing to port: the original never draws several maps at once (MD9 says so). MX vs. ATV draws a
map's 256-pixel pages at their own size, which is what alone already did.

## How it was proved

- `crates/openmx/tests/world_texture.rs`:
  - `a_world_uploads_a_mod_maps_pages_at_their_own_size_with_every_mip`: a generated world of a
    baked-size (128) map first and a generated mod map (256 page, 512 detail): the mod map gets
    array 1, arrays are 128x128 (8 mips) and 256x256 (9 mips), detail 512x512 (10 mips); detail
    repeat 8 x spacing; CPU world paint == the map's own paint.
  - `a_world_never_shrinks_a_detail_map`: a 64-pixel detail first, the array is still 512.
  - `a_mod_map_in_a_world_is_drawn_as_sharp_as_alone`: headless GPU, a two-pixel checker page,
    the map drawn alone and the same map in the world from the same (offset) camera: < 1% of pixels
    differ by more than 8. **With the old upload restored** (one array at the first's size,
    nearest) it fails: 28,448 pixels (49.4%) differ, worst 81 — so the test catches the bug.
- `gpu.rs` `every_shader_translates_for_webgl2` now also translates a world of two page sizes.
- By eye: `openmx --headless --track mxvatv_nat13 --trace-at 2600,3000,200` beside
  `--track world_ultra --trace-at 68624,10560,200` (nat13 at 66024,7560 in the world), 960x540:
  the near ground's grain and the detail map match between the two (and earlier at 2000,2000,45
  with trees in view, the props match too). Shots kept out of the repo.

## Not verified / not 1:1

- Not compared against the original PS2 picture: the task's complaint was world vs. alone, and
  alone was already the MD4 reading; rendering is ours, by eye.
- A disc map inside a world is still baked at 128 pixels a page (`BAKED_PAGE`), which cannot hold
  the splat's tiled layers that the map ridden alone draws per pixel; so an MXU freeride map is
  still softer in a world than alone. Left as is: baking at a size that holds the splat's detail is
  far more memory and load time, and the complaint was the mod maps.
- VRAM: `world_ultra`'s 256-pixel array is 3176 layers (≈1.1 GB with mips) where the old single
  128-pixel array was ≈0.3 GB. It ran on this machine (Radeon 8060S, unified memory); a smaller GPU
  may refuse it, and nothing yet budgets it (no compressed formats, no streaming of pages).
- The anisotropic sampler is unproved by a test (no pixel test asserts it); seen only in the shots,
  and a WebGL device without the extension silently gets 1.
- The by-eye shots are one spot on one map (nat13 in world_ultra); world_mxu's MXU nationals
  (also hit: 256 pages after the 128 baked ones) were checked by the `world paint:` log line only.

## Next

The work is done and committed by hand on task/MD31; only the tick is missing. `./kit done MD31`
refused it on a gate that is red for one reason not mine: `game-sim --test replay`
`the_slip_cases_replay_from_the_save_state` ("turn: the path parts at 75, not after 82"). The same
test fails the same way on bare HEAD bb13cfa6 in a scratch worktree (with the slot's kit.toml, iso
and context), and main's 785041dc says its replay gate is red since 1cdaee5a and files S119 for it.
Everything else in the gate passed (the lobby LAN test is a [flaky] load flake and passed on its
lone rerun). Once S119 lands on main, run `./kit done MD31 "<msg>" --skip-audit` (plan/DONE.md's
biplane_tire_* names still fail the audit) with DISPLAY=:0 WAYLAND_DISPLAY=wayland-1 exported.
