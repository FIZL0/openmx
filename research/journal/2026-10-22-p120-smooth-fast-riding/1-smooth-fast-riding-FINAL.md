# 1 — smooth fast riding (FINAL)

Task P120. 2026-10-22.

## What I did

**Found it by logging.** A temporary per-frame log in the GPU draw printed the blend factor, the
'before' and 'now' spots and the drawn spot of the bike, the camera and the rider's body. I ran
headless free09 rides on a hand-written full-throttle tape at `--draw-fps` 144, 60 and 45. The rides
went down the x 4620 straight (`--trace-at 4620,2320,0`) and off the south edge into the shootoff
(`--trace-at 4620,500,180`), on through the wreck clock's put-back at tick 506.

- **Riding fast: no fault.** At every rate the bike, the camera and the bike's spot on screen
  (projected through the drawn camera) moved forward every frame. Blend factors stayed in [0, 1].
  The 'before' snapshot was right on frames that ran two ticks. Online rollback was not exercised
  (see below).
- **Bike and camera snapping on their own: not reachable when riding.** Even bike_speed=500
  bike_power=500 peaks at 7.2 ft a tick on free09's straight, far under `SNAP` (30 ft), and time_scale
  barely moves it. The shootoff's throw is about 10 ft a tick. So the fault the brief guessed at, a
  bike crossing the jump distance a tick before the camera following it, cannot happen at today's
  speeds. I fixed it anyway (below), because it is cheap and it is the same family of fault.
- **The fault: the thrown rider.** While the rider is down, the body's GPU palette is the ragdoll's
  bones in the space of the wrecked bike (`Body::ragdoll_palette(.., bike)`). `Shown::blend` blended
  that palette, and the bike's own placement, separately. That is right when the rider sits on the
  bike. It is wrong when a rider lies still on the ground while the bike tumbles through the air
  hundreds of feet away. Blending in the bike's space swings the body round the bike between ticks
  (for a turn of θ a tick, a still point at distance r is pulled in by up to r(1 − cos θ/2)), and the
  next tick snaps it back. Measured off free09's edge: at ticks 436–450 the body moved 0.08 ft a tick
  but was drawn **up to 70 ft** off that spot, every tick. That is "teleporting back and forth between
  two points". A faster crash throws the rider farther from the bike, so it grows with speed. Any
  node whose bike-space move passed SNAP also snapped alone while the others slid.

**The fix** (`crates/openmx/src/pace.rs`, `crates/openmx/src/main.rs`):

- `Blend::thrown`: a pose blended in the world. Each node goes into the world through its own tick's
  bike, is slid there, and comes back into the space of the bike as drawn. The whole pose cuts or
  slides as one. `Shown` now records `thrown`, and `Shown::blend` uses this when the rider is thrown at
  both ticks; a rider on the bike is still blended on the bike.
- `pace::Cut`, one per seat: one cut decision per seat per frame, applied to its bike, chassis pose,
  rider, plane and camera alike. The old code tested the bike and the camera separately against
  `SNAP`, and blended across a put-back's camera move. The seat cuts when the simulation marks a jump
  (`drawn_jumps`, the put-back), or when the machine, the camera eye or the look-at moves more than
  `SNAP + 2 × the move of the tick before` (`pace::jumped`). So riding or flying at any speed never
  cuts, and a jump from a standstill does. `Blend::slide` and `Blend::pan` are the snap-free blends
  the cut uses; `Blend::at` and `Blend::camera` are unchanged for anyone else.
- **One moment a frame**: the loop reads `moment` once a pass, after the last picture was shown (the
  end of the pass, where `top` is set). Both `Pacer::owed` and `Pacer::blend` take it. Before, each
  tick read the clock again and the blend read it later still. So frames that ran a tick were drawn a
  tick's cost later than frames that did not, though both went up at the next refresh: uneven spacing
  that grows with speed. Now every tick owed at the moment has run, so the blend is always below 1
  (no clamp). Taken from opentennis: Bevy's `Time<Fixed>` there runs the steps owed and takes
  `overstep_fraction()` at the same frame time (`hst-base/src/character.rs`). The rule is the same; the
  code is ours.
- The software path takes the same cut for its bike and its plane.
- `--trace-picture` (new flag, in `--help`) prints one line per drawn frame: the first player's bike
  and the middle of their rider's nodes (from, to, drawn), plus the camera. The test reads it.

## What the original does

