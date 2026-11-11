# 1 — rider leaf own code (FINAL)

Task F55. 2026-11-11.

## What I did

- **`crates/game-sim/src/handling/rider.rs`** gets these new functions:
  - **`alpha`**: the rider's scene node's alpha, set at the start of the frame;
  - **`frame`**: the rider's frame. It runs `alpha`, the two rate-limited clocks, then either the
    mode machine (`modes`) or the `+0x25c` clear. After that it calls the world step and vt
    `+0x148`. The frame's answer is the world step's;
  - **`copy_after`**: the copy after the frame (rider `+0xb0c`, `+0x9e0..`, `+0x910`, `+0xaa0`,
    `+0xab0`);
  - **`extra_step`**: the step that runs while `+0xd34` is set;
  - **`listener_step`**: all of the above together with F31b5i's `wreck_spots`.

  `Call` gets three new variants for the outside calls: `WorldStep`, `AfterStep` (vt `+0x148`) and
  `NodePosition` (the scene node's position, for the copy's arm that has no wreck).
- **`rider_leaf`** (`tests/mxvatv_bike_ride.rs`) now calls `rider::listener_step` through
  `LeafRider`. `LeafRider` sends the three new calls to the original. The mode machine is no longer
  hooked: Rust's `frame` calls `modes` directly. The bone solve, part clear and hand-over hooks
  stay, for the world step and vt `+0x148`, which still run in the interpreter. The constants for
  the frame, copy, extra step and mode machine that are no longer used are gone.
- **The oracle**: `the_riders_listener_step_is_the_originals` in `tests/mxvatv_rider_modes.rs`.
- **The brief's "speed scale" is the node's alpha.** The call it names, `./kit fn -p atv
  0x0018d170`, is a setter of the node's `+0x17c`. It is the same word that `handling::update`
  writes as the hung parts' alpha. The port writes it inline, as `update.rs` does.
- Nothing taken from opentennis: it has no ragdoll rider.
- **Carried over from wip/F55.** The first session finished this work but ended before committing it. The second session cherry-picked it onto main after S289. S289 had already moved the switch and gather to Rust (`= switch`, `= gather`), so the replays' served lists were merged by hand. They now pass with those counts and the F55 calls together. Both C15 replays and the oracle were run again on the merge, and both passed.

## What the original does

`research/oracle.txt` `# F55` has every word. The instructions:
- the frame: `./kit dis -p atv 0x004f1790 0x004f18b8`;
- the alpha: `./kit dis -p atv 0x004f1610 0x004f1790`;
- the copy: `./kit dis -p atv 0x004f2970 0x004f2ac8`;
- the extra step: `./kit dis -p atv 0x004f2d50 0x004f2ea4`.

In words:

- **Frame.** With no scene node or no record it answers 1 at once. The alpha comes from the rider's
  kind. Kind 2 gives a half. Kind 3 gives a half or a whole by `+0x8fc`; the original reads that
  word twice, and the port does too. Any other kind takes the record's ask (`+0x278`), then the
  record's fade (`+0x27c` above 0). The fade is a remap clamped to 0.1..1, done on the accumulator
  (`adda`/`madd`), or plain 0.1 when its start (`+0x280`) is 0.

  Next come the two clocks. Each one's step limit is `rate · dt`. Which rate is used depends on a
  flag: `+0x350`/`+0x358` for the first clock, `+0x354`/`+0x35c` for the second. Both steps are
  worked out before either clock moves, and the move is `max(−step)` then `min(step)`. Then comes
  the mode machine when the record is ridden; otherwise `+0x25c` is set to 0. Last come the world
  step and vt `+0x148`.
- **Copy.** It sets `+0xb0c` from the record, or with no record from `+0xa74` when the mask allows.
  With `+0xa70` equal to 1, the null-record arm reads three quadwords from low memory, exactly as
  the original does. Two mask-gated quadwords follow: each comes from the wrecked record's `+0x220`,
  or else from the node's position. The second is followed by the record's `+0xa0`.
- **Extra step.** It reads a 32-byte table entry, picked by the record's `+0x1e8`. On the "down"
  side, a time of 0 or less zeroes the value and the clock. On either side, above the speed limit
  (5 down, 10 up) the clock runs on, and once it passes the entry's time the value takes the step,
  clamped to 0..`+0xd38`.

## How it was proved

- **Oracle, 60 000 cases** (`the_riders_listener_step_is_the_originals`, state 4's rider image):
  20 000 each for the frame (record not ridden), the copy and the extra step. The original runs in
  the interpreter. Its world step and vt `+0x148` are hooked to log, and the world step returns a
  random answer. Its node position writes a marker quadword, and so does the port's.

  Every word written outside the stack matches, the calls match in order, and the answers match.
  The inputs include signed zeros, denormals, words with exponent 255, `dt` of 0, and every kind,
  ask, flag and mask arm. Every path was taken at least 488 times. The tally was seen through a
  temporary probe, then taken out.
- **Two mutants fail it.** Swapping the extra step's two speed limits fails on extra case 8 (the
  answer). Reversing the clocks' clamp order (`min` first) fails on frame case 2 (the clock word).
- **Replays, C15 ride-2 and flip-1** (`ride_/flip_replays_with_the_rider_leaf_on_the_ports`) now
  run `rider::listener_step`. Every row matches C15 word for word. Flip-1 wrecks on row 125 and is
  put back on row 193. Outside the stack, the RAM matches the handing run's with the same exceptions
  as F31b5i: the ground ring, and flip-1's cue word. These replays prove the frame's ridden arm, the
  composition with `modes`, the copy's record arm and the extra step on real data. Served counts:
  - ride-2: listener step 459, extra step 459, world step 459, after step 459, node position 459;
  - flip-1: the same at 465, with node position 397, which is 465 less the 68 wreck rows (on those
    rows the copy takes the record's quadword).
- Census of "the tail: the rider": ride-2 36.6M → 36.5M, flip-1 44.3M → 44.2M. The leaf's own code is
  small; what is left is the world step (S280) and the callees.
- `./kit check -- -p game-sim --test mxvatv_bike_ride -- rider_leaf_on_the_ports`: 2 passed.
  `./kit check -- -p game-sim --test mxvatv_rider_modes`: 5 passed. Gate: `./kit check --all` in
  `./kit done`.

## Not verified / not 1:1

- **The frame's ridden arm is not in the oracle.** The oracle forces the record unridden, because
  Rust's `frame` calls `modes` directly and the original's mode machine would need every callee
  hooked. The ridden arm is proven only by C15's two replays, where the record is always ridden. The
  mode machine itself has S278b's oracle.
- **The world step, vt `+0x148` and the node position are still the original's**, through
  `Call`. The world step is S280. vt `+0x148` was not identified beyond its slot. The node position
  has no RAM port: `atv_moto::Scene::position` works on the scene model, not on RAM.
- **The copy's null-record arm reads low memory** (`0x220..0x250`, `0xa0..0xb0`), as the original
  does. Only the oracle exercises it, on the image's low memory. A ride never has a null record.
- **The replays cover only state 4's rider on C15.** On C15 the extra step's switch is on every
  frame. Which of its arms ran there was not tallied.
- **The nested calls' stack frames sit 0x80 below `sp`**, not where the original's frame keeps
  them. This is `call_from_hook_dt`, as in F31b5i. Only the stack differs, and the compares leave it
  out.

## Next

Nothing for this task. The rider leaf still waits on S280 (the world step) and on the callees'
`LeafRider` arms (S279, S286; S289's switches and gather have landed).
