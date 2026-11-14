# 1 — other emitter kind (FINAL)

Task F57a. 2026-11-14.

## What I did

- Ported the first emitter kind's other branch to `crates/game-sim/src/handling/emitter.rs` as
  `emitter::other`. `step` now calls it in place of handing out `Call::Other`, which is gone. Its small
  callees are ported with it:
  - the batch start (inline);
  - the batch's scales and pair of words (inline);
  - the light list lookup (`emitter::light`);
  - the batch's seat (`emitter::seat`);
  - the tint (private `tint`).
  It reuses F57's `count`, `base`, `look`, `size`, `in_frame` and `turned` unchanged. Their 0x40
  branches (count and size scaled by the batch's `+0x84`) first run here.
- The one call it hands out is the batch's send (580 bytes plus two callees): `Call::Send { batch, count }`.
- `tests/mxvatv_bike_ride.rs`:
  - PartWalk answers `Call::Send` with the original's send. It books "emitter other kind steps"
    (emitters whose definition has bit 0x40) and "emitter other kind send".
  - flip-1's assertion changed from "only the other kind's handed-on steps draw rand" to "nothing the
    part walk hands on draws rand". It now also asserts that a send happened.
  - F57's oracle keeps bit 0x40 clear: that branch is the new oracle's. Its `emit_react` answers
    "send" (it halves the batch count) and no longer answers "other".
  - New oracle test `the_other_emitter_kind_is_the_originals`.
- research/oracle.txt `# F57a`: every address and clause.
- opentennis has no particle emitters, so nothing was taken from it.

## What the original does

`./kit fn 0x0050db60` (2872 bytes; `./kit dis 0x0050db60 0x0050e698` for the float order). It always
answers 1. In order:

- **Early exits:** a global switch, then the pool's batch (`(*def)+0x174`). The batch is switched on
  here if it can be; with no batch, or one still off, the step ends. Then F57's node frame-id check.
- **Speed and draws:** the same move, speed and two rand draws as F57. The count uses F57's count
  function. The walk's start is `586 * u1`, where F57's is `(599 - 13·stride) * u1`.
- **Batch setup:** scales `32767/def+0x90..0x98`, two words from `def+0x70`, and a tint. The tint is
  the definition's own colour, or the scene's kind-4 light plus 0.8× its kind-2 light, clamped to 1,
  ×128. Then a fall `-32.2 × def+0x2c` and `+0x44 = -100`.
  - None of this depends on whether anything spawns: it runs even when the count is 0.
- **Spawn loop:** each particle is a 0x40-byte record in the batch. Its offset and push are
  `def+0xd0 + def+0xf0·e` and `def+0x130 + def+0x150·e`, with no centring or normalising, unlike F57.
  - The look is a word at `+0x38`.
  - Life and size go to `+0x2c` and `+0x24`.
  - A look rate goes to `+0x3c`.
  - A fade goes to `+0x30`.
  - Two coin flips go to `+0x34`.
- **After the loop:** the batch is seated (which seat holds the listener's word) and sent.

## How it was proved

- **Function oracle**, `the_other_emitter_kind_is_the_originals`: 20 000 generated emitters run
  through SLUS-21104's `0x0050db60` and through `emitter::other`. Only rand and the send are hooked;
  every other callee runs as the original's own code.
  - The inputs cover: batch absent, off, or startable; room 0–8; light kinds kept apart, found by
    scan, or missing; the own-tint and fall flags; stepped looks; both per-particle switches; node
    frame ids matching or not; listener seated or not, with up to 20 seats.
  - Calls, answer, scratch RAM and globals were all identical.
  - The tally (seen with a temporary panic, then removed): 5047 stopped before drawing, 6723 drew
    and spawned nothing (count ≤ 0), 2584 sent with no room, and 5646 spawned and sent.
- **Every-port replays.** C15's `flip_replays_with_every_port_at_once` and
  `ride_replays_with_every_port_at_once` pass with the branch in Rust: the bike matches every frame
  and the RAM matches outside the known words.
  - flip-1: 1860 other-kind steps in Rust; the send's interpreted instructions total 118 912.
  - ride-2: 1836 other-kind steps; sends 401 018 instructions.
  - On flip-1, no handed-on call moves rand any more.

## Not verified / not 1:1

- **The batch's send** (`0x00514e40` and its callees `0x00514ac0` and `0x00514910`): still
  interpreted in the replays, and a stand-in in the oracle. The oracle proves only that it is called
  with the same arguments, in the same order. Not filed: it touches no simulation state the bike reads,
  so it is presentation for later.
- **Branches the replays may not reach**, proved by the oracle alone: own tint (`def+0x28` bit
  0x800), fall (bit 0x20000), the two per-particle switches (`0x00542d70` and `0x00542d78`, both 0 in
  the state-4 image), the light scan, and the batch start. I did not count which branches C15 frames
  reach.
- **Quad stores at unaligned addresses:** same caveat as F57. The port writes the particle's quads at
  the exact address, while `sq` rounds down. Batch records are 0x40-aligned in the game (the replays
  match RAM) and in the oracle.
- **Not read for this task:** the PC exe (`-o pc`). The PS2 ELF was read instruction by instruction,
  and the oracle proves the result bit for bit.

## Next

None for F57a. The send (`0x00514e40`) is the next step if the particle batches ever need porting.
