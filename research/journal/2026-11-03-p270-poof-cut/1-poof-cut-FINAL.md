# 1 — poof cut (FINAL)

Task P270. 2026-11-03.

## What I did

A `--trace-picture` ride shows each drawn frame's rider nodes in the bike's frame (the new
`picture pose:` line, crates/openmx/src/main.rs `trace_picture`). The ride: free06, full throttle off
the first jump, Circle and stick up from tick 250, and L1+R1 at 275. In it, every node was measured
against the pose the rider settles to 20 ticks after the poof. Before the fix the frame before the
poof was 5.7 ft off. After the cut (tick 276) four things carried the old pose on, each found and cut
in turn:

1. **The riding ragdoll's spring.** On a put-back, `Rider::frame_bumped` ran `riding_tick` with the
   skeleton the shell had posed at the top of the tick: the trick's pose. The body then swung
   3.4–4.5 ft back toward the trick before settling. With the cut, `riding` is dropped on the
   put-back tick. Next tick the body is placed afresh off the held pose, which the shell poses after
   this tick.
2. **The bike's clip fade** (game-sim `motion::Driver`). A put-back restarts the 28-slot motion with
   its ramp at 1, fading out of the trick's snapshot at the blend rate, 3 per second: ten ticks. The
   chassis root slid 1.3 ft over those ten ticks, and the grips and the rider held to them went with
   it (1.2–1.6 ft off, settled exactly at tick 287). With the cut, `motion.ramp` is zeroed and so is
   the ramp on this tick's frame (new `motion::Blend::unramped`).
3. **The wreck's two late ticks.** After the wreck clock or the wreck page, the body's request had
   not yet seen the put-back (S74's fill lag). So the put-back tick drew the wreck clip at the new
   spot, 4.1 ft off. `ragdoll_shown` would have drawn the thrown bones there, with the ragdoll on.
   With the cut, the put-back tick tells the body itself (`put_back.held`, `wreck` false), drops
   `ragdoll_shown`, and freezes the motion pose. It also writes the packed pose and fore-aft at once
   (S111's held pose), where the next tick would otherwise have done it. One tick was still off:
   `body_frame`'s band comes from `ride.air.contact_vel`, which `place_back` keeps and the next physics
   rewrites, and it still held the pre-wreck 49 ft/s. That drew the rolling pose for a tick, 1 ft
   above the standing one. On that one tick, now asked through the new `PutBack::told_early`, the band
   is the placed bike's: zero, since `place_on` zeroes the velocity.
4. **The camera** (main.rs). `follow.cam.snap` was set on a put-back only for the biplane (V29). It is
   now set for the bike too, and the eye cuts with the bike, 25 ft down on this ride, then moves under
   0.1 ft.

All of game-sim's changes sit behind `Rider::cut_put_back`. It is off by default (the original), and
the shell's `new_rider` sets it on every rider: 1p, split-screen, and online (the online decode keeps
it from the local sim, like `edge_armed`). The ride itself is untouched: the lean, the contact speed
and `place_back` are left alone. Only the motion driver's ramp and pose, the packed axes and the
put-back hold change. They are animation state, but they are hashed, so with the switch on a poof's
hash differs from a switch-off run. Both peers run the same switch.

An earlier try also zeroed `ride.held.lean`. It was dropped: it changed the ride's physics, and it was
not what slid the pose (item 2 was).

What remains after the cut is the rider following the rate-limited lean back to upright, which any
riding tick does: at most 0.21 ft (ragdoll) and 0.35 ft (clip) off the settled pose, at under
0.14 ft a tick.

## What the original does

The original springs the riding body in from the pose it had (S201 matched the put-back entry to it).
The bike's motion restarts with a full cross-fade. The body sees a wreck clock put-back a fill late
(S74). Only the plane's camera snaps (V29). All of that is still the port's behaviour with the switch
off, and game-sim's tests (riding_spring, put-back, S74's drawn-jump tests) run with it off. P270 is a
user ruling against it, for the shell.

