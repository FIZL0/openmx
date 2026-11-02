# MD60 — stitched worlds reinstall

2026-11-02. Task: `./kit plan MD60`.

MD60 was to rebuild the five stitched worlds from openmx-mods main once C12 (wrapped worlds) merged
there. C12 has still not merged. I also found that MD57 (11-02) has already rebuilt and installed all
five from openmx-mods main without C12. That brought the steep wrap back: the `--seams` steepest step
went from 0.4–0.7 ft to 8–18 ft at the wrap strips. task/C12 conflicts with MD57's relief code in
`tools/stitch/src/build.rs`, so C12 cannot merge as it is. I blocked this task on that. Nothing was
installed or committed in openmx-mods. Details in 1-stitched-worlds-reinstall-PART.md.
