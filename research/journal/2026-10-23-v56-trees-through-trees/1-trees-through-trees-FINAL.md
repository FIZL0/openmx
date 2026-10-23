# 1 — trees through trees (FINAL)

Task V56. 2026-10-23.

## What I did

- **The cause, found first.** A scratch test printed the `Surface` of every plant model of free09 and
  free06 off the disc: every tree and bush model's one material (`springtime2`, `bushes`) is
  `cutoff 1, blend true, depth_write true` (the `.psm` blends with an alpha test of nothing), and the
  billboards are `cutoff 100` (free09) / `32` (free06), also blended and writing depth. The leaves'
  texture at its own size is near enough all or nothing (512×256 springtime2: 61 708 texels under
  alpha 16, 63 262 over 240, ~5 000 between), but bilinear filtering and the GPU's mips make a
  crown's edges and its far levels part-alpha, and every texel with alpha ≥ 1 writes depth. So a
  leaf card hides whatever is drawn after it behind it, see-through or not: the trees behind it
  (plants are drawn in the file's order, nearest-first only by chance) and — the worst of it in the
  shots — the tree's own cards behind it, which show as pale ghost cards inside the crown.
- **The fix: cut-out foliage** (`game-render/src/eco.rs`, `Foliage`, `cut_out`, `FOLIAGE_CUTOFF`):
  a blended, depth-writing plant material becomes solid with an alpha test at 128 (half), so a texel
  is all leaf or absent and draw order stops mattering. Both renderers get it through the same
  `Surface`; nothing in the renderers' draw loops changed.
- **The original's look is one setting away**, as V55's `Lod`: `Grove::foliage`, `Foliage::Cutout`
  by default, `Foliage::Original` draws the old surfaces. The grove keeps a cut copy of each species'
  model and matched billboard beside the original (`"<name>, cut"` in `Assets`, `Grove::cut`,
  `Draw::cut`, `Grove::parts` lists them after the others and `Draw::part_index` finds them), so the
  switch is free at run time — no re-upload. The original's own picture (`Part::Billboard`, faded in
  under `Lod::Original`) is never cut: it is part of the original's rule. Nothing in the shell sets
  the switch yet; P126 wires it (its foliage sub-switch, per the task's note).
- **Far levels keep their coverage on the GPU** (`Surface::keep_coverage`, set by `cut_out` only;
  the mesh shader's `coverage` flag): an alpha-tested texture thins at its smaller mips because
  their alpha is averaged, and the first run showed it — free09's BushA model at 326 ft dropped to 39
  pixels and V55's ride test went red. The shader now raises the sampled alpha by half again per mip
  level (`a * (1 + 0.5 * level)`, the level from the uv derivatives), the known fix for alpha-tested
  foliage. 0.25 a level still left the smooth ride's handover at 6.6% (limit 6%); 0.5 passes. The
  software renderer samples the top level only, so it needs nothing.
- **Tests** (`crates/game-render/tests/ecosystem.rs`): `a_tree_shows_through_a_nearer_trees_leaves_in_software`
  and `…_on_the_gpu`: two of free09's TreeC, the far one 40 ft behind the near one and moved aside by
  its width, drawn as models at 60, 150 and 260 ft; the pixels where the far tree is drawn alone and
  the near one is see-through (its frame alone no different from the empty one) must show the far
  tree when both are drawn. Cut-out: 100.00% at every distance on both renderers (asserted ≥ 99.5%).
  The original's materials: software 94.8 / 97.7 / 98.1%, GPU 95.4 / 96.0 / 97.2% (asserted < 99%,
  so the test is shown to catch the bug). The V48 test that checks the original's draws by model name
  now pins `Foliage::Original`.
- **Shots, before / after** (`openmx --track free09 --size 960x540 --shot-frame 3 --shot-pose …`;
  the "before" is the same build with the default flipped to `Foliage::Original`, which is HEAD's
  look; kept in the session scratchpad, not committed):
  - `2900,175,4500,0,-3,60` — a TreeE close up: before, pale ghost cards fill the middle and bottom of
    the crown; after, a full, solid crown.
  - `2850,175,4200,0,-1,25` — a tree line at 200–600 ft: before, the near tree on the right has a
    washed-out halo and the dead bush bottom-left a pale ghost of the tree behind it; after, crisp
    edges and the trees behind it show through.
  - `2850,175,4200,0,-3,60` — the wider view of the same spot; the difference is smaller at that size.
- Took from opentennis: nothing to copy, but the same choice — its cut-out materials are Bevy's
  `AlphaMode::Mask(0.5)` (`hst-base/src/gs.rs`, `character.rs`), half alpha, as here.

## What the original does

The plants' `.psm`s blend with no real alpha test and write depth; the PS2 draws them that way, so
the original has the same see-through faults at its own (unfiltered, lower) resolution. Not ported:
this task is presentation, by eye, and better than the original on purpose (user, 2026-10-22).

## How it was proved

Two disc tests through `./kit check` (software and GPU, three distances each, the original's surfaces
shown failing and the cut-out passing), V55's ride tests still green on both renderers with the cut
copies, and before/after shell shots of two wooded spots on free09, looked at by eye.

## Not verified / not 1:1

- Not 1:1 with the original by design: under `Foliage::Cutout` the leaves are solid cut-outs, so
  the original's soft, part-alpha leaf edges are hard (one-pixel aliased) edges. `Foliage::Original`
  keeps the original's materials.
- Not compared against the original running in PCSX2: no `./kit shot` of the original at the same
  spot. The task asks for better than the original, and the before shot is the port's own old
  drawing (HEAD's), not the PS2's.
- The mip boost (0.5 a level) was chosen by V55's ride tests and one look at the shots, not tuned per
  species or measured against a reference coverage; a sparse bush may look a little fuller at a
  distance than near. Not measured.
- `Foliage` is not in `settings.txt` or the menus: P126 (Original mode) wires it. Until then the
  shell always draws the cut-out.
- Only free09 (springtime2) and free06 (bushes) materials were printed; the national scenes'
  ecosystems (V48's nat19 and others) use the same `cut_out` rule but their materials were not
  looked at one by one. A plant material that is additive or does not write depth is left as it is.
- The matched billboards (V55's) are cut at 128 where the atlas's own test was 100, so a picture is a
  little smaller than before at its soft edges; V55's ride tests still pass, but no shot was taken of
  the picture band on its own.
- The original's faded picture (`Part::Billboard`, `Lod::Original`) is still blended and writes depth
  at alpha ≥ 100, so under the original's LOD rule a far picture can still hide a picture behind its
  soft edge. Left as the original's on purpose.

## Next

Nothing for V56. P126 adds `Grove::foliage` to the Original mode's switches.
