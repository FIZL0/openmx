# 1 — car captures (FINAL)

Task S149a. 2026-10-29.

## What I did

- **car_phy for all three.** `crates/game-sim/tests/car_phy.rs` proves the loader against a parked car of
  each class: the truck in first_recording's RAM image (as before), the baja in `s149a-free07.ee.bin`
  (0x00605270, kind 5) and the buggy in `s149a-free11.ee.bin` (0x00611ba0, kind 3). Both images come from
  this folder's `boot.py` (S148's scripted boot, with the event's machine kind as its 5th argument):
  `boot.py free07 2 5 <slot> 5` and `boot.py free11 2 5 <slot> 3`, then `./kit state ram <slot> --out`.
  The buggy and the baja share a class (vtable 0x0045cf80) whose driver pelvis box sits at car +0xf20/+0xf30
  (the truck's at +0x1010/+0x1020) and which has no RearTireSteerScale. With `--nocapture` the test prints
  the words it checks: baja 1272, truck 1269, buggy 1273, all bit-exact.
- **Booting a car.** The first try copied first_recording's event (+0x11c = 4, +0x120 = 1: machine_placement
  0x0020f6e0 takes its Machine%d/MachineRacer branch). That parks the car. But the load hung while ALL_AI
  was zeroed, and after a pause-menu switch the picture stayed grey. S148's event (+0x11c = machine kind,
  +0x116 = 0, +0x120 = 0, +0x128 = 6, +0x124 = 0) starts the human on the bike with the track's
  Baja%d/Buggy%d/Monster%d machine parked. That is the recipe now.
- **Switching** (`switch.py SLOT OUT SHOT`): load, set the cheat flag (read32(0x004e2778)+0x5c = 1, or
  "Switch To Machine" is greyed), Start, down twice, Cross. The controls card comes up and the shot is taken.
  Cross accepts the card, then the script saves. Each car's card reads Cross accelerate, Square brake, L1 clutch,
  Circle hand brake, left stick steer. The baja is called "Trophy Truck". `./kit pad send` returns before
  its sleeps play, so `pad()` now waits them out. Without that the save came before the accepting press, and
  the tape's first gas closed the card instead.
- **Where they were driven: free07, all three.** The free11 buggy is parked in front of a garage. Gas from
  rest drove it 57 ft into the back wall, where it stayed for the rest of the tape. Getting it out by pad
  hit a post or the building on every try. The free09 truck hit something within 85 ft and spun. free07's
  boot parks whatever kind the event asks for at 0x00605270, at its start line facing open ground, so the
  buggy (kind 3) and the truck (kind 4) were booted there too. The loader proofs above still use the parked
  car of each car's own track.
- **The tape** (`make_tape.py`, 660 ticks, two fields a tick): stand, gas from rest, gas + full left, gas +
  full right, gas, Square brake (to a stop; holding it at rest latches reverse), gas, gas + Circle + left,
  gas, gas + L1, gas (clutch drop), gas + stick back then forward, coast. The schedule is in its docstring.
  Full lock at ~80 mph rolled the truck (kept as `truck-roll.csv`), so the truck's main tape steers half
  the stick (`prep.py 8 truck 0x40`).
- **Captures** (`capture.sh`, the spec `watch/car.toml`, C7's AT-SPI playback): one row a game tick, the
  pad stamp as the recorder's clock. In `context/capture/s149a/` of the s1 worktree:

  | csv | car | rows (frame_counter) | gaps | notes |
  |---|---|---|---|---|
  | baja.csv | baja, kind 5 | 650 (401..1050) | one torn row: 813 missing, 814 twice (row 412) | 31 ticks all four tires off, longest 18 |
  | buggy.csv | buggy, kind 3 | 650 (467..1116) | none | 5 ticks all four off |
  | truck.csv | truck, kind 4, half steer | 650 (459..1108) | none | all four tires off for 28, 52 and 20 ticks; one tumble back onto its wheels (~tick 250) |
  | truck-roll.csv | truck, full steer | 650 (459..1108) | none | rolled ~tick 190, on its side 273..472, set upright at ~476 |

  RAM images at the switch (card accepted, the human in the car), in `context/states/`:
  `s149a-baja-switched.ee.bin` (frame_counter 400), `s149a-buggy-switched.ee.bin` (466) and
  `s149a-truck-switched.ee.bin` (458). The car is at 0x00605270 in all three. Each tape's state is beside
  it as `context/s149a/<car>.p2m2_SaveState.p2s`.
  Per row: frame_counter, dt (1/30), rng, the pad, the car's vtable/kind, the body (+0x160: pos +0x60,
  vel +0x70, quat +0x170, world rate +0x1b0), and raw hex blocks: car +0x200..+0x700, the engine (car
  +0x544 →) +0..+0x140, each tire (car +0x548/+0x54c/+0x550/+0x554 →) +0..+0x240.

## What the original does

Named from the captures. Offsets are into the tire object, the engine object and the car:

