# 1 — ground grain (FINAL)

Task V101. 2026-10-29.

## What I did

- Shot the original (PCSX2 copy 3) and openmx at the same pose on free09 and free06. I took raw
  internal-resolution PCSX2 shots (`ScreenshotSize=2` in the copy's ini, 639x448) so the frame
  buffer's own pixels can be read, and openmx `--size 639x448 --headless --frames 5 --overlay none
  --trace-at … --shot-pose …`.
  - free09 pose: eye 4202.86,143.45,3689.8, yaw −17.53, pitch −18.38, fov 85 (the state comes to rest
    there by itself).
  - free06 pose: 2838.24,245.51,3141.72, yaw 3.23, pitch −18.19, fov 85 (square + R2 held to stop the
    bike).
- Measured band-pass contrast (std of grey between Gaussian blurs 0-1, 1-2, 2-4, 4-8, 8-16 px) over
  regions of each pair. Scripts and shots are in the session scratchpad, not committed.
- Found the original's finest band (0–1 px) is the frame buffer, not the ground (below). Not copied.
- Found V97's `mip_floor` blurred ground seen at a grazing angle along its depth far past the floor:
  it multiplied both screen gradients by one factor `floor / min(|gx|, |gy|)`.
- `splat.rs`: `paint_depth_wgsl` now emits `paint_texel`, `mip_width` and `floor_grads` in place of
  `mip_floor`. `floor_grads` takes the pixel's footprint `J Jᵀ`, adds `w² I` (the floor as a blur
  the same every way: spreads add), and returns gradients along the eigenvectors with lengths
  `sqrt(λ + w²)`. The long axis barely grows; the short one is floored.
- `splat.rs`: `PAINT_TARGET` (512x832), `PAINT_FOV` (108.9°, 1.5), `paint_texel(depth)` (a target
  texel's width on the ground: depth × geometric mean of across and along, × √2 so a box spreads as far
  as the bilinear read back's tent). `Mip::level_through_target` is the software path's level with the
  target's texel added in quadrature; `passes_frame` uses it for mipped passes.
- `gpu.rs`: the track shader (`ground_wgsl`) and the world splat (`splat_passes_wgsl`) sample mipped
  passes with `floor_grads(…, mip_width(size, level, paint texel, scale))`.
- Test `crates/game-render/tests/ground_grain.rs`: the target texel's size and the level's
  monotonicity, and a GPU edge seen from a rider's 6 ft eye at 8.5° to the ground. It fades over 2.5 ft
  along the depth with this change (floor 4 ft), and over 16.8 ft on V97's code, which fails the test.

## What the original does

- Paint target: each freeride track's own archive (`free09.res`, `free06.res`) holds
  `terrainscreenspaceprologue.psm` with `renderTargetWidth 512`, `renderTargetHeight 832`,
  `textureFrameBufferFormat 0` (32-bit). The `_ss.res` archives say 384x448, but a ride does not load
  those. The ground reads the target through `terrainusescreenspace.psm`
  (Vu1TerrainRendererUseScreenSpace, clamp wrap, magFilter blank).
- The layers' passes in `terrainscreenspacemesh.psm` say `lodCalculationMethod = 0`, so the GS picks
  LOD from Q with K: V97's depth law stands.
- Paint camera, live on free09: the terrain object at 0xefcf20, its camera pointer at +0x924 →
  0xefd910. Fields from +0x140: eye (x, 75, z); fwd (−0.287, −0.731, 0.619); fov +0x170 108.887;
  +0x88 183.28; +0x8c 1.499, taken as the aspect; far 1000. The per-frame copy is `./kit fn 0x1dafb0`.
  It copies the player camera into it, lifts it, and adds the steep-look lift (V97's reading).
- Player viewport for `--shot-pose`: 0x174d580 on the first_recording free09 state (eye, fwd +0x10,
  up +0x20, fov +0x30); 0x946a50 on `context/s89/free06-bike.p2s`.
- The frame: a raw shot has every channel a multiple of 8, so the frame is 16-bit 5:5:5. It is
  dithered with the PS2's 4x4 matrix: a screen-fixed period-2 pattern over everything, the HUD's
  gauge bar included. Applying that dither and 5-bit quantisation to the port's frame raises its 0–1
  band from ~0.4 to 2.66–2.72, against the original's 2.91. So the original's finest band is the
  dither, and the ground's own grain is the 1–4 px bands.

## How it was proved

Looked right beside screenshots, and measured. Std of grey between blurs, per band, original /
HEAD / this change / no floor at all (the experiment, not shipped):

- free09 mid ground (y100–300, x420–620 of 639x448):
  - 1–2: 0.90 / 0.28 / 0.59 / 1.88
  - 2–4: 1.13 / 0.73 / 1.14 / 2.37
  - 4–8: 1.51 / 1.49 / 1.80 / 2.60
- free09 far (y40–200, x20–250):
  - 1–2: 1.72 / 0.55 / 0.93 / 2.16
  - 2–4: 2.42 / 1.23 / 1.59 / 2.39
  - 4–8: 3.03 / 2.07 / 2.30 / 2.66
- free06 bowl wall (y30–130, x200–600):
  - 1–2: 1.93 / 0.64 / 0.67 / 2.49
  - 2–4: 2.96 / 1.30 / 1.34 / 2.88
  - 4–8: 3.40 / 2.25 / 2.30 / 3.02
  - mean: 88 / 139.5 / 139.5 / 139

Mean |log2(port/original)| of mean-normalised contrast over four regions (bands 1–8): HEAD 1.63, this
change 1.32. By eye, free09's ground now shows the original's horizontal streaks (isotropic ~0.7 ft
detail foreshortened) where HEAD smeared them along depth.

Other floors tried and not shipped:

| floor | score |
|---|---|
| layer level capped at 1 | 0.90 |
| target texel only, no layer level | 0.63 |
| no floor | 0.55 |

The layer-level-free variants overshoot the mid ground, where V97 matched the original's mid band
(0.67 vs 0.68). They also fail V97's GPU fade test.

Tests: `ground_grain.rs` (2) and `ground_blend.rs` (V97's, still green).

### free06's bowl walls

- The floor flattens them. With no floor the wall's absolute grain is the original's (2–4 px: 2.88
  vs 2.96). Measured against its mean, it is still about half the original's: the port's wall is light
  sand, mean 139, and the original's darker grey-brown rock, mean 88.
- The wall is the stretch the `Free06_Erode` mesh covers: z ≈ 3186–3242 at x 2845, V77's "p3 cliff"
  spots. There the masks read dirt 255, erode ≤ 107, rock 2, and the light map 1.25–1.39 (brighter
  than the flat at its foot).
- The program's erode mask does not follow the cliff bands. Its correlation with the erosion mesh's
  coverage over the map is −0.04 as placed, and at most 0.17 under any swap or flip of u and v (the
  first mask: 0.25 as placed). So the masks are not misplaced; they just are not cliff masks.
- V77 left the erosion mesh out on evidence (the flat at a band's foot is not dark in the original).
  With it, the wall's paint drops from ~135 to ~75 brown. That is closer in tone, but V77's spots
  miss by twice as much.
- So the wall's remaining look is content and light, not filtering. The original also keeps more
  contrast far away and on the wall than a 512-wide, unmipped target read back bilinear could, if it
  were filtered. Its target has no mips, so where a screen pixel spans several target texels it
  aliases rather than averages. That keeps band energy the port's filtered read does not have, and
  it is not copied (rendering is ours).
- The disc's own `.pst` mips were checked against box halvings: they are a little *softer* (grass L2
  std 5.4 vs 5.7), so they do not explain the original's extra grain.

## Not verified / not 1:1

- The dither and 16-bit frame are not copied (AGENT.md: do not replay the GS). The original's 0–1
  px band (2.91) stays far above the port's (~0.5).
- The target is modelled as a blur on the ground, not drawn and read back: no aliasing where a screen
  pixel spans several target texels, no row/column anisotropy (one width, the geometric mean), and the
  along term uses the fixed 47° pitch, not each point's own angle to the paint camera.
- `PAINT_FOV` (108.9, 1.5) is one live reading. Taking +0x8c (1.499) as the projection's aspect is a
  guess from its value, not from code.
- The read back's filter is assumed bilinear. The psm leaves `magFilter` blank, which could mean
  nearest.
- Measured, the target term changes the GPU frames by less than 0.02 in any band: the layer's level
  floor dominates it at every depth seen. It is there because the task asks for it and because a fine
  layer (≤0.1 ft a texel) would feel it.
- The far ground keeps less contrast than the original's (free09 far 1–2 px: 0.93 vs 1.72). Part of
  that is the original's unmipped target aliasing, which is not copied; the rest is not separated out.
- The free06 bowl walls still look lighter and flatter than the original's: their content (dirt
  paint, bright light map) and the floor both play a part. Neither the masks nor V77's dropped erosion
  mesh explains it (above). Not fixed. Per AGENT.md no render follow-up is filed.
- The original's ground is darker than the port's on both spots (mean ×0.80 on free09, ×0.63 on the
  free06 wall). That brightness is not this task's.
- Only one pose a track was compared.

## Next

none
