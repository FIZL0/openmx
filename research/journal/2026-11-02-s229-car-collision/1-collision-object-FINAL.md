# 1 — collision object (FINAL)

Task S229. 2026-11-02.

## What I did

- `game_sim::car`: the car's collision object (`Collision`, the helicopter's class reused), built by
  `collision` off `[Mass Properties]`, the body's inertia and the car's ray file (`Kind::rays`:
  `MonsterTruck\col\monstertruckm.col`, `Baja\col\bajam.col`, `Buggy\col\buggym.col`), with the cars'
  collidable category `COLLIDABLE` (0x52058) and the terrain search off as the loader leaves it.
  `collision_of_model` is the same with the inertia worked as `Machine::new` works it, for the shell.
- `Machine::step_struck`: the object's search before the world steps (its touching flag is the car's for
  the frame); every substep, gravity then `contact::resolve` with impulses on the record's contacts, ahead
  of the car's own forces (`force` now goes through a private `force_with` that takes the resolve); in the
  post, the nearby list gathered again before the measure; after the step, the measure's terrain switch
  (`Machine::searches_terrain`). `Machine::step` is unchanged in what it does (no collision object).
- `tests/car_drive.rs`: the replay runs on free07 as the shell builds it (terrain, the scene's meshes out of
  `free07.res`/`free07_2.res`, the ecosystem's plants) with `step_struck` on every row. `STRUCK` is gone:
  fc 720–727 are stepped like the rest. `check_collision` compares the port's object with the switched
  image's (the response's tuning, masks, collidable, capacity, the record's switches and sweep numbers,
  every ray). The report counts rows touching and contacts.
- The shell's `CarRide` reads the car's `.col`, builds the object when the car is taken, and steps the car
  with `step_struck`.
- Filed S229a: the tire contact sort (below).

## What the original does

The car's collision object is the rigid-body part's (`./kit fn 0x00397f40` for the force listener).
Every record's search (vt +0x7c) runs before the world steps. The force listener first runs the body
wrapper's force (`./kit fn 0x0015af40`): gravity, then the object's resolve with impulses (vt +0x80 with
1), and only then the car's anti-slide, preload, launch, drivetrain, tires and aero. `car_measure`
(`./kit fn 0x00398150`) calls the wrapper's post early, which refills the nearby list. It ends with the
terrain switch (`./kit dis 0x00398738 0x003987b0`). That is a ground query at the body's centre of mass,
then `up · normal` on the FPU accumulator (`mula`/`madda`/`madd`) against 0.707. At or below that, record
+0x30 = 1 and the body rays search the terrain on the next frame. Above it the switch is 0, so an upright
car meets the ground only with its tires.

The record's other words on the three switched images: search 1, sweep back 1, lead 2, push-out 4,
facing −1, offset (0, 6, 0). The nearby search is the car's +0x16c (least 10, extra 30, capacity 128),
the plane's numbers. The oracle has the rest (S229 block).

The truck's strike: at fc 720 its front corner's body rays (on the body (−6.82, 3.61, 9.09) and
(−3.55, 3.61, 9.09), plus one at y 10.18) strike a free07 mesh, and the impulses take vel from
(1, 12, −129) to (−34.5, 10.6, −21.1). Rays keep touching through fc 722. By fc 723 the truck has
tipped past 45° from the terrain's normal (the switch went to 1 at the end of fc 722). Its body rays
then meet the terrain, and that contact lifts vel.y from 10 to 23.7. Without the switch the port
followed to fc 722 and parted at 723.

## How it was proved

- `./kit check -- -p game-sim --test car_drive`: 7 passed. Truck, baja and buggy were replayed against
  the s149b captures, row by row and carried, with every compared word bit-exact. The truck has 611 rows
  (38 skipped as paused or two-in-one, 1 torn) with 8 rows touching and 31 contacts, fc 720–727
  included. Baja has 641 rows and buggy 648, with no contacts. Run with `--nocapture` to see the
  counts, since a pass prints nothing in `./kit check`.
- `check_collision` passes on all three switched images (object words, the record's switches, lead,
  push-out, facing, offset, all 41 / 29 rays).
- These still pass: `car_step`, `car_tire`, `car_spawn`, `car_engine`, `car_phy`, `physics_cars`,
  `physics_car_struck`, `ragdoll_cars`, `kiowa_capture` and `kiowa` (50 tests); and
  `openmx --test car_switch --test bays` (37). The headless shell drives the monster truck 309 ft with
  the rays loaded.

A replay finding: `Body::pose` (the node the post leaves, which the search starts from) reads
`last_position`/`last_quat`, the body at the start of the frame's last substep. The rows don't hold it.
Row by row, the seed takes it from the port's own step into that row, whose words match the row.
After a skipped row it falls back to the row's own pose. Without this every row-by-row search ran at
the image's spawn.

## Not verified / not 1:1

- **The tire contact sort** (`./kit fn 0x00397120`, filed as S229a) is not ported. That is the car's
  second ray probe (four rays down through the tires) and the sort that hands its hits to the tires.
  No s149b row has a sorted tire contact, so nothing can prove it yet; it needs a capture of a car
  driving onto a mesh. Until then a car's tires meet only the terrain, never a prop's top.
- The ray probe's category 0x10008 (the plane's is 0x8) is not modelled. The port's probe filter is the
  helicopter's, and the replays didn't need the difference.
- The terrain switch's ground query is `World::ground` (contact_search::ground's triangle split, S64's
  match for the original's query). It was proved only at the truck's fc 722→723 flip, and off the
  terrain's edge it takes the port's no-ground normal.
- The replay's seed uses the port's own `last_position`/`last_quat` (see above). That isn't read from
  the game, but the carried run, which needs no seed, passes the same rows.
- The shell's car is not checked against the original: only that it still drives (car_switch). The
  shell's world for it is the live one (`Through`), with whatever props, plants and movers it has. The
  movers a strike names are resolved as in the Kiowa (`&mut []` for other bodies), so a car pushing
  another machine's body is not modelled.
- The oracle line for `car_contact_sort` above the S229 block still says "touching stays false". The
  S229 block corrects it: touching is the record's.

## Next

Nothing for S229. S229a waits on a capture.
