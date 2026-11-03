# 1 — car tire tracks (FINAL)

Task S269. 2026-11-03.

## What I did

- **`crates/openmx/src/tire_tracks.rs`:** `Bike::car(machine)` reads a driven car the way its tracks do.
  Its four tires are in `wheels` order (FL, FR, RL, RR = `AttachedToTire` 0..3), each with:
  - the contact (`TireState::contact`, tire +0x140) and the ground's normal (`ground_normal`, +0x120);
  - solid ground as the ground contact and a sorted contact as the object contact;
  - the compression, `slipping` as the spin, and the surface's low byte as the zone.

  It also reads the ground speed (+0x55c) and `Machine::wrecked`. These are the record slots from S248's
  loader wiring (`research/oracle.txt` S248). Nothing in `Tracks` changed: it already picks a tire by
  `AttachedToTire`, so four tires work as they are.
- **`crates/openmx/src/main.rs`:**
  - `load_car_tire_tracks` reads the `TireTrack` entries from each car's list (`particles::car_list`),
    with the zones file. It prints `car: <dir>: tire tracks: 4 on tires 0, 1, 2, 3`.
  - Each seat has one `Tracks` per car (`car_marks`). `step_car_tire_marks` steps the set for the car
    the seat drives, under the same conditions as the bike's (riding, not paused or waiting, not
    `--overview`). It runs right after the bike's. The noise seeds start at 0x100, so no car's seed is
    a bike's.
  - The marks are added to the bike's tracks' glow, with the same `ttrack` tread: every list on the disc
    names `Material = ttrack`. A car's marks stay when the seat changes machine, as the bike's do.
  - `load_car_fx` no longer lists the four as "not run". It prints them as laid by the tire tracks, as
    the bike's line does. `tests/car_particles.rs`'s shell test now checks that wording.
  - At the end of a ride in a car, the shell prints `car: tire tracks: <entry> <triangles>, …`.

Taken from opentennis: nothing. It has no vehicles. The pattern is this repo's bike tracks (V40) and
S248's car particles.

## What the disc says

All three lists (`buggyparticles.dat`, `monsterparticles.dat`, `bajaparticles.dat`) have the same
settings: `ZoneDescriptor TireTrack` (surface 0 off on free09), `Disable NoContact | ObjectContact |
Wrecking`, `ttrack`, 32 strips, `HeightAboveGround` 0.3, `BreakAngle` 10, `MinimumSpeedThreshold` 3, and
`FadeInDistance` 5 / `FadeOutDistance` 20. They differ in these:

| car | `Width` (front, rear) | `MinimumCompressionThreshold`, `Maximum…` | `BasePercentAlpha` |
|---|---|---|---|
| buggy | 0.25, 0.5 | 0.75, 1.0 | 0.3 |
| monster truck | 2.0, 2.0 | 1.5, 2.5 | 0.5 |
| baja | 0.5, 0.5 | 0.75, 1.0 | 0.3 |

The car loaders (`./kit fn 0x0039e230`, the truck's 0x003dfd90) read the list through the same reader
as the bike, `vehicle_fx_read` (0x003d52b0).

## What a car does with them

- **On the flat, under steady throttle,** no tire is compressed past its threshold. The buggy's
  fronts sit at about 0.5, its rears at 0.32, and the truck's rears at about 1.17 against 1.5. So on
  the flat a car marks only while a tire is slipping, which in practice means the driven rears during a
  launch.
- **When the load rises,** for example at the foot of a slope or over bumps, all four tires mark.
- **After a landing,** a tire that comes down already past its threshold lays nothing until it has had
  one tick on the ground without marking. This is the original's arithmetic, and the bike does the same
  (oracle V40). While the strip is ended, `last = tire`, so `d` is 0 and the fade-in left after the
  disable never runs down. Because of this, the shell test's monster truck, which is switched in above
  the ground and lands on its rears, lays only its fronts on free09's flat start. The buggy and the baja
  lay all four there.

## How it was proved

`cargo test --release -p openmx --test car_tire_tracks`: 12 passed. Eight of these are `vehicle.rs`'s own
tests, mounted with the file. The four new ones:

- **`each_car_lists_a_track_a_tire`:** on free09, each list has four tracks on tires 0..3, all `ttrack`
  with 32 strips, the `Disable` flags above, and only surface 0 off.
- **`a_cars_tires_read_as_the_loader_wires_them`:** a real buggy, with some fields changed. Each tire's
  point, normal and compression are its contact, ground normal and compression. The flags come out as
  2 (ground), 1 (no contact), 6 (ground and object) and 2. Slipping reads as spinning, surface 0x105
  reads as zone 5, and the wreck and ground speed carry over.
- **`a_car_on_the_gas_leaves_four_tracks`:** each car off the disc, driven through `CarRide::step` on a
  surface-1 floor that rises at 0.35 onto a top 8 ft up. Nothing is laid at rest. On the gas, the
  launch lays the driven tires. From the slope's foot to the end, all four lay more triangles. Every
  row is laid exactly at its own tire's contact (0.000 ft). On a surface-0 floor nothing is laid.

  | car | at the slope's foot (FL, FR, RL, RR) | at the end |
  |---|---|---|
  | buggy | 0, 0, 24, 24 | 14, 14, 38, 38 |
  | monster truck | 0, 0, 14, 22 | 16, 16, 34, 42 |
  | baja | 0, 0, 34, 38 | 18, 18, 64, 68 |
- **`the_shell_lays_a_driven_cars_tracks`:** the binary runs headless on free09 with a full-throttle tape
  that switches into the buggy at tick 30. All three cars load their four. At the end the buggy's four
  have laid 142, 160, 48 and 56 triangles. With the same tape, the baja laid 66/62/64/56 and the truck
  18/18/0/0, for the landing reason above.

Also green: `--test car_particles` (13), `--test tire_tracks` (8) and `--test car_switch` (1).
`cargo test --release -p openmx --no-run` compiles every target.

I took one GPU shot looking down behind the buggy on free09, at frame 190 of that tape (not committed).
It shows two dark tread lines, about the buggy's track width apart, where each axle's two marks overlap.

## Not verified / not 1:1

- **No comparison with the original.** No capture of a car's tracks in the original was taken. Places,
  alphas and timing are the bike's ported manager on S248's wiring, not checked against PINE.
- **Split screen.** The reader skips `TireTrack` entries when the event has more than one human player
  (oracle V40). That applies to the cars too, since they use the same reader. The shell's bike tracks
  do not follow that rule (every seat lays its own), so the cars lay them in a split as well, for
  consistency. This was not changed here.
- **The wreck word.** The wreck comes from `Machine::wrecked`, as in S248. The loader's +0x754 is not
  identified (C18).
- **Contact point and normal.** I took tire +0x140 to be `TireState::contact` and +0x120 to be
  `Wheel::ground_normal`, from S149b's tire layout (oracle). A sorted contact's normal is the mesh's.
  Nothing compares them with RAM.
- **The truck in the shell test.** The landing reason above is a reading of the shared arithmetic,
  checked on the port's numbers only.

## Next

Nothing for S269.
