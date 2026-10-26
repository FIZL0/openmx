# V77 — dallas dark slopes

2026-10-26. Task: `./kit plan V77`.

The dark slopes on free06 (Dallas) were V67's `Free06_Erode` screen-space mesh. It laid the erosion
texture's dark brown (74,61,46) over 688,000 sq ft of cliff bands, about fifteen feet onto the flat at
each foot, as a hard-edged slab. In four PCSX2 shots of the same spots the original shows no trace of
it: the flat at a band's foot is as light as the flat beyond. Up a cliff, the streaks run down the
slope the way the paint program's own erosion layer runs, not along the band the way the mesh's
texture does. `Free06_Erode` is the only screen-space mesh on the disc textured with one of its
track's own program layers. `ground_mesh::repeats_a_layer` finds it that way, and `Splat::load` leaves
it out. That covers the GPU, software and world paths alike. Test `game-render/tests/dallas_slopes.rs`
checks 28 named spots of the shots against the original's greys. Details in 1-…-FINAL.md.
