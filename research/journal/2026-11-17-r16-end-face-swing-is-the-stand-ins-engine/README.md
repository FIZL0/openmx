# R16 — the end-face swing is the stand-in's engine

2026-11-17. Task: `./kit plan R16`.

S213 saw the port's stock bike, rolled off the moving wagon's front onto the next car, swing its chassis
velocity to −135 then +135 ft/s in two ticks, and asked whether the mover contact response doubles the
car's motion. It does not. The swing is the stock bike's engine: its hand-written `EngineBraking` (0.3) is
six million times the disc bikes' (0.00000005). When the hanging rear spins past the idle revs, the clutch
locks and puts about −6,800,000 of drive torque on a tire that has no load. With the disc's number, the
bike lands on the hopper ahead and rolls through it with no swing, and its wheels take the car's motion
off once. Test: `tests/traffic.rs` `a_bike_rolled_off_the_wagons_front_meets_the_next_car_once`.
Follow-up S351 fixes the stand-in's number. The original was not recorded on an end face. Details in the
numbered file.
