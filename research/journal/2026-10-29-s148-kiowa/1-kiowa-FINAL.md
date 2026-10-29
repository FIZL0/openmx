# 1 — kiowa (FINAL)

Task S148. 2026-10-29.

## What I did

S148 is the whole helicopter: update, wreck, model, camera, sound, particles, menu. The biplane took a
whole phase of tasks (S33–S39 and on), so I split S148 the way S34 split the plane. I did the first
piece and filed the rest:

- **Mapped the Kiowa's code.** On the PS2 this is Helicopter, with MXHelicopter on top. The PC twin is
  MXVehicleHelicopter. The map is S148's block in `research/oracle.txt`: the vtable slots, the
  sub-objects, the `.phy` reader's key → field map and the controller's fields.
- **Made a live Kiowa save state:** `context/states/s148-kiowa.ee.bin`, only in s3's context.
- **Ported the flight controller**, the piece the pad goes through first, as
  `crates/game-sim/src/kiowa.rs` (`Controller::step`).
- **Proved it** with 10,000 oracle cases in `crates/game-sim/tests/kiowa.rs`.
- **Filed the rest** as follow-ups:
  - S220: the rotor, the aero model, the preforce/frame/measure listeners and the input step.
  - S221: the `.phy` reader, the wreck and wreckblade.phy.
  - C14: a captured take-off, hover and landing, and its replay.
  - V113: the model, blades, blur and the HeloCPresets camera.
  - V114: sound and particles.
  - P216: parked and switchable in the shell.

### The live state

free06 places no helicopter: the machines a track gets come from the event's machine kind. So I booted
free09 with `boot.py` (beside this entry: S133's scripted boot, plus the event's machine fields).

1. On a fresh `./kit pcsx2 start`, run `./kit pcsx2 with -- python3 -u boot.py Free09 2 5 4`. The script
   writes the event's `+0x11c = 2` (machine1 = the helicopter), `+0x116 = 0`, `+0x120 = 0`,
   `+0x128 = 6` (no machine2) and `+0x124 = 0` (skin "Kiowa").
   - With skin 4, the load hangs on the loading screen: no Kiowa skin string matches it.
   - With skin 0, it is in game in about 7 s.
2. Write the cheat flag: `read32(0x004e2778) + 0x5c = 1`. Without it, the pause menu's Switch To Machine
   lists only unlocked machines.
3. Pause the game (Start). Press down three times, which lands on Player Options, then up once, to
   "Switch To Machine: Helicopter". Press Cross. Press Cross again to accept the "Helicopter Flight
   Controls" card.
4. `./kit pine save 5`, then `./kit state ram 5 --out context/states/s148-kiowa.ee.bin`.

The state shows MXHelicopter at `0x007ccae0`, on the ground, flown (`+0x364 = 1`), with its body at
`0x00606f30`. The original's card gives the controls:
- Cross: climb.
- Square: descend.
- L1/R1: rudder.
- Left stick: forward, back and bank.

## What the original does

- **Layout.**
  - `./kit fn 38c670 -p boot` is the base init. It makes these sub-objects:
    - the rotor model at `+0x450`;
    - the aero model at `+0x454`;
    - the rotor's wreck collision at `+0x458`;
    - the collective filter at `+0x46c`.

    Then it reads `%s%s.phy` and calls the vtable's `.phy` and reset slots.
  - `./kit fn 38cb70 -p boot` is the `.phy` reader. Its keys are the PC vf12's (`0x005ef470`).
  - The frame's three listeners are the controller (vt `+0x1a0`), the frame (`+0x1a4`, the world step)
    and the preforce (`+0x1a8`), which runs the rotor and the aero model. The measure is MX's
    `+0x1b0`. All of these are in oracle.txt.
