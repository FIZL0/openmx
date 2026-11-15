# 1 — untamed baja capture (FINAL)

Task MD164. 2026-11-14.

## What I did

- **Setup.** A scratch KIT_CONFIG for SLUS-21701, built as MD160's: the slot's kit.toml with name "MX vs. ATV
  Untamed", serial SLUS-21701, the Untamed iso, vsync 0x410318, elf SLUS_217.01, context context/untamed and
  an `[[overlay]]` GAME_REL.BIN at 0x48ea80. PCSX2 copy 4 (`KIT_PCSX2=4`), ini NominalScalar 0.3 for the
  captures, put back to 1.0 after. Kept as kept-context/md164/slus21701-kit.toml.
- **Menus by tape.** Copy 9 no longer exists (pcsx2/s9 is gone), so I started from MD160's kept
  vehicle-truck.p2s (Player 1 Options, Vehicle = MONSTER TRUCK). Each step is a from-state tape played over
  AT-SPI and saved by MD160's own `research/md160/step.sh`, unchanged:
  - research/md164/menu1.pad: right → **TROPHY TRUCK** (the menu's name for the baja; checked below).
  - research/md164/menu2.pad (MD160's menu4.pad): up to Start Event, Cross, the load → **spawn**.
  - research/md164/air.pad (MD160's air.pad): jump.pad cut at tick 140 → **in the air**.
- **Recorder.** research/md164/baja.toml is MD160's truck.toml (Untamed's car offsets) plus decoded columns
  for the preload's ready/active/armed (+0xc40), its time/timer/min/max (+0xc4c), the cue latch (+0x724),
  the lean (+0x11bc), the pad word (+0x1114) and the car state (+0xc70). research/md164/play.sh is MD160's
  play.sh with the baja's vtable (0x003f09d0) in the start trigger and the row-0 check.
- **Drives** (context/capture/md164, all from spawn.p2s unless said; every one 2 vsyncs a row, no gaps):

  | capture | rows | what happens |
  |---|---|---|
  | throttle, throttle-2 | 370 | gas straight: gears 1→2 (row ~48) →3 (row ~108), 88 ft/s; a wall at z≈378 at row ~135 stops it nose-up |
  | steer | 280 | gas, then right from row 90 (yaw rate ≈ +1.7), left (−1.8), half right; hits a wall at x≈500 at row ~158 and **tips over by itself**: car state 14 rows 209–269 (damping 0.999), put back at row 270 |
  | brake | 340 | gas to 73 ft/s, then Square held from row 97: to rest by row ~205 at z≈374 (no reverse while held), then gas + a Circle hand-brake pull into the wall |
  | jump | 370 | MD160's jump.pad: left onto the supercross track, up a face (rows 119–126, preload ready), in the air rows 137–154, **4th gear** (the top: the baja has 4) at row 155; hard stop at row ~175 |
  | preload | 370 | jump.pad with the stick down from tick 110 and up at tick 121: the preload arms on row 119 and **fires on row 123** |
  | land | 135 | from air.p2s, gas held through the landing |

- **Kept.** kept-context/md164: vehicle-trophy, spawn and air .p2s; spawn.ee.bin and air.ee.bin; capture/
  (every csv and json); recordings/ (every tape and ticks.csv); addresses.txt (the heap addresses below);
  the scratch kit.toml. The states are also in context/untamed/md164. `research/md164/keep.py KIT.toml`
  makes the copy again.

## What the original does

**The trophy truck is the baja.** The spawned object's vtable 0x003f09d0 holds the buggy/baja class update
(0x004e46c0, MD81d's pair of SLUS-21104's buggy class update) at +0x10c, and its kind (+0xbac) is 6, the baja
truck's machine slot in untamed-machines.md (the monster truck's is 5).

**Heap addresses (spawn and air states, the same in both).** Retail heap addresses, kept here and in
kept-context/md164/addresses.txt, not in code.

| object | player's truck | idle second truck |
|---|---|---|
| truck | 0x01be4740 | 0x01c2e250 |
| body | 0x01c16d80 | 0x01c87100 |
| collision record | 0x01c19540 | 0x01c89950 |
| engine | 0x01c2b7f0 | 0x01c8abd0 |
| tires | 0x1c2b940, 0x1c2c310, 0x1c2cce0, 0x1c2d6b0 | from 0x1c8ad20 |

- Spawn at (395.66, −0.24, 155.89), heading +z, vsync 27809; 28 ft behind the monster truck's spawn.
- air.p2s: vsync 28089, body = jump.csv row 140: (265.80, 12.08, 230.85), velocity (−62.3, 5.7, 1.0).
- Car: state +0xc70 (0 riding, 14 tipped), MaxWreckTime 5 at +0xc78, WreckDamping 0.999 ×3 at +0xc90,
  WreckInertia 0.2, Inertia (1852.51, 3663.35, 1290.50) at +0xcb0, wrecks_on 1 at +0x110c.
- Engine (MD125's offsets): 4 gears, ratios 6, 3, 1.7, 1.1; final 5; redline 7000.
- Preload words all 0 at spawn: PreloadTime, min/max time, MaxPreload, force (`[Preload]` off).
- Collision record: damp flag +0x44 = 0, damping 1 ×4 (as the monster truck's).

**The preload with `[Preload]` off.** On preload.csv row 119 the truck turns ready (grounded, rising
>5 ft/s, faster than 30 ft/s, off the brake) with the lean at +1, and arms (+0xc48 = 1). On row 123 the lean
is −1: armed clears, and the timer reads −0.008333 (0xbc08850a = −1/120): it was set to PreloadTime 0, fired
active, and ran down one substep later, which cleared active. The push is 0 (min = max time = 0, MaxPreload
0). The body is untouched by it: preload.csv equals jump.csv on every decoded column through row 126; they
part at row 127, when the truck leaves the ground with the stick still up (the lean's air control). The
cue latch (+0x724) reads 0 on every row: set while active and cleared once the timer is ≤ 0, it rises and
falls inside one tick, so end-of-tick rows cannot show it. The pad word (+0x1114) jumps from 0 to −992.97
on the fire row and holds.

**A natural wreck.** steer.csv tips the truck against a wall: car state 14 from row 209, both damping
vectors 0.999, put back at row 270 (rates 0, damping 1). Unlike MD160's monster truck no poke was needed.
The truck touches the ground all through the wreck, so there is no clean in-air stretch to read the
`powf(d, 0.333/h)` decay from (MD160's wreck.csv has that).

**Ticks.** A tape's tick lands on row tick + 2 (steer parts from jump at row 57 for a tick-55 stick, brake
from throttle at row 97 for tick 95, the preload fire on row 123 for tick 121). The preload timer's −1/120
agrees with MD160's 4 substeps of h = 1/120.

## How it was proved

- Save-state capture: 8 PINE recordings at research/md164/baja.toml, read with research/md164/summary.py
  (gear, rpm, preload, lean, cue, car state per row) and decay.py.
- Repeat run: throttle against throttle-2 (`python3 -I research/md164/rows.py throttle throttle-2`, and with
  `obj0,obj1,body,record,engine`): **0 rows differ** on the decoded columns and on the raw car, body,
  record and engine blocks, over 370 rows.
- Shared prefixes (`python3 -I research/md164/same.py A B OFFSET`): jump/preload equal to row 126,
  jump/steer to row 56, throttle/brake to row 96: each pair parts on the row its input changes.
- land.csv (from air.p2s) equals jump.csv rows 140–154 exactly, then parts at its row 15, the first tire
  touch (see below).
- Nothing was ported, so no replay or oracle ran.

## Not verified / not 1:1

- **The cue post itself is not seen.** With PreloadTime 0 the cue latch sets and clears within one tick, so
  no row shows it. That the cue fired is inferred from the state words (armed cleared, timer −h) and the
  MD81d reading of the code, not observed. Seeing it needs a sub-tick read (a hook on the event post) or an
  audio capture.
- **land.csv parts from jump.csv at the first touch** (row 15 = jump row 155: speed 64.43 against 64.42).
  The air rows are equal, so the body loaded right; something the ground contact reads is not the same
  after a state load (a contact cache, or the saved controls the first tick after a load reads). Not found. A replay seeded from air.ee.bin
  should be checked against land.csv, not against jump.csv's tail.
- **Top speed in top gear.** throttle.csv reaches 3rd only (a wall at z≈378); 4th is reached only in
  jump.csv's air and landing. The stadium has no straight long enough from this spawn. Not tried
  elsewhere.
- **No reverse.** The brake held at rest did not reverse the truck (MD39a's card: Square brake); reverse
  (both pedals, per the controls code) was not driven.
- **The pad word's −992.97** on the fire row is unexplained (it is read as the preload's strength by the
  remap; with min = max time 0 it does not matter here).
- **Offsets by rule.** As MD160: the −0x10 / +0x30 offsets are checked by their values (gear stepping 1→4,
  the engine's ratios, tire touch, the car state 0→14→0, the preload words moving as the code says), not
  by reading every Untamed function. The preload words' offsets are MD81d's (oracle.txt), read in the code.
- **The trophy truck = baja** is shown by the class update in its vtable and the kind 6; the Baja.phy
  values (4 gears, the ratios) were not compared with the file.
- **NominalScalar 0.3**: the copy ran slow for the recorder; the game's ticks are unchanged (2 vsyncs a row
  throughout).
- **The idle second truck** is in the states but was not recorded.
- **Not ported.** MD81d's switches are not wired to an Untamed ride; these captures are not yet a replay
  fixture.

## Next

Seed the port's baja from spawn.ee.bin under `atv_car::Load::untamed` and replay throttle.csv (it is
run-to-run exact); then preload.csv rows 115–130 for the cue latch's state words.
