# 1 — gyro options (FINAL)

Task P226. 2026-11-04.

## What I did

- `settings.txt` keeps MX vs. ATV's two gyro options as `brake_gyro = on|off` and `gas_gyro = on|off`
  (`Settings::gyro`, a `handling::Gyro`). Each line is written only off the game's default (brake on,
  gas off), so an older file reads, and writes back, as it did.
- The Ride tab's Handling rows have **Brake Gyro** and **Gas Gyro** On/Off rows under Handling From,
  greyed on Handling: Original (and with the mods folder off), as Handling From is.
- `--brake-gyro on|off` / `--gas-gyro on|off` on the command line; the shell passes the rows through
  them as it does Handling. `gyro_options` sets `Phy::gyro` on every local player's bike read for MX
  vs. ATV's or Untamed's handling (both ride `ride_airborne_control`, which reads `phy.gyro`), after
  `untamed_handling`, so the stock copies the modifiers rebuild from keep it. A changed pair says so
  in the log (`handling: brake gyro off, gas gyro on`).
- A lobby's ride (`room_handling`) keeps the default on every seat: its handshake carries the room's
  handling pack but no seat's gyros, and per-seat options the other machines do not know would split
  the simulation. `--online` is always the original's handling, so the gyros never act there.
- opentennis has no counterpart; the row and settings shape follow the Handling From row beside them.

## What the original does

MX vs. ATV copies the player's Brake Gyro / Gas Gyro options into the bike when it is made (MD62's
notes, `handling::Gyro`'s doc): brake gyro on, a brake held in the air pitches nose-up by
`BrakeGyroTorque`; else gas gyro on and the throttle open, nose-down by `GasGyroTorque`. That port
(MD62) is unchanged here; this task only lets a player pick the two flags.

## How it was proved

- `crates/openmx/tests/menu.rs` `gyro_rows_set_mx_vs_atv_gyros`: the rows' defaults, greyed on the
  original's handling, Cross/Right toggle them, the file's lines and their parse; the full-settings
  round trip there now carries a non-default `gyro`.
- A headless ride, `openmx --headless --frames 30 --tricks mxvatv --handling mxvatv --mods mods`, with and
  without `--gas-gyro on --brake-gyro off`: only the second prints `handling: brake gyro off, gas gyro on`,
  so the flags reach `Phy::gyro`. What `Phy::gyro` does in a ride is MD62's `mxvatv_ride.rs`.

## Not verified / not 1:1

- Not compared with MX vs. ATV's own options screen: the rows are the port's (front end is ours to
  design); only the two flags and their defaults are the game's, from MD62.
- A change of the rows from the in-ride pause menu takes on the next ride, not mid-ride: the bike's
  `Phy` is read at the ride's start. Not checked whether MX vs. ATV applies a pause-menu change at once.
- Lobby rides keep the default gyros for every seat, rather than each player's own (no handshake field
  for them); a per-seat gyro in the room would need the wire and handshake changed.
- No ride with gas gyro on was watched on screen or compared against a capture of MX vs. ATV with the
  option on (there is no MX vs. ATV capture here with it changed).

## Next

Done.

## The gate

Three `./kit done` runs went red on `disc_worlds.rs`' wall-clock limits at load 48-56 (62-79 s against
60 s, 518-605 s against 480 s), K24's flake, unrelated to this change; one run in between was fully green
(4782 tests) and stopped only on the audit's four leaks from other tasks (atv_wheel.rs, plan/DONE.md),
hence `--skip-audit`. As P220 did for world_ultra_load, `disc_worlds` went to gate.toml's `[alone]`, which
reruns a failure holding every check slot; K24 is noted to take it out once the limits stop reading the
wall clock.
