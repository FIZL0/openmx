# S220 — the Kiowa's rotor, aerodynamics and force listeners

2026-11-02. Task: `./kit plan S220`.

I ported the rest of the Kiowa's per-substep flight after S148's controller into `game_sim::kiowa`.
Each part is bit-exact by oracle on S148's live helicopter (S148's Kiowa state in `context/states/`):
- `euler_quat`: the tilt's Euler-to-quaternion.
- `Rotor::step`: the main rotor's thrust, tilted by the cyclic, and `MaxRotorMoment` times the sticks.
- `Aero::step`: the body's aerodynamics, with the speed fade and the slow-speed drag.
- `Kiowa::force` and `Kiowa::preforce`: the force listener (controller, wrapper force, rotor, aero,
  skipped while wrecked) and the preforce listener (the shock-peak clear).

Two of S148's labels were wrong: preforce and force are swapped, and the aerodynamics' +0x100 is
`HighSpeedAero`, not `LowSpeedAero` (`Flight::low_speed_aero` is now `high_speed_aero`). The brief's
"altitude fade" does not exist: no rotor step reads the altitudes.

The measure listener went to S220a and the pad input step to S220b. Details in
`1-kiowa-rotor-FINAL.md`.
