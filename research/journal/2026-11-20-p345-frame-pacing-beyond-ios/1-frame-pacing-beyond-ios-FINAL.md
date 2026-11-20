# 1 — frame pacing beyond iOS (FINAL)

Task P345. 2026-11-20.

## What I did

- `--timings` lines a second now end with `, frame ends A-B ms apart, pictures C-D ms apart`: the least
  and most step between two frame ends (the wall clock read after the present), and between the
  moments the pictures were posed at. The phone's line keeps its `pictures` field only.
- Measured a 20 s free09 ride (`--timings --size 1280x720`) on this machine's desktop: AMD Radeon 8060S,
  RADV, Vulkan, Fifo, at 60 Hz.
  - The real monitor was off (Hyprland had no outputs; a window there waited ~1 s a present), so I added
    a GPU-composited headless output for the runs (`hyprctl output create headless P345`) and removed it
    after. Removing the last output leaves Hyprland's own `FALLBACK` output until a monitor comes back.
  - weston `--backend=headless` is no use for this: it renders on llvmpipe GL at ~25 fps.
- Before: frame ends 15.3-18.6 ms apart every second on Wayland, 15.6-17.8 on X11 (Xwayland). The wait
  is in the draw's surface acquire (`waited` ~15 ms a frame), after the scene was blended: the same
  order as iOS, with about half iOS's ±4 ms wander.
- Wayland windows had no refresh: winit's `current_monitor()` is `None` when the window opens there, so
  `GpuWindow::refresh` stayed `None` (and `--render-scale auto` took 60). `GpuWindow::refresh_now`
  (gpuwin.rs) asks again once the window is on an output; `Screen::refresh` uses it on the desktop.
- After, with `Refreshes` on: pictures 16.6-16.7 ms apart every second on both, the frame ends unchanged.
- main.rs now poses every native window's picture through `picture_moment` (desktop, Android, iOS). The
  browser keeps the frame's end: its loop waits on the page's frame, not a surface, and says no refresh.
  As on iOS, only with a present that waits (Fifo) and no `--draw-fps`, and never on a steady clock.

## Android

Not measured. The emulator renders on SwiftShader at ~0.7 fps, which says nothing about pacing, and
there is no phone here. Android shares this loop (`android_so.rs` is a symlink to main.rs) and its
`android_gpu.rs` draw takes the surface the same way, after the blend, so it gets the same treatment.
Its refresh comes from winit's `current_monitor()` at open; where that is `None` the moment is the frame
end, as before. The phone's `pictures A-B ms apart` field shows it in the next Android log.

## How it was proved

- `android_frame_pacing_keeps_the_pictures_on_the_refresh` (main.rs): 600 frame ends with the
  desktop's measured spread (a refresh plus 0-3 ms of work) go through `picture_moment`. Steps after
  the first second are 16.5-16.85 ms. With no refresh, or on a steady clock, the moment is the frame end.
- `./kit check -- -p openmx --bin openmx pace`: the 18 pace tests pass.
- The Android example compiles (`cargo check --release -p openmx --example openmx --features android
  --target x86_64-linux-android`, with the NDK's clang as `CC_x86_64_linux_android`).
- Not checked: the wasm build (no wasm32 target on this toolchain) and the iOS build.
- Not measured: a real monitor (it was off), refreshes above 60 Hz (this display is 60 only).
