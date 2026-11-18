# 1 — oracle touched record (FINAL)

Task K73. 2026-11-17.

## What I did

- **`Ee::written` and `Ee::reads` are now a `ps2_oracle::Bytes`** (crates/ps2-oracle/src/ee.rs). It derefs
  to the same `BTreeSet<u32>`, so every reader in the other test files compiles unchanged. A byte noted
  sets one bit of its 4 KiB page; the set is only built when something looks (deref), in bulk
  (`append`) when many bytes are pending, by inserts when a few go into a big set. Its own `clear`
  drops the bits without building the set first: `call` clears `written` as it starts, and the
  deref's `clear` built a set just to empty it.
- A first try buffered only inside `Ee::call` and folded the bits into the sets before each hook. It
  cut little (67.5 → 63 CPU-min for the binary): the hot writes and reads were Rust's between calls
  (the rigs laying regions out, the port reading RAM with the watch on), and merging at every hook
  rebuilt the whole set each time. Replaced by the type above.
- **Sampled stacks** (eu-stack, `profiling-without-perf` recipe) showed the rest was not the oracle:
  - five world-search helpers in mxvatv_bike_ride.rs (`sweep_search_in_ram`, `part_search_in_ram`,
    `fade_search_in_ram`, the sweep-all one, `sphere_search_in_ram`) added the plant parts with
    `items.contains` per part, quadratic: now a `HashSet` beside the list, same order of pushes.
  - `the_node_transforms_are_the_originals` cloned all 32 MB of RAM three times a case and compared
    all of it: now K63's `split_run` over the journal, the image put back by `undo`.
- Tests: `reads_and_writes_are_whole_when_a_hook_looks_and_when_the_call_ends` (a hook and the caller
  see every byte, a store across a page edge too) and `the_byte_record_reads_as_the_set_it_was_given`
  (20,000 random notes over scratchpad and RAM pages, looked at between, the wrap at `u32::MAX`, clear
  with notes pending) in crates/ps2-oracle/tests/interpret.rs.
- Nothing taken from ../opentennis: its oracle (crates/hst-oracle) keeps no read or write set.

Times of the whole mxvatv_bike_ride binary. Through `./kit check` (build included, its own thread count):
main 11m13s wall / 67.5 CPU-min, the first try 9m04s / 63 CPU-min. Run directly with
`--test-threads 8 --report-time`:

| build | wall | load avg | slowest tests |
|---|---|---|---|
| first try (bitmap during calls only, ~main) | 785 s | ~33 | vehicle update's callees 206 s, flip with every port 204 s, node transforms 189 s, fade search 170 s, bone collision 142 s |
| `Bytes` + test fixes | 314 s | ~32 | sweeps' global search 70 s, vehicle callees 68 s |
| + `Bytes::clear` | 227 s | ~20 | fade search 59 s; vehicle callees, bone collision, node transforms under 36 s |

## What the original does

Nothing of the original changes: this is test tooling.

## How it was proved

- The tests that use `written`/`reads` pass unchanged on the new type: the whole mxvatv_bike_ride
  binary (149 tests), ps2-oracle (32), and `./kit check --all` in `./kit done`.
- `Bytes` against a plain `BTreeSet` over 20,000 random notes (above).

## Not verified / not 1:1

- The times come from a shared machine at different loads (~33 vs ~20 for the last row); the 314 s row
  is the like-for-like one against 785 s. main itself was not run with `--report-time`: its per-test
  times are the first try's, which differed from main by little (63 vs 67.5 CPU-min).
- The fade search (59 s) and the sweeps' global search (47 s) still spend their time in the test:
  `bytes_differ` compares all 32 MB twice a case, and `GatherInRam::write_query` (gather_in_ram.rs,
  not this task's file). Filed as a low follow-up rather than done here.

## Next

Nothing; the follow-up is in the plan.
