# 1 — a world's mod.json under its own cap (FINAL)

Task MD68. 2026-11-02.

## What I did

- `crates/openmx/src/modstore.rs`: a new `MAX_MOD_JSON` (1 MiB), which `ZipDir::check_mod_json` now
  passes to `file()`. Install (`Store`) and upload (`modstore_up`) both go through it.
  `get_meta` still reads `meta.json` under `MAX_META` (64 KiB).
- I kept the cap and did not take openmx-mods' other option (joins moved into a `joins.json`). The
  biggest world, world_ultra, is 476 KB with 3,217 joins, which leaves more than twice that before
  1 MiB. A `joins.json` would change the standard, stitch and openmx-mods' writer to save nothing the
  store needs.
- Tests in `modstore_tests.rs`:
  - `a_world_mod_json_of_500_kb_is_taken_and_over_1_mib_is_not`: a generated world `mod.zip` whose
    `mod.json` holds 5,400 joins (about 510 KB) unpacks. One with 11,500 joins (over 1 MiB) is refused
    with "`mod.json` is too big". With the old cap put back, the first case fails with that message,
    which is the reported bug.
  - `a_meta_json_over_64_kib_is_refused`: on the fake store, a `meta.json` padded to exactly 64 KiB
    is listed, and one padded to 64 KiB + 1 is not.

## Checked

`cargo test --release -p openmx --bin openmx modstore`: 25 passed, 0 failed.

## Not verified

I did not upload the real world mods to the store. Only generated zips of the same size were tested.
