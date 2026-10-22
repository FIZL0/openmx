# 1 — put back keeps raised limit (FINAL)

Task P101. 2026-10-22.

## What I did

- `Ride` carries `riding_max_speed_sq`: the squared limit the bike's put-back gives the chassis again.
  `Ride::new` starts it at the original's 135² (the same constant the chassis limit is built with), and
  it is part of the rollback snapshot (`snapshot.rs`'s `wire!` for `Ride`) because P98's pause take
  can change it mid-ride.
- `off_map::unwreck_ride(ride)` sets the chassis' limit to that field. The rider's four put-backs (the
  chord, the machine's own, the wreck page's, and the marker teleport-to-spot) call it in place of
  `off_map::unwreck(&mut ride.chassis)`. `off_map::unwreck` itself is still there, unchanged, and still
  what the S59 oracle and capture tests in `game-sim/tests/off_the_map.rs` check.
- `Mods::ride` raises the field the same way it raises the chassis limit (only `BikeSpeed` > 100%).
  `Mods::rebike` (P98) still moves the chassis limit only while it is the riding one, as before, and now
  always moves `riding_max_speed_sq` to the new set's. That way a set changed while the bike is thrown
  takes effect at the put-back.
- Test: `crates/openmx/tests/modifiers.rs`
  `double_bike_speed_passes_135_after_a_wreck_and_its_put_back`. At bike_speed=200 a wreck goes
  through the real put-back. After it the limit is bit-identical to the raised 135²·4, and full gas
  then reaches 150.38 ft/s. The test also checks that a pause change on a thrown bike leaves the
  chassis at 350² and moves the put-back's limit to the new set's (and back to 135² on the original).
  With `unwreck_ride` temporarily put back to the stock constant, the test fails on "the put-back took
  the limit back to stock".

- Pins moved for the 4 new bytes, the way S71 moved them for `drag_halved`: `tests/hash.rs`'s `Sim`
  size (11160 → 11168), `tests/ride.rs`'s `Ride` size (2416 → 2424), and `tests/platforms.rs`'s riders'
  resync wire (13303 → 13319 bytes, new digest). The per-tick hashes and their digest did not move.
- Outside the task, needed for the gate: `openmx/tests/menu.rs`'s
  `the_pauses_modifiers_take_when_it_closes_unless_held` (P98) failed at HEAD before this change, which
  I checked in a scratch worktree. It counted the modifier screen's rows by `Knob::ALL`, but since P95
  the screen lists `Knob::MENU`, which leaves out PlaneSpeed. So it stepped onto Air drag instead of
  Gravity, and its Reset To Original row (`ALL.len() + 4`) could never be reached, so the loop hung
  forever. Two words changed, `ALL` → `MENU`.

Nothing came from opentennis; it has no modifiers of this kind.

## What the original does

The bike's unwreck sets the integrator's limit to 135² whatever the wreck was (S59, journal
2026-10-21-s59). The original has no modifiers. With every modifier at 100% the field stays 135², so
the put-back writes exactly what it wrote before.

## How it was proved

Unit-level, through the real rider frame: the new test above. It fails without the fix and passes with
it. The original's own behaviour at 100% is still covered by the S59 tests (`off_the_map.rs`,
`wreck_page.rs`), which go through `unwreck_ride` and expect 135².

## Not verified / not 1:1

- Modified rides have nothing in the original to match against, so this is a design choice rather than
  a port: the put-back gives a faster bike its own raised limit.
- The per-tick state hash (`hash::State` and the dump writer in `ride.rs`) does not include the new
  field, just as it leaves out `drag_halved`. It can only differ from 135² on a modified ride, and there
  it follows from the modifiers the tape carries. It is in the rollback snapshot.

## Next

Nothing.
