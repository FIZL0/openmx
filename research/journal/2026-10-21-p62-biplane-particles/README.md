# P62 — biplane particles

2026-10-21. Task: `./kit plan P62`.

P58 drew the biplane's particles only under `--renderer gpu`. The software renderer now draws them too:
`game_render::particle::draw` fills each camera-facing square as the screen-aligned rectangle it projects
to, blended and depth-tested, with the GPU's rules for size, cell, alpha ×2 and fog. The shell calls it
after the scene and before the overlay. Done; see 1.
