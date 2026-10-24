# 1 — air wrecks switch (FINAL)

Task P138. 2026-10-23.

## What I did

- `game_sim::state::Sense` gained `air_wrecks` (true, the original's, by default), beside
  `wrecks_enabled`, and `state::Allowed` an `air` gate the step fills from it. `air::wreck_test` is
  now `air::wreck_test_in(.., true)`; `wreck_test_in(.., false)` skips the airborne clause only (nose
  dive / off the back past AirbornePitchDown/Up/RollWreckAngle while not flipping or spinning). The
  landing clauses, the cannonball, the g loads and everything else are asked as before.
- Like `wrecks_enabled` the switch is not in the rider's state hash (a session-wide setting, not
  state that moves), but it is on the snapshot wire (`wire!(Sense { .., air_wrecks, .. })`), so a
  resync carries it. That moved `tests/platforms.rs`' `RIDERS_WIRE_DIGEST` / `RIDERS_WIRE_LEN`
  (13323 → 13327 bytes: one bool for each of the four riders); its per-tick hashes and digest did
  not move.
- Tapes: `PadTape.air_wrecks` (`air_wrecks <frame> on|off` lines, written only when set, so every
  existing tape is the bytes it was), `Recorder::air_wrecks`, `Head::air_wrecks`, and `play_from`
  takes it at its frame. The shell writes `air_wrecks 0 off` for a `--no-air-wrecks` ride and the
  pause's change at the tick it closes on; a played tape rides On until its own lines say otherwise,
  whatever the Settings screen says (as it rides its own modifiers).
- Settings: `air_wrecks` in `settings.txt` (`air_wrecks = off`, only written when off), a Ride-tab
  row "Air Wrecks" under Wrecks, dimmed with "Wrecks are off" when Wrecks are off, in the pause noted
  "Changes take when the pause closes." and dimmed with the reason while held (`Menu::air_lock`: a
  played tape, or online). The Original switch: On needs Air Wrecks on and puts it back on; Off leaves
  it as it is (it is not a departure: the port's default is the original's).
- Shell: `--no-air-wrecks`; the ride takes `args && settings`; the pause closing on a changed switch
  sets every seat's sense (Wrecks is every seat's too) and the tape being written.
- Netplay: `Room.air_wrecks` (serde default on), the host's `room_for` / Open Online fill it, and
  `Handshake.air_wrecks` must match the host's ("air wrecks differs (host: on, you: off)"). Online the
  room's switch holds for the ride: the pause row is held (no mid-ride change online).
- Nothing was taken from opentennis: there is no matching switch there.

## What the original does

Has no such switch: its wreck test always asks the airborne clause. On is the original's own test,
bit for bit (`the_wreck_test_is_the_originals` below); Off is a port departure.

## How it was proved

- `crates/game-sim/tests/air.rs` `the_wreck_test_is_the_originals` (function oracle, 8000 generated
  states, every wreck kind seen: a temporary tally showed None 274, codes 1..12 the rest) now asks
  `wreck_test_in(.., true)` beside the original and requires the same answer and the same
  `wreck_impact` bits, and `wreck_test_in(.., false)` to equal it except where the original answered
  NoseDive/Tail on an airborne state, and never to answer NoseDive.
- `crates/game-sim/tests/air_wrecks.rs` (stand-in bike + `[Wrecks]` limits on the flat floor): upside
  down 40 ft up, On wrecks NoseDive/Tail on the first tick in the air (upright, the same jump never
  wrecks in the air); Off rides ≥30 ticks in the air upside down, never thrown; nosed over 75°
  (past the ground's 60°, short of the air's 120°) both settings ride the same ticks and wreck Front
  on landing; a tape round-trips the switch and `play_from` rides it.
- On bit-equal to today over a ridden run: `tests/platforms.rs`' four-rider run's per-tick hashes and
  their digest are unchanged with the switch on (only the wire bytes grew by the new bool).
- `crates/openmx/tests/menu.rs` `the_air_wrecks_row_toggles_and_survives_a_restart`: the row sits
  under Wrecks, toggles by pad, is saved and read back by a new Menu, dims with Wrecks off and under
  a hold, and Original On puts it back. `openmx-net` handshake test refuses a differing switch.

## Not verified / not 1:1

- **A roof landing is not wrecked with the switch off.** On the flat floor no landing clause judges a
  bike landing upside down: the ground pitch limits read the nose's pitch, which a roll-over leaves
  level; the vertical g load is in the chassis' own frame, so a roof impact loads it negatively; and
  the stand-in bike rights itself within ~5 ticks. With the switch on that bike never gets there (it
  wrecks in the air), so Off makes a ridden-out roof landing possible. The brief asked for "Off still
  wrecks a landing on the roof"; I kept the brief's design (skip only the airborne clause, landing
  judged as is) and tested a nosed-over landing instead. Filed as P144.
- Not tried on a real disc track or by eye in the shell, nor online between two machines: only the
  unit, menu and handshake tests above.
- Modifier presets do not carry the switch (Wrecks is not on them either); the brief's "presets"
  read as tapes/settings.
- No mid-ride change online: the room's switch holds (the host's pause row is held too), unlike the
  modifiers' P100 restart.
- `Sense.air_wrecks` is not in the state hash, as `wrecks_enabled` is not: two sessions differing only
  in it hash the same until the switch changes what they do.
- The new `RIDERS_WIRE_DIGEST` / `RIDERS_WIRE_LEN` were not run on aarch64/wasm32
  (`crates/game-sim/tests/cross/platforms.sh`): its run hit the user's /tmp quota (the shared
  /tmp/openmx-cross cache and other sessions' files fill it), and I did not delete other slots' files.
  The new bytes are one bool per rider through the same `wire!` encoding as `wrecks_enabled`, so
  nothing in them depends on the platform, but that is reasoning, not a run.
- The full gate's `one_fps` frame budget failed once under a load average of 25 (the sky 8.7 ms, the
  props 18 ms) and passed alone straight after; nothing in this task draws.

## Next

None: landed. Follow-up P144 (the roof landing with the switch off).
