# 1 — mxvatv car launch and accel (FINAL)

Task P368. 2026-11-17.

## What I did

- Decided: **over 100%, MX vs. ATV car acceleration turns the car's launch off** (`MaxClutchTorqueScale`
  set to 1 in `Mods::mxvatv_car`, `crates/game-sim/src/modifiers.rs`). At or under 100% the file's
  launch stays, so 100% is bit-exact as before.
- Why not the brief's "divide the launch by k": I swept the buggy, taken a foot over a flat floor with
  the gas held from tick 1, at 110–500% in 10% steps, for three rules (throwaway probe, not kept):
  - launch / k: still held (under 10 ft/s at 1 s) at 110, 120, 130 and 140%; slower than 100% at 1 s
    from 150 to 270%.
  - launch / k²: held at 120% and 140%.
  - launch off: never held; at 1 s 26–32 ft/s against 100%'s 24, at 2 s 55–63 against 56.
  - The launch window is chaotic (the rear tires' spin step against the ground's pull), so any rule
    that keeps part of the 6.6× boost on top of k× torque lands on a held knob value somewhere.
- Why not "take the shell's car with its launch spent": that changes the car at 100% too (the original
  starts the timer at 0, R36), and it lives in the shell's `vehicle.rs`, outside this task's files.
- Tests in `crates/openmx/tests/mxvatv_car_modifiers.rs`:
  - `mxvatv_car_acceleration_over_100_turns_the_launch_off`: the key is 1 at 110/150/500%, the file's
    6.6 at 50%, and the other three knobs at 200% leave it.
  - `a_freshly_taken_mxvatv_car_on_the_gas_drives_off_over_100`: gas from the take, every 10% from
    110 to 500: speed at 1 s ≥ 100%'s, at 2 s > 0.95 × 100%'s.
  - The `SETTLE` comment no longer says the knob holds the car.
- P288's ride numbers after (buggy): top speed 72.6 / 145.2 / 215.9, grip 115 → 36, rest height
  −0.62 / 0.06 / 0.40: unchanged. Speed 2 s off the line at 150%: 62.6 → 61.1 ft/s (the launch is
  still 1 s in after `SETTLE`); 100% 53.8 and 50% 21.4 unchanged. Still over the test's 1.1× bar.
- The pack: copied openmx-mods' `mxvatv_cars` into this slot's ignored `mods/machines/` from s2, so
  the riding tests ran rather than skipped.
- opentennis: nothing there on car launches; nothing taken.

## What the original does

Nothing for the knob: Game Modifiers are an addition. The launch itself is R36's (`./kit fn 0x004cba30`,
port `atv_car::launch`): the clutch out at a standstill starts the drive torque scale at
`MaxClutchTorqueScale` (6.6 for the buggy) and eases it to 1 over `ClutchTorqueScaleTime`.

## How it was proved

- `cargo test -p openmx --test mxvatv_car_modifiers`: 16 pass, with the pack present (the numbers above
  are from its `--nocapture` output).
- 100% stays bit-exact: `every_knob_at_its_default_leaves_the_mxvatv_car_bit_exact` (P288's) still passes;
  the change only runs with the knob over 100%.

## Not verified / not 1:1

- **The knob is an addition, not the original.** With it over 100% the car has no launch boost, which
  SLUS-21104 never does. That is the design choice, not a match.
- **Only the buggy was swept.** The other four MX vs. ATV cars have their own launch keys; they get the
  same rule but were not ridden.
- **Only the shell's flat-floor take.** Other drops (heights, slopes, landing nose up) were not swept
  through the port; R36's interpreter test covers 150/200% drops with the launch spent and they drive off.
- **Not run against SLUS-21104 here.** The new tests ride the port only. R36 already proved the port's
  car frame equals the original's on these numbers, including spent launches.
- **The stock cars' Car acceleration knob** shares the launch step and may hold a car the same way; not
  checked, and left alone (filed as P370).

## Next

Nothing.
