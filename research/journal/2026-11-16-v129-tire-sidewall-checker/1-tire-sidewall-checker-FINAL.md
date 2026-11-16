# 1 — tire sidewall checker (FINAL)

Task V129. 2026-11-16.

## What I did

- **The cause: the GPU's mips, not the texture, UVs or a z-fight.** The software renderer drew the
  buggy's tires right; the GPU drew their sidewalls and treads as blocks of white, grey and orange.
  - UVs checked: the front-left tire's 395 triangles plotted over `buggy_tire_01` sit on the rubber
    ring and the tread patch, as they should. No duplicate triangles in the finest level of detail,
    and the cars are drawn at level 0 (`lod_from: None`), so no z-fight with the coarse tire.
  - The texture's alpha is a gloss mask (its `.psm`'s first pass is
    `Vu1StripDiffuseGlossySpecularMap` with `specular04`), and the rubber's alpha is 0. The material
    is opaque with its alpha test off (cutoff 0).
  - `gpu.rs`'s `mips` weighs colour by alpha and, since V55, repaints every alpha-0 texel at every
    level (the full size too) with the coarser level's colour, which is a cut-out's hole fill. On the
    tire that painted the rubber with the coarse levels' averages of the atlas's white background,
    metal and the orange lamp ring — the checker.
- **The fix (`crates/game-render/src/gpu.rs`):** `mip_keep(surface)` picks a material's levels: its
  cutoff for a kept-coverage cut-out (never under 2), `SOLID_MIPS` (1) for a material that neither
  tests, blends, adds nor depth-tests by alpha, and 0 (the old plain levels) for the rest.
  `solid_mips` box-filters all four channels alike with no hole fill. The material uniform's
  "kept coverage tests at half" now reads `keep > SOLID_MIPS`, not `keep > 0` (first try missed it,
  and every opaque material tested its alpha at 128: the rubber vanished). `garage_view.rs` builds
  its prepared levels with the same `mip_keep`, so its keys still match.
- For a texture with alpha 255 everywhere the new levels are the old ones exactly: the alpha weighting
  `(Σ c·255 + 510) / 1020` is `(Σ c + 2) / 4`, and nothing is a hole. Only opaque materials whose
  alpha is a mask change, and they now match the software renderer.
- Unit test `an_opaque_materials_levels_ignore_its_alpha` (gpu.rs): rubber at alpha 0 keeps its
  colour at every level under `SOLID_MIPS` and still gets the hole fill at keep 0; `mip_keep` for
  cut-out, blended, additive, depth-by-alpha and kept-coverage surfaces.
- Nothing taken from opentennis: its renderer is Bevy's and has no such mip builder.

## What the original does

The PS2 draws the tire with the GS's own mip levels from the `.pst` (or none), and the gloss in a
second pass by the alpha mask. The port draws the first pass only (as before) and builds its own mips.

## How it was proved

Looked at by eye, GPU beside the software renderer: free11, a tape that switches into the buggy at
tick 10 and gives gas 30–60, shot at tick 188 from `--shot-pose 4618,137,3377,-90,-3,40` at 960x672
and at 240x168 (coarse levels). Before: the checker; after: dark rubber with tread, as the software
renderer draws it. Shots kept in the session's scratchpad, not committed.

## Not verified / not 1:1

- Not compared with a PCSX2 shot of the original's buggy: rendering is matched by eye (AGENT.md), and
  the software renderer, which reads the texture directly, was the reference.
- The gloss/specular pass (`specular04` by the alpha mask) is still not drawn, as before this task.
- Other opaque materials with a masked alpha (other models, the bikes) now get plain mips too; only
  the buggy was looked at. Fully opaque textures are unchanged by arithmetic (above).
- The see-through bay ghost still draws at the buggy's spawn after it is taken (seen while shooting);
  not this task, not checked against the original.

## Next

Nothing for this task.
