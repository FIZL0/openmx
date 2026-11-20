# MD255 — untamed camera sweeps

2026-11-19. Task: `./kit plan MD255`.

MD251's Untamed follow camera only kept the eye over the ground. MD255 ports the filter's two world
sweeps, so the eye's goal stays in front of walls and props.

## What I did

- **game-sim `camera.rs`.** `UntamedFollow::update` now runs the filter in the original's order:
  - snap path: snap both springs, sweep the eye's goal, draw the goal as handed in (unswept);
  - filter path: sweep the goal (`sweep_goal`), sweep from the eye (`sweep_from_eye`), then the springs.
  - Either sweep finding a prop sets the eye's boost: the eye runs on the straight spring at 0.8 of its
    tightness that frame.
  - The floor now runs inside the goal sweep, on the eye and its goal, before the springs (MD251 floored
    the goal before and the eye after).
- **openmx `chase.rs`.** `untamed_follow` takes a scene that answers `Obstacles` too (the shell already
  passes `world.place`).

## What the original does

- **Goal sweep `0x003ab3a0`.** The line from the target point to the eye's goal:
  - up to 3 world hits. A face towards the target point puts the line's end 2 ft in front of it, along
    the normal (so the eye slides along a wall). A face looking away puts the start 0.5 ft past it.
  - then the props, target point to the moved end: 2.5 ft off, and the boost byte `+0x29`.
  - the goal becomes the moved end. A blend with the old goal by the boost time is computed into a stack
    slot and never read (checked in the disassembly at 0x3ab7b0).
  - the floor (`0x003a9e80`) on the eye and the goal, when the world pointer is set.
- **Eye sweep `0x003ab800`.** From the eye to the goal plus (goal − eye), when they are more than 0.1 ft
  apart. A prop with the goal less than 1 ft in front of it puts the goal 2 ft in front, and sets the
  boost. The contact ring (10 entries) and the last clear goal `+0xd0` are written, never read.
- **Snap path.** The output words `+0x260`/`+0x270` are the goal as handed in. They are not written again,
  so a snapped frame (and the look-back preset, which never filters) draws the unswept, unfloored goal.

## Port choices

- One `Obstacles::nearest` answers both the world and the props. It culls back faces, so the "start
  0.5 ft past a face looking away" case never fires here.
- The look-at's boost (`+0x28`, 1.5× tightness) has no setter in the filter, so it is left out.

## Checked

- `cargo test --release -p game-sim --test untamed_camera`: 9 pass. New ones, on a made-up wall:
  - the goal behind a wall goes 2 ft in front, and the eye settles there;
  - backing the quad at a wall, the goal never passes the face, and the eye settles 2 ft off once it stops;
  - the eye sweep puts a goal 0.5 ft off a prop at 2 ft, and leaves one at 1.5 ft.
- `cargo test -p openmx --bin openmx` (335), `--test untamed_atv` (20), `--test atv_camera_sound` (4).

## Not done

- **No capture by a wall.** `findcam.py` (here) finds the camera in every Untamed RAM image in
  `context/untamed`. In all of them the eye goal equals the last clear goal and the contact ring is empty:
  the eye was never by a wall. MD147's throttle ride hits a wall ahead of the quad, not behind the eye, and
  records no camera words. Filed R51 to make one.
- Backing at a wall, the eye's spring lags past the goal and can run up to ~0.5 ft through the face
  (the sweeps move only the goal). This follows from the port; a capture would show if the original does
  the same.
