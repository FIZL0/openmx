# 1 — four flights captured, and what the fields are (FINAL)

Task MD80a. 2026-11-03.

## What I did

- **Set up the copy.** I used PCSX2 copy 1 (s1-slots, PINE 28012) with a scratch kit.toml. It is the slot's
  own kit.toml with serial SLUS-21104, the MX vs. ATV iso from ~/repos/openmx-mods/iso, vsync 0x3fa6d8 and
  elf SLUS_211.04 (memory mxvatv-runs-in-your-own-copy). The kit on this branch calls
  `agent-runner home slots`, but the installed agent-runner (0.12.0) does not have it. A shim on PATH
  answered it with ~/repos/openmx-slots/s1-slots. I filed a T follow-up for it.
- **Found the map.** Desert Oasis Free Ride places both fliers.
  - The start was MD39a's menu tape, which reaches Desert Oasis's Player 1 Options. I kept that state as
    `context/mxvatv-ps2/md80a/options.p2s` (title.p2s is the title screen).
  - In Player 1 Options, Vehicle right ×2 picks the plane and ×3 the helicopter.
  - For model two, Customize Vehicle → MODEL right ×2 → Accept.
  - `research/md80a/state.sh` does all of this, saves the state at the spawn, and lists the fliers in the
    image (`ram.py`).
