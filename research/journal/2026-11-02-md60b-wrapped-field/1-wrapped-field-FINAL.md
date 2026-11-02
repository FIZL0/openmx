# 1 — wrapped field (FINAL)

Task MD60b. 2026-11-02.

## What I did

- Applied openmx-mods `git diff 65db95d d9e38b8 -- tools/stitch/src/build.rs` to
  `crates/stitch/src/build.rs` (13 of 16 hunks applied as they were). By hand: `Plan::period`,
  `images`, `shifted`, `wrapped_gap`, and in `plan_widened`'s grid path the `across` pairs. Their
  gaps go to `finish` as the `tail` past the farthest interior, the same `end` C12 computes.
- `finish` sets `period` from `size` on each axis `Params::wraps()` names, for both layouts. P251's
  `wrapped_copies` (extra sources one world over, unit layout only, radii shared per map) is gone: the
  images do the same job, and the relief (MD57) no longer has copies outside `relief`'s index to
  read. The look's per-tile share sum stays; with no copies it adds one share per tile.
- check.rs: C12's `wrap_seams` and `Report::wrap_points`/`wrap_height`. openmx.rs: `World::wrap`,
  `World::closed` (game-sim's seam joins, as openmx closes a wrapped world) and `across_seams`.
  These are for the test. openmx-mods' `stitch holes` CLI and `ultra.rs` have no counterpart in
  openmx, so they are not ported.
- New `crates/stitch/tests/wrap.rs`: openmx-mods' first wrap test (2×2 generated world, field equal a
  period on, relief by both far edges equal from `relief_cands` and from all, check's wrap points,
  closed world without holes and no step ≥ 0.5 ft across the seams; built without the wrap, 3.7 ft).
  Its ultra test is left out (no ultra in openmx).
- `stitch::VERSION` 2 → 3, so the shell's disc-world cache builds again.

## What the original does

Nothing: the stitched worlds are generated ground, not the original's.

## How it was proved

- `cargo test --release -p stitch`: all pass, the new wrap.rs (0.247 ft step across the seams;
  3.711 ft for the same tiles built unwrapped) and P251's units_wrap.rs included.
- world_mxu_free off the disc (`--test disc_worlds`, empty `$OPENMX_WORLDS`), compared with a build by
  openmx-mods d9e38b8's stitch library (a scratch crate on `~/repos/openmx-mods-md60a/tools/stitch`, the
  freeride tiles from `~/repos/openmx-mods-md54/out/stage.mxu`, the spec's params): `diff -rq` on
  joins/ finds nothing (38 joins). mod.json differs only by openmx's `"category": "World"` line.
- `openmx --headless --no-sound --frames 2 --seams` (release) on the disc builds:
  - world_mxu: 480 joins, 21853 × 19828 ft, no holes, no gaps, steepest 0.614 ft, 0.057 across the
    wrap. These are P251's numbers with `wrapped_copies`.
  - world_mxu_free: steepest 0.276 ft, 0.040 across the wrap (the installed C12-less world: 0.057).
  - world_mxu_nat: 815 joins, steepest 0.700 ft, 0.061 across the wrap.
- `cargo test --release -p openmx --test disc_worlds` with `OPENMX_WORLDS` empty: 3 pass.
  `--bin openmx disc_worlds`: 3 pass (world_mxus_widest_join_is_narrow included).

## Not verified / not 1:1

- disc_worlds' world_mxu_free comparison against the main checkout's installed world fails until MD60
  reinstalls the worlds from openmx-mods main. The installed copy is pre-C12: its mod.json join list
  and 76 join files differ. Built from d9e38b8 it is byte-equal, apart from the category line that
  MD60 puts back.
- I did not check world_mxu against openmx-mods: its spec differs since P251 (`trim`, `layout: units`).
  Only its `--seams` numbers were checked, not a byte diff against HEAD's build.
- No ride across the wrap on screen.

## Next

none
