# 1 — untamed atv capture (FINAL)

Task MD147. 2026-11-15.

## What I did

- **Setup.** MD164's scratch SLUS-21701 kit.toml (kept-context/md164/slus21701-kit.toml, unchanged), PCSX2
  copy 3, NominalScalar 0.3. The copy must start with `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
  WAYLAND_DISPLAY=wayland-1`. Without the a11y variables, step.sh fails with "no accessible app".
- **Menus by tape.** Copy 9 is gone, so I started from MD160's kept vehicle-atv.p2s. Its Player 1 Options
  already reads Vehicle ATV, Physics Type RACE. research/md147/start.pad (MD160's menu4.pad: up, Cross, the
  load), played by `research/md160/step.sh`, gives **spawn.p2s** (vsync 27273). research/md147/air.pad
  (jump.pad cut at tick 155) gives **air.p2s** (vsync 27583 = jump.csv row 154, 8.3 ft up, 57 ft/s).
- **It is the race quad.** The spawned quad's +0x134 holds `atv_Race250P.phy`. Its vtable 0x003f0470 holds
  the quad update 0x004dd000 (MD81b's twin) at +0x10c, and its kind is 1.
- **Recorder.** research/md147/quad.toml is C28's research/md37a/atv.toml moved to Untamed's quad:
  - body +0xe8, controls +0x338, euler +0x390, ground +0x354, speed +0xc28;
  - wreck word +0x708 (the run's +0x24), engine +0x143c, wheels +0x17f0;
  - new columns: the hazard words (+0xc00 stuck, +0xc04 free), the stuck time +0xde0, body rates +0x1d0,
    and each wheel's grounded word +0xd0;
  - raw blocks: the quad (0x1900 B), body, engine and the wheels.

  research/md147/play.sh is MD164's play.sh with the quad's vtable in the start trigger.
- **Hazard states.** research/md147/poke.sh loads spawn, writes all 16 entries of the quad's surface hazard
  table and saves a new state:
  - **hazard-time.p2s**: (hazard 0, time 1.0), vsync 27313;
  - **hazard-on.p2s**: (hazard 1, time 1.0), vsync 27273.
- **Rides** (context/capture/md147; 2 vsyncs a row, no vsync gaps in any of them):

  | capture | from | rows | what happens |
  |---|---|---|---|
  | throttle, throttle-2 | spawn | 370 | gas straight, 66 ft/s by row 82. It hits Rocksbury's wall at z≈236 on row 84 and **wrecks**: wreck word 1, gas drops to 0, the quad rolls on its side |
  | steer | spawn | 205 | gas, then full right from row 52, full left from row ~72, half right (stick 0.28) from ~102, straight from ~122. Yaw goes to −1.18 and back; no wall |
  | jump | spawn | 370 | MD160's jump.pad: a left turn (stick −1, ramped stick −0.4 → −1), two jumps (all wheels off rows ~124–132 and ~150–183), and a **wreck** on row 196 that leaves the quad on its side, mostly with every wheel up |
  | land | air | 135 | gas held through the landing |
  | hazard-time | hazard-time | 370 | jump.pad. The clock counts in the air only, and on row 181 puts the quad back to (282.32, 6.7) |
  | hazard-on | hazard-on | 370 | jump.pad. The quad is put back to the spawn every ~1 s from the start. After each drop its rears' +0xd0 read 0 for a few ticks while the clock runs |

- **Kept.** kept-context/md147 holds:
  - spawn, air, hazard-time and hazard-on .p2s, and their .ee.bin;
  - capture/ and recordings/;
  - addresses.txt (heap addresses and the quad words);
  - the scratch kit.toml.

  The states are also in context/untamed/md147. `python3 -I research/md147/keep.py KIT.toml` makes the
  copy again.

## What the original does

**Heap addresses (the same in all four states).** Retail heap addresses, kept here and in
kept-context/md147/addresses.txt, not in code.

| object | player's quad | idle second quad |
|---|---|---|
| quad | 0x01be4760 | 0x01c72d30 |
| body | 0x01be6de0 | 0x01c65b30 |
| engine | 0x01c45c20 | 0x01cbee00 |
| wheels | 0x1c437a0, 0x1c440c0, 0x1c449e0, 0x1c45300 | from 0x1cbc980 |

- Spawn is at (395.64, 0.79, 159.78), heading +z, 4 ft behind the monster truck's spawn.
- The vehicle state pointer is +0xbb4 (0xe40c90) and the run is +0x6a4 (0x13a6b10, its +8 = 0x40606).

**The surface hazard table** is in the quad: 16 entries of 12 B at +0xde4 (hazard word, time, a third word).
Each wheel's surface byte (+0x110) indexes it. The quad update reads it before the hazard clock call:
`./kit dis 0x004dd000 -p game` from 0x4dd0f0 to 0x4dd254. It stores the any-hazard word at +0xc10. A spawn
init copies the table in: `FUN_004c9cf0` copies 0xc0 bytes from its second argument's +0x380. On Rocksbury
**every entry is zero** and every wheel is on surface 0, so no Free Ride on it ever reaches a hazard.

**The new hazard clock, live.** Untamed's clause (MD81b, `handling::untamed::stuck`) is "grounded clear →
not free, stuck, the clock runs by the surface time".
- **Real table.** jump.csv and throttle.csv show stuck 1 / free 0 on every row where no wheel's +0xd0 is set
  (in the air, tipped), and stuck 0 / free 1 when one is. The time stays 0, since the table's time is 0.
  SLUS-21104 would read free 1 all through.
- **hazard-time** (hazard 0, time 1):
  - in the air (rows 150–180) the time grows by 0.03333 a tick: 0x3d08850a, then 0.0666, … 0.9999
    (0x3f7ff968) after 30 adds;
  - it is zeroed on the landing row when a wheel's +0xd0 comes back (rows 133, 184);
  - on the 31st tick in the air (> 1) the quad is **put back**: row 181 is at (282.32, 6.69), the time
    restarts at 0.0333, and it keeps counting while the put-back quad drops.
  With the hazard word 0, SLUS-21104's clock would only ever take the free reset here.
- **hazard-on** (hazard 1, time 1): `on` holds on the ground, so the clock counts from the first tick and
  puts the quad back to the spawn spot (395.64, 1.30) about every 30 ticks. After each drop the rears read
  +0xd0 = 0 (and the fronts' touch +0xdc = 1) for 2–3 ticks while it counts. That is the "on a hazard
  surface with a wheel off the ground" case.

**Wheel words.** +0xd0 (grounded) and +0xdc (touch) differ on a tipped quad: wheels touch with +0xd0 clear
(throttle rows 150–230: left pair "touch, not grounded").

**Stick offsets.** The stick's x, y and flicks moved −0x10 (+0x1298). Untamed then keeps the stick's angle
and length at +0x12a8/+0x12ac (π and 1 for full left). The input limiter's ramped stick is +0x12f8 (+0x10):
it ramps toward ±1 as SLUS-21104's +0x12e8 does (C28's race-steer.csv: −0.267, −0.533, −0.8, −1 every 2
rows). A tape stick of 0.5 reads 0.28.

## How it was proved

- **Captures.** 7 save-state PINE recordings at research/md147/quad.toml, read with research/md147/summary.py.
  `research/md147/dups.py` reports every vsync step as 2.
- **Repeat run.** throttle against throttle-2 (`python3 -I research/md147/rows.py throttle throttle-2`):
  63 of 370 rows differ.
  - 41 are one run holding the other's next or previous row (a late read).
  - 22 match neither row: torn reads, where the columns come from two ticks.
  - The rides re-match after each one, so it is one ride. MD160 saw the same (49 of 310); MD164 saw 0.
- **Shared prefixes** (`same.py`): jump/steer are equal to row 50 and part on row 51; throttle/jump part on
  row 56. Each pair parts where its input changes (a tape tick lands on row tick + 2).
- **land.csv** from air.p2s equals jump.csv from row 155 on **through the landing**: 132 of 135 rows.
  - Row 29 is a late read.
  - Rows 37 and 43 are torn, and the row after each equals jump again.
  - Unlike MD164's baja, the quad's state load replays the landing.
- **Hazard table.** Read from each state with research/md147/surfaces.py, and its use read in the quad
  update's disassembly.
- Nothing was ported, so no oracle or replay ran.

## Not verified / not 1:1

- **No real hazard surface.** Rocksbury's table is all zero, so both hazard rides use a poked table: every
  surface (hazard 0 or 1, time 1). The clock's code path is the original's, but the values are mine. I did
  not find which tracks or setups fill the table (the copy's source, `FUN_004c9cf0`'s argument +0x380, was
  not traced), and I tried no other track. The −1 time (immediate put-back) was not driven.
- **Torn and late rows.** 63 of 370 throttle rows differ run to run because of read phase.
  - The stuck time sometimes reads a tick behind the position: hazard-time rows 150/151 and 179/180 hold the
    same time.
  - So the put-back row is 180 in the first hazard-time run (overwritten by the re-record) and 181 in the
    kept one.
  - A replay should compare settled rows, or use a lower NominalScalar.
- **Offsets by rule, checked by value.** These are not read in each Untamed function: ground +0x354 (0
  exactly when no wheel is grounded), wreck word +0x708 (1 from the rider's fall on), euler +0x390 (pitch =
  2·quat x), speed +0xc28 (= body speed).
  - The stick (+0x1298) and ramped stick (+0x12f8) were placed by their values.
  - The other 0x12xx words were not mapped.
- **The engine block is raw only.** MD125's car engine offsets (gear +0xc0, count +0xd0) read 0 on the quad.
  The quad's engine layout was not decoded, so there is no gear or rpm column.
- **The menu's ATV = race quad** rests on the `.phy` name in the object. Physics Type PRO
  (atv_Pro250P) was not captured.
- **The idle second quad** is in the states but was not recorded.
- **NominalScalar 0.3.** The copy ran slow for the recorder. The game's ticks are unchanged (2 vsyncs a row).
- **Not ported.** `quad::Tail::untamed` is wired to no ride, and these captures are not yet a replay fixture.

## Next

Seed the port's quad from spawn.ee.bin with `Tail::untamed` and replay steer.csv. Then replay hazard-time.csv
rows 140–190 for the put-back on the 31st air tick (filed as MD167).
