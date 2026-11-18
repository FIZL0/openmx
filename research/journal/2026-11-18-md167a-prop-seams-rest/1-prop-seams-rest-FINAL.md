# 1 — prop seams rest (FINAL)

Task MD167a. 2026-11-18.

## What I did

- **Shared seats' step** (`prop_seats.rs`, `Shared::finish_flying`): reads `PropSet::across` as
  `Sim::props_step` does. Both now go through one helper, `prop_contact::across_moved` /
  `across_back`.
  - A riding bike that struck across a seam: its vehicle copy's node origin moves `by`, as in MD167.
  - New: a wreck bike (`players + i`, or `seats + seat`) that struck across a seam. Its body moves
    `by` for the step. Afterwards it moves back: a place the step did not change gets its old bits
    back (small coordinate plus a width rounds, so subtracting would not be exact); a changed one
    is moved back.
- **Wreck bike's rays** (`prop_wreck.rs`): `wreck_rays_at` takes the seams. A prop by a seam's far
  side gets a copy a width back, as `prop_contact::rays_strike` does for the riding bike. A copy's hit
  is its prop's, its mirrors go back to the prop's place, and `Strike::across` names it.
  `wreck_rays_wrapped` is the public entry. The props' collide pass in a world
  (`PropSet::search_in_among`) calls `wreck_rays_first_in`. `search_among` (no world) keeps the
  plain strike.
  - `PropSet::strike_as(strike, player)` (prop_contact.rs): `strike` plus the across bookkeeping
    under the given striker. The riding bike's path uses the same code (`across_struck`, moved out
    of `strike_then`, same behaviour). `wreck_rays_filed` puts each wreck bike's strike on with it,
    under its side. So its frame gets the ball a width back, and `PropSet::pushed`
    (via `wreck_pushed_back`) moves it back.
- **Ragdoll** (`ragdoll_scene.rs`, `KnockedProp::across`; called in `ragdoll_ride.rs`): a knockable
  prop, asleep or awake, that stands again a width over within the bones' reach gets a copy. The
  copy's mesh frame and body are moved back by the seam's shift. It goes after the resting and awake
  lists, with no static-list place. The bones never push a prop (the body is a copy), so nothing goes
  back. A sphere record the copy takes wakes the real prop.
- **A rolling prop wraps** (`prop_world.rs`: `Collection::open_seams` / `close_seams`;
  `prop_set.rs`: `PropSet::seams`, set from the world in `search_in_among`, used in `step_with`):
  - During each collection's step, the edge check is left off on wrapped axes (the edge is the
    world's area, so a ball over a seam used to be reset as if off the edge).
  - After the step the edge goes back, and each body past a seam moves a width round: centre now and
    at the last substep, node origin, collision record (`seam::record`). Its kept nearby list is
    forgotten, so it gathers fresh.
  - Bodies pinned together by joints move as one group, decided by the group's first body. A group
    with a hold to the world does not move. The put-back place is left alone.
- `tests/hash.rs`: Sim's size pin goes 12328 → 12344 for `PropSet::seams`. It is not state: it is
  read again from the world every pass, and is not walked or sent.
- Plane: its strike on props is in `flight.rs` (`Flight::frame_props`, `knocks`), outside this
  task's files. Filed as **MD254**, which can reuse `strike_as` and `across_moved`.
- opentennis: nothing to take (a court does not wrap).

## What the original does

Nothing: no disc map wraps. Wrapping is the port's own (MD29). On a world that does not wrap, every
changed path is the old one: the seam lists are empty, `strike_as` notes nothing, and `PropSet::seams`
is `None`. The test checks this word for word for the wreck rays.

## How it was proved

