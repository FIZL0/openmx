# 1 — the wreck entered from a posed body (FINAL)

Task S262. 2026-11-03. Follow-up to S257 (journal 2026-11-02-s257-trick-ragdoll-transitions).

## What the original does

recording2's trick at 3522 (gesture state 0) is bailed: 3562 is posed (mode 3, unplaced), 3563 wrecked
(mode 2, placed). The wreck entry's moving pose starts every body (`body+0x30` position, `+0x40`
velocity, in S257's parts capture of recording2) from:

- the bone: the node as the last posed tick (E−1 = 3562) left it, the clip under that tick's bike;
- the drawn matrix: the node of the tick before (E−2 = 3561). `part+0x60` read at E−1 is that same
  matrix (S257: through a posed run `part+0x90` is the previous tick's node);
- the entry tick's dt.

Bit for bit on all 18 parts, positions and velocities. Candidates that are not it: the drawn matrix
read at E (43 ft/s off), the entry tick's own nodes (2 ft, 64 ft/s off).

## The port

`rider.rs`' wreck entry: the riding body is unplaced on a posed tick, so `Riding::hand_over` is `None`
and it falls back to `ragdoll_ride::nodes(kit, skeleton, &ride.transform())` (the skeleton as last
tick's `Body::ride` posed it, on the bike as last tick left it) against `drawn_before` (the same nodes
one tick earlier, kept on every unwrecked tick, posed ones too). That is the original's choice. No
behaviour change was needed, only exposing the pose:

- `Down::entry_starts` (new, `ragdoll_ride.rs`): `enter_from`'s pose, split out unchanged so a test can
  run it. `enter_from` calls it.

## How it was proved

`crates/game-sim/tests/trick_ragdoll.rs`
`a_bailed_trick_enters_the_wreck_from_the_clip_as_the_originals`:

- steps a fresh `Riding` (the S257 per-tick feed, now `step_row`) from 30 ticks before 3522 to 3562;
  mode and placement are the original's every tick, and `hand_over` is `None` at the end;
- the port's fallback (`ragdoll_ride::nodes` + `ragdoll_ride::drawn` + `Down::entry_starts`) from the
  original's nodes: starts equal bit for bit;
- from the skeleton composed off the captured joints under the bike node: 0.00098 ft, 0.037 ft/s
  (the test's composition glue);
- read a tick late (E's nodes against E−1's), it is > 0.5 ft, > 10 ft/s off: the test tells ticks apart.

`./kit check -- -p game-sim --no-fail-fast`: all pass but `fly_and_collide`'s drawn lean (B18) and
`the_humans_tricks` (C6's capture), both known red.

## Not verified

- The start orientation and rates (`+0x160`, `+0x140`) were not captured; only position and velocity.
- What the reason's `wreck_enter` does to the bodies after the pose, and the wreck's first step, are
  not compared here (S70/S110 cover them from a riding body).
- The port's own skeleton (`rider.skeleton` from the vehicle animation) is not the captured one; the
  composed skeleton stands in for it, as in S257.
- Only this one bailed trick exists in the capture.
