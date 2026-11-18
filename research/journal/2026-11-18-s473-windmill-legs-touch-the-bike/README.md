# S473 — windmill legs touch the bike

2026-11-18. Task: `./kit plan S473`.

On C37's B15 south3 windmill ride, the original's legs (bone collisions 7 and 8) touch the tumbling
wreck bike on the tick after the entry and the port's did not. The cause was the bones' bike search: the
port measured it from where the first sphere stood at the start of the last frame, the original from
where it last stood (a substep's travel later). With that fixed the port's legs touch the bike on the
same tick, and only there, and the bones stay within 0.2 ft of the original's to tick 11 (was 1.3).
Details in 1-windmill-legs-touch-the-bike-FINAL.md.
