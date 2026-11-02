# S236 — sim tick deck

2026-11-01. Task: `./kit plan S236`.

The Deck's sim hitches on Supercross01[Quadzilla] (577 props, 23 path cars) came from the plane's and
the ride's searches building their broad-phase world afresh for every search, three or more a tick,
each filing every prop and plant into a new quadtree. The searches now keep their world between
searches, and when only some props moved (the path cars, every tick) they bring the kept one up to
date: the moved props' reach and centre are re-set, and the tree is rolled back to just before the
first moved prop was filed and the rest re-filed. That gives the same world, bit for bit, as a fresh
build. Locally on the Deck's world (free09 / first_recording.p2m2, single thread, 2400 frames) sim went
from 2.05 to 1.12 ms a tick, and on sx01 from 1.39 to 0.71 ms, with ride hashes unchanged. The Deck
itself was not measured. Details in 1-kept-worlds-FINAL.md.
