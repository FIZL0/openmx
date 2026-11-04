# 1 — untamed bike step (FINAL)

Task MD93f. 2026-11-03.

## What I did

- I read the input reader, the trick record fill, the binding lookup and the stunt table in the Untamed
  decompile (`context/untamed/decomp/game.c`) and with `./kit dis` on an overlay kit.toml in the scratchpad.
  I read the defaults off `context/untamed/ram/nat_or.bin` with a throwaway Python reader.
  - The addresses and offsets are in research/oracle.txt `# MD93f`.
- **`handling::untamed`**: no change in behaviour. The doc comments on `ride_backflip`, `ride_spin` and
  `FlipIn::lip_y` now give the traced reason for each input in place of "stand-in".
- **`tests/untamed_reworked.rs`**: new test `untamed_ride_input_is_the_shipped_bindings`. It runs on the
  image, calls the game's binding lookup for devices 0 and 1, and checks four things:
  - trick actions 0x11..0x14 are Circle, Triangle, Select and R1, which is MX Unleashed's preset-0
    TRICK_1, TRICK_2, Select and TRICK_3;
  - the spin hold and the flip hold are analogue controls 0 and 1;
  - the 360's bound-control action is unbound;
  - the game's "is the backflip bound" returns 0 for every style, on the Free Ride bike's stunt table,
    whose stunt 49 is "backflip".

## What the original does

- **The trick actions.** The reader fills a pad record whose bytes are the four trick actions, each with
  the value of its bound control > 0.
  - **Correction to MD93a:** they are not debounced.
  - **Correction to MD93a:** the record's first word is not the device. It is the binding style, which is
    always 3 (classic).
  - The lookups resolve against device 0's map. Devices 0 and 1 ship identical maps: Circle, Triangle,
    Select, R1.
  - The control numbering is MX Unleashed's `input::Control` exactly.
  - Under the port's preset 0 (TRICK_1..3 = Circle, Triangle, R1), action 0x14 (the arming one) is the
    port's `trick[2]`.
  - Action 0x13 is Select. The port has no trick command there (Select is a camera button), so it reads
    unpressed. It only matters for "any trick held", and there only when Select is pressed in the air.
- **The holds.** The flip hold (action 0x55) and the spin hold (0x54) default to analogue entries 0x21 and
  0x20. SLUS-21104's reader reads those same two entries for its hold and spin hold. So the port's flick
  and side flick, which already stand for SLUS-21104's holds, are the same controls.
- **The backflip binding.** The table at bike `+0x1510` is the driver's stunt table, built from the stunts
  file.
  - Each stunt has DefaultKeyMapping{Classic, Style1, Style2, Style3} combos.
  - The header keeps one byte per style: whether the backflip's mapping in that style is non-empty.
  - "Bound" reads the byte for the record's style, which is always classic.
  - On the image all four bytes are 0, and stunt 49's combo is empty.
  - So the shipped game never binds the backflip, `button: None` is the original, and the 360 is the same.
- **The lip's stick.** Inside the bike the lip's stick (`+0x12c4`) and the limited one (`+0x129c`) are
  plain copies of two different pad words (0xe4 and 0xe0). The d-pad overrides both the same way. I did
  not find what writes those words.
  - The pad device's own poll lays its axes out differently.
  - The bike's reader is reached only through a vtable, so `./kit xref` does not show its caller.
  - The port keeps feeding both from the step's stick.

## Checks

- `./kit check -- -p game-sim --test untamed_reworked`: 10 tests passed.
- `cargo test -p game-sim --test untamed_reworked untamed_ride_input -- --nocapture`: the new test ran on
  the image and did not skip.
- `./kit audit`: no leak in the files this task touched (it fails on four already on HEAD, in `atv_wheel.rs` and `plan/DONE.md`).

## Not verified / not 1:1

- What writes the pad record's stick words 0xdf..0xe5. MD93a said the lip's copy is "before the limiter",
  which is not shown either way. If the lip's words are unlimited and the port's step stick is limited
  (MxVsAtv's limiter leaves it unsquared), the lip's stick differs on large deflections. Filed as R30.
- Action 0x13 (Select) is never pressed in the port's feed.
- The bindings were read off one Free Ride image. A player's remapped controls (the options screen) were not
  traced.
- The stunt combo record's layout (the matcher 0x003a37d0) was not decoded. The "empty" combo rests on
  "bound" returning 0 and the zero header bytes.
