# 1 — mxvatv kiowa step (FINAL)

Task MD80c. 2026-11-05.

## What I did

- **Ported SLUS-21104's Kiowa step** into crates/game-sim/src/kiowa.rs, after MX Unleashed's code and beside
  it. Nothing of MX Unleashed's step changed. The new pieces:
  - `Controller::step_mxvatv`;
  - `Aero::step_mxvatv`, with the new `Pedal` (the three `…Pedal` keys and the fade-back timer);
  - `Crash::step_mxvatv`;
  - `Kiowa::force_mxvatv` and `Kiowa::tick_mxvatv`, which carry a `Mxvatv`;
  - the reader `Phy::read_mxvatv` / `read_mxvatv_bytes` → `MxvatvPhy`.
  - The switch is `tick_mxvatv` against `tick`. The private `Lists` got one `mxvatv: Option<&mut Mxvatv>`
    field, which picks the force and crash calls. `tick` passes `None`, so MX Unleashed's path is the old
    code.
- **The rotor is MX Unleashed's `Rotor::step`, unchanged**, because the oracle agrees bit for bit (below).
- **Read the PC exe first** for both rewritten steps.
  - `./kit fn 0x00450dd0 -o pc` is HelicopterAeroModel's step; its vf0 destructor is at 0x00450d40. It has
    the PS2 step's terms: the ±0.2618 alpha/beta clamps, the pedal branch with its timer, and the low-speed
    drag when the longitudinal stick is ≤ 0.
  - `./kit fn 0x005f0b50 -o pc` is the controller (MXVehicleHelicopter::vf85). It has the −30 floor on the
    speed command, no slow-speed pitch branch, and the feed-forward gated on airspeed and scaled by
    |ground speed|.
  - The PS2 code is what the proofs run.
- **Found the real controller.** research/oracle.txt had `atv_kiowa_flight_controller` at 0x004d45f0, but
  that is a `j 0x004a7990` thunk. The step the Kiowa's vtable calls (+0x474, slot 0x003cff44) is
  0x004d4600, now `atv_kiowa_controller_step`.
- **Found the wreck check** (the brief's "unpaired: find it"). The Kiowa's crash test is
  `atv_kiowa_crash_test` (0x004d5410, vtable +0x47c).
  - It first calls MXVehicle's wreck check (vtable +0x16c = `atv_vehicle_wreck_check`, MD80b's), unless the
    wreck reason is already 13.
  - Then it runs MX Unleashed's clocks.
  - The throw itself is `biplane::throw_mxvatv`, reused.
- **Wrote down** the fields, the call order of atv_kiowa_forces and the rounding in research/oracle.txt
  `# MD80c`.
- **Filed MD104**, the flight replay (below).

## What the original does

- **Forces** (`./kit fn atv_kiowa_forces`):
  - while the wreck reason (+0xc80) is 0, the controller (vtable +0x474, f12 dt);
  - the collision wrapper;
  - then, if the reason is 0 and *(+0x684) is set with bit 0 of its +0x50, the rotor step, then the aero step.
- **Controller** (`./kit fn atv_kiowa_controller_step`): MX Unleashed's controller with these changes.
  - The ground speed is signed: sign(forward) · sqrt(vz² + vx²), where forward is the z of the velocity
    taken into the model's node (atv_node_local).
  - The speed command is one madd, then clamped to max(−30) and min(MaxSpeedCommand).
  - The pitch target has no high-speed-aero branch.
  - The feed-forward runs only when !(airspeed ≤ min speed), and its scale curve reads |ground|.
  - The pitch rate is adda(0, ff).madd(PitchGain, pitch target − pitch), clamped by MaxRateCommand[0].
  - The collective, its integral and the filter are fused.
  - The roll wrap stays the unfused one (mul.s, sub.s), not the biplane's fused wrap.
- **Rotor** (`./kit fn atv_kiowa_rotor_step`): MX Unleashed's, float for float. The fields are 0xc lower.
- **Aero** (`./kit fn atv_kiowa_aero_step`):
  - The cross products are scalar code.
  - Roll is mula(rk, Clp·r).madda(Clbeta, beta).madd(Clda, lateral).
  - Pitch is adda(0, CM0).madda(CMalpha, alpha).madda(pk, CMq·q).madd(CMde, longitudinal).
  - **Pedal branch.** While the pedal is held, CY uses CYbetaPedal and Cn uses the Pedal pair, and the timer
    is reset to its time (5).
  - **Fade-back.** Once the pedal is let go, the timer runs down and Cnbeta/Cnr fade back in over the curve.
  - **Low-speed drag.** With the longitudinal stick ≤ 0, local velocity · LowSpeedDrag · (area·((0.5·(1 −
    fade))·density)) comes off the force.
