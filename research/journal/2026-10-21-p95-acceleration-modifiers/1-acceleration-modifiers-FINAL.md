# 1 — acceleration modifiers (FINAL)

Task P95. 2026-10-21.

## What I did

Three new per-seat knobs in `crates/game-sim/src/modifiers.rs`, beside P55's twelve:

| Knob (word) | Fields, ×k unless said | Why that holds the other end |
|---|---|---|
| Bike acceleration (`bike_accel`) | `[Engine]` HP curve (torque), peak torque, ClutchStatic/KineticFriction; `[Aerodynamics] Drag`; every tire surface's `WheelDrag`, `WheelFriction` | drive and every flat-ground drag scale alike, so the surplus is k× and the drag-limited top stays. The stock bike tops out on drag at ~88 ft/s, far under its rev limit in top gear (11000 rpm through 3.6), so the gearing can't hold the top and isn't touched |
| Biplane acceleration (`plane_accel`) | `MaxPower`; `CD0`, `CD1`, `CDde` | thrust − drag is k× the file's at every speed, so the top (thrust = drag) doesn't move |
| Biplane top speed (`plane_top_speed`) | `CD0` ÷ k³ | power = drag·speed and parasite drag ∝ v², so v_top ∝ (P/CD0)^⅓ |

P55's `plane_speed` (MaxPower and MinPropEfficiencySpeed ×k, CD0 ÷ k²) stays in `Knob::ALL` with the same
sums, so a preset or a tape carrying it loads and rides bit for bit as before. It is not on the menu: the new
`Knob::MENU` (all but `PlaneSpeed`) is the list the modifiers screen draws and steps over
(`crates/openmx/src/menu.rs`, three uses plus `MODIFIERS_PRESET`). `bike_speed` and `bike_power` are unchanged
and still on the menu. `KNOBS` is 15. Each new knob is skipped outright at 100%, like the rest.

Nothing to take from opentennis: it has no game modifiers.

## What the original does

Nothing: modifiers are an addition. At 100% every new knob is skipped, so the original's parameters load
untouched.

## How it was proved

`crates/game-sim/tests/acceleration_modifiers.rs`:

- `bike_acceleration_cuts_the_zero_to_sixty_and_holds_the_top_speed` — stand-in bike (`tests/ride.rs`'s
  ridden patches) flat out from rest for 60 s on flat ground. Stock: 0–60 ft/s in 6.43 s, top 88.36 ft/s.
  200%: 3.48 s, 89.29. 150%: 4.90 s, 89.60. 70%: 9.33 s, 87.83. Engine power 200% for contrast: top 123.1.
  The top is asserted within 2%.
- `biplane_acceleration_and_top_speed_move_apart` — disc's `biplane.phy`, built by `Flight::new`, its engine
  (`Engine::step`) and aero (`Aero::forces`) flown along a level line with the angle of attack bisected to hold
  the weight each step (so induced drag is in). Stock top 156.73 ft/s. Acceleration 200%/50%: 0–80 in
  0.87/3.43 s, top 156.73 both. Top speed 150%/70%: 235.14 (×1.500) / 109.68 (×0.700), 0–80 in 1.67/1.90 s.
  Both together: 0.87 s, 235.14.
- `at_one_hundred_percent_the_new_knobs_touch_nothing` — moved to 250 and back, each new knob leaves the
  bike's `Phy` and the plane's file Debug-identical (bit for bit); off 100% the bike knob reaches only the bike,
  the plane knobs only the plane; text round-trips.
- `the_old_words_still_load_and_ride_as_they_did` — `bike_speed`, `bike_power`, `plane_speed` (and
  `plane_speed.1`) parse, and `plane_speed=150` applies exactly P55's sums.
- `crates/openmx/tests/modifiers.rs` and `menu.rs` updated: the plane knobs are plane-only, the screen's rows
  are `Knob::MENU`'s.

## Not verified / not 1:1

- **The bike's top speed holds to ~1.4%, not exactly.** With all three drags and the torque scaled, the top
  still settled 1.0% (200%) and 1.4% (150%) high and 0.6% low at 70%. I did not find the remaining term;
  likely tire slip/clutch under a different torque. The test allows 2%.
- **Proved on the stand-in bike, not a disc bike.** A disc `.phy` that tops out on its rev limiter rather than
  drag would get its top from the limiter anyway; one whose drags have other terms (e.g. a `WheelDrag0`-like
  key the loader reads) was not checked. Reason: the ride tests here use `stock::BIKE`.
- **Side effects of scaling drags.** Bike acceleration also scales the air drag in the air (a jump bleeds k×
  the speed) and wheel friction on a spinning airborne wheel; biplane acceleration scales drag in a glide, so a
  shut-throttle plane slows k× harder. Untested in flight; accepted as the price of keeping the top speed.
- **Biplane top speed is ×k only for the parasite drag.** Induced drag (`CD1`) is not scaled; on the disc's
  plane the level-line rig still gave ×1.500 and ×0.700, but a heavier plane would land less exactly.
- **The biplane proof is a level-line rig, not the full flight.** The full `Flight::frame` climbs under its
  autopilot and trades speed for height, so it shows no clean top speed; the rig uses the plane's own engine
  and aero code but no body, wheels or autopilot.
- **A preset holding the old `plane_speed` is invisible on the menu.** It rides as it did and Reset To
  Original clears it, but there is no row to see or step it.

## Next

Nothing: the task is done.
