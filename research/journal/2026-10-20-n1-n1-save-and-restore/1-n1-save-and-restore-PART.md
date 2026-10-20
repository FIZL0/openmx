# 1 — n1 save and restore (PART)

Task N1. 2026-10-20.

## Decisions taken at the start (unattended, so written down rather than asked)

- **S54 has not landed and nobody is on it** (slots s1/s2/s3/s5 are on P36/P37/P38/P35, plan/TODO.md has
  S54 open). There is no multi-player ride to snapshot. N1 is done over what exists — `Sim`, one bike and
  one rider — and the "multi-player" tests run several independent `Sim`s side by side, which is a
  stand-in: no step order between them and no rider-to-rider contact. Whatever S54 adds lands inside
  `Sim` (or beside it in a type that holds several), and the snapshot is built so that adding a field
  without carrying it is a build failure (see the codec below), not a silent hole.
- **No ggrs in game-sim.** `tests/platforms.rs`' `the_simulation_has_no_dependency_that_could_reach_a_host_libm`
  forbids any non-path dependency line in game-sim's manifest, dev-dependencies included, and the whole
  workspace has no registry crate at all yet (Cargo.lock lists five packages). ggrs 0.13 pulls serde,
  bincode, rand, parking_lot, instant and tracing, and on wasm32 js-sys — and the cross-platform run builds
  the test binaries for wasm32-wasip1. N2 is the task that vendors ggrs (into crates/openmx). So the sync
  test here is ggrs 0.13's `SyncTestSession::advance_frame` / `checksums_consistent` / `adjust_gamestate`
  transcribed into the test, request for request (save, load, advance), not the crate.

## What I did

- **`crates/game-sim/src/snapshot.rs`** (new): `save` / `restore` / `save_into` (a `Snapshot` is the
  `Sim` cloned; `restore` is `clone_from`, so a ring of snapshots reuses its allocations), `Snapshot::hash`
  (the replay harness's `Sim::hash`, integer-only), and `encode` / `decode` for a resync: little-endian,
  floats as their bits, a 4-byte magic, every struct's codec written as a destructure plus a struct
  literal by the `wire!` macro, so a field added to the state and not to the codec does not build.
  `decode` keeps this machine's constants (tables, `.phy`, the rider's stunt set, clips, wreck files and
  body numbers), refuses a wrong magic, a short buffer, a bad bool or enum tag and trailing bytes, and
  writes nothing on an error (it decodes into a copy first).
