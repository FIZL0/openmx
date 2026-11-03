# 1 — mxvatv car captures (FINAL)

Task MD39a. 2026-11-03.

## What there is (all git-ignored, in s1)

| car | state + RAM (`context/mxvatv-ps2/md39a/`) | object | vtable | csv (`context/capture/md39a/`) | vsync |
|---|---|---|---|---|---|
| buggy | buggy.p2s, buggy.ee.bin | 0x01598e10 | 0x003cec60 | buggy.csv | 27959..29417 |
| baja (Trophy Truck) | baja.p2s, baja.ee.bin | 0x01598e10 | 0x003cec60 | baja.csv | 28315..29773 |
| sand rail | sandrail.p2s, sandrail.ee.bin | 0x01b158a0 | 0x003cec60 | sandrail.csv | 28675..30133 |
| golf cart | golf.p2s, golf.ee.bin | 0x01b0dac0 | 0x003cf600 | golf.csv | 28495..29953 |
| monster truck | truck.p2s, truck.ee.bin | 0x01598e10 | 0x003cff80 | truck.csv | 28133..29591 |

- `options.p2s`: Desert Oasis Free Ride, Player 1 Options, everything unlocked. Every car state is made
  from this one.
- The tape: `context/recordings/md39a/drive.p2m2` (+ `.ticks.csv`), made from research/md39a/drive.pad.
- Every csv has 730 rows, one a game tick (vsync +2), none missing. The columns are research/md39a/car.toml's:
  - decoded: vsync, vptr, ctl (+0x308), and the body's pos, vel, quat and angvel;
  - raw: the car object (`obj0`/`obj1`, +0x0..+0x1800), the body (640 bytes) and the engine (512 bytes);
  - per tire: touch (+0xa4), compression (+0xcc) and the raw tire (768 bytes).

Each class has two objects in RAM. The player's car is the one moving. The other sits idle at the spawn
with zero velocity.

## The drives

