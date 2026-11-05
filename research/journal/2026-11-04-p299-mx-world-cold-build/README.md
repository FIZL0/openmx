# P299 — mx world cold build

2026-11-04. Task: `./kit plan P299`.

MX Unleashed: WORLD (world_mxu) took minutes to build its joins the first time it was picked, which is
far too long on an iPhone. I measured it on the desktop. The build is not slow because of the joins it
keeps: it planned 30 times and solved the whole world ten times, and nine of those solves only found
"too steep, widen". Each disc world now starts where its last pass began (`crates/stitch/worlds/<id>.start.json`,
kept for its cache key), so it plans and solves once and writes the same bytes. Cold first ride: 181.6 s
wall / 1312 s CPU → 19.8 s / 208 s (release, same machine, quieter load), byte-identical joins for all
three worlds. The cache already survives launches and app updates (Documents). Streaming the far joins
and the store route were not needed for that cut; streaming is filed as P300 (later), and the phone's
own timing is a HUMAN.md step. Details: [1-mx-world-cold-build-FINAL.md](1-mx-world-cold-build-FINAL.md).
