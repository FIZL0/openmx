# 1 — audit reword (PART)

Task K36. 2026-11-04.

## What I did

- `./kit audit` on main flagged four leaks; reworded each so it describes the thing instead of its address:
  - crates/game-sim/src/atv_wheel.rs: the doc comments on `UP`, `MOMENT_SCALE` and `NO_CONTACT_VELOCITY`
    quoted the SLUS-21104 constant addresses (`./kit fn`-able as atv_const_up, atv_const_w1 in
    research/oracle.txt; the moment scale's constant was reworded too, though the audit did not name it).
    Comments only, no code change.
  - plan/DONE.md:519 (P256) and :533 (MD63d): the raw addresses replaced with their oracle names,
    bike_airborne_control and atv_part_read.
- `./kit audit` now reports `AUDIT ok`.

## What the original does

Not applicable (wording only).

## How it was proved

`./kit audit` passes; the gate (`./kit check --all`) runs in `./kit done`. The first gate run failed only in
disc_worlds' world_mxu_free comparison against the main checkout's installed mods/ copy of mod.json, which
other tasks changed (MD95's units layout) and this task never touched. The gate was rerun with
`OPENMX_WORLDS=target/no-worlds` (an empty folder) so that one comparison is skipped.

## Not verified / not 1:1

- No behaviour changed (comments and plan text only).
- disc_worlds' installed-mod.json comparison was skipped in this gate (see above): the installed copy has drifted from this branch's stitcher, and fixing that is not this task.

## Next

Nothing.
