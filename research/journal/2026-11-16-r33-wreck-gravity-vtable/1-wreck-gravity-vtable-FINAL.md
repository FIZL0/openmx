# 1 — wreck gravity vtable (FINAL)

Task R33. 2026-11-16.

## What I did

- Scanned SLUS-21104's state 4 RAM image for every word equal to MD63f's gravity rewriters. Each sits in
  exactly one vtable slot: vtable `0x003cfad0` `+0x368` (wreck entry) and `+0x36c` (unwreck).
- `0x003cfad0` is the **Kiowa** helicopter's vtable. That was already in `research/oracle.txt` # MD80a / # MD80c.
- Read the wreck entry, unwreck and put-back slots of all eight vehicle vtables (quad, biplane, buggy, car,
  golf cart, Kiowa, truck, bike). Only the two fliers write the gravity block:
  - Kiowa: entry writes WreckGravity, unwreck writes GravityScale.
  - Biplane: entry `./kit fn 0x004c0cc0 -o atv` (WreckGravity) and unwreck `./kit fn 0x004c4350 -o atv`
    (GravityScale). This is the "which vehicle not read" in # MD63d.
  - Bikes and quads share one entry and unwreck. Neither writes gravity (F31b4b).
  - Buggy, car, golf cart and truck share another pair. Neither writes gravity.
- Checked the port. Both fliers already rewrite gravity with the same rounding, `32.2 * (-scale * mass)`
  (neg.s, mul.s, mul.s):
  - biplane: `Flight::wreck` / `unwreck`
  - Kiowa: `kiowa::Wreckage::wreck` and its unwreck
- Rows: `research/oracle.txt` # R33. # MD63d's gravity line now points at it.
- Dropped MD63f. Its text says "the bike's wreck", and no bike does this. Filed S345 for the rest of the Kiowa's
  SLUS-21104 entry and unwreck. The mxvatv Kiowa runs MX Unleashed's shape, as the biplane did before MD112.
- Nothing taken from ../opentennis. It has no such vehicle code.

## What the original does

- Kiowa entry `./kit fn 0x004d3f90 -o atv`. In order:
  - runs the base entry
  - multiplies the rates by 1.0
  - writes the WreckGravity block
  - calls the sound part's +0x94
  - throws the blades, unless the reason is 13
  - flips two node bits
- Kiowa unwreck `./kit dis 0x004d40a0 0x004d4250 -p atv`. In order:
  - writes the GravityScale block
  - zeroes the controls and the held height
  - puts the nodes and the rotor parts back
  - runs the base unwreck
- Ghidra's `FUN_004d40b0` is the middle of this unwreck, not a function: 32.2 is built in the first four
  instructions.

## How it was proved

- A RAM-image scan of state 4 for each address word, run by a scratch script that printed only the hits.
- Each vtable's slots +0x368, +0x36c and +0x3a8, read from the same image.
- The functions read with `./kit fn` / `./kit dis -o atv`.
- No run: this is research, and nothing in the product changed.

## Not verified / not 1:1

- **The Kiowa's whole SLUS-21104 wreck entry and unwreck against the port.** Only the gravity write was checked
  (same formula and rounding, read off the disassembly; no oracle run). The rest (base entry posts, damping
  gate, inertia, node bits) is S345.
- **When a Kiowa wrecks in play.** No capture: the entry is reached through the vehicle's wreck test, which was
  not traced live. Desert Oasis Free Ride places one (MD80a).
- **`jal 0x004d40b0` at 0x001fc1e4** (engine function 0x001fb5e0). It lands mid-prologue in the game overlay,
  so it is taken to be a front-end overlay call over the same addresses. Not followed.
- **The vtable starts are the ones earlier tasks named.** Slots are offsets from them. The +0x368/+0x36c
  layout matches the bike's slots from F31b4b, which supports them, but the starts were not re-derived.

## Next

Nothing for R33. S345 holds the Kiowa's follow-up.
