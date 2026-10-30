# 1 — compositor gtt (FINAL)

Task K16. 2026-10-30.

## What I did

- **Looked for what grew Hyprland's GTT.** `~/.local/state/gtt-watch.log` only began at 08:56 on 10-30,
  minutes before the reboot, so the ten days of growth have no time series to set against the runner's log.
  The previous boot's journal has none of Hyprland's own output either. What it does have: syncthing-gtk
  logging `gtk_widget_get_scale_factor` assertions at a flat ~375 an hour for all ten days (its tray icon
  belongs to quickshell, not the compositor, so I did not chase it), four pcsx2-qt core dumps, and Hyprland's
  own crash at 08:57:16 (the reboot).
- **Measured each suspect against Hyprland's GTT** (per-pid fdinfo, `drm-memory-gtt` once per
  `drm-client-id`, MiB):
  - Ten `openmx --seconds 2` windows on X11 (`DISPLAY=:0`, GPU path), ten on Wayland, eight on the software
    path (no DISPLAY, so it falls back to `:0`): +0 to +2 MiB in all.
  - PCSX2 copy start/stop, four cycles: +2–3 MiB each, and it does not come back. Running ~2 minutes: flat.
  - grim with PCSX2 running: +8 MiB once, then flat. It did not hang (platform_.py's note says it does once
    a game runs, on 0.56.2).
  - Other slots' gates and headless chromium running beside me for 20 minutes (the sampler, 09:38–09:58):
    flat apart from my own runs.
- **The display was off all morning.** `hyprctl monitors all` was empty, so Hyprland drew nothing, which
  alone could explain a quiet morning. I added a headless output (`hyprctl output create headless K16TEST`)
  and reran the window, PCSX2 and grim tests on it. The session is locked, so the output showed the lock
  screen, as on most of the ten days. Same result: nothing near the ~400 MiB an hour that 105 GiB over ten
  days averages. Then I removed the output, and Hyprland put up its own `FALLBACK` output as it does with no
  monitor; it goes when the monitor comes back.
