# P324 — iphone frame rate

2026-11-05. Task: `./kit plan P324`.

The iPhone ride drops frames (57–59 fps) and the pause sits at 40 fps while memory climbs +130–170 MB. Step 1
is done: on iOS and Android the line a second now says where the frame goes (the stages, the surface wait, the
widest gap, sprites, and GPU time). The iOS build compiles. Step 4 is done: the desktop holds 60 in the ride
and in the pause, before and after, with ~1 ms of GPU a frame, and its RSS does not jump. Steps 2 and 3 (the
fixes) wait on a phone log from this build, because nothing on the desktop shows the cost. The pattern in the
log so far: the 40 fps pauses all fell between the controller connecting and a trip to the background.
Riding misses are about one frame a second; they follow neither the ground builds nor the triangle count.
READY: the human's step is in plan/HUMAN.md, "Where the iPhone's frame goes (P324)".
