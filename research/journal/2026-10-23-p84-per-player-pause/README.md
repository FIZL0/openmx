# P84 — per player pause

2026-10-23. Task: `./kit plan P84`.

Split-screen pause used to stop everyone. Now each player's Start opens a pause in their own view
only, driven by their own pad, and the other players ride on. Locally the menu keeps one menu state
per seat. Online the pause is a bit in the seat's shell byte, so it is a rollback input on the agreed
tick. A paused rider is frozen where they are and drawn at the put-back ghost's half alpha. Online, a
paused rider is also out of rider contact. Tested in tests/menu.rs and tests/netplay.rs. Not yet seen
on screen with two real pads. Details in 1-per-player-pause-FINAL.md.
