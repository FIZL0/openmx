# 1 — trucks captured (FINAL)

Task S219. 2026-11-02.

## What I did

- Captured the original meeting free06's construction trucks and a free09 trailer, four rides in all.
  Each ride is a save state of a bike placed in a path car's way, recorded from its first frame. The
  RAM images and captures are in s2's `context/` only:
  - `context/states/s219-<name>.eeMemory.bin`
  - `context/capture/s219/<name>.csv`
- New scripts in `research/s219/`:
  - `segments.py` finds path stretches square to a heading.
  - `launch.py` makes a state. It loads a slot and reads the car's path keys over PINE. It runs to a
    set point a little before the car reaches point P. It teleports the bike back from P along its own
    heading (S76's teleport), gives it a speed and the matching wheel spin, runs 3 frames and saves.
  - `record.sh` records a slot with no pad. It loads the slot itself, starts `./kit record` on a copy
    of the spec without its `state =` line, and unpauses over AT-SPI. `./kit record --state` stalls
    because without a pad it cannot unpause.
  - `look.py` lists where a capture's wreck reason changes.
- New watch specs:
  - `watch/s219-trucks.toml`: free06, S129's lanes plus each truck's clock and position.
  - `watch/s219-trailer.toml`: free09, S207's lanes plus the cab's clock and position and the
    trailer's position.
- New replay test `a_bike_meets_a_moving_truck_as_the_original_does` in `tests/replay.rs`. It rides
  each capture in the port from its RAM image, among the image's path cars and the two HasPhysics
  trucks, as S207's test does. It holds every lane to an agreement tick and holds both first wrecks to
  the table `S219_RIDES`.

## The rides, and what the original does

All four were recorded on copy 2 at quarter speed (NominalScalar 0.25), with nothing held on the pad.
The states and the recipe lines are in the S219 comment in `tests/replay.rs`.

| ride | state | the meeting | original | port |
|---|---|---|---|---|
| flank | S129 across (free06, the 250), slot 23 | bike at 30 ft/s, square to within 0.7°, into the flank of dump truck 15 (23.6 ft/s, `0x1cc9300`) | **6** on tick 20 | 6 on tick 20; every lane agrees to 20 (the origin is off by an ulp there) |
| slow | the same, slot 24 | the same at 6 ft/s | no wreck: knocked back from (+2.5, +3.3) to (−11, −9) ft/s, then rolls down the bank | no wreck; every lane agrees over all 200 rows |
| rear | the same, slot 25 | bike standing on the line of the company truck at `0x1cc4440` (HasPhysics, 27.9 ft/s), facing along it | **4** on tick 42; the truck stops dead | 4 on tick **41** |
| trailer | first_recording (free09, the 125), slot 26 | bike at 30 ft/s square into trailer A (`0x1268160`, 50 ft behind its cab `0x1263570`, 45 ft/s); it climbs the bank to the road and meets the trailer at 18 ft/s | no wreck: stopped dead (18.2 → −2 ft/s) | no wreck; every lane agrees to the strike on tick 45 (velocity off in the 3rd digit there, within 0.01 after) |

- **S216's answers, checked:**
  - "Reason 1 into a flank at ~30 ft/s" did not happen in the original. At 27 ft/s the bike's front
    runs in under the dump truck's body (the bed overhangs the wheels), and the head ray meets the body:
    reason 6, with g_peak only (−1.05, −1.05, 0.10). The port does exactly the same from the same state. S216's 1 came from a
    game-sim yard ride (the 500, a flat floor, full gas), so it is not this geometry. Nothing here
    contradicts the port's rays.
  - "4 for a rear-end at 28 ft/s" agrees: the original gives 4, g_peak (6.27, 10.60, 37.68).
  - "6 for a head into the body" is what the flank ride is, and the port agrees on it.
- **The rear tick:** the port's truck and the original's are at the same point, to the digit, on every
  tick to 41 (`ctruck_pos` and `ctruck_clock` against the port's car point and clock). The bike agrees
  on every lane to 40. On tick 41:
  - the port's bike is struck by the truck where the truck stands on 41;
  - the original's bike is not, and is struck on 42. Its truck is frozen from then on at the tick-41
    point (the wake: S186/B8).
  So the original's HasPhysics truck strikes a tick later than its own path point says. On the same
  ticks, the path cars of flank and trailer strike in the port and in the original alike. S207's
  ctruck had the same one-tick-early contact (B8). I noted this case on B8 rather than filing a
  second task.

## How it was proved

- The original:
  - four PINE captures from save states, rows on every frame (quarter speed), 0 torn reads and 0
    counter gaps each;
  - rider_ctl_src reads 1 on every trailer row (the free09 chain).
- The port: `./kit check -- -p game-sim --test replay`: 111 passed, plus the 1 test that is always
  ignored. This includes `a_bike_meets_a_moving_truck_as_the_original_does`, where each first wreck
  equals the table. For rear, a temporary per-tick print of the truck (since removed) compared the
  capture's `ctruck_pos` and `ctruck_clock` with the port's nearest car's point and clock on ticks
  36–43.
- `./kit audit`: the only leak is the existing `plan/DONE.md:519`.

## Not verified / not 1:1

- **The rear ride's strike is a tick early in the port** (41 against 42). The reason (4) is the same.
  It is pinned in `S219_RIDES` and is B8's work (noted there). Not fixed here: it needs the
  HasPhysics subclass's update order read in the disassembly, and probably its wake (S186).
- **No pad.** /dev/uinput is root-only here, so nothing is held. The rolling rides coast at the speed
  `launch.py` wrote, and the rider makes no input. A real rider on the gas or the bars might meet the
  truck differently.
- **The heading is the state's.** The bike keeps the heading its source state had. So the flank rides
  are square only within 0.7° (flank) or 1.5° (trailer), and the rear ride is 6.5° off the truck's
  line (39.5° against 46°).
- **The bike is teleported.** S76's teleport moves every copy of the position by one delta, and zeroes
  the angular velocities and the history. It is not a state the game makes by itself. The rear bike
  popped about 4 ft on placement and rolled back toward the truck at about −3 ft/s, so "standing" means
  rolling slowly backward. The trailer bike dropped onto a bank below the road and lost speed to 18 ft/s
  before it met the trailer. That is the speed it met the trailer at, not 30.
- **One ride per meeting.** Each is a single geometry. Other speeds, other angles, the other dump
  trucks, the scrapers and the free09 cab (as opposed to the trailer) were not captured.
- **The trailer is trailer A only.** Its 50 ft lag behind the cab is the `.toy`'s LagDistance. The
  trailer's own position in the capture is taken as its `+0x470`, and that was never checked against
  its node.
- **The states and captures exist only in s2's context** (copy 2's slots 23–26 and the RAM images).
  The test skips with a line wherever they are missing.

## Next

Nothing for this task. B8 holds the HasPhysics truck's late strike, with this ride as its second case.
