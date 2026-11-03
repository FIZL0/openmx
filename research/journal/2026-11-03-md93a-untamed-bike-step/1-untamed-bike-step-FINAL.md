# 1 — untamed bike step (FINAL)

Task MD93a. 2026-11-03.

## What I did

- **`handling::untamed::backflip`** (`crates/game-sim/src/handling/untamed.rs`), with its own `FlipIn`. It
  uses `handling`'s `FlipClip` and `FlipOut`, so the rider's side reads as SLUS-21104's does.
- **`untamed::ride_backflip`** is `handling::ride_backflip` on Untamed's input. `untamed::stunt_stand_in`
  moves the flip's id and the idle id in the air up by 8 (49, 54); `FIRST_FLIP_STUNT` and `IDLE_STUNT` do the
  same.
- **`ride.rs`**: on `Handling::Untamed` the backflip call goes to `untamed::ride_backflip`. Every other
  handling is unchanged.
- Read with a throwaway decoder over the RAM image (the kit's `r5900` decoder on the words at the address),
  since this slot has no Untamed kit.toml. The decompile is `context/untamed/decomp/game.c`.
- Filed: **MD93f** (feed the real input on the ride) and **R29** (low: SLUS-21104's port writes the lip's
  `>` where the original's compare sends a NaN the other way).

## What the original does

Read off the disassembly; the field map is in research/oracle.txt `# MD93a`. The flow is SLUS-21104's,
instruction for instruction: the same torque map (including the equal-times delay-slot answer), grace,
end, clip machine and flipping/authority tail, with the fields moved. What changed:

- **The trick buttons.** SLUS-21104 tests one nibble: `== 4` (R1 alone) arms, `== 0` lets the torque decay.
  SLUS-21701's input reader keeps four debounced bytes, one per trick action, in a pad record.
  - Arming tests the fourth byte only, so R1 with another button still arms.
  - The decay needs all four clear.
- **Two sticks.** The lip reads the reader's copy of the stick's fore-aft (a pad word of its own, written
  before the limiter; the d-pad writes ±1 to it too). Its hold is a separate control's value. The flight's
  decay reads the limited stick, the one the preload reads.
- **A bound backflip.** On the lip it first asks the controls' binding table whether this device has the
  backflip bound. If it does, the stick path is skipped entirely. If the binding is pressed, the flip fires
  at once with the torque of a 0.8 hold. Unlike the stick's trigger, it does not clear `armed`.
- **The stunt-id bound** is 48 (`slti 49`).
- The clip is stored through a one-line vt setter, and the torque goes through the body's add-moment
  call, as on SLUS-21104 (`.xyz` add, then the listeners). Neither changes what is written.

## How it was proved

`crates/game-sim/tests/untamed_reworked.rs`:

- **`untamed_backflip_is_the_originals`**: a function oracle over 20,000 generated states on the
  SLUS-21701 Free Ride image.
  - The bike, body, frame node, collision record and binding table are laid out at Untamed's offsets.
  - The binding-table read, the clip setter and the body's moment add run unhooked.
  - Start stunt, end stunt, wreck entry, the steep test and the binding's "pressed" are hooked. Their
    arguments are checked (start stunt gets the stunt word and 1). They are compared with what the port
    asked for.
  - Every word of all five regions is compared by bits. The original wrote nothing outside them and its
    stack.
  - 13 paths are covered at least 100 times each: off the lip, bound pressed, bound not pressed, arming,
    triggered, waiting, in the grace, off the lip in time, ended, ended mid-clip, turning decaying,
    turning right side down, turning right side up.
  - Stunt ids of exactly 48 and 49 are generated, and sticks exactly on ±0.8 and ±0.81.
- **Hand mutations.** Each of these went red:
  - stunt bound 41
  - the bound control clearing `armed`
  - the lip reading the decay's stick
  - the bound control firing with the stick's hold
  - decay blocked by the arming button only
  
  Writing the lip with `>` instead of `!(<=)` stayed green: no NaN is generated (R29 covers the same
  question for SLUS-21104's port).
- **`untamed_ride_backflip_arms_on_r1_with_another_button`**: the wiring test. R1 + another trick button on
  a lip arms Untamed's flip and not SLUS-21104's. The clip start, the stunt latch, the mid-clip wreck (both
  reasons) and the stand-in ids are as SLUS-21104's ride has them.
- **Nothing else moved.** These suites pass:
  - `game-sim`: `mxvatv_air` (5), `untamed_step` (2), `mxvatv_ride` (4), `ride` (67), `replay` (121) and
    `hash` (16). Both images are present here, and `--nocapture` showed no skips.
  - `cargo check -p openmx --tests` builds.
  - `./kit audit` shows only the 4 leaks already on main.

## Not verified / not 1:1

- **The ride's input is stood in (MD93f).** The port's controls have three trick buttons and no backflip
  binding. So on the ride:
  - the arming button is R1, the one SLUS-21104 arms on. Which pad button is SLUS-21701's fourth trick
    action was not traced.
  - "a trick held" is any of the three;
  - `button` is `None`;
  - the lip's stick and hold are the step's stick and flick. On SLUS-21701 they are a pre-limiter copy and
    a control of their own; how they differ from the limited stick and the preload's hold was not traced.
- **The binding table and its matcher** (1228 bytes) are hooked, not ported. What binds a backflip, and on
  which devices, is unknown.
- **The frame rows.** The frame node moved from `+0xe4` to `+0x114`, and its rows' `y` from `+0x84`/`+0x94`
  to `+0xb4`/`+0xc4`. I took them to be the same right and up rows by the 0x30 shift. This was not checked
  on a live bike's node.
- **The stand-in ids.** The flip's id 49 and the idle id 54 in the air follow MD81a's "+8" on the start stunt
  and the air control. The ground's `0xff` is assumed unchanged.
- **No NaN inputs** were generated: the original's compare forms are copied, not proved, for NaN.
- **No ride was watched on Untamed's handling**, and no capture of the original's backflip was compared.

## Next

none — MD93b (spin in the air) is the next of MD93's split. MD93f feeds this flip's real input.
