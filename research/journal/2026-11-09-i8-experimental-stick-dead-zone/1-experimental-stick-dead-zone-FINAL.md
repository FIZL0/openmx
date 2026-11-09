# 1 — experimental stick dead zone (FINAL)

Task I8. 2026-11-09.

## What I did

- **A new modifier, `Knob::StickDeadZone`** (`game_sim::modifiers`, word `stick_dead_zone`), per seat.
  Its value is the dead zone itself in hundredths, chosen from `DEAD_ZONES` = 25, 20, 15, 10, 5, 0, and
  its default `ORIGINAL` (100) means **off**, which is the original's ±0.30. PlaneMaxG already uses the
  same value scheme. I made it a modifier rather than a separate setting so the carrying comes for free.
  The modifiers' text form is what settings.txt, a tape's `modifiers` line, presets and the online
  lobby already carry. Each seat brings its own online (`online::seat_own` is true for it), and a change
  under the pause takes when the pause closes (`Mods::rebike` rebuilds the file from stock). This covers
  the brief's "a replay/tape records it and online sends it in the handshake" with no new wire.
- **Where it acts:** the bike's `Phy` has a new port-only field, `stick: AxisRange`. The reader writes
  `AxisRange::PLAYER`, and `Mods::bike` sets `AxisRange::player_with_dead_zone(z)` only when the knob is
  set. The following read the pad through `phy.stick`:
  - `Sim::hands` (`Controls::through_stick`)
  - the sim's plane (`flight::pad_through` → `PlaneInput::from_pad_through`)
  - the shell's plane, including the round stick (`vehicle::round_stick_through`)
  - the car (`CarRide::step_met` gained a `stick` argument)
  - the Kiowa (`KiowaRide::step_through`)

  The old entry points (`Controls::through`, `flight::pad`, `PlaneInput::from_pad`, `round_stick`,
  `CarRide::step`, `KiowaRide::step`) all keep their signatures and call the new ones with `PLAYER`, so
  every existing test and fixture is untouched. The `Phy` and its field are not hashed, the same as the
  bindings table. Every peer builds the field from the same room modifiers.
- **Menu:** Settings → Controls ends with an `Experimental` heading and a **Stick Dead Zone** row
  (`Key::DeadZone`). Its choices are Off, 0.25, 0.20, 0.15, 0.10, 0.05 and 0.00, and each has a hint.
  The row sets the Player row's seat when there are several players, a guest's own seat online, and
  every seat when there is one player. It is locked under the same rules as the modifiers page
  (`mods_held`, `knob_held`). It is not on the Game Modifiers page (`Knob::MENU` leaves it out).
- **Correction to the brief:** the brief says the ride maps the stick through ±0.2 (`AxisRange::STICK`).
  ±0.2 is the main menu's range. The ride's stick consumers carry ±0.3 (`AxisRange::PLAYER`, proved in
  I6/I7), and that is the range the knob replaces.
- **From opentennis:** nothing taken. Its dead zones are device-side noise gates (`hst-base/src/device.rs`),
  not the game's stick shaping.

## What the original does

The ride's four stick consumers register with lo −1, hi 1, centre 0 and a dead zone of ±0.3. A byte
maps to `v = −1 + 2·b/255`. Inside the dead zone it reads 0. Outside, it reads `(v ∓ 0.3)/0.7`
(I6/I7, `AxisRange::apply`). The new zone uses the same arithmetic with ±z, so a stick reaches full
travel at the same byte as before and only the zone's edge moves.

## How it was proved

- `crates/game-sim/tests/stick_dead_zone.rs`:
  - **Off:** with every modifier at its default, `phy.stick == AxisRange::PLAYER` field for field, and
    `player_with_dead_zone(0.3) == PLAYER`.
  - **Off:** on every byte 0..=255 of both left-stick axes, `Controls::through_stick(PLAYER)` gives
    `Controls::through`'s stick, compared as raw float bits, and `flight::pad_through(PLAYER)` matches
    `flight::pad`.
  - **Off:** a stock ride with the original modifiers laid on hashes the same as a bare `Sim::new`
    every tick for 60 ticks, at byte 140 (inside ±0.3) and at byte 200.
  - **On, ±0.05:** byte 140 reads 0 through ±0.3 and through ±0.10. Through ±0.05 the poll's stick is
    exactly `(v−0.05)·(1/0.95)` ≈ 0.05. The plane's aileron is non-zero; it is small because the plane
    cubes the stick. A 90-tick ride on that stick parts from the original's hash.
  - Text round trip (`stick_dead_zone=5`, `stick_dead_zone.2=10`), out-of-range values refused,
    `seat_own`, and the row's ring of choices.
- `crates/openmx/tests/menu.rs`, `the_dead_zone_row_is_off_by_default_and_saved_as_a_modifier`: the row
  starts Off, steps to 0.05 on every seat, is written to settings.txt and read back, and wraps past the
  narrowest choice to Off. I updated the Controls-tab key list test and the "Vibration is last" test to
  expect the new heading and row.
- All existing input, ride and replay tests are unchanged and pass in the gate. With the knob off, no
  value read by any step differs.

## Not verified / not 1:1

- **On is not the original, on purpose.** A narrower dead zone has no counterpart in MX Unleashed, so
  there is nothing to prove it against. The tests prove only that its arithmetic is `AxisRange::apply`
  with ±z, and that a small stick reaches the rider.
- **Not tried by hand on a real pad.** I had no pad here (uinput is root-only), so nobody has judged how
  0.05 or 0.10 feels to ride. The choices 0.25 to 0.00 are a guess at a useful range. A slider would
  give finer steps; I used a choice row instead.
- **The chase camera's right stick still uses the original's ±0.3** (`chase.rs`). It is presentation,
  not the rider, so I left it alone. The menus' ±0.2 is also untouched.
- **Online carrying is not exercised by a two-shell test.** It travels through the existing per-seat
  modifier path (P134 `own_set`/`with_own`/`ride_set`), and `seat_own` is tested. A peer on an older
  build cannot parse `stick_dead_zone` and refuses the room's modifiers with a parse error instead of
  riding different numbers. I did not test this against an actual older build.
- **No tape was recorded and replayed with the knob on.** The tape's `modifiers` line is `Mods::text`,
  whose round trip is tested, and the bike file is built through `Mods::bike` the same way at playback.
- **The car and Kiowa reads in the shell are only compile-checked with the new range.** No test drives
  them with the knob on.

## Next

Nothing. A person should try 0.10 and 0.05 on a pad and say whether the steps, or a slider, are right.
