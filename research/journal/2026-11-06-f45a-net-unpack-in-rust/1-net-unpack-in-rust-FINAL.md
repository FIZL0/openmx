# 1 — net unpack in rust (FINAL)

Task F45a. 2026-11-06.

## What I did

- `quad::net_interpolate` split into `net_interpolate_head` (returns a `NetLerp`: pose, velocity,
  orientation, whether a put-back is wanted) and `net_interpolate_tail`; the old function runs head,
  `Call::NetPutBack`, tail as before (atv_quad net interpolation tests still pass).
- `bike::net_post`: SpatialUpdate, NodeBounds on the bike's node, CollisionSync.
- tests/mxvatv_bike_ride.rs: the bike's update in RAM now serves `Out::NetInterpolate`
  (`net_interpolate_in_ram`: head, write the words, the original's put-back on RAM, re-read, tail),
  `Out::WheelClear/Suspension/TireForces` (`unpack_wheel_in_ram`, quad::Steps on a Nat05 terrain), and
  `Out::NetPost` (`net_post_in_ram`). A whole-RAM `both_ways` helper compares original and port.
- Filed F51 (port the wheel contacts) and C30 (capture with a remote bike for the in-frame proof).
- opentennis: nothing relevant. The PC exe: not read.

## What the original does

See research/oracle.txt `# F45a`. The interpolate blends two snapshots, and when it wants it hands the
pose back through the bike's put-back virtual, which rewrites the body (velocity too); the tail then
reads the body again. The unpack clears each wheel, finds contacts, runs suspension then tire forces;
the ground query also fills a ring cache of cells (`./kit fn 0x00541550` region) that the port does not
keep. The net post is spatial update, node bounds, collision sync.

## How it was proved

Function oracles over generated inputs, whole-RAM byte compares on state 4's bike
(crates/game-sim/tests/mxvatv_bike_ride.rs):
- `the_bikes_net_interpolate_is_the_originals`: 10 000 cases (near 2954, lags set 2436, put back 4610).
- `the_bikes_net_check_is_the_originals_with_the_interpolate_in_rust`: 3 000 cases, every stop 0–5.
- `the_bikes_net_unpack_is_the_originals_with_the_wheels_in_rust`: 2 000 cases on Nat05 (both down 645,
  both up 1023, front only 193, rear only 139); every case served the wheel calls twice and the net post once.
Mutants, each killed then restored: no RAM write before the put-back; no re-read after it; FRONT/REAR
swapped; tire forces skipped; CollisionSync dropped from net_post.

## Not verified / not 1:1

Everything this task left unproven or inexact goes here, each with its reason — or the single word `none`.
A reader must be able to tell from this section alone what they may not trust yet.

- No in-frame proof: no capture has a remote bike, so no replay reaches these paths (C30).
- The wheel contacts are still the original's in the net unpack (F51).
- SpatialUpdate and NodeBounds never ran with the bike in the spatial database (no SpatialMove on this
  image), so their in-database paths are unexercised here.
- The put-back is called with f12 = dt; what the original passes in f12 there is not known, and the
  oracle shows no difference.
- Gather and PartMove inside the net post are handed to the original.
- The ground query's ring cache is not kept by the port; those bytes are skipped in the compare.
- The wheel serving only happens when the Nat05 file is present; without it the original runs.
- The `stop n` tallies in the net check test label the intended stop, not a counted one.

## Next

F51 (port the contacts), then C30 for an in-frame replay with a remote bike.
