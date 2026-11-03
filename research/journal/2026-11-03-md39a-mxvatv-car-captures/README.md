# MD39a — mxvatv car captures

2026-11-03. Task: `./kit plan MD39a`.

MX vs. ATV's five cars (SLUS-21104), each driven from its Desert Oasis Free Ride spawn in my own PCSX2 copy
(s1) by one generated tape: gas, a right turn, left and right steer, two or more jumps, a brake to a stop,
gas, a hand-brake turn. Each drive has a save state and RAM image at its start and a csv of 730 ticks with
the car object, its body, engine and four tires (raw and decoded). The data is git-ignored and lives in s1's
`context/`; the scripts are in `research/md39a/`, the checks in `crates/game-sim/tests/mxvatv_car_captures.rs`.
The car's input words, read off the drives, are in research/oracle.txt `# MD39a`.
