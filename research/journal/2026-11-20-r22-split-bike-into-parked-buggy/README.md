# R22 — a split-screen bike into the other player's parked buggy

2026-11-20. Task: `./kit plan R22`.

Captured in the original and held whole in the interpreter. The car steps first and its own resolve
does the meeting; the bike's resolve after leaves the car's body alone. The port does it the other
way round. Addresses and values under `# R22` in research/oracle.txt.

## How the car got into split-screen

- No state had one. The front end gives it: Freestyle > Montauk Plains, then on the setup page
  Number Of Players 2 and Machine 2 right three times to Dune Buggy.
- Player 2 answers "No" to the profile question; both press Start.
- In game, player 2's pause menu: down once ("Switch To Machine"), right to Dune Buggy, cross, cross.
- research/c34/place.py put the buggy 30 ft up player 1's line (slot 32 missed by 7 ft: the bike veers
  left; slot 34 is on its line). Player 2 holds the brake (p2_brake.pad) or the buggy rolls away.
- research/r22/capture.sh plays p1_ride.pad + p2_brake.pad from slot 34 into
  context/capture/r22/strike.csv. save_at.py saved slot 36 at frame 12006 → states/r22/pre.ram.

## The four questions

1. **The bike's kind-0 entries naming the car**: one, its body ray, filed after the car's three
   mirrors.
2. **The car's kind-1 mirrors**: one, the bike ray's, at index 3 of the car's record, after its own
   three rays on the bike (kind 0, naming the bike's record).
3. **Step order**: every record collides first (car before bike, the node's order). Then the car's
   dynamics world steps the truck and the buggy. Then the riders, then the bike world.
4. **The car's speed after the bike's resolve**: 2.3528 ft/s, the same as at the end of its own step.
   The bike's resolve writes none of the car's body. The car's first resolve takes the bike from
   46.2 to 4.3 ft/s.

## Checked

- `cargo test --release -p game-sim --test ai r22_`: nine ticks, every tick bit for bit against the
  capture (bike centre and velocity, car place and velocity, both lists, struck words), the call
  order, the strike's lists and speeds. Ran, no skip.
- `cargo test --release -p game-sim --test ai`: 80 passed, 0 failed.

## Not verified / not 1:1

- The port is not changed. Its divergences are filed as follow-ups: no car rays on the bikes; the
  bike's resolve pushes the car's body; a seat-0 car steps after a seat-1 bike.
- Only seat 1 driving the car was captured. That the car steps first when seat 0 drives is read off
  the node's order, not captured.
- 0x00df8410 being player 2's parked bike's record is read off its pointers.
- The record's `+0xa0` vector the bike's resolve writes is not named.
- Only one strike at one speed; the later push (from frame 12028, the bike still on the gas) is in the
  capture but not tested.
- The buggy was placed by poking RAM, not driven there.
