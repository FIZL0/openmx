# P85 — shootoff sound

2026-10-23. Task: `./kit plan P85`.

When the original throws a rider off the map's edge (the cannonball), a sound child on the bike
fades the music to nothing over the last 200 ft before the margin while a rumble rises. At the throw
it plays a streamed 8 s boom, and it brings the music back whole the tick after the put-back. Found in
the decompilation, captured twice over PINE (once with Video Capture's sound), and ported to the
shell's audio per rider. Two PINE captures replay bit for bit, and one fixture is in the tests. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).
