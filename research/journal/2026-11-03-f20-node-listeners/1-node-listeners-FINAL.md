# 1 — node listeners (FINAL)

Task F20. 2026-11-03.

## What I did

- `PropSet::listeners: Vec<(usize, Listener)>`: the world node's one list, each entry its collection
  and its listener (S242 found the list shared by every free09 prop; S242 follow-up).
  - `PropSet::file_on(c)` / `file_listeners()` put on, at the list's end, a collection's listeners
    that are not on yet, in the order the collection put them on. They run after each collection's
    search (`search`, `search_among`, `search_in_among`), after a striker's own wake in
    `prop_props::strike_props` (before the struck prop's), in `PropSet::wake` / `struck`, and once
    more before the walk. A wake puts on only its own collection's listeners. The walks wake in
    collection order, so filing collection by collection keeps the put-on order.
  - `Collection::listeners` stays the collection's own: which of its listeners are on (the dedup
    `listen` reads), and the list a collection ticked alone walks.
  - `PropSet::walk_listeners`: after every collection's step (`step`, `step_with`), each listener in
    the node's order. A finished one is taken off by `swap_remove` (the list's last into its slot,
    read again: node_listener_remove) and dropped from its collection's own list.
- `Collection::step_world_among`: `step_among`'s world step without the listeners. `step_named` uses
  it, so in a set every collection steps before any listener runs. This is the node update's order:
  collide, step, children, then listeners (oracle S198). Before, the port ran each collection's
  listeners right after its own step.
- `Collection::hear(listener, dt)`: one listener heard. A collection's own walk (a collection ticked
  alone, the Kiowa's blades, a physics car) now removes with the node's swap as well.
- Hash: `prop_listeners`, the node list in order, only on a track with props. Wire: the list follows
  the collections in the props section. Decode checks it holds each collection's listeners once and
  no others. `Sim` is 24 bytes bigger (tests/hash.rs pin 12120 -> 12144; the leaf count is unchanged
  because the walk runs only with props).
- tests/prop_knock.rs:
  - `seed` sets the set's node list from the state's node list, in its order.
  - `resync` puts the resynced ball's listeners at the list's end.
  - `the_knocked_balls_fly_land_sleep_and_go_back`: ball 2's body sleeps at 3274 and its collection
    at 3275 (the game's `b2_active` 2 then 0). It is put back at 5075 and agrees to the capture's
    last row (5538). It parted at 3274 before.

## What the original does

The props' world node (collision_node_update, `./kit fn 0x0026f800`): clear and collide every filed
object, step, update the children (each collection's world step), then walk the node's listener list
(node +0x48 count, +0x50 items). Each listener's vt+8 is called. One that answers done is removed by
`./kit fn 0x002735d0`: `list[i] = list[count-1]`, `count -= 1`. `i` is not advanced, so the moved one
runs next. Every free09 prop's collection settle and object listener are on that one list.

In knock-2884 the node's list is `[X, O1, S1, O2, S2]`, where X (0x00e1e2b0) belongs to no prop
collection. At 3010 ball 1's settle comes off and S2 takes its slot: `[X, O1, S2, O2]`. On 3274 ball
2's settle runs before its object's listener. It reads the body still awake, so the collection sleeps
on 3275.

## How it was proved

A save-state capture with a bit-exact replay: s195 knock (`context/capture/s195/knock.csv`, state
`s195-knock-2884`, s3's context), tests/prop_knock.rs `the_knocked_balls_fly_land_sleep_and_go_back`.
Ball 2 agrees bit for bit from row 46 (frame 2885) to row 2699 (frame 5538), every lane the capture
has. That includes the body's sleep at 3274, the collection's at 3275 (asserted on the game's rows),
the sixty-second countdown and the put-back at 5075. Ball 1's run is unchanged.

Counter-check: with `walk_listeners` removing in order (`remove` for `swap_remove`), the test fails:
ball 2 agrees only to row 434 (frame 3273), as before.

Also run: game-sim prop_collision, prop_knock, prop_near, prop_plane, prop_props, prop_set,
prop_spring, props, props_search, prop_strike_paths, prop_under, prop_wake, prop_world, prop_wreck,
ragdoll_ball, ragdoll_rest, snapshot, hash, flight_bikes, kiowa, physics_cars, physics_car_struck,
state: all pass. openmx netplay (60 passed, 1 ignored) and plane_seat_props (10 passed).

## Not verified / not 1:1

- X, the node's one listener that is not a prop's (0x00e1e2b0 in knock-2884), is not in the port's
  set. If it ever came off, the original would move the list's last into its slot and the port would
  not. In knock it stays on throughout. What it is was not looked up.
- The put-on itself (`./kit fn 0x0026f7d0`) is taken to append at the end, as the state's order reads
  (O1 S1 O2 S2 after the strikes). Whether it dedups as the port's `listen` does was not read.
- A wake outside the set's walks (`Collection::activate` / `enable` called directly on a set's
  collection) is put on at the next filing, in collection order rather than wake order. Every
  set-side caller files right after.
- The collections' step order is still the order they were made (S242's note). The listener walk is
  now after all of them, as the node's is.
- A collection ticked alone now removes with the swap. This matches the node's remove, but no capture
  of such a collection (Kiowa blades, physics cars) has three or more listeners with a middle one
  coming off. Their tests pass unchanged.
- Not looked at on screen.

## Next

none
