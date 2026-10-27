# 2 — riding body ragdoll, handed out again (READY)

Task S90. 2026-10-27.

## What I did

- Found S90 open (`[ ]`) on main even though entry 1 blocked it (4c5a36fe set `[~]`, and b42635de still
  has it). A later merge of plan/TODO.md dropped the mark, so the runner handed the task out again. The user
  has not changed the ruling. I found no later note (plan/, HUMAN.md, or any journal) that lets the drawn
  riding rider become the riding ragdoll. The newest word is still 2026-10-24: "we're back on ragdoll
  model when riding the bike, fix it and make sure we dont regress". V63 and P133 are still blocked on it.
- Blocked the task again and changed no code. I did not do the sim-only half here, because that half is
  its own task, **S135**. Doing it under S90 would run the same work twice in two branches.
- I checked how much of the sim half other tasks already cover, so whoever picks up S135 knows where it starts:
  - S110 (219c7a2d): the riding ticks before run2's wreck are the original's bit for bit, against S109's
    capture (`ragdoll_riding::Riding`, now `Rider::riding`).
  - S160 (086d8c4c): the riding entry after every 2p put-back is bit for bit for ten rows past the put-back,
    both riders.
  - What is left for S135 is context/s74's node rows from T+2 through the 3 s ghost, plus the plain-riding
    stretch. Those captures hold node and joint rows and the request block, but no RAM image per
    put-back, so the riding bodies' starting state has to come from the held tick's nodes, which equal
    the pose (S74).

## What the original does

Same as entry 1. Not studied further.

## How it was proved

Nothing new was proved. No code changed.

## Not verified / not 1:1

- The whole of S90. The drawn riding rider stays the clip pose plus hand IK by the user's ruling of
  2026-10-24. The riding spring was not stepped against context/s74's node rows; that is S135.

## Next

Only the user can lift the ruling. If it is lifted, S135 comes first and the draw goes after it. If the
task is handed out again with no new ruling, the `[~]` mark has been lost again, not lifted.
