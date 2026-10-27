# P146 — plane touchdown sound

2026-10-27. Task: `./kit plan P146`.

The biplane was silent on its wheels. The original plays `machine.aud`'s `[biplaneTireTouchdown]`
(`Impact\plane_tire`) through its machine emitter on every tick the plane's grounded flag rises: a
landing, a bounce, a wreck tick with a wheel in it, a put-back. The level runs from 40 at 60 mph to 100 at
150 mph of the body's speed, and it is heard from 30 ft to 150 ft. The port now plays it per seat. A
capture of `biplane.p2m2`'s playback gives five touchdowns, and replayed through the port's sound they
start on the original's ticks at the original's levels. The pick also draws one `rand()` in the
original, and the port's plane now draws it too. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).