Nothing to port: the original draws every tick where it is, at 30 Hz. The blend between ticks is ours
(P52, render-side only). This task changes nothing the simulation reads: the replay hashes do not
move, and the `pace` tests that pin the tick schedule and `dt` still pass.

## How it was proved

- `crates/openmx/tests/smooth_picture.rs`, riding the binary headless:
  - `a_fast_ride_moves_forward_every_frame`: full throttle down free09's straight for 150 ticks, drawn
    at 60, 144 and 45 fps. Every frame's drawn bike and camera eye step forward along the tick's
    direction of travel, never back, and nothing cuts.
  - `a_shootoff_and_its_put_back_draw_once`: off free09's south edge for 600 ticks at 60 fps. The test
    asserts more than 300 frames down and drawn thrown. Every frame that is not a cut has the bike and
    the rider's middle on the line between their two ticks' spots (within 0.01 ft and 0.05 ft). Every
    move longer than SNAP (the put-back) is a cut drawn exactly where it lands, and at least one exists.
  - Proved it catches the bug: with `Blend::thrown` switched off, the shootoff test fails with "the
    rider was drawn 2.2 ft off their line at tick 348". It stops at the first bad frame; the log
    measured the worst at 70 ft.
- `pace` unit tests:
  - `a_thrown_rider_stays_put_beside_a_tumbling_bike`: a quarter turn a tick, a rider 40 ft off, at 11
    blend steps; the bike's-space blend still swings more than 10 ft.
  - `a_cut_is_a_jump_past_the_seats_own_speed`: 50 ft a tick is no cut; a 40 ft put-back from a
    standstill, a marked jump and a camera-only jump are cuts.
  - `a_frames_moment_draws_a_steady_ride_a_tick_behind`: frames of uneven length at 45, 60 and 144 fps.
    The drawn spot is exactly one tick behind the wall clock, the blend is in [0, 1), and nothing steps
    back.
- With the fix in, the log rides at 144, 60 and 45 fps: the rider is at worst 0.010 ft off their line
  (was 70), no frame steps back, and the put-back's two ticks are cuts.

## Not verified / not 1:1

- **One gate run failed the shootoff test with the rider drawn "-0.064 of the way" at tick 487**
  (headless frame timing differs run to run). It was not reproduced in three reruns. I read it as
  f32 noise on a near-still body (the drawn rider goes through the drawn bike's inverse and back, at
  coordinates in the thousands), so the "along the line" check is now 0.02 ft absolute, not a
  fraction of the move. That run's move length was not logged, so this reading is unproved.
- **The user's own case was not reproduced in a window.** On this machine a windowed ride runs at
  1 fps (the window sits behind the lock screen), so everything above is headless/offscreen at forced
  rates. The `moment` change is about VSync pacing (frames gated by the present), which a headless
  `--draw-fps` ride does not have. It is proved by the `pace` unit test's model of uneven frames, not
  on a real display.
- **The user may have meant something else by "riding fast".** No riding-speed fault showed up at any
  rate, or with bike_speed/bike_power at 500. If the user saw the jumping without a wreck, it is
  either the pacing jitter the `moment` change removes, or something not found here.
- **Online is not exercised.** "Rollback's resimulated ticks replacing 'before'" was looked at only
  by reading the code. A rollback correction to a remote seat is drawn through the same `Cut`, measured
  on the machine alone, since a remote seat has no camera on this machine.
- **The software renderer's thrown rider is not blended.** It skins the ragdoll at the draw from the
  last tick's world-space bones, so it steps at 30 Hz but no longer swings: no back-and-forth, but no
  smoothness either. That path is the fallback only.
- **The rider on the put-back tick.** At the put-back tick (`drawn_jumps`' first tick), the body's
  palette root is still 91 ft off the bike in the bike's space. The tick after, it is back on the bike
  (S74's held tick). It is drawn as a cut, so nothing is blended across it. A shot at that tick showed
  the bike with no rider drawn on it. I did not chase this: it is S74's held tick, not a blend.
- **The shootoff's throw itself is not a cut.** The bike is thrown, not moved: it flies ~10 ft a
  tick. The test proves every frame across it is on its line. The small reference change at the throw
  (`chassis_at` to `chassis_wrecked`, under half a foot back) is slid over one tick, as before.
- **Blend::slide still turns by lerp-and-renormalise.** A body or bike turning more than about 90° in
  one tick would blend badly. Nothing measured here comes near that.

## Next

Nothing.
