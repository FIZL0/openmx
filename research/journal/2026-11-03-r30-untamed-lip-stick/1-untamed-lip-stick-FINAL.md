# 1 — untamed lip stick (FINAL)

Task R30. 2026-11-03.

## What I did

- I followed the bike reader back to its caller in the Untamed decompile (`context/untamed/decomp/game.c`)
  and the ELF (`llvm-objdump`), and read the bindings off `context/untamed/ram/nat_or.bin` with a
  throwaway Python reader. The addresses and offsets are in research/oracle.txt `# R30`.
  - The reader is reached only through a vtable. Its thunk sits in the vtable of the bike's
    input-listener sub-object, in slot `+0x20`.
  - A search for calls through a listener's slot `+0x20` found the input object's per-frame update.
- **`tests/untamed_reworked.rs`**: new test `untamed_lip_stick_is_the_limited_stick`.
  - It runs the input object's update on the image, on a stand-in input object that holds device 0 and
    one hooked listener, and reads the record the listener is handed.
  - With device 0's axes at (0.3, −0.7, 0.5, 0.9), words 0xdf..0xe4 are 0.3, −0.7, 0.5, 0.9, 0.3, −0.7.
    The lip's pair equals the limited pair.
  - With device 1 standing in as the second device, at (−0.95, 0.2): 0xdf becomes −0.95, and the lip's
    words stay at 0.3 and −0.7.
  - It also checks that the image has no second device for either player.
- **`handling::untamed`**: doc comments only. The module doc, `FlipIn::lip_y`, `SpinIn::lip_x`,
  `ride_backflip` and `ride_spin` now say what fills the two pairs and why the step's stick is right. The
  module doc used to say "the lip reads the stick before the limiter". That was MD93a's guess, and it is
  replaced.

## What the original does

- **The input object** (one per player) builds the pad record on its stack each frame and hands it to
  every listener on its list. The bike is one of those listeners.
  - Word 0 is the player's index.
  - Then come 0x20 button entries from the devices, and 5 analogue entries. Those 5 are flick measures
    over each axis's 32-sample history, and they are what the holds read.
  - Then the seven stick words.
- **The stick words** are each the device's axis value for the axis that one action is bound to:
  - 0xdf/0xe0 (the bike's limited `+0x1298`/`+0x129c`) are actions 0x52/0x51;
  - 0xe3/0xe4 (the lip's `+0x12bc`/`+0x12c4`) are 0x54/0x55;
  - 0xe1/0xe2 are 0x57/0x56;
  - 0xe5 (`+0x12b4`) is 0x53.
- **The shipped bindings** (devices 0 and 1 alike) put 0x52 and 0x54 on axis 0, 0x51 and 0x55 on axis 1,
  and 0x57/0x56 on axes 2/3. 0x53 is unbound.
  - So the lip's stick and the limited one are the same two numbers.
  - Neither has been through any limiter: the value is the device's own axis value, after its deadzone
    remap.
  - The 0x54/0x55 bindings are the same ones the spin and flip holds use (MD93f), on the stick words
    rather than the flick entries.
- **The one difference: a second device.** A player's input object also holds controls entry n + 2.
  Where that device's raw axis 0 or 1 is pushed further than the pad's, it replaces 0xdf/0xe0, but not the
  lip's words. On the image both of those entries are null.
- **So the port is right as it is.** On Untamed's handling the step's stick is `held.stick` from MX vs.
  ATV's input limiter. That is the pad's stick with the d-pad applied, not squared (the limiter walks the
  lean separately). That is what both pairs hold in the original, and the bike's reader then overrides both
  pairs with the d-pad, as the port does. No code change.

## Checks

- `cargo test -q -p game-sim --test untamed_reworked`: 12 passed, the new test among them. It ran on the
  image and did not skip.
- `./kit audit`: see the commit message.

## Not verified / not 1:1

- **The second device's override is not ported.** It needs a second input device per player, which the
  port does not have and which no shipped setup creates. I did not trace what fills controls entries 2 and
  3.
- **Remapped bindings were not traced.** If the options screen can bind 0x54/0x55 apart from 0x52/0x51,
  the two pairs would read different axes, and the port feeds both from one stick.
- **The device's deadzone remap is not compared** with the port's pad read. That is outside this task.
- **Word 0xe5 (`+0x12b4`) reads a word just before the device's axis entries**, because 0x53 is unbound.
  I did not check what that word holds.
- **The test's input object is a stand-in.** Only the fields the update reads are set (devices, listener
  list, enable). Its sample history starts at zero, so the analogue entries were not checked.
