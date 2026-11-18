# 1 — ports in the wreck ride (FINAL)

Task MD92a. 2026-11-17.

## What I did

- `crates/game-sim/tests/untamed_bike_ride.rs`: hooked all seven of `handling::untamed`'s ports over
  their originals in MD92's ride (vehicle update, air state, backflip, 360, wreck entry, put-back,
  preload), as `mxvatv_bike_ride.rs` does for SLUS-21104.
  - Each port reads its state from RAM, runs, and writes back only the words it changed (shadow sync).
  - Its callouts are the original's calls, made through the same addresses or vtable slots.
  - Vehicle update and air state: the state is synced before each callout and re-read after.
  - Wreck entry, put-back, backflip, 360, preload: callbacks get no state, so the port runs staged
    (stopped at each callout, state synced, the original called, the answer cached, the port rerun).
  - Force and moment adds go through the body's own virtuals, asserted against the port's values.
- New tests `wreck_ride_replays_bit_exact_with_the_ports` and `second_run_replays_bit_exact_with_the_ports`.
  They also assert each port ran (per-frame ones every row, wreck entry and put-back once).
- The replay now also compares the bike object, not just the physics blocks.
  - The first ported replay passed even with a port bug (hang time doubled), because the bike words
    were not compared. With the compare, that bug fails on row 149 at bike+0x1180.
  - 58 bike words (`UNFED`) differ in the plain replay too, the same on both captures; they are left out.
- `the_tape_gives_every_reader_word`: the reader and pad-record words on every row of both captures
  equal what `tape_words` makes from `wreck.p2m2.ticks.csv` (tick t on row t), and the words covered
  are exactly `READER`'s plus `PAD_RECORD`'s.

## What the original does

- Reader words from a tick (bike up = the row before had wreck reason +0xc70 zero):
  - +0x340 brake = square; +0x344 gas = cross while up.
  - +0x1298 stick x and +0x12bc (the lip's copy) = lx while up; +0x12ac = |lx|; +0x12a8 = the arc
    cosine table at lx (centred 0x3fc90fda). All zero while wrecked.
  - +0x14dc = up. Pad record +0x14f4 = the device (3), +0x14f8 = way (left 3, right 4, else 0).
  - Everything else in the reader block is zero on this tape.

## How it was proved

- Bit-exact replay fixture, both captures (wreck-1 665 rows, wreck-3), with all seven ports in the frame:
  physics blocks and the bike object (but `UNFED`) on every row.
- Mutation check: hang time doubled in `untamed.rs` fails the ported replay on row 149.
- Tape test over every row of both captures.

## Not verified / not 1:1

- **Trick paths of the backflip, 360 and preload are not exercised.** The tape presses no trick button
  (+0x150c always 0), the preload axis is unbound, and the device's binding byte is 0, so `button` is
  None. Their stunt start/end, clip, steep and wreck callouts never run on this ride.
- **Binding queries are made before the port runs**, not only where the original asks (the lip). Not seen
  here, since the device has no binding.
- **The 360's node-axes query is made before the port** whenever the 360 is on, not inside it. Never on here.
- **Deferred calls after the port**: the backflip's and 360's final writes (clip 1→2, stunt end, wreck)
  land after the port's other words, not interleaved as in the original. Not exercised on this ride.
- **Shadow sync does not rewrite a word the port left unchanged** even if a callee changed it in between.
  Matches here (bit-exact), not proved in general.
- **`UNFED`'s 58 bike words are not compared.** They differ in the plain replay too (the spot and saved
  place, wreck and rider words from row 340, other updates' words). Measured, not traced to their writers.
- **The tape test asserts the tape's limits** (cross/square only, lx in {-1,0,1}, no other axis). A tape
  outside them fails the test rather than being modelled.

## Next

- Nothing for MD92a. A tape with tricks and a bound device would exercise the trick paths above.
