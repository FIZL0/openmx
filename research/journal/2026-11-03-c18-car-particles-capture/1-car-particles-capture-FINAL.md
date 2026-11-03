# 1 — a driven car's particles against the original (FINAL)

Task C18 (S248 follow-up). 2026-11-03. Slot s3, PCSX2 copy 3.

## The question

S248 ported the car's particle manager from the disc and the decompilation. It was never compared
with the original running. It also fed the manager `Machine::wrecked` (+0x2c4) for the loader's
wreck word, +0x754, which nobody had identified.

## +0x754 is never set

- **ELF scan.** The only stores to car +0x754 are zeros in the two car ctors:
  - 0x0039d3e8 in 0x0039d310 (buggy, baja);
  - 0x003ded7c in 0x003deca0 (truck).
  - No `sq`/`sd` covers it.
- **Readers.** All of them test it against 0 (or 0xd):
  - rumble 0x003a0340;
  - 0x0039e9c0;
  - 0x003a1a00;
  - the truck's four equivalents.
- **Pointers.** The feed's +0xa94 and car +0xad0 hold its address. It looks copied over from the
  bike's wreck reason (+0x748), and nothing in the car ever writes it.
- **The car's real wreck** is +0x2c4:
  - vt +0x1d0 writes the reason there and zeroes +0x2c8;
  - vt +0x1d4 undoes it;
  - the edge throw in the wreck step (reason 13) is the only caller of vt +0x1d0.
- **Live.** In `truck-edge` (below), +0x2c4 goes 13 at frame 460 and back to 0 at 671, and +0x2c8
  sets at 462. +0x754 is 0 on every row, and so is it in the other two captures.
- **Outcome.** The original's car manager always reads `NotWrecking`. Its exhausts' only Disable bit
  is `Wrecking`, and they stay flag 1 on all 211 wreck rows.
  - The port now feeds `wrecked: false` in `PlaneFrame::car`.
  - It used to cut the exhausts and every riding entry for the whole wreck.

## The captures

All three are in s3's `context/capture/c18/` (git-ignored). Each is 650 rows, one per frame, with no
gaps. They are S149a's free07 tapes (copied from s2's `context/s149a/`), played back with
`capture/capture.sh`. The spec comes from `capture/make_spec.py` and reads:

- the car's wreck words;
- the feed's throttle, clutch and speed;
- each tire's words and its record's flags and zone;
- per entry: the flag byte, the switch byte, carry, rate, point, a spray's clock and side velocity.

| name | tape | car | frames | what happens |
|---|---|---|---|---|
| `truck` | truck.p2m2 | monster truck (kind 4), manager +0xff0, 22 entries | 459–1108 | on the gas, up to 130 ft/s |
| `truck-edge` | truck-edge.p2m2 | the same | 459–1108 | parked; the shortened map's edge throws it (460), tumbles 211 frames, up to 272 ft/s |
| `baja` | baja.p2m2 | baja (kind 5), manager +0xf00, 21 entries | 401–1050 | on the gas, up to 110 ft/s |

`truck-edge.p2m2` is the truck tape with its save state changed by `capture/edge_state.py ...
2400`. The script sets the depth of the map's edge test to 2400 ft. With the truck at z ≈ 2357, the
next frame's edge test throws it (oracle C18 has the recipe).

`capture/summary.py NAME` prints a capture's wreck words and each entry's flags and carry moves.

## The comparison

The new test is `tests/car_particles.rs the_cars_emitters_follow_the_original`. For each capture:

- free07's list for the car is read off the disc.
- On each row, the row's words are written into a car `Machine` where the loader's records point.
  The frame is built by the port's own `PlaneFrame::car`, with each entry's node at the emitter point
  the row read, and the manager is stepped by the row's dt.
- The test compares:
  - every entry's flag byte;
  - each switched entry's on bit (exhaust, generic, spray);
  - a spray's fade clock;
  - whether each emitter emitted on that tick (its carry moved).
