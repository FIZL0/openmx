# The steering hinge and the wreck test, diffed and proved (FINAL)

## What the diff found

- **Steering hinge.** The dispatch (`atv_wheel_integrate`) is the port's: surface drag, spin, then on
  a steered wheel either the hinge (when it is balanced on) or a stop that zeroes rate and torque. The
  hinge keeps the PC's fields (torque `+0x60`, damping `+0x64`, inverse inertia `+0x6c`, lock
  `+0x70/+0x74`, angle `+0x8ac`, rate `+0x8b0`) and MX Unleashed's clamp order: `c.ole` against the top
  of the lock (so a NaN clamps there), then `c.olt` against the bottom, with the rate set to +0. The
  only arithmetic difference is that each of the three sums starts `adda.s 0, x`. ps2-float's
  `madd` is `add(acc, mul)`, so this only matters when `x` is a zero: `0 + (−0)` is +0. The
  oracle sees it, since MX Unleashed's hinge answers differently on ≥ 100 of the 10,000 wheels. The
  torque's own `0 +` cannot be seen: it only feeds a sum that already starts at +0, and the stored
  torque is zeroed afterwards. Dropping it stays green, and the original's form is kept anyway.
  Ported as `step::wheel_integrate`/`step::steer_integrate`. `ride.rs`'s `wheel_integrate` takes an
  `mxvatv` flag for it.
- **Wreck test.** `atv_bike_wreck_test` is MX Unleashed's machine with its wreck test folded in.
  - **Reason 14** (no driver: no AI on `+0x48c`'s `+0xe4`, and no vehicle state of kind 1): entered only
    on a bike not down, clock preloaded to MaxWreckTime, counting. This is `state.rs` exactly, and the
    oracle confirms it.
  - **Reason 15** (`atv_bike_flip_wreck_steep`, the same function the backflip's mid-clip wreck
    asks) comes before every other clause. It fires when the record is touching an object whose class
    bits share the global mask (0x20 in state04), and the bike is airborne or its fore-aft/side load
    is past a limit. The port takes the class test as `WreckIn::struck_class`.
  - **Clause chain.** MX Unleashed's on the same fields and limits, except for two things:
    - The airborne attitude limits are held off by the backflip's torque being in (`+0x1194`) or the
      360's (`+0x11d0`). MX Unleashed reads `flipping` there. Either torque on a touch wrecks 0xb.
    - The trick endings' gate is "aloft, or stunt id over 40, or the trick already a wreck". It has
      no no-hander term.
  - **Hits.** These run after the chain, over the record re-read. The wreck entry's collision call
    swaps the riding record for the wreck record, so after a clause wrecks, the hits read the wreck
    record. Its reach limit is 2.9 when `+0xbc0` is 0, else 4.5 (MX Unleashed: 6/4). An actor is
    judged by body height only (no heavier-object case). Scenery wrecks 6 only with the fore-aft/side
    loads past their limits, and the chain has already wrecked on those, so in a ride it never fires.
  - **Striker** (reason 5) and the **out-of-bounds** throw (13) are MX Unleashed's (the throw moved
    into a function at vt +0x16c). They are not ported here.
  - **Clock and put-back** differ from `state.rs` and are not ported: the clock adds dt before it
    arms, and arming posts event (8, 12). One limit (`+0xc88`) covers every reason, and the put-back
    is the bike's unwreck directly, with no page. Filed as MD71.

The port is `step::wreck_test` (chain with reason 15) and `step::hit_wreck`, plus `air::Wreck::Struck`
(0xf). `rider.rs` uses them on MX vs. ATV rides: `spin_applied` from `Spin::applied`,
`struck_class` false, `small` from the ride's existing `fifty`. Composition is as before: the hits
are not asked once a clause wrecked (S191a's reasoning holds here too, by the record swap).

## How it was checked

`tests/mxvatv_step.rs`, four oracles off `state04.ee.bin`:

- `mxvatv_steer_integrate_is_the_originals`: 10,000 wheels through `atv_wheel_integrate`, with a
  quarter of each input a signed zero and some angles on the lock. Surface drag and spin are hooked
  and their calls/dt compared, and the wheel is compared word for word. Every path (not steered,
  held, top, bottom, free) had ≥ 500 states, ≥ 300 came out with a zero angle or rate, and ≥ 100
  differ under MX Unleashed's hinge.
- `mxvatv_wreck_test_is_the_originals`: 10,000 driven, not-down states through `atv_bike_wreck_test`
  with its own vtable. The wreck entry is hooked to log the reason and do the entry's record swap
  (to an empty wreck record). The logged reasons and the throw at `+0x1260` must equal the port's
  `wreck_test`, then `hit_wreck`. All 19 paths had ≥ 50 states. MX Unleashed's
  `air::wreck_test_in` + `air::hit_wreck` answer differently on ≥ 50 states each of reason 15 (both
  ways), torqued touch, mid trick and touched-rides-on, and ≥ 10 of each hit.
- `mxvatv_no_control_is_the_machines`: 2,000 driverless states (each way of having no driver)
  through the original against `state::Ride::step`: reasons, clock, latch and wreck count.

Mutations, each of which reds its oracle: dropping the 360's torque; MX Unleashed's no-hander term in
the gate; reach 6 or 4; scenery always wrecking; reason 15 only in the air; dropping the angle's or
the rate's `0 +`. Dropping the torque's `0 +` stays green, as explained above.

`./kit check -- -p game-sim --test mxvatv_step --test mxvatv_ride --test mxvatv_air --test air
--test air_wrecks --test hash --test snapshot --test move --test levers --test wheel`: 83 passed.
`--test ride --test trick --test wreck_thresholds --test air_wrecks --test prop_wreck --test
rider_contact`: 94 passed. `cargo build -p openmx` built. `./kit audit`: the only leak is the
existing `plan/DONE.md:519`.

## Not verified / not 1:1

- No MX vs. ATV ride capture, so no whole ride wreck is replayed.
- `+0xbc0` (the reach's size word) is written once at vehicle init from the spawn record. It is 2 on
  state04's 250, and 0 is read as the small bike. That is inferred: the ride feeds the existing
  `fifty`.
- The stunt words are not ported. On a ride, "id ≤ 40" is `tricks.in_progress`, "id is the balance
  stunt" is the port's `no_hander`, and the pose/wanted/blend/trick-wrecked are the port's trick
  machine's, as for MX Unleashed. The oracle proves the clauses on those fields, not that mapping.
- `+0x11d0` is read as `Spin::applied`, the last substep's.
- Reason 15's object class is unknown (MD72), so the ride never enters 15.
- The striker (5), the out-of-bounds (13) and the clock/put-back (MD71) are left as they were.
- MD63b's controllers use the same `adda 0` forms and their oracles never fed −0: filed as MD73.
