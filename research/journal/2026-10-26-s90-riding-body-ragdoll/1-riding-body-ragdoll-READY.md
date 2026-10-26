# 1 — riding body ragdoll (READY)

Task S90. 2026-10-26.

## What I did

- Blocked the task before writing any code. The brief asks for the shell's riding rider to be drawn
  from the ragdoll's riding spring rather than from the clip pose plus hand IK. That is exactly what the user has
  ruled out, twice: 2026-10-23 ("our guy is super floppy") and 2026-10-24, after S111 turned it back on
  ("we're back on ragdoll model when riding the bike, fix it and make sure we dont regress"). The
  riding ragdoll is opt-in only (`OPENMX_RIDING_RAGDOLL=1`, `riding_ragdoll_on` in
  crates/openmx/src/main.rs, guarded by the `riding_ragdoll_is_off_by_default` test). V63 and P133 are
  blocked on the same ruling, and only the user can lift it.
- Filed **S134**: the part of S90 that doesn't touch the draw, which is stepping `ragdoll_body::Actor`'s riding
  path and matching context/s74's node rows from T+2 on all nine put-backs, then plain riding. It is a
  game-sim test only, so it changes nothing on screen and can go ahead without the user's ruling.

## What the original does

Not studied in this session. From the brief: from the second tick after a put-back (and whenever riding),
the body is mode 1. Its 22 nodes are the ragdoll bones pulled toward the animation joints by the
riding pose spring, not the joints themselves.

## How it was proved

Nothing was proved. The task was blocked before any work started.

## Not verified / not 1:1

- The whole task. The riding spring was never stepped against context/s74's node rows, and the draw was not
  changed. Reason: the user's standing ruling that the riding rider is the clip pose by default. The sim
  proof moves to S134.

## Next

The user has to decide whether the drawn riding rider may become the riding ragdoll by default. Until
then S90 stays blocked. S134 can be done at any time.
