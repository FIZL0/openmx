# N24 — planes meet on a tape

2026-11-17. Task: `./kit plan N24`.

N22a1 proved two planes meeting on a poked RAM image; N24 redid it on a two-pad PCSX2 tape, where P2
switches to a plane and P1 taxis its plane into it. P1's plane is always plane 0, whoever switched
first. Flown together from the original's RAM, both planes hold 100 of 100 ticks through the touch and
the hit only with seat 0's plane first, so the port's shell order (last flying seat first) is wrong:
N28. Two planes pushing one bike could not be set up (the flyers' bikes leave the world). Details in
1-planes-meet-on-a-tape-FINAL.md and research/n24/index.md.
