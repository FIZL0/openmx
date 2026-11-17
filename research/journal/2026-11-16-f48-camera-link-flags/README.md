# F48 — camera link flags

2026-11-16. Task: `./kit plan F48`.

F48 asked to show that the posts' store into the bike's flags (`+0x444`) cannot change them. It can.
The store is a link registry's add-link, `flags |= key`. It is a no-op on C15 because the keys (8,
0x10) are already set. A post that switches back to a remembered first-person camera would or in 0x20,
and the frame's sync reads that bit. The effect is ported as `update::camera_link` and proved by a
10 000-case oracle against the original's link pass. Whether the 0x20 case matters to the simulation
is filed as S345. Details: `1-camera-link-flags-FINAL.md`.
