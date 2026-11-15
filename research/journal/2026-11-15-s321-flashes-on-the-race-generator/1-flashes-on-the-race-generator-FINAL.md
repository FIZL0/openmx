# 1 — flashes on the race generator (FINAL)

Task S321. 2026-11-15.

## What I did

- `crates/game-sim/src/race.rs`: `Race::flashes: Option<Box<CameraFlashes>>` (None by default). The last
  thing `Race::tick` does is tick it on `Race::rng`.
- `snapshot.rs`: the field is on the race's wire (`CameraFlashes` and `PodiumFlashes` wired). Every race
  on the wire grows by one byte (the `None` tag).
- `hash.rs`: the race's walk gets a `flashes` group only when there is one, so any other race hashes as
  before.
- `camera_flash.rs`: the queue and pool fields are `pub(crate)` so the wire and the walk reach them. The
  tick is unchanged.
- The shell (`crates/openmx/src/camera_flashes.rs`, `main.rs`):
  - `Stadium` no longer has a generator of its own. `Stadium::hand_to(race)` moves the object into the
    first seat's race as P327 builds it.
  - The draw paths read the race's object.
  - `player_wins` / `card_finishes` take the race's burst. For the lap line's finishes, the burst is
    copied out before the laps block (it borrows the race) and written back after it.
- Tests (`crates/game-sim/tests/camera_flash.rs`):
  - The capture parse is split out into `capture()`. Rows now carry `rand`.
  - New: `the_flashes_draw_on_the_races_generator_as_s13ds_capture_moves`.
  - New: `a_sims_race_carries_its_flashes_through_a_snapshot`. A Sim with a race and flashes: the
    flashes draw on the race's generator, change the hash, and survive encode/decode, so the two Sims
    step on as one.
- Nothing taken from opentennis: it has no counterpart.

## What the original does

- The loader registers the flash object as a child of the score manager's node (`./kit fn 0x002faa50`,
  the call on `+0x50` with pass 2 near its end).
- The race manager's tick (`./kit fn 0x002fcff0`) ends by calling the score manager's tick on `+0x50`.
- The score manager's tick (`./kit fn 0x0025fcd0`) ends by walking its children (`./kit fn 0x00133500`:
  each child whose pass has bit 2, through its vtable `+0x38`). The flash object's `+0x38` is its tick
  (`./kit fn 0x002f95c0`, the vtable at 0x004587b0).
- So in each frame the flashes tick after the race's clocks, gate watch, countdown and lap handlers, at
  the very end of the race manager's tick. The port does the same in `Race::tick`.
- The loader registers one other child on the same node before the flash object (the result of
  `0x002fea40`, the `+0x3a8` thing). It ticks first and is not ported.

## How it was proved

- **A live capture, by its generator state.** `context/capture/s13d/sx01-finish.jsonl` stores the shared
  `rand()` state every row.
- The new test runs a `Race` (long started, gate open, no laps, so only the flashes draw) through every
  row:
  - The race's generator starts each tick at the row's `rand`.
  - The flash words are held as V127's test holds them.
  - The race's draws are counted against how far the capture's generator moved by the next row.
- Result: 454 batches, each one's draws ≤ the capture's moves, and no draws on any other tick.
- 114 batches were their frame's only draws (the capture moved exactly the batch: 11 at rest). On those,
  the race's generator lands on the next row's state bit for bit.
- 563 ticks in which nothing drew at all. 5 late recorder rows skipped. 0 rows past 4000 draws. It stops
  at the outro (frame 2775), as V127's test does.
- The shell, headless (`openmx --headless --track sx01 --race --race-win 0.5 --frames 60 --shot …`):
  it prints `1200 camera flashes, floor 0.01`. The win at 0.53 s reaches the race's flashes ("flashes
  toward 0.3"), with no panic. The flashes are too small to see in a 640×448 shot.
- The other ticks' extra draws are other callers (AI riders and the rest of the frame), which this
  capture's port does not run.

## Not verified / not 1:1

- **Where the flash draws sit among the frame's other draws is from the disassembly, not the capture.**
  The capture shows how many draws each frame took, not in what order. The 114 frames where the batch
  was alone show the count and the generator. They cannot show the order against AI riders' draws.
- **The rest of the frame's draws are not in the Sim.** The capture's generator also moves 0–30 draws a
  tick for other callers. The Sim's AI lane draws on a generator of its own (`ai::Lane::rng`), not the
  race's. So the Sim's race generator does not match the original's shared one over a whole race yet.
  The flashes' share of it is now right. Filed as S333.
- **Which flashes light is not compared.** The capture keeps the object's words, not the pool's
  contents. The pool's order after a resume (the test's gap) is numbered in queue order, which only
  changes which flash a pick shows, not how many draws it takes.
- **The other child on the score manager's node (`0x002fea40`'s object) is not ported.** It ticks just
  before the flashes. Whether it draws was not read (S334).
- **The outro's stop is still not modelled** (V127's note). The original stops ticking the object once
  the race's outro runs. The race ticks it as long as the race runs, so it keeps drawing there (S334).
- **The shell sets the burst from outside the Sim.** Finishes are heard by the shell (`race_over` after
  the tick) and written into the race's flashes, as before. The race is the first seat's only, so online
  rollback does not replay a finish's burst.
- **Every race on the wire is one byte longer** (the `None` tag of `flashes`). Netplay tests that assume
  a race's byte length would move. The gate (`./kit check --all`) passed with it.

## Next

Nothing. Follow-ups: S333 (one generator for the race and its AI riders), S334 (the outro stop).
