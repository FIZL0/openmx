# P241 — world uploads

2026-11-02. Task: `./kit plan P241`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

Worlds now work through the mod store. Upload lists the player's own worlds but not the disc-built
world_mxu ones or the stitch cache. It refuses a world over the caps before packing, in plain words. It
tells MB progress while packing, checking and sending, and resends a cut LFS part. Browse marks a world
"World · <size>" on Tracks, and the index lists it as a world. Install carries a cut download on from where
it stopped (Range), and a kept partial resumes on the next Install. The installed world reads and rides
from Free Ride. The browser lists worlds but says it can't ride or download one yet.

The proof: tests against the fake Hub, plus the real world_untamed (781 MB, packed to a 316 MB zip) sent
up to the fake Hub and back into a fresh mods folder, then ridden headless. The one thing left is the
public upload to huggingface.co, which is the user's call (plan/HUMAN.md). It is READY for that; details
in `1-world-uploads-READY.md`.
