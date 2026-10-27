# S107 — save state tick one

2026-10-25. Task: `./kit plan S107`.

The S64 save-state run's tick-1 parting was the chassis' inverse inertia. The original inverts the
`.phy`'s tensor with its general affine inverse, which differs by an ulp (and by its `−0`s) from one
reciprocal per axis. That inverse is now ported as `math::inverse` and proved on 10,000 generated
matrices, and the save-state run agrees on every lane on every tick (both ratchets now `u64::MAX`).
S89's rail rides were on the wrong bike (`race250a`, where the states hold `race250p`). With that fixed
they agree through tick 1's first substep, then part on the swept contact's point on the rail bed;
that is filed as S123. Details in 1.
