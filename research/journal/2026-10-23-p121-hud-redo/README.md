# P121 — hud redo

2026-10-23. Task: `./kit plan P121`.

The ride HUD redone from `docs/hud-concept.html` as vector shapes and our own type
(`crates/openmx/src/ride_hud.rs`): score bar, trick pop-up, DSEG7 readout or dial with the preload
meter, radar, prompts, marker menu and camera toast at the disc's 640x448 places scaled to each view,
one set of sprites for the GPU and the software path. `Overlay::Hud` is the new default; a settings
file from before it saying `original` moves to it once. The original plays a landed and a bailed combo
out the same orange (filmed in entry 2), so both colours are orange. Entry 2 carries entry 1 onto
main after its merge failed, wires V52's HUD scale and Minimap, has P126's Original switch bring
the ride HUD back when turned off, and fixes the pop-up's grow-in, shrink-out and combo total from
the film. What is unproven is listed in both entries.
