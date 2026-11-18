# 3 — untamed monster (FINAL)

Task MD81c. 2026-11-18. Reopened because MD161a and MD161b landed.

## What I did

- Checked that everything MD81c asked for is now in, through its splits:
  - Static diff of SLUS-21701's step against SLUS-21104's: entry 1 (`research/untamed-machines.md`).
  - Capture: MD160 (spawn, air, flipped images and the throttle, steer, jump and wreck drives in
    `context/untamed/md160`).
  - Port behind `atv_car::Load::untamed`, off by default: MD161 (per-time damping, yaw hold by compares),
    MD81d (preload cue, clutch, hazard clock), MD161d (denormal compares).
  - Replay: MD161a, then MD234 (ground from Rocksbury's terrain, noise from the port).
  - Pack: MD161b. Its openmx-mods commit is on openmx-mods main as ca82741 (MD244 landed it;
    `git cherry -v origin/main origin/openmx-MD161b` shows every commit there). This checkout's
    `mods/machines/untamed_cars` lists the monster truck.
- Ran the tests that prove it, all green:
  - `./kit check -- -p game-sim --test mxvatv_car_frame untamed`: 20 tests, the four drives' capture checks
    and frame/ride beside-replays included.
  - `./kit check -- -p game-sim --test untamed_truck`: 3 tests (monstertruck.phy's load, the damping oracle).
  - `./kit check -- -p openmx --test untamed_monster`: the shell rides the pack's truck on Untamed's step.
- No code changed.

## How it was proved

By the tests above, which earlier tasks wrote: MD161a's beside-replay of MD160's drives against SLUS-21701
run in the interpreter, MD161's oracle, MD161b's shell ride. "Pack off, replay hashes unchanged" is the
gate's existing replay tests, run by `./kit done`.

## Not verified / not 1:1

- **The replay is a beside-replay, not free-running.** Each frame starts from the original's words
  (MD234's note); a port run from the spawn image alone was not compared.
- **Five throttle frames and wreck frame 83 are not compared**: a tire on a loose prop (MD176) and
  Untamed's put-back, not ported (MD175). Both from MD161a.
- **The props' words after the step, early/torn capture rows and the second car** are not compared
  (MD161a's list).
- **The noise tables come from the image**, not built by the port (MD234).
- **The flipped image's wreck is a state poke**, not a natural crash (MD160).
- **The shell ride proves the pack rides as the port's `Load::untamed`**, not against a spawn image of the
  original's truck (MD161b).
- **The shell test skips where `mods/machines/untamed_cars` is not installed**; it ran here.
- **carengine.aud's halved volumes are unused**: openmx plays no pack car engine sound yet.
- **Not looked at beside the original in PCSX2**; nothing of the truck was compared on screen.

## Next

Tick it (`kit done`).
