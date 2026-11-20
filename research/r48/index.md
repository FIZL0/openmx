# R48: the biplane taxied into free09's parked truck, on the original

C16's 2p session parks no truck (both machines are planes; no car vtable 0x0045edac in its images),
so R48 booted its own free09 for one player with the plane and the truck parked.

## Steps (kit copy 2, accessibility bus on)

1. `bash research/r48/start.sh` cold-boots copy 2 and runs `boot.py`: the S149a boot with event
   +0x11c = 1 (`[Biplane1]`), +0x128 = 4 (`[Monster2]`), track free09, one player. Slot 1.
2. Slot 2: the switch cheat set. `context/recordings/r48/nav.p2m2` (from slot 2) opens the pause
   menu and switches to the plane. Slot 3: in the plane.
3. `aim.py PID 3 0 -45 5`: the plane put back 45 ft south of the truck's body, nose at it
   (`+z`), dropped onto its wheels. Slot 5, frame 1487. East tipped it on the hill; west faces the
   barn; north has posts close.
4. `context/recordings/r48/drive.p2m2` (from slot 5): Cross 160 fields (80 ticks), idle 440.
5. `KIT_PCSX2=2 ./kit pcsx2 with -- bash research/r48/play_capture.sh PID TAPE 1488 290 OUT`
   with `watch/r48-truck.toml`; `show.py OUT` prints it.

## Files (shared context)

- `capture/r48/truck.csv`: rows 1488..1777, no gaps, dt 0.03333 every row.
- `states/r48/r48-1487.ram`: slot 5's EE RAM (the drive tape's state).
- `recordings/r48/{nav,drive}.p2m2` and their `_SaveState.p2s`.

Addresses in this boot: plane 0x007ccae0, truck car 0x01490240 (body at +0x160), the human's bike
0x015613d0.

## What it shows

- The throttle (+0x34c) ramps 0.0117 a tick and holds at 0.933 once Cross ends.
- Row 1569: the truck wakes, velocity (0, -0.39, 14.02); the plane 32.38 -> 21.86 ft/s, contacts 0 -> 1.
- The plane keeps its throttle on the truck and pushes it ~40 ft (z 3877.7 by row 1628). The truck
  stays awake to the capture's end.

## The Sim against it (crates/game-sim/tests/c16_fly.rs, `r48_*`)

- Truck settles within 0.11 ft of the original's body.
- Taxi rows 1488..1568: throttle exact, plane within 0.02 ft and 0.1 ft/s.
  The first tick after the load reads the idle pad.
- Row 1569: wakes on the same row, shove 14.12 vs 14.02, plane 21.859 vs 21.864.
- Before S487 the Sim's plane stopped dead about 13 ticks on, and the truck moved ~5 ft and slept
  from about row 1595.
- S487: the woken truck now steps after the plane, as the original's bike world does.
  - Its record takes the plane's mirrors. A spring only pushes its own side, so without them the
    plane's rays never pushed the truck after the first impulse.
  - Its own rays strike the plane's hull. This doubles the push, and is what holds the truck.
  - Rows 1570-1584 now match along z: place within 0.3 ft, speed within 2.6 ft/s. That covers
    1571-1573 with no plane contact, the 18.9 ft/s kick at 1574 and the climb to 25 ft/s.
  - Both stay awake to 1640. The truck reaches z 3872.5 by 1628 (original 3877.2).
- Still off from ~1585: the Sim's plane drags at ~21 ft/s against the original's 24-30. So the
  truck runs ~7 ft/s slow, ends ~5 ft short and still rolls at 1640.
- The wreck-bike test is not captured: this boot has no P2.
