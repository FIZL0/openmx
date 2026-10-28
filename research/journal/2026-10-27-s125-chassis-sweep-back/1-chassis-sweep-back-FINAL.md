# 1 — chassis sweep back (FINAL)

Task S125. 2026-10-27.

## What I did

- The body rays were already searched against props by S138 (`rider_contact::scenery` → Bump →
  `contact::resolve_ridden`). What the ride still lacked was the chassis' sweep back, and
  `velocity_bundle` stood in for it as the port's last body probe. `velocity_bundle` is gone now.
- `contact_search::sweep_back_chassis` adds the sweep back's chassis branch, written against the
  existing `Record` / `Search` the plane's sweep uses.
- `ride::sweep_record` builds the bike's record from its `.phy` `[Collisions]` keys (offset, lead,
  push out, facing). `Ride::sweep_back` searches the bike's nearby props and trees through
  `flight_search::Scene::wooded`, using the body rays when the rider carries a `BikeCol` and the
  wheel rays otherwise. A world with no props falls back to `SolidSweep`, a one-probe solid
  sweep. A hit moves `Ride::origin`, which is the chassis node.
- `rider.rs` runs the sweep back before the ground search. A bare `Ride::frame` (no caller
  search) runs it at the top of `frame_searched`. `contacts` no longer probes along the velocity:
  scenery hits just mark `touching`, and `near` comes from the wheel probe alone.
- `stock::BIKE` gets `[Collisions] HighSpeedRayOffset = 0, 3, 0` and `HighSpeedMinCollisionAngle
  = 45`, which are race125p's values. With the file defaults (origin, cos 1°), the sweep started
  under the train wagon's deck. It pushed a parked bike up 2 ft a tick and slid it 103 ft along
  the deck (traffic.rs `a_bike_parked_on_the_train_rides_it`).
- Tests:
  - rider_contact.rs `the_sweep_back_is_the_originals` is new.
  - ride.rs `a_prop_stops_the_rider` now holds the sweep point behind the wall.
  - eco.rs's free09 tree ride now carries the disc's bike col: the bike stops at the trunk, at
    z 117.19, and goes over the bars (Wreck(1)).

## What the original does

`./kit fn 0x00270390` (collision record sweep back), chassis branch (record +0xf4 == 0, +0xf0 =
chassis):

1. r = the node's (+0xf8) rotation of the offset (+0xb0).
2. to = node position + r; from = to − chassis velocity (+0x260) · lead (+0xc0) · dt.
3. The sweep probe (+0x1f4) is searched nearby (+0x10c). If the hit is on another object or a
   mover, both speeds are > 1, and the dot of their unit directions is > facing (+0xc8), it
   returns and moves nothing.
4. Otherwise it sets the node's position to hit point + push · push out (+0xc4) − r. When the
   record has rays (+0x1f0) it calls "moved" (`./kit fn 0x00119be0`), and +0xe0 = that point.
   Only the node moves; the velocity is left as it is.

## How it was proved

- `rider_contact.rs::the_sweep_back_is_the_originals`: six riders across the two split-screen RAM
  images. Each bike's record words match `ride::sweep_record` of race125p's `.phy`:
  - +0x3c, +0x1f4 and +0x10c are set, and +0xf4 == 0.
  - +0xf0 is the chassis, and +0xf8 is the chassis node.
  - The +0xb0 quad, +0xc0, +0xc4 and +0xc8 match the `.phy` values.
- The behaviour is proved only by the ported tests above (prop wall, free09 tree, the train deck).

## Not verified / not 1:1

- No capture compares a sweep's hit/push against the original. Only the record's words are
  proved, from the RAM images, because there is no PINE capture of a bike tunnelling into a prop.
- The swept segment (+0xd0/+0xe0) is not compared, and the swept point's `w` lanes are not
  matched. Only xyz is written back.
- The sweep back runs after the rider-vs-rider gather and before the ground search, not at its
  place in the object list's order. The port has no object list.
- The sweep scene holds props and trees, but no other bikes. The original's nearby search would
  also see other riders' collision objects.
- In a world with no props (`SolidSweep`), the sweep is the port's one-probe solid bundle, not
  the original's search: those worlds have no search to run.
- Without a `BikeCol`, the search is gathered around the wheel rays, not the body's eleven.
- `near` now comes from the wheel probe only (the velocity probe used to feed it).
- The bare `Ride::frame` path (no caller search) sweeps at the top of the frame itself.
- eco.rs `a_free09_tree_against_the_original` skips in s3: its context/capture/s100 csvs are not
  in this slot.
- The stock bike's two `[Collisions]` values come from race125p and were not checked against any
  other disc bike.

## Next

none
