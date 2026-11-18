# P370a — take height and accel

2026-11-18. Task: `./kit plan P370a`.

MX Unleashed's baja (and the monster truck from 180%) taken on the gas with Car acceleration over 100%
was held for about a second. The cause was the shell's take, not the knob: it stood each car a tire radius
too high (the tire's bottom counted as two radii under its node), so the car fell 3-4 ft, its rear wheels
spun up in the air and swung on landing. Fixed in `CarRide::rest_height`; every knob value now gets away.
