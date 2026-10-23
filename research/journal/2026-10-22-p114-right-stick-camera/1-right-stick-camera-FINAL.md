# 1 — right stick camera (FINAL)

Task P114. 2026-10-22.

## What I did

- `game_sim::camera::Race::nudge` (camera.rs): the race camera's own override of the base class's
  nudge slot. It lets go of the widen command (0x1f, R1) and calls the base `Follow::nudge`, so on
  a race camera R1 never widens; L3 (0x1e) held zooms, otherwise the stick looks around.
- `chase::look_around` (crates/openmx/src/chase.rs): one tick of a seat's right stick on its own race
  camera. The stick goes through `AxisRange::PLAYER` (±0.3 dead zone, the poll's inverted right x),
  the zoom modifier is the seat's table's `Command::CAMERA_ZOOM`, and `reset` clears the nudges
  after the stick.
- `pad::Scheme::RESET_CAMERA` / `Scheme::resets_camera` (pad.rs): R3's press, under **both** layouts.
- main.rs: once per tick, per local seat, after the preset handling and the wreck camera switch and
  before the follow camera's frame, `look_around` on that seat's own `seat.sim.input.pad(0)`, its
  table and its follow camera (the bike's or the plane's, whichever is up). Only while the follow
  camera has the screen (`!wreck.on`). Keyboard right-stick keys (IJKL / YUIO, P54) already ramp the
  report's right-stick bytes in `Pads`, so they arrive the same way; the keyboard's R3 is E (original)
  / N (new).
- Stale docs fixed while I was there: main.rs's controls paragraph and `--help` said "L3 balances, R3
  is reverse". Nothing in the shell or the simulation reads either click (git: the R3 reverse went
  with the P2-era shell; the original's only reverse is gas + brake at a standstill, oracle P73). The
  wreck line said "L3 and R3 together to get back on"; the reset is L1+R1 (P20). bindings.rs's
  `Command::RIDE` doc now says the right stick's modifiers are read by the camera.

**R3, the decision.** The brief offered: the reset on R3 under the New layout and reverse elsewhere,
or ask. Neither layout has reverse on R3 — the shell reads nothing on R3, and the original binds R3 to
commands 0x07 and 0x23, for which no reader was found (oracle P73, "05-0d, 23, 25, 26: no
constant-command reader found"). So no layout loses anything, and R3 resets the camera under both,
which keeps the two layouts' cameras the same. The original never clears the nudges (its clear has
no caller); this is the port's.

Opentennis: nothing to take — it has no player-driven camera look-around (its orbits are cutaways).

## What the original does

- `./kit fn 0x001fc630` (camera_nudge, base slot +0x12c): reads commands 0x1e and 0x1f as values
  (`0 < value`); with either held, zoom += y·0.3 and/or fov += y·0.5; otherwise yaw -= x·0.05 wrapped,
  pitch += −0.05·y unless that is negative and cam+0x16c (pitch_down_locked) is set, wrapped. Already
  ported as `Follow::nudge`.
- `./kit fn 0x0021f770` (race camera vtable 0x00452c20, slot +0x12c): writes 0 into the pad's value
  slot for command 0x1f, then, if `*(*(app+0x3cc)+0x74)` is non-null with its +0x34 == 3, centres the
  stick; then the base nudge. Read in first_recording and biplane's RAM images: app+0x3cc's +0x74 is
  null in both, so the veto does not fire in a ride.
- The stick's units: a riding state registers exactly four stick consumers, the player record's
  +0x88..+0x94 (left x/y, right x/y) with ±0.3 (oracle I6), so the camera's right stick has to come
  from those — `AxisRange::PLAYER`.
- The call site of the slot was not found (both nudges are reached through thunks at 0x001fca10 and
  0x0021f8e0, i.e. a secondary vtable); see below.

## How it was proved

`crates/openmx/tests/right_stick_camera.rs` (no disc; stand-in bike plus ride.rs's wreck limits, a
`Race Cam 1` stand-in preset, two seats each with its own `Sim` and camera, ticked the shell's way):

- `a_held_right_stick_turns_1ps_camera_and_leaves_2ps_alone`: 40 ticks of full right; 1P's
  `user_yaw` is k·0.05 (F arithmetic, to the bit) and its yaw the preset's less that each tick, its
  view differs from an un-nudged camera's; 2P (centred) is bit-equal to an un-nudged camera's view.
- `with_l3_held_it_zooms_instead`: L3 + full down: zoom +0.3 a tick, yaw/pitch/fov untouched,
  distance beyond the plain camera's; R1 + full down pitches instead (the race camera drops widen).
- `an_r3_click_puts_the_camera_back`: after 20 ticks turning and 10 zooming, the R3 tick leaves
  every nudge 0 and yaw, pitch and distance bit-equal to the un-nudged camera's.
- `the_nudges_keep_the_originals_rules_through_a_preset_change`: kept through the look-back and
  back; cleared by `next_preset` between two ordinary presets.
- `the_rides_ticks_are_identical_with_and_without_the_stick`: 300 ticks of a moving bike, the stick
  circling, L3 held 60 ticks, R3 every 50: the sim hash is identical with the camera reading the same
  reports and without, and the bike's transform is identical to a ride on a centred stick.
- pad.rs `r3_resets_the_camera_under_either_layout`: R3's press, once per press, under both schemes;
  the keyboard's R3 key reaches it under each.

## Not verified / not 1:1

- **Where the original calls the nudge, and how often**: the slot's caller was not found (only
  thunks reach it). The port nudges once per simulation tick, before the camera's frame; if the
  original nudges per rendered frame or after the camera's frame, the turn rate or a one-tick phase
  differs. Not compared against a live capture of the camera turning (would need PINE reads of
  cam+0x120 while a pad holds the right stick).
- **The stick's range** is inferred (the only right-stick consumers in riding RAM carry ±0.3), not
  read off the call site.
- **The scene-state stick veto** (`+0x74`'s state 3) is left out: null in both riding states, its
  meaning unknown.
- **The original zeroes R1's value on the pad** for the rest of the frame when the race camera
  nudges; the port's camera never writes the pad. Anything that read R1 after the camera in the
  original's frame would differ; nothing known does.
- **Not fed while the wreck camera has the screen**: the original's behaviour there is unknown (the
  follow camera's nudge may or may not run under the wreck camera).
- **No end-to-end shell run** with a real pad or a played tape: the main.rs wiring is exercised only
  through the test's copy of its order (poll, step, look_around, update); the build and the gate
  cover the rest.
- The R3 reset is the port's own; it has no original to match.

## Next

Nothing for this task.
