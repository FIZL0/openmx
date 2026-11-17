# 1 — cofg world is an offset (FINAL)

Task K52. 2026-11-17.

## What I did

- A collision object's `collider.cofg_world` is the centre of mass turned into the world: an offset
  from the body's position, not a place. Stopped moving it by a seam or copy width in:
  - `Flight::wrap_round` (flight.rs);
  - `seam::crash` (the wreck bike's collider);
  - `Flight::pushed`, `car::Machine::pushed` and `quad::Ride::pushed` (the nearest-copy put-back
    after a bike's resolve now keeps the resolve's own value);
  - the plane-against-plane resolve's nearest-copy shift and put-back in flight.rs.
- `quad::Ride::pushed` was not named in the task but did the same move (MD179 copied the car's); same
  tidy, so it is in.
- Doc on `Collider::cofg_world` (contact.rs) says it is an offset that a shift leaves alone.
- `tests/wrap_rest.rs`: "collider cofg" is out of the plane's and the wreck's moved places; both tests
  now assert it is bit-equal to the unshifted run's.
- `tests/contact.rs`: a comment on the oracle's junk `cofg_world` seed, which is the proof below.
- The task's `--files` named only contact.rs and the two tests; the moves themselves live in
  flight.rs, seam.rs, car.rs and quad.rs, which the task text names. Changed them (small, local edits).
- Nothing taken from ../opentennis: it has no seam wrap.

## What the original does

The collision object's resolution writes `+0xa0` (the world centre of mass) from `+0x90` and the
body's quat (or the chassis transform) before any read of it, for its own side and each other's.
`other_at` (a wheel on another collision object) does the same.

## How it was proved

- `tests/contact.rs`'s two oracles (`the_contact_resolution_matches_the_original`, `…_on_a_chassis_…`)
  already seed `+0xa0` with junk (`p.loose(50.0)`) on every collider and match the original bit for
  bit, so a stale value does not change what the resolution does.
- `tests/wrap_rest.rs`: a plane flown over the seam and a wreck thrown over it keep a `cofg_world`
  bit-equal to the unwrapped run's, and the rest of each test (the flight a width apart, the
  put-back) still holds.

## Not verified / not 1:1

- No run of the cars', quad's or plane-vs-plane nearest-copy paths with a contact across a seam
  compared before and after. The value they no longer shift is rewritten before every read (proof
  above), so behaviour cannot change; only the state hash of a run with such a contact changes.
- The state dumps (`w.vf("cofg_world")`) hash this field, so a wrapped-world replay that crossed a
  seam with a collider in play hashes differently than before. No pinned vector covers such a run as
  far as the gate shows.

## Next

Nothing.
