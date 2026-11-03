# 1 — the shared seats' wreck bikes on the node (FINAL)

Task S260. 2026-11-03.

## What I did

- `Sim::wreck_hull(i)` (prop_wreck.rs): player `i`'s wreck bike as a hull on the node (its mesh off
  the ragdoll kit, its record's node as the pose, its body's velocity), or nothing for a rider who is
  up, a part with collision off, or no kit mesh. `Sim::frame_held_wrecked` now uses it; the checks it
  makes are the ones it made inline.
- `Shared::take` keeps each seat's hull (seats not flying `Sim::plane`), before any seat steps. The
  original's node collides every filed object before any bike's frame, and `Sim::step_all` gathers its
  riders' hulls before the first frame too.
- `Shared::held`, for the first seat (bike or plane, through `Shared::lend`), runs
  `Traffic::frame_held_among` with those hulls, each named `seats + seat` (the side `Shared::finish`'s
  step gives that seat's wreck bike). The strike for each seat's wreck bike is kept, by seat.
- `Shared::wreck_struck(sim, seat)` hands a seat's wreck bike its strike (`WreckBike::struck`).
  `Sim::step_seat` calls it where it used to call `wreck_strike_as`.
- `Shared::finish`, when no seat ran the collide pass, runs it with the hulls too
  (`search_in_among`); no frame is left to take the strikes then, so they are dropped.

Nothing from opentennis: it has no props.

## How it was proved

Save states with bit-exact checks: tests/prop_wreck.rs `the_seats_wreck_bikes_are_on_the_node`
(`seats_tick`). Two seats share free09's props, seeded from the state. Seat 1 is up, far off. Seat 2
is down with the state's wreck bike. Each tick runs the seat step's prop phases in order: take, seat
1's lend/back (the collide pass), seat 2's lend, `wreck_struck`, the wreck bike's frame,
`wreck_pushed_back`, back, then `finish`. It is checked against the next state: the ball's list in
both seats (the wreck bike named `seats + 1`), the hull in the ball's kept nearby list at the state's
distance (S243's check), the ball's body in both seats and the wreck bike's body.

- hit2-2828..2831 (s197b): the ball's rays strike the wreck bike. All match bit for bit.
- hit5-2851..2857 (s244): ball 4's impulse throws the wreck bike in the set's step at 2855. All
  match bit for bit.

Counter-check: with `Shared::take` keeping no hulls, the test fails at hit2-2828 (nothing strikes
the wreck bike). The kept-list distance check is what tells the node path from the old after-pass
search. On hit2 the old path's lists and bodies matched too, but it never puts the hull in the
ball's kept list.

Also run (`./kit check`): game-sim hash, prop_collision, prop_knock, prop_near, prop_plane,
prop_props, prop_set, prop_spring, prop_strike_paths, prop_under, prop_wake, prop_world, prop_wreck,
ragdoll_props, snapshot (73 passed). openmx plane_seat_props, netplay and the bin (307 passed).

## Not verified / not 1:1

- hit3 (s244) from 2832 does not match through the shared seats. That tick needs the wreck bike's
  own rays on the ball (`wreck_rays_on_props`). The node path does not run them, for one Sim
  (`frame_held_wrecked`, since S243) or for the seats. The old shared stand-in (`strikes()`) did run
  them, so here the shared seats lose them, to match one Sim. Filed as S260a.
- Online and split screen were not run with a wreck near a prop. The test drives `Shared`'s phases
  by hand, in `step_seat`'s order, not through `step_shared` with a full rider frame.
- A seat flying `Sim::plane` gets no hull. Split screen's plane lives in the garage, not
  `Sim::plane`, so such a seat is kept only if its rider still has a crash. As far as I can see a
  flying rider has none, but I did not check it.
- When no seat runs the collide pass, `finish` drops the wreck bikes' strikes, because no frame is
  left to take them.
- Not looked at on screen.
