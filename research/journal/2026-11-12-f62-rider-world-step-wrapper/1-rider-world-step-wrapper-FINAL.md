# 1 — rider world step wrapper (FINAL)

Task F62. 2026-11-12.

## What I did

- `handling::rider::world_step` ports the rider's world step wrapper (the frame's `Call::WorldStep`). It
  covers the paused arm, the drawn frames (`keep_drawn`, vt +0xdc), the controller's node bits
  (+0x2d bits 4 and 5), the latch's holder loop (`node::set_position`), the ragdoll world and node aims
  (`aim_nodes`, vt +0xd8, with its helper that runs over every holder), the respawn's still pose and
  settle, the node's bounds, and the base step (`base_step`: the toggle, the entry loop, `tail::walk`
  over the rider's own group).
- `node::orient_here` covers the node turn with no frame and no position (rotation rows only, the
  position's xyz kept).
- New `rider::Call` variants for what stays outside:
  - `Idle`: vt +0xd4, still the original's.
  - `RagWorld`, `PoseStill` and `Bounds`: the ports, served in the test.
  - `EntryBind`, `EntriesGrow` and `Listener`: C15 never reaches them, and the test panics if it does.
- tests/mxvatv_bike_ride.rs:
  - `rider_leaf` now serves `Call::WorldStep` with the port.
  - The `WORLD_STEP`, `RIDER_POSE` and `RIDER_SETTLE_VT` hooks inside the interpreted wrapper are gone.
    Their work is now done through `Call::RagWorld`, `Call::PoseStill` and `Call::Settle`.
  - The test asserts the vtable slots: rider +0xdc, +0xd8, +0xd4 and +0xd0, and the node's +0x94.
  - Bounds runs on `node::bounds` and `attach::spatial_move`. Any heap call the move makes is booked as
    an outside call.
- I split the task, because the idle layer alone is ~5.4M interpreted instructions over ~4 KB of MIPS,
  plus ~6.4 KB more on flip-1:
  - F62a: the idle layer (vt +0xd4: state picker, 3 KB clip chooser, blend4).
  - F62b: vt +0x88's clip advance and crossfade, and the wreck states' clip start (flip-1 only).
- F31b5i is now blocked after F62a and F62b.
- research/oracle.txt `# F62` has the whole map.

## What the original does

See research/oracle.txt `# F62` (`./kit dis -p atv` on the wrapper, vt +0xdc, vt +0xd8 and its helper, the
node turn, the base step). In words:

- The wrapper keeps each holder's node world frame as the holder's drawn frame.
- The ragdoll owns the nodes while +0x25c and +0x260 are set and +0x254 and +0x258 are clear. In that
  case the animation's node bits are cleared; otherwise they are set.
- The idle layer runs. The ragdoll world steps, and the nodes are put on the bodies: the first holder at
  the body's position less its turned centre-of-gravity offset; when wrecked, the others are only turned.
- A respawn ask poses the ragdoll still and settles it.
- The node's bounds go into the spatial database, and the controller's base step runs.

## How it was proved

C15's two bit-exact replays with the rider leaf (`ride_replays_with_the_rider_leaf_on_the_ports` over 459
rows of ride-2, `flip_replays_with_the_rider_leaf_on_the_ports` over 465 rows of flip-1). RAM matches the
handing run apart from the ground ring, the rider terrain's +0x9e0 words and, on flip-1, the cue's word,
the same exceptions as before. The tail's rider census now equals the outside calls (idle layer,
rand, cue, heap). Instructions interpreted in the leaf on ride-2 went from 10.7M to 5.36M, and 5.36M is
the idle layer alone.

## Not verified / not 1:1

- The idle layer (vt +0xd4) is still the original's. It is split as F62a (and F62b for the clip advance it
  calls on flip-1), so the acceptance line "census 0 but rand/heap/cue" is met only once those land.
- `node::orient_here` on a root node: the original copies an uninitialised stack row as the position row,
  so the root's +0x6c is that stack word. The port leaves +0x6c alone. Not portable. All of C15's holder
  nodes have parents.
- `node::orient_here` under a parent: the position row's w is taken as computed from a zero row. That
  holds when the parent's world rotation rows have w 0 (it held on C15: the replays match). A parent
  with nonzero rotation-row w would read the stack garbage, and the port would not follow.
- The base step's entry loop (entry bind, entry list growth) and the rider group's listener walk are ported
  as control flow, but C15 never runs them: the entry count is 0 and the group's step flag is clear. Their
  calls are outside calls that panic in the test. Unproved.
- The paused arm (the frame skip flag) is never set on C15. Unproved.
- The world step's return value: the served call answers 0. The replays match with it, but I did not check what, if anything, the caller reads.
- Only C15's ride-2 and flip-1 paths were run. No generated-input oracle for `keep_drawn`, `aim_nodes` or
  `orient_here` on their own.

## Next

F62a: port the idle layer (start from research/oracle.txt `# F62`; context/notes/fn_atv_004eb6f0.c is the
3 KB clip chooser it calls, and `./kit fn -p atv` on vt +0xd4 gives the layer itself).
