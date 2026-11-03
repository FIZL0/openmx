# F27a: feeding the queue from the ride

F27 ported the table's update (`material_hit::play`) and `audio::HitTable`, but nothing queued hits.
F27a records what each listened probe heard during the tick's searches. The shell builds the queue
from that once per simulated tick and plays it.

## Where the listener goes

`flight_search::Scene` is the Narrow for all three probes (bike body rays, props, plane). The
original's listener is called once per struck collidable, whenever the pair test passes. The port's
`world_search::test` does the same through `heard_hit`, so the listener is added there:

- `MATERIAL_LISTENER` (2) sits beside HIT/COLLECTION. `Scene::listen_material` puts it on the RAY
  probe's list after the probe's own listeners.
- The `heard_hit` arm records a `material_hit::Struck`. A bike gives `Bike(k)`. Anything else gives
  `Mesh { id, material, foliage }`:
  - material: the knock / plane / prop mesh's surface low byte (`material_of`), or 0 for a trunk;
  - foliage: `category & FOLIAGE_MASK` (4, the word at 0x00468f48 in s195-knock-2881), which a trunk
    (TREE = 4) passes.
- `Lent::drop` already clears the RAY listeners, so the kept world never sees the new listener.

Who listens:

- **Bike body rays.** `rider_contact::scenery_heard` is `scenery_in` with the listener added. It
  hears the scenery search (meshes, toys, trunks).
  - The bike's hits on other riders, flown planes and props are not found by that scene. They come
    from the bump's own contacts (kind 0 entries) via `heard_struck`: one entry per distinct other
    side, in list order.
  - `Rider::step` puts both into `Ride::heard`, which is cleared each tick.
- **Props.** `Collection::search_object_hulled` adds the listener to each filed object's probe.
  That covers every object the search runs for: awake ones, and those filed asleep through
  `DetectCollisionsWhenDisabled`.
  - `Collection::heard` collects `(object, Struck)`.
  - It is cleared in `search_wrecked` / `search_among_hulled`, and by `PropSet::heard_afresh` beside
    each of the three `stamp_filed` calls.
- **Plane.** `Flight::search` (the flown plane's per-tick search) adds the listener and keeps
  `Flight::heard`. Scene bike `k` is mapped to its player through `Met::bikes` when there is one.

All three fields are sound only. `material_hit::Heard` / `HeardBy` compare equal to anything, the
snapshot skips them (`Ride`'s and `Collection`'s skip lists, `Flight`'s hand-written `Wire`), and no
step reads them. The 231 game-sim tests run (replay, snapshot, two_p*, prop_*, flight*, kiowa) and
the 71 openmx ones (netplay included) are unchanged.

## The shell

`main.rs`'s `HeardRide` collects, on a simulated tick, in this order:

1. each riding bike seat's `ride.heard` as `Owner::Bike(g)`;
2. each flown plane's `Flight::heard` as `Owner::Plane(seat)`;
3. the first seat's prop collections as `Owner::Prop`.

The seats share one prop set (S197c). With several seats, bike `g` is seat `g`, the index the
riders' shared contacts use. With one seat, `g` is its sim's player.

`HeardRide` implements `material_hit::Ride` from the end-of-tick state, which is when the table's
update reads the owner:

- **Bike:** material 16, chassis velocity, `Ride::position`.
- **Plane:** the hull's material, plus the body's velocity and position.
- **Prop:** its `shapes` mesh material, plus its bone body's velocity and position.

`audio::Sound::hits` runs `play` with the track's table (`Reading::hits(track_res)`, loaded beside
`drive_on`) and plays the results with `HitTable::sound`.

## Checked against the original

New test `prop_knock.rs::the_strike_is_queued_as_the_original_queues_it`:

- knock-2882's image is ticked by the original's props node (0x0026f800) and the table's queue is
  read (+0x80 count, +0x88 owners' probes, +0xa0 struck). It holds one entry: the bike's record
  striking material 21, ball 2.
- The port's same tick (the props' search, `prop_contact::strike`, the bump) hears exactly
  `[(Bike(0), Prop{2,0})]`. The props' own searches hear nothing, as in the original.
- `material_hit::queue` gives the Hit with owner 16 and struck 21, not foliage. With 21's 5/25 it
  plays at level 75 (bike speed 18.82).

Exploring the three knock images with the same read:

- 2881 queues bike → ball 2 (21) and ball 2 → bike (16, the bike's frame collidable, category
  0x9a048);
- 2882 queues only bike → ball 2;
- 2883 queues nothing.

## Not verified / not 1:1

- **No PCSX2 Video Capture matched filter.** The S195 `.p2s` states are gone: no slot or kept-context
  has `context/s195/*.p2s`, only the RAM images. Our playback of the same ride would also need seeding
  from that image. Filed as F27a1.
- **A prop's probe on a ridden bike is never heard.** The port's prop search has no ridden bikes in
  it (props meet bikes through the strike's mirrors), so 2881's ball 2 → bike entry is missing.
  - The pair drop would keep only the faster of the two anyway. When the ball is faster, the
    original plays at the ball's speed and place, and the port at the bike's.
  - Filed as F27a2.
- The bike's hits on riders, planes and props come from its contact list, not from a listener inside
  the pair test. A collidable struck on some ray but beaten on every ray by a nearer one would be
  heard by the original but not here.
- **Queue order across riders:** [rider 0's bikes/props, then scenery], [rider 1's …], then planes,
  then props. The original's order is the node's filed list. Order only matters for the pair drop
  and the carried material. The bike pairs keep their relative order.
- Mesh ids (`Struck::Mesh`) are local to the search that struck them, so two probes striking the
  same scenery mesh are not recognised as one collidable. A mesh never owns a probe, so the pair
  drop never needs them.
- The scenery search's foliage and material cases (fence, tree) have no capture compared. Only the
  knock's prop hit is checked against the original's queue. The `sound_of` mapping follows
  P145's `rear_material` and the categories in oracle F27.
- The parked plane's probe (in the original's listener set, knock-2881) is only heard when the plane
  is flown. The port does not search a parked plane.
- A struck wreck bike's material is its mesh's surface byte, not checked against a state.
- The level-to-gain mapping and the distance roll-off are F27's (linear level/100), not measured.
