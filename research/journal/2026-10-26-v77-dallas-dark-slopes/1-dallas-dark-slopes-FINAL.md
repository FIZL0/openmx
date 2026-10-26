# 1 — dallas dark slopes (FINAL)

Task V77. 2026-10-26.

## What I did

- Shot five spots in PCSX2 (copy 2, a free06 bike state teleported by a scratch `tele.py`, pose read
  from the ride viewport in RAM, PCSX2's own System > Screenshot): p3 (the cliff north of the start),
  m1, r1 (the shaded slope west of the start), r3 (the flat at the foot of the bushy cliff east of
  the start) and r4 (the bank north-west of the start). The shell was shot at the same eye, yaw,
  pitch and fov 85 with `openmx --track free06 --headless --frames 5 --overlay none --trace-at …
  --shot-pose x,y,z,yaw,pitch,85 --shot`. One extra shot at r3 went through `./kit shot`.
  The side-by-sides are in `context/shots/v77/` (git-ignored): `*_side.jpg` shows PCSX2 | HEAD | fixed.
- Ruled out, from 8x6 grey grids of each shot against the PCSX2 one:
  - GPU vs software: the two look the same, and so do render scale 0.5 and a windowed run.
  - The light map's edge, and its scale: the tiles' `Cd * As` with `Shaderrgba` alpha 255 is the
    tile's grey over 128, as `Shadow::at` has it.
  - The vertex sun on top of the light map, inside it: tried in a scratch shader switch. It is
    brighter everywhere and fits worse, with and without the meshes.
  - Drawing the meshes after the light (unlit): almost no change, and the p3 cliff gets darker.
  - The lowercase `angle = 45` on passes 3, 5 and 7: `ini` keys are case-blind, so it is read.
  - The masks' placement: free06's erode mask correlates with the terrain's slope at 0.63 as placed,
    and at 0.32–0.34 mirrored or transposed.
- What darkens them is the `Free06_Erode` mesh (V67). It covers 2–7% of the map, most thickly the
  45–56° cliffs, and lays the program's own `free06_erode` texture over them at full alpha
  (3231 of 3606 vertex alphas are 128). By eye the slab is the dark area in every shot, and the
  original has none of it: at r3 the original reads 123 → 120 → 115 across the band's edge, where
  HEAD drops 163 → 69.
- `ground_mesh::repeats_a_layer`: whether a model's material is textured with one of the program's
  textures. Only `Free06_Erode` is, on every freeride track (checked free06, 07, 09, 10, 11). `splat.rs`
  `drawn_meshes` drops those placements before `GroundMeshes::load`, asking once per model. Nothing in
  `gpu.rs` or `ground.rs` changed: both take `Splat::meshes`.
- Test `crates/game-render/tests/dallas_slopes.rs`: 28 named spots (shot, cell, world x/z, the
  original's 40x40-patch grey). It takes the port's paint × light averaged over 9x9 ft, fits one scale
  and requires: the scale in 0.55–0.9, the mean miss under 15%, every spot under 40%, and the mesh
  laid back on to miss at least 1.4× more. Run: scale 0.68, mean miss 12.9%; with the erosion mesh,
  scale 1.03, mean miss 20.8%.

## What the original does

It draws the screen-space program, then the scene's `Type=TerrainScreenSpace` models, into a 512x832
target (`terrainscreenspaceprologue.psm`). The terrain samples that target
(`terrainusescreenspace.psm`: `Vu1TerrainRendererUseScreenSpace`, `TerrainColor 128`,
`DetailReplacementColor 112`, ground fog 1000,40,0.02,255). Why `Free06_Erode` does not show is not
known. Its material (`free06_erode.psm`: Vu1StripPrelit, `(Cs-Cd)*As+Cd`, no z write,
`depthTestMethod 1`) is the same as the road's. Its texture's decoded alpha is 255 throughout and its
vertices are 128 grey. Only the shots say it is absent. The pose came from the ride viewport struct
at 0x946a50: eye, +0x10 forward, +0x20 up, +0x30 fov.

## How it was proved

Looked right beside a screenshot: five PCSX2 shots compared with the shell's at the same poses, by
eye and as 8x6 grey grids. Fitting PCSX2 ≈ k × shell per shot, the mean error per cell was:

| spot | HEAD | fixed |
|---|---|---|
| p3 | 11.0 | 9.6 |
| m1 | 7.1 | 7.3 |
| r1 | 5.8 | 5.8 |
| r3 | 16.9 | 11.6 |
| r4 | 13.1 | 11.0 |

HEAD's best k swings from 0.77 to 1.09 between spots; the fixed shell's stays between 0.67 and 0.87.
The same comparison at 28 ground points is `dallas_slopes.rs`.

## Not verified / not 1:1

- Why the original leaves `Free06_Erode` out is not known. The rule (drop a mesh that repeats a
  program layer) fits the one mesh it catches. It is not a mechanism read from the game.
- The original's ground is about 0.7 of the port's everywhere on free06: k 0.62–0.87 per shot, while
  the sky matches. Not chased, because this is a render difference outside the slopes.
- The p3 cliff is still lighter than the original's (93→108 and 73→97 at two spots). There the
  original's erosion layer is denser and its streaks plainer than the port's pass-5 layer.
- The poses: the bike was settling after the teleport, and r3 was still moving a little; r2 was
  unusable. The cells are coarse: one ray per 1/48 of the screen, against a 40x40-pixel patch.
- The ride viewport address is an inference from its contents (eye, unit vectors, fov 85), not read
  from code.
- The screen-space target's resolution and the terrain draw's fog and detail-replacement colour are
  still not modelled (as in V25/V67).
- Only free06 was shot. free07, 09, 10 and 11 keep every mesh: the rule catches nothing there.

## Next

None.
