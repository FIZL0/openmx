# 2 — s12 ai in the sim (PART)

Task S12e. 2026-11-09.

## What I did

- **Checked what S12e stands on.** S12c and S12d have landed (that is why it reopened). But the task's
  AI riders are "driven by the route/avoidance/trick controllers", and the route controller is not
  ported. Three open follow-ups from the S12b split stand between the ported controllers and an AI rider
  the Sim can step:
  - S282, the route follower (`ai_tick_flag`, 5936 bytes, and `ai_route_follow`, with the track's AI
    path). It makes the target direction and speed `ai::controls` and `ai::avoid` take in, and it draws
    `rand()` for its wobble and drift.
  - S283, the character reader (`AIChrctr.aip` into `ai::Tuning`/`AvoidTuning`, the trick cool-down and
    chance).
  - S281, the AI tick (`ai_update`'s order and the hand-off of the controls to the bike).

  Without them, the proof the task asks for (a free09 4-racer replay of `rand_state` and the AI bikes'
  bodies) cannot match. The route draws would be missing from the interleave, and the bikes would go
  nowhere. Each of them is a session's work and its own task, so I did not do them here.
- **Ported the rider-to-brain copy** that the brief names, as `ai::feed` (with `ai::Rider`,
  `ai::Senses` and the `ai::bump` bits) in `crates/game-sim/src/ai.rs`. The brief's address 0x0027fb30
  is wrong: the copy is `ai_feed`, 0x0027fbd0. The function at 0x0027fb30 is the rider method just
  before it, described below.
- **Found what sets `flag::NO_TRICKS` (0x10).** S12d could not trace it. The feed sets it from rider
  +0xa0c, the rider's "trick in progress" word (tests/ai.rs `IN_PROGRESS`), so a flight is not rolled
  again while its trick runs. The flag doc now says so.
- **Proved it** with a function oracle (below).
- Described the rider assist (0x0027fb30) in `research/oracle.txt` (a comment: S12e's plan line names the address, and a named one fails the audit) and wrote the feed's offsets under
  `ai_feed`. Noted S281: port the assist where the AI tick calls it.
- Did **not** wire `plan_trick` / `gesture_gate` into a rider's frame. There is no AI rider in the Sim
  to call them on until S281 gives the tick its order. The feed is the input `plan_trick`'s `Flight`
  and its on-ground bit come from: `Senses::velocity.y`, `frame[3].y`, `ground` and `flags`.
- opentennis has no rider AI to take from.

## What the original does

`ai_feed` (`./kit dis 0x0027fbd0 0x0027fe90 -p boot`). `this` is the rider, the brain is at +0x10d0.

1. No brain → return 0, and nothing else happens.
2. Brain +0x290 = brain +0xc0, the position from the frame before.
3. Brain +0x90..+0xcf = the frame object's (rider +0x18) +0x80..+0xbf: four rows, side/up/forward/
   position.
4. From the body (rider +0x7a4):
   - brain +0x50 = +0x260, velocity (world);
   - brain +0x60 = +0x70, acceleration;
   - brain +0x70 = +0x290, angular velocity in the body frame.
5. Brain +0x80 = `sqrt.s(mula x·x, madda y·y, madd z·z)` of the velocity.
6. `ground_query(world +0x7ac, point, brain +0x2c0, 0, 0)`. The point is
   (pos.x + v.x·rider+0x11d0, pos.y, pos.z + v.z·+0x11d0), and its w is the stack, never set. Only the
   normal is kept.
7. Flag 0x20 = rider +0x574 (on the ground).
8. `ground_query(world, copy of brain +0xc0, 0, 0, 0)`. Its y goes to brain +0x204.
9. Flag 0x10 = rider +0xa0c (a trick is running).
10. Second flag word +0x38. Bits 1|2 are set when all of these hold:
    - on the ground;
    - body +0x264 (vertical velocity) not ≤ 5;
    - body +0x278 (speed along the body's forward axis) not ≤ 30;
    - `100·(2⁻³¹·rand())` ≤ rider +0x1100.

    Otherwise both bits clear. `rand()` is drawn only once the first three hold.
11. Return 1.

What reads the +0x38 bits: the AI update's state-3 branch tests bit 2 (and 4) and then sets 1|4
(boot.c around `*(int *)(iVar2 + 0x354) == 3`). The port names them `bump::A`/`bump::B` and says nothing
about their effect.

The rider assist (0x0027fb30, rider vtable slot 0x004551bc): body +0x284 (yaw rate, world) =
`mula(brain +0x1f4, w) + madd(+0x284, 1−w)` with w = rider +0x1138. On the ground, body velocity x and z
are blended the same way toward brain +0x1c0/+0x1c8 by rider +0x113c. Not ported.

The PC exe is not on this slot (no `context/mxvatv-pc/`), so it was not read.

## How it was proved

`the_rider_to_brain_copy_matches_over_ten_thousand_frames` in `crates/game-sim/tests/ai.rs` runs the
original `ai_feed` in `ps2_oracle::Ee` with the original `rand()`. `ground_query` is hooked: it answers
generated heights and normals and records the points it was asked at. The port gets the same answers
through a test `World`.

- **What it compares, bit for bit:** every brain field the copy writes (nine quads, speed, ground, both
  flag words, prefilled with random 64-bit words so a port that touches other bits fails), the
  generator's state, and both query points (xyz of the first, whose w is stack garbage; all four lanes
  of the second).
- **The inputs:** vertical speeds and forward speeds aimed at the 5 and 30 edges (exactly, one ulp
  either side, signed zeros), chances 0/10/50/100 ± 1, signed-zero w lanes.
- **Branches reached** (committed seed): on the ground 6639, rising 3523, rolled 1941, bumped 764, roll
  failed 1177, tricking 3424. The test asserts each count is ≥ 100.
- **The no-brain case** returns 0 with no draw and no query.
- **Two mutants each fail it:**
  - drawing without the three tests fails "the generator" at case 16;
  - asking the height at the look-ahead point fails "the point under the bike" at case 1.

## Not verified / not 1:1

- **The task itself is not done: no AI rider is in the Sim.** Spawning, stepping in actor-list order,
  the `rand()` interleave and the free09 4-racer replay all wait on S282 (route follower), S283
  (character reader) and S281 (the AI tick). Blocked `--after S282,S283,S281`.
- **`plan_trick` / `gesture_gate` are still unwired**, for the same reason.
- **The ground query is a stand-in in the oracle.** The test answers both queries with generated values
  and models the port's `World::ground` contract: the point keeps x/z/w and takes y; the normal is
  written whole. Whether the original's query writes the normal's w lane, and what it does off the
  field, is S64's ground query (proved there on the terrain), not re-proved here.
- **The first query point's w** is uninitialised stack in the original. The port passes the position's
  w. The query does not read it as far as S64's port goes; not checked in the original's query.
- **What the `bump` bits do** is not traced past the state-3 branch that reads bit 2. Their names are
  placeholders for the behaviour.
- **Rider +0x11d0 (look-ahead seconds) and +0x1100 (bump chance)**: their meaning comes from how the feed
  uses them. Where they are set (the AI set-up or the character file) was not traced.
- **The rider assist (0x0027fb30)** is not ported. It is noted on S281.
- **No live capture.** The feed is proved as a function only. Where in the frame it runs is S281's.
- **The PC exe was not read** (not on this slot).

## Next

S282, S283, then S281. Then S12e: spawn the roster's riders as A-physics bikes in `tick.rs` in
actor-list order. Each frame, run `feed` → route → `avoid` → `controls` → `plan_trick` per rider, in
`ai_update`'s order. Then record the free09 4-racer capture.
