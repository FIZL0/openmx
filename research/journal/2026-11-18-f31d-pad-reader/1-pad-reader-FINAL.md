# 1 — pad reader (FINAL)

Task F31d. 2026-11-18.

## What I did

- Ported SLUS-21104's bike pad reader and the stick's way to `crates/game-sim/src/handling/pad.rs`
  (`pad::read`, `pad::way`). Also ported the input object's record (`pad::Record`, `pad::Bindings`) and its
  flick scan over the 32-sample ring (`pad::Ring::flick`).
- `tests/mxvatv_bike_ride.rs`: the pure-Rust replay's `"tape"` feed now runs `pad::read` on each tape tick.
  The bindings are read from state 4's image, and the ring is seeded from the image and sampled by the
  port's stand-in. **No reader word comes from the rows any more**, the flip hold +0x12b0 included.
- Replaced `tape_words`/`feed_tape` (rule-based, hold from the row) with `Pad`, `pad_words`, `pad_stores`
  and `tape_stores`.
- Added two oracle tests: `the_pad_reader_is_the_originals` and `the_input_flicks_are_the_originals`.
- Added the `# F31d` section to research/oracle.txt.
- opentennis: looked; it has no pad reader of this shape (and no MX vs. ATV bike). Nothing was taken.

## What the original does

- **The reader.** It is `./kit fn 0x004e36c0`, reached through the bike's input listener.
  - It first clears the trick byte (+0x14ec's low byte).
  - Wrecked (+0xc80 != 0): it zeroes the stick, gas, brake, clutch and the riding flag +0x14d4, then
    returns. So **the way bits are not held while wrecked**: the byte is cleared. +0x14c8 (the way), the
    holds, the d-pad flags, the angle, the reach, the reverse flag and the clutch latch keep their values.
  - Otherwise:
    - the stick is the record's;
    - the spin hold +0x12b4 is entry 0x20 and the flip hold +0x12b0 is entry 0x21, the left stick's
      sideways and fore-aft flicks;
    - the d-pad, gas, brake, reverse, clutch and trick buttons follow the bindings (`./kit fn 0x00118e10`);
    - then comes the way.
- **The stick thresholds** (`./kit fn 0x004e3c80`). The angle comes from the acos table. The stick has a
  way only when reach² > 0.25 (reach 0.5). The four ways are π/4 quarters: right within π/4 of 0, down
  π/4..3π/4, left beyond 3π/4, up −3π/4..−π/4.
- **The flicks** (`./kit fn 0x00507880`). The input update scans the ring per axis:
  - a sample beyond ±0.8 is remembered;
  - the next one beyond 0.8 on the other side gives (v − from) / gap, if the remembered time is above zero.
    The gap itself is not checked, so a zero or negative gap still divides. The port first checked the gap;
    the flick oracle caught that;
  - the largest in magnitude is kept.
  
  This is the same scan as MXU's `ride::Flick`.
- **The ring's sampler** (`./kit fn 0x00505f80`) is an engine callback that is passed the time.
- **992.97 explained.** 0x44783e10 = 2 / (264·2⁻¹⁷ s).
  - The ring's last sample at −1 (tape tick 64) and its first at +1 (tick 65) were taken 2.014 ms apart.
  - The value holds while that pair stays among the 32 samples: flip-1 rows 66..71.
  - MD164's −992.97 is the same crossing the other way.
  - So the value comes from when the engine happened to call the sampler, not from the simulation.

## How it was proved

- **`the_pad_reader_is_the_originals`.** 12,000 generated cases: the interpreter's 0x004e36c0 on state 4's
  bike against `pad::read`. The cases vary:
  - strengths, including ±0 and tiny values;
  - sticks at the thresholds;
  - the image's bindings, random ones and no table;
  - wrecked or not, the brake scale, the speed around 5, any word already present.
  
  Every written word, the trick word's upper bytes, the clutch cue, and that nearby words are untouched must
  match bit for bit. Every path is taken at least 50 times. Of the 12,000, 11,151 ran (tally: wrecked 2382,
  d-pad 3537, clutch cue 2394, reverse 1364, gas with the brake 640, ways 133–200 each). Cases whose random
  bindings point past the record's last entry are drawn again: no shipped binding does that.
  Raw bike words are read with `F(f32::from_bits)`, not `F::from_bits`, because the latter flushes
  denormals and a raw word of 1 would read as 0.
- **`the_input_flicks_are_the_originals`.** 12,000 generated rings: the interpreter's 0x00507880 over a
  made-up input object against `pad::Ring::flicks`. The rings cover times in order, equal, zero and
  negative, and samples at ±1, at 0.8 and just beyond it. All four flicks and the record's axes must match.
- **`the_tape_gives_every_reader_word`.** The port off the tapes gives every reader word of every row of
  ride-2 and flip-1. The one exception is the flip hold on flip-1 rows 66..71 (60.006 against 992.97):
  both are beyond the clamp of 60, have the same sign, and occur on exactly those rows.
- **`flip_replays_in_pure_rust`** and **`ride_replays_in_pure_rust`**: bit-exact with the hold off the tape.
  Earlier, forcing the hold to 0 broke flip-1 at row 79, so the hold does matter.

## Not verified / not 1:1

- **The sampler's timing is the port's stand-in.** It pushes 150 samples a second on the simulation's
  clock, as MXU does. The original's sample times depend on the hardware's waits.
  - flip-1's hold is 60.006 in the port and 992.97 in the original. They are equal only after the clamp at
    max_backflip_time = max_preload_time = 60.
  - A bike with a larger maximum time, or a crossing slower than about 0.03 s, would differ.
- **Not traced:** the sampler's call sites and its times per frame (F71).
- **Not ported:** the input update's second device (+0x60), which overrides an axis when it is larger. C15's
  rides have none, and the oracle sets it to 0.
- **Not ported:** the sampler's d-pad override of the axes (it reads them afresh through the binding
  lookup).
- **The clutch press cue** is only reported (`read` returns it). The tape feed asserts it never fires, and
  posting it is left to the caller.
- **Only C15's two tapes** were replayed. They hold Cross, Square, R1 and the left stick only.
- **Still the row's:** the put-back spot (F31e, not this task).

## Next

- F71 (filed, low): trace the sampler's timing so the flick is the original's and not the stand-in's.
