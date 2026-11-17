# 1 — bladebent cols (FINAL)

Task MD130. 2026-11-16.

## What I did

- openmx-mods `tools/import-atv` (commit 19ae192, pushed): `fliers::LOOSE` gains `Veh\Col\BladeBent*.col`, so
  the `mxvatv_fliers` pack carries `BladeBentF.col` and `BladeBentM.col` loose at their PSR paths, listed in
  mod.json's `psr`. Its disc test now counts 18 collision files (was 16).
- `crates/openmx/tests/mxvatv_fliers.rs`: the MD109 thrown-blades test now wants every blade file off the pack
  (4 of 4) when the pack has the two `.col`; a pack built before MD130 still wants 2 and says SKIPPED for the rest.
- This worktree's `mods/machines/mxvatv_fliers` got the two files and the two mod.json `psr` lines added by hand,
  not a full rebuild. The installed pack's skins carry `name`/`about` fields that today's importer does not write,
  and a full rebuild would have dropped them (filed as MD184).
- Nothing taken from ../opentennis: it has no MX vs. ATV packs.

## What the original does

MX vs. ATV keeps the Kiowa's thrown-blade collision loose on MXVSATV.PSR as `Veh\Col\BladeBent{M,F}.col`.
openmx reads them by leaf name off a pack (MD109's blade-file reader), else MX Unleashed's
`helicopter\col\bladebent{m,f}.col`.

## How it was proved

- `cargo test -p import-atv --release --test fliers -- --ignored` (openmx-mods): passes. modcheck finds every
  pack file byte-identical to the MX vs. ATV disc, and openmx's readers parse both new files (Rays for M, Mesh for F).
- `cmp` against `context/res/machine/helicopter/col/bladebent{m,f}.col`: both identical (sha256 36ce1d97… M,
  1af0cf8a… F).
- `the_packs_kiowa_throws_the_packs_blades` passes. I checked that it really rides by briefly making it expect 5.
  It failed and printed `kiowa: thrown blades: 4 of 4 files off mxvatv_fliers`.

## Not verified / not 1:1

- Only this worktree's `mods/` has the new files. Every other checkout (main, other slots) keeps the old pack
  until someone reinstalls it. Until then the test there wants 2 and prints SKIPPED for the collision pair. Why:
  `mods/` is git-ignored, and I may not touch the main checkout.
- Thrown-blade physics did not change: the bytes are the same as the disc copies used before.

## Next

none
