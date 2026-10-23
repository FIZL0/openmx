# P121 — hud redo

2026-10-23. Task: `./kit plan P121`.

The ride HUD redone from `docs/hud-concept.html` as vector shapes and our own type
(`crates/openmx/src/ride_hud.rs`): score bar, trick pop-up, DSEG7 readout or dial with the preload
meter, radar, prompts, marker menu and camera toast at the disc's 640x448 places scaled to each view,
one set of sprites for the GPU and the software path. `Overlay::Hud` is the new default; a settings
file from before it saying `original` moves to it once. The original plays a landed and a bailed combo
out the same orange, so both colours are orange. Done; what is unproven is listed in 1-hud-redo.
