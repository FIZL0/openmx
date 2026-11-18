# 1 — drive grip decision (FINAL)

Task P370b. 2026-11-18.

## What I did

Decided **no**: Car acceleration over 100% keeps the driven tires' grip as the file has it. Comments in
`Mods::car` and on `a_car_taken_on_the_gas_over_100_is_not_held` (`tests/car_modifiers.rs`) say why. No
behaviour change. Follow-up P373 filed for the monster truck's own rearing.

Nothing taken from `../opentennis` (no cars there).

Why no:

- The car's tire (`car::Tire::surface_force`) has one `FrictionPeak` for both directions, through a
  combined-slip shape. No key raises grip along the tire alone, as the quad's `MaxDriveForce` does.
- Tried as a probe (reverted): a non-key `drive_grip` on `car::Tire`, times the forward force while it
  pushes along the wheel's spin, set to k on the driven tires (the three cars drive the rear pair).
  Cornering force untouched. On the gas from the take, speeds 1 / 2 / 3 s in (ft/s):
  - baja: 100% 37/64/83; 200% 25/65 → with it 51/99/131,
    500% 30/71 → 88/169/206. Worst with it 0.84x (120%) a second in, against 0.65x without.
  - buggy: worst 1 s ratio 0.82x → 1.05x or better.
  - monster truck: from 280% it goes over onto its tail and leaves the floor (fronts up ~60 of 80
    ticks, 3 s speed 83 against 136 at 260%). Held to min(k, c): c = 1.2 or 1.3 still tipped it from
    300%, c = 1.4/1.5 from 320-340%.
- Without the raise the monster already rears on the torque alone: fronts up 7 ticks at 240%, rising to
  26 at 480%, and at 500% it goes over (3 s speed 86 against 106 at 2 s). Any forward grip moves that
  edge down into the knob's middle, which is worse than the baja's first second.
- Traction control (the files carry `MinTractionSlip` 0.10, `MaxTractionSlip` 0.15 with `TCSEnabled` 0)
  was tried too: it cut the baja to 13 ft/s at 100% and every car below the knob's numbers now.
- A grip cap from the car's geometry (front lift at mu > rear-axle distance / CoG height) needs the
  tires' positions, which come from the model, not the `.phy` that `Mods::car` sees.

## What the original does

Nothing to port: the knob is ours. The tire force is the ported `surface_force` (one friction for
both directions).

## How it was proved

Probe runs of `tests/car_modifiers.rs`'s take-on-the-gas ride on a flat floor (disc's `machine.res`),
every 20% from 100 to 500, for the three cars, with and without the raise and with caps 1.2-2.5. The probe
is not kept (the decision changes no code).

## Not verified / not 1:1

- The monster truck leaving the floor on its tail at high torque is not checked against the original
  (the original has no such knob, so it never drives a stock car on k times the torque). Whether the drive force
  tilting up with the pitched body is the original's model is P373's question.
- Only a straight take on a flat floor was measured; turns, slopes and the clutch-drop ride were not
  tried with the raise (it was rejected on the flat ride already).

## Next

P373: decide the monster truck's nose-up at high Car acceleration; then the baja's forward grip can be
retried with the probe above.
