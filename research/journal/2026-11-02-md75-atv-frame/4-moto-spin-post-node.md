# The quad's Moto, part 3: spin air, vehicle post, node position (MD75b1)

2026-11-03. Task: `./kit plan MD75b1`.

## What the original does

Read with `./kit dis -p atv`. The field maps are in research/oracle.txt `# MD75b1`.

- **Spin air** (atv_bike_spin_air, 0x004e53d0, dt in f21) is the 360. It first clears the applied word
  (+0x11d0). After that it takes one of two branches, on the active word (+0x11d8):
  - **Not active.** It checks for a lip, as the backflip does: on the ground, no backflip applied or active,
    body velocity y > 5, contact speed > 30, and brake == 0 (c.eq.s). Off a lip, it clears active and armed.
    On a lip:
    - **Fire.** If the 360 is armed, |stick x| > 0.7, and the stick is on dir's own side (`x < 0 ? -1 : 1`),
      it fires:
      - yaw = `dir·T(yaw pair)` and pitch = `−T(pitch pair)`. T is the low torque when the time range is
        empty. Otherwise T is the |flick| inlined map and clamp (`(t − t0)/(t1 − t0)`, adda(0, lo) madd,
        then max(min) and min(max));
      - then armed = 0, active = 1, grace = 0.9.
    - **Arm.** Otherwise, with R1 alone (nibble 4), stick < −0.7 arms dir 1 and stick > 0.7 arms dir −1.
      So the stick goes one way to arm and is flicked back the other way to fire.
  - **Active.** The node's world rows z and y come from atv_node_axes (0x00194960), which brings the world
    frame up to date first.
    - **Not yet armed.** On the ground, the grace runs down (`grace − dt`) and the 360 ends at ≤ 0. In the
      air it arms and keeps the take-off heading.
    - **Armed.** The heading is the node's forward axis with y = 0, VU-normalised, z clamped twice. It is
      looked up in the acos table as `mula(frac, hi) madd(1 − frac, lo)`, then `(π − a) + π` for x < 0.
      off = `min(|h − now|, |(2π + h) − now|)`.
      - **Ends** on the ground, on record +0x34, or when |moment y| < the low yaw torque. Then active and
        armed are cleared, and a stunt id ≥ 41 becomes 255. If it was mid-clip, the clip is set to 0 (vt
        +0x540, atv_bike_set_spin_clip) and the wreck is called (vt +0x368) with 15 if
        atv_bike_flip_wreck_steep, else 11.
      - **Otherwise.** With no trick button and the stick held against dir, the moment decays: y by
        `rate·(dir·|x|)` and x by `x·rate` (adda(0, m) msub dt). Then:
        - the moment goes into the body (vt +0x78, xyz add);
        - applied and the ramp (+0x11d4) are set to 1;
        - the clip changes. At clip 0, it starts when off > 35° (0x3f1c61a9): the stunt starts (vt +0x4b8,
          id +0x13c0 for dir > 0, else +0x13bc) and the clip goes to 1 if the start answers yes. At clip 2
          with off < 35°, the stunt ends (vt +0x4ac) and the clip goes to 0. At clip 1 with off ≥ 35°, the
          clip goes to 2.
  - **Tail** (both branches). If the ramp is on and the quad is on the ground, the ramp is cleared and
    authority = authority time.
