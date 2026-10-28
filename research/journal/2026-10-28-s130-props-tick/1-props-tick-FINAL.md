# 1 — props tick (FINAL)

Task S130. 2026-10-28.

## What I did

- `crates/game-sim/src/prop_world.rs` (new): a knocked prop's tick on top of S80's loader.
  - `Collection::new` builds the world from a loaded `Pco`, its parts and node frames. Each part
    becomes an `Object` (place/look/up from the node's world rows, gravity, ResetTime, the six
    rest thresholds, DetectCollisionsWhenDisabled filed). Pins and PinToBody become `ragdoll::Pin`
    joints, with limits from the curve sine at half angles and only when min <= max in every lane.
    A BodyTorsionSpring comes back as `Unmade::TorsionSpring`.
  - `activate`, `enable`, `disable` and `tick(dt, tables)`: the world steps at
    SimulationHz/NumberOfIterations while active. Then the node's listeners run: the collection's
    settle (deactivates once every body sleeps, or counts the collection ResetTime down and puts
    the bodies back) and each object's own countdown, put-back and removal.
  - Post writes the body pose to the node (origin = position − turn(cofg)) or runs the off-the-map
    edge check. Preforce zeroes the moment, force adds gravity and clears the fresh flag.
  - `node_bounds(&BoundsNode)`: the node box merged over the children's boxes through the
    parent's rigid inverse.
- `crates/game-sim/tests/prop_world.rs` (new): the two proofs below.
- research/oracle.txt: S130 block after S80's.

## What the original does

The collection's update steps its own dynamics world, then walks the node's children. Waking a
collection (`./kit dis 0x3b27a0 0x3b2860`) sets its active bit and files it as a listener on the
node. With PinToBody it also enables every body and sets the countdown to -1. The bodies use the
same Dynamics shape as the biplane: preforce, force, integrate, the pins' solve (the prop pin's
vtable slot is the ragdoll pin_solve, so `ragdoll::Pin` is reused), then post.

At rest means every one of velocity, rates, accel, angular accel, shock and twist is under its
threshold, and a freshly woken body is never at rest. Once all of a collection's bodies rest,
ResetTime < 0 just deactivates the collection. Otherwise the countdown runs, and when it is up every
body is put back through `place` (side = look × up, up re-crossed, rsqrt normalised) and the
collection deactivates. Each object's own listener does the same with its own ResetTime. Sign_ on
free09 has 5 s, so it goes back 150 ticks after sleeping.

node_bounds starts from the node's own corners (.xyz, so w stays the centre's). Each boxed
descendant goes through child rows × the parent's rigid inverse, giving a centre and the span
|M0 h.x| + |M1 h.y| + |M2 h.z|. These are merged by vmini/vmax over xyzw, where two zeros order -0
below +0. The final centre and half are .xyz ops, so centre.w = lo.w and half.w = hi.w (the one
lane that kept the crow tree red until both were matched).

## How it was proved

Function oracle on the first_recording state in the EE interpreter (`tests/prop_world.rs`):

- `a_knocked_sign_swings_sleeps_counts_down_and_is_put_back`: free09's first `sign_` collection
  (Sign_SpeedLmt) is built by the port from the disc .pco and the RAM node frame. The loader
  fields, placement and pin are compared to RAM. The sign is woken (collection activate, then
  wrapper enable) and spun (rates 2.5, 0, -3). Then, every 1/30 s tick, the original's update
  and node listeners run against `Collection::tick`. Body, node rows, both countdowns, active
  bit, fresh flag and listener list are bit-exact on every tick through the whole lifecycle:
  limits hit on 12 frames, asleep at 13, put back and collection inactive at 164.
- `node_bounds_merges_the_childrens_boxes`: all 66 scene nodes with children in the state
  (crows, bike.psg, airider.psg, …; each has at least one boxed child), the original's
  node_bounds vs `node_bounds`, bit-exact including w lanes.

## Not verified / not 1:1

- **BodyTorsionSpring not ported.** No free09 prop has one (only the supercross banner .pco). The
  reader and force are recorded in oracle.txt; `Collection::new` reports it as `Unmade`.
- **PinToBody is built but unproven.** No free09 collection uses one. The two-body pin and the
  "enable all, countdown -1" activate path have only been read, never run against the original.
- **Contacts are left out.** The wrapper's collision object force (its vt +0x80 call) and the
  collide pass are not modelled, so a prop never touches the ground, bike or rider here. That is
  S131's job. The swing proof works because the sign hangs on its pin and never reaches the ground.
- **DetectCollisionsWhenDisabled** is only read and filed. The probes it keeps alive are part of
  the contact work above.
- **Not a captured strike.** The wake (activate + enable + chosen spin rates) is the test's
  choice, not a recorded knock off a live PCSX2 run. A real knock wakes through the struck listener
  (0x15ad40 → vt +0x124) with a contact's impulse. The tick after waking is the original's code on
  the original's state, but the kick is invented.
- **The edge check** (outside the bounds → disable + reset) is ported but never exercised: the
  sign stays inside.
- **Pin world point** is only proven for PinPosition 0 (the sign's).
- **The collection ResetTime countdown** path (collection +0x33c ≥ 0) is not exercised, because
  free09's collections all say -1. The object-level countdown is what put the sign back.
- **Pose w lanes.** The look/up w lanes of `Object` are not compared, and body accel.w is compared
  xyz only because it is a stale VU register (see `Body::integrate`).
- **node_bounds with no boxes at all** (any = false all the way down) answers vf0-based zeros by
  reading; every node in the state has a boxed child, so that path is unproven.

## Next

Done. Follow-ups: S194 (torsion spring + banner proof, PinToBody and collection countdown runs), S195 (a captured real knock, edge reset). S131 already covers contacts, hashing, drawing and the node box from the model.
