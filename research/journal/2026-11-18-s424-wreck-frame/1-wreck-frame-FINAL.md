# 1 — wreck frame (FINAL)

Task S424. 2026-11-18.

## What I did

- `crates/game-sim/src/ragdoll_ride.rs`: the wreck's frame is now the wreck bike's node, rows and
  origin (`wreck_frame`). Before, it was the body's current quaternion with the node's origin.
- That frame feeds, in `Down::enter_from` and `Down::step_posed_among`:
  - `held_at` (the hands' points in the bike's frame, taken at the entry);
  - the bike sphere's centre, the bike's tree key and the wreck bike scene;
  - the carrier's rows and the hold targets.
- The carrier's centre and quaternion are now the node's origin and `request_quat` (the request's
  `+0x180`, `+0x190`). The wreck step does not read either (only the riding pose does), so this
  changes nothing; it just keeps the carrier equal to the request.
- At the wreck's entry the old frame took `crash.origin`, which is still the riding bike's origin
  there (`Crash::begin`); later ticks took the node's. Now both use the node, so `held_at` is taken
  and read back in the same frame.
- New test `tests/ragdoll_wrecks.rs` `the_wrecks_frame_is_the_wreck_bike_nodes`.
- Nothing from opentennis: it has no wreck bike.

## What the original does

- While wrecked, the body's request is filled from the wreck bike's node (S65, S111):
  - `+0x10..+0x30` the node's rows;
  - `+0x180` the node's origin;
  - `+0x190` `mat4_to_quat` of the node's matrix (R40).
- The bike collidable (request `+0x1ac`) keeps the same matrix at its `+0x120..+0x150`. Its sphere
  centre and its place in the track's tree come from that.
- The request's `+0x50` matrix is not the node (0 of 362 rows match). The port's wreck path does not
  use it.

## How it was proved

- Save-state captures: S62's five wrecks (`front`, `side`, `back`, `tip`, `front3`;
  `kept-context/s5/capture/s62`). Each row holds the wreck bike's body (`bike_a`) and the request.
- Over all 362 wreck rows (`a_mode` 2), the port's `wreck_bike::node_of` of the captured body is,
  bit for bit:
  - the collidable's frame;
  - the request's three rows;
  - the request's centre (the node's origin);
  - and through `mat4_to_quat`, the request's quaternion.
- The old frame (the body's quaternion, the node's origin) matched the collidable's frame or the
  request's rows on 0 of 362 rows.
- The s62 captures are not in the shared `context/`. I ran the test with `KIT_ROOT` pointing at a
  scratch copy of `kit.toml` whose context adds them as links. In the shared gate the test skips,
  like the other s62 tests.

## Not verified / not 1:1

- **No replay of the port's whole wreck step against the original's bones uses this frame.** The
  S56/S62 replays drive the port's actor from the captured request, not through `Down`. So the
  proof is that the frame's inputs are the original's, not that the port's bones follow the
  original with them.
- **`held_at` itself is still the port's own idea.** The original's hand targets are the request's
  hand points (its `+0xb0`, the bike's hand dummies through the node), not a point fixed at the
  entry. Now both sides use the same frame, but a hand's target is only as right as the point fixed
  at the entry. Not compared with the captured hold targets.
- **The entry tick's frame was not checked on its own.** The port enters right after
  `Crash::begin`, before the wreck bike has stepped. There its node rows are the place's basis and
  its origin `node_of` of the new body. The 362 rows start a tick after each S62 state, which is at or after
  the entry, so they show the node is the request through the wreck, not on the entry tick. The
  port's node at the entry was not compared with the original's.
- **B15's south3 capture** is still lost (C37). It was not used.
- **The gate does not run the new test.** The s62 captures are only in `kept-context/s5`, so it
  skips there, like the other s62 tests.

## Next

- Done.
