# 1 — vehicle update callees

Task F39. 2026-11-04.

## What I did

- `crates/game-sim/src/handling/update.rs` (new, `pub mod update` in handling.rs): the eight callees
  the bike's vehicle update still ran in the interpreter, as word-level ports in the original's order,
  every word through `update::Updates`, every call out an `update::Out`:
  `fade`, `lean` (with `stunt_clock`), `prepare`, `ride` (the auto gear, returning its v0), `sync`,
  `snapshot`, `network` (returning its v0) and `parts`. `#![deny(clippy::float_arithmetic)]`, the math
  through ps2-float (`Acc` chains where the original keeps the accumulator, VU `.xyz` ops keeping `w`).
- `tests/mxvatv_bike_ride.rs`: `TailInRam::out` now serves the tail's `Fade`, `Lean`, `Prepare`, `Ride`,
  `Sync`, `Snapshot`, `Network` and `Parts` with the port (`port_update`, through `UpdateInRam`, which
  calls the deeper originals: anim, scene nodes, search, the trail/net/part virtuals, the walk).
- The leaf watch in `TreeInRam::step`: every tail listener other than the bike is looked up in
  `OTHER_LEAVES` and the bike plus its physics blocks are compared around its step.
- research/oracle.txt `# F39`: every address, slot, argument and the leaf findings.
- The gate, not F39: `crates/openmx/tests/web/store.mjs` and `gate.toml` are main's blobs (P220/P294),
  taken whole so they merge as no change: `the_browser_installs_from_the_store` failed here twice with
  `listed: null` (since P293 an install waits on the Reload popup, which main's store.mjs answers), and
  `world_ultra_load`'s first-frame budget (20 s) went red in two gates at load 27-48 (49.7 s, 24.7 s);
  main's gate.toml runs it `[alone]`. C14's Kiowa image is copied from s1's
  context/ into this slot's (git-ignored), so `kiowa_capture` runs instead of panicking.

## What the original does

Addresses and slots: research/oracle.txt `# F39`. In words: the fade runs the record's search and the
fade clock, then the alpha onto the rider's model and hung parts; the lean runs the stunt clock and picks
the next stunt (trick step, riding pose, or none); the prepare (its first call is a thunk to the held
states' net unpack) sets the fade rate, the rider's animation (a stunt clip, the riding pose's two-clip
blend, or the half pose), the seat, bars, head and aim nodes, the hung parts from two bone frames, and
moves the attachment; the ride is the auto-gear clock; sync and snapshot fill the camera's, trail's and
replay's records; network ticks the net sender's timers and sends; parts steps the record's and second
part and walks the bike's listeners.

**The task's premise about the second leaf is wrong.** `0x002cf9f0` on `0x01ed82d0` is not a leaf: it
is a manager whose last call is the listener walk of its own list. Its own body does leave the bike
alone, but its walk steps `0x010af350` (step `0x004f2ad0`), a rider character whose target is the
bike's collision record at bike+0x730, and that step writes the bike: +0x920..+0x980 through the
rider's own physics world step (`0x004f1790` -> `0x00505d50` -> `0x00496b70`; a no-op hook on any of the
three stops it), and on the wreck +0x870..+0x8a0 in `0x004f2ad0`'s camera block. The first leaf
(`0x001c9540` on `0x00eb6420`) leaves the bike and its blocks alone, as the task says.

## How it was proved

- `the_vehicle_updates_callees_are_the_originals`: 10 000 random bikes; per bike the eight callees in
  the tail's order, each run once as the original and once as the port from the same words (each from
  the previous one's result), every callee hooked to log and answer. Bike, body, both wheels, the fake
  world and the pose-speed global match word for word, the call logs match, and so do the v0 of ride and
  network. Paths (each ≥ 478): fade held 3816, off 2472, over 1415, alpha 4864, no start 1249; lean trick
  1436, no stunt 6930, clip ran out 4124, hold ran out 691, riding pose 1634; prepare part placed 6529,
  stunt's clip 634, net unpack 1688, stopped 5418, bars turned 4981, pose's blend 1634; ride counting
  1591, manual 2483, shifted or dropped 1725, slow 4201; sync camera's 4445, trail's spot 1430, wreck's
  flag 557, trail off 478, trail on 515; snapshot 3634, stunt kept 699, no stunt kept 3665, thrown 730;
  network due but held 4472, nothing due 4600, sent 928; parts both 6121, fewer 3879.
- `ride_replays_bit_exact_on_the_ports_record_pass` and
  `flip_wrecks_and_is_put_back_bit_exact_on_the_ports_record_pass` stay bit-exact with the eight ports
  in the tail (a temporary print: 924 calls of each over the two replays), under F31b4a's RAM-wide
  shadow check. They also run the leaf watch on every frame: listener `0x00eb6420` and the manager's own
  body write none of the bike's words or its physics blocks; the rider `0x010af350` writes only the
  bike's +0x870..+0x888 except +0x87c, +0x890..+0x898, +0x920, +0x930..+0x938, +0x940, and
  +0x950..+0x97c except +0x95c (ride-2 writes only +0x930..+0x938 and +0x940). The test lets it write
  `0x870..0x8a0` and `0x920..0x980` and fails on anything else. The manager's closing walk is asserted
  to be the one call `walk(itself, dt)`, which the port's `tail::walk` then makes.
- `./kit check -- -p game-sim --test mxvatv_bike_ride`: 23 passed.
- Mutants: the ride's slow test `<= 5/10` to `<`, the snapshot's kept-stunt `< 41` to `<=`, and the
  network's timer due `<=` to `<` each fail the callee oracle (reverted).

## Not verified / not 1:1

- **The rider listener writes the bike** (the words above). Nothing proves whether those words of the
  record matter to the bike's own frame. The replays are bit-exact with the original's rider step in
  place, but the port does not have that write. Filed as a follow-up.
- The deeper callees still run as the originals in the interpreter: anim stop/play/blend, the
  scene-node functions (place, set, spin, aim, frame, part from frame, position, axes, point into
  frame, rotate), the search, the attachment move, the trick step `0x004e49c0`, the gear and net-check
  virtuals, and the trail, net-sender and part virtuals. In the oracle they are fakes that log and
  answer. That proves the port's calls and its use of the answers, not the callees. Follow-ups filed.
- The four-clip blend `0x00140720`: the oracle hooks it to panic and no bike reached it. The port has
  no such call, so a path to it the inputs never hit would not be ported.
- The oracle enters each callee with f12 = dt, as the tail does. A caller that leaves another f12 is
  not covered.
- The listener walk inside `parts` is the original `0x00128af0` (`Out::Walk` → `tail::walk` on the
  replays, a logged fake in the oracle).
- The shadow check's stack window is excluded, as in F31b4a.
- Only `--test mxvatv_bike_ride` was run by hand. The whole suite runs in `done`'s gate.

## Next

None for F39; done. Follow-ups filed: F41 (the rider listener's writes into the bike's record), F42 (the
trick step and the gear and net virtuals), F43 (the anim, scene-node, attachment and search callees),
F44 (the trail, net-sender and part virtuals).
