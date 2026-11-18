# 1 — land openmx mods machine branches (FINAL)

Task MD244. 2026-11-17.

## What I did

- Checked each listed `openmx-*` branch in openmx-mods against `origin/main` with `git cherry`.
- Already on main under other hashes (nothing to do): MD39c, MD54, MD80d, MD84, P315e (MD184).
- Not on main, cherry-picked onto `origin/main` (bcaee24) in a scratch worktree, in this order, no conflicts:
  - MD163 (baja truck, `untamed_cars`), MD81e (golf cart), MD81f (sandrail), MD144 (`untamed_atv` race quads),
    MD161b (monster truck), MD144b (`untamed_atv` rider set),
  - MD66 (`category` on every map, standard §1),
  - MD157 (`mxvatv_cars` textures, cameras, engine sounds).
- Pushed to openmx-mods main: bcaee24..f35d3ea (fast-forward).
- Installed the five machine packs built from that main (`mxvatv_atv`, `mxvatv_cars`, `mxvatv_fliers`,
  `untamed_atv`, `untamed_cars`) into `mods/machines/` of every slot, s1–s6. Each pack folder was swapped
  whole, and only where it differed or was missing. All six slots now hold byte-identical packs.
- Before: s1, s3, s6 had old `mxvatv_fliers` (from before MD130/P315e); s6 had `untamed_atv` from before
  MD144b; s3, s5, s6 had no `untamed_cars`; s4 had no `mods/` at all. s2's five packs already matched the
  new build byte for byte.

## What the original does

No game logic here. The packs are copies of disc files.

## How it was proved

- `./build.sh --check --no-install` on the landed tree, with all four discs in `iso/`: every step ran,
  `modcheck: 110 ok, 0 failed`. The machine packs were compared byte for byte with the discs: mxvatv_atv
  467 files, mxvatv_cars 337, mxvatv_fliers 176, untamed_atv 158, untamed_cars (with the monster truck).
- `diff -rq` of each slot's packs against the build output: no differences after the install.

## Not verified / not 1:1

- openmx-mods' `./check.sh` (its full test suite) was not run, only `build.sh --check`, which is what the
  brief asked for. The cherry-picks carried their own tests, which their own tasks ran on their branches.
- The main checkout `~/repos/openmx/mods/` was left alone (the rules forbid touching it), so it still has no
  `mods/machines/`.
- MD66's category field reaches newly built track mods only. The track mods installed in the slots and
  in the main checkout were not reinstalled.

## Next

none
