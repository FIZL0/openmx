# 1 — mxvatv car modifiers (FINAL)

Task P288. 2026-11-14.

## What I did

Copied P287's quad category (the closest thing to this), and P272's stock car knobs. Nothing came from
opentennis: it has no game modifiers.

- **game-sim `modifiers.rs`**:
  - Four new knobs, `Knob::MxvatvCarSpeed`/`Accel`/`Grip`/`Suspension`. Their words are
    `mxvatv_car_speed`, `_accel`, `_grip` and `_suspension`. `KNOBS` goes from 32 to 36.
  - A new `Category::MxvatvCars`, labelled "MX vs. ATV Cars". It is an addition, so it is not in
    `Category::ALL`.
  - `Mods::mxvatv_car(seat, &mut car::Car)`. SLUS-21104 loads its cars with MX Unleashed's car loader
    (MD39b's `Car::read_mxvatv` wraps `Car::read`), so the cars' knobs are the same clauses on the same
    fields. The body of `Mods::car` moved into a private `car_knobs(seat, [speed, accel, grip,
    suspension], car)`, which both call. `Mods::car` behaves as before, except for the next point.
  - **The speed knob also scales `EngineBraking` × k, and this changes the stock cars' Car top speed
    too.**
    - Without it, 50% top speed held the MX vs. ATV buggy to 12 ft/s instead of half of 145. The
      engine's braking (EngineBraking × revs²) outweighs half the torque.
    - With it, 50% gives 72.6 ft/s. This matches the clause the bike's and the quad's speed knobs
      already have ("the force at the wheel the file's at any revs").
    - At 100% nothing changes. tests/machine_modifiers.rs still passes on the stock cars.
- **openmx `vehicle.rs`**:
  - `MxvatvCarRide` keeps `mods` and `seat`, which `Garage::modify` sets, as `AtvRide` does.
  - `made()` applies them to a clone of the `Load`'s `file.car` before `Load::ride` builds the keys. It
    does this only when one of the cars' knobs is off 100%; otherwise it builds from the shared load as
    before.
  - A car already out keeps its numbers until its next take, like the quad and the stock cars' engines.
- **openmx `menu_pages.rs`**:
  - `mods_categories()` puts MX vs. ATV Cars before World (after ATV when both are on) while
    `mxvatv_car_on()`: `settings.mxvatv_car` is set, or the menu is under the pause and `menu.mxvatv_car[]`
    says a seat has one.
  - `knob_held` holds its knobs online ("Online rides have no MX vs. ATV car yet."): the cars are
    offline only (MD39c).
  - Under the pause in the car, the page opens on the category. MD39c had it open on the bike's.

## What the original does

Nothing: Game Modifiers are an addition. The fields scaled are SLUS-21104's car keys, as MD39b reads
them.

## How it was proved

- `crates/openmx/tests/mxvatv_car_modifiers.rs` (new) uses the buggy off openmx-mods' `mxvatv_cars` pack,
  copied into this slot's ignored `mods/machines/` from s5. It is stepped through `MxvatvCarRide::take`
  and `step` on a flat floor.
  - With every knob at 100%, with every other machine's and the world's knobs moved, and with 2P's speed
    at 200%: the file is unchanged, and 120 ticks of gas and then a full turn are bit-exact
    (positions and velocities). The stock cars' `Mods::car` with these four at 200% leaves the file
    alone.
  - Each knob moves its fields, checked exactly. The made car's keys differ from stock.
  - Each knob changes the ride (buggy):
    - top speed 72.6 / 145.2 / 215.9 ft/s at 50/100/150%;
    - speed 2 s off the line 21.4 / 53.8 / 62.6 ft/s at 50/100/150% acceleration;
    - sideways pull in a full-lock turn 115 → 36 ft/s² at 40% grip;
    - rest height −0.62 / 0.06 / 0.40 at 50/100/200% suspension.
- `tests/menu.rs` `the_mxvatv_cars_category_shows_while_the_car_addition_is_on`:
  - off by default;
  - with a car set, the category sits before World (after ATV);
  - the page lists the four knobs, and a step sets the MX vs. ATV knob and not the stock car's;
  - with the car off, the category goes and the value stays;
  - under the pause with a car it opens on the category, and for a guest the knobs are held.
- `tests/modifiers.rs` (every knob reaches its own machine only), `machine_modifiers.rs`,
  `atv_modifiers.rs`, `remodify.rs`, and game-sim's `acceleration_modifiers.rs` pass.

## Not verified / not 1:1

- **Only the buggy is ridden in the tests.** The other four cars (Monster Truck, Baja, Golf Cart,
  Sandrail) take the same code on the same fields, but their ride under the knobs was not measured.
- **Without the pack, the riding tests skip.** They need openmx-mods' `mxvatv_cars` in `mods/machines/`,
  as MD39c's tests do. The gate's slots may not have it, and then only the category and menu tests run
  there.
- **The ride tests let the car settle for 30 ticks before the gas.** Taken a foot up on full gas at
  150% acceleration, the buggy lands with its rear wheels at about +7 rad/s. They flip to −27 rad/s and
  hold there for about a second. I did not check this against SLUS-21104; it is filed as R36. It is the
  ported step's landing, not the knob's doing.
- **Acceleration over 100% only helps after the first second.** The buggy is grip-bound off the line,
  and the cars' acceleration clause, unlike the bike's and the quad's, does not raise the driven tires'
  grip with the torque. It is kept as the stock cars' clause, as the task asked ("like the stock
  cars'").
- **Not tried in the shell by hand.** It was not driven from the pause's Game Modifiers in a windowed
  ride. The shell path is `Garage::modify` (called on every pause close and seat set-up), and then
  `MxvatvCarRide::made`, which the test calls directly.

## Next

Nothing for P288.
