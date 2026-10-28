# 1 — biplane engine stops on wreck (FINAL)

Task P187. 2026-10-27.

## What I did

- **The simulation (crates/game-sim/src/flight.rs, `Flight::wreck`)**: the wreck zeroed `engine.power`; the
  original zeroes the engine object's *note* (+0x70, the sound's rpm) and leaves the power (+0x5c) at its
  last value. Now it zeroes `engine.note`. The engine is not stepped while wrecked (the force listener
  skips it), so the note stands at 0 through the wreck and the prop angle stands still.
- **The sound (crates/openmx/src/audio.rs, `Sound::frame`)**: in the plane, a wrecked flight
  (`PlaneCrash::wrecked`, either reason) silences the plane's engine loops (`PlaneNote::silence`) instead
  of updating them; the first tick unwrecked, `PlaneNote::update` starts them again — the original's stop
  on the wreck and start on the unwreck. The unwreck runs after the engine step, so on the unwreck tick
  the restarted loops play note 0, and from the next tick the reset's closed throttle (idle).
- `audio::PlaneCrash::of(&Flight)` and `audio::plane_note(&Flight)`: what main.rs's `seat_sound` built
  inline, moved into audio.rs so the test reads the flight exactly as the shell does. main.rs calls them.
- The prop: already bent and unturned while wrecked (`Plane::prop_angle`/`surfaces` in vehicle.rs, S35's
  `biplane_anim::surfaces` with the wreck word). Nothing to change.
- The S36 model (`biplane::Plane`) already zeroed a field it calls `engine_power`; only its doc now says
  it is the note. Not renamed (other tasks' code reads it).
- Nothing taken from opentennis: it has no vehicle that wrecks.

## What the original does

`./kit fn 0x003535d0` (biplane_wreck): reason into +0x3b0, the body's inertia/damping, gravity
−WreckGravity·mass·32.2, then `+0x34c = 0` (the throttle), `(+0x400)+0x70 = 0` (the engine object's
note, which biplane_engine_step writes as "the sound's rpm with a wobble") and the engine sound
(+0x674)'s vtable +0x8c (stop). `./kit fn 0x003536e0` (biplane_unwreck): everything put back, then the
same sound's vtable +0x88 (start), then vt +0x198 (the reset). Both reasons (1 contact, 13 map edge)
go through the same two functions.

## How it was proved

- **The bit-exact replay** (crates/game-sim/tests/replay.rs `the_biplane_acceptance_run`, C7's
  biplane.csv in s2's context): S104 left it agreeing through tick 691, parting at 692 — the first crash —
  on `engine.power`. With the power left alone it agrees through **774** on both grounds (the original's
  and the port's own), and parts at 775 on `body.pos`, the port's plane already back at its spawn while
  the original's is not. `BIPLANE_AGREES_THROUGH` raised 691 → 774. That proves the power is not zeroed;
  the note itself is not a captured lane.
- **crates/openmx/tests/audio.rs** (disc's biplane.phy and rays, a flat ground, tone loops):
  `a_crashed_plane_has_no_engine_until_it_respawns` (wrecked at tick 12, back at 163) and
  `a_plane_thrown_off_the_edge_has_no_engine_until_it_respawns` (reason 13, thrown at 230, back at 622):
  the engine voice plays before the wreck, the note handed over is 0 on the wreck tick, no engine voice on
  any wrecked tick, one again on the unwreck tick, and on the next tick its gain is the loop's at the idle
  note (0.128 / 0.116) with the throttle at zero. Both probed to run (a temporary panic showed their
  lines), and both red with the audio fix taken out ("the engine still sounds [0.3]").

## Not verified / not 1:1

- The sound's stop and start are not captured from the original's sound object (no PINE lane on the
  emitter's playing state); it is read from the decompilation's two vtable calls. Whether the original's
  start (+0x88) restarts the loops from their beginning or resumes them is not known; the port starts them
  fresh. Presentation, by AGENT.md's rule matched by ear, not proved.
- The engine note (+0x70) is not a lane of C7's biplane capture, so its zero through the wreck is proved
  only through the power lane it replaced (774 vs 691), not directly.
- The new ratchet stop, tick 775 (the port's chord/waited-out respawn a tick or more ahead of the
  original's body position), is not chased here; filed as a follow-up.
- Landed with `--skip-audit`: the audit's 7 leaks are other tasks' addresses in plan/DONE.md
  (biplane_tire_*, radar_update), none in this diff; the gate itself passed (2602 tests).
- Not checked in a live shell window by ear: the shell path is the same `Sound::frame` the tests drive,
  with the flight read through the same two helpers main.rs now calls.

## Next

S186 (filed): the acceptance run's new parting at tick 775.
