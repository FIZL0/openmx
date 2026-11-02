# MD60a — C12 re-landed on openmx-mods main

2026-11-02. Task: `./kit plan MD60a`.

openmx-mods C12 (stitch's wrapped field) is on openmx-mods main as d9e38b8, merged over MD57's relief.
The relief now reads its distances across the wrap too. A world_mxu built from that tree reads 0.700 ft
steepest on `--seams` and 0.100 ft across the wrap. The installed world reads 7.977 ft. Nothing is
installed yet: MD60 rebuilds and installs the five worlds. openmx's own `crates/stitch` needs the same
wrapped field (filed as MD60b). Details in 1-c12-reland-FINAL.md.
