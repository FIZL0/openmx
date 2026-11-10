# 1 — mxvatv car update (FINAL)

Task MD106d. 2026-11-10.

## What I did

- `crates/game-sim/src/atv_car.rs`: new `atv_car::update`, the car class's once-a-frame update. It runs
  outside the world step, as the car's vt +0x10c. I ported its own code. Each thing it calls is handed out
  as a `Call`, the way MD106 did the three listeners:
  - New `Call` variants: `Ghost(dt)`, `HazardClock { dt, time, sunk }`, `ClassStep` and
    `VehicleUpdate(dt)`.
  - New `Update` struct for what only the update reads: the still-camera switch, the vehicle state's
    mode, the surface hazard table (the quad's `quad::Surface`), each tire's surface byte, and the
    on-hazard flag.
- `crates/game-sim/tests/mxvatv_car_frame.rs`: new oracle, `the_cars_update_is_the_originals`.
- research/oracle.txt: new `# MD106d` block (the update's words and calls).
- Filed MD106e: port and unhook what the update hands out. That covers the buggy's own vt +0x10c wrapper,
  the ghost step, the hazard clock, the buggy's class step, the base update, and the golf cart's and
  truck's update slots.
- Nothing taken from opentennis: it has no vehicle physics. The shape is copied from `quad::update`
  (MD64a). It is the same vehicle base and the same hazard table.

## What the original does

`./kit dis -p atv 0x4cc4e0 0x4cc750`. a0 is the car and f12 is dt.

1. **Still camera.** If bit 3 of the settings byte (`*(0x3eab60)+0x225`) is set, only the base update runs
   (`0x4a79c0`, car and dt). This is the still-camera switch noted for MD82b.
2. **Ghost step.** Otherwise vt +0x3a4 runs first, still with dt in f12. On the buggy it is a `j` to the
   vehicle's ghost step `0x4a74e0`.
3. **Latch count.** This happens when car +0xbc4 (the vehicle state) is null, or its +0x28 mode is 3 or 4.
   If the latch +0x31c is set, the counter +0x12c4 goes up by one while it is below 3 (`slti`, so the
   compare is signed). Otherwise the latch and the counter are both cleared. The PostProcess (MD106) clears
   the latch too, by its own rule.
4. **Hazard surfaces.** This happens in any other mode:
   - +0xc20 is set when any tire's hazard word is non-zero. The table is car +0xdf4 + 0xc × the tire's
     byte +0xf0; each entry is a word, then a time.
   - "Sunk" is +0xc20 and any tire's +0xa4 (the solid contact). On the quad this flag is a different word.
   - The times are folded as `max.s(FL, max.s(FR, max.s(RL, RR)))`.
   - vt +0x45c, the vehicle's hazard clock `0x4aad60`, gets dt, that time and sunk.
5. **Class step and base update.** Then vt +0x488 runs. On the buggy it is `0x4c5510`, 6 KB, takes only
   a0, and was not read. The base update runs last, in every case.

The buggy's own vt +0x10c (`0x4c6ce0`) is a wrapper that ends in this function. It does the odometer,
collision on/off and rumble first, and it is not part of this task.

## How it was proved

Function oracle: `tests/mxvatv_car_frame.rs` `the_cars_update_is_the_originals`. Run it with
`./kit check -- -p game-sim --test mxvatv_car_frame` (it passes).

- **What runs.** SLUS-21104's update runs off `context/mxvatv-ps2/md39a/buggy.ee.bin`, on the player's
  buggy, with its four calls hooked to log only. The port runs `atv_car::update`.
- **Cases.** 10,000 generated updates. Each one pokes:
  - the still bit;
  - the vehicle state: none, or the image's state object with mode 3, 4, 1, 0, 2 or a random word;
  - the latch, and a counter from −2 to 5;
  - the solid flags;
  - a 16-entry hazard table: words 0, 1, 0x80000000 or random; times random, −1 or ±0, with all four
    tires on ±0 now and then;
  - the four surface bytes, often shared;
  - the old +0xc20;
  - dt.
- **Checks.** Every compared word of the car and its tires is equal bit for bit, plus +0xc20. The call
  logs are equal. Every word the original wrote in the car and tire blocks is a compared word. A flipped
  bit goes red in exactly that word.
- **Branches.** 14 labelled branches, each seen at least 30 times:
  - still camera; no state; mode 3; mode 4; measured;
  - latch counted, counted up from below zero, let go, and no latch;
  - on a hazard sunk, on a hazard not sunk, on no hazard with a tire solid;
  - a sign-bit hazard word;
  - a tie of signed zeros.
- **Mutants.** 8 of 8 go red:
  1. mode 4 dropped
  2. `<` changed to `<=`
  3. sunk without the hazard
  4. `any` changed to `all` for hazards
  5. the still gate removed
  6. the time fold cut short
  7. solid changed to grounded
  8. the ghost call dropped

  Each was restored with cp + touch.

## Not verified / not 1:1

- **Every call is hooked.** The ghost step, hazard clock, class step and base update are not ported. The
  update is proved only as the order and arguments of its calls plus its own code. That is MD106e.
- **The buggy's wrapper is not ported.** Its own vt +0x10c (odometer, collision on/off, rumble, then this
  update) is not ported. Neither are the golf cart's and truck's update slots. All of that is in MD106e.
- **Only the buggy was run.** The update itself is the car class's. The baja and sandrail are expected to
  share it but were not run.
- **Surface bytes stop at 15.** The port reads the hazard table as 16 entries, like the quad's
  `[Surface; 16]`. A surface byte of 16 or more would panic in the port, where the original reads past the
  table (a string follows it on the image). The generated bytes stay within 0..16. No surface over 15 was
  seen on the image (every tire is on 0).
- **One state object was used.** The vehicle state is the image's own object, with only its +0x28 mode
  changed. Nothing else of it is read by the update.
- **The PC exe was not read first.** research/mxvatv-pc.md has no mapping for the car update. The PS2 ELF
  was read directly, and it is the oracle.
- **The sign of the time fold is not observable.** The fold uses `F::max` in the original's order. On ±0
  it gives +0 either way round, so the argument order cannot be told apart. The order is kept as in the
  instructions.
- **Clippy was not run.** It cannot lint game-sim. The new code uses only ps2-float ops, under
  `deny(clippy::float_arithmetic)`.

## Next

MD106e (the update's calls) and MD106a/b/c (the frame's), then the MD107 replay.
