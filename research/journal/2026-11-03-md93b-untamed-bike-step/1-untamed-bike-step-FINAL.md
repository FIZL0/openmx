# 1 — untamed bike step (FINAL)

Task MD93b. 2026-11-03.

## What I did

- **`handling::untamed::spin`** (`crates/game-sim/src/handling/untamed.rs`), with its own `SpinIn` and
  `SpinOut`. It works on `air::Spin` and `air::SpinTuning`, the same state and tuning as the port's
  `air::three_sixty`, so the rider's side (`wrecked`, `cut`, `came_round`, the clip) reads the same.
  `spin_heading` and `clamp_unit` are copies of `atv_moto`'s private ones (SLUS-21104's 360 on the quad, proved
  in `tests/atv_quad.rs`). I copied them so I would not touch that file.
- **`untamed::ride_spin`** is the ride's call. It applies `SpinTuning::tuned`, builds stand-in input and takes
  the 360's wreck as the pack ride does: `wrecked`, and `cut` when it is still in the air.
- **`ride.rs`**: in the trick-pack branch, `Handling::Untamed` calls `untamed::ride_spin`. Every other handling
  still calls `air::three_sixty`. Without a pack there is no 360 tuning, so that path has no 360 to replace.
- Filed **MD93g**: feed the real input on the ride. That work is MD93f's tracing again, for the 360's words.

## What the original does

I read it off the disassembly with a throwaway decoder over the RAM image. The decompile is Untamed's
`decomp/game.c`, and the field map is in research/oracle.txt `# MD93b`. The flow is SLUS-21104's (0x004e53d0)
with the fields moved 0x10 down, except for these changes:

- **The trick buttons.** These change as for the backflip:
  - Arming tests the fourth byte of the pad record only (0x003a2e40).
  - The torque decays only when none of the four is held (0x003a2c60).
- **Two sticks.** The lip reads the reader's stick copy at +0x12bc. The decay reads the limited stick at +0x1298.
- **A bound 360.** When the device has the control bound (0x00252630), the stick path is skipped.
  - The game asks "pressed left" (0x00252690) first. If that says no, it asks "pressed right" (0x002526e0).
  - A press sets yaw = the *old* direction × 1100 and pitch = 200, grace 0.9, active, not armed. Only then does
    it store the new direction.
  - So a bound 360 turns the way the previous one did. From a direction of 0 it has no yaw, and it ends as soon
    as it arms. The port keeps this.
- **Stunt ids.** An end sets a stunt id ≥ 49 to 255 (SLUS-21104: ≥ 41).

## How it was checked

- **`untamed_spin_is_the_originals`** runs 20,000 generated states on Untamed's RAM image `nat_or.bin`.
  - **Comparison.** Every word of bike, body, frame, collision and binding is compared by bits, and every
    original write must land in those regions or the stack.
  - **Hooked calls.** The node's axes, start stunt (word and argument), end stunt (word), wreck (reason), steep
    and the binding's left and right are hooked and compared. The node must be asked exactly when the 360 is
    active. The binding is asked only on a bound lip, and right only when left is not pressed.
  - **Unhooked calls.** The pad readers, the binding byte, the clip setter (vt +0x534) and the body's torque add
    run as they are.
  - **The arc cosine table.** Its entries and its scale (14999.5) are compared against `AcosTable::build()`.
  - **Coverage.** 18 paths are tallied from the disassembly's branches, not the port's, and each must reach
    ≥ 100 states. The paths: off the lip; bound left, right and neither; fired; arming left and right;
    waiting; in the grace; taking off; ended; ended mid-clip; turning (decaying, stick centred, held); clip
    started, playing, and came round.
- **`untamed_ride_spin_arms_on_r1_with_another_button`** is the wiring check.
  - Untamed's 360 arms with R1 and another button held, and `air::three_sixty` does not.
  - In the air with the stick against the turn, a held button keeps the torque, and with none held it decays.
  - A landing mid-clip is a wreck that is not cut.
- **Hand mutations, each red then reverted:**
  - a bound press using the new direction;
  - a stunt bound of 41;
  - firing on x instead of the lip's x;
  - arming on any button;
  - decaying on the fourth button only.
- **One mutation that cannot go red.** Letting the stick at exactly 0 decay is invisible: the decay would
  subtract dt × rate × 0. The port keeps the original's test anyway.
- **Regressions.** These game-sim tests pass: untamed_reworked, untamed_step, mxvatv_air, mxvatv_ride, ride,
  replay, hash, air, and atv_quad. Their skips are all captures and images this slot's context/ lacks, none of
  them Untamed's. `cargo check -p openmx --tests` builds.