`python3 -I research/md39a/summary.py NAME 10` prints one. Rows lag the tape by 2 ticks for the buggy and
baja and by 1 for the other three, steady through each drive (row 0 is the state's vsync + 2 in all five;
the states were paused at different points in the pad's poll).

| car | top ft/s | all four tires off, runs of 9+ rows | stopped (< 1 ft/s) under the brake | hand-brake turn |
|---|---|---|---|---|
| buggy | 100.3 | 110–125, 193–277, 401–457 | row 483 | 52.9 → 7.4 ft/s |
| baja | 92.2 | 103–126, 176–214, 263–297, 327–366, 659–686 | 455 | 65.4 → 25.6 |
| sand rail | 125.1 | 101–117, 177–233, 341–431 | 466 | 54.3 → 6.2 |
| golf cart | 67.0 | 137–154, 236–273 | 455 | 31.7 → 7.0 |
| monster truck | 119.6 | 145–153, 170–229, 314–395, 420–466 | 504 | 64.2 → 18.7 |

- The opening right turn (ticks 15–60) misses the palm ahead of the spawn and lines up the dune jump.
  - My first try went straight and hit the palm at 75 ft/s, then wrecked and was put back.
- The truck's left-right steer (ticks 300–380) falls in its second flight, so on the ground the truck only
  steers on the opening right turn and the hand-brake turn.
- The golf cart only hops.

## How the states and drives were made

There is no uinput here, so all input is a generated .p2m2 played through AT-SPI menu presses (C6's
playback scripts, research/c15/tape.py). The steps:

1. `research/md39a/step.sh PID menu.pad <title state> SLOT` gives options.p2s.
   - The title state is SLUS-21104's slot 4 from ps2-decomp-kit's s9 copy.
   - Cheats are needed: this memory card had only the sand rail unlocked. menu.pad types the stock code
     TOOLAZY under Options / Cheat Codes ("You unlocked everything!").
   - Desert Oasis offers all nine machines. St. Paul FMX offers only MX, ATV, the monster truck and the
     golf cart.
2. `research/md39a/state.sh PID NAME K SLOT` gives each car's state and RAM.
   - It presses Vehicle right K times (4 buggy, 5 monster truck, 6 Trophy Truck, 7 golf cart, 8 sand rail),
     then Start, then waits out the load.
   - `ram.py` writes the RAM image and lists the car objects.
3. `research/md39a/drive.sh PID NAME OBJ VPTR` plays drive.pad from the state and records car.toml every
   tick (play.sh, `./kit record`). Recording starts on the state's first tick, and only once the vptr is in.

The run's settings:
- kit config: SLUS-21104, the MX vs. ATV iso, elf SLUS_211.04, vsync at 0x3fa6d8.
- environment: `KIT_PCSX2=1`, `WAYLAND_DISPLAY=wayland-1`, Qt accessibility on.

Things that broke on the way:
- Play needs System/Reset first, or the movie's state does not load.
- A screenshot is only written while the VM runs.
- Menu presses need ~90 ticks between them. One made during a preview's load is dropped.
- The save lands a second or two after `pine save` returns, so state.sh waits for the file.

## How it was checked

- `./kit check -- -p game-sim --test mxvatv_car_captures -- --nocapture`: 3 passed. With `cargo test` the
  same 3 pass and print no skip.
  - `each_state_holds_the_named_car`, per RAM image:
    - the object's vptr is the car's;
    - that vtable keeps 135 to 137 of car 0x003cf130's first 144 slots;
    - the object's +0x10f0 is the car's .phy CofG, bit for bit. This tells the buggy, baja and sand rail
      apart, which share a vtable. It is CofG, not CofGAI: the truck's file has both and they differ.
  - `the_captures_load_and_name_the_cars_vtable`: 730 rows, vsync +2 a row from the image's own vsync, and
    every row's vptr (decoded and in the raw block) the car's.
  - `the_drives_follow_the_tape`:
    - gas +0x314, hand brake +0x1138 and stick steer +0x11c8 equal the tape's Cross, Circle and lx on
      every row;
    - the brake +0x310 is on whenever Square is;
    - each car has 10+ rows with all four tires off, stops under the brake, and passes 40 ft/s.
- Repeatability:
  - The buggy drive played twice from buggy.p2s: identical, every column of all 730 rows (research/c15/same.py).
  - The golf drive played again through the committed drive.sh: identical except one byte of the car
    object (+0x45, bit 0x40) on one row. The body, tires and engine match.
- Remaking the golf state with state.sh gives the same objects. It is saved one tick earlier (vsync 28491
  vs 28493), so a state is only reproducible to the tick. A drive from a given state is reproducible.
- `./kit audit`: only the 2 plan/DONE.md leaks already in HEAD.

## What the input words are

These were read off the drives against the tape (research/md39a/inputs.py), not off the code. They are
in research/oracle.txt `# MD39a`.

- MXVehicle's block:
  - +0x310 brake (also 1 at rest);
  - +0x314 gas;
  - +0x318 the gas slewed (0, 0.4, 1).
- The car's block:
  - +0x1130 brake;
  - +0x1134 brake force (5 under the hand brake);
  - +0x1138 hand brake;
  - +0x113c/+0x1140 steer, scaled by speed;
  - +0x11c8 the stick;
  - +0x11d0 the stick slewed;
  - +0x8d0 (stick + 1) / 2, slewed.
- The gas does not reach the car's block.
- MD39's guess (MX Unleashed's inputs moved +0xd3c) holds for the hand brake only.

## Not verified / not 1:1

- Nothing here is replayed: that is MD39b. The csvs are end-of-tick reads by the recorder. I have not
  checked whether a row can tear mid-tick (`frame-counter-trigger-tears-captures`), though repeat runs
  agreeing makes it unlikely.
- The one-byte difference at +0x45 between the two golf runs is unexplained. It is a flags byte, not in
  the body or the tires.
- Clutch (L1) is not in the tape.
- The truck steers left and right in the air, not on the ground.
- The copy's PCSX2 slots 1–9 (SLUS-21104) are this task's scratch; the kept states are the copies in
  `context/mxvatv-ps2/md39a/`.