- **Found the objects** by their vtable word at +4 (oracle # MD80). Each state holds two fliers: the player's,
  and an idle twin parked at (2815–2818, ~126, 3303–3304) with zero velocity. The twin's model word is 0
  even on the model-one maps. The player's flier is the one with a velocity:

  | state | vsync | player object | model word | other (idle) |
  |---|---|---|---|---|
  | biplane | 27579 | 0x014de810 | 1 (+0x1320) | 0x0150c9d0 |
  | biplane3 | 29999 | 0x01b13790 | 0 | 0x01b41550 |
  | kiowa | 27759 | 0x01ab7d20 | 1 (+0x1340) | 0x01b20740 |
  | kiowa3 | 30181 | 0x014c8f00 | 0 | 0x01537000 |

- **Flew each from a generated tape.** The pads are `research/md80a/{biplane,biplane3,kiowa,kiowa3}.pad`,
  built into a tape by research/c15/tape.py and played over AT-SPI.
  - `research/md80a/fly.sh` plays one and records `research/md80a/{biplane,kiowa}.toml` every tick.
  - Rows start on the state's own vsync once the vtable word is in.
  - It took 11 tries (biplane), 3 (biplane3), 4 (kiowa) and 4 (kiowa3) to get flights with no wreck that
    end at rest. The tries are kept beside the finals in context/capture/md80a (try*, b3try*, ktry*,
    k3try*, probes); only the four finals are meant to be read.

## The captures

`context/capture/md80a/NAME.csv`, one row a tick (vsync +2), with the tapes in `context/recordings/md80a`.
Rows lag the tape by about 2 ticks.

| flight | rows | vsync | lift-off | top | speed max | pitch | bank | touchdown | end |
|---|---|---|---|---|---|---|---|---|---|
| biplane | 1625 | 27581..30829 | row 118 | y 671 (row 677) | 138.8 ft/s | −34..58° | ±49° | row 1258, −20 ft/s | (5479, 139, 1124), 1.5 ft/s |
| biplane3 | 2338 | 30001..34675 | row 111 | y 783 (row 628) | 114.0 | −22..76° | −23..61° | row ~2060, −7.9 ft/s, one bounce | (1420, 151, 591), 1.7 ft/s |
| kiowa | 1265 | 27761..30289 | row 23 | y 436 (row 439) | 76.9 | −30..20° | −45..39° | row 1088, −16 ft/s | (3766, 142, 3477), 0.7 ft/s |
| kiowa3 | 1380 | 30183..32941 | row 24 | y 435 (row 442) | 75.9 | −30..21° | −47..44° | row 1186, −10 ft/s | (3780, 134, 3393), 0.0 ft/s |

The wreck word is 0 on every row of all four.

The phases, by tape tick (a row is about the tick minus 2):

- **Biplane and biplane3.**
  - Cross (throttle up) from tick 15, held until 776. R1 rudder 15–55, on the take-off roll.
  - Stick back 105–150 to climb, then a forward check 165–185.
  - Banks: right 335–350, left 410–440, right 500–515.
  - Rudder L1 545–565, then R1 565–585, then a right bank 595–607.
  - Dive: stick forward 647–677, pulled out at 767–776.
  - Square (throttle down) 776–866, then a glide down.
  - Biplane: Square from 1260 to 1560 brakes it on the ground.
  - Biplane3 has a longer glide. It noses down at 1654–1660, flares at 1917–1923, and brakes with Square
    from 1973 to 2273.
- **Kiowa and kiowa3.**
  - Cross (climb) 15–225, with the stick forward from 165 to 315.
  - Banks: right 345–375, left 415–445.
  - Pedals: L1 485–525, R1 545–585.
  - Dive: stick forward with Square, 605–665/720.
  - Descent: staged Square pulses (810–855, then 915–940 in the kiowa), stick-back braking (970–1010 in the
    kiowa, 915–955 and 1100–1115 in kiowa3), and a last Square to the ground.

**Repeats.** I played each final pad again and compared the two csvs with research/c15/same.py.

- The Kiowa's two flights repeat exactly.
- The biplane's two each have one torn row. In biplane.csv it is row 2, where every block equals row 1's
  (check.py lists it). In biplane3 it is at vsync 32161, and only one of the two runs has it. A torn row
  repeats the tick before (memory frame-counter-trigger-tears-captures), and the row after carries two ticks.
  Skip torn rows, don't step from them.

**Images.** `context/mxvatv-ps2/md80a/NAME.p2s` is each flight's start state. `ram.py NAME` writes
`NAME.ee.bin`, the 32 MB EE RAM image the replays seed from.

**Checked by** `python3 -I research/md80a/check.py`, which prints `ok`. For each flight it checks:

- the image has the player's object, with its vtable and the expected model word;
- the row counts and the first vsyncs are as above;
- every row has the vtable and vsync +2;
- the wreck word is 0 at the end;
- on every row, the decoded pos/vel/quat equal the raw body block's words;
- body +0x1d0 matches the quaternion's derivative (mean relative error 0.034/0.022/0.023/0.019);
- there are at most two stale rows.

I made three mutations: model word 1 → 0, 1625 → 1624 rows, and +0x1d0 → +0x1c0. Each one failed.

## The controls

- **Biplane.**
  - Cross (command 0x14, +0x314) raises the throttle (+0x318) at about 0.35/s (0.163 at row 30, 0.513 at row 60), up to 1.
  - Square (0x15, +0x310) lowers it, down to −0.0117 (a reverse throttle), and brakes on the ground.
  - The stick goes through a big dead zone and a cubic into the elevator and aileron commands
    (+0x10f4/+0x10f8).
  - L1 sets the rudder command (+0x10fc) to −1 and R1 to +1. The rudder (+0x1114) and the nose steer
    (+0x1110) ramp toward it at 1/s.
- **Kiowa.**

  | input | word | effect |
  |---|---|---|
  | Cross | +0x10f0 = 1 | climbs to ~38.8 ft/s |
  | Square | +0x10f4 = 1 | descends, speeding up toward −61 ft/s |
  | stick x | +0x10fc = −x | banks |
  | stick y | +0x1100 = −y | forward pitches the nose −30° and flies forward |
  | L1 | +0x1104 = +1 | pedal |
  | R1 | +0x1104 = −1 | pedal |

  - With no input it holds its height (vy decays to 0; the held height is +0x1138) and keeps its speed.
  - The collective (+0x1108) is 1 climbing and ~0.32–0.34 holding.

## The fields

research/oracle.txt `# MD80a` has the map. Each name is marked as checked against the numbers ("value") or
taken from MX Unleashed's name moved down 0xc as # MD80 found ("shift"). The main ones:

- **The body.** It is at *(+0xb8): +0x60 position, +0x80 velocity, +0x170 quaternion, and **+0x1d0 the
  angular velocity in the body frame**.
  - The spec I recorded with read `angvel` from +0x1c0, which is wrong. So the four csvs' `angvel.*` columns
    are body +0x1c0, a large world-frame vector that I did not decode.
  - The rate is in each row's raw `body` block at +0x1d0. research/md80a/rate.py found it, and check.py
    checks it.
  - I fixed the specs for new captures (`body_1c0`, `rate`). I left the csvs as captured: their .json
    carries the spec they were taken with.
  - MX Unleashed's body has velocity at +0x70 and a world-frame rate at +0x1b0. SLUS-21104's body is laid
    out differently.
- **The object.** +0x32c is |velocity|, +0x340..+0x348 the angular velocity in the world frame, and +0x358
  the forward component of the local velocity. +0x330/+0x334 are small angles, likely alpha/beta; their
  definitions are not checked.
- **The biplane's parts.** The engine (0x80 bytes), the aero (0x110) and the three tires (0x380 apart) sit
  back to back. So each 512-byte raw block runs into the next part:
  - the `engine` block's +0x80.. is the aero;
  - the `aero` block's +0x110.. is tire 0 (its +0x1d0 is tire 0's +0xc0).
