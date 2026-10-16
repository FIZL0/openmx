# S3 — s3 movement

2026-10-15. Task: `./kit plan S3`.

Movement on the ground: the bike's acceleration, friction, top speed and turning, in the original's
arithmetic and its update order. The spine of `bike_ground_step` is ported and proved — the pose and
how far the stick moves the centre of gravity, the steering lock fading out with speed, the donut's
roll, the suspension rates, the gearbox, the clutch's launch ramp, the drag that sets the top speed,
the rev limiter, and the substep loop that is the order the whole ground frame runs in. The
twenty-one callees it drives are not ported; the step returns them as an ordered list instead, so the
order is proved where the callee is not, and that list is the seam they come in behind. Proved against
the original over 2,000 generated frames, a 240-frame scripted ride, and four of its pieces against
their own functions. The largest omission is the engine's torque curve: this port shapes the torque
but does not yet produce it.
