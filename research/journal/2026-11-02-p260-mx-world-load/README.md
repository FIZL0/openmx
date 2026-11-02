# P260 — mx world load

2026-11-02. Task: `./kit plan P260`.

The user said: "MX WORLD is not loading and when it freezes i cant cancel or close the game". When no
installed world shadows it, MX Unleashed: WORLD is built off the disc on its first ride, and its joins
were stitched in one long step that checked for nothing. That was 132 s in a release build, and the
loading screen said "Scene" the whole time with the bar at nought. Back or the close button set the
cancel flag, and then the window's thread blocked in `join` until the stitch was done. The window froze
for up to two minutes, which is the freeze the user described. Now the stitch reports each pass and
piece under the bar, and it answers a cancel between steps (≤ 2 s, usually under 0.1 s). While a stopped
load winds down, the window keeps drawing and hears its close. A cold world_mxu builds in 92 s, down from
132 s, because its tiles are now cut side by side. An installed world reports its tile and join reads
too.
