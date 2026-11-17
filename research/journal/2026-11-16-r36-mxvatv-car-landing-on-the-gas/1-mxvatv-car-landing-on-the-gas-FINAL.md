# 1 — mxvatv car landing on the gas (FINAL)

Task R36. 2026-11-16.

## What I did

- Read the original's own gas landing first. In C29's buggy drive (`context/capture/md39a/buggy.csv`,
  rows 278–290) the buggy lands its dune jump at ~85 ft/s on full gas. The rear wheels come down
  from ~130 to ~80 rad/s, and nothing flips. MD107 and MD135 already prove that drive bit-exact.
- Reproduced the shell's case with a throwaway probe through `MxvatvCarRide::take`/`step` on a
  flat floor (not kept). At 150% the rear wheels go −27.15 → ~9.5 → +27.15 rad/s every three
  frames until about frame 37, with the engine at 4900–5600 rpm and the buggy still. ±27.15 is the
  tire's top wheel speed in first gear. At 100% they flip for two frames and then the buggy drives off.
- `crates/game-sim/tests/mxvatv_car_frame/drives.rs`:
  - MD169's `beside_posed` now takes a `Pose`: `Captured`, `Side(roll)` (MD169's, as before), or
    `Dropped { lift, pitch, accel, fresh }`.
  - `put_up_on_the_gas` lifts and pitches the image's car, still. Off 100%, it scales the same words
    P288's acceleration knob scales: the torque table, the converter's two, the wings' area, and each
    tire's wheel friction on all 16 surfaces. It multiplies in `F`.
  - `fresh` zeroes the launch timer (car +0x1168).
  - A dropped car gets full gas with the stick centred from frame 1, through the port's input step
    on a made-up tape. It runs for 60 frames.
  - `Seen.dropped` keeps each frame's tire spins, grounded flags, rpm and speed.
  - New test `a_buggy_dropped_on_the_gas_is_the_ports` runs the five drops in `DROPS`.
- Filed P355 (low): whether the acceleration knob should also scale the launch boost.

## What the original does

- **The launch** (the car's launch step, `./kit fn 0x004cba30`; port `atv_car::launch`):
  - With the clutch out, the first substep sets the launch start from `MaxClutchTorqueScale`, less
    with speed. For the buggy at a standstill that is 6.6.
  - The scale then eases to 1 over `ClutchTorqueScaleTime`.
  - The engine's drive torque is multiplied by it.
  - The timer runs whether or not the gas is held.
- MD39a's buggy image holds the timer at 2.255 s and the start at 6.6. So the original's car ran
  the full launch from 0, at a standstill, after it was placed. The port's `Load::state` starts the
  timer at 0 too, so the shell's freshly taken car has the whole 6.6× boost for its first ~2.25 s.
- Landing in that window on full gas, with 150% torque on top: the rear tires' spin step
  (`atv_car::slip`: spin += inv_inertia·(torque − force·radius)·h, clamped to ±top) overshoots each
  substep against the ground's pull at a crawl. It locks into the 3-frame swing and holds the car
  still until the boost has eased off to about 3.4.
- With the launch spent, no drop sticks. That held at 150% and 200%, from 1–2.5 ft, landing fronts
  first or rears first.

## How it was proved

- `a_buggy_dropped_on_the_gas_is_the_ports` (`tests/mxvatv_car_frame/drives.rs`), in the
  interpreter:
  - It runs SLUS-21104's sim frame from MD39a's buggy image (`context/mxvatv-ps2/md39a/buggy.ee.bin`),
    with the port's `atv_car::frame` beside it at every world step.
  - Every word of the car, body, pieces, vehicle and record must be equal on all 60 frames of all
    five drops. They are.
  - Then the test asserts the original's outcome for each drop:
    - Fresh at 150%, fronts first (lifted 1 ft) and rears first (1.5 ft, nose up 0.1): held. The
      rears sit at minus their top on ≥ 8 frames, under 1 ft/s at frame 30, and 4500–6000 rpm over
      frames 13–30.
    - Fresh at 100%: away, over 20 ft/s by frame 30.
    - Spent at 150% (1 ft) and at 200% (2 ft): away.
- The whole `mxvatv_car_frame` suite passes (36 tests), MD169's on-its-side test with the new
  `Pose` among them.
- The shell's numbers match the original's in kind: the same ±27.15 swing, the same revs, and the
  same release once the boost is near 3.4.

## Not verified / not 1:1

- **No PCSX2 capture of this landing.** The original's frame was run in the interpreter from a RAM
  image that I posed by hand: lifted, pitched, made still, the torque words scaled, the launch timer
  zeroed. The game itself never makes that state, because it has no 150% knob and does not drop a
  car on the gas. The interpreter run is SLUS-21104's own code, but it was not checked against a
  live recording.
- **Not the shell's exact landing.** The interpreter runs on Desert Oasis's spawn slope with the
  image's compressed suspension. The shell runs on a flat floor with the tires at full drop and
  lands rears first after 7 frames. The drops cover fronts first and rears first, but the shell's
  state was not put into the interpreter word for word. The two agree in kind (the same top-speed
  swing, revs and release), not frame for frame.
- **Is the launch timer 0 when the original makes a car?** This is inferred from the image: timer
  2.255 with start 6.6, so the launch ran from 0 at a standstill. I did not catch the moment the
  car was made. `Load::state` (MD149/MD150) already starts it at 0.
- **Only the buggy.** The other four cars have the same launch step but different keys. They were
  not dropped.
- **The knob's 150% is an addition.** Its fields are poked into the image the way P288 scales the
  file. That they are all the words the knob touches is P288's claim, not re-proved here.

## Next

Nothing for R36. P355 is the design question (the knob and the launch boost).
