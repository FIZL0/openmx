# 1 — tick two pads (FINAL)

Task S124. 2026-10-27.

## What I did

- **Reran S89's rides on main 2d48886b** (s1, with s3's `context/capture/s89` copied in; the states
  were already here). Tick 2 no longer parts. Along now parts at **tick 31**, on the gas: the
  original's rear wheel takes its first torque (wheel_torque 379.33) while the port's is 0. Across
  parts at 16.
- **Found what fixed tick 2, by probing commits in a scratch worktree** (same data, the test's own
  first divergence):

  | commit | what it holds | along parts at |
  |---|---|---|
  | 0fa426d3 | S114's branch alone (wheel ray probe) | 2 (vel[0] port …7aee vs …8dd1) |
  | c28ffe9c, b68f3977 | S107 alone (chassis inverse inertia) | 2 (port …8f36 vs …8dd1) |
  | 40ed17eb | neither | 2 (port …7e9c) |
  | 67d20fe9 | the S114 merge: both | 31 |

  A first-parent bisect from 67d20fe9 to S138 found tick 31 on every merge. So the tick-2 parting
  was two causes, each enough to part it: S107's inverse inertia (the +0/−0 and ulp rows) and the
  port's old collision bundle (S114). S114 measured on a branch without S107, and S107 without
  S114. Once both landed, tick 2 agreed, and nobody reran the test with the output on, because a
  passing check prints nothing.
- **Found what parts at 31: the test, not the port.** `the_dallas_rail_rides_replay_from_their_save_states`
  fed the pads by row (`inputs_from_capture`), while its lanes go on `quarter_ticks` (frame counter −
  saved − 1). A row's pad is the one the port reads `PAD_LEADS` (1) later (S58), so the gas arrived a
  tick late. The test now takes `save_state_inputs(&cap, &ticks, st.held_controls())`, which is
  S58/S117's rule.
- **Raised the ratchets** (`crates/game-sim/tests/replay.rs`). `S89_AGREES_THROUGH` goes from [1, 1]
  to **[236, 141]**. In `S89_LANES_AGREE_THROUGH`, the bed-edge contact lanes (swept_contact and
  surface, both wheels, both rides) now agree on every tick. They used to part at 106/108 along and
  50/53 across. Along's ray lanes now agree through 517/518, the locked lanes through 571/571, and
  rear spinning on every tick (it was 31/16).

## What the original does

Nothing new here. The rides follow the original's ride step tick for tick. The bed's edge, where
S89/S114 saw the contact lanes part, is ridden through bit for bit once the gas comes on the
original's tick. The first tick's buttons are the state's own controls (S117). For S89's states
these give the same run as the pads' (with `None` in their place the result was the same: 237 and
142), so the fix is the `PAD_LEADS` alignment.

## How it was proved

- **Bit-exact save-state replay** of context/capture/s89/along.csv (600 rows) and across.csv (300).
  Every chassis, wheel, slip and contact lane agrees through tick 236 along and 141 across.
- **Where they part next.** Both rides part in the air, and the angular history's z goes first:
  - Along at **237**, four ticks after the wheels leave the bed's shoulder. Both rays drop at row
    233. ang_hist.z is 3.9101942 vs 3.9101896, and world_ang_vel[1] is off by an ulp.
  - Across at **142**, 24 ticks into its flight off the bed (rays off from ~118). ang_hist.z is
    0.14096615 vs 0.14065893.
- **The commit table above.** Each row is a build of that commit with this slot's kit.toml, iso and
  context, running the same test (detached scratch worktree, own target dir).

## Not verified / not 1:1

- **The airborne partings (along 237, across 142) are not looked into.** They are the angular law in
  the air: the same lane S180 (roll's tick-67 roll rate, ang_hist.z) and S179 (the first airborne
  tick) chase. When those land, these two rides are more cases for them. No new task is filed, to
  avoid a duplicate.
- **Not Ee-stepped.** The brief asked for substep-by-substep against the s89-along image. The
  ride now agrees on every word through 236 ticks, so tick 2 had nothing left to step.
- **Which of the two fixes moved which word** (vel, ang_hist, vel_hist.y at tick 2) was not split
  further. Each fix alone still parts at tick 2, and the two together do not.
- **The first tick's own controls** (`held_controls`) were not read out for these states. Passing
  `None` gave the same result, so either they match the next pad or tick 1 does not read them here.
- **Bisect range.** The first-parent bisect between the S114 merge and S138 assumed the S114 merge
  as the old end. I then probed that commit directly (tick 31), so every merge after it already had
  the fix. Merges before S138 were not all built, only the bisect's six.

## Next

Done. S179/S180 (the airborne angular law) are where S89's rides part next.