- **Tire.** +0xb4 and +0xc0 = 1 while touching the ground. +0xdc (and +0xe0) is the compression in ft
  (0 in the air, ~1.1 max on the baja, 3.9 on the truck). +0xe4 is the normal force. +0x1f8 is the radius
  (baja 1.7, buggy 1.2, truck 2.8). +0x1a4 is the wheel's spin in rad/s and +0x1a0 its angle (−π..π).
  +0x220/+0x224/+0x228 is the contact-point velocity in the tire's frame (lateral, normal, longitudinal).
  +0x20c is the slip ratio, (v_long − ω·r) / |v_long|. It is exact on most ticks, the denominator is
  floored at low speed, and it is −0.98 in the air. +0x210 is the slip angle atan2(v_lat, |v_long|), within
  ~0.002 rad on most ticks. +0x214 tracks it a little behind. +0x110..+0x168 are world contact points and +0x120.. the
  ground normal. The tires carry no steer of their own.
- **Steer** is the car's: car +0x4b8 is the stick after a dead zone (0x3f → −0.294), +0x4c0 is that
  squared with its sign (−0.0865, slewed to it), and +0x400 is the steer angle, cut with speed (full stick:
  −0.94 at a crawl, ~0.55 at 100 mph on the baja).
- **Engine.** +0xcc is the gear. +0xe4 is that gear's ratio and +0xe8 its inverse: baja 30/15/8.5/5.5,
  buggy 27/12.6/9.45/7.2/5.4, truck 25/16.5/11/7.25. +0xec is rpm, held at 7000 by the limiter.
  +0x128/+0x138 follow it (a target and a smoothed rpm). +0x108 is the throttle actually applied: ~0.73
  at idle with no gas, 0 on the limiter. +0xf8 is the torque converter's multiplication: 2.5 at stall,
  falling to 1.0 as it locks. +0xf4/+0xfc look like torques. +0x104 is a 1..7 state. +0xac is a shift
  timer (0.733 at a shift, counting down by dt) with flag +0xb0. +0x6c is a launch boost: 1.56 → 1 over
  ~1 s from rest, and back to 1.44 when the clutch drops (buggy up to 4.26). +0x134 counts up by dt.

## How it was proved

- The loaders: `cargo test -p game-sim --test car_phy` (gate: `./kit check`). The three parked cars match
  word for word (1272 / 1269 / 1273 words).
- The captures: PINE recordings of a generated .p2m2 played on the original. The kit's recorder checks the
  frame counter. `check.py NAME` (this folder) gave consecutive frame_counter values with no gaps except the baja's
  one torn row, the expected car_kind and vtable on every row, and no empty cells. The body moves with
  the tape.
- The field names above: read off the curves against the tape and the body, and checked against
  identities with `check.py NAME` in this folder. Spin × radius equals the longitudinal speed while rolling.
  On grounded tire-ticks above 1 ft/s the slip-ratio formula holds within 2e-3 on 87-95% of them (baja
  2107/2213, buggy 2080/2355, truck 1509/1725) and the slip-angle formula on 67-79%. The rest are not sorted
  out. The names are not proved against the code.

## Not verified / not 1:1

- **The buggy and the truck were driven on free07, not on their own tracks.** Their parking spots on
  free11 and free09 are boxed in. The car's physics comes from its .phy and was not seen to depend on
  the track, but the ground (surface table, bumps) is free07's.
- **The baja capture has one torn row** (row 412: frame_counter 814 twice, 813 missing). The recorder
  reads ~23 rows/s against 30 Hz. A later truck run under load ~30 lost 48 ticks and was thrown away.
- **The truck's main capture steers half the stick**, which the squared response makes 9% steer. Its full-lock
  run rolled over, so the truck has no hard turn on its wheels.
- **The truck lies still from ~tick 300 to ~420 of truck-roll.csv**, and the set-upright at ~476 is the
  game's reset, not physics.
- **No "jump" off a ramp on the baja or the buggy.** free07's start road has bumps that left all four tires
  off for up to 18 ticks (baja) and 4 (buggy). The stick back/forward leg gave no hop. The truck jumped
  three times (28, 52 and 20 ticks).
- **The field names are from curve-reading, not from the code.** The tire and engine steps are vtable-only,
  and S149b/S149c are to read them. These are unsure: +0xf4/+0xfc torques, +0x104's states, +0xac/+0xc4
  timers, +0x128/+0x138, the dead zone's width, and the low-speed floor on the slip ratio's denominator (one
  sample fits max(|v|, 30)). The 5-13% of slip ratios and 21-33% of slip angles off their formula are
  not explained.
- **The handbrake leg is a left turn on the gas.** Whether Circle locks the rears is not looked at.
- **The first boot recipe's grey switch** (event +0x120 = 1) was not looked into.

## Next

S149b-S149d read these rows: the raw blocks hold the rest of each object. Tire and engine field names to
start from are above. `watch/car.toml` and `capture.sh` record another drive in about 30 s once a state
exists (boot.py → switch.py → prep.py → capture.sh, each named in its docstring).
