# 1 — s12 ai path follow branch (PART)

Task S309. 2026-11-10.

## What I did

- Ported the race AI's path follower into `ai::follow` (crates/game-sim/src/ai.rs), on top of S308's
  look-ahead and S282b's `spot` / `tail`:
  - `Follower` (the brain's follower words) and `Goal` (the target jump's setup leg: point and spot);
  - `reach`, the vehicle brain's vt+0x78 path reach;
  - `route_look_ahead`, vt+0x88 complete: S308 had only the no-route forward, so the route branch is
    new here;
  - `route_follow`, vt+0x8c;
  - `path`, ai_tick_flag's path branch. The caller goes on to `tail` (S282b).
  - Constants `LOOKS_AHEAD` (0x01000000), `DRIFTS` (0x00800000), `BLENDS_SPEED` (0x8000), node
    `NO_DRIFT` (0x10), `DEFAULT_SPEED` 150, `ANGLES_REST` (0,0,0,1).
- Generated-net oracle of the whole branch: `the_ais_path_branch_matches_the_tick` in
  crates/game-sim/tests/race.rs, not tests/ai.rs as the plan line says. It reuses tests/race.rs's
  generated-course helpers (`t_paths`, `put_paths`, `check_paths`, `t_forks`, `t_point`), which S293
  and S308 built there; copying them into tests/ai.rs would duplicate 300 lines.
- Real-net proof on the sx01 race image: `the_path_branch_rides_sx01s_ai_net_as_the_race_image` in
  crates/game-sim/tests/ai.rs, next to S282c's image test, using its `S282C_STATE` / `S282C_RIDERS`.
- research/oracle.txt:
  - new names `ai_path_reach` (vt+0x78) and `ai_seek_path_end` (where the branch joins the tail);
  - S309 notes on `ai_path_look_ahead` and `ai_route_follow`.
- Filed S311 (low): the route locate's cross-path spot (see below).

## What the original does

`./kit dis 0x383a84 0x383bc4` is the branch:
1. With a target jump (+0x34c) whose setup leg has no spot yet (+0x110 path 0), the net search
   (`./kit fn 0x26b410`) places it at the leg's point (+0xe0). That call takes a0 from whatever the
   seeker left there: the net.
2. The same for the rider's spot (+0x220) at its position (+0xc0).
3. vt+0x78 (0, 0, follow 1, at the position) gives the reach. vt+0x8c (dt, reach, follow 1, the goal's
   spot or 0) follows.
4. With a goal: once the way along the paths from the rider's spot to the goal's (path distance,
   keeping off) is `c.ole.s` the straight distance to the goal's point (mula/madda/madd, sqrt.s), the
   state goes to 1 (+0x354) and flag 0x08000000 is cleared.

**vt+0x78** (`./kit dis 0x2830f0 0x283560`):
- With a spot and follow, under flag 0x01000000:
  - It looks +0x2f0 ahead with the path look-ahead (no nearest place), puts that place's point in the
    aim (+0x1b0), and makes the aim the level unit way there from the spot's point.
  - When the dot product with the level unit way the path runs at the spot (path_ahead) is below
    +0x2f4, the drift (+0x164) goes to 0.
  - Then reach = +0x210 + +0x218 × the flat distance from the rider to the spot's point moved drift
    across the heading (+0x1d0: x + d·h.z, z − d·h.x), at most +0x214.
- Without a spot or follow: re-spot, then the same distance with no drift, at most 0 (a < b) or
  b − base.

**vt+0x88** (`./kit dis 0x2838c0 0x283a10`): copies the spot onto its stack. With no route word
(caller sp+8) it forwards to the look-ahead. With one it runs all of these, whatever each answers:
1. the route search (copy → goal, the list head at its sp+124, no distance, refuse);
2. the route walk (reach, copy, out, list);
3. the route locate (limit f14, list, hint the copy, point, near, no distance, refuse, holding);
4. the route free.

v0 is the three answers and-ed bitwise.

**vt+0x8c** (`./kit dis 0x283a10 0x283edc`):
- vt+0x88 with the reach, own speed (+0x80) and the most reach (+0x214) as its limit.
- When nothing is found: speed = own speed, aim = position, heading = forward (+0xb0).
- Otherwise:
  - The 10 draws (+0x22c) are redrawn when the nearest place is on another path.
  - The drift clock (+0x18c) runs. At its period (+0x198) it draws:
    1. a target (+0x178) in +0x17c..+0x180;
    2. one draw that flips its sign below ½;
    3. a new period in +0x190..+0x194.
  - spot = nearest; aim = the place ahead's point; heading = path_ahead there. (It also takes
    path_ahead at the new spot into a word it never reads.)
  - Drift: 0 on a node with +4 bit 0x10, or without flag 0x00800000. Otherwise it steps at most
    +0x184·dt toward the target, held within ±+0x188.
  - Side step: with no race object (+0xd8), or race +0x5c bit 0. While +0x174 ≤ +0x16c the clock runs,
    drift = +0x168, and +0x168 −= +0x170·dt. After that +0x168 = 0.
  - The aim moves drift across the heading. The heading becomes heading + 0.05·(aim − position), made
    level and unit (VU0).
  - Speed: the node's +0x44 (−1 → 150), or +0x2e0 without follow. Under flag 0x8000, with a next
    node, it blends by `along` toward the next node's (−1 → 150) through mula/madd.
