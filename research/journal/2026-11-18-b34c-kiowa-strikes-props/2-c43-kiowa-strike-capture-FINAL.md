# 2 — the Kiowa strikes a prop, captured (FINAL)

Task C43. 2026-11-20. The capture B34c asked for, so S471 can port the strike.

## What I did

- Listed free09's 62 knockable props in C14's image, by distance from the Kiowa's spawn. The nearest is the
  "egg", 232 ft east: index 5 of the .toy's list, asleep.
- Flew generated tapes from C14's state on PCSX2 copy 1 (`research/c43/fly.sh`, speed 1.0):
  - Stick back flies +x (the nose faces -x). Stick back alone passed 49 ft north of the egg.
  - Lateral stick banks and turns, so the path curves back. Not used.
  - R1 turns the nose and it stays there: 10 ticks gave +12.6°.
  - 15 ticks of R1 passed 31 ft south. 9 ticks (18 fields) struck the egg with the tail.
- Ran the strike's tape again with `strike.py` saving slots at 1578, 1579 and 1580. Unzipped their RAM
  images into context/states/c43-strike-*.ee.bin / .spr.bin.
- Watch `research/c43/c43.toml`: C14's columns, plus the heli's record entries and the egg's body,
  collection flags, record entries and f.col.

## What the capture shows

- Strike at frame 1579. Row 1578: egg asleep, heli record empty. Row 1579: egg enabled, at 47 ft/s.
- The heli loses about 3.5 ft/s in x. It does not wreck.
- The heli's record holds one k0 entry naming the egg's collision object, then 16 k1 mirrors.
- The egg's record holds the mirror of that k0, two ground hits, then 16 k0 naming the heli.
- Contact lasts rows 1579..1592. The generator does not move at the strike.
- Full numbers under `# C43` in research/oracle.txt.

## How it was checked

- Each image's heli and egg bodies (0x200 bytes) equal one csv row's raw blocks: 1578, 1579, 1580 in turn.
  So these images hold their own row, unlike F27a1's.
- The same tape run twice (try6.csv, strike.csv) agrees on every lane over all 460 rows. The pads, stamps
  and the two entry columns are left out (the entry columns were re-defined between the runs).

## Not verified / not 1:1

- Which of the heli's 22 rays (or its sweep) made the k0 hit is not worked out. The entry's +0x8 is
  0x014a6f20, not read.
- The f.col's +0x116 reads 0x54 on every row. I did not check that offset is the flag byte the notes mean.
- One strike, one prop, from behind (the tail). The egg is not a sign. No nose strike, no rotor strike.
- No port change. That is S471.

## Next

- S471: seed the port from c43-strike-1578, step one tick against 1579, then on through the csv.