- **So I could not name the cause** (see Not verified). This task keeps the gate's windows off the
  desktop either way, and gives the next look a log that can name the cause:
  - `ps2kit/platform_.py` `PrivateDisplay`: a headless weston (`--backend=headless --renderer=gl
    --shell=kiosk --idle-time=0`) with a rootful Xwayland in it (`-geometry 1920x1080`, so no X window
    manager is needed; weston's own `--xwayland` WM asserted in `weston_wm_handle_map_request` under both
    kiosk and desktop shells). It yields `DISPLAY=:N` and drops `WAYLAND_DISPLAY`, so every window goes
    to X11 there: the GPU window (winit X11, Vulkan through DRI3) and the software one (the shell's own
    X11 socket, which defaults to `:0` without DISPLAY). Both end with the block.
  - `ps2kit/checks.py` `_run`: every `./kit check` / `./kit done` (and so `./kit check --all` and the
    runner's merge gate) runs inside one, when gate.toml `[display] private = true`. It notes under the
    verdict when no private display could start, and when the desktop compositor's GTT grew more than
    `grew_mib` (1024) over the run. That note is the task's `t:` line, checked on every gate from now on.
  - `ps2kit/runner.toml` `before_start = "python3 -m ps2kit.platform_ room 16"`: agent-runner starts no
    new slot while MemAvailable is under 16 GiB (it logs once that it is holding). Running slots carry on.
  - Weston is not installed and a run has no sudo, so Arch's `weston-15.0.1-3` package is unpacked into
    `~/.local/opt/weston-15.0.1-3` behind a `~/.local/bin/weston` wrapper (LD_LIBRARY_PATH +
    WESTON_MODULE_MAP). Only `colord` of its dependencies is missing, and that is a plugin weston never
    loads here. plan/HUMAN.md asks for `pacman -S weston` to replace it.
  - `~/.local/bin/gtt-watch` (outside the repo) now also logs exact MiB, monitors on, and how many
    pcsx2-qt, openmx, chromium, weston and grim processes are running, so a climb between two lines names
    its suspects. The old script is at `~/.local/state/gtt-watch.prev-k16`.
- **Broke the desktop's `:0` for a minute, then fixed it.** My first PrivateDisplay used `Xwayland
  -displayfd`, which walks up from `:0`. On `:0`, Xtrans unlinked the desktop's `/tmp/.X11-unix/X0` before
  giving up on that number. Hyprland's Xwayland kept running, but nothing new could reach `:0` by path. Any
  other slot's window test that opened on `:0` between about 09:50 and 09:51 would have failed to connect.
  Hyprland also listens on `/tmp/.X11-unix/X0_` (same server), so a symlink `X0 -> X0_` put `:0` back
  (checked: a GPU window opened on it). PrivateDisplay now never uses -displayfd. It picks a number from
  `:200` up with neither lock nor socket, and Xwayland takes its lock before it touches a socket. It also
  clears a lock in that range whose pid is dead.
- **The first whole gate red one test:** `loading::a_cancelled_load_goes_back_to_the_pick_with_nothing_running`
  (`no "loading: held at models" line`; the cancel came at Props). Alone on the private display it failed 1
  in 3 at load 35. Frame pacing is the same there as on `:0` (`--timings`: 235 against 232 front-end frames in
  4 s), so the display was not the cause. The test raced: OPENMX_LOAD_CANCEL pressed Back after ten drawn
  frames whether or not the build had reached the stage OPENMX_LOAD_HOLD holds at, and a loaded machine's
  build had not. The hook now presses only once the load is held when a hold is set
  (`crates/openmx/src/main.rs`, the loading frame; doc on `loading::CANCEL`). This test is its only user.
  Then: 3 of 3 passed alone.
- The first `./kit done` (10:25-10:48, load 40+) red three frame-budget tests, each also red on its
  machine-held rerun: `gpu::a_4k_frame_fits_in_a_120hz_frame` (9.12 ms), ecosystem's densest view
  (2.95 ms against 2.18), and one_fps's headless ride. All three are headless GPU timings that open no window,
  and the gate an hour earlier passed them on the same private display. Rerun at load 25 (10:49), each passed
  through `./kit check`, private display included. Load, then, not the display (memory load-flakes-under-a-busy-gate).
- Followed up as K17: read the richer log after 3+ days, and move the PCSX2 copies to a private display
  too if they turn out to be the cause.

Nothing in opentennis does this (its gate has no windowed tests); nothing taken.

## What the original does

Not applicable: kit tooling.

## How it was proved

- `./kit check -- -p openmx --test original_mode` from a shell with neither DISPLAY nor WAYLAND_DISPLAY:
  29 passed. Before, it failed in that shell with `no "graphics:" line` (memory gate-needs-a-display): the GPU
  window came up on the private display.
- `checks._run` on `env | grep DISPLAY` from a shell with `DISPLAY=:0 WAYLAND_DISPLAY=wayland-1`: the child
  saw `DISPLAY=:200` and no WAYLAND_DISPLAY; the socket was gone afterwards.
- Two PrivateDisplays at once took `:200` and `:201`, both drew a GPU window, and both cleaned up; `:0`'s
  socket was untouched.
- `python3 -m ps2kit.platform_ room 16` exits 0 at 101 GiB available; `room 500` exits 1 with the reason.
- The whole gate, `./kit check --all` from a shell with no DISPLAY, 09:56-10:13: Hyprland's GTT was 20 MiB
  before and 18 MiB after (Xwayland's 22 → 32 MiB). That is the task's `t:` line, met. `./kit done`'s gate
  reran it.

## Not verified / not 1:1

- **What grows Hyprland's GTT is not found.** Nothing I could run this morning reproduced it. Everything
  was measured with the monitor off, or on a headless output under the lock screen. The ten days had a
  real DRM output (direct scanout, DPMS, the user at the desk), and I can neither switch that on nor
  unlock the session from a run. gtt-watch has no history from before 08:56 10-30, so there was nothing to
  line up with the runner's log. K17 reads the richer log once days of it exist.
- **PCSX2 copies still draw on Hyprland.** They leave +2–3 MiB per start/stop (measured, four cycles).
  That is a small leak: it would take ~35,000 starts to make 105 GiB, but it is not zero. Moving them is
  K17's, because `./kit shot`'s grim path and the AT-SPI menu driving both go through the desktop.
- Anything an agent runs by hand outside `./kit check` (`openmx --shot`, a windowed ride) still opens on
  whatever display its shell has, `:0` by the software path's default. Only the gate moved.
- The memory hold (`before_start`) is not tested against a live agent-runner at under 16 GiB. I could
  not starve the machine, and the hook's logic is agent-runner's (`before_start_fails`). A runner running
  now reads it only after it restarts, since `./kit run` writes agent-runner.toml at start.
- Weston repaints a headless output on its own timer, not a monitor's vblank. The `[alone]` frame-budget
  tests (one_fps, ecosystem) and `[flaky]` pace tests now time against that. The full gate passing is the
  only check of it.
- With no weston installed, the gate silently changes nothing except a note, by design. A machine without
  weston keeps the old behaviour.

## Next

Done when `./kit done` lands; K17 is the follow-up.
