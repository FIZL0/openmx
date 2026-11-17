# 1 — turn closed fliers (FINAL)

Task N26. 2026-11-16. Follow-up of P318a
(research/journal/2026-11-09-p318a-lobby-online-fliers/1-lobby-online-fliers-FINAL.md, its race note).

## What I did

Picked the brief's second way: the join's restart turns the seat. Holding the handshake to the
settled ticks would need the ride to answer the handshake on the network thread. That is a much bigger
change, and the restart is already the one tick every machine agrees on.

- **`rides::turn_closed`** (crates/openmx/src/net/rides.rs, new).
  - Every seat up in a closed flier gets a copy of the ride's own plane, with its flight put back
    onto it (`Flight::put_back`, as P141's `fly_as` and the markers use it).
  - So it flies on where and how it was: same body, surfaces, engine, wreck state.
  - What is the plane's own comes from the ride's plane: the modifier numbers, ray probe, steps
    (`mxvatv`) and skin (0). Every machine holds all of that.
  - A ride with no plane of its own leaves the seat as it is.
- **`Rides::seat`** calls it right after the spawn closes the joiner's missing fliers. That runs in
  the host's grow, before the restart state is encoded. So the state every machine decodes already
  has the seat in the ride's own plane (skin 0).
- **`Rides::decode`**: a seat whose decoded skin differs from its skin here drops its ray probe, so
  `probe` takes the new plane's probe and steps.
  - Before this, a machine that had the seat in the pack's plane kept the pack's probe and steps.
    The codec keeps this machine's for a seat already flying here.
  - This also covers any resync where a seat's plane differs from the prediction.
- **`Rides::seat`**: the joiner's own seat gets the same treatment when its skin differs from the
  first seat's. It is built on a clone of seat 1, and seat 1 may be up in a pack's plane.
- The `Flier` doc now says a seat already up in a closed flier is turned, not left flying.

Opentennis has no fliers or mods, so there was nothing to take.

## What the original does

Nothing: online play and flier packs are openmx's own.

## How it was proved

- `tests/netplay.rs` `a_seat_up_in_a_flier_a_joiner_lacks_is_turned_onto_the_rides_own_at_the_join`
  (new, runs with the disc's `biplane.phy`):
  - Three machines: the host, a peer, and a joiner that holds no fliers.
  - Seat 1 goes up in the pack's plane (0x12, MX vs. ATV steps, its own ray probe) on the host and
    the peer for 10 ticks.
  - The host's join closes 0x12. Seat 1 is now skin 0, MXU steps, the ride's probe, and its body
    position is unchanged.
  - The peer and the joiner decode the restart. All three show the same seat 1, and the same hash.
  - The hashes are still the same after 30 more ticks on all three.
  - Mutant check: with the decode change switched off, the test fails on the peer (`(0, true, true)`:
    it kept the pack's steps and probe). With the change back, it passes.
- `a_joiner_lacking_a_fliers_pack_closes_it_at_the_join_on_every_machine` (P318a): it said seat 1
  "flies on in the flier". It now says seat 1 was turned onto the ride's own, and it still ends with
  both machines on the same hash.
- `./kit check -- -p openmx --test netplay fliers`: both pass. Full gate in `./kit done`.

## Not verified / not 1:1

- **No two-shell lobby run with the actual race.** The test drives `Rides` straight through
  `seat` → `encode` → `decode` on three copies. It does not go through `admit::grow` or
  `recover::restarted` over a network, and a switch was not timed into the gap between `InUse` and
  the handshake. Reason: the same missing two-shell lobby harness P318a noted.
- **The joiner's own seat check in `Rides::seat` is not tested.** That is the case where the joiner
  flies in on the ride's plane while seat 1 is in a pack's plane. It needs `spawn_flying_on` with a
  bay. Only the decode path was mutation-checked.
- **The test's pack probe is empty, and the ride's plane has none.** That is enough to tell the
  probes apart, but no real collision with a pack's `.col` was flown after the turn.
- **The turned seat keeps the pack plane's mass, inertia and wreck state.** `put_back` keeps the
  motion's whole state, which includes the mass. This matches across machines, since it all travels
  on the wire, but it is not the ride's plane's mass. `fly_as` keeps the same set.
- **A visible change for the player who was flying.** Their plane turns into the biplane mid-flight
  at the join. That is the price of letting the joiner in. Refusing the joiner instead would need the
  handshake to read the settled ticks.
- **Host handover (N11)** was not tested, as in P318a.

## Next

Nothing: the task is done when `./kit done` lands.
