# 1 — smooth texture lod (FINAL)

Task V85. 2026-10-26.

## What I did

**Found first.** All of it was measured on the GPU renderer itself, headless (`Renderer::headless`, the
shell's renderer), not by filming the shell. A walk from far to near, 1% nearer each frame (at 60 fps a
level's octave takes about a second). Each frame is cut into patches fixed on the thing walked to (a
card's texture, a stretch of ground), found by unprojecting every pixel onto the card's or ground's
plane, so the camera's motion moves nothing between patches. Each patch's coverage, mean colour and
detail (the spread of its brightness) are then compared frame to frame. Patches under 1500 pixels, or cut by
the frame's edge, are not judged: an edge-cut patch's mean shifts as it leaves the frame, which first
read as a 12.8 colour step on the ground. A plain whole-bush measure (pixel count, mean, spread) and a
frame-to-frame warp of a whole bush both came out smooth at HEAD. A bush's cards flip at
different frames, so its level changes hide in its own shimmer. A card facing the camera has one level
across it and shows them.

- **Cut-out leaves (V58/V80), the cause.** The material sampler for `keep_coverage` uses
  `MipmapFilterMode::Nearest`, so the level switches at lod k+0.5. A made-up 1024-texel leaf card
  walked from 120 to 1300 pixels across: detail steps of **6.46 at 21.7 ft, 4.54 at 11.0 ft, 2.58 at 5.5 ft**,
  exactly where the predicted lod 2.5/1.5/0.5 distances fall (21.8, 10.9, 5.45). The floor elsewhere is
  0.3–0.9.
- **Ground pages (MD4/MD9/MD31) and splat.** Read with `textureSampleGrad`/`textureSample` on
  trilinear, anisotropic samplers. A generated page ground (4 texels a foot, every texel its own
  brightness) ridden from 150 ft to 8 ft, eye 6 ft up: colour 0.20, detail 0.09 at worst. No step.
- **Ground chunk levels (V32, `ground_for`).** Geometry only. A level is the coarsest whose height error is
  under a pixel, and the paint is looked up by world position, not by vertex, so a level change cannot
  change the texture. Not changed.
- **Streamed world (MD17/MD28/MD31).** Read in code: `set_ground_streamed` uploads every page array
  whole before the first piece, and `mips_wanted`/`put_mips` finish a model's coverage-keeping mips on
  another thread *before* the model uploads. Nothing arrives coarse first, so there is nothing to blend in.
  V84's replacements are swapped at decode, synchronously. Not changed.
- **V66 prop LODs.** Do not exist yet.

**The fix** (`crates/game-render/src/gpu.rs`, `MESH_WGSL`'s `fs`): for a material with
`coverage == 1` (`Surface::keep_coverage`, only under Leaves: Crisp), the shader works out the lod itself
from `dpdx`/`dpdy` of the uv in texels, reads levels `floor(lod)` and `floor(lod)+1` with
`textureSampleLevel`, and takes the coarser one's alpha where the 4x4 Bayer dither (the one V55's
handover uses, offset by (2,1) so the two patterns do not line up) keeps `fract(lod)` of the pixels. Each
pixel still tests one level's alpha, so V58's "two mixed fall under the cutoff" cannot happen. Where both
levels are leaf, the colour is the two mixed by the fraction, so only coverage is dithered. Every other
material is unchanged (`textureSample`). `MaterialUniform::coverage`, which the shader had stopped reading
in V58, is read again. After the fix the same card walk is 1.76 at worst, at 15.6 ft, where the card is
256 px across and the texels fall four to a pixel. It stepped 1.71 there at HEAD too, so it is not a level
change. The three level distances now read 0.94, ≤0.8 and ≤0.8.

**Original mode.** Leaves: Original (`Foliage::Original`) never sets `keep_coverage`, so its materials
take the plain trilinear path as before. Nothing else's sampling changed.

**Taken from opentennis:** nothing; it has no cut-out mip handling (it draws no foliage).

## What the original does

Not relevant: presentation only. The GS picks a level from its LOD registers per primitive, and the
original's leaves are the blended Leaves: Original path, which this does not touch.

## How it was proved

- `crates/game-render/tests/texture_lod.rs`, `a_leaf_card_walked_from_far_to_near_sharpens_without_a_step`:
  1024-texel made-up leaves (seeded), V58's cut-out surface (cutoff 240, clamped, keep_coverage), 238
  frames. Bounds `CUT_OUT_COVERAGE` 0.04, `CUT_OUT_COLOUR` 2.5, `CUT_OUT_DETAIL` 2.2. At HEAD it fails on
  detail (6.46). With the fix: 0.028 / 1.91 / 1.76. Coverage and colour do not catch HEAD (0.028 / 2.21
  at HEAD). V58's levels keep the coverage, so a level change mostly changes sharpness.
- Same file, `a_ground_spot_walked_from_far_to_near_sharpens_without_a_step`: generated page ground,
  bounds `GROUND_COLOUR`/`GROUND_DETAIL` 1.0, measured 0.20 / 0.09 (HEAD and now; the ground's sampling is
  unchanged).
- `crates/game-render/tests/ecosystem.rs`, `a_bushs_leaves_walked_from_far_to_near_sharpen_without_a_step`:
  the disc's bush leaf textures of `BUSHES` (free06 0–3 and free09 1/4 all share one 512x256 atlas), on
  a card from a sixth of its texels across to 1.5x, 2x2 patches. Bounds `BUSH_COVERAGE` 0.03,
  `BUSH_COLOUR` 2.5, `BUSH_DETAIL` 3.5. Measured with the fix 0.019 / 1.41 / 2.88, and at HEAD 0.017 / 1.59 / 2.73
  (HEAD's gpu.rs swapped in for one run). This atlas's level changes stay inside its own shimmer (about 2 a
  frame), so this test guards against regression and does not prove the fix. The made-up leaves prove it.
- V58/V80's tests still hold, with output read in a run that failed on purpose so the log was kept:
  `a_far_plant_keeps_its_near_coverage_on_both_renderers` (gpu 94–117%), `a_bush_up_close_keeps_its_leaves_on_both_renderers`
  (gpu within 1–4 points of software on all six), `a_tree_walked_from_far_to_near_keeps_its_trunk` (gpu 97%),
  `riding_toward_a_plant_the_gpu_picture_changes_smoothly` (smooth 4.2% worst, under 7%). `-p game-render`:
  205 passed, including `every_shader_translates_for_webgl2` (the shader uses `textureDimensions`,
  `dpdx`, `textureSampleLevel`: all GLSL ES 3.0).
- Shared walk code: `crates/game-render/tests/lod_walk/mod.rs`. `V85_TRACE=1` prints each frame's worst step.

## Not verified / not 1:1

- **Not filmed in the shell.** The brief asked to ride free06/free09 and a world map and shoot the
  approach. The walks drive the same `gpu::Renderer` headless, with cards and a generated ground rather
  than a ride. Nobody has looked at the shell's picture of a nearing bush after the fix. The disc's own bush
  atlas shows no step above its shimmer before or after, so on the disc the change may not be visible at
  all. Where it shows is with detailed (upscaled, V84) leaf textures, and no upscaled pack is on this
  machine (`mods/` has none) to ride with.
- **The user's pop-in is assumed to be the cut-out leaves.** It is the only swap found that steps. If what
  the user saw was something else (a prop, the billboard handover, a world piece arriving), this does not
  fix it. Each of those was ruled out by reading the code or by the ground walk, not by watching it.
- **Ground geometry levels (V32) were not walked.** The ground walk is flat, so its chunk levels never
  change. Their steps are held under a pixel by construction (`lod_level`) and do not move the paint.
  No geomorph was added.
- **A world's streamed tiles (MD17/MD28/MD31) were not ridden.** That nothing arrives coarse is from reading
  `set_ground_streamed`, `mips_wanted`/`put_mips` and `main.rs`'s level thread, not from a capture.
- **The dither shows as a 4x4 pattern** on a leaf patch that is part way between two levels, in coverage only
  (colour is mixed). Not judged by eye. Its share moves by 1/16 at a time over the octave.
- **The shader's own lod is the isotropic `max(|dx|, |dy|)` of the spec.** A GPU's own pick may differ
  a little (some approximate it), so a leaf may read a touch sharper or softer than the hardware's nearest
  level did. Not compared.
- The made-up leaves' 1.76 detail step at 15.6 ft (4 texels a pixel) was there at HEAD as well. It is
  the patches' rasterisation lining up with the texels, not a level change. It is bounded, not explained further.

## Next

Nothing for this task. If the user still sees pop-in with an upscaled pack, ride with it in the shell and
walk a card of that pack's bush through `lod_walk::walk_card`.
