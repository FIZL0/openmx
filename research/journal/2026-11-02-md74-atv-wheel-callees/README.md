# MD74 — the ATV wheel's contact velocity and hinge force

2026-11-02. Task: `./kit plan MD74`.

SLUS-21104's tire forces call two functions that MD37b left hooked. Both are now ported beside
`atv_wheel::suspension` and `tire_forces`, as `atv_wheel::contact_velocity` and `atv_wheel::hinge_force`,
and `atv_wheel::run` answers the tire forces' calls with them. In `tests/atv_wheel.rs`, two new oracles
run them on their own off MD37a's `atv-r-spawn.ee.bin`. The tire-forces oracle now runs the original's
own code for both callees and lays out what the wheel stands on in RAM. Details in the numbered file.
