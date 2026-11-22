# 1 — driven car meets parked cars (PART)

Task S499. 2026-11-21.

## What I did

- game-sim `car.rs`: a driven car's step now takes the parked cars handed in its `Met` (`met.cars`, as
  the plane's does since S489):
  - Its search adds each parked car's hull as a knock, so the car's rays strike their hulls. Those
    entries are renamed past `WRECK_FROM`, then to the bodies' places, so `knocked()` never takes them for
    props.
  - A struck parked car is woken (`InPlane`, the plane's woken-car stepper, reused). A woken one searches
    the driven car's hull, and its body joins the driven car's dynamics world after the car (body 0).
  - `AmongCars` is that world: body 0 steps as the car's own `Stepping`, the rest through `InPlane`.
  - New `Machine::step_cars` (none handed: `step_met` as before), `cars_in_car` / `cars_from_car`
    (hand the cars in and out, say which went to rest).
- game-sim `lib.rs`: `Sim::parked_cars_rest` sleeps those.
- shell `vehicle.rs`: `CarRide::step_traffic` hands the Sim's parked cars to a lone driven car (one with a
  hull and a collision object) and back; `step_met` takes `step_cars` when cars are handed.
- shell `main.rs`: split screen with one driven car and no lone plane hands the shared parked cars to that
  seat's pass in its `met`, and takes which went to rest from it, as the lone plane lane does (F72).
  The cars are not framed a second time for that tick.
- Tests (disc):
  - `the_shells_car_wakes_and_pushes_the_parked_truck`: the buggy rolled at 30 ft/s into the truck's tail
    stops at z 15.5 (19.2 with nothing parked); the truck is woken and pushed 2.35 ft.
  - `split_seats_share_the_parked_truck_the_second_seats_car_pushes`: the same through split screen's
    step with seat 0 100 ft off; both seats hold one truck; pushed 2.3517456 ft, the lone seat's figure
    to the bit.

## What the original does

One dynamics world per driven vehicle holds every woken machine it has struck. The driven machine's ray
search covers the parked machines' hulls; a struck one is woken and its body stepped among the driven
one's, pushing and pushed. This port's plane already did this (S489); the car now does it the same way.

## How it was proved

Two disc tests (above): woken, stopped short, pushed along. No capture: see below.

## Not verified / not 1:1

- **No capture of the original.** The brief asks for a capture of the original's car driven into free09's
  parked monster truck. `research/s499/` boots free09 with the buggy as the event's machine and a gas
  tape (`context/capture/s499/switch.p2m2`), but the desktop was locked (hyprlock over every output, the
  headless one too). The copy also ran at `NominalScalar = 0.05`. PINE timed out, and the recorder
  only logged null pointers at the car's cells (the tape's machine switch never put the buggy out), so it
  wrote no rows. So the
  push size (2.35 ft) and the stop point are this port's numbers, not the original's.
- **Body order assumed.** The driven car is body 0 and the parked cars follow, as R48's node holds the plane
  then the truck. Not checked for a car.
- **The tire sort** takes only prop knocks, not the parked car hulls. Whether the original's tires see
  the parked hulls is not checked.
- **The struck "driven" word** is set on the parked car as for the plane (S489). Not checked for a car.
- **Several driven cars at once in split screen** don't meet the parked cars. Only a lone driven car with no
  lone plane does, mirroring F72's lone-plane rule.

## Next

The human runs the wizard `context/notes/wizards/S499.sh`: unlock the desktop, then record the buggy
driven into the truck as `context/capture/s499/ride.p2m2` from save slot 4. Then capture rows from it with
`research/c34/capture.sh` (car and truck bodies), and compare the truck's push and the buggy's stop with
the two tests above.