- On a miss, the entry is put back on the capture.
- Rows that repeat the row before (25, 62 and 8: the car did not step) are skipped.
- 4 truck-edge rows where a record's zone differs from its tire's surface are skipped as torn.

What it showed, in order:

1. **Phase.** Fed the row before's words (as P74's bike), about 620 misses per capture. Fed the same
   row's words, the misses drop to about 100–180. So a row's state is the manager's after reading that
   row's own words.
2. **Zone-off tire dust.** Most of what was left was the four tire dusts emitting in the port while
   their flag was 3 (live and zone-off). The original's never do. On frames where the tire is grounded,
   compressed past 0.35 and over 20 ft/s:
   - zone-off dust: 0 of 228 emit;
   - live dust: about 90% emit.

   P60 had read the dust's own update as switching the emitter back on after the zone handler turned
   it off, and pinned that in `biplane_particles.rs` (decompilation only, no capture). The port's Dust
   arm now runs only while the entry is exactly `LIVE`, as the manager does for every other kind.
   biplane_particles' `a_zone_picks_the_dusts_group` now asserts no dust on a `none` zone.
3. **Result.**

   | capture | rows compared | wreck rows | misses (entry-ticks) |
   |---|---|---|---|
   | baja | 641 | 0 | 0 |
   | truck | 624 | 0 | 9 |
   | truck-edge | 583 | 161 | 35 |

   - No flag or on-bit misses anywhere, including the wreck.
   - Every remaining miss is a single emission tick or a spray's clock 1/30 s off, at a transition:
     - the four dusts as the truck leaves the ground at 462;
     - sprays' spin-ups at 674–684, 704 and 988;
     - the exhausts' switch at 704/705 and 782;
     - one tick after a switch-off at 725.

     These look like the original reading one word of the frame before as the car's state changes.
     They were not chased.
   - The test asserts no flag or switch misses, and under 1% of entry-ticks missed.

Ticks each entry emitted on, per capture (original / port), are printed by the test. On baja they
are equal for every entry. The port's particle totals are printed too. The original's count per tick
cannot be read back from carry and rate: its share is a random draw between the rate's ends, not
the rate field.

## Changes

- `crates/openmx/src/particles.rs`:
  - `PlaneFrame::car` feeds no wreck;
  - the Dust arm is off while zone-off;
  - doc lines for both.
- `crates/openmx/tests/car_particles.rs`:
  - the new capture test;
  - the frame test now expects `wrecked == false` for a wrecked car.
- `crates/openmx/tests/biplane_particles.rs`: the zone-off assertion, as above.
- `research/oracle.txt`:
  - the C18 block: +0x754, the feed and manager layout, entry vtables, phase, zone-off dust, the
    edge recipe;
  - S248's "+0x754 not identified" line updated.

## How it was checked

- `./kit check -- -p openmx --test car_particles`: 14 passed. The new test ran on all three captures
  (output above), with the disc present.
- `./kit check -- -p openmx --test` with each of biplane_particles (20), bike_particles (4),
  kiowa_particles (4), wreck_particles (3), track_particles (6) and particle_size_life (2): all pass.
- `./kit audit`: only the two existing plan/DONE.md leaks.

## Not verified / not 1:1

- **The CollisionModel entries** (Vehicle Dust/Chunks) are not compared. The capture holds no body
  contacts, and the test feeds none. The original's emitted on 4–7 ticks per capture; the port's on 0.
- **Particle counts and places per tick** are not compared. Only which ticks each emitter emits on.
  The original's per-tick count cannot be recovered from carry and rate. Particle positions were not
  captured (only the emitter point, which the test feeds as the node).
- **The node places** in the shell (the car's matrix times each node's bind) are not checked against
  the captured emitter points. The test feeds the captured points.
- **Residual misses:** 9 + 35 single-tick timing misses at transitions (above), not explained.
- **The buggy** was not captured. It shares the baja's loader and vtable.
- The tire-word phase in the air (7 torn tire-rows) is not understood.
- Not looked at on screen, online or in split screen.
- Truck-edge's wreck is the edge throw only. The car has no other wreck (oracle C18).
