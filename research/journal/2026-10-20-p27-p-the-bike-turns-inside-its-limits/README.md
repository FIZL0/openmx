# P27 — p the bike turns inside its limits

2026-10-20. Task: `./kit plan P27`.

P27 gave the bike chassis the original's angular limits. `ride.rs` used to build it with 100 rad/s on every axis.
Now it gets the rigid body constructor's ±25 rad/s windows, with the `.phy` file's `YawRateLimit` on the yaw axis,
and the constructor's angular speed limit of 1875 (the sum of the three windows' squares).

The running original's chassis holds windows (25, 3, 25) and a limit of 1875 (`capture/limits.csv`). The port's
`race125p` chassis matches that bit for bit. The platform hashes moved because the state hash includes these values;
the trajectories did not change. Details in `1-…-FINAL.md`.
