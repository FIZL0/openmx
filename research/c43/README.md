# C43 — the Kiowa strikes a knockable prop on free09

Journal: research/journal/2026-11-18-b34c-kiowa-strikes-props (2-c43-kiowa-strike-capture-FINAL.md). Note under
`# C43` in research/oracle.txt. The captures are in context/ (git-ignored, the shared ~/repos/openmx/context).

The Kiowa takes off from C14's state, turns its nose 18.6° with R1, and flies backwards (stick back) at ~30 ft/s.
Its tail strikes free09's "egg" (prop 5 of the .toy's 62, asleep) at frame 1579. No wreck.

- `c43.toml`: the watch. watch/c14-kiowa.toml's columns, then the helicopter's collision record (count and all 22
  entries), and the egg's body (enabled, position, velocity, quat, rates, raw 0x200), collection flags, record
  (count, the first 32 entries) and f.col +0x100..+0x140. The egg's addresses are fixed (C14's state is one heap).
- `fly.sh PID NAME ROWS SEGMENT...`: builds the tape (research/c14/make_tape.py's segments, two fields a tick), puts
  C14's state beside it, plays it with the recorder armed on the human's helicopter. With `C43_AT=F1,F2,..` it
  records through `strike.py`, which saves PCSX2 slots at those frames and unzips their RAM images.
- `look.py CSV [STEP]`: the track (yaw, sticks, position, velocity, distance to the egg, the egg's state).
- `strike_rows.py CSV FIRST LAST`: the record entries and the egg around the strike.
- `images.py CSV IMAGE... [--against CSV2]`: which row each image holds (the helicopter's and egg's bodies
  against body_raw/prop_body_raw); and two runs compared lane by lane.

The strike's tape: `60 30:cross 40 18:r1 72 600:-:127,255` (rest, climb, rest, R1, rest, stick back).

| file | what |
|---|---|
| `context/capture/c43/strike.csv` (+ `.json`) | 460 rows, frames 1204..1663, no gaps |
| `context/capture/c43/try6.csv` | the same tape, an earlier run; its two entry columns read the wrong words (the record, not the array) |
| `context/recordings/c43/strike.p2m2` (+ `_SaveState.p2s`, C14's state) | the tape |
| `context/states/c43-strike-1578.ee.bin` / `.spr.bin` | row 1578: the tick before the strike, the egg asleep |
| `context/states/c43-strike-1579.ee.bin` / `.spr.bin` | row 1579: the strike |
| `context/states/c43-strike-1580.ee.bin` / `.spr.bin` | row 1580: the tick after |
| `context/c43/strike-157{8,9}.p2s`, `strike-1580.p2s` | the save states the images came from |

Each image holds its own csv row (the helicopter's and egg's 0x200-byte bodies equal that row's raw blocks), not
the next one as F27a1's knock images did.

Scouting (deleted): stick back alone passed 49 ft north of the egg; full or 1/3 lateral stick banks and turns, so
the path curves back; R1 for 10 ticks turns the nose +12.6° and it holds there; R1 for 15 passed 31 ft south.
