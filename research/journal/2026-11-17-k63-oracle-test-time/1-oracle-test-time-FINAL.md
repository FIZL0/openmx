# 1 — oracle test time (FINAL)

Task K63. 2026-11-17.

## What I did

Profiled each slow test with an `eu-stack` sampler (perf is not allowed here), found where the time went,
and cut that. No test lost a case or a check.

- **camera** (22 tests, the race/preset ones ~750 s each): most of the time was `ps2_oracle::addr`. It
  reads `kit.toml` and the whole address file on every call, and the harness asks for the vtable's
  sixteen slots on every case. The test file now caches each name once per thread. Slowest test after:
  14 s (the ground probe). The curve table build still runs in each test's harness (~5 s each), left alone.
- **atv_quad**:
  - The harness zeroed and laid out RAM through `Ee::set_u32`, and each byte went into the `written`
    BTreeSet that `call` empties before it runs anyway. Those blocks now write the RAM directly (`zero`,
    `raw`). The `each_unhooked` reset+poke pairs run inside the new `Ee::quietly`, so their writes skip
    `written` but are still undone and journaled. air_spin: 170 s → 9 s.
  - The tire reader runs the original ~665k instructions per file over 10,000 files: ~190 s of pure
    interpreter on one core. The 10,000 files are now generated up front from the same seed and split
    over 8 scoped threads, each with its own image. The path tally is summed over all of them and checked
    as before. 454 s → 31 s (load ~70).
- **mxvatv_bike_ride** (ground_reset, out_of_bounds, node_box and the other F47 compares): each case
  cloned and restored the 32 MB RAM, then compared it word by word. ps2-oracle has a new page journal:
  `Ee::mark` starts it, the first write to a 4 KiB page saves that page, `undo` puts back the saved
  pages, and `at_mark` reads a page as it was. The new `split_run` helper runs the original, keeps its
  pages, undoes, runs the port, and compares only the pages either side touched. `bytes_differ`,
  `differing_bytes`, the vehicle update check and the body-damping check now skip equal pages before
  going word by word. Their output is the same.
  out_of_bounds 0.4 s, node_box 1.8 s, ground_reset 16 s, the post processes 8–14 s (out of the world:
  63 s, at 8 tests in parallel under load ~60).
- **ps2-oracle `Ee::call`**: it searched the hooks map on every step. A 64k-bit filter of hooked
  addresses now skips the search for steps with no hook. The filter is rebuilt after each hook runs,
  since only hooks can change the map during a call. `read`/`write` copy their bytes in one go instead
  of a byte-at-a-time fold.

Nothing here came from opentennis.

## How it was proved

- Same tests, same case counts (the tire reader still reads all 10,000 files from seed 0x64b0_0001).
- Mutation checks, made by hand and then reverted:
  - out_of_bounds caught a port-only page, a page both sides wrote, and an original-only page.
  - node_box (through `f47_compare`) caught a port-only page and a both-written page.
  - The cases after each mutated one still passed, so `undo` restores correctly.
- `./kit check` on ps2-oracle, camera, atv_quad and mxvatv_bike_ride; then the full gate in `./kit done`.

## Not verified / not 1:1

- Timings were taken while the machine's load average was 27–166, so they are rough. Before/after pairs
  were taken close together.
- The page journal sees only writes made through `Ee::write`. A hook or harness that writes `ee.ram`
  directly while a test is marked would not be undone. The direct `ee.ram` writes left in
  mxvatv_bike_ride (~19669, ~19766) are in tests that were not converted.
  The direct `zero`/`raw` writes in atv_quad are not under a mark.
- Not converted: other whole-RAM clone sites in mxvatv_bike_ride (~10622, 17127, 17352, 19746, 19865,
  19989). None of them was among the slow tests.
- The replay tests in mxvatv_bike_ride (flip/ride_replays_*, the fade search, the bone collision, the
  vehicle update's callees) still take 70–225 s under load. They are bound by the oracle's per-byte
  `reads`/`written` BTreeSets: filed as K73.
- camera's per-test curve table build (~5 s) is not cached.

## Next

K73: a cheaper record for `reads`/`written` in ps2-oracle.
