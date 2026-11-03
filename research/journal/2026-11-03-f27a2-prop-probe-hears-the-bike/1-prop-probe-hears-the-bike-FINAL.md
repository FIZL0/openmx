# 1 — prop probe hears the bike (FINAL)

Task F27a2. 2026-11-03.

## What I did

- `flight_search::bikes_struck(rays, node, categories, bikes)` runs a prop's ray probe, placed at
  its search node, against each ridden bike in the world. It uses the same pair test as `Scene`'s
  bike: `bikef.col` on the chassis transform, ray tree against mesh tree. The bikes are skipped when
  the probe's category list does not let `BIKE` (0x9a048) through. The ray test's answer
  (`ray_probe::pair`, true on any facet hit) does not depend on the record, so each bike is tested
  with a fresh record, apart from the scene's.
- `Collection::search_object_hulled` keeps the node and the category list the probe searched with.
  After the search, it appends `(object, Struck::Bike(player))` for each bike struck. It does this
  for the same probes the material listener is on, and after the scene's own sounds.
- The ridden bikes come from `material_hit::Ridden` (mesh and `flight_search::Bike` per player),
  carried as `HeardBy`'s second field. That makes them sound only: equal to anything, off the wire
  and the hash.
- `PropSet::bike_frame` is the mesh, a constant of the ride. It sits beside `shapes`, which a wire
  decode keeps. `PropSet::hear_ridden(bikes)` hands it to every collection with the tick's bikes.
- Who fills it:
  - `Sim::hear_ridden` (prop_wreck.rs), called at the top of `frame_held_wrecked` and in `Sim::step`'s
    plane branch: every player's `ride.transform()`, in the world while up and not a ghost, and
    player 1's bike out while the plane flies (S95).
  - `Shared::take` does the same for split seats, by seat.
- The shell sets `bike_frame` from the `bikef.col` it already reads for the biplane.
- `world_search::Categories::allows` is now `pub(crate)`.
- `hash.rs`'s size pin is now 12208 (the new `Option<Arc<Mesh>>`); the leaf count is unchanged.

## What the original does

Read off knock-2881's RAM with `prop_knock.rs`'s `queued()` (the props node ticked by the original,
then the table's queue at +0x80/+0x88/+0xa0):

- entry 0: probe 0xda2ff0 (owner the bike's record 0xda3f00, category 0x18008) struck ball 2's mesh
  collidable (category 0x18, material 21);
- entry 1: probe 0x11c23e0 (owner ball 2's record 0x11c3760, category 0x8) struck 0xda3180, whose
  owner is the bike's record, category 0x9a048 (`flight_search::BIKE`), material 16.

So the ball's probe meets the bike's own frame collidable, the one the plane's search meets (N22b).
On knock-2882, 2883 and 2897 the original queues no such entry.

## How it was proved

- New test `prop_knock.rs::the_balls_probe_is_heard_striking_the_bike` (knock-2881):
  - the original's queue is `[(bike rec, 21), (ball rec, 16)]`, and the second entry's struck
    collidable is the bike record's, category `BIKE`;
  - the port's search with no bikes heard hears nothing from the props (the gap);
  - with the bike's frame heard at its record's node: the props' collections are equal to the
    unheard search's, so hearing changes no simulation state;
  - the heard list is `[(Bike(0), Prop{2,0}), (Prop{2,0}, Bike(0))]`;
  - the queue's (owner material, struck material) is `[(16, 21), (21, 16)]`, the original's entry
    by entry, and the two are each other's pair;
  - `play` keeps one, at the faster owner's place. The ball is the faster (21.93 vs 19.09 ft/s), so
    that is the ball's place.
- `the_strike_is_queued_as_the_original_queues_it` (knock-2882) now runs with the bike's frame
  heard too, and the props still hear nothing, as in the original.
- In a throwaway test (removed), knock-2883 and 2897 run the same way: neither side has a ball → bike
  entry.
- `./kit check -- -p game-sim`: everything passes except `prop_near`'s
  `a_woken_props_tick_asks_a_world_of_tiles_only_for_the_shapes_near_it`. It fails at tick 6, the
  same failure as on HEAD (memory: prop_near red on HEAD 11-03).
- `-p openmx --test material_hit_sound` passes.

## Not verified / not 1:1

- **Every ridden bike in the world is tried, not only the ones in the prop's kept nearby list.** In
  the original, the bike is struck only if the prop's last gather put it in that list. The port's
  list is gathered without the bikes, so a bike that arrived after the last gather could be heard
  here and not there. On the four knock images the two agree. No capture with a bike entering a
  sleeping prop's reach was checked.
- **The hit wakes nothing here.** The bike's category has the wake mask's bit (0x40), so in the
  original the ball's probe striking the bike also runs its hit listener (a wake) and its
  collection's listener. The port leaves both out, as the brief says, because the props already meet
  the bikes through the strike's mirrors and match the captures that way. Whether these listeners
  ever wake or activate something the strike does not was not checked.
- **Order within a collection.** The bike entries go after the scene's sounds for the same object.
  In the original, the bike's place in the nearby list decides the order. Order only matters for
  `drop_pairs` on a tie.
- **A wrapping world (MD44)** hands each bike's raw transform, not its nearest copy.
- **Only the human's bike** was given in the tests. The knock images' three AI bikes were not placed.
  The shell gives every seat's bike, and a Sim gives every player's.
- **No playback heard.** No shell recording of a knock was matched against F27a1's capture after
  this change. B27 (the stacked repeat voices) is still open and would mask it.

## Next

Nothing.
