# 1 — wreck into a bike (PART, blocked)

Task S134. 2026-10-28.

## What I did

- Read the wreck bike's nearby list out of S62's garage state (scratch script): it holds scenery, toys
  (0x48), the three AI riders' bikes (0x9a048) and twelve 0x200020 gates, as the task says. Field
  offsets of the AI bikes' collidable, chassis and collision record are in research/oracle.txt (S134).
- Tooling to capture a wreck into an AI bike in free09 (S76's farmyard state in slot 4):
  - `research/s134_tele.py PID LEAD AHEAD SLOT [bike K] [speed S | match] [up H] [at X Y Z]` pauses the
    game (AT-SPI), reads the three AI bikes, moves the human's bike next to / above one (S76's
    teleport, rider + chassis + vehicle + pool triples), optionally matches its velocity, and saves the state.
  - `research/s134_wreck.py NAME --pad P --state SLOT` is s62_wreck.py's Late capture with
    `watch/s134-bike-hit.toml` (s56-ragdoll's fields + each AI bike's collidable, chassis and record).
  - `research/s134/wait-gas.pad`, `stand-gas.pad`.
- Thirteen attempts (try1..try13, context/capture/s134 in slot s3): aimed approaches, standing in the AI's
  line, launching at 60-70 ft/s across it, riding alongside at matched velocity, dropping from 4/10/30 ft
  onto it. Results:
  - The AI riders brake to a stop ~5 ft short of a standing human, or swerve (16 ft) round a fast one.
  - Side touches and landing on an AI bike from 10 ft push the bikes apart and do not wreck.
  - try12 (lead 0.4, matched, 30 ft drop) wrecked (reason 5) on the road, and the wreck bike slid past
    AI bike 0 with origins 5.2 ft apart at the closest (frame 2960). Nothing in the wreck bike's 4 KB
    (bike_a) ever pointed at the AI bike's collidable or record outside the static +0x2a0 list, and the
    AI chassis velocity shows no impulse-sized jump that is not its own steering, so I count it as no contact.
  - try13 (lead 0, 30 ft drop) did not wreck.
- No Rust change: without a capture the bike-as-other contact (Scene reporting `other`, a
  `contact::Other` with `Side::Vehicle` from the AI chassis) cannot be checked bit for bit, and the port's
  other bikes are other players', whose behaviour would change unproven.

## What the original does

The wreck bike's contact search takes its candidates from the nearby list above; a 0x9a048 bike there is
an actor contact for it (bike collidable 0x9a058, actors 8, collides 0x40000 vs the bike record's actors
0x2008, collides 0xffffffff). Whether the impulse reaches the AI chassis is unmeasured.

## How it was proved

Not proved: no capture of the contact exists.

## Not verified / not 1:1

- The whole task: no wreck into another rider's bike was captured, so WreckBike::tick_among still gets no
  bikes, toys or gates (the task's 0x48 and 0x200020 objects are not handed in either).
- try12's 5.2 ft pass is read as "no contact" from the absence of pointers and of an AI velocity jump; the
  contact array itself was not decoded.

## Next

Get a capture where a wreck bike surely meets a bike. Best lead: C10's 2p state (S157/S160 — two human
pads, P2's wreck bike 0xdc4550 is in the level's collision node): park P1 (pad 2 = None or held still) and
wreck P2 into it, or the other way round, with s134_wreck.py's watch extended to P1's chassis/record. With
a second pad the "other" bike does not dodge, unlike free09's AI. Then port as planned above.
