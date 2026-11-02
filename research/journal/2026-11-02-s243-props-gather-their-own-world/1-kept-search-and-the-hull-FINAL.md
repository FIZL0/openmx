# 1 — a kept search per prop, and the wreck bike's hull in it (FINAL)

## What the original does (hit2 states, RAM read with a scratch script)

- Each prop collision object's record `+0x10c` is a nearby search with least 10, extra 30, capacity 128
  and no_groups 0, the plane's values (`flight_search::search()`). It is kept between ticks.
- The body wrapper's vt `+0x110` (`0x0015b210`, the oracle's plane_refresh_search; the props use the
  plane's wrapper class) moves the probes and gathers around the ray probe (`broad_phase::gather`, kind 3).
  It is called from three places:
  - the post `0x0015b070`, only while the body is enabled, at the stepped pose;
  - the asleep check `0x0015b2a0` (the object listener), while the body sleeps with `+0x6c` detect set;
  - the wake `0x0015b790`.
  The gather only resorts while the probe stays inside the list's bound.
- hit2-2828, ball 1's list (26 entries) holds every moving-list object in reach:
  - the awake balls' meshes, category 0x18 (flags 0x14);
  - the wreck bike's mesh collidable `0xdc9030`: category 0x9a058, radius 4.303, owner (+0x80) the wreck
    bike's object `0xdc98f0`;
  - toys (0x48), 0x20a, 0x52058, the AI bikes (0x9a048) and the gates (0x200020).
  Ball 2's list has the wreck bike at distance 0. The list's order is balls, the wreck, then toys.
- Sweep probes: the prop's sweep and the wreck bike's both exclude `[0x8, 0x800]`. The globals are
  `0x00468f50` = 0x8 and `0x00468f90` = 0x800; the wake mask `0x00468f68` = 0x40. So a prop's sweep never
  meets the wreck bike (0x9a058 has 0x8) or a toy (0x48), and the wreck bike's sweep never meets a prop
  (0x18).

## What the port does now

- `prop_world::Collision` has new fields:
  - `nearby` (the kept search);
  - `regather` (`Option<Categories>`: a gather owed, with the ray category list the first owed gather
    read);
  - `wrecks` (the hull count the list was gathered with; the plants' indices come after the hulls).
  They are on the snapshot wire but not in the hash walk.
- The wake (`Collection::enable`), the post (`Collection::post`, when it writes the node) and the
  asleep branch of the object listener (with detect set) mark `regather`. The next search does that
  gather (`Scene::of`, `Probe::regather`) before it searches. A list never gathered, or one naming more
  collidables than the world has, is gathered there too, as the plane's.
- `PropSet::search_in_among` forgets a list whose centre lies in another near set than the collection's
  key, as `flight_search::near_world` does for the plane.
- A wreck bike's hull goes into the prop's world through the planes' machinery (`Scene::probed_among`).
  `Probe::hull` gives those collidables the wreck bike's category and collides-with; `Lent`'s drop puts
  the plane's back. A hit names record `1 + j`. `Collection::search_among` hands `collide` a stand-in
  record per wreck, renames the prop's entries' other side to `Wreck::side`, and turns the stand-in's
  mirrors into that wreck's `WreckStrike`. The struck body is copied after the listeners ran.
- `Probe::sweep` = `prop_world::sweep_categories()` (`[0x8, 0x800]`, exclude) on every prop search.
- `Sim::frame_held_wrecked` builds the downed riders' hulls (same conditions as `wreck_strike`) and puts
  the strikes on `crash.bike.struck`. It replaces `wreck_strike` in `Sim::step`, `step_all`,
  `step_bumped` and the unshared `step_among`. Shared seats keep `wreck_strike_as` (S197c's stand-in).

## Checked

- `tests/prop_wreck.rs`: the new `the_wreck_bike_is_in_the_balls_world` runs hit2-2828..2831 through
  `PropSet::search_among` with the hull. Both lists and both bodies match bit for bit, as S197b's
  stand-in path still does (its test kept). The hull's distance in the ball's kept list equals the state's
  for the wreck bike's entry: 0.0, 0.0, then 0.27134562 on 2830.
- `tests/prop_near.rs`: the kept list is the gather's walk of a tree built over the set it is handed, so
  over a near set its entries, bound and requery ticks differ from the whole world's. `ridden` now leaves
  the kept search out; the props themselves still match tick for tick over 600 ticks.
- `tests/prop_wake.rs`: the egg test is what forced `regather` to carry the first gather's categories.
  The wake gathers with every category, so the sculpture stays in the list, and the asleep check's later
  gathers only resort it. Waking the egg when the sculpture turns into a toy then matches the original.
- `./kit check -- -p game-sim`: see the commit message.

## Not verified / not 1:1

- The gathers are done at the next search, not at the post, wake or asleep check. The pose is the same
  (nothing moves an object between them). The toys have moved by then, though (`toys.frame` runs
  first), so a resort's distances, and a requery's entries, are measured against this tick's toys rather
  than last tick's. Order only matters on ties (the ray record keeps the nearer contact per ray).
- If the probe moves between an owed wake gather and the post's (woken, then stepped), the one gather
  runs at the stepped pose with the wake's categories. With detect set those are every category, the
  same as the post would read.
- The port's moving list is toys, probes, hulls, bikes; the original's is balls, the wreck, toys. The
  AI bikes, the gates and the other awake balls are not in a prop's world (S239/S242).
- The hull's sphere comes from `Prop::posed` (the mesh's root box through the node), and its velocity
  from the wreck bike's body. Neither is checked beyond hit2.
- When the wreck-hull count changes, the list is forgotten and gathered afresh (the plants' indices
  shift). The original's list would just gain or lose an entry at its next gather.
- Shared seats (`prop_seats`) still meet the wreck bikes through S197b's after-pass search (filed as S258).
