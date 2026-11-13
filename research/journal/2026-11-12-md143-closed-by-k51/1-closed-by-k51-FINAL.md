# 1 — closed by K51

Task MD143. 2026-11-12.

## What I did

- MD143 asked for tests/atv_quad `sim_frame::`'s three whole-frame compares (race_stoppie_short frame 109,
  race_wheelie_launch frame 88, the palm sweep frame 124) to replay again. They had gone red because the
  original writes quads at 0x003dce50.. that the port's pair never writes.
- K51 (merged on main as f5a035b6f, commit 42d6c765e) had already done this work under its own id. It
  identified the words as the scratch for the mesh tree walk's two box-against-triangle tests: corners in
  the box's frame and their centre, the mesh corners (w 1.0), two edges and a third axis, 0x003dce50..0x003dceef.
  It proved statically that each word is written before it is read in every call (xref plus a whole-ELF
  `lui`/`$gp` scan). It then took the range out of `ram_differs` as `TRIANGLE_SCRATCH`. That is the brief's
  second option ("prove nothing reads them and drop them from ram_differs with the reason"). The proof is
  in research/journal/2026-11-12-k51-atv-quad-triangle-scratch/.
- On task/MD143, based on main 72ce4d6e1, `./kit check -- -p game-sim --test atv_quad -- sim_frame::`
  passes (38 tests). No code change was needed. This entry and the tick are the whole of MD143.

## Not verified / not 1:1

- K51's own gaps carry over unchanged: the port does not write the original's triangle-test scratch, and
  write-before-read was proved from the decompilation and an ELF scan, not with a watch in a running
  original. I did not repeat that proof.
