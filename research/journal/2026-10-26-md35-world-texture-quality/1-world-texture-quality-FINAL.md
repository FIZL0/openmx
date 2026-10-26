# 1 — world texture quality (FINAL)

Task MD35. 2026-10-26. Follows MD31 (`2026-10-24-md31-world-texture-quality`), which fixed the mod
maps' page array and was never ticked.

## What was still lost

Shot first, changed after. Release build, headless, GPU (Radeon 8060S), 960x540, the user's real
mods folder, the same `--shot-pose` alone and in `world_ultra` (pose + the tile's `at`):

- **A mod map in a world: no longer lost.** `mxvatv_nat13` alone vs in `world_ultra` (at
  66024,7560), poses near (12 ft over the ground, looking down), mid (40 ft, -12°, trees and rocks in
  view) and far (80 ft, -5°): the ground's grain, the detail map, the trees, bushes and rocks match
  by eye at 1:1 crops (the sky and its light differ: the world's per-map sky). The shell's line:
  `world paint: 77 of 77 paged maps on the GPU, pages 128x128 x 720, 256x256 x 3176, detail
  512x512 x 72` — every map is on the GPU, the 256-pixel pages at 256 (MD31 holds).
- **A disc map in a world: a smear.** `free06` alone vs `mxu_free06` in `world_ultra` (at
  91008,18720) — the tile `world_ultra`, `world_mxu` and `world_mxu_free` all *spawn on*. Near, the
  alone ground has the sand layers' grain; the world's was flat brown, and at mid the rim of the
  bowl was mush. MD15 bakes a disc map's splat into 128-pixel pages (`world_dress::BAKED_PAGE`),
  64 samples a page, so 4.5 ft a pixel, and the splat's layers repeat every 25–100 ft at 256 pixels
  (0.1–0.4 ft a texel): the bake can never hold them. MD31 saw this and left it ("an MXU freeride
  map is still softer in a world than alone"); it is the first ground the user sees in those worlds.
- **Blue patches.** The same world shots had blue-grey stains on lit sand (the bowl's floor, slope
  faces) that alone does not. The bake stores `paint × light ÷ sun` as bytes, and where the baked
  light lifts bright sand past 255 the red and green clip and the blue does not. Confirmed by the
  fix below: they are gone once the factor is stored unclipped.
- Checked and not a cause: the Graphics tab has no Texture Quality or filtering setting (Draw
  Distance, Plants, Leaves, World Sky, Picture, Render Scale, VSync, Frame Limit). **But the user's
  own settings.txt has `render_scale = 50`**: Half, every 3D frame drawn at half the window's size
  and scaled up. That blurs everything, worlds and maps alone alike. I left the user's file alone.
  It may be part of what they see.
- LOD / MD17 / mips / detail tiling: as MD31 found. The ground LOD is geometry only, and page
  sampling is per pixel at every level. The near shots at 12 ft show the full-resolution page.

## What I did

A disc map in a world now draws its splat per pixel, as alone does (`gpu.rs`, `terrain.rs`,
`world_dress.rs`, `worlds.rs`, `splat.rs`):

- `terrain::SharpSplat`: a world tile's splat (`Arc<Splat>`) and its **modulate** pages, one per
  baked page. Each holds what the bake does to the splat's passes, per channel: the roads/ruts drawn
  over them (V67's meshes) and the baked light over the sun. It is stored as a base-2 log byte
  (`modulation_byte`, 128 = one, 32 steps a doubling, 1/16..16), so one is exact and nothing clips.
  `PaintTile` got a `sharp` field. `PaintTile::sharp()` gives it only when the page counts match.
- `world_dress::bake_splat_sharp` bakes both in the same loop (`bake_splat` keeps its signature
  and returns the pages). `Splat::paint_passes` is `paint` without the meshes.
- `worlds::look` keeps each disc map's splat and modulate pages.
- `WorldPaint::pack` gives a sharp map twice its pages: its modulate pages go in the same page
  array right after its baked ones (`world_page_layers`).
- The world shader has a new binding, `splat_layers`, one array of every disc map's layers after
  the last page array (`splat_binding`). A sharp map's block runs its passes there,
  (`splat_passes_wgsl`: `ground_wgsl`'s code with explicit gradients, since a map's block runs in
  non-uniform control flow) and multiplies them by the modulate page.
- The shell's line now ends `…, 5 sharp splats, layers 256x256 x 37` (world_ultra: free06, 07, 09,
  10, 11).

The software renderer still paints a world's disc maps from the bake (see below).

From opentennis: nothing. It has no terrain paint.

## How it was proved

- `crates/openmx/tests/world_texture.rs`
  `a_disc_map_in_a_world_is_drawn_as_sharp_as_alone_near_and_far`: a generated disc map has a
  one-layer splat: a two-pixel checker on 64 pixels, laid every 8 ft and turned 30°. It is drawn
  alone (`set_ground` with the splat) and in a world (baked page + `SharpSplat`) from the same
  camera at 12, 22 and 260 ft. Within 16 per channel: 0 pixels off at each distance (worst 3, 12,
  0). The same world drawn from the bake alone (MD15's way) is off on 46,700 and 46,078 of 57,600
  pixels near and mid, so the test catches the smear.
- `world_dress.rs` unit test `a_sharp_bake_modulates_by_one_without_roads_or_light_and_does_not_clip_the_light`:
  with no meshes and no light map every modulate byte is exactly 128. With a light map of 255 (×2),
  the modulation decodes to the full factor within 2% per channel, while the baked page clips.
- MD31's tests still pass (`world_texture` 4 of 4).
- By eye, release, world_ultra at free06: near, mid and far after the fix match alone's grain, and
  the blue stains are gone.
- Against the original: PCSX2 copy 1, state 4 (free09 at the garage, bike at 4642,3671 from a
  5-frame `watch/acceptance.toml` capture), beside our `free09 --trace-at 4642,3671,0` alone and
  `world_ultra` at the same spot (+76320,25920). The original's ground there is grainy dirt with
  darker patches. The world (sharp splat now) has the same kind of grain. Our free09 *alone* shows
  smooth horizontal streaks there instead. Giving alone's splat the anisotropic sampler did not
  change that, so I reverted it. It is most likely the V67 road/rut meshes over the garage flat,
  drawn alone through their own near target. That is a render difference of the map alone, noted
  and not chased.

## Not verified / not 1:1

- **Roads and ruts in a world are at the bake's 4.5 ft a pixel.** The modulate page carries them,
  so the passes under a road are sharp but the road's own edge and texture are as soft as before.
  Alone draws them through `ground_mesh`'s near target at 4 texels a foot. A world would need a
  near target per disc map. Not done: a lot of machinery, and the complaint was the ground's grain.
- **The baked light is also at 4.5 ft a pixel** in a world (alone samples the 2048-pixel light map
  per pixel, about 2.8 ft a pixel), and it is relit by the world's sun per vertex where alone uses
  the light map alone. Matching by eye in the shots, but not pixel-tested with a light map on the
  GPU (the CPU test checks the factor only).
- **The splat's sampler differs**: in a world it is the ground's anisotropic sampler with gradients
  the shader works out, alone the plain one with the hardware's. That is why the GPU test allows 16
  per channel: at 22 ft over the checker the two parted by up to 12 on 1.7% of the pixels.
- **Splat layers of different sizes** are resampled up to the largest in one array (`upload_array`,
  bilinear). world_ultra's are all 256 (37 layers 256x256), so nothing was resampled here.
- **The software renderer** still paints a world's disc maps from the 128-pixel bake (blurred, and
  with the clipped light). The task was GPU, and software is the reference/fallback.
- **Performance not measured against HEAD.** The machine was at load 42–75 on 32 cores from other
  slots, and a world_ultra run's sim times were load-inflated. The GPU cost per pixel of a disc map
  is the same splat program it runs alone. The ground stage's CPU time was 2.5 ms a frame at 1080p.
  VRAM: +720 128-pixel layers (≈63 MB with mips) and 37 256-pixel layers (≈13 MB).
- The original comparison is one spot on free09 at its garage, by eye. free06 has no save state in
  this slot.
- `render_scale = 50` in the user's settings.txt is a guess at part of what they see. Only they can
  say whether they mean it. Noted for them, not changed.
- world_mxu / world_mxu_free were not shot. They spawn on the same `mxu_free06` tile, and the log
  line shows the same sharp-splat path.
