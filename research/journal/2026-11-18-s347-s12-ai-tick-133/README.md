# S347 — s12 ai tick 133

2026-11-18. Task: `./kit plan S347`.

C33's ai0 parted at tick 133 and ai1 at 148 in the s12e replay. Both were the clutch's launch ramp: the
original rearms it on the throttle *lever* (`+0x63c`), the port on the rate-limited throttle (`+0x658`).
An AI rider that pulls the clutch and opens the throttle in one frame rearms in the original and not in
the port. The ground step's gas is now the lever. ai0 132 -> 186, ai1 147 -> 200, ai2 206 -> 273.
