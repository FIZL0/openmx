# 1 — car particles (FINAL)

Task S248. 2026-11-02.

## What I did

- **`crates/openmx/src/particles.rs`:**
  - `car_list(dir)` gives a car's list by its `machine.res` directory.
  - `PlaneFrame::car(machine, nodes, struck)` builds a frame from the car's words. It reads the
    throttle (rate-limited), the engaged clutch, the ground speed, the wreck and the body's velocity.
    It also reads the four tires and, while the collision object is out, its contacts (`touch`, as
    the plane's).
  - `TireFrame::car(wheel)` maps one tire: solid to ground contact, sorted to object contact, plus the
    compression, the surface byte as the zone, slipping as spinning, and the side ratio as the side
    force.
  - Nothing in the manager itself changed: the bike and plane kinds already cover the cars' lists.
- **`crates/openmx/src/main.rs`:**
  - `load_car_fx` loads one list per car the garage can switch to, binding the nodes on the car's
    first-livery model. It prints `car: <dir>: particles: N entries off <list>; 4 not run (…Tire Track)`.
  - Each seat gets `car_fx`, one manager per car. Each tick, the manager for the car the seat drives
    gets `PlaneFrame::car`, with the nodes placed by `car_draw::placed`. The other managers get an
    empty frame, so what they threw runs out, as the Kiowa's does.
  - The particles are drawn by both the GPU sprites and the software path, next to the Kiowa's.
  - When the ride ends in a car, the shell prints `car: particles: …` after the car's report.
- **`research/oracle.txt`:** S248 comment lines on the car loader's tire records and car words.
- **`crates/openmx/tests/car_particles.rs`:** five tests, listed under "How it was proved".

Taken from opentennis: nothing. It has no vehicles. The pattern is this repo's own Kiowa wiring
(`load_heli_fx`, `heli_fx_frame`).

## What the original does

The car setup (`./kit fn 0x0020a640`) calls the car's particle loader: `./kit fn 0x0039e230` for the
buggy and baja, and `./kit fn 0x003dfd90` for the monster truck. Each reads its `.dat` into the same
manager the bike uses, then points the manager's four tire records at the car's four tire objects:

| record | tire field | meaning |
|---|---|---|
| +8 | +0xb4 | solid ground (ground contact) |
| +0xc | +0xb8 | sorted contact (object contact) |
| +0x10 | +0xdc | compression |
| +0x14 | +0x1e4 | slipping (the spray's "spinning") |
| +0x1c | +0x1ec | locked |
| +0x20 | +0x2b0 | side ratio (the spray's side force) |
| +0x24 | +0x100 | surface (the zone byte) |
| +0 | +0x140 | contact point |
| +4 | +0x120 | contact normal |

It also reads these car words:

| word | meaning |
|---|---|
| +0x3ec | throttle |
| +0x3f0 | clutch |
| +0x55c | ground speed |
| +0x754 | wreck |

The bike's wiring (`./kit fn 0x0027adf0`) uses the same record slots, with spinning at +0x14 and the
side load at +0x20.

Every particle node (`FrontLeft_TireParticles`, `buggy_RtireL_particles`, `baja_exhaust_particles`, …)
is a child of the model's root. A node's place is therefore its bind matrix times the car's matrix.

Each list has:
- the collision model's dust and chunks;
- the exhaust(s), with `MaxSpeed 15`;
- four `Dust` entries, which emit only above `MinimumEmissionSpeed 15`;
- the dirt sprays (the monster truck has rears only);
- four tire chunks;
- the rears' collision chunks;
- four `TireTrack` entries.

The monster truck also has two `Generic` "Rear … Tire Dust Spray" entries. Its rears carry the most
compression, so they dust the hardest.

The `DirtSpray` and `Chunks` zone descriptors in `mxparticlezones.dat` say `none` to `Zone1`, which is
surface 0. A car on surface 0 therefore dusts but throws no dirt or chunks. I first tried the test on
`Flat`, which is surface 0, and nothing was thrown; the test now rides a floor of surface 1.

## How it was proved

`cargo test --release -p openmx --test car_particles`: 13 passed. Eight of these are `vehicle.rs`'s own
tests, mounted with the file. The five new ones:

- **`the_lists_are_named_by_the_cars_directory`:** the three lists, and none for the biplane.
- **`a_cars_tire_reads_as_the_loader_wires_it`:** a real buggy's tire, mutated, maps field by field,
  and its `Disable` flags come out as the loader's records give them.
- **`a_cars_frame_carries_its_four_tires_and_its_pedals`:** the frame carries the four tires, the
  throttle, the clutch, the speed, the wreck and the velocity.
- **`a_car_on_the_gas_dusts`:** each car off the disc, driven through `CarRide::step` on a flat surface-1
  floor, with free09's sheet and its own list:

  | car | top speed | dust in the air (most) | dirt in the air (most) |
  |---|---|---|---|
  | buggy | 73 ft/s | 75 | 619 |
  | monster truck | 100 ft/s | 259 | 746 |
  | baja | 103 ft/s | 221 | 764 |

  At rest, in the first 30 ticks, none of the cars made any dust or dirt. Every particle stays within
  200 ft of the car, and a debug run put each node on its tire, for example the buggy's rear-left at
  (−3.0, 0.4, −5.7) from the car.
- **`the_shell_dusts_a_driven_car`:** the binary runs headless on free09 with a full-throttle tape that
  switches into the monster truck at tick 30. All three lists load with only their four tire tracks
  left out. At the end, all four of the truck's tire dusts have particles in the air: 31, 33, 95 and 85
  at frame 200.

Also green: `--test car_switch --test kiowa_particles --test bike_particles --test car_engine_sound
--test car_picture`, and `cargo test --release -p openmx --no-run` compiles every target.

`./kit audit` found only the two leaks already in `plan/DONE.md`.

I took one software shot, `--shot-pose` beside the truck at frame 62 on free09 (not committed). It shows
the dust as a haze around the rear tire and under the chassis.

## Not verified / not 1:1

- **No comparison with the original.** No capture of a car's particles in the original was taken.
  Counts, places and timing are this port's manager on the loader's wiring, not checked against PINE.
  Filed as C18.
- **The wreck word.** The loader's wreck word +0x754 is not identified. The port uses
  `Machine::wrecked` (+0x2c4), so `Wrecking` and `NotWrecking` may switch on a different tick from the
  original's. Filed as C18.
- **Tire tracks.** The tire tracks are not run, so a car leaves no tracks. Filed as S268.
- **Liveries.** The nodes are bound on each car's first-livery model. I assumed every livery has the
  same particle nodes and did not check it.
- **Tire order.** I took the tire record order (FL, FR, RL, RR) to be the car's `wheels` order, from the
  loader's `param_1[0x152..0x155]` and the lists' own pairing: `FtireL` is `AttachedToTire 0`, and so
  on. Nothing compares the port's wheel index with the original's tire object.

## Next

Nothing for S248. C18 and S268 hold the rest.