- **Vehicle post** (atv_vehicle_post, 0x004940b0, through the thunk at 0x004a4f00). With the body off
  (body +0) it does nothing. Otherwise:
  - origin = position − `q c q*` (c is the body's cofg +0x1a0; two vopmula/vmadd/vopmsub rounds).
  - It tests the origin against the world (vt +0x120, atv_vehicle_in_world). With flag bit 0, the test is
    the world's box (+0xa8; none means inside) in x and z. With bit 6, the test is y not under the floor
    (+0x98). Otherwise the origin counts as inside.
  - Inside: atv_node_orientation(node, 0, orientation, origin).
  - Outside: vt +0x13c (body reset), then vt +0xe8 (respawn).
  - Then, in order: vt +0xe4, atv_node_bounds(node) if flag bit 1 is set, vt +0x124, and flag bit 2 is
    cleared.
- **Node orientation** (0x00194650):
  - M = atv_quat_frame(q, position). That is `q2 = q + q`, rows `q·q2.x/y/z` with w = 0, the off-diagonals
    ± the lanes of `q·q2.w`, the diagonals `0 − ((a + b) − 1)`, and position row w = 1.
  - With a parent, the parent is brought up to date, and M is multiplied by the inverse of its world frame.
    The inverse is the 3×3 transposed, with each moved lane written as `0 + x`, and position
    `0 − x·r0 − y·r1 − z·r2`.
  - Then atv_node_moved runs. If the node was current, it sets bit 7 on itself and every parent, clears
    its bit 0, and clears bit 0 down its children's sibling lists, going into a child only if that child
    was current.
- **Node position** (atv_node_position, 0x00194c20): atv_node_world_update, then the world position row
  with w = 1.0.
- **World update** (0x00193b30): it climbs the stale parents up to a current one or the root, then works
  down. Each node's world frame is `local·parent world`, or the root's own frame, and is marked current.

## The port

- `atv_moto.rs` gains:
  - `Spin`, the 360's words;
  - `Node`/`Scene`, a four-node graph whose node 0 is the quad's;
  - `Bounds`, the floor and box;
  - all three in `Parts`.
- Its functions are `spin_air`, `vehicle_post`, `quat_frame` and `Scene::{update, place, position, axes}`.
- `quad::Call` gains the vehicle post's five callees: `BodyReset`, `Respawn`, `SpatialUpdate`,
  `NodeBounds` and `CollisionSync`.
- `quad::Steps` now answers three calls:
  - `SpinAir`, whose steepness is asked as the backflip asks it;
  - `VehiclePost`, which passes the callees to `rest` in order and then clears bit 2;
  - `NodePosition`, which writes `Quad::node_pos`.

## Proof

New tests in `tests/atv_quad.rs`. Each lays out the quad's node in a graph at 0x01c20000 and, in the
original, runs the listener with the picked call unhooked. The graph has the node alone, under a parent or
two, with a child, a child's child, or a sibling, and each node is current or not. The test compares every
region, now including the scene, and the call log against the port's `quad::Steps`:

- `0x0075_b101` spin air, 20,000 states under the ground step. Every path from the decompile was seen at
  least 30 times: off, off the lip, fired, fired at the low torque, armed left and right, waiting, took
  off, grace out, in the grace, ended, ended mid-clip, spinning, spinning slowed, stunt id cleared, stunt
  started and ended, and wrecked.
- `b102` vehicle post, 10,000 under the post-process: body off, boxed, no box, floored, anywhere, placed
  under a parent or alone, was current or out of date, no children, a child, two children, a child's
  child, reset, and bounds.
- `b103` node position, 10,000: current, its own frame, under a current parent, under a root, under a
  current grandparent, and under two.
- `b104` both PostProcess calls at once, 5,000.

`each_unhooked` now hands off to `each_unhooked_in`, which also runs the post-process (`Listener::Post`).
It hooks the PostProcess's two calls and the vehicle post's five callees. The wreck tally also counts
"wrecked steep".

`cargo test --release -p game-sim --test atv_quad -- --nocapture` passed all 49 tests, with none skipped.
`./kit check -- -p game-sim --test atv_quad` also passed.

Mutation checks, each with one change to the port:

- Caught: the origin's sign, the world multiply's order, the chain not stopping at a current parent, stale
  bounds only on the node, the out-of-date walk not going into children (caught only once a child's child
  was generated), the pitch's sign, and the clip not passing to 2.
- Not caught:
  - swapping the acos lerp's mula/madd pair. This is equivalent: the same two rounded products are added;
  - `!(x <= lo)` versus `x > lo`. These differ only on NaN;
  - dropping the inverse's `0 + x`. This differs only on −0 and denormals, which the generated frames
    never hold.

## Not verified

- The steepness test's 15 is reached only through the ride's own touching/struck class. A touch the tweak
  adds is always on a non-struck class, to keep the record consistent.
- The start-stunt hook always answers yes, so a refused start (clip left at 0) is never run.
- No frame holds −0 or denormal lanes, so the inverse's `0 + x` is not exercised.
- The scene graph is four nodes. Deeper chains and longer sibling lists run the same loops but are not
  generated.
- The openmx shell does not build at this branch's base (`Blades::new`'s fifth argument, P265/S271). Main
  already fixes it (P273), so nothing was filed.
