# 1 — across meets the wagon (FINAL)

Task S190. 2026-11-01.

## What I did

- **Found which probe meets the wagon.** I put a debug print in the port's wheel probe (`ride.rs`
  `contacts`, since removed) and ran across's ticks 140–150. Every wheel hit is on the rail bed, a still
  prop (no car). The normals are the bed's: (0.008, 0.999, −0.050), (0.037, 0.973, −0.226), and the
  top (−0.0004, 1.0, 0.002). The car poses at tick 148 put both wheels outside car 5's side, which is at
  local x 4.46: the front at x 4.86, the rear at 7.3. By tick 152 the front is at x 0.25, under the
  deck. The wheel probe is a vertical segment that tops out about 2.6 ft above the car's base, so it
  passes under the deck (whose underside is at about 3.1 ft, S137). It never meets the wagon.
- **Gave the replay the bike's body rays.** `tests/replay.rs`'s new `free06_bike_col` loads the disc's
  `bikem.col` / `bikef.col` from free06's archives. `the_bike_meets_the_dallas_train_as_the_original_does`
  puts the result on the rider for both rides. The original's riding bike always carries them. With
  them, across agrees on every lane of every tick. A debug run of `Sim::car_contacts` after each tick
  found 2–5 body-ray entries on car 5 (prop 358) from tick 148 to 155. Each has normal
  ≈ (0.10…0.14, 0, −0.99), the car's side facing −z, and the car as mover. There are none before or
  after. That is the push.
- **The test now:**
  - `S129_AGREES_THROUGH` is `[1, u64::MAX]`, so across has to agree on every tick.
  - Across asserts that no lane parts.
  - A control: the same fixture with `bike_col = None` must part at exactly tick 148
    (`S129_ACROSS_BARE_PARTS`), which is the old failure.
  - It asserts the original's story: no wreck on any row, vel.z above 40 at row 145 and below −15
    within rows 150–169. It also asserts that the port's bike does not wreck.
  - The "raise the ratchet" note no longer prints once a ride is at `u64::MAX`.
- No simulation code changed. The contact was already ported, by S137 (body rays against the cars),
  S138 (the scenery search), S142/S145 (the mover response) and S151 (the car named as the mover).

## What the original does

Across is bit-exact through the wagon. So, in the original, the bike's body rays strike the wagon's side
and are resolved against the car as a mover, and that pushes the bike back out to −z (vel.z +44 → −20
over rows 146–165). It then rides off along the train with no wreck. The wheels' swept contacts in
rows 140–161 (f_swept, r_swept) are on the rail bed, not the wagon. S129 and the brief read them as
the wagon's. The port has a swept contact on exactly the same ticks, every one of them on the bed, and
the chassis matches bit for bit through them.

## How it was proved

- **The bit-exact replay fixture** `tests/replay.rs`
  `the_bike_meets_the_dallas_train_as_the_original_does`, from the s129-across save state, against
  `context/capture/s129/across.csv`. Every watched lane agrees on all 300 ticks: chassis, wheels, slip,
  engine, histories, `ray_hit` / `swept_contact` and the train clock.
- **The control** without body rays parts at 148, as before.
- Commands:
  - `cargo test --release -p game-sim --test replay the_bike_meets_the_dallas_train`: passes.
  - `cargo test --release -p game-sim --test replay train`: 3 passed (S129 both rides, the S129
    gather, S146's roof), all in s2 with the captures present.

## Not verified / not 1:1

- **The original's wheel contacts are on the bed only by inference.** The capture holds the swept flag,
  not the contact's normal or mesh. That they are on the bed follows from the port's bed contacts
  agreeing bit for bit on every chassis and wheel lane, not from reading the original's record.
- **The body-ray entries were not compared one by one** with the original's collision record: their
  count, order and points. The record is not in the capture. Only their effect is compared, bit-exact.
- **Braked still parts at tick 2**, with or without the body rays. This is S129's creep, not this task's.
- **The other replays that use `in_the_world_with_cars` still ride without body rays**: the S129 gather,
  S146's roof and S162's `idle_roll_offs_on_free06_wreck_as_the_original_does`. They pass as they are and were left alone. The roof bike rides
  above the cars' sides, but whether its rays would change anything was not run.
- **The captures and the state are only in s2's context/.** Elsewhere the test skips and reads as passing.
- **The shell was not ridden for this.** The shell already loads bike_col (`openmx` main.rs) and steps
  `Sim::step`, which is the path the replay takes, so it gets the same contact. No shell run checks it.

## Next

Nothing for S190.
