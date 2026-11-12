# 1 — ragdoll world in the leaf (PART)

Task F31b5i. 2026-11-11. Follows `../2026-11-08-f31b5i-rider-leaf-on-the-ports/1-rider-leaf-on-the-ports-PART.md`.

## What I did

- **`rider::after_step`** (`crates/game-sim/src/handling/rider.rs`) is the rider's vt `+0x148`,
  the frame's last call. It runs `hand_over`. Then, with a record whose wreck kind is set, it puts
  the spot node's (`+0xb14`) world position and `z`/`y` rows into the record's `+0x220`/`+0x230`/`+0x240`
  (`node::position`, `node::axes`). Either way, the record's `+0x128` goes to the rider's `+0x4f0`.
  The record's `+0x220..+0x250` are the bike's `+0x870..+0x8a0` the task names. Until now they came
  from the interpreter.
- **`rider_leaf`** (`crates/game-sim/tests/mxvatv_bike_ride.rs`) is now served by the ports:
  - the ragdoll world step, hooked when its world is the rider's (`rag::step`, S280). It goes
    through a new `LeafRig`, which is `RamRig`'s kinds with the bone collisions on `PortScene`
    (S279), not `OriginalScene`;
  - the pose and settle, both the mode machine's and the world-step wrapper's own respawn pair
    (`rider::pose` with `still` from `a1`, `rider::settle`; S286);
  - the copy's node position (`node::position`);
  - vt `+0x148` (`rider::after_step`), its vtable slot asserted.
  The old hand-over hook is gone, because `after_step` serves it.
- Each outside call the leaf still hands to the original now books its interpreted instructions
  by name. A new check, `leaf_census`, asserts that the tail's rider ran nothing in the interpreter
  except those calls.
- **Filed F62**: the world step's wrapper and its callees.
- `research/oracle.txt` `# F31b5i`: the rider's real vtable, the vt `+0x148` and wrapper
  layouts, and the census per callee.
- From opentennis: nothing. It has no ragdoll rider.

## What the original does

- vt `+0x148`: `./kit dis -p atv 0x004f28f0 0x004f2970` (written down in oracle.txt `# F41` and
  `# F31b5i`). The rider's vtable is `0x003d11b0`. Its look-alike at `0x003d1030` has the
  hand-over itself at `+0x148`, which is why my first try asserted the wrong slot.
- The world step's wrapper: `./kit fn -p atv 0x00505d50`. Around the ragdoll world (now the
  port's) it runs vt `+0xdc`, a holder loop through `0x00194af0` (only while `+0x254` is set),
  vt `+0xd4`, vt `+0xd8`, the respawn's pose/settle when `+0x258` is set, the node's bounds
  `0x00196e80`, and the base actor step `0x00141d30`.

## How it was proved

- **Replays**: C15 ride-2 (459 rows) and flip-1 (465 rows, wrecked on 125, put back on 193),
  `ride_/flip_replays_with_the_rider_leaf_on_the_ports`. Every port is on at once, as in
  F31b5's every-port replay. Every row is word for word C15's, and the bike and its blocks match
  the handing run's after every frame. Outside the stack, RAM matches as before: the ground
  query's ring, plus flip-1's cue word. Served by the port, exact counts asserted:

  | | ride-2 | flip-1 |
  |---|---|---|
  | ragdoll world steps (S280) | 459 | 433 |
  | bone solves (port's scene) | 12 393 | 12 303 |
  | after step (vt `+0x148`) | 459 | 465 |
  | node position | 459 | 397 |
  | pose / settle (machine) | 0 | 4 / 4 |
  | pose / settle (wrapper's respawn) | 0 | 2 / 2 |
  | still the original's: world step wrapper / rand / cue | 459 / 0 / 0 | 465 / 5 / 1 |

  The scene handed nothing to the original. `PortScene`'s `>` kinds are asserted empty every frame.
- **Census**, instructions under "the tail: the rider": ride-2 36.6M → 9.93M, flip-1 44.3M →
  9.53M. The leaf's Rust parts book 0. `leaf_census` asserts that the remainder is exactly the
  named outside calls.
- **Mutant**: `after_step` with the `z` and `y` rows swapped fails flip-1 on row 125, the wreck
  row. Six of the bike's words part from the handing run's (`0xe71e40..`). Restored with
  cp + touch.
- `./kit check -- -p game-sim --test mxvatv_bike_ride -- rider_leaf_on_the_ports`: 2 passed. Gate:
  `./kit check --all` in `./kit done`.

## Not verified / not 1:1

- **The task's acceptance is still not met.** The world step's wrapper (`0x00505d50`) and its
  callees run in the interpreter. Per ride-2's 459 frames, they are: vt `+0xd4` 5.36M (with a
  3 KB callee and rand), vt `+0xd8` 2.11M, the node's bounds 1.78M, the node set 0.57M, vt `+0xdc`
  0.19M, and the base step 0.02M. The task's dependencies did not include them. They are now F62,
  and the task is blocked on it.
- **Rand, the wreck cue and the heap resize are still the original's** (flip-1: 5 rand draws,
  1 cue, inside the wreck entry). Rand has to stay one stream with the interpreter's other
  draws while any of them run there.
- **`after_step` has no oracle of its own.** It is proved only by the replays and the mutant.
  Ride-2 never wrecks, so only its no-kind arm runs there. Flip-1 runs the wreck arm on rows
  125–192. The no-record return never occurs.
- **The wrapper's pose is served with `still` read from `a1`.** The decompiler shows the wrapper
  passing 1 and the machine passing 0. S286's oracle covered both values of `still`. In the
  replays only flip-1's 2 respawn calls run it.
- **The ground query's ring cache is still not kept** by the port's queries. It stays out of the
  RAM compare, as in the first part.
- **The nested calls' stack frames** sit below the hooked call's `sp` (stack left out of the
  compare), as before.
- **Only C15's two replays, state 4's rider.** C15 never hits a scene object, so `PortScene`'s
  pair and hit paths are not run here (S279a, S279b).

## Next

Blocked until F62 lands. Then serve `LeafRider`'s `Call::WorldStep` with the port of the wrapper,
with the ragdoll world inside it as `rag::step` and the respawn pair as today. `leaf_census` should
then leave only rand and the cue's calls.