- **Crash test** (`./kit fn atv_kiowa_crash_test`):
  - if the reason ≠ 13 and MXVehicle's check finds the helicopter past the terrain's edge: the throw, then
    wreck 13;
  - then MX Unleashed's clocks (+0xc84 clock, +0xc88 MaxWreckTime, +0xc8c down);
  - the blades (vtable +0x478) with the touchdown event (atv_post_event), and the put-back (vtable +0x36c).
- **Reader** (`./kit fn atv_kiowa_read_phy`, `./kit fn atv_kiowa_read_camera`):
  - Model{One,Two}PelvisBoxPosition/Size;
  - MaxWreckTime for WreckTime;
  - the `[Camera]` shoulders and the FP camera offset;
  - the three Pedal aero keys.

## How it was proved

crates/game-sim/tests/mxvatv_kiowa.rs is a function oracle on MD80a's `kiowa.ee.bin` (the helicopter at
0x01ab7d20, model one). Each test runs 10,000 generated cases and compares every written field bit for bit:

1. **Controller**: atv_kiowa_controller_step against `Controller::step_mxvatv`. Forward is handed in through a
   hook on atv_node_local. It compares all five commands, the altitude fraction, the speed command, the last
   stick, the hold height and timer, the integral, and the filter value and step.
2. **Rotor**: atv_kiowa_rotor_step against MX Unleashed's `Rotor::step`. It compares the pitches, the tilt,
   the thrust, and the force and moment the body's vtable is handed.
3. **Aero**: atv_kiowa_aero_step against `Aero::step_mxvatv`. It compares CL..Cn, the fade, the pedal timer,
   and the force and moment.
4. **Throw**: atv_vehicle_wreck_check on the Kiowa against `throw_mxvatv`. It compares the velocity, the spin,
   the gravity scale and the count of rand draws.
5. **Crash clocks**: atv_kiowa_crash_test with reasons 1 and 13 against `Crash::step_mxvatv`, its virtuals
   hooked. It compares the clock, the down flag, the velocity, and which of check/blades/sound/put-back ran.
6. **Reader**: kiowa.phy (context/mxvatv-ps2/md80c) read as model one and model two against what `kiowa`
   and `kiowa3` loaded. It compares the wrecks, MinWreckSpeed, the pelvis box, the shoulders, the FP camera,
   the 20 aero keys, the pedal keys and time, the 23 controller keys, the rate limits and the rotor keys.

All six pass. A scratch run that wrote each test's summary to a file showed none skipped. Mutations, one at a
time: −30 → −29 failed test 1, and the low-speed drag's `<= 0` → `< 0` failed test 3. A third, regrouping
the 0.5 product, changed no bits (a ×0.5 is exact), so it says nothing.

MX Unleashed's Kiowa still holds:

- tests/kiowa.rs (19), kiowa_phy (2) and kiowa_capture (9) pass.
- kiowa_capture's C14 flight replays ran against C14's capture, copied into this worktree's context/ from
  kept-context/s4.

## Not verified / not 1:1

- **MD80a's Kiowa flights are not replayed** (the brief's first test). The capture csvs are in no context on
  this machine; only the .p2s states are kept (kept-context/md80a). The replay also needs SLUS-21104's measure
  and input, which are not ported. Filed as MD104.
- **The ATV measure is not ported.** `Kiowa::tick_mxvatv` runs MX Unleashed's measure. Its row
  `measure.matrix` row 2 as the forward row, and `measure.velocity` as the aero's local velocity, are
  assumed to equal what atv_node_local and the helicopter's +0x350 give. `forward_mxvatv`'s VU0 dot is not
  compared with atv_node_local.
- **The forces' +0x684 gate is not ported.** The port always steps the rotor and aero while the reason is 0.
- **The crash test's reason-0 strike path** (the terrain and the rotor's collision object) is read, not
  tested. Test 5 covers reasons 1 and 13.
- **The throw test fixes the gate and the edge.** Its hooked atv_terrain_edge always says "at the edge", and
  the gate word +0x308 is set to 1. The edge test itself is proved on the plane in tests/mxvatv_biplane.rs.
- **C17/C21's strike and C24's roll replays skip** in kiowa_capture: their captures are in no context or
  kept-context here. MX Unleashed's path is unchanged code, so their hashes should hold, but this task did not
  see them run.
- **The aero constructor that sets the pedal time 5 was not found.** The 5 is proved from the loaded image.
- **No MX vs. ATV Kiowa flies in the game yet.** That is MD80e.

## Next

Nothing waits here. MD104 is the replay; MD80e puts the helicopter in the game.
