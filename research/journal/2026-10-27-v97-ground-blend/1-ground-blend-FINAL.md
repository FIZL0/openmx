# 1 — ground blend (FINAL)

Task V97. 2026-10-27.

## What I did

- Shot the original (PCSX2 copy 2, ride states on free09 and free06) and openmx at the same pose
  (`--shot-pose` from the live viewport), 936×702, side by side. The original's ground is a smooth,
  low-contrast wash; the port's had high-frequency, contrasty grain everywhere, and crisp rock and
  edges on the bowl walls.
- Found the cause (below): the original reads every layer and the detail map at a mip level picked by
  a paint camera of its own that stands high over the ground — level ~2 almost everywhere on free09.
- `splat.rs`: each `Pass` now carries a `Mip` (`MipMap_1` levels, `K/16` bias, a box-filtered chain)
  read from the `.psm`; `Splat::paint_at` / `paint_passes_at` read each pass at its level for a depth;
  `paint_depth` (and its WGSL) is the original's paint-camera depth; constants `PAINT_EYE_LEAST`,
  `PAINT_EYE_OVER`, `PAINT_PITCH`, `PAINT_NEAREST`.
- GPU (`gpu.rs`): `ground_wgsl` (a track's ground) and `splat_passes_wgsl` (a world's disc-map splat,
  the MD9/MD35 join path, through `ground_world_wgsl`) sample every mipped pass with
  `textureSampleGrad`, its gradients stretched so the footprint is never smaller than the
  paint camera's level (`mip_floor`). The renderer's own trilinear/anisotropic filtering goes on from
  there where the screen minifies more.
- Software: `ground.rs` paints each pixel with `paint_at` at its paint depth (camera eye/forward);
  `world_dress.rs` bakes a world's pages at `PAINT_NEAREST` (a page cannot know the camera).
- Test: `crates/game-render/tests/ground_blend.rs`,
  `a_layer_edge_fades_over_the_originals_width_not_one_texel` — a black/white step layer with
  free09's `MipMap_1 = 2, K = -65`: software 10–90% fade 0.8 ft at full size, 2.9 ft at the nearest
  paint depth (level 1.72), ≥2.5 ft at the last level; the GPU frame from 300 ft above (where the
  screen magnifies the layer, so only the floor can soften it) fades over 2.8 ft.

## What the original does

- Layer passes of `terrainscreenspacemesh.psm` carry `MipMap_1 = 2`, `MagFilter = 1`,
  `MinFilter = 5` (trilinear) and `K` (GS LOD bias, 1/16 units): free09 −65, free06 −75 (detail −75;
  −100 in `free06_ss.res`). The masks name none of these. GS LOD with `LCM = 0` is
  `log2(1/Q) + K/16` = `log2(depth) + K/16`, clamped to `[0, MXL]`.
- The terrain loader (`./kit fn 0x1d9930`) creates a "TerrainScreenSpace" camera (terrain object
  `+0x924`). Every frame `./kit fn 0x1dafb0` copies the player's viewport into it, then sets its eye
  height to max(the float at 0x420830, the float at 0x420838 + …) — at runtime 75 and 15 (the ELF holds
  15/15; something writes the 75). If the view looks down steeply (−fwd.y − 0.5 > 0) it lifts the
  camera by f·400 (0x49f520), the far plane by f·400 and the fov by f·5 (multiplayer 500 / 7 / 0.3).
  The paint is drawn on a flat plane.
- Live readings (terrain pointer at 0x49f518; camera eye +0x140, fwd +0x150, fov +0x170, far +0x1a0):
  free09 eye.y 75, fwd.y −0.7323, fov 106.8, far 1000 (player fwd.y −0.317);
  free06 eye.y 75, fwd.y −0.7385, fov 108.9, far 768 (player fwd.y −0.455).
  Player viewport for `--shot-pose`: 0x16dcf80 on both states (0x946a50 also on free06).
- So no ground is nearer than ~55 ft (75·sin 47°) to the camera that reads the layers: level
  log2(55) − 65/16 = 1.7 at the nearest on free09, 2 (the last) past ~67 ft — quarter-size layers
  nearly everywhere. That is the softness. Mask edges were never the difference: masks are unmipped
  and the port already reads them bilinear (one 27-ft texel wide in both).

## How it was proved

- Looked right beside a screenshot, and measured: grey band-pass contrast (blur 2 minus blur 8, std
  ×255) over the mid ground (300×120 at 600,300 of 936×702), original / before / after:
  free09 0.68 / 2.69 / 0.67; free06 1.08 / 2.70 / 1.08. Coarser band (6–20): free09 0.97 / 2.29 / 1.45,
  free06 1.37 / 2.95 / 2.30. Finest band (0–3): free09 0.69 / 2.53 / 0.28, free06 0.83 / 2.17 / 0.40.
  Shots: f09a/f06a pairs (pose free09 4309.88,215.03,3305.32 yaw 0.008 pitch −18.48; free06
  2851.69,249.53,3095.15 yaw −6.76 pitch −26.76; fov 85), not committed.
- The task test above (software and GPU), and `./kit check --all` green (2616).

## Not verified / not 1:1

- The paint camera's pitch is a fit to two live readings (sin 0.735), not read from code; it may vary
  with the player's pitch more than these two show. The steep-look lift (f·400 height, fov +f·5) is not
  modelled, so looking steeply down the port is a little sharper than the original.
- Where the runtime 75 ft comes from is not found (the ELF has 15); a track or mode that sets another
  value would differ. Multiplayer constants are not modelled.
- The "15 + …" term: modelled as 15 over the player's eye height above the point; only the 75 floor
  was ever seen to win. The software path does not know the ground's height per pixel and uses the
  eye's, so there the 75 ft floor always wins.
- The depth model treats the ground as the flat plane the original paints on; the paint target's own
  resolution (512×832 single player) and its bilinear read back onto the mesh are not modelled. That
  is probably why the finest band is now softer than the original's (0.28 vs 0.69) while the mid band
  matches: the target's texel grid adds its own fine grain.
- Mips are box-filtered halvings, not whatever the disc's `.pst` mip levels hold (if any).
- World pages (software world path) are baked at the nearest paint depth, so they are a little
  sharper far away than the original's would be.
- free06 shots: the state keeps rolling, so the PCSX2 pose was read just before the shot and the
  port's is that pose, which may be off by a frame. Only one spot a track was compared ("near and far"
  is the near ground and the bowl wall in the same frame); far-only spots were not shot separately.
- The bowl walls on free06 still look softer in the port than in the original (the original keeps
  more strata contrast); not explained — maybe the light map or the steep wall's stretch on the target.

## Next

- V99 (filed): the paint target's resolution and the finest-band grain, and free06's bowl walls.
