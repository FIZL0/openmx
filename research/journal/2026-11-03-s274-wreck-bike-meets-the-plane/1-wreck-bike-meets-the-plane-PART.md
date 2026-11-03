# 1 — wreck bike meets the plane (PART)

Task S274. 2026-11-03.

## What I did

- **Found the wreck bike's part in the image.** The rider record's `+0x7a8` is the vehicle's wreck part.
  Its `+0x110` is the wreck bike's body and its `+0x120` the wreck bike's collision object.
  - At 137910, P2's rider is 0xdee010, its part 0xdc2c50, the body 0xdc2e40 and the object 0xdc4550.
  - The object layout is as in `tests/wreck_bike.rs`'s `rec`. The body layout is the plane's
    (`seeded_plane`).
  - The masses match the port's loader: the wreck collider is 3.1055897e-6 (inv 322000.03) and the
    plane collider 310.559.
- **Read the contact lists at 137880 and 137910.**
  - The wreck bike's list: two terrain entries, then one kind-0 entry naming the plane's collision
    object.
  - The plane's list: exactly one kind-1 mirror naming the wreck bike's object.
  - At 137910 the mirror's point is (4567.279, 139.29706, 3515.2068) and its normal (0.3042866,
    0.95253515, 0.009285104).
  - The plane's own rays never strike the wreck bike. Only the wreck bike's rays meet the hull.
- **Ported it.**
  - `WreckBike::search_flown` puts the flown planes' hulls in the wreck bike's search (as
    `Scene::among_planes` puts a plane in another plane's) and hands back the mirrors.
  - `WreckBike::step_flown` steps the wreck bike with the planes' bodies as the other sides.
  - The plane's frame takes the mirrors in `Met::mirrors`, naming `flight::WRECK_FROM + j`, and the wreck
    bike's body in `Met::wrecks`, and writes that body back after its resolve.
  - With no wrecks, nothing changes.
- **Seeded a wrecked P2 off 137910** (`wrecked_p2`): `Crash::begin` with the capture's reason, then the
  body and the collision object from the image. The added JOUSTS run, 137910..138010, holds P2 100/100.
- **Ordered fly_two's tick:** the wreck bike's search, the plane's frame with the mirror, the wreck
  bike's step, then P2's seat. The seat keeps that wreck tick (its own `Crash::step` does not tick the
  wreck bike a second time).

## What the original does

- The wreck bike's probe (an empty category list) strikes the hull mesh (`biplanef.col`). The hit goes
  on the wreck bike's list after its terrain entries, and its mirror (normal reversed) goes on the
  plane's list after the plane's own entries.
- The wreck bike's collider is so light (3.1e-6) that its kind-0 entry barely pushes the plane:
  skipping that push (`plane.pushed`) leaves every number the same. The plane's change comes from the
  mirror's spring and friction against the wreck bike's body on the plane's resolve.

## How it was proved

`cargo test --release -p game-sim --test c16_fly`: the plane's body and P2's chassis against
`context/capture/c16/fly.csv` row by row, from the s272 images. Orders I tried, plane velocity at the
first tick after the wreck (row 137858 reads -28.616682, -0.0535129, 3.6458206):

| order | 137857 plane vel |
|---|---|
| no wreck bike meeting (HEAD) | vy +0.0302 |
| wreck's kind-0 push only (before or after the plane) | unchanged from HEAD |
| mirror: search, plane, wreck step (kept) | -28.609848, -0.08526486, 3.6421175 |
| mirror, wreck stepped first | -28.63398, 0.0204, 3.6552 |

From 137910 (the seed equals row 137910 exactly), the first tick lands at:

| run | first-tick plane vel |
|---|---|
| no mirror | -25.09143, 8.587877, 2.2600884 |
| kept order | -24.854322, 8.510814, 2.1630855 |
| row 137911 | -24.94349, 8.60006, 2.21980 |

The original lies between the two runs, but not as a scalar blend (y moves the other way), so the
mirror's resolve differs in some way, not only its strength.

I also compared the port's mirror at 137910 with the image's. The port's is one tick's travel away
(the image's was filed by 137909's search), so the geometry is consistent.

## Not verified / not 1:1

- **The plane's resolve against the wreck mirror is not bit-exact.** No JOUSTS plane count rose:
  - 137810: 46 plane, 17 P2.
  - 137845: 11 plane, 55 P2.
  - 137910: 0 plane, 100 P2. P2 is a wreck that holds only because its chassis is frozen.

  Not checked:
  - whether the spring's share for a kind-1 entry against a wreck body takes the body's mass or the
    collider's;
  - the friction's other velocity;
  - whether the plane's list holds the mirror at its own entries' index or later.
- **STRETCHES is unchanged** (137910's stretch 0/100). `fly()`'s Sim plane lane seeds P2 standing and
  has no wreck bike.
- **The product does not run any of this yet.** `Sim` and the shell's plane lane never pass a wreck bike
  to the plane's frame. Only `tests/c16_fly.rs` wires it.
- **137810's P2 pin (17) is not lifted.** I did not seed P2's rider fully (S228 `seeded_rider` is used,
  as before).
- The body's `+0x110`/`+0x114` words read 0 through `Ee::f32_at` (denormals flushed). Their meaning is
  unchecked.

## Next

Read the original's resolve for a kind-1 entry whose other object is a wreck bike: which mass and which
velocity it takes for the other side. Do it with `./kit dis` on the contact resolve the plane's list
runs, comparing the plane's velocity after 137910's first tick (row 137911) one term at a time. Then pin
JOUSTS and wire `wreck_plane` into the Sim plane lane, so that STRETCHES' 137910 stretch can seed a wreck.
