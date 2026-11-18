# S318 — race end hand over

2026-11-15. Task: `./kit plan S318`.

Hand the race's end to the results menu with the real cards, trackers, jumps and answers. It waited
on S301 and S319; once they landed, the shell's hand-over already did all of it, so this task added
the proof: a test in `crates/openmx/tests/menu.rs` that rides sx01 to the Over and checks the board.
Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
