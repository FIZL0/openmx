# 1 — untamed monster capture (FINAL)

Task MD160. 2026-11-14.

## What I did

- **Setup.** A scratch KIT_CONFIG for SLUS-21701: the slot's kit.toml with name "MX vs. ATV Untamed", serial
  SLUS-21701, the Untamed iso, vsync 0x410318, elf SLUS_217.01, context context/untamed, and an `[[overlay]]`
  GAME_REL.BIN at 0x48ea80. PCSX2 copy 3 (`KIT_PCSX2=3`), PINE slot 28014, ini NominalScalar 0.3. Every
  command exports `KIT_CONFIG` and `TMPDIR` to the scratch folder.
- **Menus by tape.** I started from copy 9's Untamed state 5 (track list). Each menu step is a from-state
  tape (research/c15/tape.py with `--game "MX vs. ATV Untamed"`) played over AT-SPI with the C6 playback
  scripts, then saved by `research/md160/step.sh`. The pads are menu1–4.pad: Cross on Rocksbury; down,
  right (Vehicle = ATV); right (MONSTER TRUCK); up, Cross and a 1500-tick load.
  Copy 3's slots: 11 track list, 12 Player 1 Options, 13 Vehicle = ATV, 14 Vehicle = MONSTER TRUCK,
  15 **spawn** (vsync 27631), 16 **in the air** (vsync 27911 = jump.csv row 139, pos (258.61, 14.61,
  221.92)), 17 slot 16 with body rates poked to (6, 0, 0) (`flip.sh`, saved at vsync 27931).
- **Recorder.** research/md160/truck.toml is md39a/car.toml moved to Untamed's car. The 0xb..0x12-block
  offsets are −0x10 (state pointer +0xbb4, engine +0x1240, tires +0x1244..+0x1250). The body pointer and
  the 0x3xx words are +0x30 (body +0xe8, record +0xf8, controls +0x338: brake +0x340, gas +0x344). It has
  decoded body columns and raw blocks of the car (0x1800 B), body, record, engine and the four tires.
  `play.sh` plays a pad from a state. It starts the capture on the state's own vsync and checks that row 0
  is the truck (vtable 0x003f1840).
- **Drives** (all in context/capture/md160, 2 vsyncs per row, no gaps):

  | capture | rows | from | what happens |
  |---|---|---|---|
  | throttle, throttle-2 | 310 | spawn | gas to ~76 ft/s; a collider hit at row 162 (76→39 ft/s, vy +10.6); a wall at x≈499; damping stays 1 |
  | steer | 280 | spawn | gas, then right at row 90 (yaw rate ≈ +1.3), left from ~130 (−1.1..−1.5), half right; a wall at x≈495 at row 160 |
  | jump | 370 | spawn | left turn onto a jump: on the face rows 114–132, all four tires off rows ~138–150, y up to 14.8; hard stop at row 168 at (202, 210.9), then stuck there on the gas |
  | roll | 425 | spawn | jump.pad plus left steer in the air: two jumps (air ~138–156, ~252–264); no wreck, the truck lands upright |
  | wreck, wreck-2 | 415 | slot 17 | the poked roll tips the truck: car state 14 (tipped) from row 21 with damping 0.999; put back by row 85 (damping 1, state 0); then at rest |

- **Kept.** ~/repos/openmx-slots/kept-context/md160 holds:
  - states: options, vehicle-atv, vehicle-truck, spawn, air and flipped .p2s;
  - RAM images: spawn.ee.bin and air.ee.bin;
  - capture/: every csv and its json;
  - recordings/: every tape and its ticks.csv.
  The states are also in context/untamed/md160. `research/md160/keep.py` makes the copy again.

## What the original does

**Heap addresses (spawn state).** These are retail heap addresses, kept here and not in code.

| object | player's truck | idle second truck |
|---|---|---|
| truck | 0x01be47b0 | 0x01c3fc30 |
| body | 0x01c26300 | 0x01ca8440 |
| collision record | 0x01c2a860 | 0x01caca20 |
| engine | 0x01c3d1d0 | 0x01cae360 |
| tires | 0x1c3d320, 0x1c3dcf0, 0x1c3e6c0, 0x1c3f090 | from 0x1cae4b0 |

- The heap addresses are the same in the air state. Spawn is at (395.62, 1.74, 183.37), heading +z.
- The car's state is at +0xc70 (0 riding, 13 off the edge, 14 tipped), with its clock at +0xc74 and
  MaxWreckTime 5 at +0xc78.
