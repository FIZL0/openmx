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

## R54

- **Cause: the main bone is set with last frame's rest.** Before the step, the original sets the main
  bone's quat to the request's quat times the main part's rest (0x24ce88–0x24cee0: `actor+0x488`
  `+0x190` times the joint record `actor+0x234 + 0x60*joint + 0x50`, joint from `actor+0x2f4 +0x40`,
  stored to that part's body `+0x170`). The normalise call that writes this frame's rests into those
  records runs *after* the set. So the set reads the rest the record kept from the frame before.
  The port (`ragdoll_body.rs:451`) uses this frame's `skeleton.main_rest`.
- **Why 120.** The rest only moves a little frame to frame (0.003 in the quat at 119). At 120, the
  landing, the clip changes (35→30) and the main rest jumps (x −0.071 → −0.045). The set then differs
  by 0.026, and the bodies part from there. The joint torques use this frame's rests; they agree.
- **How it was found.** Inside the copy's `rider_body_update` hook: seed the port's actor from the
  image at entry (`s483_riding_of`), call the original, then run `Actor::tick` with the points and
  rests the original's own otp/normalise calls made that frame, on a flat world far below. Compare
  every bone after. With this frame's rest: 0.002 ft at 111–119, 0.013 at 120 (main bone velocity
  0.38 ft/s off), up to 0.033 at 127. With the rest read from the record at entry: **bit-exact** on
  every tick to 130, except 4 (the seed's first entry has no history) and 15–17 (feet off the pegs;
  the probe's world has no ground).
- **Ruled out on the way**, all from the same entry state: the request (agrees to 1 ulp), the
  actor's pose damping/spring, hold velocity, hold targets, bike frame and box (all exact after the
  tick), the bike box hold (no change with it off), the Carrier spin (z only is right; adding the x
  and y rates makes 120 twenty times worse), substeps (the riding world runs one, three passes, as
  the original's single integrate store shows), and the integrate and joint passes (they move the
  main quat by the same amount on both sides; only the starting set differs).
- **The fix, tried and reverted.** Keeping each `Riding`'s main rest from its last tick and setting
  with that: the S490 replay's worst part through 130 goes from 0.0834 ft to 0.0069 ft. Bone 17
  stays within 0.004 ft at 120–130 (was 0.049 at 129) and no longer touches at 130, so the early
  foot dust should go too (not run). Filed as B42.
- **Landed (B42).** `Riding` keeps `main_rest` (on the netplay wire), set after each step; a fresh
  body's first step uses the rig's rest, and C33's seeded bodies take the main part's joint record
  (`Riding::seed_main_rest`). The S490 run is now held to 0.01 ft through 134: worst 0.0066 ft
  (part 15 at 35), and ai0's foot dust draws on 132 with the original's. Run on to 400, the bones
  stay within 0.01 ft until ai0's drawn holds jump 0.09 ft at 198 (R56's partings).
