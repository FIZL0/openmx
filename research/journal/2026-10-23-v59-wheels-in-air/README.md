# V59 — wheels in air

2026-10-23. Task: `./kit plan V59`.

In the air the bike's wheels strobed: a 36-spoke wheel spinning at 52–90 rad/s (up to about 3 rad, close to
half a turn, a 30 Hz tick) was drawn razor-sharp. Its spokes stood still or crawled, and between two ticks
the blend could turn it backwards. They now turn by at most 1.2 rad per tick (`Spin`), and past about 15 mph
each spinning wheel is drawn with eight see-through copies trailing behind it (`Chassis::blur_copies`, GPU
path), so it reads as a spinning wheel. The droop suspect was checked and is not a fault: at full droop both
tyres sit on their axles and the front tyre clears the fender by 0.6 ft. A hard landing turned up something
else: at a front compression of 1.0 the tyre comes 0.1 ft up into the fender. That is a landing, not the
air, so it went to V60. `openmx --wheels original` draws the old way. Details in `1-wheels-in-air-FINAL.md`.
