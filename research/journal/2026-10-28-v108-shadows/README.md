# V108 — shadows

2026-10-28. Task: `./kit plan V108`.

Shadows for the things that move and the world's models. Settings' Graphics tab (under Picture) has a
Shadows row: Better (the default), Original and Off; `--shadows better|original|off` does the same on the
command line. Original is the original's projected shadow: a small map round each racer, laid on the
ground in the sun's `ShadowRGB` at its `ShadowIntensity`, faded with distance as the original does.
Better is cascaded sun shadow maps on the GPU renderer. Everything near casts (bike, rider, machines,
train, toys, props, plants) onto the ground and onto each other, softly, darkened to the baked light
maps' own shadow level. The software renderer draws none. Details in 1-shadows-FINAL.md.
