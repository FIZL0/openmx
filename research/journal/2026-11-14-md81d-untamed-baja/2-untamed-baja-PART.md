# 2 — untamed baja (PART)

Task MD81d, reopened. 2026-11-15.

## What I did

- Checked what MD81d still needs now that MD164 (capture) and MD163 (pack) have landed:
  - the diff and the port: done in part 1 (`tests/untamed_car.rs`);
  - the capture: MD164, `context/untamed/md164` (spawn/air images, throttle, steer, brake, jump, land,
    preload drives);
  - the pack, off by default: MD163, `baja@untamed_cars` on Untamed's step (`Load::untamed`), proved word
    for word against MD164's spawn image (`tests/untamed_baja_load.rs`);
  - **the replay of a captured baja drive, bit-exact**: still open. MD163 split it to MD163a, which is
    blocked after MD161a, the Untamed car frame harness (open, unclaimed).
- Did not build the harness here. It is MD161a's task, and MD163a's replay sits on it. Building it inside
  MD81d would do the same work in three tasks and clash with whoever takes MD161a.
- Blocked MD81d `--after MD163a`. When MD163a lands, MD81d's test (a captured SLUS-21701 baja drive replays
  bit-exact) is met, and the run reopens it to be ticked.

## Not verified / not 1:1

- **No Untamed baja drive is replayed yet.** It needs MD161a's harness, then MD163a. Nothing new was run
  in this part.
- Part 1's list still holds, except for the items MD163 closed (the pack and the shell switch).
  MD163's journal lists what it left open: the input step's controls-block reads, the `Scene::update_untamed`
  routing, the map-edge gate's writer, and the chassis model's tire points.

## Next

MD161a, then MD163a. Then MD81d can be ticked without new work.
