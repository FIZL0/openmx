# D7 — d7 freeride bike class

2026-10-15. Task: `./kit plan D7`.

A freeride bike is a 125 of the Race tier, not the 250 the port had assumed: the vehicle build loader names
its two physics files straight out of the event's class and tier fields, and nothing on the freeride path
writes them, so a freeride world rides on the event constructor's defaults — class 1 (125), tier 0 (Race).
Measured twice, in a world the original itself had loaded (the human's `Free09` recording: those two fields,
and `Bike\Race125A.phy` / `Bike\Race125P.phy` as the only bike physics names resolved in 32 MiB) and in a
scripted `Free06` boot that set the mode but deliberately left the two fields alone. The old comment's other
claim — that the freeride archive ships only the 250 pair of each tier — came from reading the track's own
archive: all sixteen names are on the disc, in the bike *model* archives, so the disc never narrowed the
choice at all. Details in the numbered files; the newest one's suffix says whose move it is
(FINAL · PART · READY).
