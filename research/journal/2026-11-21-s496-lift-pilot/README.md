# S496 — lift pilot

2026-11-21. Task: `./kit plan S496`.

A pilot that lifts the original's code close to line for line over its own memory layout
(`crates/game-lift`) instead of porting it into Rust types. Two functions are lifted and proved
RAM-in/RAM-out against the original with no wrong bit: the start gate's crossing test, and the AI ground
controller (with its arc cosine and curve sine). A 172-line bridge swaps them in for the sim's ports
behind `--ai N`. With `--ai 1`, one AI bike laps Supercross01 behind the gate drop (66.36 s). With
`--race-bot` added, every race line matches the port's run. Each lift took minutes, against 98–112 min for
the port tasks, but the port had already found and named everything.

Only part of the brief is done. The gate's drop, the route follower, avoid, and an arena-resident bike are
not lifted, so the bike still runs on game-sim. Waiting on the user: whether to go on lifting, and the
AGENT.md exemption for game-lift. Details in 1-lift-pilot-READY.md.
