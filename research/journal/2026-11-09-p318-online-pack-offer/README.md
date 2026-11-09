# P318 — online pack offer

2026-11-09. Task: `./kit plan P318`.

Online rides offer flier packs' planes only where every seat's machine holds the pack, and grey them
with who lacks it. File 1 covers the handshake and the room: a joiner lacking a ridden pack is
refused, and one lacking an unridden pack narrows the offer. File 2 covers the ride. A seat's pick is
a pad byte (`net::FLIER`), applied on the tick its switch lands, from templates built off each
machine's own files before tick 0. No files are sent between players. Two `--peer` shells prove it:
with the pack on one side only, the pick is greyed and refused on both; with it on both, the guest
flies the Spitfire and the checksums match. The lobby Garage side is split off as P318a. Details are
in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