- Also on the car: WreckDamping (0.999 ×3) at +0xc90, WreckInertia 0.2 at +0xca0, Inertia
  (4557.86, 4892.17, 3472.46) at +0xcb0, kind 5 at +0xbac, wrecks_on 1 at +0x110c.
- Body fields: linear damping +0x140, angular damping +0x150, quat +0x170, rates +0x1d0, rate clamp 12 at
  +0x210. `python3 -I research/md160/ram.py STATE.p2s` lists all of it.

**Nothing in normal driving lowers the angular damping.** The truck's own collision record has damp flag
+0x44 = 0 and rest/contact damping 1. So the contact pass's collider damping never applies to it, and the
damping stayed 1 on every row of throttle, steer, jump and roll. The route that lowers it is the car's wreck
(atv_car.rs `wreck`): for kind ≠ 0/1 it sets the body's linear and angular damping to WreckDamping = 0.999.
The un-wreck sets them back to 1.

**Live powf damping.** wreck.csv rows 23–44 are in the air, wrecked, with no contacts. Over them the rates
decay by exactly ×0.85219 per tick on all three lanes. That is 0.999^159.84 = (0.999^39.96)^4:
`powf(0.999, 0.333/h)` at h = 1/120 with 4 substeps per tick. SLUS-21104's plain `d` would give 0.996 per
tick. The stored lanes stay 0.999, so the powf result is a temporary. Rows 19–20 (damping 1, in the air)
keep the rates constant, so nothing else decays them. This is the MD81c finding ("powf(d, 0.333/h) at the
substep h"), now seen in the running game.

## How it was proved

- A save-state capture: 7 PINE recordings at research/md160/truck.toml, read with research/md160/summary.py
  and decay.py (`python3 -I research/md160/decay.py wreck 19 44` shows the ratio).
- Repeat runs (`python3 -I research/md160/rows.py A B`):
  - throttle against throttle-2: 49 of 310 rows differ. 36 of them are one capture holding the other's
    previous or next row, a late read (the recorder misses rows under load). Both runs come back to the same
    numbers after each one (for example rows 28–34). The other 13 rows are torn the same way but match
    neither neighbour.
  - wreck against wreck-2: 6 rows differ (21, 66, 70, 82, 160, 317), all on event rows (the wreck spin, the
    un-wreck). The rows around them are equal.
- Nothing was ported, so no replay or oracle was run.

## Not verified / not 1:1

- **The wreck is a PINE poke, not a natural wreck.** Steering hard over the jump (roll.csv) did not tip the
  truck, so the rates (6, 0, 0) were written into the air state as C28's throw.py wrote velocity. The rows
  after the poke are the game's own.
- **No collider lowers the truck's damping.** The task asked for "something lowers the angular damping". The
  only thing found is the wreck. Another surface's collision record with damp flag set (a `.phy` or world
  collider) was not searched for.
- **Run-to-run rows.** The differing rows read as late or torn reads, as MD92 saw. But 13 throttle rows and
  6 wreck rows were not shown, field by field, to be mid-tick reads. The raw car blocks also differ at
  obj +0x75 and obj +0xb30, which were not decoded.
- **Offsets by rule, not by code.** The tire, engine and state-pointer offsets come from SLUS-21104's −0x10.
  They were checked only by their values making sense (tire touch and compression, engine pointer, the car's
  state stepping 0→14→0), not by reading Untamed's functions. The record at +0xf8 was checked by record
  +0xe4 = the body.
- **4 substeps** is inferred from the decay (it fits only n = 4 with h = 1/120). It was not read from the
  code.
- **NominalScalar 0.3**: the copy runs slow so the recorder keeps up. The game's ticks are unchanged, but the
  vsync count per tick was not checked at 1.0.
- **jump.csv's end** is the truck stuck against a collider with the gas held. It is kept, but there is
  nothing to learn after row 168.
- **The idle second truck** (0x01c3fc30) is in the states but was not recorded.
- **Not ported.** The four MD81c live changes behind an Untamed switch, and an openmx-mods pack, are still
  follow-ups.

## Next

Port MD81c's four changes (input guards, clamp form, preload cue, powf body damping) behind an Untamed switch.
Then replay wreck.csv rows 21–85 from flipped.p2s's RAM image as the first fixture; its decay is the
damping's test.
