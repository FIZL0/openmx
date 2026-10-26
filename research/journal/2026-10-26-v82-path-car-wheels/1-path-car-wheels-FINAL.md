# 1 — path car wheels (FINAL)

Task V82. 2026-10-26.

## What I did

- Decoded how the original turns a path car's wheels: the car class's init finds the wheel nodes, and
  its per-frame update spins and steers them (below; the long form is research/oracle.txt's V82 block).
- `game_render::toy::Wheels` (crates/game-render/src/toy.rs): a car's wheel nodes, one over the
  radius, and the `SteerFrontTires` switch (now read into `toy::Car::steer_front`). It gives the spin
  and steer at a moment of the scene's clock and the palette with the wheels posed.
- The shell (crates/openmx/src/main.rs): it builds every alone-track car's `Wheels` once at load and
  each frame works out a palette per drawn car (`traffic_pose`, next to `traffic_at`). The GPU actor
  gets the palette as its `pose`. The software path skins it into its own copy of the model
  (`<model>#car<k>`, the creature/toy copy pattern). It skins the copy whole once, then
  `Wheels::reskin` moves only the wheel nodes' vertices each frame.
- A world's tiles (crates/openmx/src/world_dress.rs): `Dressed::wheels`, built in `take`, and
  `Dressed::wheel_poses`, one palette per `cars_near` car, but only for cars within `CARS_NEAR` of a
  camera. When a tile is let go its `#car` copies go too, because a re-taken tile's textures land in new slots.
- crates/game-render/src/gpu.rs needed no change: `Actor::pose` already draws any model from a palette.
- P87's driveable machines: the port drives only the bike and the biplane (`vehicle::Kind`). It has no
  driveable truck, so this part of the brief has nothing to fix.

## What the original does

- **Which wheels.** The init (`./kit fn 0x001147c0`, vt+0xc4) looks up `TireFrontL`, `TireFrontR`,
  `TireBackL`, `TireBackR`, `TireMidL`, `TireMidR` in the model's tree (depth first, no case) and stops
  at the first one missing. One over the first wheel's box half height is the inverse radius, or 1.0
  when the box has no height. free06's dump trucks have 4 wheels (radius 6.298). Its scraper fronts
  and backs have 2 (radius 3.599); the backs have `SteerFrontTires = 0`. The free09/free11 trailer
  cabs have 4 and the trailers 3, so the trailers have `TireBackL` but no `TireBackR`. The train's
  cars have none, and nothing on them turns in the original.
- **Spin.** Each update with dt > 0 does: spin −= inv_radius × √(dz² + dx²) of the step the car's path
  point made, wrapped into ±π. All of a car's wheels share one angle, and it turns backwards about the
  wheel's x for a car moving along +z.
- **Steer.** With `SteerFrontTires` (default on), the update takes target = clamp(20 × (last heading ×
  heading).y, ±30°). It blends that in as steer = (1−f)·target + f·steer, with f = the car's
  `HeadingFilter`. Wheels 0 and 1 get it; the rest only spin.
- **Pose.** It writes the wheel node's local rotation each update and keeps the node's position.
  Forward is (sin s cos a, sin a, cos s cos a) and up is (−sin s sin a, cos a, −cos s sin a). The
  wheels that only spin use a sine table and s = 0. `./kit fn 0x00115540` has the update.

## How it was proved

- **Save-state oracle**, crates/game-render/tests/path_car_wheels.rs: S89's s89-along state of free06,
  21 path cars, driven by the original's own update in the interpreter for 60 ticks at 1/30.
  - The port's wheel count, inverse radius (bit for bit) and steer switch match what the init left
    in all 8 wheeled cars, and the other 13 have none.
  - Every tick, the original's spin moves by exactly −inv_r × the flat step (to 1e-5). This is the
    formula, checked 480 times.
  - Over the two seconds, the port's clock-driven angle moves as the original's does, within 0.01 rad
    + 0.5 %.
  - The port's replayed steer is within 3e-3 rad of the original's +0x554. The worst was 2.1e-3 on
    a truck going nearly straight; on a bend it was −0.0417 against −0.0416.
  - Every wheel node's local matrix in RAM is `Wheels::local` of the original's spin and steer at
    the node's file position, within 3e-3 per element.
- **Both renderers**, crates/openmx/tests/path_car_wheels.rs: every track on the disc, 46 wheeled
  cars (free06, free09, free11 and their _ss variants). Over one second in 30 steps, each wheel's
  angle in the palette (what the GPU actor is given) and a rim vertex of `skin::skin`'s triangles
  (what the software renderer draws) turn by −(flat distance the car moved) / radius, within 1 %.
  The distance is summed from `Traffic::placed`'s positions, not from `Wheels`. Steered wheels stay
  within the 30° stop.
- **Looked at:** headless `--shot-pose` shots of free06's truck 16 at frames 90 and 96, GPU and
  software, alone and in `world_mxu_free` (its free06 tile at +288,+288). The rear hub's spokes and
  the tread turn between the two frames in all four, and the wheels stay on their axles.

## Not verified / not 1:1

- **Stateless, not the original's running state.** The original adds to its spin every update. The
  port works out the spin from the flat length of path run since the track loaded, measured in 10
  chords a segment plus the closing chord each lap. The original's sum is over its 30 Hz steps, so
  the two drift apart by chord-versus-arc. The test bounds that at 0.5 % over two seconds. Over a
  long ride the absolute angle can differ by any amount, but a wheel's look only depends on it mod
  its spoke symmetry.
- **The spin's starting angle is not the original's.** The original starts at 0 when the car is
  created. The port starts at 0 at clock 0 of its own load, which is not the moment s89-along was
  saved at. Proved: the change in angle, not the angle.
- **Steer is replayed, not carried.** `steer_at` replays 96 heading updates using the port's own flat
  heading, so it is up to 2e-3 rad (0.12°) off the original's. Not traced: whether the original's
  heading at +0x480 is the terrain-snapped point's.
- **Not reproduced:** the original's sine table and how it quantises. The port uses `sin_cos`, and
  the pose test passes at 3e-3, which allows that.
- **Not compared by eye against the original.** No PCSX2 shot or video was set beside the port's. The
  save-state test compares the wheel nodes' own matrices, which is stronger for the pose, but the
  rendered picture of the original was not looked at.
- **Far world cars stay still.** A world car past `CARS_NEAR` (1500 ft) of every camera is drawn with
  its file's wheels, unposed. At that distance a wheel is a few pixels.
- **Software path cost: measured on a loaded machine only.** The first gate went red on one_fps's
  66.7 ms budget (load average ~40; HEAD failed it too, at 68.5 ms). In that run the scene stage cost
  15–17 ms against HEAD's 12.9, because every wheeled car was skinned whole each frame. Now
  `Wheels::reskin` moves only the wheel nodes' corners after one full skin. Interleaved with HEAD in
  a scratch worktree: a frame cost 33.5/28.7 ms against HEAD's 32.9/29.1, and the scene 7.3/6.0
  against 6.8/6.1. The openmx test checks that reskin equals a full `skin::skin` at every step.
- **Other tracks' trucks:** the disc sweep found wheel nodes only on free06/free09/free11 (and the
  _ss variants). Every other track's `.toy` cars have no `TireFrontL`, so they correctly don't turn.
  Checked by the sweep, not by eye.
- **P87 not applicable:** the port has no driveable truck (only bike and biplane), so nothing was
  changed there.

## Next

Gate and land.
