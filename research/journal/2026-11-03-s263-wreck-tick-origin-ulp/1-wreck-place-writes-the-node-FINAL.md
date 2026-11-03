# 1 — the wreck body's place writes the bike's node (FINAL)

Task S263. 2026-11-03.

## What I did

- `Ride::wreck_axes` (ride.rs): the bike transform's nose and up rows squared as the spawn squares them
  (`flight::spawn_axes`).
- `Crash::begin` gives the wreck bike the quat `spawn_turn(nose, up)` of those axes, in place of the quat it
  took before.
- `Ride::place_wreck` (ride.rs), called in rider.rs right after `Crash::begin`: the node's rows become
  `math::basis(0, nose, up, false, true)` and its origin the wreck bike's origin (body position less the
  turned centre of gravity).
- tests/replay.rs: S162, S191, S207 and S219 rides now carry the wreck kit (`carries_wreck`) when the
  ragdoll kit is there, so the wreck tick runs the place. Ratchets raised one tick each: S207 truck 105→106,
  rails 98→99; S162 bump 205→206; S191 headon 60→61, stand 93→94, roof 158→159; S219 flank 20→21.
- Windmill test: the port's own ragdoll flight moves by a tick from the ulp in the turn (south pin
  (8, 14)→(9, 14), apex (42, 85)→(41, 85), the apex loop 25..=40). Those ticks are port-only, not held to
  the original; the doc comment says so.

## What the original does

The wreck part holds the bike's model node (the same node the chassis draws through, whose matrix is the
capture's `ch_xform`). The wreck entry (`./kit fn 0x00252ca0`) transforms the wreck body's offset (0, 1.5, 0)
by that node, reads its axes, starts the wreck part, then calls the part's place (`./kit dis 0x0015a1b0`).
The place squares look and up as the spawn does, makes the quat from them, stores position and quat on the
body (`./kit dis 0x00159f20`, `./kit dis 0x00159ee0`), resets (copies to the current pose), then writes the
node: origin = position − turn(quat, cofg) (`./kit dis 0x001b2660`) and rows from the squared axes
(`./kit dis 0x001b1e20`). So on the wreck tick the chassis' drawn transform is the wreck body's place, not
the ride's last integrate: that rewrite was the ulp. Block S263 in research/oracle.txt.

## How it was proved

Save-state captures compared bit for bit (tests/replay.rs, s191 captures and states copied from s2):

- wreck-tick `ch_xform` rows equal `math::basis(_, look', up', false, true)` in headon, stand and roof;
- the origin equal in truck, rails, headon, stand and roof once the quat is `spawn_turn(row 2, row 1)`
  (the origin alone left roof 1 ulp off in y);
- with the change out, each ride's first parting tick is back on the wreck tick.

`cargo test --release -p game-sim --test replay`: 118 passed, 1 ignored. Full `cargo test --release -p
game-sim --no-fail-fast`: green but fly_and_collide `the_drawn_rider_leans…` ((1952, 282) vs 280, known
B18, red on HEAD too).

## Not verified / not 1:1

- From the tick after the wreck every ride parts on the thrown chassis' quat (`ch_quat`): the port does not
  simulate the chassis while down (known, B7's thrown bike). That is where the new ratchets stop.
- The node's rows on later down ticks: only the wreck tick's rows were compared.
- S219 rear still parts on 42 (its S251 push) and ctruck on 36; neither is a wreck-tick origin, not looked at.
- The windmill's moved pins are port-only ragdoll ticks; nothing says which is the original's.
- Online and split screen not run; not looked at on screen.

## Next

none
