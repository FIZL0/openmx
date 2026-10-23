# 1 — solid textures (FINAL)

Task V54. 2026-10-23.

## What I did

- Found the free06 materials (Dallas, the train line) that the port drew with holes where the
  original's are solid. I decoded every free06 material's texture and listed its GS alpha against
  its `.psm` pass:

  | material | `.psm` alpha test | faintest GS alpha | port before | original |
  |---|---|---|---|---|
  | mm_boxcars | none | 1 (1,528 texels) | holes under 16 | solid |
  | mm_scraper | `alphaTestOnOff = 0` | 4 | holes under 16 | solid (no test) |
  | mm_dumptruck | `alphaTestOnOff = 0` | 8 | holes under 16 | solid (no test) |
  | mm_companytruck | none | 4 | holes under 16 | solid |
  | free06_pickup | none | 2 | holes under 16 | solid |
  | free06_port_a_jon | none | 1 | holes under 16 | solid |
  | mm_caboose | on, method 5 (≥), ref 80, fail 1 (colour only), blended | 1 (4,478 texels) | no depth at all, so later draws showed through its solid sides | solid texels keep depth |

  coalcar4 / coalcarb4, mm_engine, mm_flatbed, trainwheels and mm_httower have texels of alpha 0.
  Those are holes in the original too, and they stay holes.
- **Cause.** When a `.psm` said nothing about the alpha test, the port cut every texel under 16 of the
  GS's 128. An explicit `alphaTestOnOff = 0` was cut the same way. The original fills in what a
  material leaves out from a default material: test on, method GREATER, reference 0, fail KEEP,
  blending off. So under the original only alpha 0 is a hole.
- `scene.rs`: `Material` now reads `alphaTestOnOff = 0` (`alpha_test_off`), and reads
  `alphaReferenceValue` whether or not the switch is there (`alpha_reference`). It also reads
  `alphaTestMethod` (`alpha_test_method`). Failing into colour now means fail value 1 (FB_ONLY) or
  3 (RGB_ONLY).
- `world.rs`: `Surface::of` builds the GS test with the original's defaults. The decoder doubles GS
  alpha, so:
  - GREATER r becomes a cutoff of 2r+2 (r ≥ 128 passes nothing);
  - GEQUAL r becomes 2r;
  - NEVER passes nothing;
  - ALWAYS passes everything;
  - test off passes everything.

  A test that fails into colour only now draws every texel. Only the passing texels write depth, so
  this needs a new `Surface::depth_cutoff`. Before, such a material wrote no depth for any texel.
  `Surface::default()` (a model with no `.psm`) still cuts under 32, as before.
- Software path: `raster::Look` / `Mode` take `depth_cutoff`, and `Rows::fill` writes depth only for a
  texel at or over it.
- GPU path (`gpu.rs`): when `depth_cutoff > cutoff`, a material is drawn twice, in two steps:
  1. the passing texels, with a depth write;
  2. the rest (`material.below`), without one, from a second bind group and pipeline.

  The WGSL `fs` discards outside `[cutoff, below)`.
- Updated tests:
  - seats_red's expected surface (no method named, so GREATER 80 → 162);
  - the ecosystem billboard's cutoff (GREATER 50 → 102, was 100).
- New tests:
  - `world::tests::a_material_s_alpha_test_is_the_original_s_with_its_defaults`: every case on the disc;
  - `a_texel_failing_into_colour_only_leaves_the_depth_buffer_alone`;
  - `crates/game-render/tests/solid.rs`, which checks on both paths that each material above hides the
    backdrop at every pixel. Before the fix, the boxcars left holes. The caboose hides a backdrop
    drawn after it, except about its share of see-through texels: 76 of 2,304 pixels on software and
    95 on the GPU, against 1,444 without depth. The bushes' billboard and a blended shadow still
    show what is behind them.

  The GPU half ran here: "checked on the GPU: 6 of 6".

## What the original does

- The material builder is `./kit fn 0x17eb00`. It reads each `.psm` key through the ini reader
  (`./kit fn 0x14b6f0` → `0x14b3f0` → `0x14af90`; T/F/ON/OFF/Y/N, otherwise strtol). Where a key is
  absent, it falls back to a defaults descriptor: ATE at +0xb4, ATST at +0xb8, AREF at +0xbc, AFAIL
  at +0xc0 and ABE at +0xa0.
