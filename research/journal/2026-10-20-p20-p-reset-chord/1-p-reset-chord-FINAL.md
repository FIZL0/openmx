# 1 — p reset chord (FINAL)

Task P20. 2026-10-20.

## What I did

- `Controls::from_pad` (`crates/game-sim/src/ride.rs`): `respawn` is now
  `pressed(L1) && held(R1) || pressed(R1) && held(L1)`, replacing `held(L3) && pressed(R3)`. L3 (balance)
  and R3 (reverse) keep their bindings.
- The shell (`crates/openmx/src/main.rs`): the module doc and `--help` name the new chord, and the
  keyboard reset is now A+D. The keyboard already mapped A to L1 and D to R1 in `pad.rs`, so `pad.rs` did
  not change. The camera still steps on L1, but not when L1 is pressed with R1 already down, because
  that press is the second half of the chord.
- `research/oracle.txt`: the binding map's layout, its two readers, and the shipped values.
- `crates/game-sim/tests/platforms.rs`: the canonical run's inputs are random button words, so any
  change to which buttons make the chord moves its vectors. I regenerated them with
  `print_the_vectors` and spliced in the seven simulation consts one by one.
  `FINAL_DUMP_LEAVES` and `EVERY_LEAF_HASH` did not move, so the state type did not change, and
  `HUNDRED_TICKS` did not either.

## What the original does

`./kit fn 0x27bef0` (the hand reset) is an **event handler**. It does not poll. `./kit fn 0x11ce10` asks
whether this pad event (kind 2, a press, judging by recording2) is the control bound to a command, as
either its first binding (`./kit fn 0x11d080`) or its second (`./kit fn 0x11d0e0`). The input manager's
slot +0x1c asks whether a command is currently held on that pad. The reset fires when **0x4c is pressed
while 0x4d is held, or 0x4d is pressed while 0x4c is held**. Its other gates are unchanged from S29: no
AI route, the +0x14d0 latch, the mode, the reason not being 13, and the owner's pad.

The binding map is `*(app_ptr)+0x104` (the input manager) `+0x648`. It holds 0x4e commands per pad:
first bindings at +0x138 + pad·0x138, second bindings at +0x3a8 + pad·0x138, one control index per
command, -1 when unbound. The control index is the port's `input::Control` order; the d-pad commands 0–3
read Left, Right, Up, Down, which is a sanity check on that. From recording2's save state, on both pads:

| command | first | second | what it is |
| --- | --- | --- | --- |
| 0x11, 0x12, 0x13 | Circle, Triangle, R1 | -1 | the trick decoder's three buttons, in `Controls::trick`'s order |
| 0x4c | L1 | -1 | the reset, first half |
| 0x4d | R1 | -1 | the reset, second half |
| 4 | Start | -1 | `./kit fn 0x206e90`, the pause |

So the original's R1 is both a trick button and half of the reset. The brief's condition "bind the
original's chord if they do not collide" holds: the only overlap is the original's own, and the port now
has the same one. The other two readers of 0x4c/0x4d (`./kit fn 0x355df0` and the function at
0x391830) are the same chord on other actor kinds and are not ported here.

## How it was proved

`the_reset_chord_is_the_originals` in `crates/game-sim/tests/ride.rs`, in four parts:

1. **The table.** It reads `context/states/recording2.p2m2_SaveState.eeMemory.bin` (from
   `./kit state ram`), follows the map's pointer chain, and asserts the table above for both pads. If
   the image is missing it prints a line and skips this part.
2. **The edge.** Synthetic pads through the ported input layer: L1 then R1 fires once, on R1's frame. R1
   then L1 fires likewise. Both down on one frame fires once. Neither button alone fires, and L3+R3 no
   longer fires.
3. **The human's presses.** I replayed all 1894 rows of recording2's sixteen `ctlN.held` columns through
   `Manager::tick` and `from_pad`. The chord fires on **row 1212 only**, which is the row where
   `wreck_reason` goes from 12 to 0. It does not fire on rows 927–994 or 1372–1414, where R1 was held
   alone while riding and the original did not reset.
4. **Riding.** A rider rolls 90 ticks on `ride_held`'s bike and ground with no wreck. L1 is held, then
   R1 goes down: `respawned` is set on exactly R1's tick, no wreck is entered, and the bike is back
   within half a foot of the start line.

`./kit check`: passed, after the platforms vectors were regenerated.

## Not verified / not 1:1

- **Event kind 2 is read as "press" from the recording, not from the event producer.** I did not read
  the code that queues pad events. The capture shows the reset on the tick R1 went down, and none on
  ticks where both were merely held, which is what a press event gives.
- **Both buttons pressed on the same frame.** The original queues two press events. Whether the first
  one sees the other as held depends on whether the held state is updated before the events are
  dispatched, and I did not read that. The port fires once on that frame. Nobody did this in either
  recording.
- **The binding table comes from one save state.** It is the shipped default table as the human's
  session had it. I did not read the function that writes the default table, so I have not shown that
  an options-menu remap cannot change it. The port has no remapping, so it binds the default.
- **The chord on a riding rider is proved on the port, not on the original.** The original's gate has
  no wreck test (S29), but nobody pressed L1+R1 while riding in either recording, so there is no capture
  of it.
- **The camera button.** The shell keeps P14's L1 for the camera. An L1 pressed first still steps the
  camera on its own tick before R1 completes the chord. Which button the original's camera uses is still
  unknown (P14's journal), so I can't say whether the original has the same overlap.
- **The other two readers of the chord** (other actor kinds' resets) are not ported.
- **The regenerated platforms vectors were confirmed on x86_64 only.** Cross-architecture runs need an
  upstream toolchain that is not on this machine.

## Next

Nothing in P20 is left open. The camera button (above) belongs with P14's open question, if anyone
picks that up.
