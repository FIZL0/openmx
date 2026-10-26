# 1 — near bush leaves (FINAL)

Task V80. 2026-10-26.

## What I did

- **The cause: the GPU's bilinear read cut at the original's alpha test.** It wasn't the half cut, the
  base level's alpha or a near fade (there is no near fade in either renderer).
  - A scratch probe printed every ecosystem scene's plant textures off the disc. Every one is near
    enough all or nothing at its own size. For example, free06's `scaggbusha` 512×256 has 99 927
    texels at alpha 0 and 31 145 over 224, and none between.
  - The plant materials' tests (V54) are high: 242 on free06 and the nationals, 162 on free09.
    `cut_out` keeps `max(cutoff, 128)`, so the cut copies tested at 242.
  - The software renderer reads the nearest texel, so the test only picks texels and every leaf
    texel is drawn whole.
  - The GPU reads the alpha bilinearly. Magnified, between a 255 texel and a 0 texel, it passes 242
    only in the 5% of the gap next to the solid texel. A leaf one or two texels wide drew as a
    sliver, and only the broad branches and trunk stayed: the user's "only the roots show".
  - V58's coverage-keeping levels kept that same thin coverage, measured at 242. So free06's bushes
    were thin far off too (5% of their cards at 100 ft against software's 40%). The user saw them
    right from afar because past the band they are V55's matched picture, cut at 128.
- **The fix, GPU only** (`crates/game-render/src/gpu.rs`, `mips_keeping` and `KEEP_EDGE`), for a
  material that keeps its coverage (only `cut_out`'s copies):
  - Its texture's full size is made all or nothing by the material's own test before the levels are
    built. Texel for texel, the same leaves pass as before and as on software.
  - The levels keep their coverage at half (128), not at the test.
  - The shader tests those materials at 128. Read bilinearly at half, the edge falls halfway between
    a passing texel and a failing one at every magnification, where the nearest texel puts it.
  - The texture cache is still keyed by (texture, the material's test), so nothing else moved.
- **Software unchanged.** Its picture is the reference here.
- **`Foliage::Original` unchanged.** The original's materials, blended at their own test, are still
  read bilinearly at 242 on the GPU and are as thin up close as the cut-outs were (see the shots).
  That setting is the original's rule, kept one switch away on purpose.
- `eco::cut_out`'s doc says how the texel decides on each renderer now.
- Took from opentennis: nothing. Its cut-outs are Bevy's `AlphaMode::Mask(0.5)` on textures whose
  alpha is already 0/255, which is the same choice this lands on: test at half on all-or-nothing
  texels.

## What the original does

Nothing to port: presentation only. The `.psm`s' alpha tests (V54) still decide which texels are
leaf. This change only moves where the GPU's filtered read of those texels is cut.

## How it was proved

- **`a_bush_up_close_keeps_its_leaves_on_both_renderers`** (`crates/game-render/tests/ecosystem.rs`, disc).
  - Bushes: free06's four bush species (ScaggBushA/B, DeadBushA, YellowBush) and free09's BushA/BushB.
    Each is drawn alone as its model at 1280×960, level with its middle, 3 ft off (fov wide enough to
    hold the whole bush) and 100 ft off (fov 0.9).
  - Measure: the share of its cards' pixels it draws. Cards are the same model with no alpha test.
  - Rule: the GPU's share must be 85–120% of software's at 3 ft and 80–125% at 100 ft.
  - Why judged against software and not near against far: the share itself differs near and far on
    software too (30% vs 43% for ScaggBushA). Up close the nearest cards fill the frame; from afar
    the cards behind fill each other's holes.
  - Now (software → GPU, 3 ft / 100 ft):
    - ScaggBushA: 30→30% / 43→43%.
    - ScaggBushB: 35→35% / 41→40%.
    - DeadBushA: 26→30% / 39→40%.
    - YellowBush: 59→60% / 63→64%.
    - free09 BushA: 18→19% / 30→30%.
    - free09 BushB: 75→75% / 60→60%.
  - HEAD's `gpu.rs` with this test: GPU 9, 11, 10, 36, 16, 74% at 3 ft and 5, 5, 5, 31, 25, 58% at
    100 ft. It fails on ScaggBushA at 31% of software.
- **`./kit check -- -p game-render`:** all 197 pass. That includes V56's trees-through-trees, V58's
  far-coverage and trunk walks, V55's ride tests and `cut_out_mips_keep_the_full_sizes_coverage`
  (its 0/255 texture at 128 is the same under the change).
- **Shell shots, before and after** (`openmx --track <t> --headless --shot-pose …`, 960×540, fov 60).
  - Spots: free06's ScaggBushA (2078,157.6,1740.7), ScaggBushB (2208,151.9,1221.5) and YellowBush
    (1311.8,153,1513.3), and free09's BushB (4548.7,135.3,2684.6). Each at 3, 10 and 40 ft.
  - Each shot four ways: Crisp and Original on the GPU and software.
  - Kept in the slot's `target/v80/{head,after}/`, not committed.
  - Before: at 3 ft, Crisp and Original look the same on the GPU, a few dark branches with the leaves
    gone. Software shows the bush full.
  - After: Crisp on the GPU is as full as software at every distance. Original is as before.

## Not verified / not 1:1

- **No `./kit shot` of the original in PCSX2 beside a bush.**
  - The only free06 states here are in the biplane. Getting a bike beside a bush means the
    Switch-To-Machine and teleport recipe from another slot's context. That is long, and this task
    is by eye.
  - "Beside the Original setting" was done as the shell's own `Foliage::Original`, which is the
    original's materials. The PS2's GS draws them at its own, lower resolution. Whether its filtered
    alpha test also thins them up close was not looked at.
- **`Foliage::Original` still thins up close on the GPU** (bilinear read cut at 242). It was left as
  the original's rule. It is not "as the Original setting shows them" in the user's sense if they saw
  fuller leaves there. In these shots Original is thinner than the new Crisp at every distance.
- **DeadBushA draws 116% of software's leaves at 3 ft on the GPU.** The half cut joins the corners of
  diagonal twig texels that the nearest texel draws as a staircase. Within the test's 85–120%, and
  this is the smoother look of the two.
- **Only free06's and free09's bushes were measured.** The nationals' bush and tree textures were
  only histogrammed (all near binary, tests 242/162/34), not drawn up close.
- **Trees were not judged up close**, only through V56/V58's tests, which still pass.
- **In the software shots, free06's terrain is not drawn at these spots** (grey backdrop). It is the
  same before and after, and not looked into. Render differences are noted, not chased (AGENT.md).

## Next

Nothing for V80.
