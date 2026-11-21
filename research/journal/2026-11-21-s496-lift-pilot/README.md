# S496 — lift pilot

2026-11-21. Task: `./kit plan S496`.

A pilot that lifts the original's code close to line for line over its own memory layout (`crates/game-lift`),
instead of porting it into Rust types. The start gate's crossing test and the AI ground controller (with
its arc cosine and curve sine) are lifted and proved RAM-in/RAM-out against the original, with no wrong
bit. A 173-line bridge runs both inside the sim behind `--ai N`. With `--ai 1`, one AI bike laps
Supercross01 behind the gate drop, and the race matches the port's line for line. Lifting a function took
minutes, against 98–112 min for the port tasks, but the port had already done the finding. Waiting on the
user: whether to go on lifting, and the AGENT.md wording for game-lift. Details in 1-lift-pilot-PART.md.
