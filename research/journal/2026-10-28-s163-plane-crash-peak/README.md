# S163 — plane crash peak

2026-10-28. Task: `./kit plan S163`.

The brief said the port's plane crash clanks late because its tires take the hit a tick before the wreck.
That came from the port's own synthetic 45° dive. I replayed the original's own nose-in from
biplane.p2m2 through the port's Flight instead, seeded from C7's acceptance capture and compared with
P164's gates. It wrecks on the game's tick, with the record touching and the game's shock peak (15.95
against 15.97), so the clank is heard on the wreck tick as in the game. No change to flight.rs was
needed. The second crash in the same capture is bit-exact up to its wreck tick, and then parts from the
game (S204). Details in 1-plane-crash-peak-FINAL.md.
