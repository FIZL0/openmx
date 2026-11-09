# 1 — rider leaf on the ports (PART)

Task F31b5i. 2026-11-08.

## What I did

- **`rider::wreck_spots`** (`crates/game-sim/src/handling/rider.rs`): the rider's listener step's
  own copy after the rider's frame. It is the source of the bike's `+0x870..+0x890` on the wreck. With
  `rider::hand_over` (S278: the bike's `+0x920..+0x980` and `+0x890`), every word the rider writes
  into the bike's record now comes from the port.
- **`rider_leaf`** (`crates/game-sim/tests/mxvatv_bike_ride.rs`): `TreeInRam` runs it in place of the
  original's rider step when `in_rust("rider leaf")`. The rider's frame still runs in the interpreter,
  but these parts of it are served by the port:
  - the mode machine is `rider::modes` (S278b, with S285's wreck arm). This is the first time
    `modes` has been wired into a ride, since S278b and S285 proved it only by oracle;
  - every bone collision solve is `bone::solve` (S278a);
  - every part clear is `rider::clear` and the hand-over is `rider::hand_over` (S278);
  - the bodies' vt `+0x68` is `rider::body_target`.
  The machine's other outside calls go to the original through `LeafRider`: rand, the riding reset,
  the wreck entry, pose, settle, the on/off switches and the gather. After the frame comes
  `wreck_spots`, then the original's copy after it and its extra step.
- **Two tests**, `ride_replays_with_the_rider_leaf_on_the_ports` and
  `flip_replays_with_the_rider_leaf_on_the_ports`. They run `replay_pure` with every port at once
  (F31b5's `replay_whole` names) plus `rider leaf`.
- **Filed F55**: the leaf's own code that is still interpreted (the frame's body, the copy after,
  the extra step).
- Nothing taken from opentennis: it has no rider ragdoll or interpreter-backed frame.

## What the original does

`./kit dis -p atv 0x004f2ad0 0x004f2b70`, the rider's listener step (a0 the rider, f12 dt), in order:

1. the rider's frame (`./kit fn -p atv 0x004f1790`), whose `v0` is the step's answer;
2. with a record (`+0x378`) whose wreck kind (`+0x138`) is set: `lq`/`sq` of rider `+0x660` (the
   impact aim) to the record's `+0x140`. Then the holder (`+0x2a4`) and the record are read again, and
   the main node's `+0x80` goes to the record's `+0x150`;
3. `0x004f2970` (a0 the rider);
4. while `+0xd34` is set, `0x004f2d50` (a0 the rider, f12 dt; it loads its own a1).

The frame (`./kit dis -p atv 0x004f1790 0x004f18b8`) runs `0x004f1610` (a speed scale off
`+0x3a4`, the record's `+0x278` and `+0x27c`, into `0x0018d170`). It then moves the two clocks
`+0x360`/`+0x364` toward `+0x340`/`+0x344` at rate-limited steps. With the record ridden it runs the
mode machine `0x004ef320`, else it clears `+0x25c`. After that it runs the world step `0x00505d50`,
then the rider's vt `+0x148`.

## How it was proved

- **Replays, C15 ride-2 and flip-1**, with `rider_leaf` on top of every F31b5 port. Every row is
  word for word C15's, and the bike and its blocks match the handing run's after every frame. Flip-1
  wrecks on row 125 and is put back on row 193. Outside the stack, the whole RAM matches the handing
  run's, with two exceptions: the dropped cue's word (flip-1, as before) and the ground query's ring
  (`0x541550 + 0x70·32`) with its terrain's `+0x9e0`/`+0x9e4`. The port's query keeps no ring
  (S278a, S285). The tests assert both lists exactly. Served counts:

  | | ride-2 | flip-1 |
  |---|---|---|
  | `modes` (Rust) | 459 | 465 |
  | bone solves (Rust) | 12 393 | 12 303 |
  | hand-overs (Rust) | 459 | 465 |
  | wreck spots (Rust) | 459 (writes only on wreck rows) | 465 |
  | body target (Rust) | 0 | 19 |
  | switch / gather (original) | 1 836 / 0 | 1 463 / 68 |
  | wreck entry / riding reset / pose / settle (original) | 0 | 1 / 1 / 2 / 2 |
  | extra step `+0xd34` (original) | 459 | 465 |

- **A mutant** fails flip-1 on row 125, the wreck row, where the bike's words part from the handing
  run's: `wreck_spots` reading the main node's `+0x70` instead of `+0x80`.
- `./kit check -- -p game-sim --test mxvatv_bike_ride -- rider_leaf_on_the_ports`: 2 passed. Gate:
  `./kit check --all` in `./kit done`.
- **Census** (instructions still interpreted under "the tail: the rider"): ride-2 37.2M → 36.6M,
  flip-1 46.3M → 44.3M. Almost all of the rest is the rider's world step.

## Not verified / not 1:1

- **The task's acceptance is not met: the rider leaf still runs mostly in the interpreter.** The
  rider's ragdoll world step (`0x00505d50`: the 26 other constraints, the forces, the integrate, the
  iterations) is the bulk of the 36.6M/44.3M and is not ported. That is S280. The line's "run on
  S278a's ragdoll world" cannot be done until it is. Blocked on S280.
- **The mode machine's callees are the original's**: the riding reset (S287), the wreck entry
  (S288), pose and settle (S286), the on/off switches and the gather (S289). The bone solves' scene
  place and search calls (`OriginalScene`) are also the original's, which is S279.
- **The leaf's own code is still the original's**: the frame's body (the speed scale call, the two
  clocks, the `+0x25c` clear, vt `+0x148`), the copy after the frame (`0x004f2970`) and the extra
  step (`0x004f2d50`, run every frame on C15). Filed as F55.
- **The ground query's ring cache is not kept** by the port's queries (bone solves, wreck arm). The
  RAM compare leaves the ring and the terrain's two words out. The rows stay bit-exact on C15, so the
  original's later queries are not misled by the stale ring there. Nowhere else is checked.
- **`wreck_spots` has no oracle of its own.** It is a 2-quadword copy proved only by the replays'
  wreck rows (flip-1, rows 125–192) and the mutant. Its no-record and no-kind arms are exercised only
  as "kind 0" (ride-2 and flip-1 outside the wreck). A null record never occurs.
- **The switch's vtable is picked by pointer equality** (`part == rider +0x5bc` means the aim force,
  vt at `+8`; anything else uses `+0x10`). `Call::Switch` does not say which kind it is. This is right
  on C15 (rows bit-exact). A part aliasing the aim force would be wrong, but that does not occur.
- **The nested calls' stack frames sit 0x80 below the hooked call's `sp`**, not where the original
  keeps them. Only the stack differs, and the compare leaves it out.
- **Only C15's two replays, state 4's rider.**

## Next

Blocked until S280 (the world step) lands, with S279, S286, S287, S288, S289 and F55 for the rest of
the leaf. Then swap each callee's `LeafRider` arm, and the leaf's three original calls, for the
port, and the census's "the tail: the rider" should reach 0.
