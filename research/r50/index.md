# R50 — C33 ai0 riding bones vs. the original

Probe: an Ee from C33's image (`capture/c33/free09-ai4.ee.bin`) stepped beside the Sim with
`draws::whole_frame` + `render`, hooking `rider_ragdoll_pose` (0x0030f0c0) and
`rider_body_update` (0x0024e640), comparing ai0's part node places (part+0xa0 → node+0x80) each tick.

- **Seed bug (fixed).** `c33_riding_seed` handed `seed_drawn` the part+0x60 matrices. Those sit at
  each part's centre of mass; the riding entry reads `Riding.drawn` as node places, so the offset
  went in twice. With node matrices, tick 4 matches to 1 ulp. Marks unchanged.
- **Holds ruled out.** Feeding the original's holds (a+0x5d0..0x5dc pins, in the chassis frame)
  every tick leaves bone 17 z 0.15 ft off at 133.
- **The gap is the pose spring target.** The render Body (`c33_pose` → `body.ride`) starts its
  clip machine fresh and its skeleton is 0.3–0.5 ft off from tick 5. Which skeleton the original
  uses, tried as the Sim's `rider.skeleton`:
  - joints before frame t (and bike node): exact to 6, parts at 7–8 (clip 35→30, feet off the pegs);
  - joints and bike node after frame t, or at body-update entry: wrong from tick 4 (moved bike node);
  - joints at body-update entry, bike node before frame t: ~0.1 ft by 14, 0.15 at 133;
  - **joints after frame t, bike node before frame t: within 0.04 ft, 0.004 at 120.**
- Follow-up S488: seed the render Body's animation state from the image.

## S488

- **Seeded.** `c33_anim_seed` (race4.rs) seeds each render Body's machine, clock, snapshot and the
  bike driver's pose/fore-aft (`r+0x9e8..0x9f4`; unseeded they pull the axes off for a second).
  `c33_drawn` keeps `Body.drawn` as the shell does, which fixes the fades after restarts.
- **Joints exact.** ai0's joints match the copy's after each frame bit for bit, except the fades
  after the restarts at 64 (0.034, gone by 73) and 120 (0.097, gone by 127).
- **Marks unchanged.** The bodies drift the same: mean worst 0.175 ft (ticks 5–125) against 0.188
  unseeded; max 0.45 at 28 (part 7).
- **One frame late.** The Sim's riding step in frame t uses the joints `c33_pose` left after t-1.
  Fed the copy's frame-t joints before the step, bone 17 stays within 0.06 ft through 131
  (0.29 ft off at 130 otherwise). The test then fails at 131: ai0's foot dust (entry 4) draws early.
- **Bodies still stray at 24–36** (0.44 ft, part 7) even when fed, so the spring target isn't all
  of the gap.
- Moving the pose before `step_all` (joints from the rider's pre-step state) does not help: the
  joints then go off (0.42 at 4).

## S490

- **Hook.** `Sim::step_all_posed` / `Rider::frame_posed` run a pose callback inside each rider's
  frame, after the rider's steps and before the riding ragdoll's tick. `step_all` passes a no-op.
- **Posed in the frame** (the render Body's own joints, the image's holds kept), ai0's worst part
  gap vs. an interpreter copy: 0.02 ft at 7–15 (0.04–0.13 posed after), bone 17 0.065 at 128
  (0.43 after). The test then fails at 131: ai0's foot ball touches at 130, the original's at 131,
  so the foot dust (which reads the bones a frame late, as the original's does) draws on 131, not 132.
  S12e stays posed after the step, marks unchanged.
- **Part 7 is a hand; the 24–36 stray is the test's frozen holds.** The original's grip targets move
  apart from 18 (one up and forward, one down and back: the bars turning). The replay seeds
  `Rider::holds` once from the image. Fed the copy's request of the same frame (rider `+0xb80`), every
  part stays within 0.08 ft through 130. Fed the previous frame's, worse than none (up to 1.8 ft).
- **The early touch stays** with the copy's joints *and* holds both fed: bone 17 drifts from 0.004 ft
  at 120 to 0.049 at 129, 0.02 ft low at 130. The actor's fades, foot fades, pose damping, repose,
  torques, gravity scale and hold pins (targets, gains) all agree 116–131. So the gap is in the
  riding body's own step after the restart at 120 (part 3 leads it).
- Test: `s490_c33s_bodies_posed_in_the_frame_follow_the_original` (race4.rs) holds ai0's parts
  within 0.1 ft through 130, posed in the frame on the copy's holds.
