# F31b — SLUS-21104's bike sim, split; F31b1: the bike's PreProcess

2026-11-03. Task: `./kit plan F31b`.

F31b is the whole per-frame bike sim of MX vs. ATV: too much for one session. It is split
into F31b1..F31b5 (PreProcess; Force body; PostProcess; record pass, tail, wreck and put-back;
the whole frame in pure Rust). This session did F31b1: the bike's PreProcess (ground head) is ported
as `handling::bike::ground_head`. It matches the original on 12 000 random bikes, and it replaces the
original in F31a's hybrid frame for every row of C15 ride-2 and flip-1, which stay bit-exact.
Details in 1-bike-pre-process-FINAL.md.

F31b2 (the next session): the bike's Force body is ported as `handling::bike::ground_step`. It matches
the original on 12 000 random bikes, and it runs in the hybrid frame (with F31b1's head) for every row of
ride-2 and flip-1, bit-exact. It found and fixed a `−0` in MD63b's balance. Details in
2-bike-force-body-FINAL.md.

F31b3: the bike's PostProcess is ported as `handling::bike::air_state`, with the no-hander and the vehicle
post. It matches the original on 12 000 random bikes, and it runs in the hybrid frame (with F31b1's head
and F31b2's Force body) for every row of ride-2 and flip-1, bit-exact. Details in
3-bike-post-process-FINAL.md.

F31b4: the bike's collision record pass (the sim's `+0x24` list: the clear, the sweep back, the
heightfield and world rays, the contact filing) is ported as `handling::record`. It matches the original on
12 000 random records, and it runs in the hybrid frame (with F31b1–F31b3's ports) for every row of ride-2
and flip-1, bit-exact, flip-1's 470 heightfield queries through `handling::ground::under`. The rest of
F31b4 is filed as F31b4a (the tail), F31b4b (the wreck and put-back) and F31b4c (MD63e's mass
properties). Details in 4-record-pass-FINAL.md.
