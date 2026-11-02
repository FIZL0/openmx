# 4 — riding body ragdoll, reopened by S201 landing (READY)

Task S90. 2026-11-02.

## What I did

- Found S90 open again. 68f22c34 ("reopen S90, S201 landed") was mechanical: the block line carried
  `(after S201)`, so the runner reopened the task when S201 merged (70a42e12). Entry 3 meant to block without
  `--after`, but the line on main still ended up with `(after S201)`.
- Looked for a new word from the user and found none. Nothing in plan/HUMAN.md, plan/TODO.md, or the commit
  log since 2026-10-28 lifts the 2026-10-24 ruling. `riding_ragdoll_on` in crates/openmx/src/main.rs
  is still opt-in (`OPENMX_RIDING_RAGDOLL=1`), and `riding_ragdoll_is_off_by_default` still guards it with
  "Do not flip this default from a task". V63 and P133 are still `[~]` on the same ruling.
- Blocked again, changed no code, and gave no `--after`.

## Where the work stands

- The sim half is done. S135 proved the riding spring against S74's nine put-backs and plain riding.
  S201 matched the entry ticks: the S74 entries' worst error is 0.044 ft (mean 0.0043), and settled
  ticks are within 0.149 ft (mean 0.0041). The 0.3 ft pop that entry 3 called a precondition is gone.
- The only part left is the draw: make the riding ragdoll the default drawn body (flip
  `riding_ragdoll_on`, replace its guard test, then V63 deletes the stand-in hold). Only the user can
  allow that.

## Not verified / not 1:1

- The drawn riding rider is still the clip pose plus hand IK, by the user's ruling.

## Next

The user decides. If they say yes, the change is small: flip the default, rewrite the guard test, and
watch a put-back in the shell.
