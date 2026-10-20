# P28 — the captures' speeds in the port's units

2026-10-20. Task: `./kit plan P28`.

Every speed `crates/game-sim/tests/ride.rs` read off a capture's `bike_vel` columns was the actor's
copy of the chassis velocity, which is 0.8 times the chassis velocity (P21, P22), and the file compared
those numbers with the port's `chassis.speed`. All of them now go through one divisor,
`BIKE_VEL_PER_BODY = 0.8`. In chassis units the constants are CAPTURED_TOP_SPEED 94.73 (was 75.78),
CAPTURED_SLOWING 88.6 (was 70.9) and ORIGINAL_LAUNCH (11.0, 42.75) (was 8.0, 32.0). The hand-written
bike's drag is re-tuned from 0.06 to 0.037, which puts its terminal speed at 94.73 against the
recording's 94.72. The flip ramp moves from 100 ft to 108 ft so the lip comes back under the stick
script's ticks. The bike does not make the whole restated launch (87% and 84%). That bound is now an
explicit `LAUNCH_SHARE`, and the gap is written up in the numbered file. `crates/openmx/tests/ride.rs`
and `ride-minute.pad` carry the same bike and constants. `./kit check` passes.
