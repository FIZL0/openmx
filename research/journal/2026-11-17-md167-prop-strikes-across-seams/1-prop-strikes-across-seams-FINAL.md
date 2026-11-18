# 1 — prop strikes across seams (FINAL)

Task MD167. 2026-11-17.

## What I did

- `prop_contact.rs`: `strike_wrapped` (and `Sim::prop_strike_in`). When the bike's ray sphere
  (`flight_search::reach`) is within reach of a wrapped world's seam, every knockable prop whose sphere,
  moved back by that seam's shift, comes within the rays' reach gets a copy in the same search: its
  `f.col` frame's translation moved back by the shift. The copies go after every prop's own place, so
  the broad-phase candidates of a non-wrapping or mid-world strike are unchanged. A copy's hit is filed
  on its prop (records are keyed by collection and object, so a prop met at both copies is one record).
- Every step path uses it: `Sim::step` and `step_all` (lib.rs) call `prop_strike_in` with their world;
  `prop_strike_seated` / `prop_strike_shared` (rider_contact.rs: step_bumped, step_among) now take the
  world. `Sim::prop_strike(i, dt)` stays the plain strike, for the tests that have no world.
- The rest of the tick had to follow, because a contact's lever arms are worked out from the body's
  position: a ball struck across the seam, pushed where it stands, would get an arm a world wide.
  - `Strike::across` names the bodies met at a copy and the shift; their mirrored entries (on the
    ball's list) go back to the ball's own place ("move the record back").
  - `PropSet::strike_in`/`strike` hand the bike's frame the ball a width back, by the bike; they note it
    in `PropSet::across` (one new field in prop_set.rs); `PropSet::pushed` moves it back (exact: the
    width minus a coordinate by the far seam is exact, and so is adding it back).
  - `Sim::props_step` sees each striker that struck across a seam one width over (its vehicle copy's
    node origin), so the ball's mirrors resolve against the bike by the ball, and clears the list.
- `tiles.rs`: `seam_shifts(world, at, reach)`, the shift list `Tiled::copies` gives, for any `World`
  by its `wrap()` and `area()`; `Tiled::copies` now calls the same helper (same code, moved).
- The task said to shift the bike and move the record back (as `Tiled::sweep` does). I moved the prop
  instead, as MD153a's moving-list copies do, for two reasons:
  - one search, one record: the bike can meet a prop by the near seam and another by the far seam in the
    same tick, and the one-entry-per-ray rule and the order the rays meet them stay right;
  - the shift lands on the prop's translation (whole-foot width minus a coordinate near the width:
    exact), not on the bike's frame (a small coordinate plus the width: rounded), and the record's
    points need no moving back.
- opentennis: nothing to take (a tennis court does not wrap).

## What the original does

Nothing: no disc map wraps. Wrapping is the port's own (MD29); the original's strike is unchanged on
every world that does not wrap.

## How it was proved

- `crates/game-sim/tests/wrap_moving.rs` `a_bike_by_the_near_seam_strikes_a_ball_by_the_far_one`
  (disc-backed, free09's props, ran here with the disc): a bike in front of a beach ball where its rays
  strike it; a world wrapped on `x` whose far seam is the whole foot under the bike; the bike moved a
  width back (within a foot past the near seam).
  - alone (`prop_strike`) it strikes nothing;
  - `prop_strike_in` strikes the same ball with the same entries and mirrors: kinds, others, surfaces,
    normals and `from` within 1e-3, points within 1e-3 after adding the width back;
  - `across` names the ball with the width; the mirrors' points equal the plain strike's (the ball's
    place);
  - the push hands the bike's frame the ball a width back; `pushed` puts it back bit for bit;
  - after the props' step the ball's position, velocity and rates match the same strike at its own place
    within 1e-3 (and the step did move it). With the step's striker shift taken out the test fails
    (position off by 0.1 ft): checked by mutation;
  - on a world that does not wrap, `prop_strike_in` equals `prop_strike` word for word.

## Not verified / not 1:1

- A strike across a seam is not bit-for-bit the strike at the prop's own place: the bike's frame stands
  a width over, so its rays' floats round differently; the test compares within 1e-3. No original to
  match (wrapping is the port's own).
- Corner copies (both axes at once) are not tested; they come from the same shift list `Tiled::sweep`
  and the moving list's copies use.
- A prop met at both its own place and its copy in one strike (a world narrower than twice a ray's
  reach) is folded into one record by code, untested: no world that small exists.
- Only the bike's body-ray strike crosses a seam. Still at the props' own places only: the props' own
  probes against the bike's frame (B29), the props' terrain search, the wheels standing on props (S210,
  `lend_props`), and props rolling over a seam (nothing wraps a prop). A struck prop's late search runs
  where it stands, so its probe will not find a bike a width over.
- In the props' step a striker is moved for every body as a whole: a bike that strikes one body across
  a seam and another at its own place in one tick (or is probed by an unstruck prop by it) has the
  second one resolved against it a width off. Needs a prop on both sides within a ray's reach; untested.
- The seats' shared props step (S197c, `prop_seats`) does not read `PropSet::across`: on a wrapped world
  with shared seats, a ball struck across a seam steps against the bike a width off. Online/split
  screen on a wrapped world with props is not tested.
- The wreck bike's, the plane's and the ragdoll's strikes on props (prop_wreck, wreck_plane,
  ragdoll_scene) do not cross a seam: out of this task's files. Filed with the shared-seats step as a
  follow-up.

## Next

MD167a (filed): the shared-seats step, the other strikers and wrapping a rolling prop.
