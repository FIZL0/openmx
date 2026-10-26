# 1 — riding wreck entry (FINAL)

Task S110. 2026-10-26.

## What I did

- `ragdoll_riding::Riding` moved from the shell (`main.rs`'s `Rig::riding`) into the rider:
  `Rider::riding: Option<Box<Riding>>`, stepped first thing in `Rider::frame_bumped`
  (`Rider::riding_tick`) for the tick the shell last drew, while the rider is up and has a ragdoll
  kit and **holds**. Walked by the checksum (`riding` group, only while there is one) and carried by
  the snapshot wire (`wire!` for `Riding`, `ragdoll_body::Actor`, `Look`, `Model`, `Cut`).
- `Rider::holds: Option<Arc<Holds>>` is the shell's input: the chassis model's root node and the
  four grips (hands, feet) in the chassis model frame (`ragdoll_riding::Holds`). It is not state:
  not walked, not on the wire (`decode` keeps the local sim's). The shell sets it each frame after
  `chassis.frame` (`ride_holds`), only offline (`online.is_none()`) and only with a ragdoll kit.
  Fixtures, replays and online sessions have no holds, so no riding ragdoll and the old entry: their
  hashes are unchanged (the riders' wire grows a `None` byte per rider: 13371 → 13375).
- `Riding::ride` builds the request from the rider (ride transform, packed pose, holds through the
  ride transform, posed-off-the-table flag) and steps the actor; it keeps the drawn history (`was`:
  the bodies' node matrices before the step) that the entry measures velocities from.
  `Riding::hand_over` gives the bones (`node_of` per body) and that history.
- At the verdict the entry is `Down::enter_from(riding.hand_over())` when there is a placed riding
  body, else the old one off the animated skeleton (`ragdoll_ride::nodes` + `drawn_before`). The riding
  body is dropped on a wreck or a put-back; a fresh one is placed off the pose next time up, or when
  the bike has gone more than `STRAY` (10 ft) from the main body (a teleport, a reset).
- **Fixed**: `Outside::pose`'s `carried` (the original's `a1`) was inverted in Riding's use: the
  original's `rider_ragdoll_pose` posts still bones when `a1 != 0` (the repose passes 1) and moving
  bones (velocity, spin from the drawn history) when `a1 == 0` (the wreck entry passes 0)
  (`./kit dis 0x0030f0c0`: `addiu $20,$zero,1` on the `dt <= 0` path, the oracle's "unless a1 or
  dt <= 0").
- Drawing is unchanged: the riding ragdoll is drawn only with `OPENMX_RIDING_RAGDOLL=1`
  (`riding_ragdoll_on`, `riding_ragdoll_is_off_by_default` kept), now read off
  `seat.sim.rider.riding`. Off by default it is still stepped (for the entry), just not drawn.

## What the original does

While riding, the body actor's skeleton is the riding ragdoll (S70's note): the wreck entry
(`./kit fn 0x0024ad90`) poses the bones off the nodes the riding step left (`rider_ragdoll_pose`
with `a1 = 0`: velocity = move since part `+0x90`, spin from part `+0x60`), resets them, throws
(five draws, `./kit fn 0x0024ad90` around `0x24bc5c..0x24bda0`), and its last call picks the
wreck's sound (`wreck_sound`, one draw through `0x001b5820` when the list has two or more).

run2's wreck (S109, `context/capture/s109/run2.csv`):

- row 3716 has the verdict (`wreck_reason` 0 → 11, off the back) and **no** step at all: its bones
  and the bike's transform repeat row 3715;
- row 3717 is one riding tick (`a_mode` 1);
- row 3718's frame ran two: a riding tick no row holds (its bodies are row 3718's `last_pos`), then
  the entry and the first wreck step (`a_mode` 2). The bike stepped twice in that frame too, so the
  hidden riding tick rode on a request between rows 3717's and 3718's. Laid in from 3718's, that
  tick enters the wreck itself.

## How it was proved

- `tests/ragdoll_riding.rs` `the_wreck_enters_from_the_riding_bodies` (run2, the S109 state image):
  the original is carried from the state's frame 1938 to the verdict through run2's rows (1673
  ticks, each laid in from the row before; a row holding two updates runs two) so its actor's own
  state is its own; then the two riding ticks with the port's `Riding` stepped beside it
  (`Riding::stepped` + `port_tick`: every number the original writes, bit for bit, no odd line),
  then the original's entry hooked at `rider_body_wreck_enter` against the port's
  `Riding::hand_over` → `Down::enter_from` → `Emitter::wreck`, as the rider's frame does it:
  216 lanes (18 bones × position, velocity, quat, rates × 3) all within S70's NEAR
  `[0.002, 0.1, 1e-4, 0.15]`; 39 exact. Worst: position 0.0015, velocity 0.058, quat 4e-7, rates
  0.016. The throw (`Launch`) equal, the generator after the entry equal (6 draws each).
  The replay is run2's: the first riding tick within 0.0015 of row 3717's positions.
- `tests/ragdoll.rs` `the_wreck_enters_from_the_riding_bodies`: a `Rider` with holds thrown upside
  down wrecks in the air; its entry's bones are 4e-6 from the riding bodies of that tick, a twin
  without holds (no riding ragdoll) enters 0.71 away, off the skeleton, and wrecks on the same tick.
- `tests/ragdoll.rs` `the_riding_ragdoll_is_ride_state`: a second of riding with holds keeps one
  placed riding body near the bike; a `snapshot` round trip carries it, keeps the local holds, the
  same checksum; a nudge to a riding body moves the checksum.
- Pins: `hash.rs` size of `Sim` 11496 → 11512; `platforms.rs` riders' wire 13371 → 13375 bytes
  (digest spliced, the per-tick hashes unchanged).

## Not verified / not 1:1

- **The entry is near, not exact** (39 of 216 lanes bit-equal). The port's drawn history is the
  bodies' node matrices off a quaternion; the original's part `+0x60` is the node's matrix, whose
  writer per riding tick is still not found (S70). They differ by 1e-3 in the translation (2 ulps at
  x ≈ 4100), which over dt = 1/60 is the 0.06 velocity gap.
- **The entry tick differs from the original's.** The port enters on the verdict's tick, from the
  riding body as its own last step left it. In run2 the original's body ran one more riding tick
  in the verdict's frame +1 and +2 before it entered (the game paused a frame at the verdict). The
  test proves the entry from the same bodies, not that the port's entry falls on the same tick.
- **run2's rows are not reproduced bit for bit before the wreck**: only 485 of 1673 catch-up ticks
  of the original's own update land on their row (run1's 1202 of 1203 did). The rows don't hold the
  actor's carried state (look, cut, filters), and in run2's frames that ran two updates the hidden
  middle request is unknown. The entry comparison is original vs port from the same state, so it
  does not depend on this, but the state is run2's only to ~0.9 ft at the entry row.
- **The riding step uses the current frame's dt**, for the tick the shell drew last frame; the
  original steps with its own frame's. Equal at a steady 60 Hz, not across a dt change.
- **Holds are the shell's last drawn chassis**: online and in fixtures there are none, so those
  entries are still the S70 one off the skeleton. A session with holds on one machine and not on
  another would part; the shell only sets them offline for that reason.
- **Cost**: the riding actor now steps every tick offline even with the env var off. Not measured on
  its own here; the gate's frame-budget tests (one_fps, ecosystem) passed in S110's first gate run,
  whose only reds were the known load flakes (two lobby tests, passed on the lone rerun) and
  `pad::tests::the_keyboard_is_a_seat` (a fifo unplug under load, passed alone at once).
- With `OPENMX_RIDING_RAGDOLL=1` the drawn riding pose is the sim's riding body, a tick behind the
  bike it is drawn on (stepped at the top of the next frame). Not looked at on screen.

## Next

none: the task is done. The entry tick (verdict vs two ticks later) and the drawn-history writer are the open questions above.
