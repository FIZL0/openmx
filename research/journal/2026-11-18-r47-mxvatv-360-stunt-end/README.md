# R47 — mxvatv 360 stunt end

2026-11-18. Task: `./kit plan R47`.

The port's 360 should not end at 1.5 s. The stunt clock that zeroes at the row's Duration runs only for a
bike the network drives. A local bike's trick step runs its own clock, which never zeroes, and while the
360's torque is applied the clock never ends the stunt. The 360 lasts until the 360 step clears the id
(landing, touch, yaw floor), holds phase 1 from 1.5 s, and restarts on a second pass past 35°. The port
instead leaves the 360 when it comes round. Filed as S467. Proved by running SLUS-21104's trick step on
state04 (`tests/mxvatv_stunt_end.rs`, 4 tests). Details in 1-mxvatv-360-stunt-end-FINAL.md.
