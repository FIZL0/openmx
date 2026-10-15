# 1 — axes (FINAL)

Task I2. 2026-10-15.

## What I did

Ported the analogue axes into `crates/game-sim/src/input.rs`, beside I1's controls: the six axis slots
the poll keeps, the map that says which report byte each one reads, the inversion the dispatch applies to
one of them, and the curve every consumer's value comes out of. The port is additive — a new section of
constants and types, two private passes on `Pad` (`read_axes`, `centre_axes`) called from the three
places the poll already branched to, two new fields and two accessors. Nothing in I1 moved.

The short answer to the task's question — "the original's dead zone, clamping and any curve, in its
arithmetic" — is:

- **Dead zone: yes**, ±0.2 of the output range, i.e. report bytes 102..=152 inclusive read exactly 0.
  101 and 153 are the first bytes that answer at all. It is not a cut-out: each side is stretched back
  over the full output range, so outside the dead zone the stick is exactly 1.25× steeper than the byte,
  and the ends still reach ±1.
- **Clamping: no.** There is no clamp anywhere in the chain. The input is a report byte, so the game has
  no reason to have one, and the function is linear past both ends if you feed it something else.
- **Curve: no.** Two straight segments and a flat middle. Nothing exponential, no squaring, no
  normalisation of the pair of axes against each other.

So the thing this task existed to prevent — "an approximated stick curve quietly changes every movement
in the game" — comes down to getting a dead zone and two gradients right in `F`, in the original's
expression order, which is what the oracle proof below is for.

## What the original does

Recipes (addresses live in `research/oracle.txt`, never in the port):

```sh
./kit fn 0x0011d2e0     # the curve
./kit fn 0x00134f50     # the dispatch: the consumer list of one axis
./kit fn 0x0016ab10     # the frame poll, whose axis pass is a span inside it
./kit fn 0x0011d3d0     # the single-axis reader, with the map hardcoded
./kit fn 0x0016aa40     # centre everything (pad closed)
```

**The poll's axis pass.** Each pad object keeps six axis slots. The raws are floats at `pad+0x628+4*i`
and the byte index each one reads is at `pad+0x714+4*i`; index 0 means "nothing bound", and that slot
takes 127.0 every frame instead of a report byte. Measured map, pad 0: `[6, 7, 4, 0, 0, 5]` — left X,
left Y, right X, (nothing), (nothing), right Y, in report bytes. The poll stores the raw and then calls
the dispatch per axis; **axis 2 is dispatched as `255.0 - raw`**, every other axis as the raw itself. That
inversion is the whole of what makes axis 2 interesting: it is the one axis whose value is not the byte's
direction, and a port that misses it steers the right stick backwards.

**The dispatch** (0x00134f50) walks the per-axis consumer list at `pad+0x640+axis*0x18` (count at +0,
pointer array at +8) and calls the curve on each consumer with the same two floats.

**The curve** (0x0011d2e0) is a pure function of `(raw, full)` and the consumer's own five floats:

```
v = lo + (raw / full) * (hi - lo)
if v < dead_hi:
    if v <= dead_lo:  value = (v - dead_lo) * ((centre - lo) / (dead_lo - lo)) + centre
    else:             value = centre
else:                 value = (v - dead_hi) * ((hi - centre) / (hi - dead_hi)) + centre
```

and stores it at consumer+0x24. Plain `div.s`/`sub.s`/`mul.s`/`add.s` — no `madd.s` anywhere, so the
port's expression order in `F` is the hardware's instruction order and not a rearrangement of it. The
boundary senses are the original's: `<` at the top of the dead zone, `<=` at the bottom, which is why
`v == dead_hi` is on the upper segment and `v == dead_lo` is the centre.

**The configuration.** The ranges are not a table in the ELF — the game builds each consumer at bind
time, so they were read out of a save-state RAM image and then confirmed live: every stick consumer is
`centre 0, lo -1, hi 1, dead_hi 0.2, dead_lo -0.2`. The port carries that as `AxisRange::STICK` and the
recording below asserts the game is still running with it, row by row.

**Centring.** Two branches centre the axes without a report: no pad connected, and a report whose status
byte is non-zero. The absent-pad branch takes the raws themselves to 127.0; the dispatch is still given
the inverted value for axis 2, which is 128.0, so the centred value of the inverted axis comes from a
different raw than the others. Both branches are in the port (`read_axes(None)` and `centre_axes()`).

## How it was proved

1. **The curve, against the game's own instructions.** `the_axis_curve_is_the_games_own_arithmetic` in
   `crates/game-sim/tests/input.rs` runs `pad_axis_curve` in the oracle and compares
   `AxisRange::apply` bit for bit: **11,524 calls over 43 ranges** — every byte 0..=255 plus twelve
   out-of-range and edge values (both centring constants, −1, 256, ±1000, 0.5, 127.5, ±1e20, 1e-30, 0),
   against the game's own range, 39 generated ones spanning a few exponents either side, and three
   degenerate ones that put a zero in each divisor. Zero mismatches. The degenerate ones are deliberate:
   a divide by zero is a number on this hardware, and the port has to produce the same number.
