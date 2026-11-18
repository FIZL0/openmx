# 1 — ai lane on the wire (FINAL)

Task S327. 2026-11-18.

## What I did

- `snapshot::encode` now carries the Sim's AI lane, behind its own tag (`S327ai`), only when the Sim
  has one. A state with no AI riders is the same bytes as before.
- On the wire: the seeker world the brains learn into, the AI generator, every pilot (brain, senses,
  seeker, habits, follower, character, wander, levers, recovery, card, seat, ignore list), the cards
  the brains read each other off, and the track's AI path net. The net goes too: the race driver
  writes it (`Arc::make_mut` in `Sim::think`: route pools, best marks), so it is state, not a constant.
- Kept local: the controllers' tables (an `Arc` of `ragdoll::Tables`, built, never written) and the
  race's edge net (built once off the race's laps the first time a brain thinks on them).
- `decode` refuses a state with a lane on a Sim with none (no tables to give it), and drops this
  machine's lane when the sender has none.
- The state hash walks the lane as one blob of those same bytes (as the driven car does, S249), only
  when there is a lane: every pinned hash without AI riders is unchanged.
- One `wire!` line per type (38 of them), so a field added to any AI struct and not to the codec is a
  build failure.
- opentennis: nothing to take; its snapshot (`hst-play/src/play/snapshot.rs`) has no AI lane of this
  shape. The codec is this crate's own `wire!`.

## What the original does

Nothing to port: the snapshot and hash are ours (online play). No original behaviour is involved.

## How it was proved

- `tests/snapshot.rs` `the_ai_lane_rides_the_wire_and_the_hash`: the four-rider session with players
  1–3 AI (free-ride brains, own generator, a path net with pools set). Encoded at ticks 0, 30 and 111
  and decoded onto a machine elsewhere in the ride whose lane differs: the lane, the whole state and
  the hash match, re-encoding gives the same bytes, the tables stay the receiver's, and both ride 40
  more ticks with equal hashes. Poking the generator, a pilot's levers, a card, the seeker world or the
  net's pools each changes the hash and survives the wire. The refuse / drop cases are checked too.

## Not verified / not 1:1

- The test's net has no paths (pools only): a net with real paths goes through `TrackPaths`' codec,
  which the race's laps already use (P313), but no AI-lane test sends one.
- The test's brains are free-ride ones on no seeker world and no character file: a race brain on a
  real track (mode 2, laps, edge net) was not sent over the wire in a test. Its words are the same
  structs, so the codec covers them, but no run proves a resync mid-race.
- The edge net is kept local on the claim that it is a pure function of the race track's geometry and
  never written after it is built (`Sim::think` only reads it). Not checked by a test.
- The Sim's own seeker world (`Sim::seek`, before `spawn_ai` takes it) is still not sent: nothing
  steps it (S316).

## Next

Nothing.
