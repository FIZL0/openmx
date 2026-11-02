# S229a — the tire contact sort

2026-11-02. Task: `./kit plan S229a`. Follows S229 (research/journal/2026-11-02-s229-car-collision).

## What the original does

Each frame the car's preforce (`car_preforce`) clears the four tires' contact words
(`car_tire_contacts_clear`: +0xb4 solid, +0xb8 sorted, +0x1ac, +0x1b0; **+0xbc is kept**), runs the
contact sort (`car_contact_sort`), then the four ground probes (`car_tire_probe`), then sets the front
tires' +0xc4 (fronts touching, no rear) from `solid || sorted`.

The sort is the car's second ray probe (car +0x558). `car_sort_rays_build` makes it at the end of
`car_phy_load`: one vertical ray per tire from mount + MaxCompression down to mount − radius, and a
three-node tree over them. The sort moves the probe to the machine matrix (car +0x5c0) and searches it
against the collision object's nearby list (car +0x16c, S229). Each hit goes to a tire by the ray's
quadrant: from.z > 0 is a front tire, from.x > 0 the right one. `car_tire_set_contact` stores the point
(+0x110) and normal (+0x120), the object and mover, and sets +0xb8.

`car_tire_probe` has two extra branches after its query loop:
- A sorted tire whose normal faces the car's up at under 0.707 loses its contact.
- If the terrain is not below the ray's lower end, a sorted tire sets a latch (+0xbc) when its axle
  faces the sorted normal at under 0.707. When the latch is clear, it also sets the latch if the
  terrain is more than 0.1 above the sorted point. When the latch is set, it clears it once the sorted
  point is no longer more than 0.1 above the terrain. A latched tire takes the terrain (solid) and drops
  the sorted contact. An unsorted tire takes the terrain as before.

`car_tire_suspension` runs its contact path on `solid || sorted`. Its bottom-out push still reads only
solid.

Addresses, offsets, and the ray tree layout are in research/oracle.txt (S229a block).

## The capture

No S149b row has a sorted tire. The only prop near S149a's free07 start line is a garage: a slab
0.7 ft above the ground, walled on three sides. `make_tape.py garage` writes a tape off S149a's baja
tape (state at frame_counter 400). The baja reverses with the stick full left, then straight back into
the garage's opening, brakes with all four tires on the slab, waits, drives out, and brakes. On this car,
reverse is Cross + Square together; Square alone only brakes. `capture.sh` plays the tape on the
original in copy 2 with `sort.toml` (S149b's blocks plus the sort probe and its hit list). It writes
`context/capture/s229a/baja-garage.csv` (fc 401–655). The RAM image saved at the end is
`context/states/s229a-garage-end.ee.bin`; the tape is `context/s229a/garage.p2m2`.

`path.py` prints the drive. The rear tires reach the slab at fc ~501 and all four by ~521. The baja
stops and then drives off at ~590. Every hit's object word (+0x1ac) is 0: the garage is a still
scenery mesh with no listener object. `hits.py` prints a row's hit list and each tire's words.

`capture.sh` adds `rand_state` to its private kit.toml copy, because this slot's kit.toml lacks it.

## The port

- `game_sim::car::sort_rays(mounts, tires)`: the rays in the image's order (FR, FL, RR, RL) and the
  tree's three nodes, built with `ride::ray_node` (now `pub(crate)`).
- `sorted_tire(from)`: the quadrant.
- `Machine::sort_contacts(car, collision, world)`: the probe placed at the machine matrix, searched
  through `flight_search::Scene::flown` against the collision object's nearby list. Each hit goes to
  `Wheel::set_contact`.
- `Wheel::{sorted, terrain_held}` and the two probe branches above. `Wheel::probe` now takes the car's
  up.
- `Machine::preforce_with`: the preforce with the sort between the clear and the probes.
  `Stepping::preforce` uses it whenever the machine steps with its collision object (`step_struck`).
  A plain `preforce` runs no sort.
- The suspension's contact path runs on `solid || sorted`.

## Checked

- `./kit check -- -p game-sim --test car_drive`: 10 passed. With `--nocapture`: baja-garage has 246
  rows (8 skipped where the engine clock shows a dropped frame) and 355 tire-rows sorted, bit-exact
  row by row and carried. The s149b drives (baja 641, buggy 648, truck 611 with 31 contacts) stay exact.
- With the `set_contact` call turned off, the garage replay goes wrong on 161–162 rows per field, so
  the test does depend on the sort.
- `the_sort_rays_are_the_bajas`: the built rays and the three tree nodes match the image's probe word
  for word.
- car_step, car_tire, car_spawn, car_engine, car_phy, physics_cars, physics_car_struck, ragdoll_cars,
  kiowa_capture, kiowa: all pass. openmx `--test car_switch` (1) and `--test bays` (36), `drive_on`
  (6), and `garage` (12) also pass.
- `./kit audit`: only the two leaks already in plan/DONE.md.

## Not verified / not 1:1

- The struck object's listeners, which the sort calls for each hit (hit +0x80 → +0x1f8 → +0xf4 list,
  vt +0xc), are not ported. The garage has none.
- The object and mover words a sorted contact stores (+0x1ac, +0x1b0) are not kept. The port treats a
  sorted contact as resting on still ground (zero contact velocity). A moving or struck prop under a
  tire is unchecked.
- Only the baja's sort rays are checked against an image. The truck's and buggy's are built the same
  way from their own tires but not compared.
- The tree is built only for exactly four rays (fixed nodes), not by the original's general split.
- The sort probe's category and flags (0, 0x92) are taken to filter like the body ray probe's search
  through `Scene::flown`. Trees (plants) in the sort are searched but no capture has a tire on one.
- The capture has a tire on a slab only. Ramps, where the axle-facing and up-facing 0.707 tests would
  fire, are not captured. Those branches are ported from the disassembly and checked only by reading it.
- 8 garage rows are skipped (dropped frames on the original), as in the s149b drives.
