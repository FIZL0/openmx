# R51 — Untamed's follow camera backed into Rocksbury's wall

MD255 ported the camera filter's two sweeps (`UntamedFollow::sweep_goal`, `sweep_from_eye`) against
made-up walls: no Untamed RAM image had the eye pressed on anything. R51 captures one.

## The capture

- Tape research/r51/back.pad from MD147's `md147/spawn` (Rocksbury, Free Ride, the quad facing +z with
  the stadium's south wall behind it). Cross and Square held together is the quad's reverse (Square
  alone only brakes). It backs at 12.9 ft/s straight at the wall, stops against it at z 20.07 by
  row ~355, and sits there; the tape lets go at tick ~665.
- Recorder research/r51/camera.toml, every tick, 720 rows (vsync 27275..28713, +2 every row):
  - the target block (208 bytes)
  - the filter (672 bytes)
  - the camera (192 bytes)
  - the quad's place and speed
- File: context/capture/r51/back.csv (shared context, not in git).
- The copy ran at NominalScalar 0.3. At 1.0 the recorder fell behind.

## What the original does

- Row 321 is the first push. Nothing is on the line from the target point to the eye's goal yet (the goal is 0.3 ft in
  front of the face). The eye's sweep, from where the eye was to past the goal, puts the goal 2 ft out.
  The contact ring's index goes 0 → 1 on that row and on no other.
- Rows 322 on: the line from the target point crosses the wall. The goal goes 2.5 ft out from the face
  along the face's normal: the props rule. So Rocksbury's wall is a prop to the camera, not the world's
  triangles. The world rule would have stopped it 2 ft out.
- The wall the line meets has two faces here:
  - normal (0.1073, 0, 0.9942), about 6.2° off the z axis, while the goal's line meets x ≈ 393.4;
  - normal (0.0243, 0, 0.9997), about 1.4° off, from row ~365, after the quad has drifted 0.3 ft in +x.
  - One plane can't stand in for both: replayed against the 1.4° plane alone, the goal is up to 1.3 ft off the original's.
    So the replay uses Rocksbury's solids (SX_A.scn's collision files, placed as tests/untamed_race_quad.rs
    places them), not a plane.
- The eye, on the straight spring at 0.8 of the tightness, goes through the wall. It is up to 5.68 ft
  behind the face around row 352, then comes back and settles 2.5 ft out. Nothing in the filter stops
  the eye itself from crossing a face it leaves from behind.
- The boost word (+0x28/+0x29) reads 0 on every row, the pushed ones too. Whatever sets +0x29 clears
  it before the reader looks. The 0.8 straight spring is shown by the eye's place instead: replayed,
  it matches to 3e-5 ft.
- The quad's +0x448 word (camera.toml's `dt`) reads 21..32 in this ride. It is not a dt here; it is
  unused.

## Torn rows

64 of the 717 frames have the target block and the filter a frame apart: the reader caught one of
them a frame early or late. The row's target block equals the next row's, or the filter's springs
name the next row's target. The springs' last target point (filter +0x60 / +0xa0) says which target
block the filter ran on. The test picks that row's target, and skips a row whose filter is not one
frame on from the row before's. 653 frames are left.

## The test

`r51_the_eye_backed_into_rocksburys_wall_is_the_originals` (tests/untamed_camera.rs). For each of the
653 frames:

- Set up `UntamedFollow`'s springs from the row before's filter and run `update` (dt 1/30) on the row's
  target block.
- The scene is Rocksbury's solids. They answer every ask but the goal sweep's first, the world's ask,
  which then breaks its loop. That models "the wall is a prop" with the port's one-scene trait.
- The eye goal must match the original's to 1e-5 ft, and the eye position to 1e-4 ft. Measured: goal 2e-6, eye 3e-5.
- Counted:
  - 380 rows pushed: 1 by the eye's sweep (2 ft out), 379 by the goal sweep (2.5 ft out);
  - the eye's furthest behind the wall, 5.68 ft;
  - the same 379 frames with the props also answering the world's ask (as `world::Place` does): the goal
    stops 2 ft out on all 379.

## Checked

- `./kit check -- -p game-sim --test untamed_camera`: 10 passed, no skip in the log.

## Follow-ups

- The port's scene answers the world's ask and the props' ask from one `Obstacles::nearest`. In the
  game, `world::Place` answers both from the terrain and the props. So a props wall takes the world's
  2 ft rule and never the props' 2.5 ft one, and the 0.8 straight spring doesn't run. Filed.

## Not verified / not 1:1

- One wall, one ride, straight back. A goal sliding along a wall at speed, a corner, a prop the eye
  meets from in front, and the world-triangle rule (2 ft) against the original are not captured.
- The world sweep's miss is assumed from the result (2.5 ft, the props rule). The original's world
  query was not traced on these frames.
- The ground is far down in the replay. The floor rule never acts on these rows, as both the goal and the eye
  stay at y ≈ 8.2.
- dt is taken as 1/30. The filter doesn't keep the dt it was handed.
- The contact ring and the last clear goal (write-only, MD255) are not checked beyond the ring
  index moving once.
- The look-at spring is seeded but not compared.