- **`Sim::phy` is now `Arc<Phy>`**, shared by every clone like the tables (nothing in a ride writes it).
  `size_of::<Sim>()` drops 10224 → 8928; `tests/hash.rs`' size pin updated, and kept to 64-bit targets
  (see below). Five fields widened to `pub(crate)` for the codec (rider's `flight`/`airborne`, the
  event queue's `count`/`events`, `Rng::state`, `Curve::frac_bits`); nothing moved or renamed.
- **`tests/players/mod.rs`** (new): the stand-in multi-player ride — four `Sim`s sharing one set of tables
  over a world of kickers, each with a scripted pad (gas+weave+tricks; wrecks on + the reset chord; an LCG
  pad; gas/brake/clutch/second trick). The tests assert the script reaches air, tricks, landings, wrecks
  and respawns.
- **`tests/snapshot.rs`** (new, 9 tests): save/step/restore/step at six save points, compared both by hash
  and by the whole state's `Debug` text (the hash skips the animation, body and idles); a late input
  rolled back over equals the on-time run; a snapshot shares the `Arc`s and copies the rest; encode →
  decode onto a different machine's state gives the same hash and whole text; decode keeps local
  constants and refuses broken bytes untouched; and **ggrs 0.13's `SyncTestSession` transcribed**
  (max prediction 8, input delay 2, check distances 1..8, 320 frames, save/load/advance requests in
  ggrs's order and its checksum comparison) finds no mismatch, while a deliberately unsaved counter
  is caught by the same harness (negative control).
- **`tests/platforms.rs`**: `several_riders_saved_and_restored_hash_to_their_pinned_vectors` pins the
  four riders' folded hash every 32 ticks to 256, a digest of every tick's hash, the run rolled back 8
  deep every 50 ticks (with a wrong guess for rider 1) equal to the straight run, and the encoded bytes'
  length and digest (16710 bytes for four riders). Generator: `print_the_riders_vectors` (ignored).
- **`tests/cross/platforms.sh`**: runs `--test snapshot` too, and its download is fixed — it asked for
  `rust-std-<host>-<version>-<host>` and always got a 404, so the script had never got past fetching on a
  distribution rustc.

## What the original does

Nothing to read: save/restore and the resync bytes are this port's own machinery, not the game's. The
original has no rollback.

## How it was proved

- `./kit check`: TESTS PASSED (695 on this run; the total is shared with other slots' tests).
- `crates/game-sim/tests/cross/platforms.sh`: **aarch64-unknown-linux-musl ok** (qemu-aarch64-static) and
  **wasm32-wasip1 ok** (node WASI): platforms 12 passed, snapshot 9 passed, hash 16, replay, rng and
  ps2-float's fpu/vu/libm on both. So the four riders' pinned hashes, the rollback run and the encoded
  bytes are the same on x86-64, aarch64 and wasm32.
  The first cross run failed on `the_size_of_the_state_type_is_pinned` on both targets: on aarch64
  because of the `Arc<Phy>` (fixed with the pin), on wasm32 because `size_of::<Sim>()` can never be a
  64-bit number there (each `Vec`/`usize` is half as wide) — that one was broken before N1 and never
  seen because the script could not download a std. The size assert is now
  `#[cfg(target_pointer_width = "64")]`; the leaf count beside it runs everywhere.
- Cost, measured once on this machine with a throwaway test run through `./kit check` (whatever
  profile that builds; not a benchmark, removed afterwards), after 200 ticks of the ride, averaged over
  10 000 rounds: save + restore of four riders 8.1 µs, `save_into` ×4 3.1 µs, hash ×4 19 µs, encode
  4178 bytes a rider. The hash, not the copy, is the expensive part of a rollback tick.

## Not verified / not 1:1

- **Not a multi-player ride.** S54 has not landed; the "players" are four independent `Sim`s stepped in
  turn with no step order of the original's and no rider-to-rider contact. When S54 lands its players'
  state has to go into the codec (the build will say so if it is inside `Sim`) and the `RIDERS_*`
  vectors in `tests/platforms.rs` have to be re-pinned over its ride. This is why the task is handed in
  blocked.
- **Not ggrs itself.** The sync test is ggrs 0.13's `SyncTestSession` logic transcribed into the test;
  game-sim cannot take a registry dependency (its own platforms test forbids it). The real crate is run
  against this API by N2.
- **The rider's disc constants are copied per snapshot**, not shared: the stunt set, clip table, wreck
  files and body numbers are plain fields of `Rider` (a few hundred bytes plus the stunt names'
  `String`s). Moving them behind an `Arc` would touch rider.rs well past what N1 came for. `encode`
  leaves them out and `decode` keeps the local ones, as opentennis does with its constants.
- **The plane's parameters travel in `encode`** rather than being kept local — `Flight` does not split
  its constants from its state, so the bytes carry both. Harmless for correctness, a few hundred bytes.
- **The world is not snapshotted**: nothing on a track moves yet. A moving object added to a world has to
  be state in `Sim`, not in the `World`, or rollback will not see it.
- **The camera is not in the snapshot**: it is the shell's and nothing in a tick reads it back.
- **The whole-state compare is `Debug` text**, which covers every field but would miss a field whose
  `Debug` is hand-written to skip it; there is none today.
- Nothing here was compared against the original — there is nothing in it to compare (no rollback).

## Next

- S54: put the players in the simulation, then extend the codec over them and re-pin the `RIDERS_*`
  vectors (`cargo test -p game-sim --test platforms -- --ignored print_the_riders_vectors --nocapture`).
- N2: run real ggrs's `SyncTestSession` over `snapshot::save`/`restore`/`Snapshot::hash`.
