# 1 — the wreck bike's own rays on the props' node (FINAL)

Task S260a. 2026-11-03.

## What I did

- `prop_wreck::wreck_rays_first(set, wrecks)`: each wreck bike's rays on the props (S244's
  `wreck_rays_on_props`, now through `wreck_rays_at`, where the hull stands, capacity
  `wreck_bike::rays().len()`), each struck prop woken (`PropSet::struck`) before the props search.
  The original's node lists the wreck bike's object ahead of the balls', so it searches first.
- `prop_wreck::wreck_rays_filed(set, rayed, by_props)`: after the props' searches, the mirrors go at
  the head of each struck prop's list (`PropSet::strike`, wreck bikes in reverse so the first is at the
  head), and each wreck bike's strike is joined with its own entries and struck bodies (`joined`, as
  `strikes()` does).
- `PropSet::search_among` and `PropSet::search_in_among` call both. That covers
  `Sim::frame_held_wrecked`, `prop_seats::Shared::held` and `Shared::finish`'s fallback pass. Nothing
  else calls the after-pass `strikes()`/`wreck_strike` any more, so nothing strikes twice.

## How it was proved

tests/prop_wreck.rs (s3's hit2/hit3/hit5 states, none skipped):
- new `the_wreck_bikes_rays_strike_the_ball_on_the_node`: hit3-2829..2835 through `tick(Way::Node)`.
  Both lists, the kept-list distance, the ball and the wreck bike bodies are bit for bit. The wreck
  bike's entries come first at 2832.
- `the_seats_wreck_bikes_are_on_the_node` now runs hit3-2829..2835 through the shared seats as well.
- Counter-check: with `wreck_rays_first` answering empty strikes, both fail at hit3-2832 (ball's list
  0 vs 4 entries, wreck bike's 1 vs 5).

Also run: game-sim hash, prop_* (13 binaries), ragdoll_props, snapshot (76 passed); openmx netplay
(60) and plane_seat_props (10).

## Not verified

- With two wreck bikes striking one prop, I put the mirrors in node order (first bike at the head).
  I took that order from the original's list order and did not check it against a state.
- Not run online or in split screen with a wreck at a prop. Not looked at on screen.
