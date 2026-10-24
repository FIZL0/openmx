# 1 — riding request (FINAL)

Task S111. 2026-10-23.

## What I did

- Found the writer of every field of the riding request (the block at rider `+0xb80`, the body actor's
  `+0x488`) and rebuilt `ragdoll_riding::request` from the original's sources. The new
  `ragdoll_riding::Sources` holds the rider numbers the writer copies; `Sources::request` builds the block.
- `Rider::packed_pose` (new field, on the netplay wire after `motion`): the pose as the state pack latches
  it, in front of this tick's animation header. That is the request's pitch.
- Wired `motion::Driver::freeze` into the rider's idle step while the put-back holds the body. The
  original's idle does this (`./kit fn 0x00258cc0`, its tail: `if (+0x3a0) +0x9e8 = +0x9ec = 0.5`). Before
  this, nothing called it.
- The shell (`ride_ragdoll`) passes the latched pose and the drawn bike's "Bike" root node in the world
  (the chassis model's root through `ride.transform()`) as the request's frame.
- Turned the riding ragdoll on by default (`OPENMX_RIDING_RAGDOLL=0` turns it off), after the by-eye
  look below. Online it stays off, as before (`online.is_none()`).
- Filed S112: the body's vehicle-animation request reads the same block (the biplane's fill shows
  band = request +0x178, phase/mix = +0x170/+0x174). The port's `body_frame` fills it with
  `chassis.speed` and this tick's pose, and game-render's `Chassis::request` with `local_vel.z`.

## What the original does

The request writer is `./kit fn 0x0027c2d0`, called from the player update `./kit fn 0x0027c890` after the
base update. It is gated on `(rider+8) & (rider+0xb84)`. Every field is a copy of a snapshot the state
pack writes (`./kit fn 0x00258d50`). The base update's order (oracle.txt, A5) is ground step → air state →
attachment walk → stunt_idle → **state pack** → clocks + animation header. So every bike number is this
tick's, and the pose is the one the header left last tick.

| request | source (rider offset) | port |
|---|---|---|
| +0x0c held | +0x3a0 | `put_back.held` (S74, not touched) |
| +0x10 rows (4 quads) | node `*(+0x18)` world matrix (`0x001b2a50`) | `ride.transform()` |
| +0x50 frame | node `*(+0x964)` world matrix: the bike model's "Bike" node | the drawn root, shell-side |
| +0x90 velocity | +0x840 = chassis +0x260 | `chassis.vel` |
| +0xa0 rates | `0x001b30e0(node +0x964, +0x850)`, +0x850 = chassis +0x280 | `unrotate(frame, chassis.world_ang_vel)` |
| +0xf8 looking | !(+0x93c) = !(+0x574) | `!air.on_ground` |
| +0xfc posed | +0x940 = +0x4f4, the backflip torque flag | `air.flip_torque_applied` |
| +0x100 | +0x944 = +0xa44 (no-hander) | not in the port's Request |
| +0x104 reason | +0x868 = +0x748 | not set: a wrecked rider has no riding body here |
| +0x150 hit velocity | +0x560 if reason 7, else +0x840 | the velocity |
| +0x16c tucked | +0xcec ← +0x648, the reverse command | false: no pad of the original sets it (P73) |
| +0x170 pitch | +0x898 = +0x9e8 (pose axis), packed before the header | `Rider::packed_pose` |
| +0x174 | +0x89c = +0x9ec (fore-aft) | not in the port's Request |
| +0x178 speed | +0x8c8 = +0x438 = contact_vel.z (`bike_air_state`) | `air.contact_vel.z()` |
| +0x180 centre | +0x820 = node +0x18's position, w 1 | `ride.transform()` row 3, w 1 |
| +0x190 quat | +0x830 = mat4_to_quat (`0x0013f7b0`) of node +0x18's matrix | `math::mat4_to_quat(ride.transform())` |

So the pre-S111 guesses were wrong on these: rows (chassis quat, not the node with the no-hander roll),
frame (chassis node, not the drawn bike root), rates (the body-frame `ang_vel`, not the world rates turned
into the drawn frame), speed (`local_vel.z`, not contact_vel.z), pitch (the vehicle anim's mix lag, not
the rider's packed pose axis), centre (cofg, not the node position), quat (chassis quat, not the node's).

