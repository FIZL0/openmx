# P345 — frame pacing beyond iOS

2026-11-20. Task: `./kit plan P345`.

The desktop's frame ends wander ±1-2 ms around the 60 Hz refresh (X11 and Wayland, Vulkan Fifo), so
`pace::Refreshes` (P340's iOS fix) now poses the pictures on every native window: desktop, Android and
iOS. Wayland windows also learn the refresh now. Android was not measured on a device. Details in
`1-frame-pacing-beyond-ios-FINAL.md`.
