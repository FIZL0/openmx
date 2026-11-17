# MD180 — ride respawn proved resets

2026-11-16. Task: `./kit plan MD180`.

The MX Unleashed Ride's respawn now resets its tires and engine with MD154's proved `tire_reset` /
`engine_reset`, so the tire vectors get `w` 1 as in SLUS-21104 instead of `w` 0. Ride tests rechecked.
