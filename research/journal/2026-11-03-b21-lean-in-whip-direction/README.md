# B21 — lean in whip direction

2026-11-03. Task: `./kit plan B21`.

B21 asked for `game_render::rider::lean_in` (the drawn clip's stand-in hold, V100) to move the body the
original's way at a full whip: up along the bike's own up, where it moves it across towards the grips.
It came out as a finding and no code change. The original's rise is not something the arms reach for. It
comes from the riding ragdoll's dynamics. `lean_in` is a static reach solve, so no move it can make goes
that way. The way to fix the drawn rider is to draw the ragdoll where the stand-in still draws, which V63
(delete the stand-in) and P133 (the riding ragdoll online) already cover. Details in
`1-lean-in-whip-direction-FINAL.md`.
