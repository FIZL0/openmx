# 1 — quad on the wire (FINAL)

Task MD172. 2026-11-16.

## What I did

- `game_sim::snapshot`: a `Wire` codec for `quad::Lane` and everything under it — the ride's params,
  quad, body, bike, step log, record, tail and wreck, down to the ATV wheel, contact, part and scene
  types. Plain structs use the `wire!` field lists (complete by construction: a missing field does not
  compile); the data enums (`atv_wheel::Ground`, `atv_wheel::Holder`, `atv_sync::Kind`, `quad::Call`)
  are written out by hand, the variant's place first.
- Left off the wire: only the ride's arc cosine table, a constant, built again on decode
  (`AcosTable::build`, the same call `quad::Ride::new` makes).
- The step log (`Ride::rest`) is sent: every tick clears it before stepping, but the state's hash
  (`Lane::hash_bytes`) prints it, so a decoded ride needs it for the same hash.
- `encode` writes the lane behind its own tag (`MD172qd\0`) after the car's, only while a quad is out,
  so a state with none is the bytes it was. `decode` reads it back, and the tag is in the list of
  sections that cannot be a players' list (first try misread it as one, like S131's props).
- `openmx::net::rides::Rides::encode` no longer refuses while any seat's quad is out.
- Tests in `crates/openmx/tests/netplay.rs` (they skip without `mods/machines/mxvatv_atv`):
  - `a_ride_with_quads_out_says_itself_in_bytes`: 3 seats, 2 on quads; encode → decode gives the same
    hash and the same bytes again, and both copies ride 150 more ticks to the same hash.
  - `a_desync_with_quads_out_is_resynced`: host + guest (two seats) on quads, a third seat on its bike
    knocked at tick 320 on the guest; one resync each side, >300 settled ticks agree after.
  - `a_leave_with_quads_out_restarts_and_the_ride_goes_on`: three machines on quads, g leaves; the
    restart carries the state and host and k agree after.
  - `a_freeride_with_a_quad_out_takes_a_peer_mid_ride`: a lone host on its quad takes a joiner at
    round 200; the joiner rides to its own quad, and both agree from the join.

From opentennis: nothing new; the snapshot and resync machinery was already ported (N1, N5).

## What the original does

Nothing to port: online play is ours. The quad's state is MX vs. ATV's (MD37b and on); this task only
copies it bit for bit between machines.

## How it was proved

The four tests above, through the real `Rides` and `Netplay` (ggrs rollback, lossy simulated link).
The state's hash covers every field of the lane, so "agree after the resync / join / restart" means the
decoded quad steps on bit for bit.

## Not verified / not 1:1

- Not run in two real shells on a disc track (tests/online_atv.rs style): the proofs are on a flat
  floor in the in-memory harness. A track's world adds nothing to the lane, but a quad touching props
  or path cars on a real track was not exercised through a resync.
- The resync test glitches a bike seat, not a quad: the steps' after-hook is not called for a seat on
  its quad, so a quad cannot be knocked from the harness. The quads' state is still replaced by the
  host's at the resync and is in every hash compared after.
- The step log `quad::Call` goes on the wire in full (the last tick's; 18 bytes in the test's state of
  about 20 KB). Nothing reads it across a tick; it is sent only because the hash prints it.

## Next

none
