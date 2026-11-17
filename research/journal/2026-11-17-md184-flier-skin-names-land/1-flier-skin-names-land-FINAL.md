# 1 — flier skin names land (FINAL)

Task MD184. 2026-11-17.

## What I did

- Why rebuilding dropped the names: the tool that writes them was already done, in P315e (openmx-mods commit
  `e856bd3`). That commit was pushed only to the branch `openmx-P315e` and never merged, so openmx-mods main's
  `import-atv fliers` never had it. MD130 rebuilt the pack off main and lost the names.
- So I put P315e's commit on top of today's openmx-mods main (it applied cleanly beside MD130) and pushed it to
  main as `bcaee24`. I worked in a scratch worktree on branch `openmx-MD184`. Pushing to main follows how MD96, MD111 and
  MD130 landed there, and the user's standing "commit and push to main" rule for openmx-mods. Another unmerged
  branch would leave the same gap.
- I took the whole commit, not only `tools/import-atv/**`. It also has modcheck's checks of `skins[]` fields
  and the standard's §7a text for `name`/`about`. Without those the tool would write fields the standard does
  not describe. Neither touches any openmx file.
- What the tool writes for each skin (from P315e): `name` is `UI\Store.dat`'s `StoreText`, in the case
  `Challenges.dat` uses. `about` is the model sentence, plus `Veh\Vehicles.dat`'s garage `Logo`, the store
  `Price`, and the challenge whose `Prize` is that skin.
- openmx itself is not changed. The slot's installed (git-ignored) `mods/machines/mxvatv_fliers` is left as it
  was.
- opentennis: nothing taken (it has no machine packs).

## What the original does

Not new here: see P315e's journal (`research/journal/2026-11-09-p315e-flier-skin-names`). The names come from
MX vs. ATV's store, garage and challenge tables. There are no livery names on the disc.

## How it was proved

- openmx-mods `cargo test --release -p import-atv --test fliers -- --ignored`, with openmx linked to this slot:
  passes. It builds the pack off SLUS_211.04 and asserts all eight names and two abouts.
- I built the pack with `import-atv fliers --disc` and compared it with the installed
  `mods/machines/mxvatv_fliers` (the one MD184 cites):
  - all eight `skins[]` entries are equal, `name` and `about` included;
  - every other mod.json key is equal except `source.members`, which gains MD130's two `BladeBent*.col`;
  - all 135 files the two packs share are byte-identical, and the new build adds only those two `.col`.
- modcheck `a_machine_packs_skins_are_held_to_the_standard`: passes.
- openmx-mods `./check.sh` (whole workspace): ok, 154 passed, 0 failed.

## Not verified / not 1:1

- Everything in P315e's own list still stands, unchanged here. None of it was rechecked:
  - whether MX vs. ATV's garage shows `StoreText`;
  - Challenge 8's text names Plane (2) while its `Prize` is Skin 4;
  - what `Logo` means;
  - that `BiPlane Skin n` is model `biplane<n>`.
- I did not reinstall the rebuilt pack in openmx's `mods/`. The installed copy already has the same skins.
  Only MD130's two blade `.col` are missing from it, which is MD130's business.

## Next

none.
