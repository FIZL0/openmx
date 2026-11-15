# 2 — untamed monster (PART)

Task MD81c. 2026-11-15. Reopened because MD160 and MD161 landed.

## What I did

- Checked what is left of MD81c after its two splits:
  - MD160 (capture) is done: spawn and air states and the drives are in kept-context.
  - MD161 (port) is done: the per-time damping and the yaw hold by compares are in, behind
    `atv_car::Load::untamed`, oracle-proved in `crates/game-sim/tests/untamed_truck.rs`. The preload cue
    and the clutch came with MD81d. The shell's `"step": "untamed"` switch is in.
  - MD161 split again: **MD161a** (replay MD160's drives bit-exact through an Untamed car frame harness)
    and **MD161b** (the openmx-mods pack for the monster truck). Both are still open.
- MD81c's test ("a captured SLUS-21701 monster-truck drive replays bit-exact") is MD161a's test word for
  word, and its pack is MD161b. Doing them here would duplicate two queued tasks and risk clashing with
  another slot that picks them up, so I blocked MD81c `--after MD161a,MD161b` instead.
- No code changed.

## Not verified / not 1:1

- **No replay of a captured truck drive**: open as MD161a (needs the Untamed car frame harness; MD163a's
  baja replay waits on the same harness).
- **No openmx-mods monster-truck pack, and no ride on one**: open as MD161b (one entry in import-atv's
  `untamed_cars::CARS`, on the unmerged `openmx-MD39c`/`openmx-MD163` branches).
- **carengine.aud's halved volumes** are still unread and unused: openmx does not play a pack car's
  engine sound yet.
- Everything MD161's entry lists stays open (the rates' `w` lane, the bike's and ATV's damping routing,
  `F`'s denormal compares).

## Next

When MD161a and MD161b land, MD81c reopens: check the replay and the pack ride pass, then tick it.
