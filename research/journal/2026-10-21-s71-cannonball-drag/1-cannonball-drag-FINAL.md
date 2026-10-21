# 1 — the cannonball's air drag

Task S71. 2026-10-21. Follow-up of S59 (filed there as "S65", renumbered).

## What I did

- Looked for every reader of bike `+0x6a8` in the decompilation: the `.phy` loader (0x002539e0,
  writes Drag there), the ground step (0x00257a00, the only reader), the map-edge clause (×0.5,
  0x002522c0) and the vehicle's unwreck (0x00252ee0: `/0.5` only when `+0x748 == 13`). The other
  `+0x6a8` hits are other objects (the rider body actor, the biplane, others). The wreck test runs
  from the ride state machine at the end of the bike's update, after the ground step, and the ground
  step is skipped while the reason is set. So the halved drag is never read by a ride. It is ported
  anyway, so the bike's state is the original's throughout.
- Port: `ride::Ride::drag_halved` (on the snapshot wire too). The throw sets it inside the clause,
  in `rider.rs`. Both put-backs clear it (the clock's, and the chord's, which a cannonball never
  reaches). `off_map::drag(riding, thrown)` is the value the ground step reads, and `move::air_drag`
  is `move::drag` with that value handed in. `ride.rs` reads it in the substep loop.
- Why a flag and not the value: the port keeps the `.phy` on `Phy`, and `Phy` can be swapped under
  a built rider (the modifiers, and online seats get `sim.phy` after the clone in `main.rs`). A
  stored copy would go stale there. The flag gives the same value as the original's field whenever
  the halving undoes exactly. The oracle shows that is every drag a `.phy` can hold.
- opentennis: nothing to take (no equivalent clause).

## How it was proved

All in `crates/game-sim/tests/off_the_map.rs`:

- `the_put_back_doubles_the_drag_back` (new): the original's `bike_unwreck`, run over 12,000 bikes
  whose drag is where the port says it is. Its slot calls and the trick-queue reset are stubbed.
  Results: 5513 cannonballs give the `.phy` drag back bit for bit, and 6015 other reasons are left
  untouched. 466 drags halve to a flushed zero and stay 0. 6 top-exponent patterns overflow and clamp
  to ±0x7f7fffff. Both exceptions are counted and asserted to be exactly those inputs.
- `the_map_edge_is_the_originals`: the 12,000-case clause oracle. It now writes `off_map::drag(d,
  true)` back instead of halving by hand, and every byte still matches.
- `the_original_rides_off_each_edge`: on all five s59 captures (copied in from s4's context), the
  drag column is the riding 0.025 on the row before the throw, `off_map::drag(_, true)` (0.0125)
  on every row from the throw to the one before the put-back, and riding again on the put-back's
  row, bit for bit.
- `a_rider_past_the_margin_is_thrown_back`: the wiring. The flag goes up on the throw and stays up
  through 119 frames of chord presses. The rider now carries a made-up wreck body, so the clock puts
  it back, and the flag is down after that.

## Not verified / not 1:1

- **The drag is a flag, not the original's value.** It differs from the original's `+0x6a8` after
  a cannonball only for a `.phy` drag under 2⁻¹²⁵ (the original keeps the flushed 0) or with the top
  exponent (the original clamps it). No `.phy` holds either. Both are measured in the oracle.
- **The halved drag is never read in a ride**, in the original or the port, because the ground step
  is skipped while the rider is down. So no capture can show it acting on the bike. The capture's
  drag column proves the field's value, not any effect on motion.
- **`Rider::place`** (an outside put-back, e.g. a restart) leaves the flag alone, as it leaves the
  speed limit alone. Whether the original's restart path runs the unwreck or rebuilds the bike was
  not looked at.
- The wiring test's wreck body and rider wrecks are made up (tests/ride.rs' numbers), not the disc's.
  It only needs a put-back to happen.
- **The resync wire grew one byte a rider**, so `tests/platforms.rs`' `RIDERS_WIRE_DIGEST` and
  `RIDERS_WIRE_LEN` (12923 → 12927, four riders) were re-pinned on this x86-64 host only. The
  cross-architecture run (`tests/cross/platforms.sh`) was not repeated. No other vector moved.