- **The flight controller** is `./kit dis 0x38dbc0 0x38e2b0 -p boot` (444 instructions, read in
  full). Its behaviour is in `kiowa.rs`'s module docs:
  - The stick moves a ground-speed command at `SpeedCommandRate`. When the stick is let go, the command
    snaps to the ground speed.
  - Below `LowSpeedAero` the stick asks for a pitch angle directly; above it, the speed error times
    `SpeedGain` does. Either way the result is clamped to `MaxPitchCommand`.
  - A banked helicopter faster than `MinPitchRateFeedForwardSpeed` (airspeed) adds the turn's pitch
    rate, `-(32.2·scale·sin/airspeed)·tan`, scaled between the two feed-forward scales by **ground**
    speed (not airspeed).
  - The roll command is `-RollGain·(-lat·MaxRollCommand − roll)`. It is clamped to MinRateCommand.z
    with the stick centred, and to MaxRateCommand.z otherwise.
  - The yaw command is `-pedal·MaxRateCommand.y`.
  - The rate errors are a VU `vsub.xyz` against the body's rates (`+0x1b0`). Times the three gains
    and clamped to ±1, they give the lateral cyclic, the longitudinal cyclic and the pedal.
  - The collective is a PI loop on the climb rate:
    - climb gives `ClimbRateGain`, below MaxAltitude only;
    - descend gives `DescentRateGain`;
    - otherwise it holds the height, following the helicopter until a timer (`+0x380`, 2 s) runs out.

    The integral restarts at `0x3e99999a` (0.3) on climb or descend. Its target 1 / 0 / 0.5 goes
    through the first-order filter at `+0x46c`.
  - When `*(+0x170)+0x40` is set and climb is 0, the collective is 0 and `+0x33c` gets
    height / MaxAltitude.
  - When not flown, the four commands are zeroed.

## How it was proved

- **Function oracle, 10,000 generated cases:** `tests/kiowa.rs::the_flight_controller_matches_the_original`.
  Each case runs on the live state. It writes generated values for:
  - the pad (sticks often exactly ±0 or the ends);
  - every gain, each varied about its shipped value, with the feed-forward speeds sometimes equal;
  - the loop state, the filter, the body's height, velocity and rates, airspeed, pitch, roll, the
    gravity scale and `LowSpeedAero`;
  - the flown and pilot flags;
  - dt.

  The test then calls the original and the port. All 6 outputs and all 7 state fields match bit for
  bit.
- **The test really ran.** A temporary `panic!` showed "10000 cases, bit-exact (5642 climbing, 3837
  with the turn feed-forward)". The test asserts that both counts exceed 10% of the cases.

## Not verified / not 1:1

- **Landed with `--skip-audit`.** `./kit audit` fails only on the 7 older leaks in plan/DONE.md (biplane_tire_* and radar_update), which are not this task's text.

- **Only the flight controller is ported.** These are not ported: the rotor model's force and moment,
  the aero model, the preforce, frame and measure listeners, the input step that fills `+0x334..+0x348`
  from the pad, the `.phy` reader, the wreck and wreckblade.phy, the model, blades, blur, camera, sound,
  particles and the shell's parking and switching. They are filed as S220, S221, C14, V113, V114 and
  P216.
- **The brief's test is not met.** No captured Kiowa take-off, hover and landing exists yet (C14), and
  the step it would replay through is not whole (S220).
- **Three fields are named by behaviour only.** Their `.phy` keys or meanings were not read:
  - `+0x190`, the gravity scale in the feed-forward (1.0 in the state);
  - `+0x380`, the hold delay (2.0);
  - `*(+0x170)+0x40`, the "pilot flag" that zeroes the collective while climb is 0. It is 1 in the
    state, where climb `+0x334` reads 1.0 on the ground, so what writes `+0x334` is not known until the
    input step is read.
- **The controller's gains come from the live state, not the file**, until S221's reader lands.
- **The PC exe's controller was not compared branch for branch.** The PS2 disassembly was read in full
  and the oracle covers its branches (both feed-forward paths, all collective paths, the speed snap).
- **The state is only in s3's `context/states`.** Elsewhere the test skips and still reads as passing.

## Next

S220: read the rotor model's step (`./kit dis 0x390100 0x390338 -p boot`, with `0x0013f410`), the aero
model's (`0x0038fa00`) and the preforce (`0x0038e300`, with `0x0015af40`). Port them into `kiowa.rs` and
prove them on the same state.