The request's `posed` is the backflip torque flag, which the actor latches into Posed. **This is what the
original lets go of during a backflip**: from the first substep the backflip torque goes in, the body is
drawn from its clip, held to nothing. Gestures and no-handers let go through the body machine's
Posed mode (V61's `posed_off_the_table`), which reads the same `from_table`/`holding` words.

On a held tick (the put-back's two ticks), the idle's tail writes pose = fore-aft = 0.5 before the pack.
So that tick's request carries 0.5 and not last tick's pose. run1 shows it at frame_counter 2996–2997.

## How it was proved

- `crates/game-sim/tests/ragdoll_riding.rs`
  `the_riding_request_is_the_originals_from_each_row`: on every one of S109's run1 rows (1203,
  frame_counter 1941–3142, 4 of them held), the port's `Sources::request` writes the original's rates,
  centre, quat, looking, posed, hit velocity, tucked and pitch bit for bit. The inputs are the row's
  `ch_ang_vel`, `on_ground`, last row's `shown_pose` (through `motion::Driver::freeze` on held rows) and
  the request's own rows/frame/velocity/speed. Before writing it, the lag was measured: rates are exact
  at lag 0 on 1203/1203 against 0 at lag 1. Pitch and the fore-aft are exact at lag 1 on 1202/1203 and
  1203/1203; at lag 0 they match only 403 and 921 rows, the rows where the pose did not move.
- `the_riding_requests_sources_are_the_riders_at_the_state`: at S109's save state, the sources read from
  RAM build the request the state holds, bit for bit: node +0x18 and +0x964 world matrices, chassis +0x260
  and +0x280, rider +0x438, +0x574, +0x4f4, +0x898. The speed also equals the pack's +0x8c8. It is neither
  the velocity's length (0.370624 against 0.3699714) nor its z.

- **By eye** (`target/release/openmx --track free06 --headless --size 640x360 --play T --shot … --shot-frame N`,
  ragdoll on and off, N = 100…340). The tapes were written by hand, one line a 30 Hz tick, Cross held
  for gas. The backflip uses P30's stick schedule `0:0,229:1,236:-1,239:1,242:0,295:-1,305:0` (the
  shell shows "BackFlip 10,000"). The Taunt tape holds Circle and stick up for ticks 256–261 off the
  same jump (the log shows "trick: Taunt" and "landed: Taunt"). With the ragdoll on, Taunt's arm comes
  out as in the clip draw (frame 270). The backflip is the clip pose through the whole rotation (270,
  285), with the body on the bike. Ordinary riding (100, 200) and after the landing (320, 340) has the
  hands on the grips, the feet on the pegs and the body upright, a little narrower than the clip.

## Not verified / not 1:1

- **The frame and so the shell's rates are not bit-exact.** The drawn "Bike" node in the world is
  game-render's host-f32 node matrix through `ride.transform()` (`math::mul`). The original's node
  update is not ported. `Sources::request` is exact for the frame it is given; the shell gives it an
  approximate one.
- **Per row, the rows, frame, velocity and speed are fed off the request itself**. run1 captured no
  separate copy of the chassis node, contact velocity or chassis velocity. Their sources are proved
  on one tick only: the S109 state. That the port's own ride makes the same numbers
  (`ride.transform()` = the chassis node, `air.contact_vel`, `chassis.vel`, `chassis.world_ang_vel`)
  comes from P32's and the air and chassis proofs. It was not re-proved end to end through a ridden
  port here.
- **posed (the backflip flag), tucked, hit velocity for reason 7, no-hander**: run1 has no trick, no
  reverse and no wreck. These are read off the listing only. At the state, posed = 0 = +0x4f4.
- **Request fields the port's Request has no slot for** were not ported: +0x100 no-hander, +0x174
  fore-aft, +0x1a0 gesture, +0x144, +0x160–0x168, +0x1a4/+0x1a8/+0x1b0. The port's actor does not read
  them. The listing for each is in the oracle block.
- **The held snap** (`motion.freeze()` on a put-back's held ticks) moves `motion.pose` on those ticks
  in the sim. It is proved on run1's 4 held rows (the request's pitch) only. The gate is the check
  that it moves nothing else that was proved.
- **The snapshot wire grew a field** (`Rider::packed_pose` after `motion`). A build before S111 cannot
  decode this one's snapshots. tests/platforms.rs's `RIDERS_WIRE_DIGEST`/`RIDERS_WIRE_LEN` moved
  (13323 → 13339 bytes, 4 riders × 4); no state hash moved.
- **By eye is against the port's own clip draw, not the original's picture.** No side-by-side with
  PCSX2 was made. Nor was HEAD compared, to tell how much of the backflip's look is V61's posed mode
  and how much the request's flip flag. Both say Posed through the flip.
- **Pixel diffs between shots say nothing**: two identical runs (ragdoll off, same tape) differ by
  2600–5100 pixels of 230k at the same frame, so the on/off diffs (450–8200) were not used.

## Next

- S112 (filed): the body's vehicle-animation request from the same fill.
- V63 can go ahead: the riding ragdoll is on by default.
