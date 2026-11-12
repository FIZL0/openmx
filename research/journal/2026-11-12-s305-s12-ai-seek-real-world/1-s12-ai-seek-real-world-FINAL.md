# 1 — s12 ai seek real world (FINAL)

Task S305. 2026-11-12.

## What I did

- New topic module `crates/game-sim/tests/ai/seek_real.rs` (mounted in `tests/ai/main.rs`; the task line's
  `tests/ai.rs` became `tests/ai/` in K49). One test,
  `s305_the_seeker_is_the_originals_on_free09s_real_world`.
- It runs the original's `ai_tick_flag` on recording2's three **real** AI brains, inside the image.
  - Only the path branch and the tail are hooked, both to return, as in S282a's oracle.
  - vt+0x80/vt+0x94, rand() and the pick-list build are the image's own code. They are reached through
    each brain's own vtable, which the test checks is `ai_seek_reach`/`ai_seek_hand_off`.
- The port's world is `sk::load` off the disc, which is S313's load, reused through `sim.rs`'s
  `s12e_seek_world`. Each set's pick list is read from the image.
  - The port's `Seeker` and `Ride` are read back out of the same RAM before every step.
- `sim.rs`: `S12E_RIDERS`, `S12eLists` (its two lists and `seeker`) and `s12e_seek_world` became
  `pub(crate)` so this module can share them. Nothing else in sim.rs changed.
- `ai.rs`: `NO_LEARN`'s doc comment now notes that free09's AI riders all carry the bit. No code changed.
- Took nothing from opentennis, which has no seeker.

## What the original does (what the real world holds)

These are the S282a "Not verified" questions S305 was filed to settle, now answered from free09's data:

- **The three legs are the disc's three `[HitZone …]` sections.** S313 proved this bit for bit:
  - setup (+0xc0) is `Start`;
  - the lip (+0x00) is `TakeOff`;
  - the landing (+0x60) is `Landing`.
- **What the legs carry on free09.** The test asserts both points below over all 106 jumps.
  - Every take-off has a velocity and a learnt count of its own, from free.hit's
    TargetVelocity/TargetPreload.
  - No setup or landing has either. Both are only a point and a facing. So in states 1 and 3 the seeker
    always asks for the flat-out 150. Inside the setup radius (100 ft on these interfaces) it asks for
    `slow_speed` (20). The landing leg's learnt count, which state 3 hands on, is always 0 there.
- **The legs drive the states as S282a read them, on real jumps.**
  - 5 ft (or 0.01 ft) past the setup point, on the ground, facing the lip: 1→2. LEG_DONE is set and the
    lead becomes 60.
  - Facing away, in the air, 25 ft past (outside the −20 cone) or short of the point: it stays in state 1.
  - Within 20 ft of the lip: 2→3, with CLOSING set. At 30 ft it stays in state 2.
  - In the air between lip and landing it stays in state 3.
  - A settled landing (bike +0xfc 1.0) short, long or wide by 40 ft is learnt from, and the take-off
    changes.
  - A settled landing dead on, 19 ft off (tolerance 20), or out of tries (3) drops the target. The next
    tick draws a new one from the set's pick list through rand(): another target 6 times, the same one 3
    times.
  - An unsettled landing that crosses the landing leg along its facing goes back to state 1 and keeps
    the target.
- **The done words** (bit 2 of each target's per-slot progress word, S313):
  - All 106 × 5 are 0 in this image.
  - The seeker reads them only on a course. free09's sets have none (+0x114 = 0). So a done word set on
    the rider's own target changes nothing: no retarget, and the landing is still learnt from. Both sides
    agree, and the done words are compared every step.
  - Who sets them in play is still not traced (see below).
- **NO_LEARN (0x400000) is set on all three image riders.** As they ride on free09 they never learn: a
  settled landing only crosses back to state 1. This is why the test clears it for its learning runs.
  Who sets it is still not traced.
- **The pick lists.** All four sets (the human's and the three AIs') point at one group's list.
  - The original's list build, rerun on the image after clearing them, gives the same group.
  - So does the port's `sk::build` on the loaded world with `hit_freeride` false.

## How it was proved

Save-state RAM image (recording2, free09, s2's `context/states`, symlinked in), and the original's code
run in the interpreter on its real brains. `./kit check -- -p game-sim --test ai -- s305_` passes.

- Per rider:
  - 3 ticks as the image holds it;
  - 6 setup placements;
  - 3 lip placements;
  - 1 airborne;
  - 9 landings, with a second tick after each drop;
  - 2 unsettled landings at the crossing;
  - a run of 20 settled landings (8 short, 8 long, then alternating) and one tick after it.
- That is 54 steps per rider, 162 in all, plus one run of the original's pick-list build. Each step
  restores the image and the world first, except within the run.
- Compared bit for bit every step:
  - how it ended;
  - the whole seeker;
  - the four controls;
  - all 106 jumps × 3 legs (velocity, aim, point, facing, learnt);
  - every set's pick list;
  - all done words;
  - the rand() state.
- The run went through the halving 15 times and re-aimed the lip 6 times. The test asserts at least 3 of
  each.
- Mutants, each made, run and then restored with cp + touch. All three were killed at the seeker compare:
  - the setup cone at −30;
  - the settle bar at 0.05;
  - out of tries at 4.

## Not verified / not 1:1

- **The positions are written in, not ridden.** Apart from the 3 "as is" ticks, each step puts the rider
  at a chosen spot over its own real jump. The bike's physics does not run, so the path a rider really
  takes into a jump was not compared. Reason: that needs a live capture of a free09 AI rider choosing
  and riding a jump. The emulator was not started; the port and the original stepped on the same RAM
  are bit-exact.
- **Who sets NO_LEARN, and who writes the done words, is not traced.** The image shows NO_LEARN on all
  three riders and every done word 0. Neither is written by the seeker. Reason: out of scope (the seeker
  only reads them). The S282a notes stand.
- **What bike +0xfc ("settle") is, is not traced.** The image's value is 0.099 on all three bikes. The
  test sets 1.0 to reach the learning branch.
- **Courses, the sequence and the done-word retarget were not run on real data.** free09 has no course,
  and the sequence word has no writer. Those branches are proved only by S282a's generated oracle.
- **hit_freeride.** The port's build, with `hit_freeride` false, gave the same group as the original's
  build with the owner's own answer. So the two agree on free09, but the human owner's real answer (yes
  or no) was not read. Reason: the build's result is all the seeker uses, and S306's oracle covers both
  answers.
- **One image, one map.** Only recording2/free09 holds AI riders mid-ride. Other maps' hit files were
  not stepped.
- MX vs. ATV's PC exe was not read. It is not needed: the PS2 code is the oracle and is run directly.

## Next

None for S305. Wiring `sk::load` into the Sim is S314.
