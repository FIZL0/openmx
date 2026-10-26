# 1 — plane max g roll (FINAL)

Task P160. 2026-10-26.

## What I did

- Dropped P137's aileron easing from the Biplane max G hold. `Flight::hold_g` now only holds the
  elevator command (and the pitch integral). `GLimit` has lost its `aileron` share, and `Aero::g_limit`
  no longer works out the corner speed (`full`/`reach`/`pull` went with it). The roll floor field
  `max_g_roll` is gone everywhere it lived: `phy::Autopilot`, `Flight` (new / retune / put_back / walk),
  the snapshot wire, and `Mods::plane`'s "how many times faster" sum. The wire is one F shorter; a
  flight with no limit set never hashed `max_g_roll`, so off is unchanged.
- Rewrote the knob's notes: the modifiers.rs module doc, `what_it_changes` ("ailerons and rudder
  untouched"), the blurb ("its roll and spin are left as they are"), and the biplane.rs section
  comment.
- tests/plane_max_g.rs: `a_fast_roll_is_the_stock_planes` is now `a_fast_roll_is_the_limit_off_roll`.
  For every limit 3..=15 g, on the stock plane at its top speed and the 200% / 300% top-speed planes at
  theirs, File and New schemes, a full aileron's roll rate is the limit-off one within 0.5%. It still
  asserts that the 300% plane rolls more than 3× faster than the stock one. The marker/retune and
  off-is-original tests lost their `max_g_roll` asserts.
- The KNIFE hand (the knife-edge pull's roll-and-hold controller) now scales its gains by
  `(157 / airspeed)^2`. Without the easing, the stock-tuned hand on a 500% plane bangs the aileron
  full each way at about 4 Hz. A per-frame dump showed the plane then spikes to ±18 g with the stick
  centred and the limit **off**, so the spikes come from the hand and not from the hold. Scaled, every
  P137/P148 knife-edge pull holds under its limit again.
- Nothing taken from opentennis: it has no flight model.

## What the original does

Nothing. The max G knob is an addition and not in the original. With it Off the flight never enters
the hold, and `off_flies_the_original` / `the_stock_plane_is_never_held_at_9g` still pass bit for bit.

## How it was proved

`./kit check -- -p game-sim --test plane_max_g`: 7 tests pass. To check the new roll test, I put in a
mutant that cut the aileron by 10% above 200 ft/s inside `hold_g`. The roll test went red on it
(456.5 against 508.0 deg/s at 470 ft/s), then I took the mutant out.

## Not verified / not 1:1

- Roll with a limit set is within 0.5% of limit-off, not bit-equal. In a full roll at speed, the
  elevator hold still trims the pitch (the pitch loop / stick asks 0, and the hold's lead reacts to the
  pitch rate the roll couples in). That pitch couples back into roll and yaw: up to 0.4% in roll rate
  at 470 ft/s, and frame-by-frame body rates differ by up to ~0.18 rad/s. Making it bit-equal would
  mean not holding the pull during a roll, and the task keeps the pull held.
- Yaw is not asserted separately. The rudder was never touched by the hold, and the yaw difference is
  the same coupling as above. The per-frame surface check I tried proved nothing (`air` is measured
  again after the hold), so I dropped it.
- The KNIFE hand's speed scaling is a test-side change, picked from a moment-goes-with-v² argument and
  not tuned against a human. With the easing gone, a real player's wobbling full stick on a 500% plane
  can still pull past the limit (±18 g off, as above). The hold limits the elevator's pull, not
  inertial or roll-induced load.
- No in-game feel check (no window run): the change is to a modifier only, and its tests are synthetic
  flights.

## Next

none
