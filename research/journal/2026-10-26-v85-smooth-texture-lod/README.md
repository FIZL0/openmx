# V85 — smooth texture lod

2026-10-26. Task: `./kit plan V85`.

The user saw sharper textures pop in. Found first: of the suspects, only V58's cut-out leaves stepped.
They read one mip level at a time (nearest), so on a detailed leaf texture (1024 texels, an upscaled
bush's size) each patch's detail jumped 6.5, 4.5 and 2.6 a frame where the level was 2.5, 1.5 and 0.5. The
ground (trilinear, anisotropic) steps 0.2 at most. Streamed pages and coverage mips are never coarse
first, and V66's prop LODs do not exist yet. Fixed in the GPU's mesh shader: a cut-out that keeps its
coverage now picks between the two levels around its own, dithered by the fraction, so each pixel still
reads one level (V58's coverage holds) and the sharper level comes in over the octave. Details in
1-smooth-texture-lod-FINAL.md.
