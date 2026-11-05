# 1 — mxvatv biplane port (FINAL)

Task MD101a. 2026-11-05.

## What I did

- `biplane::Body::atv` / `set_atv` map the plane's body to and from `atv_body::Body`:
  - axes free;
  - max speed `MXVATV_MAX_SPEED` 300 and max rates `MXVATV_MAX_RATE` 12;
  - max accelerations `f32::MAX`;
  - shock peak / shock as correction peak / correction.
- `integrate_mxvatv` / `end_substep_mxvatv` step through them.
- In `flight.rs`, `Listeners` and `Together` call these when `mxvatv` is set. The substep count is unchanged
  (`World::step` is the same in both games, MD101).
- In `tests/mxvatv_biplane_replay.rs`:
  - The body-step pair. `the_port_body_flies_biplane{,3}_bit_exact` interprets each flight with the body's two
    vtable calls hooked to the port. The hooks check that a0 is the plane's body, and that the fixed words
    (axes, 300, 12, FLT_MAX) hold.
  - The port-replay pair. `the_port_flies_biplane{,3}` seeds a `Flight::new_mxvatv` off the image every 100
    frames from these words: body, pad, surfaces, flown, the engine's 14 words, aero true speed, and the crash
    reason and clock. It flies the port beside the interpreter and records how many frames the bodies agree
    bit-exact. The result is pinned in `BIPLANE_AGREES_THROUGH` / `BIPLANE3_AGREES_THROUGH`, and a raise must
    be written in.
- Context copied from ../kept-context/md80a/md101/ into context/capture/md80a and context/recordings/md80a (git-ignored).

## What the original does

- The plane's body (`*(plane+0xb8)`) is the ATV's body class. `./kit fn 0x00496240` is integrate (vt+0x54) and
  `./kit fn 0x0049bde0` is end of substep (vt+0x40). Each is called once per substep, four a frame at 30 Hz.
- The body's limits are constants in both images and every row: axes free, max speed 300, max rates 12, and
  max accelerations FLT_MAX. None of them is a biplane.phy key.
- The game's input step writes the stick words before the frame that reads them. The port's `Flight` keeps
  the pad a frame is handed for the next frame. For that reason the replay sets `Flight::pad` before each
  frame. Without this, every stick change parted the bodies by a frame.

## How it was proved

- **The bit-exact replay fixture.** The original's interpreter replays MD101's two captures (biplane,
  biplane3) from their .ee.bin images with the port's body step in place of the game's. Every row of both
  flights stays bit-exact against the csv, all of each file's rows.
- **The port replay.**
  - Biplane: 17 stretches of 100 frames. 10 airborne stretches agree to the end. The landing parts at frame
    1258. Ground-seeded stretches part on frame 1.
  - Biplane3: 24 stretches. 19 agree to the end. It parts at 2059 (landing).
- **Mutation checks.**
  - Putting MXU's integrate inside `integrate_mxvatv` fails the body-step tests at row 0/1.
  - Turning off the flight switch drops every stretch to 0.
  - Both mutations were restored with cp + touch.

## Not verified / not 1:1

- `MXVATV_MAX_SPEED` / `MXVATV_MAX_RATE`:
  - They were read off both images and all rows, and the writer (the plane's constructor) was not traced.
  - Whether another model or a wreck changes them is unknown.
- The body's clamps (max speed, rates, accelerations) are never reached in these flights. The clamp code is
  proved only by MD75's body oracle.
- Ground-seeded stretches and landings do not agree. The tires and ground contact are MD101b. The tires' state
  is not seeded, because its SLUS layout is not decoded.
- The engine note differs, because the smooth-noise tables and rand() are not seeded. It is sound only, so it
  is left out of the diagnostic diff.
- The curve and acos tables are the port's own builds, not copied from the image. They agree over these
  flights.
- The port flies with `Area` None and a far flat floor, so the map-edge test is not exercised.
- The wreck machine is MD101c.
- The ratchet compares body words only. Surfaces, engine and aero are printed as diagnostics when a stretch
  parts, and they are not asserted.
- The seed sets `Flight::pad` directly, so it skips the port's input-step gate.
- Biplane's late rows and row gaps are as described in MD101.
- The PC exe was not read for the body, since MD75 already proved the PS2 routines.
- Nothing applicable was taken from opentennis.

## Next

- MD101b (tires/ground) should raise the landing and grounded entries of the two `AGREES_THROUGH` arrays.
