# 1 — online quads meet the bikes (PART, blocked)

Task MD173. 2026-11-16.

## What I found

The task says riders pass through a seat's quad online "unlike offline". Offline they pass through it too.

- `main.rs`'s split-screen pass builds its contact hulls from a flown plane (`flight_flown`) and a
  driven car (`car_up`) only. A seat on its quad is in no list there either (its `on` is
  `garage.up == Kind::Bike`, false on the quad).
- `Garage::step_among` steps the quad with `atv.step(controls, dt, world)` and drops the `Among`.
- So there is no offline quad contact to copy into `net/rides.rs`.

The car path the task points at (S249/F26) needs three things the quad does not have:

- **A hull mesh.** `rider_contact::Hull` takes a `track::Mesh` (a car's `f.col`). The quad's two
  collision objects (`m.col`, `WreckATVm.col`) are read as `track::Rays`.
- **A pushable body.** `rider_contact::Flown` carries a `biplane::Body` and a `Collider`. The quad's
  body is `atv_body::Body`, a different type.
- **Mirrors on its record.** `atv_contact::Held::refill` refuses any contact with another record or
  a mover (MD98), so the bikes' strikes have nowhere to go on the quad's side.

That is game-sim work on the quad, and it belongs offline first. Filed as **MD175**. MD173 is
blocked after it: once MD175 gives the quad a hull, a `Flown` and a met, `net/rides.rs` adds it the
way it adds `vehicle::lane_hull` / `lane_flown` / `lane_pushed` for cars (about 20 lines).

## What I took from opentennis

Nothing: tennis has no vehicles meeting each other.

## Not verified / not 1:1

- **No code changed.** The online quad still passes through riders and they through it, as offline.
  Waits on MD175.
- **What the original MX vs. ATV does when a quad meets a rider** was not read. MD175 has to find it
  (the quad's record holding another rider's record, MD98).
