# 1 — held effects item (FINAL)

Task F58a. 2026-11-16.

## What I did

- `crates/game-sim/src/handling/effects.rs`: a new `Kind::Held` and its step `held`, plus the latch word
  `HELD_LATCH` (`+0x74`). Every float goes through ps2-float (`adda`/`madd`/`msub`, `min`/`max`, `abs`,
  `neg`), and raw `lwc1`/`swc1` copies stay raw words.
- `crates/game-sim/tests/mxvatv_bike_ride.rs`:
  - `FX_KINDS` gains the seventh vtable (`0x003d1740`, research/oracle.txt `# F58a`), so PartWalk runs it
    in Rust as well.
  - `the_effects_items_steps_are_the_originals` now draws seven kinds. It sets item `+0x74` to zero or a
    float. A Held item's `+0x28` is a fade: a float, a quarter, 1/30, or a value in 0..0.3. The test
    asserts that a latched case lit and that unlatched cases both lit and doused.
- research/oracle.txt `# F58a`: the step, read whole.
- opentennis: nothing to take (it has no vehicle effect items, as F58 found).

## What the original does

The step was read whole with `./kit dis 0x004fd0c0 0x004fd37c -p atv`.

- First it reads the table, as Swing does.
- **Latch** (item `+0x74`): the entry's `+8` word sets it. If that word is clear, the entry's `+0xc` word
  clears it. This is Ramp's latch, kept in a different word.
- **Unlatched:** this works like Swing's in-band fade, with these differences:
  - the counts are `(k * fade) / 0.25`, not `k * (fade / 0.25)`;
  - the second count re-reads the fade from RAM.
- **Latched:** this works like Swing's swing, with these differences:
  - there is no gate or dead band;
  - the x spans are `adda(0, base)` then `msub`/`madd` with `0.5 * |angle|`, not `sub`/`add`;
  - the counts' final `madd` is `0.5 * (k * c)`, not `k * (0.5 * c)`.
  - It ends the same way as Swing: fade set to 0.25, the `+0x70`/`+0xb1` spin bit, then light.

## How it was proved

- **Function oracle `the_effects_items_steps_are_the_originals` (tests/mxvatv_bike_ride.rs), 20 000 cases
  over the seven kinds** (about 2 900 of them Held). It passes: the object calls match in order and
  arguments, and the whole layout matches.
- **The branch assertions hold:**
  - latched Held lit;
  - unlatched Held lit (fade still running) and doused (fade out before or after the `- dt`).
- **Mutants**, each restored with cp + touch:
  - the latch's clear removed: fails (RAM);
  - the clamp's upper bound removed: fails (RAM, case 6);
  - the second "fade ran out" douse removed: fails (calls, case 22).
  - Taking the counts' 0.5 in Swing's order did *not* fail. Scaling by a power of two is exact, so that
    order cannot be told apart except at underflow or overflow.
- Gate: `./kit check --all` in `./kit done`. This includes the every-port replays. No C15 item has this
  vtable, so the replays do not reach this kind.

## Not verified / not 1:1

- **No capture reaches this kind.** No C15 item steps it, so it is proved only by the function oracle. I
  did not find which game items use vtable `0x003d1740`.
- **Operation order under a power-of-two scale.** The counts' `0.5 * (k * c)` against `k * (0.5 * c)`, and
  the fade's `(k * fade) / 0.25` against `k * (fade / 0.25)`, give the same result except at
  underflow or overflow. The oracle cannot catch an order swap there. The port follows the original's
  order as read in the disassembly.
- **The object calls are stand-ins in the oracle**, as in F58 (light and douse are the emitters' code).
  The oracle proves only that the step makes these calls the way the original does.
- **The kind map is test-side** (`FX_KINDS`), as in F58. Nothing in the shipped frame calls
  `effects::item_step` yet.
- **dt with junk bits** is not in the oracle, as in F58. The frame never makes such a dt.
- **I did not read the PC exe for this kind.** research/mxvatv-pc.md has nothing on these classes.

## Next

(none)
