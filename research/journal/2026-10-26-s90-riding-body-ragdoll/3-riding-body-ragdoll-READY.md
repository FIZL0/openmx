# 3 — riding body ragdoll, reopened by S135 landing (READY)

Task S90. 2026-10-28.

## What I did

- Found S90 open again. b71e0504 ("reopen S90") was mechanical: entry 2's block line said `(after S135)`,
  so once S135 merged (9ad339ba) the runner reopened the task. The user's ruling was not lifted. I found no
  newer word in plan/HUMAN.md, plan/TODO.md, or the commit log since 2026-10-25. The
  default in crates/openmx/src/main.rs (`riding_ragdoll_on`, guarded by `riding_ragdoll_is_off_by_default`)
  still says the default is not to be flipped from a task, and V63 and P133 are still `[~]` on the same
  ruling.
- Blocked again with no code changed, and **without** `--after` this time, so the task cannot reopen on
  its own. Only the user can reopen it.
- Where the sim half stands now: S135 (5efec367, tests/riding_spring.rs) proved the riding spring
  against context/s74's nine put-backs and plain riding. That is 2,297 settled ticks within
  0.157 ft / 0.213 per lane, mean 0.0043 ft. The entry ticks T+2..T+5 are bounded at 0.3 ft but not
  matched, which is S201 (S135's renumbered S200). With S110 and S160 also landed, the only part of S90
  left is the draw itself: making the riding ragdoll the default drawn body in game-render/rider.rs and
  openmx main.rs.

## What the original does

See S135's journal. Nothing new was studied here.

## How it was proved

Nothing new was proved. No code changed.

## Not verified / not 1:1

- The draw half of S90. The drawn riding rider stays the clip pose plus hand IK (user, 2026-10-24;
  `OPENMX_RIDING_RAGDOLL=1` stays opt-in). The sim match is S135's, with its entry-tick gap (S201).

## Next

The user decides whether the riding ragdoll becomes the default drawn rider. If they say yes: flip
`riding_ragdoll_on`'s default, replace the `riding_ragdoll_is_off_by_default` guard, and fix S201's entry
ticks first so the put-back has no 0.3 ft pop.
