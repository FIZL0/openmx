# 1 — rider leaf census closed (FINAL)

Task F31b5i. 2026-11-13.

## What I did

- Nothing new had to be ported. F62 (the world step's wrapper), F62a (the idle layer) and F62b (the clip
  advance and the clip start) had already moved the leaf's remainder into `rider_leaf`'s ports. This
  session checked the task's acceptance against them.
- `crates/game-sim/tests/mxvatv_bike_ride.rs`:
  - `leaf_census` now also asserts that every outside call the leaf books into the interpreter is rand,
    the cue's, the sound's or the heap's. That is the acceptance's "census 0 but rand/heap/cue",
    stated outright.
  - Its doc and `rider_leaf`'s doc no longer say the clip advance and clip start are the original's.
- Nothing was taken from opentennis; it has no rider leaf.

## What the original does

Unchanged from the earlier parts and from F62/F62a/F62b's journals. Recipes are in those parts and in
`research/oracle.txt` (`# F62`, `# F62a`, `# F62b`).

## How it was proved

- **Replays:** `ride_replays_with_the_rider_leaf_on_the_ports` and
  `flip_replays_with_the_rider_leaf_on_the_ports`, run with
  `./kit check -- -p game-sim --test mxvatv_bike_ride -- rider_leaf_on_the_ports`, gave 2 passed.
  - Every row matches C15 word for word, and RAM matches the handing run after every frame.
  - The only RAM exceptions are flip-1's cue word and the ground query's ring
    (`leaf_off` = `0x851fec..0x851ff0`).
  - The served counts are asserted exactly, as in the earlier parts.
- **The tests really ran:** a pass hides skips, so I added a temporary `panic!` after `leaf_census` in
  both tests. Both panicked there, which means the fixtures loaded and the new assert held. The census
  it printed for "the tail: the rider":
  - ride-2: **0** instructions (the key is absent).
  - flip-1: **1,823**. That is rand 432 (8 draws), cue busy/playing 59/104 and sound busy/play
    11/1,217. No heap call was made.
  - For comparison: F31b5's census was 37M / 46M, the first part's 36.6M / 44.3M, the second part's
    9.93M / 9.53M.
  - I removed the probe and restored the file with cp + touch.
- **Gate:** `./kit check --all` in `./kit done`.

## Not verified / not 1:1

- **Rand, the wreck cue's audio calls and the heap stay in the interpreter**, as the acceptance allows.
  Rand has to stay one stream with the interpreter's other draws while any of them run there. Audio
  and the heap are outside the simulation.
- **The ground query's ring cache is still not kept by the port's queries.** It stays out of the RAM
  compare, as in the earlier parts.
- **The nested calls' stack frames** sit below the hooked call's `sp`, and the stack stays out of the
  compare, as before.
- **Only C15's two replays, with state 4's rider.** Paths those rides never reach are proved only by
  their tasks' oracles. Those are listed in F62/F62a/F62b's and the earlier parts' sections: scene
  pairs and hits, the per-bone list, mirror/mask arguments, the node turn/place bits and
  `after_step`'s no-record return.
- **`after_step` still has no oracle of its own.** It is proved by the replays and the second part's
  mutant only.

## Next

Nothing left for F31b5i.
