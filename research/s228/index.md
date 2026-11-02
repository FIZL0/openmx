# S228: every contact in fly_and_collide

The user's `fly_and_collide.p2m2` (with its `_SaveState.p2s`) is a two-player state of the C10
session's split free09: dt 1/60, one tape field per tick, frames 136505..140026 (3522 ticks). The
state holds frame 136504.

**Nobody flies.** Both biplanes (actor list entries 0 and 1, both src 2) stand parked on every tick:
their speed stays under 0.003 ft/s, and plane 0 creeps about 4 ft in x over the tape. P2 is idle at the
start on every tick. P1 rides a bike for the whole tape. So every contact on this tape is P1's bike: on
the ground, in its own landings, or on the scenery. There are no rider-vs-rider contacts, no contacts
with the planes or the path cars, and no truck rides.

Capture: the fly capture under context/capture/s228/, recorded with watch/2p.toml on PCSX2 copy 3 at
NominalScalar 0.05 (research/c10/play_capture.sh). It has 3522 rows, no gaps and no guard misses.
Index it with `python3 -B research/c10/index.py s228-fly` (through a symlink under context/capture/2p/).
Landings: `python3 -B research/s228/landings.py`.

## Wrecks (all P1)

| frame | reason | clause (index.py) | value | speed before |
|---|---|---|---|---|
| 137251 | 3 Right | landing crossup | hang time 1.618 s; touched down at -23.9 deg pitch and 18.6 deg roll | 50.6 |
| 138436 | 8 NoseDive | airborne roll | roll -85.28 deg (limit +-85), pitch 59.5 | 27.0 |
| 139446 | 1 Front | fore-aft g < -FrontWreckSpeedLimit | g.z -56.64 (limit -18.33): a body ray on scenery at 3.08 ft, normal (-0.98, 0, -0.19) | 59.2 |
| 139894 | 6 Scenery | a body ray on scenery | ray end y 4.58, normal (-0.83, 0, 0.56) | 19.2 |

The scenery wreck is still on at the tape's end (ragdoll 139895..140026). The other three are put back
after 155 rows.

## Landings after 10 or more airborne ticks

| touch | air ticks | hang s | pitch deg | roll deg | vy ft/s | speed | nose-first | verdict |
|---|---|---|---|---|---|---|---|---|
| 136842 | 99 | 1.652 | 7.39 | -10.47 | -35.4 | 43.4 |  | rode out |
| 137250 | 96 | 1.618 | -23.88 | 18.55 | -55.4 | 50.6 | yes | wreck 3 (crossup) at 137251 |
| 138591 | 214 | 3.570 | -1.96 | -76.26 | -8.4 | 0.0 |  | the put-back after wreck 8 |
| 138838 | 35 | 0.567 | -4.61 | 11.94 | -24.1 | 50.3 |  | rode out |
| 139063 | 114 | 1.902 | -62.62 | -4.69 | -46.5 | 70.5 | yes | rode out (ground pitch down at 89% of its 70 deg limit; vertical g 45.4, 69% of 66) |
| 139142 | 73 | 1.218 | -34.96 | 1.43 | -40.4 | 0.0 | yes | a put-back (body mode 0 at 139143), not a ride |

The nose-first landings (S227): 137250 wrecks, but on the crossup clause, not on the nose. 139063
rides out with the nose 62.6 deg down.

## Near misses (ridden out)

- touch |roll|: 75.35 deg at 136991 (89% of 85) and 72.10 deg at 137021 (85%).
- ground pitch down: 62.62 deg at 139063 (89% of 70). Vertical g: 45.38 at 139063 (69% of 66).
- ground pitch up: 49.24 deg at 138351 (70%), 45.40 at 137930, 45.12 at 137860.

## Other

- Put-backs in body mode 1 (the rider's own reset, not after a wreck): 138687, 139143, 139748.
- One backflip: flip_on 139396..139415.
- P2 plays get_ready idles 137186..137306 and 139533..139653 (body mode 4).

## Images

`context/states/s228/s228-<frame>.ram` (made with research/c10/play_at.sh and research/s228/rams.py),
at the frames in research/s228/image_ticks.txt: ten or more ticks before each event above, and every
100 ticks from 136510.

## Where the port parts

`p1_rides_each_event_as_the_original` (crates/game-sim/tests/fly_and_collide.rs) holds all nine
events above. Every tick matches the capture's chassis velocity and centre bit for bit. The lean and
fore-aft request axes match on every riding tick. All four wrecks match, with the original's reason
in the original's frame. The 139063 nose-first landing rides out as the original's does.

`p1_rides_the_tape_as_the_original` rides each 100-frame stretch from its image. Seventeen of the 35 stretches
hold to their end. The rest are pinned in `HELD`:

- **A wreck under way at the image** (137310, 138510, 139510, 139910). The seeder builds a riding
  rider, but the original's rider is a ragdoll at that point. These are not partings.
- **The rider's own put-back** (138610 at 138684, 139110 at 139141, 139710 at 139746). Select puts the
  bike back on a respawn spot, and the port does not place it the same.
- **A preload's force** (138810 at 138907, 139310 at 139384). The stick is thrown forward on a lip.
  The force is the flick's speed, and the port samples the flick its own way (`ride::Flick`), because
  the original samples it from DMA waits on a hardware timer. Not reproducible tick for tick.
- **Unexplained, on the ground at speed with the stick moving**: 136610 at 136624, 137410 at 137462,
  137610 at 137637, 137910 at 137943, 138010 at its first tick, 138110 at 138179, 138210 at 138212,
  138310 at 138311, 138710 at 138729. Many fall in the wheelie stretch 137860..138351. Not chased
  here; the follow-up is filed in the plan.

Images just around these partings were started (research/s228/parting_ticks.txt) but not finished.
Each one replays the tape from the state at NominalScalar 0.05, which takes hours.

The original's whole frame (S176's `judge_whole` in ragdoll_wrecks.rs), run from these images, stalls
in a hardware wait (call 0x133500 at pc 0x198bf0). C10's images do not. So the wrecks here are judged
on the port's ride against the capture only, not call by call against the original's wreck test.
