# 1 — marker keeps momentum (FINAL)

Task P112. 2026-10-23.

## What I did

- Looked for where the motion goes, on the real bike: the disc's `race125p.phy` with the
  `125_faisst` model's wheels, ridden through the shell's own pass order (poll, `stick_alone`, step,
  `markers::frame`). On the Down tick the marker keeps the whole `Ride`. That covers the chassis'
  velocity and angular velocity, the wheels' `mv.spin`, the engine, the air state and the
  rate-limited levers. Up puts the same `Ride` back. **The simulation loses nothing.** That holds:
  - for a fresh marker,
  - out of a wreck (`teleported()` respawns the state machine but leaves the `Ride` alone),
  - after the markers file. `to_file`/`from_file` carries every field: the loaded slot hashes the
    same as the kept one.

  The ride after the teleport steps bit for bit with the ride after the placement (120 ticks checked).
- Rode it in the shell (`openmx --headless --play <tape> --trace-picture`, free06's start). The tape:
  gas, Down held at tick 150 (slot 1 placed at ~40 mph), gas on, then Up held at tick 260.
  - The bike after the teleport retraced the post-placement path exactly (tick 268 sits where tick
    158 did).
  - **The camera did not**, and that is the momentum the user saw go:
    - With no wreck, nothing snapped the follow camera. Its lags flew in from the old spot. The eye
      was 203, 176, 152 … 22 ft from the bike over ~15 frames, moving back towards the bike faster
      than the bike moved, so on screen the bike stood still.
    - Out of a wreck (a real ragdoll wreck, reason 1, then Up), the wreck camera's "back on" snapped
      the follow camera onto this frame's raw target. Its lags then started at rest: the eye moved
      0.2 ft on the first frame while the bike moved 1.9 ft, and eased out from 13 to 24 ft. That
      reads as a standing start.
    - At the placement itself the eye sat a steady 24.5 ft behind, moving with the bike.
- Fix, in the shell only, with no sim change:
  - `Seat::teleported` (main.rs; `same_ride.rs`'s seat initialises it too) is set by the pass when the bike's marker Up fires. The camera block
    takes it once. It snaps the bike's follow camera, and after the frame it calls
    `chase::ride_out`.
  - `ride_out` moves each eye and look-at lag back along the bike's velocity to the place a
    first-order lag settles behind a target moving `d = vel·dt` a frame:
    `value = target − d(1 − a)/a`, where `a = min(dt, tau)/tau` is the lag's own blend. That frame's
    view is moved with it.
  - It needs no state kept from the placement, so a marker loaded from the file rides out the same
    as a fresh one.
  - Measured after the fix, same tapes:
    - no wreck: 24.9, 25.8, 25.8 … ft, the eye moving ~1.85 ft a frame with the bike from the first
      frame. Placement: 24.3–24.8 ft.
    - out of the wreck: 24.9, 25.1, 25.1 … ft, the same.

## What the original does

The original has no slot markers. P88's d-pad markers are the port's own feature, so there is
nothing to decompile here. The follow camera's lag (`Follow::filter`, `Lag::step`) and its snap are
S7's port of the original's camera, used as they are. `ride_out` only chooses what the lags hold on
the one frame a marker teleports.

## How it was proved

- `crates/openmx/tests/markers.rs` covers the real bike off the disc, `race125p.phy` placed at
  93 ft/s while weaving:
  - `a_marker_rides_out_at_its_speed` checks that the body velocity, chassis velocity, angular
    velocity (body and world), and front and rear wheel spin are bit-equal to the placement's, then
    hashes the `Ride` every tick for 120 ticks against the placement's ride.
  - `a_marker_rides_out_of_a_wreck_at_its_speed` does the same out of a forced wreck.
  - `a_saved_marker_rides_out_at_its_speed` does the same after `saved_markers::save` and `load`
    through a temporary directory. The loaded slot hashes the same as the kept one.
  - Ran with the disc present (a temporary print showed all three ride at 93 ft/s, none skipped).
- `the_camera_rides_out_with_the_bike`: a camera that rode along for 300 frames, compared with one
  snapped and then `ride_out` onto the same bike. Eye and look-at agree to 0.01 ft on the teleport
  frame and for 60 frames after. A plain snap starts more than a foot off.
- Shell runs: the two tapes above, before and after. Eye-to-bike distances are quoted above.

## Not verified / not 1:1

- **The user's exact ride.** I could not reproduce a loss of momentum in the simulation on any path:
  fresh, out of a wreck, through the file, or on the shell's own tape. The camera is the only loss I
  could find and measure. If the user meant something else (for example a marker placed while the
  bike was airborne and landed differently), it is not covered. A recording of the case would settle
  it.
- **A live pad and the front end.** Only `--play` tapes in the headless shell were run. The pad and
  the menus were not driven. The tape's pad path is the same `Seat::pass`.
- **Online seats.** `ride_out` is wired for the offline seats' pass only. In `net::rides` the
  marker's Did is not reported to the frame, so an online seat's camera still flies in after a
  teleport. Wiring it would need the session to hand the Did out of the tick, and with rollback that
  is more than this task's lines.
- **The plane's markers** are not touched (P107). The plane's teleport already places the flight,
  and its camera was not part of the report.
- **The forced wreck in the tests** is `state.enter(Wreck::ATTITUDE)`, not a crash ridden into. The
  shell tape c0 covers a real ragdoll wreck.
- **`ride_out` is the port's own presentation**, not anything 1:1. It assumes the bike's velocity
  holds across the next frames: on a turn or a jump the first few frames' lag differs a little from
  a camera that had been riding along. The difference is the lag's own response to the change, not a
  jump.

## Next

Nothing. The task lands.
