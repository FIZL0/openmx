# 1 — mxvatv plane wire (FINAL)

Task MD100. 2026-11-05.

## What I did

- Of the brief's two options (put `Flight::mxvatv` on the wire, or rebuild it from the machine) I took
  the second. The switch says which game's steps fly the machine, and that comes from the skin's
  name form when the plane is built (`Flight::new_mxvatv`). It belongs to the machine just as the ray
  probe, bike mesh and hull do, and the codec already leaves those off the wire for `decode` to put
  this machine's back. So the switch is handled the same way:
  - `snapshot::decode`: the decoded plane takes `here.mxvatv`, and so does every plane marker slot
    (the slots already got this machine's rays, bike and hull there).
  - `Flight::put_back`: a marker put back keeps the flying plane's switch, as it keeps its rays and
    bike. A marker read from a file (`decode_flights`) comes back off, so without this a put-back
    would have switched an MX vs. ATV plane to MX Unleashed's steps.
  - openmx `net/rides.rs` `Rides::probe`: a seat decoded flying with no plane here took the ride's
    plane's rays; now it takes that plane's switch too.
  - `snapshot::save`/`restore` (rollback) clone the Sim, so the switch already carried through them.
- The wire's bytes do not change, so RIDERS_WIRE_*, tests/hash.rs and netplay.rs's leave round
  (memory: wire growth moves the leave round) stay where they are. MD80e does not have to land first.
- Nothing taken from opentennis: it has no per-machine switch of this kind.
- Test: crates/game-sim tests/snapshot.rs `an_mxvatv_planes_switch_is_this_machines_through_a_snapshot`.
  It encodes a ride with a plane and a plane marker, switch on and switch off, and asserts the bytes
  are identical. It decodes onto a plane with the switch on and one with it off: each keeps its own
  switch, the marker slot too, and re-encoding gives the same bytes. It checks that save/restore keeps
  the switch, and that a `decode_flights` marker put back leaves an MX vs. ATV plane on SLUS-21104's
  steps. Mutant check: with the decode line removed the test fails (left false, right true).

## What the original does

Nothing to port: snapshots and netplay are the port's own. In both games the plane's step comes from
the machine that was loaded.

## How it was proved

Unit test above, plus the existing snapshot tests (12 pass), whose MX Unleashed snapshots
round-trip byte for byte.

## Not verified / not 1:1

- Two peers flying different planes (one MX vs. ATV, one MX Unleashed) online: `Rides::probe` gives a
  seat decoded with no plane here the ride's own plane's switch, just as it gives that plane's ray
  probe. With mixed planes a joiner's seat would fly the host's game's steps until its own plane is
  built. This could not be tested because MD80e (the fliers in the game, and so online) has not
  landed. If MD80e allows mixed planes, it has to send the switch with the machine's pick, the same
  way it would have to send the probe.
- No online run with an MX vs. ATV plane: MD80e has not landed, so the plane cannot be picked in the
  game yet.

## Next

Nothing for this task. MD80e: if peers can fly different planes, send the machine's pick (and with it
the switch) per seat.
