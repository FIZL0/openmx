# 1 — windmill body fling (FINAL)

Task S167. 2026-10-28.

## What I did

- Found what updates the windmill's kinematic body: it is a scene node and the last child of the group
  under the rustic windmill toy. The toy's update ends by updating its children, after its visibility
  gate. So the body moves only on ticks the toy steps (drawn, V76), and in S145's states it never had.
- Wrote research/s167_windmill.py. It loads a free09 state in copy 1 that holds the body (slot 5),
  pauses, teleports the bike as s76_tele does, resumes with no input, and logs the vehicle and the
  body's matrices per frame. It can also dump RAM before and after. The csvs are in s1's
  context/capture/s167 (git-ignored).
- The original **does** fling the bike:
  - **apex** (at rest 4 ft over the top, 2439.25,176,4059.9): the bike lands on the 2 ft top and wrecks
    5 ticks later. The vehicle's position then moves ~1967 ft a tick (~59,000 ft/s) out to x ≈ 112,000,
    until the put-back.
  - **south** (beside the south face, 2439,140,4067.5; three runs, all the same): the first contact
    throws the bike from rest to (7.8, 18.6, 133.4) ft/s, away from the wall, and it wrecks.
  - In neither run did the body update before the hit. In the south run it updates from 4 ticks later,
    once the wreck camera sees the windmill.
  - The control on open ground (2439,137,4085) just rolls downhill under 23 ft/s.
  - The RAM diff shows contact records naming the body as their mover after the south hit.
- So I ported the bogus value. In crates/game-sim/src/toys.rs, `Toys::bodies` counts each toy's steps
  (0, 1, 2+). Before a toy's second step, its body's "before" matrix is the identity, as in the
  original, so the mesh mover (`Moving::of(before, after, 1/dt)`) answers the node position × 30 and a
  turn.
  - Kind-0 `.col` meshes are now `Some` movers. They were `None`; in the original, their owner is that
    body.
  - Hierarchy parts take the same identity "before" until the second step.
- Added the count to the hash (`toy_bodies`, walked only on a track with toys) and to the snapshot wire.
  The hash.rs size pin went from 11776 to 11800.
- game-render/tests/toy_parts.rs (S133's free10/nat11 part replays) now seeds `Toys::bodies` from the
  state's RAM: 0, 1 or 2 by whether the first part body's now/before matrices are still the identity.
  Those states' bodies had already stepped twice, so without the seed the port answered the fling on
  tick 0.
- The gate's lobby plane drop-in failure ("the guest never dropped in") was a load flake: the load
  average was ~30 during the gate, and the test passes when run alone. That Sim has no toys.
- Nothing from opentennis applied.

## What the original does

- **Body update** (`./kit fn 0x001dd9b0`): +0x2e0 = +0x2a0, then +0x2a0 = the node's world matrix.
- **Who calls it:** the group node's update (`./kit fn 0x00149270`) ends in the children update
  (`./kit fn 0x00133500`), and the animated toy update (`./kit fn 0x002150f0`) calls that on its own
  children last, past the visibility gate.
- **Velocity** (`./kit fn 0x001dd7b0`): (node translation − before translation) × 1/dt.
- **Spin** (`./kit fn 0x001dd850` → `./kit fn 0x001dd8c0`): cross(before row 2, now row 2) +
  cross(before row 1, now row 1), × 1/dt, turned by the node.
- **Result:** with an identity "before", the velocity is the position × 30 (~120,000 ft/s) and the spin
  is the placement's turn × 30. The body is a contact's mover, so the bike takes that platform motion
  when it touches.

## How it was proved

- **Function oracle:** game-sim/tests/toys.rs `the_windmills_body_answers_the_originals_motion`.
  - It runs the original's animated toy update in the interpreter on first_recording's RAM over a
    10-tick drawn/not-drawn schedule.
  - On each tick it asks the body's velocity and spin virtuals and compares them bit for bit with the
    port's mesh `Moving` from `Toys::frame`.
  - 5 ticks are flung, (73177.47, 4010.912, 121798.58) and spin y 1.6167299, and 5 are still.
- **Live captures** (research/s167_windmill.py, the runs above): these show that the fling happens in
  the original. They are not compared with the port; see below.

## Not verified / not 1:1

- **No ride-level replay of the fling through the port.** The replay worlds (`in_the_world_of`) carry
  no toys. Also, the port's wheel contacts do not take a mover's motion yet (S168 is open), so a port
  bike cannot be thrown by the windmill through its wheels yet. What the port has is the mover's value,
  proved bit-exact against the original's virtuals, not the bike's resulting motion.
- **The 59,000 ft/s flight after the apex wreck and the 300.45 / 100.312 ft/s throw are not analysed or
  ported.** The throw appears in both windmill runs around the put-back. The flight is the wreck's
  vehicle position. Neither was traced to its code.
- **A hierarchy part that is not stepped** keeps, in the original, the motion from its body's last step
  (the body is not updated, so velocity reads the same stale pair). The port gives zero motion after the
  second step on a tick the toy does not step. Only the mesh body was measured.
- **Only free09's rustic windmill was measured.** Kind-0 meshes on other tracks' toys now answer a mover
  too. I did not check whether their bodies are kinematic bodies with the same update path.
- **The shell's `seen` is not the original's gate.** The original's gate is the V76 view gate;
  toys.rs's `steps` rule stands in for it. When the body first steps in a ride therefore follows the
  port's visibility, not the original's camera.
- **The flag bit 3 at toy+0x325 is not modelled.** It also gates the update; it was clear in every
  state.

## Next

- Follow-ups filed: S206, a toy-carrying replay of the south and apex captures once S168 lets wheels take a
  mover's motion, and S207, the stale motion of a hierarchy part that is not stepped.
