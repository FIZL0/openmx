# S207 — path cars into a parked bike, against the original

2026-11-02. Task: `./kit plan S207`.

S153's idle sweep had the port's parked bike struck and wrecked by path cars on free09's road and on
free06. S207 parks a bike in a car's way in the original (S76's teleport, no pad) and replays the port
from the same state. Four rides, `tests/replay.rs` `a_path_car_into_a_parked_bike_wrecks_it_as_the_original_does`:
free09's trailer truck and free06's train both wreck the bike on the original's tick with its reason;
a bike beside the rails is passed untouched in both; free06's company truck (a `HasPhysics` car) knocks
the bike without a wreck in the original, and the port, which does not stop a struck truck, wrecks
it (B8). Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
