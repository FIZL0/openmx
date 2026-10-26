# 1 — lever switches (FINAL)

Task P77. 2026-10-26.

## The question, decided

The brief asked the user first: under the New layout, R2's analogue gas (P40) becomes a switch if the bike
reads its levers as the original does. Should New stay analogue, or match the original? The run was
unattended, so nobody could answer. I decided to **match the original under both layouts**:

- P40 built New as a remap of the pad's report onto the original's Cross and Square, before the simulation
  reads it. That is how "the simulation still reads the original's controls". An analogue New gas would
  mean a bike pad read the original does not have.
- The user's 2026-10-22 note already put New's reverse on the original's own rule (gas+brake below 5,
  under both layouts), and that rule is written on the switches.
- It is easy to reverse. `ride::switch` is one call in `Controls::through`. The question is in
  plan/HUMAN.md ("The New layout's gas"), with what an analogue addition would take.

## What I did

- `crates/game-sim/src/ride.rs`
  - `switch(value)`: 1.0 for any non-zero strength, else 0.
  - `Controls::through` reads the gas, brake and clutch through `switch`. This is pad_to_gesture's lever
    half.
  - `Controls::pad_read_tail(stoppie, stoppie_brake, speed, airborne)`: while the stoppie latch
    (`mv::Ride::stoppie`, `+0x728`) is set, a held brake is `[Pitch]` StoppieBrake. Then it runs P72's
    `reverse_rule`. Before this, `phy::Pitch::stoppie_brake` was read and never used.
  - `reverse_rule`: in the air, at speed with both levers held, the gas is now 1.0, which is the
    original's own write. P72 had left the lever's strength there.
  - `Controls::hands_off`: the rider's no-hander (`+0xa44`). `Held::step` holds the gas at 0 while it is
    set.
- `crates/game-sim/src/rider.rs`: the ride's frame is handed `hands_off = tricks.no_hander`, as the rider's
  last tick left it.
- Two parts of the limiter's walk were already S58's, and I left them as they were:
  - Gas and brake rise at their rates and drop at once.
  - The clutch is limited both ways. The front brake equals the rear.
- `crates/openmx/src/pad.rs` tests. `gas_of` now returns the gas command's strength, which is what the
  biplane's throttle reads. It also asserts that the bike's gas is the switch of that strength. The tests
  that pinned the pressure plumbing (a DS2 pressure axis, `--throttle r2`, the keyboard ramp, New's
  triggers) still pin it, and they check the bike's switch beside it.
- `crates/openmx/tests/netplay.rs` `the_mashed_pads_are_a_ride`: it used to want more than five distinct gas
  values. Now it wants exactly one, 1.0.
- `crates/openmx/src/main.rs`: the module doc and the `--throttle` log line now say the bike's throttle is a
  switch and the plane's is a pressure.
- Taken from opentennis: nothing. It has no levers.

## What the original does

`./kit fn 0x00250fb0` (pad_to_gesture) and `./kit fn 0x00251a50` (bike_input_limiter). The full notes are in
research/oracle.txt under "P77".

- **Pad read, not wrecked.** For each of the three levers, the command's value reads 1.0 when it is not 0:
  - gas `+0x63c` (command 0x0e)
  - brake `+0x640` (0x0f), which is `+0x72c` StoppieBrake instead while `+0x728` is set
  - clutch `+0x644` (0x10)
  The reverse tail runs between the brake and the clutch.
- **Limiter, levers.**
  - Brake `+0x650` rises at `+0x68c` and drops at once. `+0x654` is set equal to `+0x650`.
  - Gas `+0x658` is 0 when any of these holds: throttle enable `+0x614` clear, `+0xa44` set, or reversing on
    the ground. Otherwise it rises at `+0x688` and drops at once.
  - Clutch `+0x65c` moves toward `+0x644` at `+0x690` going up and `+0x694` going down.
