# 1 — wreck bones take the skeletons motion (FINAL)

Task S70. 2026-10-21.

## What I did

- `ragdoll_ride::Down::enter_from(kit, now, before, dt, …)`: the original's hand-over. Every bone
  is posed where its node is now (`now`). Its velocity and rates come from how far the node moved
  and turned since its matrix was last drawn (`before`, one step earlier), over the tick's `dt`,
  through the S52 pose (`ragdoll_pose::pose_all`, not still). No bone gets the bike's velocity.
  Without `before`, it keeps the old behaviour: still bones, then the bike's velocity.
  `enter_posed` and `enter` now go through it.
- `ragdoll_ride::node_of` (a riding part's node from its bone body: the quaternion normalised, and
  the node at the body's position less the cofg turned by it), `nodes` (the skeleton's nodes in the
  world, from `Posed` or the `Rig`, carried by the ride's transform) and `drawn` (each node's
  drawn matrix, `basis(at, forward, up)` as the pose builds it).
- `Rider::drawn`: the drawn matrices from the last riding tick. `frame_among` computes this tick's
  nodes (step 1c) from where the bike was left, before the physics. A wreck entered this tick poses
  from those nodes with the last tick's drawn matrices and `dt`. After the tick, while still riding
  and not put back, it stores this tick's nodes' drawn matrices. It is on the wire (Snapshot), so a
  rollback restores the same entry. It is not walked, like the skeleton it comes from. Sim size
  pin 9792 → 9816. platforms.rs's riders' wire bytes went 13115 → 13147 (8 bytes a rider: the empty
  vector's length). Every tick hash is unchanged.
- research/oracle.txt S70 block. Follow-ups S76 (run the riding ragdoll and sync the skeleton from
  it) and S77 (port the skeleton's node evaluation bit-exact).

## What the original does

The wreck entry (`./kit fn` on rider_body_wreck_enter) first calls the actor's pose slot
(rider_ragdoll_pose) with "not still" and the tick's frame time in f12. A hook on side-2604 → 2605
saw 0.03333, the capture's dt. Then it calls the reset. It never hands the bones the bike's velocity
there; only the throws of some reasons set velocities afterwards (already ported in
`Ragdoll::wreck_enter`).

The pose reads each part's node in world space and its two axes, and measures the move and turn
from the part's drawn matrix (part +0x60). While riding, **the skeleton is the riding ragdoll**.
After each world step, the riding node sync (see oracle.txt S70) normalises each body's quaternion
and sets its node to body.pos − turn(q, cofg). So on entry tick T the nodes are the bodies at the
end of T−1, and the drawn matrices are the nodes one step earlier (T−2, or the step before a tick
that did not move the bodies: tip's row 2473 repeats 2472, and the original's drawn matrix there is
2471's).

## How it was proved

- `tests/ragdoll_entry.rs::the_hand_over_is_the_originals` is a save-state capture: the original's
  own `rider_body_update` on side-2604 with 2605's world laid in, hooked. One pose, not still,
  dt == the tick's. The port's `pose_all` over what the original's pose read (its 18 nodes and
  axes, the drawn matrices, the RAM cofgs) gives the original's starts **bit for bit**: position,
  velocity, quaternion and rates, for all 18 bones. Those starts are still in side-2605's RAM after
  the tick, and no bone's velocity is the bike's.
- `tests/ragdoll_entry.rs::the_riding_bodies_hand_over` runs the port's own hand-over from captured
  bodies (`node_of` on row T−1 → `now`, `drawn(node_of(row T−2))` → `before`, the pose over the
  row's dt) against the starts the original left in each post-entry state. Worst differences over 18
  bones:
  - side 2605: position 0.00098, velocity 0.029, quaternion 3.8e-6, rates 0.098
  - back 2475: position 0.00098, velocity 0.029, quaternion 3.2e-6, rates 0.067
  - tip 2474: position 0.00098, velocity 0.037, quaternion 4.6e-6, rates 0.041

  Before S70 the bones started with the bike's velocity and no rates: side bone 0 got −5.8 in x,
  where the original has −27.2.
- `./kit check`: passed (1121 tests).

## Not verified / not 1:1

- **The port's ride still does not hand over the original's bones.** The rider does not run a
  riding ragdoll, so the nodes the shipped `Rider` uses are the shell's animated `Posed` (host
  float, one frame old) or the `Rig`'s fixed pose carried by the bike. They are not the original's
  riding bodies. The hand-over math is proved; its inputs in the shipped sim are not. Follow-up S76.
  So the task's "the bones' positions, orientations and velocities on the wreck tick must be the
  original's" is met only when the inputs are the original's (both tests), not in a port ride.
- `node_of` / `drawn` are not bit-exact. The node positions are 0–4 ulps off the original's
  transform_set_pose / node_world_position, and the axes and drawn rows tens of ulps off. So the
  test from the captured bodies is within NEAR, not exact. The skeleton's node evaluation is not
  ported, and the writer of the part's +0x60 each riding tick was not found. Follow-up S77.
- Front (2600) is not compared. Its row 2599 is a torn copy of 2598, and the original's nodes had
  moved, so the capture lacks the tick the entry posed from. Its RAM was not re-captured.
- The rule "the drawn matrix is the last step that moved" rests on one case (tip's repeated
  row 2473). Whether the original skips the node sync or the step on such a tick is not known. The
  port steps every tick and always keeps the previous tick's matrices.
- In the port, a wreck on the first riding tick after a put-back, or on a rider without a kit or
  with a node count other than the kit's parts, has no history. It falls back to still bones plus
  the bike's velocity, the port's own behaviour.
- `drawn` is on the wire but not in the state walk, so two peers whose `drawn` differ hash alike
  until a wreck. It is derived from the walked ride and the unwalked skeleton input, as S65's
  skeleton is.
- The full entry after the pose (reset, wreck_enter's throws, the first step) was not compared
  against the original for the hand-over inputs. S56/S61's wreck tests cover that path from the
  original's own bones.

## Next

Nothing for S70. S76, then S77.
