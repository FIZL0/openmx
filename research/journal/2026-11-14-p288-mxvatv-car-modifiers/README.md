# P288 — mxvatv car modifiers

2026-11-14. Task: `./kit plan P288`.

Game Modifiers' MX vs. ATV Cars category: four knobs of its own (top speed, acceleration, grip,
suspension) on the five MX vs. ATV cars' loaded `.phy`, the stock cars' four applied to the same fields,
untouched at 100%. The category shows only while the car addition is on (settings' `mxvatv_car`, or a ride
under the pause that has one), and it is held online. Proved on the buggy off the pack: bit-exact at the
defaults, and each knob moves its fields and the ride. A fix came with it: the cars' top-speed knob now
scales EngineBraking too, for the stock cars as well. Details are in 1-mxvatv-car-modifiers-FINAL.md.
