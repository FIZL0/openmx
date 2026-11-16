# 1 — car on its side (FINAL)

Task MD169. 2026-11-16.

## What I did

- No change to the port. The task's guess was that MXU's straight-down ground query is wrong for a
  car on its side. It is not: the original's tire probe also asks the ground straight down, and the
  port already matches it word for word on its side.
- `tests/mxvatv_car_frame/drives.rs`: `beside` gained `beside_posed`, which can first put the car on
  its side in the image (`put_on_side`: turned a quarter about its forward axis, lifted 3 ft, still).
  On its side the run stops at the original's respawn, which the port hands out and this harness does
  not answer.
- New test `each_car_put_on_its_side_is_the_ports` (same file): all five cars on their side, the
  port's frame beside the interpreter's.
- New test `each_car_dropped_on_its_side_wrecks_tipped_or_stands_up` (`tests/mxvatv_car_mxu.rs`): each
  car rigged on free09, dropped onto its side 2 ft over its riding height.
  - Under each tire, the world's ground keeps the asked x and z bit for bit, as the original's does.
  - Each car either wrecks tipped and is put back upright at the spot kept by the wreck, or stands
    back up on its wheels. Either way it then rides on 10+ ft without wrecking.
- Fixed the doc comment on `each_car_rolled_over_wrecks_tipped_and_is_put_back`: it repeated the
  wrong guess.
- Nothing taken from opentennis: it has no car with tire probes.

## What the original does

- The tire probe (`./kit fn 0x004b51f0 -p atv`, MD127's) asks `./kit fn 0x004833c0 -p atv`, the
  heightfield query. That query keeps the point's x and z and writes the ground's y into it. It does
  not look along the car's down.
- On its side, the probe's drop is along the car-frame normal with its x zeroed. With the ground
  normal along the car's x, the drop's direction comes from the small leftover y and z. Then the
  ground is read straight down from wherever that puts the axle bottom. The port does the same.
- Put on its side at the MD39a images' spots on Desert Oasis and dropped 3 ft:
  - the buggy stands back up on its wheels and never wrecks in 150 frames (up 0.998 at the end);
  - the baja and sandrail wreck tipped at frame 2, the truck at 6 and the golf cart at 11. Each
    respawns 61 frames later.
- So "its suspension stands it up before the tipped clause holds" is the original's behaviour too,
  for the buggy at least. Whether a car wrecks depends on which touches first: the record's body rays
  (touching) or the tires (one side down, or grounded).

## How it was proved

- Interpreter beside the port: `each_car_put_on_its_side_is_the_ports`. SLUS-21104's own sim frame
  runs from each MD39a image, poked onto its side. The port's car frame runs beside it, with every
  word of car, body, pieces, vehicle and record equal every frame. That is 150 frames for the buggy,
  and 62/62/71/66 frames (up to the respawn) for the baja, sandrail, golf cart and truck. The ground
  queries come from the real terrain in the interpreter (the Oasis answers on the port's side).
- Mutation: dropping along the full car normal (x kept, the "car's down" guess) turns the side test red
  at the buggy's frame 1. Restored with cp + touch.
- On MXU: `each_car_dropped_on_its_side_wrecks_tipped_or_stands_up`. The buggy wrecks tipped at tick 91
  after rolling about. Sandrail, golf cart and truck wreck within 1 to 8 ticks. The baja stands back
  up. All five ride on.

## Not verified / not 1:1

- **The task line's test, "a car rolled onto its side wrecks tipped", is not what the original does
  for every car.** The original's buggy stands back up off its side. The MXU test therefore allows
  either outcome. It does not say which car does which, because MXU's ground differs from Desert
  Oasis's, so the outcomes on free09 cannot be compared one for one. On free09 the buggy wrecks and
  the baja stands up, the other way round from the Oasis.
- **Only one pose, from one spot per car.** A quarter roll to the same side, 3 ft up, still, at each
  image's spot. Other drops, rolls to the other side and sloped ground were not run beside the
  original.
- **No PCSX2 capture of a car on its side.** The proof is the original's code in the interpreter, from
  poked images. The pose was never reached by driving, so the scene node and collision record start
  one frame stale. Both sides start equally stale, and the next frame's post rebuilds them.
- **MXU's ground query is the original's contract (straight down) only on terrain.** Props are not
  ground on MXU (the sort strikes nothing, as MD149 notes), so a car on its side against a prop was not
  looked at.

## Next

Nothing: the task is done.
