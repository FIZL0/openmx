# 1 — far foliage (FINAL)

Task V58. 2026-10-23.

## What I did

**Bisect.** I didn't run a commit-by-commit bisect. Reading the shader at HEAD (d2cd663) together
with V54's re-land over V56 pointed straight at one line, and a scratch worktree of HEAD confirmed it.

- V56 made a cut-out's alpha `t.a * (1 + 0.5 * level)` on the GPU.
- V54 later added `a >= material.below` to the same discard, and `below` is 256 for a material that
  is not split.
- So on any mip level past ~0.01, a solid texel (255 × 1.5 > 256) was **discarded**. Only the
  part-alpha texels survived.
- The result: thin, ghostly far leaves. Bark is solid, so a trunk vanished once its texture went to a
  smaller level, and came back up close, where level 0 is read ("the stump disappears and comes back").
- The software renderer reads the full-size texel unfiltered. It was never affected: HEAD's and this
  branch's software walks are the same picture.

**Fix (GPU only; `crates/game-render/src/gpu.rs`):**

- **Coverage-preserving mips** (`mips_keeping`, after Castaño's alpha mips as The Witness has them):
  - A material that keeps its coverage (`Surface::keep_coverage`, the cut-out leaves) gets its own
    upload of the texture. The texture cache is now keyed by (texture, cutoff).
  - In that upload, every smaller level's alpha is scaled so that each 4×4-texel tile, read
    bilinearly at 4×4 points a texel, covers as much at the cutoff as the full size covers under it.
  - The scale is read off the sorted point alphas, not searched. Ties are resolved to the nearer
    count, halfway to the next value, so rounding to u8 cannot tip them.
  - All of a level's tile scales are found first and applied after, so a neighbour is never read
    twice-scaled.
  - Per tile, not per texture, because one atlas holds solid bark beside sparse twigs. A whole-texture
    scale made BushA's model worse than before.
- **The shader boost is gone**, so nothing can pass `below` any more.
- **One mip level at a time** for those materials (`mipmap_filter: Nearest`). Each level keeps the
  coverage, but a mix of a fine level's thin twigs and a coarse level's broad, fainter ones falls
  under the cutoff between them. On BushA's model this took the GPU from 79% to 87% at 300 ft.
  - I tried anisotropic filtering (16×) as well: no measurable change, so I dropped it.
- `MaterialUniform::coverage` stays in the layout. The shader no longer reads it.

I didn't change the stump's band. With the discard fixed, the trunk is drawn at every step of the walk
in (see below). Where the model and its picture both draw a trunk pixel, the dither gives it to one of
the two. The model's own extra trunk pixels are cross-faded with the rest of it, as V55 does for the
whole plant.

opentennis's only mip work is B570's `crates/hst-base/src/mips.rs`. It covers blended menu art: halved
levels whose alpha is multiplied by a flat gain. There's no alpha test and no per-region coverage, so it doesn't
fit a 128 cut-out, and nothing was taken from it.

**Cost.** A 512×512 cut-out's levels take 54 ms in a release build: a once-per-track load cost per
leaf texture (`cut_out_mips_keep_the_full_sizes_coverage` prints it). Plain textures are uploaded as
before.

## What the original does

Nothing to port. This is presentation only. The original's leaves (blended, `Foliage::Original`) stay
one setting away under P126's Plant Detail switch. Their surface is not `keep_coverage`, so their
uploads and samplers are unchanged.

## How it was proved

All against the disc, both renderers, in `crates/game-render/tests/ecosystem.rs` and `gpu.rs`. Each new
test fails at HEAD, checked in a scratch worktree of d2cd663 with this branch's test file.

- **`a_far_plant_keeps_its_near_coverage_on_both_renderers`**
  - Setup: free09's TreeC and BushA alone at 1280×960. The model alone is measured at 80–300 ft
    against 40 ft; the picture alone at 400–1400 ft against 200 ft.
  - Measure: pixels × d². It must stay within 80–125% of the near value.
  - Now:
    - TreeC model: software 115–117%, GPU 115–116% (that is the model's own shape, the same on both).
    - TreeC picture: software 101%, GPU 101–103%.
    - BushA model: software 98–102%, GPU 87–97%.
    - BushA picture: software 99–109%, GPU 98–101%.
  - At HEAD: the GPU TreeC model fell to 75% by 300 ft (and the picture had fallen to 29% at 500 ft
    in my first probe), so the test fails.
- **`a_tree_walked_from_far_to_near_keeps_its_trunk`**
  - The walk: TreeC from 420 ft to 40 ft, 4 ft a step, 960×720.
  - The trunk is judged where software draws it: the lowest fifth of the tree, where the model alone
    and the picture alone both draw.
  - Rule 1: the smooth rule must draw every such pixel on software, and 90% on the GPU (edge rounding).
  - Rule 2: it must draw at least half as many of the fifth's pixels as the thinner half alone.
  - Now: software 100% at worst, GPU 85% at worst (348 ft).
  - At HEAD: the GPU is missing 25 of 30 pixels at 420 ft.
- **`gpu::tests::cut_out_mips_keep_the_full_sizes_coverage`** (no disc needed)
  - Texture: a scattered one-in-eight sparse half beside a solid block, 256².
  - Result: levels 1–5 keep 0.099–0.109 against the full size's 0.105, and the solid half 0.367–0.375
    against 0.375.
  - The plain box mips keep 0.000 of the sparse half.
  - A cutoff of 0 gives the plain mips unchanged.
- **The shell** (`--shot-pose`, free09, release builds of HEAD and here, 640×448). Shots in this
  session's scratch folder, not committed:
  - Walk in on TreeC (3432.7, 191.95, 3744.7) from (+0.8, −0.6)·d, 20 ft up, fov 40, 400 → 40 ft:
    - HEAD GPU: sparse dotted leaves and **no trunk at any distance**.
    - Here: a solid crown and trunk all the way.
    - Software: the same before and after.
  - free09's densest view `2750,160,3800,0,-3,85`, GPU:
    - HEAD: the far trees are ghosts and the near tree has no trunk.
    - Here: full crowns and the trunk.
- V55's ride tests still pass, with two bars moved (see below). V56's trees-through-trees tests pass
  unchanged (100% shown through on both).

## Not verified / not 1:1

- **No `./kit shot` of PCSX2.** The task is presentation only, and the before is the port's own HEAD,
  not the PS2. The before/after is the shell's `--shot-pose` on both renderers.
- **No real bisect.** I didn't build V55 (15a3f71) or V56 (532e42b) alone. The cause was read from the
  code and confirmed at HEAD. Whether V56 by itself, before V54's `below`, also thinned far plants is
  not checked; its own boost was weaker than per-tile coverage in any case.
- **BushA's model on the GPU keeps 87–97%, not 100%,** of its near coverage at 150–300 ft. The likely
  remainder is the GPU's isotropic level choice on edge-on cards (an anisotropic sampler did not help
  on the headless adapter). Not measured on a real display adapter.
- **V55's two bars moved:**
  - `MOST_A_FRAME` went from 6% to 7%. The GPU's bush handover now shows a 6.1% frame at 298 ft:
    its model is half its picture's size on both renderers, and the GPU no longer hides that by
    swelling the far model.
  - The ride's sanity floor went from 50 to 40 pixels. BushA's model alone at 520 ft, where no rule
    draws it, now covers fewer than 50 pixels.
- **One mip level at a time on leaves** can show a level's pop in their texture detail as distance
  changes. The coverage holds across a pop; I did not see one in the shots, but did not look for one
  on a moving ride.
- **The trunk's cross-fade is not "drawn exactly once" everywhere.** Where the picture's painted trunk
  and the model's trunk differ (the picture's is thinner, and pale), the model's extra pixels dissolve
  with the band, as the rest of the plant does since V55. The test holds the dither's shared pixels to
  all and the fifth to half of the thinner half.
- **Only free09's TreeC and BushA are under test.** The other species (TreeA/B/F, free06's bushes,
  the national scenes') use the same mips but were only looked at in the shell: free09's walk and the
  densest view.
- **The 54 ms per 512² leaf texture load cost was measured in a unit test only,** not as a hitch in
  the shell's track load.

## Next

Nothing for V58.
