# 1 — second emitter kind (FINAL)

Task F59. 2026-11-13.

## What I did

- Ported the part walk's second particle emitter kind to a new module,
  `crates/game-sim/src/handling/ring.rs`. It has `step`, the thunk's target, and `aim`. Everything
  goes through ps2-float (`#![deny(clippy::float_arithmetic)]`).
  - It reuses F57's `emitter::{gate, count, base, look}`, which are the same functions in the
    original. `emitter::in_frame` and `turned` are now `pub(super)`; nothing else in `emitter.rs`
    changed.
  - It hands out the first kind's calls (`emitter::Call`: rand, the gate's, settle, bind). The two
    it adds go through a trait of their own, `ring::Rings`: the real-time clock read and `srand`. I
    used a separate trait so `emitter::Call` gains no variants, which keeps F57a's edits apart from
    this one.
- `tests/mxvatv_bike_ride.rs`:
  - PartWalk runs the leaf "second emitter" through `ring::step`. It also runs any effects item
    move that reaches the second kind's step, though none on C15 does.
  - PartWalk implements `Rings`. `srand` runs the original's. The clock panics: the original's read
    is a SIF RPC the interpreter cannot make, and no C15 frame reads it, because the image's seed is
    already set.
  - `walk_served` now books "second emitter steps". It asserts that the interpreted step no longer
    runs and that no step of this kind draws rand on C15.
  - New oracle test: `the_second_emitter_step_is_the_originals`.
- research/oracle.txt `# F59`: every address and clause.
- From opentennis: nothing. It has no particle emitters ported. The shape is copied from F57's
  `emitter.rs` and its oracle.

## What the original does

- The thunk (`./kit dis 0x004ffd20 0x004ffd30 -p atv`) jumps to the step
  (`./kit dis 0x004fbcc0 0x004fcbec -p atv --limit 2000`) with the object at a0−12. The object is
  the first kind's, with a ring after it: 120 direction quads at +0x140, the index +0x100, the
  stride +0x108 and the scale +0x134.
- The step does this, in order:
  - runs F57's gate and the node-owner frame check;
  - computes the move, speed and velocity; stops below the slowest speed;
  - draws rand twice, for the count and the walk, as the first kind does;
  - puts the spot back on its node position (`0x00194c20(node, 0, spot)`, called even with no
    node);
  - computes the base; with no velocity bit it adds only `x` of the base in the frame;
  - writes +0x110 = base + add;
  - aims the ring:
    - takes the angle about y of unit(+0x110) against (0,0,1), from a 30000-entry arc-cosine table
      and the sign of the cross product's y;
    - adds 3/8 of a turn and converts the angle to an index out of 120;
    - adds a third draw of rand times the stride;
    - the first time ever (seed word 0), it first seeds `srand` from the clock;
  - spawns each particle:
    - its position is the spot plus the next ring direction times the scale, plus F57's offset;
    - its velocity is the current ring direction plus the base and F57's push;
    - size, life, fade, look and fades come from the spread entries after.
- How it differs from the first kind:
  - the spawn point does not walk back along the move;
  - p+0x40 is not written;
  - the spin writes only +0x4c;
  - p+0x30 scales the particle's previous size, not its new one;
  - the spread centre is another quad.

## How it was proved

- **Function oracle.** `the_second_emitter_step_is_the_originals` runs 20 000 generated emitters
  through the original's step 0x004fbcc0 and through `ring::step`:
  - the emitters vary flags, clocks, modes, node or none, frame, turn, speeds and every definition
    option; the ring has random directions, index, stride 0–9 and scale; the seed is set or 0;
  - the image supplies the arc-cosine table, the spread table and its centre;
  - hooks react the same way on both sides: an LCG rand, clock bytes per case, `srand` reseeding the
    LCG, settle and bind;
  - it checks that the calls come in the same order with the same arguments, that the answer is the
    same, and that the scratch RAM and the globals (the seed among them) come out the same;
  - every case passed. Every kind of call was reached, including spawning frames with three draws
    and the clock-seeded path.
- **Every-port replays.** C15's flip-1 and ride-2 (`flip_replays_with_every_port_at_once`,
  `ride_replays_with_every_port_at_once`) with the second kind in Rust: both pass, RAM alike every frame. The gate run adds the no-rand assert.

## Not verified / not 1:1

- **The spawning loop and the ring's aim against a capture:** not proved by a replay. No C15 frame
  spawns with this kind; the replays assert that none draws rand. Only the oracle proves them, with
  stand-in hand-outs and generated emitters, against the image's tables.
- **The clock read:** never run. The original's is a SIF RPC to the CDVD real-time clock, which the
  interpreter cannot make. In the oracle it is a hook writing per-case bytes; the port only proves
  which bytes it reads and how it builds the seed (the original's 360 per hour, kept). In PartWalk
  it panics, because no C15 frame reaches it.
- **Every hand-out:** still interpreted in the replays (rand, the gate's notice and light/douse,
  settle, bind, srand). In the oracle they are stand-ins.
- **No node:** the original then reads the words at 0x60..0x68 from low RAM. The port reads the
  same words and the oracle covers it, but no real emitter is known to have no node.
- **Unaligned quad addresses:** the original's `lqc2`/`sq` round them down to 16 bytes; the port
  reads and writes the exact address. Emitters, rings and particles are aligned in the game's pools
  and in the oracle (F57 has the same note).
- **Not read for this task:** the PC exe.

## Next

None.
