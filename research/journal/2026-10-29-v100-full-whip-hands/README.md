# V100 — full whip hands

2026-10-29. Task: `./kit plan V100`.

A full whip alone drew the right hand 0.32 ft off its grip: the clips pose the shoulder past the arm's reach
and the limb stand-in could only straighten the arm. Stepping the ported riding ragdoll (the original's drawn
skeleton) through the same ride showed the original neither stretches the arm nor moves the grip — its hand
pins drag the body towards the bars, up to the bike box. The clip-drawn rider now leans in the same way, by
the least move that lands the hands (`rider::lean_in`); whip_tricks.rs `a_full_whip_keeps_the_hands_on_the_bars`
holds it. Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
