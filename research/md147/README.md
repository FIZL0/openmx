# MD147 — Untamed's race quad captures

Journal: research/journal/2026-11-15-md147-untamed-atv-capture. `KIT_CONFIG` must point at a SLUS-21701
kit.toml (kept-context/md147/slus21701-kit.toml); PCSX2 needs `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
QT_ACCESSIBILITY=1` (and `WAYLAND_DISPLAY`) at start for the AT-SPI tape playback.

- start.pad (+ research/md160/step.sh): from MD160's vehicle-atv.p2s, Start Event → spawn.p2s. idle.pad: a look.
- air.pad (step.sh): jump.pad cut at tick 155 → air.p2s.
- poke.sh: write the quad's surface hazard table and save → hazard-time.p2s, hazard-on.p2s.
- play.sh + quad.toml: one ride (throttle, steer, jump, land .pad) → context/capture/md147/NAME.csv.
- ram.py, surfaces.py: a state's quads and hazard table. summary.py, dups.py, rows.py, same.py: read the rides.
- keep.py: copy states, RAM images, captures and tapes into kept-context/md147.
