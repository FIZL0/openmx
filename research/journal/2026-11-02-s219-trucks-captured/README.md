# S219 — trucks captured

2026-11-02. Task: `./kit plan S219`.

I captured the original meeting free06's construction trucks and a free09 trailer. The port rides each
capture from its save state in `tests/replay.rs` (`a_bike_meets_a_moving_truck_as_the_original_does`):
- **Flank**, a bike at 27 ft/s into a dump truck's flank: the original enters reason 6 on tick 20 (the
  head under the overhanging body), not S216's 1. The port enters 6 on the same tick, with every lane
  agreeing up to it.
- **Slow**, the same at 6 ft/s: no wreck in either, bit-exact over the whole capture.
- **Rear**, a standing bike rear-ended by the HasPhysics company truck: reason 4 in both, as S216
  kept. But the port's strike comes a tick early (41 against 42), with the truck at the same point to
  the digit. That is B8's early contact; this ride is noted there as its second case.
- **Trailer**, free09's trailer A met at 18 ft/s: no wreck in either, every lane agreeing to the strike.

Details in `1-trucks-captured-FINAL.md`.