`crates/game-sim/tests/wrap_moving.rs`; the disc-backed tests (free09's props) ran here with the disc.
- `a_wreck_bike_by_the_near_seam_strikes_a_ball_by_the_far_one`:
  - Setup: a wreck probe ahead of a ball, where its rays strike it; a world wrapped on `x` whose far
    seam is under the probe; the probe moved a width back.
  - Alone it strikes nothing. Wrapped, it strikes the same ball with the same entries and mirrors
    (normals within 1e-3, points within 1e-3 after the width). Mirrors stay at the ball's own place.
    `across` names the ball.
  - `strike_as` hands the ball a width back and notes it under the striker. `pushed` restores it bit
    for bit.
  - On a world that does not wrap, the strike equals the plain one word for word, and nothing is
    noted.
- `the_props_step_moves_a_wreck_bike_that_struck_across_a_seam_and_back` (synthetic):
  - The wreck body at side `players + 1` is moved once (two notes, one striker). The other wreck body
    and the vehicles stay.
  - Unchanged places come back bit for bit, even where 0.3 + 1134 rounds. A changed place moves back
    by the width.
- `the_ragdoll_is_offered_a_ball_by_the_far_seam_one_width_over`: a ball 1–2 ft short of the far
  seam and a probe 0.5 ft past the near one.
  - `near` and a seamless `across` offer nothing.
  - `across` offers the ball, asleep and awake. Its frame rows match `near` from a width over bit for
    bit; its origin and body are a width back (within 1e-3); its distance is within 1e-3.
- `a_rolling_ball_wraps_over_a_seam`: a ball a foot short of the far seam, rolled at it at 20 ft/s
  for 30 ticks of `search_in` + `step_with`.
  - Wrapped: it ends on the world, at the wide world's place less a width (within 0.02 ft, see
    below), velocity within 1e-3, node with it.
  - The edge is the world's again after the step, and the put-back place is unchanged.
  - On a world of the same width that does not wrap, it is reset to its place, asleep, as before.
  - Checked by mutation: with `close_seams`' wrap taken out, the test fails (the ball sits at x 1781).
- The prop test binaries next to this (prop_wreck, prop_strike_paths, ragdoll_props, prop_near,
  prop_plane, prop_set, prop_knock) and hash.rs pass.

## Not verified / not 1:1

- **No original to match.** Wrapping is the port's own; only the non-wrapping paths are the
  original's, and those are unchanged (wreck rays shown word for word; the rest by code: empty seam
  lists, `None` seams).
- **The wreck strike across a seam is not bit for bit** the strike at the ball's place: the copy's
  frame is moved, so the rays' floats round differently. Compared within 1e-3.
- **No whole-tick test of the wreck bike.** The across path through a whole tick (collide pass →
  wreck frame → `wreck_pushed_back` → props' step) is not tested end to end with a real wreck. The
  pieces are tested separately: strike, `strike_as`/`pushed`, `across_moved`/`across_back`. Setting
  up a downed rider with a ragdoll kit by a seam was more than this task's tests had.
- **No whole-tick test of the shared seats on a wrapped world.** `finish_flying` uses the same
  helper as `props_step`, but split screen / online on a wrapped world with props is not run.
- **The ragdoll path is not run through a real wreck.** Only `KnockedProp::across`'s list is tested,
  not the bones' push-out against a copy.
- **The rolling wrap matches the wider world within 0.02 ft, not 1e-3:** past 1,700 ft the wide
  world's ball rounds to 1.2e-4 a substep, while the wrapped one, back by 0, rounds finer.
- **A body wrapped over the near seam rounds** (a small negative coordinate plus a width). Over the
  far seam it is exact. Same as the rider's shift.
- **Wrapping forgets the body's kept nearby list.** It regathers there, a little different from the
  list it would have kept. Only on the tick it crosses.
- **Untested wrap cases:** jointed groups and world holds (free09's balls have no joints), and
  corner crossings (both axes).
- **The seam list is picked by the striker's reach only** (as MD167's): a large prop straddling a
  seam, met where its mesh pokes past the seam, can be missed.
- **The plane's strike does not cross a seam.** Filed as MD254.
- **Still only at the props' own places** (as MD167 left them):
  - the props' own probes against a bike's frame or a wreck hull (a ball struck across a seam, woken
    and searched in the same pass, will not find the striker a width over);
  - the props' terrain search;
  - the wheels standing on props (`lend_props`).
- **One shift per striker in the props' step** (as MD167): a striker that struck one body across a
  seam and another at its own place in one tick has the second resolved against it a width off.

## Next

Done; MD254 is the plane.
