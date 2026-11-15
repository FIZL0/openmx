# 1 — the out-of-bounds check in Rust

Task F31b5s. 2026-11-15. Wrapped up early: the usage limit was near.

## What I did

- The "per-frame wreck check" F31b5 left (atv 0x004a4f20, ~21k instructions a ride) is MXVehicle's
  out-of-bounds check, the bike's vt +0x16c, which the wreck test asks every frame (`Out::OutOfBounds`).
- `handling::wreck::out_of_bounds`: held word +0x308; body +0x70 against the terrain's extent
  (+0x3b0 → +0x43c: width +0x40, depth +0x48) with 150 to spare, in the original's compare forms
  (`!(x < m) && x <= W - m`, z read only when x passes). Outside: the throw on the bike's words,
  the same arithmetic as `biplane::throw_mxvatv` (MD128 proved that against this same function for the
  car): target (0.5 W, y, 0.5 D, w of rodata 0x00525240), zero length → the quad 0x00396fc0, rsqrt,
  k = 2·sqrt(0x416e8262), speed min(0x45019999, 0.5 W)/k, lift adda/madd, velocity +0x80 by VU mul,
  +0x84 += lift, drag *(+0xe8)+4 *= 0.5, three rands (through `Out::Rand`) × (0.5, 0.4, 0.7) into
  +0x1d0..+0x1d8, +0x1dc = 1. Returns 1.
- Test switch `"wreck check"` (WreckInRam serves `Out::OutOfBounds` with the port, asserting vt +0x16c
  is 0x004a4f20); added to `replay_in_pure_rust`'s list.
- Oracle `the_out_of_bounds_check_is_the_originals`: 12,000 cases on state 4's bike (held or not;
  extent the image's, under 300 ft, or anywhere; each lane on an edge, ±1 ulp, the middle, or anywhere;
  drag word random), comparing the answer, all RAM and the scratchpad with the original in the
  interpreter. `./kit check -- -p game-sim --test mxvatv_bike_ride -- the_out_of_bounds_check`: passed.

## Not verified / not 1:1

- The oracle's pass was not shown to be a real run rather than a skip (no image → early return). The
  probe run that would have shown its path tally hit my 50-minute command limit under the slots' load
  and was cut off; then the usage limit came. Rerun it with a temporary panic after the tally to see it.
- The pure replays' interpreter counts with `"wreck check"` on (ride-2 / flip-1) were not measured, so
  the ~21k saving is not confirmed. The C15 rides never leave the map, so they only take the inside path.
- `biplane::Area::outside_mxvatv` writes the test as `W - m < x` where the original has
  `!(x <= W - m)`; they differ only on a NaN position. Not changed (not this task's code).
