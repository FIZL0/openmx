# S385 — prop rays meet props in walk order

2026-11-17. Task: `./kit plan S385`.

A prop whose rays meet two other props now meets them in the order its gather walks the static tree,
not flat order. Proved on B34b's interpreter scene: ball 1's list matches the original at 4.8 and 4.4 ft
(ball 2 before ball 0), the gaps added back to `tests/prop_plane.rs`. Details in 1-.
