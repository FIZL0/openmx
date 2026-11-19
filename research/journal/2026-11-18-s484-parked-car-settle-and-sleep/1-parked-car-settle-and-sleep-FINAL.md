# 1 — parked car settle and sleep (FINAL)

Task S484. 2026-11-18.

## What I did

- `car::Machine::at_rest`: the props' at-rest test, with the car's six `[Mass Properties]` thresholds.
- `car::Machine::settle`: step with nobody in it until at rest, then disable the body. Returns the frame it slept on.
- `car::parked_spawn`: the spawn height. It is the ground under `machine.txt`'s spot plus the model's bounds centre y.
- `Sim::park`: fills `Sim::parked` from settled machines and their hulls (`ai::parked_hull`).
- **Fix in `Machine::new`:** the body now stands at `at` itself.
  - Before, it stood at `at` + turn(CofGAI), which is 1 ft too high on the truck.
  - The original's place (vt+0xe4, `./kit dis 0x0015a1b0 0x0015a210`) puts the body at the spawn to the bit.
  - The shell's taken car keeps the old lift (`car::cofg_ai_lift` in `vehicle.rs`), so it starts where it did.
- Tests in `crates/game-sim/tests/car_spawn.rs`:
  - `a_parked_truck_settles_and_sleeps_as_the_original`
  - `a_parked_truck_comes_to_rest_where_the_original_parks_it`
- A live capture of a bike ridden into the truck: `research/s484/truck.sh` + `ride.py`. The csv is in `context/capture/s484/ride1.csv`.

## What the original does

- The car's physics object wrapper is at car+0x50. Its vtable holds:
  - +0xe0 store (`./kit dis 0x0015a180 0x0015a1b0`).
  - +0xe4 place.
  - +0xdc put-back (`./kit fn 0x00396e90`): place at the stored spot, launch on, reset each tire (`./kit fn 0x0039ace0`) and the engine.
  - +0x124 enable (`./kit dis 0x00398c50 …`): sets car+0x4ac (driven) = 1 and fresh = 1. The car's first step clears +0x4ac again when nobody is in it.
- The sleep is the props' object listener (`./kit dis 0x0015b2a0`) with their at-rest test (`./kit dis 0x0015b400`).
  - At rest means velocity, rates, accel, angular accel, shock and twist are each under its threshold. The truck's are 0.3, 0.3, 1, 1, MAX, MAX.
  - It also needs "not fresh". The first force clears fresh, so after any car step it is clear.
  - ResetTime is -1, so the car sleeps for good, filed with detect-when-disabled.
- The load steps it at 0.03333 s a frame (the asleep body's step reads 0.03333/4).
- **Bikes vs the parked truck (live, copy 3):**
  - The bike was moved 50 ft south of the nose and rode in at about 55 ft/s with the gas held.
  - The bike stops dead at the nose.
  - The truck wakes on the strike frame, gets vz 2.77 ft/s along the bike's way, and slides 0.2 ft.
  - It sleeps again after 9 frames, wakes twice more for a frame while the bike leans on it, then stays asleep.
  - So yes: bikes strike a parked car's hull, and the strike wakes and pushes its body.

## How it was proved

- **Interpreter replay, bit for bit (`a_parked_truck_settles_and_sleeps_as_the_original`):**
  - Setup: recording2's truck, put back at its spawn by its own store, put-back and enable, then stepped by free09's node update (`collision_node_update`) at 0.03333.
  - Result: position, velocity, accel, rates, angular accel, quat, last position and last quat match bit for bit on all 57 frames (0–56). Both sleep on frame 56.
  - `Machine::settle` from the same start gives frame 56 and the same final position.
  - The spawn from `parked_spawn` matches +0x270 bit for bit.
- **Near the images (`a_parked_truck_comes_to_rest_where_the_original_parks_it`):**
  - The port's own truck (its own mounts and inertia) sleeps on frame 56.
  - It rests 0.046 ft from first_recording's pose and 0.104 ft from recording2's.
  - The two images are 0.074 ft apart from each other.
- **Bikes vs truck:** a live PINE poll, one row per game frame, 240 rows (`context/capture/s484/ride1.csv`).

## Not verified / not 1:1

- **Leftover tire words zeroed.** The bit-for-bit replay zeroes the tire words the put-back leaves over from the image's own settle: +0xe0, +0x1a0, +0x1e0, +0x1f4, +0x214, +0x230.., +0x250.., +0x2b0.
  - That assumes the car's constructor leaves them zero, as `Machine::new` does. This was not checked by disassembly of the constructor.
- **Mounts and inertia seeded from RAM.** The bit-for-bit replay uses the original's tire mounts and inertia. The port's own differ by the model round trip (S149e: under 0.002 ft and 1e-5).
  - With the port's own, the bodies part from touchdown (frame 8) in the last bits. They still sleep on frame 56, about 0.05 ft from the original.
- **Images not matched to the bit.** The image rest poses cannot be matched bit for bit: each run's load frames used their own dt (the launch timers differ between the images).
- **Spawn height from one car.** The spawn height rule (ground + bounds centre y) is matched on one car only (the truck). The ground query's start point was not checked by disassembly.
- **Bikes vs parked cars not in the port.** `Sim::parked` is a static hull for the AI riders' avoidance only. Bikes in the port do not strike it, and nothing wakes or pushes a parked car.
  - The capture is a single run, with a teleported bike. Its nearby search was gathered at the old spot (see the teleported-image note), but the bike rode 50 ft first.
  - Filed as S485.
- **Shell's taken car keeps the lift.** It still starts CofGAI over its rest height, which the original does not do for its parked cars. Its rest height was tuned with the lift (P370a), so changing it is left to that work.

## Next

- S485 (filed): the bike's strike on a parked car's hull, the car's wake and push, and its sleep again. Prove it against `ride1.csv`.
- P378 can now fill `Sim::parked` with `parked_spawn` → `Machine::new` → `settle` → `Sim::park`.
