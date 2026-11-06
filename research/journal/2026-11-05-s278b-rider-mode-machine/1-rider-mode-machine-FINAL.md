# 1 — rider mode machine

Task S278b. 2026-11-05.

## What I did

- `crates/game-sim/src/handling/rider.rs`: `modes` (the whole machine, one frame), built from
  `choose`, `switch`, `place_parts`, `holds`, `lean`, `follow` and `frame_end`. `Call` and the `Rider`
  trait (`node::Ram` plus `call`) stand for the calls the machine makes outside its own words. The
  wreck arm is one call, `Call::WreckArm`, and `modes` follows it with S278's `clear_all`. Constants
  are named for the rider's and the record's words.
- `handling/anim.rs` `turn_node` and `chassis.rs` `quat_mul` are now `pub(crate)` so the respawn's
  node turn and the lean's pose product reuse them. Nothing else in them changed.
- `crates/game-sim/tests/mxvatv_rider_modes.rs` (new, kept out of the 12k-line ride test so other
  branches don't conflict): the oracle below.
- research/oracle.txt `# S278b`: the words, the stretches, the outside calls and their arguments.
- Split: the riding reset (S279), the wreck arm's body with the ground ask, i.e. its nearby gather and ground query (S281), the pose and settle
  virtuals (S280) and the wreck entry (S282). Each is a function of its own, about as big as this
  machine (the wreck arm alone is about 1 100 instructions with eight callees). The task asks for
  "the wreck entry, the riding reset … the ground ask". This task ports the machine that calls them,
  and wires each call so a later port can serve it. Porting all of them in one task would have meant
  no oracle for any of them.

## What the original does

`./kit fn 0x004ef320 -p atv` runs once per rider frame (`0x004f1790`). Read with `./kit dis`: Ghidra
calls `0x004833c0` non-returning and so drops the wreck arm's tail, the clear loop and the frame end.

- **Choose** (`0x004ef33c`): a respawn ask on the record (+0xc) sets mode 0 and the respawn count
  (+0x8f8) to 3, or to 1 when +0x3a4 is neither 0 nor 1. Otherwise a wreck kind (+0x138) gives 2, a
  +0x86d byte under 46 gives 3, the record's hold (+0x170) or +0x8ac gives 4, and anything else gives 1.
- **Switch** (`0x004ef3e0`): a positive respawn count respawns (record pose onto the main node, then
  its turn, +0x38c 0, +0x258 1, pose, settle, place the pose list), counts down, and sets acted to 0.
  Otherwise, against the acted mode (+0x36c): into 4 places the list, draws rand × 2⁻³¹ × 0.5 into
  +0x8a8, and resets. Mode 2 newly entered, or again on kind 13 that wasn't 13 last frame, makes the
  wreck entry. Leaving 2 clears four words, respawns and sets the count to 2 (1 if +0x3a4 says so).
  Into 3 places the list. Into 1 resets. Any change stores the mode as acted.
- **Holds** (`0x004ef750`), when +0x874 is −1, the mode is 1, 3 or 4 and the byte is ≥ 46: the
  orientation's up y. Riding (no +0x130, and +0x19c set or +0x1a8 negative or within ±1.5 / +0x894),
  both holds go on or off by +0x12c, with the fades 0 or 0.5. Away with byte +0x86e 0x38 and +0x1a8
  under +0x8a0 and up under 0.9, one hand reaches (by +0x1a0 over 0.7 or under 0.3) and the other
  holds full. Otherwise both hold and the two fades run in by dt over 2 s.
- **Lean** (`0x004efc60`): +0x600's xyz times a 1..2 scale from +0x1a8/+0x894 into +0x5f0. Outside the
  wreck, the main node's +0x190 is the record's quaternion times the reference pose.
- **Follow** (`0x004efd5c`): +0x258/+0x25c bookkeeping, and with the follow's +0x28 at 2 a re-pose
  when +0x388 was clear.
- The wreck arm body, then the clear loop (S278), then the **frame end** (`0x004f02b0`): the record's
  velocities (×0.8), positions less the turned offsets, the five pointer pairs, +0x60..+0x9c, and the
  kind kept.

## How it was proved

- **Oracle, `the_riders_mode_machine_is_the_originals`** (`tests/mxvatv_rider_modes.rs`): 60 000
  frames of state 4's rider. Each draws every word the machine reads: respawn ask, kind (13 among
  them), hold, ridden, acted mode, kind kept, respawn count (−1 too), +0x3a4, the two bytes on and
  around their limits, the fades, the follow's words, orientations, points, the pointer pairs (none,
  one, both, misaligned), the lean inputs at their limits, signed zeros, and the pose list count
  (17, 0 or −1). The original runs whole in the interpreter. Its calls are hooked to log and check
  their a0/a1/f12; rand answers drawn words and the port gets the same ones. The wreck arm's first pc
  logs and resumes at the clear loop. Every word either side writes, outside the stack, is equal, and
  the call lists are equal. Every switch arm, mode and holds arm is taken ≥ 20 times; the rarest holds
  arm, `L0l1R1r0` (left reaches off, right holds), about 56 times.
- A mutant (the frame end's 0.8 one ulp off) fails on case 0. The tallies were read off a failing
  run, because a passing `./kit check` prints none.
- `./kit check --all` passes.

## Not verified / not 1:1

- **The callees are not ported**: the riding reset, the wreck entry, pose, settle, the parts' on/off
  virtual and the wreck arm's body (with the ground ask) are calls through `Rider`, logged in the
  oracle and not run. Filed as S279–S282. Until they are ported, `modes` is not wired into any ride
  path. Nothing calls it outside the test.
- **No live capture or replay**: the oracle runs from state 4's RAM with drawn words, not from a ridden
  wreck. With the callees stubbed, a C15 replay would not show `modes` doing its job, so no replay was
  run.
- **One object**: holds, lean and the switch's virtual are proved on state 4's own parts (one switch
  function, `0x0049bd30`), holder and pose list. Other riders' part vtables were not looked at.
- **`+0x100` is never null in the oracle**: the respawn's turned point reads through it unchecked. A
  null would read address 0 on the PS2 and in the port alike, which is not tested.
- **Fade's range test left out**: the fade's lerp tests its range (2 − 0) with c.eq before dividing.
  The range is a constant, so that branch can't be reached and the port doesn't have it.

## Next

S279–S282, the callees, then wire `modes` into the rider's frame beside S278a's world.
