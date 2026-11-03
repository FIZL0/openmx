# P264 — the cars on the Camera page

2026-11-03. Task: `./kit plan P264` (an S246 follow-up).

Before this, a driven car's camera (S246) was framed by the Camera page's one set, and the seat
kept no start preset for it: the camera button's pick in a car was not saved. Its Lookback Cam
had never been held to a capture. Now:

- each car's start preset is kept per seat in `settings.txt` (`camera_car_start`), and the next
  ride's car camera starts on it;
- the Camera page lists each car's presets, and a seat's own numbers or presets on a car are laid
  over that car's camera;
- the cars' Lookback Cam is proved against new captures of the original.

- `make_tape.py` is S149a's car drive with L2 held on four stretches. `capture.sh` plays it on the
  original and records `watch/car-camera.toml`. The captures are `context/capture/p264/{baja,buggy,truck}.csv`
  in the s1 worktree's context, 650 ticks each, with no gaps.
- `1-car-camera-presets-FINAL.md` covers what the original does, what changed, and how it was
  checked.
