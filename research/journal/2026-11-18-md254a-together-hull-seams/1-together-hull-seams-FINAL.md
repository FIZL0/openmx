# 1 — together hull seams

Task MD254a. 2026-11-18.

## What I did

- **The props' rays meet a hull across a seam** (`prop_wreck.rs`, new `struck_on_in`):
  - `struck_on` is now `struck_on_by(.., None)`; `struck_on_in` passes the world.
  - Per prop: the hull's own place first. If that gives no hit, the hull's copies by the seams
    the prop's sphere reaches over (`tiles::seam_shifts` on the prop's centre, prop reach + hull
    radius). A copy is tried only if the prop's rays can reach it.
  - Entries on the prop stay at its place. The mirrors for the hull's list move back to the hull's
    place. Also answers `(place in struck, shift)` for each body met across.
  - On a world that does not wrap there are no seams, so the strike is `struck_on`'s word for word
    (the test checks this on a wide wrapped world too).
- **The lone plane uses it** (`flight.rs`, `Flight::knocked`, step 3 of `frame_props`): a body
  whose rays met the hull across a seam goes to the plane's frame a width back and is noted in
  `PropSet::across`, as the plane's own rays' copies are (MD254). A body the plane's rays struck
  already keeps their place.
- **Planes flown together get seam copies** (`frames_together_props`):
  - Each plane's search has its own knocks: every prop's own place, then that plane's seam copies
    (`seam_copies`, as MD254). `search_together` takes a knock list per plane.
  - The struck bodies stay one list for the fleet, at their own places. `knocked` in `shared` mode
    does not move them; it notes them with `pushed: true` (so `PropSet::pushed` does not move them
    back) and answers each body's place and shift.
  - In the joint world step, each plane's force listener moves the bodies it met across a seam back
    by its shift, resolves, and puts them back (`prop_contact::body_back`, now `pub(crate)`: word
    for word where the resolve left the place alone). So two planes on opposite sides of a seam
    push one body, substep by substep.
  - After the step, a plane the frame carried over a seam has its notes cut by that seam, as the
    lone plane's (MD254).
  - The props' step after was already right for several planes (`prop_seats::finish_flying` uses
    `across_flown` with every seat's plane, MD254).
- **A plane's mirror on another plane stands at that plane** (`search_together`): on a wrapped
  world the mirror's point is moved to the copy nearest the struck plane (`nearest_copy`), as the
  wreck bikes' mirrors already were. Before, a plane by the near seam left its mirror a width from
  the other plane; the new test found it (the planes' velocities drifted ~0.1 ft/s apart).
- `prop_contact.rs` (one word, `body_back` made `pub(crate)`) is outside the task's file list.
- opentennis: nothing to take (a court does not wrap).

## What the original does

Nothing: no disc map wraps. Wrapping is the port's own (MD29). On a world that does not wrap:
no seams, so no copies, no notes, `struck_on_in` is `struck_on`, and the shift lists are empty.
The together path's knocks per plane are the same list for every plane, as before.

## How it was proved

`crates/game-sim/tests/wrap_moving.rs` (free09's balls off the disc, the biplane and its hull off
MX.PSR; ran here, not skipped):
- `planes_flown_together_strike_a_ball_by_the_far_seam` (hulls on):
  - Plane A by the near seam, the ball by the far one; plane B on the ball's other side, inside the
    world. Against both at the ball's own place, on a world that does not wrap.
  - Both fleet orders: the ball's list (kind, other, surface; normals and points within 1e-3), the
    note for A only, the ball's body, both planes' places and velocities, then the props' step with
    both planes: the ball and both planes within 1e-3.
  - Plane A at x 0.1 flying -x on a world 0.1 ft narrower: it goes over the seam in the frame, its
    note is used up, and the ball and planes match before and after the props' step.
  - On a world that does not wrap, plane A by the near seam names nothing on the ball's list.
- `the_balls_rays_meet_a_hull_by_the_far_seam`:
  - A filed ball whose rays meet the hull at its own place: with the hull by the near seam, the same
    entries at the ball's place, the mirrors a width back at the hull, the shift answered.
  - Nothing on a world that does not wrap; `struck_on_in` equals `struck_on` where the own place hits.
  - A lone plane's `frame_props` with the hull: the ball's list, the note, the ball and the plane's
    velocity match.
- Mutations, each failing the tests, then restored: no per-plane seam copies; no shift in the
  together force listener; no seam cut after the together step; the hull pass back to `struck_on`;
  hull mirrors not moved back. The mirror fix above was found by the test failing without it.
- Also pass: prop_plane (the s241 states, `struck_on` against the original), plane_meets_plane,
  flight_bikes, prop_wreck, prop_strike_paths, hash (game-sim), plane_seat_props (openmx).

## Not verified / not 1:1

- **No original to match.** Wrapping is the port's own. The non-wrapping paths are the original's
  and are unchanged by construction (no seams); prop_plane's hull test against the s241 states
  still passes.
- **Across a seam is not bit for bit** the strike at the ball's place: frames are moved, so floats
  round differently. Compared within 1e-3.
- **The wreck bike's hull is not covered.** `struck_on`'s only wreck bike caller is
  `struck_by_props`, the test-only stand-in. The live wreck bike is a collidable in the props' own
  search on the node (`prop_world::Wreck`, `prop_set.rs`), which still meets props at their own
  places only (as MD167a left it). Not this task's files.
- **The car's hull** (`car.rs`, `struck_on`) still meets props at their own places. Not this
  task's file.
- **One shift per plane per body**: a body a plane's rays strike at its own place and its hull's
  rays meet across a seam (only on a world narrower than their reach) keeps the rays' place.
- **A hull is tried at a copy only when its own place gives no hit**, and only copies the prop's
  sphere reaches; a prop that could meet the hull at two places keeps the own one.
- **The seam cut in the together path is caught by the note assertion**, which runs first; whether
  the ball's place alone would catch a missing cut was not checked.
- **The material table (`heard`) is not renamed in the together path**: `search_together` does not
  fill it at all, before or after this task.
- **The planes' mirror fix is checked by this test only**; plane_meets_plane passes, but no test
  there runs on a wrapped world.

## Next

Nothing.