- Either way the angles (+0x1f0) become the quad at 0x426d70, (0,0,0,1), and it returns 1.

## How it was proved

- **Whole-branch oracle, generated nets.** tests/race.rs `the_ais_path_branch_matches_the_tick`:
  12,000 cases, out of 18,045 generated courses.
  - It runs SLUS-21104 from `ai_seek_path_branch` to `ai_seek_path_end`. Every callee is the
    original's: the real vt+0x78 / vt+0x8c / vt+0x88 through a fake vtable, path_spot,
    path_distance, the route search, walk, locate and free, look-ahead, spot point, path ahead, rand.
  - Compared bit for bit: every follower word, the goal's spot, the paths' scratch words and pools,
    and the generator.
  - Coverage in one run: by route 7,979, goal placed 3,973, rider placed > 1,500, followed 8,121
    (nothing ahead ~3,900), looked ahead 4,820, new drift target 4,025, drift held at its most 1,154,
    drew again 971, default speed 2,320, handed over 5,760, plus the side step and the node-speed
    blend (asserted > 500 each).
- **Real nets.** tests/ai.rs `the_path_branch_rides_sx01s_ai_net_as_the_race_image`:
  - It builds `race::TrackPaths` from the s282c sx01 race image's own AI net in RAM, with the
    ground-snapped heights, the race's scratch words and pools, and track +0x4c checked to be path 0.
  - For each of the 3 AI riders it runs the original branch and `follow::path` from the image as it
    is, then 400 more branches. Between branches both sides move the rider along its heading at its
    target speed.
  - 1,203 branches match bit for bit (follower, scratch words, pools, generator), with 15 path changes
    that redraw.
- Full `--test race --test ai` pass (63 tests).

## Not verified / not 1:1

- **vt+0x88's route list head is an unset stack word** (its sp+124, handed to the route search
  without being set). The original appends the route to whatever is there; the port starts the route
  empty, which is the same as a 0 there. The generated oracle zeroes that word through a 3-instruction
  shim in front of vt+0x88. What the real game leaves there was not read. It only matters with a goal
  (a target jump on a path net), and none of the race image's riders has one.
- **A route-locate spot naming another path's node** (filed as S311). When a route path places
  nothing, the scratch spot hands on the earlier path's node with the later path. `race::Spot`
  cannot express that, so the port reads node n of the spot's own path (or nothing past its end).
  The original's next path distance on such a spot recurses for ever. The generated oracle leaves
  these cases out by checking the original's +0x220 right after vt+0x8c: 3 of 18,045 courses. It never
  happened in the image.
- **Unset stack points.** Where `spot_point` / `path_ahead` do not answer into one of reach's stack
  points (the cone's way at a spot with no way on; the spot's point at a last node with along off 0;
  a rider or goal the net search cannot place), the original reads stack it never wrote. The port uses
  0 or skips the step:
  - the hand-over measure, and vt+0x88's route, are skipped when a spot names no node (the original
    reads its null);
  - the oracle leaves these cases out, or clears flag 0x01000000 for the cone case (1,144 cases).
- **Not run in the race image:** the route branch (no goal), the hand-over, the side step (each
  rider's kick clock 3.033 is past its time 3.0), and drift with the cone drop (once).
  These are proved only on generated nets.
- **The ride between the image test's branches is a stand-in.** It moves the rider along its heading
  at its target speed, not the game's physics. Only the first branch per rider is the image as the
  game left it.
- **reach's a < b re-spot** at race +0xd8 → +0x68 → +0x5c's +0x40/+0x50 is ported from the
  disassembly but not oracled: the path branch always passes follow = 1 with a placed spot, so it is
  never reached there. What that object is was not traced.
- **Not wired into the Sim:** `follow::path` is not called by any running tick yet. The AI tick
  around it (ai_update's order) is S281.
- **The path nets' heights from the disc** are not the ground-snapped ones (S310). The real-net proof
  reads the image's RAM net for that reason.

## Next

Nothing for S309. S311 (cross-path spots) and S281 (the AI tick around the follower) carry on.
