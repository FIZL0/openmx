# A hierarchy part's body on a tick its toy is not drawn — FINAL

## The original

- The kinematic body's velocity virtual (`0x001dd7b0`) is `(node(+0x254)+0xb0 − body+0x310) × app+0x348`:
  the node's world position *now* against the translation row of `+0x2e0`, the matrix the body kept from
  the update before last. The spin (`0x001dd850` → `0x001dd8c0`) is the same pair: the node's live rows
  against `+0x2f0`/`+0x300`. Only the body's update (`0x001dd9b0`) writes `+0x2e0`, and it runs past the
  toy's visibility gate (S167).
- So on a tick the toy is passed over, its nodes stand where its last step put them and the body still
  holds the matrix from before that step: a part answers its last step's motion, scaled by this tick's
  `1/dt`. Not zero.

## The port

- `Toys::frame`, passed-over branch: the "before" pose is the joints at `Clock::prev` (the clock the last
  step started from; `Player::advance` sets it), sampled with a copy of the hints (`Toys::worlds_at`).
  Still the identity while the bodies have had fewer than two updates (S167), and still zero for a clip
  that has finished.
- No new state: `prev` was already hashed and sent.

## The measurement

`crates/game-render/tests/toy_parts.rs`, `*_answer_their_last_step_when_not_drawn`: S133's harness with
the toys drawn five ticks in seven. On an undrawn tick the original runs nothing of the toy (no
`motion_advance`, no `transform_rebuild`, no body update) and the port's toy is not seen. Every part's
pose, velocity and spin are the body's two virtuals', bit for bit: free10's shovel over 540 ticks, nat11's
16 gondola parts over 5,467 ticks (1,562 undrawn, every one moving). Before the fix the first undrawn
tick fails (nat11 gondola_01 part 0 velocity: port zero, original (−10.33, 1.761, −15.50) ft/s; free10
shovel_01 part 1 spin x 0.01444).

The S133 states `s133-free10.ee.bin`/`s133-nat11.ee.bin` were gone from every context and kit copy save;
they were rebuilt with S133's `boot.py` on kit copy 3 (slot 8, scratch), `Free10 2 5` and `Nat11 1 1`,
into this worktree's `context/states/`. S133's own suite passes on them.

## Not verified / not 1:1

- Not run live in PCSX2: the gate is skipped by hand in the interpreter (as S133's harness steps the toy by
  hand), not by the camera looking away. S167 traced the gate to the whole update, body included.
- A clip that finished on the step before an undrawn tick: the original would answer that finishing step's
  motion; the port gives zero (its clock is reset to zero by then, and `worlds_now` already samples time
  zero for a finished clip). No hierarchy toy on the disc was seen with a non-looping clip.
