# 1 — wheel ray probe (FINAL)

Task S114. 2026-10-25.

## What I did

- `ride.rs`: the port's four-probe collision bundle is gone, and `belly_reach` and its
  `BELLY_MIN_REACH` with it. The wheels' swept contacts now come from the original's wheel ray probe
  (`wheel_rays`: one segment per wheel over the bike's attachment points, with its one-node tree,
  `ray_node`), searched through the bike's nearby search (`bike_search`: least 30, extra 50,
  capacity 256) in a `flight_search::Scene::wooded` over the world's props and plants
  (`wheel_hits`). The contacts go to `chassis::wheel_contacts` as before, but `select` is now the
  original's: the contact's segment's own `from.z` (front if > 0), not the contact point's distance
  forward of the centre of mass.
- A world that hands no props over `World::wooded` (the test worlds that only answer `solid`)
  sweeps the same two segments as a `collide::Bundle` instead.
- What is left of the body probes is one: the centre of mass along its velocity for the frame
  (`velocity_bundle`), the port's, resolved as before. The straight-down probe (S89's belly
  stand-in) is dropped.
- `node_distance` (`near`) is the smaller of the wheel record's nearest fraction and the body
  probe's.
- `flight_search::Scene::ray_record`, a getter for the ray probe's record.
- `tests/replay.rs`: the S89 test now asserts that the built probe is the image's bit for bit
  (`SaveState::wheel_probe`, `bits_of_rays`), and that the port's search at the image's pose finds
  what the image's record (`SaveState::wheel_record`) holds.
- `S89_LANES_AGREE_THROUGH`: along's front and rear `slip.locked` re-pinned 279 → 278 and
  277 → 276. Both read so on bare HEAD bf7ed904 (run in a scratch worktree, same data): S120's red,
  not this task's.

## What the original does

The bike's setup (`./kit fn 0x00253900`) builds a kind-2 ray probe at bike+0x7c8 through
`./kit fn 0x001eab40` with two segments: front wheel (+0x7b0) then rear (+0x7b4), each from the
mount (wheel+0xa0) raised by wheel+0x48 (the suspension's full compression, 1.0) to the mount
lowered by wheel+0x214 (the probe's reach, 1.2), in the chassis' frame. The tree builder
(`./kit fn 0x001115b0`, recursive `0x00111160`) makes one node for two segments; its box rule is
`./kit dis 0x0010f990` (in `oracle.txt` under bike_physics_lines).

Every substep `bike_wheel_contacts` (`./kit fn 0x00257820`) refreshes the probe from the node,
runs it against the bike's nearby search (+0x7bc) and, if the probe's hit bit is up, hands each
contact to the front wheel when its segment's `from.z` is above zero and to the rear otherwise. The
search list itself is gathered once a frame after the substep loop of `0x00257a00`, around the body
rays (+0x7c4).

From the s89-along RAM image: segments (0, 2.2332027, 2.5224998) → (0, 0.03320277, 2.5224998) and
(0, 2.1946993, −2.5897298) → (0, −0.0053005, −2.5897298); one node, volume 0x3dea5e29, radius
0x4032963f, children 0x8000 0x8001; the record holds two contacts, both on the bed's mesh, normal
(−0.0022749244, 0.9999973, −4.0e−7), fractions 0.78997 and 0.79671. first_recording's probe is the
same words (same bike).

## How it was proved

- **Save-state images.** `the_dallas_rail_rides_replay_from_their_save_states` (both S89 images,
  along and across): `ride::wheel_rays` off the port's own wheels (built from `race250a.phy`) is the
  image's probe in every word. The port's search at the image's end-of-tick pose finds the same
  number of contacts as the image's record (along 2, across 0), normals equal to the bit, points
  within 0.001 ft and fractions within 0.00003: the record is a substep older than the pose.
- **The S89 rides rerun**, every lane, before and after: the contact lanes part where they did
  (along 106/108, across 50/53), the surface lanes at 165/167 and 60/63, the spinning lanes at
  32 and 17. The whole run still parts at tick 2 on the chassis (`vel[0]` −0.0019420927 against the
  port's −0.0019415298; HEAD's was −0.0019416395), as it did before: the swap moved the port's
  value and not the tick.
- `./kit check --all` in s2 (which now holds S89's data): everything passes but
  `the_slip_cases_replay_from_the_save_state` ("turn: the path parts at 75, not after 82"), which
  fails the same way on bare HEAD bf7ed904 in a scratch worktree: main's S119. The lobby
  bike-set test flaked once and passed on the gate's own rerun.

## Not verified / not 1:1

- **The contact lanes did not move.** The brief expected the bed's edge (106/108, 50/53) to be the
  probes; with the original's probe the port still parts there, and at tick 101 along the port is
  already 0.8 ft across its heading where the original is 0.1 — the ride itself has drifted by then
  (the chassis parts at tick 2). What parts it at tick 2 is not found here.
- **The search list is gathered fresh each substep around the wheel probe**, not once a frame
  around the body rays as the original does: the same candidates within reach, in a different walk
  order. Nothing was seen to depend on it; not proved that nothing can (two props both reached by
  one segment would update its one contact in walk order).
- **The tree's split and partition are not ported**: two segments make one node with children
  0x8000 0x8001, as both images show. A bike with more segments has none.
- **The node's radius** is not kept: `flight_search::reach` takes the probe's reach off the half
  extent through the VU, the original's 0x0010f990 through the FPU accumulator. Same value here
  (2.7904203 both); not proved bit-equal in general.
- **Other bikes**: the wheel probe searches no rider's bike (`Scene::wooded` is handed none); the
  original's candidate list holds the bike frames (category 0x9a048) and its probe's mask
  (+0x84 0x18000) is not decoded, so whether its wheels can stand on another bike is not known.
- **Solid-only worlds** (`tests/ride.rs`'s `Yard`, the oracle worlds) sweep the two segments as a
  bundle with the port's sweep, not the ray probe's narrow phase.
- **The body probe that is left** (velocity sweep) is the port's; the original's body rays against
  props and the chassis' sweep back are not run by the ride (S87 runs the body rays against other
  bikes only).
- **`node_distance`** from the two records' nearest fractions is the port's mapping; the field the
  original reads it from is not traced.
- **The gate is not green**: S119's slip case reds it on main as it does here, so `./kit done`
  may refuse; the S119 lane itself was not looked at.
- **The locked lanes' re-pin** (278, 276) is HEAD's value; why HEAD moved from 279/277 is S120's.

## Next

- S123: the body rays against props, and the sweep back, in place of `velocity_bundle`.
- S124: what parts S89's along ride at tick 2.
