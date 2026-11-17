# R36 — mxvatv car landing on the gas

2026-11-16. Task: `./kit plan R36`.

Checked the shell's stuck MX vs. ATV buggy (150% acceleration, taken a foot up, full gas: the rear
wheels swing ±27 rad/s for a second) against SLUS-21104's own car frame, run in the interpreter
beside the port from MD39a's buggy image with the car lifted and gassed. The original does the same,
word for word, whenever the car's launch torque scale is still fresh, as every newly made car's is.
The wheels' spin on landing is not the cause, so dropping the car with its wheels still would not
help. New test `a_buggy_dropped_on_the_gas_is_the_ports`; the knob question is filed as P355.
Details in `1-mxvatv-car-landing-on-the-gas-FINAL.md`.