## How it was proved

- New `crates/openmx/tests/poof_cut.rs`. It rides the binary headless with `--trace-picture` and a
  generated tape (poof mid-trick at tick 275), once with the riding ragdoll and once with the clip
  pose (`OPENMX_RIDING_RAGDOLL=0`). It asserts:
  - the frame before the poof draws the trick (more than 2 ft off riding);
  - the poof's frame cuts;
  - the rider's middle is within 6 ft of the new bike and more than 20 ft from the old spot;
  - every node, from the poof's frame for 20 ticks, is within 0.6 ft of the settled riding pose;
  - the eye jumps more than 10 ft on the poof and moves less than 1 ft after.

  Both pass. With the shell's switch set false, the ragdoll test fails: tick 277 is 3.3 ft off.
- Measured by hand with the same trace (all tables in feet, worst node off the settled pose):

  | case | before | after |
  |---|---|---|
  | poof mid-trick, ragdoll | 5.7 on the cut tick, then springs 3.4–4.5 | ≤ 0.21 |
  | poof mid-trick, clip | slides 1.2–1.6 for 10 ticks | ≤ 0.35 |
  | poof mid-whip (stick left, no button) | 2.0 before the cut | ragdoll ≤ 0.31, clip ≤ 0.29 |
  | wreck page put-back, clip | 4.1 on the cut tick, 1.07 step on the next | ≤ 0.39, step ≤ 0.14 |
  | wreck page put-back, ragdoll | 4.3, then sprang | ≤ 0.27 |

- `./kit check -- -p openmx --no-fail-fast`: 3033 passed.
- `./kit check -- -p game-sim --no-fail-fast`: 1209 passed, 1 failed. The failure is kiowa_capture
  `the_rotors_probe_is_the_images`, whose c14-kiowa.ee.bin is not in this slot (it fails on HEAD too,
  per P272).
- Shots of the poof ride: `openmx --headless --steady-clock --track free06 --draw-fps 30 --play <tape>
  --shot ... --shot-frame N --size 640x448`. They are in context/shots/p270/poof-274/276/277/280.png:
  - 274: mid-trick (a taunt);
  - 276: the poof's frame, rider seated on the bike at the new spot, camera behind;
  - 280: the same pose under P57's ghost fade.

## Not verified / not 1:1

- **Not the original's, on purpose.** With the shell's switch on, a put-back is the user's ruling:
  - no spring (S201), no bike cross-fade, no late wreck ticks (S74);
  - the camera snaps for the bike (V29 snaps only the plane's).

  The original's poof-tick picture was not checked in PCSX2 against this; the S57 video was not
  rewatched.
- **The hash of a ride with a poof** differs from a switch-off run, through the motion ramp and pose,
  the packed axes and the put-back hold. Netplay with both peers on this build is unaffected. A
  replay or fixture recorded with the switch on would not match a switch-off run past the first poof.
  No such fixture exists: game-sim's tests leave the switch off.
- **Split-screen and online were not ridden.** Every seat's rider is built by the same `new_rider`
  that sets the switch. Online, the riding ragdoll is offline-only and the body is posed in the
  session's tick, which runs the same `body_frame` and `frame_bumped`. Only the 1p trace was measured.
- **The rate-limited lean** still walks back after a poof (up to 0.35 ft of pose over about 10
  ticks). That is the ride's input limiter, physics state, and was left alone.
- **The chord mid-wreck** (as opposed to the clock or page put-back) and a put-back mid-flip were not
  ridden separately. Both go through the same `out.respawned` branch.
- **A put-back two ticks after another**, landing after the physics on the tick the first hold runs
  out, also leaves `PutBack::told_early` true. With the switch on it asks band 0 for that one tick,
  which matches the placed bike; with it off, nothing reads it.
- **Not seen on screen** at the display's frame rate: only the four headless shots and the trace.

## Next

Nothing. The task lands with this entry.
