# 1 — parked cars on the other paths (FINAL)

Task S486. 2026-11-19.

## What I did

- Wire (`snapshot.rs`): a `S486park` section, only when the ride has parked cars.
  - Per car: machine, collision object (less its ray probe) and `rest`.
  - The `.phy`, hull and probe stay this machine's.
  - Decode refuses a count that differs from this machine's, or a missing section.
  - `Sim::parked` (the AI riders' hulls) is rebuilt from the decoded `rest`.
  - The hash's `parked_cars` blob is now these same bytes.
- `car::Parked::rest`: where the car last fell asleep. The AI hull stands there.
- Sleep moved after the riders (`Parked::frame` only says "at rest"; `Parked::sleep` runs after
  the bikes in `step_all` and in the plane lane).
- `parked_wreck.rs`: a downed rider's wreck bike near a parked car (within 20 ft of the hull's
  sphere) is searched with the cars' hulls and stepped with their bodies, before the rider's frame.
  - Same pieces as the plane's: `WreckBike::search_flown` / `step_flown`.
  - A mirror on a car wakes it. The bodies are written back.
  - Far from every car, the rider's own frame ticks it as before.
  - Skipped when the plane lane already ticked that wreck bike (`wreck_plane`).
- Plane lane (`Sim::step` with a plane up):
  - Awake parked cars step ahead of the bikes, and sleep after.
  - Their hulls follow the plane's in the other riders' contacts. A mirror wakes the car.
  - Each bike's bump carries the cars' bodies after the plane's (before it if the plane has no hull).
  - The other riders' wreck bikes meet the cars as in `step_all`.

## Who wakes the truck at rest (rows 68 and 70)

The wrecked rider's wreck bike, sliding back along the truck's nose.

- With only the wreck bike's search added, the port's truck stayed awake for three frames running
  (strike+9 to +11). The original wakes on alternate frames.
- The alternation fits this order: the car steps → the riders strike → the sleep check, which only
  puts to sleep a car that stepped this frame and is at rest.
  - Row 67: steps, struck, sleeps (asleep).
  - Row 68: asleep at the start, so no step; struck (awake).
  - Row 69: steps 0.001 ft, sleeps.
  - Row 70: struck again (awake). Row 71: steps, sleeps.
- With the sleep after the riders, the port matches rows 67 to 69: asleep, woken with no step,
  steps 0.0012 ft (theirs 0.0009) and sleeps. Slid 0.2024 ft (theirs 0.2029; 0.2012 before).
- Not matched: the second wake (row 70). The port's wreck bike has slid clear by then.
  - The port's wreck bike first moves 0.38 ft toward the truck, then slides back. The capture's
    vehicle place only slides back. Its measure point may differ, so this is not settled.
- The ragdoll was not needed to explain either wake.

## Not done (filed)

- The plane's own rays against a parked car's hull, and its resolve pushing the car.
- The knockable props' rays against a parked car's hull.
- In the plane lane, a wreck bike the plane's lane ticks (`wreck_plane::search_downed`) does not
  also have the parked cars in its world.
