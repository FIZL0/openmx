# P65 — menu sound

2026-10-24. Task: `./kit plan P65`.

The front end was silent. It now plays the original's own menu loop (`UI\UI_001`, proved by the
Video Capture's sound against the disc's clip) and the five clicks from `ui.aud`'s
`[UIResponseEmitter]`. The clicks are decided by comparing the menu before and after each press, on
every screen, the pause included. The loop and the ride's songs cross-fade as a ride starts and ends.
It is under the music and Menu FX sliders. Details in 1-menu-sound-FINAL.md.
