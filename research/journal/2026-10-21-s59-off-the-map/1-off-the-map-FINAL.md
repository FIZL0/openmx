# 1 — off the map (FINAL)

Task S59. 2026-10-21.

## What I did

- Decoded the map-edge clause of the bike's wreck test (`./kit fn 0x002522c0`, its exit at 0x002525e4)
  and ported it as `crates/game-sim/src/off_map.rs`: `outside` (margin 150 ft inside the terrain's
  extent, tested on two points: the rider's main bone and the chassis' centre of gravity), `throw` (the
  velocity toward the map's middle, the spin, the speed limit 350 ft/s, the transform onto the cofg),
  `entry_spin` (the three draws the cannonball's wreck entry spends on the wreck body's spin) and
  `unwreck`.
- Wired it into `rider.rs`. The clause is the first in the substep's wreck test and the only one that
  runs for a rider already down. `World::area` (Ground answers the terrain's, Flat answers none) gives
  the extent. `ragdoll_ride::Kit::main_bone` gives the bone.
- Found the motocross bike's own unwreck override (`./kit fn 0x0027bb20`, vtable 0x00455138, called
  from 0x00281c10). It runs the vehicle's unwreck (0x00252ee0) and then sets the chassis' speed limit
  to 135 ft/s. The port calls `off_map::unwreck` at both put-back sites (the chord and the clock).
- Captured the original live (copy 4, save state 7 on free09, `research/s59_edge.py` with
  `watch/s59-edge.toml`): the north, east, west and south edges ridden off on the ground, and one
  crossing in the air (`airborne`). The CSVs are in context/capture/s59/ and are not committed.
- Biplane: looked for an off-map or too-high rule and found none to add (below).
- Sounds: the wreck sound pick has no entry for reason 13, and draws nothing for it. The port's
  `wreck_sound::Emitter::wreck` already returns None for 13.

## What the original does

- **Edge.** The extent is the terrain's root cells times 256 times its sample spacing: 6912 × 6912 ft on
  free09. If either the main bone or the cofg is within 150 ft of an edge (x or z), the bike is set
  to reason 13 (the cannonball), gated by the bike's +0x570. The bike builder (0x0027aa70) sets that
  flag when its a3 is 0.
- **Throw.** Let k = 2√6.21118. The velocity points at the map's middle in xz, with speed
  min(1497.6, W/2)/k and lift 16.1k + 100/k. The angular velocity is (0.5u, 0.4u, 0.02, 1) from two
  rand draws. max_speed_sq becomes 350². The transform is moved onto the cofg. The air drag (+0x6a8) is
  halved. Reason 13's wreck entry then draws three more rand units as the wreck body's spin, instead of
  copying the bike's.
- **After.** The reset chord can't cut a cannonball short. The clock arms once the wreck body
  settles, the page is asked for when it passes the wreck time (5.0 s), and the put-back comes 3
  ticks after the page. At the put-back, max_speed_sq goes from 122500 to 18225 (the bike's unwreck)
  and the drag is doubled back.
- **Biplane.** Its wreck check (0x00353180) and its position query (0x001a6e50) have no height clause,
  and the query ignores y. "Too high" is only the engine's power fade from fade_altitude to
  MaxAltitude. That is already ported (biplane.rs `Engine::step`, proved in tests/flight.rs and
  tests/phy.rs). The biplane's edge was matched by S36. Nothing new to port.

## How it was proved

All in `crates/game-sim/tests/off_the_map.rs`:

- `the_map_edge_is_the_originals`: a **function oracle** of the clause over 12,000 generated bikes
  (inside, outside by bone, by cofg, both, on each edge, gated off). The test compares every byte of
  the bike, the chassis, the transform node and the terrain, the generator's state, and the reason.
- `the_entry_spin_is_three_units`: the entry's three draws, against the original.
- `the_original_rides_off_each_edge`: the **five live captures**, replayed in the port:
  - The row before the throw is inside and the throw row's cofg is outside. All five were thrown by
    the cofg.
  - The throw's velocity, spin and speed limit are bit-equal to the port's once the generator has
    stepped 0–5 draws (the riding draws between the samples).
  - The three spin draws match the wreck body's spin bit for bit.
  - The ride state machine (`state::Ride`) matches every row's clock and latch bit for bit, through
    the page and the put-back. The walk lets a row repeat or skip a tick or be read mid-tick: rows are
    samples, not ticks, and the test bounds how many it tolerates.
  - The put-back sets max_speed_sq to `off_map::unwreck`'s bits.
- `a_rider_past_the_margin_is_thrown_back`: the wiring. A `Rider` on a floor with free09's extent,
  set down past the margin, is thrown as a cannonball on its first frame (toward the middle and up,
  with limit 350²). The chord is refused while the cannonball lasts. A put-back gives the chassis
  the riding limit.
- The full `./kit check` passes.

## Not verified / not 1:1

- **No whole-ride bit-exact replay through the edge.** The port's ride isn't bit-exact with the
  original's yet (the Crash and the ragdoll are stand-ins, ACCEPTANCE_AGREES_THROUGH = 0). The
  captures prove the throw, the draws, the state machine and the put-back's speed limit. They don't
  prove the bike's path between the throw and the put-back.
- **The bone half of the clause is proved by the oracle only.** All five live throws were by the cofg.
  The riding main bone in the port comes from the fixed Rig pose, not from an animated skeleton.
- **The drag halving and doubling (+0x6a8) are not ported.** The port's ride has no such field on the
  bike. The oracle halves its copy to compare the rest.
- **The +0x570 gate is approximated** as "a local human". The builder's a3 and the event's league
  condition are not modelled.
- **The page's 3-tick delay.** The port's shell owner has screen: false, so it puts back on the
  request tick. The original pages and puts back 3 ticks later. This is pre-existing, not S59's.
- **The put-back position** (Crash::rest against where the original's wreck body came to rest) is not
  compared. The port's wreck body is a stand-in.
- **The entry spin's w lane** comes from the bike's angular velocity w. That is read from the oracle
  only.
- **The settle gate's sampling.** On up to 2 rows a capture the clock is matched with the row's own
  settle flag rather than the previous tick's (`late_gate`). That is a row read mid-tick, but it is
  not distinguishable from a one-tick difference in the gate.
- **The airborne crossing is staged.** No natural jump reaches an edge on free09: the edges are a
  flat strip about 480 ft deep behind a berm, and stick hops failed. So `--kick 30` writes the
  chassis' vel.y once over PINE, 126 ft before the margin. The original's own physics did the rest.
- **The camera, the fades and the announcer** at the cannonball are not ported.
- **The shell has not been ridden to the edge live.** The wiring is proved by the Rider test, not by
  openmx on free09.

## Next

Done. Follow-ups: S65 (the drag halving), S66 (the page's put-back delay), S67 (the +0x570 gate).
