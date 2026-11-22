# 1 — woken truck two way contact (PART)

Task S251. 2026-11-21.

## What I did

- Made S207's ctruck ride again on copy 3 from s89-along (`research/s251/ride.sh`, `strike.py`): the
  capture `context/capture/s251/ctruck.csv`, copied to `context/capture/s207/ctruck.csv` (S207's was
  gone), and RAM images `context/states/s251-ctruck-12060..12066.eeMemory.bin` around the strike
  (frame 12063, tick 44).
- Ran the original's wake (`./kit fn 0x003e3b00`) in the interpreter on the image before the strike,
  called as the bike's search calls it, against the port's `PhysicsCar::wake`: node and body bit for
  bit. The node update (`./kit fn 0x0026f800`) on the truck's world node, alone on that image, makes
  the strike image (truck body velocity, bike chassis velocity and rates bit for bit); chained from
  12066 it makes rows 12067..12071. Test: `tests/physics_car_struck.rs`
  `the_wake_on_a_bikes_strike_is_the_originals`.
- `Traffic::place` stands a physical car's path mesh down (empty mesh, radius 0), as the wake clears
  S208's two enable bits; the put-back restores it.
- `Sim::cars_among` / `Traffic::physics_frame_among`: each physical car's node update runs ahead of
  its held frame with the ridden bikes on its node (player 1's frame mesh at every bike's transform).
  Its rays strike the bikes, its step pushes their chassis (`bumped_all`), and its rays' kind-1 mirrors
  go on the bikes' lists (`Sim::cars_probed`, other = the car's body in `Bump::cars`). The bikes stay
  on the node through the step and the settle: the post regathers them.
- The bike's frame is resolved when it has mirrors and nothing else on its list (the solo gate and the
  two step_all loops).
- S207 ctruck: every lane agrees through tick 50 (was: parted on the strike tick); parts on 51.

## What the original does

- The node update does it all: the bike's search strikes the path mesh, the wake runs, the truck's
  body steps among the bikes, and both lists are resolved in the same call.
- While physical, the path mesh is out of the searches. The truck's step gathers the bike's frame and
  files its rays' hits twice: kind 0 on the truck's list (pushing the truck), kind 1 mirrors on the
  bike's (pushing the bike). On ticks 44..51 the bike's list holds only those mirrors: no body-ray or
  sweep entry on the f.col. Details and the tick-51 force sequence: `research/oracle.txt` S251.

## How it was proved

- Interpreter oracle on the S251 strike images: the wake, and the node update reproducing the strike
  image (one frame) and five chained frames against the csv.
- Bit-exact replay fixture: `tests/replay.rs` S207 ctruck, 51 (every lane equal on ticks 0..50).

## Not verified / not 1:1

- Tick 51 of ctruck parts (vel.x 21.917706 against 21.303883). The bike's list there is the original's
  four mirrors, but the force before the resolve differs: the port's rear wheel is on the truck's f.col
  and pushes along its normal, the original's second wheel add points the other way; and the port adds
  the fourth mirror where the logged original shows three adds. Not settled which one is wrong.
- S219 rear was not rerun against a new capture: its csv and images are gone. The unblock step (the
  wake against the interpreter) was done on S207 ctruck's new images instead.
- Every bike's frame on the truck's node is player 1's frame mesh: one mesh for all players, as
  `Sim::car_contacts` already does. A two-player strike was not captured.
- The bikes' body rays and sweep do not file the f.col: not needed on ctruck (the original's lists show
  only mirrors), not read for a bike driving fast into a woken truck.
- Main's replay row for ctruck (36) was for S207's lost capture; with the new csv in the shared context
  main reads the new ride until this lands.

## Next

- S505 (filed): the tick-51 wheel/mirror force. Log the original's per-wheel contacts on 12070
  (the wheel probe's record) and compare with the port's wheel hit on the f.col.
