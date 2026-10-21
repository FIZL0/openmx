# P73 — original bindings

2026-10-21. Task: `./kit plan P73`.

The original's whole command→control binding table, read out of three riding save states and its only
builder (four presets, rebuilt per player preset and per vehicle, never per mode), is now
`game_sim::bindings`, and every ride command — bike, biplane, camera, reset, pause — reads through it.
What the table lacks is gone: no R2 front brake, no L3 balance, no reverse button; the camera button is
Select. The New layout carries exactly the original's commands plus the user-asked R2/L2 gas/brake.
Details and what is still not 1:1 in `1-original-bindings-*.md`.