2. **The whole chain, against the running game.** `a_recording_of_the_real_sticks_replays_row_for_row`
   replays `context/capture/i2-axes.csv` (`watch/axes.toml`, save state 9, `watch/pads/stick-sweep.pad`):
   **126 frames, 630 consumer values, all bit-exact**, with 450 in the dead zone, 90 above, 90 below and
   40 at an end of the travel. Each row rebuilds the `Report` from libpad's older buffer half (the half
   the poll read — I1's lesson), ticks the port, and compares the six raws, the live map against the
   port's `AXIS_MAP`, and every live consumer's value computed through *that row's recorded range*. So
   the oracle proves the arithmetic for arguments the test picks, and the recording proves the port picks
   the game's arguments: the right byte per axis, `255 - raw` on axis 2 only, 127.0 on the two unbound
   slots, and the ±0.2 dead zone still configured in RAM.
   The recording ends at 126 of its 220 frames because the idle main menu hands over to the attract loop
   and the engine's frame counter stops there; the sweep itself is complete, which is what the coverage
   assertions in the test are for.
3. **The dead zone in bytes**, measured by the port against itself and written down as a test
   (`the_dead_zone_is_a_fifth_of_the_travel_either_side`): 102..=152 read exactly zero, 101 and 153 are
   the first bytes that answer, and `outside_the_dead_zone_the_stick_is_steeper_than_the_byte` pins the
   1.25× gradient. These are consequences of 1 and 2, kept as tests because they are the numbers a reader
   of the port will want to check against a controller by hand.
4. **The map, three independent ways**: the RAM image (`pad+0x714`), live over PINE with a stick held
   (bytes `7f 7f ff 00` → raws `[255, 0, 127, 127, 127, 127]`), and the hardcoded per-axis mapping inside
   the single-axis reader at 0x0011d3d0, which agrees including the inversion.

I also mutation-checked the recording: dropping the inversion in the port fails the replay at row 54 on
the first frame the right stick moves. A capture that cannot fail is not a proof.

## Not verified / not 1:1

- **The second poll (0x0016b1d0) is not ported.** It keeps a ring of past samples with its own index and
  is never called in any state reachable from here — the ring is all zeros and the index 0 in every save
  state and live read I took. Proving it needs the hardware timer it stamps with and a gameplay save
  state that uses it. Nothing in the port depends on it; if a later task finds a consumer, the ring is a
  new field, not a change to this code.
- **The map's writer was not found.** No function in `boot.c` writes `pad+0x714` under a name I could
  find, so the map is *measured* (three ways, above) rather than read off the code that builds it. The
  consequence: the port compiles in pad 0's map as `AXIS_MAP` for every pad, and **pad 1's map is
  unproven** — it reads as all zeros (every axis unbound) in every state I saw, which may just mean
  nothing ever bound a second pad here.
- **Axes 3 and 4 were unbound in every state I saw.** The port gives them 127.0 every frame, which is
  what the game does with index 0; whether anything ever binds them to a byte is unknown.
- **The value is stored per axis, not per consumer.** The original writes the mapped value into each
  consumer record (+0x24); the port keeps one `axis_value` per axis. They are the same number only
  because every consumer measured carries the same range (asserted per row in the replay, so a game that
  changes this breaks the test rather than passing quietly). A state that configures two different ranges
  on one axis would need a per-consumer record here — a real difference in shape, deliberately deferred
  until something needs it.
- **The two centring branches are not in the recording.** The pad never vanished and never returned a bad
  status byte during the capture, so "no pad → raws 127.0, and the inverted axis centred from 128.0" is
  read off the code and covered by unit tests, not measured live. The 128.0 asymmetry is the part most
  worth re-checking if a later task sees a pad disconnect.
- **The single-axis reader (0x0011d3d0) is not ported.** It does its own `scePadRead` and has the map
  hardcoded; in a port the pad arrives as a report, so there is nothing for it to be. Its arithmetic is
  the same curve, already proved.
- **The capture's byte coverage is the virtual pad's, not a DualShock 2's.** PCSX2 scales the pad's
  −1..1 to a report byte, so the recording holds nine byte values per axis (0, 43, 94, 111, 127, 144,
  161, 212, 255) and not all 256. Every byte is covered by the oracle instead, and the ends and the dead
  zone are covered by both.
- **`Report::set_axis` has nothing to compare against** — it is the test helper that writes a stick byte
  into a report, the inverse of what libpad does.

## Next

Nothing for this task. If the axes come up again, the two open threads worth the trouble are the history
poll (needs a gameplay state) and a per-consumer value record (needs a state that configures two ranges
on one axis). Both are written above with what would settle them.