- **The biplane's engine.** These were checked on every row of both biplane captures:
  - +0x50 power = 250 × throttle (within 1.5e-5);
  - +0x54 rpm = 500 + 2000 × max(throttle, 0) (within 2.4e-4);
  - +0x68 a clock, +1/30 a tick.
  - +0x3c is the prop angle, +0x60 the thrust and +0x64 the sound's rpm.
- **The biplane's aero.** CL..CM are at +0xdc..+0xec (shift).
- **A biplane tire.**
  - +0xc0..+0xc8 is its ground ray's hit.
  - +0xf0..+0x108 holds the gear points in the world. +0x104 − +0xc4 is the height of the gear above the
    ground, which `gnd.py` prints.
  - +0x8c..+0x94 are nonzero only on the ground.
- **The Kiowa's rotor** (0x150 bytes, then the aero).
  - +0xc0 is the blade pitch: 0.2618, which is 15°, climbing.
  - +0xd0/+0xd8 are the tilt.
  - +0xf4 is the thrust, and +0x100..+0x108 the thrust along the tilted axis (+0x104 = +0xf4 when it
    hovers level).
- **The Kiowa's aero.** +0x128 is a 0..1 fade with speed, and +0x134/+0x140 sit at −0.5236/0.0785 on the
  ground.
- **The wreck word.** +0xc80 is 13 when the biplane hits a hard object and 1 for a hard landing or a tip-over
  (biplane3 landing nose-down, the Kiowa hitting the ground at −61 ft/s, kiowa3 sitting nose-down). The
  engine/rotor and aero skip while it is set. The put-back takes ~150 rows.

## Things I learned

- The biplane wrecks near (5607, ~1110) when flying into an object: try rows at the same x,z hit at
  different heights. The ground does not cause it. Land and brake before it.
- After landing, the Kiowa rests pitched −25° to −29°. Its +0x10f8 reads 0.23 in the air and ~0.25 on the
  ground.
- The Kiowa wrecks (1) if it touches down fast and sideways. Brake in the hover before descending.
- Ground near the spawn is y ~121–124. Along the tracks it is ~130–150.

## Not verified

- Nothing was replayed. The captures only say what the original did. MD80b/MD80c will show whether the
  fields are enough to step from.
- The "shift" names are by analogy only.
- Several words are named by what they track but not derived: the aero coefficients, the Kiowa's rotor tilt
  and its aero words, and alpha/beta.
- The csvs' `angvel` columns are mislabelled (above).
