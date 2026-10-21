# 1 — game modifiers

Task P55. 2026-10-21.

## What I did

- `crates/game-sim/src/modifiers.rs` (new): twelve modifiers (`Knob`), each a percentage (100 is the
  original; 10..500 in steps of 10, gravity and air drag down to 0), kept per player in `Mods`. Nine
  are per player, three are the world's (gravity, air drag, time scale: one value, row 0). `Mods::bike`
  scales the loaded `Phy`, `Mods::ride` the built ride's speed limit, `Mods::plane` the loaded
  `biplane.phy`, `Mods::time_scale` gives the engine's time-scale factor. A knob at 100 returns
  `None` from `factor` and its block is skipped, so the original set touches **nothing** — no
  multiply by 1.0 anywhere. The text form (`gravity=50 bike_speed.2=200`, only knobs off 100) is what
  the settings file, a tape and a lobby room carry; `Mods::parse` rejects unknown words and ranges.
- No new branch in any step. Everything is applied where the shell loads a ride (`main.rs` `ride`):
  1P's `.phy` and each other player's before their riders are built, the speed limit after, the
  biplane's file before `Flight::new`, `engine.time_scale` after `pace::after_load`.
- Which set a ride uses (`ride_modifiers` in main.rs): a lobby ride's is the host's (the room's new
  `modifiers` string, serde-defaulted so an older peer's room still reads); a `--play` tape's is its
  own; otherwise `--modifiers TEXT` (new flag) or, under the front end, the Settings screen's.
- Tapes: `PadTape.modifiers`, written as a `modifiers …` line after `tape` only when off the
  original, so a stock tape's text is byte-identical to before. `--record` stamps the ride's set.
  The tape's per-frame dt is already the scaled one (it is recorded after the engine), so a
  time-scaled ride replays on its own dt.
- Online: `Online.time_scale` and `Rides.time_scale` scale each tick's scheduled dt with
  `modifiers::scaled_dt` (the same `dt + s·dt`, floor 0.001 the engine applies); the lobby's sims get
  the host's set on every seat's `.phy`.
- Menu (`menu.rs`): Settings' tenth row "Game Modifiers  Original/Modified" opens a new
  `Screen::Modifiers`: a Player row (Every Player, 1P..8P), the twelve modifiers (left/right ±10%,
  Cross back to 100; "Mixed" when players differ under Every Player), Load Preset (left/right picks,
  Cross loads), Save As Preset (the on-screen keyboard, `Typing::Preset`), Delete Preset, Reset To
  Original. The ticker says what the row scales and that it takes on the next ride. Lists longer than
  nine rows scroll (`Painter::scrolled`), which Settings needs now too. Presets and the current set
  live in `settings.txt` (`modifiers = …`, `preset = Name | …`); with the original set and no presets
  nothing is written, so a pre-P55 file round-trips as it was.
- Modified marker: the overlay draws "MODIFIED" under the score (`Gauges.modified`, per player:
  `Mods::seat_is_original`); the pause title reads "Paused  Modified".

### The modifiers, the fields they scale, and the originals

Originals are `race125p.phy` (the freeride bike) and `biplane.phy` off the disc, read 2026-10-21.

| Knob (word) | Scope | Fields | Original values |
|---|---|---|---|
| Bike top speed (`bike_speed`) | player | `[Engine]` HP curve torque ×k, ClutchStatic/KineticFriction ×k, EngineBraking ×k, FinalGearRatio ÷k (gear ratio recomputed), `[Aerodynamics] Drag` ÷k², ReverseSpeed ×k, MaxClutchTorqueScaleSpeed ×k; the ride's speed limit (`max_speed_sq`, 135 ft/s) ×k² when k > 1 | HP 14…45.5 over RPM 1000…9500; clutch 70/60; EngineBraking 5e-8; FinalGearRatio 10.538; Drag 0.025; ReverseSpeed −10; MaxClutchTorqueScaleSpeed 50; limit 135 ft/s |
| Engine power (`bike_power`) | player | HP curve torque, peak torque, ClutchStatic/KineticFriction ×k | as above |
| Tyre grip (`grip`) | player | both tires, all 16 surfaces: SideForce, SlippingSideForce, MaxDriveForce, MaxSideForce, MaxSlippingSideForce, SkidDrive/Side/SlippingSideForce ×k | zone 1 front: 0.165, 0.165, 2, 5, 5, 1.9, 4.85, 4.85 |
| Suspension (`suspension`) | player | `[Front/Rear Tire] Spring`, `SpringWhenStopped` ×k (and the spring rates derived from them) | front 1.5 / 1.1, rear 2.5 / 1.1 |
| Air control (`air_control`) | player | `[Airborne Control] AirborneControlMoment` (xyz), `AirborneControlForce` ×k | (−400, 500, −150), 150 |
| Flip spin (`spin_rate`) | player | `[Backflip] Min/MaxBackflipTorque`, `BackflipTorqueRateIncrease/Decrease` ×k | 800 / 1400, 500 / 2000 |
| Jump strength (`jump`) | player | `[Preload] MaxPreload` ×k | 600 |
| Crash tolerance (`crash_tolerance`) | player | `[Wrecks] Front/Back/Side/RacingDown/FreestyleDown/NoHander WreckSpeedLimit` ×k | 12.5, 15, 13, 40, 45, 20 |
| Biplane speed (`plane_speed`) | player (only 1P has a plane) | `[Engine] MaxPower` ×k, `MinPropEfficiencySpeed` ×k, `[Aerodynamics] CD0` ÷k² | 250, 40, 0.06 |
| Gravity (`gravity`) | world | bike `[Mass Properties] Gravity` and the gravity force built from it ×k; biplane `GravityScale` ×k | 1 (force 0, −400, 0 = −Gravity×Weight), 1.0 |
| Air drag (`air_drag`) | world | bike `[Aerodynamics] Drag`, biplane `CD0` ×k | 0.025, 0.06 |
| Time scale (`time_scale`) | world | the engine's time-scale object: `dt += (k−1)·dt` (tick.rs `time_scale`), floor 0.001 | off (`None`) |

(The words are `Knob::word`'s, as `--modifiers` and `settings.txt` take them; `word.N` sets player N
alone.)

## What the original does

Nothing of this is the original's: it has no modifiers. The one hook it has is the engine's own
time-scale object, `tick.rs`'s `time_scale`, which this drives; everything else is the files' own
fields scaled before the ride is built.

## How it was proved

`crates/openmx/tests/modifiers.rs` (6 tests, stand-in bike `stock::BIKE` with tests/ride.rs's patches
and wreck limits, flat ground, plus the disc's `biplane.phy` where a disc is configured — it was):

- `every_modifier_at_its_default_leaves_the_ride_bit_for_bit`: a set moved and moved back equals the
  original; for all 8 seats the loaded `Phy` is unchanged (compared by `Debug`, which prints every
  float at round-trip precision); the ride's speed limit's bits are the same; 1200 ticks of a ride on
  the original set match an unmodified ride tick for tick on transform, velocity, speed, ground flag
  (as bits); the biplane file is `==`; `scaled_dt(dt, None)` is dt's bits.
- `a_tape_on_the_original_modifiers_is_written_as_before`: no `modifiers` line, round-trips.
- `half_gravity_halves_the_falls_acceleration`: a bike 500 ft up, two ticks: Δv_y over the second tick
  and the first are ×0.5 (to 1%) of the stock bike's.
- `double_bike_speed_raises_the_top_speed`: 30 s flat out on flat ground: 88.33 ft/s stock, 150.38 at
  ×2 (×1.70); the integrator's limit is above it.
- `a_tape_made_with_modifiers_replays_with_them`: a ride on `gravity=70 time_scale=150
  bike_speed=130 grip.1=80`, taped, written, read back: the modifiers come back equal, the text
  round-trips, a replay on the tape's modifiers and dt matches every tick; on the original set it
  does not.
- `every_modifier_off_its_default_reaches_the_ride`: each knob at its least and at 150 changes the
  bike file (or the time scale, or the biplane file) and reads back from its text; per-player knobs
  reach only their player; world knobs are everyone's.

`crates/openmx/tests/menu.rs`: `the_game_modifiers_screen_sets_saves_and_resets` (walk to the screen,
gravity −20, 2P's bike speed +10, save a preset on the keyboard, reset, load the preset, file
round-trip, a bad line is the original); the settings round-trip test now carries a set and a
preset; the pill/logo layout test covers the new screen (10 list screens × 12 looks × 4 shapes).

## Not verified / not 1:1

- **Not ridden in the window.** No session of `openmx` with modifiers was run on the disc's tracks;
  the tests ride the stand-in bike on flat ground. How each modifier *feels* (grip, suspension, air
  control, spin, jump, crash tolerance) is unproven beyond "the field changed".
- **Bike speed ×2 gives ×1.70, not ×2**, on the stand-in bike: torque/gearing and drag scale for ×2,
  but the tire forces (MaxDriveForce, rolling drag) and engine inertia do not. The test only asks for
  "raises"; whether race125p reaches ×2 was not measured.
- **Biplane speed is not proved to raise the plane's top speed**: only that the file's fields move.
  No flight test was written (the brief asked for none); plane drag is assumed to go as CD0·v².
- **Gravity misses the wreck**: the wreck bike (`wreckbike.phy`) and the ragdoll's own gravity
  (`WreckGravity`, ragdoll kit) are not scaled, so a crash falls at the original's rate: the wreck
  part's fall is `gravity_scale × phy::G` in ride.rs, and its part is not touched. The fall test at
  ×0.5 says the airborne bike itself reads the scaled force.
- **Online, per-player knobs are 1P's for every seat.** A lobby seat's sim is a copy of this
  machine's first rider with its `.phy` swapped, so each seat gets the host's 1P values; per-seat
  online values would need each seat's rider rebuilt. `--online` (no lobby) does not exchange
  modifiers at all: each machine uses its own `--modifiers`, and different sets desync. Neither was
  run over a network; the lobby path is untested beyond compiling (no netplay test covers it).
- **Online time scale** uses `scaled_dt`, a copy of the engine's scaling step, not the engine itself;
  its equivalence to `DeltaTime::update`'s averaging-then-scale was read, not tested.
- **Changing modifiers in the pause takes effect on Restart or the next ride**, not mid-ride (by
  design: they apply at load). The ticker says so.
- **The HUD marker's place** (under the score label) is the port's own; it was not looked at in a
  window or screenshot. The pause shows it in the title only.
- **The original's "Debug" overlay (`--hud`)** does not show the marker.
- `Phy` equality is by `Debug` text: −0.0 vs 0.0 print differently (good), but NaN payloads would not.

## Next

Ride a modified session in the window on the disc's tracks and look at the HUD mark; per-seat
modifiers online (rebuild each lobby seat's rider on its own set); scale the wreck's gravity with
the world's.