- In the free06 save state (`context/states/biplane-start.ee.bin`) there are two descriptors:
  - the one at `0x004f9870` has default microprogram `Vu1StripDiffuse.vuo`, with ATE=1, ATST=6
    (GREATER), AREF=0, AFAIL=0, ABE=0;
  - the one at `0x004f9970` (`Vu1Icon.vuo`) has ATE=0, ABE=1, which is the UI.
- The GS TEST words in that RAM agree. They cluster at 0x5800d (ATE 1, GREATER, AREF 0) and 0x5800c
  (ATE 0).
- On the disc, `.psm` files use only the methods 0, 5 and 6 (some leave the value empty), and only the
  fail values 0, 1 and 3.

## How it was proved

- **The defaults.** I read the defaults descriptor out of a save state's RAM and matched it to the
  decompiled builder, which reads its fields where keys are absent. The TEST-word histogram of the
  same RAM agrees.
- **The fix.** It is shown by the disc tests in `crates/game-render/tests/solid.rs`, on the software
  renderer and the GPU (headless wgpu): the rendered texel is the texture's colour, with zero pixels
  differing between two backdrop colours.
- No side-by-side screenshot of the original was taken. The proof is from RAM and the decompilation.

## Re-landed on V56 (second session, 2026-10-23)

The first attempt (wip/V54) failed after merging: V56 had added `cover` to `raster::Look` and
`coverage` to the GPU material uniform. This session re-applied it on current main:

- The GPU `Material` uniform carries both V56's `coverage` and this task's `below`. With four
  4-byte fields after the vec4 it is 32 bytes, so the `_pad` is dropped. The fragment test is V56's
  coverage-raised alpha, checked against both `cutoff` and `below`.
- `eco::cut_out` (V56's solid leaves) now sets `depth_cutoff: 0`. Otherwise a plant pass that fails
  into colour only would keep its depth-less band between 128 and the reference.
- This task's raster unit test now gives `cover: 1.0`.
- V56's `a_tree_shows_through_a_nearer_trees_leaves_*` asserted that under `Foliage::Original` the
  near tree's part-alpha leaf edges hide the far tree (under 99% shows through). That rested on the
  old reading, where a reference with no `alphaTestOnOff` meant no test. Under V54 the free09 tree
  leaves' pass is tested at greater than 80 (cutoff 162 doubled), and the far tree shows through
  100% in software and 99.99 to 100% on the GPU at 60, 150 and 260 ft. Both foliage modes now
  assert at least 99.5%, and the `Foliage::Original` doc says so. V56's cut-out stays the default.

## Not verified / not 1:1

- Empty values (`alphaTestOnOff =`, `alphaTestMethod =`) are still treated as absent. The original's
  reader may keep the key and read 0. For hit-zone materials with an empty method, 0 would be NEVER.
  I did not trace this.
- Which defaults descriptor applies to a material (Vu1StripDiffuse vs Vu1Icon) is inferred from the
  microprogram name, not traced per material.
- NEVER with fail KEEP is drawn as passing nothing (cutoff 255 still lets alpha 255 through on the
  software path). It is not on the disc.
- ZB_ONLY (fail 2) is drawn like KEEP. It is not on the disc.
- LESS / LEQUAL / EQUAL / NOTEQUAL are drawn as GREATER. None of them is on the disc.
- Vertex alpha (the GS's modulate At·Av/128) is ignored in the test. The test uses texture alpha only,
  as before.
- Filtering differs at the edges of holes: GPU bilinear sampling against the GS's.
- A model with no `.psm` (`Surface::default()`) still cuts under 32. The original's behaviour there
  was not checked.
- The V56 merge (leaf coverage together with the colour-only split on the GPU) is covered only by
  the existing ecosystem and solid tests. No frame was compared by eye after the re-land.
- Only free06 was checked material by material. Other tracks pick up the same defaults through the
  same code but were not inspected one by one.

## Next

- none (lands with `./kit done`).
