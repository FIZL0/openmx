# S49 — sweep probe unreached paths

2026-10-20. Task: `./kit plan S49`.

S49 was meant to port the three sweep-probe paths S47 left out: the pass over the search's terrain cell grid
(class slot +0xb0), finite-plane candidates (kind 8) and the smoothed-normal sweep (probe+0x164). It came out
as a proof that none of them runs on this disc, so none was ported:

- A search's grid comes only from the level, which loads one only for a `.scn` with a [SoultreeTerrain]
  section. No `.scn` has one, and no `.tcg` exists.
- Finite planes come only from a type-8 `.col`. None of the 4 220 `.col` files on the disc is type 8.
- Nothing found writes the smoothing field, and every probe in six save states has it clear.

tests/sweep_probe_reach.rs re-proves the disc and state facts. Details are in 1-…-FINAL.md.
