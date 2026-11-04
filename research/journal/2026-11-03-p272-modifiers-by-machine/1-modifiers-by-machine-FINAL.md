# 1 — modifiers by machine (FINAL)

Task P272. 2026-11-03.

## What I did

- `game_sim::modifiers`
  - `Category` (Bike, Biplane, Kiowa, Cars, World) and `Knob::category`. The Wrecks rows sit under Bike.
  - Eight new knobs:
    - Kiowa: `kiowa_speed`, `kiowa_climb`, `kiowa_turn`, `kiowa_ceiling`.
    - Cars: `car_speed`, `car_accel`, `car_grip`, `car_suspension`.
  - `Mods::kiowa` and `Mods::car` apply them to a `kiowa::Phy` and a `car::Car`. At 100% neither one
    touches a field. Each is a scale I chose (none of them is the original's):
    - Kiowa speed: the controller's speed commands, feed-forward speeds and command rate ×k; speed gain
      ÷k; the body's zero-lift drag ÷k².
    - Kiowa climb: climb and descent rates ×k; above 100% the rotor's max collective ×√k so it can reach
      them.
    - Kiowa turn: max yaw and roll rate commands, the let-go roll rate and the rotor's yaw/roll moment
      limits ×k.
    - Kiowa ceiling: `MaxAltitude` ×k.
    - Car speed: torque (and pump torque) ×k, final drive ÷k (first gear recomputed:
      `car::Engine::first_gear`), aero area ÷k².
    - Car acceleration: torque ×k, the surfaces' wheel friction ×k, aero area ×k (top speed is kept).
    - Car grip: every surface's friction peak ×k.
    - Car suspension: spring ×k, bounce and rebound damping ×√k.
- The shell (`vehicle.rs`, `main.rs`)
  - `Garage::modify` resets each car and the helicopter from the lot's stock copy, then applies the seat's
    knobs. It runs where the garage is built and in `remodify` when the pause closes.
  - A flown Kiowa retunes at once. A driven car takes speed/acceleration (copied into its drivetrain) at
    its next take; grip and suspension apply at once, since the step reads them every tick.
- The menu (`menu_pages.rs`, `menu.rs`)
  - The Machine row (`Key::ModsMachine`) is row 1. Each choice's tip counts its changed knobs.
  - Presets and Reset still cover every machine.
  - Online, the Kiowa's and the cars' knobs are held: online rides use the disc's machines (P289).

## What the original does

Nothing like this: the original has no modifiers. Every knob is the port's own, and at 100% it is the
original.

## How it was proved

- `tests/menu.rs` `the_modifiers_page_shows_one_machines_knobs_at_a_time`:
  - Each machine's page lists exactly its knobs, and every knob falls under one machine.
  - The wrecks show only on Bike.
  - A Kiowa knob and a car knob are set from their pages, and a preset saved on Cars holds both.
  - Reset keeps the chosen machine.
  - A paused Kiowa opens on Kiowa.
- `tests/machine_modifiers.rs`, on the disc's machines through the shell's `CarRide::step` and
  `KiowaRide::step` on a flat floor:
  - `every_knob_at_its_default_leaves_each_machines_step_bit_exact`: every knob written at 100%, and every
    other machine's knobs at 150%, leave each car's and the Kiowa's file equal and 180 ticks of body
    state bit-equal.
  - `a_car_knob_changes_the_cars_ride`, per car (buggy / monster / baja):

    | Knob | Measure | buggy | monster | baja |
    | --- | --- | --- | --- | --- |
    | Speed 150% | top speed, ft/s | 146 → 219 | 201 → 307 | 216 → 342 |
    | Acceleration 50/100/150% | speed at 3 s, ft/s | 21 / 60 / 79 | 50 / 79 / 106 | 40 / 79 / 91 |
    | Grip 40% | path turned in 1 s of full lock, rad | 1.17 → 0.71 | 0.69 → 0.37 | 0.79 → 0.47 |
    | Suspension 50/100/200% | rest height, ft | −0.04 / 0.38 / 0.59 | 1.16 / 2.08 / 2.53 | 0.40 / 0.87 / 1.11 |

  - `a_kiowa_knob_changes_the_kiowas_ride`:
    - Climb 50/100/150%: 91 / 178 / 276 ft after 4 s of climb.
    - Ceiling 50%: the most height reached went from 590 to 315 ft.
    - Speed 150%: 130 → 193 ft/s.
    - Turn 150%: pedal yaw rate 1.49 → 2.25.
- `tests/modifiers.rs`: the new knobs read back and reach neither the bike nor the biplane.
- `./kit check -- -p openmx --no-fail-fast`: 3031 passed.
- `./kit check -- -p game-sim --no-fail-fast`: 1191 passed, and 1 failed. The failure is
  `kiowa_capture::the_rotors_probe_is_the_images`, which unwraps `context/states/c14-kiowa.ee.bin`, and
  that file isn't in this slot. It's unrelated.

## Not verified / not 1:1

- **"A mod machine's category appears only while it is on" is not tested.** No mod machine can be ridden
  yet (the ATV is MD37d, the MX vs. ATV cars MD39c), so `mods_categories()` returns the base five. The
  menu test checks that it is exactly those five. P287 and P288 add the categories and that test.
- The scales are my design, tuned only so that each knob moves its measure clearly. Nobody has played
  them on screen. Examples of what was not measured:
  - The Kiowa's speed knob may also change how it pitches into a run.
  - Car acceleration's wheel-friction share may change braking.
  - Car speed at 50% may stall a heavy car on a hill.
- A car being driven takes the speed and acceleration knobs only at its next take, not at once.
- Online, the car and Kiowa knobs are held (P289). World gravity and air drag don't reach the cars or the
  Kiowa (P290).
- The Machine row and its tips were not looked at on screen (no shot).

## Next

None: P287–P290 are the follow-ups.
