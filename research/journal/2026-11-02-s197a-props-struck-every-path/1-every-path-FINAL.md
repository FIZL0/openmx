# 1 — every step path strikes (FINAL)

Task S197a. 2026-11-02.

## What I did

- `Sim::step_all`: the traffic runs held (`Traffic::frame_held`); in the walk (last player first) each
  player's strike (`Sim::prop_strike(i)`) is taken just before its frame, from where the riders walked
  before it left them, put on the props and in its bump (`Sim::strike_into`: `PropSet::strike`, the
  entries after the rider and car contacts), and written back after its frame (`PropSet::pushed`).
  After every frame, `Sim::props_step` with every player's chassis as a striker, the mirrors naming the
  player by index.
- `Sim::step_bumped` and `Sim::step_among` (seats): the same in one Sim. The seat's props are its own
  copy, so its strike's mirrors name its own bike as player 1 of its Sim (the one striker its
  `props_step` builds), and its entries name the struck bodies after every seat's vehicle in the bump
  (`Sim::prop_strike_seated(players)`). `step_among` with no bump but rivals builds its own bump for a
  strike, as it does for the cars.
- openmx: its bike step is `Sim::step_among` (`Garage::step_among`, `net/rides.rs`), so it strikes now;
  its plane branch still calls `Traffic::frame` with no strike, as `Sim::step`'s plane branch steps the
  props with no strikers (the plane is S197b's).
- tests/prop_strike_paths.rs (disc only): a free09 ball and a bike stood where its rays strike it
  (the first spot found), the other rider 200 ft off. `every_player_in_a_session_strikes` (either
  player of a two-player `step_all`), `a_seat_strikes_its_own_props` (`step_bumped` and `step_among`
  with online's bump, either seat), `a_seat_with_only_rivals_strikes` (`step_among`, no bump): the
  ball woken, its mirrors naming the striker, and its body equal to one-player `Sim::step`'s. All three
  fail on the old paths (the ball sleeps).

## Not verified / not 1:1

- No 2p knock of the original was captured: the paths are proved equal to S197's one-player strike
  (itself proved on S195's knock), not against the original's two-player props node.
- Two riders striking one prop in the same tick: each strike is taken just before that rider's frame,
  so the body the second striker pushes is the first's push; the mirrors go on the list in the walk's
  order (the later-walked player's ahead). The original collides every record in one pass in its world
  node's list order; not checked.
- The seats' props are per-seat copies (filed S197c): a seat's bike strikes only its own copy, other
  seats' bikes do not, and the shell draws seat 0's copy only.
