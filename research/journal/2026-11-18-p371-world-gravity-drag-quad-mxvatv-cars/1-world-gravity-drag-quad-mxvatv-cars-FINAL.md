# 1 — world gravity drag quad mxvatv cars (FINAL)

Task P371. 2026-11-18.

## What I did

- Game Modifiers' World **Gravity** and **Air drag** now reach MX vs. ATV's quad and its five cars
  (`crates/game-sim/src/modifiers.rs`):
  - quad (`Mods::atv`): the part's gravity block (`GravityScale` and the force) x k, and
    `[Aerodynamics] Drag` (`load.mv.drag`, which the air step reads as the clutch keys' drag) x k.
  - MX vs. ATV cars: every wing's `BasicDrag` and `InducedDrag` x k in `Mods::mxvatv_car` (the wings
    the car's aero reads off its keys, lift left alone), and the gravity block x k in a new
    `Mods::mxvatv_car_part` on the load's part. The shell's `MxvatvCarRide::made` calls it and counts
    the two world knobs as "modded" so a moved world knob builds a modified load.
  - Both skipped at 100%, as every knob.
- A Kiowa or stock car already out takes the new gravity at once (`crates/openmx/src/vehicle.rs`,
  `Garage::modify`): the driven car's `Machine::gravity` (nothing writes it after the take) and the
  flown Kiowa's `gravity_scale`, and its `gravity` unless it is wrecked (its unwreck works the scale's
  out again). The take's two Kiowa gravity forms (MX Unleashed's part force, MX vs. ATV's unwreck
  form) went into one helper `taken_gravity` so modify and the take agree.
- A quad or MX vs. ATV car already out keeps its numbers until the next take, as with its other
  knobs (P287/P288's design); not changed.
- Nothing taken from ../opentennis: it has no game modifiers on these machines.

## What the original does

Nothing: the modifiers are an addition. SLUS-21104's quad and cars take their gravity off the part
reader's gravity block (`(0, -GravityScale * Weight, 0, 1)`), which the vehicle force adds each substep
and the anti-slide / slide read; that is the field scaled.

## How it was proved

- `crates/openmx/tests/atv_modifiers.rs::the_worlds_gravity_and_air_drag_reach_the_quad`: keys move
  (scale and force x2, drag x3); the quad settles lower at 200% gravity, higher at 50%
  (1.032 / 0.896 / 0.621 ft), and tops out lower at 400% air drag (120.4 -> 76.3 ft/s).
- `crates/openmx/tests/mxvatv_car_modifiers.rs::the_worlds_gravity_and_air_drag_reach_the_mxvatv_car`:
  keys move; the buggy sags with gravity (0.403 / 0.063 / -0.615) and tops out lower at 400% air drag
  (145.2 -> 116.7 ft/s).
- `crates/openmx/tests/machine_modifiers.rs::a_car_or_kiowa_already_out_takes_the_worlds_gravity_at_once`:
  after `Garage::modify` at 200% and 50%, the driven buggy's and flown Kiowa's gravity (and scale) are
  bit for bit what a fresh take on that set gives, and the stock ones again on the original set.
- The two "every knob at its default is bit-exact" tests still pass with every other knob moved; their
  "others" set now leaves out Gravity and Air drag, which reach these machines on purpose.

## Not verified / not 1:1

- The quad's and MX vs. ATV cars' gravity force is the read force x k, not `-(GravityScale x k) x Weight`
  worked out again as the stock cars' is: the gravity block keeps no weight. The two can differ by a
  rounding step at other k than powers of two. Harmless for an addition (100% is skipped); not
  checked against a file with a k-times `GravityScale`.
- The wrecked quad's and car's `WreckGravity` and the Kiowa's wreck gravity are not scaled, as the
  bike's, biplane's and P290's Kiowa's are not.
- The Kiowa that is wrecked when modify runs keeps the wreck's gravity force until its unwreck, which
  then uses the new scale; that path is not covered by a test.
- Not checked on Untamed's (SLUS-21701) quad and cars beyond them sharing the same load path; the
  tests ride SLUS-21104's packs only.

## Next

None.
