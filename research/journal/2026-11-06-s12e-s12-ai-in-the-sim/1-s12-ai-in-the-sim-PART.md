# 1 — s12 ai in the sim (PART)

Task S12e. 2026-11-06.

## What I did

- Nothing ported yet. S12e has the Sim step the AI bikes "driven by the route/avoidance/trick
  controllers". Two of those three are not on main or on this branch (based on 935f08b2):
  - S12c (avoidance pass) is only on `origin/task/S12c` (331bcd80) and still open in the plan.
  - S12d (trick decision, ai_ask_for, ai_gesture_gate) is on `origin/task/S12d`. A merge of it exists
    (73c6d81a) but no branch contains it, so it never reached main. The plan still shows it open.
- Both branches change `crates/game-sim/src/ai.rs` and `crates/game-sim/tests/ai.rs`, the same files
  S12e has to build on.
- Blocked with `--after S12c,S12d`. Why not do a route-only version first: the trick roll draws rand()
  every eligible air frame, and avoidance moves the bikes. Without them, the proof the task asks for
  (a free09 4-racer replay of rand_state and the AI bodies) cannot match. A route-only version would
  also be a second copy of the controller wiring, and it would conflict with both branches.

## What the original does

Not studied this session.

## How it was proved

Not proved: no code in this session.

## Not verified / not 1:1

- Everything in the task: the AI bikes stepped in the Sim, actor-list rand() interleave and the free09
  4-racer replay. None of it was attempted, because S12c and S12d have not landed.

## Next

Once S12c and S12d are on main: read both controllers' entry points in `ai.rs`. Then spawn the
roster's riders as A-physics bikes in `tick.rs` in actor-list order, and record a free09 4-racer
capture (rand_state plus each AI body) to replay in `tests/ai.rs`.
