# B7 — the truck strike's thrown velocity: the cab is a presampled car

2026-11-02. Task: `./kit plan B7`.

S207's truck row (free09's trailer cab into a parked bike) wrecked on the original's tick with its
reason 2, but threw the bike at (8.32, −1.86, 11.38) against the original's (8.38, −1.53, 11.77).
None of the strike code was at fault: the mover contact velocity was, because the cab was in the wrong
place. The replay's car seeding (`SaveState::traffic_of`, tests/replay.rs) ignored the cars'
`PresampleTerrain` switch, so free09's trailers rode the live ground query and its normal rather than
the state's presampled keys. With them seeded as presampled cars, the cab's pose matches the original's
bit for bit on every tick, and the strike tick's velocity, rates, quat, cofg and g_peak agree to the bit.
The row still parts on 105, but only by the ulp in origin.x that every wreck tick shows (filed as S261).
The same fix makes S219's trailer row agree over its whole capture (it parted on 45). Details in the
numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