- **The enables `+0x610`/`+0x614` and the latch `+0x61c`.** I scanned the ELF for stores
  ([[scan-the-elf-for-writers]]) and found these writers:
  - 1 at the bike's set-up and on a put-back.
  - a message handler.
  - the bike tick, which copies them each tick through the pointers at `+0x1068`/`+0x106c`. In
    first_recording those pointers point at the bike's own `+0x610`/`+0x614`.

  Both riding save states (first_recording, and the biplane state's bike) have `+0x610` and `+0x614` set,
  and `+0x61c` = 0. So in freeride the latch never sets.

## How it was proved

`crates/game-sim/tests/levers.rs` `the_levers_are_the_originals_switches_and_walk` runs both original
functions in the interpreter and holds the port against them.

- The interpreter runs pad_to_gesture and then bike_input_limiter, chained as the bike calls them.
- It covers 20,000 generated cases. Each case draws:
  - strengths for the three commands: 0, 1, or part way
  - the stoppie latch and StoppieBrake
  - speed (including exactly 5) and the air flag
  - the no-hander flag
  - dt, the four rates, and the walked levers before the call
- Hooks:
  - pad_control_index is hooked to return the command as its row, so the generated strengths are what the
    read finds.
  - stick_direction is hooked to return 0.
- It compares eight fields bit for bit: `+0x63c`, `+0x640`, `+0x644`, `+0x648`, `+0x650`, `+0x654`, `+0x658`,
  `+0x65c`.
- The port side is `switch` → `pad_read_tail` → `Held::step`.
- All 20,000 cases agree. A temporary panic printed the branch counts before I removed it: 4,499 reverse,
  3,113 stoppie brakes, 2,710 gas cuts. The test asserts each count is over 1,000.
- `a_part_pressed_lever_is_a_switch` covers `switch` on its own.
- pad.rs tests: under New, half an R2 is half of the gas command's strength and is the whole switch on the
  bike. Under Original, a part-pressed Cross is likewise the whole switch.

## Not verified / not 1:1

- **The user's question was answered by me, not the user.** New's R2/L2 are now switches on the bike. If
  the user wants analogue back, it is an addition (plan/HUMAN.md).
- **The enables, the start-line latch and the race gas cut are not ported** (filed as P156).
  - The latch is `+0x61c`. It holds the brake on and cancels reverse after the steering enable `+0x610`
    drops.
  - The gas cut is the bike tick's: gas is 0 while `*(+0xf9c)+0x1c0 > 0`.
  - The port has nothing that clears the enables, and every riding state has them set. The interpreter
    proof sets them to the riding values.
- **A zero rate.** The port reads a rate of 0 as no limit (S58's choice for a .phy missing the key). The
  original reads it as never moving. The proof leaves zero rates out. Every disc bike's rates are positive,
  but I did not check them all here.
- **The wrecked branch of the pad read** (`+0x748` set: levers zeroed, `+0x648` kept) is not compared. The
  port does not run the bike's frame while the rider is down.
- **The stick half of both functions is not compared in this proof.** That is the d-pad over the stick,
  the squaring and the lean walk. It is P30's, proved there.
- **The no-hander's timing.** The port hands the limiter the rider's `tricks.no_hander` from the end of the
  last tick. That matches the original only if the original's trick update also runs after the bike's
  limiter. I read that from the port's own order (rider.rs steps tricks after the ride), not from a
  capture of a no-hander with the gas held.
- **No live capture.** The proof is the interpreter on generated states, not a capture of the original
  riding with part-pressed Cross. This is a pure function of its arguments and RAM, so the interpreter is
  the method PROVING.md picks.
- **The keyboard throttle ramp (`KeyRamp`) no longer does anything for the bike.** A held key becomes 1.0
  on the first frame past the threshold, about frame 2. It still ramps the plane's throttle. I left it in
  place rather than reworking the shell.
- **The ride HUD's input cells** (`hud.rs`, "want" vs. "held") now show the bike's want as 0 or 1. I did not
  look at them on screen.
- **The gate's lobby LAN lane** (`crates/openmx/tests/lobby.rs`, "both seated: not in 30 s") failed in three
  runs here at load average 17–54, each time on different tests. It is LAN seating and does not read the
  levers. I did not prove it on HEAD at the time; see the gate below.
