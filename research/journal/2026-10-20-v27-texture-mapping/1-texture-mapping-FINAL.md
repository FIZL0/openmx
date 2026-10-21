# 1 — texture mapping (FINAL)

Task V27. 2026-10-20.

## What I did

- **Found the bug: the `.pst` decoder read the wrong mip.** A `.pst` is the header `0x00020002`, a kind
  (`0x28` is 8-bit with a 256-colour palette at byte 8, `0x2e` is 4-bit with 16 colours), then a mip count
  (at 1032, or 72 for 4-bit) and the mips **smallest first**. Each mip has a 16-byte head of height, width,
  byte count and 0, then its pixels, and the largest mip comes last. The old decoder took the first head's size
  as the picture and read its pixels from 1052 (or 92). That worked on single-mip files. On the common
  256² textures with 6 mips, the five small mips (21,824 bytes, 85 rows) filled the top of the image and
  pushed the real picture down out of place. That is the "scrambled jersey" and the streaky ground band.
  `pst::largest_mip` now walks the chain, checks each head (bytes = w·h·bpp, sizes never shrink) and decodes
  the last mip. I surveyed all 2,798 distinct `.pst`/`.atm` frames on the disc: every chain is consistent,
  and 1,861 of them have more than one mip. The shadow tiles (512², 4-bit) and masks (256², 4-bit) are
  single-mip, so V25's light map and row reversal are unaffected.
- **Per-material wrap or clamp.** 335 `.psm` materials say `wrapModeHorizontal/Vertical = 1` (GS CLAMP),
  103 say 0 and 2,517 say nothing (repeat). The world props among the clamping ones include palm bark and
  fronds (`palmfraunb` is v only), `nat08batbarkbranch`, `powerline_wood_b`, the nat13 cowboys and the
  terrain shadow receivers. `Material::clamp` → `Surface::clamp`. The software path clamps u/v in
  `texel_of`, tagged through two high bits of the batch's texture index. The GPU path gives clamping
  materials their own ClampToEdge sampler per axis.
- **Checked these and found them already right, so left alone:** the palette's CSM1 block swap, alpha ×2
  (0x80 → 255), texel order (no swizzle in the files), and the UV unpack. The UV unpack is V2-16 signed times
  the file's own scale, which is 1/2047 (`0x3a001002`) everywhere and equal for u and v. V orientation is
  right too: decals and signs read the right way round. A VIF survey of 2,218 `.psg` files found no STMOD,
  STROW or STCYCL, so there is no row offset to add to the UVs. The terrain's splat (`splat.rs`) layers
  look right by eye once their textures decode properly.
- Tests: `crates/game-render/tests/texture.rs`.
  - `free09_rock.pst`'s chain heads and four texels against bytes read by hand.
  - `sign_speedlmt.psg`'s first three UVs against its VIF bytes by hand.
  - A software triangle at texel (17, 200) paints [118, 113, 96].
  - One quad on both renderers paints that texel. The GPU gave exactly [118, 113, 96].
  - A red/red/blue/blue 4×1 texture over u 0..2 is red at u = 1.25 when it repeats and blue when it clamps,
    on both renderers.
  - Unit tests: a synthetic 2-mip `.pst` decodes its last mip, and the wrap keys parse.
  - I checked with temporary panics that the disc and GPU halves really ran and did not skip.

## What the original does

The `.pst` layout and wrap modes come from the files themselves (the chain's heads add up exactly to every
file's length) and from the GS's own CLAMP register meaning (0 REPEAT, 1 CLAMP). I opened no decompiled
function for this task.

## How it was proved

- I compared the original's frame beside the port's, at the same spot, on the human's `first_recording.p2s`.
  The original's frame is the `Screenshot.png` PCSX2 stores inside the `.p2s`. The port's frame is
  `openmx --moment context/shots/v27-first_recording-moment.txt` (software). The GPU and software rides
  were compared before and after too. All shots are in `context/shots/v27/` (git-ignored):

  | what | before | after |
  |---|---|---|
  | original \| port, free09 start | `fr-sbs-before.png` | `fr-sbs-after.png` |
  | port alone | `fr-port-before.png` | `fr-port-after.png` |
  | ride, GPU | `ride-gpu-before.png` | `ride-gpu-after.png` |
  | ride, software | `ride-software-before.png` | `ride-software-after.png` |
  | rider crop (before left, after right; GPU top, software bottom) | `rider-ba.png` | (same file) |
  | ground crop | `ground-crop.png` | `ground-crop2.png` |

  Before the fix, the rider's jersey and pants were scrambled patches and the ground had a smeared streak
  band. After it, the livery shows ("000", sponsors) and the ground grain matches the original's frequency,
  on both renderers.
- Hand-read bytes for the tests: `free09_rock.pst` (88,492 bytes). Its heads are at 1036 / 1116 / 1388 /
  2428 / 6540 / 22940 (8² … 256²). The texel at (0,0) is index 130 → palette at 528 → [116,118,108,0x80].
  `sign_speedlmt.psg`: the unpack at 525 (`0x650b8017`), and its first vertex is (1988, 30)/2047.
- `./kit check` passed (whole workspace).

## Not verified / not 1:1

- **Second shader passes are still not drawn.** Examples: the rider gear's alpha-blended
  `Vu1StripDiffuseScaled` pass (MaterialPassStart 150 / End 200), and the env-map, specular and BRDF passes.
  368 materials have 2 passes and 182 have 3. V25 already noted this. It is drawing, not mapping, so I left it out of scope.
- **37 supercross `.psg` files fail `Model::parse`** ("the file ends at 793 but 262158 bytes were wanted
  at 373"). Examples: `sx*_grass_*`, `dirtbridge*`, `nat03_start_slab`, `nat03_bridgedirt01/02`. They have
  mesh mask 0x1f and a different LOD layout. It is a geometry parser problem, not texture mapping, so I did not chase it. Those props are missing.
- **The ground is still brighter and more contrasty than the original.** That is lighting or the splat's
  blend, not UVs. I judged it by eye only.
- **Mip selection is not the original's.** The GPU builds its own box-filtered mips from the largest level
  instead of uploading the disc's own chain, and samples Linear with mip Linear. The software path samples
  nearest with no mips. `MipMap_N`, `MinFilter`, the L/K LOD values and `lodCalculationMethod` in the `.psm`
  are not honoured. Distant ground may shimmer or blur differently from the PS2.
- **Half-texel alignment was not compared.** No test or shot can show a half-texel offset at these sizes by eye.
- **Clamp is proved only by the synthetic test.** I took no in-game shot of a palm next to the original:
  none of the human's recordings looks at one.
- **No live `./kit shot`.** The original's frames are the screenshots stored in the `.p2s` files, and the
  emulator was never started. That is the same spot as the savestate, but only one frame per recording.
- **The `--moment` comparison is software-only.** The GPU path was compared on the ride (port before vs
  after), not beside the original at the same moment. `--moment` also draws no rider, so the rider was judged
  on the ride shots.
- **REGION_CLAMP (GS wrap modes 2 and 3) was not seen** in any `.psm`, so it is not handled.
- **Supercross ground colour map and ecosystem foliage:** still missing, as before. They are not affected by this task.

## Next

Land. Most of the render differences left to match are the second shader passes and the ground's lighting.
