# 1 — first emitter kind (FINAL)

Task F57. 2026-11-12.

## What I did

- Ported the part walk's first particle emitter kind to `crates/game-sim/src/handling/emitter.rs`. It
  has `step` (the listener thunk's target), `gate` (its first call), `count`, `base`, `look`, `size`,
  and private `place`/`idle`/`wait`/`span`/`fixed`/`in_frame`/`turned`/`draw`. Everything goes through
  ps2-float (`#![deny(clippy::float_arithmetic)]`). Words are kept raw where the original only moves
  them: the `w` lanes, and the spread table's words.
- Hand-outs go through `Call` on the `Emitters` trait: `rand`, the listener notice, the object's
  light/douse virtuals, the other kind's step, the owner's place and virtual, the follow-up, the world
  probe, settle and bind. The node calls (position, rotate, update) are F43's `node::` ports on the same RAM.
- `tests/mxvatv_bike_ride.rs`:
  - PartWalk now runs the name "emitter" through `emitter::step`. It implements `node::Ram` and
    `emitter::Emitters`, and each hand-out runs the original's function in the interpreter.
  - `walk_served` books it as "emitter steps" and asserts that the interpreted "the update's emitter"
    no longer runs.
  - New oracle test `the_emitter_step_is_the_originals`.
- research/oracle.txt `# F57`: every address.
- Split off F57a: the other kind's step (the definition's option bit 0x40), still handed out.

## What the original does

- The emitter object's listener thunk (`./kit dis 0x00511c10 0x00511c18 -p atv`) jumps to the step
  (`./kit fn 0x0050ceb0`) with the object at a0−12.
- The gate (`./kit fn 0x0050a040`) does this, in order:
  - a pause bit;
  - the spot carried to its node plus an offset, turned if flagged;
  - a notice when +8 is set;
  - a skip bit and a hold clock;
  - a "held" global;
  - three timed modes on the definition's +0x58: on/off clocks, spans drawn by rand or fixed,
    light/douse on the object.
- The step then:
  - hands the whole frame to the other kind if option bit 0x40 is set;
  - gives up if the node owner's frame id is not this frame's;
  - takes the distance moved and the speed (dist/dt), and stops below the slowest speed;
  - draws rand twice: the count (`./kit fn 0x0050e6a0`, by distance/speed when fast, otherwise a
    draw between per-second counts, with a carried fraction) and the walk over the 600-float spread
    table (5 thresholds pick the direction and stride);
  - for each particle, while the pool has room, walks the table for an offset direction, then the
    look (`0x0050eab0`), life and size (`0x0050e980`), flags, fades and spin;
  - optionally settles each particle on the world and binds it.
- Before the loop, a definition with +0x168 > 0 places the owner, calls its virtual and the
  follow-up, and probes the world to set +0xc8.

## How it was proved

- **Every-port replays.** C15's flip-1 and ride-2 (`flip_replays_with_every_port_at_once`,
  `ride_replays_with_every_port_at_once`) pass with the emitter in Rust: the bike matches every frame
  and the RAM matches outside the known race-progress/cue words.
  - flip-1: 5115 emitter steps ran in Rust. Still interpreted: 462 932 instructions of the other kind
    (128 of its steps drew rand) and 32 400 of rand's.
  - ride-2: 5049 steps in Rust.
- **Function oracle.** `the_emitter_step_is_the_originals` runs 20 000 generated emitters through the
  original's step and through `emitter::step`, with hooks on every hand-out that react the same way
  on both sides. It checks the calls in the same order with the same arguments, the same answer, the
  same scratch RAM and the same globals.

## Not verified / not 1:1

- **The other kind's step:** still interpreted. Filed as F57a.
- **Every hand-out:** still interpreted in the replays (rand, notice, light/douse, other kind, place,
  owner virtual, follow-up, probe, settle, bind). In the oracle they are stand-ins, so the oracle only
  proves the step calls them as the original does.
- **Branches no C15 frame reaches:** proved only by the oracle, not by a replay. These are gate modes
  1/2/3, the notice, the +0x168 place block, settle, bind, spin and the turned-quaternion paths.
- **The probe's stack slot:** in the replay port it is placed at sp−0x1000, not in the original's own
  frame. The slot's word is seeded and read as the original does, but the word sits at another
  stack address. The replays compare RAM outside the stack, so this cannot show up there.
- **Quad stores at unaligned addresses:** the port writes particle quads at the exact address,
  while the original's `lq`/`sq` round it down to 16 bytes. This only matters for an unaligned
  particle. The game's own pools are aligned (the replays match RAM), and so are the oracle's.
- **Not read for this task:** the PC exe.

## Next

F57a ports the other kind's step.
