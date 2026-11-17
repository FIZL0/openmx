# MD159 — mxvatv cars shell replay

2026-11-16. Task: `./kit plan MD159`.

MD39b's five car drives fed to the shell's own MX vs. ATV car (`MxvatvCarRide`), seeded once from each
drive's RAM image and then carried for 730 frames on its own input step and the Ride's chain, every
compared word equal to the interpreter's every frame (game-sim tests/mxvatv_car_frame/shell.rs). The
collision record's touch and contacts are laid in from the machine as world answers; doing so showed the
shell's Ride never sets the pieces' copy of the touch word (filed MD175).
