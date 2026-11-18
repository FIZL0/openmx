# 1 — plane prop seams

Task MD254. 2026-11-18.

## What I did

- **The plane's knocks get seam copies** (`flight.rs`, `seam_copies`, called in `Flight::frame_struck`,
  so both `frame_props` and `frame_props_in`):
  - The plane's rays' sphere (`flight_search::reach` at its node) picks the seams (`tiles::seam_shifts`).
  - Each prop whose sphere is within reach of a seam's copy gets a knock a width back: mesh frame
    and centre moved, velocity and room kept. `flight::Posed` has a new `by` field (zero at the
    prop's own place).
  - Copies go after every prop's own knock, so on a world that does not wrap the list is the old one.
- **A copy's hit is its prop's** (`Flight::knocked`):
  - The struck order, the mirrors and the plane's entries' renaming all match props by
    (collection, object), not by knock index.
  - A copy's mirrors go back to the prop's place. `Strike::across` names the body.
  - Not-late path: `PropSet::strike_as(strike, me)` (MD167a). Late path (`frame_props_in`,
    `strike.by` set): `strike_then` already does the same bookkeeping.
  - The material table hears a copy's hit under its prop's knock id.
- **A plane that crosses a seam in its own frame**: after `set.pushed`, the plane's across notes
  are cut by the seam shift it took, and dropped when they reach zero. The plane is then on the
  ball's side already.
- **The props' step sees the plane a width over** (`prop_contact.rs`):
  - `across_flown` / `across_flown_back`: move a plane's body for the step and back, word for word
    where the step left a place alone (same rule as `across_back`; shared `body_back`).
  - `PropSet::step_flown`: the props' step with one plane as striker 0, using the above. Used by
    the Sim's plane lane (`lib.rs`) and the shell's lone plane (`openmx/src/vehicle.rs`). Before,
    both built the striker list by hand.
  - Shared seats (`prop_seats.rs`, `finish_flying`): a flying seat's notes go to `across_flown`
    first, the rest to `across_moved` as before.
  - `lib.rs`, `vehicle.rs` and `prop_seats.rs` are outside the task's file list. The changes there
    are a few lines each, needed so the step actually reads the notes.
- opentennis: nothing to take (a court does not wrap).

## What the original does

Nothing: no disc map wraps. Wrapping is the port's own (MD29). On a world that does not wrap there
are no seams, so no copies, no notes, and `step_flown` is the old striker list. The only change on
that path is `strike_as` in place of `strike`, which with no `across` only clears the plane's
notes (there are none).

## How it was proved

`crates/game-sim/tests/wrap_moving.rs`, `a_plane_by_the_near_seam_strikes_a_ball_by_the_far_one`
(free09's balls off the disc, the biplane off MX.PSR; ran here, not skipped):
- Finds a spot where the plane's rays strike a ball. Puts the far seam under it, plane a width back.
- On a world that does not wrap, from there, nothing is struck.
- `frame_props`, wrapped vs at the ball's own place:
  - the ball's list gets the same mirrors (kind, other, surface; normals and points within 1e-3,
    points at the ball's place);
  - the ball is woken; the plane is noted a width over; the ball's body and the plane's velocity
    after the frame match within 1e-3.
- `step_flown` on both: the ball's position, velocity and rates match within 1e-3; notes used up.
- A whole `Sim::step` (the late path, B34), wrapped vs own place: the ball and the plane's velocity
  match within 1e-3.
- The same tick with the plane at x 0.1 flying -x on a world 0.1 ft narrower: it goes over the near
  seam in its frame, and the ball still matches.
- Mutations, each failing the test, then restored: no seam copies (ball's list empty); `step_flown`
  not moving the plane (ball off by ~1 ft); no seam-crossing cut (ball off by ~1 ft).
- Also pass: prop_plane, prop_strike_paths, prop_wreck, prop_set, plane_meets_plane, flight_bikes,
  prop_knock, hash (game-sim), plane_seat_props (openmx).

## Not verified / not 1:1

- **No original to match.** Wrapping is the port's own. Only the non-wrapping paths are the
  original's; they are unchanged by construction (no seams, no copies), and the prop_plane tests
  against the s241 states still pass.
- **Across a seam is not bit for bit** the strike at the ball's place: the copy's frame is moved, so
  floats round differently. Compared within 1e-3.
- **The props' rays on the plane's hull are not seam-aware** (`prop_wreck::struck_on`, step 3 of
  `frame_props`): a ball by the far seam does not find the hull a width over. The test plane has no
  hull for that reason. Same gap MD167a left for the wreck hull; `prop_wreck.rs` is not this task's.
- **Planes flown together** (`frames_together_props`, split screen with two planes) get no seam
  copies: one body is shared by every plane's frame, and two planes on opposite sides would need it
  in two places. Filed as MD254a (with the hull gap).
- **Mixed hits on one prop**: if a plane meets a prop at its own place and at a copy in one search
  (only on a world narrower than the plane's reach), the mirrors are joined but the body is handed
  over at the first-named place only.
- **The seam list is picked by the rays' sphere only**, not the sweep from last frame's place. A
  fast plane right at a seam could miss a copy the sweep would reach. Same rule as MD167.
- **The shared-seats change is not run on a wrapped world.** `finish_flying` with a flying seat
  uses `across_flown` the same way, and plane_seat_props passes, but no test strikes across a seam
  there.
- **The shell's lone plane (`vehicle.rs`) is covered by code only**: it calls the same
  `step_flown` that the Sim test runs.
- **One shift per striker** (as MD167): a plane that struck one body across a seam and another at
  its own place in one tick has the second resolved against it a width off in the props' step.
