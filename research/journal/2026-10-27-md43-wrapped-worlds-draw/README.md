# MD43 — wrapped worlds draw

2026-10-27. Task: `./kit plan MD43`.

A world whose file says `wrap` is now drawn round. The GPU and software paths draw the world's ground and
shapes again, one width off on each wrapped side, as far as the fog reaches (`game-render/src/wrap.rs`: the
same world drawn again from the camera moved back by a width). Other riders are drawn at the copy nearest
the eye. The radar marks riders and blips at the copy nearest the rider, and MD42's labels name the map over
the seam. Proved by a shell `--shot` across the seam on both renderers, a headless GPU test and radar tests.
Gaps are listed in `1-…`.
