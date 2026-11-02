# MD60b — wrapped field

2026-11-02. Task: `./kit plan MD60b`.

openmx's `crates/stitch` now has openmx-mods C12's wrapped field (and MD60a's relief across the wrap),
ported from openmx-mods d9e38b8. A wrapped world's field is the same a period on: `Plan::period`,
`images()`, `wrapped_gap`, and the gaps across the wrap in the grid layout. The radii, bins, blend,
clear, gated, candidates, relief_at and relief_cands all read every image. This replaces P251's
`wrapped_copies` (unit layout only), so there is one mechanism for both layouts. `stitch::VERSION` is
2 → 3. Disc builds: world_mxu_free is byte-equal to openmx-mods d9e38b8's build (joins and mod.json,
apart from openmx's `"category"` line). `--seams` steepest/across the wrap: world_mxu 0.614/0.057
(unchanged from P251), world_mxu_free 0.276/0.040, world_mxu_nat 0.700/0.061. Details in
1-wrapped-field-FINAL.md.
