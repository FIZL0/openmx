# 1 — ground reset in rust (FINAL)

Task F31b5e. 2026-11-05.

## What I did

- `crates/game-sim/src/handling/wreck.rs`: the put-back's ground reset is now Rust. `ground` does the
  unhooks, the body's staged velocity, the orient, the rehang and the parts' on/attach. `orient` crosses
  and normalizes the axes and makes the quaternion. It then calls the body's position, orientation and
  commit, sets and aims the node, and runs the spatial update and the part syncs. The body's virtuals are
  ported as `body_position`, `body_velocity`, `body_orientation` and `body_commit`, and the VU quaternion
  turn as `turned`. New `Out` variants cover every call the reset makes.
- `crates/game-sim/src/atv_moto.rs`: added `Scene::aim`, the node's aim, which builds a basis at the
  node's own position (and handles a parent's inverse).
- `crates/game-sim/tests/mxvatv_bike_ride.rs`:
  - `WreckInRam` serves `Ground` as `wreck::ground`. Each body or bike virtual the reset calls is checked
    to be the function the port stands for.
  - The node calls go through `on_scene`. The spatial update, node bounds and part syncs go through
    `quad::Steps` (`wreck_steps`). A database move is handed to the original's spatial move.
  - The flip census is now `the_flips_tail_callees_leave_the_physics_alone`. It asserts that no callee
    writes the physics blocks. "wreck Ground" is dropped from `FLIP_CALLEES`.
  - New test: `the_ground_reset_is_the_originals`.
- research/oracle.txt `# F31b5e`: the reset's field map.
- Taken from existing code: `math::basis` and `mat4_to_quat` (MXU's port), `air::cross`/`normalize`,
  `atv_moto::Scene` (`set_position`, `update`, `moved`, `frame_mul`, `frame_inverse`), and `quad::Steps`
  for the spatial update, node bounds and part sync (MD75c/MD75c2). Nothing came from opentennis.

## What the original does

See research/oracle.txt `# F31b5e` (recipes: `./kit fn --program atv <addr>`).

The ground reset takes these steps:

1. It unhooks the node from the scene if byte `+0x45` bit 1 is set.
2. It unhooks the frame, riding and active parts' database entries.
3. It zeroes the body's staged velocity (vt `+0x14`).
4. It orients the bike (vt `+0xf0`) to `+0x50`, `+0x60` and `+0x70`.
5. With bit 1 set, it rehangs the node.
6. With `+0x44` bit 0 set, it turns each part on and attaches it.

The orient takes these steps:

1. It computes up = (a with xyz a×b)×a, keeping a.w through the opmsub.
2. It normalizes both axes and sets at.w to 1.0.
3. It makes a quaternion from the basis.
4. It calls the body's staged position (vt `+0x10`), its orientation and spin (vt `+0x58`, from the
   origin), and the commit (vt `+0x3c`).
5. The commit copies the staged values in and zeroes the rest. It then moves `+0x70` by the rotated
   centre offset `+0x1a0`.
6. It sets the node's position to the body's `+0x70` less that offset, aims the node, runs the bike's
   spatial update, then the node bounds (bit 1) and the probe places (bit 0).

## How it was proved

- **Replay**: `flip_wrecks_and_is_put_back_bit_exact…` and the census replay flip-1 (C15). All 465
  rows are bit-exact with the reset in Rust. The census shows no interpreter callee writing the bike or
  its six physics blocks; the unhooks, part on and attach run on row 193 and change none of them. All 41
  tests in `mxvatv_bike_ride` pass.
- **Function oracle**: `the_ground_reset_is_the_originals` runs 10,000 draws on state 4. Each draw sets
  random `+0x50`/`+0x60`/`+0x70` (every lane, axes not parallel), a random body `+0x1a0`, a random
  `+0x44` bit 0, and the riding part present or not. The original's `0x00492ca0` runs in the
  interpreter, and `wreck::ground` runs as `WreckInRam` serves it. All of RAM (outside the stack) and the
  scratchpad must be identical. Each path is taken ≥1000 times.
- **Mutation check**: poking one body word after the port was caught by the test, so the comparison can
  fail.

## Not verified / not 1:1

- **The hung node** (`+0x45` bit 1): the node unhook, the rehang and the node bounds in the reset. The
  bit is clear on state 4 and is cleared by the put-back before the reset on C15. On state 4 the
  original's unhook of an unhung node reads past RAM, so the oracle cannot draw it. The ported order
  follows the decompile, but no run reaches it. The rehang and unhook are still the original's (passed
  out), and the node bounds is `quad::Steps`' (proved under MD75c).
- **`Scene::aim` with a parent**: the bike's node has no parent on state 4 or C15. Only the no-parent
  branch is proved (by the replay and the oracle).
- **The spatial update and the part syncs with a database entry**: bike `+8` is empty on state 4 and C15,
  and the parts are unhooked when synced, so no `SpatialMove`/`PartMove` is ever produced. `wreck_steps`
  would panic on a `SpatialMove`. The box maths is `quad::Steps`/`atv_sync`'s, proved in their own tasks.
- **Still the original's** (in the interpreter): the unhooks, part on, attach and rehang. They are
  outside this task.
- **Census coverage**: "no physics writer" covers only the bike `+0..+0x1800` and the six physics blocks
  the census watches.

## Next

Nothing for this task.
