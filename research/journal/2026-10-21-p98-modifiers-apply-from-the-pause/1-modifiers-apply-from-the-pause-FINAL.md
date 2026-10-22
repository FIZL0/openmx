# 1 — modifiers apply from the pause (FINAL)

Task P98. 2026-10-21.

## What I did

The game modifiers changed under a ride's pause now take the moment the pause closes, from where the
ride is, instead of on the next ride.

- **game-sim**
  - `Ride::retune(p)` (ride.rs): takes again from a new `.phy` what `Ride::new` copied out of the
    file and the modifiers reach: the engine's tuning (curve, gearing, clutch frictions, braking, rev
    limit, idle, inertia, min rpm, with the ratio worked out again for the gear it is in) and each
    wheel's damping (built off `Spring`). Its state (gear, revs, clutch, curve cursor, torques) and the
    chassis are untouched. Everything else the step reads of the file it reads off `Sim::phy` each tick.
  - `Flight::retune(plane)` (flight.rs): the plane's max power, full-efficiency speed, CD0 and gravity
    scale; the gravity force only while flying (a wreck keeps `WreckGravity`'s, its recovery builds the
    new scale's).
  - `Mods::rebike / resim / replane` (modifiers.rs): stock file → `Mods::bike` → new `Arc<Phy>` →
    `Ride::retune`; the integrator's speed limit moved from the old set's to the new set's while it is
    the riding one (a wreck's thrown limit is left to the put-back). `Mods::ride` now goes through a
    `speed_limit` helper, same arithmetic.
  - The tape (replay.rs): `PadTape::changes: Vec<(frame, Mods)>`, written as
    `modifiers_from <frame> <text>` (empty text = the original game), read back in order (an
    out-of-order one is a parse error by its line). `Recorder::modifiers(m)` records a change at the
    frame it is on (two between the same two frames keep the last); `Head::change()` is the change to
    put on before the next frame; `PadTape::modifiers_at / change_at`. A tape with no change is
    written byte for byte as before.
- **the shell** (main.rs): each seat's stock `.phy` is kept (`stock_phys`, beside the modified
  `other_phys`) and the biplane's stock `phy::Plane`. `remodify()` runs `Mods::resim` on every seat's
  `Sim`, `Mods::replane` on every seat's biplane flight, and re-says each rig's HUD `modified` mark.
  When the pause closes (`!riding` before the menu step, `riding` after) on a Settings set different
  from the one last seen (`menu_mods`, so a `--modifiers` ride the player did not touch keeps its own),
  the ride takes it: `mods` (now `let mut`), `engine.time_scale`, `front.menu.modified`, and a
  `--record` tape gets the change at its tick. Open to Online's `room.modifiers` reads `mods`, so it
  says the new set. A played tape puts its changes on before their frame (and a `--record` of the
  playback writes them too).
- **the menu** (menu.rs): `Menu::mods_lock`, set by the shell (`mods_lock()`): a played tape, or any
  online ride (lobby, drop-in, `--online`, and from the moment Open to Online succeeds). Under the
  pause, with it set, the knob rows, Load Preset's Cross and Reset To Original do nothing, the rows
  say `Held`, and the ticker gives the reason. The ticker now says "from when the pause closes" under
  the pause and "from the next one" from the main menu.

Online I took the brief's second option: the row is held with a note ("Online the ride keeps the
modifiers it started on: change them in the lobby"), not a host-sent change at an agreed tick —
filed as **P100**. There is no grey row style in the menu painter, so "greyed" is the `Held` word on
the row plus the ticker.

Nothing taken from opentennis: it has no mid-match parameter change (its "modifier" hits are input
keys).

## What the original does

The modifiers are the port's own (P55); the original has none. What has to be the original's is
that with every modifier at its default nothing changes: `remodify` only runs when the set actually
differs, a tape without a change is written as before, and every existing replay/acceptance test
rides unchanged.

## How it was proved

- `crates/openmx/tests/remodify.rs` (headless, the stand-in bike of `tests/modifiers.rs`):
  - `a_ride_moved_onto_a_set_at_its_start_is_the_ride_built_on_it`: for every knob at its low end and
    at 200%, plus a mixed set, both ways (original → set, set → original): the moved ride's file and
    speed limit equal the built one's, and 900 ticks of pad (Cross, a weaving stick) match tick for
    tick on the full state hash and the transform/velocity bits.
  - `a_change_mid_ride_keeps_the_place_and_rides_the_new_top_speed`: transform and velocity bit for
    bit the same across the change (the hash moves, because the engine's tuning is part of the ride's
    state), top speed 88.33 → 156.27 ft/s after bike speed ×2 mid-ride, the limit back to 135² on the
    way back.
  - `half_gravity_mid_fall_halves_the_next_ticks_acceleration`: the tick of the change falls g/4 and
    every one after g/2. The g/4 is the integrator's own memory of the last acceleration (a fall's
    first tick is 1.5 g, each after it g, so 1.5·g/2 − 0.5·g), which is what a ride built on half
    gravity from that same state would do.
  - `a_tape_recorded_across_a_change_replays_bit_for_bit`: a 900-tick recording with
    `gravity=80`, changed at 300 to `bike_speed=200 time_scale=150 grip=70`, written, read back and
    played through `Head` as the shell plays it: the same state hash every tick; without the change put
    on, the same up to 299 and different at the end.
  - `a_tape_with_no_change_is_written_as_before`, `the_biplane_takes_a_change_in_flight` (the disc's
    `biplane.phy` — it ran here, it did not skip: four sets, the retuned flight's Debug equals the one
    built on the set).
- `crates/openmx/tests/menu.rs` `the_pauses_modifiers_take_when_it_closes_unless_held`.
- The shell end to end: `openmx --play context/tapes/p98-change.tape --record
  context/tapes/p98-rerecord.tape --headless` on a generated 600-tick tape with
  `modifiers_from 300 bike_speed=200 gravity=50` printed `modifiers: bike_speed=200 gravity=50 (the
  tape's, from frame 300)` and wrote the change into the new tape at frame 300.

## Not verified / not 1:1

- **The pause path in the live shell was not driven.** Nothing here can press the front end's pause
  unattended (no keystrokes reach the window, and a played tape holds the modifiers by design). The
  code it runs is `remodify()`, the same call the played-tape path ran end to end above, and the
  menu side is covered by the menu test; the edge detection (`paused && riding()`) itself is unproven
  live.
- **Online is held, not host-changed** (P100): the brief's agreed-tick change was not built.
- **"Greyed" is a word on the row**, not a grey colour: the menu painter has no disabled style.
- **The change mid-ride is not the same state as a ride built on the new set from the start**, and is
  not meant to be: the engine state (gear, revs), the integrator's last acceleration and the
  wreck's own speed limit carry over. Proved equal only at tick 0; mid-ride the tests prove the
  place is kept and the new numbers are ridden.
- **A raised speed limit is lost at the first put-back** on any modified ride — `off_map::unwreck`
  sets 135² whatever `Mods::ride` raised it to. P55's gap, not this task's; filed as **P101**.
  `Mods::rebike` follows the same rule (it moves the limit only while it is the riding one).
- **The biplane mid-flight** was proved by field equality at build time only, not by a ride across a
  change; the wrecked-gravity branch of `Flight::retune` is untested.
- **Local multiplayer** runs the same per-seat call but no test rides two seats across a change.

## Next

- P100: the host changes the modifiers online, every peer at an agreed tick (the tape's
  `modifiers_from` is already the event to send).
- P101: the put-back keeps a raised speed limit (`unwreck` should restore the set's, not 135²).
