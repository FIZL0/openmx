# K82 — link failure not flaky

2026-11-20. Task: `./kit plan K82`.

K82 was filed as "make car_modifiers robust", but car_modifiers never ran: nine test binaries failed to
link in P383's merge gate, and the gate's short messages had dropped the linker's own words. Part 1 laid
out options; the person picked A. Part 2 built it: the gate now logs the linker's stderr, relinks once
when only the link step failed, and a second failure gets its own `LINK FAILED` headline.
